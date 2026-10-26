import { parseUI, type UIDocument } from "./schema";
/** External formats are explicit adapters. They cannot insert React, HTML or privileged callbacks. */
export interface UIAdapter {
  format: string;
  version: string;
  convert(input: unknown): UIDocument;
}
export class UIRendererRegistry {
  private adapters = new Map<string, UIAdapter>();
  register(adapter: UIAdapter) {
    if (this.adapters.has(adapter.format))
      throw Error("Adaptateur déjà installé");
    this.adapters.set(adapter.format, adapter);
  }
  decode(format: string, input: unknown) {
    const adapter = this.adapters.get(format);
    if (!adapter) throw Error(`Format UI non supporté : ${format}`);
    return parseUI(adapter.convert(input));
  }
}
export const uiAdapters = new UIRendererRegistry();
uiAdapters.register({
  format: "converge-ui",
  version: "1.0",
  convert: parseUI,
});
