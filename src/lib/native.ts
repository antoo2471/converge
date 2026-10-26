import { channelCommand } from "./channelCommand";
import { invoke, Channel, isTauri, convertFileSrc } from "@tauri-apps/api/core";
import type {
  Workspace,
  Provider,
  Message,
  StreamEvent,
  Attachment,
} from "../types";
export const native = isTauri();
export interface ZaiAuthStatus {
  state: "disconnected" | "pending" | "connected";
  loginId?: string;
  authUrl?: string;
}
export const zaiAuthAction = (
  provider: Provider,
  action: "status" | "start" | "poll" | "cancel" | "logout",
  loginId?: string,
) =>
  invoke<ZaiAuthStatus>("zai_auth_action", {
    provider,
    action,
    loginId: loginId || null,
  });
export const deepseekAuthAction = (
  provider: Provider,
  action: "status" | "start" | "poll" | "cancel" | "logout",
  loginId?: string,
) =>
  invoke<ZaiAuthStatus>("deepseek_auth_action", {
    provider,
    action,
    loginId: loginId || null,
  });
export interface CliProviderStatus {
  available: boolean;
  authenticated?: boolean;
  login?: "idle" | "running" | "completed" | "failed" | "cancelled";
  message?: string;
}
export const cliProviderAction = (
  provider: Provider,
  action: "status" | "login" | "logout" | "cancelLogin",
) => invoke<CliProviderStatus>("cli_provider_action", { provider, action });
export const pasteClipboardImage = () =>
  invoke<Attachment>("paste_clipboard_image");
let artifactRuntimeRegistered = false;
export async function registerArtifact(
  code: string,
  kind: string,
  runtime: string,
) {
  const path = await invoke<string>("register_artifact", {
    code,
    kind,
    runtime: artifactRuntimeRegistered ? null : runtime,
  });
  artifactRuntimeRegistered = true;
  return { path, url: convertFileSrc(path, "artifact") };
}
export const releaseArtifact = (path: string) =>
  invoke("release_artifact", { path });
export async function loadWorkspace(): Promise<Workspace | null> {
  return native ? invoke("load_workspace") : null;
}
export async function saveWorkspace(workspace: Workspace) {
  if (native) await invoke("save_workspace", { workspace });
}
export async function saveProvider(provider: Provider, key?: string) {
  await invoke("save_provider", { provider, key: key || null });
}
export async function removeProvider(providerId: string) {
  await invoke("remove_provider", { providerId });
}
export async function discover(
  provider: Provider,
  key?: string,
): Promise<string[]> {
  return invoke("discover_models", { provider, key: key || null });
}
export async function streamChat(
  requestId: string,
  providerId: string,
  model: string,
  messages: Message[],
  settings: Workspace["settings"],
  onEvent: (event: StreamEvent) => void,
  conversationId?: string,
) {
  return channelCommand<void, StreamEvent>(
    (deliver) => {
      const channel = new Channel<StreamEvent>();
      channel.onmessage = deliver;
      return invoke("stream_chat", {
        request: {
          id: requestId,
          providerId,
          model,
          messages,
          temperature: settings.temperature,
          maxTokens: settings.maxTokens,
          reasoningEffort:
            settings.reasoningEfforts?.[`${providerId}::${model}`] || "auto",
          toolsEnabled: settings.enabledTools.length > 0,
          conversationId: conversationId || null,
        },
        onEvent: channel,
      });
    },
    (event) => event.type === "done",
    onEvent,
  );
}

export const cancel = (requestId: string) =>
  invoke("cancel_request", { requestId });
export const readAttachment = (path: string): Promise<Attachment> =>
  invoke("read_attachment", { path });
export const exportFile = (path: string, content: string) =>
  invoke("export_file", { path, content });
export const importFile = (path: string): Promise<string> =>
  invoke("import_file", { path });
export const generateImage = (
  providerId: string,
  model: string,
  prompt: string,
  size: string,
  image?: string,
  requestId?: string,
): Promise<string[]> =>
  invoke("generate_image", {
    providerId,
    model,
    prompt,
    size,
    image: image || null,
    requestId: requestId || null,
  });
export const searchConversations = (query: string) =>
  invoke<{ id: string; title: string; snippet: string }[]>(
    "search_conversations",
    { query },
  );
export const openExternal = (url: string) => invoke("open_external", { url });
export interface ChatGPTStatus {
  account?: {
    type: "chatgpt";
    email: string | null;
    plan: string | null;
  } | null;
  models?: NonNullable<NonNullable<Provider["request"]>["chatgpt"]>["models"];
  capabilities?: Provider["capabilities"];
  limits?: {
    rateLimits?: {
      primary?: {
        usedPercent?: number;
        resetsAt?: number;
        windowDurationMins?: number;
      };
      secondary?: {
        usedPercent?: number;
        resetsAt?: number;
        windowDurationMins?: number;
      };
    };
  } | null;
  loginId?: string;
  authUrl?: string;
  transport?: string;
}
export const chatgptAction = (
  provider: Provider,
  action: "status" | "diagnose" | "login" | "cancelLogin" | "logout",
  loginId?: string,
) =>
  invoke<ChatGPTStatus>("chatgpt_action", {
    provider,
    action,
    loginId: loginId || null,
  });
export const resolveCodexTool = (
  callId: string,
  result: string,
  success: boolean,
) => invoke("resolve_codex_tool", { callId, result, success });
export async function agentTool(
  conversationId: string,
  requestId: string,
  name: string,
  args: Record<string, unknown>,
  onPermission: (permission: import("../types").ToolPermission) => void,
  onOutput: (event: {
    type: string;
    text?: string;
    added?: number;
    removed?: number;
    diff?: string;
    written?: number;
    total?: number;
  }) => void,
): Promise<string> {
  const permission = new Channel<import("../types").ToolPermission>();
  permission.onmessage = onPermission;
  type Output = Parameters<typeof onOutput>[0];
  return channelCommand<string, Output>(
    (deliver) => {
      const output = new Channel<Output>();
      output.onmessage = deliver;
      return invoke("execute_agent_tool", {
        conversationId,
        requestId,
        name,
        args,
        onPermission: permission,
        onOutput: output,
      });
    },
    (event) => event.type === "finished",
    onOutput,
  );
}

export const resolveToolPermission = (
  requestId: string,
  decision: "allow" | "deny" | "conversation",
  expectedRoot: string | null,
) => invoke("resolve_tool_permission", { requestId, decision, expectedRoot });
export const revokeConversationPermissions = (conversationId: string) =>
  invoke("revoke_conversation_permissions", { conversationId });

export interface WorkspaceContext {
  folders: string[];
  desktop?: string;
  home?: string;
}
export const workspaceContext = (conversationId: string) =>
  invoke<WorkspaceContext>("conversation_workspace_context", {
    conversationId,
  });
export const pickWorkspace = (conversationId: string) =>
  invoke<{ folders: string[]; selected: string | null }>(
    "choose_conversation_workspace",
    { conversationId },
  );
export const chooseWorkspace = async (conversationId: string) =>
  (await pickWorkspace(conversationId)).folders;
export const removeWorkspace = (conversationId: string, path: string) =>
  invoke<string[]>("remove_conversation_workspace", { conversationId, path });
