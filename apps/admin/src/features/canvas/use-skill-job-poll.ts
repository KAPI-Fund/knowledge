import { useEffect, useRef } from "react";

import { fetchSkillJob, type SkillJobProgress } from "../shared/api";

export interface RunningSkillJob {
  nodeId: string;
  jobId: string;
}

interface SkillJobHandlers {
  onDone: (nodeId: string, result: { assetId: string; url: string; title?: string }) => void;
  onError: (nodeId: string, message: string) => void;
  onProgress?: (nodeId: string, progress: SkillJobProgress) => void;
}

// Poll running skill jobs until each reaches a terminal state, then hand the
// result to a callback. Kept free of react-query on purpose: CanvasPage renders
// without a QueryClientProvider in tests, so this hook must be self-contained.
export function useSkillJobPoll(jobs: RunningSkillJob[], handlers: SkillJobHandlers) {
  const handlersRef = useRef(handlers);
  handlersRef.current = handlers;

  const jobsRef = useRef(jobs);
  jobsRef.current = jobs;

  const resolvedRef = useRef<Set<string>>(new Set());

  const key = jobs
    .map((job) => `${job.nodeId}:${job.jobId}`)
    .sort()
    .join("|");

  useEffect(() => {
    const pending = () => jobsRef.current.filter((job) => !resolvedRef.current.has(job.jobId));
    if (pending().length === 0) {
      return;
    }

    let cancelled = false;

    const poll = async () => {
      for (const job of pending()) {
        try {
          const status = await fetchSkillJob(job.jobId);
          if (cancelled || resolvedRef.current.has(job.jobId)) {
            continue;
          }
          if (status.status === "done" && status.result) {
            resolvedRef.current.add(job.jobId);
            handlersRef.current.onDone(job.nodeId, status.result);
          } else if (status.status === "error") {
            resolvedRef.current.add(job.jobId);
            handlersRef.current.onError(job.nodeId, status.error?.message ?? "生成失败");
          } else if (status.status === "running" && status.progress) {
            handlersRef.current.onProgress?.(job.nodeId, status.progress);
          } else if (status.status === "queued") {
            handlersRef.current.onProgress?.(job.nodeId, {
              stage: "queued",
              message:
                typeof status.queuePosition === "number"
                  ? `排队中 · 第 ${status.queuePosition} 位`
                  : "排队中",
            });
          }
        } catch {
          // Transient failures are retried on the next tick.
        }
      }
    };

    void poll();
    const interval = setInterval(() => void poll(), 2000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
}
