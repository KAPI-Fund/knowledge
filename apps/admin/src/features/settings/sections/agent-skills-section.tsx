import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

import { getAgentSkill, listAgentSkills, listProjects } from "../../shared/api";

export function AgentSkillsSection() {
  const [projectId, setProjectId] = useState("");
  const [openSkillId, setOpenSkillId] = useState<string | null>(null);

  const projects = useQuery({
    queryKey: ["projects"],
    queryFn: listProjects,
  });

  const skills = useQuery({
    queryKey: ["agent-skills", projectId],
    queryFn: () => listAgentSkills(projectId),
    enabled: Boolean(projectId),
  });

  const skillDetail = useQuery({
    queryKey: ["agent-skill", projectId, openSkillId],
    queryFn: () => getAgentSkill({ projectId, skillId: openSkillId ?? "" }),
    enabled: Boolean(projectId) && Boolean(openSkillId),
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Agent Skills</CardTitle>
        <CardDescription>
          SKILL.md files loaded by the chat agent. Project skills live in{" "}
          <code className="font-mono text-xs">.knowledge/skills/</code> inside the project
          workspace; global skills come from the server-wide directory. A project skill with the
          same id overrides the global one. Editing happens on the file system.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="grid gap-1.5">
          <Label htmlFor="agent-skills-project">Project</Label>
          <Select onValueChange={setProjectId} value={projectId}>
            <SelectTrigger aria-label="Project" className="w-full" id="agent-skills-project">
              <SelectValue placeholder="Select a project" />
            </SelectTrigger>
            <SelectContent>
              {(projects.data ?? []).map((project) => (
                <SelectItem key={project.id} value={project.id}>
                  {project.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {projectId && skills.isLoading ? (
          <p className="text-sm text-muted-foreground">Loading skills...</p>
        ) : null}
        {projectId && skills.isError ? (
          <p className="text-sm text-destructive">
            {skills.error instanceof Error ? skills.error.message : "Failed to load skills."}
          </p>
        ) : null}
        {projectId && skills.data && skills.data.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No skills found for this project. Add SKILL.md files under{" "}
            <code className="font-mono text-xs">.knowledge/skills/</code> or the global skills
            directory.
          </p>
        ) : null}

        {skills.data && skills.data.length > 0 ? (
          <ul className="grid gap-2">
            {skills.data.map((skill) => (
              <li
                className="flex items-start justify-between gap-3 rounded-md border border-border p-3"
                key={skill.id}
              >
                <div className="min-w-0">
                  <p className="flex items-center gap-2 text-sm font-medium">
                    <span className="truncate">{skill.name}</span>
                    <Badge variant={skill.source === "project" ? "secondary" : "outline"}>
                      {skill.source}
                    </Badge>
                  </p>
                  <p className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">
                    {skill.description}
                  </p>
                  <p className="mt-0.5 font-mono text-[10px] text-muted-foreground">{skill.id}</p>
                </div>
                <Button
                  onClick={() => setOpenSkillId(skill.id)}
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  View
                </Button>
              </li>
            ))}
          </ul>
        ) : null}
      </CardContent>

      <Dialog
        onOpenChange={(open) => {
          if (!open) setOpenSkillId(null);
        }}
        open={Boolean(openSkillId)}
      >
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>{skillDetail.data?.name ?? openSkillId}</DialogTitle>
            {skillDetail.data?.description ? (
              <DialogDescription>{skillDetail.data.description}</DialogDescription>
            ) : null}
          </DialogHeader>
          {skillDetail.isLoading ? (
            <p className="text-sm text-muted-foreground">Loading skill...</p>
          ) : null}
          {skillDetail.isError ? (
            <p className="text-sm text-destructive">
              {skillDetail.error instanceof Error
                ? skillDetail.error.message
                : "Failed to load skill."}
            </p>
          ) : null}
          {skillDetail.data ? (
            <ScrollArea className="max-h-96 rounded-md border border-border bg-muted/30">
              <pre className="whitespace-pre-wrap break-words p-3 font-mono text-xs leading-5">
                {skillDetail.data.instructions}
              </pre>
            </ScrollArea>
          ) : null}
        </DialogContent>
      </Dialog>
    </Card>
  );
}
