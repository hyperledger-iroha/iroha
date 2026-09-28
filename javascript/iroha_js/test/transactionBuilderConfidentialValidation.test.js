import test from "node:test";
import assert from "node:assert/strict";
import { createConfidentialProverClass } from "../src/confidentialProofBuilders.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import { NetworkId } from "../src/networkId.js";
const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0x13));
const ASSET_DEFINITION_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const rho = "51".repeat(32), diversifier = "52".repeat(32), ownerTag = "53".repeat(32);
const rootHex = "54".repeat(32), treeCommitment = "55".repeat(32);
const options = { networkId: NETWORK_ID, assetDefinitionId: ASSET_DEFINITION_ID, spendKey: Buffer.alloc(32, 0x42) };
const baseRequest = {
  treeCommitments: [treeCommitment], rootHex,
  inputs: [{ amount: "7", rhoHex: rho, diversifierHex: diversifier, leafIndex: 0 }],
  outputs: [{ amount: "7", rhoHex: rho, ownerTagHex: ownerTag }],
};
const validResult = {
  nullifiers: [Buffer.alloc(32, 0x61)], outputCommitments: [Buffer.alloc(32, 0x62)],
  root: Buffer.from(rootHex, "hex"), proof: Buffer.from([0x64]),
};
function wallet(binding) {
  const Prover = createConfidentialProverClass(createNativeRuntime({
    proveConfidentialTransfer: () => validResult,
    proveConfidentialRedemption: () => { throw new Error("unexpected dispatch"); },
    ...binding,
  }));
  return { Prover, prover: new Prover(options) };
}

test("canonical wallet requires exact NetworkId and rejects malformed fields before native dispatch", async () => {
  let calls = 0;
  const { Prover, prover } = wallet({ proveConfidentialTransfer: () => { calls += 1; return validResult; } });
  try {
    for (const [patch, message] of [
      [{ networkId: "test-chain" }, /networkId must be a NetworkId/u],
      [{ assetDefinitionId: `${ASSET_DEFINITION_ID} ` }, /assetDefinitionId must not contain surrounding whitespace/u],
    ]) assert.throws(() => new Prover({ ...options, ...patch }), message);
    for (const [label, patch, message] of [
        [
          "input amount",
          {
            inputs: [
              {
                amount: " 7",
                rhoHex: rho,
                diversifierHex: diversifier,
                leafIndex: 0,
              },
            ],
          },
          /inputs\[0\]\.amount must not contain surrounding whitespace/u,
        ],
        [
          "inputs rho",
          {
            inputs: [
              {
                amount: "7",
                rhoHex: `${rho} `,
                diversifierHex: diversifier,
                leafIndex: 0,
              },
            ],
          },
          /inputs\[0\]\.rhoHex must be exactly 64 lowercase hex characters/u,
        ],
        [
          "input diversifier",
          {
            inputs: [
              {
                amount: "7",
                rhoHex: rho,
                diversifierHex: ` ${diversifier}`,
                leafIndex: 0,
              },
            ],
          },
          /inputs\[0\]\.diversifierHex must be exactly 64 lowercase hex characters/u,
        ],
        [
          "missing input diversifier",
          { inputs: [{ amount: "7", rhoHex: rho, leafIndex: 0 }] },
          /inputs\[0\]\.diversifierHex must be exactly 64 lowercase hex characters/u,
        ],
        [
          "input diversifier snake alias",
          {
            inputs: [
              {
                amount: "7",
                rhoHex: rho,
                diversifier_hex: diversifier,
                leafIndex: 0,
              },
            ],
          },
          /inputs\[0\]\.diversifier_hex is retired; use canonical diversifierHex/u,
        ],
        [
          "input diversifier raw alias",
          {
            inputs: [
              {
                amount: "7",
                rhoHex: rho,
                diversifier: Buffer.alloc(32, 0x52),
                leafIndex: 0,
              },
            ],
          },
          /inputs\[0\]\.diversifier is retired; use canonical diversifierHex/u,
        ],
        [
          "input rho raw alias",
          {
            inputs: [
              {
                amount: "7",
                rho: Buffer.alloc(32, 0x51),
                diversifierHex: diversifier,
                leafIndex: 0,
              },
            ],
          },
          /inputs\[0\]\.rho is retired; use canonical rhoHex/u,
        ],
        [
          "input leaf snake alias",
          {
            inputs: [
              {
                amount: "7",
                rhoHex: rho,
                diversifierHex: diversifier,
                leaf_index: 0,
              },
            ],
          },
          /inputs\[0\]\.leaf_index is retired; use canonical leafIndex/u,
        ],
        [
          "missing input leafIndex",
          {
            inputs: [
              { amount: "7", rhoHex: rho, diversifierHex: diversifier },
            ],
          },
          /inputs\[0\]\.leafIndex must be an unsigned 32-bit integer/u,
        ],
        [
          "output amount",
          { outputs: [{ amount: "7\n", rhoHex: rho, ownerTagHex: ownerTag }] },
          /outputs\[0\]\.amount must not contain surrounding whitespace/u,
        ],
        [
          "output ownerTag",
          { outputs: [{ amount: "7", rhoHex: rho, ownerTagHex: `${ownerTag}\n` }] },
          /outputs\[0\]\.ownerTagHex must be exactly 64 lowercase hex characters/u,
        ],
        [
          "output rho raw alias",
          {
            outputs: [
              {
                amount: "7",
                rho: Buffer.alloc(32, 0x51),
                ownerTagHex: ownerTag,
              },
            ],
          },
          /outputs\[0\]\.rho is retired; use canonical rhoHex/u,
        ],
        [
          "output owner snake alias",
          {
            outputs: [
              { amount: "7", rhoHex: rho, owner_tag_hex: ownerTag },
            ],
          },
          /outputs\[0\]\.owner_tag_hex is retired; use canonical ownerTagHex/u,
        ],
        [
          "output owner raw alias",
          {
            outputs: [
              {
                amount: "7",
                rhoHex: rho,
                ownerTag: Buffer.alloc(32, 0x53),
              },
            ],
          },
          /outputs\[0\]\.ownerTag is retired; use canonical ownerTagHex/u,
        ],
        [
          "treeCommitments",
          { treeCommitments: [` ${treeCommitment}`] },
          /treeCommitments\[0\] must be exactly 64 lowercase hex characters/u,
        ],
        [
          "rootHex",
          { rootHex: `${rootHex} ` },
          /rootHex must be exactly 64 lowercase hex characters/u,
        ],
        [
          "prefixed rhoHex",
          {
            inputs: [
              {
                amount: "7",
                rhoHex: `0x${rho}`,
                diversifierHex: diversifier,
                leafIndex: 0,
              },
            ],
          },
          /inputs\[0\]\.rhoHex must be exactly 64 lowercase hex characters/u,
        ],
        [
          "uppercase ownerTagHex",
          {
            outputs: [
              {
                amount: "7",
                rhoHex: rho,
                ownerTagHex: Buffer.alloc(32, 0xab).toString("hex").toUpperCase(),
              },
            ],
          },
          /outputs\[0\]\.ownerTagHex must be exactly 64 lowercase hex characters/u,
        ],
        [
          "missing inputs array",
          { inputs: undefined },
          /inputs must be an array/u,
        ],
        [
          "non-array inputs",
          { inputs: {} },
          /inputs must be an array/u,
        ],
        [
          "missing outputs array",
          { outputs: undefined },
          /outputs must be an array/u,
        ],
        [
          "non-array outputs",
          { outputs: {} },
          /outputs must be an array/u,
        ],
        [
          "missing treeCommitments array",
          { treeCommitments: undefined },
          /treeCommitments must be an array/u,
        ],
        [
          "non-array treeCommitments",
          { treeCommitments: {} },
          /treeCommitments must be an array/u,
        ],
    ]) await assert.rejects(() => prover.proveTransfer({ ...baseRequest, ...patch }), message, label);
    const { outputs: _outputs, ...spend } = baseRequest;
    for (const publicAmount of [" 7", "7\n"]) {
      await assert.rejects(() => prover.proveRedemption({ ...spend, publicAmount }), /publicAmount must not contain surrounding whitespace/u);
    }
    assert.equal(calls, 0);
  } finally { prover.dispose(); }
});

test("canonical wallet rejects caller circuit keys and removed fields without inspecting them", async () => {
  const { Prover, prover } = wallet();
  try {
    for (const field of ["verifyingKey", "circuitId", "circuit_id", "backend", "inlineKey", "rootHintHex"]) {
      const patch = {};
      Object.defineProperty(patch, field, { enumerable: true, get() { throw new Error("must not inspect removed metadata"); } });
      // Define getters on the final argument, so object spread cannot invoke them first.
      const configuration = { ...options };
      Object.defineProperty(configuration, field, Object.getOwnPropertyDescriptor(patch, field));
      assert.throws(() => new Prover(configuration), new RegExp(`wallet options\\.${field} is not a canonical field`, "u"));
      const request = { ...baseRequest };
      Object.defineProperty(request, field, Object.getOwnPropertyDescriptor(patch, field));
      await assert.rejects(() => prover.proveTransfer(request), new RegExp(`request\\.${field} is not a canonical field`, "u"));
    }
    await assert.rejects(() => prover.proveRedemption({ ...baseRequest, publicAmount: 7 }), /request.outputs is not a canonical field/u);
  } finally { prover.dispose(); }
});

test("canonical wallet rejects every noncanonical native result shape", async () => {
  let result = validResult;
  const { prover } = wallet({ proveConfidentialTransfer: () => result });
  try {
    for (const [label, replacement, message] of [
        [
          "missing nullifiers",
          { ...validResult, nullifiers: undefined },
          /confidential proof\.nullifiers must be an array/u,
        ],
        [
          "wrong nullifiers type",
          { ...validResult, nullifiers: {} },
          /confidential proof\.nullifiers must be an array/u,
        ],
        [
          "wrong nullifier width",
          { ...validResult, nullifiers: [Buffer.alloc(31)] },
          /confidential proof\.nullifiers\[0\] must be 32 bytes/u,
        ],
        [
          "retired output commitments alias",
          {
            ...validResult,
            outputCommitments: undefined,
            output_commitments: [],
          },
          /confidential proof\.output_commitments is retired; use canonical outputCommitments/u,
        ],
        [
          "missing outputCommitments",
          { ...validResult, outputCommitments: undefined },
          /confidential proof\.outputCommitments must be an array/u,
        ],
        [
          "wrong outputCommitments type",
          { ...validResult, outputCommitments: {} },
          /confidential proof\.outputCommitments must be an array/u,
        ],
        [
          "wrong root width",
          { ...validResult, root: Buffer.alloc(31) },
          /confidential proof\.root must be 32 bytes/u,
        ],
        [
          "missing root",
          { ...validResult, root: undefined },
          /confidential proof\.root must be a Buffer or ArrayBuffer view/u,
        ],
        [
          "empty proof",
          { ...validResult, proof: Buffer.alloc(0) },
          /confidential proof\.proof must be non-empty/u,
        ],
        [
          "missing proof",
          { ...validResult, proof: undefined },
          /confidential proof\.proof must be a Buffer or ArrayBuffer view/u,
        ],
        [
          "unknown result field",
          { ...validResult, legacy: true },
          /confidential proof\.legacy is not a canonical result field/u,
        ],
    ]) {
      result = replacement;
      await assert.rejects(() => prover.proveTransfer(baseRequest), (error) => {
        assert.equal(error.code, "PROVING_FAILED", label);
        assert.match(error.cause.message, message, label);
        return true;
      });
    }
  } finally { prover.dispose(); }
});

test("canonical wallet classes isolate immutable native runtimes", async () => {
  const bindingA = {
    proveConfidentialTransfer: () => ({ ...validResult, proof: Buffer.from([0xa1]) }),
    proveConfidentialRedemption: () => { throw new Error("unexpected redemption"); },
  };
  const ProverA = createConfidentialProverClass(createNativeRuntime(bindingA));
  const a = { prover: new ProverA(options) };
  const b = wallet({ proveConfidentialTransfer: () => ({ ...validResult, proof: Buffer.from([0xb2]) }) });
  bindingA.proveConfidentialTransfer = () => ({ ...validResult, proof: Buffer.from([0xff]) });
  try {
    const [proofA, proofB] = await Promise.all([a.prover.proveTransfer(baseRequest), b.prover.proveTransfer(baseRequest)]);
    assert.equal(Object.isFrozen(a.prover), true);
    assert.equal(proofA.proof[0], 0xa1);
    assert.equal(proofB.proof[0], 0xb2);
  } finally { a.prover.dispose(); b.prover.dispose(); }
});
