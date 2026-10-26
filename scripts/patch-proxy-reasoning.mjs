import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";
const directory=process.argv[2] || path.join(process.env.LOCALAPPDATA,"npm-cache/_npx/d738525139dd3b97/node_modules/@thkdog/codex-openai-proxy/dist");
const file=path.join(directory,"transform.js");
let source=fs.readFileSync(file,"utf8");
if(!source.includes("// Converge reasoning effort transport")) {
  const marker="...convergeToolOptions(request),";
  assert.equal(source.split(marker).length-1,2,"Apply patch-proxy-tools.mjs first; expected both request builders");
  source=source.replaceAll(marker,marker+"\n        ...convergeReasoningOptions(request),");
  source+=`\n// Converge reasoning effort transport
function convergeReasoningOptions(request) {
  const effort=request.reasoning_effort ?? request.reasoning?.effort;
  if(effort == null || effort === "auto") return {};
  if(!["none","minimal","low","medium","high","xhigh","max"].includes(effort)) throw new TypeError("Unsupported reasoning effort");
  return {reasoning:{...(request.reasoning || {}),effort}};
}\n`;
}
const module=await import("data:text/javascript;base64,"+Buffer.from(source).toString("base64"));
for(const effort of ["low","medium","high","xhigh","max"]) {
  const output=module.buildCodexResponsesRequestFromChatCompletions({model:"gpt-6-luna",messages:[{role:"user",content:"Test"}],reasoning_effort:effort});
  assert.equal(output.reasoning.effort,effort);
}
assert.equal(module.buildCodexResponsesRequestFromChatCompletions({model:"gpt-6-luna",messages:[]}).reasoning,undefined);
assert.equal(module.buildCodexResponsesRequestFromResponses({model:"gpt-6-luna",input:[],reasoning:{effort:"high"}}).reasoning.effort,"high");
if(source!==fs.readFileSync(file,"utf8")) {
  if(!fs.existsSync(file+".converge-reasoning.bak"))fs.copyFileSync(file,file+".converge-reasoning.bak");
  fs.writeFileSync(file+".tmp",source);fs.renameSync(file+".tmp",file);
}
const route=path.join(directory,"routes.js");
let routes=fs.readFileSync(route,"utf8");
if (!routes.includes('!["reasoning", "reasoning_effort"].includes(key)')) routes=routes.replace('!allowedKeys.includes(key)', '!allowedKeys.includes(key) && !["reasoning", "reasoning_effort"].includes(key)');
if(routes!==fs.readFileSync(route,"utf8")){if(!fs.existsSync(route+".converge-reasoning.bak"))fs.copyFileSync(route,route+".converge-reasoning.bak");fs.writeFileSync(route+".tmp",routes);fs.renameSync(route+".tmp",route);}
console.log("Proxy reasoning forwarding verified offline for Chat Completions and Responses; no upstream requests made.");
