import { toolObservation } from "../core/toolObservation";
import { useState } from "react";
import {
  Check,
  X,
  LoaderCircle,
  Shield,
  FolderOpen,
  Square,
} from "lucide-react";
import type { ToolCall, ToolPermission } from "../types";
import { toolGroups } from "../core/toolPreferences";
import * as api from "../lib/native";
const labels = new Map<string, string>(
  toolGroups.flatMap((group) =>
    group.tools.map(([name, label]) => [name, label] as [string, string]),
  ),
);

export function ToolActivity({
  call,
  permission,
  onDecision,
  onError,
  onCancel,
}: {
  call: ToolCall;
  permission?: ToolPermission;
  onDecision: (
    permission: ToolPermission,
    decision: "allow" | "deny" | "conversation",
    root: string | null,
  ) => Promise<void>;
  onError: (error: unknown) => void;
  onCancel?: () => void;
}) {
  const [selection, setSelection] = useState<{ permissionId: string; path: string }>();
  const root = selection?.permissionId === permission?.id
    ? selection?.path || permission?.root || null
    : permission?.root || null;
  const [busy, setBusy] = useState(false);
  const active =
    call.status === "preparing" ||
    call.status === "running" ||
    call.status === "awaiting";
  const args = permission?.args || call.args;
  const observation = toolObservation(call.result);
  let searchResult: { engine: string; results: { title: string; url: string }[]; warnings?: string[] } | undefined;
  if (call.name === "web.search" || call.name === "web.images") {
    try { const value = JSON.parse(call.result || ""); if (typeof value.engine === "string" && Array.isArray(value.results)) searchResult = value; } catch { /* Failed search output remains visible below. */ }
  }
  const target = String(
    args.path || args.command || args.url || args.query || "",
  );
  const status = {
    preparing: "Rédaction…",
    awaiting: permission ? "Votre autorisation est requise" : "Préparation…",
    running: call.changes?.exact ? "Écriture…" : call.phase || "En cours…",
    succeeded: "Terminé",
    failed: "Échec",
    denied: "Refusé",
    cancelled: "Annulé",
  }[call.status];
  async function decide(decision: "allow" | "deny" | "conversation") {
    if (!permission) return;
    setBusy(true);
    try {
      await onDecision(permission, decision, root);
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }
  return (
    <section
      className={`agent-tool agent-tool-${call.status}`}
      aria-label={labels.get(call.name) || call.name}
    >
      <div className="agent-tool-heading">
        {call.status === "succeeded" ? (
          <Check size={15} />
        ) : active && !permission ? (
          <LoaderCircle size={15} className="spin" />
        ) : call.status === "denied" || call.status === "failed" ? (
          <X size={15} />
        ) : (
          <Shield size={15} />
        )}
        <strong>
          {permission?.name === "terminal.apply_changes"
            ? "Appliquer les modifications du terminal"
            : labels.get(call.name) || call.name}
        </strong>
        <span role="status">{status}</span>
        {active && (
          <button
            type="button"
            className="icon-button"
            title="Annuler cette action"
            onClick={() =>
              onCancel ? onCancel() : void api.cancel(call.id).catch(onError)
            }
          >
            <Square size={12} />
          </button>
        )}
      </div>
      {target && <div className="agent-tool-target">{target}</div>}
      {call.changes && (
        <div
          className={`agent-file-stats ${active ? "is-active" : ""}`}
          aria-label="Modifications du fichier"
        >
          <span className="file-added" key={`add-${call.changes.added}`}>
            +{call.changes.added}
          </span>
          {call.changes.exact && (
            <span
              className="file-removed"
              key={`remove-${call.changes.removed}`}
            >
              −{call.changes.removed}
            </span>
          )}
          <small>{call.changes.exact ? "lignes" : "lignes préparées"}</small>
          {active && <span className="file-writing-pulse" />}
        </div>
      )}
      {call.writeProgress && call.status === "running" && (
        <progress
          className="agent-file-progress"
          max={call.writeProgress.total || 1}
          value={call.writeProgress.written}
          aria-label="Écriture du fichier"
        />
      )}
      {call.changes?.diff && (
        <details className="agent-file-diff">
          <summary>Voir le diff</summary>
          <pre>
            {call.changes.diff.split("\n").map((line, index) => (
              <span
                key={index}
                className={
                  line.startsWith("+") && !line.startsWith("+++")
                    ? "diff-added"
                    : line.startsWith("-") && !line.startsWith("---")
                      ? "diff-removed"
                      : ""
                }
              >
                {line}
                {"\n"}
              </span>
            ))}
          </pre>
        </details>
      )}

      {permission && typeof args.content === "string" && (
        <pre className="agent-tool-change-preview">
          {args.content.slice(0, 2000)}
          {args.content.length > 2000 ? "\n…" : ""}
        </pre>
      )}
      {permission && (
        <div className="agent-tool-approval">
          {call.name.startsWith("computer.") && call.name !== "computer.windows" && (
            <p>
              Cette capture de votre écran sera transmise au modèle de la
              conversation.
            </p>
          )}
          {permission.addWorkspace && (
            <p>
              Autoriser l’IA à travailler à cet emplacement l’ajoutera aux
              espaces de travail de cette conversation.
            </p>
          )}
          {permission.requiresRoot && (
            <div className="agent-tool-folder">
              <span>
                {root ||
                  "Choisissez un espace de travail pour cette conversation."}
              </span>
              <button
                type="button"
                className="text-button"
                disabled={busy}
                onClick={async () => {
                  try {
                    const choice = await api.pickWorkspace(
                      permission.conversationId,
                    );
                    if (choice.selected) setSelection({ permissionId: permission.id, path: choice.selected });
                  } catch (error) {
                    onError(error);
                  }
                }}
              >
                <FolderOpen size={13} />
                {root ? "Changer" : "Choisir un dossier"}
              </button>
            </div>
          )}
          <div className="agent-tool-decisions">
            <button
              type="button"
              className="button primary"
              disabled={busy || (permission.requiresRoot && !root)}
              onClick={() => void decide("allow")}
            >
              Autoriser
            </button>
            <button
              type="button"
              className="button"
              disabled={busy}
              onClick={() => void decide("deny")}
            >
              Refuser
            </button>
            <button
              type="button"
              className="text-button"
              disabled={busy || (permission.requiresRoot && !root)}
              onClick={() => void decide("conversation")}
            >
              Tout autoriser dans cette conversation
            </button>
          </div>
          <small>
            L’autorisation de conversation expire à la fermeture de
            l’application. Les limites des espaces de travail restent actives.
          </small>
        </div>
      )}
      {!call.changes?.diff && (
        <details className="agent-tool-details">
          <summary>
            {call.name === "filesystem.write" || call.name === "coding.patch"
              ? "Voir les modifications demandées"
              : "Détails de l’action"}
          </summary>
          {call.name === "filesystem.write" ? (
            <pre>{String(args.content || "")}</pre>
          ) : call.name === "coding.patch" ? (
            <div>
              <pre className="diff-removed">{String(args.search || "")}</pre>
              <pre className="diff-added">{String(args.replace || "")}</pre>
            </div>
          ) : (
            <pre>{JSON.stringify(args, null, 2)}</pre>
          )}
        </details>
      )}
      {call.output && <pre className="agent-tool-output">{call.output}</pre>}
      {call.result &&
        (searchResult ? (
          <div className="agent-web-results">
            <small>{searchResult.engine} · {searchResult.results.length} résultat(s)</small>
            {searchResult.results.slice(0, 5).map((item, index) => (
              <button className="text-button" key={`${item.url}-${index}`} type="button" onClick={() => void api.openExternal(item.url).catch(onError)}>
                <span>{item.title || item.url}</span><small>{(() => { try { return new URL(item.url).hostname; } catch { return ""; } })()}</small>
              </button>
            ))}
            {searchResult.warnings?.map((warning, i) => <small key={i}>{warning}</small>)}
          </div>
        ) : observation.image ? (
          <img
            className="agent-tool-image"
            src={observation.image}
            alt="Capture réalisée par l’outil"
          />
        ) : (
          <details
            className="agent-tool-details"
            open={call.status === "failed" || call.status === "denied"}
          >
            <summary>Résultat</summary>
            <pre>{observation.text}</pre>
          </details>
        ))}
    </section>
  );
}
