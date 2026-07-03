import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// Reproduction for "cannot add any node from the toolbar". The existing
// page.test.tsx mocks CanvasToolbar to a static div, so the real button ->
// addSkillNode wiring is never exercised. Here we render the REAL toolbar and
// click its Note button, capturing the document handed to a mocked board.

function c1Data() {
  return {
    id: "c1",
    title: "B",
    document: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } },
  };
}

let currentParams: { canvasId?: string } = { canvasId: "c1" };
let canvasResult: { data: ReturnType<typeof c1Data> | undefined } = { data: c1Data() };

beforeEach(() => {
  currentParams = { canvasId: "c1" };
  canvasResult = { data: c1Data() };
});

vi.mock("react-router-dom", () => ({ useParams: () => currentParams }));
vi.mock("./api", () => ({ extractUrl: vi.fn(), saveCanvas: vi.fn() }));
vi.mock("./stream", () => ({ runCanvasNode: vi.fn(), streamCanvasChat: vi.fn() }));
vi.mock("./history-sidebar", () => ({ HistorySidebar: () => <div>history</div> }));
vi.mock("./chat-panel", () => ({ ChatPanel: () => <div>chat</div> }));
vi.mock("./kb-picker", () => ({ KbProjectPicker: () => <div>kb-picker</div> }));
vi.mock("./queries", () => ({
  useCanvas: () => canvasResult,
  useSaveCanvas: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  useCanvasCacheSave: () => vi.fn().mockResolvedValue(undefined),
}));

let boardProps: { document?: { nodes: { id: string; type: string }[] } } = {};
vi.mock("./canvas-board", () => ({
  CanvasBoard: (props: typeof boardProps) => {
    boardProps = props;
    return <div>board</div>;
  },
}));

import { CanvasPage } from "./page";

describe("adding a node from the toolbar", () => {
  it("appends a note node to the document when Note is clicked", async () => {
    render(<CanvasPage />);
    await waitFor(() => expect(screen.getByText("board")).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: /note/i }));

    await waitFor(() => {
      const nodes = boardProps.document?.nodes ?? [];
      expect(nodes.length).toBe(1);
      expect(nodes[0]?.type).toBe("note");
    });
  });
});
