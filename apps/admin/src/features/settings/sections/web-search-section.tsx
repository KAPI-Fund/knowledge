import { useEffect, useRef, useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import {
  useRunWebSearchMutation,
  useSystemSettingsQuery,
  useUpdateSystemSettingsMutation,
} from "../queries";
import { useSettingsTopLevel } from "./use-settings-top-level";

// Editable per-provider fields. apiKey is always a plain editable string; blank
// means "keep the stored key" (backend deep-merge skips empty-string fields).
interface SearchFields {
  provider: string;
  tavilyApiKey: string;
  tavilyBaseUrl: string;
  serpapiApiKey: string;
  serpapiEngine: string;
  serpapiBaseUrl: string;
  searxngUrl: string;
  searxngCategories: string;
  ollamaApiKey: string;
  ollamaUrl: string;
}

const EMPTY: SearchFields = {
  provider: "tavily",
  tavilyApiKey: "",
  tavilyBaseUrl: "",
  serpapiApiKey: "",
  serpapiEngine: "google",
  serpapiBaseUrl: "",
  searxngUrl: "",
  searxngCategories: "general",
  ollamaApiKey: "",
  ollamaUrl: "",
};

export function WebSearchSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();
  const runWebSearch = useRunWebSearchMutation();
  const topLevel = useSettingsTopLevel(settings.data);

  const [fields, setFields] = useState<SearchFields>(EMPTY);
  const [testQuery, setTestQuery] = useState("");
  const [testError, setTestError] = useState("");
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    const search = settings.data?.search;
    if (!search) {
      return;
    }
    const snapshot = JSON.stringify(search);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    const p = search.providers ?? {};
    setFields({
      provider: search.provider ?? "tavily",
      tavilyApiKey: "",
      tavilyBaseUrl: p.tavily?.baseUrl ?? "",
      serpapiApiKey: "",
      serpapiEngine: p.serpapi?.engine ?? "google",
      serpapiBaseUrl: p.serpapi?.baseUrl ?? "",
      searxngUrl: p.searxng?.url ?? "",
      searxngCategories: (p.searxng?.categories ?? ["general"]).join(", "),
      ollamaApiKey: "",
      ollamaUrl: p.ollama?.url ?? "",
    });
  }, [settings.data?.search]);

  function set<K extends keyof SearchFields>(key: K, value: SearchFields[K]) {
    setFields((f) => ({ ...f, [key]: value }));
  }

  async function save() {
    const categories = fields.searxngCategories
      .split(",")
      .map((v) => v.trim())
      .filter((v) => v.length > 0);
    // Send every provider block so switching providers never drops a stored
    // field. apiKey = "" means keep; a typed value replaces (backend merge).
    await update.mutateAsync({
      ...topLevel,
      search: {
        provider: fields.provider,
        providers: {
          tavily: { apiKey: fields.tavilyApiKey.trim(), baseUrl: fields.tavilyBaseUrl },
          serpapi: {
            apiKey: fields.serpapiApiKey.trim(),
            engine: fields.serpapiEngine,
            baseUrl: fields.serpapiBaseUrl,
          },
          searxng: { url: fields.searxngUrl, categories: categories.length ? categories : ["general"] },
          ollama: { apiKey: fields.ollamaApiKey.trim(), url: fields.ollamaUrl },
        },
      },
    });
    setFields((f) => ({ ...f, tavilyApiKey: "", serpapiApiKey: "", ollamaApiKey: "" }));
  }

  const configured = settings.data?.search?.providers;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Web Search</CardTitle>
        <CardDescription>Search provider for the canvas search node.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <label className="grid gap-1.5 text-sm font-medium">
          Search Provider
          <Select
            aria-label="Search Provider"
            value={fields.provider}
            onChange={(e) => set("provider", e.target.value)}
          >
            <option value="none">none</option>
            <option value="tavily">tavily</option>
            <option value="serpapi">serpapi</option>
            <option value="searxng">searxng</option>
            <option value="ollama">ollama</option>
          </Select>
        </label>

        {fields.provider === "tavily" ? (
          <>
            <label className="grid gap-1.5 text-sm font-medium">
              Tavily API Key
              <Input
                type="password"
                placeholder={
                  configured?.tavily?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "tvly-..."
                }
                value={fields.tavilyApiKey}
                onChange={(e) => set("tavilyApiKey", e.target.value)}
              />
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              Tavily Base URL
              <Input
                value={fields.tavilyBaseUrl}
                onChange={(e) => set("tavilyBaseUrl", e.target.value)}
                placeholder="https://api.tavily.com"
              />
            </label>
          </>
        ) : null}

        {fields.provider === "serpapi" ? (
          <>
            <label className="grid gap-1.5 text-sm font-medium">
              SerpApi API Key
              <Input
                type="password"
                placeholder={
                  configured?.serpapi?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "..."
                }
                value={fields.serpapiApiKey}
                onChange={(e) => set("serpapiApiKey", e.target.value)}
              />
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              SerpApi Engine
              <Select
                aria-label="SerpApi Engine"
                value={fields.serpapiEngine}
                onChange={(e) => set("serpapiEngine", e.target.value)}
              >
                <option value="google">google</option>
                <option value="google_news">google_news</option>
                <option value="google_scholar">google_scholar</option>
                <option value="bing">bing</option>
                <option value="duckduckgo">duckduckgo</option>
              </Select>
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              SerpApi Base URL
              <Input
                value={fields.serpapiBaseUrl}
                onChange={(e) => set("serpapiBaseUrl", e.target.value)}
                placeholder="https://serpapi.com"
              />
            </label>
          </>
        ) : null}

        {fields.provider === "searxng" ? (
          <>
            <label className="grid gap-1.5 text-sm font-medium">
              SearXNG Instance URL
              <Input
                value={fields.searxngUrl}
                onChange={(e) => set("searxngUrl", e.target.value)}
                placeholder="https://search.example.com"
              />
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              SearXNG Categories (comma-separated)
              <Input
                value={fields.searxngCategories}
                onChange={(e) => set("searxngCategories", e.target.value)}
              />
            </label>
          </>
        ) : null}

        {fields.provider === "ollama" ? (
          <>
            <label className="grid gap-1.5 text-sm font-medium">
              Ollama API Key
              <Input
                type="password"
                placeholder={
                  configured?.ollama?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "..."
                }
                value={fields.ollamaApiKey}
                onChange={(e) => set("ollamaApiKey", e.target.value)}
              />
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              Ollama Search URL
              <Input
                value={fields.ollamaUrl}
                onChange={(e) => set("ollamaUrl", e.target.value)}
                placeholder="https://ollama.com"
              />
            </label>
          </>
        ) : null}

        <div className="flex justify-end">
          <Button onClick={save} disabled={update.isPending}>
            Save
          </Button>
        </div>

        <div className="grid gap-2 border-t pt-4">
          <label className="grid gap-1.5 text-sm font-medium">
            Test Query
            <Input
              value={testQuery}
              onChange={(e) => setTestQuery(e.target.value)}
              placeholder="e.g. knowledge graphs"
            />
          </label>
          <div className="flex gap-2">
            <Button
              variant="outline"
              disabled={runWebSearch.isPending || testQuery.trim().length === 0}
              onClick={async () => {
                setTestError("");
                try {
                  await runWebSearch.mutateAsync({ query: testQuery.trim() });
                } catch (error) {
                  setTestError(error instanceof Error ? error.message : "Web search failed.");
                }
              }}
            >
              Test Search
            </Button>
          </div>
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
  );
}
