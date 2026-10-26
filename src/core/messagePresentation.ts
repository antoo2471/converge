import type { Message } from "../types";
function continues(
  previous: Message | undefined,
  current: Message | undefined,
) {
  if (
    !previous ||
    !current ||
    previous.role !== "assistant" ||
    current.role !== "assistant"
  )
    return false;
  if (previous.turnId && current.turnId)
    return previous.turnId === current.turnId;
  return (
    !!previous.toolCalls?.length &&
    previous.model === current.model &&
    previous.providerId === current.providerId
  );
}
export function messagePresentation(
  messages: Message[],
  index: number,
  activeTurns: ReadonlySet<string> = new Set(),
) {
  const message = messages[index];
  const continuation = continues(messages[index - 1], message);
  const hasContinuation = continues(message, messages[index + 1]);
  const running =
    message.role === "assistant" &&
    ((message.turnId && activeTurns.has(message.turnId)) ||
      message.toolCalls?.some((call) =>
        ["preparing", "awaiting", "running"].includes(call.status),
      ));
  return {
    continuation,
    hasContinuation,
    showHeading: message.role !== "user" && !continuation,
    showActions: !hasContinuation && !running,
  };
}
