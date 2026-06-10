import { Badge } from "@/components/ui/badge";

export function StatusBadge({ value }: { value: string }) {
  const normalized = value.toLowerCase();
  const variant =
    normalized === "failed" || normalized === "cancelled"
      ? "destructive"
      : normalized === "queued" || normalized === "running"
        ? "secondary"
        : "default";

  return <Badge variant={variant}>{value}</Badge>;
}
