import { useEffect, useRef, useState } from "react";

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
    await updateSettings.mutateAsync({
      providerMode,
      language,
      defaultQueryLimit: Number(defaultQueryLimit),
      providerBaseUrl,
      providerApiKey,
      providerModel,
      providerEmbeddingModel,
      providerTimeoutSeconds: Number(providerTimeoutSeconds),
    });
    setIsDirty(false);
  }

  return (
    <section className="stack">
      <h1>Settings</h1>
      <label>
        Provider Mode
        <input
          value={providerMode}
          onChange={(event) => {
            setIsDirty(true);
            setProviderMode(event.target.value);
          }}
        />
      </label>
      <label>
        Language
        <input
          value={language}
          onChange={(event) => {
            setIsDirty(true);
            setLanguage(event.target.value);
          }}
        />
      </label>
      <label>
        Default Query Limit
        <input
          aria-label="Default Query Limit"
          value={defaultQueryLimit}
          onChange={(event) => {
            setIsDirty(true);
            setDefaultQueryLimit(event.target.value);
          }}
        />
      </label>
      <label>
        Provider Base URL
        <input
          value={providerBaseUrl}
          onChange={(event) => {
            setIsDirty(true);
            setProviderBaseUrl(event.target.value);
          }}
        />
      </label>
      <label>
        Provider API Key
        <input
          value={providerApiKey}
          onChange={(event) => {
            setIsDirty(true);
            setProviderApiKey(event.target.value);
          }}
        />
      </label>
      {settings.data?.providerApiKeyConfigured ? <span>API key configured</span> : null}
      <label>
        Provider Model
        <input
          value={providerModel}
          onChange={(event) => {
            setIsDirty(true);
            setProviderModel(event.target.value);
          }}
        />
      </label>
      <label>
        Provider Embedding Model
        <input
          value={providerEmbeddingModel}
          onChange={(event) => {
            setIsDirty(true);
            setProviderEmbeddingModel(event.target.value);
          }}
        />
      </label>
      <label>
        Provider Timeout Seconds
        <input
          value={providerTimeoutSeconds}
          onChange={(event) => {
            setIsDirty(true);
            setProviderTimeoutSeconds(event.target.value);
          }}
        />
      </label>
      <button type="button" onClick={handleSave}>
        Save Settings
      </button>
    </section>
  );
}
