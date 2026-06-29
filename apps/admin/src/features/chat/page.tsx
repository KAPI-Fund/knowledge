import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useParams } from "react-router-dom";

import { PageHeader } from "@/components/shared/page-header";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

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
    <div className="flex min-h-0 flex-1 flex-col gap-6">
      <PageHeader
        description="Chat with the project wiki using the configured provider; responses stream from retrieved context."
        title="Chat"
      />

      <div className="grid min-h-0 flex-1 grid-rows-[auto_1fr] gap-4 md:grid-cols-[16rem_1fr] md:grid-rows-1">
        <aside className="flex min-h-0 flex-col rounded-lg border border-border bg-card p-3">
          <Button
            className="mb-3 w-full"
            onClick={() => {
              createMutation.mutate(undefined, {
                onSuccess: (created) => setActiveId(created.id),
              });
            }}
            variant="outline"
          >
            New conversation
          </Button>
          <ul className="min-h-0 flex-1 space-y-1 overflow-y-auto">
            {(conversations.data ?? []).map((conversation) => (
              <li className="flex items-center gap-1" key={conversation.id}>
                <Button
                  className="flex-1 justify-start truncate"
                  onClick={() => setActiveId(conversation.id)}
                  size="sm"
                  variant={conversation.id === activeId ? "secondary" : "ghost"}
                >
                  {conversation.title}
                </Button>
                <Button
                  aria-label={`Rename ${conversation.title}`}
                  onClick={() => handleRename(conversation.id, conversation.title)}
                  size="xs"
                  variant="ghost"
                >
                  Rename
                </Button>
                <Button
                  aria-label={`Delete ${conversation.title}`}
                  onClick={() => handleDelete(conversation.id)}
                  size="xs"
                  variant="ghost"
                >
                  Delete
                </Button>
              </li>
            ))}
          </ul>
        </aside>

        <section className="flex min-h-0 flex-col rounded-lg border border-border bg-card">
          <div aria-live="polite" className="min-h-0 flex-1 space-y-3 overflow-y-auto p-4">
            {(messages.data ?? []).map((message) => (
              <div
                className={`max-w-prose whitespace-pre-wrap rounded-md border px-3 py-2 text-sm ${
                  message.role === "user"
                    ? "ml-auto border-primary/30 bg-accent"
                    : "border-border bg-muted"
                }`}
                data-role={message.role}
                key={message.id}
              >
                {message.content}
              </div>
            ))}
            {pendingUserText !== null && (
              <div
                className="ml-auto max-w-prose whitespace-pre-wrap rounded-md border border-primary/30 bg-accent px-3 py-2 text-sm"
                data-role="user"
              >
                {pendingUserText}
              </div>
            )}
            {streamingText !== null && (
              <div
                className="max-w-prose whitespace-pre-wrap rounded-md border border-border bg-muted px-3 py-2 text-sm"
                data-role="assistant"
                data-streaming="true"
              >
                {streamingText}
              </div>
            )}
            {error && <p className="text-sm text-destructive">{error}</p>}
          </div>
          <form
            className="flex gap-2 border-t border-border p-3"
            onSubmit={(event) => {
              event.preventDefault();
              void handleSend();
            }}
          >
            <Textarea
              aria-label="Chat message"
              className="min-h-10 flex-1 resize-y"
              onChange={(event) => setDraft(event.target.value)}
              placeholder="Ask the wiki..."
              value={draft}
            />
            <Button disabled={isStreaming || !draft.trim()} type="submit">
              Send
            </Button>
          </form>
        </section>
      </div>
    </div>
  );
}
