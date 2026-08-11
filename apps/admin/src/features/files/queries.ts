import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  deleteProjectWikiPages,
  getFileHistoryEntry,
  getProjectFileContent,
  listFileHistory,
  listProjectFiles,
  restoreFileHistoryEntry,
  saveProjectFileContent,
} from "../shared/api";

export function useProjectFilesQuery(
  projectId: string,
  input: { root: string; recursive: boolean; maxFiles: number },
) {
  return useQuery({
    queryKey: ["project-files", projectId, input.root, input.recursive, input.maxFiles],
    queryFn: () => listProjectFiles({ projectId, ...input }),
  });
}

export function useProjectFileContentQuery(projectId: string, path: string) {
  return useQuery({
    queryKey: ["project-file-content", projectId, path],
    queryFn: () => getProjectFileContent({ projectId, path }),
    enabled: Boolean(projectId) && Boolean(path),
  });
}

export function useSaveFileContentMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { path: string; content: string }) =>
      saveProjectFileContent({ projectId, ...input }),
    onSuccess: (_data, input) => {
      void queryClient.invalidateQueries({ queryKey: ["project-files", projectId] });
      void queryClient.invalidateQueries({
        queryKey: ["project-file-content", projectId, input.path],
      });
      void queryClient.invalidateQueries({
        queryKey: ["file-history", projectId, input.path],
      });
    },
  });
}

export function useFileHistoryQuery(projectId: string, path: string, enabled: boolean) {
  return useQuery({
    queryKey: ["file-history", projectId, path],
    queryFn: () => listFileHistory({ projectId, path }),
    enabled: enabled && Boolean(projectId) && Boolean(path),
  });
}

export function useFileHistoryEntryQuery(projectId: string, entryId: string) {
  return useQuery({
    queryKey: ["file-history-entry", projectId, entryId],
    queryFn: () => getFileHistoryEntry({ projectId, entryId }),
    enabled: Boolean(projectId) && Boolean(entryId),
  });
}

export function useRestoreFileHistoryMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { entryId: string; path: string }) =>
      restoreFileHistoryEntry({ projectId, entryId: input.entryId }),
    onSuccess: (_data, input) => {
      void queryClient.invalidateQueries({ queryKey: ["project-files", projectId] });
      void queryClient.invalidateQueries({
        queryKey: ["project-file-content", projectId, input.path],
      });
      void queryClient.invalidateQueries({ queryKey: ["file-history", projectId, input.path] });
    },
  });
}

export function useDeleteWikiPagesMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { paths: string[] }) => deleteProjectWikiPages({ projectId, ...input }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["project-files", projectId] });
      void queryClient.invalidateQueries({ queryKey: ["project-file-content", projectId] });
    },
  });
}
