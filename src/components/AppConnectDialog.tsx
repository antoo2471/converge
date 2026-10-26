import { GitHubOAuthSetup } from "./GitHubOAuthSetup";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  X,
  LoaderCircle,
  ArrowUpRight,
  Check,
  ShieldCheck,
  Globe,
  Layers,
} from "lucide-react";
import { useDialog } from "../lib/useDialog";
import type { AppEntry } from "../core/appCatalogue";
import * as api from "../lib/extensions";
import { GoogleOAuthSetup, googleSetupFor } from "./GoogleOAuthSetup";
import { openExternal } from "../lib/native";

export function AppConnectDialog({
  app,
  mark,
  onClose,
  onConnected,
}: {
  app: AppEntry;
  mark: React.ReactNode;
  onClose: () => void;
  onConnected: () => void;
}) {
  const google = googleSetupFor(app);
  const github = app.id === "github";
  const guided = !!google || github;
  const [googleStep, setGoogleStep] = useState(0);
  const [url, setUrl] = useState(app.url || ""),
    [clientId, setClientId] = useState(""),
    [clientSecret, setClientSecret] = useState("");
  const [connectors, setConnectors] = useState<api.RegistryServer[]>([]),
    [publisher, setPublisher] = useState(
      app.url ? new URL(app.url).hostname : "",
    ),
    [registryLoading, setRegistryLoading] = useState(!app.url),
    [registryError, setRegistryError] = useState("");
  const [phase, setPhase] = useState<
      "idle" | "starting" | "pending" | "connected"
    >("idle"),
    [error, setError] = useState(""),
    [toolCount, setToolCount] = useState(0),
    [search, setSearch] = useState(app.query || app.name);
  const ref = useRef<HTMLDivElement>(null),
    alive = useRef(true),
    flow = useRef<{ id: string; loginId?: string } | null>(null),
    config = useRef(
      api.newMcp({
        name: app.name,
        auth: "oauth",
        scopes: app.scopes || [],
        redirectPort: app.redirectPort,
      }),
    ),
    searchVersion = useRef(0),
    notify = useRef(onConnected);
  notify.current = onConnected;
  const close = useCallback(() => {
    const active = flow.current;
    if (active?.loginId) void api.mcpAction("cancel", active).catch(() => {});
    flow.current = null;
    onClose();
  }, [onClose]);
  useDialog(true, ref, close);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      searchVersion.current++;
      const active = flow.current;
      if (active?.loginId) void api.mcpAction("cancel", active).catch(() => {});
    };
  }, []);
  async function loadConnectors(query: string) {
    const version = ++searchVersion.current;
    setRegistryLoading(true);
    setRegistryError("");
    try {
      const result = await api.catalogue(query);
      if (!alive.current || version !== searchVersion.current) return;
      setConnectors(
        result.servers
          .map((s) => s.server)
          .filter((s) => s?.remotes?.some((r) => r.type === "streamable-http")),
      );
    } catch (e) {
      if (alive.current && version === searchVersion.current)
        setRegistryError(String(e));
    } finally {
      if (alive.current && version === searchVersion.current)
        setRegistryLoading(false);
    }
  }
  useEffect(() => {
    if (app.url) return;
    const timer = setTimeout(() => void loadConnectors(search), 300);
    return () => {
      clearTimeout(timer);
      searchVersion.current++;
    };
  }, [search, app.url]);
  useEffect(() => {
    if (phase !== "pending") return;
    let pending = false;
    const timer = setInterval(async () => {
      if (pending || !flow.current) return;
      pending = true;
      try {
        const result = await api.snapshot();
        if (!alive.current) return;
        const server = result.servers.find((s) => s.id === flow.current?.id);
        if (server?.connected) {
          flow.current = null;
          setToolCount(server.toolCount);
          setPhase("connected");
          notify.current();
        } else if (server?.error) {
          setError(server.error);
          setPhase("idle");
          flow.current = null;
          notify.current();
        }
      } catch (e) {
        if (alive.current) setError(String(e));
      } finally {
        pending = false;
      }
    }, 1600);
    return () => clearInterval(timer);
  }, [phase]);
  async function connect() {
    setPhase("starting");
    setError("");
    try {
      const c = {
        ...config.current,
        url,
        clientId: clientId.trim(),
        auth: "oauth" as const,
      };
      await api.mcpAction("save", {
        config: c,
        ...(clientSecret ? { secret: { client_secret: clientSecret } } : {}),
      });
      const started = await api.mcpAction("authorize", { id: c.id });
      if (!alive.current) {
        if (started?.loginId)
          await api.mcpAction("cancel", { id: c.id, loginId: started.loginId });
        return;
      }
      if (!started?.loginId)
        throw Error("Le service n’a pas créé de demande OAuth.");
      flow.current = { id: c.id, loginId: started.loginId };
      setPhase("pending");
      notify.current();
      await api.mcpAction("openAuth", flow.current);
    } catch (e) {
      if (alive.current) {
        setError(String(e));
        setPhase("idle");
        notify.current();
      }
    }
  }
  function choose(server: api.RegistryServer) {
    const remote = server.remotes?.find((r) => r.type === "streamable-http");
    if (!remote) return;
    setUrl(remote.url);
    setPublisher(server.name);
    setError("");
  }
  const locked = phase === "starting" || phase === "pending";
  return createPortal(
    <div
      className="app-dialog-backdrop app-connect-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div
        ref={ref}
        className={`app-dialog app-connect-dialog${guided ? " app-connect-google" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="app-connect-title"
      >
        <button
          className="icon-button app-connect-close"
          aria-label="Fermer la connexion"
          onClick={close}
        >
          <X size={17} />
        </button>
        <div className="app-connect-marks">
          <span className="app-connect-converge">
            <Layers size={29} aria-hidden="true" />
          </span>
          <span className="app-connect-dots" aria-hidden="true">
            <i />
            <i />
            <i />
          </span>
          {mark}
        </div>
        <h2 id="app-connect-title">
          {phase === "connected"
            ? `${app.name} est connecté`
            : `Connecter ${app.name}`}
        </h2>
        <p className="app-connect-provider">
          {publisher
            ? `Connexion via ${publisher}`
            : "Choisissez le connecteur qui accède à votre app."}
        </p>
        <div className={guided ? "google-connect-body" : undefined}>
          {phase === "connected" ? (
            <div className="app-connect-success">
              <Check size={24} />
              <p>
                {toolCount} outil{toolCount > 1 ? "s" : ""} disponible
                {toolCount > 1 ? "s" : ""} dans vos conversations.
              </p>
            </div>
          ) : guided ? (
            <>
              {phase === "pending" || phase === "starting" ? (
                <div className="google-connect-pending" role="status">
                  <LoaderCircle size={25} className="spin" />
                  <h3>
                    {phase === "starting"
                      ? "Préparation de la connexion…"
                      : "Continuez dans votre navigateur"}
                  </h3>
                  <p>
                    Choisissez votre compte Google et vérifiez les accès
                    demandés. Converge vérifiera ensuite la connexion à{" "}
                    {app.name}.
                  </p>
                  {phase === "pending" && (
                    <button
                      className="text-button"
                      onClick={() => {
                        if (flow.current)
                          void api
                            .mcpAction("openAuth", flow.current)
                            .catch((e) => setError(String(e)));
                      }}
                    >
                      Rouvrir le navigateur <ArrowUpRight size={14} />
                    </button>
                  )}
                </div>
              ) : github ? (
                <GitHubOAuthSetup
                  step={googleStep}
                  clientId={clientId}
                  clientSecret={clientSecret}
                  onClientId={setClientId}
                  onClientSecret={setClientSecret}
                  onError={setError}
                />
              ) : (
                <GoogleOAuthSetup
                  app={app}
                  setup={google!}
                  step={googleStep}
                  clientId={clientId}
                  clientSecret={clientSecret}
                  onClientId={setClientId}
                  onClientSecret={setClientSecret}
                  onError={setError}
                />
              )}
            </>
          ) : (
            <>
              {!app.url && (
                <section className="app-connect-connectors">
                  <label className="field">
                    Connecteur MCP
                    <input
                      aria-label="Rechercher un connecteur"
                      value={search}
                      disabled={locked}
                      onChange={(e) => setSearch(e.target.value)}
                    />
                  </label>
                  {registryLoading ? (
                    <p role="status">
                      <LoaderCircle className="spin" size={15} />
                      Recherche des connecteurs…
                    </p>
                  ) : connectors.length > 0 ? (
                    <div
                      className="app-connect-options"
                      role="radiogroup"
                      aria-label="Connecteurs disponibles"
                    >
                      {connectors.map((server) => (
                        <label key={server.name}>
                          <input
                            type="radio"
                            name="mcp-connector"
                            checked={publisher === server.name}
                            disabled={locked}
                            onChange={() => choose(server)}
                          />
                          <span>
                            <strong>{server.title || server.name}</strong>
                            <small>Éditeur déclaré : {server.name}</small>
                          </span>
                        </label>
                      ))}
                    </div>
                  ) : (
                    <p className="muted">
                      Aucun serveur MCP HTTP identifié pour cette app dans le
                      registre. Vous pouvez renseigner un endpoint OAuth
                      compatible ci-dessous.
                    </p>
                  )}
                  {registryError && (
                    <p className="error" role="alert">
                      {registryError}
                    </p>
                  )}
                </section>
              )}
              <div className="app-connect-copy">
                <p>
                  Les outils de cette app recevront les arguments envoyés par
                  votre modèle. Vous pourrez les vérifier dans les demandes
                  d’autorisation, puis autoriser ou refuser l’action.
                </p>
                <p>
                  Votre connexion est conservée dans le coffre sécurisé du
                  système. Vous pouvez la retirer à tout moment depuis Mes
                  plugins.
                </p>
                {app.note && <p>{app.note}</p>}
                {publisher && app.url === undefined && (
                  <p>
                    Ce connecteur est un service distinct. Vérifiez son éditeur
                    et sa politique de confidentialité avant d’autoriser l’accès
                    à votre compte.
                  </p>
                )}
              </div>
              <details className="app-connect-advanced">
                <summary>Configuration OAuth</summary>
                {(app.redirectPort || clientId) && (
                  <label className="field">
                    URL de retour à enregistrer dans votre application OAuth
                    <input
                      readOnly
                      value={`http://127.0.0.1:${app.redirectPort || 32145}/oauth/callback`}
                    />
                  </label>
                )}
                <label className="field">
                  Endpoint MCP
                  <input
                    value={url}
                    disabled={locked}
                    placeholder="https://…/mcp"
                    onChange={(e) => {
                      setUrl(e.target.value);
                      setPublisher("");
                    }}
                  />
                </label>
                <label className="field">
                  ID de votre application OAuth
                  <input
                    value={clientId}
                    disabled={locked}
                    placeholder="Si le service exige un client enregistré"
                    onChange={(e) => setClientId(e.target.value)}
                  />
                </label>
                <label className="field">
                  Secret client OAuth · facultatif
                  <input
                    type="password"
                    autoComplete="off"
                    disabled={locked}
                    value={clientSecret}
                    onChange={(e) => setClientSecret(e.target.value)}
                  />
                </label>
              </details>
              {phase === "pending" && (
                <p className="app-connect-wait" role="status">
                  <LoaderCircle size={16} className="spin" />
                  Autorisez Converge dans votre navigateur.
                  <button
                    className="text-button"
                    onClick={() => {
                      if (flow.current)
                        void api
                          .mcpAction("openAuth", flow.current)
                          .catch((e) => setError(String(e)));
                    }}
                  >
                    Rouvrir
                    <ArrowUpRight size={13} />
                  </button>
                </p>
              )}
              {error && (
                <p className="error" role="alert">
                  {error}
                </p>
              )}
              <button
                className="text-button app-connect-docs"
                onClick={() =>
                  void openExternal(app.docs).catch((e) => setError(String(e)))
                }
              >
                <Globe size={13} />
                Documentation
                <ArrowUpRight size={13} />
              </button>
            </>
          )}
        </div>
        <div className="app-connect-actions">
          {guided && error && phase !== "connected" && (
            <p className="error" role="alert">
              {error}
            </p>
          )}
          {phase === "connected" ? (
            <button className="button primary" onClick={close}>
              <Check size={16} />
              Terminé
            </button>
          ) : guided ? (
            <>
              {!locked && (
                <button
                  className="button primary"
                  onClick={() => {
                    setError("");
                    if (
                      googleStep === 1 &&
                      (!clientId.trim() ||
                        (google &&
                          !clientId
                            .trim()
                            .endsWith(".apps.googleusercontent.com")) ||
                        !clientSecret.trim())
                    ) {
                      setError(
                        github
                          ? "Renseignez le client ID et le secret de votre application OAuth GitHub."
                          : "Renseignez le client ID Google complet et le secret de votre application Web.",
                      );
                      return;
                    }
                    if (googleStep < 2) setGoogleStep(googleStep + 1);
                    else void connect();
                  }}
                >
                  {googleStep === 0
                    ? github
                      ? "Configurer mon client GitHub"
                      : "Configurer mon client Google"
                    : googleStep === 1
                      ? "Continuer"
                      : `Se connecter avec ${github ? "GitHub" : "Google"}`}
                  <ArrowUpRight size={15} />
                </button>
              )}
              {!locked && googleStep > 0 && (
                <button
                  className="button"
                  onClick={() => {
                    setError("");
                    setGoogleStep(googleStep - 1);
                  }}
                >
                  Retour
                </button>
              )}
              <button className="text-button" onClick={close}>
                Annuler
              </button>
            </>
          ) : (
            <>
              <button
                className="button primary"
                disabled={!url.trim() || locked}
                onClick={() => void connect()}
              >
                {locked ? (
                  <LoaderCircle className="spin" size={16} />
                ) : (
                  <ShieldCheck size={16} />
                )}
                Connecter {app.name}
                <ArrowUpRight size={15} />
              </button>
              <button className="button" onClick={close}>
                Annuler
              </button>
            </>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}
