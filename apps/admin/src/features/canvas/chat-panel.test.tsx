import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ChatPanel } from "./chat-panel";
import { streamCanvasChat } from "./stream";

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

const MOCK_SKILLS = [
  {
    command: "ppt",
    name: "PPT 生成",
    description: "把选中内容做成幻灯片",
    requiresSelection: true,
    argumentHint: "可选:风格 / 要求",
    outputNodeType: "html",
  },
  {
    command: "search",
    name: "Web search",
    description: "Search the web and add a note",
    requiresSelection: false,
    argumentHint: null,
    outputNodeType: "search",
  },
];

vi.mock("./use-skills-query", () => ({
  useSkillsQuery: () => ({ data: MOCK_SKILLS }),
}));

const origin = { x: 300, y: 150 };

beforeEach(() => {
  vi.mocked(streamCanvasChat).mockClear();
});

describe("ChatPanel slash menu", () => {
  it("lists skills from GET /api/skills when the input starts with /", () => {
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={[]}
        onSkillNode={() => {}}
        placementOrigin={origin}
        onBeforeSend={async () => true}
      />,
    );
    const input = screen.getByPlaceholderText("Ask a question");
    fireEvent.change(input, { target: { value: "/" } });
    expect(screen.getByText("/ppt")).toBeInTheDocument();
    expect(screen.getByText("/search")).toBeInTheDocument();
    // ppt requires a selection; with none selected it is flagged.
    expect(screen.getByText("需选中节点")).toBeInTheDocument();
  });

  it("filters the menu by the typed command token", () => {
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={[]}
        onSkillNode={() => {}}
        placementOrigin={origin}
        onBeforeSend={async () => true}
      />,
    );
    const input = screen.getByPlaceholderText("Ask a question");
    fireEvent.change(input, { target: { value: "/se" } });
    expect(screen.getByText("/search")).toBeInTheDocument();
    expect(screen.queryByText("/ppt")).not.toBeInTheDocument();
  });

  it("fills the command and swaps the placeholder when a skill is chosen", () => {
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={["a"]}
        onSkillNode={() => {}}
        placementOrigin={origin}
        onBeforeSend={async () => true}
      />,
    );
    const input = screen.getByPlaceholderText("Ask a question") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "/ppt" } });
    const option = screen.getByRole("option", { name: /PPT 生成/ });
    fireEvent.click(option);
    expect(input.value).toBe("/ppt ");
    expect(screen.getByPlaceholderText("可选:风格 / 要求")).toBeInTheDocument();
  });

  it("blocks a selection-required skill when nothing is selected", () => {
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={[]}
        onSkillNode={() => {}}
        placementOrigin={origin}
        onBeforeSend={async () => true}
      />,
    );
    const input = screen.getByPlaceholderText("Ask a question") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "/ppt" } });
    const option = screen.getByRole("option", { name: /PPT 生成/ });
    fireEvent.click(option);
    // Disabled item does not fill the command.
    expect(input.value).toBe("/ppt");
  });
});

describe("ChatPanel skill submit", () => {
  it("calls onSkillNode when a search skill returns a node", async () => {
    const onSkillNode = vi.fn();
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={[]}
        onSkillNode={onSkillNode}
        placementOrigin={origin}
        onBeforeSend={async () => true}
      />,
    );
    const input = screen.getByPlaceholderText("Ask a question");
    fireEvent.change(input, { target: { value: "/search cats" } });
    fireEvent.submit(input.closest("form")!);
    await waitFor(() => expect(onSkillNode).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByText("Added a node to the canvas.")).toBeInTheDocument());
  });

  it("forwards the placement origin to streamCanvasChat", async () => {
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={["a"]}
        onSkillNode={() => {}}
        placementOrigin={origin}
        onBeforeSend={async () => true}
      />,
    );
    const input = screen.getByPlaceholderText("Ask a question");
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.submit(input.closest("form")!);
    await waitFor(() => expect(streamCanvasChat).toHaveBeenCalled());
    const call = vi.mocked(streamCanvasChat).mock.lastCall!;
    expect(call.slice(0, 3)).toEqual(["c1", "hello", ["a"]]);
    expect(call[4]).toEqual(origin);
  });

  it("saves the canvas before streaming so the backend sees the edit", async () => {
    const onBeforeSend = vi.fn().mockResolvedValue(true);
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={[]}
        onSkillNode={() => {}}
        placementOrigin={origin}
        onBeforeSend={onBeforeSend}
      />,
    );
    const input = screen.getByPlaceholderText("Ask a question");
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.submit(input.closest("form")!);

    await waitFor(() => expect(streamCanvasChat).toHaveBeenCalled());
    expect(onBeforeSend).toHaveBeenCalledTimes(1);
    // The save must resolve before the SSE run starts.
    expect(onBeforeSend.mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(streamCanvasChat).mock.invocationCallOrder[0],
    );
  });

  it("does not stream when the pre-send save fails", async () => {
    const onBeforeSend = vi.fn().mockResolvedValue(false);
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={[]}
        onSkillNode={() => {}}
        placementOrigin={origin}
        onBeforeSend={onBeforeSend}
      />,
    );
    const input = screen.getByPlaceholderText("Ask a question");
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.submit(input.closest("form")!);

    await waitFor(() => expect(onBeforeSend).toHaveBeenCalled());
    await Promise.resolve();
    expect(streamCanvasChat).not.toHaveBeenCalled();
  });
});
