import { useAppDialogs } from "./AppDialogs";
import { ChatGPTProviderEditor } from "./ChatGPTProvider";
import { PluginStore } from "./PluginStore";
import { AccountProviderEditor } from "./AccountProvider";
import { ProviderMark } from "./ProviderBrand";
import { isPreset, type PresetId } from "../core/providerPresets";
import { useState, useEffect } from "react";
import {
  Plus,
  Search,
  Server,
  Globe,
  MoreHorizontal,
  ArrowLeft,
  Check,
  LoaderCircle,
  KeyRound,
  ShieldCheck,
  Trash2,
  Power,
  Settings2,
  X,
} from "lucide-react";
import { id, type Provider, type Protocol } from "../types";
import * as api from "../lib/native";
const formats: { value: Protocol; label: string; description: string }[] = [
  {
    value: "openai",
    label: "OpenAI compatible",
    description: "Chat completions · streaming SSE",
  },
  {
    value: "anthropic",
    label: "Anthropic compatible",
    description: "Messages · streaming SSE",
  },
  {
    value: "ollama",
    label: "Ollama compatible",
    description: "Chat · streaming NDJSON",
  },
  {
    value: "comfyui",
    label: "ComfyUI",
    description: "Workflows de génération d’images",
  },
];
export function Providers({
  providers,
  onChange,
  onError,
}: {
  providers: Provider[];
  onChange: (p: Provider[]) => void;
  onError: (e: unknown) => void;
}) {
  const { askConfirm } = useAppDialogs();
  const [adding, setAdding] = useState(false),
    [addingChatGPT, setAddingChatGPT] = useState(false),
    [addingPreset, setAddingPreset] = useState<PresetId | null>(null),
    [storeOpen, setStoreOpen] = useState(false),
    [creatingPlugin, setCreatingPlugin] = useState<"custom" | "local">(
      "custom",
    ),
    [editing, setEditing] = useState<Provider | null>(null),
    [query, setQuery] = useState(""),
    [health, setHealth] = useState<Record<string, string>>({});
  useEffect(() => {
    let alive = true;
    void Promise.allSettled(
      providers
        .filter((p) => p.enabled)
        .map(async (p) => {
          try {
            await api.discover(p);
            if (alive)
              setHealth((s) => ({
                ...s,
                [p.id]:
                  p.protocol === "cli" ? "Client disponible" : "Joignable",
              }));
          } catch {
            if (alive) setHealth((s) => ({ ...s, [p.id]: "À vérifier" }));
          }
        }),
    );
    return () => {
      alive = false;
    };
  }, [providers]);
  async function toggle(p: Provider) {
    try {
      const next = { ...p, enabled: !p.enabled };
      await api.saveProvider(next);
      onChange(providers.map((x) => (x.id === p.id ? next : x)));
    } catch (e) {
      onError(e);
    }
  }
  async function remove(p: Provider) {
    if (
      !(await askConfirm(
        p.plugin === "chatgpt"
          ? `Supprimer « ${p.name} » et sa connexion ChatGPT ? Les conversations seront conservées.`
          : `Supprimer « ${p.name} » et ses identifiants ? Les conversations seront conservées.`,
      ))
    )
      return;
    try {
      await api.removeProvider(p.id);
      onChange(providers.filter((x) => x.id !== p.id));
    } catch (e) {
      onError(e);
    }
  }
  if (addingPreset || (editing && isPreset(editing.plugin))) {
    const preset = addingPreset || (editing!.plugin as PresetId);
    return (
      <AccountProviderEditor
        plugin={preset}
        existing={editing}
        onClose={() => {
          setAddingPreset(null);
          setEditing(null);
        }}
        onSave={async (p, key) => {
          await api.saveProvider(p, key);
          onChange([
            ...providers.filter((x) => x.id !== p.id),
            { ...p, headers: {} },
          ]);
          setAddingPreset(null);
          setEditing(null);
          setStoreOpen(false);
        }}
      />
    );
  }
  if (addingChatGPT || editing?.plugin === "chatgpt")
    return (
      <ChatGPTProviderEditor
        existing={editing}
        onClose={() => {
          setAddingChatGPT(false);
          setEditing(null);
        }}
        onSave={async (p) => {
          await api.saveProvider(p);
          onChange([...providers.filter((x) => x.id !== p.id), p]);
          setAddingChatGPT(false);
          setEditing(null);
          setStoreOpen(false);
        }}
      />
    );
  if (adding || editing)
    return (
      <ProviderEditor
        plugin={creatingPlugin}
        existing={editing}
        onClose={() => {
          setAdding(false);
          setEditing(null);
        }}
        onSave={async (p, key) => {
          await api.saveProvider(p, key);
          onChange([
            ...providers.filter((x) => x.id !== p.id),
            { ...p, headers: {} },
          ]);
          setAdding(false);
          setEditing(null);
          setStoreOpen(false);
        }}
      />
    );
  if (storeOpen)
    return (
      <PluginStore
        providers={providers}
        onBack={() => setStoreOpen(false)}
        onConfigure={(plugin) => {
          if (plugin === "chatgpt") setAddingChatGPT(true);
          else if (isPreset(plugin)) setAddingPreset(plugin);
          else if (plugin === "local" || plugin === "custom") {
            setCreatingPlugin(plugin);
            setAdding(true);
          }
        }}
      />
    );
  const filtered = providers.filter((p) =>
    (p.name + " " + p.url + " " + p.models.join(" "))
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  return (
    <div className="page providers-page">
      <div className="library-heading">
        <div>
          <h1>Vos fournisseurs</h1>
          <p>Votre accès aux modèles, configuré comme vous le souhaitez.</p>
        </div>
        <div className="toolbar">
          <button className="button primary" onClick={() => setStoreOpen(true)}>
            <Plus size={18} />
            Parcourir les fournisseurs
          </button>
        </div>
      </div>
      {providers.length > 0 ? (
        <>
          <div className="provider-list-toolbar">
            <span>
              {providers.length} connexion{providers.length > 1 ? "s" : ""}
            </span>
            <label className="search-field">
              <Search size={17} />
              <input
                aria-label="Rechercher un fournisseur"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Rechercher un fournisseur…"
              />
            </label>
          </div>
          <div className="provider-list">
            {filtered.map((p) => (
              <article className="provider-list-row" key={p.id}>
                <div className="connection-symbol">
                  {p.plugin === "chatgpt" || isPreset(p.plugin) ? (
                    <ProviderMark plugin={p.plugin} />
                  ) : (
                    <Globe size={23} />
                  )}
                </div>
                <button
                  className="connection-details"
                  onClick={() => setEditing(p)}
                >
                  <strong>{p.name}</strong>
                  <span>
                    {p.plugin === "chatgpt" ? "Compte ChatGPT · Codex" : p.url}
                  </span>
                </button>
                <div className="connection-models">
                  <strong>
                    {p.models.length} modèle{p.models.length > 1 ? "s" : ""}
                  </strong>
                  <small>
                    {p.plugin === "chatgpt"
                      ? "Codex App Server"
                      : p.protocol === "cli"
                        ? "Client officiel · OAuth"
                        : formats.find((f) => f.value === p.protocol)?.label}
                  </small>
                </div>
                <span
                  className={`connection-state ${p.enabled && health[p.id] === "Joignable" ? "is-online" : ""}`}
                >
                  {p.enabled ? health[p.id] || "Vérification…" : "Désactivé"}
                </span>
                <div className="toolbar">
                  <button
                    className="icon-button"
                    title="Configurer le fournisseur"
                    onClick={() => setEditing(p)}
                  >
                    <Settings2 size={17} />
                  </button>
                  <button
                    className="icon-button"
                    title={p.enabled ? "Désactiver" : "Activer"}
                    onClick={() => toggle(p)}
                  >
                    <Power size={17} />
                  </button>
                  <button
                    className="icon-button danger"
                    title="Supprimer le fournisseur"
                    onClick={() => remove(p)}
                  >
                    <Trash2 size={17} />
                  </button>
                </div>
              </article>
            ))}
            {!filtered.length && (
              <p className="muted">
                Aucun fournisseur ne correspond à cette recherche.
              </p>
            )}
          </div>
        </>
      ) : (
        <div className="provider-empty">
          <div className="provider-empty-symbol">
            <Server size={34} />
          </div>
          <h2>Tout part d’une connexion.</h2>
          <p>
            Ajoutez votre propre fournisseur : une URL, un format de requête
            <br />
            et les modèles que vous voulez utiliser. C’est vous qui choisissez.
          </p>
          <button className="button primary" onClick={() => setStoreOpen(true)}>
            <Plus size={18} />
            Explorer les plugins
          </button>
        </div>
      )}
      <div className="provider-security">
        <ShieldCheck size={17} />
        <span>Vos clés restent dans le coffre de votre ordinateur.</span>
      </div>
    </div>
  );
}
function ProviderEditor({
  plugin = "custom",
  existing,
  onClose,
  onSave,
}: {
  plugin?: "local" | "custom";
  existing: Provider | null;
  onClose: () => void;
  onSave: (p: Provider, key: string) => Promise<void>;
}) {
  const [providerId] = useState(existing?.id || id()),
    [name, setName] = useState(existing?.name || ""),
    [url, setUrl] = useState(existing?.url || ""),
    [protocol, setProtocol] = useState<Protocol>(
      existing?.protocol || "openai",
    ),
    [key, setKey] = useState(""),
    [headers, setHeaders] = useState(""),
    [models, setModels] = useState(existing?.models.join("\n") || ""),
    [caps, setCaps] = useState<Provider["capabilities"]>(
      existing?.capabilities || {
        vision: false,
        tools: false,
        images: false,
        context: 32768,
      },
    ),
    [working, setWorking] = useState(""),
    [error, setError] = useState(""),
    [success, setSuccess] = useState(""),
    [tab, setTab] = useState("connection");
  const [chatPath, setChatPath] = useState(existing?.request?.chatPath || ""),
    [modelsPath, setModelsPath] = useState(existing?.request?.modelsPath || ""),
    [body, setBody] = useState(
      existing?.request?.body
        ? JSON.stringify(existing.request.body, null, 2)
        : "",
    );
  const configuration = (): Provider => ({
    id: providerId,
    name: name.trim(),
    plugin: existing?.plugin === "local" ? "local" : plugin,
    url: url.trim().replace(/\/$/, ""),
    protocol,
    headers: headers.trim() ? JSON.parse(headers) : {},
    models: [
      ...new Set(
        models
          .split(/[\n,]/)
          .map((s) => s.trim())
          .filter(Boolean),
      ),
    ],
    capabilities: caps,
    enabled: existing?.enabled ?? true,
    request: {
      chatPath: chatPath.trim() || undefined,
      modelsPath: modelsPath.trim() || undefined,
      body: body.trim() ? JSON.parse(body) : undefined,
    },
  });
  async function discover() {
    setError("");
    setSuccess("");
    setWorking("discover");
    try {
      const p = configuration();
      if (!p.name || !p.url) {
        setTab("connection");
        throw Error("Renseignez le nom et l’URL du fournisseur.");
      }
      const result = await api.discover(
        { ...p, name: p.name || "Nouveau fournisseur" },
        key,
      );
      setModels(result.join("\n"));
      setSuccess(
        result.length
          ? `${result.length} modèle(s) trouvé(s)`
          : "Connexion établie. Le serveur n’a renvoyé aucun modèle.",
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking("");
    }
  }
  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError("");
    setWorking("save");
    try {
      const p = configuration();
      if (!p.name || !p.url) {
        setTab("connection");
        throw Error("Renseignez le nom et l’URL du fournisseur.");
      }
      if (!p.models.length)
        throw Error("Ajoutez au moins un modèle ou utilisez la découverte.");
      await onSave(p, key);
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking("");
    }
  }
  return (
    <div className="page provider-editor">
      <button className="text-button back" onClick={onClose}>
        <ArrowLeft size={16} />
        Fournisseurs
      </button>
      <div className="editor-heading">
        <div className="connection-symbol">
          <Globe size={26} />
        </div>
        <div>
          <h1>{existing ? existing.name : "Ajouter un fournisseur"}</h1>
          <p>Une connexion à votre image.</p>
        </div>
      </div>
      <div className="editor-tabs">
        {[
          { id: "connection", label: "Connexion" },
          { id: "models", label: "Modèles et capacités" },
          { id: "advanced", label: "Requêtes avancées" },
        ].map((t) => (
          <button
            className={tab === t.id ? "active" : ""}
            onClick={() => setTab(t.id)}
            key={t.id}
          >
            {t.label}
          </button>
        ))}
      </div>
      <form onSubmit={submit}>
        <div hidden={tab !== "connection"} className="editor-section">
          <label className="field">
            Nom du fournisseur
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Le nom que vous voulez lui donner"
              maxLength={120}
            />
          </label>
          <label className="field">
            URL de base
            <input
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://votre-endpoint.example/v1"
            />
            <small>
              Une adresse locale est également acceptée. Incluez le préfixe de
              votre API dans l’URL.
            </small>
          </label>
          <fieldset className="request-formats">
            <legend>Format des requêtes</legend>
            {formats
              .filter((f) => plugin !== "local" || f.value !== "anthropic")
              .map((f) => (
                <label
                  className={`request-format ${protocol === f.value ? "selected" : ""}`}
                  key={f.value}
                >
                  <input
                    type="radio"
                    name="request-format"
                    checked={protocol === f.value}
                    onChange={() => {
                      setProtocol(f.value);
                      if (f.value === "comfyui")
                        setCaps({ ...caps, images: true });
                    }}
                  />
                  <span>
                    <strong>{f.label}</strong>
                    <small>{f.description}</small>
                  </span>
                  {protocol === f.value && <Check size={16} />}
                </label>
              ))}
          </fieldset>
          <label className="field">
            <span>
              <KeyRound size={15} />
              Clé API
            </span>
            <input
              type="password"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              autoComplete="off"
              placeholder={
                existing
                  ? "Laisser vide pour conserver la clé actuelle"
                  : "Facultative si votre serveur n’en demande pas"
              }
            />
          </label>
        </div>
        <div hidden={tab !== "models"} className="editor-section">
          <div className="between">
            <div>
              <h2>Vos modèles</h2>
              <p className="muted">
                Les identifiants exacts acceptés par votre endpoint.
              </p>
            </div>
            <button
              type="button"
              className="button"
              disabled={!!working || !url.trim()}
              onClick={discover}
            >
              {working === "discover" ? (
                <LoaderCircle size={16} className="spin" />
              ) : (
                <Search size={16} />
              )}
              Détecter
            </button>
          </div>
          <textarea
            aria-label="Liste des modèles"
            rows={6}
            value={models}
            onChange={(e) => setModels(e.target.value)}
            placeholder="Un identifiant de modèle par ligne"
          />
          {success && (
            <p className="success">
              <Check size={14} />
              {success}
            </p>
          )}
          <h2>Capacités du fournisseur</h2>
          <div className="capability-switches">
            {(
              [
                {
                  key: "vision",
                  label: "Images et vision",
                  detail: "Transmettre des images avec vos messages",
                },
                {
                  key: "tools",
                  label: "Appels d’outils natifs",
                  detail: "Autoriser le modèle à demander des outils",
                },
                {
                  key: "images",
                  label: "Génération d’images",
                  detail: "Générer des images depuis une conversation",
                },
              ] as const
            ).map((c) => (
              <label className="setting-toggle-row" key={c.key}>
                <span>
                  <strong>{c.label}</strong>
                  <small>{c.detail}</small>
                </span>
                <input
                  type="checkbox"
                  checked={caps[c.key]}
                  onChange={(e) =>
                    setCaps({ ...caps, [c.key]: e.target.checked })
                  }
                />
              </label>
            ))}
          </div>
          <label className="field">
            Fenêtre de contexte
            <input
              type="number"
              min={512}
              max={2000000}
              value={caps.context}
              onChange={(e) =>
                setCaps({ ...caps, context: Number(e.target.value) })
              }
            />
            <small>Nombre de tokens annoncé par votre moteur.</small>
          </label>
        </div>
        <div hidden={tab !== "advanced"} className="editor-section">
          <div className="form-grid">
            <label className="field">
              Chemin de conversation
              <input
                value={chatPath}
                onChange={(e) => setChatPath(e.target.value)}
                placeholder={
                  protocol === "anthropic"
                    ? "messages"
                    : protocol === "ollama"
                      ? "api/chat"
                      : "chat/completions"
                }
              />
            </label>
            <label className="field">
              Chemin de découverte
              <input
                value={modelsPath}
                onChange={(e) => setModelsPath(e.target.value)}
                placeholder={protocol === "ollama" ? "api/tags" : "models"}
              />
            </label>
          </div>
          <label className="field">
            Paramètres de requête supplémentaires
            <textarea
              rows={4}
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder={'{"top_p": 0.9}'}
            />
            <small>
              Objet JSON ajouté au corps des requêtes de conversation.
            </small>
          </label>
          <label className="field">
            Headers personnalisés
            <textarea
              rows={4}
              value={headers}
              onChange={(e) => setHeaders(e.target.value)}
              placeholder={'{"X-Custom-Header": "valeur"}'}
            />
            <small>Conservés dans le coffre système, comme votre clé.</small>
          </label>
          {protocol === "comfyui" && (
            <label className="field">
              Workflow au format API
              <textarea
                rows={6}
                value={caps.workflow || ""}
                onChange={(e) => setCaps({ ...caps, workflow: e.target.value })}
              />
              <small>
                Variables : {"{{prompt}}"}, {"{{model}}"}, {"{{width}}"},{" "}
                {"{{height}}"}, {"{{seed}}"}, {"{{image}}"}.
              </small>
            </label>
          )}
        </div>
        {error && (
          <div className="inline-error" role="alert">
            {error}
          </div>
        )}
        <div className="form-actions">
          <button type="button" className="button" onClick={onClose}>
            Annuler
          </button>
          {tab === "connection" ? (
            <button
              type="button"
              className="button primary"
              onClick={() => {
                if (!name.trim() || !url.trim()) {
                  setError("Renseignez le nom et l’URL du fournisseur.");
                  return;
                }
                setError("");
                setTab("models");
              }}
            >
              Choisir les modèles
            </button>
          ) : (
            <button
              type="submit"
              className="button primary"
              disabled={!!working}
            >
              {working === "save" ? (
                <LoaderCircle size={16} className="spin" />
              ) : (
                <Check size={17} />
              )}
              Enregistrer le fournisseur
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
