import { Search } from "lucide-react";
import type { ReactNode } from "react";

import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

export function FilterToolbar({
  children,
  className,
  onSearchChange,
  searchPlaceholder = "Filter...",
  searchValue,
}: {
  children?: ReactNode;
  className?: string;
  onSearchChange: (value: string) => void;
  searchPlaceholder?: string;
  searchValue: string;
}) {
  return (
    <div className={cn("flex flex-wrap items-center gap-2", className)}>
      <div className="relative max-w-xs flex-1">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          className="pl-8"
          onChange={(event) => onSearchChange(event.target.value)}
          placeholder={searchPlaceholder}
          value={searchValue}
        />
      </div>
      {children}
    </div>
  );
}
