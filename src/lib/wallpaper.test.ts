import { expect, it } from "vitest";
import { readWallpaper } from "./wallpaper";
it("recognizes GIF bytes despite an absent or incorrect MIME type, without changing the data", async () => {
  const encoded = "R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
  const bytes = Uint8Array.from(atob(encoded), (c) => c.charCodeAt(0));
  for (const type of ["", "application/octet-stream", "image/gif"]) {
    expect(
      await readWallpaper(new File([bytes], "wallpaper.GIF", { type })),
    ).toBe(`data:image/gif;base64,${encoded}`);
  }
});
it("rejects a file merely named GIF when it does not contain GIF data", async () => {
  await expect(
    readWallpaper(
      new File(["<svg></svg>"], "not-an-image.gif", { type: "image/gif" }),
    ),
  ).rejects.toThrow();
});
