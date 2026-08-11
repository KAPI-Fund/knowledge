import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

export function PageHeader({
  actions,
  className,
  description,
  title,
}: {
  actions?: ReactNode;
  className?: string;
  description?: string;
  title: string;
}) {
  return (
    <header className={cn("flex flex-wrap items-start justify-between gap-3", className)}>
      <div className="grid gap-1">
        <h1 className="text-2xl font-semibold tracking-tight text-foreground">{title}</h1>
        {description ? <p className="text-sm text-muted-foreground">{description}</p> : null}
      </div>
      {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
    </header>
  );
}
