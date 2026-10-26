import { useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  ExternalLink,
  LoaderCircle,
  RefreshCw,
  Check,
  LogOut,
} from "lucide-react";
import { type Provider } from "../types";
import {
  accountProviders,
  newPresetProvider,
  providerPresets,
  type PresetId,
} from "../core/providerPresets";
import * as api from "../lib/native";
import { useAppDialogs } from "./AppDialogs";
import { ZaiOAuthConnection } from "./ZaiOAuthConnection";

export function AccountProviderEditor({
  plugin,
  existing,
  onClose,
  onSave,
}: {
  plugin: PresetId;
  existing: Provider | null;
  onClose: () => void;
  onSave: (provider: Provider, key?: string) => Promise<void>;
}) {
  const [provider, setProvider] = useState<Provider>(
    () => existing || newPresetProvider(plugin),
  );
  const [key, setKey] = useState("");
  const [status, setStatus] = useState<api.CliProviderStatus | null>(null);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [models, setModels] = useState(provider.models.join("\n"));
  const [verified, setVerified] = useState(false);
  const [zaiConnected, setZaiConnected] = useState(false);
  const oauthZai =
    plugin === "zai" && provider.request?.zai?.authMode === "oauth";
  const alive = useRef(true),
    revision = useRef(0);
  const { askConfirm } = useAppDialogs();
  const preset = providerPresets[plugin];
  const cli = accountProviders.includes(
    plugin as (typeof accountProviders)[number],
  );
  const current = useRef(provider);
  current.current = provider;
  async function close() {
    setBusy("close");
    try {
      if (status?.login === "running")
        await api.cliProviderAction(provider, "cancelLogin");
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy("");
    }
  }
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      revision.current++;
    };
  }, []);
  useEffect(() => {
    if (!cli) return;
    let disposed = false;
    void api
      .cliProviderAction(provider, "status")
      .then((s) => {
        if (!disposed) setStatus(s);
      })
      .catch((e) => {
        if (!disposed) setError(String(e));
      });
    return () => {
      disposed = true;
    };
  }, []);
  useEffect(() => {
    if (status?.login !== "running") return;
    let disposed = false,
      pending = false;
    const timer = setInterval(async () => {
      if (pending) return;
      pending = true;
      try {
        const result = await api.cliProviderAction(current.current, "status");
        if (disposed) return;
        setStatus(result);
        if (result.login === "completed")
          setNotice(
            "Authentification terminée. Le service vérifiera vos droits sur le modèle à chaque réponse.",
          );
        if (result.login === "failed")
          setError(
            "La connexion du client officiel a échoué ou a été fermée. Vous pouvez la relancer.",
          );
      } catch (e) {
        if (!disposed) {
          setError(String(e));
          setStatus(null);
        }
      } finally {
        pending = false;
      }
    }, 2000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [status?.login]);
  const patch = (change: Partial<Provider>) => {
    revision.current++;
    setVerified(false);
    setProvider((p) => ({ ...p, ...change }));
  };
  async function action(kind: "status" | "login" | "logout" | "cancelLogin") {
    if (
      kind === "logout" &&
      !(await askConfirm(`Déconnecter ${preset.name} de ce profil Converge ?`))
    )
      return;
    setBusy(kind);
    setError("");
    setNotice("");
    const version = revision.current;
    try {
      const result = await api.cliProviderAction(provider, kind);
      if (alive.current && version === revision.current) setStatus(result);
    } catch (e) {
      if (alive.current) setError(String(e));
    } finally {
      if (alive.current) setBusy("");
    }
  }
  async function discover() {
    setBusy("discover");
    setError("");
    setNotice("");
    const version = revision.current;
    try {
      const result = await api.discover(
        provider,
        oauthZai ? undefined : key || undefined,
      );
      if (!alive.current || version !== revision.current) return;
      const selected = [...new Set([...provider.models, ...result])];
      setModels(selected.join("\n"));
      setProvider((p) => ({ ...p, models: selected }));
      setVerified(true);
      setNotice(
        cli
          ? "Modèles du client actualisés. « default » utilise le modèle configuré par votre compte."
          : `${result.length} modèle${result.length > 1 ? "s" : ""} reçu${result.length > 1 ? "s" : ""} du fournisseur.`,
      );
    } catch (e) {
      if (alive.current) setError(String(e));
    } finally {
      if (alive.current) setBusy("");
    }
  }
  async function save() {
    setBusy("save");
    setError("");
    try {
      await onSave(
        {
          ...provider,
          models: [
            ...new Set(
              models
                .split(/[\n,]/)
                .map((m) => m.trim())
                .filter(Boolean),
            ),
          ],
        },
        oauthZai ? undefined : key || undefined,
      );
    } catch (e) {
      if (alive.current) setError(String(e));
    } finally {
      if (alive.current) setBusy("");
    }
  }
  return (
    <div className="page provider-editor account-provider-editor">
      <button
        className="text-button back"
        disabled={!!busy}
        onClick={() => void close()}
      >
        <ArrowLeft size={16} /> Fournisseurs
      </button>
      <div className="editor-heading">
        <div>
          <h1>{preset.name}</h1>
          <p>{preset.description}</p>
        </div>
      </div>
      {plugin === "zai" && (
        <div
          className="account-provider-services"
          role="group"
          aria-label="Connexion Z.ai"
        >
          <button
            className={`button ${oauthZai ? "primary" : ""}`}
            aria-pressed={oauthZai}
            disabled={!!busy}
            onClick={() => {
              setZaiConnected(false);
              patch({
                protocol: "openai",
                url: "https://api.z.ai/api/coding/paas/v4",
                request: { ...provider.request, zai: { authMode: "oauth" } },
              });
            }}
          >
            Compte Coding Plan
          </button>
          <button
            className={`button ${!oauthZai ? "primary" : ""}`}
            aria-pressed={!oauthZai}
            disabled={!!busy}
            onClick={() => {
              setZaiConnected(false);
              patch({
                request: { ...provider.request, zai: { authMode: "api-key" } },
              });
            }}
          >
            Clé API
          </button>
        </div>
      )}
      {oauthZai ? (
        <ZaiOAuthConnection
          provider={provider}
          onConnectionChange={setZaiConnected}
        />
      ) : cli ? (
        <section className="editor-section">
          <div className="chatgpt-account-row">
            <div>
              <strong>
                {status?.authenticated
                  ? "Compte connecté"
                  : status?.available
                    ? "Client officiel disponible"
                    : "Client officiel requis"}
              </strong>
              <p>
                {status?.message ||
                  (plugin === "antigravity"
                    ? "La connexion Google est gérée par Antigravity dans un profil dédié à Converge."
                    : `Converge utilise un profil ${preset.command} dédié. Les identifiants sont gérés par le client officiel.`)}
              </p>
            </div>
            {status?.authenticated && (
              <span className="chatgpt-connected">
                <Check size={14} /> Connecté
              </span>
            )}
          </div>
          <div className="toolbar">
            {status?.login === "running" ? (
              <>
                <span role="status">
                  <LoaderCircle size={15} className="spin" /> Terminez la
                  connexion dans la fenêtre du client officiel.
                </span>
                <button
                  className="button"
                  disabled={!!busy}
                  onClick={() => void action("cancelLogin")}
                >
                  Annuler
                </button>
              </>
            ) : (
              <button
                className="button primary"
                disabled={!!busy || !status?.available}
                onClick={() => void action("login")}
              >
                <ExternalLink size={15} /> Se connecter avec {preset.name}
              </button>
            )}
            <button
              className="button"
              disabled={!!busy}
              onClick={() => void action("status")}
            >
              <RefreshCw size={15} /> Vérifier le client
            </button>
            {status?.authenticated && plugin !== "antigravity" && (
              <button
                className="text-button"
                disabled={!!busy}
                onClick={() => void action("logout")}
              >
                <LogOut size={15} /> Déconnecter
              </button>
            )}
            <button
              className="text-button"
              onClick={() =>
                void api
                  .openExternal(preset.install)
                  .catch((e) => setError(String(e)))
              }
            >
              Installer le client <ExternalLink size={14} />
            </button>
          </div>
          <details className="account-provider-advanced">
            <summary>Chemin du client officiel</summary>
            <label className="field">
              Exécutable
              <input
                value={provider.request?.cli?.executable || ""}
                placeholder={`${preset.command} · détection automatique`}
                disabled={status?.login === "running" || !!busy}
                onChange={(e) => {
                  setStatus(null);
                  patch({
                    request: {
                      ...provider.request,
                      cli: { executable: e.target.value },
                    },
                  });
                }}
              />
            </label>
          </details>
        </section>
      ) : (
        <section className="editor-section">
          {plugin === "opencode" && (
            <div
              className="account-provider-services"
              role="group"
              aria-label="Service OpenCode"
            >
              {[
                {
                  name: "Go · abonnement",
                  url: "https://opencode.ai/inference/go",
                },
                { name: "Zen · crédits", url: "https://opencode.ai/inference" },
              ].map((service) => (
                <button
                  key={service.url}
                  className={`button ${provider.url === service.url ? "primary" : ""}`}
                  aria-pressed={provider.url === service.url}
                  onClick={() => {
                    patch({ url: service.url, models: [] });
                    setModels("");
                  }}
                >
                  {service.name}
                </button>
              ))}
            </div>
          )}
          <label className="field">
            Clé API
            <input
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={key}
              placeholder={
                existing
                  ? "Clé enregistrée · laisser vide pour la conserver"
                  : "Collez votre clé API"
              }
              onChange={(e) => {
                setKey(e.target.value);
                setVerified(false);
              }}
            />
          </label>
          <p className="muted">
            La clé est enregistrée dans le coffre sécurisé de votre système.
          </p>
          <label className="field">
            URL du service
            <input
              value={provider.url}
              onChange={(e) => patch({ url: e.target.value })}
            />
          </label>
        </section>
      )}
      <section className="editor-section">
        <div className="account-provider-section-heading">
          <div>
            <h2>Modèles</h2>
            <p>
              {cli
                ? "Utilisez le modèle de votre compte ou ajoutez des identifiants précis."
                : "Actualisez le catalogue ou saisissez vos identifiants de modèles."}
            </p>
          </div>
          <button
            className="button"
            disabled={!!busy || (cli && !status?.available)}
            onClick={() => void discover()}
          >
            {busy === "discover" ? (
              <LoaderCircle size={15} className="spin" />
            ) : (
              <RefreshCw size={15} />
            )}{" "}
            Actualiser
          </button>
        </div>
        <label className="field">
          Un identifiant par ligne
          <textarea
            rows={4}
            value={models}
            spellCheck={false}
            onChange={(e) => {
              setModels(e.target.value);
              patch({
                models: e.target.value
                  .split(/[\n,]/)
                  .map((m) => m.trim())
                  .filter(Boolean),
              });
            }}
          />
        </label>
        {plugin === "claude-code" && (
          <p className="muted">
            sonnet, opus et haiku sont des alias officiels. Leur disponibilité
            dépend de votre compte.
          </p>
        )}
        {plugin === "opencode" && (
          <p className="muted">
            Le format de requête est adapté au modèle : Messages, Responses,
            Gemini ou Chat Completions.
          </p>
        )}
        {!cli && !oauthZai && (
          <label className="field">
            Format par défaut
            <select
              value={provider.protocol}
              onChange={(e) =>
                patch({ protocol: e.target.value as Provider["protocol"] })
              }
            >
              <option value="openai">OpenAI · Chat Completions</option>
              <option value="anthropic">Anthropic · Messages</option>
              <option value="responses">OpenAI · Responses</option>
            </select>
          </label>
        )}
      </section>
      <section className="editor-section">
        <label className="field">
          Nom de la connexion
          <input
            value={provider.name}
            onChange={(e) => patch({ name: e.target.value })}
          />
        </label>
        {!cli && (
          <label className="check">
            <input
              type="checkbox"
              checked={provider.capabilities.vision}
              onChange={(e) =>
                patch({
                  capabilities: {
                    ...provider.capabilities,
                    vision: e.target.checked,
                  },
                })
              }
            />{" "}
            Modèles compatibles avec les images
          </label>
        )}
      </section>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {notice && (
        <p className="muted" role="status">
          {notice}
        </p>
      )}
      <div className="editor-footer">
        <button
          className="button"
          disabled={!!busy}
          onClick={() => void close()}
        >
          Annuler
        </button>
        <button
          className="button primary"
          disabled={
            !!busy ||
            !provider.name.trim() ||
            !models.trim() ||
            (cli && (!status?.available || status.login === "running")) ||
            (oauthZai && !zaiConnected) ||
            (!cli && !oauthZai && !existing && !key.trim())
          }
          onClick={() => void save()}
        >
          {busy === "save" && <LoaderCircle size={15} className="spin" />}{" "}
          Enregistrer{verified ? " la connexion" : ""}
        </button>
      </div>
    </div>
  );
}
