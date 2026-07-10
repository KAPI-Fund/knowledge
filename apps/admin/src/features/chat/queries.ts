import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  createConversation,
  deleteConversation,
  listAgentSkills,
  listConversationMessages,
  listConversations,
  renameConversation,
  saveMessageToWiki,
} from "../shared/api";

export const conversationKeys = {
  list: (projectId: string) => ["conversations", projectId] as const,
  messages: (projectId: string, conversationId: string) =>
    ["conversation-messages", projectId, conversationId] as const,
  agentSkills: (projectId: string) => ["agent-skills", projectId] as const,
};

export function useAgentSkillsQuery(projectId: string) {
  return useQuery({
    queryKey: conversationKeys.agentSkills(projectId),
    queryFn: () => listAgentSkills(projectId),
    enabled: Boolean(projectId),
    staleTime: 60_000,
  });
}

export function useConversationsQuery(projectId: string) {
  return useQuery({
    queryKey: conversationKeys.list(projectId),
    queryFn: () => listConversations(projectId),
    enabled: Boolean(projectId),
  });
}

export function useConversationMessagesQuery(projectId: string, conversationId: string | null) {
  return useQuery({
    queryKey: conversationKeys.messages(projectId, conversationId ?? "none"),
    queryFn: () => listConversationMessages({ projectId, conversationId: conversationId ?? "" }),
    enabled: Boolean(projectId) && Boolean(conversationId),
  });
}

export function useCreateConversationMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (title?: string) => createConversation({ projectId, title }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: conversationKeys.list(projectId) });
    },
  });
}

export function useRenameConversationMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { conversationId: string; title: string }) =>
      renameConversation({ projectId, ...input }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: conversationKeys.list(projectId) });
    },
  });
}

export function useSaveMessageToWikiMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { conversationId: string; messageId: string }) =>
      saveMessageToWiki({ projectId, ...input }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["project-files", projectId] });
    },
  });
}

export function useDeleteConversationMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (conversationId: string) => deleteConversation({ projectId, conversationId }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: conversationKeys.list(projectId) });
    },
  });
}
