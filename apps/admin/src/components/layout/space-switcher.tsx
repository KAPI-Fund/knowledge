import { Building2, ChevronsUpDown, LibraryBig, Plus, User } from "lucide-react";
import { useState } from "react";
import { useMatch, useNavigate } from "react-router-dom";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import { CreateOrgDialog } from "@/features/spaces/create-org-dialog";
import { useSpacesQuery } from "@/features/spaces/use-spaces";

export function SpaceSwitcher() {
  const navigate = useNavigate();
  const nestedOrgMatch = useMatch("/orgs/:orgId/*");
  const exactOrgMatch = useMatch("/orgs/:orgId");
  const activeOrgId = (nestedOrgMatch ?? exactOrgMatch)?.params.orgId ?? null;
  const spaces = useSpacesQuery();
  const orgs = spaces.data?.orgs ?? [];
  const activeOrg = orgs.find((org) => org.id === activeOrgId) ?? null;
  const [createOrgOpen, setCreateOrgOpen] = useState(false);

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <SidebarMenuButton
              size="lg"
              className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
            >
              <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                <LibraryBig className="size-4" />
              </div>
              <div className="grid flex-1 text-left text-sm leading-tight">
                <span className="truncate font-semibold">Knowledge</span>
                <span className="truncate text-xs text-muted-foreground">
                  {activeOrg ? activeOrg.name : "Personal"}
                </span>
              </div>
              <ChevronsUpDown className="ml-auto size-4" />
            </SidebarMenuButton>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="start"
            side="bottom"
            sideOffset={4}
            className="w-(--radix-dropdown-menu-trigger-width) min-w-56 rounded-lg"
          >
            <DropdownMenuLabel className="text-xs text-muted-foreground">
              Spaces
            </DropdownMenuLabel>
            <DropdownMenuItem className="gap-2 p-2" onSelect={() => navigate("/projects")}>
              <div className="flex size-6 items-center justify-center rounded-md border">
                <User className="size-3.5 shrink-0" />
              </div>
              Personal
            </DropdownMenuItem>
            {orgs.map((org) => (
              <DropdownMenuItem
                className="gap-2 p-2"
                key={org.id}
                onSelect={() => navigate(`/orgs/${org.id}`)}
              >
                <div className="flex size-6 items-center justify-center rounded-md border">
                  <Building2 className="size-3.5 shrink-0" />
                </div>
                {org.name}
              </DropdownMenuItem>
            ))}
            <DropdownMenuSeparator />
            <DropdownMenuItem className="gap-2 p-2" onSelect={() => setCreateOrgOpen(true)}>
              <div className="flex size-6 items-center justify-center rounded-md border bg-transparent">
                <Plus className="size-3.5" />
              </div>
              <span className="font-medium text-muted-foreground">New organization</span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
      <CreateOrgDialog onOpenChange={setCreateOrgOpen} open={createOrgOpen} />
    </SidebarMenu>
  );
}
