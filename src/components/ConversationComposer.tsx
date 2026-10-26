import { EffortPicker } from "./EffortPicker";
import type { ReasoningEffort } from "../core/reasoning";
import {
  useEffect,
  useRef,
  useState,
  type RefObject,
} from "react";
import {
  Plus,
  Paperclip,
  ClipboardPaste,
  FolderOpen,
  Columns2,
  Check,
  ArrowUp,
  Square,
  X,
} from "lucide-react";
import { ModelPicker, type ModelOption } from "./ModelPicker";
import type { Attachment } from "../types";

interface Props {
  effort?: ReasoningEffort;
  effortLevels?: readonly ReasoningEffort[];
  onEffortChange?: (value: ReasoningEffort) => void;
  value: string;
  onChange: (value: string) => void;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  models: ModelOption[];
  model: string;
  onModelChange: (value: string) => void;
  onAddProvider: () => void;
  onWorkspaces: () => void;
  workspaceFolders: string[];
  comparing: boolean;
  comparison: string;
  onComparisonChange: (value: string) => void;
  onComparingChange: (value: boolean) => void;
  attachments: Attachment[];
  onRemoveAttachment: (id: string) => void;
  onFiles: (files: File[]) => Promise<void>;
  onAttach: () => Promise<void>;
  onPasteImage: (silent?: boolean) => Promise<void>;
  onSend: () => void;
  onStop: () => void;
  busy: boolean;
  canSend: boolean;
  native: boolean;
  visionSupported: boolean;
}
export function ConversationComposer(props: Props) {
  const [menuOpen, setMenuOpen] = useState(false),
    [dragging, setDragging] = useState(false);
  const menu = useRef<HTMLDivElement>(null),
    trigger = useRef<HTMLButtonElement>(null),
    pasteHandled = useRef(false);
  useEffect(() => {
    const textarea = props.textareaRef.current;
    if (textarea) {
      textarea.style.height = "auto";
      textarea.style.height = `${Math.min(190, Math.max(48, textarea.scrollHeight))}px`;
    }
  }, [props.value, props.textareaRef]);
  useEffect(() => {
    if (!menuOpen) return;
    menu.current
      ?.querySelector<HTMLButtonElement>("button:not(:disabled)")
      ?.focus();
    const outside = (event: PointerEvent) => {
      if (
        !menu.current?.contains(event.target as Node) &&
        !trigger.current?.contains(event.target as Node)
      )
        setMenuOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [menuOpen]);
  const invoke = (action: () => void) => {
    setMenuOpen(false);
    action();
  };
  return (
    <div className="composer-area chat-composer-area">
      {props.workspaceFolders.length > 0 && (
        <button
          type="button"
          className="composer-workspace-chip"
          onClick={props.onWorkspaces}
        >
          <FolderOpen size={13} />
          <span>
            {props.workspaceFolders[0].split(/[\\/]/).filter(Boolean).at(-1)}
          </span>
          {props.workspaceFolders.length > 1 && (
            <small>+{props.workspaceFolders.length - 1}</small>
          )}
        </button>
      )}
      <div
        className={`chat-composer ${dragging ? "is-dragging" : ""}`}
        onPaste={(event) => {
          const files = Array.from(event.clipboardData.items)
            .filter((item) => item.kind === "file")
            .map((item) => item.getAsFile())
            .filter((file): file is File => !!file);
          pasteHandled.current = true;
          if (files.length) {
            event.preventDefault();
            void props.onFiles(files);
          } else if (
            !event.clipboardData.getData("text/plain") &&
            props.native
          ) {
            event.preventDefault();
            void props.onPasteImage(true);
          }
        }}
        onDragOver={(event) => {
          if (event.dataTransfer.types.includes("Files")) {
            event.preventDefault();
            setDragging(true);
          }
        }}
        onDragLeave={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node))
            setDragging(false);
        }}
        onDrop={(event) => {
          event.preventDefault();
          setDragging(false);
          void props.onFiles(Array.from(event.dataTransfer.files));
        }}
      >
        {!!props.attachments.length && (
          <div className="chat-attachments">
            {props.attachments.map((attachment) => (
              <div className="chat-attachment" key={attachment.id}>
                {attachment.mime.startsWith("image/") ? (
                  <img
                    src={`data:${attachment.mime};base64,${attachment.data}`}
                    alt={attachment.name}
                  />
                ) : (
                  <Paperclip size={19} />
                )}
                <span>{attachment.name}</span>
                <button
                  aria-label={`Retirer ${attachment.name}`}
                  title="Retirer la pièce jointe"
                  onClick={() => props.onRemoveAttachment(attachment.id)}
                >
                  <X size={13} />
                </button>
              </div>
            ))}
          </div>
        )}
        {props.comparing && (
          <div className="chat-comparison">
            <Columns2 size={15} />
            <span>Comparer avec</span>
            <ModelPicker
              compact
              options={props.models.filter(
                (model) => model.value !== props.model,
              )}
              value={props.comparison}
              onChange={props.onComparisonChange}
              onAdd={props.onAddProvider}
              label="Deuxième modèle"
            />
            <button
              className="icon-button"
              aria-label="Arrêter la comparaison"
              onClick={() => props.onComparingChange(false)}
            >
              <X size={14} />
            </button>
          </div>
        )}
        <textarea
          ref={props.textareaRef}
          aria-label="Votre message"
          rows={1}
          placeholder="Écrivez votre message…"
          value={props.value}
          onChange={(event) => props.onChange(event.target.value)}
          onKeyDown={(event) => {
            if (
              (event.ctrlKey || event.metaKey) &&
              event.key.toLowerCase() === "v"
            ) {
              pasteHandled.current = false;
              setTimeout(() => {
                if (!pasteHandled.current && props.native)
                  void props.onPasteImage(true);
              }, 60);
            }
            if (
              event.key === "Enter" &&
              !event.shiftKey &&
              !event.nativeEvent.isComposing
            ) {
              event.preventDefault();
              if (props.canSend && !props.busy) props.onSend();
            }
          }}
        />
        {props.attachments.some((a) => a.mime.startsWith("image/")) &&
          !props.visionSupported && (
            <div className="chat-attachment-warning">
              <span>La vision est désactivée pour ce fournisseur.</span>
              <button onClick={props.onAddProvider}>Configurer</button>
            </div>
          )}
        <div className="chat-composer-toolbar">
          <div className="chat-composer-more">
            <button
              ref={trigger}
              className={`composer-plus ${menuOpen ? "open" : ""}`}
              aria-label="Ajouter au message"
              title="Ajouter au message"
              aria-expanded={menuOpen}
              aria-haspopup="menu"
              onClick={() => setMenuOpen((value) => !value)}
            >
              <Plus size={21} />
            </button>
            {menuOpen && (
              <div
                className="composer-more-menu"
                ref={menu}
                role="menu"
                aria-label="Options du message"
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    event.stopPropagation();
                    setMenuOpen(false);
                    trigger.current?.focus();
                  }
                  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                    event.preventDefault();
                    const buttons = Array.from(
                      menu.current?.querySelectorAll<HTMLButtonElement>(
                        "button:not(:disabled)",
                      ) || [],
                    );
                    const index = buttons.indexOf(
                      document.activeElement as HTMLButtonElement,
                    );
                    buttons[
                      (index +
                        (event.key === "ArrowDown" ? 1 : -1) +
                        buttons.length) %
                        buttons.length
                    ]?.focus();
                  }
                }}
              >
                <button
                  role="menuitem"
                  disabled={!props.native}
                  onClick={() => invoke(() => void props.onAttach())}
                >
                  <Paperclip size={17} />
                  <span>Joindre des fichiers</span>
                </button>
                <button
                  role="menuitem"
                  disabled={!props.native}
                  onClick={() => invoke(() => void props.onPasteImage())}
                >
                  <ClipboardPaste size={17} />
                  <span>Coller depuis le presse-papiers</span>
                  <kbd>Ctrl V</kbd>
                </button>
                <button
                  role="menuitem"
                  disabled={!props.native}
                  onClick={() => invoke(props.onWorkspaces)}
                >
                  <FolderOpen size={17} />
                  <span>Espaces de travail</span>
                  {props.workspaceFolders.length > 0 && (
                    <small>{props.workspaceFolders.length}</small>
                  )}
                </button>
                <div className="composer-menu-divider" />
                <button
                  role="menuitemcheckbox"
                  aria-checked={props.comparing}
                  disabled={props.models.length < 2}
                  title={
                    props.models.length < 2
                      ? "Ajoutez au moins deux modèles pour comparer"
                      : undefined
                  }
                  onClick={() =>
                    invoke(() => props.onComparingChange(!props.comparing))
                  }
                >
                  <Columns2 size={17} />
                  <span>Comparer des modèles</span>
                  {props.comparing && <Check size={15} />}
                </button>
              </div>
            )}
          </div>
          <div className="chat-composer-spacer" />
          <ModelPicker
            compact
            options={props.models}
            value={props.model}
            onChange={props.onModelChange}
            onAdd={props.onAddProvider}
            label="Choisir un modèle"
          />
          {props.onEffortChange && (
            <EffortPicker
              value={props.effort || "auto"}
              levels={props.effortLevels || []}
              onChange={props.onEffortChange}
            />
          )}
          {props.busy ? (
            <button
              className="chat-send stop"
              title="Arrêter la réponse"
              aria-label="Arrêter la réponse"
              onClick={props.onStop}
            >
              <Square size={14} fill="currentColor" />
            </button>
          ) : (
            <button
              className="chat-send"
              title="Envoyer"
              aria-label="Envoyer"
              disabled={!props.canSend}
              onClick={props.onSend}
            >
              <ArrowUp size={19} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
