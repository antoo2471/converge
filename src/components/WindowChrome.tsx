import { Minus, Square, X, Layers } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { native } from "../lib/native";
export function WindowChrome() {
  return (
    <header className="window-chrome" data-tauri-drag-region>
      <div className="chrome-brand" data-tauri-drag-region>
        <Layers size={24} />
        <span data-tauri-drag-region>converge</span>
      </div>
      <div className="window-controls">
        <button
          aria-label="Réduire la fenêtre"
          onClick={() => native && getCurrentWindow().minimize()}
        >
          <Minus size={16} />
        </button>
        <button
          aria-label="Agrandir ou restaurer la fenêtre"
          onClick={() => native && getCurrentWindow().toggleMaximize()}
        >
          <Square size={13} />
        </button>
        <button
          className="window-close"
          aria-label="Fermer la fenêtre"
          onClick={() => native && getCurrentWindow().close()}
        >
          <X size={17} />
        </button>
      </div>
    </header>
  );
}
