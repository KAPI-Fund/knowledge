import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

import { useProjectAuditLogsQuery } from "./queries";

export function AuditPage() {
  const { projectId = "" } = useParams();
  const items = useProjectAuditLogsQuery(projectId);

  return (
    <PageSection
      description="Inspect the backend event trail for project mutations and queue actions."
      title="Audit"
    >
      <Card>
        <CardHeader>
          <CardTitle>Audit Trail</CardTitle>
          <CardDescription>Append-only events emitted by project workflows and system actions.</CardDescription>
        </CardHeader>
        <CardContent className="p-0">
          {items.data?.length ? (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Action</TableHead>
                  <TableHead>Summary</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {items.data.map((item) => (
                  <TableRow key={item.id}>
                    <TableCell className="font-medium">{item.action}</TableCell>
                    <TableCell className="text-muted-foreground">{item.summary}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          ) : (
            <div className="p-6">
              <EmptyState
                description="Audit events will appear once the backend records project mutations."
                title="No audit records"
              />
            </div>
          )}
        </CardContent>
      </Card>
    </PageSection>
  );
}
