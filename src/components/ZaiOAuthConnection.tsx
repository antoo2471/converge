import { useEffect, useRef, useState } from "react";
import { ExternalLink, LoaderCircle, LogOut } from "lucide-react";
import type { Provider } from "../types";
import * as api from "../lib/native";

export function ZaiOAuthConnection({
  provider,
  onConnectionChange,
}: {
  provider: Provider;
  onConnectionChange: (connected: boolean) => void;
}) {
  const [status, setStatus] = useState<api.ZaiAuthStatus>({
    state: "disconnected",
  });
  const [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const active = useRef(true),
    login = useRef<string | undefined>(undefined),
    current = useRef(provider),
    notify = useRef(onConnectionChange);
  current.current = provider;
  notify.current = onConnectionChange;
  const apply = (result: api.ZaiAuthStatus) => {
    login.current = result.state === "pending" ? result.loginId : undefined;
    setStatus(result);
    notify.current(result.state === "connected");
  };
  useEffect(() => {
    active.current = true;
    void api
      .zaiAuthAction(current.current, "status")
      .then((result) => {
        if (active.current) apply(result);
      })
      .catch((e) => {
        if (active.current) setError(String(e));
      });
    return () => {
      active.current = false;
      if (login.current)
        void api
          .zaiAuthAction(current.current, "cancel", login.current)
          .catch(() => {});
    };
  }, []);
  useEffect(() => {
    if (status.state !== "pending") return;
    let disposed = false,
      inFlight = false;
    const timer = setInterval(async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        const result = await api.zaiAuthAction(
          current.current,
          "poll",
          status.loginId,
        );
        if (!disposed) apply(result);
      } catch (e) {
        if (!disposed) {
          setError(String(e));
          void api
            .zaiAuthAction(current.current, "cancel", status.loginId)
            .catch(() => {});
          apply({ state: "disconnected" });
        }
      } finally {
        inFlight = false;
      }
    }, 2000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, [status.state, status.loginId]);
  async function action(kind: "start" | "cancel" | "logout") {
    setBusy(true);
    setError("");
    try {
      const result = await api.zaiAuthAction(provider, kind, status.loginId);
      if (!active.current) {
        if (result.loginId)
          void api
            .zaiAuthAction(provider, "cancel", result.loginId)
            .catch(() => {});
        return;
      }
      apply(result);
      if (kind === "start" && result.authUrl)
        await api.openExternal(result.authUrl);
    } catch (e) {
      if (active.current) setError(String(e));
    } finally {
      if (active.current) setBusy(false);
    }
  }
  return (
    <section className="editor-section">
      <div className="chatgpt-account-row">
        <div>
          <strong>
            {status.state === "connected"
              ? "Coding Plan connecté"
              : "Votre compte Z.ai"}
          </strong>
          <p>
            Autorisez la connexion dans votre navigateur. Une clé Converge
            dédiée à votre Coding Plan personnel est créée et protégée dans le
            coffre du système.
          </p>
        </div>
      </div>
      <div className="toolbar">
        {status.state === "pending" ? (
          <>
            <span role="status">
              <LoaderCircle size={15} className="spin" /> En attente de votre
              autorisation…
            </span>
            <button
              className="button"
              disabled={busy}
              onClick={() => void action("cancel")}
            >
              Annuler la connexion
            </button>
            {status.authUrl && (
              <button
                className="text-button"
                onClick={() =>
                  void api
                    .openExternal(status.authUrl!)
                    .catch((e) => setError(String(e)))
                }
              >
                Rouvrir la page <ExternalLink size={14} />
              </button>
            )}
          </>
        ) : status.state === "connected" ? (
          <button
            className="button"
            disabled={busy}
            onClick={() => void action("logout")}
          >
            <LogOut size={15} /> Déconnecter le compte
          </button>
        ) : (
          <button
            className="button primary"
            disabled={busy}
            onClick={() => void action("start")}
          >
            {busy ? (
              <LoaderCircle size={15} className="spin" />
            ) : (
              <ExternalLink size={15} />
            )}{" "}
            Se connecter avec Z.ai
          </button>
        )}
      </div>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
