import { expect, it } from "vitest";
import { defaults } from "../types";
import {
  applyResponsePreset,
  responsePresetInstruction,
  selectedResponsePreset,
} from "./responsePresets";
it("recognizes existing defaults without replacing customized legacy settings", () => {
  expect(selectedResponsePreset(defaults.settings)?.id).toBe("balanced");
  const legacy = { ...defaults.settings, maxTokens: 1234 };
  expect(selectedResponsePreset(legacy)).toBeUndefined();
  expect(legacy.maxTokens).toBe(1234);
});
it("changes response behavior while preserving personal instructions and permissions", () => {
  const settings = {
    ...defaults.settings,
    systemPrompt: "Réponds en français",
    enabledTools: ["filesystem.read"],
    generativeUI: false,
  };
  const next = applyResponsePreset(settings, "thorough");
  expect(next.maxTokens).toBeGreaterThan(settings.maxTokens);
  expect(next.contextMessages).toBeGreaterThan(settings.contextMessages);
  expect(next.systemPrompt).toBe(settings.systemPrompt);
  expect(next.enabledTools).toEqual(settings.enabledTools);
  expect(next.generativeUI).toBe(false);
  expect(selectedResponsePreset(JSON.parse(JSON.stringify(next)))?.id).toBe(
    "thorough",
  );
  expect(responsePresetInstruction(next)).toContain(
    "explicit request takes priority",
  );
});
