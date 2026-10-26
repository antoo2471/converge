import { useEffect, useRef, useState } from "react";
import { ArrowUpRight, Check, Copy } from "lucide-react";
import type { AppEntry } from "../core/appCatalogue";
import { openExternal } from "../lib/native";

const services: Record<string, [string, string]> = {
  gmail: ["gmail.googleapis.com", "gmailmcp.googleapis.com"],
  "google-drive": ["drive.googleapis.com", "drivemcp.googleapis.com"],
  "google-docs": ["docs.googleapis.com", "docsmcp.googleapis.com"],
  "google-sheets": ["sheets.googleapis.com", "sheetsmcp.googleapis.com"],
  "google-slides": ["slides.googleapis.com", "slidesmcp.googleapis.com"],
  "google-calendar": [
    "calendar-json.googleapis.com",
    "calendarmcp.googleapis.com",
  ],
  "google-chat": ["chat.googleapis.com", "chatmcp.googleapis.com"],
  "google-contacts": ["people.googleapis.com", "people.googleapis.com"],
};
export function googleSetupFor(app: AppEntry) {
  return app.url && app.redirectPort && services[app.id]
    ? services[app.id]
    : null;
}

export function GoogleOAuthSetup({
  app,
  setup,
  step,
  clientId,
  clientSecret,
  onClientId,
  onClientSecret,
  onError,
}: {
  app: AppEntry;
  setup: [string, string];
  step: number;
  clientId: string;
  clientSecret: string;
  onClientId: (value: string) => void;
  onClientSecret: (value: string) => void;
  onError: (error: string) => void;
}) {
  const title = useRef<HTMLHeadingElement>(null);
  const [copied, setCopied] = useState("");
  const callback = `http://127.0.0.1:${app.redirectPort}/oauth/callback`;
  useEffect(() => {
    title.current?.focus({ preventScroll: true });
    const body = title.current?.closest(".google-connect-body");
    if (body) body.scrollTop = 0;
  }, [step]);
  const link = (label: string, url: string) => (
    <button
      className="text-button google-setup-link"
      onClick={() => void openExternal(url).catch((e) => onError(String(e)))}
    >
      {label}
      <ArrowUpRight size={13} />
    </button>
  );
  const copy = async (value: string, name: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(name);
    } catch {
      onError(
        "La copie a échoué. Sélectionnez le texte pour le copier manuellement.",
      );
    }
  };
  return (
    <section className="google-setup">
      <ol
        className="google-setup-progress"
        aria-label="Étapes de connexion Google"
      >
        {["Préparer", "Configurer", "Se connecter"].map((name, index) => (
          <li
            key={name}
            aria-current={index === step ? "step" : undefined}
            data-complete={index < step}
          >
            {index < step ? <Check size={12} /> : <span>{index + 1}</span>}
            {name}
          </li>
        ))}
      </ol>
      <h3 ref={title} tabIndex={-1}>
        {
          [
            "Préparez votre projet Google",
            "Ajoutez votre client OAuth",
            "Tout est prêt pour vous connecter",
          ][step]
        }
      </h3>
      {step === 0 ? (
        <>
          <p>
            Google demande une configuration initiale pour ses MCP officiels.
            Elle se fait dans votre propre projet Google Cloud.
          </p>
          <ol className="google-setup-instructions">
            <li>
              <strong>Choisissez ou créez un projet.</strong>
              <p>
                Dans ce projet, activez l’API {app.name} et son service MCP.
              </p>
              {link(
                "Ouvrir Google Cloud",
                "https://console.cloud.google.com/projectselector2/home/dashboard",
              )}
              {link(
                "Activer l’API",
                `https://console.cloud.google.com/apis/library/${setup[0]}`,
              )}
              {setup[0] !== setup[1] &&
                link(
                  "Activer le service MCP",
                  `https://console.cloud.google.com/apis/library/${setup[1]}`,
                )}
            </li>
            <li>
              <strong>Préparez l’écran de consentement.</strong>
              <p>
                Dans Google Auth Platform, configurez Branding et Audience. En
                mode Externe / Test, ajoutez votre adresse aux utilisateurs
                test. Dans Accès aux données, ajoutez les autorisations listées
                ci-dessous.
              </p>
              {link(
                "Ouvrir Google Auth Platform",
                "https://console.cloud.google.com/auth/overview",
              )}
            </li>
          </ol>
          {app.id === "google-chat" && (
            <p>
              Google Chat nécessite aussi une app Chat configurée dans ce
              projet. Le guide Google détaille cette étape.
            </p>
          )}
          <details className="google-setup-scopes">
            <summary>
              Autorisations à ajouter ({app.scopes?.length || 0})
            </summary>
            <pre>{app.scopes?.join("\n")}</pre>
            <button
              className="text-button"
              onClick={() => void copy(app.scopes?.join("\n") || "", "scopes")}
            >
              <Copy size={13} />
              {copied === "scopes"
                ? "Autorisations copiées"
                : "Copier les autorisations"}
            </button>
          </details>
        </>
      ) : step === 1 ? (
        <>
          <p>
            Dans Google Auth Platform → Clients, créez un client de type{" "}
            <strong>Application Web</strong>. Activez{" "}
            <strong>« Use this client for an AI-powered agent »</strong>, puis
            ajoutez cette URI de redirection autorisée :
          </p>
          <div className="google-setup-callback">
            <label className="field">
              URI de redirection
              <input readOnly value={callback} />
            </label>
            <button
              className="icon-button"
              aria-label="Copier l’URI de redirection"
              onClick={() => void copy(callback, "callback")}
            >
              {copied === "callback" ? <Check size={16} /> : <Copy size={16} />}
            </button>
          </div>
          <p className="google-setup-caption">
            Recopiez l’URI exactement, avec le port et le chemin. Gardez le même
            projet que celui préparé à l’étape précédente.
          </p>
          {link(
            "Créer mon client dans Google Cloud",
            "https://console.cloud.google.com/auth/clients/create",
          )}
          <label className="field">
            Client ID Google
            <input
              value={clientId}
              onChange={(e) => onClientId(e.target.value)}
              placeholder="…apps.googleusercontent.com"
              autoComplete="off"
              spellCheck={false}
            />
          </label>
          <label className="field">
            Secret client Google
            <input
              type="password"
              value={clientSecret}
              onChange={(e) => onClientSecret(e.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
          </label>
          <p className="google-setup-caption">
            Copiez les deux valeurs fournies à la création du client. Le secret
            sera conservé dans le coffre du système.
          </p>
        </>
      ) : (
        <>
          <p>
            Converge va ouvrir Google dans votre navigateur. Choisissez votre
            compte, puis vérifiez les autorisations avant de les accepter.
          </p>
          <dl className="google-setup-review">
            <dt>Application</dt>
            <dd>{app.name}</dd>
            <dt>Client OAuth</dt>
            <dd>{clientId.trim()}</dd>
            <dt>Retour vers Converge</dt>
            <dd>{callback}</dd>
          </dl>
          <p>
            La connexion ne sera indiquée comme active qu’après le retour de
            Google et la vérification du serveur MCP.
          </p>
          <details className="google-setup-scopes">
            <summary>Voir les autorisations demandées</summary>
            <pre>{app.scopes?.join("\n")}</pre>
          </details>
          <p className="google-setup-caption">
            Les actions de l’IA restent soumises aux permissions de la
            conversation. Vous pourrez retirer cette connexion dans Mes plugins.
          </p>
        </>
      )}
    </section>
  );
}
