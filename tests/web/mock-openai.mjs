import http from "node:http";

const port = Number(process.env.KNOWLEDGE_MOCK_OPENAI_PORT ?? "18080");

const DETECTOR_REPLY = JSON.stringify({
  groups: [
    {
      slugs: ["attention", "attention-mechanism"],
      reason: "Both describe the attention mechanism.",
      confidence: "high",
    },
  ],
});

const MERGER_REPLY = [
  "---",
  "title: Attention Mechanism",
  "type: concept",
  "---",
  "",
  "Attention focuses computation on relevant tokens across the sequence.",
].join("\n");

const DEFAULT_REPLY = "Attention focuses computation on relevant tokens.";

function chatContentFor(parsed) {
  const messages = Array.isArray(parsed.messages) ? parsed.messages : [];
  const system = messages.find((message) => message.role === "system");
  const systemText = typeof system?.content === "string" ? system.content : "";

  if (systemText.includes("Identify groups of slugs")) {
    return DETECTOR_REPLY;
  }
  if (systemText.includes("describe the same entity or concept under different names")) {
    return MERGER_REPLY;
  }
  return DEFAULT_REPLY;
}

function chatPayload(content) {
  return JSON.stringify({
    id: "chatcmpl-web-mock-1",
    object: "chat.completion",
    created: 1717171717,
    model: "mock-model",
    choices: [
      {
        index: 0,
        message: { role: "assistant", content },
        finish_reason: "stop",
      },
    ],
    usage: {
      prompt_tokens: 17,
      completion_tokens: 25,
      total_tokens: 42,
    },
  });
}

function fakeEmbeddingForText(text) {
  const lower = String(text).toLowerCase();
  if (lower.includes("rope") || lower.includes("rotary")) {
    return [1, 0, 0];
  }
  if (lower.includes("attention")) {
    return [0, 1, 0];
  }
  return [0, 0, 1];
}

const server = http.createServer((request, response) => {
  if (request.method === "POST" && request.url === "/v1/chat/completions") {
    let body = "";
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      let parsed = {};
      try {
        parsed = JSON.parse(body);
      } catch {
        parsed = {};
      }

      if (parsed.stream === true) {
        response.writeHead(200, { "content-type": "text/event-stream" });
        response.write('data: {"choices":[{"delta":{"content":"Attention "}}]}\n\n');
        response.write(
          'data: {"choices":[{"delta":{"content":"focuses computation on relevant tokens."}}]}\n\n',
        );
        response.write("data: [DONE]\n\n");
        response.end();
        return;
      }

      response.writeHead(200, { "content-type": "application/json" });
      response.end(chatPayload(chatContentFor(parsed)));
    });
    return;
  }

  if (request.method === "POST" && request.url === "/v1/embeddings") {
    let body = "";
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      let parsed = {};
      try {
        parsed = JSON.parse(body);
      } catch {
        parsed = {};
      }

      const input = typeof parsed.input === "string" ? parsed.input : "";
      response.writeHead(200, { "content-type": "application/json" });
      response.end(
        JSON.stringify({
          object: "list",
          data: [
            {
              object: "embedding",
              index: 0,
              embedding: fakeEmbeddingForText(input),
            },
          ],
          model: "mock-embedding",
          usage: { prompt_tokens: 4, total_tokens: 4 },
        }),
      );
    });
    return;
  }

  if (request.method === "GET" && request.url?.startsWith("/search")) {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(
      JSON.stringify({
        results: [
          {
            title: "Knowledge graphs explained",
            url: "https://example.com/knowledge-graphs",
            content: "An overview of knowledge graphs and their applications.",
            engine: "mock",
          },
        ],
      }),
    );
    return;
  }

  response.writeHead(404, { "content-type": "application/json" });
  response.end(JSON.stringify({ error: "not found" }));
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`mock-openai listening on ${port}\n`);
});

function shutdown() {
  server.close(() => process.exit(0));
}

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
