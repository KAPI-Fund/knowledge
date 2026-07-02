import { useCallback, useEffect, useRef, useState } from "react";

export type SaveStatus = "idle" | "pending" | "saving" | "saved" | "error";

interface UseAutosaveOptions<T> {
  value: T;
  delayMs: number;
  onSave: (value: T) => Promise<unknown>;
}

export function useAutosave<T>({ value, delayMs, onSave }: UseAutosaveOptions<T>) {
  const [status, setStatus] = useState<SaveStatus>("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastSaved = useRef<string | null>(null);
  const latest = useRef(value);
  latest.current = value;

  const reset = useCallback((next: T) => {
    lastSaved.current = JSON.stringify(next);
    if (timer.current) clearTimeout(timer.current);
    setStatus("idle");
  }, []);

  // Persist the latest value immediately if it differs from what was last
  // saved, then cancel the pending debounce. Callers pass their own save
  // function so an edit can be flushed to the right target (e.g. the canvas
  // being navigated away from) instead of whatever the debounced onSave is
  // currently bound to.
  const flush = useCallback((save: (value: T) => Promise<unknown>) => {
    if (timer.current) clearTimeout(timer.current);
    const serialized = JSON.stringify(latest.current);
    if (serialized === lastSaved.current) {
      return;
    }
    lastSaved.current = serialized;
    void save(latest.current);
  }, []);

  useEffect(() => {
    const serialized = JSON.stringify(value);
    if (lastSaved.current === null) {
      lastSaved.current = serialized;
      return;
    }
    if (serialized === lastSaved.current) {
      return;
    }
    setStatus("pending");
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      const snapshot = JSON.stringify(latest.current);
      setStatus("saving");
      onSave(latest.current)
        .then(() => {
          lastSaved.current = snapshot;
          setStatus("saved");
        })
        .catch(() => setStatus("error"));
    }, delayMs);
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [value, delayMs, onSave]);

  return { status, reset, flush };
}
