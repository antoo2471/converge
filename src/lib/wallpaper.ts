export async function readWallpaper(file: File): Promise<string> {
  if (file.size > 12 * 1024 * 1024)
    throw Error("Le fond d’écran dépasse la limite de 12 Mo.");
  const bytes = new Uint8Array(await file.arrayBuffer());
  const header = String.fromCharCode(...bytes.slice(0, 64));
  const mime =
    header.startsWith("GIF87a") || header.startsWith("GIF89a")
      ? "image/gif"
      : bytes[0] === 137 &&
          header.slice(1, 4) === "PNG" &&
          bytes[4] === 13 &&
          bytes[5] === 10
        ? "image/png"
        : bytes[0] === 255 && bytes[1] === 216 && bytes[2] === 255
          ? "image/jpeg"
          : header.startsWith("RIFF") && header.slice(8, 12) === "WEBP"
            ? "image/webp"
            : header.slice(4, 8) === "ftyp" && /avif|avis/.test(header.slice(8))
              ? "image/avif"
              : "";
  if (!mime)
    throw Error(
      "Ce fichier n’est pas une image PNG, JPG, WebP, AVIF ou GIF reconnue.",
    );
  // Keep the original bytes, including every GIF animation frame. Do not flatten through canvas.
  let binary = "";
  for (let i = 0; i < bytes.length; i += 32768)
    binary += String.fromCharCode(...bytes.subarray(i, i + 32768));
  return `data:${mime};base64,${btoa(binary)}`;
}
