import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ChatPanel } from "./chat-panel";

describe("ChatPanel slash menu", () => {
  it("lists the four v1 skills when input starts with /", () => {
    render(<ChatPanel canvasId="c1" selectedNodeIds={[]} onSkillNode={() => {}} />);
    const input = screen.getByPlaceholderText("/ 或提问");
    fireEvent.change(input, { target: { value: "/" } });
    expect(screen.getByText("/search")).toBeInTheDocument();
    expect(screen.getByText("/image")).toBeInTheDocument();
    expect(screen.getByText("/analyze")).toBeInTheDocument();
    expect(screen.getByText("/kb")).toBeInTheDocument();
  });
});
