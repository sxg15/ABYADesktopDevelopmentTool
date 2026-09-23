import { describe, expect, it } from "vitest";
import {
  MCP_CLIENT_PRESETS,
  buildMcpClientConfig,
} from "./mcpConfigTemplates";

const endpoint = "http://127.0.0.1:47600/mcp";
const token = "desktop-token";

describe("MCP client configuration templates", () => {
  it("provides common clients plus a generic format", () => {
    expect(MCP_CLIENT_PRESETS.map((preset) => preset.id)).toEqual([
      "codex",
      "grok",
      "claude-code",
      "vscode",
      "cursor",
      "windsurf",
      "gemini-cli",
      "generic-json",
    ]);
  });

  it("includes the active endpoint and bearer token in every template", () => {
    for (const preset of MCP_CLIENT_PRESETS) {
      const result = buildMcpClientConfig(preset.id, endpoint, token);
      expect(result).toContain(endpoint);
      expect(result).toContain(`Bearer ${token}`);
    }
  });

  it("generates valid JSON for JSON-based clients", () => {
    for (const preset of MCP_CLIENT_PRESETS.filter(
      (item) => item.id !== "codex" && item.id !== "grok",
    )) {
      expect(() =>
        JSON.parse(buildMcpClientConfig(preset.id, endpoint, token)),
      ).not.toThrow();
    }
  });

  it("uses each client's expected top-level transport field", () => {
    expect(buildMcpClientConfig("codex", endpoint, token)).toContain(
      "[mcp_servers.abya-desktop]",
    );
    const grok = buildMcpClientConfig("grok", endpoint, token);
    expect(grok).toContain("[mcp_servers.abya-desktop]");
    expect(grok).toContain(
      'headers = { Authorization = "Bearer desktop-token" }',
    );
    expect(grok).not.toContain("http_headers");
    expect(
      JSON.parse(buildMcpClientConfig("vscode", endpoint, token)).servers,
    ).toBeDefined();
    expect(
      JSON.parse(buildMcpClientConfig("windsurf", endpoint, token)).mcpServers[
        "abya-desktop"
      ].serverUrl,
    ).toBe(endpoint);
    expect(
      JSON.parse(buildMcpClientConfig("gemini-cli", endpoint, token))
        .mcpServers["abya-desktop"].httpUrl,
    ).toBe(endpoint);
  });
});
