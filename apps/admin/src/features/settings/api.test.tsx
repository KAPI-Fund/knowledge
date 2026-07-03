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
