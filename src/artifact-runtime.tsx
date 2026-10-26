import * as React from 'react';
import {createRoot} from 'react-dom/client';
import {compileArtifact} from './core/compileArtifact';
Object.assign(window,{React,ReactDOM:{createRoot}});
function showError(error:unknown){const node=document.createElement('pre');node.setAttribute('role','alert');node.style.cssText='color:#9b332a;white-space:pre-wrap;padding:12px';node.textContent=String(error);document.body.append(node)}
window.addEventListener('error',event=>showError(event.message));
window.addEventListener('unhandledrejection',event=>showError(event.reason));
let initialized=false;
window.addEventListener('message',event=>{
 if(initialized||event.source!==parent||event.data?.type!=='converge-artifact')return;
 initialized=true;
 try{if(event.data.kind==='react'){const output=compileArtifact(event.data.code);const script=document.createElement('script');script.src=URL.createObjectURL(new Blob([output],{type:'text/javascript'}));document.body.append(script)}}catch(error){showError(error)}
});
parent.postMessage({type:'converge-artifact-ready'},'*');
