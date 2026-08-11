import { AlertTriangle, ShieldAlert } from "lucide-react";
import type { ReactNode } from "react";

import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

export function LoadingState({ className, rows = 4 }: { className?: string; rows?: number }) {
  return (
    <div aria-label="Loading" className={cn("grid gap-2", className)} role="status">
      {Array.from({ length: rows }).map((_, index) => (
        <Skeleton className="h-10 w-full" key={index} />
      ))}
    </div>
  );
}

function MessagePane({
  action,
  description,
  icon,
  title,
}: {
  action?: ReactNode;
  description?: string;
  icon: ReactNode;
  title: string;
}) {
  return (
    <div className="grid place-items-center gap-3 rounded-lg border border-border bg-card px-6 py-10 text-center">
      <div className="flex size-10 items-center justify-center rounded-full bg-muted text-muted-foreground">
        {icon}
      </div>
      <div className="grid max-w-sm gap-1">
        <p className="text-sm font-medium text-foreground">{title}</p>
        {description ? <p className="text-sm text-muted-foreground">{description}</p> : null}
      </div>
      {action}
    </div>
  );
}

export function ErrorState({
  action,
  description,
  title = "Something went wrong",
}: {
  action?: ReactNode;
  description?: string;
  title?: string;
}) {
  return (
    <MessagePane
      action={action}
      description={description}
      icon={<AlertTriangle className="size-5" />}
      title={title}
    />
  );
}

export function ForbiddenState({
  description = "You do not have permission to view this resource.",
  title = "Access denied",
}: {
  description?: string;
  title?: string;
}) {
  return (
    <MessagePane description={description} icon={<ShieldAlert className="size-5" />} title={title} />
  );
}
