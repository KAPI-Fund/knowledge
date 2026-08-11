import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { Input } from "@/components/ui/input";

import { useProjectAuditLogsQuery } from "./queries";

type AuditRow = NonNullable<ReturnType<typeof useProjectAuditLogsQuery>["data"]>[number];

const columns: ColumnDef<AuditRow>[] = [
  {
    accessorKey: "action",
    header: "Action",
    cell: ({ row }) => (
      <code className="font-mono text-[11px] text-muted-foreground">{row.original.action}</code>
    ),
  },
  {
    accessorKey: "summary",
    header: "Summary",
    cell: ({ row }) => <span className="text-sm">{row.original.summary}</span>,
  },
];

export function AuditPage() {
  const { projectId = "" } = useParams();
  const items = useProjectAuditLogsQuery(projectId);
  const [filter, setFilter] = useState("");

  const rows = items.data ?? [];
  const filtered = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) {
      return rows;
    }
    return rows.filter(
      (item) =>
        item.action.toLowerCase().includes(needle) ||
        item.summary.toLowerCase().includes(needle),
    );
  }, [rows, filter]);

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Inspect the backend event trail for project mutations and queue actions."
        title="Audit"
      />
      {rows.length ? (
        <div className="grid gap-3">
          <Input
            aria-label="Filter audit records"
            className="h-8 w-40 lg:w-64"
            onChange={(event) => setFilter(event.target.value)}
            placeholder="Filter by action or summary"
            value={filter}
          />
          <DataTable columns={columns} data={filtered} isLoading={items.isLoading} />
        </div>
      ) : (
        <EmptyState
          description="Audit events will appear once the backend records project mutations."
          title="No audit records"
        />
      )}
    </div>
  );
}
