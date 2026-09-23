// Inert Node24 event mechanics only: never imports or executes an SDK/addon.
import assert from "node:assert/strict";
import test, { after } from "node:test";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { SorafsJavascriptTestEvents } from "../sorafs_javascript_test_events.mjs";

const contractBytes = readFileSync(new URL("../../javascript/iroha_js/test/fixtures/sorafs_native_suite_contract_v1.json", import.meta.url));
const contract = JSON.parse(contractBytes);
const root = fileURLToPath(new URL("../../", import.meta.url));
const temporary = path.join(root, "target");
mkdirSync(temporary, { recursive: true });
const directory = mkdtempSync(path.join(temporary, "sorafs-test-events-"));
after(() => rmSync(directory, { recursive: true, force: true }));
const originalNode = readFileSync(process.execPath);
const results = new Map();

function execute(mode) {
  if (results.has(mode)) return results.get(mode);
  assert.equal(Number(process.versions.node.split(".")[0]), 24, "event controls require actual selected Node24");
  const work = path.join(directory, mode);
  const suiteRoot = path.join(work, "suites");
  mkdirSync(suiteRoot, { recursive: true });
  writeFileSync(path.join(work, "package.json"), '{"type":"module"}\n');
  writeFileSync(path.join(work, "owner.mjs"), `
import {threadId} from "node:worker_threads";
export const owner={pid:process.pid,threadId,token:Symbol("held original"),prepared:false,setup:false,entered:false,finished:false};
export function enter(){if(!owner.prepared||!owner.setup||owner.entered||threadId!==owner.threadId)throw Error("lost original owner");owner.entered=true;return owner.token;}
`);
  const registrations = [];
  for (const suite of contract.suites) {
    const calls = [];
    for (const item of suite.cases) {
      const last = suite === contract.suites.at(-1) && item === suite.cases.at(-1);
      if (last && mode === "missing") continue;
      const options = last && ["skip", "todo"].includes(mode) ? `{${mode}:true},` : "";
      let body = "assert.equal(owner.token,token);assert.equal(owner.pid,process.pid);";
      if (item.name === "fixture-bundle wrapper matches all nine release-wide outcome goldens byte-for-byte") {
        body += suite.nested_case_names.map((name) => `await t.test(${JSON.stringify(name)},()=>assert.equal(owner.token,token));`).join("\n");
      }
      if (last && mode === "case-error") body += 'throw Error("case failed");';
      if (last && mode === "late-error") body += 'setTimeout(()=>{throw Error("late activity")},10);';
      calls.push(`test(${JSON.stringify(item.name)},${options}async t=>{${body}});`);
    }
    if (suite === contract.suites.at(-1) && mode === "extra") calls.push('test("unexpected extra case",()=>{});');
    writeFileSync(path.join(suiteRoot, suite.name + ".js"), `
import assert from "node:assert/strict";
import {test} from "node:test";
import {owner} from "../owner.mjs";
export function register(){const token=owner.token;${calls.join("\n")}}
`);
    registrations.push(`import {register as register${registrations.length}} from ${JSON.stringify("./suites/" + suite.name + ".js")};`);
  }
  writeFileSync(path.join(work, "entry.mjs"), `
import assert from "node:assert/strict";
import {after} from "node:test";
import {owner,enter} from "./owner.mjs";
${registrations.join("\n")}
const original=enter();
after(()=>{assert.equal(owner.token,original);owner.finished=true;${mode === "after-error" ? 'throw Error("final hook failed");' : ""}});
${mode === "registration-error" ? 'throw Error("registration failed");' : ""}
${registrations.map((_, index) => `register${index}();`).join("\n")}
`);
  writeFileSync(path.join(work, "runner.mjs"), `
import {run} from "node:test";
import {fileURLToPath} from "node:url";
import {owner} from "./owner.mjs";
owner.prepared=true;const token=owner.token;
const events=[];
for await(const event of run({files:[fileURLToPath(new URL("./entry.mjs",import.meta.url))],isolation:"none",concurrency:false,setup(){owner.setup=true;}})) events.push(event);
process.stdout.write(JSON.stringify({events,owner:{pid:owner.pid,threadId:owner.threadId,entered:owner.entered,finished:owner.finished,sameToken:owner.token===token},pid:process.pid})+"\\n");
`);
  const child = spawnSync(process.execPath, [path.join(work, "runner.mjs")], {
    cwd: work, env: { PATH: "/usr/bin:/bin", HOME: work, TMPDIR: work, LC_ALL: "C" },
    encoding: "utf8", timeout: 20_000, maxBuffer: 2 * 1024 * 1024,
  });
  assert.equal(child.error, undefined);
  assert.equal(child.signal, null);
  assert.equal(child.stderr, "");
  const observed = JSON.parse(child.stdout);
  assert.deepEqual(readFileSync(process.execPath), originalNode, "original runtime bytes changed");
  const result = { ...observed, status: child.status, suiteRoot };
  results.set(mode, result);
  return result;
}
function observer(result) {
  return new SorafsJavascriptTestEvents(contractBytes, { suiteRoot: result.suiteRoot });
}
function consume(events, result = execute("positive")) {
  const owner = observer(result);
  for (const event of events) owner.accept(event);
  return owner.finish();
}
function mutate(fn) {
  const result = execute("positive");
  const events = structuredClone(result.events);
  fn(events);
  const owner = observer(result);
  assert.throws(() => { for (const event of events) owner.accept(event); owner.finish(); });
  assert.throws(() => owner.accept(result.events[0]), /refused|consumed/u);
  assert.throws(() => owner.finish(), /refused|consumed/u);
}
function event(events, type, nesting = 0) {
  return events.find((item) => item.type === type && item.data.nesting === nesting);
}

test("actual Node24 preserves original same-process owner across all fixed case identities", () => {
  const result = execute("positive");
  assert.equal(result.status, 0);
  assert.deepEqual(result.owner, { pid: result.pid, threadId: 0, entered: true, finished: true, sameToken: true });
  const observed = consume(result.events, result);
  assert.equal(observed.cases.length, 55);
  assert.equal(observed.counts.topLevel, 46);
  assert.equal(observed.contract_sha256, createHash("sha256").update(contractBytes).digest("hex"));
  assert.deepEqual(observed.cases.filter((item) => item.path.length === 2).map((item) => item.path[1]), contract.suites[2].nested_case_names);
  const parent = observed.cases.find((item) => item.path[0] === "fixture-bundle wrapper matches all nine release-wide outcome goldens byte-for-byte");
  assert.ok(observed.cases.filter((item) => item.path.length === 2).every((item) => item.parentId === parent.testId));
  assert.ok(Object.isFrozen(observed) && Object.isFrozen(observed.cases) && observed.cases.every((item) => Object.isFrozen(item.path)));
  const nestedComplete = result.events.findIndex((item) => item.type === "test:complete" && item.data.nesting === 1);
  const nestedStart = result.events.findIndex((item) => item.type === "test:start" && item.data.nesting === 1);
  assert.ok(nestedComplete < nestedStart, "actual buffering remains accepted");
});

for (const mode of ["registration-error", "case-error", "after-error", "skip", "todo", "extra", "missing", "late-error"]) {
  test(`actual Node24 ${mode} cannot qualify the fixed case stream`, () => {
    const result = execute(mode);
    assert.throws(() => consume(result.events, result));
    if (mode === "after-error") {
      assert.equal(result.status, 0);
      assert.equal(result.events.find((item) => item.type === "test:summary").data.success, true);
      assert.ok(result.events.some((item) => item.type === "test:fail"));
    }
  });
}

function moveBefore(events, selected, before) {
  const index = events.findIndex(selected); assert.ok(index >= 0);
  const [moved] = events.splice(index, 1);
  const destination = events.findIndex(before); assert.ok(destination >= 0);
  events.splice(destination, 0, moved);
}
const mutations = {
  "root execution blocks swapped": (events) => {
    const roots = events.filter((e) => e.type === "test:enqueue" && e.data.nesting === 0);
    const a = roots[1].data.testId, b = roots[2].data.testId;
    const at = events.findIndex((e) => e.type === "test:dequeue" && e.data.testId === a);
    const later = events.findIndex((e) => e.type === "test:dequeue" && e.data.testId === b);
    assert.equal(later, at + 4);
    const block = events.splice(later, 4); events.splice(at, 0, ...block);
  },
  "root execution overlaps preceding case": (e) => {
    const roots = e.filter((x) => x.type === "test:enqueue" && x.data.nesting === 0);
    moveBefore(e, (x) => x.type === "test:dequeue" && x.data.testId === roots[1].data.testId,
      (x) => x.type === "test:complete" && x.data.testId === roots[0].data.testId);
  },
  "nested registration overlaps preceding child": (e) => {
    const nested = e.filter((x) => x.type === "test:enqueue" && x.data.nesting === 1);
    const from = e.findIndex((x) => x === nested[1]);
    const picked = e.splice(from, 2);
    assert.deepEqual(picked.map((x) => x.type), ["test:enqueue", "test:dequeue"]);
    e.splice(e.findIndex((x) => x.type === "test:complete" && x.data.testId === nested[0].data.testId), 0, ...picked);
  },
  "negative zero summary count": (e) => { e.at(-1).data.counts.failed = -0; },
  "unknown case": (e) => { event(e, "test:enqueue").data.name = "substituted"; },
  "unknown file": (e) => { event(e, "test:enqueue").data.file += "/../other.js"; },
  "wrong parent": (e) => { event(e, "test:enqueue", 1).data.parentId = 0; },
  "wrong nesting": (e) => { event(e, "test:enqueue", 1).data.nesting = 0; },
  "foreign id": (e) => { event(e, "test:complete").data.testId += 1000; },
  "id reuse": (e) => { e.filter((x) => x.type === "test:enqueue" && x.data.nesting === 0)[1].data.testId = 1; },
  "fractional id": (e) => { event(e, "test:enqueue").data.testId = 1.5; },
  "negative zero id": (e) => { event(e, "test:enqueue").data.parentId = -0; },
  "coordinate mutation": (e) => { event(e, "test:pass").data.column += 1; },
  "ordinal mutation": (e) => { event(e, "test:pass").data.testNumber += 1; },
  "parent label": (e) => { event(e, "test:pass", 1).data.classname = "other"; },
  "duplicate enqueue": (e) => { e.splice(1, 0, structuredClone(e[0])); },
  "duplicate pass": (e) => { const i = e.findIndex((x) => x.type === "test:pass"); e.splice(i + 1, 0, structuredClone(e[i])); },
  "missing complete": (e) => { e.splice(e.findIndex((x) => x.type === "test:complete"), 1); },
  "missing pass": (e) => { e.splice(e.findIndex((x) => x.type === "test:pass"), 1); },
  "missing start": (e) => { e.splice(e.findIndex((x) => x.type === "test:start"), 1); },
  "missing dequeue": (e) => { e.splice(e.findIndex((x) => x.type === "test:dequeue"), 1); },
  "failed complete": (e) => { event(e, "test:complete").data.details.passed = false; },
  "skipped pass": (e) => { event(e, "test:pass").data.skip = true; },
  "todo pass": (e) => { event(e, "test:pass").data.todo = true; },
  "tagged case": (e) => { event(e, "test:enqueue").data.tags.push("only"); },
  "suite instead of test": (e) => { event(e, "test:dequeue").data.type = "suite"; },
  "paired negative zero case duration": (e) => {
    const completed = event(e, "test:complete"); completed.data.details.duration_ms = -0;
    e.find((x) => x.type === "test:pass" && x.data.testId === completed.data.testId).data.details.duration_ms = -0;
  },
  "negative zero summary duration": (e) => {
    e.at(-1).data.duration_ms = -0;
    e.find((x) => x.type === "test:diagnostic" && x.data.message.startsWith("duration_ms ")).data.message = "duration_ms 0";
  },
  "nonfinite duration": (e) => { event(e, "test:complete").data.details.duration_ms = Infinity; },
  "duration substitution": (e) => { event(e, "test:pass").data.details.duration_ms += 1; },
  "nested plan count": (e) => { event(e, "test:plan", 1).data.count = 8; },
  "nested plan location": (e) => { event(e, "test:plan", 1).data.line += 1; },
  "root plan count": (e) => { event(e, "test:plan").data.count = 55; },
  "missing plan": (e) => { e.splice(e.findIndex((x) => x.type === "test:plan"), 1); },
  "extra diagnostic": (e) => { event(e, "test:diagnostic").data.message = "Error: asynchronous activity"; },
  "successful summary hiding failed counts": (e) => { e.at(-1).data.counts.failed = 1; },
  "wrong summary type": (e) => { e.at(-1).data.counts.tests = "55"; },
  "wrong summary success": (e) => { e.at(-1).data.success = false; },
  "missing summary": (e) => { e.pop(); },
  "duplicate summary": (e) => { e.push(structuredClone(e.at(-1))); },
  "event after summary": (e) => { e.push(structuredClone(e[0])); },
  "oversized text": (e) => { e[0].data.name = "a".repeat(4097); },
  "unexpected event fields": (e) => { e[0].data.foreign = true; },
  "deep event object": (e) => { e[0].data.extra = { a: { b: { c: { d: { e: 1 } } } } }; },
  "oversized array": (e) => { e[0].data.tags = Array(1000000); },
  "symbol event key": (e) => { e[0].data[Symbol("unowned")] = 1; },
  "excess event fields": (e) => { for (let n = 0; n < 25; n += 1) e[0].data["field" + n] = 1; },
  "unknown event": (e) => { e[0].type = "test:coverage"; },
  "accessor event": (e) => { Object.defineProperty(e[0].data, "name", { get() { throw Error("must not execute"); } }); },
};
for (const [name, change] of Object.entries(mutations)) test(`malformed ${name} poisons the sole owner`, () => mutate(change));

test("foreign contract bytes and noncanonical suite roots cannot select cases", () => {
  const result = execute("positive");
  const changed = Buffer.from(contractBytes); changed[0] ^= 1;
  for (const raw of [changed, Buffer.alloc(0), Buffer.alloc(128 * 1024 + 1), contractBytes.toString()]) {
    assert.throws(() => new SorafsJavascriptTestEvents(raw, { suiteRoot: result.suiteRoot }));
  }
  for (const suiteRoot of ["relative", result.suiteRoot + "/../suites", result.suiteRoot + "\0", true]) {
    assert.throws(() => new SorafsJavascriptTestEvents(contractBytes, { suiteRoot }));
  }
});

test("caller mutations cannot alter retained complete observations or contract", () => {
  const result = execute("positive"); const bytes = Buffer.from(contractBytes);
  const owner = new SorafsJavascriptTestEvents(bytes, { suiteRoot: result.suiteRoot }); bytes.fill(0);
  for (const original of result.events) {
    const copy = structuredClone(original); owner.accept(copy);
    if (copy.type === "test:complete") copy.data.details.duration_ms = -1;
  }
  assert.equal(owner.finish().cases.length, 55);
  assert.throws(() => owner.finish(), /consumed/u);
  assert.throws(() => owner.accept(result.events[0]), /consumed/u);
});


test("aggregate text budget refuses bounded individual paths and retains exact threshold", () => {
  const result = execute("positive");
  function attempt(length) {
    const suiteRoot = "/" + "a".repeat(length);
    const owner = new SorafsJavascriptTestEvents(contractBytes, { suiteRoot });
    for (const original of result.events) {
      const copy = structuredClone(original);
      if (copy.data.file) copy.data.file = path.join(suiteRoot, path.basename(copy.data.file));
      owner.accept(copy);
    }
    return owner.finish();
  }
  assert.equal(attempt(1024).cases.length, 55);
  assert.throws(() => attempt(3800), /aggregate event text bound/u);
  let low = 1024, high = 3800;
  while (high - low > 1) {
    const middle = Math.floor((high + low) / 2);
    try { attempt(middle); low = middle; } catch (error) {
      assert.match(error.message, /aggregate event text bound/u); high = middle;
    }
  }
  assert.equal(attempt(low).cases.length, 55);
  assert.throws(() => attempt(low + 1), /aggregate event text bound/u);
});

test("reentrant refusal cannot be swallowed to reuse the original owner", () => {
  const result = execute("positive"); const owner = observer(result);
  let first = true;
  const substituted = new Proxy(result.events[0], {
    getOwnPropertyDescriptor(target, key) {
      if (first) {
        first = false;
        assert.throws(() => owner.accept(result.events[0]), /refused/u);
      }
      return Object.getOwnPropertyDescriptor(target, key);
    },
  });
  assert.throws(() => owner.accept(substituted), /refused/u);
  assert.throws(() => owner.finish(), /refused/u);
});


test("zero and fractional measured durations remain valid without integer coercion", () => {
  const result = execute("positive");
  for (const measured of [0, 0.125]) {
    const events = structuredClone(result.events);
    const complete = event(events, "test:complete"); complete.data.details.duration_ms = measured;
    events.find((item) => item.type === "test:pass" && item.data.testId === complete.data.testId).data.details.duration_ms = measured;
    assert.equal(consume(events, result).cases.find((item) => item.testId === complete.data.testId).duration_ms, measured);
  }
});
