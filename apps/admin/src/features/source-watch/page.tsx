import { useEffect, useRef, useState } from "react";
import { useParams } from "react-router-dom";

import { ProjectNav } from "../projects/project-nav";

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

  return (
    <section className="stack">
      <h1>Source Watch</h1>
      <ProjectNav projectId={projectId} />

      <div className="card stack panel">
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(event) => setEnabled(event.target.checked)}
          />
          <span>Enable source watch</span>
        </label>

        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={autoIngest}
            onChange={(event) => setAutoIngest(event.target.checked)}
          />
          <span>Automatically enqueue ingest tasks</span>
        </label>

        <label>
          Watch Path
          <input value={path} onChange={(event) => setPath(event.target.value)} />
        </label>

        <label>
          Include Extensions
          <input
            value={includeExtensions}
            onChange={(event) => setIncludeExtensions(event.target.value)}
          />
        </label>

        <label>
          Exclude Extensions
          <input
            value={excludeExtensions}
            onChange={(event) => setExcludeExtensions(event.target.value)}
          />
        </label>

        <label>
          Exclude Dirs
          <input value={excludeDirs} onChange={(event) => setExcludeDirs(event.target.value)} />
        </label>

        <label>
          Exclude Globs
          <input value={excludeGlobs} onChange={(event) => setExcludeGlobs(event.target.value)} />
        </label>

        <label>
          Max File Size MB
          <input value={maxFileSizeMb} onChange={(event) => setMaxFileSizeMb(event.target.value)} />
        </label>

        <label>
          Interval Minutes
          <input value={intervalMinutes} onChange={(event) => setIntervalMinutes(event.target.value)} />
        </label>

        {settings.data?.lastScanAt ? <p>Last scan: {settings.data.lastScanAt}</p> : null}
        {statusMessage ? <p aria-live="polite">{statusMessage}</p> : null}

        <div className="action-row">
          <button type="button" onClick={handleSave} disabled={updateSettings.isPending}>
            Save Source Watch
          </button>
          <button type="button" onClick={handleScanNow} disabled={scanSourceWatch.isPending}>
            Scan Now
          </button>
        </div>
      </div>
    </section>
  );
}

function parseCsv(input: string) {
  return input
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
}
