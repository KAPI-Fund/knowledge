import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const extractUrl = vi.fn();

vi.mock("./api", () => ({ extractUrl: (url: string) => extractUrl(url) }));
vi.mock("./history-sidebar", () => ({ HistorySidebar: () => <div>history</div> }));
vi.mock("./chat-panel", () => ({ ChatPanel: () => <div>chat</div> }));
vi.mock("./canvas-toolbar", () => ({ CanvasToolbar: () => <div>toolbar</div> }));

let boardProps: {
  onFetchUrl?: (id: string) => void;
} = {};
vi.mock("./canvas-board", () => ({
  CanvasBoard: (props: { onFetchUrl?: (id: string) => void }) => {
    boardProps = props;
    return <div>board</div>;
  },
}));

vi.mock("./queries", () => ({
  useCanvasList: () => ({ data: [], isLoading: false }),
  useCanvas: () => ({
    data: {
      id: "c1",
      title: "B",
      document: {
        nodes: [{ id: "u1", type: "url", x: 0, y: 0, w: 280, h: 160, data: { url: "https://x.test" } }],
        edges: [],
        viewport: { x: 0, y: 0, zoom: 1 },
      },
    },
  }),
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

  it("fetches a url node through the extract endpoint", async () => {
    extractUrl.mockResolvedValue({ status: "ok", title: "Hello", markdown: "# hi", error: null });
    render(<CanvasPage />);
    await waitFor(() => expect(boardProps.onFetchUrl).toBeTypeOf("function"));
    boardProps.onFetchUrl?.("u1");
    await waitFor(() => expect(extractUrl).toHaveBeenCalledWith("https://x.test"));
  });
});
