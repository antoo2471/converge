import { SkillsLibrary } from "./SkillsLibrary";
import { useEffect, useRef, useState } from "react";
import {
  Puzzle,
  Search,
  Plus,
  ChevronDown,
  ArrowLeft,
  ArrowUpRight,
  RefreshCw,
  LoaderCircle,
  Check,
  Settings2,
  Trash2,
  FileText,
  FolderOpen,
  PackageOpen,
  Server,
  Globe,
  X,
  BookOpen,
  Unplug,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppDialogs } from "./AppDialogs";
import * as api from "../lib/extensions";
import { native, openExternal } from "../lib/native";
import {
  appCatalogue,
  appCategories,
  type AppEntry,
} from "../core/appCatalogue";
import { AppConnectDialog } from "./AppConnectDialog";
import github from "../assets/brands/github.svg";
import supabase from "../assets/brands/supabase.svg";
import huggingface from "../assets/brands/huggingface.svg";
import figma from "../assets/brands/figma.svg";
import notion from "../assets/brands/notion.svg";
import linear from "../assets/brands/linear.svg";
import atlassian from "../assets/brands/atlassian.svg";
const logos: Record<string, string> = {
  github,
  supabase,
  huggingface,
  figma,
  notion,
  linear,
  atlassian,
  ...Object.fromEntries(
    Object.entries(
      import.meta.glob<string>("../assets/brands/apps/*.svg", {
        eager: true,
        query: "?url",
        import: "default",
      }),
    ).map(([path, value]) => [
      path.split("/").at(-1)!.replace(".svg", ""),
      value,
    ]),
  ),
};
const initialSkill = `---\nname: mon-skill\ndescription: Décrivez quand et pourquoi utiliser ce skill.\n---\n\n# Instructions\n\nDécrivez la méthode à suivre, les étapes et le résultat attendu.\n`;
function Mark({ name }: { name?: string }) {
  return (
    <span className="extension-mark">
      {name && logos[name] ? (
        <img
          className="provider-monochrome-mark"
          src={logos[name]}
          width={25}
          height={25}
          alt=""
        />
      ) : (
        <Puzzle size={23} />
      )}
    </span>
  );
}
export function ExtensionsPage() {
  const [connectingApp, setConnectingApp] = useState<AppEntry | null>(null);
  const [tab, setTab] = useState<"plugins" | "skills">("plugins"),
    [scope, setScope] = useState<"discover" | "installed" | "registry">(
      "discover",
    ),
    [query, setQuery] = useState("");
  const [data, setData] = useState<api.ExtensionSnapshot>({
      skills: [],
      servers: [],
    }),
    [loading, setLoading] = useState(true),
    [busy, setBusy] = useState(""),
    [error, setError] = useState("");
  const [menu, setMenu] = useState(false),
    [editor, setEditor] = useState<api.McpConfig | null>(null),
    [appInfo, setAppInfo] = useState<AppEntry | null>(null),
    [skillEditor, setSkillEditor] = useState(false),
    [skillText, setSkillText] = useState(initialSkill),
    [preview, setPreview] = useState<{ name: string; content: string } | null>(
      null,
    );
  const [registry, setRegistry] = useState<api.RegistryServer[]>([]),
    [cursor, setCursor] = useState<string | undefined>(),
    [registryLoading, setRegistryLoading] = useState(false),
    [registryError, setRegistryError] = useState("");
  const menuRef = useRef<HTMLDivElement>(null),
    alive = useRef(true),
    registryVersion = useRef(0);
  const [skillScope, setSkillScope] = useState<"installed" | "library">(
    "installed",
  );
  const { askConfirm } = useAppDialogs();
  async function refresh() {
    if (!native) {
      setLoading(false);
      return;
    }
    const snapshot = await api.snapshot();
    if (alive.current) {
      setData(snapshot);
      setLoading(false);
    }
  }
  useEffect(() => {
    alive.current = true;
    void refresh().catch((e) => {
      if (alive.current) {
        setError(String(e));
        setLoading(false);
      }
    });
    return () => {
      alive.current = false;
      registryVersion.current++;
    };
  }, []);
  const pending = data.servers.some((s) => s.authPending);
  useEffect(() => {
    if (!native || (!pending && !data.servers.length)) return;
    let inFlight = false;
    const timer = setInterval(
      () => {
        if (inFlight) return;
        inFlight = true;
        void refresh()
          .catch((e) => {
            if (alive.current) setError(String(e));
          })
          .finally(() => {
            inFlight = false;
          });
      },
      pending ? 2000 : 10000,
    );
    return () => clearInterval(timer);
  }, [pending, data.servers.length]);
  useEffect(() => {
    if (!menu) return;
    const close = (e: PointerEvent) => {
      if (!menuRef.current?.contains(e.target as Node)) setMenu(false);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenu(false);
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", key);
    };
  }, [menu]);
  async function searchRegistry(append = false) {
    const version = ++registryVersion.current;
    setRegistryLoading(true);
    setRegistryError("");
    try {
      const result = await api.catalogue(query, append ? cursor : undefined);
      if (!alive.current || version !== registryVersion.current) return;
      const entries = result.servers
        .map((v) => v.server)
        .filter(
          (s) =>
            s &&
            typeof s.name === "string" &&
            typeof s.description === "string",
        );
      setRegistry((old) =>
        append
          ? [
              ...old,
              ...entries.filter(
                (v) =>
                  !old.some(
                    (o) => o.name === v.name && o.version === v.version,
                  ),
              ),
            ]
          : entries,
      );
      setCursor(result.metadata?.nextCursor);
    } catch (e) {
      if (alive.current && version === registryVersion.current)
        setRegistryError(String(e));
    } finally {
      if (alive.current && version === registryVersion.current)
        setRegistryLoading(false);
    }
  }
  useEffect(() => {
    if (tab !== "plugins" || scope !== "registry" || !native) return;
    const timer = setTimeout(() => void searchRegistry(), 350);
    return () => {
      clearTimeout(timer);
      registryVersion.current++;
    };
  }, [query, scope, tab]);
  async function perform(key: string, operation: () => Promise<void>) {
    setBusy(key);
    setError("");
    try {
      await operation();
      await refresh();
    } catch (e) {
      if (alive.current) setError(String(e));
    } finally {
      if (alive.current) setBusy("");
    }
  }
  function configure(app?: AppEntry) {
    setMenu(false);
    if (app) {
      setConnectingApp(app);
      return;
    }
    setAppInfo(null);
    setEditor(api.newMcp());
    setError("");
  }
  async function importSkill(directory = false) {
    setMenu(false);
    const selected = await open(
      directory
        ? {
            directory: true,
            multiple: false,
            title: "Choisir le dossier du skill",
          }
        : {
            multiple: false,
            title: "Importer un skill",
            filters: [
              {
                name: "Skill Markdown ou archive ZIP",
                extensions: ["md", "zip"],
              },
            ],
          },
    );
    if (typeof selected !== "string") return;
    await perform("import", async () => {
      await api.skillAction("import", { source: selected });
      setTab("skills");
      setSkillScope("installed");
    });
  }
  async function connect(server: api.McpServer, authorize = false) {
    await perform(server.id, async () => {
      const result = await api.mcpAction(authorize ? "authorize" : "connect", {
        id: server.id,
      });
      if (result?.authUrl)
        await api.mcpAction("openAuth", {
          id: server.id,
          loginId: result.loginId,
        });
    });
  }
  async function removeSkill(skill: api.Skill) {
    if (
      await askConfirm(
        `Supprimer « ${skill.name} » de Converge ? Le dossier d’origine est conservé.`,
      )
    )
      await perform(skill.id, async () => {
        await api.skillAction("remove", { id: skill.id });
      });
  }
  async function removeServer(server: api.McpServer) {
    if (
      await askConfirm(
        `Retirer « ${server.name} » et ses identifiants de Converge ?`,
      )
    )
      await perform(server.id, async () => {
        await api.mcpAction("remove", { id: server.id });
      });
  }
  if (editor)
    return (
      <McpEditor
        key={editor.id}
        value={editor}
        appInfo={appInfo}
        existing={data.servers.some((s) => s.id === editor.id)}
        onBack={() => {
          setEditor(null);
          setAppInfo(null);
        }}
        onSaved={async (config, secret, connectNow) => {
          await api.mcpAction("save", {
            config,
            ...(secret ? { secret } : {}),
          });
          setEditor(null);
          setAppInfo(null);
          setScope("installed");
          await refresh();
          if (connectNow) {
            try {
              const result = await api.mcpAction(
                config.auth === "oauth" ? "authorize" : "connect",
                { id: config.id },
              );
              if (result?.authUrl)
                await api.mcpAction("openAuth", {
                  id: config.id,
                  loginId: result.loginId,
                });
              await refresh();
            } catch (e) {
              setError(String(e));
              await refresh();
            }
          }
        }}
      />
    );
  if (skillEditor || preview)
    return (
      <div className="page extension-library extension-editor">
        <button
          className="text-button back"
          onClick={() => {
            setSkillEditor(false);
            setPreview(null);
          }}
        >
          <ArrowLeft size={16} />
          Skills
        </button>
        <header>
          <h1>{preview?.name || "Créer un skill"}</h1>
          <p>
            {preview
              ? "Instructions enregistrées dans votre bibliothèque."
              : "Une méthode réutilisable que vos modèles peuvent charger au bon moment."}
          </p>
        </header>
        <label className="field">
          SKILL.md
          <textarea
            className="extension-skill-source"
            spellCheck={false}
            value={preview?.content ?? skillText}
            readOnly={!!preview}
            onChange={(e) => setSkillText(e.target.value)}
          />
        </label>
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        {!preview && (
          <div className="editor-footer">
            <button className="button" onClick={() => setSkillEditor(false)}>
              Annuler
            </button>
            <button
              className="button primary"
              disabled={!!busy || !native}
              onClick={() =>
                void perform("create", async () => {
                  await api.skillAction("create", { content: skillText });
                  setSkillEditor(false);
                  setTab("skills");
                  setSkillScope("installed");
                })
              }
            >
              {busy ? (
                <LoaderCircle size={15} className="spin" />
              ) : (
                <Plus size={15} />
              )}
              Ajouter le skill
            </button>
          </div>
        )}
      </div>
    );
  const matching = (text: string) =>
    text.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase());
  const skills = data.skills.filter((s) =>
    matching(`${s.name} ${s.description}`),
  );
  const servers = data.servers.filter((s) =>
    matching(`${s.name} ${s.url} ${s.command}`),
  );
  return (
    <div className="page extension-library">
      {connectingApp && (
        <AppConnectDialog
          app={connectingApp}
          mark={<Mark name={connectingApp.icon || connectingApp.id} />}
          onClose={() => {
            setConnectingApp(null);
            void refresh().catch((e) => setError(String(e)));
          }}
          onConnected={() => void refresh().catch((e) => setError(String(e)))}
        />
      )}
      <div className="extension-topbar">
        <div
          className="extension-tabs"
          role="tablist"
          aria-label="Bibliothèque d’extensions"
        >
          {(
            [
              ["plugins", "Plugins"],
              ["skills", "Skills"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              role="tab"
              aria-selected={tab === id}
              onClick={() => {
                setTab(id);
                setQuery("");
                setError("");
              }}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="extension-add" ref={menuRef}>
          <button
            className="button"
            aria-expanded={menu}
            aria-haspopup="menu"
            disabled={!native || !!busy}
            onClick={() => setMenu((v) => !v)}
          >
            <Plus size={16} />
            Ajouter
            <ChevronDown size={14} />
          </button>
          {menu && (
            <div className="extension-add-menu" role="menu">
              <button role="menuitem" onClick={() => configure()}>
                <Server size={16} />
                Serveur MCP personnalisé
              </button>
              <button
                role="menuitem"
                onClick={() => {
                  setMenu(false);
                  setSkillText(initialSkill);
                  setSkillEditor(true);
                  setError("");
                }}
              >
                <FileText size={16} />
                Créer un skill
              </button>
              <button
                role="menuitem"
                onClick={() =>
                  void importSkill().catch((e) => setError(String(e)))
                }
              >
                <PackageOpen size={16} />
                Importer un skill ou un ZIP
              </button>
              <button
                role="menuitem"
                onClick={() =>
                  void importSkill(true).catch((e) => setError(String(e)))
                }
              >
                <FolderOpen size={16} />
                Importer un dossier de skill
              </button>
            </div>
          )}
        </div>
      </div>
      <header className="extension-heading">
        <h1>{tab === "plugins" ? "Plugins" : "Skills"}</h1>
        <p>
          {tab === "plugins"
            ? "Connectez vos outils et travaillez avec eux depuis Converge."
            : "Ajoutez des skills pour retrouver vos instructions et vos façons de faire."}
        </p>
      </header>
      <label className="search-field extension-search">
        <Search size={18} />
        <input
          aria-label={
            tab === "plugins"
              ? "Rechercher des plugins"
              : "Rechercher des skills"
          }
          placeholder={
            tab === "plugins"
              ? "Rechercher une app ou un serveur MCP…"
              : skillScope === "library"
                ? "Rechercher dans la bibliothèque skills.sh…"
                : "Rechercher dans vos skills…"
          }
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {query && (
          <button
            className="icon-button"
            aria-label="Effacer la recherche"
            onClick={() => setQuery("")}
          >
            <X size={15} />
          </button>
        )}
      </label>
      {tab === "plugins" && (
        <nav className="extension-scopes" aria-label="Catalogue des plugins">
          {(
            [
              ["discover", "Découvrir"],
              ["installed", "Mes plugins"],
              ["registry", "Registre MCP"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              aria-current={scope === id ? "page" : undefined}
              onClick={() => setScope(id)}
            >
              {label}
              {id === "installed" && data.servers.length > 0 && (
                <span>{data.servers.length}</span>
              )}
            </button>
          ))}
        </nav>
      )}
      {!native && (
        <p className="muted">
          Ouvrez Converge desktop pour connecter des apps et importer des
          skills.
        </p>
      )}
      {error && (
        <p className="error extension-error" role="alert">
          {error}
          <button
            className="icon-button"
            aria-label="Fermer l’erreur"
            onClick={() => setError("")}
          >
            <X size={15} />
          </button>
        </p>
      )}
      {tab === "plugins" && scope === "discover" && (
        <>
          {!appCatalogue.some((app) =>
            matching(`${app.name} ${app.description}`),
          ) && (
            <Empty
              icon={<Search size={28} />}
              title="Aucune app trouvée"
              text="Essayez un autre nom ou explorez le registre MCP."
            >
              <button className="button" onClick={() => setQuery("")}>
                Effacer la recherche
              </button>
            </Empty>
          )}
          {appCategories.map((category) => {
            const apps = appCatalogue.filter(
              (app) =>
                app.category === category &&
                matching(`${app.name} ${app.description}`),
            );
            return (
              apps.length > 0 && (
                <section className="extension-section" key={category}>
                  <h2>{category}</h2>
                  <div className="extension-app-grid">
                    {apps.map((app) => {
                      const installed = data.servers.some(
                        (s) => s.url === app.url,
                      );
                      return (
                        <article className="extension-app-row" key={app.id}>
                          <Mark name={app.icon || app.id} />
                          <div>
                            <h3>{app.name}</h3>
                            <p>{app.description}</p>
                          </div>
                          <button
                            className="icon-button"
                            aria-label={`Ajouter ${app.name}`}
                            title={
                              installed
                                ? "Ajouter une autre connexion"
                                : "Configurer la connexion"
                            }
                            disabled={!native}
                            onClick={() => configure(app)}
                          >
                            {installed ? (
                              <Check size={18} />
                            ) : (
                              <Plus size={19} />
                            )}
                          </button>
                        </article>
                      );
                    })}
                  </div>
                </section>
              )
            );
          })}
          <button
            className="text-button extension-registry-link"
            onClick={() => setScope("registry")}
          >
            <Globe size={16} />
            Explorer le registre MCP
            <ArrowUpRight size={15} />
          </button>
        </>
      )}
      {tab === "plugins" && scope === "installed" && (
        <section className="extension-section">
          {loading ? (
            <Loading />
          ) : servers.length === 0 ? (
            <Empty
              icon={<Puzzle size={30} />}
              title={
                query ? "Aucun plugin trouvé" : "Connectez votre première app"
              }
              text="Vos connexions MCP et leurs outils apparaîtront ici."
            >
              <button className="button" onClick={() => setScope("discover")}>
                Découvrir les apps
              </button>
            </Empty>
          ) : (
            <div className="extension-installed">
              {servers.map((server) => (
                <article className="extension-installed-row" key={server.id}>
                  <div className="extension-row-heading">
                    <Mark
                      name={appCatalogue.find((a) => a.url === server.url)?.id}
                    />
                    <div className="extension-row-main">
                      <h3>
                        {server.name}
                        <span
                          className={`extension-status ${server.connected ? "connected" : ""}`}
                        >
                          {server.connected
                            ? `${server.toolCount} outil${server.toolCount > 1 ? "s" : ""}`
                            : server.authPending
                              ? "Connexion en cours"
                              : server.enabled
                                ? "Déconnecté"
                                : "Désactivé"}
                        </span>
                      </h3>
                      <p
                        title={
                          server.transport === "http"
                            ? server.url
                            : server.command
                        }
                      >
                        {server.transport === "http"
                          ? server.url
                          : server.command}
                      </p>
                    </div>
                    <button
                      className="icon-button"
                      aria-label={`Configurer ${server.name}`}
                      onClick={() => {
                        setAppInfo(null);
                        setEditor(server);
                      }}
                    >
                      <Settings2 size={17} />
                    </button>
                    <button
                      className="icon-button"
                      aria-label={`Retirer ${server.name}`}
                      disabled={!!busy}
                      onClick={() => void removeServer(server)}
                    >
                      <Trash2 size={16} />
                    </button>
                    <button
                      role="switch"
                      aria-checked={server.enabled}
                      aria-label={`Activer ${server.name}`}
                      className="switch"
                      disabled={!!busy}
                      onClick={() =>
                        void perform(server.id, async () => {
                          await api.mcpAction("save", {
                            config: { ...server, enabled: !server.enabled },
                          });
                        })
                      }
                    >
                      <span />
                    </button>
                  </div>
                  <div className="extension-row-actions">
                    {server.authPending ? (
                      <>
                        <span role="status">
                          <LoaderCircle size={14} className="spin" />
                          Autorisez la connexion dans votre navigateur.
                        </span>
                        <button
                          className="text-button"
                          onClick={() =>
                            void api
                              .mcpAction("openAuth", {
                                id: server.id,
                                loginId: server.loginId,
                              })
                              .catch((e) => setError(String(e)))
                          }
                        >
                          Rouvrir
                        </button>
                        <button
                          className="text-button"
                          onClick={() =>
                            void perform(server.id, async () => {
                              await api.mcpAction("cancel", {
                                id: server.id,
                                loginId: server.loginId,
                              });
                            })
                          }
                        >
                          Annuler
                        </button>
                      </>
                    ) : server.enabled ? (
                      <>
                        {server.connected ? (
                          <button
                            className="text-button"
                            disabled={!!busy}
                            onClick={() =>
                              void perform(server.id, async () => {
                                await api.mcpAction("disconnect", {
                                  id: server.id,
                                });
                              })
                            }
                          >
                            <Unplug size={14} />
                            Déconnecter
                          </button>
                        ) : (
                          <button
                            className="button"
                            disabled={!!busy}
                            onClick={() => void connect(server)}
                          >
                            {busy === server.id ? (
                              <LoaderCircle size={14} className="spin" />
                            ) : (
                              <RefreshCw size={14} />
                            )}
                            Connecter
                          </button>
                        )}
                        {server.auth === "oauth" && (
                          <button
                            className="text-button"
                            disabled={!!busy}
                            onClick={() => void connect(server, true)}
                          >
                            Reconnecter le compte
                          </button>
                        )}
                        {server.connected && (
                          <details>
                            <summary>Outils disponibles</summary>
                            <div className="extension-tool-list">
                              {server.toolNames.map((tool) => (
                                <p key={tool.name}>
                                  <code>{tool.name}</code>
                                  <span>{tool.description}</span>
                                </p>
                              ))}
                            </div>
                          </details>
                        )}
                      </>
                    ) : null}
                  </div>
                  {server.error && (
                    <p className="error" role="alert">
                      {server.error}
                    </p>
                  )}
                </article>
              ))}
            </div>
          )}
        </section>
      )}
      {tab === "plugins" && scope === "registry" && (
        <section className="extension-section">
          <div className="extension-section-heading">
            <div>
              <h2>Le registre MCP</h2>
              <p>
                Serveurs publiés par leurs éditeurs et la communauté. Vérifiez
                leur origine avant de les ajouter.
              </p>
            </div>
            <button
              className="icon-button"
              title="Actualiser le registre"
              disabled={!native || registryLoading}
              onClick={() => void searchRegistry()}
            >
              <RefreshCw size={16} />
            </button>
          </div>
          {registryError && (
            <p className="error" role="alert">
              {registryError}
            </p>
          )}
          {registryLoading && registry.length === 0 ? (
            <Loading />
          ) : registry.length === 0 ? (
            <Empty
              icon={<Search size={28} />}
              title="Aucun serveur affiché"
              text={
                native
                  ? "Recherchez une app ou actualisez le catalogue."
                  : "La recherche du registre est disponible dans Converge desktop."
              }
            />
          ) : (
            <div className="extension-app-grid">
              {registry.map((server) => {
                const config = api.registryConfig(server);
                return (
                  <article
                    className="extension-app-row registry-row"
                    key={`${server.name}:${server.version}`}
                  >
                    <Mark />
                    <div>
                      <h3>{server.title || server.name}</h3>
                      <p>{server.description}</p>
                      <small>{server.name}</small>
                    </div>
                    {config ? (
                      <button
                        className="icon-button"
                        aria-label={`Ajouter ${server.title || server.name}`}
                        onClick={() => {
                          setAppInfo(null);
                          setEditor(config);
                        }}
                      >
                        <Plus size={18} />
                      </button>
                    ) : (
                      <span className="muted">Configuration spécifique</span>
                    )}
                  </article>
                );
              })}
            </div>
          )}
          {cursor && (
            <button
              className="button extension-more"
              disabled={registryLoading}
              onClick={() => void searchRegistry(true)}
            >
              {registryLoading ? (
                <LoaderCircle size={15} className="spin" />
              ) : (
                <Plus size={15} />
              )}
              Afficher plus
            </button>
          )}
        </section>
      )}
      {tab === "skills" && (
        <nav className="extension-scopes" aria-label="Catalogue des skills">
          <button
            className={skillScope === "library" ? "active" : ""}
            onClick={() => setSkillScope("library")}
          >
            Bibliothèque
          </button>
          <button
            className={skillScope === "installed" ? "active" : ""}
            onClick={() => setSkillScope("installed")}
          >
            Mes skills
          </button>
        </nav>
      )}
      {tab === "skills" && skillScope === "library" && (
        <SkillsLibrary
          query={query}
          installed={data.skills}
          onInstalled={refresh}
        />
      )}
      {tab === "skills" && skillScope === "installed" && (
        <section className="extension-section">
          {loading ? (
            <Loading />
          ) : skills.length === 0 ? (
            <Empty
              icon={<BookOpen size={30} />}
              title={
                query ? "Aucun skill trouvé" : "Un peu de votre savoir-faire."
              }
              text="Importez un dossier, un SKILL.md ou un ZIP. Vous pouvez aussi écrire vos propres instructions."
            >
              <button
                className="button"
                disabled={!native || !!busy}
                onClick={() =>
                  void importSkill().catch((e) => setError(String(e)))
                }
              >
                <Plus size={15} />
                Importer un skill
              </button>
            </Empty>
          ) : (
            <div className="extension-installed">
              {skills.map((skill) => (
                <article className="extension-skill-row" key={skill.id}>
                  <Mark />
                  <div className="extension-row-main">
                    <h3>{skill.name}</h3>
                    <p>{skill.description}</p>
                    <button
                      className="text-button"
                      onClick={() =>
                        void perform(skill.id, async () => {
                          const value = await api.skillAction("view", {
                            id: skill.id,
                          });
                          if (value && "content" in value)
                            setPreview({
                              name: skill.name,
                              content: value.content,
                            });
                        })
                      }
                    >
                      Voir les instructions
                      <ArrowUpRight size={13} />
                    </button>
                  </div>
                  <button
                    className="icon-button"
                    aria-label={`Supprimer ${skill.name}`}
                    disabled={!!busy}
                    onClick={() => void removeSkill(skill)}
                  >
                    <Trash2 size={16} />
                  </button>
                  <button
                    role="switch"
                    aria-checked={skill.enabled}
                    aria-label={`Activer ${skill.name}`}
                    className="switch"
                    disabled={!!busy}
                    onClick={() =>
                      void perform(skill.id, async () => {
                        await api.skillAction("toggle", {
                          id: skill.id,
                          enabled: !skill.enabled,
                        });
                      })
                    }
                  >
                    <span />
                  </button>
                </article>
              ))}
            </div>
          )}
        </section>
      )}
    </div>
  );
}
function Loading() {
  return (
    <p className="extension-loading" role="status">
      <LoaderCircle size={17} className="spin" />
      Chargement de la bibliothèque…
    </p>
  );
}
function Empty({
  icon,
  title,
  text,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  text: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="extension-empty">
      {icon}
      <h2>{title}</h2>
      <p>{text}</p>
      {children}
    </div>
  );
}
function McpEditor({
  value,
  appInfo,
  existing,
  onBack,
  onSaved,
}: {
  value: api.McpConfig;
  appInfo: AppEntry | null;
  existing: boolean;
  onBack: () => void;
  onSaved: (
    config: api.McpConfig,
    secret: api.McpSecrets | undefined,
    connect: boolean,
  ) => Promise<void>;
}) {
  const [config, setConfig] = useState(value),
    [token, setToken] = useState(""),
    [headers, setHeaders] = useState(""),
    [env, setEnv] = useState(""),
    [clientSecret, setClientSecret] = useState(""),
    [args, setArgs] = useState(JSON.stringify(value.args, null, 2)),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const patch = (v: Partial<api.McpConfig>) =>
    setConfig((old) => ({ ...old, ...v }));
  async function save(connect: boolean) {
    setBusy(true);
    setError("");
    try {
      let parsed: unknown;
      try {
        parsed = JSON.parse(args);
      } catch {
        throw Error("Les arguments doivent être une liste JSON de textes.");
      }
      if (!Array.isArray(parsed) || parsed.some((v) => typeof v !== "string"))
        throw Error("Les arguments doivent être une liste JSON de textes.");
      const map = (text: string) => {
        if (!text.trim()) return {};
        try {
          const v: unknown = JSON.parse(text);
          if (
            !v ||
            typeof v !== "object" ||
            Array.isArray(v) ||
            Object.values(v).some((x) => typeof x !== "string")
          )
            throw Error();
          return v as Record<string, string>;
        } catch {
          throw Error(
            "Les headers et variables doivent être des objets JSON avec des valeurs texte.",
          );
        }
      };
      const secret =
        token || headers.trim() || env.trim() || clientSecret
          ? {
              ...(token ? { token } : {}),
              ...(headers.trim() ? { headers: map(headers) } : {}),
              ...(env.trim() ? { env: map(env) } : {}),
              ...(clientSecret ? { client_secret: clientSecret } : {}),
            }
          : undefined;
      await onSaved({ ...config, args: parsed }, secret, connect);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="page extension-library extension-editor">
      <button className="text-button back" disabled={busy} onClick={onBack}>
        <ArrowLeft size={16} />
        Plugins
      </button>
      <header>
        <h1>
          {appInfo?.name || (existing ? config.name : "Ajouter un serveur MCP")}
        </h1>
        <p>
          {appInfo?.description ||
            "Connectez un service distant ou un serveur installé sur votre ordinateur."}
        </p>
      </header>
      {appInfo?.note && <p className="muted">{appInfo.note}</p>}
      {appInfo && (
        <button
          className="text-button"
          onClick={() =>
            void openExternal(appInfo.docs).catch((e) => setError(String(e)))
          }
        >
          Documentation officielle
          <ArrowUpRight size={14} />
        </button>
      )}
      <section className="editor-section">
        <label className="field">
          Nom de la connexion
          <input
            value={config.name}
            maxLength={120}
            onChange={(e) => patch({ name: e.target.value })}
          />
        </label>
        <div className="extension-tabs" role="group" aria-label="Transport MCP">
          <button
            aria-pressed={config.transport === "http"}
            onClick={() => patch({ transport: "http" })}
          >
            <Globe size={15} />
            Serveur distant
          </button>
          <button
            aria-pressed={config.transport === "stdio"}
            onClick={() => patch({ transport: "stdio", auth: "none" })}
          >
            <Server size={15} />
            Serveur local
          </button>
        </div>
        {config.transport === "http" ? (
          <>
            <label className="field">
              URL du serveur MCP
              <input
                type="url"
                placeholder="https://…/mcp"
                value={config.url}
                onChange={(e) => patch({ url: e.target.value })}
              />
            </label>
            <label className="field">
              Connexion
              <select
                value={config.auth}
                onChange={(e) =>
                  patch({ auth: e.target.value as api.McpConfig["auth"] })
                }
              >
                <option value="none">Sans authentification</option>
                <option value="oauth">Compte · OAuth</option>
                <option value="token">Token d’accès</option>
              </select>
            </label>
            {config.auth === "token" && (
              <label className="field">
                Token d’accès
                <input
                  type="password"
                  autoComplete="off"
                  value={token}
                  placeholder={
                    existing
                      ? "Laisser vide pour conserver le token"
                      : "Votre token"
                  }
                  onChange={(e) => setToken(e.target.value)}
                />
              </label>
            )}
            {config.auth === "oauth" && (
              <p className="muted">
                Votre navigateur s’ouvrira pour autoriser Converge. Les jetons
                et leur renouvellement sont gérés dans le coffre du système.
              </p>
            )}
          </>
        ) : (
          <>
            <p className="muted">
              Un serveur local est un programme qui s’exécute avec les droits de
              votre compte. Ajoutez uniquement un serveur dont vous connaissez
              l’origine.
            </p>
            <label className="field">
              Exécutable
              <input
                value={config.command}
                placeholder="node, npx, uvx ou chemin de l’exécutable"
                onChange={(e) => patch({ command: e.target.value })}
              />
            </label>
            <label className="field">
              Arguments · liste JSON
              <textarea
                rows={4}
                spellCheck={false}
                value={args}
                onChange={(e) => setArgs(e.target.value)}
              />
            </label>
            <label className="field">
              Dossier de travail · facultatif
              <div className="extension-folder-field">
                <input
                  value={config.cwd}
                  onChange={(e) => patch({ cwd: e.target.value })}
                />
                <button
                  className="button"
                  onClick={() =>
                    void open({ directory: true, multiple: false })
                      .then((v) => {
                        if (typeof v === "string") patch({ cwd: v });
                      })
                      .catch((e) => setError(String(e)))
                  }
                >
                  <FolderOpen size={16} />
                  Choisir
                </button>
              </div>
            </label>
            <label className="field">
              Variables d’environnement · objet JSON
              <textarea
                rows={3}
                spellCheck={false}
                autoComplete="off"
                placeholder={
                  existing
                    ? "Laisser vide pour conserver les identifiants enregistrés"
                    : '{"VARIABLE": "valeur"}'
                }
                value={env}
                onChange={(e) => setEnv(e.target.value)}
              />
            </label>
          </>
        )}
        <details className="extension-advanced">
          <summary>Configuration avancée</summary>
          {config.transport === "http" && (
            <>
              <label className="field">
                Headers · objet JSON
                <textarea
                  rows={3}
                  autoComplete="off"
                  spellCheck={false}
                  value={headers}
                  placeholder={
                    existing
                      ? "Laisser vide pour conserver les headers"
                      : '{"X-Header": "valeur"}'
                  }
                  onChange={(e) => setHeaders(e.target.value)}
                />
              </label>
              {config.auth === "oauth" && (
                <>
                  <label className="field">
                    ID client OAuth · facultatif
                    <input
                      value={config.clientId}
                      placeholder="Si le service exige votre propre application OAuth"
                      onChange={(e) => patch({ clientId: e.target.value })}
                    />
                  </label>
                  <label className="field">
                    Secret client OAuth · facultatif
                    <input
                      type="password"
                      autoComplete="off"
                      value={clientSecret}
                      onChange={(e) => setClientSecret(e.target.value)}
                    />
                  </label>
                  <label className="field">
                    Scopes · séparés par des espaces
                    <input
                      value={config.scopes.join(" ")}
                      onChange={(e) =>
                        patch({
                          scopes: e.target.value.split(/\s+/).filter(Boolean),
                        })
                      }
                    />
                  </label>
                </>
              )}
            </>
          )}
          <label className="check">
            <input
              type="checkbox"
              checked={config.enabled}
              onChange={(e) => patch({ enabled: e.target.checked })}
            />
            Disponible dans les conversations
          </label>
        </details>
        <p className="muted">
          Les tokens, headers et variables sont conservés dans le coffre
          sécurisé du système.
        </p>
      </section>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="editor-footer">
        <button className="button" disabled={busy} onClick={onBack}>
          Annuler
        </button>
        <button
          className="button"
          disabled={busy || !config.name.trim()}
          onClick={() => void save(false)}
        >
          Enregistrer
        </button>
        <button
          className="button primary"
          disabled={busy || !config.enabled || !config.name.trim()}
          onClick={() => void save(true)}
        >
          {busy ? (
            <LoaderCircle size={15} className="spin" />
          ) : (
            <Plus size={15} />
          )}
          Enregistrer et connecter
        </button>
      </div>
    </div>
  );
}
