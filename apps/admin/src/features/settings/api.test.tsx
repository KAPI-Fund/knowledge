import { afterEach, describe, expect, it, vi } from "vitest";

import { getSystemSettings } from "../shared/api";
import {
  activateProviderConnection,
  createProviderConnection,
  deleteProviderConnection,
  updateProviderConnection,
  updateSystemSettings,
} from "../shared/api";

afterEach(() => vi.restoreAllMocks());

const NEW_SHAPE = {
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

function okResponse() {
  return new Response(JSON.stringify({}), { status: 200 });
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
      image: { baseUrl: "https://img", model: "gpt-image-1", size: "1024x1024", apiKey: "" },
      embedding: { enabled: true, baseUrl: "https://emb", model: "e", apiKey: "sk-e" },
      search: {
        provider: "tavily",
        providers: { tavily: { apiKey: "", baseUrl: "https://api.tavily.com" } },
      },
      defaults: { language: "fr", defaultQueryLimit: 12 },
    });
    const body = JSON.parse(fetchMock.mock.calls[0][1]?.body as string);
    expect(body.image).toMatchObject({ baseUrl: "https://img", model: "gpt-image-1", size: "1024x1024" });
    expect(body.image).not.toHaveProperty("apiKey");
    expect(body.embedding).toMatchObject({ enabled: true, apiKey: "sk-e" });
    expect(body.search.providers.tavily.baseUrl).toBe("https://api.tavily.com");
    expect(body.search.providers.tavily).not.toHaveProperty("apiKey");
    expect(body.defaults).toMatchObject({ language: "fr", defaultQueryLimit: 12 });
  });

  it("sends image.clearApiKey when flagged and key blank", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(okResponse());
    await updateSystemSettings({
      image: { baseUrl: "https://img", model: "m", apiKey: "", clearApiKey: true },
    });
    const body = JSON.parse(fetchMock.mock.calls[0][1]?.body as string);
    expect(body.image.clearApiKey).toBe(true);
    expect(body.image).not.toHaveProperty("apiKey");
  });
});
