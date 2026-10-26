// @vitest-environment jsdom
import {it,expect} from 'vitest';
import * as React from 'react';
import {act} from 'react';
import {createRoot} from 'react-dom/client';
import {compileArtifact} from './compileArtifact';
it('compiles multiline React imports, TypeScript and JSX into a working interactive component',async()=>{
 const source=`import React, {\nuseState as useNumber\n} from 'react'; export default function Calculator(){ const [value,setValue]=useNumber<number>(3);return <button onClick={()=>setValue(value+1)}>{value}</button> }`;
 const output=compileArtifact(source);const host=document.createElement('div');host.id='app';document.body.append(host);let root:ReturnType<typeof createRoot>|undefined;
 const isolatedWindow={React,ReactDOM:{createRoot:(node:HTMLElement)=>{root=createRoot(node);return root}}};
 // Trusted test fixture only: the product executes this string in its opaque-origin frame.
 await act(async()=>{new Function('window','document',output)(isolatedWindow,document)});expect(host.textContent).toBe('3');await act(async()=>{host.querySelector('button')!.click()});expect(host.textContent).toBe('4');await act(async()=>root?.unmount());host.remove();
});
it('rejects dependency imports that would need network access',()=>{expect(()=>compileArtifact(`import anything from 'https://external.example/code.js';export default ()=>null;`)).toThrow(/uniquement React/)});
