import { useState, useEffect, useRef } from "react";
import { Code2, Eye, Maximize2, Download, X, RefreshCw } from "lucide-react";
import runtime from "../generated/artifact-runtime.txt?raw";
import { save } from "@tauri-apps/plugin-dialog";
import { exportFile,native,registerArtifact,releaseArtifact } from "../lib/native";
export function artifactDocument(
  code: string,
  kind: string,
  runtimeUrl?: string,
) {
  const policy = `default-src 'none'; script-src blob:; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'; object-src 'none'; frame-src 'none'`;
  const head = `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${policy}"><meta name="viewport" content="width=device-width,initial-scale=1"><style>body{margin:0;padding:24px;font:14px Inter,system-ui,sans-serif;color:#202522;background:#fafbf8}*{box-sizing:border-box}button,input,select{font:inherit}button{cursor:pointer}</style>`;
  if (kind === "react")
    return `<!doctype html><html><head>${head}</head><body><div id="app"></div><script src="${runtimeUrl}"></script></body></html>`;
  // Inline artifact scripts become separate blob resources; no parent bridge, network or same-origin privilege.
  const parsed = new DOMParser().parseFromString(code, "text/html");
  parsed.querySelectorAll("script").forEach((script) => {
    if (script.src) {
      script.remove();
      return;
    }
    const link = URL.createObjectURL(
      new Blob([script.textContent || ""], { type: "text/javascript" }),
    );
    script.textContent = "";
    script.src = link;
  });
  const handlers: string[] = [];
  parsed.querySelectorAll("*").forEach((el, i) => {
    for (const attribute of Array.from(el.attributes)) {
      if (attribute.name.startsWith("on")) {
        const marker = `handler-${i}`;
        el.setAttribute("data-converge-handler", marker);
        handlers.push(
          `document.querySelector('[data-converge-handler="${marker}"]').addEventListener(${JSON.stringify(attribute.name.slice(2))},function(event){${attribute.value}\n});`,
        );
        el.removeAttribute(attribute.name);
      }
      if (
        (attribute.name === "href" || attribute.name === "src") &&
        attribute.value.trim().toLowerCase().startsWith("javascript:")
      )
        el.removeAttribute(attribute.name);
    }
  });
  if (handlers.length) {
    const script = parsed.createElement("script");
    script.src = URL.createObjectURL(
      new Blob([handlers.join("\n")], { type: "text/javascript" }),
    );
    parsed.body.append(script);
  }
  const html = parsed.head.innerHTML + parsed.body.innerHTML;
  return `<!doctype html><html><head>${head}</head><body>${html}</body></html>`;
}
export default function Artifact({
  source,
  kind,
}: {
  source: string;
  kind: "html" | "react";
}) {
  const [code, setCode] = useState(source),
    [mode, setMode] = useState<"preview" | "code">("preview"),
    [full, setFull] = useState(false),
    [revision, setRevision] = useState(0),
    [error, setError] = useState("");
  const frame = useRef<HTMLIFrameElement>(null),
    [document, setDocument] = useState(""),[url,setUrl]=useState('');
  const edited = useRef(false);
  useEffect(() => {
    if (!edited.current) setCode(source);
  }, [source]);
  useEffect(() => {
    let disposed=false,path:string|undefined;
    if(native){
      const ready=(event:MessageEvent)=>{if(event.source===frame.current?.contentWindow&&event.data?.type==='converge-artifact-ready')frame.current?.contentWindow?.postMessage({type:'converge-artifact',kind,code},'*')};window.addEventListener('message',ready);
      const timer=setTimeout(()=>{registerArtifact(code,kind,runtime).then(result=>{path=result.path;if(disposed){void releaseArtifact(path);return}setUrl(result.url);setDocument('')}).catch(e=>{if(!disposed)setError(String(e))})},150);
      return()=>{disposed=true;clearTimeout(timer);window.removeEventListener('message',ready);if(path)void releaseArtifact(path)};
    }
    const runtimeUrl = URL.createObjectURL(
      new Blob([runtime], { type: "text/javascript" }),
    );
    const doc = artifactDocument(code, kind, runtimeUrl);
    setDocument(doc);
    const urls = [
      runtimeUrl,
      ...Array.from(doc.matchAll(/src="(blob:[^"]+)"/g)).map((m) => m[1]),
    ];
    const ready = (event: MessageEvent) => {
      if (
        event.source === frame.current?.contentWindow &&
        event.data?.type === "converge-artifact-ready"
      )
        frame.current?.contentWindow?.postMessage(
          { type: "converge-artifact", kind, code },
          "*",
        );
    };
    window.addEventListener("message", ready);
    return () => {
      window.removeEventListener("message", ready);
      urls.forEach((u) => URL.revokeObjectURL(u));
    };
  }, [code, kind, revision]);
  async function download() {
    try {
      const path = await save({
        defaultPath: kind === "react" ? "artifact.tsx" : "artifact.html",
      });
      if (path) await exportFile(path, code);
    } catch (e) {
      setError(String(e));
    }
  }
  return (
    <section className={`artifact ${full ? "artifact-full" : ""}`}>
      <header>
        <span>
          <Code2 size={16} />
          {kind === "react" ? "Application React" : "Artifact HTML"}
        </span>
        <div className="segmented">
          <button
            className={mode === "preview" ? "active" : ""}
            onClick={() => setMode("preview")}
          >
            <Eye size={14} />
            Aperçu
          </button>
          <button
            className={mode === "code" ? "active" : ""}
            onClick={() => setMode("code")}
          >
            <Code2 size={14} />
            Code
          </button>
        </div>
        <div className="toolbar">
          <button
            className="icon-button"
            title="Relancer"
            onClick={() => setRevision((v) => v + 1)}
          >
            <RefreshCw size={15} />
          </button>
          <button
            className="icon-button"
            title="Exporter le code"
            onClick={download}
          >
            <Download size={15} />
          </button>
          <button
            className="icon-button"
            title={full ? "Fermer le plein écran" : "Plein écran"}
            onClick={() => setFull(!full)}
          >
            {full ? <X size={16} /> : <Maximize2 size={15} />}
          </button>
        </div>
      </header>
      {mode === "code" ? (
        <textarea
          className="artifact-editor"
          spellCheck={false}
          aria-label="Code de l’artifact"
          value={code}
          onChange={(e) => {
            edited.current = true;
            setCode(e.target.value);
          }}
        />
      ) : (
        <iframe
          key={revision}
          ref={frame}
          title="Artifact isolé"
          sandbox="allow-scripts"
          referrerPolicy="no-referrer"
          src={url||undefined}
          srcDoc={native?undefined:document}
        />
      )}
      <footer>
        Exécution isolée · réseau, fichiers et accès système bloqués
      </footer>
      {error && <div className="inline-error">{error}</div>}
    </section>
  );
}
