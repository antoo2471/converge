import { extractToolRequests } from "../core/agentTools";
import { lazy, Suspense, memo, useState } from "react";
import { openExternal } from "../lib/native";
import Markdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import rehypeHighlight from "rehype-highlight";
import { Copy, Check } from "lucide-react";
import { GenerativeUI, type ActionHandler } from "../generative/Renderer";
const Artifact = lazy(() => import("./Artifact"));
export const MessageContent = memo(function MessageContent({
  content,
  streaming = false,
  onAction,
}: {
  content: string;
  streaming?: boolean;
  onAction: ActionHandler;
  onTool: (
    name: string,
    args: Record<string, unknown>,
    onOutput?: (text: string) => void,
  ) => Promise<string>;
}) {
  return (
    <Markdown
      urlTransform={(url) =>
        /^data:image\/(?:png|jpeg|webp|gif);base64,/.test(url)
          ? url
          : defaultUrlTransform(url)
      }
      remarkPlugins={[remarkGfm, remarkMath]}
      rehypePlugins={[rehypeKatex, rehypeHighlight]}
      components={{
        a: ({ children, href }) => (
          <ExternalLink href={href}>{children}</ExternalLink>
        ),
        img: ({ src, alt }) =>
          src?.startsWith("data:image/") ? (
            <img src={src} alt={alt || ""} />
          ) : (
            <span className="muted">Image externe bloquée : {alt}</span>
          ),
        pre: ({ children }) => <div className="code-wrapper">{children}</div>,
        code: ({ className, children, ...props }) => {
          const language = /language-([\w-]+)/.exec(className || "")?.[1],
            source = String(children).replace(/\n$/, "");
          if (language === "converge-ui")
            return (
              <GenerativeUI
                source={source}
                streaming={streaming}
                onAction={onAction}
              />
            );
          if (language === "artifact-html" || language === "artifact-react")
            return (
              <Suspense
                fallback={
                  <div className="muted">Chargement du runtime isolé…</div>
                }
              >
                <Artifact
                  source={source}
                  kind={language === "artifact-react" ? "react" : "html"}
                />
              </Suspense>
            );
          if (language === "converge-tool")
            return streaming ? (
              <span className="muted">Préparation de l’action…</span>
            ) : (
              <span className="inline-error">
                Demande d’outil invalide. Relancez la réponse pour réessayer.
              </span>
            );
          if (language)
            return (
              <CodeBlock source={source} className={className}>
                {children}
              </CodeBlock>
            );
          return (
            <code className={className} {...props}>
              {children}
            </code>
          );
        },
      }}
    >
      {streaming ? content : extractToolRequests(content).text}
    </Markdown>
  );
});
function ExternalLink({
  href,
  children,
}: {
  href?: string;
  children: React.ReactNode;
}) {
  const [error, setError] = useState("");
  return (
    <>
      <a
        className="external-link"
        href={href}
        onClick={(e) => {
          e.preventDefault();
          if (href) void openExternal(href).catch((e) => setError(String(e)));
        }}
      >
        {children}
      </a>
      {error && <small role="alert">{error}</small>}
    </>
  );
}
function CodeBlock({
  source,
  className,
  children,
}: {
  source: string;
  className?: string;
  children: React.ReactNode;
}) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="code-block">
      <div>
        <span>{className?.replace(/language-/, "")}</span>
        <button
          className="text-button"
          onClick={async () => {
            await navigator.clipboard.writeText(source);
            setCopied(true);
            setTimeout(() => setCopied(false), 1800);
          }}
        >
          {copied ? <Check size={13} /> : <Copy size={13} />}Copier
        </button>
      </div>
      <pre>
        <code className={className}>{children}</code>
      </pre>
    </div>
  );
}
