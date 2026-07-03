import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const cacheSave = vi.fn();
vi.mock("./queries", () => ({ useCanvasCacheSave: () => cacheSave }));

import { PendingSaveBanner } from "./pending-save-banner";
import { pendingSaveStore } from "./pending-save-store";
import type { CanvasDocument } from "./types";

const doc: CanvasDocument = { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };

beforeEach(() => {
  pendingSaveStore.clear();
  cacheSave.mockReset();
});

describe("PendingSaveBanner", () => {
  it("renders nothing when there are no pending saves", () => {
    const { container } = render(<PendingSaveBanner />);
    expect(container.firstChild).toBeNull();
  });

  it("auto-drains a pending entry and disappears once it succeeds", async () => {
    cacheSave.mockResolvedValue(undefined);
    pendingSaveStore.enqueue({ id: "c1", title: "A", document: doc, seq: 1 });
    render(<PendingSaveBanner />);
    await waitFor(() =>
      expect(cacheSave).toHaveBeenCalledWith("c1", { title: "A", document: doc }),
    );
    await waitFor(() => expect(pendingSaveStore.list()).toHaveLength(0));
  });

  it("shows the banner and does not loop when the auto-drain keeps failing", async () => {
    cacheSave.mockRejectedValue(new Error("nope"));
    pendingSaveStore.enqueue({ id: "c1", title: "A", document: doc, seq: 1 });
    render(<PendingSaveBanner />);
    await screen.findByRole("alert");
    await waitFor(() => expect(cacheSave).toHaveBeenCalledTimes(1));
    // setError re-renders the banner; the id:seq guard must stop a re-attempt.
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(cacheSave).toHaveBeenCalledTimes(1);
  });

  it("retries pending saves when Retry is clicked", async () => {
    cacheSave.mockRejectedValueOnce(new Error("nope")).mockResolvedValue(undefined);
    pendingSaveStore.enqueue({ id: "c1", title: "A", document: doc, seq: 1 });
    render(<PendingSaveBanner />);
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: /retry/i }));
    await waitFor(() => expect(pendingSaveStore.list()).toHaveLength(0));
  });
});
