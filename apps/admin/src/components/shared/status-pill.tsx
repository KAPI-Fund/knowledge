import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const statusPillVariants = cva(
  "inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 font-mono text-[10.5px] font-semibold leading-none",
  {
    variants: {
      status: {
        queued: "border-border bg-muted text-muted-foreground",
        running:
          "border-blue-200 bg-blue-50 text-blue-700 dark:border-blue-900 dark:bg-blue-950 dark:text-blue-300",
        succeeded:
          "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-300",
        failed:
          "border-red-200 bg-red-50 text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300",
        retry_waiting:
          "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-300",
      },
    },
    defaultVariants: { status: "queued" },
  },
);

const statusDotVariants = cva("size-1.5 shrink-0 rounded-full", {
  variants: {
    status: {
      queued: "bg-muted-foreground/60",
      running: "bg-blue-500 dark:bg-blue-400",
      succeeded: "bg-emerald-500 dark:bg-emerald-400",
      failed: "bg-red-500 dark:bg-red-400",
      retry_waiting: "bg-amber-500 dark:bg-amber-400",
    },
  },
  defaultVariants: { status: "queued" },
});

type StatusVariant = NonNullable<VariantProps<typeof statusPillVariants>["status"]>;

const STATUS_ALIASES: Record<string, StatusVariant> = {
  queued: "queued",
  pending: "queued",
  running: "running",
  in_progress: "running",
  active: "succeeded",
  succeeded: "succeeded",
  completed: "succeeded",
  resolved: "succeeded",
  failed: "failed",
  cancelled: "failed",
  canceled: "failed",
  revoked: "failed",
  retry_waiting: "retry_waiting",
  unresolved: "retry_waiting",
};

export function StatusPill({ className, value }: { className?: string; value: string }) {
  const variant = STATUS_ALIASES[value] ?? "queued";
  return (
    <span className={cn(statusPillVariants({ status: variant }), className)}>
      <span aria-hidden className={statusDotVariants({ status: variant })} />
      {value}
    </span>
  );
}
