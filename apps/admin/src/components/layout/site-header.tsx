import { Separator } from "@/components/ui/separator";
import { SidebarTrigger } from "@/components/ui/sidebar";

import { AppBreadcrumbs } from "./app-breadcrumbs";
import { CommandMenu } from "./command-menu";
import { ModeToggle } from "./mode-toggle";

export function SiteHeader() {
  return (
    <header className="sticky top-0 z-20 flex h-14 shrink-0 items-center gap-2 border-b border-border bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80">
      <div className="flex w-full items-center gap-2 px-4">
        <SidebarTrigger className="-ml-1" />
        <Separator orientation="vertical" className="mr-1 data-[orientation=vertical]:h-4" />
        <AppBreadcrumbs />
        <div className="ml-auto flex items-center gap-2">
          <CommandMenu />
          <ModeToggle />
        </div>
      </div>
    </header>
  );
}
