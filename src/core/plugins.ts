import type { Protocol, Capabilities, Provider } from "../types";
import {
  accountProviders,
  providerPresets,
  newPresetProvider,
} from "./providerPresets";
export interface PluginMode {
  id: string;
  name: string;
  protocols: Protocol[];
  authentication: ("none" | "api-key" | "headers" | "oauth")[];
  capabilities: (keyof Capabilities)[];
}
export interface PluginManifest {
  catalogue?: {
    category: "accounts" | "local" | "api";
    summary: string;
    features: string[];
  };
  schemaVersion: 1;
  id: string;
  name: string;
  version: string;
  modes: PluginMode[];
  description: string;
}
export interface ProviderPlugin {
  manifest: PluginManifest;
  defaults(modeId: string): Partial<Provider>;
  supports(protocol: Protocol): boolean;
}
/** Registry owns integration identity. Models and conversations reference IDs, never plugin instances. */
export class ProviderRegistry {
  private plugins = new Map<string, ProviderPlugin>();
  register(plugin: ProviderPlugin) {
    if (this.plugins.has(plugin.manifest.id))
      throw Error("Plugin déjà enregistré");
    this.plugins.set(plugin.manifest.id, Object.freeze(plugin));
  }
  get(id: string) {
    return this.plugins.get(id);
  }
  list() {
    return [...this.plugins.values()];
  }
}
export const providerRegistry = new ProviderRegistry();
for (const [id, preset] of Object.entries(providerPresets)) {
  const cli = accountProviders.includes(
    id as (typeof accountProviders)[number],
  );
  providerRegistry.register({
    manifest: {
      schemaVersion: 1,
      id,
      name: preset.name,
      version: "1.0.0",
      description: preset.description,
      catalogue: {
        category: cli || id === "zai" || id === "deepseek" ? "accounts" : "api",
        summary: preset.description,
        features: cli
          ? ["Connexion OAuth", "Client officiel", "Outils Converge"]
          : id === "zai"
            ? ["Connexion OAuth", "Coding Plan", "Clé API"]
            : id === "deepseek"
              ? ["Connexion de compte", "Clé API", "Streaming"]
              : ["Clé API", "Go / Zen", "Streaming"],
      },
      modes: [
        {
          id: cli ? "account" : "api",
          name: cli ? "Compte" : "API",
          protocols: cli
            ? ["cli"]
            : id === "deepseek"
              ? ["openai", "anthropic"]
              : ["openai", "anthropic", "responses"],
          authentication: cli
            ? ["oauth"]
            : id === "zai" || id === "deepseek"
              ? ["oauth", "api-key"]
              : ["api-key"],
          capabilities: ["vision", "tools", "context"],
        },
      ],
    },
    defaults: () => newPresetProvider(id as keyof typeof providerPresets),
    supports: (p) =>
      cli
        ? p === "cli"
        : (id === "deepseek"
            ? ["openai", "anthropic"]
            : ["openai", "anthropic", "responses"]
          ).includes(p),
  });
}
providerRegistry.register({
  manifest: {
    schemaVersion: 1,
    id: "chatgpt",
    catalogue: {
      category: "accounts",
      summary:
        "Retrouvez Codex dans votre espace Converge, avec votre compte ChatGPT.",
      features: ["Compte ChatGPT", "Outils Converge", "Raisonnement"],
    },
    name: "ChatGPT",
    version: "1.0.0",
    description: "ChatGPT avec votre abonnement, via Codex.",
    modes: [
      {
        id: "codex",
        name: "Codex",
        protocols: ["codex"],
        authentication: ["oauth"],
        capabilities: ["vision", "tools", "context"],
      },
    ],
  },
  defaults: () => ({
    plugin: "chatgpt",
    protocol: "codex",
    url: "https://chatgpt.com",
    enabled: true,
    models: [],
    headers: {},
    capabilities: { vision: false, tools: true, images: false, context: 32768 },
  }),
  supports: (p) => p === "codex",
});
providerRegistry.register({
  manifest: {
    schemaVersion: 1,
    id: "local",
    catalogue: {
      category: "local",
      summary:
        "Connectez vos modèles locaux avec Ollama ou un serveur compatible.",
      features: ["Sur votre ordinateur", "Streaming", "Sans abonnement"],
    },
    name: "Local AI",
    version: "1.0.0",
    description: "Serveurs de modèles et moteurs d’images locaux",
    modes: [
      {
        id: "chat",
        name: "Conversation",
        protocols: ["ollama", "openai"],
        authentication: ["none", "api-key", "headers"],
        capabilities: ["vision", "tools", "context"],
      },
      {
        id: "images",
        name: "Création d’images",
        protocols: ["comfyui", "openai"],
        authentication: ["none", "api-key", "headers"],
        capabilities: ["images"],
      },
    ],
  },
  defaults: () => ({ plugin: "local", enabled: true }),
  supports: (p) => ["ollama", "openai", "comfyui"].includes(p),
});
providerRegistry.register({
  manifest: {
    schemaVersion: 1,
    id: "custom",
    catalogue: {
      category: "api",
      summary:
        "Reliez une API à votre mesure : endpoint, modèles et authentification.",
      features: [
        "OpenAI compatible",
        "Anthropic compatible",
        "Configuration libre",
      ],
    },
    name: "Custom API",
    version: "1.0.0",
    description: "Endpoints compatibles configurables",
    modes: [
      {
        id: "chat",
        name: "Conversation",
        protocols: ["openai", "anthropic", "ollama"],
        authentication: ["none", "api-key", "headers"],
        capabilities: ["vision", "tools", "context"],
      },
      {
        id: "images",
        name: "Création d’images",
        protocols: ["openai", "comfyui"],
        authentication: ["none", "api-key", "headers"],
        capabilities: ["images"],
      },
    ],
  },
  defaults: () => ({ plugin: "custom", enabled: true }),
  supports: (p) => ["openai", "anthropic", "ollama", "comfyui"].includes(p),
});
export interface RegisteredModel {
  id: string;
  providerId: string;
  capabilities: Capabilities;
  available: boolean;
}
export function modelRegistry(providers: Provider[]): RegisteredModel[] {
  return providers.flatMap((p) =>
    p.models.map((id) => ({
      id,
      providerId: p.id,
      capabilities:
        p.plugin === "chatgpt"
          ? {
              ...p.capabilities,
              vision:
                p.request?.chatgpt?.models?.find((m) => `codex/${m.id}` === id)
                  ?.vision ?? false,
              images: false,
            }
          : p.capabilities,
      available: p.enabled,
    })),
  );
}
