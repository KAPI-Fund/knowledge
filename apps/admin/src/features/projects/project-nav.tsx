import { NavLink } from "react-router-dom";

import { projectRoutes } from "@/lib/route-meta";
import { cn } from "@/lib/utils";

export function ProjectNav({ projectId }: { projectId: string }) {
  return (
    <nav aria-label="Project navigation" className="subnav">
      {projectRoutes.map((route) => {
        const to = `/projects/${projectId}${route.suffix}`;
        return (
          <NavLink
            key={to}
            className={({ isActive }) =>
              cn(
                "inline-flex items-center rounded-lg border border-border px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground",
                isActive && "bg-accent text-accent-foreground",
              )
            }
            end={!route.suffix}
            role="tab"
            to={to}
          >
            {route.label}
          </NavLink>
        );
      })}
    </nav>
  );
}
