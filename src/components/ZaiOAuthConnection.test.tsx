// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ZaiOAuthConnection } from "./ZaiOAuthConnection";
import { AccountProviderEditor } from "./AccountProvider";
import { AppDialogs } from "./AppDialogs";
import { newPresetProvider } from "../core/providerPresets";
import { importedWorkspace } from "../core/workspace";
import * as api from "../lib/native";
vi.mock("../lib/native", () => ({
  zaiAuthAction: vi.fn(),
  deepseekAuthAction: vi.fn(),
  openExternal: vi.fn(),
  discover: vi.fn(),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("uses DeepSeek's native account flow and cancels its own pending login", async () => {
  vi.mocked(api.deepseekAuthAction).mockImplementation(async (_, action) => action === "start" ? { state: "pending", loginId: "deepseek-flow", authUrl: "https://platform.deepseek.com/dsh/authorize" } : { state: "disconnected" });
  const { unmount } = render(<ZaiOAuthConnection provider={newPresetProvider("deepseek")} onConnectionChange={() => {}} />);
  await waitFor(() => expect(api.deepseekAuthAction).toHaveBeenCalledWith(expect.anything(), "status"));
  expect(api.openExternal).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Se connecter avec DeepSeek" }));
  await screen.findByText(/En attente de votre autorisation/);
  expect(api.openExternal).toHaveBeenCalledWith("https://platform.deepseek.com/dsh/authorize");
  expect(api.zaiAuthAction).not.toHaveBeenCalled();
  unmount();
  expect(api.deepseekAuthAction).toHaveBeenLastCalledWith(expect.anything(), "cancel", "deepseek-flow");
});
it("starts OAuth only on click, opens the browser and observes completion without a key in the GUI", async () => {
  vi.mocked(api.zaiAuthAction).mockImplementation(async (_, action) =>
    action === "start"
      ? {
          state: "pending",
          loginId: "flow",
          authUrl: "https://chat.z.ai/authorize",
        }
      : action === "poll"
        ? { state: "connected" }
        : { state: "disconnected" },
  );
  vi.mocked(api.openExternal).mockResolvedValue(undefined);
  const notify = vi.fn();
  render(
    <ZaiOAuthConnection
      provider={newPresetProvider("zai")}
      onConnectionChange={notify}
    />,
  );
  await waitFor(() => expect(api.zaiAuthAction).toHaveBeenCalledTimes(1));
  expect(api.zaiAuthAction).toHaveBeenLastCalledWith(
    expect.anything(),
    "status",
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Se connecter avec Z.ai" }),
  );
  await screen.findByText(/En attente de votre autorisation/);
  expect(api.openExternal).toHaveBeenCalledWith("https://chat.z.ai/authorize");
  await screen.findByText("Coding Plan connecté", {}, { timeout: 4000 });
  expect(notify).toHaveBeenLastCalledWith(true);
  expect(screen.queryByLabelText("Clé API")).toBeNull();
});
it("cancels an unfinished authorization when the configuration is closed", async () => {
  vi.mocked(api.zaiAuthAction).mockImplementation(async (_, action) =>
    action === "start"
      ? {
          state: "pending",
          loginId: "flow",
          authUrl: "https://chat.z.ai/authorize",
        }
      : { state: "disconnected" },
  );
  const { unmount } = render(
    <ZaiOAuthConnection
      provider={newPresetProvider("zai")}
      onConnectionChange={() => {}}
    />,
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Se connecter avec Z.ai" }),
  );
  await screen.findByText(/En attente de votre autorisation/);
  unmount();
  expect(api.zaiAuthAction).toHaveBeenLastCalledWith(
    expect.anything(),
    "cancel",
    "flow",
  );
});
it.each(["zai", "deepseek"] as const)("saves an OAuth %s provider without accepting or exporting its secret", async (plugin) => {
  vi.mocked(plugin === "deepseek" ? api.deepseekAuthAction : api.zaiAuthAction).mockResolvedValue({ state: "connected" });
  const save = vi.fn().mockResolvedValue(undefined);
  render(
    <AppDialogs>
      <AccountProviderEditor
        plugin={plugin}
        existing={null}
        onClose={() => {}}
        onSave={save}
      />
    </AppDialogs>,
  );
  await screen.findByText(plugin === "deepseek" ? "Compte DeepSeek connecté" : "Coding Plan connecté");
  fireEvent.change(
    screen.getByRole("textbox", { name: "Un identifiant par ligne" }),
    { target: { value: "glm-test" } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
  await waitFor(() => expect(save).toHaveBeenCalled());
  expect(save.mock.calls[0][1]).toBeUndefined();
  const exported = importedWorkspace.parse({
    version: 1,
    projects: [],
    images: [],
    conversations: [],
    providers: [save.mock.calls[0][0]],
  });
  expect(exported.providers[0].request?.[plugin]?.authMode).toBe("oauth");
});
