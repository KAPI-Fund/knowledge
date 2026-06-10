import { Link, useNavigate } from "react-router-dom";
import { useState } from "react";

import { PageSection } from "../../components/layout/page-section";
import { ProjectlessState } from "../../components/layout/projectless-state";
import { Button } from "../../components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
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
  const [rootPath, setRootPath] = useState("");
  const [errorMessage, setErrorMessage] = useState("");

  async function handleCreateProject() {
    const trimmedName = name.trim();
    const trimmedRootPath = rootPath.trim();

    if (!trimmedName || !trimmedRootPath) {
      setErrorMessage("Name and root path are required.");
      return;
    }

    try {
      const project = await createProject.mutateAsync({
        name: trimmedName,
        rootPath: trimmedRootPath,
        csrfToken: window.sessionStorage.getItem("knowledge.csrfToken") ?? "",
      });
      setCreateOpen(false);
      setName("");
      setRootPath("");
      setErrorMessage("");
      navigate(`/projects/${project.id}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create project.");
    }
  }

  return (
    <PageSection
      actions={
        <Dialog onOpenChange={setCreateOpen} open={createOpen}>
          <DialogTrigger>
            <Button>Create Project</Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Create Project</DialogTitle>
              <DialogDescription>
                Register a project root and open the workbench when creation succeeds.
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
              <label className="grid gap-2 text-sm font-medium">
                Root Path
                <Input
                  onChange={(event) => setRootPath(event.target.value)}
                  placeholder="E:/Projects/Js/knowledge/.e2e/research-notes"
                  value={rootPath}
                />
              </label>
              {errorMessage ? <p className="text-sm text-destructive">{errorMessage}</p> : null}
            </div>
            <DialogFooter>
              <DialogClose>
                <Button variant="outline">Cancel</Button>
              </DialogClose>
              <Button onClick={handleCreateProject}>Create Project</Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      }
      description="Manage registered knowledge workspaces and jump directly into project operations."
      title="Projects"
    >
      {projects.data?.length ? (
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
                {projects.data.map((project) => (
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
        <ProjectlessState onCreateProject={() => setCreateOpen(true)} />
      )}
    </PageSection>
  );
}
