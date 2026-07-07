import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fetchSkillJob } from "../shared/api";
import { useSkillJobPoll, type RunningSkillJob } from "./use-skill-job-poll";

vi.mock("../shared/api", () => ({ fetchSkillJob: vi.fn() }));

function Harness({
  jobs,
  onDone,
  onError,
}: {
  jobs: RunningSkillJob[];
  onDone: (nodeId: string, result: { assetId: string; url: string; title?: string }) => void;
  onError: (nodeId: string, message: string) => void;
}) {
  useSkillJobPoll(jobs, { onDone, onError });
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
