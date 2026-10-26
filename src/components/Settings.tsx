import { UpdateSettings } from "./AppUpdates";
import appManifest from "../../package.json";
import { ToolSettings } from "./ToolSettings";
import { ResponseSettings } from "./ResponseSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { useState } from "react";
import {
  Shield,
  Download,
  Upload,
  Sun,
  Moon,
  Monitor,
  Search,
  X,
  SlidersHorizontal,
  Database,
} from "lucide-react";
import type { Workspace, Settings as Config } from "../types";
import { importedWorkspace } from "../core/workspace";
export function Settings({
  settings,
  onChange,
  onExport,
  onImport,
  onError,
  onClose,
  busy = false,
}: {
  settings: Config;
  onChange: (s: Config) => void;
  onExport: () => void;
  onImport: () => void;
  onError: (e: unknown) => void;
  onClose?: () => void;
  busy?: boolean;
}) {
  const [section, setSection] = useState("appearance"),
    [query, setQuery] = useState("");
  const sections = [
    {
      id: "updates",
      label: "Mises à jour",
      group: "Application",
      icon: Download,
    },
    { id: "appearance", label: "Apparence", group: "Interface", icon: Sun },
    {
      id: "generation",
      label: "Réponses de l’IA",
      group: "Intelligence",
      icon: SlidersHorizontal,
    },
    {
      id: "tools",
      label: "Outils et permissions",
      group: "Intelligence",
      icon: Shield,
    },
    {
      id: "data",
      label: "Données et stockage",
      group: "Application",
      icon: Database,
    },
  ];
  const change = (values: Partial<Config>) =>
    onChange({ ...settings, ...values });
  return (
    <div className="settings-layout">
      <aside className="settings-nav">
        <label className="settings-search">
          <Search size={16} />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Rechercher un paramètre…"
            aria-label="Rechercher un paramètre"
          />
        </label>
        {["Interface", "Intelligence", "Application"].map((group) => (
          <div className="settings-nav-group" key={group}>
            <span>{group}</span>
            {sections
              .filter(
                (s) =>
                  s.group === group &&
                  s.label.toLowerCase().includes(query.toLowerCase()),
              )
              .map((s) => (
                <button
                  key={s.id}
                  className={section === s.id ? "active" : ""}
                  onClick={() => setSection(s.id)}
                >
                  <s.icon size={18} />
                  {s.label}
                </button>
              ))}
          </div>
        ))}
        <div className="settings-profile">
          <span>C</span>
          <div>
            <strong>Espace personnel</strong>
            <small>Stockage local</small>
          </div>
        </div>
      </aside>
      <div className="page settings-page">
        {onClose && (
          <button
            className="icon-button settings-close"
            title="Fermer les paramètres"
            onClick={onClose}
          >
            <X size={20} />
          </button>
        )}
        <div className="page-heading">
          <div>
            <h1>{sections.find((s) => s.id === section)?.label}</h1>
            <p>
              {
                {
                  updates: "Gardez Converge à jour depuis le dépôt officiel.",
                  appearance:
                    "Faites de Converge un espace qui vous ressemble.",
                  generation:
                    "Ajustez la façon dont vos modèles échangent avec vous.",
                  tools: "Choisissez ce que vos modèles peuvent faire.",
                  data: "Gardez la maîtrise de votre espace de travail.",
                }[section]
              }
            </p>
          </div>
        </div>
        <section className="settings-section" hidden={section !== "appearance"}>
          <h2>Apparence</h2>
          <div className="theme-options">
            {(
              [
                { value: "dark", label: "Sombre", icon: Moon },
                { value: "light", label: "Clair", icon: Sun },
                { value: "system", label: "Système", icon: Monitor },
              ] as const
            ).map(({ value, label, icon: Icon }) => (
              <button
                key={value}
                className={`theme-option ${settings.theme === value ? "active" : ""}`}
                onClick={() => change({ theme: value })}
              >
                <Icon size={21} />
                {label}
              </button>
            ))}
          </div>
          <AppearanceSettings
            settings={settings}
            onChange={onChange}
            onError={onError}
          />
        </section>
        <section className="settings-section" hidden={section !== "generation"}>
          <ResponseSettings settings={settings} onChange={onChange} />
        </section>
        <section className="settings-section" hidden={section !== "tools"}>
          <ToolSettings
            settings={settings}
            onChange={onChange}
            onError={onError}
          />
        </section>
        <section className="settings-section" hidden={section !== "data"}>
          <h2>Données locales</h2>
          <p className="muted">
            Conversations, projets et images dans SQLite. Les exports ne
            contiennent aucune clé API ni header secret.
          </p>
          <div className="toolbar">
            <button className="button" onClick={onExport}>
              <Download size={16} />
              Exporter l’espace
            </button>
            <button className="button" onClick={onImport}>
              <Upload size={16} />
              Importer
            </button>
          </div>
        </section>
        {section === "updates" && <UpdateSettings busy={busy} />}
        <footer className="settings-footer">
          Converge {appManifest.version} · Every AI. One space.
        </footer>
      </div>
    </div>
  );
}
export function normalizeWorkspace(input: unknown) {
  return importedWorkspace.parse(input);
}
