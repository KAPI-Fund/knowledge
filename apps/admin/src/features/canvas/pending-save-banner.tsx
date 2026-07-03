import { useCallback, useEffect, useRef, useSyncExternalStore } from "react";

import { useCanvasCacheSave } from "./queries";
import { pendingSaveStore, type PendingCanvasSave } from "./pending-save-store";

function message(error: unknown): string {
  return error instanceof Error ? error.message : "save failed";
}

// App-level recovery for canvas saves that failed after their page was gone
// (see page.tsx's unmount leave-save). Mounted once in the app shell so it
// outlives the canvas route. Drains the persistent pending-save store; a
// successful drain resolves the entry (and cancels a superseded stale retry via
// the seq token), a failed one leaves the entry with an error for manual Retry.
export function PendingSaveBanner() {
  const entries = useSyncExternalStore(
    pendingSaveStore.subscribe,
    pendingSaveStore.list,
    pendingSaveStore.list,
  );
  const cacheSave = useCanvasCacheSave();
  // Keyed by id+seq so a newer edit re-attempts automatically but a failed
  // attempt does not, which prevents a tight auto-retry loop against the server.
  const attempted = useRef<Set<string>>(new Set());

  const drain = useCallback(
    (entry: PendingCanvasSave) =>
      cacheSave(entry.id, { title: entry.title, document: entry.document })
        .then(() => pendingSaveStore.resolve(entry.id, entry.seq))
        .catch((error: unknown) => pendingSaveStore.setError(entry.id, message(error))),
    [cacheSave],
  );

  useEffect(() => {
    for (const entry of entries) {
      const key = `${entry.id}:${entry.seq}`;
      if (attempted.current.has(key)) {
        continue;
      }
      attempted.current.add(key);
      void drain(entry);
    }
  }, [entries, drain]);

  const retryAll = useCallback(() => {
    for (const entry of entries) {
      attempted.current.add(`${entry.id}:${entry.seq}`);
      void drain(entry);
    }
  }, [entries, drain]);

  if (entries.length === 0) {
    return null;
  }

  return (
    <div
      role="alert"
      className="flex items-center justify-between gap-3 border-b border-destructive/30 bg-destructive/5 px-4 py-2 text-sm text-destructive"
    >
      <span className="truncate">
        {entries.length === 1
          ? `Couldn't save "${entries[0].title || "Untitled canvas"}" - it will be retried.`
          : `${entries.length} canvas changes couldn't be saved.`}
      </span>
      <button
        type="button"
        onClick={retryAll}
        className="shrink-0 rounded border border-destructive/40 px-2 py-0.5 text-xs font-medium hover:bg-destructive/10"
      >
        Retry
      </button>
    </div>
  );
}
