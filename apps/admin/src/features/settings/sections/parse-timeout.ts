/// Coerce a timeout text input to a positive integer, falling back to `fallback`
/// when the field is empty, non-numeric, or not >= 1. Prevents 0/NaN from being
/// sent to the API when the box is cleared.
export function parseTimeoutSeconds(value: string, fallback: number): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 1 ? Math.floor(parsed) : fallback;
}
