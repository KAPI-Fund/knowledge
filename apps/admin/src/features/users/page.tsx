import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";

import { EmptyState } from "@/components/layout/empty-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";

import { useUsersQuery } from "./queries";

type UserRow = NonNullable<ReturnType<typeof useUsersQuery>["data"]>[number];

export function UsersPage() {
  const users = useUsersQuery();
  const userList = users.data ?? [];

  const columns = useMemo<ColumnDef<UserRow>[]>(
    () => [
      {
        accessorKey: "username",
        header: "Username",
        cell: ({ row }) => <span className="font-medium text-foreground">{row.original.username}</span>,
      },
      {
        accessorKey: "role",
        header: "Role",
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">{row.original.role}</span>
        ),
      },
    ],
    [],
  );

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Read-only user directory for the current installation."
        title="Users"
      />
      {userList.length ? (
        <div className="grid gap-3">
          <p className="text-sm text-muted-foreground">{userList.length} users</p>
          <DataTable columns={columns} data={userList} isLoading={users.isLoading} />
        </div>
      ) : (
        <EmptyState
          description="No users are currently available from the backend directory."
          title="No users returned"
        />
      )}
    </div>
  );
}
