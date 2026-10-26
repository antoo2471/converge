import { useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  Check,
  ExternalLink,
  LoaderCircle,
  LogOut,
  RefreshCw,
  ShieldCheck,
} from "lucide-react";
import { ChatGPTMark } from "./ProviderBrand";
export { ChatGPTMark } from "./ProviderBrand";
import { id, type Provider } from "../types";
import * as api from "../lib/native";
import { useAppDialogs } from "./AppDialogs";

export function newChatGPTProvider(): Provider {
  return {
    id: id(),
    name: "ChatGPT",
    plugin: "chatgpt",
    protocol: "codex",
    url: "https://chatgpt.com",
    headers: {},
    models: [],
    capabilities: { vision: false, tools: true, images: false, context: 32768 },
    enabled: true,
    request: { chatgpt: {} },
  };
}
export function ChatGPTProviderEditor({
  existing,
  onClose,
  onSave,
}: {
  existing: Provider | null;
  onClose: () => void;
  onSave: (provider: Provider) => Promise<void>;
}) {
  const [provider, setProvider] = useState<Provider>(() =>
    existing
      ? {
          ...existing,
          request: {
            ...existing.request,
            chatgpt: {
              ...existing.request?.chatgpt,
            },
          },
        }
      : newChatGPTProvider(),
  );
  const [status, setStatus] = useState<api.ChatGPTStatus | null>(null);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [login, setLogin] = useState<{ id: string; url: string } | null>(null);
  const alive = useRef(true),
    generation = useRef(0),
    current = useRef(provider);
  current.current = provider;
  const { askConfirm } = useAppDialogs();
  const config = provider.request!.chatgpt!;
  const close = () => {
    if (login)
      void api.chatgptAction(provider, "cancelLogin", login.id).catch(() => {});
    onClose();
  };
  const setConfig = (patch: Partial<typeof config>) => {
    generation.current++;
    setStatus(null);
    setError("");
    setNotice("");
    setProvider((p) => ({
      ...p,
      models: [],
      capabilities: { ...p.capabilities, vision: false },
      request: {
        ...p.request,
        chatgpt: { ...p.request!.chatgpt!, ...patch, models: [] },
      },
    }));
  };
  const applyStatus = (result: api.ChatGPTStatus) => {
    setStatus(result);
    setProvider((p) => ({
      ...p,
      models: (result.models || []).map((m) => `codex/${m.id}`),
      capabilities: result.capabilities || p.capabilities,
      request: {
        ...p.request,
        chatgpt: { ...p.request!.chatgpt!, models: result.models || [] },
      },
    }));
  };
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      generation.current++;
    };
  }, []);
  useEffect(() => {
    if (!existing) return;
    let disposed = false;
    const version = generation.current;
    void api
      .chatgptAction(provider, "status")
      .then((result) => {
        if (!disposed && version === generation.current) applyStatus(result);
      })
      .catch((e) => {
        if (!disposed && version === generation.current) setError(String(e));
      });
    return () => {
      disposed = true;
    };
  }, []);
  async function action(
    kind: "status" | "diagnose" | "login" | "logout" | "cancelLogin",
  ) {
    setBusy(kind);
    setError("");
    setNotice("");
    const version = generation.current;
    try {
      if (
        kind === "logout" &&
        !(await askConfirm("Déconnecter ce compte ChatGPT de Converge ?"))
      )
        return;
      const result = await api.chatgptAction(provider, kind, login?.id);
      if (!alive.current || version !== generation.current) return;
      if (kind === "login" && result.loginId && result.authUrl) {
        setLogin({ id: result.loginId, url: result.authUrl });
      } else if (kind === "cancelLogin") {
        setLogin(null);
        setNotice("Connexion annulée.");
      } else {
        applyStatus(result);
        if (kind === "diagnose")
          setNotice(
            result.account
              ? "Compte connecté. Le catalogue ne garantit pas l’accès : le service vérifie chaque génération."
              : "Codex est disponible. Connectez votre compte ChatGPT.",
          );
      }
    } catch (e) {
      if (alive.current) setError(String(e));
    } finally {
      if (alive.current) setBusy("");
    }
  }
  // Observe completion without reading, copying, or exposing tokens. No model request is made by polling.
  useEffect(() => {
    if (!login) return;
    let disposed = false,
      inFlight = false;
    const start = Date.now();
    const timer = setInterval(async () => {
      if (inFlight || disposed) return;
      if (Date.now() - start > 5 * 60 * 1000) {
        setError("Connexion expirée. Relancez la connexion.");
        setLogin(null);
        return;
      }
      inFlight = true;
      try {
        const result = await api.chatgptAction(current.current, "status");
        if (!disposed && result.account) {
          applyStatus(result);
          setLogin(null);
          setNotice("Compte ChatGPT connecté.");
        }
      } catch (e) {
        if (!disposed) {
          setError(String(e));
          setLogin(null);
        }
      } finally {
        inFlight = false;
      }
    }, 2000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [login]);
  async function save() {
    setBusy("save");
    setError("");
    try {
      await onSave(provider);
    } catch (e) {
      setError(String(e));
    } finally {
      if (alive.current) setBusy("");
    }
  }
  const windows = [
    status?.limits?.rateLimits?.primary,
    status?.limits?.rateLimits?.secondary,
  ].filter(Boolean);
  return (
    <div className="page provider-editor chatgpt-editor">
      <button className="text-button back" disabled={!!busy} onClick={close}>
        <ArrowLeft size={16} />
        Fournisseurs
      </button>
      <div className="editor-heading">
        <div className="connection-symbol">
          <ChatGPTMark />
        </div>
        <div>
          <h1>ChatGPT</h1>
          <p>Votre abonnement ChatGPT, dans Converge.</p>
        </div>
      </div>
      <section className="editor-section">
        <div className="chatgpt-account-row">
          <div>
            <strong>{status?.account?.email || "Votre compte ChatGPT"}</strong>
            <p>
              {status?.account
                ? `Connecté${status.account.plan ? ` · ${status.account.plan}` : ""}`
                : "Connectez-vous pour utiliser Codex avec votre abonnement ChatGPT."}
            </p>
          </div>
          {status?.account ? (
            <span className="chatgpt-connected">
              <Check size={14} />
              Connecté
            </span>
          ) : null}
        </div>
        <div className="chatgpt-auth-actions">
          {!status?.account && !login && (
            <button
              className="button primary"
              disabled={!!busy}
              onClick={() => void action("login")}
            >
              {busy === "login" ? (
                <LoaderCircle size={16} className="spin" />
              ) : (
                <ExternalLink size={16} />
              )}
              Se connecter avec ChatGPT
            </button>
          )}
          <button
            className="button"
            disabled={!!busy}
            onClick={() => void action("diagnose")}
          >
            <RefreshCw
              size={15}
              className={busy === "diagnose" ? "spin" : ""}
            />
            Actualiser
          </button>
          {status?.account && (
            <button
              className="text-button"
              disabled={!!busy}
              onClick={() => void action("logout")}
            >
              <LogOut size={15} />
              Déconnecter
            </button>
          )}
        </div>
        {login && (
          <div className="chatgpt-login" role="status">
            <LoaderCircle size={16} className="spin" />
            <span>Terminez la connexion dans votre navigateur.</span>
            <button
              className="text-button"
              disabled={!!busy}
              onClick={() => void action("cancelLogin")}
            >
              Annuler
            </button>
          </div>
        )}
        <section className="chatgpt-models">
          <div className="panel-heading">
            <h2>Modèles Codex</h2>
            <span>{status?.models?.length ?? config.models?.length ?? 0}</span>
          </div>
          {(status?.models || config.models || []).length ? (
            <>
              <p className="muted">
                Catalogue proposé par Codex. L’accès et le quota sont vérifiés
                lors de chaque réponse.
              </p>
              <div className="chatgpt-model-list">
                {(status?.models || config.models || []).map((model) => (
                  <div key={model.id}>
                    <span>
                      <strong>{model.name}</strong>
                      <small>{model.id}</small>
                    </span>
                    <span>
                      {model.vision ? "Texte et images" : "Texte"}
                      {model.efforts.length ? " · Raisonnement" : ""}
                    </span>
                  </div>
                ))}
              </div>
            </>
          ) : (
            <p className="muted">
              Les modèles apparaissent après la connexion et l’actualisation du
              compte.
            </p>
          )}
        </section>
        {!!windows.length && (
          <section className="chatgpt-limits">
            <h2>Limites d’utilisation</h2>
            {windows.map((limit, i) => (
              <div key={i}>
                <span>
                  {limit?.windowDurationMins
                    ? `Fenêtre de ${limit.windowDurationMins >= 60 ? `${Math.round(limit.windowDurationMins / 60)} h` : `${limit.windowDurationMins} min`}`
                    : `Fenêtre ${i + 1}`}
                </span>
                <strong>
                  {Math.max(0, 100 - (limit?.usedPercent || 0))}% restant
                </strong>
                {limit?.resetsAt && (
                  <small>
                    Réinitialisation{" "}
                    {new Date(limit.resetsAt * 1000).toLocaleString()}
                  </small>
                )}
              </div>
            ))}
          </section>
        )}
        {status?.account && !windows.length && (
          <p className="muted">
            Les limites d’utilisation ne sont pas exposées par cette session.
          </p>
        )}
        <div className="chatgpt-capabilities">
          <ShieldCheck size={16} />
          <p>
            Chat natif, Markdown, Converge UI, artifacts et outils avec les
            autorisations Converge. Images en entrée selon le modèle. Les images
            générées par Codex sont affichées directement lorsqu’elles sont
            renvoyées par le service.
          </p>
        </div>
        <details className="chatgpt-advanced">
          <summary>Paramètres avancés et diagnostic</summary>
          <label className="field">
            Exécutable Codex
            <input
              value={config.executable || ""}
              disabled={!!busy || !!login}
              onChange={(e) => setConfig({ executable: e.target.value })}
              placeholder="Détection automatique de Codex CLI"
            />
          </label>
          <p className="muted">
            Transport officiel : Codex App Server, JSON-RPC via stdio. Converge
            garde votre historique ; seuls les messages inclus dans le contexte
            sont transmis.
          </p>
          <p className="muted">
            Les paramètres température et longueur ne sont pas transmis à ce
            transport. Les efforts proposés proviennent du catalogue du modèle.
          </p>
          <label className="field">
            Nom de la connexion
            <input
              value={provider.name}
              onChange={(e) =>
                setProvider({ ...provider, name: e.target.value })
              }
              maxLength={120}
            />
          </label>
        </details>
      </section>
      {error && (
        <div className="inline-error" role="alert">
          {error}
        </div>
      )}
      {notice && (
        <p className="chatgpt-notice" role="status">
          {notice}
        </p>
      )}
      <div className="form-actions">
        <button className="button" disabled={!!busy} onClick={close}>
          Fermer
        </button>
        <button
          className="button primary"
          disabled={!!busy || !!login || !provider.name.trim()}
          onClick={() => void save()}
        >
          {busy === "save" ? (
            <LoaderCircle size={16} className="spin" />
          ) : (
            <Check size={16} />
          )}
          Enregistrer
        </button>
      </div>
    </div>
  );
}
