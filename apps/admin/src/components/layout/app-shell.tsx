import { NavLink, Outlet } from "react-router-dom";

import { Card } from "@/components/ui/card";
import { SystemSidebar } from "@/components/layout/system-sidebar";
import { TopBar } from "@/components/layout/top-bar";

export function AppShell() {
  return (
    <div className="shell">
      <div className="mx-auto flex min-h-screen w-full max-w-[1600px] gap-6 px-4 py-4 lg:px-6">
        <Card className="hidden w-72 shrink-0 p-3 lg:flex lg:flex-col">
          <div className="mb-4 rounded-2xl bg-primary px-4 py-4 text-primary-foreground">
            <p className="text-xs font-semibold uppercase tracking-[0.16em] text-primary-foreground/80">
              Knowledge
            </p>
            <h1 className="mt-2 text-xl font-semibold">Admin Console</h1>
          </div>
          <SystemSidebar />
        </Card>
        <div className="content flex min-w-0 flex-1 flex-col gap-4">
          <TopBar />
          <main className="content min-w-0">
            <Outlet />
          </main>
        </div>
      </div>
    </div>
  );
}
