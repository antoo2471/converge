import { expect, it } from "vitest";
import { hexToHsv, hsvToHex } from "./color";
it("round trips grayscale, saturated and mixed colors", () => {
  for (const hex of [
    "#000000",
    "#ffffff",
    "#ff0000",
    "#00ff00",
    "#0000ff",
    "#123456",
    "#ab78cd",
  ]) {
    const hsv = hexToHsv(hex);
    expect(hsvToHex(hsv.h, hsv.s, hsv.v)).toBe(hex);
  }
  expect(hsvToHex(360, 100, 100)).toBe("#ff0000");
});
