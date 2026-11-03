import assert from 'node:assert/strict';
import test from 'node:test';
import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
const repos=[['photocraft','photo'],['vectorcraft','vector'],['effectcraft','motion'],['filmcraft','video'],['soundcraft','audio']];
function host(source,{embedded=true,stored=null,ready=false}={}){
 const listeners={},sent=[],calls=[],opened=[],timers=[];let available=ready;
 const w={};const parent=embedded?{postMessage:(...a)=>sent.push(a)}:w;
 const context={window:w,parent,location:{origin:'https://academy.test',href:'https://academy.test/studios/photo/app/',assign:u=>opened.push(u)},document:{documentElement:{dataset:{},style:{}}},localStorage:{getItem:()=>stored},URL,setTimeout:f=>timers.push(f),addEventListener:(n,f)=>{listeners[n]=f;},removeEventListener:()=>{}};
 w.open=(...a)=>{opened.push(a);return true;};w.wasmBindings={academyReady:()=>available,academyTheme:v=>calls.push(v)};
 vm.runInNewContext(source,context);return{context,parent,listeners,sent,calls,opened,timers,setReady(){available=true;},message(data,origin='https://academy.test',source=parent){listeners.message({data,origin,source});}};
}
for(const [repo,id]of repos){
 const p=new URL(`../${repo}/apps/${repo}-web/${['effectcraft','filmcraft'].includes(repo)?'web/':''}academy-bridge.js`,import.meta.url);const source=await readFile(p,'utf8');
 test(`${id}: strict host messages, deferred readiness and native palette only`,()=>{const h=host(source);for(const m of [{type:'aca:theme',version:0,theme:'light'},{type:'aca:theme',version:1,theme:'script'},{type:'arbitrary',version:1,theme:'light'}])h.message(m);h.message({type:'aca:theme',version:1,theme:'light'},'https://other.test');h.message({type:'aca:theme',version:1,theme:'light'},undefined,{});assert.equal(h.calls.length,0);h.message({type:'aca:theme',version:1,theme:'light'});assert.equal(h.calls.length,0);h.setReady();h.timers.shift()();assert.deepEqual(h.calls,[true]);assert.equal(h.sent[0][0].type,'aca:ready');h.message({type:'aca:theme',version:1,theme:'dark'});assert.deepEqual(h.calls,[true,false]);});
 test(`${id}: exact tutorial request stays local; unrelated links keep normal behavior`,()=>{const h=host(source,{ready:true});h.context.window.open('/2d/'+id+'#tutorial','_blank');assert.equal(h.sent.at(-1)[0].type,'aca:tutorial');assert.equal(h.sent.at(-1)[0].appId,id);assert.equal(h.opened.length,0);h.context.window.open('https://other.test/2d/'+id+'#tutorial');assert.equal(h.opened.length,1);});
 test(`${id}: standalone preference applies without parent or project commands`,()=>{const h=host(source,{embedded:false,stored:'{"version":1,"theme":"light"}',ready:true});assert.deepEqual(h.calls,[true,true]);assert.equal(h.sent.length,0);h.context.window.open('/2d/'+id+'#tutorial');assert.equal(h.opened[0],'https://academy.test/2d/'+id+'#tutorial');});
}
