import { useState, useMemo, useId, type ReactNode } from "react";
import {
  ChevronLeft,
  ChevronRight,
  Info,
  CheckCircle2,
  AlertTriangle,
} from "lucide-react";
import { ZodError } from "zod";
import {
  parseUI,
  evaluate,
  type UINode,
  type Scalar,
  type UIAction,
  type UIDocument,
} from "./schema";
export type ActionHandler = (
  action: UIAction,
  state: Record<string, Scalar>,
) => void;
export function GenerativeUI({
  source,
  streaming = false,
  onAction,
}: {
  source: string;
  streaming?: boolean;
  onAction: ActionHandler;
}) {
  const parsed = useMemo(() => {
    if (streaming) return { pending: true };
    try {
      return { doc: parseUI(JSON.parse(source)) };
    } catch (e) {
      return {
        error:
          e instanceof ZodError
            ? e.issues
                .map(
                  (issue) =>
                    `${issue.path.join(".") || "document"} : ${issue.message}`,
                )
                .join("\n")
            : e instanceof SyntaxError
              ? "Le bloc JSON est incomplet ou mal formé."
              : String(e),
      };
    }
  }, [source, streaming]);
  if (parsed.pending)
    return (
      <div className="gui-pending" role="status">
        Création de l’interface…
      </div>
    );
  if (!parsed.doc)
    return (
      <div className="inline-error">
        <span>Cette interface n’a pas pu être affichée.</span>
        <details>
          <summary>Détails du format à corriger</summary>
          <pre>{parsed.error}</pre>
        </details>
      </div>
    );
  return <LiveUI key={source} doc={parsed.doc} onAction={onAction} />;
}
function LiveUI({
  doc,
  onAction,
}: {
  doc: UIDocument;
  onAction: ActionHandler;
}) {
  const [state, setState] = useState<Record<string, Scalar>>({ ...doc.state });
  const instance = useId();
  function action(a: UIAction) {
    if (a.type === "reset") setState({ ...doc.state });
    else if (a.type === "set")
      setState((s) => ({ ...s, [a.key]: evaluate(a.value, s) }));
    else onAction(a, state);
  }
  const update = (key: string | undefined, value: Scalar) => {
    if (key) setState((s) => ({ ...s, [key]: value }));
  };
  function render(n: UINode, index: number | string = 0): ReactNode {
    if (n.visible !== undefined && !evaluate(n.visible, state)) return null;
    const value = n.bind ? state[n.bind] : evaluate(n.value ?? n.text, state),
      label = n.label;
    const children = n.children?.map((child, i) => (
      <div className={`gui-node gui-node-${child.type}`} key={child.id || i}>
        {render(child, `${index}-${i}`)}
      </div>
    ));
    const inputId = `${instance}-${n.id || index}`;
    switch (n.type) {
      case "stack":
        return <div className="gui-stack">{children}</div>;
      case "row":
        return <div className="gui-row">{children}</div>;
      case "card":
      case "tab":
        return (
          <section className="gui-section">
            {(label || n.description) && (
              <div className="gui-section-heading">
                {label && <h4>{label}</h4>}
                {n.description && <p>{n.description}</p>}
              </div>
            )}
            {children}
          </section>
        );
      case "heading":
        return <h3>{String(value ?? label ?? "")}</h3>;
      case "text":
        return <p>{String(value ?? label ?? "")}</p>;
      case "badge":
        return (
          <span className={`gui-badge gui-tone-${n.tone || "neutral"}`}>
            {String(value ?? label ?? "")}
          </span>
        );
      case "notice": {
        const Icon =
          n.tone === "positive"
            ? CheckCircle2
            : n.tone === "warning" || n.tone === "negative"
              ? AlertTriangle
              : Info;
        return (
          <aside className={`gui-notice gui-tone-${n.tone || "neutral"}`}>
            <Icon size={16} />
            <div>
              {label && <strong>{label}</strong>}
              {value !== null && <p>{String(value)}</p>}
              {n.description && <p>{n.description}</p>}
            </div>
          </aside>
        );
      }
      case "divider":
        return <hr />;
      case "metric":
        return (
          <div className={`gui-metric gui-tone-${n.tone || "neutral"}`}>
            <span>{label}</span>
            <strong>
              {typeof value === "number"
                ? new Intl.NumberFormat("fr-FR", {
                    maximumFractionDigits: 3,
                  }).format(value)
                : String(value ?? "—")}
            </strong>
            {n.unit && <span className="gui-metric-unit">{n.unit}</span>}
            {n.delta !== undefined && (
              <small className="gui-metric-delta">
                {String(evaluate(n.delta, state) ?? "")}
              </small>
            )}
            {n.description && (
              <small className="gui-metric-description">{n.description}</small>
            )}
          </div>
        );
      case "input":
      case "number":
      case "textarea":
        return (
          <label className="field" htmlFor={inputId}>
            {label}
            {n.type === "textarea" ? (
              <textarea
                id={inputId}
                placeholder={n.placeholder}
                value={String(value ?? "")}
                onChange={(e) => update(n.bind, e.target.value)}
              />
            ) : (
              <input
                id={inputId}
                placeholder={n.placeholder}
                type={n.type === "number" ? "number" : "text"}
                min={n.min}
                max={n.max}
                step={n.step}
                value={String(value ?? "")}
                onChange={(e) =>
                  update(
                    n.bind,
                    n.type === "number"
                      ? Number(e.target.value)
                      : e.target.value,
                  )
                }
              />
            )}
          </label>
        );
      case "slider":
        return (
          <label className="field" htmlFor={inputId}>
            <span className="between">
              {label}
              <output>
                {String(value ?? n.min ?? 0)}
                {n.unit}
              </output>
            </span>
            <input
              id={inputId}
              type="range"
              min={n.min ?? 0}
              max={n.max ?? 100}
              step={n.step ?? 1}
              value={Number(value ?? n.min ?? 0)}
              onChange={(e) => update(n.bind, Number(e.target.value))}
            />
          </label>
        );
      case "checkbox":
        return (
          <label className="check">
            <input
              type="checkbox"
              checked={Boolean(value)}
              onChange={(e) => update(n.bind, e.target.checked)}
            />
            {label}
          </label>
        );
      case "radio":
        return (
          <fieldset className="gui-radio">
            <legend>{label}</legend>
            {n.options?.map((o, i) => (
              <label
                className={`gui-radio-option ${value === o.value ? "selected" : ""}`}
                key={i}
              >
                <input
                  type="radio"
                  name={inputId}
                  checked={value === o.value}
                  onChange={() => update(n.bind, o.value)}
                />
                {o.label}
              </label>
            ))}
          </fieldset>
        );
      case "select":
        return (
          <label className="field">
            {label}
            <select
              value={String(
                n.options?.findIndex((o) => o.value === value) ?? -1,
              )}
              onChange={(e) =>
                update(
                  n.bind,
                  n.options?.[Number(e.target.value)]?.value ?? null,
                )
              }
            >
              <option value="-1" disabled>
                Choisir…
              </option>
              {n.options?.map((o, i) => (
                <option key={i} value={i}>
                  {o.label}
                </option>
              ))}
            </select>
          </label>
        );
      case "button":
        return (
          <button
            className="button"
            type="button"
            disabled={!n.action}
            onClick={() => n.action && action(n.action)}
          >
            {label || String(value ?? "Valider")}
          </button>
        );
      case "form":
        return (
          <form
            className="gui-stack"
            onSubmit={(e) => {
              e.preventDefault();
              if (n.action) action(n.action);
            }}
          >
            {children}
            <button type="submit" className="button primary">
              {label || "Valider"}
            </button>
          </form>
        );
      case "progress":
        return (
          <label className="field">
            <span className="between">
              <span>{label}</span>
              <output>
                {Number(value ?? 0)}
                {n.unit}
              </output>
            </span>
            <progress max={n.max ?? 100} value={Number(value ?? 0)} />
          </label>
        );
      case "table":
        return (
          <InteractiveTable
            columns={n.columns || []}
            rows={(n.rows || []).map((row) =>
              Object.fromEntries(
                Object.entries(row).map(([k, v]) => [k, evaluate(v, state)]),
              ),
            )}
          />
        );
      case "chart":
        return (
          <Chart
            label={label}
            variant={n.variant}
            points={(n.points || []).map((p) => ({
              label: p.label,
              value: Number(evaluate(p.value, state)) || 0,
            }))}
          />
        );
      case "tabs":
      case "carousel":
        return <Pages kind={n.type} nodes={n.children || []} render={render} />;
      case "diagram":
        return (
          <div className="gui-diagram">
            {n.children?.map((c, i) => (
              <div key={i}>
                {i > 0 && <ChevronRight size={18} />}
                <div>{render(c, i)}</div>
              </div>
            ))}
          </div>
        );
      case "map":
        return (
          <CoordinateMap
            lat={Number(evaluate(n.lat, state)) || 0}
            lng={Number(evaluate(n.lng, state)) || 0}
            zoom={n.zoom || 2}
            label={label}
          />
        );
      default:
        return null;
    }
  }
  return (
    <section className="generative-ui" aria-label={doc.title}>
      <div className="gui-body">{render(doc.root)}</div>
    </section>
  );
}
function Pages({
  kind,
  nodes,
  render,
}: {
  kind: string;
  nodes: UINode[];
  render: (n: UINode, i: number) => ReactNode;
}) {
  const [active, setActive] = useState(0);
  return (
    <div>
      <div className="gui-tabs">
        {kind === "tabs" ? (
          nodes.map((n, i) => (
            <button
              key={i}
              className={i === active ? "selected" : ""}
              onClick={() => setActive(i)}
            >
              {n.label || `Onglet ${i + 1}`}
            </button>
          ))
        ) : (
          <>
            <button
              className="icon-button"
              onClick={() => setActive((i) => Math.max(0, i - 1))}
              disabled={!active}
            >
              <ChevronLeft size={16} />
            </button>
            <span>
              {active + 1} / {nodes.length}
            </span>
            <button
              className="icon-button"
              onClick={() =>
                setActive((i) => Math.min(nodes.length - 1, i + 1))
              }
              disabled={active >= nodes.length - 1}
            >
              <ChevronRight size={16} />
            </button>
          </>
        )}
      </div>
      {nodes[active] && render(nodes[active], active)}
    </div>
  );
}
function InteractiveTable({
  columns,
  rows,
}: {
  columns: string[];
  rows: Record<string, Scalar>[];
}) {
  const [sort, setSort] = useState(""),
    [reverse, setReverse] = useState(false),
    [query, setQuery] = useState("");
  const data = rows
    .filter((row) =>
      Object.values(row).join(" ").toLowerCase().includes(query.toLowerCase()),
    )
    .sort((a, b) => {
      const x = a[sort],
        y = b[sort];
      return (
        (typeof x === "number" && typeof y === "number"
          ? x - y
          : String(x ?? "").localeCompare(String(y ?? ""))) * (reverse ? -1 : 1)
      );
    });
  return (
    <div className="gui-table">
      <input
        aria-label="Filtrer le tableau"
        placeholder="Filtrer…"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      <div className="table-scroll">
        <table>
          <thead>
            <tr>
              {columns.map((c) => (
                <th key={c}>
                  <button
                    onClick={() => {
                      setSort(c);
                      setReverse(sort === c ? !reverse : false);
                    }}
                  >
                    {c}
                    {sort === c ? (reverse ? " ↓" : " ↑") : ""}
                  </button>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.map((r, i) => (
              <tr key={i}>
                {columns.map((c) => (
                  <td key={c}>{String(r[c] ?? "")}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
function Chart({
  points,
  label,
  variant,
}: {
  points: { label: string; value: number }[];
  label?: string;
  variant?: string;
}) {
  const [hidden, setHidden] = useState<number[]>([]);
  const [hover, setHover] = useState<number | null>(null);
  const visible = points
    .map((p, index) => ({ ...p, index }))
    .filter((p) => !hidden.includes(p.index));
  const minimum = Math.min(0, ...visible.map((p) => p.value));
  const maximum = Math.max(1, ...visible.map((p) => p.value));
  const w = 600,
    h = 244,
    left = 48,
    right = 16,
    top = 16,
    bottom = 32;
  const plotWidth = w - left - right,
    plotHeight = h - top - bottom;
  const y = (value: number) =>
    top + ((maximum - value) / (maximum - minimum)) * plotHeight;
  const x = (index: number) =>
    variant === "line"
      ? left +
        (visible.length === 1
          ? plotWidth / 2
          : (index * plotWidth) / Math.max(1, visible.length - 1))
      : left + ((index + 0.5) * plotWidth) / Math.max(1, visible.length);
  const number = new Intl.NumberFormat("fr-FR", {
    maximumFractionDigits: 1,
    notation: "compact",
  });
  const active = visible.find((p) => p.index === hover);
  return (
    <figure className="gui-chart">
      {(label || active) && (
        <figcaption>
          <span>{label}</span>
          <strong>
            {active ? `${active.label} : ${number.format(active.value)}` : ""}
          </strong>
        </figcaption>
      )}
      <div className="gui-chart-plot">
        {!visible.length ? (
          <p className="gui-chart-empty">Aucune donnée affichée</p>
        ) : (
          <svg
            role="img"
            aria-label={label || "Graphique interactif"}
            viewBox={`0 0 ${w} ${h}`}
          >
            {[0, 0.25, 0.5, 0.75, 1].map((tick) => {
              const value = minimum + tick * (maximum - minimum);
              return (
                <g key={tick}>
                  <line
                    x1={left}
                    x2={w - right}
                    y1={y(value)}
                    y2={y(value)}
                    stroke="var(--line)"
                    strokeDasharray="3 4"
                  />
                  <text
                    x={left - 10}
                    y={y(value) + 3}
                    textAnchor="end"
                    fill="var(--muted)"
                    fontSize="10"
                  >
                    {number.format(value)}
                  </text>
                </g>
              );
            })}
            {variant === "line" && (
              <polyline
                fill="none"
                stroke="var(--gui-chart-color)"
                strokeWidth="2"
                strokeLinejoin="round"
                points={visible
                  .map((p, i) => `${x(i)},${y(p.value)}`)
                  .join(" ")}
              />
            )}
            {visible.map((p, i) => (
              <g
                key={p.index}
                tabIndex={0}
                role="graphics-symbol"
                aria-label={`${p.label}: ${p.value}`}
                onFocus={() => setHover(p.index)}
                onBlur={() => setHover(null)}
                onMouseEnter={() => setHover(p.index)}
                onMouseLeave={() => setHover(null)}
              >
                <title>
                  {p.label}: {p.value}
                </title>
                {variant === "line" ? (
                  <>
                    <rect
                      x={x(i) - 12}
                      y={top}
                      width="24"
                      height={plotHeight}
                      fill="transparent"
                    />
                    <circle
                      cx={x(i)}
                      cy={y(p.value)}
                      r={hover === p.index ? 5 : 3}
                      fill="var(--gui-chart-color)"
                    />
                  </>
                ) : (
                  <rect
                    x={
                      x(i) -
                      Math.max(2, (plotWidth / visible.length) * 0.65) / 2
                    }
                    y={Math.min(y(0), y(p.value))}
                    width={Math.max(2, (plotWidth / visible.length) * 0.65)}
                    height={Math.max(1, Math.abs(y(p.value) - y(0)))}
                    rx="3"
                    fill="var(--gui-chart-color)"
                    opacity={hover === null || hover === p.index ? 0.9 : 0.4}
                  />
                )}
                {(visible.length <= 12 ||
                  i % Math.ceil(visible.length / 8) === 0 ||
                  i === visible.length - 1) && (
                  <text
                    x={x(i)}
                    y={h - 10}
                    textAnchor="middle"
                    fill="var(--muted)"
                    fontSize="10"
                  >
                    {p.label.slice(0, 12)}
                  </text>
                )}
              </g>
            ))}
          </svg>
        )}
      </div>
      <div className="chart-legend">
        {points.map((p, i) => (
          <button
            type="button"
            key={i}
            aria-pressed={!hidden.includes(i)}
            onClick={() => {
              setHover(null);
              setHidden((current) =>
                current.includes(i)
                  ? current.filter((v) => v !== i)
                  : [...current, i],
              );
            }}
          >
            <span className="chart-legend-dot" />
            {p.label}
          </button>
        ))}
      </div>
    </figure>
  );
}
function CoordinateMap({
  lat,
  lng,
  zoom,
  label,
}: {
  lat: number;
  lng: number;
  zoom: number;
  label?: string;
}) {
  const [z, setZ] = useState(zoom),
    [offset, setOffset] = useState([0, 0]);
  return (
    <figure className="coordinate-map">
      <figcaption>
        {label || "Carte de coordonnées"} · {lat.toFixed(4)}, {lng.toFixed(4)}
      </figcaption>
      <svg
        viewBox="0 0 600 240"
        role="img"
        aria-label="Carte de coordonnées hors ligne"
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
        }}
        onPointerMove={(e) => {
          if (e.buttons)
            setOffset(([x, y]) => [x + e.movementX, y + e.movementY]);
        }}
      >
        <rect width="600" height="240" fill="var(--surface)" />
        {Array.from({ length: 13 }, (_, i) => (
          <g key={i}>
            <path
              d={`M ${i * 50 + (offset[0] % 50)} 0 V240 M0 ${i * 50 + (offset[1] % 50)} H600`}
              stroke="var(--line)"
            />
            <text x={i * 50} y="230" fill="var(--muted)" fontSize="9">
              {(lng + ((i - 6) * 180) / 2 ** z).toFixed(1)}°
            </text>
          </g>
        ))}
        <circle
          cx={300 + offset[0]}
          cy={110 + offset[1]}
          r="7"
          fill="var(--accent)"
        />
        <text
          x={315 + offset[0]}
          y={114 + offset[1]}
          fill="var(--text)"
          fontSize="12"
        >
          {lat.toFixed(3)}°, {lng.toFixed(3)}°
        </text>
      </svg>
      <div className="between">
        <small>Coordonnées hors ligne · glisser pour déplacer</small>
        <div>
          <button
            className="button"
            onClick={() => setZ((v) => Math.max(1, v - 1))}
          >
            −
          </button>
          <button
            className="button"
            onClick={() => setZ((v) => Math.min(18, v + 1))}
          >
            +
          </button>
        </div>
      </div>
    </figure>
  );
}
