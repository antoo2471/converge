import type { Message } from "../types";

/** Stop at the question being retried, including in a multi-model comparison. */
export function historyForRetry(messages: Message[], index: number): Message[] {
  let promptIndex = Math.min(index, messages.length - 1);
  while (promptIndex >= 0 && messages[promptIndex].role !== "user")
    promptIndex--;
  return promptIndex < 0 ? [] : messages.slice(0, promptIndex + 1);
}
