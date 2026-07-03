import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "../queries";
import { useSettingsTopLevel } from "./use-settings-top-level";

const SIZES = ["256x256", "512x512", "1024x1024", "1024x1792", "1792x1024"];

export function ImageSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();
  const topLevel = useSettingsTopLevel(settings.data);

  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [size, setSize] = useState("1024x1024");
  const [timeoutSeconds, setTimeoutSeconds] = useState("60");
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    const img = settings.data?.image;
    if (!img) {
      return;
    }
    const snapshot = JSON.stringify(img);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    setBaseUrl(img.baseUrl ?? "");
    setModel(img.model ?? "");
    setSize(img.size ?? "1024x1024");
    setTimeoutSeconds(String(img.timeoutSeconds ?? 60));
  }, [settings.data?.image]);

  async function save() {
    await update.mutateAsync({
      ...topLevel,
      image: {
        baseUrl,
        model,
        size,
        timeoutSeconds: Number(timeoutSeconds),
        ...(apiKey.trim() ? { apiKey: apiKey.trim() } : {}),
      },
    });
    setApiKey("");
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Image Generation</CardTitle>
        <CardDescription>OpenAI-compatible image endpoint for the canvas image node.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <label className="grid gap-1.5 text-sm font-medium">
          Base URL
          <Input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          API Key
          <Input
            type="password"
            placeholder={
              settings.data?.image?.apiKeyConfigured
                ? "Leave blank to keep the current key"
                : "sk-..."
            }
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
          />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Model
          <Input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder="gpt-image-1, dall-e-3"
          />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Image Size
          <Select aria-label="Image Size" value={size} onChange={(e) => setSize(e.target.value)}>
            {SIZES.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </Select>
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Timeout Seconds
          <Input value={timeoutSeconds} onChange={(e) => setTimeoutSeconds(e.target.value)} />
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
