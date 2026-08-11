/// Coerce a query-limit text input to a positive integer within the backend's
/// accepted range (1..=100), falling back to `fallback` when the field is empty,
/// non-numeric, or out of range. Prevents 0/NaN/negatives from being sent to the
/// API when the box is cleared or mistyped (the backend rejects those with a 400).
export function parseQueryLimit(value: string, fallback: number): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  const floored = Math.floor(parsed);
  if (floored < 1 || floored > 100) {
    return fallback;
  }
  return floored;
}
