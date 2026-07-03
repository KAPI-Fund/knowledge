import { beforeEach, describe, expect, it, vi } from "vitest";

import { createPendingSaveStore } from "./pending-save-store";
import type { CanvasDocument } from "./types";

const doc: CanvasDocument = { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };

function entry(id: string, seq: number) {
  return { id, title: id, document: doc, seq };
}

beforeEach(() => localStorage.clear());

describe("pending-save-store", () => {
  it("allocates monotonically increasing seqs", () => {
    const s = createPendingSaveStore();
    expect(s.allocateSeq()).toBe(1);
    expect(s.allocateSeq()).toBe(2);
    expect(s.peekSeq()).toBe(2);
  });

  it("enqueues and lists an entry", () => {
    const s = createPendingSaveStore();
    s.enqueue(entry("c1", 1));
    expect(s.list().map((e) => e.id)).toEqual(["c1"]);
  });

  it("keeps only the highest seq per id", () => {
    const s = createPendingSaveStore();
    s.enqueue(entry("c1", 5));
    s.enqueue(entry("c1", 3)); // stale, must be ignored
    expect(s.list()).toHaveLength(1);
    expect(s.list()[0].seq).toBe(5);
    s.enqueue(entry("c1", 9)); // newer, replaces
    expect(s.list()[0].seq).toBe(9);
  });

  it("resolves an entry when the saved seq is at least the entry seq", () => {
    const s = createPendingSaveStore();
    s.enqueue(entry("c1", 5));
    s.resolve("c1", 5);
    expect(s.list()).toHaveLength(0);
  });

  it("keeps an entry when a newer edit is pending beyond the resolved seq", () => {
    const s = createPendingSaveStore();
    s.enqueue(entry("c1", 9));
    s.resolve("c1", 5); // an older save succeeded; the newer edit must survive
    expect(s.list()).toHaveLength(1);
    expect(s.list()[0].seq).toBe(9);
  });

  it("annotates an entry with an error and keeps it", () => {
    const s = createPendingSaveStore();
    s.enqueue(entry("c1", 1));
    s.setError("c1", "boom");
    expect(s.list()[0].error).toBe("boom");
  });

  it("returns a stable snapshot reference between mutations", () => {
    const s = createPendingSaveStore();
    s.enqueue(entry("c1", 1));
    expect(s.list()).toBe(s.list());
  });

  it("returns a new snapshot reference after a mutation", () => {
    const s = createPendingSaveStore();
    const before = s.list();
    s.enqueue(entry("c1", 1));
    expect(s.list()).not.toBe(before);
  });

  it("notifies subscribers on mutation and stops after unsubscribe", () => {
    const s = createPendingSaveStore();
    const listener = vi.fn();
    const unsub = s.subscribe(listener);
    s.enqueue(entry("c1", 1));
    expect(listener).toHaveBeenCalledTimes(1);
    unsub();
    s.enqueue(entry("c2", 2));
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("persists entries and the seq counter across store instances", () => {
    const a = createPendingSaveStore();
    a.allocateSeq(); // seq -> 1
    a.enqueue(entry("c1", 1));
    const b = createPendingSaveStore();
    expect(b.list().map((e) => e.id)).toEqual(["c1"]);
    expect(b.allocateSeq()).toBe(2); // continues from the persisted 1
  });

  it("clear empties entries and resets the counter", () => {
    const s = createPendingSaveStore();
    s.allocateSeq();
    s.enqueue(entry("c1", 1));
    s.clear();
    expect(s.list()).toHaveLength(0);
    expect(s.peekSeq()).toBe(0);
  });
});
