import type { Settings } from "../types";

export const responsePresets = [
  {
    id: "concise",
    label: "Concis",
    description: "L’essentiel, avec peu de développements.",
    temperature: 0.3,
    maxTokens: 2048,
    contextMessages: 20,
    instruction:
      "Prefer brief, direct answers. Include details needed to answer correctly; expand when the user explicitly requests it.",
  },
  {
    id: "balanced",
    label: "Équilibré",
    description: "Des réponses claires, avec le détail utile au quotidien.",
    temperature: 0.7,
    maxTokens: 4096,
    contextMessages: 30,
    instruction:
      "Adapt the level of detail to the task. Give a clear answer, with explanations and examples when useful.",
  },
  {
    id: "thorough",
    label: "Approfondi",
    description:
      "Davantage d’explications, d’exemples et de suivi de la conversation.",
    temperature: 0.4,
    maxTokens: 8192,
    contextMessages: 60,
    instruction:
      "Prefer thorough, well-structured explanations with useful examples and relevant context. Still answer simple questions simply, and respect requests for brevity.",
  },
  {
    id: "creative",
    label: "Créatif",
    description: "Pour explorer des idées, écrire et varier les propositions.",
    temperature: 1,
    maxTokens: 4096,
    contextMessages: 30,
    instruction:
      "For creative tasks, explore varied and original ideas. Keep factual claims accurate and distinguish imaginative suggestions from facts.",
  },
] as const;
export type ResponsePreset = (typeof responsePresets)[number]["id"];
export function selectedResponsePreset(settings: Settings) {
  return responsePresets.find(
    (p) =>
      p.temperature === settings.temperature &&
      p.maxTokens === settings.maxTokens &&
      p.contextMessages === settings.contextMessages &&
      (!settings.responsePreset || p.id === settings.responsePreset),
  );
}
export function applyResponsePreset(
  settings: Settings,
  preset: ResponsePreset,
): Settings {
  const p = responsePresets.find((p) => p.id === preset)!;
  return {
    ...settings,
    responsePreset: p.id,
    temperature: p.temperature,
    maxTokens: p.maxTokens,
    contextMessages: p.contextMessages,
  };
}
export function responsePresetInstruction(settings: Settings): string {
  const preset = selectedResponsePreset(settings);
  return preset
    ? `Response preference (the user's explicit request takes priority): ${preset.instruction}`
    : "";
}
