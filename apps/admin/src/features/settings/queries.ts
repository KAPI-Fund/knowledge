import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  activateProviderConnection,
  createProviderConnection,
  deleteProviderConnection,
  getSystemSettings,
  runWebSearch,
  updateProviderConnection,
  updateSystemSettings,
  type SystemSettings,
} from "../shared/api";

const SETTINGS_KEY = ["system-settings"] as const;

export function useSystemSettingsQuery() {
  return useQuery({
    queryKey: SETTINGS_KEY,
    queryFn: getSystemSettings,
  });
}

export function useUpdateSystemSettingsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: updateSystemSettings,
    onSuccess: async (data) => {
      queryClient.setQueryData<SystemSettings>(SETTINGS_KEY, data);
      await queryClient.invalidateQueries({ queryKey: SETTINGS_KEY });
    },
  });
}

export function useRunWebSearchMutation() {
  return useMutation({ mutationFn: runWebSearch });
}

// Each connection endpoint returns the full settings response, so write it into
// the cache directly (and invalidate for safety) — the sections re-read the
// active connection / connection list from the same query.
function useConnectionMutation<Vars>(fn: (vars: Vars) => Promise<SystemSettings>) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (vars: Vars) => fn(vars),
    onSuccess: async (data) => {
      queryClient.setQueryData<SystemSettings>(SETTINGS_KEY, data);
      await queryClient.invalidateQueries({ queryKey: SETTINGS_KEY });
    },
  });
}

export function useCreateConnectionMutation() {
  return useConnectionMutation(createProviderConnection);
}

export function useUpdateConnectionMutation() {
  return useConnectionMutation(
    (vars: {
      id: string;
      label: string;
      baseUrl: string;
      model: string;
      apiKey?: string;
      clearApiKey?: boolean;
      timeoutSeconds?: number;
    }) => {
      const { id, ...body } = vars;
      return updateProviderConnection(id, body);
    },
  );
}

export function useDeleteConnectionMutation() {
  return useConnectionMutation(deleteProviderConnection);
}

export function useActivateConnectionMutation() {
  return useConnectionMutation(activateProviderConnection);
}
