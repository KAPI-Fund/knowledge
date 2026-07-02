import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const extractUrl = vi.fn();
const saveCanvas = vi.fn();
const runCanvasNode = vi.fn();

function c1Data() {
  return {
    id: "c1",
    title: "B",
    document: {
      nodes: [{ id: "u1", type: "url", x: 0, y: 0, w: 280, h: 160, data: { url: "https://x.test" } }],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    },
  };
}

let currentParams: { canvasId?: string } = { canvasId: "c1" };
let canvasResult: { data: ReturnType<typeof c1Data> | undefined } = { data: c1Data() };

beforeEach(() => {
  currentParams = { canvasId: "c1" };
  canvasResult = { data: c1Data() };
  saveCanvas.mockReset();
  saveCanvas.mockResolvedValue(undefined);
  runCanvasNode.mockReset();
  runCanvasNode.mockResolvedValue(undefined);
});

vi.mock("react-router-dom", () => ({ useParams: () => currentParams }));
vi.mock("./api", () => ({
  extractUrl: (url: string) => extractUrl(url),
  saveCanvas: (id: string, body: unknown) => saveCanvas(id, body),
}));
vi.mock("./stream", () => ({
  runCanvasNode: (id: string, nodeId: string, handlers: unknown) =>
    runCanvasNode(id, nodeId, handlers),
}));
vi.mock("./history-sidebar", () => ({ HistorySidebar: () => <div>history</div> }));
vi.mock("./canvas-toolbar", () => ({ CanvasToolbar: () => <div>toolbar</div> }));

let chatProps: {
  selectedNodeIds?: string[];
  onSkillNode?: (payload: { node: unknown; x: number; y: number }) => void;
  placementOrigin?: { x: number; y: number };
} = {};
vi.mock("./chat-panel", () => ({
  ChatPanel: (props: {
    selectedNodeIds?: string[];
    onSkillNode?: (p: { node: unknown; x: number; y: number }) => void;
    placementOrigin?: { x: number; y: number };
  }) => {
    chatProps = props;
    return <div>chat</div>;
  },
}));

let boardProps: {
  onFetchUrl?: (id: string) => void;
  onRunNode?: (id: string) => void;
  onSelectionChange?: (ids: string[]) => void;
  document?: {
    nodes: { id: string; x: number; y: number }[];
    edges: { source: string; target: string }[];
  };
  onChange?: (doc: unknown) => void;
} = {};
vi.mock("./canvas-board", () => ({
  CanvasBoard: (props: typeof boardProps) => {
    boardProps = props;
    return <div>board</div>;
  },
}));

vi.mock("./queries", () => ({
  useCanvasList: () => ({ data: [], isLoading: false }),
  useCanvas: () => canvasResult,
  useCreateCanvas: () => ({ mutate: vi.fn() }),
  useSaveCanvas: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  useDeleteCanvas: () => ({ mutate: vi.fn() }),
}));

import { CanvasPage } from "./page";

describe("CanvasPage", () => {
  it("renders the three panes", () => {
    render(<CanvasPage />);
    expect(screen.getByText("history")).toBeInTheDocument();
    expect(screen.getByText("board")).toBeInTheDocument();
    expect(screen.getByText("chat")).toBeInTheDocument();
  });

  it("clears the loaded canvas when navigating to the index route", async () => {
    const { rerender } = render(<CanvasPage />);
    await waitFor(() => expect(screen.getByText("board")).toBeInTheDocument());

    // Navigate from /canvas/c1 to /canvas: params clear and the detail query
    // is disabled, so canvas.data becomes undefined.
    currentParams = {};
    canvasResult = { data: undefined };
    rerender(<CanvasPage />);

    await waitFor(() =>
      expect(screen.getByText(/select a canvas on the left/i)).toBeInTheDocument(),
    );
    expect(screen.queryByText("board")).not.toBeInTheDocument();
  });

  it("drops the old board while switching to a still-loading canvas", async () => {
    const { rerender } = render(<CanvasPage />);
    await waitFor(() => expect(screen.getByText("board")).toBeInTheDocument());

    // Navigate to /canvas/c2 before its detail query resolves. The c1 board
    // must disappear so an edit cannot autosave under the c2 id.
    currentParams = { canvasId: "c2" };
    canvasResult = { data: undefined };
    rerender(<CanvasPage />);
    await waitFor(() => expect(screen.queryByText("board")).not.toBeInTheDocument());

    // c2 finishes loading -> its board appears.
    canvasResult = { data: { ...c1Data(), id: "c2", title: "C2" } };
    rerender(<CanvasPage />);
    await waitFor(() => expect(screen.getByText("board")).toBeInTheDocument());
  });

  it("does not show a stale board when the new canvas fails to load", async () => {
    const { rerender } = render(<CanvasPage />);
    await waitFor(() => expect(screen.getByText("board")).toBeInTheDocument());

    // Navigate to /canvas/c2 whose detail query errors (data stays undefined).
    currentParams = { canvasId: "c2" };
    canvasResult = { data: undefined };
    rerender(<CanvasPage />);
    await waitFor(() => expect(screen.queryByText("board")).not.toBeInTheDocument());
  });

  it("fetches a url node through the extract endpoint", async () => {
    extractUrl.mockResolvedValue({ status: "ok", title: "Hello", markdown: "# hi", error: null });
    render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onFetchUrl).toBeTypeOf("function"));
    boardProps.onFetchUrl?.("u1");
    await waitFor(() => expect(extractUrl).toHaveBeenCalledWith("https://x.test"));
  });

  it("creates reference edges when an analyze skill node references selected nodes", async () => {
    render(<CanvasPage />);
    await waitFor(() => expect(chatProps.onSkillNode).toBeTypeOf("function"));
    chatProps.onSkillNode?.({
      node: {
        type: "ai_analyze",
        data: { prompt: "sum", sourceNodeIds: ["u1"] },
      },
      x: 0,
      y: 0,
    });
    await waitFor(() => {
      const edges = boardProps.document?.edges ?? [];
      expect(edges.some((e) => e.source === "u1")).toBe(true);
    });
  });

  it("derives the chat placement origin from the current viewport", async () => {
    const data = c1Data();
    canvasResult = { data: { ...data, document: { ...data.document, viewport: { x: -200, y: -100, zoom: 1 } } } };
    render(<CanvasPage />);
    // Screen top-left maps to world (-vp.x, -vp.y) / zoom; add an 80px margin.
    await waitFor(() => expect(chatProps.placementOrigin).toEqual({ x: 280, y: 180 }));
  });

  it("places a skill node at non-zero payload coordinates", async () => {
    render(<CanvasPage />);
    await waitFor(() => expect(chatProps.onSkillNode).toBeTypeOf("function"));
    chatProps.onSkillNode?.({
      node: { type: "note", data: {} },
      x: 321,
      y: 654,
    });
    await waitFor(() => {
      const nodes = boardProps.document?.nodes ?? [];
      expect(nodes.some((n) => n.x === 321 && n.y === 654)).toBe(true);
    });
  });

  it("flushes a pending edit to its own canvas id when navigating away", async () => {
    saveCanvas.mockResolvedValue(undefined);
    const { rerender } = render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onChange).toBeTypeOf("function"));

    // Edit c1 while still inside the 800ms autosave debounce window.
    const edited = {
      nodes: [{ id: "u1", type: "url", x: 42, y: 99, w: 280, h: 160, data: { url: "https://x.test" } }],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    };
    act(() => boardProps.onChange?.(edited));

    // Navigate to /canvas before the debounce fires. The pending edit must be
    // persisted under c1's own id, not dropped and not saved under the new route.
    currentParams = {};
    canvasResult = { data: undefined };
    rerender(<CanvasPage />);

    await waitFor(() =>
      expect(saveCanvas).toHaveBeenCalledWith("c1", expect.objectContaining({ document: edited })),
    );
  });

  it("saves the current doc before running a node so the backend sees the edit", async () => {
    render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onRunNode).toBeTypeOf("function"));

    // Add a node client-side (inside the debounce), then run a node immediately.
    const edited = {
      nodes: [
        { id: "u1", type: "url", x: 0, y: 0, w: 280, h: 160, data: { url: "https://x.test" } },
        { id: "n2", type: "note", x: 10, y: 10, w: 280, h: 160, data: {} },
      ],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    };
    act(() => boardProps.onChange?.(edited));
    act(() => boardProps.onRunNode?.("n2"));

    await waitFor(() =>
      expect(saveCanvas).toHaveBeenCalledWith("c1", expect.objectContaining({ document: edited })),
    );
    await waitFor(() =>
      expect(runCanvasNode).toHaveBeenCalledWith("c1", "n2", expect.anything()),
    );
  });

  it("does not run a node when the pre-run save fails", async () => {
    saveCanvas.mockRejectedValue(new Error("save failed"));
    render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onRunNode).toBeTypeOf("function"));

    const edited = {
      nodes: [{ id: "u1", type: "url", x: 1, y: 2, w: 280, h: 160, data: { url: "https://x.test" } }],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    };
    act(() => boardProps.onChange?.(edited));
    act(() => boardProps.onRunNode?.("u1"));

    await waitFor(() => expect(saveCanvas).toHaveBeenCalled());
    // Give any queued microtasks a chance; the SSE run must not have started.
    await Promise.resolve();
    expect(runCanvasNode).not.toHaveBeenCalled();
  });

  it("flushes a pending edit when the page unmounts (leaving for another route)", async () => {
    saveCanvas.mockResolvedValue(undefined);
    const { unmount } = render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onChange).toBeTypeOf("function"));

    // Edit c1 while inside the 800ms debounce, then leave the canvas section
    // entirely (e.g. click a sidebar link to /projects) which unmounts the page.
    const edited = {
      nodes: [{ id: "u1", type: "url", x: 7, y: 8, w: 280, h: 160, data: { url: "https://x.test" } }],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    };
    act(() => boardProps.onChange?.(edited));
    unmount();

    await waitFor(() =>
      expect(saveCanvas).toHaveBeenCalledWith("c1", expect.objectContaining({ document: edited })),
    );
  });

  it("retains a failed leave-save so it can be retried under the old canvas id", async () => {
    // The save triggered by leaving c1 fails. The old content must not be
    // discarded: the snapshot is retained and retriable under c1's own id,
    // not the new route and not the (now empty) current doc.
    saveCanvas.mockRejectedValueOnce(new Error("save failed"));
    const { rerender } = render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onChange).toBeTypeOf("function"));

    const edited = {
      nodes: [{ id: "u1", type: "url", x: 55, y: 66, w: 280, h: 160, data: { url: "https://x.test" } }],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    };
    act(() => boardProps.onChange?.(edited));

    // Leave c1 for the index route before the debounce fires; the flush rejects.
    currentParams = {};
    canvasResult = { data: undefined };
    rerender(<CanvasPage />);

    const retry = await screen.findByRole("button", { name: /couldn't save/i });
    saveCanvas.mockClear();
    fireEvent.click(retry);

    await waitFor(() =>
      expect(saveCanvas).toHaveBeenCalledWith("c1", expect.objectContaining({ document: edited })),
    );
    // A successful retry clears the failed-save affordance.
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: /couldn't save/i })).not.toBeInTheDocument(),
    );
  });

  it("does not create edges to source ids absent from the canvas", async () => {
    render(<CanvasPage />);
    await waitFor(() => expect(chatProps.onSkillNode).toBeTypeOf("function"));
    chatProps.onSkillNode?.({
      node: {
        type: "ai_analyze",
        data: { prompt: "sum", sourceNodeIds: ["ghost"] },
      },
      x: 0,
      y: 0,
    });
    await waitFor(() => {
      const nodes = boardProps.document?.nodes ?? [];
      expect(nodes.length).toBe(2);
    });
    const edges = boardProps.document?.edges ?? [];
    expect(edges.some((e) => e.source === "ghost")).toBe(false);
  });
});
