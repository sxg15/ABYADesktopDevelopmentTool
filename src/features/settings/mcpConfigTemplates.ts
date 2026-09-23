export type McpClientPresetId =
  | "codex"
  | "grok"
  | "claude-code"
  | "vscode"
  | "cursor"
  | "windsurf"
  | "gemini-cli"
  | "generic-json";

export interface McpClientPreset {
  id: McpClientPresetId;
  label: string;
  configPath: string;
}

export const MCP_CLIENT_PRESETS: readonly McpClientPreset[] = [
  {
    id: "codex",
    label: "Codex",
    configPath: "~/.codex/config.toml",
  },
  {
    id: "grok",
    label: "Grok",
    configPath: "~/.grok/config.toml",
  },
  {
    id: "claude-code",
    label: "Claude Code",
    configPath: ".mcp.json",
  },
  {
    id: "vscode",
    label: "Visual Studio Code",
    configPath: ".vscode/mcp.json",
  },
  {
    id: "cursor",
    label: "Cursor",
    configPath: ".cursor/mcp.json",
  },
  {
    id: "windsurf",
    label: "Windsurf",
    configPath: "~/.codeium/windsurf/mcp_config.json",
  },
  {
    id: "gemini-cli",
    label: "Gemini CLI",
    configPath: "~/.gemini/settings.json",
  },
  {
    id: "generic-json",
    label: "Generic MCP JSON",
    configPath: "mcp.json",
  },
] as const;

const SERVER_NAME = "abya-desktop";

export function buildMcpClientConfig(
  preset: McpClientPresetId,
  endpoint: string,
  token: string,
): string {
  const authorization = `Bearer ${token}`;
  switch (preset) {
    case "codex":
      return [
        `[mcp_servers.${SERVER_NAME}]`,
        `url = "${escapeToml(endpoint)}"`,
        `http_headers = { Authorization = "${escapeToml(authorization)}" }`,
      ].join("\n");
    case "grok":
      return [
        `[mcp_servers.${SERVER_NAME}]`,
        `url = "${escapeToml(endpoint)}"`,
        `headers = { Authorization = "${escapeToml(authorization)}" }`,
      ].join("\n");
    case "vscode":
      return formatJson({
        servers: {
          [SERVER_NAME]: {
            type: "http",
            url: endpoint,
            headers: { Authorization: authorization },
          },
        },
      });
    case "windsurf":
      return formatJson({
        mcpServers: {
          [SERVER_NAME]: {
            serverUrl: endpoint,
            headers: { Authorization: authorization },
          },
        },
      });
    case "gemini-cli":
      return formatJson({
        mcpServers: {
          [SERVER_NAME]: {
            httpUrl: endpoint,
            headers: { Authorization: authorization },
          },
        },
      });
    case "generic-json":
      return formatJson({
        mcpServers: {
          [SERVER_NAME]: {
            type: "streamable-http",
            url: endpoint,
            headers: { Authorization: authorization },
          },
        },
      });
    case "claude-code":
      return formatJson({
        mcpServers: {
          [SERVER_NAME]: {
            type: "http",
            url: endpoint,
            headers: { Authorization: authorization },
          },
        },
      });
    case "cursor":
      return formatJson({
        mcpServers: {
          [SERVER_NAME]: {
            url: endpoint,
            headers: { Authorization: authorization },
          },
        },
      });
  }
}

function formatJson(value: object): string {
  return JSON.stringify(value, null, 2);
}

function escapeToml(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}
