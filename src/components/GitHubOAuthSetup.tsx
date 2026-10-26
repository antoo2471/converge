import { useEffect, useRef, useState } from "react";
import { ArrowUpRight, Check, Copy } from "lucide-react";
import { openExternal } from "../lib/native";

export function GitHubOAuthSetup({
  step,
  clientId,
  clientSecret,
  onClientId,
  onClientSecret,
  onError,
}: {
  step: number;
  clientId: string;
  clientSecret: string;
  onClientId: (value: string) => void;
  onClientSecret: (value: string) => void;
  onError: (value: string) => void;
}) {
  const [copied, setCopied] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const callback = "http://127.0.0.1:32145/oauth/callback";
  const copyCallback = () =>
    void navigator.clipboard
      .writeText(callback)
      .then(() => setCopied(true))
      .catch(() => onError("Copiez manuellement l’adresse affichée."));
  useEffect(() => {
    heading.current?.focus({ preventScroll: true });
    const body = heading.current?.closest(".google-connect-body");
    if (body) body.scrollTop = 0;
  }, [step]);
  const link = (name: string, url: string) => (
    <button
      className="text-button google-setup-link"
      onClick={() => void openExternal(url).catch((e) => onError(String(e)))}
    >
      {name}
      <ArrowUpRight size={13} />
    </button>
  );
  return (
    <section className="google-setup">
      <ol
        className="google-setup-progress"
        aria-label="Étapes de connexion GitHub"
      >
        {["Préparer", "Configurer", "Se connecter"].map((name, i) => (
          <li key={name} aria-current={i === step ? "step" : undefined}>
            {i < step ? <Check size={12} /> : <span>{i + 1}</span>}
            {name}
          </li>
        ))}
      </ol>
      <h3 ref={heading} tabIndex={-1}>
        {
          [
            "Créez votre connexion GitHub",
            "Renseignez votre application OAuth",
            "Autorisez Converge sur GitHub",
          ][step]
        }
      </h3>
      {step === 0 ? (
        <>
          <p>
            Le serveur MCP distant de GitHub demande une application OAuth
            enregistrée. Cette préparation se fait une seule fois sur votre
            compte.
          </p>
          <ol className="google-setup-instructions">
            <li>
              <strong>Ouvrez les paramètres développeur GitHub.</strong>
              <p>
                Dans OAuth Apps, choisissez New OAuth App. Pour le nom, utilisez
                « Converge personnel ».
              </p>
              {link(
                "Créer mon application OAuth",
                "https://github.com/settings/applications/new",
              )}
            </li>
            <li>
              <strong>Renseignez les deux adresses.</strong>
              <p>
                Homepage URL :{" "}
                <code>https://github.com/antoo2471/converge</code>.
              </p>
              <div className="google-setup-callback">
                <label className="field">
                  Authorization callback URL
                  <input readOnly value={callback} />
                </label>
                <button
                  className="icon-button"
                  aria-label="Copier l’URI de redirection"
                  onClick={copyCallback}
                >
                  {copied ? <Check size={16} /> : <Copy size={16} />}
                </button>
              </div>
            </li>
            <li>
              <strong>Créez un secret client.</strong>
              <p>
                Après Register application, copiez le Client ID. Cliquez
                Generate a new client secret, puis copiez le secret. Ces valeurs
                restent dans Converge, jamais dans le chat.
              </p>
            </li>
          </ol>
          <p className="google-setup-caption">
            Pour une organisation, ses règles OAuth et son éventuelle
            autorisation SSO peuvent aussi s’appliquer. Votre compte conserve
            ses propres droits.
          </p>
        </>
      ) : step === 1 ? (
        <>
          <p>
            Dans l’OAuth App que vous venez de créer, utilisez exactement cette
            Authorization callback URL :
          </p>
          <div className="google-setup-callback">
            <label className="field">
              URI de redirection
              <input readOnly value={callback} />
            </label>
            <button
              className="icon-button"
              aria-label="Copier l’URI de redirection"
              onClick={() =>
                void navigator.clipboard
                  .writeText(callback)
                  .then(() => setCopied(true))
                  .catch(() =>
                    onError("Copiez manuellement l’adresse affichée."),
                  )
              }
            >
              {copied ? <Check size={16} /> : <Copy size={16} />}
            </button>
          </div>
          {link(
            "Mes applications OAuth GitHub",
            "https://github.com/settings/developers",
          )}
          <label className="field">
            Client ID GitHub
            <input
              autoComplete="off"
              spellCheck={false}
              value={clientId}
              onChange={(e) => onClientId(e.target.value)}
            />
          </label>
          <label className="field">
            Secret client GitHub
            <input
              type="password"
              autoComplete="off"
              value={clientSecret}
              onChange={(e) => onClientSecret(e.target.value)}
            />
          </label>
          <p className="google-setup-caption">
            Le Client ID n’est pas un Personal Access Token. Le secret client
            est conservé dans le coffre du système.
          </p>
        </>
      ) : (
        <>
          <p>
            Votre navigateur va ouvrir GitHub. Vérifiez le compte et les
            autorisations, puis validez la connexion.
          </p>
          <dl className="google-setup-review">
            <dt>Serveur officiel</dt>
            <dd>api.githubcopilot.com</dd>
            <dt>Client OAuth</dt>
            <dd>{clientId}</dd>
            <dt>Retour vers Converge</dt>
            <dd>{callback}</dd>
          </dl>
          <p>
            Les accès demandés comprennent les dépôts (<code>repo</code>), le
            profil et la lecture des organisations. Le scope repo peut inclure
            vos dépôts privés. Vérifiez ces accès sur l’écran GitHub.
          </p>
          <p className="google-setup-caption">
            Converge attendra le retour OAuth et vérifiera les outils
            disponibles. Les actions du modèle restent soumises aux permissions
            de la conversation.
          </p>
        </>
      )}
    </section>
  );
}
