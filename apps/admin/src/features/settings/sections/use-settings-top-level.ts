import type { SystemSettings } from "../../shared/api";

/// PATCH /api/system/settings still requires providerMode/language/
/// defaultQueryLimit at the top level even when a section only edits a
/// capability block. Derive them (from the new `defaults` block, falling back to
/// legacy flat fields) so every section sends a valid request.
export function useSettingsTopLevel(data: SystemSettings | undefined) {
  return {
    providerMode: data?.providerMode ?? "openai-compatible",
    language: data?.defaults?.language ?? data?.language ?? "en",
    defaultQueryLimit: data?.defaults?.defaultQueryLimit ?? data?.defaultQueryLimit ?? 5,
  };
}
