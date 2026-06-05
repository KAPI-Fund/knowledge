import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { getSystemSettings, updateSystemSettings } from "../shared/api";

export function useSystemSettingsQuery() {
  return useQuery({
    queryKey: ["system-settings"],
    queryFn: getSystemSettings,
  });
}

export function useUpdateSystemSettingsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: updateSystemSettings,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["system-settings"] });
    },
  });
}
