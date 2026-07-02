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

  return { status, reset };
}
