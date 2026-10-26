import { beforeEach, expect, it, vi } from "vitest";
import type { StreamEvent } from "../types";
const mock = vi.hoisted(() => ({
  channels: [] as { onmessage: (event: any) => void }[],
  invoke: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => true,
  convertFileSrc: (path: string) => path,
  invoke: mock.invoke,
  Channel: class {
    onmessage = (_event: any) => {};
    constructor() {
      mock.channels.push(this);
    }
  },
}));
import { streamChat, agentTool } from "./native";
import { defaults } from "../types";
beforeEach(() => {
  mock.channels.length = 0;
  mock.invoke.mockReset();
  mock.invoke.mockResolvedValue(undefined);
});
it("does not resolve the actual chat adapter before delayed tool and done callbacks", async () => {
  const events: StreamEvent[] = [];
  let resolved = false;
  const streaming = streamChat(
    "request",
    "provider",
    "model",
    [],
    defaults.settings,
    (event) => events.push(event),
  ).then(() => {
    resolved = true;
  });
  await Promise.resolve();
  await Promise.resolve();
  expect(resolved).toBe(false);
  mock.channels[0].onmessage({
    type: "tool",
    name: "filesystem_write",
    args: { path: "long.txt", content: "long line\n".repeat(30000) },
  });
  mock.channels[0].onmessage({ type: "done" });
  await streaming;
  expect(events.map((event) => event.type)).toEqual(["tool", "done"]);
});
it("waits for the file diff channel before returning a completed tool result", async () => {
  mock.invoke.mockResolvedValue("Written");
  const output: unknown[] = [];
  let resolved = false;
  const request = agentTool(
    "cid",
    "request",
    "filesystem.write",
    {},
    () => {},
    (event) => output.push(event),
  ).then((result) => {
    resolved = true;
    return result;
  });
  await Promise.resolve();
  await Promise.resolve();
  expect(resolved).toBe(false);
  mock.channels[1].onmessage({
    type: "file_changes",
    added: 40000,
    removed: 12000,
    diff: "+line\n".repeat(10000),
  });
  mock.channels[1].onmessage({ type: "finished" });
  expect(await request).toBe("Written");
  expect(output).toHaveLength(2);
});

it("transmits the saved effort for the actual selected model",async()=>{
 const call=streamChat("request","provider","model",[],{...defaults.settings,reasoningEfforts:{"provider::model":"high"}},()=>{});
 expect(mock.invoke).toHaveBeenCalledWith("stream_chat",expect.objectContaining({request:expect.objectContaining({reasoningEffort:"high"})}));
 mock.channels[0].onmessage({type:"done"});await call;
});
