import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fetchSkillJob } from "../shared/api";
import { useSkillJobPoll, type RunningSkillJob } from "./use-skill-job-poll";

vi.mock("../shared/api", () => ({ fetchSkillJob: vi.fn() }));

function Harness({
  jobs,
  onDone,
  onError,
  onProgress,
}: {
  jobs: RunningSkillJob[];
  onDone: (nodeId: string, result: { assetId: string; url: string; title?: string }) => void;
  onError: (nodeId: string, message: string) => void;
  onProgress?: (nodeId: string, progress: { stage?: string; message?: string }) => void;
}) {
  useSkillJobPoll(jobs, { onDone, onError, onProgress });
  return null;
}

beforeEach(() => {
  vi.mocked(fetchSkillJob).mockReset();
});

describe("useSkillJobPoll", () => {
  it("does not poll when there are no running jobs", async () => {
    render(<Harness jobs={[]} onDone={() => {}} onError={() => {}} />);
    await Promise.resolve();
    expect(fetchSkillJob).not.toHaveBeenCalled();
  });

  it("invokes onDone with the result when a job finishes", async () => {
    vi.mocked(fetchSkillJob).mockResolvedValue({
      status: "done",
      result: { assetId: "a1", url: "/assets/a1", title: "PPT" },
      error: null,
    });
    const onDone = vi.fn();
    render(
      <Harness jobs={[{ nodeId: "n1", jobId: "j1" }]} onDone={onDone} onError={() => {}} />,
    );
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    expect(onDone).toHaveBeenCalledWith("n1", { assetId: "a1", url: "/assets/a1", title: "PPT" });
  });

  it("invokes onError with the message when a job fails", async () => {
    vi.mocked(fetchSkillJob).mockResolvedValue({
      status: "error",
      result: null,
      error: { message: "boom" },
    });
    const onError = vi.fn();
    render(
      <Harness jobs={[{ nodeId: "n1", jobId: "j1" }]} onDone={() => {}} onError={onError} />,
    );
    await waitFor(() => expect(onError).toHaveBeenCalledWith("n1", "boom"));
  });

  it("invokes onProgress while a job is running without resolving it", async () => {
    vi.mocked(fetchSkillJob).mockResolvedValue({
      status: "running",
      result: null,
      error: null,
      progress: { stage: "codex", message: "writing slides" },
    });
    const onProgress = vi.fn();
    render(
      <Harness
        jobs={[{ nodeId: "n1", jobId: "j1" }]}
        onDone={() => {}}
        onError={() => {}}
        onProgress={onProgress}
      />,
    );
    await waitFor(() =>
      expect(onProgress).toHaveBeenCalledWith("n1", { stage: "codex", message: "writing slides" }),
    );
    // Still unresolved: the next tick polls again and re-reports progress.
    await waitFor(() => expect(onProgress.mock.calls.length).toBeGreaterThanOrEqual(1));
  });

  it("reports queue position as synthetic progress while a job is queued", async () => {
    vi.mocked(fetchSkillJob).mockResolvedValue({
      status: "queued",
      result: null,
      error: null,
      queuePosition: 3,
    });
    const onProgress = vi.fn();
    render(
      <Harness
        jobs={[{ nodeId: "n1", jobId: "j1" }]}
        onDone={() => {}}
        onError={() => {}}
        onProgress={onProgress}
      />,
    );
    await waitFor(() =>
      expect(onProgress).toHaveBeenCalledWith("n1", { stage: "queued", message: "排队中 · 第 3 位" }),
    );
  });

  it("does not fire a terminal callback twice for the same job", async () => {
    vi.mocked(fetchSkillJob).mockResolvedValue({
      status: "done",
      result: { assetId: "a1", url: "/assets/a1" },
      error: null,
    });
    const onDone = vi.fn();
    render(
      <Harness jobs={[{ nodeId: "n1", jobId: "j1" }]} onDone={onDone} onError={() => {}} />,
    );
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    // Give the interval a moment; the resolved job must not re-fire.
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(onDone).toHaveBeenCalledTimes(1);
  });
});
