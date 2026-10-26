// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ExtensionsPage } from "./ExtensionsPage";
import { AppDialogs } from "./AppDialogs";
import * as api from "../lib/extensions";
import { open } from "@tauri-apps/plugin-dialog";
vi.mock("../lib/native", () => ({ native: true, openExternal: vi.fn() }));
vi.mock("../lib/extensions", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/extensions")>();
  return {
    ...original,
    snapshot: vi.fn(),
    catalogue: vi.fn(),
    mcpAction: vi.fn(),
    skillAction: vi.fn(),
  };
});
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
beforeEach(() => {
  vi.mocked(api.snapshot).mockResolvedValue({ servers: [], skills: [] });
  vi.mocked(api.catalogue).mockResolvedValue({ servers: [] });
  vi.mocked(api.mcpAction).mockResolvedValue(null);
  vi.mocked(api.skillAction).mockResolvedValue(null);
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
const mount = () =>
  render(
    <AppDialogs>
      <ExtensionsPage />
    </AppDialogs>,
  );
it("guides Google setup before starting OAuth, validates credentials and preserves them when going back", async () => {
  vi.mocked(api.mcpAction).mockImplementation(async (action) =>
    action === "authorize" ? { loginId: "google-flow" } : null,
  );
  mount();
  fireEvent.click(screen.getByRole("button", { name: "Ajouter Gmail" }));
  const dialog = screen.getByRole("dialog", { name: "Connecter Gmail" });
  expect(
    within(dialog).getByRole("heading", {
      name: "Préparez votre projet Google",
    }),
  ).toBeTruthy();
  expect(within(dialog).queryByText(/Developer Preview/i)).toBeNull();
  expect(api.mcpAction).not.toHaveBeenCalled();
  fireEvent.click(
    within(dialog).getByRole("button", {
      name: "Configurer mon client Google",
    }),
  );
  expect(
    (within(dialog).getByLabelText("URI de redirection") as HTMLInputElement)
      .value,
  ).toBe("http://127.0.0.1:32145/oauth/callback");
  fireEvent.click(within(dialog).getByRole("button", { name: "Continuer" }));
  expect(within(dialog).getByRole("alert").textContent).toContain(
    "client ID Google complet",
  );
  expect(
    within(dialog).getByRole("alert").closest(".app-connect-actions"),
  ).toBeTruthy();
  expect(api.mcpAction).not.toHaveBeenCalled();
  fireEvent.change(within(dialog).getByLabelText("Client ID Google"), {
    target: { value: "my-app.apps.googleusercontent.com" },
  });
  fireEvent.change(within(dialog).getByLabelText("Secret client Google"), {
    target: { value: "my-google-secret" },
  });
  fireEvent.click(within(dialog).getByRole("button", { name: "Continuer" }));
  expect(
    within(dialog).getByRole("heading", {
      name: "Tout est prêt pour vous connecter",
    }),
  ).toBeTruthy();
  fireEvent.click(within(dialog).getByRole("button", { name: "Retour" }));
  expect(
    (within(dialog).getByLabelText("Client ID Google") as HTMLInputElement)
      .value,
  ).toBe("my-app.apps.googleusercontent.com");
  fireEvent.click(within(dialog).getByRole("button", { name: "Continuer" }));
  fireEvent.click(
    within(dialog).getByRole("button", { name: "Se connecter avec Google" }),
  );
  await waitFor(() =>
    expect(api.mcpAction).toHaveBeenCalledWith(
      "openAuth",
      expect.objectContaining({ loginId: "google-flow" }),
    ),
  );
  expect(api.mcpAction).toHaveBeenCalledWith(
    "save",
    expect.objectContaining({
      config: expect.objectContaining({
        url: "https://gmailmcp.googleapis.com/mcp/v1",
        clientId: "my-app.apps.googleusercontent.com",
        redirectPort: 32145,
      }),
      secret: { client_secret: "my-google-secret" },
    }),
  );
  expect(
    within(dialog).getByRole("heading", {
      name: "Continuez dans votre navigateur",
    }),
  ).toBeTruthy();
  fireEvent.click(within(dialog).getByRole("button", { name: "Annuler" }));
  expect(api.mcpAction).toHaveBeenCalledWith(
    "cancel",
    expect.objectContaining({ loginId: "google-flow" }),
  );
});
it("discovers the requested apps and opens an OAuth consent dialog without API-key fields", async () => {
  mount();
  expect(
    screen.queryByRole("heading", { name: "Google Workspace" }),
  ).toBeNull();
  for (const name of [
    "Stripe",
    "Gmail",
    "Google Drive",
    "Google Docs",
    "Google Sheets",
    "Google Slides",
    "Google Calendar",
    "Figma",
    "Canva",
    "Microsoft Teams",
    "Slack",
    "Booking.com",
    "Tripadvisor",
    "Spotify",
    "LinkedIn",
    "Shopify",
    "Wix",
    "Skyscanner",
    "Binance",
    "Base44",
    "Vercel",
    "Exa",
    "Lovable",
    "Remote Desktop Commander",
    "Runway",
    "Higgsfield",
    "Adobe",
    "Zoom",
    "Hostinger Mail",
    "Todoist",
    "Outlook",
    "Outlook Calendar",
    "Notion",
    "Dropbox",
    "Sonos",
  ])
    expect(screen.getByRole("heading", { name })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Ajouter GitHub" }));
  const dialog = screen.getByRole("dialog", { name: "Connecter GitHub" });
  expect(within(dialog).queryByLabelText("Clé API")).toBeNull();
  expect(within(dialog).queryByLabelText("Token d’accès")).toBeNull();
  expect(api.mcpAction).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button", { name: "Annuler" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(api.mcpAction).not.toHaveBeenCalled();
});
it("saves an OAuth connection only after explicit consent and opens only its bound authorization URL", async () => {
  vi.mocked(api.mcpAction).mockImplementation(async (action) =>
    action === "authorize"
      ? { loginId: "flow", authUrl: "https://oauth.example.test/authorize" }
      : null,
  );
  mount();
  fireEvent.click(screen.getByRole("button", { name: "Ajouter Notion" }));
  const dialog = screen.getByRole("dialog", { name: "Connecter Notion" });
  fireEvent.click(
    within(dialog).getByRole("button", { name: "Connecter Notion" }),
  );
  await within(dialog).findByText("Autorisez Converge dans votre navigateur.");
  expect(api.mcpAction).toHaveBeenCalledWith(
    "save",
    expect.objectContaining({
      config: expect.objectContaining({
        name: "Notion",
        auth: "oauth",
        url: "https://mcp.notion.com/mcp",
      }),
    }),
  );
  expect(api.mcpAction).toHaveBeenCalledWith(
    "openAuth",
    expect.objectContaining({ loginId: "flow" }),
  );
  fireEvent.click(within(dialog).getByRole("button", { name: "Annuler" }));
  expect(api.mcpAction).toHaveBeenCalledWith(
    "cancel",
    expect.objectContaining({ loginId: "flow" }),
  );
});
it("keeps the declared connector publisher visible before connecting a community service", async () => {
  vi.mocked(api.catalogue).mockResolvedValue({
    servers: [
      {
        server: {
          name: "org.example/spotify",
          title: "Spotify connector",
          description: "A connector",
          version: "1",
          remotes: [
            { type: "streamable-http", url: "https://example.org/mcp" },
          ],
        },
      },
    ],
  });
  mount();
  fireEvent.click(screen.getByRole("button", { name: "Ajouter Spotify" }));
  const dialog = screen.getByRole("dialog", { name: "Connecter Spotify" });
  await within(dialog).findByText("Éditeur déclaré : org.example/spotify");
  expect(
    (
      within(dialog).getByRole("button", {
        name: "Connecter Spotify",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  fireEvent.click(within(dialog).getByRole("radio"));
  expect(
    (
      within(dialog).getByRole("button", {
        name: "Connecter Spotify",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(false);
  expect(api.mcpAction).not.toHaveBeenCalled();
});
it("imports a skill from a chosen file without requiring an AI provider", async () => {
  vi.mocked(open).mockResolvedValue("C:/skills/example.zip");
  mount();
  await waitFor(() => expect(api.snapshot).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("tab", { name: "Skills" }));
  fireEvent.click(screen.getByRole("button", { name: "Importer un skill" }));
  await waitFor(() =>
    expect(api.skillAction).toHaveBeenCalledWith("import", {
      source: "C:/skills/example.zip",
    }),
  );
  expect(api.mcpAction).not.toHaveBeenCalled();
});
it("retains API-key and stdio configuration in the custom MCP flow", async () => {
  mount();
  fireEvent.click(screen.getByRole("button", { name: "Ajouter" }));
  fireEvent.click(
    screen.getByRole("menuitem", { name: "Serveur MCP personnalisé" }),
  );
  fireEvent.change(screen.getByLabelText("Nom de la connexion"), {
    target: { value: "My server" },
  });
  fireEvent.change(screen.getByLabelText("URL du serveur MCP"), {
    target: { value: "https://example.org/mcp" },
  });
  fireEvent.change(screen.getByLabelText("Connexion"), {
    target: { value: "token" },
  });
  fireEvent.change(screen.getByLabelText("Token d’accès"), {
    target: { value: "test-secret" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
  await waitFor(() =>
    expect(api.mcpAction).toHaveBeenCalledWith(
      "save",
      expect.objectContaining({ secret: { token: "test-secret" } }),
    ),
  );
  const saved = vi
    .mocked(api.mcpAction)
    .mock.calls.find(([action]) => action === "save")![1]!;
  expect(JSON.stringify(saved.config)).not.toContain("test-secret");
});

it("guides GitHub OAuth setup without asking for a PAT", async () => {
  vi.mocked(api.mcpAction).mockImplementation(async (action) =>
    action === "authorize" ? { loginId: "github-flow" } : null,
  );
  mount();
  fireEvent.click(screen.getByRole("button", { name: "Ajouter GitHub" }));
  const dialog = screen.getByRole("dialog", { name: "Connecter GitHub" });
  expect(
    within(dialog).getByRole("heading", {
      name: "Créez votre connexion GitHub",
    }),
  ).toBeTruthy();
  fireEvent.click(
    within(dialog).getByRole("button", {
      name: "Configurer mon client GitHub",
    }),
  );
  fireEvent.change(within(dialog).getByLabelText("Client ID GitHub"), {
    target: { value: "my-github-client" },
  });
  fireEvent.change(within(dialog).getByLabelText("Secret client GitHub"), {
    target: { value: "test-secret" },
  });
  fireEvent.click(within(dialog).getByRole("button", { name: "Continuer" }));
  expect(api.mcpAction).not.toHaveBeenCalled();
  fireEvent.click(
    within(dialog).getByRole("button", { name: "Se connecter avec GitHub" }),
  );
  await waitFor(() =>
    expect(api.mcpAction).toHaveBeenCalledWith(
      "save",
      expect.objectContaining({
        config: expect.objectContaining({
          clientId: "my-github-client",
          redirectPort: 32145,
          auth: "oauth",
        }),
        secret: { client_secret: "test-secret" },
      }),
    ),
  );
});
