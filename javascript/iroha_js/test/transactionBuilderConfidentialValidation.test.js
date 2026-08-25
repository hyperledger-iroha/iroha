import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import {
  buildConfidentialTransferProofV2,
  buildConfidentialUnshieldProofV2,
  buildConfidentialUnshieldProofV3,
} from "../src/transaction.js";
import { NetworkId } from "../src/networkId.js";

const NETWORK_ID = NetworkId.parse(
  "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
);
const NETWORK_ID_BYTES = Buffer.from(NETWORK_ID.toBytes());
const ASSET_DEFINITION_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

baseTest("confidential proof builders require exact NetworkId and reject malformed fields before native dispatch", () => {
  const calls = [];
  const verifyingKey = {
    id: { backend: "halo2/ipa" },
    record: {
      circuit_id: "confidential-transfer-v2",
      backend: "halo2/ipa",
      inline_key: {
        backend: "halo2/ipa",
        bytes_b64: Buffer.from([1, 2, 3]).toString("base64"),
      },
    },
  };
  const spendKey = Buffer.alloc(32, 0x42);
  const rho = Buffer.alloc(32, 0x51).toString("hex");
  const diversifier = Buffer.alloc(32, 0x52).toString("hex");
  const ownerTag = Buffer.alloc(32, 0x53).toString("hex");
  const rootHint = Buffer.alloc(32, 0x54).toString("hex");
  const treeCommitment = Buffer.alloc(32, 0x55).toString("hex");
  const baseRequest = {
    networkId: NETWORK_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    spendKey,
    treeCommitments: [treeCommitment],
    inputs: [{ amount: "7", rhoHex: rho, diversifierHex: diversifier, leafIndex: 0 }],
    outputs: [{ amount: "7", rhoHex: rho, ownerTagHex: ownerTag }],
    rootHintHex: rootHint,
    verifyingKey,
  };
  withNativeBinding(
    {
      buildConfidentialTransferProofV2: (...args) => {
        calls.push(args);
        return {
          nullifiers: [],
          outputCommitments: [],
          root: Buffer.alloc(32, 0x61),
          proof: Buffer.from([0x62]),
        };
      },
      buildConfidentialUnshieldProofV2: () => {
        throw new Error("unshield v2 publicAmount should fail before native call");
      },
      buildConfidentialUnshieldProofV3: () => {
        throw new Error("unshield v3 publicAmount should fail before native call");
      },
    },
    () => {
      buildConfidentialTransferProofV2(baseRequest);
      assert.equal(calls.length, 1);
      assert.deepEqual(calls[0][0], NETWORK_ID_BYTES);
      assert.equal(calls[0][1], ASSET_DEFINITION_ID);
      assert.deepEqual(calls[0][3], [treeCommitment]);
      assert.equal(calls[0][4][0].rhoHex, rho);
      assert.equal(calls[0][4][0].diversifierHex, diversifier);
      assert.equal(calls[0][5][0].ownerTagHex, ownerTag);

      calls.length = 0;
      for (const [label, patch, message] of [
        [
          "networkId",
          { networkId: "test-chain" },
          /confidentialTransferProofV2\.networkId must be a NetworkId/u,
        ],
        [
          "assetDefinitionId",
          { assetDefinitionId: `${ASSET_DEFINITION_ID} ` },
          /confidentialTransferProofV2\.assetDefinitionId must not contain surrounding whitespace/u,
        ],
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
          "rootHintHex",
          { rootHintHex: `${rootHint} ` },
          /rootHintHex must be exactly 64 lowercase hex characters/u,
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
        [
          "top-level verifying key alias",
          {
            verifyingKey: {
              id: { backend: "halo2/ipa" },
              record: verifyingKey.record,
              inlineKey: {
                bytesBase64: Buffer.from([1, 2, 3]).toString("base64"),
              },
            },
          },
          /verifyingKey\.inlineKey is retired/u,
        ],
        [
          "record circuit camel alias",
          {
            verifyingKey: {
              id: { backend: "halo2/ipa" },
              record: {
                ...verifyingKey.record,
                circuit_id: undefined,
                circuitId: "confidential-transfer-v2",
              },
            },
          },
          /verifyingKey\.record\.circuitId is retired/u,
        ],
        [
          "inline bytes camel alias",
          {
            verifyingKey: {
              id: { backend: "halo2/ipa" },
              record: {
                ...verifyingKey.record,
                inline_key: {
                  backend: "halo2/ipa",
                  bytesBase64: Buffer.from([1, 2, 3]).toString("base64"),
                },
              },
            },
          },
          /verifyingKey\.record\.inline_key\.bytesBase64 is retired/u,
        ],
        [
          "mismatched verifying key backend",
          {
            verifyingKey: {
              id: { backend: "halo2/ipa" },
              record: { ...verifyingKey.record, backend: "stark/fri" },
            },
          },
          /verifyingKey backend fields must match exactly/u,
        ],
        [
          "noncanonical verifying key base64",
          {
            verifyingKey: {
              id: { backend: "halo2/ipa" },
              record: {
                ...verifyingKey.record,
                inline_key: {
                  backend: "halo2/ipa",
                  bytes_b64: " AQID",
                },
              },
            },
          },
          /bytes_b64 must be canonical non-empty base64/u,
        ],
      ]) {
        assert.throws(
          () => buildConfidentialTransferProofV2({ ...baseRequest, ...patch }),
          message,
          label,
        );
      }

      assert.throws(
        () =>
          buildConfidentialUnshieldProofV2({
            networkId: NETWORK_ID,
            assetDefinitionId: ASSET_DEFINITION_ID,
            spendKey,
            treeCommitments: [treeCommitment],
            inputs: [
              {
                amount: "7",
                rhoHex: rho,
                diversifierHex: diversifier,
                leafIndex: 0,
              },
            ],
            publicAmount: " 7",
            rootHintHex: rootHint,
            verifyingKey,
          }),
        /publicAmount must not contain surrounding whitespace/u,
      );
      assert.throws(
        () =>
          buildConfidentialUnshieldProofV3({
            networkId: NETWORK_ID,
            assetDefinitionId: ASSET_DEFINITION_ID,
            spendKey,
            treeCommitments: [treeCommitment],
            inputs: [
              {
                amount: "7",
                rhoHex: rho,
                diversifierHex: diversifier,
                leafIndex: 0,
              },
            ],
            outputs: [{ amount: "7", rhoHex: rho }],
            publicAmount: "7\n",
            rootHintHex: rootHint,
            verifyingKey,
          }),
        /publicAmount must not contain surrounding whitespace/u,
      );
    },
  );

  assert.deepEqual(calls, []);
});

baseTest("confidential proof builders reject noncanonical native result shapes", () => {
  const backend = "halo2/ipa";
  const verifyingKey = {
    id: { backend },
    record: {
      circuit_id: "confidential-transfer-v2",
      backend,
      inline_key: { backend, bytes_b64: "AQID" },
    },
  };
  const rhoHex = Buffer.alloc(32, 0x51).toString("hex");
  const request = {
    networkId: NETWORK_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    spendKey: Buffer.alloc(32, 0x42),
    treeCommitments: [Buffer.alloc(32, 0x55)],
    inputs: [
      {
        amount: "7",
        rhoHex,
        diversifierHex: Buffer.alloc(32, 0x52).toString("hex"),
        leafIndex: 0,
      },
    ],
    outputs: [
      {
        amount: "7",
        rhoHex,
        ownerTagHex: Buffer.alloc(32, 0x53).toString("hex"),
      },
    ],
    rootHintHex: Buffer.alloc(32, 0x54).toString("hex"),
    verifyingKey,
  };
  const validResult = {
    nullifiers: [Buffer.alloc(32, 0x61)],
    outputCommitments: [Buffer.alloc(32, 0x62)],
    root: Buffer.alloc(32, 0x63),
    proof: Buffer.from([0x64]),
  };
  let result = validResult;

  withNativeBinding(
    { buildConfidentialTransferProofV2: () => result },
    () => {
      for (const [label, replacement, message] of [
        [
          "missing nullifiers",
          { ...validResult, nullifiers: undefined },
          /result\.nullifiers must be an array/u,
        ],
        [
          "wrong nullifiers type",
          { ...validResult, nullifiers: {} },
          /result\.nullifiers must be an array/u,
        ],
        [
          "wrong nullifier width",
          { ...validResult, nullifiers: [Buffer.alloc(31)] },
          /result\.nullifiers\[0\] must be 32 bytes/u,
        ],
        [
          "retired output commitments alias",
          {
            ...validResult,
            outputCommitments: undefined,
            output_commitments: [],
          },
          /result\.output_commitments is retired; use canonical outputCommitments/u,
        ],
        [
          "missing outputCommitments",
          { ...validResult, outputCommitments: undefined },
          /result\.outputCommitments must be an array/u,
        ],
        [
          "wrong outputCommitments type",
          { ...validResult, outputCommitments: {} },
          /result\.outputCommitments must be an array/u,
        ],
        [
          "wrong root width",
          { ...validResult, root: Buffer.alloc(31) },
          /result\.root must be 32 bytes/u,
        ],
        [
          "missing root",
          { ...validResult, root: undefined },
          /result\.root must be a Buffer or ArrayBuffer view/u,
        ],
        [
          "empty proof",
          { ...validResult, proof: Buffer.alloc(0) },
          /result\.proof must be non-empty/u,
        ],
        [
          "missing proof",
          { ...validResult, proof: undefined },
          /result\.proof must be a Buffer or ArrayBuffer view/u,
        ],
        [
          "unknown result field",
          { ...validResult, legacy: true },
          /result\.legacy is not a canonical result field/u,
        ],
      ]) {
        result = replacement;
        assert.throws(
          () => buildConfidentialTransferProofV2(request),
          message,
          label,
        );
      }
    },
  );
});


function withNativeBinding(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return fn();
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
}
