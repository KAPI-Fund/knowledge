import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { useParams } from "react-router-dom";

import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { ForbiddenState, LoadingState } from "@/components/shared/states";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

import { useSpacesQuery } from "../spaces/use-spaces";
import { useOrgMembersQuery } from "./members-queries";
import {
  useAddOrgMemberMutation,
  useSetOrgMemberRoleMutation,
  useRemoveOrgMemberMutation,
} from "./members-mutations";

type OrgMember = NonNullable<ReturnType<typeof useOrgMembersQuery>["data"]>["members"][number];

function MembersTable({
  members,
  isAdmin,
  onChangeRole,
  onRemove,
}: {
  members: OrgMember[];
  isAdmin: boolean;
  onChangeRole: (userId: string, role: "org_admin" | "org_member") => void;
  onRemove: (userId: string) => void;
}) {
  const columns = useMemo<ColumnDef<OrgMember>[]>(
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
        cell: ({ row }) =>
          isAdmin ? (
            <select
              aria-label={`Role for ${row.original.username}`}
              value={row.original.role}
              onChange={(event) =>
                onChangeRole(
                  row.original.userId,
                  event.target.value as "org_admin" | "org_member",
                )
              }
            >
              <option value="org_admin">org_admin</option>
              <option value="org_member">org_member</option>
            </select>
          ) : (
            <StatusPill value={row.original.role} />
          ),
      },
      ...(isAdmin
        ? [
            {
              id: "actions",
              header: "Actions",
              cell: ({ row }: { row: { original: OrgMember } }) => (
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
            } satisfies ColumnDef<OrgMember>,
          ]
        : []),
    ],
    [isAdmin, onChangeRole, onRemove],
  );

  return (
    <DataTable
      columns={columns}
      data={members}
      emptyMessage="No members yet."
    />
  );
}

export function OrgMembersPage() {
  const { orgId = "" } = useParams();
  const spaces = useSpacesQuery();
  const org = spaces.data?.orgs.find((entry) => entry.id === orgId);
  const isAdmin = org?.role === "org_admin";

  const members = useOrgMembersQuery(orgId);
  const addMember = useAddOrgMemberMutation(orgId);
  const setRole = useSetOrgMemberRoleMutation(orgId);
  const removeMember = useRemoveOrgMemberMutation(orgId);

  const [username, setUsername] = useState("");
  const [role, setRoleValue] = useState<"org_admin" | "org_member">("org_member");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  if (spaces.isLoading) return <LoadingState rows={5} />;
  if (!org) return <ForbiddenState description="You do not have access to this organization, or it does not exist." />;

  const add = async () => {
    setErrorMessage(null);
    try {
      await addMember.mutateAsync({ usernameOrEmail: username.trim(), role });
      setUsername("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to add member");
    }
  };

  const changeRole = async (
    userId: string,
    nextRole: "org_admin" | "org_member",
  ) => {
    setErrorMessage(null);
    try {
      await setRole.mutateAsync({ userId, role: nextRole });
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to update role");
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

  const addControls = isAdmin ? (
    <div className="flex items-end gap-2">
      <Input
        placeholder="Username"
        value={username}
        onChange={(event) => setUsername(event.target.value)}
      />
      <select
        aria-label="New member role"
        value={role}
        onChange={(event) =>
          setRoleValue(event.target.value as "org_admin" | "org_member")
        }
      >
        <option value="org_member">org_member</option>
        <option value="org_admin">org_admin</option>
      </select>
      <Button type="button" onClick={add} disabled={addMember.isPending}>
        Add member
      </Button>
    </div>
  ) : null;

  return (
    <div className="grid gap-6">
      <PageHeader
        title={`${org.name} members`}
        actions={addControls ?? undefined}
      />

      <MembersTable
        members={members.data?.members ?? []}
        isAdmin={isAdmin}
        onChangeRole={changeRole}
        onRemove={remove}
      />

      {errorMessage ? (
        <p className="text-sm text-destructive">{errorMessage}</p>
      ) : null}
    </div>
  );
}
