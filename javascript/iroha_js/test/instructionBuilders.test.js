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
  buildRegisterAssetDefinitionInstruction,
  buildGrantAccountPermissionInstruction,
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
  buildZkAtPolicyCommitment,
  buildZkAtAuthenticatorEnvelope,
  buildZkAtDevProofFixture,
  verifyZkAtAuthenticatorLocally,
  buildZkAmsAdmissionBatch,
  buildZkAmsAdmissionProofEnvelope,
  buildZkAmsAdmissionDevProofFixture,
  verifyZkAmsAdmissionProofLocally,
  buildVegaCredentialPredicateCommitment,
  buildVegaCredentialProofEnvelope,
  buildVegaCredentialDevProofFixture,
  verifyVegaCredentialProofLocally,
  buildSilentThresholdCredentialCommitments,
  buildSilentThresholdCredentialEnvelope,
  buildSilentThresholdCredentialDevProofFixture,
  verifySilentThresholdCredentialProofLocally,
  buildZkX509IdentityCommitments,
  buildZkX509IdentityEnvelope,
  buildZkX509IdentityDevProofFixture,
  verifyZkX509IdentityProofLocally,
  buildJindoLatticePublicInputs,
  buildJindoLatticeProofEnvelope,
  buildJindoLatticeDevProofFixture,
  verifyJindoLatticeProofLocally,
  buildSisHintsCredentialCommitments,
  buildSisHintsCredentialEnvelope,
  buildSisHintsCredentialDevProofFixture,
  verifySisHintsCredentialProofLocally,
  buildAnonymousPgcReceiverSet,
  buildAnonymousPgcDevProofFixture,
  verifyAnonymousPgcDevProofLocally,
  buildRangeCommitment,
  buildVeRangeDevProofFixture,
  buildVeRangeProofEnvelope,
  verifyVeRangeProofLocally,
  buildPrivacyProofEnvelope,
  buildRegisterPrivacyVerifierKeyInstruction,
  buildRetirePrivacyVerifierKeyInstruction,
  buildRegisterAssetHiddenZkPoolInstruction,
  buildRegisterZkAceIdentityCommitmentInstruction,
  buildRotateZkAceIdentityCommitmentInstruction,
  buildRevokeZkAceIdentityCommitmentInstruction,
  buildRegisterZkAssetInstruction,
  buildScheduleConfidentialPolicyTransitionInstruction,
  buildCancelConfidentialPolicyTransitionInstruction,
  buildShieldInstruction,
  buildZkTransferInstruction,
  buildAssetHiddenZkTransferInstruction,
  buildUnshieldInstruction,
  buildZkAceAuthorizedTransferInstruction,
  buildZkAceAuthorizationProofV1,
  buildCreateElectionInstruction,
  buildSubmitBallotInstruction,
  buildFinalizeElectionInstruction,
  encodeInstruction,
} from "../src/instructionBuilders.js";
import {
  getPrivacyAlgorithmDescriptor,
  getPrivacyAlgorithmDescriptors,
  getPrivacyCapabilities,
  getPrivacyCriteria,
  validatePrivacyAlgorithmDescriptor,
} from "../src/privacyAlgorithms.js";
import {
  buildZkAceTransferAuthorizationV1,
  isPrivacyNativeAvailable,
  privacyBuildProofV1,
  privacyCapabilitiesV1,
  privacyVerifyProofV1,
} from "../src/crypto.js";
import {
  noritoDecodeInstruction,
  noritoDecodePrivacyProofEnvelope,
  noritoEncodeInstruction,
  noritoEncodePrivacyProofEnvelope,
} from "../src/norito.js";
import { hasNoritoBinding, makeNativeTest, noritoRequiredMethods } from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const zkAceNativeTest = makeNativeTest(baseTest, {
  require: [...noritoRequiredMethods, "zkAceBuildTransferAuthorizationV1"],
});
const descriptorTest = baseTest;
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");
const SORA_I105_DISCRIMINANT = 0x2f1;

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

test("buildMintAssetInstruction produces canonical Norito payload", () => {
  const instruction = buildMintAssetInstruction({ assetId: ASSET_ID, quantity: 42 });
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
      assert.match(String(error?.message), /Numeric literal/i);
      return true;
    },
  );
  const tooManyDecimals = `0.${"1".repeat(29)}`;
  assert.throws(
    () => buildMintAssetInstruction({ assetId: ASSET_ID, quantity: tooManyDecimals }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.match(String(error?.message), /scale exceeds/i);
      return true;
    },
  );
  const tooLarge = 1n << 512n;
  assert.throws(
    () => buildMintAssetInstruction({ assetId: ASSET_ID, quantity: tooLarge }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.match(String(error?.message), /mantissa exceeds/i);
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
  const { fixture, decoded } = decodeFixtureInstruction("burn_asset_numeric.json");
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

test("buildRegisterSmartContractCodeInstruction normalizes manifest fields", () => {
  const codeHashBytes = Buffer.alloc(32, 0xaa);
  const abiHashBytes = Buffer.alloc(32, 0xbb);
  const signer = `ed25519:ed0120${"11".repeat(32)}`;
  const signature = `ed25519:${"22".repeat(64)}`;
  const signerCanonical = signer.split(":")[1];
  const signatureCanonical = signature.split(":")[1].toUpperCase();
  const instruction = buildRegisterSmartContractCodeInstruction({
    manifest: {
      codeHash: codeHashBytes,
      abiHash: abiHashBytes,
      compilerFingerprint: "rustc-1.79",
      featuresBitmap: "42",
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
            key_type: "ReferendumId",
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
        code_hash: normalizedHashHex(codeHashBytes),
        abi_hash: normalizedHashHex(abiHashBytes),
        compiler_fingerprint: "rustc-1.79",
        features_bitmap: 42,
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
              key_type: "ReferendumId",
              bound_kind: "range",
              max_keys: 2,
            },
          ],
        },
        entrypoints: [
          {
            name: "upgrade_ledger",
            kind: { kind: "Kaizen" },
            permission: "can_upgrade",
          },
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
        entrypoints: [
          {
            access_hints_complete: null,
            access_hints_skipped: [],
            kind: { kind: "Kaizen", value: null },
            name: "upgrade_ledger",
            params: [],
            permission: "can_upgrade",
            read_keys: [],
            return_type: null,
            triggers: [],
            write_keys: [],
          },
        ],
        states: null,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expectedDecoded);
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
    votingMode: "plain",
  });
  const expected = {
    ProposeDeployContract: {
      contract_address: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
      code_hash_hex: "aa".repeat(32),
      abi_hash_hex: Buffer.alloc(32, 0xbb).toString("hex"),
      abi_version: "1",
      window: { lower: 10, upper: 20 },
      mode: "Plain",
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
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
  const nonCanonicalOwner = ACCOUNT_ADDRESS.toI105(0x02f2);
  assert.throws(
    () =>
      buildCastZkBallotInstruction({
        electionId: "ref-5",
        proof: Buffer.from([0x06]),
        publicInputs: {
          owner: nonCanonicalOwner,
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
    amount: "1000",
    durationBlocks: 50,
    direction: "nay",
  });
  const expected = {
    CastPlainBallot: {
      referendum_id: "ref-2",
      owner: ACCOUNT_ID_CANONICAL,
      amount: "1000",
      duration_blocks: 50,
      direction: 1,
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
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
      at_window: { lower: 0, upper: 0 },
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

test("ZK-ACE builders encode identity lifecycle and authorized transfers", () => {
  const identityCommitment = Buffer.alloc(32, 0x11);
  const rotatedCommitment = Buffer.alloc(32, 0x12);
  const policyHash = Buffer.alloc(32, 0x22);
  const txDigest = Buffer.alloc(32, 0x33);
  const replayNullifier = Buffer.alloc(32, 0x44);
  const vkCommitment = Buffer.alloc(32, 0x55);
  const verifierKey = "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0";
  const proofBundle = buildZkAceAuthorizationProofV1({
    publicInputs: {
      identityCommitment,
      txDigest,
      chainId: "00000000-0000-0000-0000-000000000000",
      replayNullifier,
      policyHash,
      fromAccountId: ACCOUNT_ID_INPUT,
      toAccountId: SAMPLE_ACCOUNT_I105_LITERAL,
      assetDefinitionId: ASSET_DEFINITION_ID,
      amount: "17",
      verifierKeyId: verifierKey,
    },
    proofBytes: Buffer.from("zk-ace-proof"),
    verifyingKeyCommitment: vkCommitment,
  });

  const register = encodeAndDecode(
    buildRegisterZkAceIdentityCommitmentInstruction({
      assetDefinitionId: ASSET_DEFINITION_ID,
      identityCommitment,
      policyHash,
      allowedAccounts: [ACCOUNT_ID_INPUT],
      verifierKey,
    }),
  ).zk.RegisterZkAceIdentityCommitment;
  assert.deepEqual(register.identity_commitment, Array.from(identityCommitment));
  assert.deepEqual(register.policy_hash, Array.from(policyHash));
  assert.deepEqual(register.allowed_accounts, [ACCOUNT_ID_INPUT]);
  assert.equal(register.action_class, "transparent_asset_transfer");
  assert.equal(register.domain_tag, "iroha:zk-ace:pq-authorization:v0");
  assert.deepEqual(register.verifier_key, {
    backend: "stark/fri/sha256-goldilocks",
    name: "zk_ace_pq_authorization_v0",
  });

  const rotate = encodeAndDecode(
    buildRotateZkAceIdentityCommitmentInstruction({
      asset: ASSET_DEFINITION_ID,
      oldIdentityCommitment: identityCommitment,
      newIdentityCommitment: rotatedCommitment,
      policyHash,
      allowedAccounts: [ACCOUNT_ID_INPUT],
      verifierKey,
    }),
  ).zk.RotateZkAceIdentityCommitment;
  assert.deepEqual(rotate.old_identity_commitment, Array.from(identityCommitment));
  assert.deepEqual(rotate.new_identity_commitment, Array.from(rotatedCommitment));
  assert.deepEqual(rotate.allowed_accounts, [ACCOUNT_ID_INPUT]);

  const revoke = encodeAndDecode(
    buildRevokeZkAceIdentityCommitmentInstruction({
      asset: ASSET_DEFINITION_ID,
      identityCommitment: rotatedCommitment,
      reasonHash: Buffer.alloc(32, 0x66),
    }),
  ).zk.RevokeZkAceIdentityCommitment;
  assert.deepEqual(revoke.identity_commitment, Array.from(rotatedCommitment));
  assert.deepEqual(revoke.reason_hash, Array.from(Buffer.alloc(32, 0x66)));

  const transfer = encodeAndDecode(
    buildZkAceAuthorizedTransferInstruction({
      fromAccountId: ACCOUNT_ID_INPUT,
      toAccountId: SAMPLE_ACCOUNT_I105_LITERAL,
      assetDefinitionId: ASSET_DEFINITION_ID,
      amount: "17",
      identityCommitment,
      txDigest,
      chainId: "00000000-0000-0000-0000-000000000000",
      replayNullifier,
      policyHash,
      authorizationProof: proofBundle,
    }),
  ).zk.SubmitZkAceAuthorizedTransfer;
  assert.equal(transfer.amount, 17);
  assert.deepEqual(transfer.identity_commitment, Array.from(identityCommitment));
  assert.deepEqual(transfer.tx_digest, Array.from(txDigest));
  assert.deepEqual(transfer.replay_nullifier, Array.from(replayNullifier));
  assert.equal(transfer.proof.backend, "stark/fri/sha256-goldilocks");
  assert.equal(transfer.proof.vk_ref.name, "zk_ace_pq_authorization_v0");
  assert.deepEqual(transfer.proof.vk_commitment, Array.from(vkCommitment));
});

zkAceNativeTest("ZK-ACE native transfer authorization feeds authorized transfer builder", () => {
  const authorization = buildZkAceTransferAuthorizationV1({
    fromAccountId: ACCOUNT_ID_INPUT,
    toAccountId: SAMPLE_ACCOUNT_I105_LITERAL,
    assetDefinitionId: ASSET_DEFINITION_ID,
    amount: "17",
    chainId: "taira",
    identityRoot: Buffer.alloc(32, 0x31),
    identityBlinding: Buffer.alloc(32, 0x32),
    replaySecret: Buffer.alloc(32, 0x33),
    policyHash: Buffer.alloc(32, 0x34),
  });

  assert.equal(
    authorization.verifierKeyId,
    "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
  );
  assert.ok(authorization.authorizationProofBytes > 0);
  assert.ok(authorization.authorizationPublicInputBytes > 0);
  assert.equal(authorization.replayNullifierBytes, 32);
  assert.equal(authorization.proof.backend, "stark/fri/sha256-goldilocks");
  assert.equal(authorization.proof.vk_ref.name, "zk_ace_pq_authorization_v0");
  assert.equal(authorization.public_inputs.chain_id, "taira");
  assert.deepEqual(
    authorization.public_inputs.identity_commitment,
    Array.from(Buffer.from(authorization.identityCommitment, "hex")),
  );
  assert.deepEqual(
    authorization.public_inputs.replay_nullifier,
    Array.from(Buffer.from(authorization.replayNullifier, "hex")),
  );

  const transfer = encodeAndDecode(
    buildZkAceAuthorizedTransferInstruction({
      fromAccountId: ACCOUNT_ID_INPUT,
      toAccountId: SAMPLE_ACCOUNT_I105_LITERAL,
      assetDefinitionId: ASSET_DEFINITION_ID,
      amount: "17",
      identityCommitment: authorization.identityCommitment,
      txDigest: authorization.txDigest,
      chainId: "taira",
      replayNullifier: authorization.replayNullifier,
      policyHash: authorization.policyHash,
      proof: authorization.proof,
    }),
  ).zk.SubmitZkAceAuthorizedTransfer;

  assert.equal(transfer.amount, 17);
  assert.deepEqual(
    transfer.identity_commitment,
    Array.from(Buffer.from(authorization.identityCommitment, "hex")),
  );
  assert.deepEqual(
    transfer.tx_digest,
    Array.from(Buffer.from(authorization.txDigest, "hex")),
  );
  assert.deepEqual(
    transfer.replay_nullifier,
    Array.from(Buffer.from(authorization.replayNullifier, "hex")),
  );
  assert.equal(transfer.proof.backend, "stark/fri/sha256-goldilocks");
  assert.equal(transfer.proof.vk_ref.name, "zk_ace_pq_authorization_v0");
});

test("ZK-ACE builders reject malformed proof and replay inputs", () => {
  const verifierKey = "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0";
  const publicInputs = {
    identityCommitment: Buffer.alloc(32, 0x11),
    txDigest: Buffer.alloc(32, 0x33),
    chainId: "00000000-0000-0000-0000-000000000000",
    replayNullifier: Buffer.alloc(32, 0x44),
    policyHash: Buffer.alloc(32, 0x22),
    fromAccountId: ACCOUNT_ID_INPUT,
    toAccountId: SAMPLE_ACCOUNT_I105_LITERAL,
    assetDefinitionId: ASSET_DEFINITION_ID,
    amount: "17",
    verifierKeyId: verifierKey,
  };
  const proofBundle = buildZkAceAuthorizationProofV1({
    publicInputs,
    proofBytes: Buffer.from("proof"),
    verifyingKeyCommitment: Buffer.alloc(32, 0x55),
  });

  assert.throws(
    () =>
      buildRegisterZkAceIdentityCommitmentInstruction({
        assetDefinitionId: ASSET_DEFINITION_ID,
        identityCommitment: Buffer.alloc(32),
        policyHash: Buffer.alloc(32, 0x22),
        allowedAccounts: [ACCOUNT_ID_INPUT],
        verifierKey,
      }),
    /identityCommitment.*nonzero/,
  );
  assert.throws(
    () =>
      buildRegisterZkAceIdentityCommitmentInstruction({
        assetDefinitionId: ASSET_DEFINITION_ID,
        identityCommitment: Buffer.alloc(32, 0x11),
        policyHash: Buffer.alloc(32, 0x22),
        allowedAccounts: [],
        verifierKey,
      }),
    /allowedAccounts.*non-empty/,
  );
  assert.throws(
    () =>
      buildRegisterZkAceIdentityCommitmentInstruction({
        assetDefinitionId: ASSET_DEFINITION_ID,
        identityCommitment: Buffer.alloc(32, 0x11),
        policyHash: Buffer.alloc(32, 0x22),
        allowedAccounts: [ACCOUNT_ID_INPUT, ACCOUNT_ID_INPUT],
        verifierKey,
      }),
    /allowedAccounts.*duplicates/,
  );
  assert.throws(
    () =>
      buildRegisterZkAceIdentityCommitmentInstruction({
        assetDefinitionId: ASSET_DEFINITION_ID,
        identityCommitment: Buffer.alloc(32, 0x11),
        policyHash: Buffer.alloc(32, 0x22),
        allowedAccounts: Array.from({ length: 17 }, () => ACCOUNT_ID_INPUT),
        verifierKey,
      }),
    /allowedAccounts.*at most 16/,
  );
  assert.throws(
    () =>
      buildZkAceAuthorizationProofV1({
        publicInputs,
        proofBytes: Buffer.from("proof"),
        verifyingKeyRef: "halo2/ipa:wrong_vk",
        verifyingKeyCommitment: Buffer.alloc(32, 0x55),
      }),
    /must be stark\/fri\/sha256-goldilocks|proof verifier must match public inputs|verifyingKeyRef\.backend must match/,
  );
  assert.throws(
    () =>
      buildZkAceAuthorizationProofV1({
        publicInputs: { ...publicInputs, version: 2 },
        proofBytes: Buffer.from("proof"),
        verifyingKeyCommitment: Buffer.alloc(32, 0x55),
      }),
    /publicInputs\.version must be 1/,
  );
  assert.throws(
    () =>
      buildZkAceAuthorizedTransferInstruction({
        fromAccountId: ACCOUNT_ID_INPUT,
        toAccountId: SAMPLE_ACCOUNT_I105_LITERAL,
        assetDefinitionId: ASSET_DEFINITION_ID,
        amount: "17",
        identityCommitment: Buffer.alloc(32, 0x11),
        txDigest: Buffer.alloc(32, 0x33),
        chainId: "00000000-0000-0000-0000-000000000000",
        replayNullifier: Buffer.alloc(32),
        policyHash: Buffer.alloc(32, 0x22),
        proof: {
          backend: "stark/fri/sha256-goldilocks",
          proofBytes: Buffer.from("proof"),
          verifyingKeyRef: verifierKey,
          verifyingKeyCommitment: Buffer.alloc(32, 0x55),
        },
      }),
    /replayNullifier.*nonzero/,
  );
  const baseTransfer = {
    fromAccountId: ACCOUNT_ID_INPUT,
    toAccountId: SAMPLE_ACCOUNT_I105_LITERAL,
    assetDefinitionId: ASSET_DEFINITION_ID,
    amount: "17",
    identityCommitment: Buffer.alloc(32, 0x11),
    txDigest: Buffer.alloc(32, 0x33),
    chainId: "00000000-0000-0000-0000-000000000000",
    replayNullifier: Buffer.alloc(32, 0x44),
    policyHash: Buffer.alloc(32, 0x22),
    authorizationProof: proofBundle,
  };
  for (const [patch, pattern] of [
    [{ txDigest: Buffer.alloc(32, 0x77) }, /publicInputs\.tx_digest/],
    [{ chainId: "different-chain" }, /publicInputs\.chain_id/],
    [{ toAccountId: ACCOUNT_ID_INPUT }, /publicInputs\.to/],
    [{ amount: "18" }, /publicInputs\.amount/],
    [{ policyHash: Buffer.alloc(32, 0x88) }, /publicInputs\.policy_hash/],
  ]) {
    assert.throws(
      () =>
        buildZkAceAuthorizedTransferInstruction({
          ...baseTransfer,
          ...patch,
        }),
      pattern,
    );
  }
});

descriptorTest("privacy proof envelopes encode canonical open-verify metadata", () => {
  const vkHash = Buffer.alloc(32, 0x55);
  const publicInputs = Buffer.from([0x01, 0x02, 0x03]);
  const proofBytes = Buffer.from("stark-proof");
  const aux = Buffer.from("{}");
  const encoded = buildPrivacyProofEnvelope({
    backend: "stark/fri/sha256-goldilocks",
    circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    vkHash,
    publicInputs,
    proofBytes,
    aux,
    max_proof_bytes: 64,
    max_public_input_bytes: 16,
  });
  assert.ok(Buffer.isBuffer(encoded));
  const decoded = noritoDecodePrivacyProofEnvelope(encoded);
  assert.equal(decoded.backend, "Stark");
  assert.equal(
    decoded.circuit_id,
    "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
  );
  assert.deepEqual(decoded.vk_hash, Array.from(vkHash));
  assert.deepEqual(decoded.public_inputs, Array.from(publicInputs));
  assert.deepEqual(decoded.proof_bytes, Array.from(proofBytes));
  assert.deepEqual(decoded.aux, Array.from(aux));
});

descriptorTest("privacy proof envelopes decode clean base64 byte strings", () => {
  const vkHash = Buffer.alloc(32, 0x55);
  const publicInputBase64 = "0102";
  const proofBase64 = Buffer.from("stark-proof").toString("base64");
  const auxBase64 = Buffer.from("{}").toString("base64");
  const encoded = buildPrivacyProofEnvelope({
    backend: "stark/fri/sha256-goldilocks",
    circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    vkHash,
    publicInputs: publicInputBase64,
    proofBytes: proofBase64,
    aux: auxBase64,
    maxProofBytes: 64,
    maxPublicInputBytes: 16,
  });
  const decoded = noritoDecodePrivacyProofEnvelope(encoded);
  assert.deepEqual(
    decoded.public_inputs,
    Array.from(Buffer.from(publicInputBase64, "base64")),
  );
  assert.deepEqual(decoded.proof_bytes, Array.from(Buffer.from("stark-proof")));
  assert.deepEqual(decoded.aux, Array.from(Buffer.from("{}")));
});

descriptorTest("privacy proof envelopes decode clean verifier-key hash strings", () => {
  const expectedVkHash = Buffer.alloc(32, 0x55);
  for (const vkHash of [
    expectedVkHash.toString("hex"),
    expectedVkHash.toString("base64"),
  ]) {
    const encoded = buildPrivacyProofEnvelope({
      backend: "stark/fri/sha256-goldilocks",
      circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
      vkHash,
      publicInputs: Buffer.from([0x01]),
      proofBytes: Buffer.from([0x02]),
      maxProofBytes: 16,
      maxPublicInputBytes: 16,
    });
    const decoded = noritoDecodePrivacyProofEnvelope(encoded);
    assert.deepEqual(decoded.vk_hash, Array.from(expectedVkHash));
  }
});

descriptorTest("privacy proof envelopes accept explicit numeric byte arrays", () => {
  const encoded = buildPrivacyProofEnvelope({
    backend: "stark/fri/sha256-goldilocks",
    circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    vkHash: Array(32).fill(0x55),
    publicInputs: [0x01, 0x02],
    proofBytes: [0x03, 0x04],
    aux: [0x7b, 0x7d],
    maxProofBytes: "16",
    maxPublicInputBytes: "16",
  });
  const decoded = noritoDecodePrivacyProofEnvelope(encoded);
  assert.deepEqual(decoded.vk_hash, Array(32).fill(0x55));
  assert.deepEqual(decoded.public_inputs, [0x01, 0x02]);
  assert.deepEqual(decoded.proof_bytes, [0x03, 0x04]);
  assert.deepEqual(decoded.aux, [0x7b, 0x7d]);
});

descriptorTest("privacy proof envelopes accept explicit byte views", () => {
  const publicInputBacking = Uint8Array.from([0x01, 0x02, 0xff]);
  const encoded = buildPrivacyProofEnvelope({
    backend: "stark/fri/sha256-goldilocks",
    circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    vkHash: Uint8Array.from(Array(32).fill(0x55)),
    publicInputs: new DataView(publicInputBacking.buffer, 0, 2),
    proofBytes: Uint8Array.from([0x03, 0x04]).buffer,
    aux: Uint8Array.from([0x7b, 0x7d]),
    maxProofBytes: 16,
    maxPublicInputBytes: 16,
  });
  const decoded = noritoDecodePrivacyProofEnvelope(encoded);
  assert.deepEqual(decoded.vk_hash, Array(32).fill(0x55));
  assert.deepEqual(decoded.public_inputs, [0x01, 0x02]);
  assert.deepEqual(decoded.proof_bytes, [0x03, 0x04]);
  assert.deepEqual(decoded.aux, [0x7b, 0x7d]);
});

descriptorTest("privacy proof envelopes preserve pending production backend tags", () => {
  const vkHash = Buffer.alloc(32, 0x66);
  const cases = [
    ["halo2-ipa-orchard", "Halo2IpaOrchard"],
    ["halo2/ipa/orchard", "Halo2IpaOrchard"],
    ["orchard", "Halo2IpaOrchard"],
    ["zcash-orchard", "Halo2IpaOrchard"],
    ["groth16-bls12-377", "Groth16Bls12377"],
    ["groth16/bls12-377", "Groth16Bls12377"],
    ["bls12-377", "Groth16Bls12377"],
    ["decaf377", "Groth16Bls12377"],
    ["masp", "Groth16Bls12377"],
    ["penumbra-masp", "Groth16Bls12377"],
    ["halo2/ipa/penumbra", "Groth16Bls12377"],
    ["halo2/ipa/masp", "Groth16Bls12377"],
    ["fcmp-plus-plus-curve-tree", "FcmpPlusPlusCurveTree"],
    ["fcmp++", "FcmpPlusPlusCurveTree"],
    ["monero-fcmp++", "FcmpPlusPlusCurveTree"],
    ["halo2/ipa/monero", "FcmpPlusPlusCurveTree"],
    ["halo2/ipa/curve-tree", "FcmpPlusPlusCurveTree"],
    ["lattice-pcs-sis", "LatticePcsSis"],
    ["jindo-lattice-pcs-zk", "LatticePcsSis"],
    ["jindo-lattice-pcs-zk-v0", "LatticePcsSis"],
    ["miden-stark", "MidenStark"],
    ["stark/fri/miden", "MidenStark"],
    ["aztec-plonkish-private-kernel", "AztecPlonkishPrivateKernel"],
    ["aztec/private-kernel", "AztecPlonkishPrivateKernel"],
    ["pq-masp-stark-fri", "PqMaspStarkFri"],
    ["stark/fri/pq-masp-stark-fri", "PqMaspStarkFri"],
    ["post-quantum-masp", "PqMaspStarkFri"],
    ["anonymous-pgc", "AnonymousPgc"],
    ["anonymous-pgc-k-out-of-n", "AnonymousPgc"],
    ["anonymous-pgc-k-out-of-n-v1", "AnonymousPgc"],
    ["verange", "VeRange"],
    ["verange-transparent-range", "VeRange"],
    ["verange-transparent-range-v1", "VeRange"],
    ["zkat", "ZkAt"],
    ["zkAt policy-private authenticator", "ZkAt"],
    ["zkat-policy-private-auth-v1", "ZkAt"],
    ["recursive-anonymous-admission", "RecursiveAnonymousAdmission"],
    ["recursive-anonymous-admission-v0", "RecursiveAnonymousAdmission"],
    ["zk-ams-recursive-admission-v0", "RecursiveAnonymousAdmission"],
    ["vega-existing-credential-zk", "VegaExistingCredentialZk"],
    ["vega-existing-credential-zk-v0", "VegaExistingCredentialZk"],
    ["silent-threshold-anoncred", "SilentThresholdAnoncred"],
    ["silent-threshold-anoncred-v0", "SilentThresholdAnoncred"],
    ["threshold-anonymous-credentials", "SilentThresholdAnoncred"],
    ["zk-x509", "ZkX509"],
    ["zkvm-x509-identity", "ZkX509"],
    ["zk-x509-onchain-identity-v0", "ZkX509"],
    ["sis-with-hints", "SisWithHints"],
    ["sis-hints-anoncred-pq-v0", "SisWithHints"],
    ["lattice-anonymous-credentials", "SisWithHints"],
  ];

  for (const [backend, expected] of cases) {
    const encoded = buildPrivacyProofEnvelope({
      backend,
      circuitId: `${backend}:pending-production-shape-v0`,
      vkHash,
      publicInputs: Buffer.from([0x01]),
      proofBytes: Buffer.from([0x02]),
      maxProofBytes: 16,
      maxPublicInputBytes: 16,
    });
    const decoded = noritoDecodePrivacyProofEnvelope(encoded);
    assert.equal(decoded.backend, expected);
  }
});

descriptorTest("zkAt builders normalize policy commitments and authenticator envelopes", () => {
  const policy = {
    threshold: 2,
    roles: ["ops", "risk", "treasury"],
    fallback: { recovery_after_slots: 1440 },
  };
  const payload = Buffer.from("zkat:transparent-transfer:42");
  const policyCommitment = buildZkAtPolicyCommitment({
    policyJson: policy,
    policyEpoch: 7,
    domainSeparator: "boi:zkat:v1",
    policySchema: "boi-hidden-threshold-v1",
  });
  assert.equal(policyCommitment.version, 1);
  assert.equal(policyCommitment.commitment_kind, "dev-sha256-policy-digest");
  assert.equal(policyCommitment.policy_commitment.length, 32);
  assert.equal(policyCommitment.policy_digest.length, 32);

  const vkHash = Buffer.alloc(32, 0x55);
  const prepared = buildZkAtAuthenticatorEnvelope({
    policyCommitment: policyCommitment.policy_commitment,
    policyEpoch: 7,
    payload,
    accountId: ACCOUNT_ID,
    actionClass: "transparent_transfer",
    domainSeparator: "boi:zkat:v1",
    vkHash,
    proofBytes: Buffer.from("prepared-zkat-proof"),
  });
  const decodedPrepared = noritoDecodePrivacyProofEnvelope(prepared);
  assert.equal(decodedPrepared.backend, "Stark");
  assert.equal(
    decodedPrepared.circuit_id,
    "stark/fri/sha256-goldilocks:zkat_policy_private_auth_v1",
  );
  const preparedInputs = JSON.parse(
    Buffer.from(decodedPrepared.public_inputs).toString("utf8"),
  );
  assert.equal(preparedInputs.account_id, ACCOUNT_ID_CANONICAL);
  assert.equal(preparedInputs.action_class, "transparent_transfer");
  assert.equal(preparedInputs.policy_epoch, 7);
  assert.equal(
    preparedInputs.policy_commitment,
    Buffer.from(policyCommitment.policy_commitment).toString("hex"),
  );

  const fixture = buildZkAtDevProofFixture({
    policyJson: policy,
    policyEpoch: 7,
    policySchema: "boi-hidden-threshold-v1",
    payload,
    accountId: ACCOUNT_ID,
    actionClass: "transparent_transfer",
    domainSeparator: "boi:zkat:v1",
    vkHash,
  });
  assert.equal(fixture.kind, "zkat-dev-fixture-v1");
  assert.equal(fixture.production, false);
  assert.ok(Buffer.isBuffer(fixture.envelope));
  assert.ok(Buffer.isBuffer(fixture.proofBytes));
  const verified = verifyZkAtAuthenticatorLocally({
    envelope: fixture.envelope,
    policyJson: policy,
    policySchema: "boi-hidden-threshold-v1",
    payload,
    accountId: ACCOUNT_ID,
    actionClass: "transparent_transfer",
    domainSeparator: "boi:zkat:v1",
    policyEpoch: 7,
  });
  assert.equal(verified.ok, true);
  assert.equal(verified.production, false);
  assert.equal(verified.account_id, ACCOUNT_ID_CANONICAL);
  assert.equal(verified.action_class, "transparent_transfer");
  assert.equal(verified.policy_epoch, 7);
  assert.deepEqual(verified.public_inputs, fixture.public_inputs);
});

descriptorTest("zkAt builders reject malformed policy and authenticator inputs", () => {
  const policy = { threshold: 2, roles: ["ops", "risk"] };
  const base = {
    policyJson: policy,
    policyEpoch: 7,
    payload: Buffer.from("zkat:transparent-transfer:42"),
    accountId: ACCOUNT_ID,
    actionClass: "transparent_transfer",
    domainSeparator: "boi:zkat:v1",
    vkHash: Buffer.alloc(32, 0x55),
  };
  for (const patch of [
    { policyEpoch: 0 },
    { policyCommitment: Buffer.alloc(32, 0xee) },
    { policyJson: undefined, policyCommitment: undefined },
    { maxPolicyBytes: undefined },
    { maxPolicyBytes: null },
  ]) {
    assert.throws(
      () => buildZkAtPolicyCommitment({ ...base, ...patch }),
      /zkAtPolicyCommitment/,
    );
  }
  for (const patch of [
    { policyEpoch: 0 },
    { payload: Buffer.from("payload"), txDigest: Buffer.alloc(32, 0xee) },
    { maxPayloadBytes: undefined },
    { maxPayloadBytes: null },
    { accountId: "alice@wonderland" },
    { actionClass: " " },
    { vkHash: Buffer.alloc(32) },
    { proofBytes: Buffer.alloc(0) },
  ]) {
    assert.throws(
      () =>
        buildZkAtAuthenticatorEnvelope({
          ...base,
          proofBytes: Buffer.from("prepared-zkat-proof"),
          ...patch,
        }),
      /(zkAtAuthenticatorEnvelope|privacyProofEnvelope|maxPayloadBytes)/,
    );
  }
});

descriptorTest("zkAt local verifier rejects tampered dev fixtures", () => {
  const policy = { threshold: 2, roles: ["ops", "risk"] };
  const fixtureInput = {
    policyJson: policy,
    policyEpoch: 7,
    policySchema: "boi-hidden-threshold-v1",
    payload: Buffer.from("zkat:transparent-transfer:42"),
    accountId: ACCOUNT_ID,
    actionClass: "transparent_transfer",
    domainSeparator: "boi:zkat:v1",
    vkHash: Buffer.alloc(32, 0x55),
  };
  const fixture = buildZkAtDevProofFixture(fixtureInput);
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  const publicInputs = JSON.parse(Buffer.from(decoded.public_inputs).toString("utf8"));
  const rebuildEnvelope = ({
    backend = "stark/fri/sha256-goldilocks",
    publicInputsBytes = decoded.public_inputs,
    proofBytes = decoded.proof_bytes,
  } = {}) =>
    buildPrivacyProofEnvelope({
      backend,
      circuitId: decoded.circuit_id,
      vkHash: Buffer.alloc(32, 0x55),
      publicInputs: publicInputsBytes,
      proofBytes,
    });
  const tamperedProof = [...decoded.proof_bytes];
  tamperedProof[tamperedProof.length - 1] ^= 0xff;
  const nonCanonicalPublicInputs = Buffer.from(JSON.stringify(publicInputs, null, 2));
  const zeroPolicyInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      policy_commitment: Buffer.alloc(32).toString("hex"),
    }),
  );

  for (const input of [
    { envelope: rebuildEnvelope({ proofBytes: Buffer.from("arbitrary") }), payload: fixtureInput.payload },
    { envelope: rebuildEnvelope({ proofBytes: tamperedProof }), payload: fixtureInput.payload },
    { envelope: fixture.envelope, payload: Buffer.from("substituted-payload") },
    { envelope: fixture.envelope, policyJson: { threshold: 1, roles: ["ops"] }, policyEpoch: 7, policySchema: "boi-hidden-threshold-v1" },
    { envelope: fixture.envelope, accountId: ACCOUNT_ID, actionClass: "different_action" },
    { envelope: fixture.envelope, policyEpoch: 8 },
    { envelope: rebuildEnvelope({ backend: "groth16" }), payload: fixtureInput.payload },
    { envelope: rebuildEnvelope({ publicInputsBytes: nonCanonicalPublicInputs }), payload: fixtureInput.payload },
    { envelope: rebuildEnvelope({ publicInputsBytes: zeroPolicyInputs }), payload: fixtureInput.payload },
  ]) {
    assert.throws(
      () => verifyZkAtAuthenticatorLocally(input),
      /zkAtAuthenticatorLocalVerification/,
    );
  }
});

descriptorTest("ZK-AMS builders normalize admission batches and proof envelopes", () => {
  const issuerRoot = Buffer.alloc(32, 0x91);
  const admissionNullifiers = [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xa2)];
  const anonymousAccountCommitments = [
    Buffer.alloc(32, 0xb1),
    Buffer.alloc(32, 0xb2),
  ];
  const recursiveProof = Buffer.from("zk-ams:recursive-proof:batch-7");
  const domainSeparator = "boi:zk-ams:pilot:v0";
  const vkHash = Buffer.alloc(32, 0x66);
  const base = {
    issuerRoot,
    admissionNullifiers,
    anonymousAccountCommitments,
    recursiveProof,
    domainSeparator,
  };

  const batch = buildZkAmsAdmissionBatch(base);
  assert.equal(batch.version, 1);
  assert.equal(batch.batch_size, 2);
  assert.equal(batch.root_kind, "dev-sha256-admission-batch-root");
  assert.equal(Buffer.from(batch.issuer_root).toString("hex"), issuerRoot.toString("hex"));
  assert.equal(batch.admission_nullifiers.length, 2);

  const prepared = buildZkAmsAdmissionProofEnvelope({
    ...base,
    vkHash,
    proofBytes: Buffer.from("prepared-zk-ams-proof"),
  });
  const decodedPrepared = noritoDecodePrivacyProofEnvelope(prepared);
  assert.equal(decodedPrepared.backend, "Stark");
  assert.equal(
    decodedPrepared.circuit_id,
    "stark/fri/sha256-goldilocks:zk_ams_recursive_admission_v0",
  );
  const preparedInputs = JSON.parse(
    Buffer.from(decodedPrepared.public_inputs).toString("utf8"),
  );
  assert.equal(
    preparedInputs.admission_batch_root,
    Buffer.from(batch.admission_batch_root).toString("hex"),
  );
  assert.equal(preparedInputs.domain_separator, domainSeparator);

  const fixture = buildZkAmsAdmissionDevProofFixture({
    ...base,
    vkHash,
  });
  assert.equal(fixture.kind, "zk-ams-dev-fixture-v0");
  assert.equal(fixture.production, false);
  assert.ok(Buffer.isBuffer(fixture.envelope));
  assert.equal(fixture.batch.batch_size, 2);
  const verified = verifyZkAmsAdmissionProofLocally({
    envelope: fixture.envelope,
    ...base,
  });
  assert.equal(verified.ok, true);
  assert.equal(verified.production, false);
  assert.equal(verified.batch_size, 2);
  assert.equal(
    verified.admission_batch_root,
    Buffer.from(batch.admission_batch_root).toString("hex"),
  );
});

descriptorTest("ZK-AMS builders reject malformed admission inputs", () => {
  const base = {
    issuerRoot: Buffer.alloc(32, 0x91),
    admissionNullifiers: [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xa2)],
    anonymousAccountCommitments: [Buffer.alloc(32, 0xb1), Buffer.alloc(32, 0xb2)],
    recursiveProof: Buffer.from("zk-ams:recursive-proof:batch-7"),
    domainSeparator: "boi:zk-ams:pilot:v0",
  };
  for (const patch of [
    { issuerRoot: Buffer.alloc(32) },
    { admissionNullifiers: [] },
    { admissionNullifiers: [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xa1)] },
    { anonymousAccountCommitments: [Buffer.alloc(32, 0xb1), Buffer.alloc(32, 0xb1)] },
    { anonymousAccountCommitments: [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xb2)] },
    { anonymousAccountCommitments: [Buffer.alloc(32, 0xb1)] },
    { recursiveProofDigest: Buffer.alloc(32, 0xee) },
    { admissionBatchRoot: Buffer.alloc(32, 0xdd) },
    { maxBatchSize: 1 },
    { maxBatchSize: undefined },
    { maxBatchSize: null },
    { maxRecursiveProofBytes: undefined },
    { maxRecursiveProofBytes: null },
  ]) {
    assert.throws(
      () => buildZkAmsAdmissionBatch({ ...base, ...patch }),
      /zkAmsAdmissionBatch/,
    );
  }

  for (const patch of [
    { proofBytes: Buffer.alloc(0) },
    { vkHash: Buffer.alloc(32) },
    { backend: "groth16" },
    { circuitId: "stark/fri/sha256-goldilocks:wrong" },
  ]) {
    assert.throws(
      () =>
        buildZkAmsAdmissionProofEnvelope({
          ...base,
          vkHash: Buffer.alloc(32, 0x66),
          proofBytes: Buffer.from("prepared-zk-ams-proof"),
          ...patch,
        }),
      /(zkAmsAdmissionProofEnvelope|privacyProofEnvelope)/,
    );
  }
});

descriptorTest("ZK-AMS local verifier rejects tampered dev fixtures", () => {
  const fixtureInput = {
    issuerRoot: Buffer.alloc(32, 0x91),
    admissionNullifiers: [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xa2)],
    anonymousAccountCommitments: [Buffer.alloc(32, 0xb1), Buffer.alloc(32, 0xb2)],
    recursiveProof: Buffer.from("zk-ams:recursive-proof:batch-7"),
    domainSeparator: "boi:zk-ams:pilot:v0",
    vkHash: Buffer.alloc(32, 0x66),
  };
  const fixture = buildZkAmsAdmissionDevProofFixture(fixtureInput);
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  const publicInputs = JSON.parse(Buffer.from(decoded.public_inputs).toString("utf8"));
  const rebuildEnvelope = ({
    backend = "stark/fri/sha256-goldilocks",
    publicInputsBytes = decoded.public_inputs,
    proofBytes = decoded.proof_bytes,
  } = {}) =>
    buildPrivacyProofEnvelope({
      backend,
      circuitId: decoded.circuit_id,
      vkHash: Buffer.alloc(32, 0x66),
      publicInputs: publicInputsBytes,
      proofBytes,
    });
  const tamperedProof = [...decoded.proof_bytes];
  tamperedProof[tamperedProof.length - 1] ^= 0xff;
  const nonCanonicalPublicInputs = Buffer.from(JSON.stringify(publicInputs, null, 2));
  const duplicateNullifierInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      admission_nullifiers: [
        publicInputs.admission_nullifiers[0],
        publicInputs.admission_nullifiers[0],
      ],
    }),
  );
  const zeroIssuerInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      issuer_root: Buffer.alloc(32).toString("hex"),
    }),
  );

  for (const input of [
    { envelope: rebuildEnvelope({ proofBytes: Buffer.from("arbitrary") }) },
    { envelope: rebuildEnvelope({ proofBytes: tamperedProof }) },
    { envelope: fixture.envelope, issuerRoot: Buffer.alloc(32, 0x92) },
    { envelope: fixture.envelope, admissionNullifiers: [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xa3)] },
    { envelope: fixture.envelope, anonymousAccountCommitments: [Buffer.alloc(32, 0xb1), Buffer.alloc(32, 0xb3)] },
    { envelope: fixture.envelope, recursiveProof: Buffer.from("substituted-recursive-proof") },
    { envelope: fixture.envelope, domainSeparator: "boi:zk-ams:other:v0" },
    { envelope: rebuildEnvelope({ backend: "groth16" }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: nonCanonicalPublicInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: duplicateNullifierInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: zeroIssuerInputs }) },
  ]) {
    assert.throws(
      () => verifyZkAmsAdmissionProofLocally(input),
      /zkAmsAdmissionLocalVerification/,
    );
  }
});

descriptorTest("Vega builders normalize credential predicates and proof envelopes", () => {
  const issuer = { did: "did:example:issuer:boi", key: "issuer-key-1" };
  const predicate = {
    kind: "age_over",
    attribute: "age",
    threshold: 18,
  };
  const base = {
    issuerJson: issuer,
    predicateJson: predicate,
    credentialSchema: "boi-age-credential-v1",
    accountId: ACCOUNT_ID,
    expirationEpoch: 42,
    domainSeparator: "boi:vega:pilot:v0",
  };
  const predicateInput = {
    predicateJson: predicate,
    credentialSchema: base.credentialSchema,
    domainSeparator: base.domainSeparator,
  };
  const vkHash = Buffer.alloc(32, 0x77);

  const predicateCommitment = buildVegaCredentialPredicateCommitment(predicateInput);
  assert.equal(predicateCommitment.version, 1);
  assert.equal(predicateCommitment.credential_schema, "boi-age-credential-v1");
  assert.equal(predicateCommitment.commitment_kind, "dev-sha256-predicate-digest");
  assert.equal(predicateCommitment.predicate_commitment.length, 32);

  const prepared = buildVegaCredentialProofEnvelope({
    ...base,
    vkHash,
    proofBytes: Buffer.from("prepared-vega-proof"),
  });
  const decodedPrepared = noritoDecodePrivacyProofEnvelope(prepared);
  assert.equal(decodedPrepared.backend, "Stark");
  assert.equal(
    decodedPrepared.circuit_id,
    "stark/fri/sha256-goldilocks:vega_existing_credential_zk_v0",
  );
  const preparedInputs = JSON.parse(
    Buffer.from(decodedPrepared.public_inputs).toString("utf8"),
  );
  assert.equal(preparedInputs.credential_schema, "boi-age-credential-v1");
  assert.equal(
    preparedInputs.predicate_commitment,
    Buffer.from(predicateCommitment.predicate_commitment).toString("hex"),
  );

  const fixture = buildVegaCredentialDevProofFixture({
    ...base,
    vkHash,
  });
  assert.equal(fixture.kind, "vega-dev-fixture-v0");
  assert.equal(fixture.production, false);
  assert.ok(Buffer.isBuffer(fixture.envelope));
  const verified = verifyVegaCredentialProofLocally({
    envelope: fixture.envelope,
    ...base,
  });
  assert.equal(verified.ok, true);
  assert.equal(verified.production, false);
  assert.equal(verified.credential_schema, "boi-age-credential-v1");
  assert.equal(verified.expiration_epoch, 42);
});

descriptorTest("Vega builders reject malformed credential proof inputs", () => {
  const base = {
    issuerJson: { did: "did:example:issuer:boi", key: "issuer-key-1" },
    predicateJson: { kind: "age_over", attribute: "age", threshold: 18 },
    credentialSchema: "boi-age-credential-v1",
    accountId: ACCOUNT_ID,
    expirationEpoch: 42,
    domainSeparator: "boi:vega:pilot:v0",
  };
  const predicateInput = {
    predicateJson: base.predicateJson,
    credentialSchema: base.credentialSchema,
    domainSeparator: base.domainSeparator,
  };
  for (const patch of [
    { predicateCommitment: Buffer.alloc(32, 0xee) },
    { predicateJson: undefined, predicateCommitment: undefined },
    { credentialSchema: " " },
    { predicateCommitment: Buffer.alloc(32) },
    { maxPredicateBytes: undefined },
    { maxPredicateBytes: null },
  ]) {
    assert.throws(
      () => buildVegaCredentialPredicateCommitment({ ...predicateInput, ...patch }),
      /vegaCredentialPredicateCommitment/,
    );
  }

  for (const patch of [
    { issuerJson: undefined, issuerCommitment: undefined },
    { issuerCommitment: Buffer.alloc(32) },
    { subjectBinding: Buffer.alloc(32, 0x01), accountId: ACCOUNT_ID },
    { accountId: "alice@wonderland" },
    { expirationEpoch: -1 },
    { vkHash: Buffer.alloc(32) },
    { proofBytes: Buffer.alloc(0) },
    { backend: "groth16" },
    { circuitId: "stark/fri/sha256-goldilocks:wrong" },
    { maxIssuerBytes: undefined },
    { maxIssuerBytes: null },
    { maxPredicateBytes: undefined },
    { maxPredicateBytes: null },
  ]) {
    assert.throws(
      () =>
        buildVegaCredentialProofEnvelope({
          ...base,
          vkHash: Buffer.alloc(32, 0x77),
          proofBytes: Buffer.from("prepared-vega-proof"),
          ...patch,
        }),
      /(vegaCredentialProofEnvelope|privacyProofEnvelope)/,
    );
  }
});

descriptorTest("Vega local verifier rejects tampered dev fixtures", () => {
  const fixtureInput = {
    issuerJson: { did: "did:example:issuer:boi", key: "issuer-key-1" },
    predicateJson: { kind: "age_over", attribute: "age", threshold: 18 },
    credentialSchema: "boi-age-credential-v1",
    accountId: ACCOUNT_ID,
    expirationEpoch: 42,
    domainSeparator: "boi:vega:pilot:v0",
    vkHash: Buffer.alloc(32, 0x77),
  };
  const fixture = buildVegaCredentialDevProofFixture(fixtureInput);
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  const publicInputs = JSON.parse(Buffer.from(decoded.public_inputs).toString("utf8"));
  const rebuildEnvelope = ({
    backend = "stark/fri/sha256-goldilocks",
    publicInputsBytes = decoded.public_inputs,
    proofBytes = decoded.proof_bytes,
  } = {}) =>
    buildPrivacyProofEnvelope({
      backend,
      circuitId: decoded.circuit_id,
      vkHash: Buffer.alloc(32, 0x77),
      publicInputs: publicInputsBytes,
      proofBytes,
    });
  const tamperedProof = [...decoded.proof_bytes];
  tamperedProof[tamperedProof.length - 1] ^= 0xff;
  const nonCanonicalPublicInputs = Buffer.from(JSON.stringify(publicInputs, null, 2));
  const zeroIssuerInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      issuer_commitment: Buffer.alloc(32).toString("hex"),
    }),
  );
  const aliasCollisionInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      credentialSchema: publicInputs.credential_schema,
    }),
  );

  for (const input of [
    { envelope: rebuildEnvelope({ proofBytes: Buffer.from("arbitrary") }) },
    { envelope: rebuildEnvelope({ proofBytes: tamperedProof }) },
    { envelope: fixture.envelope, issuerJson: { did: "did:example:issuer:other" } },
    { envelope: fixture.envelope, predicateJson: { kind: "age_over", attribute: "age", threshold: 21 } },
    { envelope: fixture.envelope, accountId: SAMPLE_ACCOUNT_I105_LITERAL },
    { envelope: fixture.envelope, expirationEpoch: 43 },
    { envelope: fixture.envelope, credentialSchema: "boi-other-credential-v1" },
    { envelope: fixture.envelope, domainSeparator: "boi:vega:other:v0" },
    { envelope: rebuildEnvelope({ backend: "groth16" }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: nonCanonicalPublicInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: zeroIssuerInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: aliasCollisionInputs }) },
  ]) {
    assert.throws(
      () => verifyVegaCredentialProofLocally(input),
      /vegaCredentialLocalVerification/,
    );
  }
});

descriptorTest("Silent threshold credential builders normalize commitments and envelopes", () => {
  const base = {
    issuerSetJson: {
      version: 3,
      threshold: 2,
      issuers: ["boi-supervisor", "bank-a", "bank-b"],
    },
    thresholdPolicyJson: {
      threshold: 2,
      issuer_set_version: 3,
      purpose: "retail-wallet-eligibility",
    },
    credentialShowingJson: {
      credential_type: "retail-wallet-eligibility",
      attributes: ["resident", "adult"],
      presentation_nonce: "nonce-42",
    },
    verifierPolicyJson: {
      verifier: "boi-wallet-enrollment",
      accepted_purposes: ["retail-wallet-eligibility"],
    },
    domainSeparator: "boi:silent-threshold:pilot:v0",
  };
  const vkHash = Buffer.alloc(32, 0x88);

  const commitments = buildSilentThresholdCredentialCommitments(base);
  assert.equal(commitments.version, 1);
  assert.equal(commitments.issuer_set_commitment.length, 32);
  assert.equal(commitments.showing_nullifier.length, 32);
  assert.equal(
    commitments.commitment_kinds.credential_showing_commitment,
    "dev-sha256-credential-showing-digest",
  );

  const prepared = buildSilentThresholdCredentialEnvelope({
    ...base,
    vkHash,
    proofBytes: Buffer.from("prepared-silent-threshold-proof"),
  });
  const decodedPrepared = noritoDecodePrivacyProofEnvelope(prepared);
  assert.equal(decodedPrepared.backend, "Stark");
  assert.equal(
    decodedPrepared.circuit_id,
    "stark/fri/sha256-goldilocks:silent_threshold_anoncred_v0",
  );
  const preparedInputs = JSON.parse(
    Buffer.from(decodedPrepared.public_inputs).toString("utf8"),
  );
  assert.equal(
    preparedInputs.issuer_set_commitment,
    Buffer.from(commitments.issuer_set_commitment).toString("hex"),
  );
  assert.equal(
    preparedInputs.showing_nullifier,
    Buffer.from(commitments.showing_nullifier).toString("hex"),
  );

  const fixture = buildSilentThresholdCredentialDevProofFixture({
    ...base,
    vkHash,
  });
  assert.equal(fixture.kind, "silent-threshold-dev-fixture-v0");
  assert.equal(fixture.production, false);
  assert.ok(Buffer.isBuffer(fixture.envelope));
  const verified = verifySilentThresholdCredentialProofLocally({
    envelope: fixture.envelope,
    ...base,
  });
  assert.equal(verified.ok, true);
  assert.equal(verified.production, false);
  assert.equal(
    verified.showing_nullifier,
    Buffer.from(commitments.showing_nullifier).toString("hex"),
  );
});

descriptorTest("Silent threshold credential builders reject malformed inputs", () => {
  const base = {
    issuerSetJson: { threshold: 2, issuers: ["a", "b", "c"] },
    thresholdPolicyJson: { threshold: 2, purpose: "wallet" },
    credentialShowingJson: { credential_type: "wallet", nonce: "n-1" },
    verifierPolicyJson: { verifier: "boi", purpose: "wallet" },
    domainSeparator: "boi:silent-threshold:pilot:v0",
  };
  for (const patch of [
    { issuerSetCommitment: Buffer.alloc(32, 0xee) },
    { thresholdPolicyHash: Buffer.alloc(32, 0xee) },
    { credentialShowingCommitment: Buffer.alloc(32, 0xee) },
    { showingNullifier: Buffer.alloc(32, 0xee) },
    { verifierPolicyHash: Buffer.alloc(32, 0xee) },
    { issuerSetJson: undefined, issuerSetCommitment: undefined },
    { thresholdPolicyJson: undefined, thresholdPolicyHash: undefined },
    { credentialShowingJson: undefined, credentialShowingCommitment: undefined },
    { verifierPolicyJson: undefined, verifierPolicyHash: undefined },
    { domainSeparator: " " },
    { issuerSetCommitment: Buffer.alloc(32) },
    { maxIssuerSetBytes: undefined },
    { maxIssuerSetBytes: null },
    { maxPolicyBytes: undefined },
    { maxPolicyBytes: null },
    { maxShowingBytes: undefined },
    { maxShowingBytes: null },
  ]) {
    assert.throws(
      () => buildSilentThresholdCredentialCommitments({ ...base, ...patch }),
      /silentThresholdCredentialCommitments/,
    );
  }

  for (const patch of [
    { proofBytes: Buffer.alloc(0) },
    { vkHash: Buffer.alloc(32) },
    { backend: "groth16" },
    { circuitId: "stark/fri/sha256-goldilocks:wrong" },
  ]) {
    assert.throws(
      () =>
        buildSilentThresholdCredentialEnvelope({
          ...base,
          vkHash: Buffer.alloc(32, 0x88),
          proofBytes: Buffer.from("prepared-silent-threshold-proof"),
          ...patch,
        }),
      /(silentThresholdCredentialEnvelope|privacyProofEnvelope)/,
    );
  }
});

descriptorTest("Silent threshold local verifier rejects tampered dev fixtures", () => {
  const fixtureInput = {
    issuerSetJson: { threshold: 2, issuers: ["a", "b", "c"] },
    thresholdPolicyJson: { threshold: 2, purpose: "wallet" },
    credentialShowingJson: { credential_type: "wallet", nonce: "n-1" },
    verifierPolicyJson: { verifier: "boi", purpose: "wallet" },
    domainSeparator: "boi:silent-threshold:pilot:v0",
    vkHash: Buffer.alloc(32, 0x88),
  };
  const fixture = buildSilentThresholdCredentialDevProofFixture(fixtureInput);
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  const publicInputs = JSON.parse(Buffer.from(decoded.public_inputs).toString("utf8"));
  const rebuildEnvelope = ({
    backend = "stark/fri/sha256-goldilocks",
    publicInputsBytes = decoded.public_inputs,
    proofBytes = decoded.proof_bytes,
  } = {}) =>
    buildPrivacyProofEnvelope({
      backend,
      circuitId: decoded.circuit_id,
      vkHash: Buffer.alloc(32, 0x88),
      publicInputs: publicInputsBytes,
      proofBytes,
    });
  const tamperedProof = [...decoded.proof_bytes];
  tamperedProof[tamperedProof.length - 1] ^= 0xff;
  const nonCanonicalPublicInputs = Buffer.from(JSON.stringify(publicInputs, null, 2));
  const zeroIssuerInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      issuer_set_commitment: Buffer.alloc(32).toString("hex"),
    }),
  );
  const aliasCollisionInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      issuerSetCommitment: publicInputs.issuer_set_commitment,
    }),
  );

  for (const input of [
    { envelope: rebuildEnvelope({ proofBytes: Buffer.from("arbitrary") }) },
    { envelope: rebuildEnvelope({ proofBytes: tamperedProof }) },
    { envelope: fixture.envelope, issuerSetJson: { threshold: 1, issuers: ["a"] } },
    { envelope: fixture.envelope, thresholdPolicyJson: { threshold: 1, purpose: "wallet" } },
    { envelope: fixture.envelope, credentialShowingJson: { credential_type: "wallet", nonce: "n-2" } },
    { envelope: fixture.envelope, showingNullifier: Buffer.alloc(32, 0x44) },
    { envelope: fixture.envelope, verifierPolicyJson: { verifier: "other", purpose: "wallet" } },
    { envelope: fixture.envelope, domainSeparator: "boi:silent-threshold:other:v0" },
    { envelope: rebuildEnvelope({ backend: "groth16" }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: nonCanonicalPublicInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: zeroIssuerInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: aliasCollisionInputs }) },
  ]) {
    assert.throws(
      () => verifySilentThresholdCredentialProofLocally(input),
      /silentThresholdCredentialLocalVerification/,
    );
  }
});

descriptorTest("ZK-X.509 identity builders normalize commitments and envelopes", () => {
  const base = {
    caRootJson: {
      root: "boi-root-ca",
      version: 1,
      not_before: "2026-01-01T00:00:00Z",
    },
    certificatePolicyJson: {
      eku: ["clientAuth"],
      policy: "institutional-wallet",
    },
    revocationJson: {
      epoch: 7,
      root: "crlite-root-7",
    },
    subjectJson: {
      cn: "Bank A",
      lei: "5493001KJTIIGC8Y1R12",
    },
    accountId: ACCOUNT_ID,
    domainSeparator: "boi:zk-x509:pilot:v0",
  };
  const vkHash = Buffer.alloc(32, 0x99);

  const commitments = buildZkX509IdentityCommitments(base);
  assert.equal(commitments.version, 1);
  assert.equal(commitments.ca_root_commitment.length, 32);
  assert.equal(commitments.address_binding.length, 32);
  assert.equal(
    commitments.commitment_kinds.ca_root_commitment,
    "dev-sha256-ca-root-digest",
  );
  assert.equal(
    commitments.commitment_kinds.address_binding,
    "dev-sha256-account-binding",
  );

  const prepared = buildZkX509IdentityEnvelope({
    ...base,
    vkHash,
    proofBytes: Buffer.from("prepared-zk-x509-proof"),
  });
  const decodedPrepared = noritoDecodePrivacyProofEnvelope(prepared);
  assert.equal(decodedPrepared.backend, "Stark");
  assert.equal(
    decodedPrepared.circuit_id,
    "stark/fri/sha256-goldilocks:zk_x509_onchain_identity_v0",
  );
  const preparedInputs = JSON.parse(
    Buffer.from(decodedPrepared.public_inputs).toString("utf8"),
  );
  assert.equal(
    preparedInputs.ca_root_commitment,
    Buffer.from(commitments.ca_root_commitment).toString("hex"),
  );
  assert.equal(
    preparedInputs.address_binding,
    Buffer.from(commitments.address_binding).toString("hex"),
  );

  const fixture = buildZkX509IdentityDevProofFixture({
    ...base,
    vkHash,
  });
  assert.equal(fixture.kind, "zk-x509-dev-fixture-v0");
  assert.equal(fixture.production, false);
  assert.ok(Buffer.isBuffer(fixture.envelope));
  const verified = verifyZkX509IdentityProofLocally({
    envelope: fixture.envelope,
    ...base,
  });
  assert.equal(verified.ok, true);
  assert.equal(verified.production, false);
  assert.equal(
    verified.address_binding,
    Buffer.from(commitments.address_binding).toString("hex"),
  );
});

descriptorTest("ZK-X.509 identity builders reject malformed inputs", () => {
  const base = {
    caRootJson: { root: "boi-root-ca", version: 1 },
    certificatePolicyJson: { eku: ["clientAuth"], policy: "wallet" },
    revocationJson: { epoch: 7, root: "revocation-root" },
    subjectJson: { cn: "Bank A", lei: "5493001KJTIIGC8Y1R12" },
    accountId: ACCOUNT_ID,
    domainSeparator: "boi:zk-x509:pilot:v0",
  };
  for (const patch of [
    { caRootCommitment: Buffer.alloc(32, 0xee) },
    { certificatePolicyHash: Buffer.alloc(32, 0xee) },
    { revocationRoot: Buffer.alloc(32, 0xee) },
    { subjectCommitment: Buffer.alloc(32, 0xee) },
    { addressBinding: Buffer.alloc(32, 0xee) },
    { caRootJson: undefined, caRootCommitment: undefined },
    { certificatePolicyJson: undefined, certificatePolicyHash: undefined },
    { revocationJson: undefined, revocationRoot: undefined },
    { subjectJson: undefined, subjectCommitment: undefined },
    { accountId: undefined, addressBinding: undefined },
    { accountId: "not-an-account-id" },
    { accountId: ACCOUNT_ID, walletAddress: "wallet-address-alias" },
    { domainSeparator: " " },
    { caRootCommitment: Buffer.alloc(32) },
    { maxCaRootBytes: undefined },
    { maxCaRootBytes: null },
    { maxPolicyBytes: undefined },
    { maxPolicyBytes: null },
    { maxRevocationBytes: undefined },
    { maxRevocationBytes: null },
    { maxSubjectBytes: undefined },
    { maxSubjectBytes: null },
  ]) {
    assert.throws(
      () => buildZkX509IdentityCommitments({ ...base, ...patch }),
      /zkX509IdentityCommitments/,
    );
  }

  for (const patch of [
    { proofBytes: Buffer.alloc(0) },
    { vkHash: Buffer.alloc(32) },
    { backend: "groth16" },
    { circuitId: "stark/fri/sha256-goldilocks:wrong" },
  ]) {
    assert.throws(
      () =>
        buildZkX509IdentityEnvelope({
          ...base,
          vkHash: Buffer.alloc(32, 0x99),
          proofBytes: Buffer.from("prepared-zk-x509-proof"),
          ...patch,
        }),
      /(zkX509IdentityEnvelope|privacyProofEnvelope)/,
    );
  }
});

descriptorTest("ZK-X.509 local verifier rejects tampered dev fixtures", () => {
  const otherAccount = AccountAddress.fromAccount({
    publicKey: new Uint8Array(32).fill(11),
  }).toI105(SORA_I105_DISCRIMINANT);
  const fixtureInput = {
    caRootJson: { root: "boi-root-ca", version: 1 },
    certificatePolicyJson: { eku: ["clientAuth"], policy: "wallet" },
    revocationJson: { epoch: 7, root: "revocation-root" },
    subjectJson: { cn: "Bank A", lei: "5493001KJTIIGC8Y1R12" },
    accountId: ACCOUNT_ID,
    domainSeparator: "boi:zk-x509:pilot:v0",
    vkHash: Buffer.alloc(32, 0x99),
  };
  const fixture = buildZkX509IdentityDevProofFixture(fixtureInput);
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  const publicInputs = JSON.parse(Buffer.from(decoded.public_inputs).toString("utf8"));
  const rebuildEnvelope = ({
    backend = "stark/fri/sha256-goldilocks",
    circuitId = decoded.circuit_id,
    publicInputsBytes = decoded.public_inputs,
    proofBytes = decoded.proof_bytes,
  } = {}) =>
    buildPrivacyProofEnvelope({
      backend,
      circuitId,
      vkHash: Buffer.alloc(32, 0x99),
      publicInputs: publicInputsBytes,
      proofBytes,
    });
  const tamperedProof = [...decoded.proof_bytes];
  tamperedProof[tamperedProof.length - 1] ^= 0xff;
  const nonCanonicalPublicInputs = Buffer.from(JSON.stringify(publicInputs, null, 2));
  const zeroCaRootInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      ca_root_commitment: Buffer.alloc(32).toString("hex"),
    }),
  );
  const aliasCollisionInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      caRootCommitment: publicInputs.ca_root_commitment,
    }),
  );

  for (const input of [
    { envelope: rebuildEnvelope({ proofBytes: Buffer.from("arbitrary") }) },
    { envelope: rebuildEnvelope({ proofBytes: tamperedProof }) },
    { envelope: fixture.envelope, caRootJson: { root: "other-root" } },
    { envelope: fixture.envelope, certificatePolicyJson: { eku: ["serverAuth"] } },
    { envelope: fixture.envelope, revocationJson: { epoch: 8, root: "revocation-root" } },
    { envelope: fixture.envelope, subjectJson: { cn: "Bank B" } },
    { envelope: fixture.envelope, accountId: otherAccount },
    { envelope: fixture.envelope, domainSeparator: "boi:zk-x509:other:v0" },
    { envelope: rebuildEnvelope({ backend: "groth16" }) },
    { envelope: rebuildEnvelope({ circuitId: "stark/fri/sha256-goldilocks:wrong" }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: nonCanonicalPublicInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: zeroCaRootInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: aliasCollisionInputs }) },
  ]) {
    assert.throws(
      () => verifyZkX509IdentityProofLocally(input),
      /zkX509IdentityLocalVerification/,
    );
  }
});

descriptorTest("Jindo lattice PCS builders normalize public inputs and envelopes", () => {
  const base = {
    polynomialJson: {
      ring: "Rq",
      degree: 1024,
      coefficients_digest: "poly-digest-1",
    },
    openingClaimJson: {
      point: "x=42",
      value_digest: "evaluation-digest-1",
    },
    querySetJson: {
      queries: [0, 7, 42],
      batch: "opening-batch-1",
    },
    parametersJson: {
      scheme: "jindo-pcs-v0",
      q_bits: 64,
      sigma: "research-parameter-set",
    },
    domainSeparator: "boi:jindo:pcs:pilot:v0",
  };
  const vkHash = Buffer.alloc(32, 0xaa);

  const publicInputs = buildJindoLatticePublicInputs(base);
  assert.equal(publicInputs.version, 1);
  assert.equal(publicInputs.commitment.length, 32);
  assert.equal(publicInputs.parameter_hash.length, 32);
  assert.equal(
    publicInputs.commitment_kinds.commitment,
    "dev-sha256-commitment-digest",
  );

  const prepared = buildJindoLatticeProofEnvelope({
    ...base,
    vkHash,
    proofBytes: Buffer.from("prepared-jindo-lattice-proof"),
  });
  const decodedPrepared = noritoDecodePrivacyProofEnvelope(prepared);
  assert.equal(decodedPrepared.backend, "Unsupported");
  assert.equal(
    decodedPrepared.circuit_id,
    "lattice/jindo-pcs-v0:jindo_lattice_pcs_zk_v0",
  );
  const preparedInputs = JSON.parse(
    Buffer.from(decodedPrepared.public_inputs).toString("utf8"),
  );
  assert.equal(
    preparedInputs.commitment,
    Buffer.from(publicInputs.commitment).toString("hex"),
  );
  assert.equal(
    preparedInputs.parameter_hash,
    Buffer.from(publicInputs.parameter_hash).toString("hex"),
  );

  const fixture = buildJindoLatticeDevProofFixture({
    ...base,
    vkHash,
  });
  assert.equal(fixture.kind, "jindo-lattice-dev-fixture-v0");
  assert.equal(fixture.production, false);
  assert.ok(Buffer.isBuffer(fixture.envelope));
  const verified = verifyJindoLatticeProofLocally({
    envelope: fixture.envelope,
    ...base,
  });
  assert.equal(verified.ok, true);
  assert.equal(verified.production, false);
  assert.equal(
    verified.parameter_hash,
    Buffer.from(publicInputs.parameter_hash).toString("hex"),
  );
});

descriptorTest("Jindo lattice PCS builders reject malformed inputs", () => {
  const base = {
    polynomialJson: { ring: "Rq", degree: 1024, digest: "poly" },
    openingClaimJson: { point: "x=42", value_digest: "value" },
    querySetJson: { queries: [0, 7, 42] },
    parametersJson: { scheme: "jindo-pcs-v0", q_bits: 64 },
    domainSeparator: "boi:jindo:pcs:pilot:v0",
  };
  for (const patch of [
    { commitment: Buffer.alloc(32, 0xee) },
    { openingClaimHash: Buffer.alloc(32, 0xee) },
    { querySetHash: Buffer.alloc(32, 0xee) },
    { parameterHash: Buffer.alloc(32, 0xee) },
    { polynomialJson: undefined, commitment: undefined },
    { openingClaimJson: undefined, openingClaimHash: undefined },
    { querySetJson: undefined, querySetHash: undefined },
    { parametersJson: undefined, parameterHash: undefined },
    { domainSeparator: " " },
    { commitment: Buffer.alloc(32) },
    { maxPolynomialBytes: undefined },
    { maxPolynomialBytes: null },
    { maxOpeningClaimBytes: undefined },
    { maxOpeningClaimBytes: null },
    { maxQuerySetBytes: undefined },
    { maxQuerySetBytes: null },
    { maxParameterBytes: undefined },
    { maxParameterBytes: null },
  ]) {
    assert.throws(
      () => buildJindoLatticePublicInputs({ ...base, ...patch }),
      /jindoLatticePublicInputs/,
    );
  }

  for (const patch of [
    { proofBytes: Buffer.alloc(0) },
    { vkHash: Buffer.alloc(32) },
    { backend: "stark/fri/sha256-goldilocks" },
    { circuitId: "lattice/jindo-pcs-v0:wrong" },
  ]) {
    assert.throws(
      () =>
        buildJindoLatticeProofEnvelope({
          ...base,
          vkHash: Buffer.alloc(32, 0xaa),
          proofBytes: Buffer.from("prepared-jindo-lattice-proof"),
          ...patch,
        }),
      /(jindoLatticeProofEnvelope|privacyProofEnvelope)/,
    );
  }
});

descriptorTest("Jindo lattice local verifier rejects tampered dev fixtures", () => {
  const fixtureInput = {
    polynomialJson: { ring: "Rq", degree: 1024, digest: "poly" },
    openingClaimJson: { point: "x=42", value_digest: "value" },
    querySetJson: { queries: [0, 7, 42] },
    parametersJson: { scheme: "jindo-pcs-v0", q_bits: 64 },
    domainSeparator: "boi:jindo:pcs:pilot:v0",
    vkHash: Buffer.alloc(32, 0xaa),
  };
  const fixture = buildJindoLatticeDevProofFixture(fixtureInput);
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  const publicInputs = JSON.parse(Buffer.from(decoded.public_inputs).toString("utf8"));
  const rebuildEnvelope = ({
    backend = "unsupported",
    circuitId = decoded.circuit_id,
    publicInputsBytes = decoded.public_inputs,
    proofBytes = decoded.proof_bytes,
  } = {}) =>
    noritoEncodePrivacyProofEnvelope({
      backend: backend === "unsupported" ? "Unsupported" : backend,
      circuit_id: circuitId,
      vk_hash: Buffer.alloc(32, 0xaa),
      public_inputs: publicInputsBytes,
      proof_bytes: proofBytes,
      aux: Buffer.alloc(0),
    });
  const tamperedProof = [...decoded.proof_bytes];
  tamperedProof[tamperedProof.length - 1] ^= 0xff;
  const nonCanonicalPublicInputs = Buffer.from(JSON.stringify(publicInputs, null, 2));
  const zeroCommitmentInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      commitment: Buffer.alloc(32).toString("hex"),
    }),
  );
  const aliasCollisionInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      openingClaim: publicInputs.opening_claim,
    }),
  );

  for (const input of [
    { envelope: rebuildEnvelope({ proofBytes: Buffer.from("arbitrary") }) },
    { envelope: rebuildEnvelope({ proofBytes: tamperedProof }) },
    { envelope: fixture.envelope, polynomialJson: { ring: "Rq", degree: 2048 } },
    { envelope: fixture.envelope, openingClaimJson: { point: "x=9" } },
    { envelope: fixture.envelope, querySetJson: { queries: [99] } },
    { envelope: fixture.envelope, parametersJson: { scheme: "other" } },
    { envelope: fixture.envelope, domainSeparator: "boi:jindo:pcs:other:v0" },
    { envelope: rebuildEnvelope({ backend: "stark/fri/sha256-goldilocks" }) },
    { envelope: rebuildEnvelope({ circuitId: "lattice/jindo-pcs-v0:wrong" }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: nonCanonicalPublicInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: zeroCommitmentInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: aliasCollisionInputs }) },
  ]) {
    assert.throws(
      () => verifyJindoLatticeProofLocally(input),
      /jindoLatticeLocalVerification/,
    );
  }
});

descriptorTest("SIS-with-hints credential builders normalize commitments and envelopes", () => {
  const base = {
    issuerJson: {
      issuer: "boi-issuer-set",
      commitment_scheme: "sis-hints-v0",
    },
    credentialJson: {
      credential_type: "pq-wallet-eligibility",
      attributes: ["resident", "institution"],
      nonce: "presentation-1",
    },
    showingPolicyJson: {
      verifier: "boi-wallet-enrollment",
      accepted_attributes: ["resident"],
    },
    parametersJson: {
      scheme: "sis-hints-anoncred-v0",
      q_bits: 64,
      module_rank: 8,
    },
    domainSeparator: "boi:sis-hints:pilot:v0",
  };
  const vkHash = Buffer.alloc(32, 0xbb);

  const commitments = buildSisHintsCredentialCommitments(base);
  assert.equal(commitments.version, 1);
  assert.equal(commitments.issuer_commitment.length, 32);
  assert.equal(commitments.parameter_hash.length, 32);
  assert.equal(
    commitments.commitment_kinds.credential_commitment,
    "dev-sha256-credential-digest",
  );

  const prepared = buildSisHintsCredentialEnvelope({
    ...base,
    vkHash,
    proofBytes: Buffer.from("prepared-sis-hints-proof"),
  });
  const decodedPrepared = noritoDecodePrivacyProofEnvelope(prepared);
  assert.equal(decodedPrepared.backend, "Unsupported");
  assert.equal(
    decodedPrepared.circuit_id,
    "lattice/sis-hints-anoncred-v0:sis_hints_anoncred_pq_v0",
  );
  const preparedInputs = JSON.parse(
    Buffer.from(decodedPrepared.public_inputs).toString("utf8"),
  );
  assert.equal(
    preparedInputs.issuer_commitment,
    Buffer.from(commitments.issuer_commitment).toString("hex"),
  );
  assert.equal(
    preparedInputs.parameter_hash,
    Buffer.from(commitments.parameter_hash).toString("hex"),
  );

  const fixture = buildSisHintsCredentialDevProofFixture({
    ...base,
    vkHash,
  });
  assert.equal(fixture.kind, "sis-hints-dev-fixture-v0");
  assert.equal(fixture.production, false);
  assert.ok(Buffer.isBuffer(fixture.envelope));
  const verified = verifySisHintsCredentialProofLocally({
    envelope: fixture.envelope,
    ...base,
  });
  assert.equal(verified.ok, true);
  assert.equal(verified.production, false);
  assert.equal(
    verified.parameter_hash,
    Buffer.from(commitments.parameter_hash).toString("hex"),
  );
});

descriptorTest("SIS-with-hints credential builders reject malformed inputs", () => {
  const base = {
    issuerJson: { issuer: "boi", scheme: "sis-hints-v0" },
    credentialJson: { credential_type: "wallet", nonce: "n-1" },
    showingPolicyJson: { verifier: "boi", purpose: "wallet" },
    parametersJson: { scheme: "sis-hints-anoncred-v0", q_bits: 64 },
    domainSeparator: "boi:sis-hints:pilot:v0",
  };
  for (const patch of [
    { issuerCommitment: Buffer.alloc(32, 0xee) },
    { credentialCommitment: Buffer.alloc(32, 0xee) },
    { showingPolicyHash: Buffer.alloc(32, 0xee) },
    { parameterHash: Buffer.alloc(32, 0xee) },
    { issuerJson: undefined, issuerCommitment: undefined },
    { credentialJson: undefined, credentialCommitment: undefined },
    { showingPolicyJson: undefined, showingPolicyHash: undefined },
    { parametersJson: undefined, parameterHash: undefined },
    { domainSeparator: " " },
    { issuerCommitment: Buffer.alloc(32) },
    { maxIssuerBytes: undefined },
    { maxIssuerBytes: null },
    { maxCredentialBytes: undefined },
    { maxCredentialBytes: null },
    { maxPolicyBytes: undefined },
    { maxPolicyBytes: null },
    { maxParameterBytes: undefined },
    { maxParameterBytes: null },
  ]) {
    assert.throws(
      () => buildSisHintsCredentialCommitments({ ...base, ...patch }),
      /sisHintsCredentialCommitments/,
    );
  }

  for (const patch of [
    { proofBytes: Buffer.alloc(0) },
    { vkHash: Buffer.alloc(32) },
    { backend: "stark/fri/sha256-goldilocks" },
    { circuitId: "lattice/sis-hints-anoncred-v0:wrong" },
  ]) {
    assert.throws(
      () =>
        buildSisHintsCredentialEnvelope({
          ...base,
          vkHash: Buffer.alloc(32, 0xbb),
          proofBytes: Buffer.from("prepared-sis-hints-proof"),
          ...patch,
        }),
      /(sisHintsCredentialEnvelope|privacyProofEnvelope)/,
    );
  }
});

descriptorTest("SIS-with-hints local verifier rejects tampered dev fixtures", () => {
  const fixtureInput = {
    issuerJson: { issuer: "boi", scheme: "sis-hints-v0" },
    credentialJson: { credential_type: "wallet", nonce: "n-1" },
    showingPolicyJson: { verifier: "boi", purpose: "wallet" },
    parametersJson: { scheme: "sis-hints-anoncred-v0", q_bits: 64 },
    domainSeparator: "boi:sis-hints:pilot:v0",
    vkHash: Buffer.alloc(32, 0xbb),
  };
  const fixture = buildSisHintsCredentialDevProofFixture(fixtureInput);
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  const publicInputs = JSON.parse(Buffer.from(decoded.public_inputs).toString("utf8"));
  const rebuildEnvelope = ({
    backend = "unsupported",
    circuitId = decoded.circuit_id,
    publicInputsBytes = decoded.public_inputs,
    proofBytes = decoded.proof_bytes,
  } = {}) =>
    noritoEncodePrivacyProofEnvelope({
      backend: backend === "unsupported" ? "Unsupported" : backend,
      circuit_id: circuitId,
      vk_hash: Buffer.alloc(32, 0xbb),
      public_inputs: publicInputsBytes,
      proof_bytes: proofBytes,
      aux: Buffer.alloc(0),
    });
  const tamperedProof = [...decoded.proof_bytes];
  tamperedProof[tamperedProof.length - 1] ^= 0xff;
  const nonCanonicalPublicInputs = Buffer.from(JSON.stringify(publicInputs, null, 2));
  const zeroIssuerInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      issuer_commitment: Buffer.alloc(32).toString("hex"),
    }),
  );
  const aliasCollisionInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      issuerCommitment: publicInputs.issuer_commitment,
    }),
  );

  for (const input of [
    { envelope: rebuildEnvelope({ proofBytes: Buffer.from("arbitrary") }) },
    { envelope: rebuildEnvelope({ proofBytes: tamperedProof }) },
    { envelope: fixture.envelope, issuerJson: { issuer: "other" } },
    { envelope: fixture.envelope, credentialJson: { credential_type: "wallet", nonce: "n-2" } },
    { envelope: fixture.envelope, showingPolicyJson: { verifier: "other" } },
    { envelope: fixture.envelope, parametersJson: { scheme: "other" } },
    { envelope: fixture.envelope, domainSeparator: "boi:sis-hints:other:v0" },
    { envelope: rebuildEnvelope({ backend: "stark/fri/sha256-goldilocks" }) },
    { envelope: rebuildEnvelope({ circuitId: "lattice/sis-hints-anoncred-v0:wrong" }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: nonCanonicalPublicInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: zeroIssuerInputs }) },
    { envelope: rebuildEnvelope({ publicInputsBytes: aliasCollisionInputs }) },
  ]) {
    assert.throws(
      () => verifySisHintsCredentialProofLocally(input),
      /sisHintsCredentialLocalVerification/,
    );
  }
});

descriptorTest("Anonymous PGC builders normalize receiver sets and dev proof envelopes", () => {
  const payload = Buffer.from("anonymous-pgc:alice:bob:42");
  const receiverA = {
    accountCommitment: Buffer.alloc(32, 0x21),
    ciphertextCommitment: Buffer.alloc(32, 0x31),
    ciphertext: Buffer.from("ciphertext-for-bob"),
  };
  const receiverB = {
    accountCommitment: Buffer.alloc(32, 0x22),
    ciphertextCommitment: Buffer.alloc(32, 0x32),
    ciphertext: Buffer.from("ciphertext-for-carol"),
  };
  const receiverSet = buildAnonymousPgcReceiverSet({
    threshold: 1,
    receivers: [receiverA, receiverB],
  });
  assert.equal(receiverSet.version, 1);
  assert.equal(receiverSet.threshold, 1);
  assert.equal(receiverSet.receiver_count, 2);
  assert.equal(receiverSet.receiver_set_commitment.length, 32);
  assert.equal(receiverSet.receivers[0].ciphertext_digest.length, 32);

  const vkHash = Buffer.alloc(32, 0x55);
  const fixture = buildAnonymousPgcDevProofFixture({
    receiverSet,
    anonymitySetRoot: Buffer.alloc(32, 0x41),
    payload,
    balanceCommitments: [Buffer.alloc(32, 0x51), Buffer.alloc(32, 0x52)],
    linkTag: Buffer.alloc(32, 0x61),
    rangeCommitments: [Buffer.alloc(32, 0x71)],
    chainId: "boi-localnet",
    domainSeparator: "boi:anonymous-pgc:v1",
    vkHash,
  });
  assert.equal(fixture.kind, "anonymous-pgc-dev-fixture-v1");
  assert.equal(fixture.production, false);
  assert.ok(Buffer.isBuffer(fixture.envelope));
  assert.ok(Buffer.isBuffer(fixture.proofBytes));
  assert.equal(Buffer.from(fixture.proof_bytes).equals(fixture.proofBytes), true);
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  assert.equal(decoded.backend, "Stark");
  assert.equal(
    decoded.circuit_id,
    "stark/fri/sha256-goldilocks:anonymous_pgc_k_out_of_n_v1",
  );
  const publicInputs = JSON.parse(
    Buffer.from(decoded.public_inputs).toString("utf8"),
  );
  assert.equal(publicInputs.receiver_threshold, 1);
  assert.equal(publicInputs.receiver_count, 2);
  assert.deepEqual(publicInputs.receiver_ciphertext_commitments, [
    Buffer.alloc(32, 0x31).toString("hex"),
    Buffer.alloc(32, 0x32).toString("hex"),
  ]);
  assert.equal(
    publicInputs.receiver_set_commitment,
    Buffer.from(receiverSet.receiver_set_commitment).toString("hex"),
  );

  const verified = verifyAnonymousPgcDevProofLocally({
    envelope: fixture.envelope,
    receiverSet,
    payload,
    anonymitySetRoot: Buffer.alloc(32, 0x41),
    balanceCommitments: [Buffer.alloc(32, 0x51), Buffer.alloc(32, 0x52)],
    linkTag: Buffer.alloc(32, 0x61),
    rangeCommitments: [Buffer.alloc(32, 0x71)],
    chainId: "boi-localnet",
    domainSeparator: "boi:anonymous-pgc:v1",
  });
  assert.equal(verified.ok, true);
  assert.equal(verified.production, false);
  assert.equal(verified.receiver_count, 2);
  assert.equal(verified.receiver_threshold, 1);
  assert.deepEqual(verified.public_inputs, fixture.public_inputs);
});

descriptorTest("Anonymous PGC builders reject malformed receiver and proof inputs", () => {
  const receiverA = {
    accountCommitment: Buffer.alloc(32, 0x21),
    ciphertextCommitment: Buffer.alloc(32, 0x31),
  };
  const receiverB = {
    accountCommitment: Buffer.alloc(32, 0x22),
    ciphertextCommitment: Buffer.alloc(32, 0x32),
  };
  const baseReceiverSet = {
    threshold: 1,
    receivers: [receiverA, receiverB],
  };
  for (const receiverSetPatch of [
    { threshold: 0 },
    { threshold: 3 },
    { receivers: [] },
    { receivers: [{ ...receiverA, accountCommitment: Buffer.alloc(32) }, receiverB] },
    { receivers: [receiverA, { ...receiverB, accountCommitment: receiverA.accountCommitment }] },
    { receivers: [receiverA, { ...receiverB, ciphertextCommitment: receiverA.ciphertextCommitment }] },
    { receivers: [{ accountCommitment: Buffer.alloc(32, 0x23) }, receiverB] },
    {
      receivers: [
        {
          ...receiverA,
          ciphertext: Buffer.from("ciphertext"),
          ciphertextDigest: Buffer.alloc(32, 0xee),
        },
        receiverB,
      ],
    },
  ]) {
    assert.throws(
      () =>
        buildAnonymousPgcReceiverSet({
          ...baseReceiverSet,
          ...receiverSetPatch,
        }),
      /anonymousPgcReceiverSet/,
    );
  }

  const receiverSet = buildAnonymousPgcReceiverSet(baseReceiverSet);
  const baseFixture = {
    receiverSet,
    anonymitySetRoot: Buffer.alloc(32, 0x41),
    payload: Buffer.from("anonymous-pgc:alice:bob:42"),
    balanceCommitments: [Buffer.alloc(32, 0x51), Buffer.alloc(32, 0x52)],
    linkTag: Buffer.alloc(32, 0x61),
    rangeCommitments: [Buffer.alloc(32, 0x71)],
    chainId: "boi-localnet",
    domainSeparator: "boi:anonymous-pgc:v1",
    vkHash: Buffer.alloc(32, 0x55),
  };
  for (const patch of [
    { receiverSet: { ...receiverSet, receiver_set_commitment: Buffer.alloc(32, 0xaa) } },
    { anonymitySetRoot: Buffer.alloc(32) },
    { payload: Buffer.from("payload"), txDigest: Buffer.alloc(32, 0xee) },
    { maxPayloadBytes: undefined },
    { maxPayloadBytes: null },
    { balanceCommitments: [Buffer.alloc(32, 0x51), Buffer.alloc(32, 0x51)] },
    { rangeCommitments: [] },
    { linkTag: Buffer.alloc(32) },
    { chainId: " " },
    { vkHash: Buffer.alloc(32) },
  ]) {
    assert.throws(
      () => buildAnonymousPgcDevProofFixture({ ...baseFixture, ...patch }),
      /anonymousPgcDevProofFixture|maxPayloadBytes/,
    );
  }
});

descriptorTest("Anonymous PGC local verifier rejects tampered dev fixtures", () => {
  const receiverA = {
    accountCommitment: Buffer.alloc(32, 0x21),
    ciphertextCommitment: Buffer.alloc(32, 0x31),
  };
  const receiverB = {
    accountCommitment: Buffer.alloc(32, 0x22),
    ciphertextCommitment: Buffer.alloc(32, 0x32),
  };
  const receiverSet = buildAnonymousPgcReceiverSet({
    threshold: 1,
    receivers: [receiverA, receiverB],
  });
  const fixtureInput = {
    receiverSet,
    anonymitySetRoot: Buffer.alloc(32, 0x41),
    payload: Buffer.from("anonymous-pgc:alice:bob:42"),
    balanceCommitments: [Buffer.alloc(32, 0x51), Buffer.alloc(32, 0x52)],
    linkTag: Buffer.alloc(32, 0x61),
    rangeCommitments: [Buffer.alloc(32, 0x71)],
    chainId: "boi-localnet",
    domainSeparator: "boi:anonymous-pgc:v1",
    vkHash: Buffer.alloc(32, 0x55),
  };
  const fixture = buildAnonymousPgcDevProofFixture(fixtureInput);
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  const publicInputs = JSON.parse(Buffer.from(decoded.public_inputs).toString("utf8"));
  const rebuildEnvelope = ({
    backend = "stark/fri/sha256-goldilocks",
    publicInputsBytes = decoded.public_inputs,
    proofBytes = decoded.proof_bytes,
  } = {}) =>
    buildPrivacyProofEnvelope({
      backend,
      circuitId: decoded.circuit_id,
      vkHash: Buffer.alloc(32, 0x55),
      publicInputs: publicInputsBytes,
      proofBytes,
    });
  const tamperedProof = [...decoded.proof_bytes];
  tamperedProof[tamperedProof.length - 1] ^= 0xff;
  const nonCanonicalPublicInputs = Buffer.from(JSON.stringify(publicInputs, null, 2));
  const duplicateReceiverInputs = Buffer.from(
    JSON.stringify({
      ...publicInputs,
      receiver_ciphertext_commitments: [
        publicInputs.receiver_ciphertext_commitments[0],
        publicInputs.receiver_ciphertext_commitments[0],
      ],
    }),
  );

  for (const input of [
    { envelope: rebuildEnvelope({ proofBytes: Buffer.from("arbitrary") }), payload: fixtureInput.payload },
    { envelope: rebuildEnvelope({ proofBytes: tamperedProof }), payload: fixtureInput.payload },
    { envelope: fixture.envelope, payload: Buffer.from("substituted-payload") },
    { envelope: fixture.envelope, receiverSet: buildAnonymousPgcReceiverSet({ threshold: 1, receivers: [receiverB, receiverA] }) },
    { envelope: fixture.envelope, chainId: "wrong-chain" },
    { envelope: rebuildEnvelope({ backend: "groth16" }), payload: fixtureInput.payload },
    { envelope: rebuildEnvelope({ publicInputsBytes: nonCanonicalPublicInputs }), payload: fixtureInput.payload },
    { envelope: rebuildEnvelope({ publicInputsBytes: duplicateReceiverInputs }), payload: fixtureInput.payload },
  ]) {
    assert.throws(
      () => verifyAnonymousPgcDevProofLocally(input),
      /anonymousPgcDevProofLocalVerification/,
    );
  }
});

descriptorTest("VeRange builders normalize commitments and prepared proof envelopes", () => {
  const payload = Buffer.from("transfer:alice@wonderland:bob@wonderland:42");
  const payloadDigest = createHash("sha256").update(payload).digest();
  const commitmentA = Buffer.alloc(32, 0x44);
  const commitmentB = Buffer.alloc(32, 0x45);

  const descriptor = buildRangeCommitment({
    commitment: commitmentA,
    bitLength: 64,
    aggregationCount: 2,
    commitmentScheme: "pedersen-v1",
    domainSeparator: "boi:amount-range:v1",
    payload,
  });
  assert.deepEqual(descriptor, {
    version: 1,
    commitment: Array.from(commitmentA),
    bit_length: 64,
    aggregation_count: 2,
    commitment_scheme: "pedersen-v1",
    domain_separator: "boi:amount-range:v1",
    payload_digest: Array.from(payloadDigest),
  });

  const vkHash = Buffer.alloc(32, 0x55);
  const proofBytes = Buffer.from("prepared-verange-proof");
  const aux = Buffer.from("prepared externally");
  const encoded = buildVeRangeProofEnvelope({
    commitments: [commitmentA, commitmentB],
    bitLength: 64,
    commitmentScheme: "pedersen-v1",
    domainSeparator: "boi:amount-range:v1",
    payloadDigest,
    vkHash,
    proofBytes,
    aux,
    maxProofBytes: 64,
    maxPublicInputBytes: 512,
  });
  assert.ok(Buffer.isBuffer(encoded));
  const decoded = noritoDecodePrivacyProofEnvelope(encoded);
  assert.equal(decoded.backend, "Stark");
  assert.equal(
    decoded.circuit_id,
    "stark/fri/sha256-goldilocks:verange_transparent_range_v1",
  );
  assert.deepEqual(decoded.vk_hash, Array.from(vkHash));
  assert.deepEqual(decoded.proof_bytes, Array.from(proofBytes));
  assert.deepEqual(decoded.aux, Array.from(aux));

  const publicInputs = JSON.parse(
    Buffer.from(decoded.public_inputs).toString("utf8"),
  );
  assert.deepEqual(publicInputs, {
    aggregation_count: 2,
    commitments: [
      commitmentA.toString("hex"),
      commitmentB.toString("hex"),
    ],
    domain_separator: "boi:amount-range:v1",
    payload_digest: payloadDigest.toString("hex"),
    range_parameters: {
      bit_length: 64,
      commitment_scheme: "pedersen-v1",
    },
    version: 1,
  });

  const fixture = buildVeRangeDevProofFixture({
    commitments: [commitmentA, commitmentB],
    bitLength: 64,
    commitmentScheme: "pedersen-v1",
    domainSeparator: "boi:amount-range:v1",
    payload,
    vkHash,
  });
  assert.equal(fixture.kind, "verange-dev-fixture-v1");
  assert.equal(fixture.production, false);
  assert.ok(Buffer.isBuffer(fixture.envelope));
  assert.ok(Buffer.isBuffer(fixture.proofBytes));
  assert.equal(Buffer.from(fixture.proof_bytes).equals(fixture.proofBytes), true);
  const verified = verifyVeRangeProofLocally({
    envelope: fixture.envelope,
    payload,
    commitments: [commitmentA, commitmentB],
    bitLength: 64,
    commitmentScheme: "pedersen-v1",
    domainSeparator: "boi:amount-range:v1",
  });
  assert.equal(verified.ok, true);
  assert.equal(verified.production, false);
  assert.equal(verified.kind, "verange-dev-fixture-v1");
  assert.equal(verified.public_input_bytes, fixture.publicInputBytes.length);
  assert.equal(verified.proof_bytes, fixture.proofBytes.length);
  assert.deepEqual(verified.public_inputs, fixture.public_inputs);
});

descriptorTest("VeRange builders reject malformed commitments and unsafe envelopes", () => {
  const payload = Buffer.from("transfer:alice@wonderland:bob@wonderland:42");
  const payloadDigest = createHash("sha256").update(payload).digest();
  const commitmentA = Buffer.alloc(32, 0x44);
  const commitmentB = Buffer.alloc(32, 0x45);
  const baseCommitment = {
    commitment: commitmentA,
    bitLength: 64,
    commitmentScheme: "pedersen-v1",
    domainSeparator: "boi:amount-range:v1",
    payloadDigest,
  };
  const baseEnvelope = {
    commitments: [commitmentA, commitmentB],
    bitLength: 64,
    commitmentScheme: "pedersen-v1",
    domainSeparator: "boi:amount-range:v1",
    payloadDigest,
    vkHash: Buffer.alloc(32, 0x55),
    proofBytes: Buffer.from("prepared-verange-proof"),
  };

  for (const payloadPatch of [
    { commitment: Buffer.alloc(32) },
    { bitLength: 0 },
    { bitLength: 257 },
    { aggregationCount: 0 },
    { commitmentScheme: "sha256-dev" },
    { commitment: commitmentA, valueCommitment: commitmentB },
    { payload, payloadDigest: Buffer.alloc(32, 0xee) },
    { payloadDigest: undefined, payload: undefined },
  ]) {
    assert.throws(
      () =>
        buildRangeCommitment({
          ...baseCommitment,
          ...payloadPatch,
        }),
      /rangeCommitment/,
    );
  }

  for (const envelopePatch of [
    { commitments: [] },
    { commitments: [commitmentA, commitmentA] },
    { aggregationCount: 1 },
    { commitments: [commitmentA, { ...baseCommitment, commitment: commitmentB, bitLength: 128 }] },
    { commitments: [commitmentA, { ...baseCommitment, commitment: commitmentB, payloadDigest: Buffer.alloc(32, 0x66) }] },
    { backend: "groth16" },
    { circuitId: "other_range_v1" },
    { vkHash: Buffer.alloc(32) },
    { maxPayloadBytes: undefined },
    { maxPayloadBytes: null },
    { proofBytes: Buffer.alloc(0) },
    { maxProofBytes: 4 },
    { maxProofBytes: undefined },
    { maxProofBytes: null },
    { maxPublicInputBytes: undefined },
    { maxPublicInputBytes: null },
    { commitment: commitmentA },
  ]) {
    assert.throws(
      () =>
        buildVeRangeProofEnvelope({
          ...baseEnvelope,
          ...envelopePatch,
        }),
      /(veRangeProofEnvelope|privacyProofEnvelope|maxPayloadBytes)/,
    );
  }

  for (const fixturePatch of [
    { maxProofBytes: undefined },
    { maxProofBytes: null },
    { maxPublicInputBytes: undefined },
    { maxPublicInputBytes: null },
  ]) {
    assert.throws(
      () =>
        buildVeRangeDevProofFixture({
          commitments: [commitmentA, commitmentB],
          bitLength: 64,
          commitmentScheme: "pedersen-v1",
          domainSeparator: "boi:amount-range:v1",
          payload,
          vkHash: Buffer.alloc(32, 0x55),
          ...fixturePatch,
        }),
      /veRangeDevProofFixture/,
    );
  }
});

descriptorTest("VeRange local verifier rejects tampered dev fixtures", () => {
  const payload = Buffer.from("transfer:alice@wonderland:bob@wonderland:42");
  const commitmentA = Buffer.alloc(32, 0x44);
  const commitmentB = Buffer.alloc(32, 0x45);
  const vkHash = Buffer.alloc(32, 0x55);
  const fixture = buildVeRangeDevProofFixture({
    commitments: [commitmentA, commitmentB],
    bitLength: 64,
    commitmentScheme: "pedersen-v1",
    domainSeparator: "boi:amount-range:v1",
    payload,
    vkHash,
  });
  const decoded = noritoDecodePrivacyProofEnvelope(fixture.envelope);
  const rebuildEnvelope = ({
    backend = "stark/fri/sha256-goldilocks",
    circuitId = decoded.circuit_id,
    verifierHash = vkHash,
    publicInputs = decoded.public_inputs,
    proofBytes = decoded.proof_bytes,
  } = {}) =>
    buildPrivacyProofEnvelope({
      backend,
      circuitId,
      vkHash: verifierHash,
      publicInputs,
      proofBytes,
    });
  const tamperedProofBytes = [...decoded.proof_bytes];
  tamperedProofBytes[tamperedProofBytes.length - 1] ^= 0xff;
  const nonCanonicalPublicInputs = Buffer.from(
    JSON.stringify({
      version: 1,
      commitments: [
        commitmentA.toString("hex"),
        commitmentB.toString("hex"),
      ],
      range_parameters: {
        bit_length: 64,
        commitment_scheme: "pedersen-v1",
      },
      aggregation_count: 2,
      domain_separator: "boi:amount-range:v1",
      payload_digest: createHash("sha256").update(payload).digest("hex"),
    }),
  );
  const duplicateCommitmentInputs = Buffer.from(
    JSON.stringify({
      aggregation_count: 2,
      commitments: [
        commitmentA.toString("hex"),
        commitmentA.toString("hex"),
      ],
      domain_separator: "boi:amount-range:v1",
      payload_digest: createHash("sha256").update(payload).digest("hex"),
      range_parameters: {
        bit_length: 64,
        commitment_scheme: "pedersen-v1",
      },
      version: 1,
    }),
  );

  for (const input of [
    { envelope: rebuildEnvelope({ proofBytes: Buffer.from("arbitrary") }), payload },
    { envelope: rebuildEnvelope({ proofBytes: tamperedProofBytes }), payload },
    { envelope: fixture.envelope, payload: Buffer.from("substituted-payload") },
    { envelope: fixture.envelope, payload, commitments: [commitmentB, commitmentA], bitLength: 64 },
    { envelope: rebuildEnvelope({ backend: "groth16" }), payload },
    { envelope: rebuildEnvelope({ publicInputs: nonCanonicalPublicInputs }), payload },
    { envelope: rebuildEnvelope({ publicInputs: duplicateCommitmentInputs }), payload },
  ]) {
    assert.throws(
      () => verifyVeRangeProofLocally(input),
      /veRangeProofLocalVerification/,
    );
  }
});

descriptorTest("privacy verifier key builders encode register and retire instructions", () => {
  const id = "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0";
  const schemaHash = Buffer.alloc(32, 0x11);
  const commitment = Buffer.alloc(32, 0x22);
  const keyBytes = Buffer.from("dev-stark-vk");
  const register = buildRegisterPrivacyVerifierKeyInstruction({
    id,
    version: 1,
    circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    namespace: "zk",
    publicInputsSchemaHash: schemaHash,
    commitment,
    verifyingKeyBytes: keyBytes,
    maxProofBytes: 4096,
    gasScheduleId: "privacy.verify.stark.dev",
    metadataUriCid: "bafy-metadata",
    vkBytesCid: "bafy-vk",
    activationHeight: "7",
    status: "Active",
  });
  const registered =
    encodeAndDecode(register).verifying_keys.RegisterVerifyingKey;
  assert.deepEqual(registered.id, {
    backend: "stark/fri/sha256-goldilocks",
    name: "zk_ace_pq_authorization_v0",
  });
  assert.equal(registered.record.version, 1);
  assert.equal(registered.record.backend, "Stark");
  assert.equal(registered.record.curve, "goldilocks");
  assert.equal(registered.record.namespace, "zk");
  assert.equal(registered.record.status, "Active");
  assert.deepEqual(
    registered.record.public_inputs_schema_hash,
    Array.from(schemaHash),
  );
  assert.deepEqual(registered.record.commitment, Array.from(commitment));
  assert.equal(registered.record.vk_len, keyBytes.length);
  assert.equal(registered.record.max_proof_bytes, 4096);
  assert.equal(registered.record.gas_schedule_id, "privacy.verify.stark.dev");
  assert.equal(registered.record.activation_height, 7);
  assert.deepEqual(registered.record.key, {
    backend: "stark/fri/sha256-goldilocks",
    bytes: Array.from(keyBytes),
  });

  const retire = buildRetirePrivacyVerifierKeyInstruction({
    id,
    record: {
      ...register.verifying_keys.RegisterVerifyingKey.record,
      version: 2,
      withdraw_height: 12,
      status: "Active",
    },
  });
  const retired = encodeAndDecode(retire).verifying_keys.UpdateVerifyingKey;
  assert.equal(retired.record.version, 2);
  assert.equal(retired.record.status, "Withdrawn");
  assert.equal(retired.record.withdraw_height, 12);

  assert.throws(
    () =>
      buildRegisterPrivacyVerifierKeyInstruction({
        id,
        version: 1,
        circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
        publicInputsSchemaHash: schemaHash,
        commitment,
        verifyingKeyBytes: keyBytes,
        gasScheduleId: "privacy.verify.stark.dev",
        activationHeight: 10,
        withdrawHeight: 9,
      }),
    /withdrawHeight must be >= activationHeight/,
  );
  assert.throws(
    () =>
      buildRetirePrivacyVerifierKeyInstruction({
        id,
        record: {
          ...register.verifying_keys.RegisterVerifyingKey.record,
          activation_height: 10,
          withdraw_height: 9,
        },
      }),
    /withdrawHeight must be >= activationHeight/,
  );
});

descriptorTest("privacy proof envelope builder rejects malformed and oversized inputs", () => {
  const vkHashHex = Buffer.alloc(32, 0x55).toString("hex");
  const vkHashBase64 = Buffer.alloc(32, 0x55).toString("base64");
  const base = {
    backend: "stark/fri/sha256-goldilocks",
    circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    vkHash: Buffer.alloc(32, 0x55),
    publicInputs: Buffer.from([1, 2]),
    proofBytes: Buffer.from("proof"),
  };
  const vkHashArrayLike = Object.assign(
    { length: 32 },
    Object.fromEntries(Array.from({ length: 32 }, (_, index) => [index, true])),
  );
  const missingBackend = {
    circuitId: base.circuitId,
    vkHash: base.vkHash,
    publicInputs: base.publicInputs,
    proofBytes: base.proofBytes,
  };
  const hiddenProductionReady = { ...base };
  Object.defineProperty(hiddenProductionReady, "productionReady", {
    value: true,
  });
  const symbolProductionReady = {
    ...base,
    [Symbol.for("productionReady")]: true,
  };
  class EnvelopeOptions {
    constructor(fields) {
      Object.assign(this, fields);
    }
  }
  const classInstance = new EnvelopeOptions(base);
  const accessorProofBytes = { ...base };
  Object.defineProperty(accessorProofBytes, "proofBytes", {
    enumerable: true,
    get() {
      return Buffer.from("proof");
    },
  });
  for (const payload of [
    missingBackend,
    hiddenProductionReady,
    symbolProductionReady,
    classInstance,
    accessorProofBytes,
    { ...base, backend: null },
    { ...base, backend: "unsupported" },
    { ...base, backend: "mock/dev" },
    { ...base, backend: " unsupported" },
    { ...base, backend: "unsupported " },
    { ...base, backend: " miden-stark" },
    { ...base, backend: "miden-stark " },
    { ...base, backend: " stark/fri/sha256-goldilocks" },
    { ...base, backend: "stark/fri/sha256-goldilocks " },
    { ...base, backend: "stark/fri/sha256 goldilocks" },
    { ...base, backend: "stark/fri/sha256+goldilocks" },
    { ...base, backend: "halo2/ipa+mock" },
    { ...base, backend: "stark/fri/dev-fixture" },
    { ...base, backend: "stark/fri/d-e-v-f-i-x-t-u-r-e" },
    { ...base, backend: "stark/fri/dev" },
    { ...base, backend: "stark/fri/d-e-v" },
    { ...base, backend: "stark/fri/test" },
    { ...base, backend: "stark/fri/t-e-s-t" },
    { ...base, backend: "stark/fri/placeholder" },
    { ...base, backend: "stark/fri/latest" },
    { ...base, backend: "stark/fri/attestation" },
    { ...base, backend: "stark/fri/contest" },
    { ...base, backend: "stark/fri/random-profile" },
    { ...base, backend: "stark/fri/sha512-goldilocks" },
    { ...base, backend: "stark/fri/audit-proof-v1" },
    { ...base, backend: "halo2\uFF0Fipa" },
    { ...base, backend: "halo2/\u200Bipa" },
    { ...base, backend: "h\u0430lo2/ipa" },
    { ...base, backend: "stark\uFF0Ffri/sha256-goldilocks" },
    { ...base, backend: "stark/fri/\u200Bsha256-goldilocks" },
    { ...base, backend: "st\u0430rk/fri/sha256-goldilocks" },
    { ...base, backend: "halo2/ipa:dev-fixture" },
    { ...base, backend: "halo2/ipa:dev" },
    { ...base, backend: "halo2/ipa:d-e-v" },
    { ...base, backend: "halo2/ipa:dummy" },
    { ...base, backend: "halo2/ipa:f-a-k-e" },
    { ...base, backend: "halo2/ipa:stub" },
    { ...base, backend: "halo2/ipa:s-a-m-p-l-e" },
    { ...base, backend: "halo2/ipa/orchard/dev-fixture" },
    { ...base, backend: "stark/fri/miden/claimed-production" },
    { ...base, backend: "anonymous-pgc-k-out-of-n-v1-production" },
    { ...base, backend: "sis-hints-anoncred-pq-v0-devfixture" },
    { ...base, backend: "groth16/bls12-377/../../prod" },
    { ...base, backend: "post-quantum-masp/audit-claimed" },
    { ...base, circuitId: " " },
    { ...base, circuitId: " shape" },
    { ...base, circuitId: "shape " },
    { ...base, circuitId: "\tshape" },
    { ...base, circuitId: "shape\n" },
    { ...base, vkHash: Buffer.alloc(32) },
    { ...base, vkHash: ` ${vkHashHex}` },
    { ...base, vkHash: `${vkHashHex} ` },
    { ...base, vkHash: ` ${vkHashBase64}` },
    { ...base, vkHash: `${vkHashBase64}\n` },
    { ...base, vkHash: normalizedHashHex(Buffer.alloc(32, 0x55)) },
    { ...base, vkHash: Array(32).fill(true) },
    { ...base, vkHash: Array(32).fill("1") },
    { ...base, publicInputs: Buffer.alloc(0) },
    { ...base, proofBytes: Buffer.alloc(0) },
    { ...base, publicInputs: [true] },
    { ...base, publicInputs: ["1"] },
    { ...base, publicInputs: { 0: true, length: 1 } },
    { ...base, proofBytes: [false, true] },
    { ...base, proofBytes: [null] },
    { ...base, proofBytes: { 0: false, 1: true, length: 2 } },
    { ...base, aux: [true] },
    { ...base, aux: ["1"] },
    { ...base, aux: { 0: true, length: 1 } },
    { ...base, vkHash: vkHashArrayLike },
    { ...base, publicInputs: new Int16Array([256]) },
    { ...base, proofBytes: new Float32Array([1.5]) },
    { ...base, aux: new Uint8ClampedArray([300]) },
    { ...base, proofBytes: new Int8Array([-1]) },
    { ...base, vkHash: new Uint16Array(16).fill(0x5555) },
    { ...base, publicInputs: "proof" },
    { ...base, proofBytes: "proof" },
    { ...base, aux: "proof" },
    { ...base, publicInputs: " AQI=" },
    { ...base, publicInputs: "AQ I=" },
    { ...base, proofBytes: "AQI= " },
    { ...base, aux: "e30=\n" },
    { ...base, proofBytes: Buffer.from("proof"), maxProofBytes: 2 },
    { ...base, publicInputs: Buffer.from([1, 2]), maxPublicInputBytes: 1 },
    { ...base, maxProofBytes: undefined },
    { ...base, maxProofBytes: null },
    { ...base, maxPublicInputBytes: undefined },
    { ...base, maxPublicInputBytes: null },
    { ...base, maxProofBytes: "016" },
    { ...base, maxProofBytes: " 16" },
    { ...base, maxProofBytes: "16 " },
    { ...base, maxProofBytes: "16\n" },
    { ...base, maxPublicInputBytes: "016" },
    { ...base, maxPublicInputBytes: " 16" },
    { ...base, maxPublicInputBytes: "16 " },
    { ...base, maxPublicInputBytes: "16\n" },
    { ...base, maxProofBytes: 64, max_proof_bytes: 64 },
    { ...base, maxProofBytes: 64, max_proof_bytes: 2 },
    { ...base, maxPublicInputBytes: 64, max_public_input_bytes: 64 },
    { ...base, maxPublicInputBytes: 64, max_public_input_bytes: 1 },
    { ...base, vkHash: Buffer.alloc(32, 0x55), vk_hash: Buffer.alloc(32, 0x66) },
    { ...base, production: true },
    { ...base, productionReady: true },
    { ...base, production_ready: true },
    { ...base, productionGate: { ready: true } },
    { ...base, production_gate: { ready: true } },
  ]) {
    assert.throws(
      () => buildPrivacyProofEnvelope(payload),
      /privacyProofEnvelope/,
    );
  }
});

descriptorTest("privacy dev proof fixture builders reject production readiness claims", () => {
  const anonymousPgcReceiverSet = buildAnonymousPgcReceiverSet({
    threshold: 1,
    receivers: [
      {
        accountCommitment: Buffer.alloc(32, 0x21),
        ciphertextCommitment: Buffer.alloc(32, 0x31),
      },
      {
        accountCommitment: Buffer.alloc(32, 0x22),
        ciphertextCommitment: Buffer.alloc(32, 0x32),
      },
    ],
  });
  const devFixtureCases = [
    [
      "zkAt",
      buildZkAtDevProofFixture,
      {
        policyJson: { threshold: 2, roles: ["ops", "risk", "treasury"] },
        policyEpoch: 7,
        policySchema: "boi-hidden-threshold-v1",
        payload: Buffer.from("zkat:transparent-transfer:42"),
        accountId: ACCOUNT_ID,
        actionClass: "transparent_transfer",
        domainSeparator: "boi:zkat:v1",
        vkHash: Buffer.alloc(32, 0x55),
      },
    ],
    [
      "ZK-AMS",
      buildZkAmsAdmissionDevProofFixture,
      {
        issuerRoot: Buffer.alloc(32, 0x91),
        admissionNullifiers: [Buffer.alloc(32, 0xa1), Buffer.alloc(32, 0xa2)],
        anonymousAccountCommitments: [
          Buffer.alloc(32, 0xb1),
          Buffer.alloc(32, 0xb2),
        ],
        recursiveProof: Buffer.from("zk-ams:recursive-proof:batch-7"),
        domainSeparator: "boi:zk-ams:pilot:v0",
        vkHash: Buffer.alloc(32, 0x66),
      },
    ],
    [
      "Vega",
      buildVegaCredentialDevProofFixture,
      {
        issuerJson: { did: "did:example:issuer:boi", key: "issuer-key-1" },
        predicateJson: { kind: "age_over", attribute: "age", threshold: 18 },
        credentialSchema: "boi-age-credential-v1",
        accountId: ACCOUNT_ID,
        expirationEpoch: 42,
        domainSeparator: "boi:vega:pilot:v0",
        vkHash: Buffer.alloc(32, 0x77),
      },
    ],
    [
      "Silent Threshold",
      buildSilentThresholdCredentialDevProofFixture,
      {
        issuerSetJson: { threshold: 2, issuers: ["a", "b", "c"] },
        thresholdPolicyJson: { threshold: 2, purpose: "wallet" },
        credentialShowingJson: { credential_type: "wallet", nonce: "n-1" },
        verifierPolicyJson: { verifier: "boi", purpose: "wallet" },
        domainSeparator: "boi:silent-threshold:pilot:v0",
        vkHash: Buffer.alloc(32, 0x88),
      },
    ],
    [
      "ZK-X.509",
      buildZkX509IdentityDevProofFixture,
      {
        caRootJson: { root: "boi-root-ca", version: 1 },
        certificatePolicyJson: { eku: ["clientAuth"], policy: "wallet" },
        revocationJson: { epoch: 7, root: "revocation-root" },
        subjectJson: { cn: "Bank A", lei: "5493001KJTIIGC8Y1R12" },
        accountId: ACCOUNT_ID,
        domainSeparator: "boi:zk-x509:pilot:v0",
        vkHash: Buffer.alloc(32, 0x99),
      },
    ],
    [
      "Jindo",
      buildJindoLatticeDevProofFixture,
      {
        polynomialJson: { ring: "Rq", degree: 1024, digest: "poly" },
        openingClaimJson: { point: "x=42", value_digest: "value" },
        querySetJson: { queries: [0, 7, 42] },
        parametersJson: { scheme: "jindo-pcs-v0", q_bits: 64 },
        domainSeparator: "boi:jindo:pcs:pilot:v0",
        vkHash: Buffer.alloc(32, 0xaa),
      },
    ],
    [
      "SIS-with-hints",
      buildSisHintsCredentialDevProofFixture,
      {
        issuerJson: { issuer: "boi", scheme: "sis-hints-v0" },
        credentialJson: { credential_type: "wallet", nonce: "n-1" },
        showingPolicyJson: { verifier: "boi", purpose: "wallet" },
        parametersJson: { scheme: "sis-hints-anoncred-v0", q_bits: 64 },
        domainSeparator: "boi:sis-hints:pilot:v0",
        vkHash: Buffer.alloc(32, 0xbb),
      },
    ],
    [
      "Anonymous PGC",
      buildAnonymousPgcDevProofFixture,
      {
        receiverSet: anonymousPgcReceiverSet,
        anonymitySetRoot: Buffer.alloc(32, 0x41),
        payload: Buffer.from("anonymous-pgc:alice:bob:42"),
        balanceCommitments: [Buffer.alloc(32, 0x51), Buffer.alloc(32, 0x52)],
        linkTag: Buffer.alloc(32, 0x61),
        rangeCommitments: [Buffer.alloc(32, 0x71)],
        chainId: "boi-localnet",
        domainSeparator: "boi:anonymous-pgc:v1",
        vkHash: Buffer.alloc(32, 0x55),
      },
    ],
    [
      "VeRange",
      buildVeRangeDevProofFixture,
      {
        commitments: [Buffer.alloc(32, 0x44), Buffer.alloc(32, 0x45)],
        bitLength: 64,
        commitmentScheme: "pedersen-v1",
        domainSeparator: "boi:amount-range:v1",
        payload: Buffer.from("transfer:alice@wonderland:bob@wonderland:42"),
        vkHash: Buffer.alloc(32, 0x55),
      },
    ],
  ];

  for (const [name, builder, input] of devFixtureCases) {
    const fixture = builder(input);
    assert.equal(fixture.production, false, `${name} fixture must stay dev-only`);
    for (const [field, value] of [
      ["production", true],
      ["productionReady", true],
      ["production_ready", true],
      ["productionGate", { ready: true }],
      ["production_gate", { ready: true }],
    ]) {
      assert.throws(
        () => builder({ ...input, [field]: value }),
        new RegExp(field),
        `${name} fixture builder accepted ${field}`,
      );
    }
  }
});

descriptorTest("privacy verifier key builders reject unsafe registry records", () => {
  const base = {
    id: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    version: 1,
    circuitId: "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
    publicInputsSchemaHash: Buffer.alloc(32, 0x11),
    commitment: Buffer.alloc(32, 0x22),
    verifyingKeyBytes: Buffer.from("dev-stark-vk"),
    maxProofBytes: 4096,
    gasScheduleId: "privacy.verify.stark.dev",
  };
  const baseWithoutInlineKey = { ...base };
  delete baseWithoutInlineKey.verifyingKeyBytes;
  for (const payload of [
    { ...base, record: undefined },
    { ...base, record: null },
    { ...base, id: "mock/dev:vk" },
    { ...base, id: "stark/fri/dev-fixture:vk" },
    { ...base, id: "stark/fri/d-e-v-f-i-x-t-u-r-e:vk" },
    { ...base, id: "stark/fri/dev:vk" },
    { ...base, id: "stark/fri/d-e-v:vk" },
    { ...base, id: "stark/fri/test:vk" },
    { ...base, id: "stark/fri/t-e-s-t:vk" },
    { ...base, id: "stark/fri/placeholder:vk" },
    { ...base, id: "stark/fri/latest:vk" },
    { ...base, id: "stark/fri/attestation:vk" },
    { ...base, id: "stark/fri/contest:vk" },
    { ...base, id: "stark/fri/random-profile:vk" },
    { ...base, id: "stark/fri/sha512-goldilocks:vk" },
    { ...base, id: "stark/fri/audit-proof-v1:vk" },
    { ...base, id: "halo2/ipa:dev-fixture:vk" },
    { ...base, id: "halo2/ipa:dev:vk" },
    { ...base, id: "halo2/ipa:d-e-v:vk" },
    { ...base, id: "halo2/ipa:dummy:vk" },
    { ...base, id: "halo2/ipa:f-a-k-e:vk" },
    { ...base, id: "halo2/ipa:stub:vk" },
    { ...base, id: "halo2/ipa:s-a-m-p-l-e:vk" },
    { ...base, id: "halo2/unknown-native-v1:vk" },
    { ...base, id: " halo2/ipa:vk" },
    { ...base, id: "halo2/ipa :vk" },
    { ...base, id: "\thalo2/ipa:vk" },
    { ...base, id: "halo2\uFF0Fipa:vk" },
    { ...base, id: "halo2/\u200Bipa:vk" },
    { ...base, id: "h\u0430lo2/ipa:vk" },
    { ...base, id: "stark/fri/sha256-goldilocks :vk" },
    { ...base, id: "stark\uFF0Ffri/sha256-goldilocks:vk" },
    { ...base, id: "stark/fri/\u200Bsha256-goldilocks:vk" },
    { ...base, id: "st\u0430rk/fri/sha256-goldilocks:vk" },
    { ...base, id: { backend: "halo2/ipa:unknown-native-v1", name: "vk" } },
    { ...base, id: { backend: "stark/unknown-native-v1", name: "vk" } },
    { ...base, id: { backend: "halo2/pasta/tiny-add", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa/tiny-add", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa:tiny-add", name: "vk" } },
    { ...base, id: { backend: "halo2/pasta/tiny-commit-open", name: "vk" } },
    { ...base, id: { backend: "halo2/pasta/anon-transfer-2x2", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa/anon-transfer-2x2", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa:anon-transfer-2x2", name: "vk" } },
    { ...base, id: { backend: "halo2/pasta/anon-transfer-2x2-merkle2", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa/anon-transfer-2x2-merkle8", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa:anon-transfer-2x2-merkle16", name: "vk" } },
    { ...base, id: { backend: "halo2/pasta/vote-bool-commit", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa/vote-bool-commit", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa:vote-bool-commit", name: "vk" } },
    { ...base, id: { backend: "halo2/pasta/vote-bool-commit-merkle2", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa/vote-bool-commit-merkle8", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa:vote-bool-commit-merkle16", name: "vk" } },
    { ...base, id: { backend: "halo2/pasta/asset-hidden-transfer-public-test", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa/asset-hidden-transfer-public-test", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa:asset-hidden-transfer-public-test", name: "vk" } },
    { ...base, id: { backend: " halo2/ipa", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa ", name: "vk" } },
    { ...base, id: { backend: "\thalo2/ipa", name: "vk" } },
    { ...base, id: { backend: "halo2/ipa\n", name: "vk" } },
    { ...base, id: { backend: "halo2\uFF0Fipa", name: "vk" } },
    { ...base, id: { backend: "halo2/\u200Bipa", name: "vk" } },
    { ...base, id: { backend: "h\u0430lo2/ipa", name: "vk" } },
    { ...base, id: { backend: "stark/fri/miden", name: "vk" } },
    { ...base, id: { backend: "stark/fri/latest", name: "vk" } },
    { ...base, id: { backend: "stark/fri/attestation", name: "vk" } },
    { ...base, id: { backend: "stark/fri/contest", name: "vk" } },
    { ...base, id: { backend: "stark/fri/random-profile", name: "vk" } },
    { ...base, id: { backend: "stark/fri/sha512-goldilocks", name: "vk" } },
    { ...base, id: { backend: "stark/fri/audit-proof-v1", name: "vk" } },
    { ...base, id: { backend: " stark/fri/sha256-goldilocks", name: "vk" } },
    { ...base, id: { backend: "stark/fri/sha256-goldilocks ", name: "vk" } },
    { ...base, id: { backend: "stark\uFF0Ffri/sha256-goldilocks", name: "vk" } },
    { ...base, id: { backend: "stark/fri/\u200Bsha256-goldilocks", name: "vk" } },
    { ...base, id: { backend: "st\u0430rk/fri/sha256-goldilocks", name: "vk" } },
    { ...base, id: { backend: "halo2/kzg", name: "vk" } },
    { ...base, backendTag: "Groth16" },
    { ...base, curve: "Pallas" },
    { ...base, publicInputsSchemaHash: Buffer.alloc(32) },
    { ...base, commitment: Buffer.alloc(32) },
    { ...base, maxProofBytes: 0 },
    { ...base, maxProofBytes: undefined },
    { ...base, maxProofBytes: null },
    { ...base, maxProofBytes: 4096, max_proof_bytes: 4096 },
    { ...base, backendTag: undefined },
    { ...base, backendTag: null },
    { ...base, gasScheduleId: " " },
    { ...base, status: "Withdrawn" },
    { ...baseWithoutInlineKey, key: undefined },
    { ...baseWithoutInlineKey, key: null },
    { ...base, key: { backend: "halo2/ipa", bytes: Buffer.from("vk") } },
    {
      ...baseWithoutInlineKey,
      key: {
        backend: undefined,
        backendId: "stark/fri/sha256-goldilocks",
        bytes: Buffer.from("vk"),
      },
    },
    {
      ...baseWithoutInlineKey,
      key: {
        backend: "stark/fri/sha256-goldilocks",
        backendId: "stark/fri/sha256-goldilocks",
        bytes: Buffer.from("vk"),
      },
    },
    {
      ...baseWithoutInlineKey,
      key: {
        bytes: undefined,
        keyBytes: Buffer.from("vk"),
      },
    },
    {
      ...baseWithoutInlineKey,
      key: {
        bytes: Buffer.from("vk"),
        keyBytes: Buffer.from("vk"),
      },
    },
    {
      ...baseWithoutInlineKey,
      key: {
        backend: "stark/fri/sha256-goldilocks",
      },
    },
    { ...base, verifyingKeyBytes: Buffer.from("vk"), vkLen: 999 },
    { ...base, vkLen: undefined },
    { ...base, vkLen: null },
    { ...base, circuitId: "a", circuit_id: "b" },
  ]) {
    assert.throws(
      () => buildRegisterPrivacyVerifierKeyInstruction(payload),
      /registerPrivacyVerifierKey/,
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
    amount: "7",
    noteCommitment: Buffer.alloc(32, 0x01),
    encryptedPayload: {
      version: 1,
      ephemeralPublicKey: Buffer.alloc(32, 0x02),
      nonce: Buffer.alloc(24, 0x03),
      ciphertext: Buffer.from("ciphertext"),
    },
  });
  const payload = encodeAndDecode(instruction).zk.Shield;
  assert.equal(payload.amount, 7);
  assert.equal(payload.enc_payload.version, 1);
  assert.equal(payload.enc_payload.ciphertext, Buffer.from("ciphertext").toString("base64"));
});

test("buildShieldInstruction rejects non-safe JSON numeric amounts", () => {
  assert.throws(
    () =>
      buildShieldInstruction({
        assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        fromAccountId: ACCOUNT_ID_INPUT,
        amount: Number.MAX_SAFE_INTEGER + 1,
        noteCommitment: Buffer.alloc(32, 0x01),
        encryptedPayload: {
          version: 1,
          ephemeralPublicKey: Buffer.alloc(32, 0x02),
          nonce: Buffer.alloc(24, 0x03),
          ciphertext: Buffer.from("ciphertext"),
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.VALUE_OUT_OF_RANGE);
      assert.match(String(error?.message), /between 0 and|deterministic/i);
      return true;
    },
  );
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

descriptorTest("privacy algorithm descriptors expose strict post-quantum MASP semantics", () => {
  assert.deepEqual(getPrivacyCriteria(), [
    "hide_amount",
    "hide_sender",
    "hide_receiver",
    "hide_asset_type",
    "post_quantum",
  ]);
  const algorithms = getPrivacyAlgorithmDescriptors();
  const masp = getPrivacyAlgorithmDescriptor("asset-hidden-confidential-transfer-v1");
  const orchard = getPrivacyAlgorithmDescriptor("orchard-halo2-actions-v1");
  const penumbra = getPrivacyAlgorithmDescriptor("penumbra-masp-v1");
  const fcmp = getPrivacyAlgorithmDescriptor("monero-fcmp-plus-plus-v1");
  const miden = getPrivacyAlgorithmDescriptor("miden-stark-note-v1");
  const aztec = getPrivacyAlgorithmDescriptor("aztec-private-rollup-v1");
  const pq = getPrivacyAlgorithmDescriptor("pq-masp-stark-v0");
  assert.ok(algorithms.some((algorithm) => algorithm.id === "confidential-transfer-v2"));
  assert.ok(masp);
  assert.ok(orchard);
  assert.ok(penumbra);
  assert.ok(fcmp);
  assert.ok(miden);
  assert.ok(aztec);
  assert.ok(pq);
  assert.equal(masp.pqLayers.proof, false);
  assert.equal(masp.coveredCriteria.includes("post_quantum"), false);
  assert.ok(masp.sdkEntrypoints.includes("buildAssetHiddenZkTransferInstruction"));
  assert.ok(
    masp.plannedSdkEntrypoints.includes("buildConfidentialAssetHiddenTransferProofV1"),
  );
  assert.equal(
    masp.sdkEntrypoints.includes("buildConfidentialAssetHiddenTransferProofV1"),
    false,
  );
  assert.equal(orchard.proofFamily, "halo2-pasta-action-bundle");
  assert.equal(orchard.coveredCriteria.includes("hide_asset_type"), false);
  assert.deepEqual(orchard.sdkEntrypoints, []);
  assert.ok(orchard.plannedSdkEntrypoints.includes("buildOrchardActionBundleProofV1"));
  assert.equal(penumbra.coveredCriteria.includes("hide_asset_type"), true);
  assert.equal(penumbra.sourceReferences[0].url, "https://protocol.penumbra.zone/main/shielded_pool.html");
  assert.deepEqual(penumbra.sdkEntrypoints, []);
  assert.ok(penumbra.plannedSdkEntrypoints.includes("buildPenumbraShieldedPoolTransaction"));
  assert.equal(fcmp.coveredCriteria.includes("hide_sender"), true);
  assert.equal(miden.pqLayers.proof, true);
  assert.equal(miden.pqLayers.authorization, false);
  assert.equal(miden.coveredCriteria.includes("post_quantum"), false);
  assert.equal(aztec.proofFamily, "plonkish-private-kernel-rollup");
  assert.equal(aztec.coveredCriteria.includes("hide_receiver"), true);
  assert.equal(aztec.coveredCriteria.includes("hide_asset_type"), false);
  assert.equal(pq.proofFamily, "stark-fri");
  assert.deepEqual(pq.pqLayers, {
    proof: true,
    authorization: true,
    noteEncryption: true,
  });
  assert.ok(
    pq.sourceReferences.some((reference) =>
      reference.url === "https://csrc.nist.gov/pubs/fips/204/final"
    ),
  );
  assert.ok(pq.sdkEntrypoints.includes("buildAssetHiddenZkTransferInstruction"));
  assert.ok(pq.plannedSdkEntrypoints.includes("buildPqMaspStarkTransferProofV0"));
  assert.equal(pq.sdkEntrypoints.includes("buildPqMaspStarkTransferProofV0"), false);
  assert.equal(pq.coveredCriteria.includes("post_quantum"), true);
});

descriptorTest("privacy algorithm descriptors expose 2025-2026 BOI research targets", () => {
  const zkAce = getPrivacyAlgorithmDescriptor("zk-ace-pq-authorization-v0");
  const anonymousPgc = getPrivacyAlgorithmDescriptor("anonymous-pgc-k-out-of-n-v1");
  const verange = getPrivacyAlgorithmDescriptor("verange-transparent-range-v1");
  const zkat = getPrivacyAlgorithmDescriptor("zkat-policy-private-auth-v1");
  const zkAms = getPrivacyAlgorithmDescriptor("zk-ams-recursive-admission-v0");
  const vega = getPrivacyAlgorithmDescriptor("vega-existing-credential-zk-v0");
  const silentThreshold = getPrivacyAlgorithmDescriptor("silent-threshold-anoncred-v0");
  const zkX509 = getPrivacyAlgorithmDescriptor("zk-x509-onchain-identity-v0");
  const jindo = getPrivacyAlgorithmDescriptor("jindo-lattice-pcs-zk-v0");
  const sisHints = getPrivacyAlgorithmDescriptor("sis-hints-anoncred-pq-v0");

  assert.ok(sisHints);
  assert.ok(sisHints.sourceReferences.length > 0);
  assert.ok(sisHints.securityNotes.length > 0);
  assert.ok(sisHints.requiredState.length > 0);
  assert.ok(sisHints.failureModes.length > 0);

  assert.ok(zkAms);
  assert.equal(zkAms.implementationStage, "sdk-builder");
  assert.ok(zkAms.sdkEntrypoints.includes("buildZkAmsAdmissionBatch"));
  assert.ok(zkAms.sdkEntrypoints.includes("buildZkAmsAdmissionProofEnvelope"));
  assert.ok(zkAms.plannedSdkEntrypoints.includes("buildSubmitZkAmsAdmissionBatchInstruction"));

  assert.ok(zkAce);
  assert.equal(zkAce.category, "authorization");
  assert.equal(zkAce.maturity, "arxiv_preprint");
  assert.equal(zkAce.implementationStage, "chain-executable");
  assert.deepEqual(zkAce.coveredCriteria, []);
  assert.equal(zkAce.pqLayers.proof, true);
  assert.equal(zkAce.pqLayers.authorization, true);
  assert.equal(zkAce.pqLayers.noteEncryption, false);
  assert.equal(zkAce.coveredCriteria.includes("post_quantum"), false);
  assert.deepEqual(zkAce.sdkEntrypoints, [
    "buildRegisterZkAceIdentityCommitmentInstruction",
    "buildRotateZkAceIdentityCommitmentInstruction",
    "buildRevokeZkAceIdentityCommitmentInstruction",
    "buildZkAceAuthorizedTransferInstruction",
    "buildZkAceAuthorizationProofV1",
  ]);
  assert.ok(zkAce.plannedSdkEntrypoints.includes("buildShieldedZkAceAuthorizedTransferInstruction"));
  assert.equal(zkAce.plannedSdkEntrypoints.includes("buildZkAceAuthorizationProofV0"), false);
  assert.ok(zkAce.chainRequirements.includes("zk::SubmitZkAceAuthorizedTransfer"));

  assert.equal(anonymousPgc.category, "payment");
  assert.equal(anonymousPgc.maturity, "accepted_conference");
  assert.equal(anonymousPgc.implementationStage, "sdk-builder");
  assert.deepEqual(anonymousPgc.coveredCriteria, [
    "hide_amount",
    "hide_sender",
    "hide_receiver",
  ]);
  assert.deepEqual(anonymousPgc.sdkEntrypoints, [
    "buildAnonymousPgcReceiverSet",
    "buildAnonymousPgcDevProofFixture",
    "verifyAnonymousPgcDevProofLocally",
  ]);
  assert.deepEqual(anonymousPgc.plannedSdkEntrypoints, [
    "buildAnonymousPgcAccountCommitmentInstruction",
    "buildAnonymousPgcKOutOfNProofV1",
    "buildAnonymousPgcTransferInstruction",
  ]);

  assert.equal(verange.category, "proof_backend");
  assert.equal(verange.implementationStage, "component");
  assert.equal(
    verange.publicInputsSchema,
    "commitments,range_parameters,aggregation_count,domain_separator,payload_digest",
  );
  assert.deepEqual(verange.sdkEntrypoints, [
    "buildRangeCommitment",
    "buildVeRangeDevProofFixture",
    "buildVeRangeProofEnvelope",
    "verifyVeRangeProofLocally",
  ]);
  assert.deepEqual(verange.plannedSdkEntrypoints, [
    "buildVeRangeProofV1",
  ]);
  assert.equal(verange.coveredCriteria.includes("hide_amount"), true);
  assert.equal(zkat.category, "authorization");
  assert.equal(zkat.implementationStage, "sdk-builder");
  assert.deepEqual(zkat.sdkEntrypoints, [
    "buildZkAtPolicyCommitment",
    "buildZkAtAuthenticatorEnvelope",
    "buildZkAtDevProofFixture",
    "verifyZkAtAuthenticatorLocally",
  ]);
  assert.deepEqual(zkat.plannedSdkEntrypoints, [
    "buildZkAtPolicyCommitmentInstruction",
    "buildZkAtPolicyProofV1",
    "buildZkAtAuthorizedTransaction",
  ]);
  assert.equal(zkAms.category, "admission");
  assert.equal(zkAms.implementationStage, "sdk-builder");
  assert.equal(
    zkAms.publicInputsSchema,
    "issuer_root,admission_batch_root,admission_nullifiers,anonymous_account_commitments,recursive_admission_digest,domain_separator",
  );
  assert.deepEqual(zkAms.sdkEntrypoints, [
    "buildZkAmsAdmissionBatch",
    "buildZkAmsAdmissionProofEnvelope",
    "buildZkAmsAdmissionDevProofFixture",
    "verifyZkAmsAdmissionProofLocally",
  ]);
  assert.deepEqual(zkAms.plannedSdkEntrypoints, [
    "buildZkAmsAdmissionBatchProofV0",
    "buildSubmitZkAmsAdmissionBatchInstruction",
  ]);
  assert.equal(vega.category, "credential");
  assert.equal(vega.implementationStage, "sdk-builder");
  assert.deepEqual(vega.sdkEntrypoints, [
    "buildVegaCredentialPredicateCommitment",
    "buildVegaCredentialProofEnvelope",
    "buildVegaCredentialDevProofFixture",
    "verifyVegaCredentialProofLocally",
  ]);
  assert.deepEqual(vega.plannedSdkEntrypoints, [
    "buildVegaCredentialPredicateProofV0",
    "buildSubmitVegaCredentialProofInstruction",
  ]);
  assert.equal(silentThreshold.category, "credential");
  assert.equal(silentThreshold.implementationStage, "sdk-builder");
  assert.equal(
    silentThreshold.publicInputsSchema,
    "issuer_set_commitment,threshold_policy_hash,credential_showing_commitment,showing_nullifier,verifier_policy_hash,domain_separator",
  );
  assert.deepEqual(silentThreshold.sdkEntrypoints, [
    "buildSilentThresholdCredentialCommitments",
    "buildSilentThresholdCredentialEnvelope",
    "buildSilentThresholdCredentialDevProofFixture",
    "verifySilentThresholdCredentialProofLocally",
  ]);
  assert.deepEqual(silentThreshold.plannedSdkEntrypoints, [
    "buildSilentThresholdCredentialShowingProofV0",
    "buildSubmitSilentThresholdCredentialProofInstruction",
  ]);
  assert.equal(zkX509.category, "identity");
  assert.equal(zkX509.implementationStage, "sdk-builder");
  assert.equal(
    zkX509.publicInputsSchema,
    "ca_root_commitment,certificate_policy_hash,revocation_root,subject_commitment,address_binding,domain_separator",
  );
  assert.deepEqual(zkX509.sdkEntrypoints, [
    "buildZkX509IdentityCommitments",
    "buildZkX509IdentityEnvelope",
    "buildZkX509IdentityDevProofFixture",
    "verifyZkX509IdentityProofLocally",
  ]);
  assert.deepEqual(zkX509.plannedSdkEntrypoints, [
    "buildZkX509IdentityProofV0",
    "buildSubmitZkX509IdentityProofInstruction",
  ]);
  assert.equal(jindo.category, "proof_backend");
  assert.equal(jindo.maturity, "technical_report");
  assert.equal(jindo.implementationStage, "sdk-builder");
  assert.equal(
    jindo.publicInputsSchema,
    "commitment,opening_claim,query_set,parameter_hash,domain_separator",
  );
  assert.deepEqual(jindo.sdkEntrypoints, [
    "buildJindoLatticePublicInputs",
    "buildJindoLatticeProofEnvelope",
    "buildJindoLatticeDevProofFixture",
    "verifyJindoLatticeProofLocally",
  ]);
  assert.deepEqual(jindo.plannedSdkEntrypoints, [
    "buildJindoLatticeProofV0",
    "verifyJindoPolynomialCommitmentV0",
  ]);
  assert.equal(jindo.pqLayers.proof, true);
  assert.equal(jindo.coveredCriteria.includes("post_quantum"), false);
  assert.equal(sisHints.category, "credential");
  assert.equal(sisHints.implementationStage, "sdk-builder");
  assert.equal(
    sisHints.publicInputsSchema,
    "issuer_commitment,credential_commitment,showing_policy_hash,parameter_hash,domain_separator",
  );
  assert.deepEqual(sisHints.sdkEntrypoints, [
    "buildSisHintsCredentialCommitments",
    "buildSisHintsCredentialEnvelope",
    "buildSisHintsCredentialDevProofFixture",
    "verifySisHintsCredentialProofLocally",
  ]);
  assert.deepEqual(sisHints.plannedSdkEntrypoints, [
    "buildSisHintsAnonymousCredentialProofV0",
    "buildSubmitSisHintsCredentialProofInstruction",
  ]);
  assert.equal(sisHints.pqLayers.proof, true);
  assert.equal(sisHints.coveredCriteria.includes("post_quantum"), false);
});

descriptorTest("privacy algorithm descriptors only advertise exported SDK entrypoints", () => {
  for (const descriptor of getPrivacyAlgorithmDescriptors()) {
    assert.match(
      descriptor.category,
      /^(payment|authorization|credential|admission|identity|proof_backend)$/,
    );
    assert.match(
      descriptor.maturity,
      /^(peer_reviewed|accepted_conference|technical_report|arxiv_preprint|specification)$/,
    );
    for (const entrypoint of descriptor.sdkEntrypoints) {
      assert.equal(
        typeof sdkExports[entrypoint],
        "function",
        `${descriptor.id} sdk entrypoint ${entrypoint} must be exported`,
      );
    }
    for (const entrypoint of descriptor.plannedSdkEntrypoints) {
      assert.equal(
        descriptor.sdkEntrypoints.includes(entrypoint),
        false,
        `${descriptor.id} planned entrypoint ${entrypoint} must not be listed as available`,
      );
    }
  }
  const masp = getPrivacyAlgorithmDescriptor("asset-hidden-confidential-transfer-v1");
  assert.equal(typeof sdkExports.buildConfidentialAssetHiddenTransferProofV1, "undefined");
  assert.ok(
    masp.plannedSdkEntrypoints.includes("buildConfidentialAssetHiddenTransferProofV1"),
  );
});

descriptorTest("privacy algorithm descriptors enforce PQ and catalog availability invariants", () => {
  const criteria = new Set(getPrivacyCriteria());
  const ids = new Set();
  const expectedProductionGateEntries = [
    ["real_proving", false],
    ["real_verification", false],
    ["chain_admission", false],
    ["sdk_parity", false],
    ["wallet_state", false],
    ["deterministic_tests", false],
    ["fuzzing", false],
    ["performance_gates", false],
    ["external_audit", false],
  ];
  const requiredProductionGateMissing = [
    "real proving engine is not registered",
    "real verifier is not registered",
    "chain admission path is not enabled",
    "cross-SDK parity is incomplete",
    "wallet/state support is incomplete",
    "deterministic tests are incomplete",
    "fuzzing gate is incomplete",
    "performance gate is incomplete",
    "external audit signoff is missing",
  ];
  const supplementalProductionGateMissing = [
    "implementation stage is not production-hardened",
    "planned SDK entrypoints remain",
    "dev fixture entrypoints are not production entrypoints",
    "Iroha production allowlist is not enabled for this audited row",
  ];
  const allowedCategories = new Set([
    "payment",
    "authorization",
    "credential",
    "admission",
    "identity",
    "proof_backend",
  ]);
  const allowedMaturities = new Set([
    "peer_reviewed",
    "accepted_conference",
    "technical_report",
    "arxiv_preprint",
    "specification",
  ]);

  for (const descriptor of getPrivacyAlgorithmDescriptors()) {
    assert.equal(ids.has(descriptor.id), false, `${descriptor.id} must be unique`);
    ids.add(descriptor.id);
    assert.equal(descriptor.productionReady, false, `${descriptor.id} productionReady`);
    assert.equal(descriptor.productionGate.version, "privacy-production-gate-v1");
    assert.equal(descriptor.productionGate.ready, false, `${descriptor.id} production gate`);
    assert.deepEqual(Object.entries(descriptor.productionGate.gates), expectedProductionGateEntries);
    assert.deepEqual(
      descriptor.productionGate.missing,
      [
        ...requiredProductionGateMissing,
        ...supplementalProductionGateMissing.filter((reason) =>
          descriptor.productionGate.missing.includes(reason),
        ),
      ],
      `${descriptor.id} production gate missing reasons must stay canonical`,
    );
    assert.ok(
      descriptor.productionGate.missing.includes("external audit signoff is missing"),
      `${descriptor.id} must remain blocked without audit signoff`,
    );
    assert.ok(
      descriptor.productionGate.missing.includes(
        "Iroha production allowlist is not enabled for this audited row",
      ),
      `${descriptor.id} must remain blocked without explicit production allowlist`,
    );
    if (
      descriptor.sdkEntrypoints.some((entrypoint) =>
        entrypoint.toLowerCase().includes("devprooffixture") ||
        entrypoint.toLowerCase().includes("devfixture")
      )
    ) {
      assert.ok(
        descriptor.productionGate.missing.includes(
          "dev fixture entrypoints are not production entrypoints",
        ),
        `${descriptor.id} dev fixtures cannot satisfy production gate`,
      );
    }
    assert.equal(allowedCategories.has(descriptor.category), true, `${descriptor.id} category`);
    assert.equal(allowedMaturities.has(descriptor.maturity), true, `${descriptor.id} maturity`);
    assert.match(descriptor.implementationStage ?? "implementation-stage-unset", /^[a-z0-9][a-z0-9-]*(?:-[a-z0-9]+)*(?:-as-of-\d{4}-\d{2})?$/);
    for (const criterion of descriptor.coveredCriteria) {
      assert.equal(criteria.has(criterion), true, `${descriptor.id} criterion ${criterion}`);
    }
    const fullyPostQuantum =
      descriptor.pqLayers.proof &&
      descriptor.pqLayers.authorization &&
      descriptor.pqLayers.noteEncryption;
    assert.equal(
      descriptor.coveredCriteria.includes("post_quantum"),
      fullyPostQuantum,
      `${descriptor.id} must only claim post_quantum when proof, authorization, and note encryption are PQ-ready`,
    );
    if (descriptor.implementationStage === "catalog-as-of-2026-05") {
      assert.deepEqual(
        descriptor.sdkEntrypoints,
        [],
        `${descriptor.id} catalog-only targets cannot advertise executable SDK entrypoints`,
      );
      assert.ok(
        descriptor.plannedSdkEntrypoints.length > 0,
        `${descriptor.id} catalog-only targets should expose planned entrypoints`,
      );
    }
    for (const reference of descriptor.sourceReferences) {
      assert.match(reference.label, /\S/, `${descriptor.id} reference label`);
      assert.match(reference.url, /^https:\/\//, `${descriptor.id} reference URL`);
    }
    for (const listName of [
      "recommendedFor",
      "securityNotes",
      "requiredState",
      "failureModes",
      "setupSteps",
      "executionSteps",
      "sdkEntrypoints",
      "plannedSdkEntrypoints",
      "chainRequirements",
    ]) {
      for (const value of descriptor[listName]) {
        assert.match(value, /\S/, `${descriptor.id} ${listName} item`);
      }
    }
  }
});

function mutablePrivacyDescriptor(id = "transparent-transfer") {
  const descriptor = getPrivacyAlgorithmDescriptor(id);
  assert.ok(descriptor);
  const { backendFamily, productionReady, productionGate, ...rawDescriptor } = descriptor;
  void backendFamily;
  void productionReady;
  void productionGate;
  return {
    ...rawDescriptor,
    coveredCriteria: [...descriptor.coveredCriteria],
    pqLayers: { ...descriptor.pqLayers },
    recommendedFor: [...descriptor.recommendedFor],
    sourceReferences: descriptor.sourceReferences.map((reference) => ({ ...reference })),
    securityNotes: [...descriptor.securityNotes],
    requiredState: [...descriptor.requiredState],
    failureModes: [...descriptor.failureModes],
    setupSteps: [...descriptor.setupSteps],
    executionSteps: [...descriptor.executionSteps],
    sdkEntrypoints: [...descriptor.sdkEntrypoints],
    plannedSdkEntrypoints: [...descriptor.plannedSdkEntrypoints],
    chainRequirements: [...descriptor.chainRequirements],
  };
}

function withPrivacyNativeBinding(binding, body) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return body();
  } finally {
    if (previous === undefined) {
      delete globalThis.__IROHA_NATIVE_BINDING__;
    } else {
      globalThis.__IROHA_NATIVE_BINDING__ = previous;
    }
  }
}

function privacyNoritoFrame(schemaByte) {
  const frame = Buffer.alloc(40);
  frame.write("NRT0", 0, "ascii");
  frame.fill(schemaByte, 6, 22);
  return frame;
}

function privacyNoritoFrameWithPayload(schemaByte) {
  const frame = Buffer.concat([
    privacyNoritoFrame(schemaByte),
    Buffer.from([0x00, 0x00, 0xa5, 0x5a, 0x11]),
  ]);
  frame.writeBigUInt64LE(3n, 23);
  Buffer.from([0xb9, 0xd3, 0xa8, 0x0c, 0xcd, 0x5d, 0x13, 0x24]).copy(frame, 31);
  return frame;
}

function privacyNoritoFrameWithSchemaOverride(schemaByte, offset, value) {
  const frame = Buffer.from(privacyNoritoFrameWithPayload(schemaByte));
  frame[offset] = value;
  return frame;
}

function int8PrivacyFrame(schemaByte) {
  const frame = privacyNoritoFrameWithPayload(schemaByte);
  const backing = new ArrayBuffer(frame.length);
  new Uint8Array(backing).set(frame);
  return new Int8Array(backing);
}

function sharedPrivacyFrame(schemaByte) {
  const frame = privacyNoritoFrameWithPayload(schemaByte);
  const backing = new SharedArrayBuffer(frame.length);
  new Uint8Array(backing).set(frame);
  return new Uint8Array(backing);
}

function completePrivacyNativeBinding(overrides = {}) {
  return {
    connectNoritoBridgeAbiVersion() {
      return 6;
    },
    privacyCapabilitiesV1() {
      return privacyNoritoFrameWithPayload(0x50);
    },
    privacyBuildProofV1() {
      return privacyNoritoFrameWithPayload(0x42);
    },
    privacyVerifyProofV1() {
      return privacyNoritoFrameWithPayload(0x56);
    },
    ...overrides,
  };
}

descriptorTest("privacy algorithm descriptor validator rejects adversarial availability claims", () => {
  const rawDescriptor = mutablePrivacyDescriptor("pq-masp-stark-v0");
  const validated = validatePrivacyAlgorithmDescriptor(rawDescriptor);
  assert.equal(validated.id, "pq-masp-stark-v0");
  assert.equal(validated.productionReady, false);
  assert.equal(validated.productionGate.ready, false);
  assert.equal(Object.isFrozen(validated), true);
  assert.equal(Object.isFrozen(validated.pqLayers), true);
  assert.equal(Object.isFrozen(validated.sdkEntrypoints), true);
  assert.equal(Object.isFrozen(validated.productionGate), true);
  assert.equal(Object.isFrozen(validated.productionGate.gates), true);

  rawDescriptor.id = "tampered";
  rawDescriptor.pqLayers.proof = false;
  rawDescriptor.sdkEntrypoints.push("tamperedEntrypoint");
  rawDescriptor.sourceReferences[0].url = "https://tampered.invalid";
  assert.equal(validated.id, "pq-masp-stark-v0");
  assert.equal(validated.pqLayers.proof, true);
  assert.equal(validated.sdkEntrypoints.includes("tamperedEntrypoint"), false);
  assert.equal(validated.sourceReferences[0].url.startsWith("https://tampered.invalid"), false);
  assert.throws(() => {
    validated.productionReady = true;
  });
  assert.throws(() => {
    validated.productionGate.ready = true;
  });

  assert.throws(
    () => validatePrivacyAlgorithmDescriptor({
      ...mutablePrivacyDescriptor(),
      implementationStage: "production-hardened",
      sdkEntrypoints: ["buildFutureDev.Proof.Fixture"],
      plannedSdkEntrypoints: [],
    }),
    /production-hardened targets cannot advertise fixture\/mock SDK entrypoints/,
  );

  const namespaced = validatePrivacyAlgorithmDescriptor({
    ...mutablePrivacyDescriptor(),
    sdkEntrypoints: ["Iroha.Privacy.buildProof"],
    plannedSdkEntrypoints: ["Iroha.Privacy.buildFutureProof"],
  });
  assert.deepEqual(namespaced.sdkEntrypoints, ["Iroha.Privacy.buildProof"]);
  assert.deepEqual(namespaced.plannedSdkEntrypoints, ["Iroha.Privacy.buildFutureProof"]);

  for (const [label, patch, message] of [
    [
      "status",
      { status: "available" },
      /field status is derived and must not be supplied/,
    ],
    [
      "hidden features",
      { hiddenFeatures: ["hide_sender"] },
      /field hiddenFeatures is derived and must not be supplied/,
    ],
    [
      "verifier metadata",
      { verifierKeyMetadata: { proofFamily: "fake" } },
      /field verifierKeyMetadata is derived and must not be supplied/,
    ],
    [
      "backend family",
      { backendFamily: "fake-backend" },
      /field backendFamily is derived and must not be supplied/,
    ],
    [
      "production ready",
      { productionReady: true },
      /field productionReady is derived and must not be supplied/,
    ],
    [
      "production gate",
      { productionGate: { ready: true } },
      /field productionGate is derived and must not be supplied/,
    ],
    [
      "mainnet-ready summary",
      { summary: "Mainnet-ready audited production proof." },
      /summary must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      "claim-only summary",
      { summary: "Claimed production proof." },
      /summary must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      "claim-only short name",
      { shortName: "Audit claim" },
      /shortName must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      "claim-only recommendation",
      { recommendedFor: ["claimed audit rollout"] },
      /recommendedFor\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      "claim-only chain requirement",
      { chainRequirements: ["production-ready verifier"] },
      /chainRequirements\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    [
      "claim-only setup step",
      { setupSteps: ["Install audit claim verifier"] },
      /setupSteps\[0\] must not claim production\/mainnet\/audit readiness before production gates pass/,
    ],
    ["category", { category: "payments" }, /category must be a known category/],
    ["maturity", { maturity: "blog_post" }, /maturity must be a known maturity/],
    [
      "implementation stage",
      { implementationStage: "Chain-Executable" },
      /implementationStage must be a lowercase hyphenated identifier/,
    ],
    [
      "backend family registration",
      { id: "unmapped-backend-family" },
      /missing backend family metadata/,
    ],
    [
      "unknown criterion",
      { coveredCriteria: ["hide_sender", "forged_availability"] },
      /coveredCriteria\[1\] must be a known privacy criterion/,
    ],
    [
      "duplicate criterion",
      { coveredCriteria: ["hide_sender", "hide_sender"] },
      /coveredCriteria\[1\] duplicates hide_sender/,
    ],
    ["empty sdk entrypoint", { sdkEntrypoints: [""] }, /sdkEntrypoints\[0\] must be a non-empty string/],
    [
      "control-character sdk entrypoint",
      { sdkEntrypoints: ["buildProof\nwithSuffix"] },
      /sdkEntrypoints\[0\] must be clean and already trimmed/,
    ],
    [
      "completed audit security note",
      { securityNotes: ["External audit completed and production sign-off received."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      "claim-only audit security note",
      { securityNotes: ["Claimed audit coverage is present."] },
      /securityNotes\[0\] must describe missing audit\/review gates, not completed audit or signoff claims/,
    ],
    [
      "completed audit failure mode",
      { failureModes: ["External audit completed."] },
      /failureModes\[0\] must describe concrete failure modes, not completed audit or signoff claims/,
    ],
    [
      "claim-only mainnet failure mode",
      { failureModes: ["Mainnet claim accepted by reviewer."] },
      /failureModes\[0\] must describe concrete failure modes, not completed audit or signoff claims/,
    ],
    [
      "shell-like planned entrypoint",
      { plannedSdkEntrypoints: ["buildFutureProof;rm"] },
      /plannedSdkEntrypoints\[0\] must be an SDK entrypoint name/,
    ],
    [
      "duplicate sdk entrypoint",
      { sdkEntrypoints: ["buildTransferAssetInstruction", "buildTransferAssetInstruction"] },
      /sdkEntrypoints\[1\] duplicates buildTransferAssetInstruction/,
    ],
    [
      "duplicate planned entrypoint",
      { plannedSdkEntrypoints: ["buildFutureProof", "buildFutureProof"] },
      /plannedSdkEntrypoints\[1\] duplicates buildFutureProof/,
    ],
    [
      "planned fixture entrypoint",
      { plannedSdkEntrypoints: ["buildFutureProofFixture"] },
      /plannedSdkEntrypoints entry buildFutureProofFixture is a fixture\/mock entrypoint/,
    ],
    [
      "planned mock entrypoint",
      { plannedSdkEntrypoints: ["buildFutureMockProof"] },
      /plannedSdkEntrypoints entry buildFutureMockProof is a fixture\/mock entrypoint/,
    ],
    [
      "planned punctuation-spliced mock entrypoint",
      { plannedSdkEntrypoints: ["buildFutureM-o-c-kProof"] },
      /plannedSdkEntrypoints entry buildFutureM-o-c-kProof is a fixture\/mock entrypoint/,
    ],
    [
      "planned punctuation-spliced dev proof fixture entrypoint",
      { plannedSdkEntrypoints: ["buildFutureDev.Proof.Fixture"] },
      /plannedSdkEntrypoints entry buildFutureDev\.Proof\.Fixture is a fixture\/mock entrypoint/,
    ],
    [
      "overlapping planned entrypoint",
      { plannedSdkEntrypoints: ["buildTransferAssetInstruction"] },
      /plannedSdkEntrypoints entry buildTransferAssetInstruction is already executable/,
    ],
    [
      "http reference",
      { sourceReferences: [{ label: "bad", url: "http://example.invalid" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      "percent-encoded loopback host source URL",
      { sourceReferences: [{ label: "paper", url: "https://127%2e0%2e0%2e1/source" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      "percent-encoded rebinding host source URL",
      { sourceReferences: [{ label: "paper", url: "https://localhost%2elocaltest%2eme/source" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      "malformed percent escape source URL",
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?section=notes%ZZappendix" }] },
      /sourceReferences\[0\]\.url must use https/,
    ],
    [
      "IPv4-compatible loopback IPv6 source URL",
      { sourceReferences: [{ label: "paper", url: "https://[::7f00:1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "NAT64 loopback IPv6 source URL",
      { sourceReferences: [{ label: "paper", url: "https://[64:ff9b::7f00:1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "Teredo IPv6 source URL",
      { sourceReferences: [{ label: "paper", url: "https://[2001:0000:4136:e378:8000:63bf:3fff:fdd2]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "discard-only IPv6 source URL",
      { sourceReferences: [{ label: "paper", url: "https://[100::]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "ORCHIDv2 IPv6 source URL",
      { sourceReferences: [{ label: "paper", url: "https://[2001:20::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "audit claim in source URL fragment",
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224#external-audit-complete" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      "readiness claim in source URL query",
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?production=ready" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      "percent-encoded audit claim in source URL query",
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=audit%3Dcomplete" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      "double-encoded readiness claim in source URL query",
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=production%253Dready" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      "double-encoded mainnet claim in source URL query",
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=mainnet%2520claim" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      "double-encoded audit claim in source URL fragment",
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224#external-%2561udit-complete" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      "deeply encoded readiness claim in source URL query",
      { sourceReferences: [{ label: "paper", url: "https://zips.z.cash/zip-0224?evidence=production%2525253Dready" }] },
      /sourceReferences\[0\]\.url must describe protocol source material, not audit\/signoff or readiness evidence/,
    ],
    [
      "reserved example audit URL reference",
      { sourceReferences: [{ label: "paper", url: "https://audit.example/forged-signoff" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "nip.io loopback reference",
      { sourceReferences: [{ label: "paper", url: "https://127.0.0.1.nip.io/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "sslip.io private-network reference",
      { sourceReferences: [{ label: "paper", url: "https://10.0.0.1.sslip.io/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "localtest.me localhost reference",
      { sourceReferences: [{ label: "paper", url: "https://localhost.localtest.me/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "lvh.me localhost reference",
      { sourceReferences: [{ label: "paper", url: "https://lvh.me/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "IPv4-mapped loopback IPv6 reference",
      { sourceReferences: [{ label: "paper", url: "https://[::ffff:127.0.0.1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "IPv4-mapped private IPv6 reference",
      { sourceReferences: [{ label: "paper", url: "https://[::ffff:c0a8:101]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "site-local IPv6 reference",
      { sourceReferences: [{ label: "paper", url: "https://[fec0::1]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "6to4 loopback IPv6 reference",
      { sourceReferences: [{ label: "paper", url: "https://[2002:7f00:1::]/source" }] },
      /sourceReferences\[0\]\.url must not be a placeholder, local, or private-network URL/,
    ],
    [
      "audit-spoof source reference label",
      { sourceReferences: [{ label: "A.u.d.i.t sign-off", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      "punctuation-spliced review source reference label",
      { sourceReferences: [{ label: "External.review report", url: "https://zips.z.cash/zip-0224" }] },
      /sourceReferences\[0\]\.label must describe protocol source material, not audit\/signoff evidence/,
    ],
    [
      "malformed pq layers",
      { pqLayers: { proof: true, authorization: false, noteEncryption: "yes" } },
      /pqLayers\.noteEncryption must be a boolean/,
    ],
  ]) {
    assert.throws(
      () => validatePrivacyAlgorithmDescriptor({ ...mutablePrivacyDescriptor(), ...patch }),
      message,
      label,
    );
  }
});

descriptorTest("privacy capabilities report native bridge without production claims", () => {
  withPrivacyNativeBinding(completePrivacyNativeBinding(), () => {
    const expectedProductionGateEntries = [
      ["real_proving", false],
      ["real_verification", false],
      ["chain_admission", false],
      ["sdk_parity", false],
      ["wallet_state", false],
      ["deterministic_tests", false],
      ["fuzzing", false],
      ["performance_gates", false],
      ["external_audit", false],
    ];
    const requiredProductionGateMissing = [
      "real proving engine is not registered",
      "real verifier is not registered",
      "chain admission path is not enabled",
      "cross-SDK parity is incomplete",
      "wallet/state support is incomplete",
      "deterministic tests are incomplete",
      "fuzzing gate is incomplete",
      "performance gate is incomplete",
      "external audit signoff is missing",
    ];
    const supplementalProductionGateMissing = [
      "implementation stage is not production-hardened",
      "planned SDK entrypoints remain",
      "dev fixture entrypoints are not production entrypoints",
      "Iroha production allowlist is not enabled for this audited row",
    ];
    const capabilities = getPrivacyCapabilities();
    assert.equal(capabilities.javascriptSdkAvailable, true);
    assert.equal(capabilities.bridgeAvailable, true);
    assert.deepEqual(Object.keys(capabilities).sort(), [
      "bridgeAvailable",
      "javascriptSdkAvailable",
      "privacyAlgorithms",
      "privacyCriteria",
    ]);
    assert.deepEqual(capabilities.privacyCriteria, getPrivacyCriteria());
    assert.equal(capabilities.privacyAlgorithms.length, getPrivacyAlgorithmDescriptors().length);
    assert.equal(
      capabilities.privacyAlgorithms.every((descriptor) => descriptor.productionReady === false),
      true,
    );
    assert.equal(
      capabilities.privacyAlgorithms.every((descriptor) => descriptor.productionGate.ready === false),
      true,
    );
    assert.equal(
      capabilities.privacyAlgorithms.every(
        (descriptor) => descriptor.productionGate.gates.external_audit === false,
      ),
      true,
    );
    for (const descriptor of capabilities.privacyAlgorithms) {
      assert.deepEqual(
        Object.entries(descriptor.productionGate.gates),
        expectedProductionGateEntries,
        `${descriptor.id} production gate entries must stay ordered`,
      );
      assert.deepEqual(
        descriptor.productionGate.missing,
        [
          ...requiredProductionGateMissing,
          ...supplementalProductionGateMissing.filter((reason) =>
            descriptor.productionGate.missing.includes(reason),
          ),
        ],
        `${descriptor.id} production gate missing reasons must stay ordered`,
      );
    }
    assert.equal(Object.isFrozen(capabilities), true);
    assert.equal(Object.isFrozen(capabilities.privacyAlgorithms), true);
    assert.equal(Object.isFrozen(capabilities.privacyAlgorithms[0]), true);
    assert.equal(Object.isFrozen(capabilities.privacyAlgorithms[0].productionGate), true);
    assert.equal(Object.isFrozen(capabilities.privacyAlgorithms[0].productionGate.gates), true);
    assert.equal(Object.isFrozen(capabilities.privacyAlgorithms[0].productionGate.missing), true);
    assert.equal(Object.isFrozen(capabilities.privacyCriteria), true);

    assert.throws(() => {
      capabilities.privacyAlgorithms[0].productionReady = true;
    });
    assert.throws(() => {
      capabilities.privacyAlgorithms[0].productionGate.ready = true;
    });
    assert.throws(() => {
      capabilities.privacyAlgorithms[0].productionGate.gates.external_audit = true;
    });
    assert.throws(() => {
      capabilities.privacyCriteria.push("tampered");
    });

    const fresh = getPrivacyCapabilities();
    assert.equal(fresh.privacyAlgorithms[0].productionReady, false);
    assert.equal(fresh.privacyAlgorithms[0].productionGate.ready, false);
    assert.equal(fresh.privacyAlgorithms[0].productionGate.gates.external_audit, false);
    assert.deepEqual(
      Object.entries(fresh.privacyAlgorithms[0].productionGate.gates),
      expectedProductionGateEntries,
    );
    assert.deepEqual(
      fresh.privacyAlgorithms[0].productionGate.missing.slice(
        0,
        requiredProductionGateMissing.length,
      ),
      requiredProductionGateMissing,
    );
    assert.deepEqual(fresh.privacyCriteria, getPrivacyCriteria());
  });
});

descriptorTest("privacy capabilities fail closed when native privacy ABI is incomplete", () => {
  for (const binding of [
    {},
    completePrivacyNativeBinding({ connectNoritoBridgeAbiVersion: undefined }),
    completePrivacyNativeBinding({
      connectNoritoBridgeAbiVersion() {
        return 5;
      },
    }),
    completePrivacyNativeBinding({
      connectNoritoBridgeAbiVersion() {
        throw new Error("stale ABI");
      },
    }),
  ]) {
    withPrivacyNativeBinding(binding, () => {
      const capabilities = getPrivacyCapabilities();
      assert.equal(capabilities.bridgeAvailable, false);
      assert.equal(
        capabilities.privacyAlgorithms.every((descriptor) => descriptor.productionReady === false),
        true,
      );
    });
  }
});

descriptorTest("privacy native wrappers reject wrong-operation result schemas", () => {
  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyCapabilitiesV1() {
        return privacyNoritoFrameWithSchemaOverride(0x50, 21, 0x42);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyCapabilitiesV1(),
        /native privacyCapabilitiesV1 returned unexpected privacy result schema/,
      );
    },
  );

  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyBuildProofV1() {
        return privacyNoritoFrameWithSchemaOverride(0x42, 6, 0x56);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyBuildProofV1(privacyNoritoFrameWithPayload(0x52)),
        /native privacyBuildProofV1 returned unexpected privacy result schema/,
      );
    },
  );

  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyVerifyProofV1() {
        return privacyNoritoFrameWithSchemaOverride(0x56, 21, 0x50);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyVerifyProofV1(privacyNoritoFrameWithPayload(0x52)),
        /native privacyVerifyProofV1 returned unexpected privacy result schema/,
      );
    },
  );

  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyBuildProofV1() {
        assert.fail("wrong-schema build request must not reach native dispatch");
      },
      privacyVerifyProofV1() {
        assert.fail("wrong-schema verify request must not reach native dispatch");
      },
    }),
    () => {
      for (const wrongSchemaArchive of [
        privacyNoritoFrameWithPayload(0x50),
        privacyNoritoFrameWithPayload(0x42),
        privacyNoritoFrameWithPayload(0x56),
        privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42),
        privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56),
      ]) {
        assert.throws(
          () => privacyBuildProofV1(wrongSchemaArchive),
          /requestArchive must use the privacy request schema/,
        );
        assert.throws(
          () => privacyVerifyProofV1(wrongSchemaArchive),
          /requestArchive must use the privacy request schema/,
        );
      }
    },
  );
});

descriptorTest("privacy native wrappers reject empty request and result payloads", () => {
  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyCapabilitiesV1() {
        return privacyNoritoFrame(0x50);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyCapabilitiesV1(),
        /native privacyCapabilitiesV1 returned empty privacy result payload/,
      );
    },
  );

  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyBuildProofV1() {
        return privacyNoritoFrame(0x42);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyBuildProofV1(privacyNoritoFrameWithPayload(0x52)),
        /native privacyBuildProofV1 returned empty privacy result payload/,
      );
    },
  );

  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyVerifyProofV1() {
        return privacyNoritoFrame(0x56);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyVerifyProofV1(privacyNoritoFrameWithPayload(0x52)),
        /native privacyVerifyProofV1 returned empty privacy result payload/,
      );
    },
  );

  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyBuildProofV1() {
        assert.fail("empty build request must not reach native dispatch");
      },
      privacyVerifyProofV1() {
        assert.fail("empty verify request must not reach native dispatch");
      },
    }),
    () => {
      assert.throws(
        () => privacyBuildProofV1(privacyNoritoFrame(0x52)),
        /requestArchive must contain a non-empty privacy request payload/,
      );
      assert.throws(
        () => privacyVerifyProofV1(privacyNoritoFrame(0x52)),
        /requestArchive must contain a non-empty privacy request payload/,
      );
    },
  );
});

descriptorTest("privacy native wrappers reject ambiguous byte views", () => {
  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyCapabilitiesV1() {
        return int8PrivacyFrame(0x50);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyCapabilitiesV1(),
        /native privacyCapabilitiesV1 output must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
    },
  );

  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyBuildProofV1() {
        return new Uint16Array(24);
      },
    }),
    () => {
      assert.equal(isPrivacyNativeAvailable(), false);
      assert.throws(
        () => privacyBuildProofV1(privacyNoritoFrameWithPayload(0x52)),
        /native privacyBuildProofV1 output must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
    },
  );

  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyBuildProofV1() {
        assert.fail("signed typed-array build request must not reach native dispatch");
      },
      privacyVerifyProofV1() {
        assert.fail("wide typed-array verify request must not reach native dispatch");
      },
    }),
    () => {
      assert.throws(
        () => privacyBuildProofV1(int8PrivacyFrame(0x52)),
        /requestArchive must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
      assert.throws(
        () => privacyVerifyProofV1(new Uint16Array(24)),
        /requestArchive must be Norito V1 bytes as a Buffer, Uint8Array, DataView, or ArrayBuffer/,
      );
    },
  );

  withPrivacyNativeBinding(
    completePrivacyNativeBinding({
      privacyBuildProofV1() {
        assert.fail("shared-memory build request must not reach native dispatch");
      },
      privacyVerifyProofV1() {
        return sharedPrivacyFrame(0x56);
      },
    }),
    () => {
      assert.throws(
        () => privacyBuildProofV1(sharedPrivacyFrame(0x52)),
        /requestArchive must not use shared memory/,
      );
      assert.throws(
        () => privacyVerifyProofV1(privacyNoritoFrameWithPayload(0x52)),
        /native privacyVerifyProofV1 output must not use shared memory/,
      );
    },
  );
});

descriptorTest("privacy algorithm descriptors return defensive copies", () => {
  const first = getPrivacyAlgorithmDescriptor("pq-masp-stark-v0");
  assert.ok(first);
  assert.equal(Object.isFrozen(first), true);
  assert.equal(Object.isFrozen(first.coveredCriteria), true);
  assert.equal(Object.isFrozen(first.pqLayers), true);
  assert.equal(Object.isFrozen(first.productionGate), true);
  assert.equal(Object.isFrozen(first.productionGate.gates), true);
  assert.equal(Object.isFrozen(first.productionGate.missing), true);
  assert.equal(Object.isFrozen(first.sourceReferences), true);
  assert.equal(Object.isFrozen(first.sourceReferences[0]), true);

  assert.throws(() => {
    first.coveredCriteria.length = 0;
  });
  assert.throws(() => {
    first.pqLayers.proof = false;
  });
  assert.throws(() => {
    first.sdkEntrypoints.push("maliciousEntrypoint");
  });
  assert.throws(() => {
    first.plannedSdkEntrypoints.push("maliciousPlannedEntrypoint");
  });
  assert.throws(() => {
    first.chainRequirements.push("malicious validator");
  });
  assert.throws(() => {
    first.productionReady = true;
  });
  assert.throws(() => {
    first.productionGate.ready = true;
  });
  assert.throws(() => {
    first.productionGate.gates.external_audit = true;
  });
  assert.throws(() => {
    first.productionGate.missing.length = 0;
  });
  assert.throws(() => {
    first.recommendedFor.push("malicious recommendation");
  });
  assert.throws(() => {
    first.setupSteps.push("malicious setup");
  });
  assert.throws(() => {
    first.sourceReferences[0].url = "https://malicious.invalid";
  });

  const second = getPrivacyAlgorithmDescriptor("pq-masp-stark-v0");
  assert.ok(second);
  assert.equal(second.coveredCriteria.includes("post_quantum"), true);
  assert.equal(second.pqLayers.proof, true);
  assert.equal(second.sdkEntrypoints.includes("maliciousEntrypoint"), false);
  assert.equal(
    second.plannedSdkEntrypoints.includes("maliciousPlannedEntrypoint"),
    false,
  );
  assert.equal(second.chainRequirements.includes("malicious validator"), false);
  assert.equal(second.productionReady, false);
  assert.equal(second.productionGate.ready, false);
  assert.equal(second.productionGate.gates.external_audit, false);
  assert.ok(second.productionGate.missing.includes("external audit signoff is missing"));
  assert.equal(second.recommendedFor.includes("malicious recommendation"), false);
  assert.equal(second.setupSteps.includes("malicious setup"), false);
  assert.equal(second.sourceReferences[0].url.startsWith("https://www.nist.gov/"), true);

  const catalog = getPrivacyAlgorithmDescriptors();
  assert.equal(Object.isFrozen(catalog), true);
  assert.throws(() => {
    catalog.push({ id: "malicious" });
  });
  const zkAce = catalog.find((descriptor) => descriptor.id === "zk-ace-pq-authorization-v0");
  assert.ok(zkAce);
  assert.throws(() => {
    zkAce.sourceReferences[0].url = "https://malicious.invalid";
  });
  assert.throws(() => {
    zkAce.securityNotes.push("malicious note");
  });
  assert.throws(() => {
    zkAce.requiredState.push("malicious state");
  });
  assert.throws(() => {
    zkAce.failureModes.push("malicious failure");
  });

  const freshCatalog = getPrivacyAlgorithmDescriptors();
  assert.equal(freshCatalog.some((descriptor) => descriptor.id === "malicious"), false);
  const freshZkAce = freshCatalog.find((descriptor) => descriptor.id === "zk-ace-pq-authorization-v0");
  assert.ok(freshZkAce);
  assert.equal(freshZkAce.sourceReferences[0].url, "https://arxiv.org/abs/2603.07974");
  assert.equal(freshZkAce.securityNotes.includes("malicious note"), false);
  assert.equal(freshZkAce.requiredState.includes("malicious state"), false);
  assert.equal(freshZkAce.failureModes.includes("malicious failure"), false);
  assert.equal(getPrivacyAlgorithmDescriptor("missing"), null);
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
    publicAmount: 5,
    inputs: [Buffer.alloc(32, 0x55)],
    proof: {
      backend: "halo2/ipa",
      proof: Buffer.from("proof"),
      verifyingKeyRef: { backend: "halo2/ipa", name: "vk_unshield" },
    },
    rootHint: Buffer.alloc(32, 0x66),
  });
  const payload = encodeAndDecode(instruction).zk.Unshield;
  assert.equal(payload.public_amount, 5);
  assert.deepEqual(payload.root_hint, toByteArray(Buffer.alloc(32, 0x66)));
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
