import { expect, it } from "vitest";
import { channelCommand } from "./channelCommand";
type Event = { type: string; content?: string };
it("waits for buffered large messages and done even when invoke resolves first", async () => {
  let deliver!: (event: Event) => void;
  const events: Event[] = [];
  let completed = false;
  const result = channelCommand<string, Event>(
    async (receive) => {
      deliver = receive;
      return "native-return";
    },
    (event) => event.type === "done",
    (event) => events.push(event),
  ).then((value) => {
    completed = true;
    return value;
  });
  await Promise.resolve();
  await Promise.resolve();
  expect(completed).toBe(false);
  deliver({ type: "tool", content: "line\n".repeat(40000) });
  await Promise.resolve();
  expect(completed).toBe(false);
  deliver({ type: "done" });
  expect(await result).toBe("native-return");
  expect(events[0].content).toHaveLength(200000);
  deliver({ type: "late-progress" });
  expect(events).toHaveLength(2);
});
it("propagates early command failures immediately and ignores orphaned callbacks", async () => {
  let deliver!: (event: Event) => void;
  const events: Event[] = [];
  await expect(
    channelCommand<string, Event>(
      async (receive) => {
        deliver = receive;
        throw Error("Native failure");
      },
      (event) => event.type === "done",
      (event) => events.push(event),
    ),
  ).rejects.toThrow("Native failure");
  deliver({ type: "tool" });
  expect(events).toHaveLength(0);
});
it("does not finish before invoke when the terminal channel message arrives first", async () => {
  let resolve!: (value: string) => void;
  const result = channelCommand<string, Event>(
    (receive) => {
      receive({ type: "done" });
      return new Promise((r) => {
        resolve = r;
      });
    },
    (event) => event.type === "done",
    () => {},
  );
  resolve("ok");
  expect(await result).toBe("ok");
});
