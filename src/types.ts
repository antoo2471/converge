import type { ReasoningEffort } from "./core/reasoning";
import { allTools } from "./core/toolPreferences";
import type { ResponsePreset } from "./core/responsePresets";
import type { Appearance } from "./core/appearance";
export type Protocol =
  "ollama" | "openai" | "anthropic" | "comfyui" | "codex" | "cli" | "responses";
export type ProviderPluginId =
  | "local"
  | "custom"
  | "chatgpt"
  | "antigravity"
  | "zai"
  | "claude-code"
  | "opencode"
  | "grok"
  | "minimax"
  | "kimi";
export interface Capabilities {
  vision: boolean;
  tools: boolean;
  images: boolean;
  context: number;
  workflow?: string;
}
export interface Provider {
  request?: {
    zai?: { authMode: "oauth" | "api-key" };
    cli?: { executable?: string };
    chatPath?: string;
    modelsPath?: string;
    body?: Record<string, unknown>;
    chatgpt?: {
      executable?: string;
      models?: {
        id: string;
        name: string;
        efforts: string[];
        vision: boolean;
        defaultEffort?: string | null;
      }[];
    };
  };
  id: string;
  name: string;
  plugin: ProviderPluginId;
  protocol: Protocol;
  url: string;
  headers: Record<string, string>;
  models: string[];
  capabilities: Capabilities;
  enabled: boolean;
}
export interface Attachment {
  id: string;
  name: string;
  mime: string;
  data: string;
}
export interface Message {
  reasoning?: string;
  execution?: string;
  id: string;
  turnId?: string;
  role: "user" | "assistant" | "system";
  content: string;
  model?: string;
  providerId?: string;
  modelId?: string;
  attachments?: Attachment[];
  error?: string;
  interrupted?: boolean;
  excluded?: boolean;
  tool?: { name: string; args: Record<string, unknown>; result?: string };
  toolCalls?: ToolCall[];
}
export interface ToolCall {
  phase?: string;
  id: string;
  name: string;
  args: Record<string, unknown>;
  status:
    | "preparing"
    | "awaiting"
    | "running"
    | "succeeded"
    | "failed"
    | "denied"
    | "cancelled";
  result?: string;
  output?: string;
  changes?: { added: number; removed: number; exact: boolean; diff?: string };
  writeProgress?: { written: number; total: number };
}
export interface ToolPermission {
  id: string;
  conversationId: string;
  name: string;
  args: Record<string, unknown>;
  root: string | null;
  requiresRoot: boolean;
  addWorkspace?: boolean;
}
export interface Conversation {
  selection?: { providerId: string; model: string; mode?: "codex" };
  id: string;
  title: string;
  titleSource?: "pending" | "generated" | "manual";
  projectId: string | null;
  messages: Message[];
  createdAt: number;
  updatedAt: number;
  parentId?: string;
}
export interface Project {
  id: string;
  name: string;
}
export interface ImageRecord {
  id: string;
  prompt: string;
  model: string;
  data: string;
  createdAt: number;
}
export interface Settings {
  reasoningEfforts?: Record<string, ReasoningEffort>;
  toolDefaultsVersion?: 1 | 2;
  responsePreset?: ResponsePreset;
  appearance?: Appearance;
  theme: "dark" | "light" | "system";
  temperature: number;
  maxTokens: number;
  contextMessages: number;
  systemPrompt: string;
  generativeUI: boolean;
  searchUrl: string;
  enabledTools: string[];
  imageTarget: string;
  sidebarWidth: number;
}
export interface Workspace {
  version: 1;
  providers: Provider[];
  conversations: Conversation[];
  projects: Project[];
  images: ImageRecord[];
  settings: Settings;
}
export const defaults: Workspace = {
  version: 1,
  providers: [],
  conversations: [],
  projects: [],
  images: [],
  settings: {
    theme: "dark",
    temperature: 0.7,
    maxTokens: 4096,
    contextMessages: 30,
    systemPrompt: "",
    generativeUI: true,
    searchUrl: "",
    enabledTools: [...allTools],
    toolDefaultsVersion: 2,
    imageTarget: "",
    sidebarWidth: 248,
  },
};
export type StreamEvent =
  | { type: "image"; data: string; prompt: string }
  | { type: "reasoning"; text: string }
  | { type: "execution"; text: string }
  | {
      type: "codex_tool";
      callId: string;
      name: string;
      args: Record<string, unknown>;
    }
  | { type: "delta"; text: string }
  | { type: "tool"; name: string; args: Record<string, unknown> }
  | { type: "tool_progress"; index: number; name: string; arguments: string }
  | { type: "done"; cancelled?: boolean };
export const id = () => crypto.randomUUID();
