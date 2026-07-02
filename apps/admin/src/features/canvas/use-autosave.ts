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
  // currently bound to. Mirrors the debounced path: only advance lastSaved
  // after the save resolves so a failed flush surfaces "error" and stays
  // retriable instead of being silently marked saved. Resolves true when the
  // value is persisted (or there was nothing to save) and false on failure,
  // so callers can gate follow-up work (e.g. an SSE run) on a durable write.
  const flush = useCallback(async (save: (value: T) => Promise<unknown>): Promise<boolean> => {
    if (timer.current) clearTimeout(timer.current);
    const snapshot = JSON.stringify(latest.current);
    if (snapshot === lastSaved.current) {
      return true;
    }
    setStatus("saving");
    try {
      await save(latest.current);
      lastSaved.current = snapshot;
      setStatus("saved");
      return true;
    } catch {
      setStatus("error");
      return false;
    }
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
