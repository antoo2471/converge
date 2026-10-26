import { useEffect, useState } from "react";
import {
  ArrowUpRight,
  BookOpen,
  Check,
  Download,
  LoaderCircle,
} from "lucide-react";
import * as api from "../lib/extensions";
import { native, openExternal } from "../lib/native";

export function SkillsLibrary({
  query,
  installed,
  onInstalled,
}: {
  query: string;
  installed: api.Skill[];
  onInstalled: () => Promise<void>;
}) {
  const [skills, setSkills] = useState<api.CatalogueSkill[]>([]);
  const [loading, setLoading] = useState(false),
    [error, setError] = useState(""),
    [busy, setBusy] = useState("");
  useEffect(() => {
    let alive = true;
    if (!native) return;
    const timer = setTimeout(() => {
      setLoading(true);
      setError("");
      void api
        .skillCatalogue(query)
        .then((result) => {
          if (alive) setSkills(result.skills);
        })
        .catch((e) => {
          if (alive) setError(String(e));
        })
        .finally(() => {
          if (alive) setLoading(false);
        });
    }, 300);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [query]);
  async function install(skill: api.CatalogueSkill) {
    setBusy(skill.id);
    setError("");
    try {
      await api.installSkill(skill.source, skill.skillId || skill.name);
      await onInstalled();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy("");
    }
  }
  return (
    <section className="extension-section skills-library">
      <div className="skills-library-source">
        <p>
          Le catalogue de skills.sh, utilisé par <code>npx skills</code>. Les
          skills sont importés dans Converge sans exécuter leurs scripts.
        </p>
        <button
          className="text-button"
          onClick={() =>
            void openExternal("https://skills.sh").catch((e) =>
              setError(String(e)),
            )
          }
        >
          skills.sh <ArrowUpRight size={13} />
        </button>
      </div>
      {!native && (
        <p className="muted">
          Ouvrez Converge desktop pour parcourir et installer les skills.
        </p>
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {loading ? (
        <p className="muted" role="status">
          <LoaderCircle size={16} className="spin" /> Recherche dans skills.sh…
        </p>
      ) : skills.length ? (
        <div className="extension-app-grid">
          {skills.map((skill) => {
            const exists = installed.some(
              (s) =>
                s.source === skill.source &&
                s.catalogueSkill === (skill.skillId || skill.name),
            );
            return (
              <article className="extension-app-row" key={skill.id}>
                <span className="extension-mark">
                  <BookOpen size={22} />
                </span>
                <div>
                  <h3>{skill.name}</h3>
                  <p>{skill.source}</p>
                  <small className="muted">
                    {new Intl.NumberFormat("fr").format(skill.installs || 0)}{" "}
                    installations
                  </small>
                </div>
                <button
                  className="icon-button"
                  disabled={!native || exists || !!busy}
                  aria-label={`${exists ? "Installé" : "Installer"} ${skill.name}`}
                  title={exists ? "Déjà installé" : "Importer ce skill"}
                  onClick={() => void install(skill)}
                >
                  {busy === skill.id ? (
                    <LoaderCircle size={17} className="spin" />
                  ) : exists ? (
                    <Check size={17} />
                  ) : (
                    <Download size={17} />
                  )}
                </button>
              </article>
            );
          })}
        </div>
      ) : (
        native &&
        !error && (
          <p className="muted">Aucun skill trouvé. Essayez un autre mot-clé.</p>
        )
      )}
    </section>
  );
}
