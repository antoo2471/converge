import { AlignLeft, Scale, BookOpen, Sparkles } from "lucide-react";
import {
  responsePresets,
  selectedResponsePreset,
  applyResponsePreset,
} from "../core/responsePresets";
import type { Settings } from "../types";

export function ResponseSettings({
  settings,
  onChange,
}: {
  settings: Settings;
  onChange: (settings: Settings) => void;
}) {
  const selected = selectedResponsePreset(settings);
  const icons = {
    concise: AlignLeft,
    balanced: Scale,
    thorough: BookOpen,
    creative: Sparkles,
  };
  return (
    <>
      <fieldset className="response-presets">
        <legend>Quel type de réponse préférez-vous ?</legend>
        {responsePresets.map((preset) => {
          const Icon = icons[preset.id];
          return (
            <label className="response-preset" key={preset.id}>
              <input
                type="radio"
                name="response-preset"
                value={preset.id}
                checked={selected?.id === preset.id}
                onChange={() =>
                  onChange(applyResponsePreset(settings, preset.id))
                }
              />
              <Icon size={19} />
              <span>
                <strong>{preset.label}</strong>
                <small>{preset.description}</small>
              </span>
            </label>
          );
        })}
      </fieldset>
      <p className="response-preset-note">
        S’applique aux prochaines réponses, dans toutes vos conversations.
      </p>
      {!selected && (
        <p className="response-preset-note">
          Vos anciens réglages sont conservés. Choisissez un profil pour les
          remplacer.
        </p>
      )}
      <label className="appearance-toggle response-ui-toggle">
        <span>
          Interfaces interactives
          <small>
            L’IA peut proposer un schéma ou des contrôles quand ils sont utiles.
          </small>
        </span>
        <input
          type="checkbox"
          role="switch"
          checked={settings.generativeUI}
          onChange={(e) =>
            onChange({ ...settings, generativeUI: e.target.checked })
          }
        />
      </label>
      <details className="response-preferences">
        <summary>Préférences personnelles</summary>
        <label className="field">
          Comment souhaitez-vous que l’IA vous réponde ?
          <textarea
            rows={4}
            value={settings.systemPrompt}
            placeholder="Par exemple : réponds en français et explique les termes techniques."
            onChange={(e) =>
              onChange({ ...settings, systemPrompt: e.target.value })
            }
          />
        </label>
      </details>
    </>
  );
}
