import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ChevronDown } from "lucide-react";
import { effortLabels, type ReasoningEffort } from "../core/reasoning";
export function EffortPicker({ value, levels, onChange }: {
  value: ReasoningEffort;
  levels: readonly ReasoningEffort[];
  onChange: (value: ReasoningEffort) => void;
}) {
  const [position, setPosition] = useState<{ left: number; bottom: number } | null>(null);
  const trigger = useRef<HTMLButtonElement>(null), popup = useRef<HTMLDivElement>(null);
  const close = (restore = false) => { setPosition(null); if (restore) trigger.current?.focus(); };
  useEffect(() => {
    if (!position) return;
    popup.current?.querySelector<HTMLInputElement>('input[type="range"]')?.focus({ preventScroll: true });
    const outside = (event: PointerEvent) => {
      if (!popup.current?.contains(event.target as Node) && !trigger.current?.contains(event.target as Node)) close();
    };
    const reposition = () => close();
    document.addEventListener("pointerdown", outside);
    window.addEventListener("resize", reposition);
    return () => { document.removeEventListener("pointerdown", outside); window.removeEventListener("resize", reposition); };
  }, [position]);
  if (!levels.length) return null;
  const index = Math.max(0, levels.indexOf(value));
  const progress = levels.length > 1 ? index / (levels.length - 1) * 100 : 0;
  return <>
    <button ref={trigger} type="button" className="effort-trigger" aria-label={`Effort de raisonnement : ${effortLabels[value]}`} aria-haspopup="dialog" aria-expanded={!!position} onClick={() => {
      if (position) { close(); return; }
      const rect = trigger.current!.getBoundingClientRect();
      setPosition({ left: Math.max(8, Math.min(rect.right - 272, window.innerWidth - 280)), bottom: window.innerHeight - rect.top + 8 });
    }}>{effortLabels[value]}<ChevronDown size={12}/></button>
    {position && createPortal(<div ref={popup} role="dialog" aria-label="Effort de raisonnement" className="effort-popup" style={position} onKeyDown={event => {
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(true); }
      if (event.key === "Tab") {
        const items = Array.from(popup.current!.querySelectorAll<HTMLElement>('button:not(:disabled), input'));
        const current = items.indexOf(document.activeElement as HTMLElement);
        if ((event.shiftKey && current === 0) || (!event.shiftKey && current === items.length - 1)) close(true);
      }
    }}>
      <div className="effort-heading">
        <strong>Raisonnement</strong>
        <span className="effort-current" aria-live="polite">{effortLabels[value]}</span>
      </div>
      <div className="effort-slider">
        <div className="effort-slider-track" aria-hidden="true">
          <div className="effort-slider-fill" style={{ width: `calc(8px + (100% - 16px) * ${progress / 100})` }}/>
          <div className="effort-slider-stops">{levels.map(level => <i key={level}/>)}</div>
          <div className="effort-slider-thumb" style={{ left: `calc((100% - 16px) * ${progress / 100})` }}/>
        </div>
        <input type="range" aria-label="Effort de raisonnement" min={0} max={levels.length - 1} step={1} value={index} aria-valuetext={effortLabels[value]} onChange={event => { const level = levels[Number(event.target.value)]; if (level) onChange(level); }}/>
      </div>
      <div className="effort-footer">
        <button type="button" className="effort-reset" aria-label="Rétablir l’effort automatique" onClick={() => onChange("auto")} disabled={value === "auto"}>Auto</button>
        <span>{effortLabels[levels[levels.length - 1]]}</span>
      </div>
    </div>, document.body)}
  </>;
}
