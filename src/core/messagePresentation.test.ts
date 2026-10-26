import { expect, it } from "vitest";
import type { Message } from "../types";
import { messagePresentation } from "./messagePresentation";
it("shows one model label and final actions for a sequence of tool calls", () => {
  const messages: Message[] = [
    {
      id: "first",
      role: "assistant",
      model: "Local",
      turnId: "turn",
      content: "",
      toolCalls: [
        { id: "tool", name: "filesystem.read", args: {}, status: "succeeded" },
      ],
    },
    {
      id: "second",
      role: "assistant",
      model: "Local",
      turnId: "turn",
      content: "",
      toolCalls: [
        {
          id: "tool-2",
          name: "filesystem.write",
          args: {},
          status: "succeeded",
        },
      ],
    },
    {
      id: "final",
      role: "assistant",
      model: "Local",
      turnId: "turn",
      content: "Terminé",
    },
  ];
  expect(
    messages.map((_, i) => messagePresentation(messages, i).showHeading),
  ).toEqual([true, false, false]);
  expect(
    messages.map((_, i) => messagePresentation(messages, i).showActions),
  ).toEqual([false, false, true]);
  expect(messagePresentation(messages, 2, new Set(["turn"])).showActions).toBe(
    false,
  );
});
it("keeps model comparisons and separate user turns distinct", () => {
  const messages: Message[] = [
    { id: "a", role: "assistant", turnId: "a", model: "One", content: "" },
    { id: "b", role: "assistant", turnId: "b", model: "Two", content: "" },
    { id: "u", role: "user", content: "Next" },
    { id: "c", role: "assistant", turnId: "c", model: "One", content: "" },
  ];
  expect(messagePresentation(messages, 1).showHeading).toBe(true);
  expect(messagePresentation(messages, 3).showHeading).toBe(true);
});
