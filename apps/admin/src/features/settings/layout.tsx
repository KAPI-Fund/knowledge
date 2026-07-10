import { NavLink, Outlet } from "react-router-dom";

import { PageHeader } from "@/components/shared/page-header";
import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export const settingsSections = [
  { to: "llm", label: "LLM" },
  { to: "embedding", label: "Embedding" },
  { to: "image", label: "Image" },
  { to: "search", label: "Web Search" },
  { to: "fetch", label: "Web Fetch" },
  { to: "defaults", label: "Defaults" },
];

export function SettingsLayout() {
  return (
    <div className="grid gap-6">
      <PageHeader
        description="Configure model providers, embeddings, image generation, and web search."
        title="Settings"
      />
      <div className="grid items-start gap-6 lg:grid-cols-[200px_1fr]">
        <nav
          aria-label="Settings sections"
          className="flex gap-1 overflow-x-auto lg:flex-col lg:overflow-visible"
        >
          {settingsSections.map((section) => (
            <NavLink
              className={({ isActive }) =>
                cn(
                  buttonVariants({ variant: "ghost", size: "sm" }),
                  "justify-start",
                  isActive
                    ? "bg-muted hover:bg-muted"
                    : "text-muted-foreground hover:text-foreground",
                )
              }
              key={section.to}
              to={section.to}
            >
              {section.label}
            </NavLink>
          ))}
        </nav>
        <div className="min-w-0">
          <Outlet />
        </div>
      </div>
    </div>
  );
}
