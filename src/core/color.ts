export function hexToHsv(hex: string) {
  const [r, g, b] = [1, 3, 5].map(
    (i) => parseInt(hex.slice(i, i + 2), 16) / 255,
  );
  const max = Math.max(r, g, b),
    min = Math.min(r, g, b),
    delta = max - min;
  let h =
    delta === 0
      ? 0
      : max === r
        ? ((g - b) / delta) % 6
        : max === g
          ? (b - r) / delta + 2
          : (r - g) / delta + 4;
  h = (h * 60 + 360) % 360;
  return { h, s: max === 0 ? 0 : (delta / max) * 100, v: max * 100 };
}
export function hsvToHex(h: number, s: number, v: number) {
  const chroma = ((v / 100) * s) / 100,
    x = chroma * (1 - Math.abs(((h / 60) % 2) - 1)),
    m = v / 100 - chroma;
  const channels =
    h < 60
      ? [chroma, x, 0]
      : h < 120
        ? [x, chroma, 0]
        : h < 180
          ? [0, chroma, x]
          : h < 240
            ? [0, x, chroma]
            : h < 300
              ? [x, 0, chroma]
              : [chroma, 0, x];
  return (
    "#" +
    channels
      .map((c) =>
        Math.round((c + m) * 255)
          .toString(16)
          .padStart(2, "0"),
      )
      .join("")
  );
}
