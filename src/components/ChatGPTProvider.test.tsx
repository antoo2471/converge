// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ChatGPTProviderEditor, newChatGPTProvider } from "./ChatGPTProvider";
import { AppDialogs } from "./AppDialogs";
import * as api from "../lib/native";
import { effortLevels } from "../core/reasoning";
vi.mock("../lib/native", () => ({ chatgptAction: vi.fn() }));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("loads the Converge OAuth account and saves the discovered models and supported efforts", async () => {
  vi.mocked(api.chatgptAction).mockResolvedValue({
    account: { type: "chatgpt", email: "user@example.com", plan: "plus" },
    models: [
      {
        id: "available",
        name: "Available",
        efforts: ["low", "high"],
        vision: true,
      },
    ],
    capabilities: { vision: true, tools: true, images: false, context: 32768 },
  });
  const save = vi.fn().mockResolvedValue(undefined);
  render(
    <AppDialogs>
      <ChatGPTProviderEditor existing={null} onClose={() => {}} onSave={save} />
    </AppDialogs>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Actualiser" }));
  await screen.findByText("user@example.com");
  fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
  await waitFor(() => expect(save).toHaveBeenCalled());
  const provider = save.mock.calls[0][0];
  expect(provider.models).toEqual(["codex/available"]);
  expect(provider.request.chatgpt).not.toHaveProperty("authSource");
  expect(provider.capabilities.images).toBe(false);
  expect(effortLevels(provider, "codex/available")).toEqual([
    "auto",
    "low",
    "high",
  ]);
  expect(api.chatgptAction).not.toHaveBeenCalledWith(
    expect.anything(),
    "login",
  );
});
it("exposes only Converge OAuth and no retired transports", () => {
  render(
    <AppDialogs>
      <ChatGPTProviderEditor
        existing={null}
        onClose={() => {}}
        onSave={vi.fn()}
      />
    </AppDialogs>,
  );
  expect(
    screen.getByRole("button", { name: "Se connecter avec ChatGPT" }),
  ).toBeTruthy();
  expect(screen.queryByText(/Session Codex existante/)).toBeNull();
  expect(screen.queryByText(/grand public/)).toBeNull();
  expect(screen.queryByRole("tablist")).toBeNull();
});
it("keeps a disconnected plugin empty rather than inventing models", () => {
  const provider = newChatGPTProvider();
  expect(provider.models).toEqual([]);
  expect(provider.capabilities.images).toBe(false);
});
