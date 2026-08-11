import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "../queries";
import { parseTimeoutSeconds } from "./parse-timeout";

const GPT_IMAGE_SIZES = ["1024x1024", "1024x1536", "1536x1024"];
const DALLE3_SIZES = ["1024x1024", "1024x1792", "1792x1024"];
const DALLE2_SIZES = ["256x256", "512x512", "1024x1024"];
const ALL_SIZES = [
  "256x256",
  "512x512",
  "1024x1024",
  "1024x1536",
  "1536x1024",
  "1024x1792",
  "1792x1024",
];

// Each OpenAI image model family only accepts a fixed set of sizes; offering an
// out-of-family size just produces a 400 at call time. Unknown models fall back
// to the full list so custom endpoints aren't blocked.
export function allowedSizesForModel(model: string): string[] {
  const m = model.trim().toLowerCase();
  if (m.startsWith("gpt-image")) return GPT_IMAGE_SIZES;
  if (m.startsWith("dall-e-3") || m.startsWith("dalle-3")) return DALLE3_SIZES;
  if (m.startsWith("dall-e-2") || m.startsWith("dalle-2")) return DALLE2_SIZES;
  return ALL_SIZES;
}

export function ImageSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();

  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [clearApiKey, setClearApiKey] = useState(false);
  const [model, setModel] = useState("");
  const [size, setSize] = useState("1024x1024");
  // Image generation is slow (gpt-image returns multi-MB base64 that takes tens
  // of seconds to a few minutes), so default the request timeout to 5 minutes.
  const [timeoutSeconds, setTimeoutSeconds] = useState("300");
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
    setTimeoutSeconds(String(img.timeoutSeconds ?? 300));
  }, [settings.data?.image]);

  useEffect(() => {
    const allowed = allowedSizesForModel(model);
    setSize((cur) => (allowed.includes(cur) ? cur : allowed[0]));
  }, [model]);

  const apiKeyConfigured = settings.data?.image?.apiKeyConfigured ?? false;

  async function save() {
    await update.mutateAsync({
      image: {
        baseUrl,
        model,
        size,
        timeoutSeconds: parseTimeoutSeconds(timeoutSeconds, 300),
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
          <Input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder="gpt-image-1, dall-e-3"
          />
        </label>
        <div className="grid gap-1.5">
          <Label htmlFor="image-size">Image Size</Label>
          <Select value={size} onValueChange={setSize}>
            <SelectTrigger id="image-size" aria-label="Image Size" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {allowedSizesForModel(model).map((s) => (
                <SelectItem key={s} value={s}>
                  {s}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
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
