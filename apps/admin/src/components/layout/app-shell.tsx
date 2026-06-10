import { NavLink, Outlet } from "react-router-dom";

import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export function AppShell() {
  const navItems = [
    { to: "/", label: "Dashboard" },
    { to: "/projects", label: "Projects" },
    { to: "/users", label: "Users" },
    { to: "/settings", label: "Settings" },
  ];

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
          <nav aria-label="System navigation" className="sidebar">
            {navItems.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                className={({ isActive }) =>
                  cn(
                    "rounded-xl px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground",
                    isActive && "bg-accent text-accent-foreground",
                  )
                }
                end={item.to === "/"}
              >
                {item.label}
              </NavLink>
            ))}
          </nav>
        </Card>
        <div className="content flex min-w-0 flex-1 flex-col gap-4">
          <header className="rounded-2xl border border-border/70 bg-card/95 px-5 py-4 shadow-[0_18px_50px_-30px_rgba(15,23,42,0.35)] backdrop-blur">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                  System
                </p>
                <p className="mt-1 text-base font-semibold text-foreground">Route-centered operations workbench</p>
              </div>
              <nav aria-label="Compact navigation" className="flex flex-wrap gap-2 lg:hidden">
                {navItems.map((item) => (
                  <NavLink
                    key={item.to}
                    to={item.to}
                    className={({ isActive }) =>
                      cn(
                        "rounded-lg border border-border px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground",
                        isActive && "bg-accent text-accent-foreground",
                      )
                    }
                    end={item.to === "/"}
                  >
                    {item.label}
                  </NavLink>
                ))}
              </nav>
            </div>
          </header>
          <main className="content min-w-0">
            <Outlet />
          </main>
        </div>
      </div>
    </div>
  );
}
