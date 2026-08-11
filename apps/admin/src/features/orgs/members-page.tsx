import type { ColumnDef } from "@tanstack/react-table";
import { UserPlus } from "lucide-react";
import { useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/shared/confirm-dialog";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { ForbiddenState, LoadingState } from "@/components/shared/states";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

import { useSpacesQuery } from "../spaces/use-spaces";
import { useOrgMembersQuery } from "./members-queries";
import {
  useAddOrgMemberMutation,
  useSetOrgMemberRoleMutation,
  useRemoveOrgMemberMutation,
} from "./members-mutations";

type OrgMember = NonNullable<ReturnType<typeof useOrgMembersQuery>["data"]>["members"][number];
type OrgRole = "org_admin" | "org_member";

function MembersTable({
  members,
  isAdmin,
  onChangeRole,
  onRemove,
}: {
  members: OrgMember[];
  isAdmin: boolean;
  onChangeRole: (userId: string, role: OrgRole) => void;
  onRemove: (member: OrgMember) => void;
}) {
  const columns = useMemo<ColumnDef<OrgMember>[]>(
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
        cell: ({ row }) =>
          isAdmin ? (
            <Select
              value={row.original.role}
              onValueChange={(value) => onChangeRole(row.original.userId, value as OrgRole)}
            >
              <SelectTrigger
                aria-label={`Role for ${row.original.username}`}
                size="sm"
                className="w-36"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="org_admin">org_admin</SelectItem>
                <SelectItem value="org_member">org_member</SelectItem>
              </SelectContent>
            </Select>
          ) : (
            <Badge variant={row.original.role === "org_admin" ? "default" : "secondary"}>
              {row.original.role}
            </Badge>
          ),
      },
      ...(isAdmin
        ? [
            {
              id: "actions",
              header: "",
              cell: ({ row }: { row: { original: OrgMember } }) => (
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
  const [role, setRoleValue] = useState<OrgRole>("org_member");
  const [removeTarget, setRemoveTarget] = useState<OrgMember | null>(null);

  if (spaces.isLoading) return <LoadingState rows={5} />;
  if (!org) return <ForbiddenState description="You do not have access to this organization, or it does not exist." />;

  const add = async () => {
    try {
      await addMember.mutateAsync({ usernameOrEmail: username.trim(), role });
      toast.success(`Added ${username.trim()} to ${org.name}.`);
      setUsername("");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to add member");
    }
  };

  const changeRole = async (userId: string, nextRole: OrgRole) => {
    try {
      await setRole.mutateAsync({ userId, role: nextRole });
      toast.success("Role updated.");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to update role");
    }
  };

  const addControls = isAdmin ? (
    <div className="flex flex-wrap items-center gap-2">
      <Input
        aria-label="New member username"
        className="w-44"
        placeholder="Username"
        value={username}
        onChange={(event) => setUsername(event.target.value)}
      />
      <Select value={role} onValueChange={(value) => setRoleValue(value as OrgRole)}>
        <SelectTrigger aria-label="New member role" className="w-36">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="org_member">org_member</SelectItem>
          <SelectItem value="org_admin">org_admin</SelectItem>
        </SelectContent>
      </Select>
      <Button
        type="button"
        onClick={add}
        disabled={addMember.isPending || username.trim().length === 0}
      >
        <UserPlus />
        Add member
      </Button>
    </div>
  ) : null;

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Manage who belongs to this organization and what they can do."
        title={`${org.name} members`}
        actions={addControls ?? undefined}
      />

      <MembersTable
        members={members.data?.members ?? []}
        isAdmin={isAdmin}
        onChangeRole={changeRole}
        onRemove={setRemoveTarget}
      />

      <ConfirmDialog
        confirmLabel="Remove member"
        description={
          removeTarget
            ? `${removeTarget.username} will lose access to all projects in ${org.name}.`
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
    </div>
  );
}
