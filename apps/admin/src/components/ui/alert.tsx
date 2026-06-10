import type { HTMLAttributes } from "react";

import { cn } from "@/lib/utils";

export function Alert({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("rounded-xl border border-border bg-card px-4 py-3 text-sm text-card-foreground", className)}
      role="alert"
      {...props}
    />
  );
}

export function AlertTitle({ className, ...props }: HTMLAttributes<HTMLHeadingElement>) {
  return <h5 className={cn("font-medium", className)} {...props} />;
}

export function AlertDescription({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("mt-1 text-sm text-muted-foreground", className)} {...props} />;
}
