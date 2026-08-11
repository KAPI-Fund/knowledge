const dateFormatter = new Intl.DateTimeFormat("en-US", {
  year: "numeric",
  month: "short",
  day: "numeric",
});

const dateTimeFormatter = new Intl.DateTimeFormat("en-US", {
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

const relativeFormatter = new Intl.RelativeTimeFormat("en-US", { numeric: "auto" });

function parse(value: string | number | Date | null | undefined): Date | null {
  if (value == null || value === "") return null;
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

export function formatDate(value: string | number | Date | null | undefined): string {
  const date = parse(value);
  return date ? dateFormatter.format(date) : "—";
}

export function formatDateTime(value: string | number | Date | null | undefined): string {
  const date = parse(value);
  return date ? dateTimeFormatter.format(date) : "—";
}

const RELATIVE_UNITS: Array<[Intl.RelativeTimeFormatUnit, number]> = [
  ["year", 1000 * 60 * 60 * 24 * 365],
  ["month", 1000 * 60 * 60 * 24 * 30],
  ["week", 1000 * 60 * 60 * 24 * 7],
  ["day", 1000 * 60 * 60 * 24],
  ["hour", 1000 * 60 * 60],
  ["minute", 1000 * 60],
];

export function formatRelative(value: string | number | Date | null | undefined): string {
  const date = parse(value);
  if (!date) return "—";
  const delta = date.getTime() - Date.now();
  for (const [unit, ms] of RELATIVE_UNITS) {
    if (Math.abs(delta) >= ms) {
      return relativeFormatter.format(Math.round(delta / ms), unit);
    }
  }
  return "just now";
}
