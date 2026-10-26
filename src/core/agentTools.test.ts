import { expect, it } from "vitest";
import {
  extractToolRequests,
  expandToolHistory,
  settledCall,
} from "./agentTools";
import type { Message, ToolCall } from "../types";
const request = {
  name: "filesystem.write",
  args: { path: "hello.txt", content: "hello\n" },
};
it("recognizes a tool fence joined directly to the preceding sentence", () => {
  const content =
    'Je crée `hello.txt` dans le dossier choisi.```converge-tool\n{"name":"filesystem.write","args":{"path":"hello.txt","content":""}}\n```';
  expect(extractToolRequests(content)).toEqual({
    requests: [
      { name: "filesystem.write", args: { path: "hello.txt", content: "" } },
    ],
    text: "Je crée `hello.txt` dans le dossier choisi.",
  });
  expect(
    extractToolRequests(`\`\`\`json\n${content}\n\`\`\``).requests,
  ).toEqual([]);
});
it("recognizes completed native-compatible blocks and inline requests without executing prose or partial JSON", () => {
  expect(
    extractToolRequests(
      `Je crée le fichier.\n\n\`\`\`converge-tool\n${JSON.stringify(request)}\n\`\`\``,
    ),
  ).toEqual({ requests: [request], text: "Je crée le fichier." });
  expect(
    extractToolRequests(`\`converge-tool ${JSON.stringify(request)}\``)
      .requests,
  ).toEqual([request]);
  expect(
    extractToolRequests(`\`\`\`converge-tool\n${JSON.stringify(request)}`)
      .requests,
  ).toEqual([]);
  expect(extractToolRequests(JSON.stringify(request)).requests).toEqual([]);
  expect(
    extractToolRequests(
      '```converge-tool\n{"name":"filesystem.write","args":null}\n```',
    ).requests,
  ).toEqual([]);
});
it("returns actual results to the model without modifying or replaying persisted messages", () => {
  const call: ToolCall = { ...request, id: "test", status: "running" };
  const settled = settledCall(call, "Fichier enregistré");
  const history: Message[] = [
    { id: "a", role: "assistant", content: "", toolCalls: [settled] },
  ];
  const context = expandToolHistory(history);
  expect(context).toHaveLength(2);
  expect(context[1].content).toContain("Fichier enregistré");
  expect(context[1].role).toBe("user");
  expect(history).toHaveLength(1);
  expect(settledCall(call, "REFUSED: Action refusée", true).status).toBe(
    "denied",
  );
  expect(settledCall(call, "CANCELLED: Arrêt", true).status).toBe("cancelled");
});
it("never extracts tool examples embedded in another code block or unfinished artifact", () => {
  const inline = `\`converge-tool ${JSON.stringify(request)}\``;
  expect(extractToolRequests(`\`\`\`json\n${inline}\n\`\`\``).requests).toEqual(
    [],
  );
  expect(
    extractToolRequests(
      `\`\`\`artifact-html\n<script>const sample = '${inline}';</script>`,
    ).requests,
  ).toEqual([]);
  expect(
    extractToolRequests(
      `~~~~html\n\`\`\`converge-tool\n${JSON.stringify(request)}\n\`\`\`\n~~~~`,
    ).requests,
  ).toEqual([]);
});
it("keeps screenshots out of text and attaches them as real vision inputs", () => {
  const context = expandToolHistory([
    {
      id: "a",
      role: "assistant",
      content: "",
      toolCalls: [
        {
          id: "capture",
          name: "computer.capture",
          args: {},
          status: "succeeded",
          result: "data:image/png;base64,aGVsbG8=",
        },
      ],
    },
  ]);
  expect(context[1].content).not.toContain("aGVsbG8=");
  expect(context[1].attachments?.[0]).toMatchObject({
    mime: "image/png",
    data: "aGVsbG8=",
  });
});

it("forwards screenshot metadata, attaching only the newest observation", () => {
 const observation=(id:string)=>JSON.stringify({kind:"computer_observation",image:"data:image/jpeg;base64,aGVsbG8=",frameId:id,width:1568,height:882});
 const context=expandToolHistory(["old","latest"].map(id=>({id,role:"assistant" as const,content:"",toolCalls:[{id,name:"computer.click",args:{x:1,y:1},status:"succeeded" as const,result:observation(id)}]})));
 expect(context[1].attachments).toEqual([]);
 expect(context[3].attachments?.[0].mime).toBe("image/jpeg");
 expect(JSON.parse(context[3].content.split("\n")[1])[0].result).toContain('"frameId":"latest"');
 expect(context[3].content).not.toContain("aGVsbG8=");
});
