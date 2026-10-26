import type { Provider } from "../types";
export const reasoningLevels = [
  "auto",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
] as const;
export type ReasoningEffort =
  (typeof reasoningLevels)[number] | "none" | "minimal" | "ultra";
export const effortLabels: Record<ReasoningEffort, string> = {
  auto: "Auto",
  none: "Sans",
  minimal: "Minimal",
  low: "Faible",
  medium: "Moyen",
  high: "Élevé",
  xhigh: "Très élevé",
  max: "Maximum",
  ultra: "Ultra",
};
export function effortLevels(
  provider: Provider | undefined,
  model: string,
): readonly ReasoningEffort[] {
  if (!provider || provider.protocol === "comfyui") return [];
  if (provider.plugin === "deepseek") return ["auto", "low", "high", "max"];
  if (provider.protocol === "cli")
    return ["claude-code", "grok", "antigravity"].includes(provider.plugin)
      ? reasoningLevels
      : ["auto"];
  if (provider.plugin === "chatgpt") {
    const modelId = model.replace(/^codex\//, "");
    const discovered =
      provider.request?.chatgpt?.models?.find((m) => m.id === modelId)
        ?.efforts || [];
    const levels: ReasoningEffort[] = [
      "none",
      "minimal",
      "low",
      "medium",
      "high",
      "xhigh",
      "max",
      "ultra",
    ];
    return ["auto", ...levels.filter((e) => discovered.includes(e))];
  }
  if (provider.protocol === "ollama")
    return /gpt-oss/i.test(model) ? ["auto", "low", "medium", "high"] : [];
  return reasoningLevels;
}
