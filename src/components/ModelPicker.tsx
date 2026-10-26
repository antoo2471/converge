import { useState, useRef, useEffect, useId } from "react";
import { createPortal } from "react-dom";
import { ChevronDown, Search, Check, Plus, Box } from "lucide-react";
import { ProviderMark } from "./ProviderBrand";
import { isPreset } from "../core/providerPresets";
export interface ModelOption {
  label?: string;
  value: string;
  model: string;
  provider: { name: string; id: string; plugin?: string };
}
export function ModelPicker({
  options,
  value,
  onChange,
  onAdd,
  label = "Choisir un modèle",
  compact = false,
  triggerText,
  disabled = false,
}: {
  options: ModelOption[];
  value: string;
  onChange: (value: string) => void;
  onAdd?: () => void;
  label?: string;
  compact?: boolean;
  triggerText?: string;
  disabled?: boolean;
}) {
  const [position, setPosition] = useState<{
    left: number;
    top?: number;
    bottom?: number;
    width: number;
    height: number;
  } | null>(null);
  const [query, setQuery] = useState("");
  const [providerId, setProviderId] = useState("");
  const [index, setIndex] = useState(0);
  const trigger = useRef<HTMLButtonElement>(null),
    popup = useRef<HTMLDivElement>(null),
    search = useRef<HTMLInputElement>(null);
  const listId = useId();
  const selected = options.find((o) => o.value === value);
  const matches = options.filter((o) =>
    `${o.provider.name} ${o.label || ""} ${o.model}`
      .toLowerCase()
      .includes(query.toLowerCase().trim()),
  );
  const providers = Array.from(
    new Map(matches.map((o) => [o.provider.id, o.provider])).values(),
  );
  const activeProvider =
    providers.find((p) => p.id === providerId) || providers[0];
  const models = matches.filter((o) => o.provider.id === activeProvider?.id);
  const activeIndex = Math.min(index, Math.max(0, models.length - 1));
  const close = (restore = false) => {
    setPosition(null);
    if (restore) trigger.current?.focus();
  };
  const choose = (option: ModelOption) => {
    close(true);
    onChange(option.value);
  };
  useEffect(() => {
    if (!position) return;
    search.current?.focus({ preventScroll: true });
    const down = (e: PointerEvent) => {
      if (
        !popup.current?.contains(e.target as Node) &&
        !trigger.current?.contains(e.target as Node)
      )
        setPosition(null);
    };
    const scroll = (e: Event) => {
      if (!popup.current?.contains(e.target as Node)) setPosition(null);
    };
    const resize = () => setPosition(null);
    document.addEventListener("pointerdown", down);
    document.addEventListener("scroll", scroll, true);
    window.addEventListener("resize", resize);
    return () => {
      document.removeEventListener("pointerdown", down);
      document.removeEventListener("scroll", scroll, true);
      window.removeEventListener("resize", resize);
    };
  }, [position]);
  useEffect(() => {
    if (position)
      popup.current
        ?.querySelectorAll<HTMLElement>('[role="option"]')
        [activeIndex]?.scrollIntoView?.({ block: "nearest" });
  }, [position, activeIndex, activeProvider?.id, query]);
  function toggle() {
    if (position) {
      close();
      return;
    }
    const rect = trigger.current!.getBoundingClientRect(),
      edge = 12,
      gap = 8;
    const below = window.innerHeight - rect.bottom - edge - gap,
      above = rect.top - edge - gap;
    const upwards = below < 390 && above > below,
      width = Math.min(440, window.innerWidth - edge * 2);
    setQuery("");
    setProviderId(selected?.provider.id || options[0]?.provider.id || "");
    setIndex(
      Math.max(
        0,
        options
          .filter((o) => o.provider.id === selected?.provider.id)
          .findIndex((o) => o.value === value),
      ),
    );
    setPosition({
      left: Math.max(
        edge,
        Math.min(rect.right - width, window.innerWidth - width - edge),
      ),
      ...(upwards
        ? { bottom: window.innerHeight - rect.top + gap }
        : { top: rect.bottom + gap }),
      width,
      height: Math.min(390, Math.max(0, upwards ? above : below)),
    });
  }
  return (
    <div className={`model-select ${compact ? "compact" : ""}`}>
      <button
        ref={trigger}
        type="button"
        className={
          triggerText ? "thread-model-trigger" : "model-select-trigger"
        }
        aria-label={label}
        aria-expanded={!!position}
        aria-haspopup="dialog"
        disabled={disabled}
        onClick={toggle}
      >
        {!compact && !triggerText && <Box size={15} />}
        <span>
          {triggerText || selected?.label || selected?.model || label}
        </span>
        <ChevronDown size={13} />
      </button>
      {position &&
        createPortal(
          <div
            ref={popup}
            className="model-browser"
            role="dialog"
            aria-label={label}
            style={position}
            onBlur={(event) => {
              if (
                event.relatedTarget &&
                !event.currentTarget.contains(event.relatedTarget) &&
                !trigger.current?.contains(event.relatedTarget)
              )
                close();
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                close(true);
              }
              if (
                e.target === search.current ||
                (e.target as HTMLElement).getAttribute("role") === "option"
              ) {
                if (
                  ["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key) &&
                  (e.target !== search.current || e.key.startsWith("Arrow"))
                ) {
                  e.preventDefault();
                  const next =
                    e.key === "Home"
                      ? 0
                      : e.key === "End"
                        ? models.length - 1
                        : (activeIndex +
                            (e.key === "ArrowDown" ? 1 : -1) +
                            models.length) %
                          Math.max(1, models.length);
                  setIndex(next);
                  if (e.target !== search.current)
                    popup.current
                      ?.querySelectorAll<HTMLElement>('[role="option"]')
                      [next]?.focus();
                }
                if (
                  e.key === "Enter" &&
                  e.target === search.current &&
                  models[activeIndex]
                ) {
                  e.preventDefault();
                  choose(models[activeIndex]);
                }
              }
            }}
          >
            <div
              className="model-provider-rail"
              role="toolbar"
              aria-label="Fournisseurs"
              aria-orientation="vertical"
            >
              <div className="model-provider-list">
                {providers.map((provider, i) => (
                  <button
                    type="button"
                    key={provider.id}
                    className="model-provider-badge"
                    title={provider.name}
                    aria-label={provider.name}
                    aria-pressed={provider.id === activeProvider?.id}
                    onClick={() => {
                      setProviderId(provider.id);
                      setIndex(0);
                    }}
                    onKeyDown={(e) => {
                      if (["ArrowDown", "ArrowUp"].includes(e.key)) {
                        e.preventDefault();
                        const next =
                          (i +
                            (e.key === "ArrowDown" ? 1 : -1) +
                            providers.length) %
                          providers.length;
                        setProviderId(providers[next].id);
                        setIndex(0);
                        e.currentTarget.parentElement
                          ?.querySelectorAll<HTMLButtonElement>("button")
                          [next]?.focus();
                      }
                      if (e.key === "ArrowRight") {
                        e.preventDefault();
                        popup.current
                          ?.querySelector<HTMLElement>('[role="option"]')
                          ?.focus();
                      }
                    }}
                  >
                    {provider.plugin === "chatgpt" ||
                    isPreset(provider.plugin || "") ? (
                      <ProviderMark plugin={provider.plugin!} />
                    ) : (
                      <span>
                        {provider.name
                          .trim()
                          .split(/\s+/)
                          .slice(0, 2)
                          .map((word) => Array.from(word)[0])
                          .join("")
                          .toUpperCase() || "AI"}
                      </span>
                    )}
                  </button>
                ))}
              </div>
              {onAdd && (
                <button
                  type="button"
                  className="model-provider-add"
                  title="Ajouter un fournisseur"
                  aria-label="Ajouter un fournisseur"
                  onClick={() => {
                    close();
                    onAdd();
                  }}
                >
                  <Plus size={18} />
                </button>
              )}
            </div>
            <div className="model-browser-main">
              <label className="model-browser-search">
                <Search size={15} />
                <input
                  ref={search}
                  value={query}
                  onChange={(e) => {
                    setQuery(e.target.value);
                    setIndex(0);
                  }}
                  placeholder="Rechercher un modèle…"
                  aria-label="Rechercher un modèle"
                  role="combobox"
                  aria-expanded="true"
                  aria-controls={listId}
                  aria-activedescendant={
                    models[activeIndex] ? `${listId}-${activeIndex}` : undefined
                  }
                  aria-autocomplete="list"
                />
              </label>
              <div className="model-browser-heading">
                <strong>{activeProvider?.name || "Vos modèles"}</strong>
                <span>{models.length}</span>
              </div>
              <div
                className="model-browser-models"
                id={listId}
                role="listbox"
                aria-label={
                  activeProvider
                    ? `Modèles de ${activeProvider.name}`
                    : "Modèles"
                }
              >
                {models.map((option, i) => (
                  <button
                    type="button"
                    role="option"
                    id={`${listId}-${i}`}
                    aria-selected={value === option.value}
                    className={i === activeIndex ? "focused" : ""}
                    key={option.value}
                    onMouseEnter={() => setIndex(i)}
                    onFocus={() => setIndex(i)}
                    onClick={() => choose(option)}
                  >
                    <span>{option.label || option.model}</span>
                    {value === option.value && <Check size={15} />}
                  </button>
                ))}
                {!models.length && (
                  <p>
                    {options.length
                      ? "Aucun modèle trouvé."
                      : "Ajoutez un fournisseur pour retrouver ses modèles ici."}
                  </p>
                )}
              </div>
            </div>
          </div>,
          document.body,
        )}
    </div>
  );
}
