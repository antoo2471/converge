import type { Settings } from "../types";
export const toolGroups = [
  {
    id: "files",
    label: "Fichiers et projets",
    description: "Lire, rechercher et modifier les fichiers du dossier choisi.",
    tools: [
      ["filesystem.read", "Lire les fichiers"],
      ["filesystem.list", "Parcourir les dossiers"],
      ["filesystem.write", "Créer et enregistrer des fichiers"],
      ["coding.search", "Rechercher dans un projet"],
      ["coding.patch", "Modifier le code"],
    ],
  },
  {
    id: "terminal",
    label: "Commandes",
    description:
      "Lancer des commandes et des tests dans un environnement isolé.",
    tools: [["terminal.execute", "Exécuter des commandes"]],
  },
  {
    id: "web",
    label: "Web",
    description: "Consulter des pages et rechercher sur Internet.",
    tools: [
      ["web.fetch", "Consulter des pages"],
      ["web.search", "Rechercher sur le Web"],
      ["web.images", "Rechercher des images"],
    ],
  },
  {
    id: "computer",
    label: "Ordinateur",
    description:
      "Voir l’écran et agir sur les fenêtres, la souris et le clavier.",
    tools: [
      ["computer.capture", "Capturer l’écran"],
      ["computer.actions", "Enchaîner des actions"],
      ["computer.windows", "Lister les fenêtres"],
      ["computer.focus", "Changer de fenêtre"],
      ["computer.click", "Utiliser la souris"],
      ["computer.type", "Saisir du texte"],
      ["computer.key", "Utiliser les touches et raccourcis"],
      ["computer.scroll", "Faire défiler"],
    ],
  },
] as const;
export const allTools = toolGroups.flatMap((group) =>
  group.tools.map(([name]) => name),
);
export function setTools(
  enabled: string[],
  names: readonly string[],
  active: boolean,
): string[] {
  const next = new Set(enabled);
  for (const name of names) active ? next.add(name) : next.delete(name);
  return [...next];
}
/** Apply the user's new default once; preserve later opt-outs across restarts. */
export function migrateToolDefaults(settings: Settings): Settings {
  if (settings.toolDefaultsVersion === 2) return settings;
  if (settings.toolDefaultsVersion === 1) {
    const enabled = settings.enabledTools;
    const previousComputer = toolGroups.find(g => g.id === "computer")!.tools.filter(([name]) => name !== "computer.actions");
    return { ...settings, toolDefaultsVersion: 2, enabledTools: previousComputer.every(([name]) => enabled.includes(name)) ? [...new Set([...enabled, "computer.actions"])] : enabled };
  }
  return { ...settings, toolDefaultsVersion: 2, enabledTools: [...allTools] };
}
