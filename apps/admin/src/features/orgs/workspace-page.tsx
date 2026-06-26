import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { ForbiddenState, LoadingState } from "@/components/shared/states";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

import { ManageAccessDialog } from "../kb-access/manage-access-dialog";
import { useSpacesQuery } from "../spaces/use-spaces";
import { CreatePublicProjectDialog } from "./create-public-project-dialog";
import { CreateTeamDialog } from "./create-team-dialog";
import { useOrgProjectsQuery, useOrgTeamsQuery } from "./workspace-queries";

type OrgProject = {
  id: string;
  name: string;
  rootPath: string;
  createdAt: string;
  spaceKind: string;
  teamId: string | null;
  teamSlug: string | null;
  role: string;
};

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

  if (spaces.isLoading) return <LoadingState rows={6} />;
  if (!org)
    return (
      <ForbiddenState description="You do not have access to this organization, or it does not exist." />
    );

  const isAdmin = org.role === "org_admin";
  const allProjects = projects.data ?? [];
  const publicProjects = allProjects.filter((p) => p.spaceKind === "org");
  const teamList = teams.data?.teams ?? [];

  const openManage = (projectId: string, teamId: string | null) => {
    setManageProjectId(projectId);
    setManageTeamId(teamId);
  };

  const adminActions = (
    <>
      <Button type="button" onClick={() => setPublicDialogOpen(true)}>
        New public project
      </Button>
      <Button asChild variant="outline">
        <Link to={`/orgs/${orgId}/members`}>Members</Link>
      </Button>
      <Button type="button" onClick={() => setTeamDialogOpen(true)}>
        New team
      </Button>
    </>
  );

  const nonAdminActions = (
    <Button type="button" onClick={() => setTeamDialogOpen(true)}>
      New team
    </Button>
  );

  return (
    <div className="grid gap-6">
      <PageHeader
        title={`${org.name} workspace`}
        actions={isAdmin ? adminActions : nonAdminActions}
      />

      <PublicProjectsSection
        projects={publicProjects}
        isAdmin={isAdmin}
        onManage={(id) => openManage(id, null)}
      />

      {teamList.map((team) => {
        const teamProjects = allProjects.filter((p) => p.teamId === team.id);
        const canManageTeam = isAdmin || team.role === "leader";
        const teamSpaceId = team.spaceId;
        return (
          <TeamSection
            key={team.id}
            team={team}
            orgId={orgId}
            projects={teamProjects}
            canManageTeam={canManageTeam}
            teamSpaceId={teamSpaceId}
            onNewKb={() => setNewKbTeamSpaceId(teamSpaceId)}
            onManage={(id) => openManage(id, team.id)}
          />
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

function PublicProjectsSection({
  projects,
  isAdmin,
  onManage,
}: {
  projects: OrgProject[];
  isAdmin: boolean;
  onManage: (id: string) => void;
}) {
  const columns = useMemo<ColumnDef<OrgProject>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => (
          <Link
            className="font-medium underline-offset-4 hover:underline"
            to={`/projects/${row.original.id}`}
          >
            {row.original.name}
          </Link>
        ),
      },
      ...(isAdmin
        ? [
            {
              id: "actions",
              header: "",
              cell: ({ row }: { row: { original: OrgProject } }) => (
                <div className="flex justify-end">
                  <Button
                    size="sm"
                    type="button"
                    variant="outline"
                    onClick={() => onManage(row.original.id)}
                  >
                    Manage access
                  </Button>
                </div>
              ),
            } satisfies ColumnDef<OrgProject>,
          ]
        : []),
    ],
    [isAdmin, onManage],
  );

  return (
    <section>
      <h2 className="mb-3 text-sm font-medium">Public projects</h2>
      <DataTable
        columns={columns}
        data={projects}
        emptyMessage="No public projects yet."
      />
    </section>
  );
}

type TeamEntry = {
  id: string;
  name: string;
  slug: string;
  orgId: string;
  spaceId: string;
  role: string | null;
};

function TeamSection({
  team,
  orgId,
  projects,
  canManageTeam,
  teamSpaceId,
  onNewKb,
  onManage,
}: {
  team: TeamEntry;
  orgId: string;
  projects: OrgProject[];
  canManageTeam: boolean;
  teamSpaceId: string;
  onNewKb: () => void;
  onManage: (id: string) => void;
}) {
  const columns = useMemo<ColumnDef<OrgProject>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => (
          <Link
            className="font-medium underline-offset-4 hover:underline"
            to={`/projects/${row.original.id}`}
          >
            {row.original.name}
          </Link>
        ),
      },
      ...(canManageTeam
        ? [
            {
              id: "actions",
              header: "",
              cell: ({ row }: { row: { original: OrgProject } }) => (
                <div className="flex justify-end">
                  <Button
                    size="sm"
                    type="button"
                    variant="outline"
                    onClick={() => onManage(row.original.id)}
                  >
                    Manage access
                  </Button>
                </div>
              ),
            } satisfies ColumnDef<OrgProject>,
          ]
        : []),
    ],
    [canManageTeam, onManage],
  );

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-2">
          <CardTitle>Team - {team.name}</CardTitle>
          <div className="flex items-center gap-2">
            {canManageTeam && teamSpaceId ? (
              <Button size="sm" type="button" onClick={onNewKb}>
                New team KB
              </Button>
            ) : null}
            <Button asChild size="sm" variant="outline">
              <Link to={`/orgs/${orgId}/teams/${team.id}`}>Manage</Link>
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <DataTable
          columns={columns}
          data={projects}
          emptyMessage="No team KBs yet."
        />
      </CardContent>
    </Card>
  );
}
