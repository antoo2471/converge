import { ModelPicker, type ModelOption } from "./ModelPicker";

export function MessageModelMenu({
  options,
  disabled,
  onSelect,
}: {
  options: ModelOption[];
  disabled: boolean;
  onSelect: (value: string) => void;
}) {
  return (
    <ModelPicker
      options={options}
      value=""
      onChange={onSelect}
      disabled={disabled || !options.length}
      label="Répondre avec un autre modèle"
      triggerText="Changer de modèle"
      compact
    />
  );
}
