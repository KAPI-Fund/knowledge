import { NavLink } from "react-router-dom";

import { systemRoutes } from "@/lib/route-meta";
import { cn } from "@/lib/utils";

export function SystemSidebar() {
  return (
    <nav aria-label="System navigation" className="sidebar">
      {systemRoutes.map((item) => (
        <NavLink
          key={item.to}
          className={({ isActive }) =>
            cn(
              "rounded-xl px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground",
              isActive && "bg-accent text-accent-foreground",
            )
          }
          end={item.to === "/"}
          to={item.to}
        >
          {item.label}
        </NavLink>
      ))}
    </nav>
  );
}
