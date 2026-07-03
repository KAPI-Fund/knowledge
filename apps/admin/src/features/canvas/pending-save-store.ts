import type { CanvasDocument } from "./types";

// A canvas edit that could not be saved and is kept for a later retry. `seq` is
// a monotonic generation token: a successful save of the same canvas at a seq
// >= this one supersedes the entry (see `resolve`), which is how a stale
// leave-retry is cancelled once the canvas is reopened and saved again.
export interface PendingCanvasSave {
  id: string;
  title: string;
  document: CanvasDocument;
  seq: number;
  error?: string;
}

export interface PendingSaveStore {
  allocateSeq(): number;
  peekSeq(): number;
  enqueue(entry: PendingCanvasSave): void;
  resolve(id: string, savedSeq: number): void;
  setError(id: string, error: string): void;
  list(): PendingCanvasSave[];
  subscribe(listener: () => void): () => void;
  clear(): void;
}

export const ENTRIES_KEY = "canvas:pending-saves";
export const SEQ_KEY = "canvas:pending-save-seq";

interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

function safeStorage(): StorageLike | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

// The store survives reloads via localStorage so a leave-save that keeps failing
// is not lost when the tab is closed and reopened. The App-level banner drains
// it; the canvas page enqueues into it on an unmount leave-save failure.
export function createPendingSaveStore(
  storage: StorageLike | null = safeStorage(),
): PendingSaveStore {
  let entries: PendingCanvasSave[] = [];
  let seq = 0;
  // Cached so `list` returns a stable reference between mutations, which
  // useSyncExternalStore requires to avoid re-render loops.
  let snapshot: PendingCanvasSave[] = entries;
  let loaded = false;
  const listeners = new Set<() => void>();

  function load() {
    if (loaded) {
      return;
    }
    loaded = true;
    if (!storage) {
      return;
    }
    try {
      const rawSeq = storage.getItem(SEQ_KEY);
      seq = rawSeq ? Number.parseInt(rawSeq, 10) || 0 : 0;
      const rawEntries = storage.getItem(ENTRIES_KEY);
      const parsed = rawEntries ? (JSON.parse(rawEntries) as unknown) : [];
      entries = Array.isArray(parsed) ? (parsed as PendingCanvasSave[]) : [];
    } catch {
      entries = [];
      seq = 0;
    }
    snapshot = entries;
  }

  function persist() {
    if (!storage) {
      return;
    }
    try {
      storage.setItem(ENTRIES_KEY, JSON.stringify(entries));
      storage.setItem(SEQ_KEY, String(seq));
    } catch {
      // best-effort; a full/blocked quota must not break saving
    }
  }

  function commit(next: PendingCanvasSave[]) {
    entries = next;
    snapshot = entries;
    persist();
    for (const listener of listeners) {
      listener();
    }
  }

  return {
    allocateSeq() {
      load();
      seq += 1;
      persist();
      return seq;
    },
    peekSeq() {
      load();
      return seq;
    },
    enqueue(entry) {
      load();
      const existing = entries.find((e) => e.id === entry.id);
      if (existing && existing.seq > entry.seq) {
        return;
      }
      commit([...entries.filter((e) => e.id !== entry.id), entry]);
    },
    resolve(id, savedSeq) {
      load();
      const existing = entries.find((e) => e.id === id);
      if (!existing || existing.seq > savedSeq) {
        return;
      }
      commit(entries.filter((e) => e.id !== id));
    },
    setError(id, error) {
      load();
      if (!entries.some((e) => e.id === id)) {
        return;
      }
      commit(entries.map((e) => (e.id === id ? { ...e, error } : e)));
    },
    list() {
      load();
      return snapshot;
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    clear() {
      loaded = true;
      entries = [];
      seq = 0;
      snapshot = entries;
      if (storage) {
        try {
          storage.removeItem(ENTRIES_KEY);
          storage.removeItem(SEQ_KEY);
        } catch {
          // ignore
        }
      }
      for (const listener of listeners) {
        listener();
      }
    },
  };
}

export const pendingSaveStore = createPendingSaveStore();
