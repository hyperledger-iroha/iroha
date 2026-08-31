import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildBurnAssetInstruction,
  buildMintAssetInstruction,
  buildMintTriggerRepetitionsInstruction,
  buildBurnTriggerRepetitionsInstruction,
  buildRegisterDomainInstruction,
  buildRegisterAccountInstruction,
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
  buildUnregisterKaigiRelayInstruction,
  buildReportKaigiRelayHealthInstruction,
  KAIGI_MAX_PARTICIPANTS_V1,
  KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1,
  KAIGI_RELAY_MANIFEST_MAX_HOPS_V1,
  buildRegisterSmartContractCodeInstruction,
  buildRegisterSmartContractBytesInstruction,
  buildRemoveSmartContractBytesInstruction,
  buildProposeDeployContractInstruction,
  buildCastZkBallotInstruction,
  buildCastPlainBallotInstruction,
  buildSubmitAgendaProposalInstruction,
  buildClaimTwitterFollowRewardInstruction,
  buildSendToTwitterInstruction,
  buildCancelTwitterEscrowInstruction,
  buildRegisterZkAssetInstruction,
  buildScheduleConfidentialPolicyTransitionInstruction,
  buildCancelConfidentialPolicyTransitionInstruction,
  buildCreateElectionInstruction,
  buildSubmitBallotInstruction,
  buildFinalizeElectionInstruction,
} from "../src/instructionBuilders.js";
import * as instructionBuilderExports from "../src/instructionBuilders.js";
import { blake2b256 } from "../src/blake2b.js";
import { analyzeEntrypointValueTypeV1 } from "../src/entrypointSchema.js";
import { isCanonicalGovernanceSelectorV1 } from "../src/governanceSelector.js";
import {
  PROOF_BOX_MAX_ENCODED_BYTES,
  proofBoxEncodedLength,
  proofBoxMaxProofBytes,
} from "../src/proofAttachment.js";
import {
  _createNoritoInstructionApi,
  noritoDecodeInstruction,
  noritoEncodeInstruction,
  validateNoritoFrame,
} from "../src/norito.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import {
  hasNoritoBinding,
  makeNativeTest,
  nativeBinding,
  noritoRequiredMethods,
} from "./helpers/native.js";
import {
  assertNativeAndPureInstructionParity,
  normalizedHashHex,
  toByteArray,
  withPureJsInstructionCodec,
} from "./helpers/instructionCodec.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const descriptorTest = baseTest;
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");
const SORA_I105_DISCRIMINANT = 0x2f1;
const CANCEL_ASSET_LOCK_ESCROW_ID =
  "hash:996264C84790C64086AAB0EF693A1D33EC18FC0B1C1229774C461A00939A6687#F2BD";

baseTest("governance V1 selectors share the exact bounded unreserved grammar", () => {
  assert.equal(isCanonicalGovernanceSelectorV1("referendum-1"), true);
  assert.equal(
    isCanonicalGovernanceSelectorV1(`a${".".repeat(126)}z`),
    true,
  );
  for (const invalid of [
    "",
    ".",
    "..",
    ".hidden",
    "a/b",
    "a%2Fb",
    "has space",
    "a\n",
    "a\0",
    "a\u007f",
    "投票",
    "a".repeat(129),
  ]) {
    assert.equal(isCanonicalGovernanceSelectorV1(invalid), false, invalid);
  }
});

baseTest("all governance instruction builders reject selector aliases", () => {
  const cases = [
    () => buildCastZkBallotInstruction({ electionId: "a/b", proof: "AA==" }),
    () => buildCastPlainBallotInstruction({ referendumId: ".hidden" }),
    () => buildCreateElectionInstruction({ electionId: "a%2Fb" }),
    () => buildSubmitBallotInstruction({ electionId: "a".repeat(129) }),
    () => buildFinalizeElectionInstruction({ electionId: "..", tally: [0] }),
  ];
  for (const build of cases) {
    assert.throws(build, /must be 1-128 RFC 3986 unreserved ASCII/);
  }
});

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
import { AccountAddress } from "../src/address.js";

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

function mlDsaManifestSigner(keyLength, fill = 0x5a) {
  const lengthVarint = [];
  let remaining = keyLength;
  do {
    const byte = remaining & 0x7f;
    remaining = Math.floor(remaining / 0x80);
    lengthVarint.push(remaining === 0 ? byte : byte | 0x80);
  } while (remaining !== 0);
  const multihash = Buffer.concat([
    Buffer.from([0xee, 0x01, ...lengthVarint]),
    Buffer.alloc(keyLength, fill),
  ]);
  return `ml-dsa:${multihash.toString("hex")}`;
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

function buildTruncatedCanonicalHexLiteral(address) {
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
const SAMPLE_ACCOUNT_TRUNCATED_HEX_LITERAL =
  buildTruncatedCanonicalHexLiteral(SAMPLE_ACCOUNT_ADDRESS);

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

function readInstructionEnvelopeWireId(encoded, context) {
  const outer = validateNoritoFrame(encoded);
  const wire = readCompactFieldPayload(outer.payload, 0, `${context}.wire`);
  const wireValue = readCompactFieldPayload(
    wire.payload,
    0,
    `${context}.wire.value`,
  );
  assert.equal(wireValue.next, wire.payload.length);
  const inner = readCompactFieldPayload(
    outer.payload,
    wire.next,
    `${context}.inner`,
  );
  assert.equal(inner.next, outer.payload.length);
  return wireValue.payload.toString("utf8");
}

function encodeAndDecode(
  instruction,
  {
    noritoEncodeInstruction: encode = noritoEncodeInstruction,
    noritoDecodeInstruction: decode = noritoDecodeInstruction,
  } = {},
) {
  let encoded;
  try {
    encoded = encode(instruction);
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
    const decoded = decode(encoded);
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
    const reencoded = encode(canonical);
    assert.deepEqual(toByteArray(encoded), toByteArray(reencoded));
    return canonical;
  }
}

function nativeInstructionDecoder(decoded) {
  return _createNoritoInstructionApi(createNativeRuntime({
    noritoDecodeInstruction() {
      return JSON.stringify(decoded);
    },
  })).noritoDecodeInstruction;
}

const NORITO_CRC64_MASK = 0xffff_ffff_ffff_ffffn;
const NORITO_CRC64_POLY = 0xc96c_5795_d787_0f42n;

function noritoCrc64(payload) {
  let crc = NORITO_CRC64_MASK;
  for (const byte of payload) {
    let tableEntry = (crc ^ BigInt(byte)) & 0xffn;
    for (let bit = 0; bit < 8; bit += 1) {
      tableEntry =
        (tableEntry & 1n) === 0n
          ? tableEntry >> 1n
          : (tableEntry >> 1n) ^ NORITO_CRC64_POLY;
    }
    crc = tableEntry ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ NORITO_CRC64_MASK);
}

function rewriteNoritoFrameCrc(buffer, frameStart, frameEnd) {
  const payloadLength = Number(buffer.readBigUInt64LE(frameStart + 23));
  const payloadStart = frameEnd - payloadLength;
  assert.ok(payloadStart >= frameStart + 40, "Norito payload must follow its header");
  buffer.writeBigUInt64LE(
    noritoCrc64(buffer.subarray(payloadStart, frameEnd)),
    frameStart + 31,
  );
}

function rewriteNestedInstructionCrcs(buffer) {
  const innerStart = buffer.indexOf(Buffer.from("NRT0", "ascii"), 4);
  assert.ok(innerStart > 0, "nested Norito instruction frame must be present");
  rewriteNoritoFrameCrc(buffer, innerStart, buffer.length);
  rewriteNoritoFrameCrc(buffer, 0, buffer.length);
}

function encodeCompactTestLength(length) {
  let remaining = length;
  const bytes = [];
  do {
    const chunk = remaining & 0x7f;
    remaining = Math.floor(remaining / 128);
    bytes.push(remaining === 0 ? chunk : chunk | 0x80);
  } while (remaining !== 0);
  return Buffer.from(bytes);
}

function rebuildNoritoTestFrame(frame, payload) {
  const oldPayloadLength = Number(frame.readBigUInt64LE(23));
  const payloadStart = frame.length - oldPayloadLength;
  const prefix = Buffer.from(frame.subarray(0, payloadStart));
  prefix.writeBigUInt64LE(BigInt(payload.length), 23);
  prefix.writeBigUInt64LE(noritoCrc64(payload), 31);
  return Buffer.concat([prefix, payload]);
}

function appendFinalizeProofAttachmentTail(encoded, tailPayload) {
  const outer = validateNoritoFrame(encoded);
  const wire = readCompactFieldPayload(outer.payload, 0, "outer.wire");
  const innerField = readCompactFieldPayload(outer.payload, wire.next, "outer.inner");
  assert.equal(innerField.next, outer.payload.length);
  const innerFrameLength = Number(innerField.payload.readBigUInt64LE(0));
  const innerFrame = innerField.payload.subarray(8);
  assert.equal(innerFrame.length, innerFrameLength);
  const inner = validateNoritoFrame(innerFrame);
  const election = readCompactFieldPayload(inner.payload, 0, "finalize.election");
  const tally = readCompactFieldPayload(inner.payload, election.next, "finalize.tally");
  const attachment = readCompactFieldPayload(
    inner.payload,
    tally.next,
    "finalize.attachment",
  );
  assert.equal(attachment.next, inner.payload.length);

  const expandedAttachment = Buffer.concat([
    attachment.payload,
    encodeCompactTestLength(tailPayload.length),
    tailPayload,
  ]);
  const rebuiltInnerPayload = Buffer.concat([
    inner.payload.subarray(0, tally.next),
    encodeCompactTestLength(expandedAttachment.length),
    expandedAttachment,
  ]);
  const rebuiltInnerFrame = rebuildNoritoTestFrame(innerFrame, rebuiltInnerPayload);
  const rebuiltInnerFieldPayload = Buffer.allocUnsafe(8 + rebuiltInnerFrame.length);
  rebuiltInnerFieldPayload.writeBigUInt64LE(BigInt(rebuiltInnerFrame.length), 0);
  rebuiltInnerFrame.copy(rebuiltInnerFieldPayload, 8);
  const rebuiltOuterPayload = Buffer.concat([
    outer.payload.subarray(0, wire.next),
    encodeCompactTestLength(rebuiltInnerFieldPayload.length),
    rebuiltInnerFieldPayload,
  ]);
  return rebuildNoritoTestFrame(Buffer.from(encoded), rebuiltOuterPayload);
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

test("normalizeAccountId rejects truncated canonical-hex inputs", () => {
  assert.throws(
    () => exportedNormalizeAccountId(SAMPLE_ACCOUNT_TRUNCATED_HEX_LITERAL),
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
    withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
      noritoEncodeInstruction(implicit)),
  );
  const nativeImplicit = Buffer.from(
    nativeBinding.noritoEncodeInstruction(JSON.stringify(implicit)),
  );
  const pureExplicit = Buffer.from(
    withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
      noritoEncodeInstruction(explicit)),
  );
  assert.deepEqual(pureImplicit, nativeImplicit);
  assert.deepEqual(pureImplicit, pureExplicit);
  assert.deepEqual(
    JSON.parse(nativeBinding.noritoDecodeInstruction(pureImplicit)),
    explicit,
  );
  assert.deepEqual(
    withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(nativeImplicit)),
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
      withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
        noritoEncodeInstruction(decomposed)),
    ),
    Buffer.from(nativeBinding.noritoEncodeInstruction(JSON.stringify(decomposed))),
  );
  assert.deepEqual(
    Buffer.from(
      withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
        noritoEncodeInstruction(decomposed)),
    ),
    Buffer.from(
      withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
        noritoEncodeInstruction(composed)),
    ),
  );

  const invalid = buildTransferNftInstruction({
    sourceAccountId: ACCOUNT_ID,
    nftId: "bad@name$wonderland",
    destinationAccountId: ACCOUNT_ID,
  });
  assert.throws(
    () =>
      withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
        noritoEncodeInstruction(invalid)),
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
      withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
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
const THIRD_RELAY_ACCOUNT_ID = exportedNormalizeAccountId(
  AccountAddress.fromAccount({
    publicKey: hexToBytes(SEED_11_ED25519_PUBLIC_KEY_HEX),
  }).toI105(SORA_I105_DISCRIMINANT),
);
const KAIGI_RELAY_PUBLIC_KEYS_HEX = Object.freeze([
  "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c",
  "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394",
  "ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1",
  "ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c",
  "6e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1",
  "8a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17",
  "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c",
  "1398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca",
]);

function kaigiRelayHops() {
  return [
    {
      relayId: RELAY_ACCOUNT_ID,
      hpkePublicKey: Buffer.alloc(32, 0x01),
      weight: 5,
    },
    {
      relayId: SAMPLE_ACCOUNT_CANONICAL,
      hpkePublicKey: Buffer.alloc(32, 0x02),
      weight: 3,
    },
    {
      relayId: THIRD_RELAY_ACCOUNT_ID,
      hpkePublicKey: Buffer.alloc(32, 0x03),
      weight: 4,
    },
  ];
}

function normalizedKaigiRelayHops() {
  return kaigiRelayHops().map((hop) => ({
    relay_id: hop.relayId,
    hpke_public_key: hop.hpkePublicKey.toString("base64"),
    weight: hop.weight,
  }));
}

function maximumKaigiRelayHops() {
  return KAIGI_RELAY_PUBLIC_KEYS_HEX.map((publicKey, index) => ({
    relayId: exportedNormalizeAccountId(
      AccountAddress.fromAccount({ publicKey: hexToBytes(publicKey) }).toI105(
        SORA_I105_DISCRIMINANT,
      ),
    ),
    hpkePublicKey: Buffer.alloc(
      index === 0 ? KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1 : 1,
      index + 1,
    ),
    weight: 1,
  }));
}

baseTest("Kaigi relay builders enforce the V1 hop and decoded-key bounds", () => {
  assert.equal(KAIGI_RELAY_MANIFEST_MAX_HOPS_V1, 8);
  assert.equal(KAIGI_RELAY_HPKE_PUBLIC_KEY_MAX_BYTES_V1, 4_096);

  const hops = maximumKaigiRelayHops();
  const acceptedManifest = buildSetKaigiRelayManifestInstruction({
    callId: "wonderland.sora:bounded-relays",
    relayManifest: { hops, expiryMs: 1 },
  });
  assert.equal(
    acceptedManifest.Kaigi.SetKaigiRelayManifest.relay_manifest.hops.length,
    8,
  );
  assert.equal(
    Buffer.from(
      acceptedManifest.Kaigi.SetKaigiRelayManifest.relay_manifest.hops[0]
        .hpke_public_key,
      "base64",
    ).length,
    4_096,
  );

  assert.throws(
    () =>
      buildSetKaigiRelayManifestInstruction({
        callId: "wonderland.sora:too-many-relays",
        relayManifest: { hops: [...hops, hops[0]], expiryMs: 1 },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.equal(error?.path, "setKaigiRelayManifest.relayManifest.hops");
      return true;
    },
  );

  const acceptedRegistration = buildRegisterKaigiRelayInstruction({
    relayId: RELAY_ACCOUNT_ID,
    hpkePublicKey: Buffer.alloc(4_096, 0xA5).toString("base64"),
    bandwidthClass: 1,
  });
  assert.equal(
    Buffer.from(
      acceptedRegistration.Kaigi.RegisterKaigiRelay.relay.hpke_public_key,
      "base64",
    ).length,
    4_096,
  );

  const oversizedKey = Buffer.alloc(4_097, 0xA5);
  assert.throws(
    () =>
      buildSetKaigiRelayManifestInstruction({
        callId: "wonderland.sora:oversized-hop-key",
        relayManifest: {
          hops: [
            { ...hops[0], hpkePublicKey: oversizedKey },
            hops[1],
            hops[2],
          ],
          expiryMs: 1,
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.equal(
        error?.path,
        "setKaigiRelayManifest.relayManifest.hops[0].hpkePublicKey",
      );
      return true;
    },
  );
  assert.throws(
    () =>
      buildRegisterKaigiRelayInstruction({
        relayId: RELAY_ACCOUNT_ID,
        hpkePublicKey: oversizedKey.toString("base64"),
        bandwidthClass: 1,
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.equal(error?.path, "registerKaigiRelay.hpkePublicKey");
      return true;
    },
  );
});

baseTest("Kaigi instruction envelopes use the exact V1 registry wire IDs", () => {
  const callId = "wonderland.sora:wire-id";
  const cases = [
    [
      "CreateKaigi",
      buildCreateKaigiInstruction({ id: callId, host: ACCOUNT_ID }),
    ],
    [
      "JoinKaigi",
      buildJoinKaigiInstruction({ callId, participant: ACCOUNT_ID }),
    ],
    [
      "LeaveKaigi",
      buildLeaveKaigiInstruction({ callId, participant: ACCOUNT_ID }),
    ],
    ["EndKaigi", buildEndKaigiInstruction({ callId })],
    [
      "RecordKaigiUsage",
      buildRecordKaigiUsageInstruction({
        callId,
        durationMs: 1,
        billedGas: 2,
      }),
    ],
    [
      "SetKaigiRelayManifest",
      buildSetKaigiRelayManifestInstruction({ callId, relayManifest: null }),
    ],
    [
      "RegisterKaigiRelay",
      buildRegisterKaigiRelayInstruction({
        relayId: RELAY_ACCOUNT_ID,
        hpkePublicKey: Buffer.alloc(32, 0xa5),
        bandwidthClass: 1,
      }),
    ],
    [
      "ReportKaigiRelayHealth",
      buildReportKaigiRelayHealthInstruction({
        callId,
        relayId: RELAY_ACCOUNT_ID,
        status: "Healthy",
        reportedAtMs: 3,
      }),
    ],
  ];

  withPureJsInstructionCodec(({
    noritoDecodeInstruction,
    noritoEncodeInstruction,
  }) => {
    for (const [name, instruction] of cases) {
      const encoded = noritoEncodeInstruction(instruction);
      assert.equal(
        readInstructionEnvelopeWireId(encoded, `Kaigi.${name}`),
        `iroha.instruction.v1::kaigi::${name}`,
      );
      assert.deepEqual(noritoDecodeInstruction(encoded), instruction);
    }
  });
});

function assertKaigiManifestRejected(relayManifest, code, path) {
  assert.throws(
    () =>
      buildCreateKaigiInstruction({
        id: "wonderland.sora:weekly-sync",
        host: ACCOUNT_ID,
        relayManifest,
      }),
    (error) => {
      assert.equal(error?.code, code);
      assert.equal(error?.path, path);
      return true;
    },
  );
}

baseTest("Kaigi relay manifests reject fewer than three hops", () => {
  assertKaigiManifestRejected(
    { expiryMs: 1700111000000 },
    ValidationErrorCode.INVALID_OBJECT,
    "call.relayManifest.hops",
  );
  assertKaigiManifestRejected(
    { expiryMs: 1700111000000, hops: kaigiRelayHops().slice(0, 2) },
    ValidationErrorCode.VALUE_OUT_OF_RANGE,
    "call.relayManifest.hops",
  );
});

baseTest("Kaigi relay manifests reject invalid hop contents", () => {
  const duplicateHops = kaigiRelayHops();
  duplicateHops[2].relayId = duplicateHops[0].relayId;
  assertKaigiManifestRejected(
    { expiryMs: 1700111000000, hops: duplicateHops },
    ValidationErrorCode.INVALID_OBJECT,
    "call.relayManifest.hops[2].relayId",
  );

  const emptyKeyHops = kaigiRelayHops();
  emptyKeyHops[1].hpkePublicKey = Buffer.alloc(0);
  assertKaigiManifestRejected(
    { expiryMs: 1700111000000, hops: emptyKeyHops },
    ValidationErrorCode.INVALID_STRING,
    "call.relayManifest.hops[1].hpkePublicKey",
  );

  const zeroWeightHops = kaigiRelayHops();
  zeroWeightHops[0].weight = 0;
  assertKaigiManifestRejected(
    { expiryMs: 1700111000000, hops: zeroWeightHops },
    ValidationErrorCode.VALUE_OUT_OF_RANGE,
    "call.relayManifest.hops[0].weight",
  );

  const sparseHops = new Array(3);
  sparseHops[0] = kaigiRelayHops()[0];
  sparseHops[2] = kaigiRelayHops()[2];
  assertKaigiManifestRejected(
    { expiryMs: 1700111000000, hops: sparseHops },
    ValidationErrorCode.INVALID_OBJECT,
    "call.relayManifest.hops[1]",
  );
});

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
      hops: kaigiRelayHops(),
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
            hops: normalizedKaigiRelayHops(),
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
      hops: kaigiRelayHops(),
    },
  });
  const encoded = noritoEncodeInstruction(instruction);
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
    commitment: { commitment: commitmentBytes },
    nullifier: { digest: nullifierBytes, issuedAtMs: 0 },
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
          alias_tag: null,
        },
        nullifier: {
          digest: normalizedHashHex(nullifierBytes),
          issued_at_ms: 0,
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
      issuedAtMs: 0,
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
          issued_at_ms: 0,
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

baseTest("buildLeaveKaigiInstruction rejects reserved V1 privacy artifacts", () => {
  assert.throws(
    () =>
      buildLeaveKaigiInstruction({
        callId: "wonderland.sora:weekly-sync",
        participant: ACCOUNT_ID,
        proof: Buffer.from([0x01]),
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_OBJECT);
      assert.equal(error?.path, "leaveKaigi");
      assert.match(error?.message ?? "", /privacy artifacts are reserved/u);
      return true;
    },
  );
});

baseTest("Kaigi builders preserve full-width u64 values and the participant limit", () => {
  const maxU64 = "18446744073709551615";
  const maxParticipants = KAIGI_MAX_PARTICIPANTS_V1;
  assert.equal(maxParticipants, 4_096);
  const create = buildCreateKaigiInstruction({
    id: "wonderland.sora:full-width",
    host: ACCOUNT_ID,
    maxParticipants,
    gasRatePerMinute: BigInt(maxU64),
    scheduledStartMs: maxU64,
    relayManifest: {
      expiryMs: maxU64,
      hops: kaigiRelayHops(),
    },
  });
  assert.equal(create.Kaigi.CreateKaigi.call.max_participants, maxParticipants);
  assert.equal(create.Kaigi.CreateKaigi.call.gas_rate_per_minute, maxU64);
  assert.equal(create.Kaigi.CreateKaigi.call.scheduled_start_ms, maxU64);
  assert.equal(create.Kaigi.CreateKaigi.call.relay_manifest.expiry_ms, maxU64);
  assert.deepEqual(encodeAndDecode(create), create);

  const usage = buildRecordKaigiUsageInstruction({
    callId: "wonderland.sora:full-width",
    durationMs: BigInt(maxU64),
    billedGas: maxU64,
  });
  assert.equal(usage.Kaigi.RecordKaigiUsage.duration_ms, maxU64);
  assert.equal(usage.Kaigi.RecordKaigiUsage.billed_gas, maxU64);
  assert.deepEqual(encodeAndDecode(usage), usage);

  const end = buildEndKaigiInstruction({
    callId: "wonderland.sora:full-width",
    endedAtMs: maxU64,
  });
  assert.equal(end.Kaigi.EndKaigi.ended_at_ms, maxU64);
  assert.deepEqual(encodeAndDecode(end), end);

  const health = buildReportKaigiRelayHealthInstruction({
    callId: "wonderland.sora:full-width",
    relayId: RELAY_ACCOUNT_ID,
    status: "Healthy",
    reportedAtMs: maxU64,
  });
  assert.equal(health.Kaigi.ReportKaigiRelayHealth.reported_at_ms, maxU64);
  assert.deepEqual(encodeAndDecode(health), health);
});

baseTest("Kaigi builders reject values outside their protocol bounds", () => {
  const overflowU64 = "18446744073709551616";
  assert.throws(
    () =>
      buildRecordKaigiUsageInstruction({
        callId: "wonderland.sora:overflow",
        durationMs: overflowU64,
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.equal(error?.path, "recordKaigiUsage.durationMs");
      return true;
    },
  );
  assert.throws(
    () =>
      buildCreateKaigiInstruction({
        id: "wonderland.sora:overflow",
        host: ACCOUNT_ID,
        maxParticipants: 4_097,
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.equal(error?.path, "call.maxParticipants");
      return true;
    },
  );
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
    commitment: { commitment: commitmentBytes },
    nullifier: { digest: nullifierBytes, issuedAtMs: 0 },
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
          alias_tag: null,
        },
        nullifier: {
          digest: normalizedHashHex(nullifierBytes),
          issued_at_ms: 0,
        },
        roster_root: normalizedHashHex(rosterRootBytes),
        proof: proofBytes.toString("base64"),
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

baseTest("Kaigi privacy builders reject ledger-visible identity hints", () => {
  const commitment = Buffer.alloc(32, 0x77);
  const nullifier = Buffer.alloc(32, 0x88);
  assert.throws(
    () =>
      buildCreateKaigiInstruction({
        id: "wonderland.sora:private-room",
        host: ACCOUNT_ID,
        privacyMode: "ZkRosterV1",
        commitment: { commitment, aliasTag: "host" },
      }),
    /aliasTag is off-chain only and must be omitted/u,
  );
  assert.throws(
    () =>
      buildEndKaigiInstruction({
        callId: "wonderland.sora:private-room",
        nullifier: { digest: nullifier, issuedAtMs: 1 },
      }),
    /issuedAtMs is off-chain only and must be zero/u,
  );
  assert.throws(
    () =>
      buildEndKaigiInstruction({
        callId: "wonderland.sora:private-room",
        nullifier: {
          digest: nullifier,
          issued_at_ms: 0,
          issuedAtMs: 1,
        },
      }),
    /issuedAtMs is off-chain only and must be zero/u,
  );
  assert.throws(
    () =>
      buildCreateKaigiInstruction({
        id: "wonderland.sora:private-room",
        host: ACCOUNT_ID,
        privacyMode: { mode: "ZkRosterV1", state: { alias: "host" } },
      }),
    /privacyMode\.state must be null/u,
  );
  assert.throws(
    () =>
      buildCreateKaigiInstruction({
        id: "wonderland.sora:private-room",
        host: ACCOUNT_ID,
        roomPolicy: { policy: "Authenticated", state: "hidden" },
      }),
    /roomPolicy\.state must be null/u,
  );
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
  assertNativeAndPureInstructionParity(
    instruction,
    "Kaigi.RegisterKaigiRelay",
  );
});

test("buildUnregisterKaigiRelayInstruction encodes the canonical relay id", () => {
  const instruction = buildUnregisterKaigiRelayInstruction({
    relayId: RELAY_ACCOUNT_ID,
  });
  const expected = {
    Kaigi: {
      UnregisterKaigiRelay: {
        relay_id: RELAY_ACCOUNT_ID,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
  assertNativeAndPureInstructionParity(
    instruction,
    "Kaigi.UnregisterKaigiRelay",
  );
});

baseTest("RegisterKaigiRelay requires a non-zero bandwidth class", () => {
  const baseRelay = {
    relayId: RELAY_ACCOUNT_ID,
    hpkePublicKey: Buffer.alloc(32, 0xaa),
  };
  for (const [bandwidthClass, code] of [
    [undefined, ValidationErrorCode.INVALID_NUMERIC],
    [0, ValidationErrorCode.VALUE_OUT_OF_RANGE],
  ]) {
    assert.throws(
      () =>
        buildRegisterKaigiRelayInstruction({
          ...baseRelay,
          ...(bandwidthClass === undefined ? {} : { bandwidthClass }),
        }),
      (error) => {
        assert.equal(error?.code, code);
        assert.equal(error?.path, "registerKaigiRelay.bandwidthClass");
        return true;
      },
    );
  }
});

baseTest("RegisterKaigiRelay encodes its HPKE key as a Norito Vec<u8>", () => {
  const hpkePublicKey = Buffer.alloc(32, 0xaa);
  const instruction = buildRegisterKaigiRelayInstruction({
    relayId: RELAY_ACCOUNT_ID,
    hpkePublicKey,
    bandwidthClass: 7,
  });
  const encoded = Buffer.from(
    withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
      noritoEncodeInstruction(instruction)),
  );
  const keyOffset = encoded.indexOf(hpkePublicKey);
  assert.ok(keyOffset >= 8, "encoded HPKE key must have a Vec length prefix");
  const expectedLength = Buffer.alloc(8);
  expectedLength.writeBigUInt64LE(BigInt(hpkePublicKey.length));
  assert.deepEqual(encoded.subarray(keyOffset - 8, keyOffset), expectedLength);
  assert.deepEqual(
    withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(encoded)),
    instruction,
  );
});

test("buildReportKaigiRelayHealthInstruction normalizes relay feedback", () => {
  const instruction = buildReportKaigiRelayHealthInstruction({
    callId: "wonderland.sora:weekly-sync",
    relayId: RELAY_ACCOUNT_ID,
    status: "Degraded",
    reportedAtMs: "1701123456789",
    notes: "latency spike observed",
  });
  const expected = {
    Kaigi: {
      ReportKaigiRelayHealth: {
        call_id: {
          domain_id: "wonderland.sora",
          call_name: "weekly-sync",
        },
        relay_id: RELAY_ACCOUNT_ID,
        status: { status: "Degraded", state: null },
        reported_at_ms: 1701123456789,
        notes: "latency spike observed",
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
  assertNativeAndPureInstructionParity(
    instruction,
    "Kaigi.ReportKaigiRelayHealth",
  );
});

baseTest("ReportKaigiRelayHealth validates status, timestamp, and notes", () => {
  const report = {
    callId: "wonderland.sora:weekly-sync",
    relayId: RELAY_ACCOUNT_ID,
    status: "Healthy",
    reportedAtMs: 7,
  };
  const accepted = buildReportKaigiRelayHealthInstruction({
    ...report,
    notes: "😀".repeat(512),
  });
  assert.equal(
    Array.from(accepted.Kaigi.ReportKaigiRelayHealth.notes).length,
    512,
  );
  assert.deepEqual(
    buildReportKaigiRelayHealthInstruction({
      ...report,
      callId: "Wonderland.SORA:cafe\u0301",
    }).Kaigi.ReportKaigiRelayHealth.call_id,
    {
      domain_id: "wonderland.sora",
      call_name: "caf\u00e9",
    },
  );

  for (const [override, code, path] of [
    [
      { callId: "wonderland.sora:bad\u0000name" },
      ValidationErrorCode.INVALID_STRING,
      "reportKaigiRelayHealth.callId.call_name",
    ],
    [
      { callId: "wonderland:weekly-sync" },
      ValidationErrorCode.INVALID_STRING,
      "reportKaigiRelayHealth.callId.domain_id",
    ],
    [
      { relayId: "not-an-account" },
      ValidationErrorCode.INVALID_ACCOUNT_ID,
      "reportKaigiRelayHealth.relayId",
    ],
    [
      { status: "healthy" },
      ValidationErrorCode.INVALID_STRING,
      "reportKaigiRelayHealth.status",
    ],
    [
      { reportedAtMs: -1 },
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      "reportKaigiRelayHealth.reportedAtMs",
    ],
    [
      { notes: "😀".repeat(513) },
      ValidationErrorCode.VALUE_OUT_OF_RANGE,
      "reportKaigiRelayHealth.notes",
    ],
    [
      { notes: "\ud800" },
      ValidationErrorCode.INVALID_STRING,
      "reportKaigiRelayHealth.notes",
    ],
  ]) {
    assert.throws(
      () => buildReportKaigiRelayHealthInstruction({ ...report, ...override }),
      (error) => {
        assert.equal(error?.code, code);
        assert.equal(error?.path, path);
        return true;
      },
    );
  }
});

baseTest("ReportKaigiRelayHealth pure-JS codec preserves canonical field order", () => {
  const instruction = buildReportKaigiRelayHealthInstruction({
    callId: "wonderland.sora:weekly-sync",
    relayId: RELAY_ACCOUNT_ID,
    status: "Degraded",
    reportedAtMs: 1701123456789,
    notes: "latency spike observed",
  });
  withPureJsInstructionCodec(({ noritoEncodeInstruction }) => {
    const encoded = noritoEncodeInstruction(instruction);
    const outer = validateNoritoFrame(encoded);
    assert.equal(outer.flags, 0x02);
    const wire = readCompactFieldPayload(
      outer.payload,
      0,
      "ReportKaigiRelayHealth.wire",
    );
    const wireValue = readCompactFieldPayload(
      wire.payload,
      0,
      "ReportKaigiRelayHealth.wire.value",
    );
    assert.equal(wireValue.next, wire.payload.length);
    assert.equal(
      wireValue.payload.toString("utf8"),
      "iroha.instruction.v1::kaigi::ReportKaigiRelayHealth",
    );
    const innerField = readCompactFieldPayload(
      outer.payload,
      wire.next,
      "ReportKaigiRelayHealth.inner",
    );
    assert.equal(innerField.next, outer.payload.length);
    const innerFrameLength = Number(innerField.payload.readBigUInt64LE(0));
    const innerFrame = innerField.payload.subarray(8);
    assert.equal(innerFrame.length, innerFrameLength);
    const inner = validateNoritoFrame(innerFrame, {
      expectedTypeName:
        "iroha_data_model::isi::kaigi::ReportKaigiRelayHealth",
      expectedPaddingLength: 0,
    });
    const callId = readCompactFieldPayload(
      inner.payload,
      0,
      "ReportKaigiRelayHealth.call_id",
    );
    const relayId = readCompactFieldPayload(
      inner.payload,
      callId.next,
      "ReportKaigiRelayHealth.relay_id",
    );
    const status = readCompactFieldPayload(
      inner.payload,
      relayId.next,
      "ReportKaigiRelayHealth.status",
    );
    const reportedAt = readCompactFieldPayload(
      inner.payload,
      status.next,
      "ReportKaigiRelayHealth.reported_at_ms",
    );
    const notes = readCompactFieldPayload(
      inner.payload,
      reportedAt.next,
      "ReportKaigiRelayHealth.notes",
    );
    assert.equal(notes.next, inner.payload.length);
    assert.deepEqual(status.payload, Buffer.from([1, 0, 0, 0]));
    assert.equal(reportedAt.payload.readBigUInt64LE(0), 1701123456789n);
    assert.equal(notes.payload[0], 1);
    assert.deepEqual(noritoDecodeInstruction(encoded), instruction);
  });
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

baseTest("buildProposeDeployContractInstruction normalizes the typed V1 payload", () => {
  const instruction = buildProposeDeployContractInstruction({
    proposalOperator: ACCOUNT_ID,
    contractAddress: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
    codeHash: `blake2b32:0x${"AA".repeat(32)}`,
    abiHash: `0X${"BB".repeat(32)}`,
    abiVersion: 1,
  });
  const expected = {
    ProposeDeployContract: {
      proposal_operator: ACCOUNT_ID_CANONICAL,
      contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
      code_hash: "aa".repeat(32),
      abi_hash: "bb".repeat(32),
      abi_version: 1,
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = withPureJsInstructionCodec((codec) =>
    encodeAndDecode(instruction, codec));
  assert.deepEqual(decoded, expected);
});

baseTest("buildProposeDeployContractInstruction encodes manifest provenance after the bound operator", () => {
  const instruction = buildProposeDeployContractInstruction({
    proposalOperator: ACCOUNT_ID,
    contractAddress: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
    codeHash: "aa".repeat(32),
    abiHash: "bb".repeat(32),
    manifestProvenance: {
      signer: `ed25519:ed0120${SEED_11_ED25519_PUBLIC_KEY_HEX}`,
      signature: `ed25519:${"22".repeat(64)}`,
    },
  });
  assert.deepEqual(instruction.ProposeDeployContract.manifest_provenance, {
    signer: `ed0120${SEED_11_ED25519_PUBLIC_KEY_HEX}`,
    signature: "22".repeat(64).toUpperCase(),
  });
  assert.equal(Object.hasOwn(instruction.ProposeDeployContract, "limits"), false);

  const decoded = withPureJsInstructionCodec((codec) =>
    encodeAndDecode(instruction, codec));
  assert.deepEqual(
    decoded.ProposeDeployContract.manifest_provenance,
    instruction.ProposeDeployContract.manifest_provenance,
  );
  assert.equal(Object.hasOwn(decoded.ProposeDeployContract, "limits"), false);
});

baseTest("buildProposeDeployContractInstruction validates ML-DSA manifest signer keys", () => {
  const base = {
    proposalOperator: ACCOUNT_ID,
    contractAddress: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
    codeHash: "aa".repeat(32),
    abiHash: "bb".repeat(32),
  };
  const withSigner = (signer) => ({
    ...base,
    manifestProvenance: {
      signer,
      signature: "22".repeat(64),
    },
  });

  const validSigner = mlDsaManifestSigner(1_952);
  const instruction = buildProposeDeployContractInstruction(withSigner(validSigner));
  assert.equal(
    instruction.ProposeDeployContract.manifest_provenance.signer,
    `ee01a00f${"5A".repeat(1_952)}`,
  );

  for (const [label, signer, errorPattern] of [
    ["one-byte", mlDsaManifestSigner(1), /expected 1952 bytes/u],
    ["short", mlDsaManifestSigner(1_951), /expected 1952 bytes/u],
    ["overlong", mlDsaManifestSigner(1_953), /expected 1952 bytes/u],
    ["all-zero", mlDsaManifestSigner(1_952, 0), /all-zero/u],
  ]) {
    assert.throws(
      () => buildProposeDeployContractInstruction(withSigner(signer)),
      errorPattern,
      `${label} ML-DSA signer must be rejected before instruction emission`,
    );
  }
});

baseTest("buildProposeDeployContractInstruction has a closed canonical local target", () => {
  const base = {
    proposalOperator: ACCOUNT_ID,
    contractAddress: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
    codeHash: "aa".repeat(32),
    abiHash: "bb".repeat(32),
  };
  for (const field of [
    "contractAlias",
    "contract_address",
    "code_hash",
    "abi_hash",
    "abi_version",
    "window",
    "votingMode",
    "mode",
    "limits",
    "manifest_provenance",
  ]) {
    assert.throws(
      () => buildProposeDeployContractInstruction({ ...base, [field]: "retired" }),
      new RegExp(field, "u"),
    );
  }
  for (const contractAddress of [
    base.contractAddress.toUpperCase(),
    ` ${base.contractAddress}`,
    `${base.contractAddress.slice(0, -1)}p`,
    "merchant@paynet",
  ]) {
    assert.throws(
      () => buildProposeDeployContractInstruction({ ...base, contractAddress }),
      /contractAddress|contract address|Bech32/u,
    );
  }
});

baseTest("buildProposeDeployContractInstruction accepts only numeric ABI V1", () => {
  const base = {
    proposalOperator: ACCOUNT_ID,
    contractAddress: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
    codeHash: "aa".repeat(32),
    abiHash: "bb".repeat(32),
  };
  for (const abiVersion of ["1", "01", "1 ", "2", 0, 2, 1n, null]) {
    assert.throws(
      () => buildProposeDeployContractInstruction({ ...base, abiVersion }),
      /exactly 1/u,
    );
  }
  assert.equal(
    buildProposeDeployContractInstruction({ ...base, abiVersion: 1 })
      .ProposeDeployContract.abi_version,
    1,
  );
});

baseTest("buildProposeDeployContractInstruction enforces the governance hash grammar", () => {
  const base = {
    proposalOperator: ACCOUNT_ID,
    contractAddress: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
    codeHash: "aa".repeat(32),
    abiHash: "bb".repeat(32),
  };
  for (const codeHash of [
    ` ${"aa".repeat(32)}`,
    `${"aa".repeat(32)} `,
    `sha256:${"aa".repeat(32)}`,
    `blake2b32:${"aa".repeat(31)}`,
    `blake2b32:0x${"gg".repeat(32)}`,
  ]) {
    assert.throws(
      () => buildProposeDeployContractInstruction({ ...base, codeHash }),
      /codeHash/u,
    );
  }
});

baseTest("governance proposal builder rejects every private-key alias recursively", () => {
  const base = {
    proposalOperator: ACCOUNT_ID,
    contractAddress: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
    codeHash: "aa".repeat(32),
    abiHash: "bb".repeat(32),
  };
  for (const alias of [
    "private_key",
    "privateKey",
    "private_key_hex",
    "privateKeyHex",
    "private_key_bytes",
    "privateKeyBytes",
    "private_key_seed",
    "privateKeySeed",
    "private_key_multihash",
    "privateKeyMultihash",
    "private_key_algorithm",
    "privateKeyAlgorithm",
  ]) {
    assert.throws(
      () =>
        buildProposeDeployContractInstruction({
          ...base,
          nested: [{ [alias]: "secret" }],
        }),
      new RegExp(alias, "u"),
    );
  }
});

baseTest("buildProposeDeployContractInstruction rejects retired lifecycle controls", () => {
  const base = {
    proposalOperator: ACCOUNT_ID,
    contractAddress: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
    codeHash: "aa".repeat(32),
    abiHash: "bb".repeat(32),
  };
  for (const field of ["window", "votingMode", "mode"]) {
    assert.throws(
      () => buildProposeDeployContractInstruction({ ...base, [field]: null }),
      new RegExp(field, "u"),
    );
  }

  for (const field of ["window", "mode", "code_hash_hex", "abi_hash_hex"]) {
    assert.throws(
      () =>
        noritoEncodeInstruction({
          ProposeDeployContract: {
            proposal_operator: ACCOUNT_ID,
            contract_address: base.contractAddress,
            code_hash: base.codeHash,
            abi_hash: base.abiHash,
            abi_version: 1,
            [field]: null,
          },
        }),
      new RegExp(field, "u"),
    );
  }
});

baseTest("buildCastZkBallotInstruction encodes proof and closed public inputs", () => {
  const publicInputs = { direction: "Aye" };
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

baseTest("buildCastZkBallotInstruction defaults public inputs to empty object", () => {
  const instruction = buildCastZkBallotInstruction({
    electionId: "ref-2",
    proof: Buffer.from([0x03]),
  });
  assert.equal(instruction.CastZkBallot.public_inputs_json, "{}");
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, instruction);
});

baseTest("buildCastZkBallotInstruction rejects unsupported public input keys", () => {
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

baseTest("buildCastZkBallotInstruction has a closed camel-case request shape", () => {
  const base = {
    electionId: "ref-closed",
    proof: Buffer.from([0x04]),
  };
  for (const field of [
    "election_id",
    "proofB64",
    "proof_b64",
    "publicInputsJson",
    "public_inputs_json",
  ]) {
    assert.throws(
      () => buildCastZkBallotInstruction({ ...base, [field]: "retired" }),
      new RegExp(field, "u"),
    );
  }
});

baseTest("buildCastZkBallotInstruction canonicalizes all six scalar inputs losslessly", () => {
  const instruction = buildCastZkBallotInstruction({
    electionId: "ref-3",
    proof: Buffer.from([0x04]),
    publicInputs: {
      owner: SAMPLE_ACCOUNT_I105_LITERAL,
      amount: "18446744073709551616.25",
      duration_blocks: 0xffff_ffff_ffff_ffffn,
      direction: "Nay",
      root_hint: `0x${"Aa".repeat(32)}`,
      nullifier: `blake2b32:${"BB".repeat(32)}`,
    },
  });
  assert.equal(
    instruction.CastZkBallot.public_inputs_json,
    `{"root_hint":"${"aa".repeat(32)}","owner":"${SAMPLE_ACCOUNT_I105_LITERAL}","amount":"18446744073709551616.25","duration_blocks":18446744073709551615,"direction":"Nay","nullifier":"${"bb".repeat(32)}"}`,
  );
  const decoded = withPureJsInstructionCodec((codec) =>
    encodeAndDecode(instruction, codec));
  assert.equal(
    decoded.CastZkBallot.public_inputs_json,
    instruction.CastZkBallot.public_inputs_json,
  );
});

baseTest("buildCastZkBallotInstruction rejects formerly accepted meta and badge fields", () => {
  for (const publicInputs of [
    { meta: { z: 1, a: 2 } },
    { badge: "voter" },
    { direction: "Aye", tally: "aye" },
  ]) {
    assert.throws(
      () =>
        buildCastZkBallotInstruction({
          electionId: "ref-4",
          proof: Buffer.from([0x05]),
          publicInputs,
        }),
      /not supported/u,
    );
  }
});

baseTest("buildCastZkBallotInstruction rejects non-object public inputs", () => {
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

baseTest("buildCastZkBallotInstruction requires complete lock hints", () => {
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

baseTest("buildCastZkBallotInstruction rejects noncanonical owner", () => {
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

baseTest("buildCastZkBallotInstruction rejects noncanonical direction and Quantity", () => {
  const base = {
    electionId: "ref-6",
    proof: Buffer.from([0x07]),
  };
  for (const direction of ["aye", "NAY", "Abstain ", 0]) {
    assert.throws(
      () => buildCastZkBallotInstruction({
        ...base,
        publicInputs: { direction },
      }),
      /exactly Aye, Nay, or Abstain/u,
    );
  }
  for (const amount of [250, "01", "-1", "1.", " 1"] ) {
    assert.throws(
      () => buildCastZkBallotInstruction({
        ...base,
        publicInputs: {
          owner: SAMPLE_ACCOUNT_I105_LITERAL,
          amount,
          duration_blocks: 1,
        },
      }),
      /Quantity|quantity|numeric/u,
    );
  }
});

baseTest("buildCastZkBallotInstruction rejects lossy or out-of-range durations", () => {
  const base = {
    electionId: "ref-duration",
    proof: Buffer.from([0x07]),
  };
  for (const duration_blocks of [
    -1,
    Number.MAX_SAFE_INTEGER + 1,
    "01",
    "1 ",
    0x1_0000_0000_0000_0000n,
  ]) {
    assert.throws(
      () => buildCastZkBallotInstruction({
        ...base,
        publicInputs: {
          owner: SAMPLE_ACCOUNT_I105_LITERAL,
          amount: "1",
          duration_blocks,
        },
      }),
      /duration_blocks/u,
    );
  }
});

baseTest("governance ZK ballot rejects every private-key alias recursively", () => {
  for (const alias of [
    "private_key",
    "privateKey",
    "private_key_hex",
    "privateKeyHex",
    "private_key_bytes",
    "privateKeyBytes",
    "private_key_seed",
    "privateKeySeed",
    "private_key_multihash",
    "privateKeyMultihash",
    "private_key_algorithm",
    "privateKeyAlgorithm",
  ]) {
    assert.throws(
      () => buildCastZkBallotInstruction({
        electionId: "ref-secret",
        proof: Buffer.from([0x08]),
        publicInputs: { meta: [{ [alias]: "secret" }] },
      }),
      new RegExp(alias, "u"),
    );
  }
});

baseTest("direct governance Norito validation runs before native dispatch", () => {
  let nativeCalls = 0;
  const encodeWithNative = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction() {
      nativeCalls += 1;
      return Buffer.from([0]);
    },
  })).noritoEncodeInstruction;
  assert.throws(
      () => encodeWithNative({
        ProposeDeployContract: {
          proposal_operator: ACCOUNT_ID,
          contract_address:
            "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
          code_hash: "aa".repeat(32),
          abi_hash: "bb".repeat(32),
          abi_version: 1,
          limits: {},
        },
    }),
    /unknown field limits/u,
  );
  assert.throws(
    () => encodeWithNative({
        CastZkBallot: {
          election_id: "ref-direct",
          proof_b64: "AQ==",
          public_inputs_json: '{"meta":{"privateKey":"secret"}}',
        },
    }),
    /privateKey/u,
  );
  assert.equal(nativeCalls, 0);
});

baseTest("direct pure-JS CastZkBallot preserves a raw max-u64 JSON token", () => {
  const instructionJson = JSON.stringify({
    CastZkBallot: {
      election_id: "ref-direct",
      proof_b64: "AQ==",
      public_inputs_json:
        '{"duration_blocks":18446744073709551615,"owner":"' +
        SAMPLE_ACCOUNT_I105_LITERAL +
        '","amount":"1","direction":"Abstain"}',
    },
  });
  const decoded = withPureJsInstructionCodec(({
    noritoDecodeInstruction,
    noritoEncodeInstruction,
  }) =>
    noritoDecodeInstruction(noritoEncodeInstruction(instructionJson)));
  assert.equal(
    decoded.CastZkBallot.public_inputs_json,
    `{"owner":"${SAMPLE_ACCOUNT_I105_LITERAL}","amount":"1","duration_blocks":18446744073709551615,"direction":"Abstain"}`,
  );
});

baseTest("direct pure-JS deploy proposal roundtrips typed hashes and ABI V1", () => {
  const instructionJson =
    '{"ProposeDeployContract":{' +
    '"contract_address":"irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",' +
    `"code_hash":"${"aa".repeat(32)}",` +
    `"abi_hash":"${"bb".repeat(32)}",` +
    '"abi_version":1}}';
  const decoded = withPureJsInstructionCodec(({
    noritoDecodeInstruction,
    noritoEncodeInstruction,
  }) =>
    noritoDecodeInstruction(noritoEncodeInstruction(instructionJson)));
  assert.equal(decoded.ProposeDeployContract.code_hash, "aa".repeat(32));
  assert.equal(decoded.ProposeDeployContract.abi_hash, "bb".repeat(32));
  assert.equal(decoded.ProposeDeployContract.abi_version, 1);
});

baseTest("buildCastZkBallotInstruction rejects empty proof bytes", () => {
  assert.throws(
    () =>
      buildCastZkBallotInstruction({
        electionId: "ref-1",
        proof: Buffer.alloc(0),
        publicInputs: {},
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
  withPureJsInstructionCodec(({
    noritoDecodeInstruction,
    noritoEncodeInstruction,
  }) => {
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
    unshieldVerifyingKey: { backend: "halo2/ipa", name: "vk_unshield" },
  });
  const payload = encodeAndDecode(instruction).zk.RegisterZkAsset;
  assert.deepEqual(payload.vk_unshield, { backend: "halo2/ipa", name: "vk_unshield" });
});

test("buildRegisterZkAssetInstruction rejects unknown retired fields", () => {
  const base = { assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" };
  assert.throws(
    () =>
      buildRegisterZkAssetInstruction({
        ...base,
        shieldVerifyingKey: "halo2/ipa:vk_shield",
      }),
    /requires vkUnshield/,
  );
  assert.throws(
    () => buildRegisterZkAssetInstruction({ ...base, mode: "Hybrid" }),
    /is not supported/,
  );
  assert.throws(
    () =>
      buildRegisterZkAssetInstruction({
        ...base,
        transferVerifyingKey: "halo2\/ipa:vk_transfer",
      }),
    /is not supported/,
  );
  assert.throws(
    () => buildRegisterZkAssetInstruction({ ...base, allowShield: true }),
    /is not supported/,
  );
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

descriptorTest("retired generic confidential instructions stay absent from builders and codec discriminants", () => {
  const noritoSource = fs.readFileSync(
    path.join(repoRoot, "javascript", "iroha_js", "src", "norito.js"),
    "utf8",
  );
  const retiredVariants = [
    ["Shi", "eld"],
    ["Zk", "Transfer"],
    ["Un", "shield"],
  ].map((parts) => parts.join(""));

  for (const variant of retiredVariants) {
    const builder = ["build", variant, "Instruction"].join("");
    const wireId = ["iroha_data_model::isi::zk::", variant].join("");
    assert.equal(instructionBuilderExports[builder], undefined, builder);
    assert.throws(
      () => noritoEncodeInstruction({ zk: { [variant]: {} } }),
      /retired|does not support|unsupported/u,
      variant,
    );
    assert.equal(noritoSource.includes(wireId), false, wireId);
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
      verifyingKeyRef: { backend: "halo2/ipa", name: "vk_ballot" },
      verifyingKeyCommitment: Buffer.alloc(32, 0x44),
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
          verifyingKeyRef: { backend: "halo2/ipa", name: "vk_ballot" },
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
            verifyingKeyRef: { backend: "halo2/ipa", name: "vk_ballot" },
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
          verifyingKeyRef: { backend: "halo2/ipa", name: "vk_ballot" },
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
      verifyingKeyRef: { backend: "halo2/ipa", name: "vk_tally" },
      verifyingKeyCommitment: Buffer.alloc(32, 0x55),
    },
  });
  const payload = encodeAndDecode(instruction).zk.FinalizeElection;
  assert.deepEqual(payload.tally, [1, 2]);
});

baseTest("proof attachments support lane privacy merkle witnesses", () => {
  const leaf = Buffer.alloc(32, 1);
  const sibling = Buffer.alloc(32, 2);
  const result = buildFinalizeElectionInstruction({
    electionId: "elec-1",
    tally: [1],
    proof: {
      backend: "lane/privacy",
      proof: new Uint8Array([1, 2, 3]),
      verifyingKeyRef: { backend: "lane/privacy", name: "vk_lane_privacy" },
      lanePrivacy: {
        commitmentId: 9,
        merkle: {
          leaf,
          leafIndex: 0,
          auditPath: [sibling],
        },
      },
    },
  });
  const proof = result.zk.FinalizeElection.tally_proof;
  assert.equal(proof.backend, "lane/privacy");
  assert.equal(proof.lane_privacy.commitment_id, 9);
  assert.equal(proof.lane_privacy.witness.kind, "merkle");
  assert.deepEqual(proof.lane_privacy.witness.payload.leaf, Array.from(leaf));
  const canonicalSibling = Array.from(sibling);
  canonicalSibling[31] |= 1;
  assert.deepEqual(
    proof.lane_privacy.witness.payload.proof.audit_path[0],
    canonicalSibling,
  );
  assert.equal(sibling[31], 2, "builder must not mutate caller-owned sibling bytes");
});

baseTest("proof attachments reject empty lane privacy merkle paths", () => {
  assert.throws(
    () =>
      buildFinalizeElectionInstruction({
        electionId: "elec-1",
        tally: [1],
        proof: {
          backend: "lane/privacy",
          proof: new Uint8Array([1, 2, 3]),
          verifyingKeyRef: { backend: "lane/privacy", name: "vk_lane_privacy" },
          lanePrivacy: {
            commitmentId: 9,
            merkle: {
              leaf: Buffer.alloc(32, 1),
              leafIndex: 0,
              auditPath: [],
            },
          },
        },
      }),
    /must contain 1\.\.=255 siblings/,
  );
});

baseTest("proof attachments reject malformed and impossible lane Merkle witnesses", () => {
  const baseMerkle = {
    leaf: Buffer.alloc(32, 1),
    leafIndex: 0,
    auditPath: [Buffer.alloc(32, 2)],
  };
  const attacks = [
    [{ ...baseMerkle, auditPath: [null] }, /must contain a sibling/],
    [{ ...baseMerkle, auditPath: [undefined] }, /must contain a sibling/],
    [{ ...baseMerkle, auditPath: [Buffer.alloc(31)] }, /must be 32 bytes/],
    [{ ...baseMerkle, auditPath: [Buffer.alloc(33)] }, /must be 32 bytes/],
    [
      { ...baseMerkle, auditPath: Array.from({ length: 256 }, () => Buffer.alloc(32)) },
      /1\.\.=255 siblings/,
    ],
    [{ ...baseMerkle, leafIndex: 2 }, /impossible for the Merkle path depth/],
    [{ ...baseMerkle, leafIndex: 0x1_0000_0000 }, /must fit within a u32/],
  ];
  for (const [merkle, expected] of attacks) {
    assert.throws(
      () =>
        buildFinalizeElectionInstruction({
          electionId: "elec-1",
          tally: [1],
          proof: {
            backend: "lane/privacy",
            proof: new Uint8Array([1, 2, 3]),
            verifyingKeyRef: {
              backend: "lane/privacy",
              name: "vk_lane_privacy",
            },
            lanePrivacy: { commitmentId: 0, merkle },
          },
        }),
      expected,
    );
  }
});

baseTest("pure JS Norito rejects non-canonical lane HashOf markers", () => {
  const instruction = buildFinalizeElectionInstruction({
    electionId: "elec-1",
    tally: [1],
    proof: {
      backend: "lane/privacy",
      proof: new Uint8Array([1, 2, 3]),
      verifyingKeyRef: {
        backend: "lane/privacy",
        name: "vk_lane_privacy",
      },
      lanePrivacy: {
        commitmentId: 9,
        merkle: {
          leaf: Buffer.alloc(32, 1),
          leafIndex: 0,
          auditPath: [Buffer.alloc(32, 2)],
        },
      },
    },
  });
  const encoded = Buffer.from(
    withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
      noritoEncodeInstruction(instruction)),
  );
  const decoded = withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
    noritoDecodeInstruction(encoded),
  );
  const decodedSibling =
    decoded.zk.FinalizeElection.tally_proof.lane_privacy.witness.payload.proof
      .audit_path[0];
  assert.match(decodedSibling, /^hash:[0-9A-F]{64}#[0-9A-F]{4}$/);
  assert.equal(Number.parseInt(decodedSibling.slice(67, 69), 16) & 1, 1);
  instruction.zk.FinalizeElection.tally_proof.lane_privacy.witness.payload.proof
    .audit_path[0][31] &= 0xfe;
  assert.throws(
    () => withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
      noritoEncodeInstruction(instruction)),
    /native hash with its marker bit set|canonical prehashed HashOf/,
  );

  const canonicalSibling = Buffer.alloc(32, 2);
  canonicalSibling[31] |= 1;
  const siblingOffset = encoded.indexOf(canonicalSibling);
  assert.ok(siblingOffset > 0, "encoded lane sibling must be present");
  const countOffset = siblingOffset - 11;
  const leafIndexOffset = countOffset - 5;
  assert.equal(encoded.readBigUInt64LE(countOffset), 1n);
  assert.equal(encoded.readUInt32LE(leafIndexOffset), 0);

  const missingSibling = Buffer.from(encoded);
  assert.equal(missingSibling[siblingOffset - 2], 1, "sibling option must be Some");
  missingSibling[siblingOffset - 2] = 0;
  rewriteNestedInstructionCrcs(missingSibling);
  assert.throws(
    () => withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(missingSibling)),
    /None option contained trailing bytes/,
  );

  const emptyPath = Buffer.from(encoded);
  emptyPath.writeBigUInt64LE(0n, countOffset);
  rewriteNestedInstructionCrcs(emptyPath);
  assert.throws(
    () => withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(emptyPath)),
    /trailing bytes/,
  );

  const deepPath = Buffer.from(encoded);
  deepPath.writeBigUInt64LE(256n, countOffset);
  rewriteNestedInstructionCrcs(deepPath);
  assert.throws(
    () => withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(deepPath)),
    /exceeds the 255-item limit/,
  );

  const impossibleIndex = Buffer.from(encoded);
  impossibleIndex.writeUInt32LE(2, leafIndexOffset);
  rewriteNestedInstructionCrcs(impossibleIndex);
  assert.throws(
    () => withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(impossibleIndex)),
    /impossible for the Merkle path depth/,
  );

  encoded[siblingOffset + 31] &= 0xfe;
  rewriteNestedInstructionCrcs(encoded);
  assert.throws(
    () => withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(encoded)),
    /native hash with its marker bit set|canonical prehashed HashOf/,
  );
});

baseTest("pure JS ProofAttachment decoder rejects invalid ids and extra tails", () => {
  const proofBytes = Buffer.from([0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe]);
  const instruction = buildFinalizeElectionInstruction({
    electionId: "elec-1",
    tally: [1],
    proof: {
      backend: "lane/privacy",
      proof: proofBytes,
      verifyingKeyRef: {
        backend: "lane/privacy",
        name: "unique_vk_name",
      },
      lanePrivacy: {
        commitmentId: 7,
        merkle: {
          leaf: Buffer.alloc(32, 1),
          leafIndex: 0,
          auditPath: [Buffer.alloc(32, 2)],
        },
      },
    },
  });
  const encoded = Buffer.from(
    withPureJsInstructionCodec(({ noritoEncodeInstruction }) =>
      noritoEncodeInstruction(instruction)),
  );
  const invalidId = Buffer.from(encoded);
  const nameOffset = invalidId.indexOf(Buffer.from("unique_vk_name", "utf8"));
  assert.ok(nameOffset > 0, "encoded verifying-key name must be present");
  invalidId[nameOffset] = "U".charCodeAt(0);
  rewriteNestedInstructionCrcs(invalidId);
  assert.throws(
    () => withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(invalidId)),
    /portable verifier-key registry syntax/,
  );

  const oversizedProof = Buffer.from(encoded);
  const proofOffset = oversizedProof.indexOf(proofBytes);
  assert.ok(proofOffset > 8, "encoded proof bytes must be present");
  const proofLengthOffset = proofOffset - 8;
  assert.equal(
    oversizedProof.readBigUInt64LE(proofLengthOffset),
    BigInt(proofBytes.length),
  );
  oversizedProof.writeBigUInt64LE(
    BigInt(proofBoxMaxProofBytes("lane/privacy") + 1),
    proofLengthOffset,
  );
  rewriteNestedInstructionCrcs(oversizedProof);
  assert.throws(
    () =>
      withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
        noritoDecodeInstruction(oversizedProof),
      ),
    /exceeds its \d+-byte decoding limit/,
  );

  const extraTail = appendFinalizeProofAttachmentTail(encoded, Buffer.of(0));
  assert.throws(
    () => withPureJsInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(extraTail)),
    /trailing bytes/,
  );

  const invalidNativeResult = JSON.parse(JSON.stringify(instruction));
  invalidNativeResult.zk.FinalizeElection.tally_proof.lane_privacy.witness.payload
    .proof.audit_path = [];
  for (const options of [undefined, { parseJson: false }]) {
    const decodeWithNative = nativeInstructionDecoder(invalidNativeResult);
    assert.throws(
      () => decodeWithNative(Buffer.of(1), options),
      /must contain 1\.\.=255 siblings/,
    );
  }
});
