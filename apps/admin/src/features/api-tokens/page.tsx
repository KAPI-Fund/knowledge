import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";

import { EmptyState } from "@/components/layout/empty-state";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

import type { ApiTokenCreateResponse } from "../shared/api";
import { useProjectsQuery } from "../projects/queries";

import {
  useApiTokensQuery,
  useCreateApiTokenMutation,
  useRevokeApiTokenMutation,
} from "./queries";

type TokenRow = NonNullable<ReturnType<typeof useApiTokensQuery>["data"]>["tokens"][number];

export function ApiTokensPage() {
  const [name, setName] = useState("");
  const [scopeProjectId, setScopeProjectId] = useState<string>("");
  const [mintedToken, setMintedToken] = useState<ApiTokenCreateResponse | null>(null);
  const tokensQuery = useApiTokensQuery();
  const projectsQuery = useProjectsQuery();
  const createMutation = useCreateApiTokenMutation();
  const revokeMutation = useRevokeApiTokenMutation();
  const tokens = tokensQuery.data?.tokens ?? [];
  const projects = projectsQuery.data ?? [];

  const projectsById = useMemo(
    () => new Map(projects.map((project) => [project.id, project.name] as const)),
    [projects],
  );

  const columns = useMemo<ColumnDef<TokenRow>[]>(
    () => [
      {
        accessorKey: "name",
        header: "Name",
        cell: ({ row }) => <span className="font-medium text-foreground">{row.original.name}</span>,
      },
      {
        accessorKey: "prefix",
        header: "Prefix",
        cell: ({ row }) => (
          <code className="font-mono text-[11px] text-muted-foreground">{row.original.prefix}…</code>
        ),
      },
      {
        id: "project",
        header: "Project",
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">
            {row.original.projectId
              ? projectsById.get(row.original.projectId) ?? row.original.projectId
              : "—"}
          </span>
        ),
      },
      {
        id: "status",
        header: "Status",
        cell: ({ row }) => <StatusPill value={row.original.revokedAt ? "revoked" : "active"} />,
      },
      {
        id: "lastUsed",
        header: "Last Used",
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">{row.original.lastUsedAt ?? "—"}</span>
        ),
      },
      {
        accessorKey: "createdAt",
        header: "Created",
        cell: ({ row }) => (
          <span className="text-sm text-muted-foreground">{row.original.createdAt}</span>
        ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) =>
          row.original.revokedAt ? null : (
            <div className="flex justify-end">
              <Button
                disabled={revokeMutation.isPending}
                onClick={() => revokeMutation.mutateAsync({ tokenId: row.original.id })}
                size="sm"
                variant="outline"
              >
                Revoke
              </Button>
            </div>
          ),
      },
    ],
    [projectsById, revokeMutation],
  );

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Mint and revoke API tokens for programmatic access. Tokens carry your user identity and project memberships."
        title="API Tokens"
      />

      <Card>
        <CardHeader>
          <CardTitle>Mint a New Token</CardTitle>
          <CardDescription>
            Provide a descriptive name. The token plaintext is shown once and is not recoverable
            afterwards.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:max-w-xl">
          <label className="grid gap-2 text-sm font-medium">
            Token Name
            <Input
              aria-label="Token Name"
              onChange={(event) => setName(event.target.value)}
              placeholder="e.g. ci-bot"
              value={name}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Token Scope
            <select
              aria-label="Token Scope"
              className="rounded-md border border-input bg-background px-3 py-2 text-sm"
              onChange={(event) => setScopeProjectId(event.target.value)}
              value={scopeProjectId}
            >
              <option value="">No scope (system-wide)</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </label>
          <div className="flex justify-end">
            <Button
              disabled={createMutation.isPending || name.trim().length === 0}
              onClick={async () => {
                const result = await createMutation.mutateAsync({
                  name: name.trim(),
                  projectId: scopeProjectId.length > 0 ? scopeProjectId : null,
                });
                setMintedToken(result);
                setName("");
                setScopeProjectId("");
              }}
            >
              Mint Token
            </Button>
          </div>
        </CardContent>
      </Card>

      {mintedToken ? (
        <Card>
          <CardHeader>
            <CardTitle>New Token: {mintedToken.name}</CardTitle>
            <CardDescription>Copy this token now — it will not be shown again.</CardDescription>
          </CardHeader>
          <CardContent>
            <code className="block break-all rounded bg-muted px-3 py-2 text-sm">
              {mintedToken.token}
            </code>
            <div className="mt-3 flex gap-2">
              <Button
                onClick={() =>
                  navigator.clipboard?.writeText(mintedToken.token).catch(() => undefined)
                }
                variant="outline"
              >
                Copy
              </Button>
              <Button onClick={() => setMintedToken(null)} variant="outline">
                Dismiss
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : null}

      {tokens.length ? (
        <div className="grid gap-3">
          <h2 className="text-sm font-medium text-muted-foreground">Existing Tokens</h2>
          <DataTable columns={columns} data={tokens} isLoading={tokensQuery.isLoading} />
        </div>
      ) : (
        <EmptyState description="You haven't minted any API tokens yet." title="No tokens" />
      )}
    </div>
  );
}
