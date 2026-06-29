import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MarkdownMessage } from "./markdown-message";

vi.mock("mermaid", () => ({
  default: {
    initialize: vi.fn(),
    parse: vi.fn().mockResolvedValue(false),
    render: vi.fn().mockRejectedValue(new Error("no dom in jsdom")),
  },
}));

describe("MarkdownMessage", () => {
  it("renders headings, emphasis, and lists", () => {
    render(<MarkdownMessage content={"# Title\n\nSome **bold** text\n\n- one\n- two"} />);

    expect(screen.getByRole("heading", { level: 1, name: "Title" })).toBeInTheDocument();
    expect(screen.getByText("bold").tagName).toBe("STRONG");
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
  });

  it("opens links in a new tab with a safe rel", () => {
    render(<MarkdownMessage content={"[docs](https://example.com)"} />);

    const link = screen.getByRole("link", { name: "docs" });
    expect(link).toHaveAttribute("href", "https://example.com");
    expect(link).toHaveAttribute("target", "_blank");
    expect(link.getAttribute("rel")).toContain("noopener");
  });

  it("highlights fenced code blocks with the language class", () => {
    const { container } = render(<MarkdownMessage content={"```ts\nconst answer = 42;\n```"} />);

    const code = container.querySelector("code.hljs.language-ts");
    expect(code).not.toBeNull();
    expect(code?.textContent).toContain("const");
  });

  it("renders inline code without the block treatment", () => {
    const { container } = render(<MarkdownMessage content={"use `npm test` here"} />);

    const code = container.querySelector("code");
    expect(code?.textContent).toBe("npm test");
    expect(code?.className).not.toContain("hljs");
  });

  it("renders GFM tables", () => {
    render(<MarkdownMessage content={"| Col |\n| --- |\n| Val |"} />);

    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "Col" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "Val" })).toBeInTheDocument();
  });

  it("renders KaTeX math", () => {
    const { container } = render(<MarkdownMessage content={"Energy: $E = mc^2$"} />);

    expect(container.querySelector(".katex")).not.toBeNull();
  });

  it("falls back to the diagram source when mermaid cannot render", () => {
    render(<MarkdownMessage content={"```mermaid\ngraph TD\n  A-->B\n```"} />);

    expect(screen.getByText(/graph TD/)).toBeInTheDocument();
  });

  it("renders every top-level block when content spans multiple blocks", () => {
    render(
      <MarkdownMessage
        content={"# Heading\n\nFirst paragraph.\n\n```ts\nconst x = 1;\n```\n\nLast paragraph."}
        id="msg-1"
      />,
    );

    expect(screen.getByRole("heading", { level: 1, name: "Heading" })).toBeInTheDocument();
    expect(screen.getByText("First paragraph.")).toBeInTheDocument();
    expect(screen.getByText("Last paragraph.")).toBeInTheDocument();
  });
});
