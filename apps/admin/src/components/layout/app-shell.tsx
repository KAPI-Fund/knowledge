import { Outlet, useLocation } from "react-router-dom";

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { PendingSaveBanner } from "@/features/canvas/pending-save-banner";
import { cn } from "@/lib/utils";

import { AppSidebar } from "./app-sidebar";
import { SiteHeader } from "./site-header";

export function AppShell() {
  const { pathname } = useLocation();
  // The canvas is a full-bleed three-pane workspace; the default page padding
  // and vertical scroll would box it in and leave dead space above and below.
  const fullBleed = pathname === "/canvas" || pathname.startsWith("/canvas/");
  return (
    <SidebarProvider className="h-svh">
      <AppSidebar />
      <SidebarInset>
        <SiteHeader />
        <PendingSaveBanner />
        <main
          className={cn(
            "flex min-h-0 min-w-0 flex-1 flex-col",
            fullBleed ? "overflow-hidden" : "overflow-y-auto p-4 lg:p-6",
          )}
        >
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
