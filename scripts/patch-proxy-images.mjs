import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';

const root=process.argv[2];
if(!root)throw Error('Usage: node scripts/patch-proxy-images.mjs <package-directory>');
const manifest=JSON.parse(fs.readFileSync(path.join(root,'package.json'),'utf8'));
assert.equal(manifest.name,'@thkdog/codex-openai-proxy','Refusing to modify a different package');
const file=path.join(root,'dist','transform.js');
const source=fs.readFileSync(file,'utf8');
const signature='function toInputTextParts(content) {';
const end='export function chatCompletionsToResponsesInput(request) {';
const startIndex=source.indexOf(signature),endIndex=source.indexOf(end);
if(!source.includes('function toInputTextParts(content, includeImages = false) {')){
 assert.ok(startIndex>=0&&endIndex>startIndex,'Unknown proxy implementation; no files changed');
 const replacement=`function toInputTextParts(content, includeImages = false) {
    if (!content) return [];
    if (typeof content === "string") return [{ type: "input_text", text: content }];
    if (!Array.isArray(content)) return [];
    return content.flatMap((part) => {
        if (!part || typeof part !== "object") return [];
        if ((part.type === "text" || part.type === "input_text") && typeof part.text === "string") {
            return [{ type: "input_text", text: part.text }];
        }
        // Converge: preserve multimodal input when converting Chat Completions to Responses.
        if (!includeImages) return [];
        if (part.type === "image_url") {
            const imageUrl = typeof part.image_url === "string" ? part.image_url : part.image_url?.url;
            if (typeof imageUrl !== "string" || !imageUrl) throw new TypeError("image_url requires a nonempty URL or data URL");
            const image = { type: "input_image", image_url: imageUrl };
            if (typeof part.image_url?.detail === "string") image.detail = part.image_url.detail;
            return [image];
        }
        if (part.type === "input_image") {
            const image = { type: "input_image" };
            if (typeof part.image_url === "string" && part.image_url) image.image_url = part.image_url;
            else if (typeof part.file_id === "string" && part.file_id) image.file_id = part.file_id;
            else throw new TypeError("input_image requires image_url or file_id");
            if (typeof part.detail === "string") image.detail = part.detail;
            return [image];
        }
        return [];
    });
}
`;
 const patched=(source.slice(0,startIndex)+replacement+source.slice(endIndex)).replace('content: toInputTextParts(message.content),','content: toInputTextParts(message.content, message.role === "user"),');
 assert.ok(patched.includes('content: toInputTextParts(message.content, message.role === "user"),'),'Conversion call site not found');
 const backup=file+'.converge-images.bak';if(!fs.existsSync(backup))fs.copyFileSync(file,backup);
 fs.writeFileSync(file,patched);
}

const {chatCompletionsToResponsesInput:convert,buildCodexResponsesRequestFromChatCompletions:build}=await import(pathToFileURL(file).href+'?verify='+Date.now());
const url='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aB1cAAAAASUVORK5CYII=';
const image={type:'image_url',image_url:{url,detail:'high'}};
const mixed=convert({messages:[{role:'system',content:'System instructions'},{role:'user',content:[{type:'text',text:'Describe this image'},image]}]});
assert.equal(mixed.instructions,'System instructions');
assert.deepEqual(mixed.input[0].content,[{type:'input_text',text:'Describe this image'},{type:'input_image',image_url:url,detail:'high'}]);
const imageOnly=convert({messages:[{role:'user',content:[image]}]});assert.equal(imageOnly.input.length,1);assert.equal(imageOnly.input[0].content[0].image_url,url);
const multiple=build({model:'contract-model',messages:[{role:'user',content:[image,image]}]});assert.equal(multiple.input[0].content.length,2);assert.equal(multiple.model,'contract-model');
assert.equal(convert({messages:[{role:'user',content:'Text only'}]}).input[0].content[0].text,'Text only');
assert.equal(convert({messages:[{role:'system',content:[image,{type:'text',text:'Instructions'}]}]}).instructions,'Instructions');
assert.equal(convert({messages:[{role:'user',content:[{type:'input_image',file_id:'file-fixture'}]}]}).input[0].content[0].file_id,'file-fixture');
console.log('Proxy patched. Verified: text + image, image-only messages, multiple images, detail, existing text, system instructions, and file references. No upstream requests sent.');
