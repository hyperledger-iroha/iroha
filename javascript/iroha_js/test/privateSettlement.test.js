import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import * as production from "../src/privateSettlement.js";

// Source adapter contracts, not installed verification. Only this private test module
// replaces the loader import; production exposes no injected runtime or capability issuer.
const source = await readFile(new URL("../src/privateSettlement.js", import.meta.url), "utf8");
const registry = globalThis[Symbol.for("iroha.private-settlement.contract-tests")] = new Map();
let serial = 0;
async function adapter(binding) {
  const id = ++serial;
  registry.set(id, binding);
  const loader = `export const defaultNativeRuntime = Object.freeze({});
const binding = globalThis[Symbol.for("iroha.private-settlement.contract-tests")].get(${id});
export function resolveNativeRuntimeBinding(runtime) {
  if (runtime !== defaultNativeRuntime) throw new Error("injected authority");
  return binding;
}`;
  const url = `data:text/javascript;base64,${Buffer.from(loader).toString("base64")}`;
  assert.equal(source.split('"./nativeRuntime.js"').length, 2);
  try {
    return await import(`data:text/javascript;base64,${Buffer.from(source.replace('"./nativeRuntime.js"', JSON.stringify(url))).toString("base64")}`);
  } finally { registry.delete(id); }
}
const limits = Object.freeze({ maximumSessions: 2, maximumOriginalBytes: 2000, maximumArtifactBytes: 4000, maximumPreparedBytes: 8000 });
const defaults = Object.freeze({ maxFinalityProofReads: 17, observationTimeoutMs: 701, pollIntervalMs: 23, retransmitIdenticalBytes: false, retryRejectedStep: false });
function gate() {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
}
async function fixture({ opening = Promise.resolve(), openingKind = "opened", progress = null } = {}) {
  const calls = [];
  const cleanup = gate();
  const nativeData = new Uint8Array([1, 7]);
  function ticket(kind, completion = Promise.resolve()) {
    return {
      status() { return "ready"; },
      wait() { calls.push(["wait", kind]); return completion.then(() => "ready"); },
      take() {
        calls.push(["take", kind]);
        return {
          kind: () => kind, failureCode: () => kind === "failed" ? "authorization" : null,
          progress: () => progress,
          data() { calls.push(["data", kind]); return Promise.resolve(nativeData); },
        };
      },
    };
  }
  const anchor = {
    ...ticket(openingKind, opening),
    start(command, ...options) { calls.push(["start", command, ...options]); return ticket("pending"); },
    stageFiles(bytes) { calls.push(["stageFiles", bytes]); return ticket("staged"); },
    close() { calls.push(["close"]); return cleanup.promise; },
  };
  const api = await adapter({
    privateSettlementDescriptorMaxBytes: 16,
    privateSettlementDefaultRunOptions: () => defaults,
    async privateSettlementCreateHost(...args) {
      calls.push(["create", ...args]);
      return {
        openFiles(bytes) { calls.push(["openFiles", bytes]); return anchor; },
        shutdown() { calls.push(["shutdown"]); return cleanup.promise; },
      };
    },
  });
  return { calls, cleanup, nativeData, api, create: () => api.createPrivateSettlementHost(limits) };
}

test("capabilities cannot be constructed or forged from data", () => {
  for (const name of ["PrivateSettlementHost", "PrivateSettlementOperation", "PrivateSettlementTicket", "PrivateSettlementOutcome"]) {
    assert.throws(() => new production[name](), /native-issued/u);
  }
  const forged = Object.create(production.PrivateSettlementOutcome.prototype);
  assert.throws(() => forged.kind, TypeError);
  assert.throws(() => forged.data(), TypeError);
});

test("host forwards finite limits and exact descriptor at synchronous admission", async () => {
  const f = await fixture();
  const host = await f.create();
  assert.deepEqual(f.calls, [["create", 2, 2000, 4000, 8000]]);
  const bytes = new Uint8Array(16);
  const operation = host.open(bytes);
  assert.strictEqual(f.calls[1][1], bytes);
  assert.ok(Object.isFrozen(host));
  assert.ok(Object.isFrozen(operation));
  assert.equal((await operation.ready()).kind, "opened");
  assert.throws(() => host.open(new Uint8Array(17)), /native byte limit/u);
  assert.throws(() => host.open(new Uint8Array()), /native byte limit/u);
  assert.throws(() => host.open("private path"), /unshared Uint8Array/u);
  assert.throws(() => host.open(new Uint8Array(new SharedArrayBuffer(16))), /unshared Uint8Array/u);
  assert.throws(() => host.open(new Uint8Array(new ArrayBuffer(8, { maxByteLength: 16 }))), /native byte limit/u);
  assert.equal(f.calls.filter(([name]) => name === "openFiles").length, 1);
});

test("host rejects absent, unknown, accessor and out-of-range limits before native creation", async () => {
  const f = await fixture();
  for (const bad of [{}, { ...limits, maximumSessions: 0 }, { ...limits, maximumSessions: 65536 }, { ...limits, maximumOriginalBytes: Infinity }, { ...limits, extra: 1 }]) {
    await assert.rejects(f.api.createPrivateSettlementHost(bad));
  }
  const accessor = Object.defineProperty({ ...limits }, "maximumOriginalBytes", { get() { assert.fail("accessor executed"); } });
  await assert.rejects(f.api.createPrivateSettlementHost(accessor), /unsupported field/u);
  assert.deepEqual(f.calls, []);
});

test("native defaults and zero-read recovery pass unchanged without provisioning", async () => {
  const f = await fixture();
  const operation = (await f.create()).open(new Uint8Array([1]));
  assert.equal((await operation.recover({ maxFinalityProofReads: 0 })).kind, "pending");
  assert.deepEqual(f.calls.find(([name, command]) => name === "start" && command === 3), ["start", 3, 0, 701, 23, false, false]);
  await operation.run();
  assert.deepEqual(f.calls.find(([name, command]) => name === "start" && command === 2), ["start", 2, 17, 701, 23, false, false]);
  await operation.prepare({ retransmitIdenticalBytes: true, retryRejectedStep: true });
  assert.deepEqual(f.calls.find(([name, command]) => name === "start" && command === 0), ["start", 0, 17, 701, 23, true, true]);
  await operation.prepared({ maxFinalityProofReads: 0 });
  assert.equal(f.calls.some(([name]) => name === "stageFiles" || name === "close"), false);
});

test("intrinsic descriptor checks reject shadowed backing, length and resizability", async () => {
  const f = await fixture();
  const host = await f.create();
  const shared = new Uint8Array(new SharedArrayBuffer(8));
  Object.defineProperty(shared, "buffer", { value: new ArrayBuffer(8) });
  assert.throws(() => host.open(shared), /unshared/u);
  const oversized = new Uint8Array(17);
  Object.defineProperty(oversized, "byteLength", { value: 1 });
  assert.throws(() => host.open(oversized), /byte limit/u);
  const resizable = new ArrayBuffer(8, { maxByteLength: 16 });
  Object.defineProperty(resizable, "resizable", { value: false });
  assert.throws(() => host.open(new Uint8Array(resizable)), /byte limit/u);
  const wrong = new Uint16Array(4);
  Object.setPrototypeOf(wrong, Uint8Array.prototype);
  assert.throws(() => host.open(wrong), /unshared/u);
  assert.equal(f.calls.length, 1);
});

test("invalid options and commands refuse before entering native work", async () => {
  const f = await fixture();
  const operation = (await f.create()).open(new Uint8Array([1]));
  for (const bad of [{ maxFinalityProofReads: -1 }, { maxFinalityProofReads: 65536 }, { pollIntervalMs: 0 }, { retryRejectedStep: 1 }, { witness: "secret" }]) {
    await assert.rejects(operation.run(bad));
  }
  await assert.rejects(operation.start("refund"), /unsupported/u);
  const accessor = Object.defineProperty({}, "pollIntervalMs", { get() { assert.fail("accessor executed"); } });
  await assert.rejects(operation.run(accessor), /unsupported field/u);
  assert.deepEqual(f.calls.map(([name]) => name), ["create", "openFiles"]);
});

test("explicit stage admits synchronously and never runs a payment", async () => {
  const f = await fixture();
  const operation = (await f.create()).open(new Uint8Array([1]));
  await operation.ready();
  const bytes = new Uint8Array(16);
  const ticket = operation.stageFiles(bytes);
  assert.strictEqual(f.calls.at(-1)[1], bytes);
  assert.equal((await ticket.result()).kind, "staged");
  assert.throws(() => operation.stageFiles(new Uint8Array(17)), /native byte limit/u);
  assert.equal(f.calls.some(([name]) => name === "start"), false);
});

test("result waiters take one native outcome and export only its original data", async () => {
  const f = await fixture();
  const operation = (await f.create()).open(new Uint8Array([1]));
  const ticket = await operation.start("run");
  assert.equal(ticket.status, "ready");
  const first = ticket.result();
  assert.strictEqual(first, ticket.result());
  const result = await first;
  assert.ok(Object.isFrozen(result));
  assert.equal(result.failureCode, null);
  assert.equal(result.progress, null);
  assert.strictEqual(await result.data(), f.nativeData);
  assert.equal(f.calls.filter(([name, kind]) => name === "take" && kind === "pending").length, 1);
});

test("opening failure stays opaque without starting or retrying monetary work", async () => {
  const f = await fixture({ openingKind: "failed" });
  const operation = (await f.create()).open(new Uint8Array([1]));
  const original = await operation.ready();
  assert.strictEqual(await operation.run(), original);
  assert.strictEqual(await operation.recover(), original);
  assert.equal(original.failureCode, "authorization");
  assert.equal(f.calls.some(([name]) => name === "start"), false);
});

test("close refuses immediately while retaining original pending cleanup", async () => {
  const opening = gate();
  const f = await fixture({ opening: opening.promise });
  const operation = (await f.create()).open(new Uint8Array([1]));
  const waiting = operation.run();
  const closed = operation.close();
  assert.deepEqual(f.calls.at(-1), ["close"]);
  assert.strictEqual(operation.close(), closed);
  await assert.rejects(operation.recover(), /closing/u);
  assert.throws(() => operation.stageFiles(new Uint8Array([1])), /closing/u);
  opening.resolve();
  await assert.rejects(waiting, /closing/u);
  assert.equal(f.calls.some(([name]) => name === "start"), false);
  let finished = false;
  closed.then(() => { finished = true; });
  await Promise.resolve();
  assert.equal(finished, false);
  f.cleanup.resolve();
  await closed;
});

test("shutdown immediately refuses open and awaits original native cleanup", async () => {
  const f = await fixture();
  const host = await f.create();
  const closed = host.shutdown();
  assert.deepEqual(f.calls.at(-1), ["shutdown"]);
  assert.strictEqual(host.shutdown(), closed);
  assert.throws(() => host.open(new Uint8Array([1])), /closing/u);
  f.cleanup.resolve();
  await closed;
});

test("missing native support fails without a JavaScript workflow fallback", async () => {
  const api = await adapter({});
  await assert.rejects(api.createPrivateSettlementHost(limits), /unavailable/u);
});

test("factory ignores an injected provider and resolves only its fixed production runtime", async () => {
  const f = await fixture();
  const injected = Object.defineProperty({}, "nativeRuntime", { get() { assert.fail("injected provider accessed"); } });
  const host = await f.api.createPrivateSettlementHost(limits, injected);
  assert.equal((await host.open(new Uint8Array([1])).ready()).kind, "opened");
  assert.deepEqual(f.calls[0], ["create", 2, 2000, 4000, 8000]);
});

test("redacted progress keeps exact sequence and freezes only returned observation data", async () => {
  const progress = {
    workflowId: "07".repeat(32), status: "in_progress", sequence: "18446744073709551615",
    cancelRequested: false, currentStep: 2, failureCode: null,
    confirmedSteps: [0, 1], unresolvedSteps: [2], failedSteps: [],
  };
  const f = await fixture({ progress });
  const outcome = await (await f.create()).open(new Uint8Array([1])).run();
  assert.strictEqual(outcome.progress, progress);
  assert.equal(outcome.progress.sequence, "18446744073709551615");
  assert.ok(Object.isFrozen(outcome.progress));
  assert.throws(() => outcome.progress.confirmedSteps.push(2), TypeError);
  assert.equal(outcome.kind, "pending");
});
