import { useMemo } from "react";
import { appearanceSchema } from "../core/appearance";

export function Wallpaper({
  source,
  preview = false,
  onError,
}: {
  source?: string;
  preview?: boolean;
  onError?: () => void;
}) {
  const safe = useMemo(
    () => appearanceSchema.shape.backgroundImage.safeParse(source || ""),
    [source],
  );
  if (!safe.success || !safe.data) return null;
  return (
    <img
      className={preview ? "appearance-preview-image" : "app-wallpaper"}
      src={safe.data}
      alt=""
      aria-hidden="true"
      draggable={false}
      onError={onError}
    />
  );
}
