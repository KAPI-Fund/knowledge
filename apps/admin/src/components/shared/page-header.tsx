import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

export function PageHeader({
  actions,
  breadcrumb,
  className,
  description,
  title,
}: {
  actions?: ReactNode;
  breadcrumb?: ReactNode;
  className?: string;
  description?: string;
  title: string;
}) {
  return (
    <header className={cn("flex flex-col gap-3 border-b border-border pb-4", className)}>
      {breadcrumb ? <div className="text-xs text-muted-foreground">{breadcrumb}</div> : null}
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid gap-1">
          <h1 className="text-lg font-semibold tracking-[-0.01em] text-foreground">{title}</h1>
          {description ? <p className="text-[13px] text-muted-foreground">{description}</p> : null}
        </div>
        {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
      </div>
    </header>
  );
}
