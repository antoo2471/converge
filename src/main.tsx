import React from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/inter";
import "@fontsource-variable/manrope";
import "katex/dist/katex.min.css";
import "./styles.css";
import "./redesign.css";
import App from "./App";
import { AppDialogs } from "./components/AppDialogs";
import { ErrorBoundary } from "./components/ErrorBoundary";
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <AppDialogs>
        <App />
      </AppDialogs>
    </ErrorBoundary>
  </React.StrictMode>,
);
