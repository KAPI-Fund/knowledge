import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "../queries";
import { parseTimeoutSeconds } from "./parse-timeout";

export function EmbeddingSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();

  const [enabled, setEnabled] = useState(false);
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [clearApiKey, setClearApiKey] = useState(false);
  const [model, setModel] = useState("");
  const [timeoutSeconds, setTimeoutSeconds] = useState("60");
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    const emb = settings.data?.embedding;
    if (!emb) {
      return;
    }
    const snapshot = JSON.stringify(emb);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    setEnabled(emb.enabled);
    setBaseUrl(emb.baseUrl ?? "");
    setModel(emb.model ?? "");
    setTimeoutSeconds(String(emb.timeoutSeconds ?? 60));
  }, [settings.data?.embedding]);

  const apiKeyConfigured = settings.data?.embedding?.apiKeyConfigured ?? false;

  async function save() {
    await update.mutateAsync({
      embedding: {
        enabled,
        baseUrl,
        model,
        timeoutSeconds: parseTimeoutSeconds(timeoutSeconds, 60),
        ...(apiKey.trim()
          ? { apiKey: apiKey.trim() }
          : clearApiKey
            ? { clearApiKey: true }
            : {}),
      },
    });
    setApiKey("");
    setClearApiKey(false);
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Embedding</CardTitle>
        <CardDescription>Independent endpoint for retrieval / RAG embeddings.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <label className="flex items-center justify-between text-sm font-medium">
          <span>Enable embedding</span>
          <Switch aria-label="Enable embedding" checked={enabled} onCheckedChange={setEnabled} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Base URL
          <Input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          API Key
          <Input
            type="password"
            placeholder={
              clearApiKey
                ? "Saved key will be removed on save"
                : apiKeyConfigured
                  ? "Leave blank to keep the current key"
                  : "sk-..."
            }
            disabled={clearApiKey}
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
          />
        </label>
        {apiKeyConfigured ? (
          <label className="flex items-center justify-between text-sm font-medium">
            <span>Clear saved key</span>
            <Switch
              aria-label="Clear saved key"
              checked={clearApiKey}
              onCheckedChange={(checked) => {
                setClearApiKey(checked);
                if (checked) {
                  setApiKey("");
                }
              }}
            />
          </label>
        ) : null}
        <label className="grid gap-1.5 text-sm font-medium">
          Model
          <Input value={model} onChange={(e) => setModel(e.target.value)} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Timeout Seconds
          <Input
            type="number"
            min={1}
            value={timeoutSeconds}
            onChange={(e) => setTimeoutSeconds(e.target.value)}
          />
        </label>
        <div className="flex justify-end">
          <Button onClick={save} disabled={update.isPending}>
            Save
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
