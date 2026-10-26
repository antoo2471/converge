import {
  id,
  type Provider,
  type ProviderPluginId,
  type Protocol,
} from "../types";

export const accountProviders = [
  "antigravity",
  "claude-code",
  "grok",
  "minimax",
  "kimi",
] as const;
export const providerPresets = {
  antigravity: {
    name: "Antigravity",
    command: "agy",
    url: "https://antigravity.google",
    description: "Votre compte Google, via le client officiel Antigravity.",
    install: "https://www.antigravity.google/docs/install/",
    models: ["default"],
  },
  "claude-code": {
    name: "Claude Code",
    command: "claude",
    url: "https://claude.ai",
    description: "Votre abonnement Claude, via Claude Code officiel.",
    install: "https://code.claude.com/docs/en/overview",
    models: ["sonnet", "opus", "haiku"],
  },
  grok: {
    name: "Grok",
    command: "grok",
    url: "https://grok.com",
    description: "Votre compte Grok, via Grok Build officiel.",
    install: "https://github.com/xai-org/grok-build",
    models: ["default"],
  },
  minimax: {
    name: "MiniMax",
    command: "mmx",
    url: "https://platform.minimax.io",
    description: "Votre compte MiniMax, via le CLI officiel mmx.",
    install: "https://github.com/MiniMax-AI/cli",
    models: ["default"],
  },
  kimi: {
    name: "Kimi",
    command: "kimi",
    url: "https://kimi.com",
    description: "Votre compte Kimi Code, via son client officiel.",
    install: "https://moonshotai.github.io/kimi-cli/en/",
    models: ["default"],
  },
  zai: {
    name: "Z.ai · GLM",
    command: "",
    url: "https://api.z.ai/api/coding/paas/v4",
    description:
      "Connectez votre compte Z.ai à son Coding Plan, ou utilisez une clé dédiée.",
    install: "https://docs.z.ai/devpack/overview",
    models: [] as string[],
  },
  opencode: {
    name: "OpenCode Go / Zen",
    command: "",
    url: "https://opencode.ai/inference",
    description:
      "Votre clé OpenCode Go ou Zen. Choisissez le service correspondant à votre abonnement.",
    install: "https://opencode.ai/docs/go/",
    models: [] as string[],
  },
  deepseek: {
    name: "DeepSeek",
    command: "",
    url: "https://api.deepseek.com",
    description: "Connectez votre compte DeepSeek ou utilisez votre clé API.",
    install: "https://platform.deepseek.com/api_keys",
    models: [] as string[],
  },
} as const;
export type PresetId = keyof typeof providerPresets;
export const isPreset = (plugin: string): plugin is PresetId =>
  Object.hasOwn(providerPresets, plugin);
export function newPresetProvider(plugin: PresetId): Provider {
  const preset = providerPresets[plugin];
  const cli = accountProviders.includes(
    plugin as (typeof accountProviders)[number],
  );
  return {
    id: id(),
    name: preset.name,
    plugin: plugin as ProviderPluginId,
    protocol: (cli
      ? "cli"
      : plugin === "deepseek"
        ? "anthropic"
        : "openai") as Protocol,
    url: preset.url,
    headers: {},
    models: [...preset.models],
    enabled: true,
    capabilities: {
      vision: plugin === "claude-code",
      tools: true,
      images: false,
      context: plugin === "deepseek" ? 1_000_000 : 32768,
    },
    ...(cli ? { request: { cli: {} } } : {}),
    ...(plugin === "zai"
      ? { request: { zai: { authMode: "oauth" as const } } }
      : {}),
    ...(plugin === "deepseek"
      ? {
          request: {
            deepseek: { authMode: "oauth" as const },
            chatPath: "anthropic/v1/messages",
          },
        }
      : {}),
  };
}
