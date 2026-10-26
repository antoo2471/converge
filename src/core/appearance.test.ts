// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import {
  applyAppearance,
  defaultAppearance,
  normalizeAppearance,
  appearanceSchema,
} from "./appearance";
afterEach(() => {
  document.documentElement.removeAttribute("style");
  for (const key of [
    "theme",
    "ambient",
    "messageActions",
    "reduceMotion",
    "shadows",
  ])
    delete document.documentElement.dataset[key];
});
it("migrates existing settings and survives a persisted JSON round trip", () => {
  expect(normalizeAppearance(undefined)).toEqual(defaultAppearance);
  const saved = normalizeAppearance({
    accent: "#77cc99",
    radius: 4,
    messageWidth: 1040,
    actions: "hover",
  });
  expect(normalizeAppearance(JSON.parse(JSON.stringify(saved)))).toEqual(saved);
});
it("persists animated GIF backgrounds and applies the ambient color and intensity", () => {
  const backgroundImage =
    "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
  const a = normalizeAppearance({
    backgroundImage,
    ambientColor: "#2244ff",
    ambientIntensity: 65,
  });
  expect(a.backgroundImage).toBe(backgroundImage);
  applyAppearance(a);
  expect(
    document.documentElement.style.getPropertyValue("--personal-ambient-color"),
  ).toBe("#2244ff");
  expect(
    document.documentElement.style.getPropertyValue(
      "--personal-ambient-intensity",
    ),
  ).toBe("65%");
});
it("applies customization and removes custom colors on reset", () => {
  const root = document.documentElement;
  applyAppearance({
    background: "#223344",
    accent: "#ffffff",
    radius: 3,
    messageFontSize: 19,
    reduceMotion: true,
    shadows: false,
  });
  expect(root.style.getPropertyValue("--bg")).toBe("#223344");
  expect(root.style.getPropertyValue("--accent-text")).toBe("#161616");
  expect(root.style.getPropertyValue("--personal-radius")).toBe("3px");
  expect(root.dataset.reduceMotion).toBe("true");
  applyAppearance(defaultAppearance);
  expect(root.style.getPropertyValue("--bg")).toBe("");
  expect(root.style.getPropertyValue("--accent-text")).toBe("");
  expect(root.dataset.shadows).toBe("true");
});
it("uses the theme for borders and chooses readable accent foregrounds", () => {
  const root = document.documentElement;
  root.dataset.theme = "light";
  applyAppearance({ accent: "#000000" });
  expect(root.style.getPropertyValue("--accent-text")).toBe("#ffffff");
  expect(root.style.getPropertyValue("--line")).toContain("#231c18");
});
it("rejects remote or executable wallpapers, CSS injection and out of bounds options", () => {
  for (const input of [
    { backgroundImage: "https://example.com/track.png" },
    { backgroundImage: "data:image/svg+xml;base64,PHN2Zz4=" },
    { background: "red;display:none" },
    { radius: -1 },
    { surfaceOpacity: 0 },
  ]) {
    expect(appearanceSchema.safeParse(input).success).toBe(false);
    expect(normalizeAppearance(input)).toEqual(defaultAppearance);
  }
});
