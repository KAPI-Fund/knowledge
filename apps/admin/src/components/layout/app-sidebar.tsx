import { useMatch } from "react-router-dom";

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarRail,
} from "@/components/ui/sidebar";

import { NavMain } from "./nav-main";
import { NavUser } from "./nav-user";
import { SpaceSwitcher } from "./space-switcher";

export function AppSidebar() {
  const nestedMatch = useMatch("/projects/:projectId/*");
  const exactMatch = useMatch("/projects/:projectId");
  const projectMatch = nestedMatch ?? exactMatch;
  const projectId = projectMatch?.params.projectId ?? null;

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SpaceSwitcher />
      </SidebarHeader>
      <SidebarContent>
        <NavMain projectId={projectId} />
      </SidebarContent>
      <SidebarFooter>
        <NavUser />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
