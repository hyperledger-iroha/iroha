import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createPrivateSettlementHost } from "../src/privateSettlement.js";
import { defaultNativeRuntime, resolveNativeRuntimeBinding } from "../src/nativeRuntime.js";

// These tests require the authenticated installed addon. Missing or stale artifacts fail;
// no injected binding or fabricated settlement result can satisfy this file.
const limits = {
  maximumSessions: 1, maximumOriginalBytes: 1024,
  maximumArtifactBytes: 1024, maximumPreparedBytes: 1024,
};
function native() { return resolveNativeRuntimeBinding(defaultNativeRuntime); }
function createHost() {
  const binding = native();
  return binding.privateSettlementCreateHost(1, binding.privateSettlementDescriptorMaxBytes, 1024, 1024);
}

test("installed native descriptor admission rejects shared, detached, empty and oversized backing", async () => {
  const host = await createHost();
  const empty = mkdtempSync(join(tmpdir(), "iroha-native-private-source-"));
  try {
    const shared = new Uint8Array(new SharedArrayBuffer(8));
    Object.defineProperty(shared, "buffer", { value: new ArrayBuffer(8) });
    const detachedBuffer = new ArrayBuffer(8);
    const detached = new Uint8Array(detachedBuffer);
    structuredClone(detachedBuffer, { transfer: [detachedBuffer] });
    for (const descriptor of [
      shared, detached, new Uint8Array(), new Uint16Array(2),
      new Uint8Array(native().privateSettlementDescriptorMaxBytes + 1),
    ]) {
      assert.throws(() => host.openFiles(descriptor));
    }
    // Exact offset must reach the real missing source read. Losing the offset instead
    // produces BindingMismatch at JSON decoding, so the two refusals distinguish the path.
    const descriptor = Buffer.from(JSON.stringify({
      layout: "iroha.private-settlement.files.v1",
      client_file: join(empty, "client.toml"), request_file: join(empty, "request.json"),
      pins_file: join(empty, "pins.json"), genesis_file: join(empty, "genesis.norito"),
      custody_key_file: join(empty, "custody.key"), custody_root: join(empty, "custody"),
      maximum_wallet_bytes: 1,
    }));
    const original = Buffer.concat([Buffer.from("x"), descriptor, Buffer.from("x")]).subarray(1, 1 + descriptor.length);
    const ticket = host.openFiles(original);
    await ticket.wait();
    const outcome = ticket.take();
    assert.equal(outcome.kind(), "failed");
    assert.equal(outcome.progress(), null);
    assert.equal(outcome.failureCode(), "LocalResourceUnavailable");
    await ticket.close();
  } finally {
    await host.shutdown();
    rmSync(empty, { recursive: true, force: true });
  }
});

test("installed native command IDs reject truncation and eager close refuses work", async () => {
  const host = await createHost();
  try {
    const ticket = host.openFiles(Buffer.from("{}"));
    await ticket.wait();
    ticket.take();
    const defaults = native().privateSettlementDefaultRunOptions();
    const args = [defaults.maxFinalityProofReads, defaults.observationTimeoutMs,
      defaults.pollIntervalMs, defaults.retransmitIdenticalBytes, defaults.retryRejectedStep];
    for (const command of [2.5, 2 ** 32 + 2, -1, Infinity, NaN]) {
      assert.throws(() => ticket.start(command, ...args), /input invalid/u);
    }
    assert.throws(() => ticket.start("x".repeat(1024 * 1024), ...args));
    assert.throws(() => ticket.start(2, ...args), /original is not open/u);
    const closing = ticket.close();
    // Cleanup may already have removed the closed session; either way no command is admitted.
    assert.throws(() => ticket.start(2, ...args), /owner is closing|handle is unavailable/u);
    await closing;
  } finally { await host.shutdown(); }
});

test("installed host drains accepted result export and eagerly refuses subsequent exports", async () => {
  const host = await createHost();
  try {
    const ticket = host.openFiles(Buffer.from("{}"));
    await ticket.wait();
    const outcome = ticket.take();
    const exported = outcome.data();
    const shutdown = host.shutdown();
    assert.throws(() => host.openFiles(Buffer.from("{}")), /closing/iu);
    assert.throws(() => outcome.data(), /closing/iu);
    const bytes = await exported;
    await shutdown;
    assert.equal(bytes.byteLength, 0);
    assert.equal(outcome.kind(), "failed");
  } finally { await host.shutdown(); }
});

test("installed public wrapper preserves the native failed outcome without payment commands", async () => {
  const host = await createPrivateSettlementHost(limits);
  try {
    const operation = host.open(Buffer.from("{}"));
    try {
      const original = await operation.ready();
      assert.equal(original.kind, "failed");
      assert.strictEqual(await operation.recover({ maxFinalityProofReads: 0 }), original);
      assert.equal(original.progress, null);
      assert.equal((await original.data()).byteLength, 0);
    } finally { await operation.close(); }
  } finally { await host.shutdown(); }
});

test("installed opaque native classes refuse construction and prototype-forged receivers", async () => {
  const binding = native();
  for (const name of [
    "PrivateSettlementNativeHost", "PrivateSettlementNativeTicket", "PrivateSettlementNativeOutcome",
  ]) {
    assert.equal(typeof binding[name], "function", `${name} must be provided by the installed addon`);
    assert.throws(() => Reflect.construct(binding[name], []));
    assert.throws(() => Reflect.construct(binding[name], [{ kind: "settled", verified: true }]));
  }
  const host = await createHost();
  try {
    const forgedHost = Object.create(Object.getPrototypeOf(host));
    assert.throws(() => Reflect.apply(host.openFiles, forgedHost, [Buffer.from("{}")]));
    const ticket = host.openFiles(Buffer.from("{}"));
    await ticket.wait();
    const forgedTicket = Object.create(Object.getPrototypeOf(ticket));
    assert.throws(() => Reflect.apply(ticket.status, forgedTicket, []));
    assert.throws(() => Reflect.apply(ticket.take, forgedTicket, []));
    const outcome = ticket.take();
    // The locked napi runtime records same-addon class identity before pointer casting;
    // these are wrong native classes, not merely objects without any native backing.
    for (const receiver of [ticket, outcome]) {
      assert.throws(() => Reflect.apply(host.openFiles, receiver, [Buffer.from("{}")]));
      assert.throws(() => Reflect.apply(host.shutdown, receiver, []));
    }
    for (const receiver of [host, outcome]) {
      assert.throws(() => Reflect.apply(ticket.status, receiver, []));
      assert.throws(() => Reflect.apply(ticket.take, receiver, []));
      assert.throws(() => Reflect.apply(ticket.stageFiles, receiver, [Buffer.from("{}")]));
      assert.throws(() => Reflect.apply(ticket.close, receiver, []));
    }
    const dataOnly = JSON.parse('{"kind":"settled","verified":true,"receipt":{}}');
    const forgedOutcome = Object.assign(Object.create(Object.getPrototypeOf(outcome)), dataOnly);
    for (const receiver of [{}, dataOnly, forgedOutcome, host, ticket]) {
      for (const method of [outcome.kind, outcome.failureCode, outcome.progress, outcome.data]) {
        assert.throws(() => Reflect.apply(method, receiver, []));
      }
    }
    // Refusals neither consume the real result nor promote the failed original to authority.
    assert.equal(outcome.kind(), "failed");
    assert.equal(outcome.progress(), null);
    assert.equal((await outcome.data()).byteLength, 0);
    await ticket.close();
  } finally { await host.shutdown(); }
});

test("installed native take is once-only and closing returns capacity without reviving stale tickets", async () => {
  const host = await createHost();
  try {
    const first = host.openFiles(Buffer.from("{}"));
    assert.throws(() => host.openFiles(Buffer.from("{}")), /capacity unavailable/u);
    assert.equal(await first.wait(), "ready");
    assert.throws(() => host.openFiles(Buffer.from("{}")), /capacity unavailable/u);
    const original = first.take();
    assert.equal(original.kind(), "failed");
    assert.equal(first.status(), "idle");
    assert.throws(() => first.take(), /result is unavailable/u);
    // Taking transfers the result, but the original session still owns its admission.
    assert.throws(() => host.openFiles(Buffer.from("{}")), /capacity unavailable/u);
    await first.close();

    const replacement = host.openFiles(Buffer.from("{}"));
    assert.throws(() => first.status(), /handle is unavailable/u);
    assert.throws(() => first.take(), /handle is unavailable/u);
    await assert.rejects(first.wait(), /handle is unavailable/u);
    assert.throws(() => first.stageFiles(Buffer.from("{}")), /handle is unavailable/u);
    assert.equal(await replacement.wait(), "ready");
    const second = replacement.take();
    assert.equal(second.kind(), "failed");
    assert.notStrictEqual(second, original);
    assert.throws(() => replacement.take(), /result is unavailable/u);
    // A taken result remains caller-owned even after its original session has closed.
    assert.equal((await original.data()).byteLength, 0);
    await replacement.close();
  } finally { await host.shutdown(); }
});

test("installed raw host limits reject every non-integral or out-of-range number before ownership", async () => {
  const binding = native();
  const original = [1, binding.privateSettlementDescriptorMaxBytes, 1024, 1024];
  for (let index = 0; index < original.length; index += 1) {
    const maximum = index === 0 ? 65535 : 0xffffffff;
    for (const invalid of [0, -1, 1.5, maximum + 1, 2 ** 32 + 1, NaN, Infinity, -Infinity]) {
      const args = [...original];
      args[index] = invalid;
      let unexpected;
      try {
        await assert.rejects(async () => {
          unexpected = await binding.privateSettlementCreateHost(...args);
        }, /input invalid/u);
      } finally {
        // Preserve cleanup if a regression incorrectly constructed an owner.
        if (unexpected) await unexpected.shutdown();
      }
    }
  }
});

test("installed raw observation controls validate exact numbers before session dispatch", async () => {
  const host = await createHost();
  try {
    const ticket = host.openFiles(Buffer.from("{}"));
    await ticket.wait();
    const original = ticket.take();
    const defaults = native().privateSettlementDefaultRunOptions();
    const selected = [defaults.maxFinalityProofReads, defaults.observationTimeoutMs,
      defaults.pollIntervalMs, false, false];
    for (let index = 0; index < 3; index += 1) {
      const maximum = index === 0 ? 65535 : 0xffffffff;
      const invalidValues = [-1, 1.5, maximum + 1, 2 ** 32 + 1, NaN, Infinity, -Infinity];
      if (index === 2) invalidValues.push(0);
      for (const invalid of invalidValues) {
        const args = [...selected];
        args[index] = invalid;
        assert.throws(() => ticket.start(3, ...args), /input invalid/u);
        assert.equal(ticket.status(), "idle");
      }
    }
    // Exact zero-read/timeout controls and each original upper bound reach the ordinary
    // NotOpen gate. Malformed numeric controls above must never reach that gate.
    assert.throws(() => ticket.start(3, 0, 0, 1, false, false), /original is not open/u);
    assert.throws(() => ticket.start(3, 65535, 0xffffffff, 0xffffffff, false, false), /original is not open/u);
    assert.equal(original.kind(), "failed");
    assert.equal(ticket.status(), "idle");
    await ticket.close();
  } finally { await host.shutdown(); }
});
