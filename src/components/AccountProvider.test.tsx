// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AccountProviderEditor } from "./AccountProvider";
import { AppDialogs } from "./AppDialogs";
import { providerPresets, newPresetProvider } from "../core/providerPresets";
import { providerRegistry } from "../core/plugins";
import { importedWorkspace } from "../core/workspace";
import * as api from "../lib/native";
vi.mock("../lib/native", () => ({
  cliProviderAction: vi.fn(),
  discover: vi.fn(),
  openExternal: vi.fn(),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

it.each(Object.keys(providerPresets))(
  "registers a configurable %s plugin",
  (plugin) => {
    const registered = providerRegistry.get(plugin)!;
    expect(registered).toBeDefined();
    const provider = registered.defaults("account");
    expect(registered.supports(provider.protocol!)).toBe(true);
    expect(provider).not.toHaveProperty("apiKey");
  },
);
it("does not start OAuth automatically or allow saving a missing client", async () => {
  vi.mocked(api.cliProviderAction).mockResolvedValue({
    available: false,
    message: "Client introuvable",
  });
  render(
    <AppDialogs>
      <AccountProviderEditor
        plugin="grok"
        existing={null}
        onClose={() => {}}
        onSave={vi.fn()}
      />
    </AppDialogs>,
  );
  await screen.findByText("Client introuvable");
  expect(
    (
      screen.getByRole("button", {
        name: "Se connecter avec Grok",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    (screen.getByRole("button", { name: "Enregistrer" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(api.cliProviderAction).toHaveBeenCalledTimes(1);
  expect(api.cliProviderAction).toHaveBeenCalledWith(
    expect.anything(),
    "status",
  );
});
it("runs the explicit OAuth action and reports the external client's progress", async () => {
  vi.mocked(api.cliProviderAction).mockImplementation(async (_, action) => ({
    available: true,
    login: action === "login" ? "running" : "idle",
  }));
  render(
    <AppDialogs>
      <AccountProviderEditor
        plugin="minimax"
        existing={null}
        onClose={() => {}}
        onSave={vi.fn()}
      />
    </AppDialogs>,
  );
  await screen.findByText("Client officiel disponible");
  fireEvent.click(
    screen.getByRole("button", { name: "Se connecter avec MiniMax" }),
  );
  await screen.findByText(/Terminez la connexion/);
  expect(api.cliProviderAction).toHaveBeenLastCalledWith(
    expect.objectContaining({ plugin: "minimax", protocol: "cli" }),
    "login",
  );
});
it("separates OpenCode Go and Zen, discovers real IDs and passes the key separately", async () => {
  vi.mocked(api.discover).mockResolvedValue(["available-model"]);
  const save = vi.fn().mockResolvedValue(undefined);
  render(
    <AppDialogs>
      <AccountProviderEditor
        plugin="opencode"
        existing={null}
        onClose={() => {}}
        onSave={save}
      />
    </AppDialogs>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Zen · crédits" }));
  fireEvent.change(screen.getByLabelText("Clé API"), {
    target: { value: "test-key" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Actualiser" }));
  await screen.findByText(/1 modèle reçu/);
  fireEvent.click(
    screen.getByRole("button", { name: "Enregistrer la connexion" }),
  );
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls[0][0]).toMatchObject({
    url: "https://opencode.ai/inference",
    models: ["available-model"],
  });
  expect(JSON.stringify(save.mock.calls[0][0])).not.toContain("test-key");
  expect(save.mock.calls[0][1]).toBe("test-key");
  expect(api.cliProviderAction).not.toHaveBeenCalled();
});
it("never imports an executable supplied by a shared workspace", () => {
  const provider = newPresetProvider("grok");
  provider.request = { cli: { executable: "C:\\untrusted.exe" } };
  const result = importedWorkspace.parse({
    version: 1,
    providers: [provider],
    conversations: [],
    projects: [],
    images: [],
  });
  expect(result.providers[0].request?.cli?.executable).toBeUndefined();
});
