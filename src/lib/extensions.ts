import { invoke, isTauri } from "@tauri-apps/api/core";
export interface Skill {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
  subdirectory: string;
  source?: string;
  catalogueSkill?: string;
}
export interface McpConfig {
  id: string;
  name: string;
  enabled: boolean;
  transport: "http" | "stdio";
  url: string;
  command: string;
  args: string[];
  cwd: string;
  auth: "none" | "token" | "oauth";
  clientId: string;
  scopes: string[];
  redirectPort?: number;
}
export interface McpServer extends McpConfig {
  connected: boolean;
  toolCount: number;
  toolNames: { name: string; description: string }[];
  authPending?: boolean;
  authUrl?: string;
  loginId?: string;
  error?: string | null;
}
export interface ExtensionSnapshot {
  skills: Skill[];
  servers: McpServer[];
}
export interface McpSecrets {
  token?: string;
  headers?: Record<string, string>;
  env?: Record<string, string>;
  client_secret?: string;
}
export const snapshot = () => invoke<ExtensionSnapshot>("extensions_snapshot");
export interface CatalogueSkill {
  id: string;
  name: string;
  source: string;
  skillId?: string;
  installs: number;
}
export const skillCatalogue = (query: string) =>
  invoke<{ skills: CatalogueSkill[] }>("skills_catalogue", { query });
export const installSkill = (source: string, skill: string) =>
  invoke<Skill>("install_catalogue_skill", { source, skill });
export const context = () =>
  isTauri() ? invoke<string>("extension_context") : Promise.resolve("");
export const skillAction = (
  action: string,
  args: {
    id?: string;
    source?: string;
    content?: string;
    enabled?: boolean;
  } = {},
) =>
  invoke<Skill | { content: string } | null>("skill_action", {
    action,
    id: null,
    source: null,
    content: null,
    enabled: null,
    ...args,
  });
export const mcpAction = (
  action: string,
  args: {
    id?: string;
    config?: McpConfig;
    secret?: McpSecrets;
    loginId?: string;
  } = {},
) =>
  invoke<{ authUrl?: string; loginId?: string } | null>("mcp_action", {
    action,
    id: null,
    config: null,
    secret: null,
    loginId: null,
    ...args,
  });
export interface RegistryServer {
  name: string;
  title?: string;
  description: string;
  version: string;
  websiteUrl?: string;
  repository?: { url: string };
  remotes?: { type: string; url: string }[];
  packages?: { registryType: string; identifier: string; version?: string }[];
}
export interface RegistryResult {
  servers: { server: RegistryServer }[];
  metadata?: { nextCursor?: string };
}
export const catalogue = (query: string, cursor?: string) =>
  invoke<RegistryResult>("extension_catalog", {
    query,
    cursor: cursor || null,
  });
export const newMcp = (patch: Partial<McpConfig> = {}): McpConfig => ({
  id: crypto.randomUUID(),
  name: "",
  enabled: true,
  transport: "http",
  url: "",
  command: "",
  args: [],
  cwd: "",
  auth: "none",
  clientId: "",
  scopes: [],
  ...patch,
});
export function registryConfig(server: RegistryServer): McpConfig | null {
  const remote = server.remotes?.find((r) => r.type === "streamable-http");
  if (remote)
    return newMcp({ name: server.title || server.name, url: remote.url });
  const pkg = server.packages?.find(
    (p) => p.registryType === "npm" || p.registryType === "pypi",
  );
  if (!pkg) return null;
  return newMcp({
    name: server.title || server.name,
    transport: "stdio",
    command: pkg.registryType === "npm" ? "npx" : "uvx",
    args:
      pkg.registryType === "npm"
        ? ["-y", `${pkg.identifier}${pkg.version ? `@${pkg.version}` : ""}`]
        : [`${pkg.identifier}${pkg.version ? `==${pkg.version}` : ""}`],
  });
}
