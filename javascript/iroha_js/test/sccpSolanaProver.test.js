import { test } from "node:test";
import assert from "node:assert/strict";
import {
  SCCP_DOMAIN_SOL,
  SCCP_DOMAIN_SORA,
  SCCP_DOMAIN_TON,
  SCCP_SOLANA_MAINNET_GENESIS_HASH,
  SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1,
  SCCP_TON_MESSAGE_BODY_BOC_V1,
  SolanaSccpProver,
  TonSccpProver,
  buildSccpTonMessageBodyBoc,
  buildSolanaSccpProofRequest,
  buildTonSccpSubmission,
  canonicalSolanaSccpMessageProofBytes,
  canonicalSolanaSccpWitnessBytes,
  canonicalSccpMessageTransparentPublicInputsBytes,
  normalizeSolanaSccpWitness,
  solanaSccpMessageProofHash,
} from "../src/sccp.js";

const HEX32_A = `0x${"aa".repeat(32)}`;
const HEX32_B = `0x${"bb".repeat(32)}`;
const HEX32_C = `0x${"cc".repeat(32)}`;
const HEX32_D = `0x${"dd".repeat(32)}`;
const HEX32_E = `0x${"ee".repeat(32)}`;
const HEX32_F = `0x${"12".repeat(32)}`;
const HEX32_G = `0x${"56".repeat(32)}`;

const sampleTonPublicInputs = {
  version: 1,
  messageId: HEX32_D,
  payloadHash: HEX32_E,
  targetDomain: SCCP_DOMAIN_TON,
  commitmentRoot: HEX32_F,
  finalityHeight: 19n,
  finalityBlockHash: HEX32_A,
};

function sampleWitness(overrides = {}) {
  return {
    targetDomain: SCCP_DOMAIN_SORA,
    finalizedSlot: 321n,
    blockhash: "9xQeWvG816bUx9EPfYdLSdJH7Gq2Xv3yQPG8mD3kAcL7",
    bankHash: HEX32_A,
    transactionStatusRoot: HEX32_B,
    messageProofHash: HEX32_C,
    transactionSignature: "5eykt4Signature111111111111111111111111111111",
    emitterProgramId: "Bridge111111111111111111111111111111111111",
    messageId: HEX32_D,
    payloadHash: HEX32_E,
    commitmentRoot: HEX32_F,
    sourceEventDigest: `0x${"34".repeat(32)}`,
    ...overrides,
  };
}

test("normalizes Solana SCCP witness input for local proof requests", () => {
  const witness = normalizeSolanaSccpWitness(sampleWitness());

  assert.equal(witness.version, 1);
  assert.equal(witness.sourceDomain, SCCP_DOMAIN_SOL);
  assert.equal(witness.targetDomain, SCCP_DOMAIN_SORA);
  assert.equal(witness.mainnetGenesisHash, SCCP_SOLANA_MAINNET_GENESIS_HASH);
  assert.equal(witness.finalizedSlot, "321");
  assert.equal(witness.messageId, HEX32_D);
  assert.equal(witness.sourceEventDigest, `0x${"34".repeat(32)}`);
  assert.ok(canonicalSolanaSccpWitnessBytes(witness).length > 0);
});

test("requires caller-supplied Solana source event digest", () => {
  assert.throws(
    () => normalizeSolanaSccpWitness(sampleWitness({ sourceEventDigest: undefined })),
    /sourceEventDigest must be a hex string/,
  );
});

test("derives Solana message proof hash from inclusion witness", () => {
  const inclusionBranch = [HEX32_G];
  const derived = solanaSccpMessageProofHash({
    sourceEventDigest: `0x${"34".repeat(32)}`,
    transactionStatusRoot: HEX32_B,
    inclusionBranch,
  });
  assert.match(derived, /^0x[0-9a-f]{64}$/);
  assert.ok(
    canonicalSolanaSccpMessageProofBytes({
      sourceEventDigest: `0x${"34".repeat(32)}`,
      transactionStatusRoot: HEX32_B,
      inclusionBranch,
    }).length > 0,
  );
  assert.equal(
    normalizeSolanaSccpWitness(sampleWitness({ messageProofHash: undefined, inclusionBranch }))
      .messageProofHash,
    derived,
  );
  assert.throws(
    () =>
      solanaSccpMessageProofHash({
        sourceEventDigest: `0x${"34".repeat(32)}`,
        transactionStatusRoot: HEX32_B,
        inclusionBranch: [`0x${"ab".repeat(31)}`],
      }),
    /inclusionBranch\[0\] must be 32 bytes/,
  );
});

test("builds deterministic Solana SCCP proof requests", () => {
  const request = buildSolanaSccpProofRequest(sampleWitness());

  assert.equal(request.version, 1);
  assert.equal(request.backend, SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1);
  assert.equal(request.sourceDomain, SCCP_DOMAIN_SOL);
  assert.equal(request.publicInputs.messageId, HEX32_D);
  assert.match(request.witnessHash, /^0x[0-9a-f]{64}$/);
});

test("does not generate Solana SCCP proofs without a linked local prover", async () => {
  const prover = new SolanaSccpProver();

  await assert.rejects(
    () => prover.prove(sampleWitness()),
    (error) => error?.code === "ERR_SCCP_SOLANA_PROVER_UNAVAILABLE",
  );
});

test("wraps externally generated Solana SCCP proof bytes", async () => {
  const prover = new SolanaSccpProver({
    prove: async (request) => {
      assert.equal(request.backend, SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1);
      return { proofBytes: [1, 2, 3, 4] };
    },
  });

  const result = await prover.prove(sampleWitness());

  assert.deepEqual(Array.from(result.proofBytes), [1, 2, 3, 4]);
  assert.equal(result.proofBase64, "AQIDBA==");
  assert.match(result.envelopeHash, /^0x[0-9a-f]{64}$/);
});

test("builds TON SCCP internal message BOC in browser-safe JavaScript", () => {
  const messageBodyBoc = buildSccpTonMessageBodyBoc({
    publicInputs: sampleTonPublicInputs,
    proofBytes: Uint8Array.from([1, 2, 3, 4]),
    bundleBytes: Uint8Array.from([5, 6, 7]),
    statementHash: HEX32_B,
    destinationBindingHash: HEX32_G,
    metadataBytes: Uint8Array.from([8, 9]),
  });

  assert.deepEqual(Array.from(messageBodyBoc.slice(0, 4)), [0xb5, 0xee, 0x9c, 0x72]);
  assert.ok(messageBodyBoc.length > canonicalSccpMessageTransparentPublicInputsBytes(sampleTonPublicInputs).length);

  const submission = buildTonSccpSubmission({
    publicInputs: sampleTonPublicInputs,
    proofBytes: Uint8Array.from([1, 2, 3, 4]),
    bundleBytes: Uint8Array.from([5, 6, 7]),
    statementHash: HEX32_B,
    destinationBindingHash: HEX32_G,
    metadataBytes: Uint8Array.from([8, 9]),
  });
  assert.equal(submission.envelopeEncoding, SCCP_TON_MESSAGE_BODY_BOC_V1);
  assert.equal(submission.arguments[0].key, "message_body_boc");
  assert.equal(submission.arguments[0].encoding, "ton_boc");
  assert.equal(submission.envelopeHex, submission.messageBodyBocHex);
});

test("does not generate TON SCCP proofs without a linked local prover", async () => {
  const prover = new TonSccpProver();

  await assert.rejects(
    () =>
      prover.prove({
        publicInputs: sampleTonPublicInputs,
        bundleBytes: [5, 6, 7],
      }),
    (error) => error?.code === "ERR_SCCP_TON_PROVER_UNAVAILABLE",
  );
});
