import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "../queries";

// Editable firecrawl fields. apiKey is always a plain editable string; blank
// means "keep the stored key" (backend deep-merge skips empty-string fields).
interface FetchFields {
  provider: string;
  firecrawlApiKey: string;
  firecrawlBaseUrl: string;
}

const EMPTY: FetchFields = {
  provider: "firecrawl",
  firecrawlApiKey: "",
  firecrawlBaseUrl: "",
};

export function FetchSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();

  const [fields, setFields] = useState<FetchFields>(EMPTY);
  // "Remove the stored api key" toggle. Separate from `fields` because a key can
  // only ever be kept, replaced, or cleared — never edited in place (GET redacts
  // it). When on, save sends apiKey=null (backend clears it).
  const [clearKey, setClearKey] = useState(false);
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    const fetch = settings.data?.fetch;
    if (!fetch) {
      return;
    }
    const snapshot = JSON.stringify(fetch);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    const p = fetch.providers ?? {};
    setFields({
      provider: fetch.provider ?? "firecrawl",
      firecrawlApiKey: "",
      firecrawlBaseUrl: p.firecrawl?.baseUrl ?? "",
    });
    setClearKey(false);
  }, [settings.data?.fetch]);

  function set<K extends keyof FetchFields>(key: K, value: FetchFields[K]) {
    setFields((f) => ({ ...f, [key]: value }));
  }

  async function save() {
    // Visible URL field: a blank box means "clear it" (revert to the firecrawl
    // default) -> send null. A typed value sets it. api.ts passes null through
    // and the backend merge drops the stored field.
    const urlOrClear = (v: string): string | null => (v.trim() === "" ? null : v.trim());
    // api key: clear toggle wins (null = remove stored key); otherwise "" keeps
    // the stored key and a typed value replaces it.
    const keyValue = (cleared: boolean, typed: string): string | null =>
      cleared ? null : typed.trim();
    await update.mutateAsync({
      fetch: {
        provider: fields.provider,
        providers: {
          firecrawl: {
            apiKey: keyValue(clearKey, fields.firecrawlApiKey),
            baseUrl: urlOrClear(fields.firecrawlBaseUrl),
          },
        },
      },
    });
    setFields((f) => ({ ...f, firecrawlApiKey: "" }));
    setClearKey(false);
  }

  const configured = settings.data?.fetch?.providers;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Web Fetch</CardTitle>
        <CardDescription>Scraping provider for the canvas URL node.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <label className="grid gap-1.5 text-sm font-medium">
          Fetch Provider
          <Select
            aria-label="Fetch Provider"
            value={fields.provider}
            onChange={(e) => set("provider", e.target.value)}
          >
            <option value="none">none</option>
            <option value="firecrawl">firecrawl</option>
          </Select>
        </label>

        {fields.provider === "firecrawl" ? (
          <>
            <label className="grid gap-1.5 text-sm font-medium">
              Firecrawl API Key
              <Input
                type="password"
                disabled={clearKey}
                placeholder={
                  configured?.firecrawl?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "fc-... (optional for self-hosted)"
                }
                value={fields.firecrawlApiKey}
                onChange={(e) => set("firecrawlApiKey", e.target.value)}
              />
            </label>
            {configured?.firecrawl?.apiKeyConfigured ? (
              <label className="flex items-center justify-between text-sm text-muted-foreground">
                <span>Clear stored key</span>
                <Switch
                  aria-label="Clear stored Firecrawl key"
                  checked={clearKey}
                  onCheckedChange={(v) => setClearKey(v)}
                />
              </label>
            ) : null}
            <label className="grid gap-1.5 text-sm font-medium">
              Firecrawl Base URL
              <Input
                value={fields.firecrawlBaseUrl}
                onChange={(e) => set("firecrawlBaseUrl", e.target.value)}
                placeholder="https://api.firecrawl.dev"
              />
            </label>
          </>
        ) : null}

        <div className="flex justify-end">
          <Button onClick={save} disabled={update.isPending}>
            Save
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
