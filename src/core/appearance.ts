import { z } from "zod";
const color = z
  .union([z.literal(""), z.string().regex(/^#[\da-f]{6}$/i)])
  .default("");
export const appearanceSchema = z.object({
  background: color,
  surface: color,
  text: color,
  accent: color,
  userBubble: color,
  backgroundImage: z
    .string()
    .max(17 * 1024 * 1024)
    .refine(
      (s) =>
        !s ||
        /^data:image\/(png|jpeg|webp|avif|gif);base64,[A-Za-z0-9+/=]+$/.test(s),
    )
    .default(""),
  backgroundFit: z.enum(["cover", "contain"]).default("cover"),
  backgroundPosition: z.enum(["center", "top", "bottom"]).default("center"),
  backgroundOpacity: z.number().min(0).max(100).default(30),
  backgroundBlur: z.number().min(0).max(30).default(0),
  ambient: z.boolean().default(true),
  ambientColor: color,
  ambientIntensity: z.number().min(0).max(100).default(32),
  surfaceOpacity: z.number().min(45).max(100).default(100),
  radius: z.number().min(0).max(28).default(14),
  borderContrast: z.number().min(3).max(35).default(8),
  font: z.enum(["manrope", "inter", "system"]).default("manrope"),
  uiFontSize: z.number().min(12).max(16).default(13),
  messageFontSize: z.number().min(12).max(22).default(14),
  lineHeight: z.number().min(1.4).max(2.2).default(1.85),
  messageWidth: z.number().min(600).max(1200).default(860),
  density: z
    .enum(["compact", "comfortable", "spacious"])
    .default("comfortable"),
  bubble: z.boolean().default(true),
  actions: z.enum(["always", "hover"]).default("always"),
  reduceMotion: z.boolean().default(false),
  shadows: z.boolean().default(true),
});
export type Appearance = z.infer<typeof appearanceSchema>;
export const defaultAppearance = appearanceSchema.parse({});
export function normalizeAppearance(input: unknown): Appearance {
  const parsed = appearanceSchema.safeParse(input ?? {});
  return parsed.success ? parsed.data : { ...defaultAppearance };
}
export function applyAppearance(
  input: unknown,
  root = document.documentElement,
) {
  const a = normalizeAppearance(input);
  const light = root.dataset.theme === "light";
  const contrast = (hex: string) => {
    const rgb = [1, 3, 5]
      .map((i) => parseInt(hex.slice(i, i + 2), 16) / 255)
      .map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722 > 0.179
      ? "#161616"
      : "#ffffff";
  };
  for (const property of [
    "--bg",
    "--surface",
    "--raised",
    "--surface-hover",
    "--text",
    "--secondary",
    "--muted",
    "--accent",
    "--accent-text",
    "--font-sans",
    "--personal-background",
  ])
    root.style.removeProperty(property);
  if (a.background) root.style.setProperty("--bg", a.background);
  if (a.surface) {
    root.style.setProperty("--surface", a.surface);
    root.style.setProperty(
      "--raised",
      `color-mix(in srgb, ${a.surface}, ${light ? "black" : "white"} 8%)`,
    );
    root.style.setProperty(
      "--surface-hover",
      `color-mix(in srgb, ${a.surface}, ${light ? "black" : "white"} 14%)`,
    );
  }
  if (a.text) {
    root.style.setProperty("--text", a.text);
    root.style.setProperty(
      "--secondary",
      `color-mix(in srgb, ${a.text} 85%, var(--bg))`,
    );
    root.style.setProperty(
      "--muted",
      `color-mix(in srgb, ${a.text} 65%, var(--bg))`,
    );
  }
  if (a.accent) {
    root.style.setProperty("--accent", a.accent);
    root.style.setProperty("--accent-text", contrast(a.accent));
  }
  root.style.setProperty(
    "--font-sans",
    a.font === "inter"
      ? '"Inter Variable", sans-serif'
      : a.font === "system"
        ? "system-ui, sans-serif"
        : '"Manrope Variable", "Inter Variable", sans-serif',
  );
  const variables: Record<string, string> = {
    "--personal-ambient-color":
      a.ambientColor || (light ? "#d4a57a" : "#8a5b36"),
    "--personal-ambient-intensity": `${a.ambientIntensity}%`,
    "--personal-radius": `${a.radius}px`,
    "--personal-ui-size": `${a.uiFontSize}px`,
    "--personal-message-size": `${a.messageFontSize}px`,
    "--personal-line-height": `${a.lineHeight}`,
    "--personal-message-width": `${a.messageWidth}px`,
    "--personal-surface-opacity": `${a.surfaceOpacity}%`,
    "--personal-background-opacity": `${a.backgroundOpacity / 100}`,
    "--personal-background-blur": `${a.backgroundBlur}px`,
    "--personal-background-fit": a.backgroundFit,
    "--personal-background-position": a.backgroundPosition,
    "--personal-bubble": a.bubble
      ? a.userBubble || "var(--raised)"
      : "transparent",
    "--personal-gap": `${{ compact: 22, comfortable: 34, spacious: 48 }[a.density]}px`,
    "--line": `color-mix(in srgb, ${light ? "#231c18" : "#ffffff"} ${a.borderContrast}%, transparent)`,
  };
  for (const [name, value] of Object.entries(variables))
    root.style.setProperty(name, value);
  root.dataset.ambient = String(a.ambient);
  root.dataset.messageActions = a.actions;
  root.dataset.reduceMotion = String(a.reduceMotion);
  root.dataset.shadows = String(a.shadows);
}
