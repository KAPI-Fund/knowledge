# Multi-Provider Configuration — Frontend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the admin Settings page into a left sub-nav with five capability sections (LLM connections, Embedding, Image, Web search, Defaults), wire the new backend multi-provider API into the frontend, and make the canvas board's node model tag capability-aware.

**Architecture:** The backend plan (`2026-07-03-multi-provider-config-backend.md`) already defines the API this frontend consumes. `GET /api/system/settings` now returns `connections[]`, `embedding{}`, `image{}`, `search{}`, and `defaults{}` blocks (keys redacted to `apiKeyConfigured` booleans) alongside the retained legacy flat `provider*` fields. Four new connection endpoints (`POST/PATCH/DELETE /api/system/provider-connections[/:id]`, `POST …/:id/activate`) each return the full settings response. `PATCH /api/system/settings` still requires `providerMode`/`language`/`defaultQueryLimit` at the top level but now also accepts optional `image`/`embedding`/`search`/`defaults` blocks (blank-key-keeps, `clearApiKey` clears; search `providers` is deep-merged server-side). The Settings page decomposes into a nav shell plus one self-contained component per section, each hydrating local form state from the shared settings query and calling a focused mutation.

**Tech Stack:** React 19, TanStack Query v5, Zod, shadcn/ui (on `radix-ui` v1.6.0 + `lucide-react`), `@xyflow/react` v12, Vitest v4, Testing Library, `@/` → `apps/admin/src`.

**Reference (per project memory — port, don't invent):** `upstream_llm_wiki/src/components/settings/**` (per-capability category rail, LLM preset list with one active, independent embedding block, multi-provider web-search with Tavily default) and `upstream_llm_wiki/src/stores/wiki-store.ts` (`LlmConfig`/`ProviderConfigs`/`SearchProviderConfigs` shapes). All UI composes existing shadcn primitives in `apps/admin/src/components/ui/`.

---

## File Structure

**Create:**
- `apps/admin/src/components/ui/switch.tsx` — shadcn Switch (embedding enable toggle).
- `apps/admin/src/components/ui/switch.test.tsx` — render/toggle test.
- `apps/admin/src/features/settings/settings-nav.tsx` — left category rail.
- `apps/admin/src/features/settings/sections/llm-connections-section.tsx` — connection list + row edit + draft create.
- `apps/admin/src/features/settings/sections/embedding-section.tsx`
- `apps/admin/src/features/settings/sections/image-section.tsx`
- `apps/admin/src/features/settings/sections/web-search-section.tsx`
- `apps/admin/src/features/settings/sections/defaults-section.tsx`
- Section tests: `llm-connections-section.test.tsx`, `image-section.test.tsx`, `web-search-section.test.tsx` (co-located under `sections/`).
- `apps/admin/src/features/settings/api.test.tsx` — fetch-mock tests for the new API functions.

**Modify:**
- `apps/admin/src/features/shared/api.ts` — extend `settingsSchema`; add connection CRUD functions; extend `updateSystemSettings`.
- `apps/admin/src/features/settings/queries.ts` — add connection CRUD mutations.
- `apps/admin/src/features/settings/page.tsx` — replace with nav shell + section switch.
- `apps/admin/src/features/settings/page.test.tsx` — rewrite for the new shell.
- `apps/admin/src/features/canvas/canvas-board.tsx` — capability-aware `__model`.
- `apps/admin/src/features/dashboard/page.tsx` — read `defaults.language` / `defaults.defaultQueryLimit`.
- `apps/admin/src/features/dashboard/page.test.tsx` — nest `defaults` in the mock.
- Canvas test mocks: `board-render.test.tsx`, `canvas-board.test.tsx`, `add-node-integration.test.tsx`.

**Reuse:** `@/components/ui/{card,input,button,badge,select,switch,alert,separator}`, `@/lib/utils` (`cn`), `features/settings/queries.ts`, `@knowledge/api-client` (`apiFetch`).

---

## Task 1: Add the shadcn `Switch` primitive

The embedding section needs an enable toggle. `components/ui/` has no `switch.tsx`; `radix-ui` v1.6.0 exports `Switch`. Follow the repo's shadcn convention (`import { Switch as SwitchPrimitive } from "radix-ui"`, `data-slot` attributes, `cn`), matching `badge.tsx`/`dropdown-menu.tsx`.

**Files:**
- Create: `apps/admin/src/components/ui/switch.tsx`
- Test: `apps/admin/src/components/ui/switch.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
// apps/admin/src/components/ui/switch.test.tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import { Switch } from "./switch";

function Controlled() {
  const [on, setOn] = useState(false);
  return <Switch aria-label="toggle" checked={on} onCheckedChange={setOn} />;
}

describe("Switch", () => {
  it("renders a switch role", () => {
    render(<Switch aria-label="toggle" />);
    expect(screen.getByRole("switch", { name: "toggle" })).toBeInTheDocument();
  });

  it("toggles checked state on click", async () => {
    const user = userEvent.setup();
    render(<Controlled />);
    const el = screen.getByRole("switch", { name: "toggle" });
    expect(el).toHaveAttribute("data-state", "unchecked");
    await user.click(el);
    expect(el).toHaveAttribute("data-state", "checked");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/admin && npx vitest run src/components/ui/switch.test.tsx`
Expected: FAIL — cannot resolve `./switch`.

- [ ] **Step 3: Write the component**

```tsx
// apps/admin/src/components/ui/switch.tsx
import * as React from "react";
import { Switch as SwitchPrimitive } from "radix-ui";

import { cn } from "@/lib/utils";

function Switch({
  className,
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary data-[state=unchecked]:bg-input",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className={cn(
          "pointer-events-none block h-4 w-4 rounded-full bg-background shadow-sm ring-0 transition-transform data-[state=checked]:translate-x-4 data-[state=unchecked]:translate-x-0",
        )}
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd apps/admin && npx vitest run src/components/ui/switch.test.tsx`
Expected: PASS — both tests.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/components/ui/switch.tsx apps/admin/src/components/ui/switch.test.tsx
git commit -m "feat(admin): add shadcn Switch primitive"
```

---

## Task 2: Extend `settingsSchema` for the new GET response shape

The new `GET /api/system/settings` returns `connections[]`, `embedding{}`, `image{}`, `search{}`, and `defaults{}` blocks. Critically, it moves `language`/`defaultQueryLimit` **under `defaults`** and no longer returns them top-level — so the current *required* `language: z.string()` / `defaultQueryLimit: z.number()` fields would make Zod parsing **fail**. Make the legacy flat fields optional (retained for backward compat during migration) and add the new blocks. Export the inferred type so sections and the dashboard share it.

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts` (`settingsSchema`, lines ~181-198)

- [ ] **Step 1: Write the failing test** — create `apps/admin/src/features/settings/api.test.tsx`:

```tsx
import { afterEach, describe, expect, it, vi } from "vitest";

import { getSystemSettings } from "../shared/api";

afterEach(() => vi.restoreAllMocks());

const NEW_SHAPE = {
  providerMode: "openai-compatible",
  providerBaseUrl: "https://api.openai.com",
  providerApiKeyConfigured: true,
  providerModel: "gpt-4o",
  connections: [
    {
      id: "c1",
      label: "OpenAI",
      baseUrl: "https://api.openai.com",
      model: "gpt-4o",
      timeoutSeconds: 30,
      isActive: true,
      apiKeyConfigured: true,
    },
  ],
  embedding: {
    enabled: true,
    baseUrl: "https://api.openai.com",
    model: "text-embedding-3-small",
    timeoutSeconds: null,
    apiKeyConfigured: true,
  },
  image: {
    baseUrl: "https://api.openai.com",
    model: "gpt-image-1",
    size: "1024x1024",
    timeoutSeconds: null,
    apiKeyConfigured: false,
  },
  search: {
    provider: "tavily",
    providers: {
      tavily: { apiKeyConfigured: true, baseUrl: "https://api.tavily.com" },
      serpapi: { apiKeyConfigured: false, engine: "google", baseUrl: "https://serpapi.com" },
      searxng: { url: "", categories: ["general"] },
      ollama: { apiKeyConfigured: false, url: "https://ollama.com" },
    },
  },
  defaults: { language: "en", defaultQueryLimit: 8 },
};

describe("getSystemSettings", () => {
  it("parses the new blocked response shape", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify(NEW_SHAPE), { status: 200 }),
    );

    const settings = await getSystemSettings();

    expect(settings.connections?.[0]).toMatchObject({ id: "c1", isActive: true, apiKeyConfigured: true });
    expect(settings.embedding?.enabled).toBe(true);
    expect(settings.image?.size).toBe("1024x1024");
    expect(settings.search?.provider).toBe("tavily");
    expect(settings.search?.providers.tavily?.apiKeyConfigured).toBe(true);
    expect(settings.defaults?.defaultQueryLimit).toBe(8);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/settings/api.test.tsx`
Expected: FAIL — Zod throws because `connections`/`embedding`/`image`/`search`/`defaults` are unknown and `language`/`defaultQueryLimit` are missing (required).

- [ ] **Step 3: Replace `settingsSchema`** in `apps/admin/src/features/shared/api.ts` (the block currently at lines ~181-198):

```ts
const connectionSchema = z.object({
  id: z.string(),
  label: z.string(),
  baseUrl: z.string(),
  model: z.string(),
  timeoutSeconds: z.number().nullable().optional(),
  isActive: z.boolean(),
  apiKeyConfigured: z.boolean(),
});

const searchProviderConfigsSchema = z.object({
  tavily: z
    .object({ apiKeyConfigured: z.boolean().optional(), baseUrl: z.string().optional() })
    .partial()
    .optional(),
  serpapi: z
    .object({
      apiKeyConfigured: z.boolean().optional(),
      engine: z.string().optional(),
      baseUrl: z.string().optional(),
    })
    .partial()
    .optional(),
  searxng: z
    .object({ url: z.string().optional(), categories: z.array(z.string()).optional() })
    .partial()
    .optional(),
  ollama: z
    .object({ apiKeyConfigured: z.boolean().optional(), url: z.string().optional() })
    .partial()
    .optional(),
});

const settingsSchema = z.object({
  // Retained legacy flat fields (backward compat during migration). Now optional
  // because the redesigned GET nests language/defaultQueryLimit under `defaults`.
  providerMode: z.string(),
  language: z.string().optional(),
  defaultQueryLimit: z.number().optional(),
  providerBaseUrl: z.string().nullable().optional(),
  providerApiKeyConfigured: z.boolean().optional(),
  providerModel: z.string().nullable().optional(),
  providerEmbeddingModel: z.string().nullable().optional(),
  providerTimeoutSeconds: z.number().nullable().optional(),
  searchProvider: z.string().nullable().optional(),
  searchApiKeyConfigured: z.boolean().optional(),
  serpapiEngine: z.string().nullable().optional(),
  searxngUrl: z.string().nullable().optional(),
  searxngCategories: z.array(z.string()).nullable().optional(),
  ollamaSearchUrl: z.string().nullable().optional(),
  tavilyBaseUrl: z.string().nullable().optional(),
  serpapiBaseUrl: z.string().nullable().optional(),
  // New structured capability blocks.
  connections: z.array(connectionSchema).optional(),
  embedding: z
    .object({
      enabled: z.boolean(),
      baseUrl: z.string().nullable().optional(),
      model: z.string().nullable().optional(),
      timeoutSeconds: z.number().nullable().optional(),
      apiKeyConfigured: z.boolean(),
    })
    .optional(),
  image: z
    .object({
      baseUrl: z.string().nullable().optional(),
      model: z.string().nullable().optional(),
      size: z.string().nullable().optional(),
      timeoutSeconds: z.number().nullable().optional(),
      apiKeyConfigured: z.boolean(),
    })
    .optional(),
  search: z
    .object({
      provider: z.string().nullable().optional(),
      providers: searchProviderConfigsSchema,
    })
    .optional(),
  defaults: z
    .object({
      language: z.string(),
      defaultQueryLimit: z.number(),
    })
    .optional(),
});

export type SystemSettings = z.infer<typeof settingsSchema>;
export type ProviderConnection = z.infer<typeof connectionSchema>;
```

> Note: `SystemSettings`/`ProviderConnection` are exported so the sections, dashboard, and canvas board import a single source of truth rather than re-declaring shapes.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/settings/api.test.tsx`
Expected: PASS.

- [ ] **Step 5: Type-check**

Run: `cd apps/admin && npx tsc --noEmit`
Expected: May surface downstream type errors where code reads `settings.data.language` non-optionally (dashboard, settings page). Those are fixed in Tasks 6-8; if `tsc` fails only in `dashboard/page.tsx`, `settings/page.tsx`, or their tests, that is expected and resolved later. If it fails in `shared/api.ts` itself, fix before committing.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/shared/api.ts apps/admin/src/features/settings/api.test.tsx
git commit -m "feat(admin): extend settings schema with connections/embedding/image/search/defaults blocks"
```

---

## Task 3: Add connection CRUD API functions + extend `updateSystemSettings`

Four new API functions hit the connection endpoints (each returns the full settings response, so they reuse `settingsSchema`). `updateSystemSettings` gains optional `image`/`embedding`/`search`/`defaults` blocks, each honoring blank-key-keeps + `clear*ApiKey`. The blocks are only included in the payload when provided, so existing legacy-only callers are unaffected. All use `csrfHeader()` and the `{id}` path style from the backend routes.

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts` (add functions near `getSystemSettings` ~line 750; extend `updateSystemSettings` ~977-1035)
- Test: `apps/admin/src/features/settings/api.test.tsx` (append)

- [ ] **Step 1: Write the failing tests** — append to `apps/admin/src/features/settings/api.test.tsx`:

```tsx
import {
  activateProviderConnection,
  createProviderConnection,
  deleteProviderConnection,
  updateProviderConnection,
  updateSystemSettings,
} from "../shared/api";

function okResponse() {
  return new Response(JSON.stringify({ providerMode: "openai-compatible" }), { status: 200 });
}

describe("provider connection CRUD", () => {
  it("POSTs a new connection with camelCase body", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(okResponse());
    await createProviderConnection({
      label: "OpenAI",
      baseUrl: "https://api.openai.com",
      apiKey: "sk-123",
      model: "gpt-4o",
      timeoutSeconds: 30,
    });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/system/provider-connections");
    expect(init?.method).toBe("POST");
    expect(JSON.parse(init?.body as string)).toMatchObject({
      label: "OpenAI",
      baseUrl: "https://api.openai.com",
      apiKey: "sk-123",
      model: "gpt-4o",
      timeoutSeconds: 30,
    });
  });

  it("omits apiKey on update when blank and sends clearApiKey when flagged", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(okResponse());
    await updateProviderConnection("c1", {
      label: "L",
      baseUrl: "u",
      model: "m",
      apiKey: "",
      clearApiKey: true,
    });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/system/provider-connections/c1");
    expect(init?.method).toBe("PATCH");
    const body = JSON.parse(init?.body as string);
    expect(body).not.toHaveProperty("apiKey");
    expect(body.clearApiKey).toBe(true);
  });

  it("PATCH keeps a non-blank apiKey and drops clearApiKey", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(okResponse());
    await updateProviderConnection("c1", {
      label: "L",
      baseUrl: "u",
      model: "m",
      apiKey: "sk-new",
      clearApiKey: true,
    });
    const body = JSON.parse(fetchMock.mock.calls[0][1]?.body as string);
    expect(body.apiKey).toBe("sk-new");
    expect(body).not.toHaveProperty("clearApiKey");
  });

  it("DELETEs a connection by id", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(okResponse());
    await deleteProviderConnection("c1");
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/system/provider-connections/c1");
    expect(init?.method).toBe("DELETE");
  });

  it("POSTs to the activate endpoint", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(okResponse());
    await activateProviderConnection("c1");
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/system/provider-connections/c1/activate");
    expect(init?.method).toBe("POST");
  });
});

describe("updateSystemSettings capability blocks", () => {
  it("sends image/embedding/search/defaults blocks and omits blank keys", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(okResponse());
    await updateSystemSettings({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 8,
      image: { baseUrl: "https://img", model: "gpt-image-1", size: "512x512", apiKey: "" },
      embedding: { enabled: true, baseUrl: "https://emb", model: "e", apiKey: "sk-e" },
      search: {
        provider: "tavily",
        providers: { tavily: { apiKey: "", baseUrl: "https://api.tavily.com" } },
      },
      defaults: { language: "fr", defaultQueryLimit: 12 },
    });
    const body = JSON.parse(fetchMock.mock.calls[0][1]?.body as string);
    expect(body.image).toMatchObject({ baseUrl: "https://img", model: "gpt-image-1", size: "512x512" });
    expect(body.image).not.toHaveProperty("apiKey");
    expect(body.embedding).toMatchObject({ enabled: true, apiKey: "sk-e" });
    expect(body.search.providers.tavily.baseUrl).toBe("https://api.tavily.com");
    expect(body.search.providers.tavily).not.toHaveProperty("apiKey");
    expect(body.defaults).toMatchObject({ language: "fr", defaultQueryLimit: 12 });
  });

  it("sends image.clearApiKey when flagged and key blank", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(okResponse());
    await updateSystemSettings({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 8,
      image: { baseUrl: "https://img", model: "m", apiKey: "", clearApiKey: true },
    });
    const body = JSON.parse(fetchMock.mock.calls[0][1]?.body as string);
    expect(body.image.clearApiKey).toBe(true);
    expect(body.image).not.toHaveProperty("apiKey");
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/settings/api.test.tsx`
Expected: FAIL — the four connection functions don't exist; `updateSystemSettings` input has no block fields.

- [ ] **Step 3: Add the connection CRUD functions** in `apps/admin/src/features/shared/api.ts`, immediately after `getSystemSettings` (~line 750):

```ts
export async function createProviderConnection(input: {
  label: string;
  baseUrl: string;
  apiKey?: string;
  model: string;
  timeoutSeconds?: number;
}) {
  return apiFetch(
    "/api/system/provider-connections",
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        label: input.label,
        baseUrl: input.baseUrl,
        model: input.model,
        timeoutSeconds: input.timeoutSeconds,
        ...(input.apiKey?.trim() ? { apiKey: input.apiKey.trim() } : {}),
      }),
    },
    settingsSchema,
  );
}

export async function updateProviderConnection(
  id: string,
  input: {
    label: string;
    baseUrl: string;
    model: string;
    apiKey?: string;
    clearApiKey?: boolean;
    timeoutSeconds?: number;
  },
) {
  return apiFetch(
    `/api/system/provider-connections/${id}`,
    {
      method: "PATCH",
      headers: csrfHeader(),
      body: JSON.stringify({
        label: input.label,
        baseUrl: input.baseUrl,
        model: input.model,
        timeoutSeconds: input.timeoutSeconds,
        ...(input.apiKey?.trim() ? { apiKey: input.apiKey.trim() } : {}),
        ...(input.clearApiKey && !input.apiKey?.trim() ? { clearApiKey: true } : {}),
      }),
    },
    settingsSchema,
  );
}

export async function deleteProviderConnection(id: string) {
  return apiFetch(
    `/api/system/provider-connections/${id}`,
    { method: "DELETE", headers: csrfHeader() },
    settingsSchema,
  );
}

export async function activateProviderConnection(id: string) {
  return apiFetch(
    `/api/system/provider-connections/${id}/activate`,
    { method: "POST", headers: csrfHeader() },
    settingsSchema,
  );
}
```

- [ ] **Step 4: Extend `updateSystemSettings`** in `apps/admin/src/features/shared/api.ts`. Add the block types to its input type and build the block payloads. Replace the input type's closing and the `payload` construction (lines ~977-1024) with:

```ts
export async function updateSystemSettings(input: {
  providerMode: string;
  language: string;
  defaultQueryLimit: number;
  providerBaseUrl?: string;
  providerApiKey?: string;
  providerModel?: string;
  providerEmbeddingModel?: string;
  providerTimeoutSeconds?: number;
  searchProvider?: string;
  searchApiKey?: string;
  serpapiEngine?: string;
  searxngUrl?: string;
  searxngCategories?: string[];
  ollamaSearchUrl?: string;
  tavilyBaseUrl?: string;
  serpapiBaseUrl?: string;
  clearProviderApiKey?: boolean;
  clearSearchApiKey?: boolean;
  image?: {
    baseUrl?: string;
    model?: string;
    size?: string;
    timeoutSeconds?: number;
    apiKey?: string;
    clearApiKey?: boolean;
  };
  embedding?: {
    enabled?: boolean;
    baseUrl?: string;
    model?: string;
    timeoutSeconds?: number;
    apiKey?: string;
    clearApiKey?: boolean;
  };
  search?: {
    provider?: string;
    providers?: Record<string, Record<string, unknown>>;
  };
  defaults?: {
    language?: string;
    defaultQueryLimit?: number;
  };
}) {
  const payload = {
    providerMode: input.providerMode,
    language: input.language,
    defaultQueryLimit: input.defaultQueryLimit,
    providerBaseUrl: input.providerBaseUrl,
    providerModel: input.providerModel,
    providerEmbeddingModel: input.providerEmbeddingModel,
    providerTimeoutSeconds: input.providerTimeoutSeconds,
    searchProvider: input.searchProvider,
    serpapiEngine: input.serpapiEngine,
    searxngUrl: input.searxngUrl,
    searxngCategories: input.searxngCategories,
    ollamaSearchUrl: input.ollamaSearchUrl,
    tavilyBaseUrl: input.tavilyBaseUrl,
    serpapiBaseUrl: input.serpapiBaseUrl,
    ...(input.providerApiKey?.trim() ? { providerApiKey: input.providerApiKey.trim() } : {}),
    ...(input.searchApiKey?.trim() ? { searchApiKey: input.searchApiKey.trim() } : {}),
    ...(input.clearProviderApiKey && !input.providerApiKey?.trim()
      ? { clearProviderApiKey: true }
      : {}),
    ...(input.clearSearchApiKey && !input.searchApiKey?.trim() ? { clearSearchApiKey: true } : {}),
    ...(input.image
      ? {
          image: {
            baseUrl: input.image.baseUrl,
            model: input.image.model,
            size: input.image.size,
            timeoutSeconds: input.image.timeoutSeconds,
            ...(input.image.apiKey?.trim() ? { apiKey: input.image.apiKey.trim() } : {}),
            ...(input.image.clearApiKey && !input.image.apiKey?.trim()
              ? { clearApiKey: true }
              : {}),
          },
        }
      : {}),
    ...(input.embedding
      ? {
          embedding: {
            enabled: input.embedding.enabled,
            baseUrl: input.embedding.baseUrl,
            model: input.embedding.model,
            timeoutSeconds: input.embedding.timeoutSeconds,
            ...(input.embedding.apiKey?.trim() ? { apiKey: input.embedding.apiKey.trim() } : {}),
            ...(input.embedding.clearApiKey && !input.embedding.apiKey?.trim()
              ? { clearApiKey: true }
              : {}),
          },
        }
      : {}),
    ...(input.search ? { search: input.search } : {}),
    ...(input.defaults ? { defaults: input.defaults } : {}),
  };

  return apiFetch(
    "/api/system/settings",
    {
      method: "PATCH",
      headers: csrfHeader(),
      body: JSON.stringify(payload),
    },
    settingsSchema,
  );
}
```

> Note: the `search` block is passed through verbatim. The section (Task 5d) is responsible for setting an edited provider's `apiKey` to a trimmed value or `""` (blank = keep), which the backend deep-merge (`merge_search_provider_configs`) honors. No per-key stripping is needed here because the backend skips blank-string fields.

- [ ] **Step 5: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/settings/api.test.tsx`
Expected: PASS — all connection + block tests.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/shared/api.ts apps/admin/src/features/settings/api.test.tsx
git commit -m "feat(admin): connection CRUD API + capability blocks in updateSystemSettings"
```

---

## Task 4: Add connection CRUD mutations to `queries.ts`

The sections need mutations that write the cache from each endpoint's full-settings response and keep the `["system-settings"]` query fresh. Since each connection endpoint returns the complete settings object, `onSuccess` writes it directly into the cache (avoiding a redundant refetch) and also invalidates for safety.

**Files:**
- Modify: `apps/admin/src/features/settings/queries.ts`
- Test: `apps/admin/src/features/settings/queries.test.tsx` (new)

- [ ] **Step 1: Write the failing test** — create `apps/admin/src/features/settings/queries.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as api from "../shared/api";
import {
  useActivateConnectionMutation,
  useCreateConnectionMutation,
  useDeleteConnectionMutation,
  useUpdateConnectionMutation,
} from "./queries";

afterEach(() => vi.restoreAllMocks());

function wrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

describe("connection mutations", () => {
  it("create calls createProviderConnection", async () => {
    const spy = vi
      .spyOn(api, "createProviderConnection")
      .mockResolvedValue({ providerMode: "x" } as never);
    const { result } = renderHook(() => useCreateConnectionMutation(), { wrapper: wrapper() });
    await act(async () => {
      await result.current.mutateAsync({ label: "L", baseUrl: "u", model: "m" });
    });
    expect(spy).toHaveBeenCalledWith({ label: "L", baseUrl: "u", model: "m" });
  });

  it("update calls updateProviderConnection with id + body", async () => {
    const spy = vi
      .spyOn(api, "updateProviderConnection")
      .mockResolvedValue({ providerMode: "x" } as never);
    const { result } = renderHook(() => useUpdateConnectionMutation(), { wrapper: wrapper() });
    await act(async () => {
      await result.current.mutateAsync({ id: "c1", label: "L", baseUrl: "u", model: "m" });
    });
    expect(spy).toHaveBeenCalledWith("c1", { label: "L", baseUrl: "u", model: "m" });
  });

  it("delete calls deleteProviderConnection", async () => {
    const spy = vi
      .spyOn(api, "deleteProviderConnection")
      .mockResolvedValue({ providerMode: "x" } as never);
    const { result } = renderHook(() => useDeleteConnectionMutation(), { wrapper: wrapper() });
    await act(async () => {
      await result.current.mutateAsync("c1");
    });
    expect(spy).toHaveBeenCalledWith("c1");
  });

  it("activate calls activateProviderConnection", async () => {
    const spy = vi
      .spyOn(api, "activateProviderConnection")
      .mockResolvedValue({ providerMode: "x" } as never);
    const { result } = renderHook(() => useActivateConnectionMutation(), { wrapper: wrapper() });
    await act(async () => {
      await result.current.mutateAsync("c1");
    });
    await waitFor(() => expect(spy).toHaveBeenCalledWith("c1"));
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/settings/queries.test.tsx`
Expected: FAIL — the four mutation hooks don't exist.

- [ ] **Step 3: Rewrite `apps/admin/src/features/settings/queries.ts`**:

```ts
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
    mutationFn: fn,
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
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/settings/queries.test.tsx`
Expected: PASS — all four mutation tests.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/settings/queries.ts apps/admin/src/features/settings/queries.test.tsx
git commit -m "feat(admin): connection CRUD mutations with cache write-through"
```

---

## Task 5: Settings page shell + left category rail

Replace the three-Card `SettingsPage` with a shell: a left `SettingsNav` (category rail mirroring upstream) and a right pane that renders the active section. The active section is local state (default `"llm"`). Each section is a self-contained component (Tasks 6-10) that reads the settings query itself and owns its save. The shell only renders the header, nav, and the switch.

**Files:**
- Create: `apps/admin/src/features/settings/settings-nav.tsx`
- Modify: `apps/admin/src/features/settings/page.tsx`
- Rewrite: `apps/admin/src/features/settings/page.test.tsx`

- [ ] **Step 1: Write the failing nav shell test** — replace the contents of `apps/admin/src/features/settings/page.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SettingsPage } from "./page";

// Sections are exercised in their own tests; here we only assert the shell wires
// the nav to section switching. Stub each section with a marker.
vi.mock("./sections/llm-connections-section", () => ({
  LlmConnectionsSection: () => <div>LLM_SECTION</div>,
}));
vi.mock("./sections/embedding-section", () => ({
  EmbeddingSection: () => <div>EMBEDDING_SECTION</div>,
}));
vi.mock("./sections/image-section", () => ({
  ImageSection: () => <div>IMAGE_SECTION</div>,
}));
vi.mock("./sections/web-search-section", () => ({
  WebSearchSection: () => <div>WEB_SEARCH_SECTION</div>,
}));
vi.mock("./sections/defaults-section", () => ({
  DefaultsSection: () => <div>DEFAULTS_SECTION</div>,
}));

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <SettingsPage />
    </QueryClientProvider>,
  );
}

describe("SettingsPage shell", () => {
  it("shows the LLM section by default", () => {
    renderPage();
    expect(screen.getByText("LLM_SECTION")).toBeInTheDocument();
  });

  it("switches sections when a nav item is clicked", async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(screen.getByRole("button", { name: /image/i }));
    expect(screen.getByText("IMAGE_SECTION")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /web search/i }));
    expect(screen.getByText("WEB_SEARCH_SECTION")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/settings/page.test.tsx`
Expected: FAIL — sections don't exist yet / `SettingsPage` still renders the old Cards.

- [ ] **Step 3: Create `apps/admin/src/features/settings/settings-nav.tsx`**:

```tsx
import { cn } from "@/lib/utils";

export type SettingsSectionId = "llm" | "embedding" | "image" | "search" | "defaults";

const ITEMS: { id: SettingsSectionId; label: string }[] = [
  { id: "llm", label: "LLM" },
  { id: "embedding", label: "Embedding" },
  { id: "image", label: "Image" },
  { id: "search", label: "Web Search" },
  { id: "defaults", label: "Defaults" },
];

interface SettingsNavProps {
  active: SettingsSectionId;
  onSelect: (id: SettingsSectionId) => void;
}

export function SettingsNav({ active, onSelect }: SettingsNavProps) {
  return (
    <nav aria-label="Settings sections" className="flex flex-col gap-1">
      {ITEMS.map((item) => (
        <button
          key={item.id}
          type="button"
          aria-current={active === item.id ? "page" : undefined}
          onClick={() => onSelect(item.id)}
          className={cn(
            "rounded-md px-3 py-2 text-left text-sm font-medium transition-colors",
            active === item.id
              ? "bg-accent text-accent-foreground"
              : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
          )}
        >
          {item.label}
        </button>
      ))}
    </nav>
  );
}
```

- [ ] **Step 4: Rewrite `apps/admin/src/features/settings/page.tsx`**:

```tsx
import { useState } from "react";

import { PageHeader } from "@/components/shared/page-header";
import { Card, CardContent } from "@/components/ui/card";

import { DefaultsSection } from "./sections/defaults-section";
import { EmbeddingSection } from "./sections/embedding-section";
import { ImageSection } from "./sections/image-section";
import { LlmConnectionsSection } from "./sections/llm-connections-section";
import { WebSearchSection } from "./sections/web-search-section";
import { SettingsNav, type SettingsSectionId } from "./settings-nav";

export function SettingsPage() {
  const [section, setSection] = useState<SettingsSectionId>("llm");

  return (
    <div className="grid gap-6">
      <PageHeader
        description="Configure model providers, embeddings, image generation, and web search."
        title="Settings"
      />
      <div className="grid gap-6 md:grid-cols-[200px_1fr]">
        <Card className="h-fit">
          <CardContent className="p-2">
            <SettingsNav active={section} onSelect={setSection} />
          </CardContent>
        </Card>
        <div className="min-w-0">
          {section === "llm" ? <LlmConnectionsSection /> : null}
          {section === "embedding" ? <EmbeddingSection /> : null}
          {section === "image" ? <ImageSection /> : null}
          {section === "search" ? <WebSearchSection /> : null}
          {section === "defaults" ? <DefaultsSection /> : null}
        </div>
      </div>
    </div>
  );
}
```

> The `page.test.tsx` above stubs all five section modules, so this step compiles and passes once the section files exist as stubs. Create minimal stub files now so imports resolve; Tasks 6-10 replace each with the real implementation and its own test:
>
> ```tsx
> // apps/admin/src/features/settings/sections/llm-connections-section.tsx
> export function LlmConnectionsSection() { return null; }
> ```
> Repeat for `embedding-section.tsx` (`EmbeddingSection`), `image-section.tsx` (`ImageSection`), `web-search-section.tsx` (`WebSearchSection`), `defaults-section.tsx` (`DefaultsSection`).

- [ ] **Step 5: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/settings/page.test.tsx`
Expected: PASS — default LLM section shows; clicking Image/Web Search switches.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/settings/page.tsx apps/admin/src/features/settings/page.test.tsx apps/admin/src/features/settings/settings-nav.tsx apps/admin/src/features/settings/sections/
git commit -m "feat(admin): settings page shell with left category rail + section stubs"
```

---

## Task 6: LLM connections section

The LLM section is the multi-connection preset list (upstream model). It reads `settings.data.connections`, renders a row per connection (active radio, label + model summary, "Active"/"Configured" badge, Edit/Delete/Activate), an expand-to-edit form, and a `[+ Add]` draft-row that creates a new connection. Uses the Task 4 mutations. Compose shadcn `Card`, `Input`, `Button`, `Badge`.

**Files:**
- Create/replace: `apps/admin/src/features/settings/sections/llm-connections-section.tsx`
- Test: `apps/admin/src/features/settings/sections/llm-connections-section.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { LlmConnectionsSection } from "./llm-connections-section";

const settingsData = vi.fn();
const createConnection = vi.fn();
const updateConnection = vi.fn();
const deleteConnection = vi.fn();
const activateConnection = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useCreateConnectionMutation: () => ({ mutateAsync: createConnection, isPending: false }),
  useUpdateConnectionMutation: () => ({ mutateAsync: updateConnection, isPending: false }),
  useDeleteConnectionMutation: () => ({ mutateAsync: deleteConnection, isPending: false }),
  useActivateConnectionMutation: () => ({ mutateAsync: activateConnection, isPending: false }),
}));

function twoConnections() {
  return {
    connections: [
      { id: "c1", label: "OpenAI", baseUrl: "https://api.openai.com", model: "gpt-4o", timeoutSeconds: 30, isActive: true, apiKeyConfigured: true },
      { id: "c2", label: "Local vLLM", baseUrl: "http://localhost:8000", model: "qwen2", timeoutSeconds: null, isActive: false, apiKeyConfigured: false },
    ],
  };
}

describe("LlmConnectionsSection", () => {
  it("lists each connection with its label, model, and active badge", () => {
    settingsData.mockReturnValue(twoConnections());
    render(<LlmConnectionsSection />);
    expect(screen.getByText("OpenAI")).toBeInTheDocument();
    expect(screen.getByText("Local vLLM")).toBeInTheDocument();
    expect(screen.getByText(/gpt-4o/)).toBeInTheDocument();
    expect(screen.getByText("Active")).toBeInTheDocument();
  });

  it("activates an inactive connection", async () => {
    const user = userEvent.setup();
    activateConnection.mockResolvedValue({});
    settingsData.mockReturnValue(twoConnections());
    render(<LlmConnectionsSection />);
    const row = screen.getByText("Local vLLM").closest("li") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: /activate/i }));
    expect(activateConnection).toHaveBeenCalledWith("c2");
  });

  it("expands a row and updates the connection with a kept key", async () => {
    const user = userEvent.setup();
    updateConnection.mockResolvedValue({});
    settingsData.mockReturnValue(twoConnections());
    render(<LlmConnectionsSection />);
    const row = screen.getByText("OpenAI").closest("li") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: /edit/i }));
    await user.clear(within(row).getByLabelText(/model/i));
    await user.type(within(row).getByLabelText(/model/i), "gpt-4o-mini");
    await user.click(within(row).getByRole("button", { name: /^save$/i }));
    expect(updateConnection).toHaveBeenCalledWith(
      expect.objectContaining({ id: "c1", model: "gpt-4o-mini" }),
    );
    // Blank key field → no apiKey sent.
    expect(updateConnection.mock.calls[0][0]).not.toHaveProperty("apiKey");
  });

  it("adds a draft connection and creates it", async () => {
    const user = userEvent.setup();
    createConnection.mockResolvedValue({});
    settingsData.mockReturnValue({ connections: [] });
    render(<LlmConnectionsSection />);
    await user.click(screen.getByRole("button", { name: /add/i }));
    await user.type(screen.getByLabelText(/label/i), "New");
    await user.type(screen.getByLabelText(/base url/i), "https://x");
    await user.type(screen.getByLabelText(/model/i), "m");
    await user.click(screen.getByRole("button", { name: /^create$/i }));
    expect(createConnection).toHaveBeenCalledWith(
      expect.objectContaining({ label: "New", baseUrl: "https://x", model: "m" }),
    );
  });

  it("deletes a connection after confirming", async () => {
    const user = userEvent.setup();
    deleteConnection.mockResolvedValue({});
    settingsData.mockReturnValue(twoConnections());
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<LlmConnectionsSection />);
    const row = screen.getByText("Local vLLM").closest("li") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: /delete/i }));
    expect(deleteConnection).toHaveBeenCalledWith("c2");
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/llm-connections-section.test.tsx`
Expected: FAIL — section is a `null` stub.

- [ ] **Step 3: Implement `llm-connections-section.tsx`**

```tsx
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import type { ProviderConnection } from "../../shared/api";
import {
  useActivateConnectionMutation,
  useCreateConnectionMutation,
  useDeleteConnectionMutation,
  useSystemSettingsQuery,
  useUpdateConnectionMutation,
} from "../queries";

interface DraftFields {
  label: string;
  baseUrl: string;
  apiKey: string;
  model: string;
  timeoutSeconds: string;
}

const EMPTY_DRAFT: DraftFields = { label: "", baseUrl: "", apiKey: "", model: "", timeoutSeconds: "60" };

function ConnectionForm({
  fields,
  onChange,
  showKeyConfigured,
}: {
  fields: DraftFields;
  onChange: (next: DraftFields) => void;
  showKeyConfigured?: boolean;
}) {
  return (
    <div className="grid gap-3">
      <label className="grid gap-1.5 text-sm font-medium">
        Label
        <Input value={fields.label} onChange={(e) => onChange({ ...fields, label: e.target.value })} />
      </label>
      <label className="grid gap-1.5 text-sm font-medium">
        Base URL
        <Input value={fields.baseUrl} onChange={(e) => onChange({ ...fields, baseUrl: e.target.value })} />
      </label>
      <label className="grid gap-1.5 text-sm font-medium">
        API Key
        <Input
          type="password"
          placeholder={showKeyConfigured ? "Leave blank to keep the current key" : "sk-..."}
          value={fields.apiKey}
          onChange={(e) => onChange({ ...fields, apiKey: e.target.value })}
        />
      </label>
      <label className="grid gap-1.5 text-sm font-medium">
        Model
        <Input value={fields.model} onChange={(e) => onChange({ ...fields, model: e.target.value })} />
      </label>
      <label className="grid gap-1.5 text-sm font-medium">
        Timeout Seconds
        <Input
          value={fields.timeoutSeconds}
          onChange={(e) => onChange({ ...fields, timeoutSeconds: e.target.value })}
        />
      </label>
    </div>
  );
}

function ConnectionRow({ connection }: { connection: ProviderConnection }) {
  const activate = useActivateConnectionMutation();
  const update = useUpdateConnectionMutation();
  const remove = useDeleteConnectionMutation();
  const [editing, setEditing] = useState(false);
  const [fields, setFields] = useState<DraftFields>({
    label: connection.label,
    baseUrl: connection.baseUrl,
    apiKey: "",
    model: connection.model,
    timeoutSeconds: String(connection.timeoutSeconds ?? 60),
  });

  async function save() {
    await update.mutateAsync({
      id: connection.id,
      label: fields.label,
      baseUrl: fields.baseUrl,
      model: fields.model,
      timeoutSeconds: Number(fields.timeoutSeconds),
      ...(fields.apiKey.trim() ? { apiKey: fields.apiKey.trim() } : {}),
    });
    setFields((f) => ({ ...f, apiKey: "" }));
    setEditing(false);
  }

  async function del() {
    if (!window.confirm(`Delete connection "${connection.label}"?`)) {
      return;
    }
    await remove.mutateAsync(connection.id);
  }

  return (
    <li className="rounded-lg border p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-3">
          <input
            type="radio"
            aria-label={`Activate ${connection.label}`}
            checked={connection.isActive}
            onChange={() => {
              if (!connection.isActive) {
                void activate.mutateAsync(connection.id);
              }
            }}
          />
          <div className="min-w-0">
            <p className="truncate font-medium">{connection.label}</p>
            <p className="truncate font-mono text-xs text-muted-foreground">{connection.model}</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Badge variant={connection.isActive ? "default" : "secondary"}>
            {connection.isActive ? "Active" : "Configured"}
          </Badge>
          {!connection.isActive ? (
            <Button size="sm" variant="outline" onClick={() => activate.mutateAsync(connection.id)}>
              Activate
            </Button>
          ) : null}
          <Button size="sm" variant="ghost" onClick={() => setEditing((v) => !v)}>
            Edit
          </Button>
          <Button size="sm" variant="ghost" className="text-destructive" onClick={del}>
            Delete
          </Button>
        </div>
      </div>
      {editing ? (
        <div className="mt-3 border-t pt-3">
          <ConnectionForm fields={fields} onChange={setFields} showKeyConfigured={connection.apiKeyConfigured} />
          <div className="mt-3 flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={() => setEditing(false)}>
              Cancel
            </Button>
            <Button size="sm" onClick={save} disabled={update.isPending}>
              Save
            </Button>
          </div>
        </div>
      ) : null}
    </li>
  );
}

export function LlmConnectionsSection() {
  const settings = useSystemSettingsQuery();
  const create = useCreateConnectionMutation();
  const connections = settings.data?.connections ?? [];
  const [draft, setDraft] = useState<DraftFields | null>(null);

  async function createDraft() {
    if (!draft) {
      return;
    }
    await create.mutateAsync({
      label: draft.label,
      baseUrl: draft.baseUrl,
      model: draft.model,
      timeoutSeconds: Number(draft.timeoutSeconds),
      ...(draft.apiKey.trim() ? { apiKey: draft.apiKey.trim() } : {}),
    });
    setDraft(null);
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <div>
          <CardTitle>LLM Connections</CardTitle>
          <CardDescription>Chat and analyze use the active connection.</CardDescription>
        </div>
        <Button size="sm" onClick={() => setDraft(EMPTY_DRAFT)} disabled={draft !== null}>
          + Add
        </Button>
      </CardHeader>
      <CardContent className="grid gap-3">
        {connections.length === 0 && !draft ? (
          <p className="text-sm text-muted-foreground">No connections configured yet.</p>
        ) : null}
        <ul className="grid gap-2">
          {connections.map((c) => (
            <ConnectionRow key={c.id} connection={c} />
          ))}
        </ul>
        {draft ? (
          <div className="rounded-lg border border-dashed p-3">
            <ConnectionForm fields={draft} onChange={setDraft} />
            <div className="mt-3 flex justify-end gap-2">
              <Button size="sm" variant="ghost" onClick={() => setDraft(null)}>
                Cancel
              </Button>
              <Button size="sm" onClick={createDraft} disabled={create.isPending}>
                Create
              </Button>
            </div>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/llm-connections-section.test.tsx`
Expected: PASS — list, activate, edit-with-kept-key, add-draft-create, delete-confirm.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/settings/sections/llm-connections-section.tsx apps/admin/src/features/settings/sections/llm-connections-section.test.tsx
git commit -m "feat(admin): LLM connections section (list/activate/edit/add/delete)"
```

---

## Task 7: Embedding section

Independent config block: enable `Switch`, base URL, API key (blank-keeps), model, timeout. Saves via `useUpdateSystemSettingsMutation` with an `embedding` block. Hydrates from `settings.data.embedding`.

**Files:**
- Create/replace: `apps/admin/src/features/settings/sections/embedding-section.tsx`
- Test: `apps/admin/src/features/settings/sections/embedding-section.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { EmbeddingSection } from "./embedding-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
}));

describe("EmbeddingSection", () => {
  it("hydrates fields from settings and saves an embedding block with kept key", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      defaults: { language: "en", defaultQueryLimit: 8 },
      embedding: {
        enabled: true,
        baseUrl: "https://emb.example.com",
        model: "text-embedding-3-small",
        timeoutSeconds: 60,
        apiKeyConfigured: true,
      },
    });

    render(<EmbeddingSection />);

    expect(screen.getByLabelText(/base url/i)).toHaveValue("https://emb.example.com");
    await user.clear(screen.getByLabelText(/model/i));
    await user.type(screen.getByLabelText(/model/i), "text-embedding-3-large");
    await user.click(screen.getByRole("button", { name: /save/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.embedding).toMatchObject({
      enabled: true,
      baseUrl: "https://emb.example.com",
      model: "text-embedding-3-large",
    });
    expect(payload.embedding).not.toHaveProperty("apiKey");
    // Top-level required fields still present.
    expect(payload).toMatchObject({ providerMode: "openai-compatible", language: "en", defaultQueryLimit: 8 });
  });

  it("toggles enabled off and includes it in the block", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      defaults: { language: "en", defaultQueryLimit: 8 },
      embedding: { enabled: true, baseUrl: "", model: "", timeoutSeconds: null, apiKeyConfigured: false },
    });

    render(<EmbeddingSection />);
    await user.click(screen.getByRole("switch", { name: /enable embedding/i }));
    await user.click(screen.getByRole("button", { name: /save/i }));

    expect(updateSettings.mock.calls[0][0].embedding.enabled).toBe(false);
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/embedding-section.test.tsx`
Expected: FAIL — stub renders `null`.

- [ ] **Step 3: Implement `embedding-section.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "../queries";
import { useSettingsTopLevel } from "./use-settings-top-level";

export function EmbeddingSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();
  const topLevel = useSettingsTopLevel(settings.data);

  const [enabled, setEnabled] = useState(false);
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [timeoutSeconds, setTimeoutSeconds] = useState("60");
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    const emb = settings.data?.embedding;
    if (!emb) {
      return;
    }
    const snapshot = JSON.stringify(emb);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    setEnabled(emb.enabled);
    setBaseUrl(emb.baseUrl ?? "");
    setModel(emb.model ?? "");
    setTimeoutSeconds(String(emb.timeoutSeconds ?? 60));
  }, [settings.data?.embedding]);

  async function save() {
    await update.mutateAsync({
      ...topLevel,
      embedding: {
        enabled,
        baseUrl,
        model,
        timeoutSeconds: Number(timeoutSeconds),
        ...(apiKey.trim() ? { apiKey: apiKey.trim() } : {}),
      },
    });
    setApiKey("");
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Embedding</CardTitle>
        <CardDescription>Independent endpoint for retrieval / RAG embeddings.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <label className="flex items-center justify-between text-sm font-medium">
          <span>Enable embedding</span>
          <Switch aria-label="Enable embedding" checked={enabled} onCheckedChange={setEnabled} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Base URL
          <Input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          API Key
          <Input
            type="password"
            placeholder={
              settings.data?.embedding?.apiKeyConfigured
                ? "Leave blank to keep the current key"
                : "sk-..."
            }
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
          />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Model
          <Input value={model} onChange={(e) => setModel(e.target.value)} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Timeout Seconds
          <Input value={timeoutSeconds} onChange={(e) => setTimeoutSeconds(e.target.value)} />
        </label>
        <div className="flex justify-end">
          <Button onClick={save} disabled={update.isPending}>
            Save
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
```

> This section (and Image/Web-search/Defaults) needs the three top-level fields (`providerMode`/`language`/`defaultQueryLimit`) that PATCH still requires. Factor that into a tiny shared helper so each section doesn't re-derive it — created in the next step.

- [ ] **Step 4: Create the shared top-level helper** `apps/admin/src/features/settings/sections/use-settings-top-level.ts`:

```ts
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
```

- [ ] **Step 5: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/embedding-section.test.tsx`
Expected: PASS — hydrate + save-with-kept-key + toggle-off.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/settings/sections/embedding-section.tsx apps/admin/src/features/settings/sections/embedding-section.test.tsx apps/admin/src/features/settings/sections/use-settings-top-level.ts
git commit -m "feat(admin): embedding settings section with enable toggle"
```

---

## Task 8: Image section

Net-new OpenAI-compatible generation block: base URL, API key (blank-keeps), model, `size` (shadcn `Select` with common sizes), timeout. Fixes the broken image node once configured. Saves an `image` block. Hydrates from `settings.data.image`.

**Files:**
- Create/replace: `apps/admin/src/features/settings/sections/image-section.tsx`
- Test: `apps/admin/src/features/settings/sections/image-section.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ImageSection } from "./image-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
}));

describe("ImageSection", () => {
  it("hydrates from settings and saves an image block including size", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      defaults: { language: "en", defaultQueryLimit: 8 },
      image: {
        baseUrl: "https://img.example.com",
        model: "gpt-image-1",
        size: "1024x1024",
        timeoutSeconds: 60,
        apiKeyConfigured: false,
      },
    });

    render(<ImageSection />);

    expect(screen.getByLabelText(/base url/i)).toHaveValue("https://img.example.com");
    expect(screen.getByLabelText(/model/i)).toHaveValue("gpt-image-1");
    expect(screen.getByLabelText(/image size/i)).toHaveValue("1024x1024");

    await user.selectOptions(screen.getByLabelText(/image size/i), "512x512");
    await user.click(screen.getByRole("button", { name: /save/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.image).toMatchObject({
      baseUrl: "https://img.example.com",
      model: "gpt-image-1",
      size: "512x512",
    });
    expect(payload.image).not.toHaveProperty("apiKey");
    expect(payload).toMatchObject({ providerMode: "openai-compatible", language: "en", defaultQueryLimit: 8 });
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/image-section.test.tsx`
Expected: FAIL — stub renders `null`.

- [ ] **Step 3: Implement `image-section.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "../queries";
import { useSettingsTopLevel } from "./use-settings-top-level";

const SIZES = ["256x256", "512x512", "1024x1024", "1024x1792", "1792x1024"];

export function ImageSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();
  const topLevel = useSettingsTopLevel(settings.data);

  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [size, setSize] = useState("1024x1024");
  const [timeoutSeconds, setTimeoutSeconds] = useState("60");
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    const img = settings.data?.image;
    if (!img) {
      return;
    }
    const snapshot = JSON.stringify(img);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    setBaseUrl(img.baseUrl ?? "");
    setModel(img.model ?? "");
    setSize(img.size ?? "1024x1024");
    setTimeoutSeconds(String(img.timeoutSeconds ?? 60));
  }, [settings.data?.image]);

  async function save() {
    await update.mutateAsync({
      ...topLevel,
      image: {
        baseUrl,
        model,
        size,
        timeoutSeconds: Number(timeoutSeconds),
        ...(apiKey.trim() ? { apiKey: apiKey.trim() } : {}),
      },
    });
    setApiKey("");
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Image Generation</CardTitle>
        <CardDescription>OpenAI-compatible image endpoint for the canvas image node.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <label className="grid gap-1.5 text-sm font-medium">
          Base URL
          <Input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          API Key
          <Input
            type="password"
            placeholder={
              settings.data?.image?.apiKeyConfigured
                ? "Leave blank to keep the current key"
                : "sk-..."
            }
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
          />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Model
          <Input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder="gpt-image-1, dall-e-3"
          />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Image Size
          <Select aria-label="Image Size" value={size} onChange={(e) => setSize(e.target.value)}>
            {SIZES.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </Select>
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Timeout Seconds
          <Input value={timeoutSeconds} onChange={(e) => setTimeoutSeconds(e.target.value)} />
        </label>
        <div className="flex justify-end">
          <Button onClick={save} disabled={update.isPending}>
            Save
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/image-section.test.tsx`
Expected: PASS — hydrate + size-change + save.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/settings/sections/image-section.tsx apps/admin/src/features/settings/sections/image-section.test.tsx
git commit -m "feat(admin): image generation settings section with size select"
```

---

## Task 9: Web search section

Four-provider selector (Tavily default), per-provider fields persisted via the `search.providers` map so switching retains each provider's key, plus the retained "Test Search" button. On save it sends a `search` block: `{ provider, providers: {...} }`. Because GET redacts keys to `apiKeyConfigured`, the section never holds a stored key — it sends the edited apiKey field, or `""` (blank = keep) which the backend deep-merge honors. Non-secret fields (baseUrl, engine, url, categories) are sent as-is and overlaid server-side.

**Files:**
- Create/replace: `apps/admin/src/features/settings/sections/web-search-section.tsx`
- Test: `apps/admin/src/features/settings/sections/web-search-section.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { WebSearchSection } from "./web-search-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();
const runWebSearch = vi.fn();
const resetWebSearch = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
  useRunWebSearchMutation: () => ({
    mutateAsync: runWebSearch,
    reset: resetWebSearch,
    isPending: false,
    data: undefined,
  }),
}));

function tavilyActive() {
  return {
    providerMode: "openai-compatible",
    defaults: { language: "en", defaultQueryLimit: 8 },
    search: {
      provider: "tavily",
      providers: {
        tavily: { apiKeyConfigured: true, baseUrl: "https://api.tavily.com" },
        serpapi: { apiKeyConfigured: false, engine: "google", baseUrl: "https://serpapi.com" },
        searxng: { url: "", categories: ["general"] },
        ollama: { apiKeyConfigured: false, url: "https://ollama.com" },
      },
    },
  };
}

describe("WebSearchSection", () => {
  it("defaults to the active tavily provider and renders its fields", () => {
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);
    expect(screen.getByLabelText(/Search Provider/i)).toHaveValue("tavily");
    expect(screen.getByLabelText(/Tavily Base URL/i)).toHaveValue("https://api.tavily.com");
  });

  it("saves a search block; blank tavily key sends empty string (keep) and edited baseUrl", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);

    await user.clear(screen.getByLabelText(/Tavily Base URL/i));
    await user.type(screen.getByLabelText(/Tavily Base URL/i), "https://tavily.local");
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.search.provider).toBe("tavily");
    expect(payload.search.providers.tavily.baseUrl).toBe("https://tavily.local");
    // Blank key box → empty string means "keep the stored key" server-side.
    expect(payload.search.providers.tavily.apiKey).toBe("");
    expect(payload).toMatchObject({ providerMode: "openai-compatible", language: "en", defaultQueryLimit: 8 });
  });

  it("switching provider keeps each provider's fields in the payload", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);

    await user.selectOptions(screen.getByLabelText(/Search Provider/i), "searxng");
    await user.type(screen.getByLabelText(/SearXNG Instance URL/i), "https://searx.local");
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.search.provider).toBe("searxng");
    expect(payload.search.providers.searxng.url).toBe("https://searx.local");
    // Tavily's baseUrl is still carried so switching does not drop it.
    expect(payload.search.providers.tavily.baseUrl).toBe("https://api.tavily.com");
  });

  it("runs a test search with the typed query", async () => {
    const user = userEvent.setup();
    runWebSearch.mockResolvedValue({ results: [] });
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);

    await user.type(screen.getByLabelText(/Test Query/i), "hello world");
    await user.click(screen.getByRole("button", { name: /Test Search/i }));
    expect(runWebSearch).toHaveBeenCalledWith({ query: "hello world" });
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/web-search-section.test.tsx`
Expected: FAIL — stub renders `null`.

- [ ] **Step 3: Implement `web-search-section.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import {
  useRunWebSearchMutation,
  useSystemSettingsQuery,
  useUpdateSystemSettingsMutation,
} from "../queries";
import { useSettingsTopLevel } from "./use-settings-top-level";

// Editable per-provider fields. apiKey is always a plain editable string; blank
// means "keep the stored key" (backend deep-merge skips empty-string fields).
interface SearchFields {
  provider: string;
  tavilyApiKey: string;
  tavilyBaseUrl: string;
  serpapiApiKey: string;
  serpapiEngine: string;
  serpapiBaseUrl: string;
  searxngUrl: string;
  searxngCategories: string;
  ollamaApiKey: string;
  ollamaUrl: string;
}

const EMPTY: SearchFields = {
  provider: "tavily",
  tavilyApiKey: "",
  tavilyBaseUrl: "",
  serpapiApiKey: "",
  serpapiEngine: "google",
  serpapiBaseUrl: "",
  searxngUrl: "",
  searxngCategories: "general",
  ollamaApiKey: "",
  ollamaUrl: "",
};

export function WebSearchSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();
  const runWebSearch = useRunWebSearchMutation();
  const topLevel = useSettingsTopLevel(settings.data);

  const [fields, setFields] = useState<SearchFields>(EMPTY);
  const [testQuery, setTestQuery] = useState("");
  const [testError, setTestError] = useState("");
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    const search = settings.data?.search;
    if (!search) {
      return;
    }
    const snapshot = JSON.stringify(search);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    const p = search.providers ?? {};
    setFields({
      provider: search.provider ?? "tavily",
      tavilyApiKey: "",
      tavilyBaseUrl: p.tavily?.baseUrl ?? "",
      serpapiApiKey: "",
      serpapiEngine: p.serpapi?.engine ?? "google",
      serpapiBaseUrl: p.serpapi?.baseUrl ?? "",
      searxngUrl: p.searxng?.url ?? "",
      searxngCategories: (p.searxng?.categories ?? ["general"]).join(", "),
      ollamaApiKey: "",
      ollamaUrl: p.ollama?.url ?? "",
    });
  }, [settings.data?.search]);

  function set<K extends keyof SearchFields>(key: K, value: SearchFields[K]) {
    setFields((f) => ({ ...f, [key]: value }));
  }

  async function save() {
    const categories = fields.searxngCategories
      .split(",")
      .map((v) => v.trim())
      .filter((v) => v.length > 0);
    // Send every provider block so switching providers never drops a stored
    // field. apiKey = "" means keep; a typed value replaces (backend merge).
    await update.mutateAsync({
      ...topLevel,
      search: {
        provider: fields.provider,
        providers: {
          tavily: { apiKey: fields.tavilyApiKey.trim(), baseUrl: fields.tavilyBaseUrl },
          serpapi: {
            apiKey: fields.serpapiApiKey.trim(),
            engine: fields.serpapiEngine,
            baseUrl: fields.serpapiBaseUrl,
          },
          searxng: { url: fields.searxngUrl, categories: categories.length ? categories : ["general"] },
          ollama: { apiKey: fields.ollamaApiKey.trim(), url: fields.ollamaUrl },
        },
      },
    });
    setFields((f) => ({ ...f, tavilyApiKey: "", serpapiApiKey: "", ollamaApiKey: "" }));
  }

  const configured = settings.data?.search?.providers;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Web Search</CardTitle>
        <CardDescription>Search provider for the canvas search node.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <label className="grid gap-1.5 text-sm font-medium">
          Search Provider
          <Select
            aria-label="Search Provider"
            value={fields.provider}
            onChange={(e) => set("provider", e.target.value)}
          >
            <option value="none">none</option>
            <option value="tavily">tavily</option>
            <option value="serpapi">serpapi</option>
            <option value="searxng">searxng</option>
            <option value="ollama">ollama</option>
          </Select>
        </label>

        {fields.provider === "tavily" ? (
          <>
            <label className="grid gap-1.5 text-sm font-medium">
              Tavily API Key
              <Input
                type="password"
                placeholder={
                  configured?.tavily?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "tvly-..."
                }
                value={fields.tavilyApiKey}
                onChange={(e) => set("tavilyApiKey", e.target.value)}
              />
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              Tavily Base URL
              <Input
                value={fields.tavilyBaseUrl}
                onChange={(e) => set("tavilyBaseUrl", e.target.value)}
                placeholder="https://api.tavily.com"
              />
            </label>
          </>
        ) : null}

        {fields.provider === "serpapi" ? (
          <>
            <label className="grid gap-1.5 text-sm font-medium">
              SerpApi API Key
              <Input
                type="password"
                placeholder={
                  configured?.serpapi?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "..."
                }
                value={fields.serpapiApiKey}
                onChange={(e) => set("serpapiApiKey", e.target.value)}
              />
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              SerpApi Engine
              <Select
                aria-label="SerpApi Engine"
                value={fields.serpapiEngine}
                onChange={(e) => set("serpapiEngine", e.target.value)}
              >
                <option value="google">google</option>
                <option value="google_news">google_news</option>
                <option value="google_scholar">google_scholar</option>
                <option value="bing">bing</option>
                <option value="duckduckgo">duckduckgo</option>
              </Select>
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              SerpApi Base URL
              <Input
                value={fields.serpapiBaseUrl}
                onChange={(e) => set("serpapiBaseUrl", e.target.value)}
                placeholder="https://serpapi.com"
              />
            </label>
          </>
        ) : null}

        {fields.provider === "searxng" ? (
          <>
            <label className="grid gap-1.5 text-sm font-medium">
              SearXNG Instance URL
              <Input
                value={fields.searxngUrl}
                onChange={(e) => set("searxngUrl", e.target.value)}
                placeholder="https://search.example.com"
              />
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              SearXNG Categories (comma-separated)
              <Input
                value={fields.searxngCategories}
                onChange={(e) => set("searxngCategories", e.target.value)}
              />
            </label>
          </>
        ) : null}

        {fields.provider === "ollama" ? (
          <>
            <label className="grid gap-1.5 text-sm font-medium">
              Ollama API Key
              <Input
                type="password"
                placeholder={
                  configured?.ollama?.apiKeyConfigured
                    ? "Leave blank to keep the current key"
                    : "..."
                }
                value={fields.ollamaApiKey}
                onChange={(e) => set("ollamaApiKey", e.target.value)}
              />
            </label>
            <label className="grid gap-1.5 text-sm font-medium">
              Ollama Search URL
              <Input
                value={fields.ollamaUrl}
                onChange={(e) => set("ollamaUrl", e.target.value)}
                placeholder="https://ollama.com"
              />
            </label>
          </>
        ) : null}

        <div className="flex justify-end">
          <Button onClick={save} disabled={update.isPending}>
            Save
          </Button>
        </div>

        <div className="grid gap-2 border-t pt-4">
          <label className="grid gap-1.5 text-sm font-medium">
            Test Query
            <Input
              value={testQuery}
              onChange={(e) => setTestQuery(e.target.value)}
              placeholder="e.g. knowledge graphs"
            />
          </label>
          <div className="flex gap-2">
            <Button
              variant="outline"
              disabled={runWebSearch.isPending || testQuery.trim().length === 0}
              onClick={async () => {
                setTestError("");
                try {
                  await runWebSearch.mutateAsync({ query: testQuery.trim() });
                } catch (error) {
                  setTestError(error instanceof Error ? error.message : "Web search failed.");
                }
              }}
            >
              Test Search
            </Button>
          </div>
          {testError ? (
            <Alert>
              <AlertTitle>Web search failed</AlertTitle>
              <AlertDescription>{testError}</AlertDescription>
            </Alert>
          ) : null}
          {runWebSearch.data?.results.length ? (
            <ul className="grid gap-3">
              {runWebSearch.data.results.map((result) => (
                <li key={result.url} className="rounded border p-3">
                  <a className="font-medium" href={result.url} rel="noreferrer" target="_blank">
                    {result.title}
                  </a>
                  {result.source ? (
                    <p className="text-xs text-muted-foreground">{result.source}</p>
                  ) : null}
                  {result.snippet ? <p className="mt-1 text-sm">{result.snippet}</p> : null}
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/web-search-section.test.tsx`
Expected: PASS — default tavily, blank-key-keeps + edited baseUrl, provider-switch carries all blocks, test search.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/settings/sections/web-search-section.tsx apps/admin/src/features/settings/sections/web-search-section.test.tsx
git commit -m "feat(admin): web search section (Tavily default, per-provider persisted configs)"
```

---

## Task 10: Defaults section

`language` + `defaultQueryLimit`. Saves a `defaults` block (and, per the PATCH contract, these also become the required top-level `language`/`defaultQueryLimit` — so the section sends both consistently). Hydrates from `settings.data.defaults`.

**Files:**
- Create/replace: `apps/admin/src/features/settings/sections/defaults-section.tsx`
- Test: `apps/admin/src/features/settings/sections/defaults-section.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { DefaultsSection } from "./defaults-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
}));

describe("DefaultsSection", () => {
  it("hydrates from defaults block and saves both defaults + top-level", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      defaults: { language: "en", defaultQueryLimit: 5 },
    });

    render(<DefaultsSection />);

    expect(screen.getByLabelText(/language/i)).toHaveValue("en");
    await user.clear(screen.getByLabelText(/default query limit/i));
    await user.type(screen.getByLabelText(/default query limit/i), "12");
    await user.click(screen.getByRole("button", { name: /save/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.defaults).toMatchObject({ language: "en", defaultQueryLimit: 12 });
    expect(payload).toMatchObject({ language: "en", defaultQueryLimit: 12, providerMode: "openai-compatible" });
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/defaults-section.test.tsx`
Expected: FAIL — stub renders `null`.

- [ ] **Step 3: Implement `defaults-section.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useSystemSettingsQuery, useUpdateSystemSettingsMutation } from "../queries";

export function DefaultsSection() {
  const settings = useSystemSettingsQuery();
  const update = useUpdateSystemSettingsMutation();

  const [language, setLanguage] = useState("en");
  const [defaultQueryLimit, setDefaultQueryLimit] = useState("5");
  const hydratedFrom = useRef<string>("");

  useEffect(() => {
    const defaults = settings.data?.defaults;
    if (!defaults) {
      return;
    }
    const snapshot = JSON.stringify(defaults);
    if (hydratedFrom.current === snapshot) {
      return;
    }
    hydratedFrom.current = snapshot;
    setLanguage(defaults.language);
    setDefaultQueryLimit(String(defaults.defaultQueryLimit));
  }, [settings.data?.defaults]);

  async function save() {
    const limit = Number(defaultQueryLimit);
    await update.mutateAsync({
      providerMode: settings.data?.providerMode ?? "openai-compatible",
      language,
      defaultQueryLimit: limit,
      defaults: { language, defaultQueryLimit: limit },
    });
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Defaults</CardTitle>
        <CardDescription>Language and default query behavior.</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <label className="grid gap-1.5 text-sm font-medium">
          Language
          <Input value={language} onChange={(e) => setLanguage(e.target.value)} />
        </label>
        <label className="grid gap-1.5 text-sm font-medium">
          Default Query Limit
          <Input
            aria-label="Default Query Limit"
            value={defaultQueryLimit}
            onChange={(e) => setDefaultQueryLimit(e.target.value)}
          />
        </label>
        <div className="flex justify-end">
          <Button onClick={save} disabled={update.isPending}>
            Save
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/settings/sections/defaults-section.test.tsx`
Expected: PASS.

- [ ] **Step 5: Run the whole settings feature + type-check**

Run: `cd apps/admin && npx vitest run src/features/settings && npx tsc --noEmit`
Expected: settings tests PASS. `tsc` may still fail in `dashboard/page.tsx` and the canvas board (fixed in Tasks 11-13) — that is expected.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/settings/sections/defaults-section.tsx apps/admin/src/features/settings/sections/defaults-section.test.tsx
git commit -m "feat(admin): defaults settings section"
```

---

## Task 11: Capability-aware node model tag in the canvas board

Today `canvas-board.tsx` injects one `__model` (from the legacy top-level `providerModel`) into every node. The redesigned settings no longer treat that as the source of truth: the `ai_analyze` node should show the **active connection's** model; the `ai_image` node should show the **image** model. Compute both from settings and inject the right one per node type.

**Files:**
- Modify: `apps/admin/src/features/canvas/canvas-board.tsx`
- Test: `apps/admin/src/features/canvas/canvas-board.test.tsx` (extend)

- [ ] **Step 1: Write the failing test** — replace the `useSystemSettingsQuery` mock and add a capability-aware assertion in `apps/admin/src/features/canvas/canvas-board.test.tsx`. Change the mock at the top (currently `{ data: { providerModel: "gpt-test" } }`) to:

```tsx
vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      providerMode: "openai-compatible",
      connections: [
        { id: "c1", label: "Active", baseUrl: "u", model: "analyze-model", timeoutSeconds: null, isActive: true, apiKeyConfigured: true },
      ],
      image: { baseUrl: "u", model: "image-model", size: "1024x1024", timeoutSeconds: null, apiKeyConfigured: true },
    },
  }),
}));
```

Then add this test inside `describe("CanvasBoard", ...)`:

```tsx
it("injects the active-connection model into ai_analyze and the image model into ai_image", () => {
  const analyzeDoc: CanvasDocument = {
    nodes: [
      { id: "a1", type: "ai_analyze", x: 0, y: 0, w: 360, h: 320, data: {} },
      { id: "i1", type: "ai_image", x: 0, y: 0, w: 320, h: 400, data: {} },
    ],
    edges: [],
    viewport: { x: 0, y: 0, zoom: 1 },
  };
  render(
    <CanvasBoard
      document={analyzeDoc}
      onChange={() => {}}
      onRunNode={() => {}}
      onFetchUrl={() => {}}
      onSearchNode={() => {}}
    />,
  );
  expect(screen.getByText("analyze-model")).toBeInTheDocument();
  expect(screen.getByText("image-model")).toBeInTheDocument();
});
```

> The `@xyflow/react` mock in this test file renders `flow` text only, not real nodes — so this new test needs real node rendering. Rather than fight the existing lightweight mock, put the capability-model assertion in `board-render.test.tsx` (which uses the real React Flow + ResizeObserver polyfill). **Do the following instead:** keep the `canvas-board.test.tsx` mock update above (so its existing tests still compile), but add the *rendering* assertion to `board-render.test.tsx` in Step 1b.

- [ ] **Step 1b: Add the rendering assertion to `board-render.test.tsx`** — update its settings mock and append a test:

```tsx
// Replace the existing mock:
vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      providerMode: "openai-compatible",
      connections: [
        { id: "c1", label: "Active", baseUrl: "u", model: "analyze-model", timeoutSeconds: null, isActive: true, apiKeyConfigured: true },
      ],
      image: { baseUrl: "u", model: "image-model", size: "1024x1024", timeoutSeconds: null, apiKeyConfigured: true },
    },
  }),
}));
```

```tsx
// Append inside describe("CanvasBoard rendering", ...):
it("shows the active-connection model on ai_analyze and image model on ai_image", () => {
  const doc: CanvasDocument = {
    nodes: [
      { id: "a1", type: "ai_analyze", x: 0, y: 0, w: 360, h: 320, data: {} },
      { id: "i1", type: "ai_image", x: 0, y: 0, w: 320, h: 400, data: {} },
    ],
    edges: [],
    viewport: { x: 0, y: 0, zoom: 1 },
  };
  const { getByText } = render(
    <CanvasBoard document={doc} onChange={vi.fn()} onRunNode={vi.fn()} onFetchUrl={vi.fn()} onSearchNode={vi.fn()} />,
  );
  expect(getByText("analyze-model")).toBeInTheDocument();
  expect(getByText("image-model")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/canvas/board-render.test.tsx`
Expected: FAIL — `ai_image` still shows the analyze/legacy model (or `not configured`), so `getByText("image-model")` throws.

- [ ] **Step 3: Make the board capability-aware.** In `apps/admin/src/features/canvas/canvas-board.tsx`:

Replace the single model read (line ~166):

```tsx
  const model = useSystemSettingsQuery().data?.providerModel ?? null;
```

with:

```tsx
  const settings = useSystemSettingsQuery().data;
  const analyzeModel = settings?.connections?.find((c) => c.isActive)?.model ?? null;
  const imageModel = settings?.image?.model ?? null;
```

Replace the `__model: model` injection (line ~210) with a per-type selection:

```tsx
          __model: n.type === "ai_image" ? imageModel : analyzeModel,
```

Update the `useMemo` deps (line ~216): replace `model` with `analyzeModel, imageModel`:

```tsx
    [document.nodes, patchNode, onRunNode, onFetchUrl, onSearchNode, analyzeModel, imageModel, selectedIds],
```

> `modelOf(data)` and the adapters are unchanged — each node still reads `data.__model`; the board now feeds the correct capability model per node type. Only `ai_analyze` and `ai_image` render a `ModelTag`, so other node types receiving `analyzeModel` is harmless (they never display it).

- [ ] **Step 4: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/canvas/board-render.test.tsx src/features/canvas/canvas-board.test.tsx`
Expected: PASS — both files (existing tests + the new capability assertion).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/canvas-board.tsx apps/admin/src/features/canvas/board-render.test.tsx apps/admin/src/features/canvas/canvas-board.test.tsx
git commit -m "feat(admin): capability-aware node model tag (analyze=active connection, image=image model)"
```

---

## Task 12: Repoint the dashboard to the `defaults` block

The new GET nests `language`/`defaultQueryLimit` under `defaults`. `dashboard/page.tsx` reads them top-level (lines 59, 65, 95), which now yields `undefined`. Repoint to `settings.data?.defaults?.…`. `providerMode` stays top-level.

**Files:**
- Modify: `apps/admin/src/features/dashboard/page.tsx`
- Modify: `apps/admin/src/features/dashboard/page.test.tsx`

- [ ] **Step 1: Update the test mock first** — in `apps/admin/src/features/dashboard/page.test.tsx`, change the settings mock (lines ~28-37) to nest defaults:

```tsx
vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      providerMode: "openai-compatible",
      defaults: { language: "en", defaultQueryLimit: 5 },
    },
    isLoading: false,
  }),
}));
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd apps/admin && npx vitest run src/features/dashboard/page.test.tsx`
Expected: FAIL — the page still reads `settings.data?.language` (now `undefined`), so the "Language" card renders `unknown` instead of `en`. (The existing test asserts `getByText("openai-compatible")` which still passes, but add an explicit assertion.) Append to the existing test body:

```tsx
    expect(screen.getByText("en")).toBeInTheDocument();
    expect(screen.getByText("5")).toBeInTheDocument();
```

Re-run — Expected: FAIL on `getByText("en")`.

- [ ] **Step 3: Repoint `dashboard/page.tsx`.** Replace the three reads:

Line ~59:
```tsx
          <CardTitle className="text-2xl font-semibold">{settings.data?.defaults?.language ?? "unknown"}</CardTitle>
```

Line ~65:
```tsx
          <CardTitle className="text-2xl font-semibold">{settings.data?.defaults?.defaultQueryLimit ?? 0}</CardTitle>
```

Lines ~95-96 (inside the Workspace Settings card):
```tsx
            <p>{settings.data?.providerMode ?? "unknown"}</p>
            <p>{settings.data?.defaults?.language ?? "unknown"}</p>
            <p>{settings.data?.defaults?.defaultQueryLimit ?? 0}</p>
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd apps/admin && npx vitest run src/features/dashboard/page.test.tsx`
Expected: PASS — `openai-compatible`, `en`, and `5` all render.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/dashboard/page.tsx apps/admin/src/features/dashboard/page.test.tsx
git commit -m "fix(admin): dashboard reads language/queryLimit from defaults block"
```

---

## Task 13: Update remaining settings-shape test mocks

Two canvas tests still mock the flat `{ providerModel: "gpt-test" }` shape. `add-node-integration.test.tsx` renders the board; with the capability-aware board (Task 11), AI nodes now read `connections`/`image`. If the test asserts nothing about the model tag it will still pass, but the mock should match reality to avoid future confusion and silent `not configured` renders. The projects operations test mocks (`operations-actions.test.tsx`, `operations-pages.test.tsx`, `detail-page.test.tsx`) provide flat settings but those pages don't read the moved fields — verify they still pass untouched.

**Files:**
- Modify: `apps/admin/src/features/canvas/add-node-integration.test.tsx`
- Verify (no change expected): `projects/operations-actions.test.tsx`, `projects/operations-pages.test.tsx`, `projects/detail-page.test.tsx`

- [ ] **Step 1: Update the `add-node-integration.test.tsx` settings mock** (line ~45):

```tsx
vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      providerMode: "openai-compatible",
      connections: [
        { id: "c1", label: "Active", baseUrl: "u", model: "gpt-test", timeoutSeconds: null, isActive: true, apiKeyConfigured: true },
      ],
      image: { baseUrl: "u", model: "img-test", size: "1024x1024", timeoutSeconds: null, apiKeyConfigured: true },
    },
  }),
}));
```

- [ ] **Step 2: Run the full canvas + projects suites**

Run: `cd apps/admin && npx vitest run src/features/canvas src/features/projects`
Expected: PASS — all canvas and projects tests. If a projects test fails because it read a moved field, repoint it the same way as the dashboard (nest under `defaults`); otherwise leave untouched.

- [ ] **Step 3: Commit** (only if files changed)

```bash
git add apps/admin/src/features/canvas/add-node-integration.test.tsx
git commit -m "test(admin): align canvas settings mock with capability-aware board"
```

---

## Task 14: Full verification

- [ ] **Step 1: Run the entire admin test suite + type-check**

Run: `cd apps/admin && npx vitest run && npx tsc --noEmit`
Expected: All tests PASS; `tsc` clean (no errors).

- [ ] **Step 2: Lint (if the repo lints admin)**

Run: `cd apps/admin && npm run lint`
Expected: clean (or no lint script — skip if absent).

- [ ] **Step 3: Manual e2e** (requires the backend plan implemented + running)

```bash
docker compose build admin backend && docker compose up -d
```

Hard-refresh `http://localhost:4173/settings` and verify:
- The Settings page shows the left rail (LLM / Embedding / Image / Web Search / Defaults).
- **LLM:** the migrated "Default" connection shows Active. Add a second connection; activate it; confirm exactly one Active badge. Delete a connection — the list updates and another auto-activates.
- **Image:** set base URL + image model + size; save. On a canvas, an `ai_image` node's tag shows the image model and generating produces a real image preview (this is the core bug fix).
- **Web search:** Tavily is the default/active provider with its key retained (placeholder says "Leave blank to keep"); the canvas search node returns markdown results. Switch to searxng and back — Tavily's baseUrl is retained.
- **Embedding:** enable + configure; retrieval uses the embedding model.
- **Defaults:** change language / query limit; the Dashboard cards reflect the new values.
- On a canvas, an `ai_analyze` node's tag shows the **active connection's** model (switch active connection → tag updates after refetch).

- [ ] **Step 4: Finish the branch**

Use the `superpowers:finishing-a-development-branch` skill.

---

## Self-Review (completed by plan author)

- **Spec coverage (Phases 3-4):**
  - Phase 3 "Settings UI — sub-nav + five sections, redesigned queries/schema": Tasks 2 (schema), 3-4 (queries/API), 5 (sub-nav shell), 6-10 (five sections). ✔
  - Phase 4 "Node wiring + verification — capability-aware model tag; verify image + search": Task 11 (capability-aware `__model`), Task 14 Step 3 (image + search e2e verification). ✔
  - Downstream consumers of the changed GET shape: dashboard (Task 12), canvas mocks (Tasks 11, 13). ✔
  - `switch.tsx` for the embedding toggle (memory: use shadcn, don't hand-roll): Task 1. ✔
- **Placeholder scan:** every code step contains complete code; every run step has an exact command + expected result. No TBD/TODO. ✔
- **Type consistency:** `SystemSettings`/`ProviderConnection` exported from `shared/api.ts` (Task 2) and consumed by sections (Task 6), the top-level helper (Task 7), and the board (Task 11). Mutation names (`useCreateConnectionMutation`/`useUpdateConnectionMutation`/`useDeleteConnectionMutation`/`useActivateConnectionMutation`) and API fn names (`createProviderConnection`/`updateProviderConnection`/`deleteProviderConnection`/`activateProviderConnection`) match between Tasks 3, 4, and 6. Section component names match the shell imports and stubs (Task 5) exactly. ✔
- **Contract alignment:** payload shapes (camelCase, blank-keeps-key, `clearApiKey`, `search.providers` deep-merge with blank-string=keep) match the backend plan Tasks 8-10 verbatim. ✔

