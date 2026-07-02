import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ChatPanel } from "./chat-panel";

vi.mock("./stream", () => ({
  streamCanvasChat: vi.fn(
    async (
      _canvasId: string,
      _message: string,
      _ids: string[],
      handlers: {
        onNode?: (p: { node: unknown; x: number; y: number }) => void;
        onDone: (p: unknown) => void;
      },
    ) => {
      handlers.onNode?.({ node: { type: "note", data: {} }, x: 0, y: 0 });
      handlers.onDone({});
    },
  ),
}));

describe("ChatPanel slash menu", () => {
  it("lists the four v1 skills when input starts with /", () => {
    render(<ChatPanel canvasId="c1" selectedNodeIds={[]} onSkillNode={() => {}} />);
    const input = screen.getByPlaceholderText("/ or ask");
    fireEvent.change(input, { target: { value: "/" } });
    expect(screen.getByText("/search")).toBeInTheDocument();
    expect(screen.getByText("/image")).toBeInTheDocument();
    expect(screen.getByText("/analyze")).toBeInTheDocument();
    expect(screen.getByText("/kb")).toBeInTheDocument();
  });
});

describe("ChatPanel skill submit", () => {
  it("calls onSkillNode when a search skill returns a node", async () => {
    const onSkillNode = vi.fn();
    render(<ChatPanel canvasId="c1" selectedNodeIds={[]} onSkillNode={onSkillNode} />);
    const input = screen.getByPlaceholderText("/ or ask");
    fireEvent.change(input, { target: { value: "/search cats" } });
    fireEvent.submit(input.closest("form")!);
    await waitFor(() => expect(onSkillNode).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByText("Added a node to the canvas.")).toBeInTheDocument());
  });
});
