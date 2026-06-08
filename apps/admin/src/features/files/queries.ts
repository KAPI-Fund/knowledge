import { useQuery } from "@tanstack/react-query";

import { getProjectFileContent, listProjectFiles } from "../shared/api";

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
