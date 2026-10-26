import { useEffect, useRef, useState, useId } from "react";
import { ChevronDown } from "lucide-react";
import { hexToHsv, hsvToHex } from "../core/color";

export function ColorPicker({
  label,
  value,
  automatic,
  onChange,
}: {
  label: string;
  value: string;
  automatic: boolean;
  onChange: (hex: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [hsv, setHsv] = useState(() => hexToHsv(value));
  const [hex, setHex] = useState(value);
  const last = useRef(value);
  const id = useId();
  useEffect(() => {
    if (value !== last.current) {
      setHsv(hexToHsv(value));
      setHex(value);
      last.current = value;
    }
  }, [value]);
  function update(next: typeof hsv) {
    setHsv(next);
    const color = hsvToHex(next.h, next.s, next.v);
    last.current = color;
    setHex(color);
    onChange(color);
  }
  const valid = /^#[a-f\d]{6}$/i.test(hex);
  return (
    <div className="custom-color-picker">
      <button
        type="button"
        className="custom-color-trigger"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen((v) => !v)}
      >
        <span
          className="custom-color-swatch"
          style={{ backgroundColor: value }}
        />
        <span>
          {label}
          <small>
            {automatic ? "Automatique selon le thème" : value.toUpperCase()}
          </small>
        </span>
        <ChevronDown size={13} />
      </button>
      {open && (
        <div
          className="custom-color-panel"
          id={id}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.stopPropagation();
              setOpen(false);
            }
          }}
        >
          <div
            className="custom-color-plane"
            aria-hidden="true"
            style={{ backgroundColor: hsvToHex(hsv.h, 100, 100) }}
            onPointerDown={(e) => {
              e.currentTarget.setPointerCapture(e.pointerId);
              const rect = e.currentTarget.getBoundingClientRect();
              update({
                ...hsv,
                s: Math.min(
                  100,
                  Math.max(0, ((e.clientX - rect.left) / rect.width) * 100),
                ),
                v: Math.min(
                  100,
                  Math.max(0, (1 - (e.clientY - rect.top) / rect.height) * 100),
                ),
              });
            }}
            onPointerMove={(e) => {
              if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
              const rect = e.currentTarget.getBoundingClientRect();
              update({
                ...hsv,
                s: Math.min(
                  100,
                  Math.max(0, ((e.clientX - rect.left) / rect.width) * 100),
                ),
                v: Math.min(
                  100,
                  Math.max(0, (1 - (e.clientY - rect.top) / rect.height) * 100),
                ),
              });
            }}
          >
            <span style={{ left: `${hsv.s}%`, top: `${100 - hsv.v}%` }} />
          </div>
          {(
            [
              ["h", "Teinte", 360],
              ["s", "Saturation", 100],
              ["v", "Luminosité", 100],
            ] as const
          ).map(([key, name, max]) => (
            <label className="custom-color-range" key={key}>
              <span>
                {name}
                <output>
                  {Math.round(hsv[key])}
                  {key === "h" ? "°" : "%"}
                </output>
              </span>
              <input
                className={key === "h" ? "custom-hue-range" : ""}
                type="range"
                aria-label={`${label} : ${name}`}
                min={0}
                max={max}
                value={hsv[key]}
                onChange={(e) =>
                  update({ ...hsv, [key]: Number(e.target.value) })
                }
              />
            </label>
          ))}
          <label className="custom-color-hex">
            HEX
            <input
              aria-label={`${label} : code hexadécimal`}
              value={hex}
              spellCheck={false}
              maxLength={7}
              aria-invalid={!valid}
              onChange={(e) => {
                const next = e.target.value.startsWith("#")
                  ? e.target.value
                  : `#${e.target.value}`;
                setHex(next);
                if (/^#[a-f\d]{6}$/i.test(next)) {
                  last.current = next;
                  setHsv(hexToHsv(next));
                  onChange(next);
                }
              }}
              onBlur={() => {
                if (!valid) setHex(value);
              }}
            />
          </label>
        </div>
      )}
    </div>
  );
}
