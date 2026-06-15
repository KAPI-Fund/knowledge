import { useState } from "react";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

import type { ApiTokenCreateResponse } from "../shared/api";
import { useProjectsQuery } from "../projects/queries";

import {
  useApiTokensQuery,
  useCreateApiTokenMutation,
  useRevokeApiTokenMutation,
} from "./queries";

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

  return (
    <PageSection
      description="Mint and revoke API tokens for programmatic access. Tokens carry your user identity and project memberships."
      title="API Tokens"
    >
      <Card>
        <CardHeader>
          <CardTitle>Mint a New Token</CardTitle>
          <CardDescription>
            Provide a descriptive name. The token plaintext is shown once and is not recoverable
            afterwards.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
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
              className="rounded border bg-background px-3 py-2"
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
            <CardDescription>
              Copy this token now — it will not be shown again.
            </CardDescription>
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
        <Card>
          <CardHeader>
            <CardTitle>Existing Tokens</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Prefix</TableHead>
                  <TableHead>Project</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Last Used</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {tokens.map((token) => (
                  <TableRow key={token.id}>
                    <TableCell className="font-medium">{token.name}</TableCell>
                    <TableCell>
                      <code>{token.prefix}…</code>
                    </TableCell>
                    <TableCell>{token.projectId ?? "—"}</TableCell>
                    <TableCell>
                      {token.revokedAt ? (
                        <Badge variant="outline">revoked</Badge>
                      ) : (
                        <Badge>active</Badge>
                      )}
                    </TableCell>
                    <TableCell>{token.lastUsedAt ?? "—"}</TableCell>
                    <TableCell>{token.createdAt}</TableCell>
                    <TableCell className="text-right">
                      {token.revokedAt ? null : (
                        <Button
                          disabled={revokeMutation.isPending}
                          onClick={() => revokeMutation.mutateAsync({ tokenId: token.id })}
                          variant="outline"
                        >
                          Revoke
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      ) : (
        <EmptyState
          description="You haven't minted any API tokens yet."
          title="No tokens"
        />
      )}
    </PageSection>
  );
}
