import { Link } from "react-router-dom";

import { EmptyState } from "../../components/layout/empty-state";
import { PageSection } from "../../components/layout/page-section";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../../components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "../../components/ui/table";
import { useSystemSettingsQuery } from "../settings/queries";
import { useProjectsQuery } from "../projects/queries";

export function DashboardPage() {
  const projects = useProjectsQuery();
  const settings = useSystemSettingsQuery();
  const projectList = projects.data ?? [];
  const recentProjects = projectList.slice(0, 5);

  return (
    <PageSection description="Workspace overview" title="Dashboard">
      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader>
            <CardDescription>Projects</CardDescription>
            <CardTitle>{projectList.length}</CardTitle>
          </CardHeader>
        </Card>
        <Card>
          <CardHeader>
            <CardDescription>Language</CardDescription>
            <CardTitle>{settings.data?.language ?? "unknown"}</CardTitle>
          </CardHeader>
        </Card>
        <Card>
          <CardHeader>
            <CardDescription>Default Query Limit</CardDescription>
            <CardTitle>{settings.data?.defaultQueryLimit ?? 0}</CardTitle>
          </CardHeader>
        </Card>
      </div>

      {recentProjects.length ? (
        <Card className="panel">
          <CardHeader>
            <CardTitle>Recent Projects</CardTitle>
            <CardDescription>Jump directly into active workspaces.</CardDescription>
          </CardHeader>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Root Path</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {recentProjects.map((project) => (
                  <TableRow key={project.id}>
                    <TableCell className="font-medium">
                      <Link className="inline-link" to={`/projects/${project.id}`}>
                        {project.name}
                      </Link>
                    </TableCell>
                    <TableCell className="text-muted-foreground">{project.rootPath}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      ) : (
        <EmptyState
          action={
            <Link className="inline-link" to="/projects">
              Open Projects
            </Link>
          }
          description="Create a project before importing sources or running retrieval workflows."
          title="Workspace not configured"
        />
      )}

      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Workspace Settings</CardTitle>
            <CardDescription>Current provider and query defaults.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-2 text-sm text-muted-foreground">
            <p>{settings.data?.providerMode ?? "unknown"}</p>
            <p>{settings.data?.language ?? "unknown"}</p>
            <p>{settings.data?.defaultQueryLimit ?? 0}</p>
            <Link className="inline-link" to="/settings">
              Settings
            </Link>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Entry Points</CardTitle>
            <CardDescription>Start from the workspace list or system controls.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-2 text-sm">
            <Link className="inline-link" to="/projects">
              Projects
            </Link>
            <Link className="inline-link" to="/settings">
              Settings
            </Link>
          </CardContent>
        </Card>
      </div>
    </PageSection>
  );
}
