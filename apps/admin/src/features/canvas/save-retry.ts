// Best-effort background retry for a canvas save whose caller has already gone
// away. The leave-save runs in the page's unmount cleanup, so a rejected save
// cannot surface the in-page Retry button -- the edit would be silently lost.
// This loop retries the save a few times with linear backoff and, only after
// exhausting them, logs the failure. Pass a save closure that updates the
// query cache (e.g. useCanvasCacheSave) so a successful retry stays consistent
// with the rest of the app.
export function retrySaveInBackground(
  save: () => Promise<unknown>,
  { attempts = 3, baseDelayMs = 500 }: { attempts?: number; baseDelayMs?: number } = {},
): void {
  let tried = 0;
  const run = () => {
    tried += 1;
    void save().catch((error: unknown) => {
      if (tried >= attempts) {
        console.error("canvas leave-save failed after retries", error);
        return;
      }
      setTimeout(run, baseDelayMs * tried);
    });
  };
  run();
}
