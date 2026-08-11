import type { ColumnDef } from "@tanstack/react-table";
import { ArrowLeft, Plus, UserPlus } from "lucide-react";
import { useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/shared/confirm-dialog";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { ForbiddenState, LoadingState } from "@/components/shared/states";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
  onRemove: (member: TeamMember) => void;
}) {
  const columns = useMemo<ColumnDef<TeamMember>[]>(
    () => [
      {
        accessorKey: "username",
        header: "Username",
        cell: ({ row }) => (
          <div className="flex items-center gap-3">
            <Avatar className="size-8">
              <AvatarFallback className="text-xs uppercase">
                {row.original.username.slice(0, 2)}
              </AvatarFallback>
            </Avatar>
            <span className="font-medium">{row.original.username}</span>
          </div>
        ),
      },
      {
        accessorKey: "role",
        header: "Role",
        cell: ({ row }) => (
          <Badge variant={row.original.role === "leader" ? "default" : "secondary"}>
            {row.original.role}
          </Badge>
        ),
      },
      ...(canManage
        ? [
            {
              id: "actions",
              header: "",
              cell: ({ row }: { row: { original: TeamMember } }) =>
                row.original.role === "leader" ? null : (
                  <div className="flex justify-end">
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      aria-label={`Remove ${row.original.username}`}
                      onClick={() => onRemove(row.original)}
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
  const [removeTarget, setRemoveTarget] = useState<TeamMember | null>(null);

  if (spaces.isLoading || teamsQuery.isLoading) return <LoadingState rows={5} />;
  if (!team) return <ForbiddenState description="You do not have access to this team, or it does not exist." />;

  const add = async () => {
    try {
      await addMember.mutateAsync({ usernameOrEmail: username.trim() });
      toast.success(`Added ${username.trim()} to ${team.name}.`);
      setUsername("");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to add member");
    }
  };

  const addKb = async () => {
    try {
      await createKb.mutateAsync({ name: kbName.trim() });
      toast.success(`Created "${kbName.trim()}".`);
      setKbName("");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create KB");
    }
  };

  const backAction = (
    <Button asChild variant="outline">
      <Link to={`/orgs/${orgId}`}>
        <ArrowLeft />
        Back to workspace
      </Link>
    </Button>
  );

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Manage this team's members and knowledge bases."
        title={`Team - ${team.name}`}
        actions={backAction}
      />

      <Card>
        <CardHeader>
          <CardTitle>Members</CardTitle>
          <CardDescription>People who can work in this team's spaces.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          <TeamMembersTable
            members={members.data?.members ?? []}
            canManage={canManage}
            onRemove={setRemoveTarget}
          />
          {canManage ? (
            <div className="flex flex-wrap items-center gap-2">
              <Input
                aria-label="New team member username"
                className="w-44"
                placeholder="Username"
                value={username}
                onChange={(event) => setUsername(event.target.value)}
              />
              <Button
                type="button"
                onClick={add}
                disabled={addMember.isPending || username.trim().length === 0}
              >
                <UserPlus />
                Add member
              </Button>
            </div>
          ) : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Team KBs</CardTitle>
          <CardDescription>Knowledge bases owned by this team.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3">
          <TeamKbsTable
            projects={projects.data ?? []}
            canManage={canManage}
            onManage={(id) => setManageProjectId(id)}
          />
          {canManage ? (
            <div className="flex flex-wrap items-center gap-2">
              <Input
                aria-label="New team KB name"
                className="w-56"
                placeholder="New team KB name"
                value={kbName}
                onChange={(event) => setKbName(event.target.value)}
              />
              <Button
                type="button"
                onClick={addKb}
                disabled={createKb.isPending || kbName.trim().length === 0}
              >
                <Plus />
                New team KB
              </Button>
            </div>
          ) : null}
        </CardContent>
      </Card>

      <ConfirmDialog
        confirmLabel="Remove member"
        description={
          removeTarget
            ? `${removeTarget.username} will lose access to ${team.name}'s knowledge bases.`
            : ""
        }
        destructive
        isPending={removeMember.isPending}
        onConfirm={async () => {
          if (!removeTarget) return;
          try {
            await removeMember.mutateAsync({ userId: removeTarget.userId });
            toast.success(`Removed ${removeTarget.username}.`);
          } catch (error) {
            toast.error(error instanceof Error ? error.message : "Failed to remove member");
          }
        }}
        onOpenChange={(open) => {
          if (!open) setRemoveTarget(null);
        }}
        open={Boolean(removeTarget)}
        title="Remove this member?"
      />

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
