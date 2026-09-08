import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { buildKaigiAuthorizationProofV1, buildKaigiUsageProofV1 } from "@iroha/iroha-js/crypto";
import { NetworkId } from "@iroha/iroha-js";
import { makeNativeTest, nativeBinding } from "./helpers/native.js";

const nativeTest = makeNativeTest(test, {
  require: ["buildKaigiAuthorizationProofV1", "buildKaigiUsageProofV1"],
});
const fixture = JSON.parse(readFileSync(new URL(
  "./fixtures/kaigi_authorization_scalar_wire_v1.json", import.meta.url,
), "utf8"));
const host = fixture[0].instruction.Kaigi.CreateKaigi.call.host;
const network = NetworkId.fromBytes(Buffer.alloc(32, 0x13));
const preRosterRoot = Buffer.alloc(32, 0x35);

function nativeArguments(blinding) {
  return [network.toBytes(), "wonderland.sora", "native-authorization", host,
    host, 0n, "hostCreate", preRosterRoot, blinding];
}

nativeTest("packaged Kaigi authorization rejects invalid context and wipes its native input view", () => {
  for (const [index, value] of [
    [0, new Uint8Array(31)], [1, " wonderland.sora"], [3, "invalid-account"],
    [4, "invalid-account"], [5, -1n], [5, 1n << 64n], [5, 1n],
    [6, "join"], [6, "host-create"], [7, new Uint8Array(31)],
  ]) {
    const owner = Buffer.alloc(40, 0x11);
    const blinding = owner.subarray(4, 36);
    const args = nativeArguments(blinding);
    args[index] = value;
    assert.throws(() => nativeBinding.buildKaigiAuthorizationProofV1(...args));
    assert.deepEqual(blinding, Buffer.alloc(32));
    assert.deepEqual(owner.subarray(0, 4), Buffer.alloc(4, 0x11));
    assert.deepEqual(owner.subarray(36), Buffer.alloc(4, 0x11));
  }
  for (const blinding of [Buffer.alloc(32), Buffer.alloc(32, 0xff),
    Buffer.alloc(31, 1), Buffer.alloc(33, 1)]) {
    assert.throws(() => nativeBinding.buildKaigiAuthorizationProofV1(...nativeArguments(blinding)));
    assert.ok(blinding.every((byte) => byte === 0));
  }
  const overlapping = Buffer.alloc(32, 0x11);
  const args = nativeArguments(overlapping);
  args[0] = overlapping;
  args[1] = "invalid";
  // The original marked network remains valid after the shared view is wiped;
  // parsing must reach the invalid domain, not observe a cleared network.
  assert.throws(() => nativeBinding.buildKaigiAuthorizationProofV1(...args), /domain/u);
  assert.deepEqual(overlapping, Buffer.alloc(32));
});

nativeTest("packaged Kaigi usage rejects invalid metrics and host commitment and wipes its native input view", () => {
  for (const [index, value] of [
    [0, new Uint8Array(31)], [1, " wonderland.sora"], [3, "invalid-account"],
    [4, new Uint8Array(31)], [5, -1], [5, 0.5], [5, NaN], [5, Infinity], [5, 0x1_0000_0000],
    [6, -1n], [6, 0n], [6, 1n << 64n], [7, -1n], [7, 1n << 64n],
    [8, Buffer.alloc(31)], [8, Buffer.alloc(32, 0xff)],
  ]) {
    const owner = Buffer.alloc(40, 0x11);
    const blinding = owner.subarray(4, 36);
    const args = [network.toBytes(), "wonderland.sora", "native-authorization", host,
      preRosterRoot, 0, 1n, 0n, Buffer.alloc(32), blinding];
    args[index] = value;
    assert.throws(() => nativeBinding.buildKaigiUsageProofV1(...args));
    assert.deepEqual(blinding, Buffer.alloc(32));
    assert.deepEqual(owner.subarray(0, 4), Buffer.alloc(4, 0x11));
    assert.deepEqual(owner.subarray(36), Buffer.alloc(4, 0x11));
  }
  const overlapping = Buffer.alloc(32, 0x11);
  assert.throws(() => nativeBinding.buildKaigiUsageProofV1(overlapping, "invalid", "native-authorization",
    host, overlapping, 0, 1n, 0n, overlapping, overlapping), /domain/u);
  assert.deepEqual(overlapping, Buffer.alloc(32));
});

nativeTest("packaged Kaigi authorization and usage construct Core-verified final proofs and consume blinding", {
  timeout: 20 * 60 * 1000,
}, () => {
  const owner = Buffer.alloc(40, 0x11);
  const blinding = owner.subarray(4, 36);
  // The native implementation returns only after the canonical Core backend
  // verifies the proof against the final circuit's freshly derived key.
  const result = buildKaigiAuthorizationProofV1({
    networkId: network,
    callId: { domainId: "wonderland.sora", callName: "native-authorization" },
    hostId: host,
    subjectId: host,
    participationSequence: 0n,
    action: "hostCreate",
    preRosterRoot,
    blinding,
  });
  assert.deepEqual(blinding, Buffer.alloc(32));
  assert.deepEqual(owner.subarray(0, 4), Buffer.alloc(4, 0x11));
  assert.deepEqual(owner.subarray(36), Buffer.alloc(4, 0x11));
  assert.ok(Object.isFrozen(result));
  assert.deepEqual(result.preRosterRoot, preRosterRoot);
  assert.equal(result.proof.subarray(0, 4).toString("ascii"), "NRT0");
  assert.ok(result.proof.length > 31 * 32);
  for (const field of ["commitment", "nullifier", "authorization"]) {
    assert.ok(Buffer.isBuffer(result[field]));
    assert.equal(result[field].length, 32);
    assert.ok(result[field].some((byte) => byte !== 0));
  }
  assert.equal("buildKaigiRosterJoinProof" in nativeBinding, false);

  const usageOptions = {
    networkId: network, callId: {domainId: "wonderland.sora", callName: "native-authorization"},
    hostId: host, preRosterRoot: Buffer.alloc(32, 0x37), segmentIndex: 0xffff_ffff,
    durationMs: 0xffff_ffff_ffff_ffffn, billedGas: 0xffff_ffff_ffff_ffffn,
    hostCommitment: result.commitment,
  };
  for (const changed of [
    {hostCommitment: Buffer.alloc(32)}, {blinding: Buffer.alloc(32, 0x12)},
    {networkId: NetworkId.fromBytes(Buffer.alloc(32, 0x15))},
    {callId: {...usageOptions.callId, callName: "different-call"}},
  ]) {
    const input = {...usageOptions, blinding: Buffer.alloc(32, 0x11), ...changed};
    assert.throws(() => buildKaigiUsageProofV1(input), /does not open the stored hostCommitment/u);
    assert.deepEqual(input.blinding, Buffer.alloc(32));
  }
  owner.fill(0x11);
  const usage = buildKaigiUsageProofV1({...usageOptions, blinding});
  assert.deepEqual(blinding, Buffer.alloc(32));
  assert.deepEqual(owner.subarray(0, 4), Buffer.alloc(4, 0x11));
  assert.deepEqual(owner.subarray(36), Buffer.alloc(4, 0x11));
  assert.deepEqual(usage.hostCommitment, result.commitment);
  assert.deepEqual(usage.preRosterRoot, usageOptions.preRosterRoot);
  assert.equal(usage.proof.subarray(0, 4).toString("ascii"), "NRT0");
  assert.ok(usage.proof.length > 25 * 32);
  assert.ok(Object.isFrozen(usage));
  assert.ok(usage.usageCommitment.some((byte) => byte !== 0));
});
