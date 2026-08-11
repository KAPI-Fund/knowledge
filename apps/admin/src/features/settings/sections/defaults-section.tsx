import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "../queries";
import { parseQueryLimit } from "./parse-query-limit";

export function DefaultsSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();

  const [language, setLanguage] = useState("en");
  const [defaultQueryLimit, setDefaultQueryLimit] = useState("5");
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    const defaults = settings.data?.defaults;
    if (!defaults) {
      return;
    }
    const snapshot = JSON.stringify(defaults);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    setLanguage(defaults.language);
    setDefaultQueryLimit(String(defaults.defaultQueryLimit));
  }, [settings.data?.defaults]);

  async function save() {
    // On invalid/empty input, fall back to the currently-saved limit (not a fixed
    // constant) so a stray edit can't silently reset a saved 25 down to 5.
    const savedLimit = settings.data?.defaults?.defaultQueryLimit ?? 5;
    const limit = parseQueryLimit(defaultQueryLimit, savedLimit);
    // Reflect the coerced value back into the field so the user sees exactly what
    // was persisted (e.g. a cleared/invalid box snaps back to the saved value).
    setDefaultQueryLimit(String(limit));
    await update.mutateAsync({
      defaults: { language, defaultQueryLimit: limit },
    });
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Defaults</CardTitle>
        <CardDescription>Language and default query behavior.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <label className="grid gap-1.5 text-sm font-medium">
          Language
          <Input value={language} onChange={(e) => setLanguage(e.target.value)} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Default Query Limit
          <Input
            type="number"
            min={1}
            max={100}
            aria-label="Default Query Limit"
            value={defaultQueryLimit}
            onChange={(e) => setDefaultQueryLimit(e.target.value)}
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
