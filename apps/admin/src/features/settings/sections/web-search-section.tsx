import { useEffect, useRef, useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  useRunWebSearchMutation,
  useSystemSettingsQuery,
  useUpdateSystemSettingsMutation,
} from "../queries";

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

  const [fields, setFields] = useState<SearchFields>(EMPTY);
  // Per-provider "remove the stored api key" toggles. Separate from `fields`
  // because a key can only ever be kept, replaced, or cleared — never edited in
  // place (GET redacts it). When on, save sends apiKey=null (backend clears it).
  const [clearKeys, setClearKeys] = useState({ tavily: false, serpapi: false, ollama: false });
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
    setClearKeys({ tavily: false, serpapi: false, ollama: false });
  }, [settings.data?.search]);

  function set<K extends keyof SearchFields>(key: K, value: SearchFields[K]) {
    setFields((f) => ({ ...f, [key]: value }));
  }

  async function save() {
    const categories = fields.searxngCategories
      .split(",")
      .map((v) => v.trim())
      .filter((v) => v.length > 0);
    // Visible URL fields: a blank box means "clear it" (revert to the provider
    // default) -> send null. A typed value sets it. api.ts passes null through
    // and the backend merge drops the stored field.
    const urlOrClear = (v: string): string | null => (v.trim() === "" ? null : v.trim());
    // api key: clear toggle wins (null = remove stored key); otherwise "" keeps
    // the stored key and a typed value replaces it.
    const keyValue = (cleared: boolean, typed: string): string | null =>
      cleared ? null : typed.trim();
    // Send every provider block so switching providers never drops a stored
    // field.
    await update.mutateAsync({
      search: {
        provider: fields.provider,
        providers: {
          tavily: {
            apiKey: keyValue(clearKeys.tavily, fields.tavilyApiKey),
            baseUrl: urlOrClear(fields.tavilyBaseUrl),
          },
          serpapi: {
            apiKey: keyValue(clearKeys.serpapi, fields.serpapiApiKey),
            engine: fields.serpapiEngine,
            baseUrl: urlOrClear(fields.serpapiBaseUrl),
          },
          searxng: { url: urlOrClear(fields.searxngUrl), categories: categories.length ? categories : ["general"] },
          ollama: {
            apiKey: keyValue(clearKeys.ollama, fields.ollamaApiKey),
            url: urlOrClear(fields.ollamaUrl),
          },
        },
      },
    });
    setFields((f) => ({ ...f, tavilyApiKey: "", serpapiApiKey: "", ollamaApiKey: "" }));
    setClearKeys({ tavily: false, serpapi: false, ollama: false });
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
                disabled={clearKeys.tavily}
                placeholder={
                  configured?.tavily?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "tvly-..."
                }
                value={fields.tavilyApiKey}
                onChange={(e) => set("tavilyApiKey", e.target.value)}
              />
            </label>
            {configured?.tavily?.apiKeyConfigured ? (
              <label className="flex items-center justify-between text-sm text-muted-foreground">
                <span>Clear stored key</span>
                <Switch
                  aria-label="Clear stored Tavily key"
                  checked={clearKeys.tavily}
                  onCheckedChange={(v) => setClearKeys((c) => ({ ...c, tavily: v }))}
                />
              </label>
            ) : null}
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
                disabled={clearKeys.serpapi}
                placeholder={
                  configured?.serpapi?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "..."
                }
                value={fields.serpapiApiKey}
                onChange={(e) => set("serpapiApiKey", e.target.value)}
              />
            </label>
            {configured?.serpapi?.apiKeyConfigured ? (
              <label className="flex items-center justify-between text-sm text-muted-foreground">
                <span>Clear stored key</span>
                <Switch
                  aria-label="Clear stored SerpApi key"
                  checked={clearKeys.serpapi}
                  onCheckedChange={(v) => setClearKeys((c) => ({ ...c, serpapi: v }))}
                />
              </label>
            ) : null}
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
                disabled={clearKeys.ollama}
                placeholder={
                  configured?.ollama?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "..."
                }
                value={fields.ollamaApiKey}
                onChange={(e) => set("ollamaApiKey", e.target.value)}
              />
            </label>
            {configured?.ollama?.apiKeyConfigured ? (
              <label className="flex items-center justify-between text-sm text-muted-foreground">
                <span>Clear stored key</span>
                <Switch
                  aria-label="Clear stored Ollama key"
                  checked={clearKeys.ollama}
                  onCheckedChange={(v) => setClearKeys((c) => ({ ...c, ollama: v }))}
                />
              </label>
            ) : null}
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
            <ScrollArea className="max-h-72 pr-2">
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
            </ScrollArea>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}
