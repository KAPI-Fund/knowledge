import { getCsrfToken } from "../auth/csrf";
import type {
  AgentEvent,
  AgentMessageOptions,
  AgentReference,
  AgentUserInputRequest,
} from "./agent-types";

export interface ChatStreamEvent {
  event: string;
  data: string;
}

export function parseSseBuffer(buffer: string): { events: ChatStreamEvent[]; rest: string } {
  const events: ChatStreamEvent[] = [];
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

export interface ChatDonePayload {
  messageId: string | null;
  content: string;
  contextSummary?: string | null;
  agentMode?: string;
  references?: AgentReference[];
  userInputRequest?: AgentUserInputRequest;
}

export interface ChatStreamHandlers {
  onDelta: (text: string) => void;
  onDone: (payload: ChatDonePayload) => void;
  onError: (message: string) => void;
  onAgentEvent?: (event: AgentEvent) => void;
}

export async function streamChatMessage(
  input: {
    projectId: string;
    conversationId: string;
    content: string;
    agent?: AgentMessageOptions;
  },
  handlers: ChatStreamHandlers,
) {
  const response = await fetch(
    `/api/projects/${input.projectId}/conversations/${input.conversationId}/messages`,
    {
      method: "POST",
      credentials: "include",
      headers: {
        "content-type": "application/json",
        "x-csrf-token": getCsrfToken(),
      },
      body: JSON.stringify({ content: input.content, agent: input.agent }),
    },
  );

  if (!response.ok || !response.body) {
    let message = `chat request failed with status ${response.status}`;
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

function dispatchEvent(event: ChatStreamEvent, handlers: ChatStreamHandlers) {
  if (event.event === "delta") {
    const payload = JSON.parse(event.data) as { text: string };
    handlers.onDelta(payload.text);
    return;
  }
  if (event.event === "agentEvent") {
    const payload = JSON.parse(event.data) as AgentEvent;
    if (payload.type === "messageDelta") {
      handlers.onDelta(payload.text);
    }
    handlers.onAgentEvent?.(payload);
    return;
  }
  if (event.event === "done") {
    handlers.onDone(JSON.parse(event.data) as ChatDonePayload);
    return;
  }
  if (event.event === "error") {
    const payload = JSON.parse(event.data) as { message: string };
    handlers.onError(payload.message);
  }
}
