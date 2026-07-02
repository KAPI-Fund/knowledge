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

const origin = { x: 300, y: 150 };

beforeEach(() => {
  vi.mocked(streamCanvasChat).mockClear();
});

describe("ChatPanel slash menu", () => {
  it("lists the four v1 skills when input starts with /", () => {
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={[]}
        onSkillNode={() => {}}
        placementOrigin={origin}
        onBeforeSend={async () => true}
      />,
    );
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
    render(
      <ChatPanel
        canvasId="c1"
        selectedNodeIds={[]}
        onSkillNode={onSkillNode}
        placementOrigin={origin}
        onBeforeSend={async () => true}
      />,
    );
    const input = screen.getByPlaceholderText("/ or ask");
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
    const input = screen.getByPlaceholderText("/ or ask");
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
    const input = screen.getByPlaceholderText("/ or ask");
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
    const input = screen.getByPlaceholderText("/ or ask");
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.submit(input.closest("form")!);

    await waitFor(() => expect(onBeforeSend).toHaveBeenCalled());
    await Promise.resolve();
    expect(streamCanvasChat).not.toHaveBeenCalled();
  });
});
