import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';

const directory = process.argv[2] || path.join(process.env.LOCALAPPDATA, 'npm-cache/_npx/d738525139dd3b97/node_modules/@thkdog/codex-openai-proxy/dist');
const transformPath = path.join(directory, 'transform.js');
const routesPath = path.join(directory, 'routes.js');
let transform = fs.readFileSync(transformPath, 'utf8');
let routes = fs.readFileSync(routesPath, 'utf8');
const helper = `
// Converge native tool transport: definitions in, function-call events out.
export function convergeToolOptions(request) {
    const tools = (request.tools || []).map(tool => {
        if (tool.type !== "function") throw new TypeError("Unsupported tool type: " + tool.type);
        const fn = tool.function || tool;
        if (typeof fn.name !== "string") throw new TypeError("A function tool requires a name");
        return { type: "function", name: fn.name, description: fn.description || "", parameters: fn.parameters || { type: "object", properties: {} }, strict: fn.strict ?? false };
    });
    const choice = request.tool_choice;
    return { ...(tools.length ? { tools } : {}), tool_choice: choice?.function ? { type: "function", name: choice.function.name } : choice || "auto", parallel_tool_calls: request.parallel_tool_calls ?? true };
}
// Converge streaming tools v2: keep argument deltas live without duplicate function calls.
export function createConvergeToolCollector(responseId, model) {
    const calls = [], indices = new Map(), completed = new Set();
    function chunk(index, fn, extra = {}) { return { id: responseId, object: "chat.completion.chunk", created: Math.floor(Date.now()/1000), model, choices: [{ index: 0, delta: { tool_calls: [{ index, ...extra, function: fn }] }, finish_reason: null }] }; }
    function begin(item, args) {
        const index = calls.length, id = item.call_id || item.id || "call_" + index;
        const call = { id, type: "function", function: { name: item.name, arguments: args } }; calls.push(call);
        for (const key of [id, item.id, item.call_id]) if (key) indices.set(key, index);
        return chunk(index, call.function, { id, type: "function" });
    }
    return { calls, ingest(event) {
        if (event.type === "response.output_item.added" && event.item?.type === "function_call") {
            const item = event.item; if (indices.has(item.id) || indices.has(item.call_id)) return [];
            return [begin(item, typeof item.arguments === "string" ? item.arguments : "")];
        }
        if (event.type === "response.function_call_arguments.delta") {
            const index = indices.get(event.item_id); if (index === undefined) return [];
            const delta = event.delta || ""; calls[index].function.arguments += delta;
            return delta ? [chunk(index, { arguments: delta })] : [];
        }
        const items = event.type === "response.output_item.done" ? [event.item] : event.type === "response.completed" ? event.response?.output || [] : [];
        const chunks = [];
        for (const item of items) {
            if (item?.type !== "function_call") continue;
            const key = item.call_id || item.id || item.name + ":" + item.arguments;
            if (completed.has(key)) continue;
            if (typeof item.name !== "string") throw new TypeError("Function call name missing");
            const args = typeof item.arguments === "string" ? item.arguments : JSON.stringify(item.arguments || {}); JSON.parse(args);
            const index = indices.get(item.id) ?? indices.get(item.call_id);
            if (index === undefined) chunks.push(begin(item, args));
            else {
                const previous = calls[index].function.arguments;
                if (!args.startsWith(previous)) throw new TypeError("Function argument stream was inconsistent");
                const tail = args.slice(previous.length); if (tail) chunks.push(chunk(index, { arguments: tail }));
                calls[index].function.arguments = args;
            }
            completed.add(key);
        }
        if (event.type === "response.completed") for (const call of calls) JSON.parse(call.function.arguments);
        return chunks;
    } };
}

`;

if (!transform.includes('// Converge native tool transport:')) {
  const oldOptions = 'tool_choice: "auto",\n        parallel_tool_calls: true,';
  assert.equal(transform.split(oldOptions).length - 1, 2, 'Expected the two known proxy request builders');
  transform = transform.replaceAll(oldOptions, '...convergeToolOptions(request),') + helper;
}
if (!transform.includes('// Converge streaming tools v2:')) {
  const start = transform.indexOf('// Converge native tool transport:');
  assert.ok(start >= 0);
  transform = transform.slice(0, start) + helper;
}
if (!routes.includes('// Converge: forward native function calls')) {
  assert.ok(routes.includes('collectTextFromCodexEvents, createChatCompletion,'));
  routes = routes.replace('collectTextFromCodexEvents, createChatCompletion,', 'collectTextFromCodexEvents, createConvergeToolCollector, createChatCompletion,');
  routes = routes.replace('["model", "messages", "stream"]', '["model", "messages", "stream", "tools", "tool_choice", "parallel_tool_calls"]');
  routes = routes.replace('["model", "instructions", "input", "stream"]', '["model", "instructions", "input", "stream", "tools", "tool_choice", "parallel_tool_calls"]');
  const responseLine = 'const responseId = `chatcmpl_${Date.now()}`;';
  assert.ok(routes.includes(responseLine));
  routes = routes.replace(responseLine, responseLine + '\n            const toolCollector = createConvergeToolCollector(responseId, model);');
  const textEvent = 'if (event.type === "response.output_text.delta" && event.delta) {';
  assert.ok(routes.includes(textEvent));
  routes = routes.replace(textEvent, '// Converge: forward native function calls, not only assistant prose.\n                    for (const chunk of toolCollector.ingest(event)) writeSSE(reply, chunk);\n                    ' + textEvent);
  const finish = 'writeSSE(reply, createChatCompletionDoneChunk(responseId, model));';
  assert.ok(routes.includes(finish));
  routes = routes.replace(finish, 'const done = createChatCompletionDoneChunk(responseId, model);\n                        if (toolCollector.calls.length) done.choices[0].finish_reason = "tool_calls";\n                        writeSSE(reply, done);');
  const nonstream = 'return createChatCompletion(responseId, model, text);';
  assert.ok(routes.includes(nonstream));
  routes = routes.replace(nonstream, 'const toolCollector = createConvergeToolCollector(responseId, model);\n        for (const event of events) toolCollector.ingest(event);\n        const completion = createChatCompletion(responseId, model, text);\n        if (toolCollector.calls.length) { completion.choices[0].message.tool_calls = toolCollector.calls; completion.choices[0].finish_reason = "tool_calls"; }\n        return completion;');
  routes = routes.replace('return createOpenAIResponsesOutput(responseId, text);', 'return responseObject || createOpenAIResponsesOutput(responseId, text);');
}

// Verify the actual patched route against a simulated upstream. No login or network used.
const pure = await import('data:text/javascript;base64,' + Buffer.from(transform).toString('base64'));
const definition = { type: 'function', function: { name: 'filesystem_write', description: 'Write a file', parameters: { type: 'object', properties: { path: { type: 'string' }, content: { type: 'string' } }, required: ['path', 'content'], additionalProperties: false } } };
const input = { model: 'fixture', messages: [{ role: 'user', content: 'Create hello.txt' }], tools: [definition], stream: true };
const built = pure.buildCodexResponsesRequestFromChatCompletions(input);
assert.equal(built.tools[0].name, 'filesystem_write');
assert.deepEqual(built.tools[0].parameters, definition.function.parameters);
assert.equal(pure.buildCodexResponsesRequestFromResponses({ tools: [{ type: 'function', ...definition.function }] }).tools[0].name, 'filesystem_write');
const item = { type: 'function_call', id: 'fc_fixture', call_id: 'call_fixture', name: 'filesystem_write', arguments: '{"path":"hello.txt","content":"hello"}' };
const events = [{ type: 'response.output_text.delta', delta: 'I will create the file.' }, { type: 'response.output_item.done', item }, { type: 'response.completed', response: { id: 'resp_fixture', output: [item] } }];
globalThis.__convergeProxyTransform = pure;
globalThis.__convergeProxyClient = { fetchCodexModels: async () => ({}), collectCodexResponseEvents: async () => events, streamCodexResponseToSSE: async (body, _reply, emit) => { assert.equal(body.tools[0].name, 'filesystem_write'); for (const event of events) emit(event); } };
const routeFixture = routes
  .replace(/import \{[\s\S]*?\} from "\.\/codex-client\.js";/, 'const {collectCodexResponseEvents, fetchCodexModels, streamCodexResponseToSSE} = globalThis.__convergeProxyClient;')
  .replace(/import \{([^}]+)\} from "\.\/transform\.js";/, 'const {$1} = globalThis.__convergeProxyTransform;');
const routeModule = await import('data:text/javascript;base64,' + Buffer.from(routeFixture).toString('base64'));
const handlers = new Map();
await routeModule.registerRoutes({ get() {}, post(url, handler) { handlers.set(url, handler); } });
const output = [];
await handlers.get('/v1/chat/completions')({ body: input, log: { warn() {}, error(error) { throw error; } } }, { hijack() {}, raw: { writeHead() {}, write(chunk) { output.push(chunk); }, end() {} } });
const chunks = output.join('').split('\n').filter(line => line.startsWith('data: ') && !line.includes('[DONE]')).map(line => JSON.parse(line.slice(6)));
assert.equal(chunks.flatMap(chunk => chunk.choices[0].delta.tool_calls || []).length, 1, 'A function must not be duplicated by response.completed');
assert.equal(chunks.at(-1).choices[0].finish_reason, 'tool_calls');
const final = await handlers.get('/v1/chat/completions')({ body: { ...input, stream: false }, log: { warn() {} } }, {});
assert.equal(final.choices[0].message.tool_calls[0].function.name, 'filesystem_write');
const live = pure.createConvergeToolCollector('fixture', 'model');
assert.equal(live.ingest({type:'response.output_item.added',item:{...item,arguments:''}}).length,1);
const firstArgs = item.arguments.slice(0,15), lastArgs = item.arguments.slice(15);
assert.equal(live.ingest({type:'response.function_call_arguments.delta',item_id:item.id,delta:firstArgs})[0].choices[0].delta.tool_calls[0].function.arguments,firstArgs);
live.ingest({type:'response.function_call_arguments.delta',item_id:item.id,delta:lastArgs});
assert.equal(live.ingest({type:'response.output_item.done',item}).length,0);
assert.equal(live.ingest(events.at(-1)).length,0);
assert.equal(live.calls[0].function.arguments,item.arguments);
const fallback = pure.createConvergeToolCollector('fixture', 'model');
assert.equal(fallback.ingest(events.at(-1)).length, 1, 'Completed-only upstreams must retain tool calls');

for (const [file, content] of [[transformPath, transform], [routesPath, routes]]) {
  if (content === fs.readFileSync(file, 'utf8')) continue;
  if (!fs.existsSync(file + '.converge-tools.bak')) fs.copyFileSync(file, file + '.converge-tools.bak');
  const temporary = file + '.converge-tools.tmp'; fs.writeFileSync(temporary, content); fs.renameSync(temporary, file);
}
delete globalThis.__convergeProxyClient; delete globalThis.__convergeProxyTransform;
console.log('Proxy patched and verified offline: tool schemas, streamed function calls, deduplication, completed-only events, and non-streaming results. No upstream requests made.');
