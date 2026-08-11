import { MoreHorizontal } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

export function DataTableRowActions({
  label = "Open menu",
  children,
}: {
  label?: string;
  children: ReactNode;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          aria-label={label}
          className="size-8 text-muted-foreground data-[state=open]:bg-muted"
          size="icon"
          variant="ghost"
        >
          <MoreHorizontal />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-40">
        {children}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
