import { ChevronLeft } from "lucide-react";
import { Link, useLocation } from "react-router-dom";

import {
  SidebarGroup,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import { useProjectDetailQuery } from "@/features/projects/detail-queries";
import { globalNav, projectNavGroups } from "@/lib/route-meta";

function isPathActive(pathname: string, to: string, end: boolean) {
  if (end) return pathname === to;
  return pathname === to || pathname.startsWith(`${to}/`);
}

export function NavMain({ projectId }: { projectId: string | null }) {
  if (projectId) return <ProjectNav projectId={projectId} />;
  return <GlobalNav />;
}

function GlobalNav() {
  const { pathname } = useLocation();
  return (
    <SidebarGroup>
      <SidebarGroupLabel>Platform</SidebarGroupLabel>
      <SidebarMenu>
        {globalNav.map((item) => (
          <SidebarMenuItem key={item.to}>
            <SidebarMenuButton
              asChild
              isActive={isPathActive(pathname, item.to, item.to === "/")}
              tooltip={item.label}
            >
              <Link to={item.to}>
                <item.icon />
                <span>{item.label}</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        ))}
      </SidebarMenu>
    </SidebarGroup>
  );
}

function ProjectNav({ projectId }: { projectId: string }) {
  const { pathname } = useLocation();
  const project = useProjectDetailQuery(projectId);
  const projectName = project.data?.project.name ?? "Project";

  return (
    <>
      <SidebarGroup>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton asChild tooltip="All projects">
              <Link className="text-muted-foreground" to="/projects">
                <ChevronLeft />
                <span>All projects</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        <SidebarGroupLabel className="mt-1 truncate text-sm font-semibold text-sidebar-foreground">
          {projectName}
        </SidebarGroupLabel>
      </SidebarGroup>
      {projectNavGroups.map((group, index) => (
        <SidebarGroup key={group.label ?? `group-${index}`}>
          {group.label ? <SidebarGroupLabel>{group.label}</SidebarGroupLabel> : null}
          <SidebarMenu>
            {group.items.map((item) => {
              const to = `/projects/${projectId}${item.suffix}`;
              return (
                <SidebarMenuItem key={to}>
                  <SidebarMenuButton
                    asChild
                    isActive={isPathActive(pathname, to, item.suffix === "")}
                    tooltip={item.label}
                  >
                    <Link to={to}>
                      <item.icon />
                      <span>{item.label}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              );
            })}
          </SidebarMenu>
        </SidebarGroup>
      ))}
    </>
  );
}
