import { Wallpaper } from "./Wallpaper";
import { readWallpaper } from "../lib/wallpaper";
import { ColorPicker } from "./ColorPicker";
import { useRef } from "react";
import { ImagePlus, RotateCcw, X, Check } from "lucide-react";
import type { Settings } from "../types";
import {
  normalizeAppearance,
  defaultAppearance,
  type Appearance,
} from "../core/appearance";

export function AppearanceSettings({
  settings,
  onChange,
  onError,
}: {
  settings: Settings;
  onChange: (s: Settings) => void;
  onError: (e: unknown) => void;
}) {
  const a = normalizeAppearance(settings.appearance);
  const file = useRef<HTMLInputElement>(null);
  const light =
    settings.theme === "light" ||
    (settings.theme === "system" &&
      window.matchMedia("(prefers-color-scheme: light)").matches);
  const lightColors = {
    background: "#f4efea",
    surface: "#ffffff",
    accent: "#3c2d24",
    text: "#2a211c",
    userBubble: "#ece2d8",
  };
  const change = (patch: Partial<Appearance>) =>
    onChange({ ...settings, appearance: { ...a, ...patch } });
  const slider = (
    key: keyof Appearance,
    label: string,
    min: number,
    max: number,
    unit: string,
    step = 1,
  ) => (
    <label className="appearance-range">
      <span>
        {label}
        <output>
          {a[key]}
          {unit}
        </output>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={Number(a[key])}
        onChange={(e) => change({ [key]: Number(e.target.value) })}
      />
    </label>
  );
  const toggle = (
    key: "ambient" | "bubble" | "reduceMotion" | "shadows",
    label: string,
  ) => (
    <label className="appearance-toggle">
      <span>{label}</span>
      <input
        type="checkbox"
        role="switch"
        checked={a[key]}
        onChange={(e) => change({ [key]: e.target.checked })}
      />
    </label>
  );
  const colors = [
    ["background", "Fond de l’application", "#151210"],
    ["surface", "Panneaux et menus", "#29221e"],
    ["accent", "Couleur d’accent", "#eee9e4"],
    ["text", "Texte", "#f6f3f0"],
    ["userBubble", "Bulle utilisateur", "#352b25"],
  ] as const;
  const palettes = [
    {
      name: "Converge",
      background: "",
      surface: "",
      accent: "",
      text: "",
      theme: settings.theme,
      sample: "#352b25",
    },
    {
      name: "Graphite",
      background: "#131519",
      surface: "#22262d",
      accent: "#d8e0ec",
      text: "#f1f3f6",
      theme: "dark" as const,
      sample: "#22262d",
    },
    {
      name: "Océan",
      background: "#101b24",
      surface: "#1b303e",
      accent: "#9ed9e7",
      text: "#eaf4f6",
      theme: "dark" as const,
      sample: "#1b5063",
    },
    {
      name: "Prune",
      background: "#1d151e",
      surface: "#322335",
      accent: "#ddbadf",
      text: "#f5edf6",
      theme: "dark" as const,
      sample: "#64436b",
    },
    {
      name: "Papier",
      background: "#f6f2e9",
      surface: "#e9e2d6",
      accent: "#5e4934",
      text: "#30271f",
      theme: "light" as const,
      sample: "#e9e2d6",
    },
  ];
  return (
    <div className="appearance-options">
      <div
        className="appearance-preview"
        aria-label="Aperçu de votre apparence"
      >
        <Wallpaper source={a.backgroundImage} preview />
        <span className="appearance-preview-label">Aperçu en direct</span>
        <div className="appearance-preview-question">
          Un espace à votre image.
        </div>
        <p>Vos idées, vos modèles, votre façon de travailler.</p>
        <div className="appearance-preview-controls">
          <span>Converge</span>
          <button
            type="button"
            className="button primary"
            aria-label="Exemple de couleur d’accent"
            disabled
          >
            <Check size={14} /> Prêt
          </button>
        </div>
      </div>
      <div className="appearance-group">
        <h3>Palettes</h3>
        <div className="appearance-palettes">
          {palettes.map((p) => (
            <button
              type="button"
              key={p.name}
              aria-pressed={
                a.background === p.background &&
                a.surface === p.surface &&
                a.accent === p.accent &&
                a.text === p.text
              }
              onClick={() =>
                onChange({
                  ...settings,
                  theme: p.theme,
                  appearance: {
                    ...a,
                    background: p.background,
                    surface: p.surface,
                    accent: p.accent,
                    text: p.text,
                    userBubble: "",
                  },
                })
              }
            >
              <span style={{ background: p.sample }} />
              {p.name}
            </button>
          ))}
        </div>
      </div>
      <div className="appearance-group">
        <h3>Couleurs personnalisées</h3>
        <div className="appearance-colors">
          {colors.map(([key, label, fallback]) => (
            <div className="appearance-color" key={key}>
              <ColorPicker
                label={label}
                value={a[key] || (light ? lightColors[key] : fallback)}
                automatic={!a[key]}
                onChange={(value) => change({ [key]: value })}
              />
              <button
                type="button"
                className="icon-button"
                title={`Rétablir ${label.toLowerCase()}`}
                disabled={!a[key]}
                onClick={() => change({ [key]: "" })}
              >
                <RotateCcw size={13} />
              </button>
            </div>
          ))}
        </div>
      </div>
      <div className="appearance-group">
        <h3>Fond d’écran</h3>
        <p className="muted">
          Image locale · PNG, JPG, WebP, AVIF ou GIF animé · 12 Mo maximum
        </p>
        <div className="toolbar">
          <button
            type="button"
            className="button"
            onClick={() => file.current?.click()}
          >
            <ImagePlus size={16} />
            {a.backgroundImage ? "Changer l’image" : "Choisir une image"}
          </button>
          {a.backgroundImage && (
            <button
              type="button"
              className="button"
              onClick={() => change({ backgroundImage: "" })}
            >
              <X size={15} />
              Retirer
            </button>
          )}
        </div>
        <input
          ref={file}
          type="file"
          accept="image/png,image/jpeg,image/webp,image/avif,image/gif"
          hidden
          onChange={async (e) => {
            const image = e.target.files?.[0];
            e.target.value = "";
            if (!image) return;
            try {
              const data = await readWallpaper(image);
              change({ backgroundImage: data });
            } catch (error) {
              onError(error);
            }
          }}
        />
        {a.backgroundImage && (
          <>
            <div className="appearance-grid">
              <label className="field">
                Cadrage
                <select
                  value={a.backgroundFit}
                  onChange={(e) =>
                    change({
                      backgroundFit: e.target
                        .value as Appearance["backgroundFit"],
                    })
                  }
                >
                  <option value="cover">Remplir</option>
                  <option value="contain">Image entière</option>
                </select>
              </label>
              <label className="field">
                Position
                <select
                  value={a.backgroundPosition}
                  onChange={(e) =>
                    change({
                      backgroundPosition: e.target
                        .value as Appearance["backgroundPosition"],
                    })
                  }
                >
                  <option value="center">Centrée</option>
                  <option value="top">Haut</option>
                  <option value="bottom">Bas</option>
                </select>
              </label>
            </div>
            {slider("backgroundOpacity", "Visibilité de l’image", 0, 100, " %")}
            {slider("backgroundBlur", "Flou de l’image", 0, 30, " px")}
          </>
        )}
        {toggle("ambient", "Éclairage d’ambiance du fond")}
        {a.ambient && (
          <div className="appearance-ambient">
            <ColorPicker
              label="Teinte du halo supérieur"
              value={a.ambientColor || (light ? "#d4a57a" : "#8a5b36")}
              automatic={!a.ambientColor}
              onChange={(ambientColor) => change({ ambientColor })}
            />
            <button
              type="button"
              className="text-button"
              disabled={!a.ambientColor}
              onClick={() => change({ ambientColor: "" })}
            >
              Couleur du thème
            </button>
            {slider("ambientIntensity", "Intensité du halo", 0, 100, " %")}
          </div>
        )}
      </div>
      <div className="appearance-group">
        <h3>Surfaces et formes</h3>
        {slider("surfaceOpacity", "Opacité des panneaux", 45, 100, " %")}
        {slider("radius", "Arrondis", 0, 28, " px")}
        {slider("borderContrast", "Visibilité des bordures", 3, 35, " %")}
        {toggle("shadows", "Ombres des menus et fenêtres")}
        {toggle("bubble", "Fond des bulles utilisateur")}
      </div>
      <div className="appearance-group">
        <h3>Typographie</h3>
        <label className="field">
          Police de l’interface
          <select
            value={a.font}
            onChange={(e) =>
              change({ font: e.target.value as Appearance["font"] })
            }
          >
            <option value="manrope">Manrope</option>
            <option value="inter">Inter</option>
            <option value="system">Police du système</option>
          </select>
        </label>
        {slider("uiFontSize", "Taille de l’interface", 12, 16, " px")}
        {slider("messageFontSize", "Taille des messages", 12, 22, " px")}
        {slider("lineHeight", "Interligne des messages", 1.4, 2.2, "", 0.05)}
      </div>
      <div className="appearance-group">
        <h3>Disposition et confort</h3>
        {slider("messageWidth", "Largeur du fil", 600, 1200, " px", 20)}
        <label className="appearance-range">
          <span>
            Largeur de l’historique<output>{settings.sidebarWidth} px</output>
          </span>
          <input
            type="range"
            min={200}
            max={360}
            step={4}
            value={settings.sidebarWidth}
            onChange={(e) =>
              onChange({ ...settings, sidebarWidth: Number(e.target.value) })
            }
          />
        </label>
        <div className="appearance-grid">
          <label className="field">
            Espacement
            <select
              value={a.density}
              onChange={(e) =>
                change({ density: e.target.value as Appearance["density"] })
              }
            >
              <option value="compact">Compact</option>
              <option value="comfortable">Équilibré</option>
              <option value="spacious">Aéré</option>
            </select>
          </label>
          <label className="field">
            Actions des messages
            <select
              value={a.actions}
              onChange={(e) =>
                change({ actions: e.target.value as Appearance["actions"] })
              }
            >
              <option value="always">Toujours visibles</option>
              <option value="hover">Au survol ou au clavier</option>
            </select>
          </label>
        </div>
        {toggle("reduceMotion", "Réduire les animations")}
      </div>
      <div className="appearance-reset">
        <span>Repartir de l’apparence originale.</span>
        <button
          type="button"
          className="button"
          onClick={() =>
            onChange({
              ...settings,
              theme: "dark",
              sidebarWidth: 248,
              appearance: { ...defaultAppearance },
            })
          }
        >
          <RotateCcw size={14} />
          Tout réinitialiser
        </button>
      </div>
    </div>
  );
}
