// Actual builtin-only Node24 loader controls with inert named modules. These
// synthetic descriptor rows cannot pass parent original-package qualification;
// no SDK/addon or production child is executed or reported as qualified.
import { test } from "node:test";
import assert from "node:assert/strict";
import { closeSync, mkdirSync, mkdtempSync, openSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { canonical, fixture, hash } from "./sorafs_javascript_child_fixture.mjs";

const INPUT = new URL("../sorafs_javascript_child_input.mjs", import.meta.url).href;
const LOADS = new URL("../sorafs_javascript_child_loads.mjs", import.meta.url).href;
for (const mode of ["normal", "changed-source", "data-url", "query-url", "unknown-file", "late-quiet-hook", "late-transform-hook", "fake-input-owner"])
  test(`actual inert loader observation: ${mode}`, () => {
    const root = mkdtempSync(join(process.env.TMPDIR, "sorafs-child-loads-"));
    const input = fixture(); input.environmentRoot = root; input.temporaryRoot = join(root, "temporary");
    const tools = join(root, "qualification/tools"), dependency = join(root, "node_modules/base64-js/index.js");
    const jsonPath = join(root, "node_modules/base64-js/data.json"), entry = join(tools, "sorafs_javascript_child_entry.mjs");
    mkdirSync(tools, { recursive: true }); mkdirSync(dirname(dependency), { recursive: true });
    const sources = [[dependency, "module.exports = 7;\n"], [jsonPath, '{"value":4}\n'],
      [entry, `import x from ${JSON.stringify(pathToFileURL(dependency).href)}; import y from ${JSON.stringify(pathToFileURL(jsonPath).href)} with {type:'json'}; export const value=x+y.value;\n`]];
    for (const [path, text] of sources) {
      writeFileSync(path, text, { mode: 0o644 });
      const row = { path: path.startsWith(tools) ? path.slice(tools.length + 1) : path.slice(join(root, "node_modules").length + 1),
        sha256: hash(Buffer.from(text)), size: Buffer.byteLength(text), mode: 0o644 };
      if (path === entry) input.tools[input.tools.findIndex((r) => r.path === row.path)] = row;
      else input.installed.push(row);
    }
    input.installed.sort((a,b)=>a.path<b.path?-1:1);
    const controller = `import assert from 'node:assert/strict';
import {registerHooks,createRequire} from 'node:module';
import {writeFileSync} from 'node:fs';
import {OriginalChildInput} from ${JSON.stringify(INPUT)};
import {ChildLoadObservations} from ${JSON.stringify(LOADS)};
const mode=${JSON.stringify(mode)};const owner=new OriginalChildInput(process.argv[2]);let observed,secondary,error,result;
try {
 if(mode==='fake-input-owner') {assert.throws(()=>new ChildLoadObservations({value:owner.value}));console.log('brand-refused');}
 else {
 observed=new ChildLoadObservations(owner);
 if(mode==='changed-source')writeFileSync(${JSON.stringify(dependency)},'module.exports = 99;\\n');
 if(mode==='late-quiet-hook')secondary=registerHooks({load(url,context,next){if(url===${JSON.stringify(pathToFileURL(entry).href)})return {format:'module',source:'export const value=99;',shortCircuit:true};return next(url,context);}});
 if(mode==='late-transform-hook')secondary=registerHooks({load(url,context,next){const row=next(url,context);if(url===${JSON.stringify(pathToFileURL(entry).href)})return {...row,source:'export const value=99;'};return row;}});
 const target=mode==='data-url'?'data:text/javascript,export const value=99;':mode==='unknown-file'?${JSON.stringify(pathToFileURL(join(root,'unknown.mjs')).href)}:${JSON.stringify(pathToFileURL(entry).href)}+(mode==='query-url'?'?alias':'');
 try {const first=await import(target);result=first.value;if(mode==='normal'){assert.equal(await import(target),first);assert.equal(createRequire(import.meta.url)(${JSON.stringify(dependency)}),7);}}
 catch(failure){error=String(failure);}
 if(mode==='normal') {assert.equal(error,undefined);assert.equal(result,11);}
 else if(mode.startsWith('late-')){assert.equal(error,undefined);assert.equal(result,99);}
 else assert.ok(error);
 // With no actual native load this owner cannot yield a completed child load
 // observation, even when every inert source was positive. No fake cache used.
 assert.throws(()=>observed.recheck());assert.throws(()=>observed.recheck());
 }
} finally {secondary?.deregister();observed?.close();owner.close();}
console.log(JSON.stringify({scope:'inert loader component, not SDK qualification',mode,result:result??null,error:error??null}));`;
    const controllerPath = join(tools, "sorafs_javascript_child.mjs");
    writeFileSync(controllerPath, controller, { mode: 0o644 });
    const row = input.tools.find((r)=>r.path==='sorafs_javascript_child.mjs'); row.sha256=hash(Buffer.from(controller));row.size=Buffer.byteLength(controller);
    writeFileSync(join(root, "unknown.mjs"), 'export const value=99;\n');
    const raw = canonical(input), inputPath = join(root, "input.json"); writeFileSync(inputPath, raw, { mode: 0o600 });
    const fd = openSync(inputPath, "r");
    try {
      const result = spawnSync(process.execPath,[controllerPath,hash(raw)],{stdio:["ignore","pipe","pipe",fd],
        timeout:5000,maxBuffer:64*1024,env:{PATH:dirname(process.execPath),TMPDIR:root},cwd:root});
      assert.equal(result.status,0,result.stderr.toString());assert.equal(result.stderr.length,0);
      const line = result.stdout.toString().trim().split("\n").at(-1), report = JSON.parse(line);
      assert.equal(report.mode,mode);assert.equal(report.scope,'inert loader component, not SDK qualification');
    } finally {closeSync(fd);rmSync(root,{recursive:true,force:true});}
  });
