import { describe, expect, it } from "vitest";
import { parseCurrentUser } from "./schemas";

describe("parseCurrentUser", () => {
  it("parses a valid current-user payload", () => {
    expect(parseCurrentUser({ id: "u1", username: "admin", role: "admin" }).username).toBe(
      "admin",
    );
  });
});
