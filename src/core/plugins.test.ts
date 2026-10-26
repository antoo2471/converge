import { it, expect } from "vitest";
import { providerRegistry, modelRegistry } from "./plugins";
import { defaults } from "../types";
it("ships authorized plugins and starts without models", () => {
  expect(providerRegistry.list().map((p) => p.manifest.id)).toEqual([
    "antigravity",
    "claude-code",
    "grok",
    "minimax",
    "kimi",
    "zai",
    "opencode",
    "chatgpt",
    "local",
    "custom",
  ]);
  expect(modelRegistry(defaults.providers)).toEqual([]);
});
