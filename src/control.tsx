import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { Layers, Square } from "lucide-react";
import "@fontsource-variable/inter";
import "./control.css";
createRoot(document.getElementById("root")!).render(
  <main>
    <Layers size={22} />
    <div>
      <strong>Converge a le contrôle</strong>
      <span>Appuyez sur Échap pour reprendre la main</span>
    </div>
    <button
      aria-label="Arrêter le contrôle"
      onClick={() => void invoke("computer_control_stop")}
    >
      <Square size={13} fill="currentColor" />
    </button>
  </main>,
);
