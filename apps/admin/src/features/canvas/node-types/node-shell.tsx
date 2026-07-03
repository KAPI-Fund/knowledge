import { Handle, Position } from "@xyflow/react";
import type { ReactNode } from "react";

import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export type NodeShellStatus = "idle" | "running" | "loading" | "error" | "ok" | "no-access";

interface StatusMeta {
  label: string;
  accent: string;
  badgeClassName?: string;
  showBadge: boolean;
}

const STATUS_META: Record<NodeShellStatus, StatusMeta> = {
  idle: { label: "", accent: "border-l-border", showBadge: false },
  ok: { label: "", accent: "border-l-border", showBadge: false },
  running: {
    label: "RUNNING",
    accent: "border-l-amber-500",
    badgeClassName: "border-transparent bg-amber-500/15 text-amber-600 dark:text-amber-400",
    showBadge: true,
  },
  loading: {
    label: "LOADING",
    accent: "border-l-amber-500",
    badgeClassName: "border-transparent bg-amber-500/15 text-amber-600 dark:text-amber-400",
    showBadge: true,
  },
  error: {
    label: "ERROR",
    accent: "border-l-destructive",
    badgeClassName: "border-transparent bg-destructive/15 text-destructive",
    showBadge: true,
  },
  "no-access": {
    label: "NO ACCESS",
    accent: "border-l-destructive",
    badgeClassName: "border-transparent bg-destructive/15 text-destructive",
    showBadge: true,
  },
};

interface NodeShellProps {
  icon: ReactNode;
  label: string;
  nodeId: string;
  index?: number;
  selected?: boolean;
  status?: NodeShellStatus;
  statusLabel?: string;
  headerRight?: ReactNode;
  // Source nodes (e.g. URL · WEB) fetch their own content and never take an
  // incoming edge, so they hide the left target handle.
  targetHandle?: boolean;
  children: ReactNode;
}

export function NodeShell({
  icon,
  label,
  nodeId,
  index,
  selected,
  status = "idle",
  statusLabel,
  headerRight,
  targetHandle = true,
  children,
}: NodeShellProps) {
  const meta = STATUS_META[status];
  const pillLabel = statusLabel ?? meta.label;

  return (
    <div
      className={cn(
        "flex h-full flex-col rounded-lg border border-l-[3px] bg-card text-card-foreground shadow-sm",
        meta.accent,
        selected && "ring-2 ring-primary ring-offset-1 ring-offset-background",
      )}
    >
      {targetHandle ? (
        <Handle
          type="target"
          position={Position.Left}
          className="!size-3 !rounded-full !border-2 !border-background !bg-primary"
        />
      ) : null}

      <div className="flex items-center gap-1.5 rounded-t-lg border-b px-2.5 py-1.5">
        {index !== undefined ? (
          <span className="flex size-4 shrink-0 items-center justify-center rounded-full bg-primary/10 font-mono text-[10px] font-semibold leading-none text-primary">
            {index}
          </span>
        ) : null}
        <span className="text-muted-foreground">{icon}</span>
        <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          {label}
        </span>
        <span className="rounded bg-muted px-1 py-0.5 font-mono text-[10px] leading-none text-muted-foreground">
          {nodeId.slice(0, 4)}
        </span>
        <div className="ml-auto flex items-center gap-1.5">
          {meta.showBadge && pillLabel ? (
            <Badge className={cn("px-1.5 py-0 text-[10px]", meta.badgeClassName)}>{pillLabel}</Badge>
          ) : null}
          {headerRight}
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-2 p-2.5">{children}</div>

      <Handle
        type="source"
        position={Position.Right}
        className="!size-3 !rounded-full !border-2 !border-background !bg-primary"
      />
    </div>
  );
}
