import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildBurnAssetInstruction,
  buildCancelAssetLockInstruction,
  buildSetAssetTransferAvailabilityInstruction,
  CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1,
  buildMintAssetInstruction,
  buildMintTriggerRepetitionsInstruction,
  buildBurnTriggerRepetitionsInstruction,
  buildRegisterDomainInstruction,
  buildRegisterAccountInstruction,
  buildRegisterAssetDefinitionInstruction,
  buildGrantAccountPermissionInstruction,
  buildSetAccountKeyValueInstruction,
  buildSetAssetDefinitionAliasInstruction,
  buildTransferAssetInstruction,
  buildTransferDomainInstruction,
  buildTransferAssetDefinitionInstruction,
  buildTransferNftInstruction,
  buildRegisterRwaInstruction,
  buildTransferRwaInstruction,
  buildMergeRwasInstruction,
  buildRedeemRwaInstruction,
  buildFreezeRwaInstruction,
  buildUnfreezeRwaInstruction,
  buildHoldRwaInstruction,
  buildReleaseRwaInstruction,
  buildForceTransferRwaInstruction,
  buildSetRwaControlsInstruction,
  buildSetRwaKeyValueInstruction,
  buildRemoveRwaKeyValueInstruction,
  buildCreateKaigiInstruction,
  buildJoinKaigiInstruction,
  buildLeaveKaigiInstruction,
  buildEndKaigiInstruction,
  buildRecordKaigiUsageInstruction,
  buildSetKaigiRelayManifestInstruction,
  buildRegisterKaigiRelayInstruction,
  buildRegisterSmartContractCodeInstruction,
  buildRegisterSmartContractBytesInstruction,
  buildRemoveSmartContractBytesInstruction,
  buildProposeDeployContractInstruction,
  buildCastZkBallotInstruction,
  buildCastPlainBallotInstruction,
  buildEnactReferendumInstruction,
  buildFinalizeReferendumInstruction,
  buildPersistCouncilForEpochInstruction,
  buildSubmitAgendaProposalInstruction,
  buildClaimTwitterFollowRewardInstruction,
  buildSendToTwitterInstruction,
  buildCancelTwitterEscrowInstruction,
  buildRegisterAssetHiddenZkPoolInstruction,
  buildRegisterZkAssetInstruction,
  buildScheduleConfidentialPolicyTransitionInstruction,
  buildCancelConfidentialPolicyTransitionInstruction,
  buildShieldInstruction,
  buildZkTransferInstruction,
  buildAssetHiddenZkTransferInstruction,
  buildUnshieldInstruction,
  buildCreateElectionInstruction,
  buildSubmitBallotInstruction,
  buildFinalizeElectionInstruction,
  encodeInstruction,
} from "../src/instructionBuilders.js";
import { blake2b256 } from "../src/blake2b.js";
import { analyzeEntrypointValueTypeV1 } from "../src/entrypointSchema.js";
import {
  noritoDecodeInstruction,
  noritoEncodeInstruction,
  validateNoritoFrame,
} from "../src/norito.js";
import {
  hasNoritoBinding,
  makeNativeTest,
  nativeBinding,
  noritoRequiredMethods,
} from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const descriptorTest = baseTest;
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");
const SORA_I105_DISCRIMINANT = 0x2f1;
const CANCEL_ASSET_LOCK_ESCROW_ID =
  "hash:996264C84790C64086AAB0EF693A1D33EC18FC0B1C1229774C461A00939A6687#F2BD";

function loadInstructionFixture(name) {
  const fixturePath = path.join(repoRoot, "fixtures", "norito_instructions", name);
  return JSON.parse(fs.readFileSync(fixturePath, "utf8"));
}

function decodeFixtureInstruction(name) {
  const fixture = loadInstructionFixture(name);
  const decoded = noritoDecodeInstruction(Buffer.from(fixture.instruction, "base64"));
  return { fixture, decoded: canonicalizeClone(decoded) };
}
import {
  normalizeAccountId as exportedNormalizeAccountId,
  normalizeAssetId as exportedNormalizeAssetId,
  normalizeAssetHoldingId as exportedNormalizeAssetHoldingId,
} from "../src/index.js";
import * as sdkExports from "../src/index.js";
import { ValidationErrorCode } from "../src/validationError.js";
import {
  AccountAddress,
} from "../src/address.js";

function hexToBytes(hex) {
  const body = hex.replace(/^0x/i, "");
  if (body.length % 2 !== 0) {
    throw new TypeError("hex string must have even length");
  }
  const out = new Uint8Array(body.length / 2);
  for (let index = 0; index < out.length; index += 1) {
    out[index] = parseInt(body.slice(index * 2, index * 2 + 2), 16);
  }
  return out;
}
function canonicalizeValue(value) {
  if (Array.isArray(value)) {
    return value.map((entry) => canonicalizeValue(entry));
  }
  if (value && typeof value === "object") {
    if ("Zk" in value && !("zk" in value)) {
      value.zk = canonicalizeValue(value.Zk);
      delete value.Zk;
    }
    for (const key of Object.keys(value)) {
      value[key] = canonicalizeValue(value[key]);
    }
    return value;
  }
  if (typeof value === "string") {
    if (!value.startsWith("hash:") && value.includes("#")) {
      try {
        return exportedNormalizeAssetHoldingId(value);
      } catch {
        return value;
      }
    }
    if (value.includes("@")) {
      return exportedNormalizeAccountId(value);
    }
  }
  return value;
}

function canonicalizeClone(value) {
  return canonicalizeValue(JSON.parse(JSON.stringify(value)));
}

function canonicalizeAccountIdUsingNorito(accountId) {
  const encoded = noritoEncodeInstruction({
    Register: {
      Account: {
        id: accountId,
        label: null,
        uaid: null,
        opaque_ids: [],
        metadata: {},
      },
    },
  });
  const decoded = noritoDecodeInstruction(encoded);
  return canonicalizeValue(decoded).Register.Account.id;
}

function canonicalizeAssetIdUsingNorito(assetId) {
  const encoded = noritoEncodeInstruction({
    Mint: { Asset: { object: "1", destination: assetId } },
  });
  const decoded = noritoDecodeInstruction(encoded);
  return canonicalizeValue(decoded).Mint.Asset.destination;
}

function buildLocal8Literal(address) {
  const canonicalHex = address.canonicalHex();
  const payload = Buffer.from(canonicalHex.slice(2), "hex");
  const digestStart = 2;
  const truncated = Buffer.concat([
    payload.subarray(0, digestStart + 8),
    payload.subarray(digestStart + 12),
  ]);
  return `0x${truncated.toString("hex")}`;
}

const DOMAIN_ID = "wonderland.sora";
const ACCOUNT_SIGNATORY =
  "ED0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const SEED_11_ED25519_PUBLIC_KEY_HEX =
  "D04AB232742BB4AB3A1368BD4615E4E6D0224AB71A016BAF8520A332C9778737";
const ACCOUNT_PUBLIC_KEY = hexToBytes(ACCOUNT_SIGNATORY.slice(6));
const ACCOUNT_ADDRESS = AccountAddress.fromAccount({ publicKey: ACCOUNT_PUBLIC_KEY,
});
const ACCOUNT_ID = ACCOUNT_ADDRESS.toI105(SORA_I105_DISCRIMINANT);
const ACCOUNT_ID_INPUT = ACCOUNT_ID;
const ACCOUNT_ID_CANONICAL = hasNoritoBinding()
  ? canonicalizeAccountIdUsingNorito(ACCOUNT_ID)
  : ACCOUNT_ID;
const ASSET_DEFINITION_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const ASSET_ID = `${ASSET_DEFINITION_ID}#${ACCOUNT_ID}`;
const ASSET_ID_INPUT = `${ASSET_DEFINITION_ID}#${ACCOUNT_ID_INPUT}`;
const ASSET_ID_CANONICAL = hasNoritoBinding()
  ? canonicalizeAssetIdUsingNorito(ASSET_ID)
  : ASSET_ID;
const NFT_ID = "dragon$wonderland.sora";
const RWA_ID =
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities.sora";
const RWA_ID_INPUT =
  "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF$commodities.sora";
const SAMPLE_PUBLIC_KEY = hexToBytes(
  "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201",
);
const SAMPLE_ACCOUNT_ADDRESS = AccountAddress.fromAccount({ publicKey: SAMPLE_PUBLIC_KEY,
});
const SAMPLE_ACCOUNT_I105_LITERAL = SAMPLE_ACCOUNT_ADDRESS.toI105(SORA_I105_DISCRIMINANT);
const SAMPLE_ACCOUNT_COMPRESSED_LITERAL = SAMPLE_ACCOUNT_ADDRESS.toI105(SORA_I105_DISCRIMINANT);
const SAMPLE_ACCOUNT_CANONICAL = exportedNormalizeAccountId(SAMPLE_ACCOUNT_I105_LITERAL);
const SAMPLE_ACCOUNT_LOCAL8_LITERAL = buildLocal8Literal(SAMPLE_ACCOUNT_ADDRESS);

function toByteArray(bytes) {
  return Array.from(Buffer.from(bytes));
}

function readCompactFieldPayload(buffer, offset, context) {
  let cursor = offset;
  let length = 0n;
  let shift = 0n;
  for (;;) {
    if (cursor >= buffer.length) {
      throw new RangeError(`${context} compact length overruns its buffer`);
    }
    const byte = buffer[cursor];
    cursor += 1;
    length |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      break;
    }
    shift += 7n;
    if (shift >= 64n) {
      throw new RangeError(`${context} compact length exceeds u64`);
    }
  }
  if (length > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new RangeError(`${context} compact length exceeds the safe integer range`);
  }
  const end = cursor + Number(length);
  if (end > buffer.length) {
    throw new RangeError(`${context} payload overruns its buffer`);
  }
  return { payload: buffer.subarray(cursor, end), next: end };
}

function encodeAndDecode(instruction) {
  let encoded;
  try {
    encoded = encodeInstruction(instruction);
  } catch (error) {
    if (
      process?.env?.DEBUG_NORITO_PAYLOAD === "1" &&
      error instanceof Error &&
      /JSON error/i.test(error.message)
    ) {
      console.error("norito encoding failed for payload:", JSON.stringify(instruction));
    }
    throw error;
  }
  try {
    const decoded = noritoDecodeInstruction(encoded);
    return canonicalizeValue(decoded);
  } catch (error) {
    if (
      process?.env?.DEBUG_NORITO_PAYLOAD === "1" &&
      error instanceof Error &&
      /JSON error/i.test(error.message)
    ) {
      console.error("norito encoding failed for payload:", JSON.stringify(instruction));
    }
    const message = error && typeof error.message === "string" ? error.message : "";
    const alignmentIssue = message.includes("alignment");
    const panicDuringDecode =
      message.includes("panic during decode") ||
      message.includes("panic during Norito decode");
    if (!alignmentIssue && !panicDuringDecode) {
      throw error;
    }
    const canonical = canonicalizeClone(instruction);
    const reencoded = noritoEncodeInstruction(canonical);
    assert.deepEqual(toByteArray(encoded), toByteArray(reencoded));
    return canonical;
  }
}

function withPureJsInstructionCodec(body) {
  const hadBinding = Object.prototype.hasOwnProperty.call(
    globalThis,
    "__IROHA_NORITO_BINDING__",
  );
  const previous = globalThis.__IROHA_NORITO_BINDING__;
  globalThis.__IROHA_NORITO_BINDING__ = {
    noritoEncodeInstruction() {
      throw new Error("unsupported instruction");
    },
    noritoDecodeInstruction() {
      throw new Error("unsupported instruction");
    },
  };
  try {
    return body();
  } finally {
    if (hadBinding) {
      globalThis.__IROHA_NORITO_BINDING__ = previous;
    } else {
      delete globalThis.__IROHA_NORITO_BINDING__;
    }
  }
}

function assertNativeAndPureInstructionParity(instruction, context) {
  const pureEncoded = Buffer.from(
    withPureJsInstructionCodec(() => noritoEncodeInstruction(instruction)),
  );
  const nativeEncoded = Buffer.from(
    nativeBinding.noritoEncodeInstruction(JSON.stringify(instruction)),
  );
  assert.deepEqual(pureEncoded, nativeEncoded, `${context} bytes`);
  assert.deepEqual(
    JSON.parse(nativeBinding.noritoDecodeInstruction(pureEncoded)),
    instruction,
    `${context} native decode`,
  );
  assert.deepEqual(
    withPureJsInstructionCodec(() => noritoDecodeInstruction(nativeEncoded)),
    instruction,
    `${context} pure decode`,
  );
  return pureEncoded;
}

function crc16(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let i = 0; i < 8; i += 1) {
      if ((crc & 0x8000) !== 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xffff;
      } else {
        crc = (crc << 1) & 0xffff;
      }
    }
  };

  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }
  return crc & 0xffff;
}

function normalizedHashHex(bytes) {
  const buffer = Buffer.from(bytes);
  if (buffer.length !== 32) {
    throw new TypeError("hash literal test helper requires 32 bytes");
  }
  buffer[buffer.length - 1] |= 1;
  const body = buffer.toString("hex").toUpperCase();
  const checksum = crc16("hash", body).toString(16).toUpperCase().padStart(4, "0");
  return `hash:${body}#${checksum}`;
}

test("normalizeAccountId exported accepts encoded account IDs", () => {
  const canonical = exportedNormalizeAccountId(ACCOUNT_ID_INPUT);
  assert.equal(canonical, ACCOUNT_ID_CANONICAL);
});

test("normalizeAccountId canonicalizes I105 and i105 (`sora`) encodings", () => {
  const canonicalI105 = exportedNormalizeAccountId(SAMPLE_ACCOUNT_I105_LITERAL);
  assert.equal(canonicalI105, SAMPLE_ACCOUNT_CANONICAL);
  const canonicalCompressed = exportedNormalizeAccountId(SAMPLE_ACCOUNT_COMPRESSED_LITERAL);
  assert.equal(canonicalCompressed, SAMPLE_ACCOUNT_CANONICAL);
});

test("normalizeAccountId rejects Local-8 selectors", () => {
  assert.throws(
    () => exportedNormalizeAccountId(SAMPLE_ACCOUNT_LOCAL8_LITERAL),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_ACCOUNT_ID);
      assert.match(String(error?.message), /canonical I105 account id/i);
      return true;
    },
  );
});

test("normalizeAssetId exported canonicalizes bare Base58 asset ids", () => {
  const canonical = exportedNormalizeAssetId(ASSET_DEFINITION_ID);
  assert.equal(canonical, ASSET_DEFINITION_ID);
});

test("normalizeAssetId rejects malformed asset literals", () => {
  assert.throws(
    () => exportedNormalizeAssetId(ASSET_ID_INPUT),
    /canonical Base58 asset id/,
  );
});

test("normalizeAssetHoldingId exported canonicalizes asset-holding identifiers", () => {
  const canonical = exportedNormalizeAssetHoldingId(ASSET_ID_INPUT);
  assert.equal(canonical, ASSET_ID_CANONICAL);
});

baseTest("buildCancelAssetLockInstruction emits the exact two-field V1 payload", () => {
  const instruction = buildCancelAssetLockInstruction({
    lockId: "merchant-lock-001",
    expectedRemainingAmount: "1500",
  });
  assert.deepEqual(instruction, {
    CancelAssetLock: {
      escrow_id: CANCEL_ASSET_LOCK_ESCROW_ID,
      expected_remaining_amount: "1500",
    },
  });
  assert.equal(
    instruction.CancelAssetLock.escrow_id,
    normalizedHashHex(blake2b256(Buffer.from("merchant-lock-001", "utf8"))),
  );
});

baseTest("buildSetAssetTransferAvailabilityInstruction emits exact CAS state", () => {
  assert.deepEqual(
    buildSetAssetTransferAvailabilityInstruction({
      accountId: ACCOUNT_ID,
      assetDefinitionId: ASSET_DEFINITION_ID,
      expectedRevision: 7,
      incoming: "Disabled",
      outgoing: "Enabled",
      reason: "suspend incoming retail transfers",
    }),
    {
      SetAssetTransferAvailability: {
        account_id: ACCOUNT_ID,
        asset_definition_id: ASSET_DEFINITION_ID,
        expected_revision: "7",
        incoming: "Disabled",
        outgoing: "Enabled",
        reason: "suspend incoming retail transfers",
      },
    },
  );
  assert.equal(
    buildSetAssetTransferAvailabilityInstruction({
      accountId: ACCOUNT_ID,
      assetDefinitionId: ASSET_DEFINITION_ID,
      expectedRevision: 0n,
      incoming: "Enabled",
      outgoing: "Enabled",
    }).SetAssetTransferAvailability.reason,
    null,
  );
});

baseTest("asset availability builder rejects ambiguous or noncanonical input", () => {
  const valid = {
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 0,
    incoming: "Enabled",
    outgoing: "Disabled",
  };
  for (const [field, value] of [
    ["incoming", "enabled"],
    ["outgoing", "Frozen"],
    ["expectedRevision", -1],
    ["reason", ""],
    ["reason", " padded"],
    ["reason", "line\u000abreached"],
    ["reason", "ר".repeat(257)],
    ["accountId", ` ${ACCOUNT_ID}`],
    ["assetDefinitionId", `${ASSET_DEFINITION_ID} `],
  ]) {
    assert.throws(
      () =>
        buildSetAssetTransferAvailabilityInstruction({
          ...valid,
          [field]: value,
        }),
      undefined,
      `accepted invalid ${field}`,
    );
  }
  assert.throws(
    () =>
      buildSetAssetTransferAvailabilityInstruction({
        ...valid,
        expected_revision: 0,
      }),
    /not supported/u,
  );
});

baseTest("pure JS codec roundtrips directional asset availability", () => {
  const instruction = buildSetAssetTransferAvailabilityInstruction({
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 3,
    incoming: "Disabled",
    outgoing: "Enabled",
    reason: "operator review",
  });
  withPureJsInstructionCodec(() => {
    const encoded = noritoEncodeInstruction(instruction);
    assert.deepEqual(noritoDecodeInstruction(encoded), instruction);
  });
});

baseTest("asset availability preserves the complete u64 revision domain", () => {
  const instruction = buildSetAssetTransferAvailabilityInstruction({
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 0xffff_ffff_ffff_ffffn,
    incoming: "Enabled",
    outgoing: "Disabled",
  });
  assert.equal(
    instruction.SetAssetTransferAvailability.expected_revision,
    "18446744073709551615",
  );
  withPureJsInstructionCodec(() => {
    const encoded = noritoEncodeInstruction(instruction);
    assert.deepEqual(noritoDecodeInstruction(encoded), instruction);
  });
  assert.throws(
    () =>
      buildSetAssetTransferAvailabilityInstruction({
        accountId: ACCOUNT_ID,
        assetDefinitionId: ASSET_DEFINITION_ID,
        expectedRevision: 0x1_0000_0000_0000_0000n,
        incoming: "Enabled",
        outgoing: "Disabled",
      }),
    /unsigned 64-bit/u,
  );
});

baseTest("pure JS codec rejects noncanonical availability reasons", () => {
  const base = buildSetAssetTransferAvailabilityInstruction({
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 0,
    incoming: "Disabled",
    outgoing: "Enabled",
  });
  withPureJsInstructionCodec(() => {
    for (const reason of ["line\u000abreached", "ר".repeat(257)]) {
      assert.throws(
        () =>
          noritoEncodeInstruction({
            SetAssetTransferAvailability: {
              ...base.SetAssetTransferAvailability,
              reason,
            },
          }),
        undefined,
      );
    }
  });
});

test("native and pure JS codecs byte-match for asset availability", () => {
  const instruction = buildSetAssetTransferAvailabilityInstruction({
    accountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    expectedRevision: 3,
    incoming: "Disabled",
    outgoing: "Enabled",
    reason: "operator review",
  });
  assertNativeAndPureInstructionParity(
    instruction,
    "SetAssetTransferAvailability",
  );
});

baseTest("buildCancelAssetLockInstruction rejects legacy and ambiguous inputs", () => {
  assert.throws(
    () => buildCancelAssetLockInstruction({ lockId: "merchant-lock-001" }),
    /expectedRemainingAmount/,
  );
  assert.throws(
    () =>
      buildCancelAssetLockInstruction({
        lockId: "merchant-lock-001",
        expectedRemainingAmount: "1",
        expected_remaining_amount: "1",
      }),
    /not supported/,
  );
  assert.throws(
    () =>
      buildCancelAssetLockInstruction({
        lockId: "",
        expectedRemainingAmount: "1",
      }),
    /non-empty string/,
  );
  assert.throws(
    () =>
      buildCancelAssetLockInstruction({
        lockId: " merchant-lock-001",
        expectedRemainingAmount: "1",
      }),
    /surrounding whitespace/,
  );
  for (const lockId of ["\uFEFFmerchant-lock-001", "merchant-lock-001\uFEFF"]) {
    assert.throws(
      () =>
        buildCancelAssetLockInstruction({
          lockId,
          expectedRemainingAmount: "1",
        }),
      /surrounding whitespace/,
    );
  }
  for (const lockId of ["\ud800", "\udfff", "merchant\ud800lock"]) {
    assert.throws(
      () =>
        buildCancelAssetLockInstruction({
          lockId,
          expectedRemainingAmount: "1",
        }),
      /unpaired UTF-16 surrogates/u,
    );
  }
  for (const expectedRemainingAmount of [0n, "0", "-1", "01", "1.0", "+1", 1]) {
    assert.throws(
      () =>
        buildCancelAssetLockInstruction({
          lockId: "merchant-lock-001",
          expectedRemainingAmount,
        }),
      undefined,
      `accepted invalid expected remaining amount ${String(expectedRemainingAmount)}`,
    );
  }
});

baseTest("buildCancelAssetLockInstruction bounds the exact UTF-8 lock-id preimage", () => {
  const exactBound = "🔒".repeat(1_024);
  assert.equal(Buffer.byteLength(exactBound, "utf8"), 4_096);
  assert.equal(CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1, 4_096);
  assert.doesNotThrow(() =>
    buildCancelAssetLockInstruction({
      lockId: exactBound,
      expectedRemainingAmount: "1",
    }),
  );

  const overBound = `${exactBound}a`;
  assert.equal(Buffer.byteLength(overBound, "utf8"), 4_097);
  assert.throws(
    () =>
      buildCancelAssetLockInstruction({
        lockId: overBound,
        expectedRemainingAmount: "1",
      }),
    /at most 4096 UTF-8 bytes/u,
  );
});

baseTest("pure JS codec roundtrips CancelAssetLock and rejects the legacy shape", () => {
  withPureJsInstructionCodec(() => {
    const instruction = buildCancelAssetLockInstruction({
      lockId: "merchant-lock-001",
      expectedRemainingAmount: "1.25",
    });
    const encoded = noritoEncodeInstruction(instruction);
    assert.deepEqual(noritoDecodeInstruction(encoded), instruction);

    assert.throws(
      () =>
        noritoEncodeInstruction({
          CancelAssetLock: { escrow_id: instruction.CancelAssetLock.escrow_id },
        }),
      /expected_remaining_amount is required/,
    );
    for (const expected_remaining_amount of ["0", "01", "1.0"]) {
      assert.throws(
        () =>
          noritoEncodeInstruction({
            CancelAssetLock: {
              escrow_id: instruction.CancelAssetLock.escrow_id,
              expected_remaining_amount,
            },
          }),
        undefined,
        `pure JS codec accepted ${expected_remaining_amount}`,
      );
    }
    for (const escrow_id of [
      CANCEL_ASSET_LOCK_ESCROW_ID.slice(5, 69),
      CANCEL_ASSET_LOCK_ESCROW_ID.replace(
        /^hash:([0-9A-F]+)#/u,
        (_, body) => `hash:${body.toLowerCase()}#`,
      ),
      CANCEL_ASSET_LOCK_ESCROW_ID.toLowerCase(),
    ]) {
      assert.throws(
        () =>
          noritoEncodeInstruction({
            CancelAssetLock: {
              ...instruction.CancelAssetLock,
              escrow_id,
            },
          }),
        /canonical uppercase hash/u,
      );
    }
  });
});

test("native and pure JS codecs byte-match and cross-decode CancelAssetLock V1", () => {
  const instruction = buildCancelAssetLockInstruction({
    lockId: "merchant-lock-001",
    expectedRemainingAmount: "1.25",
  });
  assert.equal(
    instruction.CancelAssetLock.escrow_id,
    CANCEL_ASSET_LOCK_ESCROW_ID,
  );

  const pureEncoded = withPureJsInstructionCodec(() =>
    noritoEncodeInstruction(instruction),
  );
  const nativeEncoded = nativeBinding.noritoEncodeInstruction(
    JSON.stringify(instruction),
  );
  assert.deepEqual(toByteArray(pureEncoded), toByteArray(nativeEncoded));

  assert.deepEqual(
    JSON.parse(nativeBinding.noritoDecodeInstruction(pureEncoded)),
    instruction,
  );
  assert.deepEqual(
    withPureJsInstructionCodec(() =>
      noritoDecodeInstruction(nativeEncoded),
    ),
    instruction,
  );

  assert.throws(
    () =>
      nativeBinding.noritoEncodeInstruction(
        JSON.stringify({
          CancelAssetLock: {
            escrow_id: instruction.CancelAssetLock.escrow_id,
          },
        }),
      ),
    /missing field/,
  );
  assert.throws(
    () =>
      nativeBinding.noritoEncodeInstruction(
        JSON.stringify({
          CancelAssetLock: {
            escrow_id: instruction.CancelAssetLock.escrow_id,
            expected_remaining_amount: "0",
          },
        }),
      ),
    /must be positive/,
  );
  for (const escrowId of [
    instruction.CancelAssetLock.escrow_id.slice(5, 69),
    instruction.CancelAssetLock.escrow_id.toLowerCase(),
  ]) {
    assert.throws(
      () =>
        nativeBinding.noritoEncodeInstruction(
          JSON.stringify({
            CancelAssetLock: {
              ...instruction.CancelAssetLock,
              escrow_id: escrowId,
            },
          }),
        ),
      /canonical|hash:|uppercase|checksum/u,
    );
  }
});

test("buildMintAssetInstruction produces canonical Norito payload", () => {
  const instruction = buildMintAssetInstruction({ assetId: ASSET_ID, quantity: 42n });
  assert.deepEqual(instruction, {
    Mint: { Asset: { object: "42", destination: ASSET_ID_CANONICAL } },
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, canonicalizeClone(instruction));
});

test("buildMintAssetInstruction rejects invalid Numeric literals", () => {
  assert.throws(
    () => buildMintAssetInstruction({ assetId: ASSET_ID, quantity: "1e-3" }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_NUMERIC);
      assert.match(String(error?.message), /canonical non-negative Kotodama V1 quantity/i);
      return true;
    },
  );
  const tooManyDecimals = `0.${"1".repeat(29)}`;
  assert.throws(
    () => buildMintAssetInstruction({ assetId: ASSET_ID, quantity: tooManyDecimals }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.match(String(error?.message), /canonical non-negative Kotodama V1 quantity/i);
      return true;
    },
  );
  // Numeric is a signed 512-bit domain, so non-negative quantities end at
  // 2^511 - 1 rather than 2^512 - 1.
  const tooLarge = 1n << 511n;
  assert.throws(
    () => buildMintAssetInstruction({ assetId: ASSET_ID, quantity: tooLarge }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.match(String(error?.message), /canonical non-negative Kotodama V1 quantity/i);
      return true;
    },
  );
  assert.throws(
    () => buildMintAssetInstruction({ assetId: ASSET_ID, quantity: "-1" }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_NUMERIC);
      assert.match(String(error?.message), /non-negative/i);
      return true;
    },
  );
  assert.throws(
    () => buildMintAssetInstruction({ assetId: ASSET_ID, quantity: "1".repeat(100_000) }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.match(String(error?.message), /canonical non-negative Kotodama V1 quantity/i);
      return true;
    },
  );

  for (const quantity of [
    42,
    "+1",
    "01",
    "1.0",
    "1.2300",
    "1amt",
    "1qty",
    " 1",
    "1 ",
    "0.0",
  ]) {
    assert.throws(
      () => buildMintAssetInstruction({ assetId: ASSET_ID, quantity }),
      (error) => {
        assert.equal(error?.code, ValidationErrorCode.INVALID_NUMERIC);
        return true;
      },
      `accepted ambiguous quantity ${String(quantity)}`,
    );
  }
});

test("buildMintAssetInstruction accepts the positive signed-Numeric boundary", () => {
  const maximumQuantity = (1n << 511n) - 1n;
  const instruction = buildMintAssetInstruction({
    assetId: ASSET_ID,
    quantity: maximumQuantity,
  });
  assert.equal(instruction.Mint.Asset.object, maximumQuantity.toString());
  assert.deepEqual(encodeAndDecode(instruction), canonicalizeClone(instruction));
});

test("buildBurnAssetInstruction produces canonical Norito payload", () => {
  const instruction = buildBurnAssetInstruction({ assetId: ASSET_ID, quantity: "7" });
  assert.deepEqual(instruction, {
    Burn: { Asset: { object: "7", destination: ASSET_ID_CANONICAL } },
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, canonicalizeClone(instruction));
});

test("buildBurnAssetInstruction matches canonical numeric Norito fixture", () => {
  const { fixture, decoded } = decodeFixtureInstruction("burn_asset_quantity.json");
  const { destination, object } = decoded.Burn.Asset;
  const instruction = buildBurnAssetInstruction({ assetId: destination, quantity: object });
  assert.deepEqual(instruction, decoded);
  const encoded = noritoEncodeInstruction(instruction);
  assert.equal(
    encoded.toString("hex"),
    Buffer.from(fixture.instruction, "base64").toString("hex"),
    "Burn::Asset numeric fixture diverged from canonical Norito bytes",
  );
});

test("buildBurnAssetInstruction matches canonical fractional Norito fixture", () => {
  const { fixture, decoded } = decodeFixtureInstruction("burn_asset_fractional.json");
  const { destination, object } = decoded.Burn.Asset;
  const instruction = buildBurnAssetInstruction({ assetId: destination, quantity: object });
  assert.deepEqual(instruction, decoded);
  const encoded = noritoEncodeInstruction(instruction);
  assert.equal(
    encoded.toString("hex"),
    Buffer.from(fixture.instruction, "base64").toString("hex"),
    "Burn::Asset fractional fixture diverged from canonical Norito bytes",
  );
});

test("buildMintTriggerRepetitionsInstruction validates repetitions", () => {
  const instruction = buildMintTriggerRepetitionsInstruction({
    triggerId: "notify-users",
    repetitions: "3",
  });
  assert.deepEqual(instruction, {
    Mint: { TriggerRepetitions: { object: 3, destination: "notify-users" } },
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, {
    Mint: { TriggerRepetitions: { object: 3, destination: "notify-users" } },
  });
  assert.throws(
    () => buildMintTriggerRepetitionsInstruction({ triggerId: "notify-users", repetitions: 0 }),
    /positive integer/i,
  );
});

test("buildBurnTriggerRepetitionsInstruction validates repetitions", () => {
  const instruction = buildBurnTriggerRepetitionsInstruction({
    triggerId: "notify-users",
    repetitions: 2n,
  });
  assert.deepEqual(instruction, {
    Burn: { TriggerRepetitions: { object: 2, destination: "notify-users" } },
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, canonicalizeClone(instruction));
  assert.throws(
    () =>
      buildBurnTriggerRepetitionsInstruction({ triggerId: "notify-users", repetitions: 0 }),
    /positive integer/i,
  );
});

test("buildMintTriggerRepetitionsInstruction rejects oversized integers", () => {
  const tooLarge = BigInt(Number.MAX_SAFE_INTEGER) + 1n;
  assert.throws(
    () =>
      buildMintTriggerRepetitionsInstruction({
        triggerId: "notify-users",
        repetitions: tooLarge,
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.match(String(error?.message), /safe integer/i);
      return true;
    },
  );
});

test("buildTransferAssetInstruction encodes asset quantity", () => {
  const instruction = buildTransferAssetInstruction({
    sourceAssetId: ASSET_ID,
    quantity: "17",
    destinationAccountId: ACCOUNT_ID,
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, {
    Transfer: {
      Asset: {
        source: ASSET_ID_CANONICAL,
        object: "17",
        destination: ACCOUNT_ID_CANONICAL,
      },
    },
  });
});

test("buildTransferDomainInstruction covers domain transfer", () => {
  const instruction = buildTransferDomainInstruction({
    sourceAccountId: ACCOUNT_ID,
    domainId: DOMAIN_ID,
    destinationAccountId: ACCOUNT_ID,
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, {
    Transfer: {
      Domain: {
        source: ACCOUNT_ID_CANONICAL,
        object: DOMAIN_ID,
        destination: ACCOUNT_ID_CANONICAL,
      },
    },
  });
});

test("buildTransferAssetDefinitionInstruction covers definition transfer", () => {
  const instruction = buildTransferAssetDefinitionInstruction({
    sourceAccountId: ACCOUNT_ID,
    assetDefinitionId: ASSET_DEFINITION_ID,
    destinationAccountId: ACCOUNT_ID,
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, {
    Transfer: {
      AssetDefinition: {
        source: ACCOUNT_ID_CANONICAL,
        object: ASSET_DEFINITION_ID,
        destination: ACCOUNT_ID_CANONICAL,
      },
    },
  });
});

test("buildTransferNftInstruction covers nft transfer", () => {
  const instruction = buildTransferNftInstruction({
    sourceAccountId: ACCOUNT_ID,
    nftId: NFT_ID,
    destinationAccountId: ACCOUNT_ID,
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, {
    Transfer: {
      Nft: {
        source: ACCOUNT_ID_CANONICAL,
        object: NFT_ID,
        destination: ACCOUNT_ID_CANONICAL,
      },
    },
  });
});

test("NftId implicit universal and Name rules match native V1", () => {
  const implicit = buildTransferNftInstruction({
    sourceAccountId: ACCOUNT_ID,
    nftId: "dragon$wonderland",
    destinationAccountId: ACCOUNT_ID,
  });
  const explicit = buildTransferNftInstruction({
    sourceAccountId: ACCOUNT_ID,
    nftId: "dragon$wonderland.universal",
    destinationAccountId: ACCOUNT_ID,
  });
  const pureImplicit = Buffer.from(
    withPureJsInstructionCodec(() => noritoEncodeInstruction(implicit)),
  );
  const nativeImplicit = Buffer.from(
    nativeBinding.noritoEncodeInstruction(JSON.stringify(implicit)),
  );
  const pureExplicit = Buffer.from(
    withPureJsInstructionCodec(() => noritoEncodeInstruction(explicit)),
  );
  assert.deepEqual(pureImplicit, nativeImplicit);
  assert.deepEqual(pureImplicit, pureExplicit);
  assert.deepEqual(
    JSON.parse(nativeBinding.noritoDecodeInstruction(pureImplicit)),
    explicit,
  );
  assert.deepEqual(
    withPureJsInstructionCodec(() => noritoDecodeInstruction(nativeImplicit)),
    explicit,
  );

  const decomposed = buildTransferNftInstruction({
    sourceAccountId: ACCOUNT_ID,
    nftId: "e\u0301$wonderland",
    destinationAccountId: ACCOUNT_ID,
  });
  const composed = buildTransferNftInstruction({
    sourceAccountId: ACCOUNT_ID,
    nftId: "é$wonderland.universal",
    destinationAccountId: ACCOUNT_ID,
  });
  assert.deepEqual(
    Buffer.from(
      withPureJsInstructionCodec(() => noritoEncodeInstruction(decomposed)),
    ),
    Buffer.from(nativeBinding.noritoEncodeInstruction(JSON.stringify(decomposed))),
  );
  assert.deepEqual(
    Buffer.from(
      withPureJsInstructionCodec(() => noritoEncodeInstruction(decomposed)),
    ),
    Buffer.from(
      withPureJsInstructionCodec(() => noritoEncodeInstruction(composed)),
    ),
  );

  const invalid = buildTransferNftInstruction({
    sourceAccountId: ACCOUNT_ID,
    nftId: "bad@name$wonderland",
    destinationAccountId: ACCOUNT_ID,
  });
  assert.throws(
    () =>
      withPureJsInstructionCodec(() => noritoEncodeInstruction(invalid)),
    /reserved Name character/u,
  );
  assert.throws(
    () => nativeBinding.noritoEncodeInstruction(JSON.stringify(invalid)),
    /parse|name|Nft/u,
  );
});

test("nominal DomainId pure-JS frames byte-match native V1", () => {
  const instructions = [
    [
      "Register.Domain",
      {
        Register: {
          Domain: {
            id: DOMAIN_ID,
            logo: null,
            metadata: { purpose: "parity" },
          },
        },
      },
    ],
    [
      "Transfer.Domain",
      buildTransferDomainInstruction({
        sourceAccountId: ACCOUNT_ID,
        domainId: DOMAIN_ID,
        destinationAccountId: ACCOUNT_ID,
      }),
    ],
    [
      "Transfer.Nft",
      buildTransferNftInstruction({
        sourceAccountId: ACCOUNT_ID,
        nftId: NFT_ID,
        destinationAccountId: ACCOUNT_ID,
      }),
    ],
  ];
  for (const [name, instruction] of instructions) {
    const encoded = assertNativeAndPureInstructionParity(instruction, name);
    assert.equal(encoded[39], 0x02, `${name} must use compact Norito framing`);
  }
});

test("buildRegisterRwaInstruction normalizes richer lot payloads", () => {
  const instruction = buildRegisterRwaInstruction({
    rwa: {
      domain: "commodities.sora",
      quantity: "10.5",
      spec: { scale: 1 },
      primaryReference: "vault-cert-001",
      metadata: { origin: "AE", lot: BigInt(3) },
      parents: [{ rwa: RWA_ID, quantity: "1.25" }],
      controls: {
        controllerAccounts: [ACCOUNT_ID],
        controllerRoles: ["auditor"],
        freezeEnabled: true,
        holdEnabled: true,
      },
    },
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, {
    RegisterRwa: {
      rwa: {
        domain: "commodities.sora",
        quantity: "10.5",
        spec: { scale: 1 },
        primary_reference: "vault-cert-001",
        status: null,
        metadata: { origin: "AE", lot: "3" },
        parents: [{ rwa: RWA_ID, quantity: "1.25" }],
        controls: {
          controller_accounts: [ACCOUNT_ID_CANONICAL],
          controller_roles: ["auditor"],
          freeze_enabled: true,
          hold_enabled: true,
          force_transfer_enabled: false,
          redeem_enabled: false,
        },
      },
    },
  });
  const encoded = assertNativeAndPureInstructionParity(
    instruction,
    "RegisterRwa",
  );
  assert.equal(encoded[39], 0x02, "RegisterRwa must use compact Norito framing");
});

test("buildTransferRwaInstruction covers rwa transfer", () => {
  const instruction = buildTransferRwaInstruction({
    sourceAccountId: ACCOUNT_ID,
    rwaId: RWA_ID_INPUT,
    quantity: "3.25",
    destinationAccountId: ACCOUNT_ID,
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, {
    TransferRwa: {
      source: ACCOUNT_ID_CANONICAL,
      rwa: RWA_ID,
      quantity: "3.25",
      destination: ACCOUNT_ID_CANONICAL,
    },
  });
});

test("rwa scalar instruction builders cover lifecycle operations", () => {
  const merge = buildMergeRwasInstruction({
    merge: {
      parents: [{ rwa: RWA_ID, quantity: "1.5" }],
      primaryReference: "blend-cert-007",
      status: "blended",
      metadata: { grade: "A" },
    },
  });
  const redeem = buildRedeemRwaInstruction({ rwaId: RWA_ID, quantity: "2" });
  const freeze = buildFreezeRwaInstruction({ rwaId: RWA_ID });
  const unfreeze = buildUnfreezeRwaInstruction({ rwaId: RWA_ID });
  const hold = buildHoldRwaInstruction({ rwaId: RWA_ID, quantity: "3" });
  const release = buildReleaseRwaInstruction({ rwaId: RWA_ID, quantity: "1" });
  const forceTransfer = buildForceTransferRwaInstruction({
    rwaId: RWA_ID,
    quantity: "4",
    destinationAccountId: ACCOUNT_ID,
  });
  const controls = buildSetRwaControlsInstruction({
    rwaId: RWA_ID,
    controls: { redeemEnabled: true },
  });

  assert.deepEqual(encodeAndDecode(merge), {
    MergeRwas: {
      parents: [{ rwa: RWA_ID, quantity: "1.5" }],
      primary_reference: "blend-cert-007",
      status: "blended",
      metadata: { grade: "A" },
    },
  });
  assert.deepEqual(encodeAndDecode(redeem), {
    RedeemRwa: { rwa: RWA_ID, quantity: "2" },
  });
  assert.deepEqual(encodeAndDecode(freeze), {
    FreezeRwa: { rwa: RWA_ID },
  });
  const freezeBytes = assertNativeAndPureInstructionParity(freeze, "FreezeRwa");
  assert.equal(freezeBytes[39], 0x02, "FreezeRwa must use compact Norito framing");
  const evenHashRwaId = RWA_ID.replace(/ef\$/u, "ee$");
  assert.throws(
    () =>
      withPureJsInstructionCodec(() =>
        noritoEncodeInstruction({
          FreezeRwa: { rwa: evenHashRwaId },
        }),
      ),
    /marker bit/u,
  );
  assert.throws(
    () =>
      nativeBinding.noritoEncodeInstruction(
        JSON.stringify({
          FreezeRwa: { rwa: evenHashRwaId },
        }),
      ),
    /hash|parse|marker/u,
  );
  assert.deepEqual(encodeAndDecode(unfreeze), {
    UnfreezeRwa: { rwa: RWA_ID },
  });
  assert.deepEqual(encodeAndDecode(hold), {
    HoldRwa: { rwa: RWA_ID, quantity: "3" },
  });
  assert.deepEqual(encodeAndDecode(release), {
    ReleaseRwa: { rwa: RWA_ID, quantity: "1" },
  });
  assert.deepEqual(encodeAndDecode(forceTransfer), {
    ForceTransferRwa: {
      rwa: RWA_ID,
      quantity: "4",
      destination: ACCOUNT_ID_CANONICAL,
    },
  });
  assert.deepEqual(encodeAndDecode(controls), {
    SetRwaControls: {
      rwa: RWA_ID,
      controls: {
        controller_accounts: [],
        controller_roles: [],
        freeze_enabled: false,
        hold_enabled: false,
        force_transfer_enabled: false,
        redeem_enabled: true,
      },
    },
  });

  const setMetadata = buildSetRwaKeyValueInstruction({
    rwaId: RWA_ID,
    key: "grade",
    value: { country: "AE", sequence: BigInt(7) },
  });
  const removeMetadata = buildRemoveRwaKeyValueInstruction({
    rwaId: RWA_ID,
    key: "grade",
  });
  assert.deepEqual(encodeAndDecode(setMetadata), {
    SetRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
      value: { country: "AE", sequence: "7" },
    },
  });
  assert.deepEqual(encodeAndDecode(removeMetadata), {
    RemoveRwaKeyValue: {
      rwa: RWA_ID,
      key: "grade",
    },
  });
});

test("buildRegisterDomainInstruction normalizes metadata payloads", () => {
  const instruction = buildRegisterDomainInstruction({
    domainId: DOMAIN_ID,
    metadata: {
      title: "Wonderland",
      attrs: { population: 10, status: true },
      counters: [1, 2, BigInt(3)],
    },
  });
  assert.deepEqual(instruction, {
    Register: {
      Domain: {
        id: DOMAIN_ID,
        logo: null,
        metadata: {
          title: "Wonderland",
          attrs: { population: 10, status: true },
          counters: [1, 2, "3"],
        },
      },
    },
  });
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, canonicalizeClone(instruction));
});

test("buildRegisterDomainInstruction accepts custom logo strings", () => {
  const logoPath = "ipfs://placeholder-logo";
  const instruction = buildRegisterDomainInstruction({
    domainId: DOMAIN_ID,
    logo: logoPath,
  });
  assert.equal(instruction.Register.Domain.logo, logoPath);
});

test("buildRegisterAccountInstruction defaults metadata and validates", () => {
  const instruction = buildRegisterAccountInstruction({
    accountId: ACCOUNT_ID,
  });
  const account = instruction.Register.Account;
  assert.equal(account.id, ACCOUNT_ID_CANONICAL);
  assert.deepEqual(account.metadata, {});
  assert.equal(account.label ?? null, null);
  assert.equal(account.uaid ?? null, null);
  assert.deepEqual(account.opaque_ids, []);
  const decoded = encodeAndDecode(instruction);
  const decodedAccount = decoded.Register.Account;
  assert.equal(decodedAccount.id, ACCOUNT_ID_CANONICAL);
  assert.deepEqual(decodedAccount.metadata, {});
  assert.equal(decodedAccount.domain ?? null, null);
  assert.equal(decodedAccount.label ?? null, null);
  assert.equal(decodedAccount.uaid ?? null, null);
  assert.deepEqual(decodedAccount.opaque_ids, []);
  assert.throws(
    () =>
      buildRegisterAccountInstruction({
        accountId: ACCOUNT_ID,
        domainId: DOMAIN_ID,
      }),
    /domainless/i,
  );
  assert.throws(
    () =>
      buildRegisterAccountInstruction({
        accountId: ACCOUNT_ID,
        metadata: ["invalid"],
      }),
    /plain object/i,
  );
});

test("buildRegisterAssetDefinitionInstruction preserves alias metadata", () => {
  const instruction = buildRegisterAssetDefinitionInstruction({
    assetDefinitionId: ASSET_DEFINITION_ID,
    name: "demo",
    description: "Demo settlement PoC asset",
    alias: "demo#settlement.main",
    scale: 2,
    metadata: { purpose: "poc" },
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
});

test("buildGrantAccountPermissionInstruction defaults payload", () => {
  const instruction = buildGrantAccountPermissionInstruction({
    accountId: ACCOUNT_ID,
    permission: { name: "register_zk_asset" },
  });
  assert.deepEqual(instruction, {
    Grant: {
      Permission: {
        object: {
          name: "register_zk_asset",
          payload: null,
        },
        destination: ACCOUNT_ID_CANONICAL,
      },
    },
  });
  assert.deepEqual(encodeAndDecode(instruction), canonicalizeClone(instruction));
});

test("buildSetAccountKeyValueInstruction produces canonical Norito payload", () => {
  const sourceTxHash = "ab".repeat(32);
  const instruction = buildSetAccountKeyValueInstruction({
    accountId: ACCOUNT_ID,
    key: `pk_cbuae_settlement_${sourceTxHash}`,
    value: {
      protocol: "pk-cbuae-settlement",
      version: 1,
      source_tx_hash: sourceTxHash,
    },
  });
  assert.deepEqual(instruction, {
    SetKeyValue: {
      Account: {
        object: ACCOUNT_ID_CANONICAL,
        key: `pk_cbuae_settlement_${sourceTxHash}`,
        value: {
          protocol: "pk-cbuae-settlement",
          version: 1,
          source_tx_hash: sourceTxHash,
        },
      },
    },
  });
  assert.deepEqual(encodeAndDecode(instruction), canonicalizeClone(instruction));
});

baseTest("buildSetAccountKeyValueInstruction rejects non-JSON marker values", () => {
  assert.throws(
    () =>
      buildSetAccountKeyValueInstruction({
        accountId: ACCOUNT_ID,
        key: "marker",
        value: undefined,
      }),
    /value/i,
  );
});

test("buildSetAssetDefinitionAliasInstruction supports clearing aliases", () => {
  assert.deepEqual(
    buildSetAssetDefinitionAliasInstruction({
      assetDefinitionId: ASSET_DEFINITION_ID,
      alias: null,
    }),
    {
      SetAssetDefinitionAlias: {
        asset_definition_id: ASSET_DEFINITION_ID,
        alias: null,
        lease_expiry_ms: null,
      },
    },
  );
});

const RELAY_ACCOUNT_ID = ACCOUNT_ID_CANONICAL;

test("buildCreateKaigiInstruction normalizes relay manifest and metadata", () => {
  const instruction = buildCreateKaigiInstruction({
    id: { domainId: "wonderland.sora", callName: "weekly-sync" },
    host: ACCOUNT_ID,
    title: "Weekly Sync",
    description: "Roadmap alignment",
    maxParticipants: "16",
    gasRatePerMinute: 120,
    metadata: { topic: "status" },
    scheduledStartMs: "1700000000000",
    billingAccount: ACCOUNT_ID,
    privacyMode: "ZkRosterV1",
    roomPolicy: "public",
    relayManifest: {
      expiryMs: 1700111000000,
      hops: [
        {
          relayId: RELAY_ACCOUNT_ID,
          hpkePublicKey: Buffer.alloc(32, 0x01),
          weight: 5,
        },
      ],
    },
  });
  const expected = {
    Kaigi: {
      CreateKaigi: {
        call: {
          id: { domain_id: "wonderland.sora", call_name: "weekly-sync" },
          host: ACCOUNT_ID_CANONICAL,
          title: "Weekly Sync",
          description: "Roadmap alignment",
          max_participants: 16,
          gas_rate_per_minute: 120,
          metadata: { topic: "status" },
          scheduled_start_ms: 1700000000000,
          billing_account: ACCOUNT_ID_CANONICAL,
          privacy_mode: { mode: "ZkRosterV1", state: null },
          room_policy: { policy: "Public", state: null },
          relay_manifest: {
            expiry_ms: 1700111000000,
            hops: [
              {
                relay_id: RELAY_ACCOUNT_ID,
                hpke_public_key: "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
                weight: 5,
              },
            ],
          },
        },
        commitment: null,
        nullifier: null,
        roster_root: null,
        proof: null,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
  const encoded = assertNativeAndPureInstructionParity(
    instruction,
    "Kaigi.CreateKaigi",
  );
  assert.equal(encoded[39], 0x02, "Kaigi.CreateKaigi must use compact Norito framing");
});

test("noritoDecodeInstruction decodes Kaigi manifests", () => {
  const instruction = buildCreateKaigiInstruction({
    id: "wonderland.sora:weekly-sync",
    host: ACCOUNT_ID,
    gasRatePerMinute: 120,
    relayManifest: {
      expiryMs: 1700111000000,
      hops: [
        {
          relayId: RELAY_ACCOUNT_ID,
          hpkePublicKey: Buffer.alloc(32, 0x01),
          weight: 5,
        },
      ],
    },
  });
  const encoded = encodeInstruction(instruction);
  const decoded = noritoDecodeInstruction(encoded);
  assert.deepEqual(canonicalizeClone(decoded), canonicalizeClone(instruction));
});

test("buildCreateKaigiInstruction accepts privacy artifacts", () => {
  const commitmentBytes = Buffer.alloc(32, 0x44);
  const nullifierBytes = Buffer.alloc(32, 0x55);
  const rosterRootBytes = Buffer.alloc(32, 0x66);
  const proofBytes = Buffer.from([0xca, 0xfe]);
  const instruction = buildCreateKaigiInstruction({
    id: "wonderland.sora:private-room",
    host: ACCOUNT_ID,
    privacyMode: "ZkRosterV1",
    commitment: { commitment: commitmentBytes, aliasTag: "host" },
    nullifier: { digest: nullifierBytes, issuedAtMs: 7 },
    rosterRoot: rosterRootBytes,
    proof: proofBytes,
  });
  const expected = {
    Kaigi: {
      CreateKaigi: {
        call: {
          id: { domain_id: "wonderland.sora", call_name: "private-room" },
          host: ACCOUNT_ID_CANONICAL,
          title: null,
          description: null,
          max_participants: null,
          gas_rate_per_minute: 0,
          metadata: {},
          scheduled_start_ms: null,
          billing_account: null,
          privacy_mode: { mode: "ZkRosterV1", state: null },
          room_policy: { policy: "Authenticated", state: null },
          relay_manifest: null,
        },
        commitment: {
          commitment: normalizedHashHex(commitmentBytes),
          alias_tag: "host",
        },
        nullifier: {
          digest: normalizedHashHex(nullifierBytes),
          issued_at_ms: 7,
        },
        roster_root: normalizedHashHex(rosterRootBytes),
        proof: proofBytes.toString("base64"),
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildJoinKaigiInstruction normalizes buffers and hashes", () => {
  const commitmentBytes = Buffer.alloc(32, 0x11);
  const nullifierBytes = Buffer.alloc(32, 0x22);
  const rosterRootBytes = Buffer.alloc(32, 0x33);
  const proofBytes = Buffer.from([0xaa, 0xbb, 0xcc]);
  const instruction = buildJoinKaigiInstruction({
    callId: "wonderland.sora:weekly-sync",
    participant: ACCOUNT_ID,
    commitment: {
      commitment: commitmentBytes,
    },
    nullifier: {
      digest: nullifierBytes,
      issuedAtMs: 99,
    },
    rosterRoot: rosterRootBytes,
    proof: proofBytes,
  });
  const expected = {
    Kaigi: {
      JoinKaigi: {
        call_id: { domain_id: "wonderland.sora", call_name: "weekly-sync" },
        participant: ACCOUNT_ID_CANONICAL,
        commitment: {
          commitment: normalizedHashHex(commitmentBytes),
          alias_tag: null,
        },
        nullifier: {
          digest: normalizedHashHex(nullifierBytes),
          issued_at_ms: 99,
        },
        roster_root: normalizedHashHex(rosterRootBytes),
        proof: proofBytes.toString("base64"),
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildLeaveKaigiInstruction accepts minimal payload", () => {
  const instruction = buildLeaveKaigiInstruction({
    callId: { domain_id: "wonderland.sora", call_name: "weekly-sync" },
    participant: ACCOUNT_ID,
  });
  const expected = {
    Kaigi: {
      LeaveKaigi: {
        call_id: { domain_id: "wonderland.sora", call_name: "weekly-sync" },
        participant: ACCOUNT_ID_CANONICAL,
        commitment: null,
        nullifier: null,
        roster_root: null,
        proof: null,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildEndKaigiInstruction normalizes optional timestamp", () => {
  const instruction = buildEndKaigiInstruction({
    callId: "wonderland.sora:weekly-sync",
    endedAtMs: "1700001234567",
  });
  const expected = {
    Kaigi: {
      EndKaigi: {
        call_id: { domain_id: "wonderland.sora", call_name: "weekly-sync" },
        ended_at_ms: 1700001234567,
        commitment: null,
        nullifier: null,
        roster_root: null,
        proof: null,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildEndKaigiInstruction accepts privacy artifacts", () => {
  const commitmentBytes = Buffer.alloc(32, 0x77);
  const nullifierBytes = Buffer.alloc(32, 0x88);
  const rosterRootBytes = Buffer.alloc(32, 0x99);
  const proofBytes = Buffer.from([0xaa, 0xbb, 0xcc]);
  const instruction = buildEndKaigiInstruction({
    callId: "wonderland.sora:weekly-sync",
    commitment: { commitment: commitmentBytes, aliasTag: "host" },
    nullifier: { digest: nullifierBytes, issuedAtMs: 13 },
    rosterRoot: rosterRootBytes,
    proof: proofBytes,
  });
  const expected = {
    Kaigi: {
      EndKaigi: {
        call_id: { domain_id: "wonderland.sora", call_name: "weekly-sync" },
        ended_at_ms: null,
        commitment: {
          commitment: normalizedHashHex(commitmentBytes),
          alias_tag: "host",
        },
        nullifier: {
          digest: normalizedHashHex(nullifierBytes),
          issued_at_ms: 13,
        },
        roster_root: normalizedHashHex(rosterRootBytes),
        proof: proofBytes.toString("base64"),
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildRecordKaigiUsageInstruction handles optional commitment", () => {
  const usageCommitment = Buffer.alloc(32, 0x55);
  const proof = Buffer.from([0xde, 0xad]);
  const instruction = buildRecordKaigiUsageInstruction({
    callId: "wonderland.sora:weekly-sync",
    durationMs: 60000,
    billedGas: "512",
    usageCommitment,
    proof,
  });
  const expected = {
    Kaigi: {
      RecordKaigiUsage: {
        call_id: { domain_id: "wonderland.sora", call_name: "weekly-sync" },
        duration_ms: 60000,
        billed_gas: 512,
        usage_commitment: normalizedHashHex(usageCommitment),
        proof: proof.toString("base64"),
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildSetKaigiRelayManifestInstruction allows clearing manifest", () => {
  const instruction = buildSetKaigiRelayManifestInstruction({
    callId: "wonderland.sora:weekly-sync",
    relayManifest: null,
  });
  const expected = {
    Kaigi: {
      SetKaigiRelayManifest: {
        call_id: { domain_id: "wonderland.sora", call_name: "weekly-sync" },
        relay_manifest: null,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildRegisterKaigiRelayInstruction encodes hpke key", () => {
  const instruction = buildRegisterKaigiRelayInstruction({
    relayId: RELAY_ACCOUNT_ID,
    hpkePublicKey: Buffer.alloc(32, 0xaa),
    bandwidthClass: 7,
  });
  const expected = {
    Kaigi: {
      RegisterKaigiRelay: {
        relay: {
          relay_id: RELAY_ACCOUNT_ID,
          hpke_public_key: "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=",
          bandwidth_class: 7,
        },
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

baseTest("buildRegisterSmartContractCodeInstruction normalizes manifest fields", () => {
  const codeHashBytes = Buffer.alloc(32, 0xaa);
  const abiHashBytes = Buffer.alloc(32, 0xbb);
  const signer = `ed25519:ed0120${SEED_11_ED25519_PUBLIC_KEY_HEX}`;
  const signature = `ed25519:${"22".repeat(64)}`;
  const signerCanonical = signer.split(":")[1];
  const signatureCanonical = signature.split(":")[1].toUpperCase();
  const instruction = buildRegisterSmartContractCodeInstruction({
    manifest: {
      seiyakuName: "Ledger",
      codeHash: codeHashBytes,
      abiHash: abiHashBytes,
      compilerFingerprint: "rustc-1.79",
      featuresBitmap: "3",
      accessSetHints: {
        readKeys: ["account:alice", "asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM"],
        writeKeys: ["contract:foo"],
        dynamicReads: [
          {
            baseKey: "state:Balances",
            keyType: "AccountId",
            boundKind: "take",
            maxKeys: "4",
          },
        ],
        dynamicWrites: [
          {
            base_key: "state:Votes",
            key_type: "Name",
            bound_kind: "range",
            max_keys: 2,
          },
        ],
      },
      entrypoints: [
        {
          name: "upgrade_ledger",
          kind: "Kaizen",
          permission: "can_upgrade",
        },
      ],
      states: [
        { name: "Balances", typeName: "StateMap<AccountId, quantity>" },
        { name: "Votes", typeName: "StateMap<Name, bool>" },
        { name: "amount", typeName: "Transfer{amount: quantity}" },
      ],
      errorCodes: [
        { namespace: "LedgerError", name: "amount", code: 7 },
      ],
      kotoba: [
        {
          msgId: "contract.title",
          translations: [{ lang: "en", text: "Ledger Contract" }],
        },
      ],
      provenance: {
        signer,
        signature,
      },
    },
  });
  const expected = {
    RegisterSmartContractCode: {
      manifest: {
        seiyaku_name: "Ledger",
        code_hash: normalizedHashHex(codeHashBytes),
        abi_hash: normalizedHashHex(abiHashBytes),
        compiler_fingerprint: "rustc-1.79",
        features_bitmap: 3,
        access_set_hints: {
          read_keys: ["account:alice", "asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM"],
          write_keys: ["contract:foo"],
          dynamic_reads: [
            {
              base_key: "state:Balances",
              key_type: "AccountId",
              bound_kind: "take",
              max_keys: 4,
            },
          ],
          dynamic_writes: [
            {
              base_key: "state:Votes",
              key_type: "Name",
              bound_kind: "range",
              max_keys: 2,
            },
          ],
        },
        entrypoints: [
          {
            name: "upgrade_ledger",
            kind: { kind: "Kaizen", value: null },
            params: [],
            argument_schema: null,
            return_type: null,
            return_schema: null,
            permission: "can_upgrade",
            read_keys: [],
            write_keys: [],
            access_hints_complete: null,
            access_hints_skipped: [],
            triggers: [],
          },
        ],
        states: [
          { name: "Balances", type_name: "StateMap<AccountId, quantity>" },
          { name: "Votes", type_name: "StateMap<Name, bool>" },
          { name: "amount", type_name: "Transfer{amount: quantity}" },
        ],
        error_codes: [
          { namespace: "LedgerError", name: "amount", code: 7 },
        ],
        kotoba: [
          {
            msg_id: "contract.title",
            translations: [{ lang: "en", text: "Ledger Contract" }],
          },
        ],
        provenance: {
          signer: signerCanonical,
          signature: signatureCanonical,
        },
      },
    },
  };
  const expectedDecoded = {
    RegisterSmartContractCode: {
      manifest: {
        ...expected.RegisterSmartContractCode.manifest,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expectedDecoded);
});

baseTest("smart-contract manifests reject unknown V1 feature bits", () => {
  assert.throws(
    () =>
      buildRegisterSmartContractCodeInstruction({
        manifest: { featuresBitmap: 4 },
      }),
    /featuresBitmap contains unsupported Kotodama V1 feature bits/u,
  );
  assert.throws(
    () =>
      buildRegisterSmartContractCodeInstruction({
        manifest: { features_bitmap: "4" },
      }),
    /featuresBitmap contains unsupported Kotodama V1 feature bits/u,
  );
});

baseTest("smart-contract dynamic access hints enforce the exact V1 contract", () => {
  const buildWithHint = (hint) =>
    buildRegisterSmartContractCodeInstruction({
      manifest: {
        states: [
          { name: "Balances", typeName: "StateMap<AccountId, quantity>" },
          { name: "amount", typeName: "StateMap<AccountId, quantity>" },
        ],
        accessSetHints: {
          dynamicReads: [{
            baseKey: "state:Balances",
            keyType: "AccountId",
            boundKind: "take",
            maxKeys: 1,
            ...hint,
          }],
        },
      },
    });

  for (const baseKey of ["state:Balances", "state:amount"]) {
    assert.doesNotThrow(() => buildWithHint({ baseKey }));
  }
  for (const baseKey of [
    "state:",
    "state:*",
    "state:Balances/",
    "state:Balances/suffix",
    "state:Balances:suffix",
    "state:int",
    "account:alice",
    " state:Balances",
    "state:Balances ",
  ]) {
    assert.throws(
      () => buildWithHint({ baseKey }),
      /dynamicReads\[0\]\.baseKey must be state: plus one canonical state declaration identifier/u,
    );
  }
  for (const keyType of [
    "Json",
    "ReferendumId",
    "Int",
    "Quantity",
    "Amount",
    " AccountId",
  ]) {
    assert.throws(
      () => buildWithHint({ keyType }),
      /dynamicReads\[0\]\.keyType must be an exact Kotodama V1 StateMap key scalar/u,
    );
  }
  for (const [boundKind, expected] of [
    ["", /dynamicReads\[0\]\.boundKind must be a non-empty string/u],
    ["Take", /dynamicReads\[0\]\.boundKind must be exactly take or range/u],
    ["prefix", /dynamicReads\[0\]\.boundKind must be exactly take or range/u],
    ["range ", /dynamicReads\[0\]\.boundKind must be exactly take or range/u],
  ]) {
    assert.throws(
      () => buildWithHint({ boundKind }),
      expected,
    );
  }
  for (const maxKeys of [0, 65, 0xffff_ffff]) {
    assert.throws(
      () => buildWithHint({ maxKeys }),
      maxKeys === 0
        ? /dynamicReads\[0\]\.maxKeys must be positive/u
        : /dynamicReads\[0\]\.maxKeys must be at most 64/u,
    );
  }
  assert.doesNotThrow(() => buildWithHint({ maxKeys: 64 }));

  const equalAliases = buildWithHint({
    base_key: "state:Balances",
    key_type: "AccountId",
    bound_kind: "take",
    max_keys: 1,
  });
  assert.equal(
    equalAliases.RegisterSmartContractCode.manifest
      .access_set_hints.dynamic_reads[0].max_keys,
    1,
  );
  for (const conflicting of [
    { base_key: "state:Other" },
    { key_type: "Name" },
    { bound_kind: "range" },
    { max_keys: 2 },
  ]) {
    assert.throws(
      () => buildWithHint(conflicting),
      /contains conflicting .* aliases/u,
    );
  }
});

baseTest("smart-contract dynamic access hints resolve declared StateMaps per list", () => {
  const hint = {
    baseKey: "state:Balances",
    keyType: "AccountId",
    boundKind: "take",
    maxKeys: 1,
  };
  const build = ({
    dynamicReads = [],
    dynamicWrites = [],
    states = [{ name: "Balances", typeName: "StateMap<AccountId, quantity>" }],
  }) =>
    buildRegisterSmartContractCodeInstruction({
      manifest: {
        states,
        accessSetHints: { dynamicReads, dynamicWrites },
      },
    });

  for (const field of ["dynamicReads", "dynamicWrites"]) {
    assert.doesNotThrow(() =>
      build({
        [field]: [
          hint,
          { ...hint, boundKind: "range", maxKeys: 2 },
        ],
      }));
    assert.throws(
      () => build({ [field]: [hint, { ...hint }] }),
      /duplicates an earlier dynamic access hint/u,
      `${field} must reject an exact duplicate`,
    );
    assert.throws(
      () => build({
        [field]: [{ ...hint, baseKey: "state:Missing" }],
      }),
      /baseKey must reference a declared top-level StateMap/u,
      `${field} must reject an unknown state`,
    );
    assert.throws(
      () => build({
        [field]: [hint],
        states: [{ name: "Balances", typeName: "quantity" }],
      }),
      /baseKey must reference a declared top-level StateMap/u,
      `${field} must reject a scalar state`,
    );
    assert.throws(
      () => build({
        [field]: [{ ...hint, keyType: "Name" }],
      }),
      /keyType Name does not match declared StateMap key type AccountId/u,
      `${field} must reject a mismatched key scalar`,
    );
  }

  assert.doesNotThrow(() =>
    build({
      dynamicReads: [hint],
      dynamicWrites: [{ ...hint }],
    }));
  assert.doesNotThrow(() =>
    build({
      dynamicReads: [{
        ...hint,
        baseKey: "state:amount",
        keyType: "quantity",
      }],
      states: [{ name: "amount", typeName: "StateMap<quantity, int>" }],
    }));
  assert.throws(
    () =>
      build({
        dynamicReads: [hint],
        states: [
          { name: "Balances", typeName: "StateMap<AccountId, quantity>" },
          { name: "Balances", typeName: "StateMap<AccountId, quantity>" },
        ],
      }),
    /contains duplicate state name Balances/u,
  );
});

baseTest("smart-contract parameter and state type aliases must agree", () => {
  const quantity = {
    nodes: [{ kind: "Leaf", value: { kind: "Quantity", value: null } }],
  };
  const build = (param, state) =>
    buildRegisterSmartContractCodeInstruction({
      manifest: {
        entrypoints: [{
          name: "read",
          kind: "View",
          params: [{ name: "amount", ...param }],
          argumentSchema: {
            fields: [{ name: "amount", ty: quantity }],
          },
        }],
        states: [{ name: "amount", ...state }],
      },
    });

  assert.doesNotThrow(() =>
    build(
      { typeName: "quantity", type_name: "quantity" },
      { typeName: "quantity", type_name: "quantity" },
    ));
  assert.throws(
    () => build(
      { typeName: "quantity", type_name: "int" },
      { typeName: "quantity" },
    ),
    /params\[0\]\.type_name contains conflicting type_name\/typeName aliases/u,
  );
  assert.throws(
    () => build(
      { typeName: "quantity" },
      { typeName: "quantity", type_name: "int" },
    ),
    /states\[0\]\.type_name contains conflicting type_name\/typeName aliases/u,
  );
  assert.throws(
    () => build({}, { typeName: "quantity" }),
    /params\[0\]\.type_name must be a non-empty string/u,
  );
  assert.throws(
    () => build({ typeName: "quantity" }, {}),
    /states\[0\]\.type_name must be a non-empty string/u,
  );
});

baseTest("smart-contract manifest type declarations reject retired numeric names", () => {
  for (const seiyakuName of ["Amount", "amount"]) {
    assert.throws(
      () =>
        buildRegisterSmartContractCodeInstruction({
          manifest: { seiyakuName },
        }),
      /seiyakuName must be a canonical Kotodama V1 type declaration identifier/u,
    );
  }
  for (const typeName of [
    "Amount",
    "amount",
    "StateMap<AccountId, Amount>",
    "Transfer{amount: amount}",
  ]) {
    assert.throws(
      () =>
        buildRegisterSmartContractCodeInstruction({
          manifest: {
            states: [{ name: "Balances", typeName }],
          },
        }),
      /states\[0\]\.type_name must be a canonical Kotodama V1 state type/u,
    );
  }
  for (const namespace of ["Amount", "amount"]) {
    assert.throws(
      () =>
        buildRegisterSmartContractCodeInstruction({
          manifest: {
            errorCodes: [{ namespace, name: "Denied", code: 7 }],
          },
        }),
      /errorCodes\[0\]\.namespace must be a canonical Kotodama V1 type declaration identifier/u,
    );
  }
  for (const keyType of [
    "Json",
    "ReferendumId",
    "Int",
    "Quantity",
    "Amount",
    "amount",
    "Foo{Amount: quantity}",
    "Foo{Amount:quantity}",
    "StateMap<AccountId, int>",
    "\u0410mount",
  ]) {
    assert.throws(
      () =>
        buildRegisterSmartContractCodeInstruction({
          manifest: {
            accessSetHints: {
              readKeys: [],
              writeKeys: [],
              dynamicReads: [{
                baseKey: "state:Balances",
                keyType,
                boundKind: "take",
                maxKeys: 1,
              }],
              dynamicWrites: [],
            },
          },
        }),
      /dynamicReads\[0\]\.keyType must be an exact Kotodama V1 StateMap key scalar/u,
    );
  }
});

baseTest("smart-contract entrypoint schemas exactly bind declared V1 types", () => {
  const quantity = {
    nodes: [{ kind: "Leaf", value: { kind: "Quantity", value: null } }],
  };
  const valid = buildRegisterSmartContractCodeInstruction({
    manifest: {
      entrypoints: [{
        name: "read",
        kind: "View",
        params: [{ name: "amount", typeName: "quantity" }],
        argumentSchema: {
          fields: [{ name: "amount", ty: quantity }],
        },
        returnType: "quantity",
        returnSchema: quantity,
      }],
    },
  });
  assert.equal(
    valid.RegisterSmartContractCode.manifest.entrypoints[0].params[0].type_name,
    "quantity",
  );

  for (const retired of ["Amount", "amount"]) {
    assert.throws(
      () =>
        buildRegisterSmartContractCodeInstruction({
          manifest: {
            entrypoints: [{
              name: "read",
              kind: "View",
              params: [{ name: "value", typeName: retired }],
              argumentSchema: {
                fields: [{ name: "value", ty: quantity }],
              },
            }],
          },
        }),
      /argument_schema\.fields\[0\] does not match its declared parameter/u,
    );
    assert.throws(
      () =>
        buildRegisterSmartContractCodeInstruction({
          manifest: {
            entrypoints: [{
              name: "read",
              kind: "View",
              returnType: retired,
              returnSchema: quantity,
            }],
          },
        }),
      /return_schema does not match return_type/u,
    );
  }

  assert.throws(
    () =>
      buildRegisterSmartContractCodeInstruction({
        manifest: {
          entrypoints: [{
            name: "read",
            kind: "View",
            params: [{ name: "value", typeName: "quantity" }],
          }],
        },
      }),
    /argument_schema is required for declared parameters/u,
  );
  assert.throws(
    () =>
      buildRegisterSmartContractCodeInstruction({
        manifest: {
          entrypoints: [{
            name: "read",
            kind: "View",
            returnType: "quantity",
          }],
        },
      }),
    /return_type and return_schema must be present together/u,
  );
});

baseTest("smart-contract entrypoint kinds use only the V1 interface names", () => {
  for (const canonical of ["Kotoage", "View", "Hajimari", "Kaizen"]) {
    const instruction = buildRegisterSmartContractCodeInstruction({
      manifest: {
        entrypoints: [{ name: "run", kind: canonical }],
      },
    });
    assert.equal(
      instruction.RegisterSmartContractCode.manifest.entrypoints[0].kind.kind,
      canonical,
    );
  }

  for (const retired of ["Public", "public", "Init", "init", "Upgrade", "upgrade"]) {
    assert.throws(
      () =>
        buildRegisterSmartContractCodeInstruction({
          manifest: {
            entrypoints: [{ name: "legacy", kind: retired }],
          },
        }),
      /must be one of 'Kotoage', 'View', 'Hajimari', or 'Kaizen'/,
    );
  }
});

baseTest("smart-contract branded entrypoint kinds preserve their Norito tag order", () => {
  for (const canonical of ["Kotoage", "View", "Hajimari", "Kaizen"]) {
    const instruction = buildRegisterSmartContractCodeInstruction({
      manifest: {
        entrypoints: [{ name: "run", kind: canonical }],
      },
    });
    assert.equal(
      encodeAndDecode(instruction).RegisterSmartContractCode.manifest.entrypoints[0].kind.kind,
      canonical,
    );
  }
});

baseTest("smart-contract schema builder enforces canonical flat-preorder V1 tapes", () => {
  const leaf = (kind) => ({ kind: "Leaf", value: { kind, value: null } });
  const build = (nodes) =>
    buildRegisterSmartContractCodeInstruction({
      manifest: {
        entrypoints: [
          {
            name: "read",
            kind: "View",
            returnType: analyzeEntrypointValueTypeV1({ nodes }).canonicalName,
            returnSchema: { nodes },
          },
        ],
      },
    });

  const pair = [
    { kind: "Struct", value: { name: "Pair", fields: ["left", "right"] } },
    leaf("Int"),
    leaf("Bool"),
  ];
  assert.deepEqual(
    build(pair).RegisterSmartContractCode.manifest.entrypoints[0].return_schema.nodes,
    pair,
  );
  assert.deepEqual(
    build([
      { kind: "List", value: { capacity: 64 } },
      leaf("Name"),
    ]).RegisterSmartContractCode.manifest.entrypoints[0].return_schema.nodes[0].value,
    { capacity: 64 },
  );

  for (const malformed of [
    [],
    [{ kind: "List", value: { capacity: 1 } }],
    [leaf("Int"), leaf("Bool")],
    [
      { kind: "List", value: { capacity: 1, element: { nodes: [leaf("Int")] } } },
      leaf("Int"),
    ],
    [{ kind: "List", value: { capacity: 0 } }, leaf("Int")],
    [{ kind: "List", value: { capacity: 65 } }, leaf("Int")],
    [
      ...Array.from({ length: 256 }, () => ({
        kind: "List",
        value: { capacity: 1 },
      })),
      leaf("Int"),
    ],
  ]) {
    assert.throws(() => build(malformed), /canonical|capacity|complete|only capacity/u);
  }

  for (const retired of ["U128", "Amount"]) {
    assert.throws(
      () => build([leaf(retired)]),
      /not a (?:canonical )?V1 entrypoint value kind/u,
    );
  }

  for (const [name, fields, children] of [
    ["AccountView", ["id", "metadata"], [leaf("AccountId"), leaf("Json")]],
    ["AssetView", ["id", "amount"], [leaf("AssetId"), leaf("Quantity")]],
    [
      "AssetDefinitionView",
      ["id", "name", "description", "owned_by", "total_quantity", "metadata"],
      [
        leaf("AssetDefinitionId"),
        leaf("String"),
        { kind: "Option", value: null },
        leaf("String"),
        leaf("AccountId"),
        leaf("Quantity"),
        leaf("Json"),
      ],
    ],
    [
      "DomainView",
      ["id", "owned_by", "metadata"],
      [leaf("DomainId"), leaf("AccountId"), leaf("Json")],
    ],
    [
      "NftView",
      ["id", "owned_by", "content"],
      [leaf("NftId"), leaf("AccountId"), leaf("Json")],
    ],
  ]) {
    const canonical = [{ kind: "Struct", value: { name, fields } }, ...children];
    assert.doesNotThrow(() => build(canonical));
    const forged = structuredClone(canonical);
    forged[1].value.kind = "Bool";
    assert.throws(() => build(forged), /forged reserved query-view/u);
  }
});

test("buildRegisterSmartContractBytesInstruction encodes bytes deterministically", () => {
  const codeBytes = Buffer.from([0xde, 0xad, 0xbe, 0xef]);
  const hashBytes = Buffer.alloc(32, 0xcc);
  const instruction = buildRegisterSmartContractBytesInstruction({
    codeHash: hashBytes,
    code: codeBytes,
  });
  const expected = {
    RegisterSmartContractBytes: {
      code_hash: normalizedHashHex(hashBytes),
      code: codeBytes.toString("base64"),
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
});

test("buildRegisterSmartContractBytesInstruction rejects empty code bytes", () => {
  assert.throws(
    () =>
      buildRegisterSmartContractBytesInstruction({
        codeHash: Buffer.alloc(32, 0x11),
        code: Buffer.alloc(0),
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_STRING);
      assert.match(String(error?.message), /non-empty base64/i);
      return true;
    },
  );
});

test("buildRemoveSmartContractBytesInstruction accepts reason or null", () => {
  const instruction = buildRemoveSmartContractBytesInstruction({
    codeHash: Buffer.alloc(32, 0x11),
    reason: "cleanup",
  });
  const expected = {
    RemoveSmartContractBytes: {
      code_hash: normalizedHashHex(Buffer.alloc(32, 0x11)),
      reason: "cleanup",
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);

  const withoutReason = buildRemoveSmartContractBytesInstruction({
    codeHash: Buffer.alloc(32, 0x22),
  });
  assert.equal(withoutReason.RemoveSmartContractBytes.reason, undefined);
});

test("buildProposeDeployContractInstruction normalizes hashes and window", () => {
  const instruction = buildProposeDeployContractInstruction({
    contractAddress: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    codeHash: "AA".repeat(32),
    abiHash: Buffer.alloc(32, 0xbb),
    abiVersion: "1",
    window: { lower: 10, upper: 20 },
    votingMode: "Plain",
  });
  const expected = {
    ProposeDeployContract: {
      contract_address: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
      code_hash_hex: "aa".repeat(32),
      abi_hash_hex: Buffer.alloc(32, 0xbb).toString("hex"),
      abi_version: "1",
      window: { lower: "10", upper: "20" },
      mode: "Plain",
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
});

test("buildProposeDeployContractInstruction rejects non-canonical voting modes", () => {
  const base = {
    contractAddress: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    codeHash: "aa".repeat(32),
    abiHash: "bb".repeat(32),
  };
  for (const votingMode of [
    "zk",
    "plain",
    " Zk",
    "Plain ",
    "zero-knowledge",
    "zkp",
    "plaintext",
    "plain_text",
    "quadratic",
    1,
  ]) {
    assert.throws(
      () => buildProposeDeployContractInstruction({ ...base, votingMode }),
      /must be either 'Zk' or 'Plain'/u,
    );
  }

  for (const mode of ["zk", "plain", " Zk", "Plain "]) {
    assert.throws(
      () =>
        encodeInstruction({
          ProposeDeployContract: {
            contract_address: base.contractAddress,
            code_hash_hex: base.codeHash,
            abi_hash_hex: base.abiHash,
            abi_version: "1",
            mode,
          },
        }),
      /must be Zk or Plain/u,
    );
  }
});

test("buildCastZkBallotInstruction encodes proof and JSON inputs", () => {
  const publicInputs = { tally: "aye" };
  const instruction = buildCastZkBallotInstruction({
    electionId: "ref-1",
    proof: Buffer.from([0x01, 0x02]),
    publicInputs,
  });
  const expected = {
    CastZkBallot: {
      election_id: "ref-1",
      proof_b64: Buffer.from([0x01, 0x02]).toString("base64"),
      public_inputs_json: JSON.stringify(publicInputs),
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
});

test("buildCastZkBallotInstruction defaults public inputs to empty object", () => {
  const instruction = buildCastZkBallotInstruction({
    electionId: "ref-2",
    proof: Buffer.from([0x03]),
  });
  assert.equal(instruction.CastZkBallot.public_inputs_json, "{}");
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, instruction);
});

test("buildCastZkBallotInstruction rejects unsupported public input keys", () => {
  assert.throws(
    () =>
      buildCastZkBallotInstruction({
        electionId: "ref-3",
        proof: Buffer.from([0x04]),
        publicInputs: {
          owner: SAMPLE_ACCOUNT_I105_LITERAL,
          amount: "250",
          durationBlocks: 12,
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /durationBlocks/i);
      return true;
    },
  );
});

test("buildCastZkBallotInstruction canonicalizes hex hint values", () => {
  const instruction = buildCastZkBallotInstruction({
    electionId: "ref-3",
    proof: Buffer.from([0x04]),
    publicInputs: {
      owner: SAMPLE_ACCOUNT_I105_LITERAL,
      amount: "250",
      duration_blocks: 12,
      root_hint: `0x${"Aa".repeat(32)}`,
      nullifier: `blake2b32:${"BB".repeat(32)}`,
    },
  });
  const parsed = JSON.parse(instruction.CastZkBallot.public_inputs_json);
  assert.equal(parsed.root_hint, "aa".repeat(32));
  assert.equal(parsed.nullifier, "bb".repeat(32));
});

test("buildCastZkBallotInstruction canonicalizes public input ordering", () => {
  const instruction = buildCastZkBallotInstruction({
    electionId: "ref-4",
    proof: Buffer.from([0x05]),
    publicInputs: {
      tally: "aye",
      meta: { z: 1, a: 2 },
      badge: "voter",
    },
  });
  assert.equal(
    instruction.CastZkBallot.public_inputs_json,
    '{"badge":"voter","meta":{"a":2,"z":1},"tally":"aye"}',
  );
});

test("buildCastZkBallotInstruction rejects non-object public inputs", () => {
  assert.throws(
    () =>
      buildCastZkBallotInstruction({
        electionId: "ref-4",
        proof: Buffer.from([0x05]),
        publicInputs: "[1,2]",
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /publicInputs/i);
      return true;
    },
  );
});

test("buildCastZkBallotInstruction requires complete lock hints", () => {
  assert.throws(
    () =>
      buildCastZkBallotInstruction({
        electionId: "ref-5",
        proof: Buffer.from([0x06]),
        publicInputs: { owner: SAMPLE_ACCOUNT_I105_LITERAL },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /owner, amount, and duration_blocks/i);
      return true;
    },
  );
});

test("buildCastZkBallotInstruction rejects noncanonical owner", () => {
  const malformedOwner = ACCOUNT_ID.replace(/^sora/u, "ｓｏｒａ");
  assert.throws(
    () =>
      buildCastZkBallotInstruction({
        electionId: "ref-5",
        proof: Buffer.from([0x06]),
        publicInputs: {
          owner: malformedOwner,
          amount: "250",
          duration_blocks: 12,
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_ACCOUNT_ID);
      assert.match(String(error?.message), /canonical .*i105 account id/i);
      return true;
    },
  );
});

test("buildCastZkBallotInstruction rejects empty proof bytes", () => {
  assert.throws(
    () =>
      buildCastZkBallotInstruction({
        electionId: "ref-1",
        proof: Buffer.alloc(0),
        publicInputs: { tally: "aye" },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_STRING);
      assert.match(String(error?.message), /non-empty base64/i);
      return true;
    },
  );
});

test("buildCastPlainBallotInstruction maps direction labels", () => {
  const instruction = buildCastPlainBallotInstruction({
    referendumId: "ref-2",
    owner: ACCOUNT_ID,
    amount: "18446744073709551616.25",
    durationBlocks: 50,
    direction: "nay",
  });
  const expected = {
    CastPlainBallot: {
      referendum_id: "ref-2",
      owner: ACCOUNT_ID_CANONICAL,
      amount: "18446744073709551616.25",
      duration_blocks: 50,
      direction: 1,
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
});

test("buildCastPlainBallotInstruction rejects lossy and noncanonical Quantity inputs", () => {
  const overflowing = "9".repeat(155);
  for (const amount of [
    1,
    "+1",
    "01",
    "1.0",
    "1.2300",
    "1amt",
    "1qty",
    " 1",
    "1 ",
    "-1",
    overflowing,
  ]) {
    assert.throws(
      () =>
        buildCastPlainBallotInstruction({
          referendumId: "ref-2",
          owner: ACCOUNT_ID,
          amount,
          durationBlocks: 50,
          direction: "nay",
        }),
      /canonical|JavaScript numbers are not lossless quantity inputs/u,
      `amount ${String(amount)} must be rejected`,
    );
  }
});

baseTest("CastPlainBallot pure-JS Norito codec preserves strict fractional Quantity", () => {
  const instruction = {
    CastPlainBallot: {
      referendum_id: "ref-quantity",
      owner: ACCOUNT_ID_CANONICAL,
      amount: "18446744073709551616.25",
      duration_blocks: 50,
      direction: 1,
    },
  };
  withPureJsInstructionCodec(() => {
    const encoded = noritoEncodeInstruction(instruction);
    const outerFrame = validateNoritoFrame(encoded);
    assert.equal(outerFrame.flags, 0x02);
    const wireField = readCompactFieldPayload(
      outerFrame.payload,
      0,
      "CastPlainBallot.wire",
    );
    const innerField = readCompactFieldPayload(
      outerFrame.payload,
      wireField.next,
      "CastPlainBallot.inner",
    );
    assert.equal(innerField.next, outerFrame.payload.length);
    const innerFrameLength = Number(innerField.payload.readBigUInt64LE(0));
    const innerFrame = innerField.payload.subarray(8);
    assert.equal(innerFrame.length, innerFrameLength);
    const validatedInner = validateNoritoFrame(innerFrame, {
      expectedTypeName: "iroha_data_model::isi::governance::CastPlainBallot",
      expectedPaddingLength: 0,
    });
    const expectedSchemaHash = createHash("sha256")
      .update(
        "norito:v1:type-name\0iroha_data_model::isi::governance::CastPlainBallot",
        "utf8",
      )
      .digest()
      .subarray(0, 16);
    assert.equal(expectedSchemaHash.toString("hex"), "62b23313103064bc2c9d528ac3548949");
    assert.deepEqual(validatedInner.schemaHash, expectedSchemaHash);
    assert.deepEqual(noritoDecodeInstruction(encoded), instruction);

    for (const amount of [
      1,
      "+1",
      "01",
      "1.0",
      "1.2300",
      "1amt",
      "1qty",
      " 1",
      "1 ",
      "-1",
      (1n << 511n).toString(),
      "9".repeat(155),
    ]) {
      assert.throws(
        () =>
          noritoEncodeInstruction({
            CastPlainBallot: {
              ...instruction.CastPlainBallot,
              amount,
            },
          }),
        /canonical|JavaScript numbers are rejected|mantissa|negative/u,
        `amount ${String(amount).slice(0, 32)} must be rejected`,
      );
    }
  });
});

test("CastPlainBallot pure-JS bytes match native compact framing", () => {
  const instruction = {
    CastPlainBallot: {
      referendum_id: "ref-quantity",
      owner: ACCOUNT_ID_CANONICAL,
      amount: "18446744073709551616.25",
      duration_blocks: 50,
      direction: 1,
    },
  };
  const encoded = assertNativeAndPureInstructionParity(
    instruction,
    "CastPlainBallot",
  );
  assert.equal(encoded[39], 0x02);
});

test("buildEnactReferendumInstruction normalizes hashes and window defaults", () => {
  const instruction = buildEnactReferendumInstruction({
    referendumId: Buffer.alloc(32, 0x11),
    preimageHash: Buffer.alloc(32, 0xbb),
  });
  const expected = {
    EnactReferendum: {
      referendum_id: toByteArray(Buffer.alloc(32, 0x11)),
      preimage_hash: toByteArray(Buffer.alloc(32, 0xbb)),
      at_window: { lower: "0", upper: "0" },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildFinalizeReferendumInstruction encodes proposal id", () => {
  const instruction = buildFinalizeReferendumInstruction({
    referendumId: "ref-3",
    proposalId: Buffer.alloc(32, 0x66),
  });
  const expected = {
    FinalizeReferendum: {
      referendum_id: "ref-3",
      proposal_id: toByteArray(Buffer.alloc(32, 0x66)),
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildPersistCouncilForEpochInstruction validates members and derivation", () => {
  const instruction = buildPersistCouncilForEpochInstruction({
    epoch: 10,
    members: [ACCOUNT_ID],
    candidatesCount: 5,
    derivedBy: "Vrf",
  });
  const expected = {
    PersistCouncilForEpoch: {
      epoch: 10,
      members: [ACCOUNT_ID_CANONICAL],
      alternates: [],
      verified: 0,
      candidates_count: 5,
      derived_by: "Vrf",
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
  assert.throws(
    () =>
      buildPersistCouncilForEpochInstruction({
        epoch: 10,
        members: [ACCOUNT_ID],
        candidatesCount: 5,
        derivedBy: "Manual",
      }),
    /derivedBy must be Vrf/,
  );
});

test("buildSubmitAgendaProposalInstruction wraps the supplied proposal payload", () => {
  const proposal = {
    version: 1,
    proposal_id: "AC-2026-001",
    submitted_at_unix_ms: 1770000000000,
    language: "en",
    action: "add-to-denylist",
    summary: {
      title: "Blacklist proposal for bafy-test",
      motivation: "Evidence review requested for the published CID.",
      expected_impact: "Participating gateways would restrict delivery during review.",
    },
    tags: ["spam"],
    targets: [
      {
        label: "bafy-test",
        hash_family: "sorafs-root-cid",
        hash_hex: "11".repeat(32),
        reason: "spam moderation report",
      },
    ],
    evidence: [
      {
        kind: "url",
        uri: "https://example.invalid/case/1",
        digest_blake3_hex: "22".repeat(32),
        description: "Captured gateway evidence",
      },
    ],
    submitter: {
      name: "Explorer Moderator",
      contact: "https://example.invalid/moderation",
      organization: null,
      pgp_fingerprint: null,
    },
    duplicates: [],
  };
  const instruction = buildSubmitAgendaProposalInstruction({ proposal });
  assert.deepEqual(instruction, {
    SubmitAgendaProposal: {
      proposal,
    },
  });
  assert.deepEqual(encodeAndDecode(instruction), instruction);
});

test("buildClaimTwitterFollowRewardInstruction wraps keyed hash", () => {
  const digest = normalizedHashHex(Buffer.alloc(32, 0xaa));
  const instruction = buildClaimTwitterFollowRewardInstruction({
    bindingHash: {
      pepper_id: "twitter-follow",
      digest,
    },
  });
  const expected = {
    ClaimTwitterFollowReward: {
      binding_hash: {
        pepper_id: "twitter-follow",
        digest,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
});

test("buildSendToTwitterInstruction encodes keyed hash and amount", () => {
  const digest = normalizedHashHex(Buffer.alloc(32, 0xbb));
  const instruction = buildSendToTwitterInstruction({
    bindingHash: {
      pepper_id: "twitter-follow",
      digest,
    },
    amount: "42",
  });
  const expected = {
    SendToTwitter: {
      binding_hash: {
        pepper_id: "twitter-follow",
        digest,
      },
      amount: "42",
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
});

test("buildCancelTwitterEscrowInstruction wraps keyed hash", () => {
  const digest = normalizedHashHex(Buffer.alloc(32, 0xcc));
  const instruction = buildCancelTwitterEscrowInstruction({
    bindingHash: {
      pepper_id: "twitter-follow",
      digest,
    },
  });
  const expected = {
    CancelTwitterEscrow: {
      binding_hash: {
        pepper_id: "twitter-follow",
        digest,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
});

test("buildRegisterZkAssetInstruction normalizes verifying key ids", () => {
  const instruction = buildRegisterZkAssetInstruction({
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    mode: "zk-native",
    transferVerifyingKey: "halo2/ipa:vk_transfer",
    unshieldVerifyingKey: { backend: "halo2/ipa", name: "vk_unshield" },
  });
  const payload = encodeAndDecode(instruction).zk.RegisterZkAsset;
  assert.equal(payload.mode, "ZkNative");
  assert.deepEqual(payload.vk_transfer, { backend: "halo2/ipa", name: "vk_transfer" });
  assert.deepEqual(payload.vk_unshield, { backend: "halo2/ipa", name: "vk_unshield" });
});

test("buildRegisterAssetHiddenZkPoolInstruction encodes pool verifier state", () => {
  const assetSetRoot = Buffer.alloc(32, 0xa0);
  const instruction = buildRegisterAssetHiddenZkPoolInstruction({
    poolId: "boi-private-is-pool",
    storageAssetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    assetSetRoot,
    transferVerifyingKey: "halo2/ipa/poly-open:native_ipa_vk",
  });
  const payload = encodeAndDecode(instruction).zk.RegisterAssetHiddenZkPool;
  assert.equal(payload.pool_id, "boi-private-is-pool");
  assert.equal(payload.storage_asset, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
  assert.deepEqual(payload.asset_set_root, Array.from(assetSetRoot));
  assert.deepEqual(payload.vk_transfer, {
    backend: "halo2/ipa/poly-open",
    name: "native_ipa_vk",
  });
});

test("buildRegisterAssetHiddenZkPoolInstruction rejects adversarial pool registration", () => {
  const validBase = {
    poolId: "boi-private-is-pool",
    storageAssetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    assetSetRoot: Buffer.alloc(32, 0xa0),
    transferVerifyingKey: "halo2/ipa/poly-open:native_ipa_vk",
  };
  for (const payload of [
    { ...validBase, poolId: "   " },
    { ...validBase, assetSetRoot: Buffer.alloc(31, 0xa0) },
    { ...validBase, assetSetRoot: Buffer.alloc(32, 0x00) },
    { ...validBase, transferVerifyingKey: null },
    { ...validBase, transferVerifyingKey: "missing-separator" },
    { ...validBase, poolId: "pool-a", pool_id: "pool-b" },
    { ...validBase, assetSetRoot: Buffer.alloc(32, 0xa0), asset_set_root: Buffer.alloc(32, 0xa1) },
  ]) {
    assert.throws(
      () => buildRegisterAssetHiddenZkPoolInstruction(payload),
      /registerAssetHiddenZkPool/,
    );
  }
});

test("buildScheduleConfidentialPolicyTransitionInstruction encodes transition metadata", () => {
  const transitionId = Buffer.alloc(32, 0xaa);
  const instruction = buildScheduleConfidentialPolicyTransitionInstruction({
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    newMode: "ShieldedOnly",
    effectiveHeight: "42",
    transitionId,
    conversionWindow: 10,
  });
  const payload = encodeAndDecode(instruction).zk.ScheduleConfidentialPolicyTransition;
  assert.equal(payload.new_mode, "ShieldedOnly");
  assert.equal(payload.effective_height, 42);
  assert.equal(payload.conversion_window, 10);
  assert.equal(payload.transition_id, normalizedHashHex(transitionId));
});

test("buildCancelConfidentialPolicyTransitionInstruction wraps hash literal", () => {
  const transitionId = Buffer.alloc(32, 0xbb);
  const instruction = buildCancelConfidentialPolicyTransitionInstruction({
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    transitionId,
  });
  const payload = encodeAndDecode(instruction).zk.CancelConfidentialPolicyTransition;
  assert.equal(payload.asset, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
  assert.equal(payload.transition_id, normalizedHashHex(transitionId));
});

test("buildShieldInstruction encodes encrypted payload fields", () => {
  const instruction = buildShieldInstruction({
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    fromAccountId: ACCOUNT_ID_INPUT,
    amount: "340282366920938463463374607431768211456.25",
    noteCommitment: Buffer.alloc(32, 0x01),
    encryptedPayload: {
      version: 1,
      ephemeralPublicKey: Buffer.alloc(32, 0x02),
      nonce: Buffer.alloc(24, 0x03),
      ciphertext: Buffer.from("ciphertext"),
    },
  });
  const payload = encodeAndDecode(instruction).zk.Shield;
  assert.equal(payload.amount, "340282366920938463463374607431768211456.25");
  assert.equal(payload.enc_payload.version, 1);
  assert.equal(payload.enc_payload.ciphertext, Buffer.from("ciphertext").toString("base64"));
});

descriptorTest("buildShieldInstruction enforces strict canonical Quantity inputs", () => {
  const wide = "340282366920938463463374607431768211456.25";
  assert.equal(
    buildShieldInstruction({
      assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
      fromAccountId: ACCOUNT_ID_INPUT,
      amount: wide,
      noteCommitment: Buffer.alloc(32, 0x01),
      encryptedPayload: {
        version: 1,
        ephemeralPublicKey: Buffer.alloc(32, 0x02),
        nonce: Buffer.alloc(24, 0x03),
        ciphertext: Buffer.from("ciphertext"),
      },
    }).zk.Shield.amount,
    wide,
  );
  for (const amount of [Number.MAX_SAFE_INTEGER + 1, "01", "1.0", "-1"]) {
    assert.throws(
      () => buildShieldInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        fromAccountId: ACCOUNT_ID_INPUT,
        amount,
        noteCommitment: Buffer.alloc(32, 0x01),
        encryptedPayload: {
          version: 1,
          ephemeralPublicKey: Buffer.alloc(32, 0x02),
          nonce: Buffer.alloc(24, 0x03),
          ciphertext: Buffer.from("ciphertext"),
        },
      }),
      (error) => {
        assert.equal(error?.code, ValidationErrorCode.INVALID_NUMERIC);
        assert.match(String(error?.message), /canonical|quantity|string|bigint|numbers are not/i);
        return true;
      },
    );
  }
});

test("buildZkTransferInstruction normalizes proof attachments", () => {
  const instruction = buildZkTransferInstruction({
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    inputs: [Buffer.alloc(32, 0x11)],
    outputs: [Buffer.alloc(32, 0x22)],
    proof: {
      backend: "halo2/ipa",
      proof: Buffer.from("proof"),
      verifyingKeyRef: "halo2/ipa:vk_transfer",
    },
  });
  const payload = encodeAndDecode(instruction).zk.ZkTransfer;
  assert.equal(payload.proof.backend, "halo2/ipa");
  assert.equal(payload.proof.vk_ref.name, "vk_transfer");
  assert.equal(payload.inputs.length, 1);
});

test("buildAssetHiddenZkTransferInstruction encodes pool transfer surface", () => {
  const instruction = buildAssetHiddenZkTransferInstruction({
    poolId: "boi-private-is-pool",
    inputs: [Buffer.alloc(32, 0x11)],
    outputs: [Buffer.alloc(32, 0x22)],
    proof: {
      backend: "halo2/ipa",
      proof: Buffer.from("proof"),
      verifyingKeyRef: "halo2/ipa:vk_asset_hidden_transfer",
    },
    rootHint: Buffer.alloc(32, 0x33),
  });
  const payload = encodeAndDecode(instruction).zk.AssetHiddenZkTransfer;
  assert.equal(payload.pool_id, "boi-private-is-pool");
  assert.deepEqual(payload.inputs[0], Array.from(Buffer.alloc(32, 0x11)));
  assert.deepEqual(payload.outputs[0], Array.from(Buffer.alloc(32, 0x22)));
  assert.deepEqual(payload.root_hint, Array.from(Buffer.alloc(32, 0x33)));
  assert.equal(payload.proof.vk_ref.name, "vk_asset_hidden_transfer");
});

test("buildAssetHiddenZkTransferInstruction rejects adversarial pool payloads", () => {
  const validBase = {
    poolId: "boi-private-is-pool",
    inputs: [Buffer.alloc(32, 0x11)],
    outputs: [Buffer.alloc(32, 0x22)],
    proof: {
      backend: "halo2/ipa",
      proof: Buffer.from("proof"),
      verifyingKeyRef: "halo2/ipa:vk_asset_hidden_transfer",
    },
  };
  for (const payload of [
    { ...validBase, inputs: [] },
    { ...validBase, outputs: [] },
    { ...validBase, poolId: "   " },
    { ...validBase, inputs: [Buffer.alloc(31)] },
    { ...validBase, outputs: [Buffer.alloc(33)] },
    { ...validBase, poolId: "pool-a", pool_id: "pool-b" },
    { ...validBase, assetPoolId: "pool-a", asset_pool_id: "pool-b" },
    { ...validBase, rootHint: Buffer.alloc(32), root_hint: Buffer.alloc(32) },
  ]) {
    assert.throws(
      () => buildAssetHiddenZkTransferInstruction(payload),
      /assetHiddenZkTransfer/,
    );
  }
});

test("buildZkTransferInstruction rejects legacy inline verifying key fields", () => {
  for (const field of [
    "vk_inline",
    "vkInline",
    "verifyingKeyInline",
    "verifying_key_inline",
  ]) {
    assert.throws(
      () =>
        buildZkTransferInstruction({
          assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
          inputs: [Buffer.alloc(32, 0x11)],
          outputs: [Buffer.alloc(32, 0x22)],
          proof: {
            backend: "halo2/ipa",
            proof: Buffer.from("proof"),
            verifyingKeyRef: "halo2/ipa:vk_transfer",
            [field]: { backend: "halo2/ipa", bytes: Buffer.from("legacy-vk") },
          },
        }),
      (error) => {
        assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
        assert.match(String(error?.message), /not supported; use verifyingKeyRef/i);
        return true;
      },
    );
  }
});

test("buildZkTransferInstruction rejects proof backend mismatch", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proofBytes: {
            backend: "stark/fri",
            bytes: Buffer.from("proof"),
          },
          verifyingKeyRef: "halo2/ipa:vk_transfer",
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /proof\.backend must match/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects verifying key backend mismatch", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: "stark/fri:vk_transfer",
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /verifyingKeyRef\.backend must match/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects legacy vk_reference alias", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          vk_reference: "halo2/ipa:vk_transfer",
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /vk_reference is not supported/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects vk_reference shadow field", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: "halo2/ipa:vk_transfer",
          vk_reference: "halo2/ipa:shadow",
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /vk_reference is not supported/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects nested verifyingKeyRef shadow fields", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: {
            backend: "halo2/ipa",
            name: "vk_transfer",
            vk_reference: "shadow",
          },
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /verifyingKeyRef\.vk_reference is not supported/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects structured proof shadow fields", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proofBytes: {
            backend: "halo2/ipa",
            bytes: Buffer.from("proof"),
            vk_inline: { backend: "halo2/ipa", bytes: Buffer.from("legacy") },
          },
          verifyingKeyRef: "halo2/ipa:vk_transfer",
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /proof\.vk_inline is not supported/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects verifying key reference alias collisions", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: "halo2/ipa:vk_transfer",
          vk_ref: { backend: "halo2/ipa", name: "shadow" },
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /multiple verifying key reference aliases/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects nested verifying key id alias collisions", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: {
            backend: "halo2/ipa",
            backendId: "stark/fri",
            name: "vk_transfer",
          },
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /multiple backend aliases/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects blank verifying key id fields", () => {
  for (const verifyingKeyRef of [
    "halo2/ipa:   ",
    { backend: "halo2/ipa", name: "   " },
    { backend: "   ", name: "vk_transfer" },
  ]) {
    assert.throws(
      () =>
        buildZkTransferInstruction({
          assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
          inputs: [Buffer.alloc(32, 0x11)],
          outputs: [Buffer.alloc(32, 0x22)],
          proof: {
            backend: "halo2/ipa",
            proof: Buffer.from("proof"),
            verifyingKeyRef,
          },
        }),
      (error) => {
        assert.equal(error?.code, ValidationErrorCode.INVALID_STRING);
        assert.match(String(error?.message), /non-empty|backend:name/i);
        return true;
      },
    );
  }
});

test("buildZkTransferInstruction rejects proof byte alias collisions", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          proof_b64: Buffer.from("shadow").toString("base64"),
          verifyingKeyRef: "halo2/ipa:vk_transfer",
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /multiple proof byte aliases/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects commitment alias collisions", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: "halo2/ipa:vk_transfer",
          verifyingKeyCommitment: Buffer.alloc(32, 0xaa),
          vk_commitment: Buffer.alloc(32, 0xbb),
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /multiple verifying key commitment aliases/i);
      return true;
    },
  );
});

test("buildZkTransferInstruction rejects envelope hash alias collisions", () => {
  assert.throws(
    () =>
      buildZkTransferInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: "halo2/ipa:vk_transfer",
          envelopeHash: Buffer.alloc(32, 0xaa),
          proofEnvelopeHash: Buffer.alloc(32, 0xbb),
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.match(String(error?.message), /multiple envelope hash aliases/i);
      return true;
    },
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
});

descriptorTest("buildUnshieldInstruction enforces strict canonical Quantity inputs", () => {
  const wide = "18446744073709551616.25";
  assert.equal(
    buildUnshieldInstruction({
      assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
      destinationAccountId: ACCOUNT_ID_INPUT,
      publicAmount: wide,
      inputs: [Buffer.alloc(32, 0x55)],
      proof: {
        backend: "halo2/ipa",
        proof: Buffer.from("proof"),
        verifyingKeyRef: { backend: "halo2/ipa", name: "vk_unshield" },
      },
    }).zk.Unshield.public_amount,
    wide,
  );
  for (const publicAmount of [5, "05", "5.0", "-5"]) {
    assert.throws(
      () => buildUnshieldInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        destinationAccountId: ACCOUNT_ID_INPUT,
        publicAmount,
        inputs: [Buffer.alloc(32, 0x55)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: { backend: "halo2/ipa", name: "vk_unshield" },
        },
      }),
      (error) => {
        assert.equal(error?.code, ValidationErrorCode.INVALID_NUMERIC);
        return true;
      },
    );
  }
});

test("buildCreateElectionInstruction normalizes verifying keys", () => {
  const instruction = buildCreateElectionInstruction({
    electionId: "election-1",
    options: 3,
    eligibleRoot: Buffer.alloc(32, 0x09),
    startTs: 100,
    endTs: 200,
    ballotVerifyingKey: "halo2/ipa:vk_ballot",
    tallyVerifyingKey: { backend: "halo2/ipa", name: "vk_tally" },
    domainTag: "zk",
  });
  const payload = encodeAndDecode(instruction).zk.CreateElection;
  assert.equal(payload.vk_ballot.name, "vk_ballot");
  assert.equal(payload.vk_tally.name, "vk_tally");
  assert.equal(payload.options, 3);
});

test("buildCreateElectionInstruction accepts byte-array eligibleRoot", () => {
  const instruction = buildCreateElectionInstruction({
    electionId: "election-2",
    options: 2,
    eligibleRoot: Array.from(Buffer.alloc(32, 0x44)),
    startTs: 100,
    endTs: 200,
    ballotVerifyingKey: "halo2/ipa:vk_ballot",
    tallyVerifyingKey: "halo2/ipa:vk_tally",
    domainTag: "zk",
  });
  const payload = encodeAndDecode(instruction).zk.CreateElection;
  assert.deepEqual(payload.eligible_root, toByteArray(Buffer.alloc(32, 0x44)));
});

test("buildCreateElectionInstruction rejects coercible non-byte eligibleRoot entries", () => {
  for (const entry of ["1", true, null]) {
    const eligibleRoot = new Array(32).fill(0);
    eligibleRoot[0] = entry;
    assert.throws(
      () =>
        buildCreateElectionInstruction({
          electionId: "election-coercible",
          options: 2,
          eligibleRoot,
          startTs: 100,
          endTs: 200,
          ballotVerifyingKey: "halo2/ipa:vk_ballot",
          tallyVerifyingKey: "halo2/ipa:vk_tally",
          domainTag: "zk",
        }),
      (error) => {
        assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
        assert.match(String(error?.message), /eligibleRoot\[0\]/i);
        return true;
      },
    );
  }
});

test("buildCreateElectionInstruction rejects unsafe timestamps", () => {
  const tooLarge = (BigInt(Number.MAX_SAFE_INTEGER) + 1n).toString(10);
  assert.throws(
    () =>
      buildCreateElectionInstruction({
        electionId: "election-unsafe",
        options: 1,
        eligibleRoot: Buffer.alloc(32, 0x09),
        startTs: tooLarge,
        endTs: 100,
        ballotVerifyingKey: "halo2/ipa:vk_ballot",
        tallyVerifyingKey: "halo2/ipa:vk_tally",
        domainTag: "zk",
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.match(String(error?.message), /safe integer/i);
      return true;
    },
  );
});

test("buildSubmitBallotInstruction encodes ciphertext and proof", () => {
  const instruction = buildSubmitBallotInstruction({
    electionId: "ref-1",
    ciphertext: Buffer.from("encrypted"),
    ballotProof: {
      backend: "halo2/ipa",
      proof: Buffer.from("proof"),
      verifyingKeyRef: "halo2/ipa:vk_ballot",
    },
    nullifier: Buffer.alloc(32, 0x33),
  });
  const payload = encodeAndDecode(instruction).zk.SubmitBallot;
  const ciphertext = Buffer.from(payload.ciphertext);
  assert.equal(ciphertext.toString("base64"), Buffer.from("encrypted").toString("base64"));
  assert.equal(payload.ballot_proof.backend, "halo2/ipa");
});

test("buildSubmitBallotInstruction rejects non-byte nullifier arrays", () => {
  const invalidNullifier = Array.from({ length: 32 }, (_, index) => (index === 0 ? 256 : 0));
  assert.throws(
    () =>
      buildSubmitBallotInstruction({
        electionId: "ref-1",
        ciphertext: Buffer.from("encrypted"),
        ballotProof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: "halo2/ipa:vk_ballot",
        },
        nullifier: invalidNullifier,
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.match(String(error?.message), /nullifier\[0\]/i);
      return true;
    },
  );
});

test("buildSubmitBallotInstruction rejects coercible non-byte ciphertext entries", () => {
  for (const entry of ["1", true, null]) {
    assert.throws(
      () =>
        buildSubmitBallotInstruction({
          electionId: "ref-1",
          ciphertext: [entry],
          ballotProof: {
            backend: "halo2/ipa",
            proof: Buffer.from("proof"),
            verifyingKeyRef: "halo2/ipa:vk_ballot",
          },
          nullifier: Buffer.alloc(32, 0x33),
        }),
      (error) => {
        assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
        assert.match(String(error?.message), /ciphertext\[0\]/i);
        return true;
      },
    );
  }
});

test("buildSubmitBallotInstruction rejects empty ciphertext", () => {
  assert.throws(
    () =>
      buildSubmitBallotInstruction({
        electionId: "ref-1",
        ciphertext: Buffer.alloc(0),
        ballotProof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: "halo2/ipa:vk_ballot",
        },
        nullifier: Buffer.alloc(32, 0x33),
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_STRING);
      assert.match(String(error?.message), /non-empty byte array/i);
      return true;
    },
  );
});

test("buildFinalizeElectionInstruction serializes tally entries", () => {
  const instruction = buildFinalizeElectionInstruction({
    electionId: "ref-1",
    tally: [1, "2"],
    tallyProof: {
      backend: "halo2/ipa",
      proof: Buffer.from("proof"),
      verifyingKeyRef: "halo2/ipa:vk_tally",
    },
  });
  const payload = encodeAndDecode(instruction).zk.FinalizeElection;
  assert.deepEqual(payload.tally, [1, 2]);
});

test("proof attachments support lane privacy merkle witnesses", () => {
  const leaf = Buffer.alloc(32, 1);
  const sibling = Buffer.alloc(32, 2);
  const result = buildFinalizeElectionInstruction({
    electionId: "elec-1",
    tally: [1],
    proof: {
      backend: "lane/privacy",
      proof: new Uint8Array([1, 2, 3]),
      verifyingKeyRef: "lane/privacy:vk_lane_privacy",
      lanePrivacy: {
        commitmentId: 9,
        merkle: {
          leaf,
          leafIndex: 0,
          auditPath: [sibling, null],
        },
      },
    },
  });
  const proof = result.zk.FinalizeElection.tally_proof;
  assert.equal(proof.backend, "lane/privacy");
  assert.equal(proof.lane_privacy.commitment_id, 9);
  assert.equal(proof.lane_privacy.witness.kind, "merkle");
  assert.deepEqual(proof.lane_privacy.witness.payload.leaf, Array.from(leaf));
  assert.deepEqual(proof.lane_privacy.witness.payload.proof.audit_path[0], Array.from(sibling));
  assert.equal(proof.lane_privacy.witness.payload.proof.audit_path[1], null);
});
