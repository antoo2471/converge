import { useState } from "react";
import {
  ArrowLeft,
  ArrowUpRight,
  Search,
  Server,
  Globe,
  Layers,
  ShieldCheck,
} from "lucide-react";
import { providerRegistry } from "../core/plugins";
import type { Provider } from "../types";
import { ProviderMark } from "./ProviderBrand";
import { isPreset } from "../core/providerPresets";

const categories = [
  { id: "all", label: "Tous les plugins" },
  { id: "accounts", label: "Comptes IA" },
  { id: "local", label: "En local" },
  { id: "api", label: "API personnalisées" },
] as const;
export function PluginStore({
  providers,
  onBack,
  onConfigure,
}: {
  providers: Provider[];
  onBack: () => void;
  onConfigure: (id: string) => void;
}) {
  const [query, setQuery] = useState(""),
    [category, setCategory] = useState<string>("all");
  const plugins = providerRegistry.list().filter((p) => p.manifest.catalogue);
  const filtered = plugins.filter(
    ({ manifest: m }) =>
      (category === "all" || m.catalogue!.category === category) &&
      `${m.name} ${m.catalogue!.summary} ${m.catalogue!.features.join(" ")}`
        .toLocaleLowerCase()
        .includes(query.trim().toLocaleLowerCase()),
  );
  return (
    <div className="page plugin-store">
      <button className="text-button back" onClick={onBack}>
        <ArrowLeft size={16} />
        Mes connexions
      </button>
      <header className="plugin-store-heading">
        <div>
          <h1>Bibliothèque de fournisseurs</h1>
          <p>Les intelligences et les moteurs qui composent votre espace.</p>
        </div>
        <Layers size={27} aria-hidden="true" />
      </header>
      <label className="search-field plugin-store-search">
        <Search size={18} />
        <input
          autoFocus
          aria-label="Rechercher un fournisseur"
          placeholder="Rechercher un fournisseur, une capacité…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>
      <div className="plugin-store-layout">
        <nav
          className="plugin-store-categories"
          aria-label="Catégories de fournisseurs"
        >
          {categories.map((c) => (
            <button
              key={c.id}
              aria-current={category === c.id ? "page" : undefined}
              onClick={() => setCategory(c.id)}
            >
              {c.label}
              <span>
                {
                  plugins.filter(
                    (p) =>
                      c.id === "all" || p.manifest.catalogue!.category === c.id,
                  ).length
                }
              </span>
            </button>
          ))}
        </nav>
        <section
          className="plugin-store-results"
          aria-label="Fournisseurs disponibles"
        >
          <div className="plugin-store-count">
            {filtered.length} fournisseur{filtered.length > 1 ? "s" : ""}{" "}
            disponible
            {filtered.length > 1 ? "s" : ""}
          </div>
          {filtered.map(({ manifest: m }) => {
            const count = providers.filter((p) => p.plugin === m.id).length;
            return (
              <article className="plugin-store-entry" key={m.id}>
                <div
                  className={`plugin-store-symbol ${m.id === "chatgpt" ? "is-chatgpt" : ""}`}
                >
                  {m.id === "chatgpt" || isPreset(m.id) ? (
                    <ProviderMark plugin={m.id} />
                  ) : m.id === "local" ? (
                    <Server size={27} />
                  ) : (
                    <Globe size={27} />
                  )}
                </div>
                <div className="plugin-store-info">
                  <div className="plugin-store-name">
                    <h2>{m.name}</h2>
                    {count > 0 && (
                      <span>
                        {count} connexion{count > 1 ? "s" : ""}
                      </span>
                    )}
                  </div>
                  <p>{m.catalogue!.summary}</p>
                  <div className="plugin-store-features">
                    {m.catalogue!.features.map((feature) => (
                      <span key={feature}>{feature}</span>
                    ))}
                  </div>
                </div>
                <button
                  className="button plugin-store-connect"
                  onClick={() => onConfigure(m.id)}
                >
                  {count
                    ? "Ajouter"
                    : m.catalogue!.category === "accounts"
                      ? "Connecter"
                      : "Configurer"}
                  <ArrowUpRight size={15} />
                </button>
              </article>
            );
          })}
          {!filtered.length && (
            <div className="plugin-store-empty">
              <Search size={24} />
              <h2>Aucun plugin trouvé</h2>
              <p>Essayez un autre nom ou une autre catégorie.</p>
              <button
                className="text-button"
                onClick={() => {
                  setQuery("");
                  setCategory("all");
                }}
              >
                Voir tous les plugins
              </button>
            </div>
          )}
        </section>
      </div>
      <div className="plugin-store-footer">
        <ShieldCheck size={16} />
        <span>
          Vous choisissez vos connexions. Vos identifiants restent sur cet
          ordinateur.
        </span>
      </div>
    </div>
  );
}
