// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ModelPicker, type ModelOption } from "./ModelPicker";
afterEach(cleanup);
const options: ModelOption[] = [
  {
    value: "local::qwen",
    model: "Qwen",
    provider: { id: "local", name: "Local AI" },
  },
  {
    value: "custom::sonnet",
    model: "Sonnet",
    provider: { id: "custom", name: "Custom API" },
  },
  {
    value: "custom::opus",
    model: "Opus",
    provider: { id: "custom", name: "Custom API" },
  },
];
it("browses providers without changing the model, then commits a model choice", () => {
  const onChange = vi.fn();
  const { container } = render(
    <ModelPicker
      options={options}
      value="local::qwen"
      onChange={onChange}
      onAdd={vi.fn()}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Choisir un modèle" }));
  expect(screen.getAllByRole("option")).toHaveLength(1);
  expect(container.querySelector('[role="dialog"]')).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Custom API" }));
  expect(onChange).not.toHaveBeenCalled();
  expect(screen.getAllByRole("option")).toHaveLength(2);
  fireEvent.click(screen.getByRole("option", { name: "Opus" }));
  expect(onChange).toHaveBeenCalledWith("custom::opus");
  expect(screen.queryByRole("dialog")).toBeNull();
});
it("searches other providers and supports Enter and Escape", () => {
  const onChange = vi.fn();
  render(
    <ModelPicker options={options} value="local::qwen" onChange={onChange} />,
  );
  const trigger = screen.getByRole("button", { name: "Choisir un modèle" });
  fireEvent.click(trigger);
  fireEvent.change(screen.getByRole("combobox"), {
    target: { value: "Sonnet" },
  });
  expect(screen.getAllByRole("option")).toHaveLength(1);
  fireEvent.keyDown(screen.getByRole("combobox"), { key: "Enter" });
  expect(onChange).toHaveBeenCalledWith("custom::sonnet");
  fireEvent.click(trigger);
  fireEvent.keyDown(screen.getByRole("combobox"), { key: "Escape" });
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(document.activeElement).toBe(trigger);
});
