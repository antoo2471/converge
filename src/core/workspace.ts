import { z } from "zod";
const attachment = z.object({
  id: z.string().max(100),
  name: z.string().max(500),
  mime: z.string().max(100),
  data: z.string().max(15 * 1024 * 1024),
});
const message = z.object({
  reasoning: z.string().max(200000).optional(),
  execution: z.string().max(500).optional(),
  id: z.string().max(100),
  turnId: z.string().max(100).optional(),
  role: z.enum(["user", "assistant", "system"]),
  content: z.string().max(32 * 1024 * 1024),
  model: z.string().max(500).optional(),
  providerId: z.string().optional(),
  modelId: z.string().optional(),
  attachments: z.array(attachment).max(8).optional(),
  error: z.string().optional(),
  interrupted: z.boolean().optional(),
  excluded: z.boolean().optional(),
  toolCalls: z
    .array(
      z.object({
        id: z.string().uuid(),
        name: z.string().max(80),
        args: z.record(z.string(), z.unknown()),
        status: z.enum([
          "preparing",
          "awaiting",
          "running",
          "succeeded",
          "failed",
          "denied",
          "cancelled",
        ]),
        result: z
          .string()
          .max(16 * 1024 * 1024)
          .optional(),
        output: z.string().max(200000).optional(),
        changes: z
          .object({
            added: z.number().nonnegative(),
            removed: z.number().nonnegative(),
            exact: z.boolean(),
            diff: z.string().max(100000).optional(),
          })
          .optional(),
        writeProgress: z
          .object({
            written: z.number().nonnegative(),
            total: z.number().nonnegative(),
          })
          .optional(),
      }),
    )
    .max(20)
    .optional(),
});
const conversation = z.object({
  selection: z
    .object({
      providerId: z.string(),
      model: z.string(),
      mode: z
        .enum(["codex", "chat"])
        .transform(() => "codex" as const)
        .optional(),
    })
    .optional(),
  id: z.string().max(100),
  title: z.string().max(500),
  titleSource: z.enum(["pending", "generated", "manual"]).optional(),
  projectId: z.string().nullable(),
  messages: z.array(message).max(50000),
  createdAt: z.number().finite(),
  updatedAt: z.number().finite(),
  parentId: z.string().optional(),
});
export const importedWorkspace = z.object({
  version: z.literal(1),
  providers: z
    .array(
      z.object({
        id: z.string().uuid(),
        name: z.string().min(1).max(120),
        plugin: z.enum([
          "local",
          "custom",
          "chatgpt",
          "antigravity",
          "zai",
          "claude-code",
          "opencode",
          "deepseek",
          "grok",
          "minimax",
          "kimi",
        ]),
        protocol: z.enum([
          "ollama",
          "openai",
          "anthropic",
          "comfyui",
          "codex",
          "cli",
          "responses",
        ]),
        url: z.string().url(),
        headers: z.object({}).default({}),
        request: z
          .object({
            zai: z
              .object({ authMode: z.enum(["oauth", "api-key"]) })
              .optional(),
            deepseek: z
              .object({ authMode: z.enum(["oauth", "api-key"]) })
              .optional(),
            cli: z
              .object({
                executable: z
                  .string()
                  .optional()
                  .transform(() => undefined),
              })
              .optional(),
            chatPath: z.string().max(500).optional(),
            modelsPath: z.string().max(500).optional(),
            body: z.record(z.string(), z.unknown()).optional(),
            chatgpt: z
              .object({
                executable: z
                  .string()
                  .max(2000)
                  .optional()
                  .transform(() => undefined),
                models: z
                  .array(
                    z.object({
                      id: z.string(),
                      name: z.string(),
                      efforts: z.array(z.string()),
                      vision: z.boolean(),
                      defaultEffort: z.string().nullable().optional(),
                    }),
                  )
                  .max(1000)
                  .optional(),
              })
              .optional(),
          })
          .optional(),
        models: z.array(z.string().max(500)).max(1000),
        capabilities: z.object({
          vision: z.boolean(),
          tools: z.boolean(),
          images: z.boolean(),
          context: z.number().int().min(512).max(2000000),
          workflow: z
            .string()
            .max(1024 * 1024)
            .optional(),
        }),
        enabled: z.boolean(),
      }),
    )
    .max(500)
    .default([]),
  conversations: z.array(conversation).max(50000),
  projects: z
    .array(z.object({ id: z.string().max(100), name: z.string().max(500) }))
    .max(10000),
  images: z
    .array(
      z.object({
        id: z.string(),
        prompt: z.string(),
        model: z.string(),
        data: z
          .string()
          .startsWith("data:image/")
          .max(40 * 1024 * 1024),
        createdAt: z.number().finite(),
      }),
    )
    .max(5000),
});
