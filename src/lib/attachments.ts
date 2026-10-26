import { id, type Attachment } from "../types";
export async function attachmentFromFile(file: File): Promise<Attachment> {
  if (file.size > 10 * 1024 * 1024)
    throw Error(`« ${file.name || "Image"} » dépasse la limite de 10 Mo.`);
  if (file.type.startsWith("image/")) {
    const data = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(",")[1]);
      reader.onerror = () => reject(Error("Impossible de lire cette image."));
      reader.readAsDataURL(file);
    });
    return {
      id: id(),
      name:
        file.name && file.name !== "image.png"
          ? file.name
          : `Image ${new Date().toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" })}.png`,
      mime: file.type,
      data,
    };
  }
  return {
    id: id(),
    name: file.name,
    mime: "text/plain",
    data: await file.text(),
  };
}
