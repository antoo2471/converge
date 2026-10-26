import {
  FolderOpen,
  Terminal,
  Globe,
  MousePointer2,
  ShieldCheck,
  Puzzle,
} from "lucide-react";
import type { Settings } from "../types";
import { allTools, toolGroups, setTools } from "../core/toolPreferences";

export function ToolSettings({
  settings,
  onChange,
  onError,
}: {
  settings: Settings;
  onChange: (s: Settings) => void;
  onError: (e: unknown) => void;
}) {
  const icons = {
    files: FolderOpen,
    terminal: Terminal,
    web: Globe,
    computer: MousePointer2,
    extensions: Puzzle,
  };
  const update = (enabledTools: string[]) =>
    onChange({ ...settings, toolDefaultsVersion: 3, enabledTools });
  return (
    <div className="tools-simple">
      <div className="tools-safety-note">
        <ShieldCheck size={18} />
        <p>
          Vous gardez la main : les actions demandent votre accord avant leur
          exécution.
        </p>
      </div>
      <div className="tools-bulk">
        <span>Choisissez ce que l’IA peut utiliser.</span>
        <button
          type="button"
          className="text-button"
          onClick={() => update([...allTools])}
          disabled={allTools.every((name) =>
            settings.enabledTools.includes(name),
          )}
        >
          Tout activer
        </button>
        <button
          type="button"
          className="text-button"
          onClick={() => update([])}
          disabled={!settings.enabledTools.length}
        >
          Tout désactiver
        </button>
      </div>
      <div className="tools-groups">
        {toolGroups.map((group) => {
          const Icon = icons[group.id];
          const names = group.tools.map(([name]) => name);
          const count = names.filter((name) =>
            settings.enabledTools.includes(name),
          ).length;
          const status = !count
            ? "Désactivé"
            : count < names.length
              ? "Accès personnalisés"
              : "Activé";
          return (
            <div className="tools-group" key={group.id}>
              <label className="tools-group-row">
                <Icon size={20} />
                <span>
                  <strong>{group.label}</strong>
                  <small>{group.description}</small>
                  <em>{status}</em>
                </span>
                <input
                  type="checkbox"
                  aria-label={group.label}
                  ref={(input) => {
                    if (input)
                      input.indeterminate = count > 0 && count < names.length;
                  }}
                  checked={count === names.length}
                  onChange={(e) =>
                    update(
                      setTools(settings.enabledTools, names, e.target.checked),
                    )
                  }
                />
              </label>
              {(group.id === "files" || group.id === "terminal") &&
                count > 0 && (
                  <p className="tools-workspace-hint">
                    Les espaces de travail se choisissent dans le menu + de
                    chaque conversation.
                  </p>
                )}
              {group.id === "web" && count > 0 && (
                <details className="tools-search-config">
                  <summary>
                    {settings.searchUrl
                      ? "Modifier le moteur de recherche"
                      : "Ajouter un moteur personnel (facultatif)"}
                  </summary>
                  <label className="field">
                    Adresse de votre moteur SearXNG
                    <input
                      type="url"
                      placeholder="https://votre-moteur.example"
                      value={settings.searchUrl}
                      onChange={(e) =>
                        onChange({ ...settings, searchUrl: e.target.value })
                      }
                    />
                    <small>
                      La recherche intégrée fonctionne sans configuration.
                      Vous pouvez utiliser votre instance SearXNG avec JSON activé ;
                      les blocages déclenchent un repli vers le moteur intégré.
                    </small>
                  </label>
                </details>
              )}
              {group.tools.length > 1 && (
                <details className="tools-fine-tuning">
                  <summary>Choisir les actions autorisées</summary>
                  {group.tools.map(([name, label]) => (
                    <label className="check" key={name}>
                      <input
                        type="checkbox"
                        checked={settings.enabledTools.includes(name)}
                        onChange={(e) =>
                          update(
                            setTools(
                              settings.enabledTools,
                              [name],
                              e.target.checked,
                            ),
                          )
                        }
                      />
                      {label}
                    </label>
                  ))}
                </details>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
