import { Link, useNavigate } from "react-router-dom";
import { useState } from "react";

import { PageSection } from "../../components/layout/page-section";
import { ProjectlessState } from "../../components/layout/projectless-state";
import { Button } from "../../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "../../components/ui/table";
import { Card, CardContent } from "../../components/ui/card";

import { useCreateProjectMutation } from "./mutations";
import { useProjectsQuery } from "./queries";

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
      <PageSection
        actions={
          hasProjects ? <Button onClick={openCreateProject}>Create Project</Button> : undefined
        }
        description="Manage registered knowledge workspaces and jump directly into project operations."
        title="Projects"
      >
        {hasProjects ? (
          <Card>
            <CardContent className="p-0">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Root Path</TableHead>
                    <TableHead>Created</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {projectList.map((project) => (
                    <TableRow key={project.id}>
                      <TableCell className="font-medium">
                        <Link className="inline-link" to={`/projects/${project.id}`}>
                          {project.name}
                        </Link>
                      </TableCell>
                      <TableCell className="text-muted-foreground">{project.rootPath}</TableCell>
                      <TableCell className="text-muted-foreground">{project.createdAt}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        ) : (
          <ProjectlessState onCreateProject={openCreateProject} />
        )}
      </PageSection>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create Project</DialogTitle>
          <DialogDescription>
            Register a project workspace and let the backend create the root structure.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 px-6 pb-6">
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
          <Button variant="outline" onClick={closeCreateProject}>
            Cancel
          </Button>
          <Button onClick={handleCreateProject}>Create Project</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
