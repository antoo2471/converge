// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { PluginStore } from "./PluginStore";
afterEach(cleanup);
it("browses real plugins, filters the catalogue and opens the selected configuration", () => {
  const configure = vi.fn();
  render(
    <PluginStore providers={[]} onBack={() => {}} onConfigure={configure} />,
  );
  expect(screen.getByRole("heading", { name: "ChatGPT" })).toBeTruthy();
  expect(screen.getByRole("heading", { name: "Claude Code" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /API personnalisées/ }));
  expect(screen.queryByRole("heading", { name: "ChatGPT" })).toBeNull();
  fireEvent.click(
    within(
      screen.getByRole("heading", { name: "Custom API" }).closest("article")!,
    ).getByRole("button", { name: "Configurer" }),
  );
  expect(configure).toHaveBeenCalledWith("custom");
  fireEvent.change(
    screen.getByRole("textbox", { name: "Rechercher un fournisseur" }),
    { target: { value: "introuvable" } },
  );
  expect(screen.getByText("Aucun plugin trouvé")).toBeTruthy();
  fireEvent.click(
    screen.getByRole("button", { name: "Voir tous les plugins" }),
  );
  fireEvent.click(
    within(
      screen.getByRole("heading", { name: "ChatGPT" }).closest("article")!,
    ).getByRole("button", { name: "Connecter" }),
  );
  expect(configure).toHaveBeenLastCalledWith("chatgpt");
});
