import { useCallback, useEffect, useRef, useState } from "react";
import { FolderOpen, Plus, X } from "lucide-react";
import { useDialog } from "../lib/useDialog";
import * as api from "../lib/native";

export function ConversationWorkspaces({
  conversationId,
  folders,
  onChange,
  onClose,
  onError,
}: {
  conversationId: string;
  folders: string[];
  onChange: (folders: string[]) => void;
  onClose: () => void;
  onError: (error: unknown) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [busy, setBusy] = useState(false);
  const close = useCallback(() => onClose(), [onClose]);
  useDialog(true, ref, close);
  useEffect(() => {
    void api
      .workspaceContext(conversationId)
      .then((context) => onChange(context.folders))
      .catch(onError);
  }, [conversationId]);
  async function add() {
    setBusy(true);
    try {
      onChange(await api.chooseWorkspace(conversationId));
    } catch (error) {
      onError(error);
    } finally {
      setBusy(false);
    }
  }
  return (
    <div
      className="workspace-dialog-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div
        className="workspace-dialog"
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-labelledby="workspace-dialog-title"
      >
        <div className="workspace-dialog-heading">
          <FolderOpen size={20} />
          <h2 id="workspace-dialog-title">Espaces de travail</h2>
          <button
            type="button"
            className="icon-button"
            aria-label="Fermer les espaces de travail"
            onClick={close}
          >
            <X size={18} />
          </button>
        </div>
        <p>
          Les dossiers accessibles à l’IA dans cette conversation. Ils restent
          enregistrés pour la prochaine fois.
        </p>
        <div className="workspace-folder-list">
          {folders.map((folder, index) => (
            <div key={folder}>
              <FolderOpen size={17} />
              <span>
                <strong>
                  {folder.split(/[\\/]/).filter(Boolean).at(-1) || folder}
                </strong>
                <small>{folder}</small>
                {index === 0 && <em>Par défaut</em>}
              </span>
              <button
                type="button"
                className="icon-button"
                title={`Retirer ${folder}`}
                disabled={busy}
                onClick={async () => {
                  try {
                    onChange(await api.removeWorkspace(conversationId, folder));
                  } catch (error) {
                    onError(error);
                  }
                }}
              >
                <X size={15} />
              </button>
            </div>
          ))}
        </div>
        {!folders.length && (
          <div className="workspace-folder-empty">
            Ajoutez un dossier de projet, ou laissez l’IA proposer un
            emplacement quand une tâche le nécessite.
          </div>
        )}
        <button
          type="button"
          className="button primary"
          disabled={busy}
          onClick={() => void add()}
        >
          <Plus size={16} />
          {busy ? "Choix du dossier…" : "Ajouter un dossier"}
        </button>
        <small className="workspace-dialog-note">
          Retirer un espace supprime son accès pour cette conversation, sans
          supprimer ses fichiers.
        </small>
      </div>
    </div>
  );
}
