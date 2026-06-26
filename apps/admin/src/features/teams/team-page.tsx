import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { ForbiddenState, LoadingState } from "@/components/shared/states";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

import { ManageAccessDialog } from "../kb-access/manage-access-dialog";
import { useSpacesQuery } from "../spaces/use-spaces";
import { useOrgTeamsQuery } from "../orgs/workspace-queries";
import { useTeamMembersQuery, useTeamProjectsQuery } from "./team-queries";
import {
  useAddTeamMemberMutation,
  useRemoveTeamMemberMutation,
  useCreateTeamKbMutation,
} from "./team-mutations";

type TeamMember = NonNullable<ReturnType<typeof useTeamMembersQuery>["data"]>["members"][number];
type TeamProject = NonNullable<ReturnType<typeof useTeamProjectsQuery>["data"]>[number];

function TeamMembersTable({
  members,
  canManage,
  onRemove,
}: {
  members: TeamMember[];
  canManage: boolean;
  onRemove: (userId: string) => void;
}) {
  const columns = useMemo<ColumnDef<TeamMember>[]>(
    () => [
      {
        accessorKey: "username",
        header: "Username",
        cell: ({ row }) => (
          <span className="font-medium">{row.original.username}</span>
        ),
      },
      {
        accessorKey: "role",
        header: "Role",
        cell: ({ row }) => <span className="text-sm text-muted-foreground">{row.original.role}</span>,
      },
      ...(canManage
        ? [
            {
              id: "actions",
              header: "Actions",
              cell: ({ row }: { row: { original: TeamMember } }) =>
                row.original.role === "leader" ? null : (
                  <div className="flex justify-end">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      aria-label={`Remove ${row.original.username}`}
                      onClick={() => onRemove(row.original.userId)}
                    >
                      Remove
                    </Button>
                  </div>
                ),
            } satisfies ColumnDef<TeamMember>,
          ]
        : []),
    ],
    [canManage, onRemove],
  );

  return (
    <DataTable
      columns={columns}
      data={members}
      emptyMessage="No members yet."
    />
  );
}

function TeamKbsTable({
  projects,
  canManage,
  onManage,
}: {
  projects: TeamProject[];
  canManage: boolean;
  onManage: (projectId: string) => void;
}) {
  const columns = useMemo<ColumnDef<TeamProject>[]>(
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
      ...(canManage
        ? [
            {
              id: "actions",
              header: "",
              cell: ({ row }: { row: { original: TeamProject } }) => (
                <div className="flex justify-end">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => onManage(row.original.id)}
                  >
                    Manage access
                  </Button>
                </div>
              ),
            } satisfies ColumnDef<TeamProject>,
          ]
        : []),
    ],
    [canManage, onManage],
  );

  return (
    <DataTable
      columns={columns}
      data={projects}
      emptyMessage="No team KBs yet."
    />
  );
}

export function TeamPage() {
  const { orgId = "", teamId = "" } = useParams();
  const spaces = useSpacesQuery();
  const org = spaces.data?.orgs.find((entry) => entry.id === orgId);
  const teamsQuery = useOrgTeamsQuery(orgId);
  const team = teamsQuery.data?.teams.find((entry) => entry.id === teamId);
  const teamSpaceId = team?.spaceId ?? "";

  const isAdmin = org?.role === "org_admin";
  const isLeader = team?.role === "leader";
  const canManage = Boolean(isAdmin || isLeader);

  const members = useTeamMembersQuery(orgId, teamId);
  const projects = useTeamProjectsQuery(teamSpaceId, teamId);
  const addMember = useAddTeamMemberMutation(orgId, teamId);
  const removeMember = useRemoveTeamMemberMutation(orgId, teamId);
  const createKb = useCreateTeamKbMutation(teamSpaceId);

  const [username, setUsername] = useState("");
  const [kbName, setKbName] = useState("");
  const [manageProjectId, setManageProjectId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  if (spaces.isLoading || teamsQuery.isLoading) return <LoadingState />;
  if (!team) return <ForbiddenState description="You do not have access to this team, or it does not exist." />;

  const add = async () => {
    setErrorMessage(null);
    try {
      await addMember.mutateAsync({ usernameOrEmail: username.trim() });
      setUsername("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to add member");
    }
  };

  const remove = async (userId: string) => {
    setErrorMessage(null);
    try {
      await removeMember.mutateAsync({ userId });
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to remove member");
    }
  };

  const addKb = async () => {
    setErrorMessage(null);
    try {
      await createKb.mutateAsync({ name: kbName.trim() });
      setKbName("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create KB");
    }
  };

  const backAction = (
    <Button asChild variant="ghost">
      <Link to={`/orgs/${orgId}`}>Back to workspace</Link>
    </Button>
  );

  return (
    <div className="grid gap-6">
      <PageHeader
        title={`Team - ${team.name}`}
        actions={backAction}
      />

      <section className="grid gap-3">
        <h2 className="text-sm font-medium">Members</h2>
        <TeamMembersTable
          members={members.data?.members ?? []}
          canManage={canManage}
          onRemove={remove}
        />
        {canManage ? (
          <div className="flex items-end gap-2">
            <Input
              placeholder="Username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
            />
            <Button type="button" onClick={add} disabled={addMember.isPending}>
              Add member
            </Button>
          </div>
        ) : null}
      </section>

      <section className="grid gap-3">
        <h2 className="text-sm font-medium">Team KBs</h2>
        <TeamKbsTable
          projects={projects.data ?? []}
          canManage={canManage}
          onManage={(id) => setManageProjectId(id)}
        />
        {canManage ? (
          <div className="flex items-end gap-2">
            <Input
              placeholder="New team KB name"
              value={kbName}
              onChange={(event) => setKbName(event.target.value)}
            />
            <Button type="button" onClick={addKb} disabled={createKb.isPending}>
              New team KB
            </Button>
          </div>
        ) : null}
      </section>

      {errorMessage ? (
        <p className="text-sm text-destructive">{errorMessage}</p>
      ) : null}

      {manageProjectId ? (
        <ManageAccessDialog
          projectId={manageProjectId}
          spaceKind="team"
          orgId={orgId}
          teamId={teamId}
          open={Boolean(manageProjectId)}
          onOpenChange={(open) => {
            if (!open) setManageProjectId(null);
          }}
        />
      ) : null}
    </div>
  );
}
