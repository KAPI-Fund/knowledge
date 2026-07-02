import { getCsrfToken } from "../auth/csrf";

export interface CanvasStreamEvent {
  event: string;
  data: string;
}

// Copied from features/chat/stream.ts parseSseBuffer.
export function parseSseBuffer(buffer: string): { events: CanvasStreamEvent[]; rest: string } {
  const events: CanvasStreamEvent[] = [];
  let rest = buffer;
  let separatorIndex = rest.indexOf("\n\n");
  while (separatorIndex !== -1) {
    const block = rest.slice(0, separatorIndex);
    rest = rest.slice(separatorIndex + 2);
    let event = "message";
    const dataLines: string[] = [];
    for (const line of block.split("\n")) {
      if (line.startsWith("event:")) {
        event = line.slice("event:".length).trim();
      } else if (line.startsWith("data:")) {
        dataLines.push(line.slice("data:".length).trimStart());
      }
    }
    if (dataLines.length > 0) {
      events.push({ event, data: dataLines.join("\n") });
    }
    separatorIndex = rest.indexOf("\n\n");
  }
  return { events, rest };
}

export interface NodeRunHandlers {
  onDelta: (text: string) => void;
  onDone: (payload: {
    versionId: string;
    createdAt: string;
    content?: string;
    url?: string;
  }) => void;
  onError: (message: string) => void;
}

export interface ChatHandlers {
  onDelta: (text: string) => void;
  onNode?: (payload: { node: unknown; x: number; y: number }) => void;
  onDone: (payload: unknown) => void;
  onError: (message: string) => void;
}

export async function runCanvasNode(
  canvasId: string,
  nodeId: string,
  handlers: NodeRunHandlers,
): Promise<void> {
  const response = await fetch(`/api/canvases/${canvasId}/nodes/${nodeId}/run`, {
    method: "POST",
    credentials: "include",
    headers: { "x-csrf-token": getCsrfToken() },
  });
  await consumeSse(response, handlers);
}

export async function streamCanvasChat(
  canvasId: string,
  message: string,
  selectedNodeIds: string[],
  handlers: ChatHandlers,
  origin?: { x: number; y: number },
): Promise<void> {
  const payload: { message: string; selectedNodeIds: string[]; x?: number; y?: number } = {
    message,
    selectedNodeIds,
  };
  if (origin) {
    payload.x = origin.x;
    payload.y = origin.y;
  }
  const response = await fetch(`/api/canvases/${canvasId}/chat`, {
    method: "POST",
    credentials: "include",
    headers: { "content-type": "application/json", "x-csrf-token": getCsrfToken() },
    body: JSON.stringify(payload),
  });
  await consumeSse(response, handlers);
}

async function consumeSse(
  response: Response,
  handlers: NodeRunHandlers | ChatHandlers,
): Promise<void> {
  if (!response.ok || !response.body) {
    let message = `canvas request failed with status ${response.status}`;
    try {
      const payload = (await response.json()) as { error?: unknown };
      if (typeof payload.error === "string" && payload.error.trim()) {
        message = payload.error;
      }
    } catch {
      // non-JSON body: keep the status fallback
    }
    handlers.onError(message);
    return;
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  for (;;) {
    const { value, done } = await reader.read();
    if (done) {
      break;
    }
    buffer += decoder.decode(value, { stream: true });
    const parsed = parseSseBuffer(buffer);
    buffer = parsed.rest;
    for (const event of parsed.events) {
      dispatchEvent(event, handlers);
    }
  }
}

function dispatchEvent(event: CanvasStreamEvent, handlers: NodeRunHandlers | ChatHandlers) {
  if (event.event === "delta") {
    const payload = JSON.parse(event.data) as { text: string };
    handlers.onDelta(payload.text);
    return;
  }
  if (event.event === "node") {
    if ("onNode" in handlers && handlers.onNode) {
      handlers.onNode(JSON.parse(event.data) as { node: unknown; x: number; y: number });
    }
    return;
  }
  if (event.event === "done") {
    handlers.onDone(JSON.parse(event.data));
    return;
  }
  if (event.event === "error") {
    const payload = JSON.parse(event.data) as { message?: string };
    handlers.onError(payload.message ?? "canvas stream error");
  }
}
