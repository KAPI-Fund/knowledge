import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const statusPillVariants = cva(
  "inline-flex items-center rounded-md border px-2 py-0.5 font-mono text-[10.5px] font-semibold leading-none",
  {
    variants: {
      status: {
        queued: "border-[#d0d7de] bg-[#eaeef2] text-[#57606a]",
        running: "border-[#b6d4fe] bg-[#ddebff] text-[#0a53c4]",
        succeeded: "border-[#abe0ba] bg-[#ddf4e4] text-[#1a7f37]",
        failed: "border-[#f5b5ba] bg-[#ffe3e3] text-[#c21f2e]",
        retry_waiting: "border-[#f3d98b] bg-[#fff1d6] text-[#9a6700]",
      },
    },
    defaultVariants: { status: "queued" },
  },
);

type StatusVariant = NonNullable<VariantProps<typeof statusPillVariants>["status"]>;

const STATUS_ALIASES: Record<string, StatusVariant> = {
  queued: "queued",
  pending: "queued",
  running: "running",
  in_progress: "running",
  succeeded: "succeeded",
  completed: "succeeded",
  failed: "failed",
  cancelled: "failed",
  canceled: "failed",
  retry_waiting: "retry_waiting",
};

export function StatusPill({ className, value }: { className?: string; value: string }) {
  const variant = STATUS_ALIASES[value] ?? "queued";
  return <span className={cn(statusPillVariants({ status: variant }), className)}>{value}</span>;
}
