import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Button } from "../../components/ui/button";
import { useSpacesQuery } from "../spaces/use-spaces";
import { useOrgProjectsQuery, useOrgTeamsQuery } from "./workspace-queries";
import { CreatePublicProjectDialog } from "./create-public-project-dialog";
import { CreateTeamDialog } from "./create-team-dialog";
import { ManageAccessDialog } from "../kb-access/manage-access-dialog";

export function OrgWorkspacePage() {
  const { orgId = "" } = useParams();
  const spaces = useSpacesQuery();
  const org = spaces.data?.orgs.find((entry) => entry.id === orgId);
  const orgSpaceId = org?.spaceId ?? "";

  const projects = useOrgProjectsQuery(orgSpaceId);
  const teams = useOrgTeamsQuery(orgId);

  const [publicDialogOpen, setPublicDialogOpen] = useState(false);
  const [teamDialogOpen, setTeamDialogOpen] = useState(false);
  const [newKbTeamSpaceId, setNewKbTeamSpaceId] = useState<string | null>(null);
  const [manageProjectId, setManageProjectId] = useState<string | null>(null);
  const [manageTeamId, setManageTeamId] = useState<string | null>(null);

  if (spaces.isLoading) return <p>Loading…</p>;
  if (!org) return <p>Access denied or organization not found.</p>;

  const isAdmin = org.role === "org_admin";
  const allProjects = projects.data ?? [];
  const publicProjects = allProjects.filter((p) => p.spaceKind === "org");
  const teamList = teams.data?.teams ?? [];

  const leaderTeamIds = new Set(
    (spaces.data?.teams ?? [])
      .filter((t) => t.orgId === orgId && t.role === "leader")
      .map((t) => t.id),
  );

  const openManage = (projectId: string, teamId: string | null) => {
    setManageProjectId(projectId);
    setManageTeamId(teamId);
  };

  return (
    <div className="flex flex-col gap-6">
      <header className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">{org.name} workspace</h1>
        {isAdmin ? (
          <div className="flex items-center gap-2">
            <Button type="button" onClick={() => setPublicDialogOpen(true)}>
              New public project
            </Button>
            <Link to={`/orgs/${orgId}/members`}>Members</Link>
            <Button type="button" onClick={() => setTeamDialogOpen(true)}>
              New team
            </Button>
          </div>
        ) : (
          <Button type="button" onClick={() => setTeamDialogOpen(true)}>
            New team
          </Button>
        )}
      </header>

      <section>
        <h2 className="text-sm font-medium">Public projects</h2>
        {publicProjects.length === 0 ? (
          <p className="text-sm text-muted-foreground">No public projects yet.</p>
        ) : (
          <ul className="flex flex-col gap-1">
            {publicProjects.map((project) => (
              <li key={project.id} className="flex items-center justify-between">
                <Link to={`/projects/${project.id}`}>{project.name}</Link>
                {isAdmin ? (
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => openManage(project.id, null)}
                  >
                    Manage access
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </section>

      {teamList.map((team) => {
        const teamProjects = allProjects.filter((p) => p.teamId === team.id);
        const canManageTeam = isAdmin || leaderTeamIds.has(team.id);
        const teamSpaceId = (spaces.data?.teams ?? []).find((t) => t.id === team.id)?.spaceId ?? "";
        return (
          <section key={team.id}>
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-medium">Team · {team.name}</h2>
              <div className="flex items-center gap-2">
                {canManageTeam && teamSpaceId ? (
                  <Button
                    type="button"
                    onClick={() => setNewKbTeamSpaceId(teamSpaceId)}
                  >
                    New team KB
                  </Button>
                ) : null}
                <Link to={`/orgs/${orgId}/teams/${team.id}`}>Manage</Link>
              </div>
            </div>
            {teamProjects.length === 0 ? (
              <p className="text-sm text-muted-foreground">No team KBs yet.</p>
            ) : (
              <ul className="flex flex-col gap-1">
                {teamProjects.map((project) => (
                  <li key={project.id} className="flex items-center justify-between">
                    <Link to={`/projects/${project.id}`}>{project.name}</Link>
                    {canManageTeam ? (
                      <Button
                        type="button"
                        variant="outline"
                        onClick={() => openManage(project.id, team.id)}
                      >
                        Manage access
                      </Button>
                    ) : null}
                  </li>
                ))}
              </ul>
            )}
          </section>
        );
      })}

      <CreatePublicProjectDialog
        targetSpaceId={orgSpaceId}
        listSpaceId={orgSpaceId}
        title="New public project"
        open={publicDialogOpen}
        onOpenChange={setPublicDialogOpen}
      />
      <CreatePublicProjectDialog
        targetSpaceId={newKbTeamSpaceId ?? ""}
        listSpaceId={orgSpaceId}
        title="New team KB"
        open={Boolean(newKbTeamSpaceId)}
        onOpenChange={(open) => {
          if (!open) setNewKbTeamSpaceId(null);
        }}
      />
      <CreateTeamDialog
        orgId={orgId}
        open={teamDialogOpen}
        onOpenChange={setTeamDialogOpen}
      />
      {manageProjectId ? (
        <ManageAccessDialog
          projectId={manageProjectId}
          spaceKind={manageTeamId ? "team" : "org"}
          orgId={orgId}
          teamId={manageTeamId}
          open={Boolean(manageProjectId)}
          onOpenChange={(open) => {
            if (!open) {
              setManageProjectId(null);
              setManageTeamId(null);
            }
          }}
        />
      ) : null}
    </div>
  );
}
