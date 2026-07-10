import { zodResolver } from "@hookform/resolvers/zod";
import type { ColumnDef } from "@tanstack/react-table";
import { Check, Copy, KeyRound, Loader2 } from "lucide-react";
import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { toast } from "sonner";
import { z } from "zod";

import { EmptyState } from "@/components/layout/empty-state";
import { ConfirmDialog } from "@/components/shared/confirm-dialog";
import { DataTable } from "@/components/shared/data-table";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { formatDate, formatDateTime } from "@/lib/format";

import type { ApiTokenCreateResponse } from "../shared/api";
import { useProjectsQuery } from "../projects/queries";

import {
  useApiTokensQuery,
  useCreateApiTokenMutation,
  useRevokeApiTokenMutation,
} from "./queries";

type TokenRow = NonNullable<ReturnType<typeof useApiTokensQuery>["data"]>["tokens"][number];

const NO_SCOPE = "__none__";

const mintTokenSchema = z.object({
  name: z.string().trim().min(1, "Token name is required."),
  scopeProjectId: z.string(),
});

type MintTokenValues = z.infer<typeof mintTokenSchema>;

export function ApiTokensPage() {
  const [mintedToken, setMintedToken] = useState<ApiTokenCreateResponse | null>(null);
  const [copied, setCopied] = useState(false);
  const [revokeTarget, setRevokeTarget] = useState<TokenRow | null>(null);
  const tokensQuery = useApiTokensQuery();
  const projectsQuery = useProjectsQuery();
  const createMutation = useCreateApiTokenMutation();
  const revokeMutation = useRevokeApiTokenMutation();
  const tokens = tokensQuery.data?.tokens ?? [];
  const projects = projectsQuery.data ?? [];

  const form = useForm<MintTokenValues>({
    resolver: zodResolver(mintTokenSchema),
    defaultValues: { name: "", scopeProjectId: NO_SCOPE },
  });

  const projectsById = useMemo(
    () => new Map(projects.map((project) => [project.id, project.name] as const)),
    [projects],
  );

  async function onSubmit(values: MintTokenValues) {
    try {
      const result = await createMutation.mutateAsync({
        name: values.name,
        projectId: values.scopeProjectId === NO_SCOPE ? null : values.scopeProjectId,
      });
      setMintedToken(result);
      setCopied(false);
      form.reset({ name: "", scopeProjectId: NO_SCOPE });
      toast.success(`Token "${result.name}" minted.`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to mint token.");
    }
  }

  async function copyMintedToken() {
    if (!mintedToken) return;
    try {
      await navigator.clipboard.writeText(mintedToken.token);
      setCopied(true);
      toast.success("Token copied to clipboard.");
    } catch {
      toast.error("Failed to copy token.");
    }
  }

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
          <span className="text-muted-foreground">
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
          <span className="text-muted-foreground">
            {row.original.lastUsedAt ? formatDateTime(row.original.lastUsedAt) : "—"}
          </span>
        ),
      },
      {
        accessorKey: "createdAt",
        header: "Created",
        cell: ({ row }) => (
          <span className="text-muted-foreground">{formatDate(row.original.createdAt)}</span>
        ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row }) =>
          row.original.revokedAt ? null : (
            <div className="flex justify-end">
              <Button onClick={() => setRevokeTarget(row.original)} size="sm" variant="outline">
                Revoke
              </Button>
            </div>
          ),
      },
    ],
    [projectsById],
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
        <CardContent>
          <Form {...form}>
            <form className="grid gap-4 md:max-w-xl" onSubmit={form.handleSubmit(onSubmit)}>
              <FormField
                control={form.control}
                name="name"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Token Name</FormLabel>
                    <FormControl>
                      <Input placeholder="e.g. ci-bot" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="scopeProjectId"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Token Scope</FormLabel>
                    <Select onValueChange={field.onChange} value={field.value}>
                      <FormControl>
                        <SelectTrigger className="w-full">
                          <SelectValue />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        <SelectItem value={NO_SCOPE}>No scope (system-wide)</SelectItem>
                        {projects.map((project) => (
                          <SelectItem key={project.id} value={project.id}>
                            {project.name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <div className="flex justify-end">
                <Button disabled={createMutation.isPending} type="submit">
                  {createMutation.isPending ? <Loader2 className="animate-spin" /> : null}
                  Mint Token
                </Button>
              </div>
            </form>
          </Form>
        </CardContent>
      </Card>

      {mintedToken ? (
        <Alert>
          <KeyRound className="size-4" />
          <AlertTitle>New Token: {mintedToken.name}</AlertTitle>
          <AlertDescription className="grid gap-3">
            <p>Copy this token now — it will not be shown again.</p>
            <code className="block break-all rounded-md bg-muted px-3 py-2 font-mono text-sm text-foreground">
              {mintedToken.token}
            </code>
            <div className="flex gap-2">
              <Button onClick={copyMintedToken} size="sm" variant="outline">
                {copied ? <Check /> : <Copy />}
                Copy
              </Button>
              <Button onClick={() => setMintedToken(null)} size="sm" variant="ghost">
                Dismiss
              </Button>
            </div>
          </AlertDescription>
        </Alert>
      ) : null}

      {tokens.length || tokensQuery.isLoading ? (
        <div className="grid gap-3">
          <h2 className="text-sm font-medium text-muted-foreground">Existing Tokens</h2>
          <DataTable columns={columns} data={tokens} isLoading={tokensQuery.isLoading} />
        </div>
      ) : (
        <EmptyState
          description="You haven't minted any API tokens yet."
          icon={KeyRound}
          title="No tokens"
        />
      )}

      <ConfirmDialog
        confirmLabel="Revoke token"
        description={
          revokeTarget
            ? `"${revokeTarget.name}" will stop working immediately. This cannot be undone.`
            : ""
        }
        destructive
        isPending={revokeMutation.isPending}
        onConfirm={async () => {
          if (!revokeTarget) return;
          try {
            await revokeMutation.mutateAsync({ tokenId: revokeTarget.id });
            toast.success(`Token "${revokeTarget.name}" revoked.`);
          } catch (error) {
            toast.error(error instanceof Error ? error.message : "Failed to revoke token.");
          }
        }}
        onOpenChange={(open) => {
          if (!open) setRevokeTarget(null);
        }}
        open={Boolean(revokeTarget)}
        title="Revoke this token?"
      />
    </div>
  );
}
