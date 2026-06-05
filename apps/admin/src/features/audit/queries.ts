import { useQuery } from "@tanstack/react-query";

import { listProjectAuditLogs } from "../shared/api";

export function useProjectAuditLogsQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-audit", projectId],
    queryFn: () => listProjectAuditLogs(projectId),
  });
}
