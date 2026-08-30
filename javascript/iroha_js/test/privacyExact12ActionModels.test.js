import assert from "node:assert/strict";
import test from "node:test";

import {
  PRIVACY_EXACT12_ACTION_OPERATIONS_V1,
  PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1,
  PRIVACY_LEDGER_EFFECT_KINDS_V1,
  PrivacyActionOperationViewV1,
  PrivacyExact12ActionRequestV1,
  privacyExact12LedgerEffectKindV1,
  privacyExact12ProtocolIdV1,
} from "../src/privacyExact12ActionModels.js";

const PROTOCOLS = Object.freeze([
  "zk-ace-pq-authorization-v0",
  "anonymous-pgc-k-out-of-n-v1",
  "verange-transparent-range-v1",
  "iroha-zk-ams-v1",
  "iroha-zk-ams-v1",
  "vega-existing-credential-zk-v0",
  "iroha-zk-x509-stark-p256-v0",
  "iroha-jindo-polynomial-commitment-v0",
  "iroha-bootle-lantern-anoncred-v1",
  "orchard-halo2-actions-v1",
  "monero-fcmp-plus-plus-v1",
  "iroha-ivm-private-note-stark-v1",
  "pq-masp-stark-v0",
]);

const EFFECTS = Object.freeze([
  "zk_ace_transparent_transfer",
  "anonymous_pgc_account_state_transition",
  "verification_only",
  "zk_ams_batch_admission",
  "zk_ams_provision_account",
  "verification_only",
  "zk_x509_certificate_nullifier",
  "verification_only",
  "verification_only",
  "orchard_note_state_transition",
  "fcmp_membership_payment",
  "ivm_private_note_state_transition",
  "pq_masp_note_state_transition",
]);

function fixed32(byte) {
  return new Uint8Array(32).fill(byte);
}

function view(overrides = {}) {
  const operationSchema = overrides.operationSchema ?? "orchard_note_action_v1";
  return new PrivacyActionOperationViewV1({
    protocolId: privacyExact12ProtocolIdV1(operationSchema),
    operationSchema,
    transactionHash: fixed32(1),
    transactionIntentDigest: fixed32(2),
    statementDigest: fixed32(3),
    proofEnvelopeHash: fixed32(4),
    localState: "submitted",
    terminalChainState: null,
    committedHeight: null,
    rejectionReason: null,
    ledgerEffectKind: privacyExact12LedgerEffectKindV1(operationSchema),
    capabilityManifestDigest: fixed32(5),
    capabilityCommittedHeight: 10n,
    ...overrides,
  });
}

test("Exact12 action vocabulary and protocol/effect mappings are closed", () => {
  assert.equal(PRIVACY_EXACT12_ACTION_OPERATIONS_V1.length, 13);
  assert.equal(new Set(PRIVACY_EXACT12_ACTION_OPERATIONS_V1).size, 13);
  assert.equal(PRIVACY_LEDGER_EFFECT_KINDS_V1.length, 10);
  assert.equal(new Set(PRIVACY_LEDGER_EFFECT_KINDS_V1).size, 10);
  assert.deepEqual(
    PRIVACY_EXACT12_ACTION_OPERATIONS_V1.map(privacyExact12ProtocolIdV1),
    PROTOCOLS,
  );
  assert.deepEqual(
    PRIVACY_EXACT12_ACTION_OPERATIONS_V1.map(
      privacyExact12LedgerEffectKindV1,
    ),
    EFFECTS,
  );
  assert.deepEqual(new Set(EFFECTS), new Set(PRIVACY_LEDGER_EFFECT_KINDS_V1));
  assert.throws(
    () => privacyExact12ProtocolIdV1("zk_ams_admission_and_provisioning_v1"),
    /one exact PrivacyExact12ActionOperationV1/u,
  );
});

test("Exact12 requests bound and defensively snapshot wire and digest bytes", () => {
  const wire = Uint8Array.from([1, 2]);
  const digest = fixed32(0x21);
  const request = new PrivacyExact12ActionRequestV1({
    operation: "zk_ams_provision_account_action_v1",
    signedTransactionVersioned: wire,
    expectedManifestDigest: digest,
  });
  wire[0] = 0xff;
  digest[0] = 0xff;
  assert.deepEqual(request.signedTransactionVersioned, Uint8Array.from([1, 2]));
  assert.deepEqual(request.expectedManifestDigest, fixed32(0x21));
  const leaked = request.signedTransactionVersioned;
  leaked[0] = 0xee;
  assert.equal(request.signedTransactionVersioned[0], 1);

  assert.doesNotThrow(
    () => new PrivacyExact12ActionRequestV1(
      "verange_range_proof_v1",
      new Uint8Array(PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1),
    ),
  );
  for (const construct of [
    () => new PrivacyExact12ActionRequestV1(
      "verange_range_proof_v1",
      new Uint8Array(),
    ),
    () => new PrivacyExact12ActionRequestV1(
      "verange_range_proof_v1",
      new Uint8Array(PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1 + 1),
    ),
    () => new PrivacyExact12ActionRequestV1(
      "verange_range_proof_v1",
      Uint8Array.of(1),
      new Uint8Array(32),
    ),
    () => new PrivacyExact12ActionRequestV1(
      "verange_range_proof_v1",
      Uint8Array.of(1),
      new Uint8Array(31).fill(1),
    ),
  ]) {
    assert.throws(construct);
  }
});

test("Exact12 operation views accept only authenticated lifecycle shapes", () => {
  assert.equal(view().localState, "submitted");
  const committed = view({
    localState: "terminal",
    terminalChainState: "Committed",
    committedHeight: 42,
  });
  assert.equal(committed.committedHeight, 42n);
  assert.equal(committed.executionCapabilityManifestDigest, null);

  const applied = view({
    localState: "terminal",
    terminalChainState: "Applied",
    committedHeight: 42,
    executionCapabilityManifestDigest: fixed32(0x31),
    executionCapabilityCommittedHeight: 41n,
    executionReceiptFinalizedHeight: 43n,
    executionReceiptFinalizedBlockHash: fixed32(0x32),
  });
  assert.equal(applied.committedHeight, 42n);
  assert.deepEqual(applied.executionCapabilityManifestDigest, fixed32(0x31));
  assert.equal(applied.executionCapabilityCommittedHeight, 41n);
  assert.equal(applied.executionReceiptFinalizedHeight, 43n);
  assert.deepEqual(applied.executionReceiptFinalizedBlockHash, fixed32(0x32));
  const rejected = view({
    localState: "terminal",
    terminalChainState: "Rejected",
    committedHeight: 43n,
    rejectionReason: "proof envelope expired",
  });
  assert.equal(rejected.rejectionReason, "proof envelope expired");
  const expired = view({
    localState: "terminal",
    terminalChainState: "Expired",
  });
  assert.equal(expired.committedHeight, null);

  const hostile = [
    { localState: "submitted", terminalChainState: "Committed" },
    { localState: "submitted", committedHeight: 1n },
    { localState: "terminal", terminalChainState: null },
    { localState: "terminal", terminalChainState: "Committed" },
    {
      localState: "terminal",
      terminalChainState: "Applied",
      committedHeight: 1n,
    },
    {
      localState: "terminal",
      terminalChainState: "Applied",
      committedHeight: 1n,
      rejectionReason: "unexpected",
    },
    {
      localState: "terminal",
      terminalChainState: "Applied",
      committedHeight: 2n,
      executionCapabilityManifestDigest: fixed32(0x31),
      executionCapabilityCommittedHeight: 1n,
      executionReceiptFinalizedHeight: 2n,
    },
    {
      localState: "terminal",
      terminalChainState: "Applied",
      committedHeight: 2n,
      executionCapabilityManifestDigest: fixed32(0x31),
      executionCapabilityCommittedHeight: 3n,
      executionReceiptFinalizedHeight: 3n,
      executionReceiptFinalizedBlockHash: fixed32(0x32),
    },
    {
      localState: "terminal",
      terminalChainState: "Applied",
      committedHeight: 2n,
      executionCapabilityManifestDigest: fixed32(0x31),
      executionCapabilityCommittedHeight: 1n,
      executionReceiptFinalizedHeight: 1n,
      executionReceiptFinalizedBlockHash: fixed32(0x32),
    },
    {
      localState: "terminal",
      terminalChainState: "Rejected",
      rejectionReason: "rejected",
    },
    {
      localState: "terminal",
      terminalChainState: "Rejected",
      committedHeight: 1n,
      rejectionReason: " rejected ",
    },
    {
      localState: "terminal",
      terminalChainState: "Rejected",
      committedHeight: 1n,
      rejectionReason: "policy\u0001rejected",
    },
    {
      localState: "terminal",
      terminalChainState: "Rejected",
      committedHeight: 1n,
      rejectionReason: "é".repeat(513),
    },
    {
      localState: "terminal",
      terminalChainState: "Expired",
      committedHeight: 1n,
    },
    {
      executionCapabilityManifestDigest: fixed32(0x31),
    },
  ];
  hostile.forEach((overrides, index) => {
    assert.throws(() => view(overrides), `accepted hostile state ${index}`);
  });
});

test("Exact12 operation views reject forged mappings, bytes, and u64 heights", () => {
  for (const overrides of [
    { protocolId: "iroha-zk-ams-v1" },
    { ledgerEffectKind: "verification_only" },
    { transactionHash: new Uint8Array(32) },
    { capabilityManifestDigest: new Uint8Array(31).fill(1) },
    { capabilityCommittedHeight: 0n },
    { capabilityCommittedHeight: 1n << 64n },
    { capabilityCommittedHeight: Number.MAX_SAFE_INTEGER + 1 },
    {
      localState: "terminal",
      terminalChainState: "Committed",
      committedHeight: 0n,
    },
    {
      localState: "terminal",
      terminalChainState: "Committed",
      committedHeight: 9n,
    },
    {
      localState: "terminal",
      terminalChainState: "Rejected",
      committedHeight: 9n,
      rejectionReason: "rejected before the observed capability snapshot",
    },
    {
      localState: "terminal",
      terminalChainState: "Applied",
      committedHeight: 9n,
      executionCapabilityManifestDigest: fixed32(0x31),
      executionCapabilityCommittedHeight: 8n,
      executionReceiptFinalizedHeight: 9n,
      executionReceiptFinalizedBlockHash: fixed32(0x32),
    },
  ]) {
    assert.throws(() => view(overrides));
  }

  const transactionHash = fixed32(0x11);
  const capabilityDigest = fixed32(0x12);
  const executionDigest = fixed32(0x13);
  const finalizedBlockHash = fixed32(0x14);
  const snapshot = view({
    localState: "terminal",
    terminalChainState: "Applied",
    committedHeight: (1n << 64n) - 1n,
    transactionHash,
    capabilityManifestDigest: capabilityDigest,
    capabilityCommittedHeight: (1n << 64n) - 1n,
    executionCapabilityManifestDigest: executionDigest,
    executionCapabilityCommittedHeight: (1n << 64n) - 1n,
    executionReceiptFinalizedHeight: (1n << 64n) - 1n,
    executionReceiptFinalizedBlockHash: finalizedBlockHash,
  });
  transactionHash[0] = 0;
  capabilityDigest[0] = 0;
  executionDigest[0] = 0;
  finalizedBlockHash[0] = 0;
  assert.deepEqual(snapshot.transactionHash, fixed32(0x11));
  assert.deepEqual(snapshot.capabilityManifestDigest, fixed32(0x12));
  assert.deepEqual(snapshot.executionCapabilityManifestDigest, fixed32(0x13));
  assert.deepEqual(snapshot.executionReceiptFinalizedBlockHash, fixed32(0x14));
  const leaked = snapshot.transactionHash;
  leaked[0] = 0;
  assert.equal(snapshot.transactionHash[0], 0x11);
  const leakedExecutionDigest = snapshot.executionCapabilityManifestDigest;
  leakedExecutionDigest[0] = 0;
  assert.equal(snapshot.executionCapabilityManifestDigest[0], 0x13);
  assert.equal(snapshot.capabilityCommittedHeight, (1n << 64n) - 1n);
});
