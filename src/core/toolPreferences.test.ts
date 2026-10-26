import { expect, it } from "vitest";
import { defaults } from "../types";
import {
  allTools,
  migrateToolDefaults,
  setTools,
  toolGroups,
} from "./toolPreferences";
it("enables all existing tools once, preserving subsequent user opt-outs", () => {
  const old = {
    ...defaults.settings,
    toolDefaultsVersion: undefined,
    enabledTools: ["filesystem.read"],
  };
  const migrated = migrateToolDefaults(old);
  expect(migrated.enabledTools).toEqual(allTools);
  expect(allTools).toHaveLength(22);
  expect(new Set(allTools).size).toBe(allTools.length);
  const disabled = { ...migrated, enabledTools: [] };
  expect(
    migrateToolDefaults(JSON.parse(JSON.stringify(disabled))).enabledTools,
  ).toEqual([]);
});
it("changes a category without altering other permissions or duplicating actions", () => {
  const files = toolGroups[0].tools.map(([name]) => name);
  const changed = setTools(allTools, files, false);
  expect(changed).not.toContain("filesystem.write");
  expect(changed).toContain("computer.capture");
  expect(changed).toContain("terminal.execute");
  const restored = setTools(changed, files, true);
  expect(new Set(restored)).toEqual(new Set(allTools));
  expect(setTools(restored, files, true)).toHaveLength(allTools.length);
});

it("only enables grouped computer actions when all previous computer actions were enabled", () => {
 const prior = allTools.filter(n => n !== "computer.actions");
 expect(migrateToolDefaults({...defaults.settings,toolDefaultsVersion:1,enabledTools:prior}).enabledTools).toContain("computer.actions");
 expect(migrateToolDefaults({...defaults.settings,toolDefaultsVersion:1,enabledTools:prior.filter(n => n !== "computer.click")}).enabledTools).not.toContain("computer.actions");
});
