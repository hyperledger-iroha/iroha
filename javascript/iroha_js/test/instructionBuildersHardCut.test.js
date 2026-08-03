import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import {
  buildRegisterAssetDefinitionInstruction,
  buildUnshieldInstruction,
  encodeInstruction,
} from "../src/instructionBuilders.js";
import { AccountAddress } from "../src/address.js";
import { ValidationErrorCode } from "../src/validationError.js";
import {
  noritoDecodeInstruction,
  noritoEncodeInstruction,
} from "../src/norito.js";
import {
  makeNativeTest,
  nativeBinding,
  noritoRequiredMethods,
} from "./helpers/native.js";
import {
  toByteArray,
  withPureJsInstructionCodec,
} from "./helpers/instructionCodec.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const descriptorTest = baseTest;
const SORA_I105_DISCRIMINANT = 0x2f1;
const ACCOUNT_SIGNATORY =
  "ED0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const ACCOUNT_ADDRESS = AccountAddress.fromAccount({
  publicKey: Buffer.from(ACCOUNT_SIGNATORY.slice(6), "hex"),
});
const ACCOUNT_ID_INPUT = ACCOUNT_ADDRESS.toI105(SORA_I105_DISCRIMINANT);
const ASSET_DEFINITION_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";

function canonicalizeValue(value) {
  if (Array.isArray(value)) return value.map(canonicalizeValue);
  if (value && typeof value === "object") {
    if ("Zk" in value && !("zk" in value)) {
      value.zk = canonicalizeValue(value.Zk);
      delete value.Zk;
    }
    for (const key of Object.keys(value)) value[key] = canonicalizeValue(value[key]);
  }
  return value;
}

function canonicalizeClone(value) {
  return canonicalizeValue(JSON.parse(JSON.stringify(value)));
}

function encodeAndDecode(instruction) {
  return canonicalizeValue(noritoDecodeInstruction(encodeInstruction(instruction)));
}

const LEGACY_UNSHIELD_WITH_OUTPUT_WIRE_BASE64 = [
  "TlJUMAAAhip9dwddTSP/bBJh2wJ4EQCxAQAAAAAAACBu70EtkU8QAiQjaXJvaGFfZGF0YV9tb2RlbDo6aXNpOjp6",
  "azo6VW5zaGllbGSKA4IBAAAAAAAATlJUMAAAHLVezH/ZJiWyvuM+SRpKDABSAQAAAAAAAGVNTsPtTT6lAgAAAAAA",
  "AAAAIAFoAXIBRQFOAZwBBAFGAUEBqgFYAR4BxQHzAYABFgEZTwAAAABKIQAAAAAAAAABAAHOAX8BpAFsAZ0BzgF+",
  "AaQBsQElAeIB4wFrAdsBYwHqATMBBwE+AXUBkAGsAZIBgQFqAeEB6AFhAbcBBAGLAQMLBQEAAAABBAAAAABJAQAA",
  "AAAAAABAAREBEQERAREBEQERAREBEQERAREBEQERAREBEQERAREBEQERAREBEQERAREBEQERAREBEQERAREBEQER",
  "AREBEQERAREBEUkBAAAAAAAAAEABIgEiASIBIgEiASIBIgEiASIBIgEiASIBIgEiASIBIgEiASIBIgEiASIBIgEi",
  "ASIBIgEiASIBIgEiASIBIgEiASIBIgEiPgoJaGFsbzIvaXBhGQoJaGFsbzIvaXBhDQUAAAAAAAAAcHJvb2YYCglo",
  "YWxvMi9pcGEMC3ZrX3Vuc2hpZWxkAQA=",
].join("");
test("buildRegisterAssetDefinitionInstruction preserves alias metadata", () => {
  const instruction = buildRegisterAssetDefinitionInstruction({
    assetDefinitionId: ASSET_DEFINITION_ID,
    name: "demo",
    description: "Demo settlement PoC asset",
    alias: "demo#settlement.main",
    scale: 2,
    metadata: { purpose: "poc" },
    owningDomain: null,
    balanceScopePolicy: "Global",
  });
  assert.deepEqual(instruction, {
    Register: {
      AssetDefinition: {
        id: ASSET_DEFINITION_ID,
        name: "demo",
        description: "Demo settlement PoC asset",
        alias: "demo#settlement.main",
        spec: { scale: 2 },
        mintable: "Infinitely",
        logo: null,
        metadata: { purpose: "poc" },
        balance_scope_policy: "Global",
        owning_domain: null,
        confidential_policy: {
          mode: "TransparentOnly",
          vk_set_hash: null,
          poseidon_params_id: null,
          pedersen_params_id: null,
          pending_transition: null,
        },
      },
    },
  });
  assert.deepEqual(encodeAndDecode(instruction), canonicalizeClone(instruction));
  assert.throws(
    () =>
      buildRegisterAssetDefinitionInstruction({
        assetDefinitionId: ASSET_DEFINITION_ID,
      }),
    /owningDomain is required/u,
  );
  assert.throws(
    () =>
      buildRegisterAssetDefinitionInstruction({
        assetDefinitionId: ASSET_DEFINITION_ID,
        owningDomain: null,
      }),
    /balanceScopePolicy is required/u,
  );
});

test("buildUnshieldInstruction honours optional root hints", () => {
  const instruction = buildUnshieldInstruction({
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    destinationAccountId: ACCOUNT_ID_INPUT,
    publicAmount: "18446744073709551616.25",
    inputs: [Buffer.alloc(32, 0x55)],
    proof: {
      backend: "halo2/ipa",
      proof: Buffer.from("proof"),
      verifyingKeyRef: { backend: "halo2/ipa", name: "vk_unshield" },
    },
    rootHint: Buffer.alloc(32, 0x66),
  });
  const payload = encodeAndDecode(instruction).zk.Unshield;
  assert.equal(payload.public_amount, "18446744073709551616.25");
  assert.deepEqual(payload.root_hint, toByteArray(Buffer.alloc(32, 0x66)));
  assert.deepEqual(Object.keys(payload).sort(), [
    "asset",
    "inputs",
    "proof",
    "public_amount",
    "root_hint",
    "to",
  ]);
});

descriptorTest("Unshield builders and the pure codec reject the retired outputs field", () => {
  const options = {
    assetDefinitionId: ASSET_DEFINITION_ID,
    destinationAccountId: ACCOUNT_ID_INPUT,
    publicAmount: "1",
    inputs: [Buffer.alloc(32, 0x55)],
    proof: {
      backend: "halo2/ipa",
      proof: Buffer.from("proof"),
      verifyingKeyRef: { backend: "halo2/ipa", name: "vk_unshield" },
    },
  };
  assert.throws(
    () => buildUnshieldInstruction({ ...options, outputs: [Buffer.alloc(32, 0x77)] }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /unshield\.outputs.*not supported/i);
      return true;
    },
  );

  const canonical = canonicalizeClone(buildUnshieldInstruction(options));
  const encoded = withPureJsInstructionCodec(() => noritoEncodeInstruction(canonical));
  const decoded = withPureJsInstructionCodec(() => noritoDecodeInstruction(encoded));
  assert.deepEqual(decoded, canonical);
  assert.deepEqual(Object.keys(decoded.zk.Unshield).sort(), [
    "asset",
    "inputs",
    "proof",
    "public_amount",
    "root_hint",
    "to",
  ]);

  const stale = canonicalizeClone(canonical);
  stale.zk.Unshield.outputs = [Array(32).fill(0x77)];
  assert.throws(
    () => withPureJsInstructionCodec(() => noritoEncodeInstruction(stale)),
    /zk\.Unshield contains unknown field outputs/i,
  );

  assert.throws(
    () =>
      withPureJsInstructionCodec(() =>
        noritoDecodeInstruction(
          Buffer.from(LEGACY_UNSHIELD_WITH_OUTPUT_WIRE_BASE64, "base64"),
        ),
      ),
    /trailing bytes/i,
  );
});

test("native Unshield codec rejects the retired output-bearing shape", () => {
  const instruction = canonicalizeClone(
    buildUnshieldInstruction({
      assetDefinitionId: ASSET_DEFINITION_ID,
      destinationAccountId: ACCOUNT_ID_INPUT,
      publicAmount: "1",
      inputs: [Buffer.alloc(32, 0x55)],
      proof: {
        backend: "halo2/ipa",
        proof: Buffer.from("proof"),
        verifyingKeyRef: { backend: "halo2/ipa", name: "vk_unshield" },
      },
    }),
  );
  instruction.zk.Unshield.outputs = [Array(32).fill(0x77)];
  assert.throws(
    () => nativeBinding.noritoEncodeInstruction(JSON.stringify(instruction)),
    /outputs|unknown field/i,
  );
  assert.throws(
    () =>
      nativeBinding.noritoDecodeInstruction(
        Buffer.from(LEGACY_UNSHIELD_WITH_OUTPUT_WIRE_BASE64, "base64"),
      ),
    /decode|canonical|trailing|field/i,
  );
});
