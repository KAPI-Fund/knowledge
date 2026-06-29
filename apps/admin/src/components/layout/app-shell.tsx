import { Outlet } from "react-router-dom";

import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar";

import { AppSidebar } from "./app-sidebar";

export function AppShell() {
  return (
    <SidebarProvider className="h-svh">
      <AppSidebar />
      <SidebarInset>
        <div className="flex h-12 items-center gap-2 border-b border-border px-4 lg:hidden">
          <SidebarTrigger />
        </div>
        <main className="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto p-4 lg:p-6">
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
