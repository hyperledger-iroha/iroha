// Failure-only actual bootstrap controls. No original SDK or addon is imported.
import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { closeSync, mkdtempSync, openSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { canonical, fixture, hash } from "./sorafs_javascript_child_fixture.mjs";
const SESSION = new URL("../sorafs_javascript_child_session.mjs", import.meta.url).href;
const ENTRY = new URL("../sorafs_javascript_child_entry.mjs", import.meta.url).href;
for(const mode of ["entry-without-owner", "failed-preparation-is-terminal"])
  test(`fixed session actual refusal: ${mode}`,()=>{
    const root=mkdtempSync(join(process.env.TMPDIR,"sorafs-child-session-"));
    const raw=canonical(fixture()), input=join(root,"input.json");writeFileSync(input,raw,{mode:0o600});
    const program=`import assert from 'node:assert/strict';import {createRequire} from 'node:module';
import {fstatSync,openSync,closeSync,writeFileSync} from 'node:fs';
const {prepareChild,registerPreparedChild}=await import(${JSON.stringify(SESSION)});
const require=createRequire(import.meta.url),before=Object.keys(require.cache).filter(p=>p.endsWith('.node'));
if(${JSON.stringify(mode)}==='entry-without-owner'){
 await assert.rejects(import(${JSON.stringify(ENTRY)}),/no original prepared owner/);
 await assert.rejects(registerPreparedChild(),/no original prepared owner/);
}else{
 assert.throws(()=>prepareChild(process.argv[2]),/tool execution location differs/);
 assert.throws(()=>fstatSync(3),{code:'EBADF'});
 const path=${JSON.stringify(join(root,'reused'))};writeFileSync(path,'sentinel');const fd=openSync(path,'r');assert.equal(fd,3);
 assert.throws(()=>prepareChild(process.argv[2]),/one-shot/);
 await assert.rejects(registerPreparedChild(),/no original prepared owner/);
 assert.equal(fstatSync(fd).size,8);closeSync(fd);
}
assert.deepEqual(Object.keys(require.cache).filter(p=>p.endsWith('.node')),before);
console.log('refused without SDK/native execution');`;
    const controller=join(root,'controller.mjs');writeFileSync(controller,program);
    const fd=openSync(input,'r');
    try{
      const result=spawnSync(process.execPath,[controller,hash(raw)],{stdio:['ignore','pipe','pipe',fd],cwd:root,
        env:{PATH:dirname(process.execPath),TMPDIR:root},timeout:5000,maxBuffer:64*1024});
      assert.equal(result.status,0,result.stderr.toString());assert.equal(result.stderr.length,0);
      assert.equal(result.stdout.toString(),'refused without SDK/native execution\n');
    }finally{closeSync(fd);rmSync(root,{recursive:true,force:true});}
  });

test("actual Node24 fixed run keeps the original inert module owner through its final hook and EOF",()=>{
  const root=mkdtempSync(join(process.env.TMPDIR,"sorafs-child-mechanics-"));
  writeFileSync(join(root,"owner.mjs"),"export const owner={phase:'original'};\n");
  writeFileSync(join(root,"entry.mjs"),`import {test,after} from 'node:test';import assert from 'node:assert/strict';import {owner} from './owner.mjs';
test('inert same-process ownership',()=>{assert.equal(owner.phase,'prepared');owner.phase='executed';});
after(()=>{assert.equal(owner.phase,'executed');owner.phase='hook-complete';});`);
  writeFileSync(join(root,"controller.mjs"),`import {run} from 'node:test';import assert from 'node:assert/strict';import {fileURLToPath} from 'node:url';import {owner} from './owner.mjs';
const original=owner;owner.phase='prepared';const events=[];
for await(const event of run({files:[fileURLToPath(new URL('./entry.mjs',import.meta.url))],isolation:'none',concurrency:false}))events.push(event);
assert.equal(owner,original);assert.equal(owner.phase,'hook-complete');
assert.equal(events.filter(e=>e.type==='test:pass'&&e.data.name==='inert same-process ownership').length,1);
assert.equal(events.filter(e=>e.type==='test:fail').length,0);assert.equal(events.at(-1).type,'test:summary');assert.equal(events.at(-1).data.success,true);
console.log('inert same-process final-hook/EOF observed');`);
  try{
    const result=spawnSync(process.execPath,[join(root,"controller.mjs")],{cwd:root,env:{PATH:dirname(process.execPath),TMPDIR:root},timeout:5000,maxBuffer:64*1024});
    assert.equal(result.status,0,result.stderr.toString());assert.equal(result.stderr.length,0);
    assert.equal(result.stdout.toString(),"inert same-process final-hook/EOF observed\n");
  }finally{rmSync(root,{recursive:true,force:true});}
});
