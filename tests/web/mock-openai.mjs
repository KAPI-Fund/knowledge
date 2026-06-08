import http from "node:http";

const port = Number(process.env.KNOWLEDGE_MOCK_OPENAI_PORT ?? "18080");

const payload = JSON.stringify({
  id: "chatcmpl-web-mock-1",
  object: "chat.completion",
  created: 1717171717,
  model: "mock-model",
  choices: [
    {
      index: 0,
      message: {
        role: "assistant",
        content: "Attention focuses computation on relevant tokens.",
      },
      finish_reason: "stop",
    },
  ],
  usage: {
    prompt_tokens: 17,
    completion_tokens: 25,
    total_tokens: 42,
  },
});

const server = http.createServer((request, response) => {
  if (request.method === "POST" && request.url === "/v1/chat/completions") {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(payload);
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
