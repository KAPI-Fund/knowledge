import { ScanSearch } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

import { RouteStatePane } from "@/components/layout/route-state-pane";
import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { normalizeAppError } from "@/lib/app-error";
import { formatDateTime } from "@/lib/format";

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
  const [lastScanSummary, setLastScanSummary] = useState("");
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
    try {
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
      toast.success("Saved source watch settings.");
    } catch (error) {
      toast.error(normalizeAppError(error).message);
    }
  }

  async function handleScanNow() {
    try {
      const result = await scanSourceWatch.mutateAsync(projectId);
      const summary = `Scanned ${result.watchedCount} file(s), copied ${result.copiedCount}, queued ${result.queuedIngestCount} ingest task(s).`;
      setLastScanSummary(summary);
      toast.success(summary);
    } catch (error) {
      toast.error(normalizeAppError(error).message);
    }
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
              <ScanSearch />
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
          <div className="flex items-center justify-between gap-3 rounded-lg border px-4 py-3">
            <Label htmlFor="source-watch-enabled">Enable source watch</Label>
            <Switch
              checked={enabled}
              id="source-watch-enabled"
              onCheckedChange={setEnabled}
            />
          </div>

          <div className="flex items-center gap-3 rounded-lg border px-4 py-3">
            <Checkbox
              checked={autoIngest}
              id="source-watch-auto-ingest"
              onCheckedChange={(checked) => setAutoIngest(checked === true)}
            />
            <Label htmlFor="source-watch-auto-ingest">Automatically enqueue ingest tasks</Label>
          </div>

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
            <p>
              {settings.data?.lastScanAt
                ? formatDateTime(settings.data.lastScanAt)
                : "No scans recorded yet."}
            </p>
          </div>
          {lastScanSummary ? <p aria-live="polite">{lastScanSummary}</p> : null}
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
