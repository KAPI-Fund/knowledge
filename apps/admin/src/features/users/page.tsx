import type { ColumnDef } from "@tanstack/react-table";
import { Users } from "lucide-react";
import { useMemo } from "react";

import { EmptyState } from "@/components/layout/empty-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";

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
        cell: ({ row }) => (
          <div className="flex items-center gap-3">
            <Avatar className="size-8">
              <AvatarFallback className="text-xs uppercase">
                {row.original.username.slice(0, 2)}
              </AvatarFallback>
            </Avatar>
            <span className="font-medium text-foreground">{row.original.username}</span>
          </div>
        ),
      },
      {
        accessorKey: "role",
        header: "Role",
        cell: ({ row }) => (
          <Badge variant={row.original.role === "admin" ? "default" : "secondary"}>
            {row.original.role}
          </Badge>
        ),
      },
      {
        accessorKey: "id",
        header: "User ID",
        cell: ({ row }) => (
          <span className="font-mono text-[11px] text-muted-foreground">{row.original.id}</span>
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
      {userList.length || users.isLoading ? (
        <DataTable
          columns={columns}
          data={userList}
          isLoading={users.isLoading}
          searchKey="username"
          searchPlaceholder="Filter users..."
        />
      ) : (
        <EmptyState
          description="No users are currently available from the backend directory."
          icon={Users}
          title="No users returned"
        />
      )}
    </div>
  );
}
