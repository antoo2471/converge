import { it, expect } from "vitest";
import { importedWorkspace } from "./workspace";
it("rejects malformed attachments and strips unknown imported privileges", () => {
  const base = {
    version: 1,
    projects: [],
    images: [],
    conversations: [
      {
        id: "c",
        title: "Conversation",
        projectId: null,
        createdAt: 1,
        updatedAt: 1,
        messages: [
          { id: "m", role: "user", content: "hello", attachments: "invalid" },
        ],
      },
    ],
  };
  expect(() => importedWorkspace.parse(base)).toThrow();
  base.conversations[0].messages = [];
  const parsed = importedWorkspace.parse({
    ...base,
    settings: { enabledTools: ["terminal.execute"] },
    apiKey: "secret",
  });
  expect(parsed).not.toHaveProperty("apiKey");
  expect(parsed).not.toHaveProperty("settings");
});
it("imports ChatGPT modes without granting shared authentication or executing an imported binary", () => {
  const parsed = importedWorkspace.parse({
    version: 1,
    projects: [],
    images: [],
    conversations: [
      {
        id: "c",
        title: "Chat",
        projectId: null,
        createdAt: 1,
        updatedAt: 1,
        messages: [],
        selection: { providerId: "p", model: "codex/model", mode: "chat" },
      },
    ],
    providers: [
      {
        id: crypto.randomUUID(),
        name: "ChatGPT",
        plugin: "chatgpt",
        protocol: "codex",
        url: "https://chatgpt.com",
        headers: {},
        models: [],
        enabled: true,
        capabilities: {
          vision: false,
          tools: true,
          images: false,
          context: 32768,
        },
        request: {
          chatgpt: {
            mode: "codex",
            authSource: "codex",
            executable: "C:\\untrusted.exe",
          },
        },
      },
    ],
  });
  expect(parsed.providers[0].request?.chatgpt?.executable).toBeUndefined();
  expect(parsed.providers[0].request?.chatgpt).not.toHaveProperty("authSource");
  expect(parsed.conversations[0].selection?.mode).toBe("codex");
});
