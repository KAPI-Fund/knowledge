import { Fragment } from "react";
import { Link, useLocation } from "react-router-dom";

import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { useProjectDetailQuery } from "@/features/projects/detail-queries";
import { useSpacesQuery } from "@/features/spaces/use-spaces";
import { routeLabel } from "@/lib/route-meta";

type Crumb = { label: string; to: string };

export function AppBreadcrumbs() {
  const { pathname } = useLocation();
  const segments = pathname.split("/").filter(Boolean);

  const projectId = segments[0] === "projects" && segments[1] ? segments[1] : "";
  const orgId = segments[0] === "orgs" && segments[1] ? segments[1] : "";
  const teamId = segments[0] === "orgs" && segments[2] === "teams" && segments[3] ? segments[3] : "";

  const project = useProjectDetailQuery(projectId);
  const spaces = useSpacesQuery();

  const crumbs: Crumb[] = [];
  let path = "";
  for (const [index, segment] of segments.entries()) {
    path += `/${segment}`;
    let label: string | null = routeLabel(segment);
    if (index === 1 && segments[0] === "projects") {
      label = project.data?.project.name ?? segment;
    } else if (index === 1 && segments[0] === "orgs") {
      label = spaces.data?.orgs.find((org) => org.id === segment)?.name ?? segment;
    } else if (index === 3 && teamId && segment === teamId) {
      label = spaces.data?.teams.find((team) => team.id === segment)?.name ?? segment;
    }
    if (!label) label = segment;
    crumbs.push({ label, to: path });
  }

  if (crumbs.length === 0) {
    crumbs.push({ label: "Dashboard", to: "/" });
  }

  return (
    <Breadcrumb>
      <BreadcrumbList>
        {crumbs.map((crumb, index) => {
          const isLast = index === crumbs.length - 1;
          return (
            <Fragment key={crumb.to}>
              {index > 0 ? <BreadcrumbSeparator className="hidden md:block" /> : null}
              <BreadcrumbItem className={isLast ? undefined : "hidden md:block"}>
                {isLast ? (
                  <BreadcrumbPage className="max-w-48 truncate">{crumb.label}</BreadcrumbPage>
                ) : (
                  <BreadcrumbLink asChild>
                    <Link className="max-w-40 truncate" to={crumb.to}>
                      {crumb.label}
                    </Link>
                  </BreadcrumbLink>
                )}
              </BreadcrumbItem>
            </Fragment>
          );
        })}
      </BreadcrumbList>
    </Breadcrumb>
  );
}
