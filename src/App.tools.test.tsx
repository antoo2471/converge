vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
} from "@testing-library/react";
import App from "./App";
import { AppDialogs } from "./components/AppDialogs";
import { defaults } from "./types";
import * as api from "./lib/native";
vi.mock("./lib/native", () => ({
  native: true,
  workspaceContext: vi
    .fn()
    .mockResolvedValue({ folders: [], desktop: "C:\\Desktop" }),
  loadWorkspace: vi.fn(),
  saveWorkspace: vi.fn().mockResolvedValue(undefined),
  streamChat: vi.fn(),
  agentTool: vi.fn(),
  resolveToolPermission: vi.fn(),
  chooseWorkspace: vi.fn(),
  removeWorkspace: vi.fn(),
  cancel: vi.fn(),
  revokeConversationPermissions: vi.fn(),
  resolveCodexTool: vi.fn().mockResolvedValue(undefined),
  chatgptAction: vi.fn(),
}));
let finishTool: (result: string) => void, rejectTool: (error: string) => void;
let performed = vi.fn<() => void>();
beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("matchMedia", () => ({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
  vi.stubGlobal("requestAnimationFrame", (callback: () => void) =>
    setTimeout(callback, 16),
  );
  vi.stubGlobal("cancelAnimationFrame", clearTimeout);
  HTMLElement.prototype.scrollIntoView = vi.fn();
  vi.mocked(api.loadWorkspace).mockResolvedValue({
    ...defaults,
    providers: [
      {
        id: "provider",
        name: "Local",
        plugin: "custom",
        protocol: "openai",
        url: "http://localhost:1234",
        enabled: true,
        headers: {},
        models: ["model"],
        capabilities: {
          vision: false,
          tools: true,
          images: false,
          context: 32000,
        },
      },
    ],
  });
  let round = 0;
  vi.mocked(api.streamChat).mockImplementation(
    async (_rid, _provider, _model, messages, _settings, event) => {
      const title = messages[0]?.content.startsWith(
        "Generate a short conversation title",
      );
      event({
        type: "delta",
        text: title
          ? "Création du fichier"
          : round++ === 0
            ? 'Je prépare le fichier.```converge-tool\n{"name":"filesystem.write","args":{"path":"hello.txt","content":"hello"}}\n```'
            : "Le fichier a été créé.",
      });
      event({ type: "done" });
    },
  );
  performed = vi.fn<() => void>();
  vi.mocked(api.agentTool).mockImplementation(
    (cid, _rid, name, args, permission, output) =>
      new Promise((resolve, reject) => {
        finishTool = (result) => {
          performed();
          output({
            type: "file_changes",
            added: 2,
            removed: 1,
            diff: "-old\n+one\n+two\n",
          });
          output({ type: "file_progress", written: 8, total: 8 });
          resolve(result);
        };
        rejectTool = reject;
        permission({
          id: "permission",
          conversationId: cid,
          name,
          args,
          root: "C:\\Desktop",
          requiresRoot: true,
        });
      }),
  );
  vi.mocked(api.resolveToolPermission).mockImplementation(
    async (_id, decision) => {
      if (decision === "deny") rejectTool("REFUSED: Action refusée");
      else finishTool("Fichier enregistré");
    },
  );
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
async function start() {
  render(
    <AppDialogs>
      <App />
    </AppDialogs>,
  );
  const composer = await screen.findByRole("textbox", {
    name: "Votre message",
  });
  fireEvent.change(composer, {
    target: { value: "Crée hello.txt avec hello" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
  await screen.findByRole("button", { name: "Autoriser" });
  return composer;
}
it("routes a Codex dynamic tool through GUI permission and returns its actual result without another model round", async () => {
  vi.mocked(api.loadWorkspace).mockResolvedValue({
    ...defaults,
    providers: [
      {
        id: "chatgpt",
        name: "ChatGPT",
        plugin: "chatgpt",
        protocol: "codex",
        url: "https://chatgpt.com",
        enabled: true,
        headers: {},
        models: ["codex/model"],
        capabilities: {
          vision: false,
          tools: true,
          images: false,
          context: 32768,
        },
        request: {
          chatgpt: {
            models: [
              {
                id: "model",
                name: "Model",
                efforts: ["medium"],
                vision: false,
              },
            ],
          },
        },
      },
    ],
  });
  let reply: () => void = () => {};
  vi.mocked(api.resolveCodexTool).mockImplementation(async () => {
    reply();
  });
  vi.mocked(api.streamChat).mockImplementation(
    async (_rid, _provider, _model, messages, _settings, event) => {
      if (
        messages[0]?.content.startsWith("Generate a short conversation title")
      ) {
        event({ type: "delta", text: "Création hello" });
        event({ type: "done" });
        return;
      }
      event({ type: "reasoning", text: "Je prépare le fichier demandé." });
      event({
        type: "codex_tool",
        callId: "codex-call",
        name: "filesystem_write",
        args: { path: "hello.txt", content: "hello" },
      });
      await new Promise<void>((resolve) => {
        reply = resolve;
      });
      event({
        type: "delta",
        text: "Le fichier a été créé par l’outil Converge.",
      });
      event({ type: "done" });
    },
  );
  await start();
  expect(api.resolveCodexTool).not.toHaveBeenCalled();
  expect(performed).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Autoriser" }));
  await screen.findByText("Le fichier a été créé par l’outil Converge.");
  expect(api.resolveCodexTool).toHaveBeenCalledWith(
    "codex-call",
    "Fichier enregistré",
    true,
  );
  expect(performed).toHaveBeenCalledTimes(1);
  expect(
    vi
      .mocked(api.streamChat)
      .mock.calls.filter(
        (c) =>
          !c[3][0]?.content.startsWith("Generate a short conversation title"),
      ),
  ).toHaveLength(1);
  await waitFor(() =>
    expect(
      vi.mocked(api.saveWorkspace).mock.calls.at(-1)?.[0]?.conversations[0]
        .selection,
    ).toMatchObject({
      providerId: "chatgpt",
      mode: "codex",
      model: "codex/model",
    }),
  );
});
it("displays and persists native generated images received from Codex", async () => {
  const data = "data:image/png;base64,iVBORw0KGgo=";
  vi.mocked(api.streamChat).mockImplementation(
    async (_rid, _pid, _model, _history, _settings, event) => {
      event({ type: "image", data, prompt: "BMW M3" });
      event({ type: "done" });
    },
  );
  render(
    <AppDialogs>
      <App />
    </AppDialogs>,
  );
  const composer = await screen.findByRole("textbox", {
    name: "Votre message",
  });
  fireEvent.change(composer, { target: { value: "Génère une BMW M3" } });
  fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
  const image = await screen.findByRole("img", { name: "BMW M3" });
  expect(image.getAttribute("src")).toBe(data);
  expect(image.classList.contains("generated-image")).toBe(true);
  await waitFor(() =>
    expect(
      vi.mocked(api.saveWorkspace).mock.calls.at(-1)?.[0].images[0]?.data,
    ).toBe(data),
  );
});
it("waits for GUI permission, executes once and automatically feeds the result back to the model", async () => {
  const composer = await start();
  expect(performed).not.toHaveBeenCalled();
  expect(screen.queryByText(/converge-tool/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Autoriser" }));
  await screen.findByText("Le fichier a été créé.");
  expect(performed).toHaveBeenCalledTimes(1);
  expect(screen.getByText("+2")).toBeTruthy();
  expect(screen.getByText("−1")).toBeTruthy();
  const continuations = vi
    .mocked(api.streamChat)
    .mock.calls.filter((call) =>
      call[3].some((message) =>
        message.content.includes("Converge tool execution results"),
      ),
    );
  expect(continuations).toHaveLength(1);
  expect(
    continuations[0][3].some((message) =>
      message.content.includes("Fichier enregistré"),
    ),
  ).toBe(true);
  expect((composer as HTMLTextAreaElement).value).toBe("");
  expect(screen.getAllByText("Local · model")).toHaveLength(1);
  expect(
    screen.getAllByRole("button", { name: "Copier le message" }),
  ).toHaveLength(2);
});
it("never executes or asks the model for further actions after refusal", async () => {
  await start();
  fireEvent.click(screen.getByRole("button", { name: "Refuser" }));
  await waitFor(() => expect(screen.getByText("Refusé")).toBeTruthy());
  expect(performed).not.toHaveBeenCalled();
  expect(screen.queryByText("Le fichier a été créé.")).toBeNull();
});
it("shows live prepared lines, then keeps the real additions and removals after execution", async () => {
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  let round = 0;
  vi.mocked(api.streamChat).mockImplementation(
    async (_rid, _provider, _model, messages, _settings, event) => {
      if (
        messages[0]?.content.startsWith("Generate a short conversation title")
      ) {
        event({ type: "delta", text: "Titre" });
        event({ type: "done" });
        return;
      }
      if (round++ === 0) {
        event({
          type: "tool_progress",
          index: 0,
          name: "filesystem_write",
          arguments: '{"path":"site/index.html","content":"one\\ntwo\\n',
        });
        await pending;
        event({
          type: "tool",
          name: "filesystem_write",
          args: { path: "site/index.html", content: "one\ntwo\n" },
        });
      } else event({ type: "delta", text: "Le fichier a été créé." });
      event({ type: "done" });
    },
  );
  const started = start();
  await screen.findByText("Rédaction…");
  expect(screen.getByText("+2")).toBeTruthy();
  expect(screen.getByText("lignes préparées")).toBeTruthy();
  expect(api.agentTool).not.toHaveBeenCalled();
  release();
  await started;
  fireEvent.click(screen.getByRole("button", { name: "Autoriser" }));
  await screen.findByText("Le fichier a été créé.");
  expect(screen.getByText("+2")).toBeTruthy();
  expect(screen.getByText("−1")).toBeTruthy();
  expect(screen.queryByText("lignes préparées")).toBeNull();
});
it("executes a large native tool payload directly, including embedded Markdown fences", async () => {
  const content = "text with ``` delimiters\n".repeat(10000);
  let round = 0;
  vi.mocked(api.streamChat).mockImplementation(
    async (_rid, _provider, _model, messages, _settings, event) => {
      if (
        messages[0]?.content.startsWith("Generate a short conversation title")
      ) {
        event({ type: "done" });
        return;
      }
      if (round++ === 0) {
        event({
          type: "tool_progress",
          index: 0,
          name: "filesystem_write",
          arguments: JSON.stringify({
            path: "long.txt",
            content: content.slice(0, 20000),
          }),
        });
        event({
          type: "tool",
          name: "filesystem_write",
          args: { path: "long.txt", content },
        });
      } else event({ type: "delta", text: "Le fichier a été créé." });
      event({ type: "done" });
    },
  );
  await start();
  fireEvent.click(screen.getByRole("button", { name: "Autoriser" }));
  await screen.findByText("Le fichier a été créé.");
  expect(vi.mocked(api.agentTool).mock.calls[0][3].content).toBe(content);
  expect(performed).toHaveBeenCalledTimes(1);
  expect(screen.queryByText(/demande n’a pas été finalisée/)).toBeNull();
});

it("feeds a new computer screenshot and its coordinate metadata back as vision input", async () => {
  const initial = await vi.mocked(api.loadWorkspace).getMockImplementation()!();
  if (!initial) throw Error("Test workspace missing");
  initial.providers[0].capabilities.vision = true;
  vi.mocked(api.loadWorkspace).mockResolvedValue(initial);
  const image = "aGVsbG8=";
  vi.mocked(api.agentTool).mockResolvedValue(
    JSON.stringify({
      kind: "computer_observation",
      image: `data:image/jpeg;base64,${image}`,
      frameId: "fresh-frame",
      width: 720,
      height: 300,
    }),
  );
  let round = 0;
  vi.mocked(api.streamChat).mockImplementation(
    async (_rid, _provider, _model, messages, _settings, event) => {
      if (
        messages[0]?.content.startsWith("Generate a short conversation title")
      ) {
        event({ type: "delta", text: "Test" });
        event({ type: "done" });
        return;
      }
      if (round++ === 0)
        event({ type: "tool", name: "computer_focus", args: { windowId: 42 } });
      else {
        const result = messages.find((m) =>
          m.content.includes("Converge tool execution results"),
        );
        expect(result?.attachments?.[0]).toMatchObject({
          mime: "image/jpeg",
          data: image,
        });
        expect(result?.content).toContain("fresh-frame");
        expect(result?.content).not.toContain(image);
        event({ type: "delta", text: "La fenêtre est visible." });
      }
      event({ type: "done" });
    },
  );
  render(
    <AppDialogs>
      <App />
    </AppDialogs>,
  );
  const composer = await screen.findByRole("textbox", {
    name: "Votre message",
  });
  fireEvent.change(composer, { target: { value: "Observe la fenêtre" } });
  fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
  await screen.findByText("La fenêtre est visible.");
  expect(round).toBe(2);
});

it("halts repeated computer requests before sending another desktop action", async () => {
  vi.mocked(api.agentTool).mockResolvedValue("[]");
  vi.mocked(api.streamChat).mockImplementation(
    async (_r, _p, _m, _messages, _s, event) => {
      event({ type: "tool", name: "computer_windows", args: {} });
      event({ type: "done" });
    },
  );
  render(
    <AppDialogs>
      <App />
    </AppDialogs>,
  );
  const composer = await screen.findByRole("textbox", {
    name: "Votre message",
  });
  fireEvent.change(composer, { target: { value: "Trouve la fenêtre" } });
  fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
  await screen.findByText(/Contrôle arrêté : l’IA répétait la même action/);
  expect(api.agentTool).toHaveBeenCalledTimes(3);
  expect(api.streamChat).toHaveBeenCalledTimes(4);
  await screen.findByRole("button", { name: "Envoyer" });
});
