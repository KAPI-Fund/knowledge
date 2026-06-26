import { ChevronLeft, Plus } from "lucide-react";
import { useState } from "react";
import { Link, NavLink, useMatch } from "react-router-dom";

import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import { useSession } from "@/features/auth/use-session";
import { useProjectDetailQuery } from "@/features/projects/detail-queries";
import { CreateOrgDialog } from "@/features/spaces/create-org-dialog";
import { useSpacesQuery } from "@/features/spaces/use-spaces";
import { globalNav, projectNavGroups } from "@/lib/route-meta";
import { cn } from "@/lib/utils";

function activeNavClass(isActive: boolean) {
  return cn(
    isActive &&
      "bg-sidebar-primary text-sidebar-primary-foreground hover:bg-sidebar-primary hover:text-sidebar-primary-foreground",
  );
}

export function AppSidebar() {
  const nestedMatch = useMatch("/projects/:projectId/*");
  const exactMatch = useMatch("/projects/:projectId");
  const projectMatch = nestedMatch ?? exactMatch;
  const projectId = projectMatch?.params.projectId ?? "";
  const isProjectContext = Boolean(projectId);

  return (
    <Sidebar>
      {isProjectContext ? (
        <ProjectSidebarHeader projectId={projectId} />
      ) : (
        <SidebarHeader>
          <div className="px-2 py-1.5">
            <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground">
              Knowledge
            </p>
            <p className="text-sm font-semibold text-foreground">Admin Console</p>
          </div>
        </SidebarHeader>
      )}

      <SidebarContent>
        {isProjectContext ? (
          projectNavGroups.map((group, index) => (
            <SidebarGroup key={group.label ?? `group-${index}`}>
              {group.label ? <SidebarGroupLabel>{group.label}</SidebarGroupLabel> : null}
              <SidebarMenu>
                {group.items.map((item) => {
                  const to = `/projects/${projectId}${item.suffix}`;
                  return (
                    <SidebarMenuItem key={to}>
                      <SidebarMenuButton asChild>
                        <NavLink
                          end={item.suffix === ""}
                          to={to}
                          className={({ isActive }) => activeNavClass(isActive)}
                        >
                          {item.label}
                        </NavLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  );
                })}
              </SidebarMenu>
            </SidebarGroup>
          ))
        ) : (
          <SidebarGroup>
            <SidebarMenu>
              {globalNav.map((item) => (
                <SidebarMenuItem key={item.to}>
                  <SidebarMenuButton asChild>
                    <NavLink
                      end={item.to === "/"}
                      to={item.to}
                      className={({ isActive }) => activeNavClass(isActive)}
                    >
                      {item.label}
                    </NavLink>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroup>
        )}
      </SidebarContent>

      <SidebarFooter>
        <UserChip />
      </SidebarFooter>
    </Sidebar>
  );
}

function ProjectSidebarHeader({ projectId }: { projectId: string }) {
  const project = useProjectDetailQuery(projectId);
  const detail = project.data?.project;
  return (
    <SidebarHeader className="gap-2">
      <Link
        to="/projects"
        className="flex items-center gap-1 px-2 text-[11px] font-semibold text-muted-foreground hover:text-foreground"
      >
        <ChevronLeft className="size-3" />
        All projects
      </Link>
      <div className="px-2">
        <p className="text-sm font-semibold text-foreground">{detail?.name ?? "Project"}</p>
        <p className="font-mono text-[10px] text-muted-foreground">{projectId}</p>
      </div>
    </SidebarHeader>
  );
}

function UserChip() {
  const { user } = useSession();
  const spaces = useSpacesQuery();
  const orgs = spaces.data?.orgs ?? [];
  const label = user?.username ?? "Account";
  const initial = label.slice(0, 1).toUpperCase();
  const [createOrgOpen, setCreateOrgOpen] = useState(false);

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <SidebarMenuButton className="gap-2">
              <Avatar className="size-6">
                <AvatarFallback className="bg-sidebar-primary text-[11px] text-sidebar-primary-foreground">
                  {initial}
                </AvatarFallback>
              </Avatar>
              <span className="truncate">{label}</span>
            </SidebarMenuButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-56" side="top">
            <DropdownMenuLabel>Spaces</DropdownMenuLabel>
            <DropdownMenuItem asChild>
              <Link to="/projects">Personal</Link>
            </DropdownMenuItem>
            {orgs.map((org) => (
              <DropdownMenuItem asChild key={org.id}>
                <Link to={`/orgs/${org.id}`}>{org.name}</Link>
              </DropdownMenuItem>
            ))}
            <DropdownMenuItem onSelect={() => setCreateOrgOpen(true)}>
              <Plus className="size-4" />
              New organization
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem asChild>
              <Link to="/settings">Settings</Link>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
      <CreateOrgDialog onOpenChange={setCreateOrgOpen} open={createOrgOpen} />
    </SidebarMenu>
  );
}
