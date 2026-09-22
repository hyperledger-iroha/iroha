// Synthetic input/actual inherited-file controls only; no installed SDK/addon or
// production child runs. Minimal inert installed/tool rows are deliberately not
// a valid original package projection and cannot qualify candidate execution.
import { test } from "node:test";
import assert from "node:assert/strict";
import { chmodSync, closeSync, ftruncateSync, mkdtempSync, openSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { parseChildInput } from "../sorafs_javascript_child_input.mjs";

const MODULE = new URL("../sorafs_javascript_child_input.mjs", import.meta.url).href;
import { canonical, fixture, hash } from "./sorafs_javascript_child_fixture.mjs";
function parsed(value) { const raw = canonical(value); return parseChildInput(raw, hash(raw)); }

test("pure input preserves the fixed original191 source census without claiming an installed projection", () => {
  const input = fixture(), result = parsed(input);
  assert.equal(result.source.length, 191); assert.equal(result.tools.length, 8);
  assert.equal(result.source.filter((row) => row.path.endsWith(".js")).length, 7);
  input.source.length = 0; assert.equal(result.source.length, 191);
  assert.ok(Object.isFrozen(result) && Object.isFrozen(result.source) && Object.isFrozen(result.source[0]));
});
for (const [name, mutate] of [
  ["unknown input field", (v) => { v.callback = "arbitrary"; }],
  ["missing native", (v) => { delete v.native; }],
  ["old schema", (v) => { v.schema += ".old"; }],
  ["root alias", (v) => { v.environmentRoot += "/../environment"; }],
  ["noncanonical native name", (v) => { v.native.originalPath = "/owned/native/other.node"; }],
  ["oversize native", (v) => { v.native.size = 1024 ** 3 + 1; }],
  ["boolean native size", (v) => { v.native.size = true; }],
  ["source code substitution", (v) => { v.source.find((row) => row.path.endsWith("cancelAssetLockV1.js")).sha256 = "f".repeat(64); }],
  ["source selector substitution", (v) => { v.source[0].path += ".extra"; }],
  ["missing source", (v) => { v.source.pop(); }],
  ["source fixture mode", (v) => { v.source[0].mode = 0o755; }],
  ["oversize source", (v) => { v.source[0].size = 16 * 1024 ** 2 + 1; }],
  ["duplicate source", (v) => { v.source[1] = v.source[0]; }],
  ["source reordering", (v) => { v.source.reverse(); }],
  ["unreviewed catalog", (v) => { v.catalog = Buffer.from("{}").toString("base64"); }],
  ["base64 alias", (v) => { v.catalog += "\n"; }],
  ["missing subject", (v) => { v.installed.pop(); }],
  ["extra tool", (v) => { v.tools.push(v.tools[0]); }],
  ["missing tool", (v) => { v.tools.pop(); }],
  ["mutable output mode", (v) => { v.tools[0].mode = 0o666; }],
]) test(`input rejects ${name}`, () => { const value = fixture(); mutate(value); assert.throws(() => parsed(value)); });
test("canonical encoding rejects duplicate fields and negative-zero spelling", () => {
  const raw = canonical(fixture());
  for (const altered of [Buffer.from(raw.toString().replace('{', '{"schema":"foreign",')),
    Buffer.from(raw.toString().replace('"size":1,', '"size":-0,')), Buffer.concat([raw, Buffer.from(" ")])])
    assert.throws(() => parseChildInput(altered, hash(altered)));
});
test("canonical input retains Unicode paths losslessly including supplementary characters", () => {
  const value = fixture(); value.environmentRoot = "/owned/漢字/𒀀";
  assert.equal(parsed(value).environmentRoot, value.environmentRoot);
});
test("input refuses a different independent digest and exact8MiB overflow", () => {
  const raw = canonical(fixture()); assert.throws(() => parseChildInput(raw, "f".repeat(64)));
  const oversized = Buffer.alloc(8 * 1024 * 1024 + 1, 32);
  assert.throws(() => parseChildInput(oversized, hash(oversized)));
});

const CHILD = `import {OriginalChildInput} from ${JSON.stringify(MODULE)};
import {openSync,closeSync,fstatSync} from 'node:fs';
import assert from 'node:assert/strict';
let owner;
try {owner=new OriginalChildInput(process.argv[1]);}
catch(error){assert.throws(()=>fstatSync(3),{code:'EBADF'});console.log(JSON.stringify({refused:true,error:String(error)}));process.exit(0);}
assert.equal(owner.recheck(),owner.value); const digest=owner.value.inputSha256;
owner.close();assert.throws(()=>owner.recheck());
const fd=openSync(process.argv[2],'r');assert.equal(fd,3);owner.close();fstatSync(fd);closeSync(fd);
console.log(JSON.stringify({digest,closedOnce:true}));`;
for (const mode of ["valid", "world-mode", "changed-bytes", "oversized", "directory"])
  test(`actual inherited regular-file owner: ${mode}`, () => {
    const directory = mkdtempSync(join(process.env.TMPDIR, "sorafs-child-input-"));
    const name = join(directory, "input.json"), sentinel = join(directory, "sentinel");
    const raw = canonical(fixture()); writeFileSync(name, raw, { mode: 0o600 }); writeFileSync(sentinel, "original sentinel");
    if (mode === "world-mode") chmodSync(name, 0o644);
    if (mode === "changed-bytes") writeFileSync(name, Buffer.concat([raw, Buffer.from(" ")]));
    const fd = openSync(mode === "directory" ? directory : name, mode === "oversized" ? "r+" : "r");
    try {
      if (mode === "oversized") ftruncateSync(fd, 8 * 1024 * 1024 + 1);
      const result = spawnSync(process.execPath, ["--input-type=module", "-e", CHILD, hash(raw), sentinel], {
        stdio: ["ignore", "pipe", "pipe", fd], timeout: 5000, maxBuffer: 64 * 1024,
        env: { PATH: dirname(process.execPath), TMPDIR: directory }, cwd: directory,
      });
      assert.equal(result.status, 0, result.stderr.toString()); assert.equal(result.stderr.length, 0);
      const observed = JSON.parse(result.stdout);
      if (mode === "valid") assert.deepEqual(observed, { digest: hash(raw), closedOnce: true });
      else assert.equal(observed.refused, true);
    } finally { closeSync(fd); rmSync(directory, { recursive: true, force: true }); }
  });

// Actual inherited-descriptor lifecycle controls. Mutation is confined to this
// test child's builtin instrumentation; production exposes no injected callback.
for (const mode of ["close-during-recheck", "swallowed-reentry", "ambiguous-close-reused-fd"])
  test(`inherited input lifecycle refuses ${mode}`, () => {
    const root = mkdtempSync(join(process.env.TMPDIR, "sorafs-child-input-lifecycle-"));
    const raw = canonical(fixture()), path = join(root,"input.json");
    writeFileSync(path,raw,{mode:0o600});
    const controller=`import assert from 'node:assert/strict';import fs from 'node:fs';import {syncBuiltinESMExports} from 'node:module';
import {OriginalChildInput} from ${JSON.stringify(MODULE)};
const owner=new OriginalChildInput(process.argv[2]), mode=${JSON.stringify(mode)};
const fstat=fs.fstatSync, close=fs.closeSync, open=fs.openSync;let armed=true,reused;
try{
 if(mode==='ambiguous-close-reused-fd'){
  const path=${JSON.stringify(join(root,'replacement'))};fs.writeFileSync(path,'replacement');
  const original=new Error('close succeeded before reporting failure');
  fs.closeSync=(fd)=>{if(fd===3&&armed){armed=false;close(fd);reused=open(path,'r');assert.equal(reused,3);throw original;}return close(fd);};syncBuiltinESMExports();
  assert.throws(()=>owner.close(),e=>e===original);owner.close();assert.equal(fstat(reused).size,11);close(reused);reused=undefined;
 }else{
  fs.fstatSync=(fd,...args)=>{const result=fstat(fd,...args);if(fd===3&&armed){armed=false;
    if(mode==='close-during-recheck')owner.close();else assert.throws(()=>owner.recheck());
  }return result;};syncBuiltinESMExports();
  assert.throws(()=>owner.recheck());assert.throws(()=>owner.recheck());assert.throws(()=>owner.value);
 }
}finally{fs.fstatSync=fstat;fs.closeSync=close;syncBuiltinESMExports();owner.close();if(reused!==undefined)close(reused);}
console.log('terminal lifecycle refusal observed');`;
    const program=join(root,'controller.mjs');writeFileSync(program,controller);
    const fd=openSync(path,'r');
    try{
      const result=spawnSync(process.execPath,[program,hash(raw)],{stdio:['ignore','pipe','pipe',fd],cwd:root,
        env:{PATH:dirname(process.execPath),TMPDIR:root},timeout:5000,maxBuffer:64*1024});
      assert.equal(result.status,0,result.stderr.toString());assert.equal(result.stderr.length,0);
      assert.equal(result.stdout.toString(),'terminal lifecycle refusal observed\n');
    }finally{closeSync(fd);rmSync(root,{recursive:true,force:true});}
  });
