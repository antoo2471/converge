export function toolObservation(result: string | undefined): { image?: string; text: string } {
  if (!result) return { text: "" };
  if (result.startsWith("data:image/")) return { image: result, text: "Capture de l’écran jointe." };
  if (result.startsWith("{")) {
    try {
      const value = JSON.parse(result);
      if (value.kind === "computer_observation" && typeof value.image === "string" && /^data:image\/(png|jpeg);base64,/.test(value.image)) {
        const { image, ...metadata } = value;
        return { image, text: JSON.stringify(metadata) };
      }
    } catch { /* Ordinary tool results remain plain text. */ }
  }
  return { text: result };
}
