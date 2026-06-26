import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";

import { ProjectlessState } from "@/components/layout/projectless-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

import { useCreateProjectMutation } from "./mutations";
import { useProjectsQuery } from "./queries";

type ProjectRow = NonNullable<ReturnType<typeof useProjectsQuery>["data"]>[number];

export function ProjectsPage() {
  const navigate = useNavigate();
  const projects = useProjectsQuery();
  const createProject = useCreateProjectMutation();
  const [createOpen, setCreateOpen] = useState(false);
  const [name, setName] = useState("");
  const [errorMessage, setErrorMessage] = useState("");

  const projectList = projects.data ?? [];
  const hasProjects = projectList.length > 0;

  function openCreateProject() {
    setName("");
    setErrorMessage("");
    setCreateOpen(true);
  }

  function closeCreateProject() {
    setCreateOpen(false);
    setName("");
    setErrorMessage("");
  }

  async function handleCreateProject() {
    const trimmedName = name.trim();
    if (!trimmedName) {
      setErrorMessage("Name is required.");
      return;
    }
    try {
      const project = await createProject.mutateAsync({
        name: trimmedName,
        csrfToken: window.sessionStorage.getItem("knowledge.csrfToken") ?? "",
      });
      closeCreateProject();
      navigate(`/projects/${project.id}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create project.");
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
          <span className="text-sm text-muted-foreground">{row.original.createdAt}</span>
        ),
      },
    ],
    [],
  );

  return (
    <Dialog
      onOpenChange={(open) => {
        setCreateOpen(open);
        if (!open) {
          setName("");
          setErrorMessage("");
        }
      }}
      open={createOpen}
    >
      <div className="grid gap-6">
        <PageHeader
          actions={hasProjects ? <Button onClick={openCreateProject}>Create Project</Button> : undefined}
          description="Manage registered knowledge workspaces and jump directly into project operations."
          title="Projects"
        />
        {hasProjects ? (
          <DataTable columns={columns} data={projectList} isLoading={projects.isLoading} />
        ) : (
          <ProjectlessState onCreateProject={openCreateProject} />
        )}
      </div>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create Project</DialogTitle>
          <DialogDescription>
            Register a project workspace and let the backend create the root structure.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4">
          <label className="grid gap-2 text-sm font-medium">
            Name
            <Input
              onChange={(event) => setName(event.target.value)}
              placeholder="research-notes"
              value={name}
            />
          </label>
          {errorMessage ? <p className="text-sm text-destructive">{errorMessage}</p> : null}
        </div>
        <DialogFooter>
          <Button onClick={closeCreateProject} variant="outline">
            Cancel
          </Button>
          <Button onClick={handleCreateProject}>Create Project</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
