import { Copy } from "lucide-react";
import { Link } from "react-router-dom";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { normalizeAppError } from "@/lib/app-error";

import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "../queries";

export function McpSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();
  const enabled = settings.data?.mcp?.enabled ?? false;

  const origin = window.location.origin;
  const endpoint = `${origin}/api/mcp`;
  const configSnippet = JSON.stringify(
    {
      mcpServers: {
        knowledge: {
          type: "http",
          url: endpoint,
          headers: { Authorization: "Bearer <api-token>" },
        },
      },
    },
    null,
    2,
  );
  const cliCommand = `claude mcp add --transport http knowledge ${endpoint} --header "Authorization: Bearer <api-token>"`;

  async function copyText(label: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(`${label} copied to clipboard.`);
    } catch {
      toast.error("Failed to copy to clipboard.");
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>MCP</CardTitle>
        <CardDescription>
          Expose this server as an MCP server so agent clients such as Claude Desktop and Claude
          Code can search, read, and chat with your knowledge projects.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="flex items-center justify-between">
          <Label htmlFor="mcp-enabled">Enable MCP access</Label>
          <Switch
            aria-label="Enable MCP access"
            checked={enabled}
            disabled={settings.isLoading || update.isPending}
            id="mcp-enabled"
            onCheckedChange={async (next) => {
              try {
                await update.mutateAsync({ mcp: { enabled: next } });
                toast.success(next ? "MCP access enabled." : "MCP access disabled.");
              } catch (error) {
                toast.error(normalizeAppError(error).message);
              }
            }}
          />
        </div>

        {enabled ? (
          <div className="grid gap-4">
            <div className="grid gap-1.5">
              <Label>Endpoint</Label>
              <code className="rounded-md border bg-muted px-3 py-2 text-sm">{endpoint}</code>
            </div>

            <div className="grid gap-1.5">
              <div className="flex items-center justify-between">
                <Label>Claude Desktop configuration</Label>
                <Button
                  onClick={() => copyText("Configuration", configSnippet)}
                  size="sm"
                  variant="ghost"
                >
                  <Copy /> Copy
                </Button>
              </div>
              <pre className="overflow-x-auto rounded-md border bg-muted px-3 py-2 text-sm">
                {configSnippet}
              </pre>
            </div>

            <div className="grid gap-1.5">
              <div className="flex items-center justify-between">
                <Label>Claude Code command</Label>
                <Button
                  onClick={() => copyText("Command", cliCommand)}
                  size="sm"
                  variant="ghost"
                >
                  <Copy /> Copy
                </Button>
              </div>
              <pre className="overflow-x-auto rounded-md border bg-muted px-3 py-2 text-sm">
                {cliCommand}
              </pre>
            </div>

            <p className="text-sm text-muted-foreground">
              Replace <code>&lt;api-token&gt;</code> with a token from the{" "}
              <Link className="underline underline-offset-4 hover:text-foreground" to="/api-tokens">
                API tokens
              </Link>{" "}
              page. Project-scoped tokens limit MCP tools to that project.
            </p>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
