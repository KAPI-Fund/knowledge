import type { ColumnDef } from "@tanstack/react-table";
import {
  ArrowRight,
  Cpu,
  FolderKanban,
  Globe,
  KeyRound,
  ListFilter,
  Settings2,
} from "lucide-react";
import { useMemo } from "react";
import { Link } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { formatDate } from "@/lib/format";

import { useProjectsQuery } from "../projects/queries";
import { useSystemSettingsQuery } from "../settings/queries";

type ProjectRow = NonNullable<ReturnType<typeof useProjectsQuery>["data"]>[number];

function StatCard({
  icon: Icon,
  label,
  value,
  isLoading,
}: {
  icon: typeof FolderKanban;
  label: string;
  value: string | number;
  isLoading?: boolean;
}) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardDescription className="text-sm font-medium text-foreground">{label}</CardDescription>
        <Icon className="size-4 text-muted-foreground" />
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <Skeleton className="h-8 w-24" />
        ) : (
          <p className="truncate text-2xl font-semibold tracking-tight">{value}</p>
        )}
      </CardContent>
    </Card>
  );
}

const quickActions = [
  {
    to: "/projects",
    icon: FolderKanban,
    title: "Projects",
    description: "Create and manage knowledge projects.",
  },
  {
    to: "/settings",
    icon: Settings2,
    title: "Settings",
    description: "Configure providers and query defaults.",
  },
  {
    to: "/api-tokens",
    icon: KeyRound,
    title: "API Tokens",
    description: "Mint tokens for programmatic access.",
  },
];

export function DashboardPage() {
  const projects = useProjectsQuery();
  const settings = useSystemSettingsQuery();
  const projectList = projects.data ?? [];
  const recentProjects = projectList.slice(0, 5);
  const activeConnection = settings.data?.connections?.find((c) => c.isActive);

  const columns = useMemo<ColumnDef<ProjectRow>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => (
          <Link
            className="font-medium text-foreground underline-offset-4 hover:underline"
            to={`/projects/${row.original.id}`}
          >
            {row.original.name}
          </Link>
        ),
      },
      {
        accessorKey: "rootPath",
        header: "Root Path",
        cell: ({ row }) => (
          <span className="font-mono text-[11px] text-muted-foreground">{row.original.rootPath}</span>
        ),
      },
      {
        accessorKey: "createdAt",
        header: "Created",
        cell: ({ row }) => (
          <span className="text-muted-foreground">{formatDate(row.original.createdAt)}</span>
        ),
      },
    ],
    [],
  );

  return (
    <div className="grid gap-6">
      <PageHeader description="Workspace overview" title="Dashboard" />

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <StatCard
          icon={FolderKanban}
          isLoading={projects.isLoading}
          label="Projects"
          value={projectList.length}
        />
        <StatCard
          icon={Cpu}
          isLoading={settings.isLoading}
          label="Active LLM"
          value={activeConnection?.label ?? "Not configured"}
        />
        <StatCard
          icon={Globe}
          isLoading={settings.isLoading}
          label="Language"
          value={settings.data?.defaults?.language ?? "unknown"}
        />
        <StatCard
          icon={ListFilter}
          isLoading={settings.isLoading}
          label="Default Query Limit"
          value={settings.data?.defaults?.defaultQueryLimit ?? 0}
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardHeader className="flex flex-row items-center justify-between space-y-0">
            <div className="grid gap-1">
              <CardTitle>Recent Projects</CardTitle>
              <CardDescription>The latest projects in this workspace.</CardDescription>
            </div>
            <Button asChild size="sm" variant="ghost">
              <Link to="/projects">
                View all
                <ArrowRight />
              </Link>
            </Button>
          </CardHeader>
          <CardContent>
            {recentProjects.length || projects.isLoading ? (
              <DataTable columns={columns} data={recentProjects} isLoading={projects.isLoading} />
            ) : (
              <EmptyState
                action={
                  <Button asChild size="sm">
                    <Link to="/projects">Create a project</Link>
                  </Button>
                }
                description="Create a project before importing sources or running retrieval workflows."
                icon={FolderKanban}
                title="No projects yet"
              />
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Quick actions</CardTitle>
            <CardDescription>Jump into common workflows.</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-2">
            {quickActions.map((action) => (
              <Link
                className="group flex items-start gap-3 rounded-lg border border-border p-3 transition-colors hover:bg-muted/50"
                key={action.to}
                to={action.to}
              >
                <div className="flex size-8 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
                  <action.icon className="size-4" />
                </div>
                <div className="grid gap-0.5">
                  <p className="text-sm font-medium leading-none">{action.title}</p>
                  <p className="text-xs text-muted-foreground">{action.description}</p>
                </div>
                <ArrowRight className="ml-auto size-4 self-center text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />
              </Link>
            ))}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
