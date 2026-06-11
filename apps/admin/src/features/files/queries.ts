import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  deleteProjectWikiPages,
  getProjectFileContent,
  listProjectFiles,
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
