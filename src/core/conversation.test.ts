import { describe, expect, it } from "vitest";
import { historyForRetry } from "./conversation";
import type { Message } from "../types";

describe("retry context", () => {
  const messages: Message[] = [
    {
      id: "q1",
      role: "user",
      content: "Describe this",
      attachments: [
        { id: "image", name: "photo", mime: "image/png", data: "abc" },
      ],
    },
    { id: "a1", role: "assistant", content: "First model" },
    { id: "a2", role: "assistant", content: "Second model" },
    { id: "q2", role: "user", content: "Continue" },
    { id: "a3", role: "assistant", content: "Later answer" },
  ];
  it("excludes both comparison answers and subsequent turns", () => {
    expect(historyForRetry(messages, 2)).toEqual([messages[0]]);
    expect(historyForRetry(messages, 1)).toEqual([messages[0]]);
    expect(messages).toHaveLength(5);
  });
  it("preserves earlier turns and attachments", () => {
    expect(historyForRetry(messages, 4)).toEqual(messages.slice(0, 4));
    expect(historyForRetry(messages, 1)[0].attachments).toEqual(
      messages[0].attachments,
    );
  });
  it("does not invent a question", () => {
    expect(historyForRetry([messages[1]], 0)).toEqual([]);
  });
});
