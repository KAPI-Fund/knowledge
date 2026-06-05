import { useEffect, useState } from "react";

import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "./queries";

export function SettingsPage() {
  const settings = useSystemSettingsQuery();
  const updateSettings = useUpdateSystemSettingsMutation();
  const [providerMode, setProviderMode] = useState("");
  const [language, setLanguage] = useState("");
  const [defaultQueryLimit, setDefaultQueryLimit] = useState("0");

  useEffect(() => {
    if (!settings.data) {
      return;
    }
    setProviderMode(settings.data.providerMode);
    setLanguage(settings.data.language);
    setDefaultQueryLimit(String(settings.data.defaultQueryLimit));
  }, [
    settings.data?.providerMode,
    settings.data?.language,
    settings.data?.defaultQueryLimit,
  ]);

  async function handleSave() {
    await updateSettings.mutateAsync({
      providerMode,
      language,
      defaultQueryLimit: Number(defaultQueryLimit),
    });
  }

  return (
    <section className="stack">
      <h1>Settings</h1>
      <label>
        Provider Mode
        <input value={providerMode} onChange={(event) => setProviderMode(event.target.value)} />
      </label>
      <label>
        Language
        <input value={language} onChange={(event) => setLanguage(event.target.value)} />
      </label>
      <label>
        Default Query Limit
        <input
          aria-label="Default Query Limit"
          value={defaultQueryLimit}
          onChange={(event) => setDefaultQueryLimit(event.target.value)}
        />
      </label>
      <button type="button" onClick={handleSave}>
        Save Settings
      </button>
    </section>
  );
}
