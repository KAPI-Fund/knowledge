import { zodResolver } from "@hookform/resolvers/zod";
import type { ColumnDef } from "@tanstack/react-table";
import { Plus } from "lucide-react";
import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { Link, useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { z } from "zod";

import { ProjectlessState } from "@/components/layout/projectless-state";
import { DataTable, DataTableRowActions } from "@/components/shared/data-table";
import { FormDialog } from "@/components/shared/form-dialog";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import { DropdownMenuItem } from "@/components/ui/dropdown-menu";
import {
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { formatDate } from "@/lib/format";

import { getCsrfToken } from "@/features/auth/csrf";

import { useCreateProjectMutation } from "./mutations";
import { useProjectsQuery } from "./queries";

type ProjectRow = NonNullable<ReturnType<typeof useProjectsQuery>["data"]>[number];

const createProjectSchema = z.object({
  name: z.string().trim().min(1, "Name is required."),
});

type CreateProjectValues = z.infer<typeof createProjectSchema>;

export function ProjectsPage() {
  const navigate = useNavigate();
  const projects = useProjectsQuery();
  const createProject = useCreateProjectMutation();
  const [createOpen, setCreateOpen] = useState(false);

  const projectList = projects.data ?? [];
  const hasProjects = projectList.length > 0;

  const form = useForm<CreateProjectValues>({
    resolver: zodResolver(createProjectSchema),
    defaultValues: { name: "" },
  });

  function openCreateProject() {
    form.reset({ name: "" });
    setCreateOpen(true);
  }

  async function onSubmit(values: CreateProjectValues) {
    try {
      const project = await createProject.mutateAsync({
        name: values.name,
        csrfToken: getCsrfToken(),
      });
      setCreateOpen(false);
      toast.success(`Project "${project.name}" created.`);
      navigate(`/projects/${project.id}`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create project.");
    }
  }

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
      {
        id: "actions",
        cell: ({ row }) => (
          <DataTableRowActions>
            <DropdownMenuItem onSelect={() => navigate(`/projects/${row.original.id}`)}>
              Open
            </DropdownMenuItem>
            <DropdownMenuItem
              onSelect={() => {
                void navigator.clipboard.writeText(row.original.id);
                toast.success("Project ID copied.");
              }}
            >
              Copy ID
            </DropdownMenuItem>
          </DataTableRowActions>
        ),
      },
    ],
    [navigate],
  );

  return (
    <div className="grid gap-6">
      <PageHeader
        actions={
          hasProjects ? (
            <Button onClick={openCreateProject}>
              <Plus />
              Create Project
            </Button>
          ) : undefined
        }
        description="Manage registered knowledge workspaces and jump directly into project operations."
        title="Projects"
      />
      {hasProjects || projects.isLoading ? (
        <DataTable
          columns={columns}
          data={projectList}
          isLoading={projects.isLoading}
          searchKey="name"
          searchPlaceholder="Filter projects..."
        />
      ) : (
        <ProjectlessState onCreateProject={openCreateProject} />
      )}
      <FormDialog
        description="Register a project workspace and let the backend create the root structure."
        form={form}
        isPending={createProject.isPending}
        onOpenChange={setCreateOpen}
        onSubmit={onSubmit}
        open={createOpen}
        submitLabel="Create Project"
        title="Create Project"
      >
        <FormField
          control={form.control}
          name="name"
          render={({ field }) => (
            <FormItem>
              <FormLabel>Name</FormLabel>
              <FormControl>
                <Input placeholder="research-notes" {...field} />
              </FormControl>
              <FormMessage />
            </FormItem>
          )}
        />
      </FormDialog>
    </div>
  );
}
