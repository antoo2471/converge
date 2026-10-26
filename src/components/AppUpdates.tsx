import { useEffect, useSyncExternalStore } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { ArrowDownToLine, LoaderCircle, RefreshCw, X } from "lucide-react";
import { native } from "../lib/native";
import manifest from "../../package.json";
type UpdateState = {
  phase: "idle" | "checking" | "available" | "installing" | "current" | "error";
  version?: string;
  error?: string;
  downloaded?: number;
  total?: number;
  dismissed?: boolean;
};
let state: UpdateState = { phase: "idle" };
const subscribers = new Set<() => void>();
const publish = (next: UpdateState) => {
  state = next;
  for (const listener of subscribers) listener();
};
function useUpdate() {
  return useSyncExternalStore(
    (listener) => {
      subscribers.add(listener);
      return () => {
        subscribers.delete(listener);
      };
    },
    () => state,
  );
}
async function check(quiet = false) {
  if (!native || state.phase === "installing" || state.phase === "checking")
    return;
  if (!quiet) publish({ phase: "checking" });
  try {
    const update = await invoke<{ version: string } | null>("app_update_check");
    publish(
      update
        ? { phase: "available", version: update.version }
        : { phase: "current" },
    );
  } catch (e) {
    if (!quiet) publish({ phase: "error", error: String(e) });
  }
}
async function install() {
  const version = state.version;
  if (!version || state.phase === "installing") return;
  publish({ phase: "installing", version });
  const channel = new Channel<{ downloaded: number; total?: number }>();
  channel.onmessage = (event) =>
    publish({ phase: "installing", version, ...event });
  try {
    await invoke("app_update_install", { version, onEvent: channel });
  } catch (e) {
    publish({ phase: "error", version, error: String(e) });
  }
}
export function AppUpdateNotice({ busy }: { busy: boolean }) {
  const update = useUpdate();
  useEffect(() => {
    if (!native || !import.meta.env.PROD) return;
    const start = setTimeout(() => void check(true), 10000);
    const timer = setInterval(() => void check(true), 6 * 60 * 60 * 1000);
    return () => {
      clearTimeout(start);
      clearInterval(timer);
    };
  }, []);
  if (update.dismissed || !["available", "installing"].includes(update.phase))
    return null;
  return (
    <aside className="app-update-notice" role="status">
      <ArrowDownToLine size={18} />
      <div>
        <strong>Converge {update.version}</strong>
        <p>
          {update.phase === "installing"
            ? "Installation de la mise à jour signée…"
            : busy
              ? "La mise à jour sera disponible après votre tâche."
              : "Une mise à jour est disponible."}
        </p>
      </div>
      <button
        className="button"
        disabled={busy || update.phase === "installing"}
        onClick={() => void install()}
      >
        {update.phase === "installing" ? (
          <LoaderCircle size={15} className="spin" />
        ) : (
          "Installer et relancer"
        )}
      </button>
      {update.phase !== "installing" && (
        <button
          className="icon-button"
          aria-label="Masquer la mise à jour"
          onClick={() => publish({ ...state, dismissed: true })}
        >
          <X size={15} />
        </button>
      )}
    </aside>
  );
}
export function UpdateSettings({ busy = false }: { busy?: boolean }) {
  const update = useUpdate();
  return (
    <section className="settings-section update-settings">
      <h2>Converge {manifest.version}</h2>
      <p>
        La disponibilité des nouvelles versions est vérifiée automatiquement
        auprès du dépôt officiel. Les téléchargements doivent porter une
        signature valide.
      </p>
      <div className="toolbar">
        <button
          className="button"
          disabled={
            !native ||
            update.phase === "checking" ||
            update.phase === "installing"
          }
          onClick={() => void check()}
        >
          <RefreshCw
            size={16}
            className={update.phase === "checking" ? "spin" : ""}
          />
          Vérifier les mises à jour
        </button>
        {update.version && (
          <button
            className="button primary"
            disabled={busy || update.phase === "installing"}
            onClick={() => void install()}
          >
            Installer {update.version}
          </button>
        )}
      </div>
      {busy && update.version && (
        <p role="status">
          Terminez la tâche en cours avant d’installer la mise à jour.
        </p>
      )}
      {update.phase === "current" && (
        <p role="status">Vous utilisez la dernière version disponible.</p>
      )}
      {update.phase === "installing" && (
        <p role="status">
          Téléchargement et installation…{" "}
          {update.total
            ? `${Math.round(((update.downloaded || 0) / update.total) * 100)} %`
            : ""}
        </p>
      )}
      {update.error && (
        <p role="alert" className="error">
          {update.error}
        </p>
      )}
      <p className="muted">Source : github.com/antoo2471/converge</p>
    </section>
  );
}
