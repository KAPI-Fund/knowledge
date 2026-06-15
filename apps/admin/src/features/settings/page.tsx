import { useEffect, useRef, useState } from "react";

import { PageSection } from "../../components/layout/page-section";
import { Alert, AlertDescription, AlertTitle } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card, CardContent } from "../../components/ui/card";
import { Input } from "../../components/ui/input";

import { useRunWebSearchMutation, useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "./queries";

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
  const [searchProvider, setSearchProvider] = useState("none");
  const [searchApiKey, setSearchApiKey] = useState("");
  const [serpapiEngine, setSerpapiEngine] = useState("google");
  const [searxngUrl, setSearxngUrl] = useState("");
  const [searxngCategories, setSearxngCategories] = useState("general");
  const [ollamaSearchUrl, setOllamaSearchUrl] = useState("");
  const [tavilyBaseUrl, setTavilyBaseUrl] = useState("");
  const [serpapiBaseUrl, setSerpapiBaseUrl] = useState("");
  const [testQuery, setTestQuery] = useState("");
  const [testError, setTestError] = useState("");
  const runWebSearch = useRunWebSearchMutation();
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
    setSearchProvider(settings.data.searchProvider ?? "none");
    setSerpapiEngine(settings.data.serpapiEngine ?? "google");
    setSearxngUrl(settings.data.searxngUrl ?? "");
    setSearxngCategories((settings.data.searxngCategories ?? ["general"]).join(", "));
    setOllamaSearchUrl(settings.data.ollamaSearchUrl ?? "");
    setTavilyBaseUrl(settings.data.tavilyBaseUrl ?? "");
    setSerpapiBaseUrl(settings.data.serpapiBaseUrl ?? "");
  }, [
    settings.data?.providerMode,
    settings.data?.language,
    settings.data?.defaultQueryLimit,
    settings.data?.providerBaseUrl,
    settings.data?.providerModel,
    settings.data?.providerEmbeddingModel,
    settings.data?.providerTimeoutSeconds,
    settings.data?.searchProvider,
    settings.data?.serpapiEngine,
    settings.data?.searxngUrl,
    settings.data?.searxngCategories,
    settings.data?.ollamaSearchUrl,
    settings.data?.tavilyBaseUrl,
    settings.data?.serpapiBaseUrl,
    isDirty,
  ]);

  useEffect(() => {
    if (isDirty) {
      runWebSearch.reset();
      setTestError("");
    }
  }, [isDirty, runWebSearch]);

  async function handleSave() {
    const categories = searxngCategories
      .split(",")
      .map((value) => value.trim())
      .filter((value) => value.length > 0);
    const payload = {
      providerMode,
      language,
      defaultQueryLimit: Number(defaultQueryLimit),
      providerBaseUrl,
      providerModel,
      providerEmbeddingModel,
      providerTimeoutSeconds: Number(providerTimeoutSeconds),
      searchProvider,
      serpapiEngine,
      searxngUrl,
      searxngCategories: categories.length > 0 ? categories : ["general"],
      ollamaSearchUrl,
      tavilyBaseUrl,
      serpapiBaseUrl,
      ...(providerApiKey.trim()
        ? {
            providerApiKey: providerApiKey.trim(),
          }
        : {}),
      ...(searchApiKey.trim()
        ? {
            searchApiKey: searchApiKey.trim(),
          }
        : {}),
    };

    try {
      await updateSettings.mutateAsync(payload);
      setIsDirty(false);
      setSaveMessage("Settings saved.");
      setProviderApiKey("");
      setSearchApiKey("");
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

      <Card className="panel">
        <CardContent className="grid gap-4 p-6">
          <h2 className="text-lg font-semibold">Web Search</h2>
          <label className="grid gap-2 text-sm font-medium">
            Search Provider
            <select
              aria-label="Search Provider"
              className="rounded border bg-background px-3 py-2"
              onChange={(event) => {
                setIsDirty(true);
                setSearchProvider(event.target.value);
              }}
              value={searchProvider}
            >
              <option value="none">none</option>
              <option value="tavily">tavily</option>
              <option value="serpapi">serpapi</option>
              <option value="searxng">searxng</option>
              <option value="ollama">ollama</option>
            </select>
          </label>
          {searchProvider !== "none" && searchProvider !== "searxng" ? (
            <label className="grid gap-2 text-sm font-medium">
              Search API Key
              <Input
                onChange={(event) => {
                  setIsDirty(true);
                  setSearchApiKey(event.target.value);
                }}
                placeholder="Leave blank to keep the current key"
                type="password"
                value={searchApiKey}
              />
            </label>
          ) : null}
          {settings.data?.searchApiKeyConfigured && searchProvider !== "none" && searchProvider !== "searxng" ? (
            <p className="text-sm text-muted-foreground">Search API key configured</p>
          ) : null}
          {searchProvider === "serpapi" ? (
            <label className="grid gap-2 text-sm font-medium">
              SerpApi Engine
              <select
                aria-label="SerpApi Engine"
                className="rounded border bg-background px-3 py-2"
                onChange={(event) => {
                  setIsDirty(true);
                  setSerpapiEngine(event.target.value);
                }}
                value={serpapiEngine}
              >
                <option value="google">google</option>
                <option value="google_news">google_news</option>
                <option value="google_scholar">google_scholar</option>
                <option value="bing">bing</option>
                <option value="duckduckgo">duckduckgo</option>
              </select>
            </label>
          ) : null}
          {searchProvider === "searxng" ? (
            <>
              <label className="grid gap-2 text-sm font-medium">
                SearXNG Instance URL
                <Input
                  onChange={(event) => {
                    setIsDirty(true);
                    setSearxngUrl(event.target.value);
                  }}
                  placeholder="https://search.example.com"
                  value={searxngUrl}
                />
              </label>
              <label className="grid gap-2 text-sm font-medium">
                SearXNG Categories (comma-separated)
                <Input
                  onChange={(event) => {
                    setIsDirty(true);
                    setSearxngCategories(event.target.value);
                  }}
                  value={searxngCategories}
                />
              </label>
            </>
          ) : null}
          {searchProvider === "ollama" ? (
            <label className="grid gap-2 text-sm font-medium">
              Ollama Search URL
              <Input
                onChange={(event) => {
                  setIsDirty(true);
                  setOllamaSearchUrl(event.target.value);
                }}
                placeholder="https://ollama.com"
                value={ollamaSearchUrl}
              />
            </label>
          ) : null}
          {searchProvider === "tavily" ? (
            <label className="grid gap-2 text-sm font-medium">
              Tavily Base URL (advanced)
              <Input
                onChange={(event) => {
                  setIsDirty(true);
                  setTavilyBaseUrl(event.target.value);
                }}
                placeholder="https://api.tavily.com"
                value={tavilyBaseUrl}
              />
            </label>
          ) : null}
          {searchProvider === "serpapi" ? (
            <label className="grid gap-2 text-sm font-medium">
              SerpApi Base URL (advanced)
              <Input
                onChange={(event) => {
                  setIsDirty(true);
                  setSerpapiBaseUrl(event.target.value);
                }}
                placeholder="https://serpapi.com"
                value={serpapiBaseUrl}
              />
            </label>
          ) : null}
          <div className="grid gap-2 border-t pt-4">
            <label className="grid gap-2 text-sm font-medium">
              Test Query
              <Input
                onChange={(event) => setTestQuery(event.target.value)}
                placeholder="e.g. knowledge graphs"
                value={testQuery}
              />
            </label>
            <div className="flex gap-2">
              <Button
                disabled={
                  runWebSearch.isPending ||
                  testQuery.trim().length === 0 ||
                  isDirty
                }
                onClick={async () => {
                  setTestError("");
                  try {
                    await runWebSearch.mutateAsync({ query: testQuery.trim() });
                  } catch (error) {
                    setTestError(error instanceof Error ? error.message : "Web search failed.");
                  }
                }}
                variant="outline"
              >
                Test Search
              </Button>
            </div>
            {isDirty ? (
              <p className="text-xs text-muted-foreground">
                Save settings before testing — current config in the form differs from the saved config.
              </p>
            ) : null}
            {testError ? (
              <Alert>
                <AlertTitle>Web search failed</AlertTitle>
                <AlertDescription>{testError}</AlertDescription>
              </Alert>
            ) : null}
            {runWebSearch.data?.results.length ? (
              <ul className="grid gap-3">
                {runWebSearch.data.results.map((result) => (
                  <li key={result.url} className="rounded border p-3">
                    <a className="font-medium" href={result.url} rel="noreferrer" target="_blank">
                      {result.title}
                    </a>
                    {result.source ? (
                      <p className="text-xs text-muted-foreground">{result.source}</p>
                    ) : null}
                    {result.snippet ? <p className="mt-1 text-sm">{result.snippet}</p> : null}
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </CardContent>
      </Card>
    </PageSection>
  );
}
