import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";

import { RouteStatePane } from "@/components/layout/route-state-pane";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { normalizeAppError } from "@/lib/app-error";

import {
  useProjectSourceWatchQuery,
  useScanProjectSourceWatchMutation,
  useUpdateProjectSourceWatchMutation,
} from "./queries";

export function SourceWatchPage() {
  const { projectId = "" } = useParams();
  const settings = useProjectSourceWatchQuery(projectId);
  const updateSettings = useUpdateProjectSourceWatchMutation();
  const scanSourceWatch = useScanProjectSourceWatchMutation();
  const [enabled, setEnabled] = useState(false);
  const [autoIngest, setAutoIngest] = useState(true);
  const [path, setPath] = useState("");
  const [includeExtensions, setIncludeExtensions] = useState("");
  const [excludeExtensions, setExcludeExtensions] = useState("");
  const [excludeDirs, setExcludeDirs] = useState("");
  const [excludeGlobs, setExcludeGlobs] = useState("");
  const [maxFileSizeMb, setMaxFileSizeMb] = useState("100");
  const [intervalMinutes, setIntervalMinutes] = useState("5");
  const [statusMessage, setStatusMessage] = useState("");
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    if (!settings.data) {
      return;
    }

    const snapshot = JSON.stringify(settings.data);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;

    setEnabled(settings.data.enabled);
    setAutoIngest(settings.data.autoIngest);
    setPath(settings.data.path);
    setIncludeExtensions(settings.data.includeExtensions.join(", "));
    setExcludeExtensions(settings.data.excludeExtensions.join(", "));
    setExcludeDirs(settings.data.excludeDirs.join(", "));
    setExcludeGlobs(settings.data.excludeGlobs.join(", "));
    setMaxFileSizeMb(String(settings.data.maxFileSizeMb));
    setIntervalMinutes(String(settings.data.intervalMinutes));
  }, [settings.data]);

  async function handleSave() {
    await updateSettings.mutateAsync({
      projectId,
      enabled,
      autoIngest,
      path: path.trim(),
      includeExtensions: parseCsv(includeExtensions),
      excludeExtensions: parseCsv(excludeExtensions),
      excludeDirs: parseCsv(excludeDirs),
      excludeGlobs: parseCsv(excludeGlobs),
      maxFileSizeMb: Number(maxFileSizeMb) || 100,
      intervalMinutes: Number(intervalMinutes) || 5,
    });
    setStatusMessage("Saved source watch settings.");
  }

  async function handleScanNow() {
    const result = await scanSourceWatch.mutateAsync(projectId);
    setStatusMessage(
      `Scanned ${result.watchedCount} file(s), copied ${result.copiedCount}, queued ${result.queuedIngestCount} ingest task(s).`,
    );
  }

  if (settings.isLoading) {
    return <RouteStatePane description="Loading source watch settings." state="loading" title="Source Watch" />;
  }

  if (settings.error) {
    const normalized = normalizeAppError(settings.error);
    return <RouteStatePane description={normalized.message} state="failed" title="Source Watch unavailable" />;
  }

  return (
    <div className="grid gap-6">
      <PageHeader
        actions={
          <div className="flex flex-wrap gap-2">
            <Button
              disabled={scanSourceWatch.isPending}
              onClick={handleScanNow}
              variant="outline"
            >
              Scan Now
            </Button>
            <Button disabled={updateSettings.isPending} onClick={handleSave}>
              Save Source Watch
            </Button>
          </div>
        }
        description="Configure background syncing from a watched source directory."
        title="Source Watch"
      />

      <Card>
        <CardHeader>
          <CardTitle>Watcher Settings</CardTitle>
          <CardDescription>Mirror upstream source folders and optionally queue ingest work.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 sm:grid-cols-2">
          <label className="flex items-center gap-3 rounded-xl border border-border/70 px-4 py-3 text-sm font-medium">
            <input
              checked={enabled}
              onChange={(event) => setEnabled(event.target.checked)}
              type="checkbox"
            />
            Enable source watch
          </label>

          <label className="flex items-center gap-3 rounded-xl border border-border/70 px-4 py-3 text-sm font-medium">
            <input
              checked={autoIngest}
              onChange={(event) => setAutoIngest(event.target.checked)}
              type="checkbox"
            />
            Automatically enqueue ingest tasks
          </label>

          <label className="grid gap-2 text-sm font-medium sm:col-span-2">
            Watch Path
            <Input onChange={(event) => setPath(event.target.value)} value={path} />
          </label>

          <label className="grid gap-2 text-sm font-medium">
            Include Extensions
            <Input
              onChange={(event) => setIncludeExtensions(event.target.value)}
              value={includeExtensions}
            />
          </label>

          <label className="grid gap-2 text-sm font-medium">
            Exclude Extensions
            <Input
              onChange={(event) => setExcludeExtensions(event.target.value)}
              value={excludeExtensions}
            />
          </label>

          <label className="grid gap-2 text-sm font-medium">
            Exclude Dirs
            <Input onChange={(event) => setExcludeDirs(event.target.value)} value={excludeDirs} />
          </label>

          <label className="grid gap-2 text-sm font-medium">
            Exclude Globs
            <Input onChange={(event) => setExcludeGlobs(event.target.value)} value={excludeGlobs} />
          </label>

          <label className="grid gap-2 text-sm font-medium">
            Max File Size MB
            <Input onChange={(event) => setMaxFileSizeMb(event.target.value)} value={maxFileSizeMb} />
          </label>

          <label className="grid gap-2 text-sm font-medium">
            Interval Minutes
            <Input onChange={(event) => setIntervalMinutes(event.target.value)} value={intervalMinutes} />
          </label>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Watcher Status</CardTitle>
          <CardDescription>Most recent scan metadata returned by the backend.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-3 text-sm">
          <div className="grid gap-1">
            <p className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">Last Scan</p>
            <p>{settings.data?.lastScanAt ?? "No scans recorded yet."}</p>
          </div>
          {statusMessage ? (
            <p aria-live="polite">{statusMessage}</p>
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}

function parseCsv(input: string) {
  return input
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
}
