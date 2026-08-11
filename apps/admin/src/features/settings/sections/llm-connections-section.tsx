import { Plus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/shared/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Switch } from "@/components/ui/switch";
import type { ProviderConnection } from "../../shared/api";
import {
  useActivateConnectionMutation,
  useCreateConnectionMutation,
  useDeleteConnectionMutation,
  useSystemSettingsQuery,
  useUpdateConnectionMutation,
} from "../queries";
import { parseTimeoutSeconds } from "./parse-timeout";

interface DraftFields {
  label: string;
  baseUrl: string;
  apiKey: string;
  clearApiKey: boolean;
  model: string;
  timeoutSeconds: string;
}

const EMPTY_DRAFT: DraftFields = {
  label: "",
  baseUrl: "",
  apiKey: "",
  clearApiKey: false,
  model: "",
  timeoutSeconds: "60",
};

/// Backend rejects blank label/base_url/model with a 400; mirror that here so the
/// Create/Save buttons stay disabled until the required fields are filled.
function draftIsComplete(fields: DraftFields): boolean {
  return (
    fields.label.trim() !== "" &&
    fields.baseUrl.trim() !== "" &&
    fields.model.trim() !== ""
  );
}

function ConnectionForm({
  fields,
  onChange,
  showKeyConfigured,
}: {
  fields: DraftFields;
  onChange: (next: DraftFields) => void;
  showKeyConfigured?: boolean;
}) {
  return (
    <div className="grid gap-3">
      <label className="grid gap-1.5 text-sm font-medium">
        Label
        <Input value={fields.label} onChange={(e) => onChange({ ...fields, label: e.target.value })} />
      </label>
      <label className="grid gap-1.5 text-sm font-medium">
        Base URL
        <Input value={fields.baseUrl} onChange={(e) => onChange({ ...fields, baseUrl: e.target.value })} />
      </label>
      <label className="grid gap-1.5 text-sm font-medium">
        API Key
        <Input
          type="password"
          placeholder={
            fields.clearApiKey
              ? "Saved key will be removed on save"
              : showKeyConfigured
                ? "Leave blank to keep the current key"
                : "sk-..."
          }
          disabled={fields.clearApiKey}
          value={fields.apiKey}
          onChange={(e) => onChange({ ...fields, apiKey: e.target.value })}
        />
      </label>
      {showKeyConfigured ? (
        <label className="flex items-center justify-between text-sm font-medium">
          <span>Clear saved key</span>
          <Switch
            aria-label="Clear saved key"
            checked={fields.clearApiKey}
            onCheckedChange={(checked) =>
              onChange({ ...fields, clearApiKey: checked, apiKey: checked ? "" : fields.apiKey })
            }
          />
        </label>
      ) : null}
      <label className="grid gap-1.5 text-sm font-medium">
        Model
        <Input value={fields.model} onChange={(e) => onChange({ ...fields, model: e.target.value })} />
      </label>
      <label className="grid gap-1.5 text-sm font-medium">
        Timeout Seconds
        <Input
          type="number"
          min={1}
          value={fields.timeoutSeconds}
          onChange={(e) => onChange({ ...fields, timeoutSeconds: e.target.value })}
        />
      </label>
    </div>
  );
}

function ConnectionRow({ connection }: { connection: ProviderConnection }) {
  const activate = useActivateConnectionMutation();
  const update = useUpdateConnectionMutation();
  const remove = useDeleteConnectionMutation();
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [fields, setFields] = useState<DraftFields>({
    label: connection.label,
    baseUrl: connection.baseUrl,
    apiKey: "",
    clearApiKey: false,
    model: connection.model,
    timeoutSeconds: String(connection.timeoutSeconds ?? 60),
  });

  async function save() {
    await update.mutateAsync({
      id: connection.id,
      label: fields.label,
      baseUrl: fields.baseUrl,
      model: fields.model,
      timeoutSeconds: parseTimeoutSeconds(fields.timeoutSeconds, 60),
      ...(fields.apiKey.trim()
        ? { apiKey: fields.apiKey.trim() }
        : fields.clearApiKey
          ? { clearApiKey: true }
          : {}),
    });
    setFields((f) => ({ ...f, apiKey: "", clearApiKey: false }));
    setEditing(false);
  }

  return (
    <li className="rounded-lg border p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <RadioGroupItem
            aria-label={`Activate ${connection.label}`}
            value={connection.id}
          />
          <div className="min-w-0">
            <p className="truncate font-medium">{connection.label}</p>
            <p className="truncate font-mono text-xs text-muted-foreground">{connection.model}</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant={connection.isActive ? "default" : "secondary"}>
            {connection.isActive ? "Active" : "Configured"}
          </Badge>
          {!connection.isActive ? (
            <Button size="sm" variant="outline" onClick={() => activate.mutateAsync(connection.id)}>
              Activate
            </Button>
          ) : null}
          <Button size="sm" variant="ghost" onClick={() => setEditing((v) => !v)}>
            Edit
          </Button>
          <Button
            size="sm"
            variant="ghost"
            className="text-destructive"
            onClick={() => setConfirmDelete(true)}
          >
            Delete
          </Button>
        </div>
      </div>
      {editing ? (
        <div className="mt-3 border-t pt-3">
          <ConnectionForm fields={fields} onChange={setFields} showKeyConfigured={connection.apiKeyConfigured} />
          <div className="mt-3 flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={() => setEditing(false)}>
              Cancel
            </Button>
            <Button size="sm" onClick={save} disabled={update.isPending || !draftIsComplete(fields)}>
              Save
            </Button>
          </div>
        </div>
      ) : null}
      <ConfirmDialog
        confirmLabel="Delete connection"
        description={`"${connection.label}" will be removed. Chat and analyze will stop working until another connection is activated.`}
        destructive
        isPending={remove.isPending}
        onConfirm={async () => {
          try {
            await remove.mutateAsync(connection.id);
            toast.success(`Connection "${connection.label}" deleted.`);
          } catch (error) {
            toast.error(error instanceof Error ? error.message : "Failed to delete connection.");
          }
        }}
        onOpenChange={setConfirmDelete}
        open={confirmDelete}
        title="Delete this connection?"
      />
    </li>
  );
}

export function LlmConnectionsSection() {
  const settings = useSystemSettingsQuery();
  const create = useCreateConnectionMutation();
  const activate = useActivateConnectionMutation();
  const connections = settings.data?.connections ?? [];
  const [draft, setDraft] = useState<DraftFields | null>(null);
  const activeId = connections.find((c) => c.isActive)?.id ?? "";

  async function createDraft() {
    if (!draft) {
      return;
    }
    await create.mutateAsync({
      label: draft.label,
      baseUrl: draft.baseUrl,
      model: draft.model,
      timeoutSeconds: parseTimeoutSeconds(draft.timeoutSeconds, 60),
      ...(draft.apiKey.trim() ? { apiKey: draft.apiKey.trim() } : {}),
    });
    setDraft(null);
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <div>
          <CardTitle>LLM Connections</CardTitle>
          <CardDescription>Chat and analyze use the active connection.</CardDescription>
        </div>
        <Button size="sm" onClick={() => setDraft(EMPTY_DRAFT)} disabled={draft !== null}>
          <Plus />
          Add
        </Button>
      </CardHeader>
      <CardContent className="grid gap-3">
        {connections.length === 0 && !draft ? (
          <p className="text-sm text-muted-foreground">No connections configured yet.</p>
        ) : null}
        <ScrollArea className="max-h-[420px] pr-2">
          <RadioGroup
            onValueChange={(id) => {
              if (id !== activeId) void activate.mutateAsync(id);
            }}
            value={activeId}
          >
            <ul className="grid gap-2">
              {connections.map((c) => (
                <ConnectionRow key={c.id} connection={c} />
              ))}
            </ul>
          </RadioGroup>
        </ScrollArea>
        {draft ? (
          <div className="rounded-lg border border-dashed p-3">
            <ConnectionForm fields={draft} onChange={setDraft} />
            <div className="mt-3 flex justify-end gap-2">
              <Button size="sm" variant="ghost" onClick={() => setDraft(null)}>
                Cancel
              </Button>
              <Button size="sm" onClick={createDraft} disabled={create.isPending || !draftIsComplete(draft)}>
                Create
              </Button>
            </div>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
