import { useEffect, useRef, useState } from "react";

import { PageSection } from "../../components/layout/page-section";
import { Alert, AlertDescription, AlertTitle } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card, CardContent } from "../../components/ui/card";
import { Input } from "../../components/ui/input";

import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "./queries";

export function SettingsPage() {
  const settings = useSystemSettingsQuery();
  const updateSettings = useUpdateSystemSettingsMutation();
  const [providerMode, setProviderMode] = useState("");
  const [language, setLanguage] = useState("");
  const [defaultQueryLimit, setDefaultQueryLimit] = useState("0");
  const [providerBaseUrl, setProviderBaseUrl] = useState("");
  const [providerApiKey, setProviderApiKey] = useState("");
  const [providerModel, setProviderModel] = useState("");
  const [providerEmbeddingModel, setProviderEmbeddingModel] = useState("");
  const [providerTimeoutSeconds, setProviderTimeoutSeconds] = useState("60");
  const [saveMessage, setSaveMessage] = useState("");
  const [isDirty, setIsDirty] = useState(false);
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    if (!settings.data || isDirty) {
      return;
    }
    const snapshot = JSON.stringify(settings.data);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    setProviderMode(settings.data.providerMode);
    setLanguage(settings.data.language);
    setDefaultQueryLimit(String(settings.data.defaultQueryLimit));
    setProviderBaseUrl(settings.data.providerBaseUrl ?? "");
    setProviderModel(settings.data.providerModel ?? "");
    setProviderEmbeddingModel(settings.data.providerEmbeddingModel ?? "");
    setProviderTimeoutSeconds(String(settings.data.providerTimeoutSeconds ?? 60));
  }, [
    settings.data?.providerMode,
    settings.data?.language,
    settings.data?.defaultQueryLimit,
    settings.data?.providerBaseUrl,
    settings.data?.providerModel,
    settings.data?.providerEmbeddingModel,
    settings.data?.providerTimeoutSeconds,
    isDirty,
  ]);

  async function handleSave() {
    const payload = {
      providerMode,
      language,
      defaultQueryLimit: Number(defaultQueryLimit),
      providerBaseUrl,
      providerModel,
      providerEmbeddingModel,
      providerTimeoutSeconds: Number(providerTimeoutSeconds),
      ...(providerApiKey.trim()
        ? {
            providerApiKey: providerApiKey.trim(),
          }
        : {}),
    };

    try {
      await updateSettings.mutateAsync(payload);
      setIsDirty(false);
      setSaveMessage("Settings saved.");
      setProviderApiKey("");
    } catch (error) {
      setSaveMessage(error instanceof Error ? error.message : "Failed to save settings.");
    }
  }

  return (
    <PageSection
      description="Configure the provider bridge and default system behavior for the admin workbench."
      title="Settings"
    >
      <Card className="panel">
        <CardContent className="grid gap-4 p-6">
          <label className="grid gap-2 text-sm font-medium">
            Provider Mode
            <Input
              onChange={(event) => {
                setIsDirty(true);
                setProviderMode(event.target.value);
              }}
              value={providerMode}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Language
            <Input
              onChange={(event) => {
                setIsDirty(true);
                setLanguage(event.target.value);
              }}
              value={language}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Default Query Limit
            <Input
              aria-label="Default Query Limit"
              onChange={(event) => {
                setIsDirty(true);
                setDefaultQueryLimit(event.target.value);
              }}
              value={defaultQueryLimit}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Provider Base URL
            <Input
              onChange={(event) => {
                setIsDirty(true);
                setProviderBaseUrl(event.target.value);
              }}
              value={providerBaseUrl}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Provider API Key
            <Input
              onChange={(event) => {
                setIsDirty(true);
                setProviderApiKey(event.target.value);
              }}
              placeholder="Leave blank to keep the current key"
              type="password"
              value={providerApiKey}
            />
          </label>
          {settings.data?.providerApiKeyConfigured ? (
            <p className="text-sm text-muted-foreground">API key configured</p>
          ) : null}
          <label className="grid gap-2 text-sm font-medium">
            Provider Model
            <Input
              onChange={(event) => {
                setIsDirty(true);
                setProviderModel(event.target.value);
              }}
              value={providerModel}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Provider Embedding Model
            <Input
              onChange={(event) => {
                setIsDirty(true);
                setProviderEmbeddingModel(event.target.value);
              }}
              value={providerEmbeddingModel}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Provider Timeout Seconds
            <Input
              onChange={(event) => {
                setIsDirty(true);
                setProviderTimeoutSeconds(event.target.value);
              }}
              value={providerTimeoutSeconds}
            />
          </label>
          {saveMessage ? (
            <Alert>
              <AlertTitle>{saveMessage === "Settings saved." ? "Saved" : "Update failed"}</AlertTitle>
              <AlertDescription>{saveMessage}</AlertDescription>
            </Alert>
          ) : null}
          <div className="flex justify-end">
            <Button onClick={handleSave}>Save Settings</Button>
          </div>
        </CardContent>
      </Card>
    </PageSection>
  );
}
