import { EmptyState } from "../../components/layout/empty-state";
import { PageSection } from "../../components/layout/page-section";
import { Card, CardContent } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "../../components/ui/table";

import { useUsersQuery } from "./queries";

export function UsersPage() {
  const users = useUsersQuery();
  const userList = users.data ?? [];

  return (
    <PageSection description="Read-only user directory for the current installation." title="Users">
      <div className="stats">
        <span>{userList.length} users</span>
      </div>
      {userList.length ? (
        <Card className="panel">
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Username</TableHead>
                  <TableHead>Role</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {userList.map((user) => (
                  <TableRow key={user.id}>
                    <TableCell className="font-medium">{user.username}</TableCell>
                    <TableCell>
                      <Badge variant="outline">{user.role}</Badge>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      ) : (
        <EmptyState
          description="No users are currently available from the backend directory."
          title="No users returned"
        />
      )}
    </PageSection>
  );
}
