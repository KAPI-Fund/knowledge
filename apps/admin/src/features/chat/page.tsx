import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useParams } from "react-router-dom";

import {
  conversationKeys,
  useConversationMessagesQuery,
  useConversationsQuery,
  useCreateConversationMutation,
  useDeleteConversationMutation,
  useRenameConversationMutation,
} from "./queries";
import { streamChatMessage } from "./stream";

export function ChatPage() {
  const { projectId = "" } = useParams();
  const queryClient = useQueryClient();
  const conversations = useConversationsQuery(projectId);
  const [activeId, setActiveId] = useState<string | null>(null);
  const messages = useConversationMessagesQuery(projectId, activeId);
  const createMutation = useCreateConversationMutation(projectId);
  const renameMutation = useRenameConversationMutation(projectId);
  const deleteMutation = useDeleteConversationMutation(projectId);

  const [draft, setDraft] = useState("");
  const [streamingText, setStreamingText] = useState<string | null>(null);
  const [pendingUserText, setPendingUserText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const isStreaming = streamingText !== null;

  async function handleSend() {
    const content = draft.trim();
    if (!content || isStreaming) {
      return;
    }

    let conversationId = activeId;
    if (!conversationId) {
      const created = await createMutation.mutateAsync(content.slice(0, 60));
      conversationId = created.id;
      setActiveId(conversationId);
    }

    setDraft("");
    setError(null);
    setPendingUserText(content);
    setStreamingText("");

    await streamChatMessage(
      { projectId, conversationId, content },
      {
        onDelta: (text) => {
          setStreamingText((current) => (current ?? "") + text);
        },
        onDone: () => {
          setStreamingText(null);
          setPendingUserText(null);
          void queryClient.invalidateQueries({
            queryKey: conversationKeys.messages(projectId, conversationId),
          });
          void queryClient.invalidateQueries({ queryKey: conversationKeys.list(projectId) });
        },
        onError: (message) => {
          setStreamingText(null);
          setPendingUserText(null);
          setError(message);
          // The user turn may already be persisted server-side; refetch so it
          // does not vanish from the transcript.
          void queryClient.invalidateQueries({
            queryKey: conversationKeys.messages(projectId, conversationId),
          });
          void queryClient.invalidateQueries({ queryKey: conversationKeys.list(projectId) });
        },
      },
    );
  }

  function handleRename(conversationId: string, currentTitle: string) {
    const title = window.prompt("Conversation title", currentTitle);
    if (title?.trim()) {
      renameMutation.mutate({ conversationId, title: title.trim() });
    }
  }

  function handleDelete(conversationId: string) {
    deleteMutation.mutate(conversationId, {
      onSuccess: () => {
        if (activeId === conversationId) {
          setActiveId(null);
        }
      },
    });
  }

  return (
    <div className="grid gap-4 md:grid-cols-[16rem_1fr]">
      <aside className="rounded-lg border border-border p-3">
        <button
          className="mb-3 w-full rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-accent"
          onClick={() => {
            createMutation.mutate(undefined, {
              onSuccess: (created) => setActiveId(created.id),
            });
          }}
          type="button"
        >
          New conversation
        </button>
        <ul className="space-y-1">
          {(conversations.data ?? []).map((conversation) => (
            <li className="flex items-center gap-1" key={conversation.id}>
              <button
                className={`flex-1 truncate rounded px-2 py-1 text-left text-sm ${
                  conversation.id === activeId ? "bg-accent" : "hover:bg-accent/50"
                }`}
                onClick={() => setActiveId(conversation.id)}
                type="button"
              >
                {conversation.title}
              </button>
              <button
                aria-label={`Rename ${conversation.title}`}
                className="rounded px-1 text-xs text-muted-foreground hover:bg-accent"
                onClick={() => handleRename(conversation.id, conversation.title)}
                type="button"
              >
                Rename
              </button>
              <button
                aria-label={`Delete ${conversation.title}`}
                className="rounded px-1 text-xs text-muted-foreground hover:bg-accent"
                onClick={() => handleDelete(conversation.id)}
                type="button"
              >
                Delete
              </button>
            </li>
          ))}
        </ul>
      </aside>

      <section className="flex min-h-[24rem] flex-col rounded-lg border border-border">
        <div aria-live="polite" className="flex-1 space-y-3 overflow-y-auto p-4">
          {(messages.data ?? []).map((message) => (
            <div
              className={`max-w-prose whitespace-pre-wrap rounded-lg px-3 py-2 text-sm ${
                message.role === "user" ? "ml-auto bg-accent" : "bg-muted"
              }`}
              data-role={message.role}
              key={message.id}
            >
              {message.content}
            </div>
          ))}
          {pendingUserText !== null && (
            <div
              className="ml-auto max-w-prose whitespace-pre-wrap rounded-lg bg-accent px-3 py-2 text-sm"
              data-role="user"
            >
              {pendingUserText}
            </div>
          )}
          {streamingText !== null && (
            <div
              className="max-w-prose whitespace-pre-wrap rounded-lg bg-muted px-3 py-2 text-sm"
              data-role="assistant"
              data-streaming="true"
            >
              {streamingText}
            </div>
          )}
          {error && <p className="text-sm text-red-600">{error}</p>}
        </div>
        <form
          className="flex gap-2 border-t border-border p-3"
          onSubmit={(event) => {
            event.preventDefault();
            void handleSend();
          }}
        >
          <textarea
            aria-label="Chat message"
            className="min-h-10 flex-1 resize-y rounded-lg border border-border px-3 py-2 text-sm"
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Ask the wiki..."
            value={draft}
          />
          <button
            className="rounded-lg border border-border px-4 py-2 text-sm font-medium hover:bg-accent disabled:opacity-50"
            disabled={isStreaming || !draft.trim()}
            type="submit"
          >
            Send
          </button>
        </form>
      </section>
    </div>
  );
}
