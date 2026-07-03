import { useState } from "react";

import { MarkdownMessage } from "@/components/shared/markdown-message";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { uuid } from "@/lib/uuid";

import { streamCanvasChat } from "./stream";

interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
}

export interface SkillNodePayload {
  node: unknown;
  x: number;
  y: number;
}

interface ChatPanelProps {
  canvasId: string;
  selectedNodeIds: string[];
  onSkillNode: (payload: SkillNodePayload) => void;
  placementOrigin: { x: number; y: number };
  // Persist the current canvas before an SSE chat. Chat re-reads the canvas
  // from the DB, so an unsaved edit (new node, edge, note content) would be
  // invisible. Resolves false when the save fails, and the chat is aborted.
  onBeforeSend: () => Promise<boolean>;
}

export function ChatPanel({
  canvasId,
  selectedNodeIds,
  onSkillNode,
  placementOrigin,
  onBeforeSend,
}: ChatPanelProps) {
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);

  const appendAssistantDelta = (id: string, text: string) => {
    setMessages((prev) =>
      prev.map((message) =>
        message.id === id ? { ...message, content: message.content + text } : message,
      ),
    );
  };

  const submit = async () => {
    const text = input.trim();
    if (!text || streaming || !canvasId) {
      return;
    }

    const userMessage: ChatMessage = { id: uuid(), role: "user", content: text };
    const assistantId = uuid();
    setMessages((prev) => [...prev, userMessage, { id: assistantId, role: "assistant", content: "" }]);
    setInput("");
    setStreaming(true);
    const saved = await onBeforeSend();
    if (!saved) {
      setMessages((prev) =>
        prev.map((message) =>
          message.id === assistantId
            ? { ...message, content: "> Error: could not save canvas" }
            : message,
        ),
      );
      setStreaming(false);
      return;
    }
    try {
      await streamCanvasChat(
        canvasId,
        text,
        selectedNodeIds,
        {
          onDelta: (delta) => appendAssistantDelta(assistantId, delta),
          onNode: (payload) => {
            onSkillNode(payload);
            setMessages((prev) =>
              prev.map((message) =>
                message.id === assistantId
                  ? { ...message, content: "Added a node to the canvas." }
                  : message,
              ),
            );
          },
          onDone: () => setStreaming(false),
          onError: (message) => {
            appendAssistantDelta(assistantId, `\n\n> Error: ${message}`);
            setStreaming(false);
          },
        },
        placementOrigin,
      );
    } finally {
      setStreaming(false);
    }
  };

  return (
    <section className="flex h-full w-80 shrink-0 flex-col border-l bg-card">
      <div className="border-b p-3 text-sm font-semibold">Chat</div>
      <ScrollArea className="min-h-0 flex-1 space-y-3 p-3">
        {messages.length === 0 ? (
          <p className="text-xs text-muted-foreground">Ask a question about this canvas.</p>
        ) : (
          messages.map((message) => (
            <div
              key={message.id}
              className={cn(
                "rounded-md px-3 py-2 text-sm",
                message.role === "user" ? "bg-primary/10" : "bg-muted",
              )}
            >
              <MarkdownMessage content={message.content || "..."} />
            </div>
          ))
        )}
      </ScrollArea>

      <div className="border-t p-3">
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <Input
            value={input}
            onChange={(event) => setInput(event.target.value)}
            placeholder="Ask a question"
            disabled={streaming}
          />
        </form>
      </div>
    </section>
  );
}

