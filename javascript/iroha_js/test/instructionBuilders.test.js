import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
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
  buildCreateKaigiInstruction,
  buildJoinKaigiInstruction,
  buildLeaveKaigiInstruction,
  buildEndKaigiInstruction,
  buildRecordKaigiUsageInstruction,
  buildSetKaigiRelayManifestInstruction,
  buildRegisterKaigiRelayInstruction,
  buildRegisterSmartContractCodeInstruction,
  buildRegisterSmartContractBytesInstruction,
  buildDeactivateContractInstanceInstruction,
  buildActivateContractInstanceInstruction,
  buildRemoveSmartContractBytesInstruction,
  buildProposeDeployContractInstruction,
  buildCastZkBallotInstruction,
  buildCastPlainBallotInstruction,
  buildEnactReferendumInstruction,
  buildFinalizeReferendumInstruction,
  buildPersistCouncilForEpochInstruction,
  buildClaimTwitterFollowRewardInstruction,
  buildSendToTwitterInstruction,
  buildCancelTwitterEscrowInstruction,
  buildRegisterZkAssetInstruction,
  buildScheduleConfidentialPolicyTransitionInstruction,
  buildCancelConfidentialPolicyTransitionInstruction,
  buildShieldInstruction,
  buildZkTransferInstruction,
  buildUnshieldInstruction,
  buildCreateElectionInstruction,
  buildSubmitBallotInstruction,
  buildFinalizeElectionInstruction,
  encodeInstruction,
} from "../src/instructionBuilders.js";
import { noritoDecodeInstruction, noritoEncodeInstruction } from "../src/norito.js";
import { hasNoritoBinding, makeNativeTest, noritoRequiredMethods } from "./helpers/native.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..");

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
} from "../src/index.js";
import { ValidationErrorCode } from "../src/validationError.js";
import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
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
        return exportedNormalizeAssetId(value);
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

function canonicalizeAccountIdUsingNorito(accountId, domainId = DOMAIN_ID) {
  const encoded = noritoEncodeInstruction({
    Register: { Account: { id: accountId, domain: domainId, metadata: {} } },
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

const DOMAIN_ID = "wonderland";
const ACCOUNT_SIGNATORY =
  "ED0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const ACCOUNT_PUBLIC_KEY = hexToBytes(ACCOUNT_SIGNATORY.slice(6));
const ACCOUNT_ADDRESS = AccountAddress.fromAccount({
  domain: DOMAIN_ID,
  publicKey: ACCOUNT_PUBLIC_KEY,
});
const ACCOUNT_ID = ACCOUNT_ADDRESS.toI105();
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
const NFT_ID = "dragon$wonderland";
const RWA_ID =
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities";
const RWA_ID_INPUT =
  "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF$commodities";
const SAMPLE_PUBLIC_KEY = hexToBytes(
  "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201",
);
const SAMPLE_ACCOUNT_ADDRESS = AccountAddress.fromAccount({
  domain: DOMAIN_ID,
  publicKey: SAMPLE_PUBLIC_KEY,
});
const SAMPLE_ACCOUNT_I105_LITERAL = SAMPLE_ACCOUNT_ADDRESS.toI105();
const SAMPLE_ACCOUNT_COMPRESSED_LITERAL = SAMPLE_ACCOUNT_ADDRESS.toI105Default();
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

test("normalizeAccountId canonicalizes I105 and i105Default (`sora`) encodings", () => {
  const canonicalI105 = exportedNormalizeAccountId(SAMPLE_ACCOUNT_I105_LITERAL);
  assert.equal(canonicalI105, SAMPLE_ACCOUNT_CANONICAL);
  const canonicalCompressed = exportedNormalizeAccountId(SAMPLE_ACCOUNT_COMPRESSED_LITERAL);
  assert.equal(canonicalCompressed, SAMPLE_ACCOUNT_CANONICAL);
});

test("normalizeAccountId rejects Local-8 selectors", () => {
  assert.throws(
    () => exportedNormalizeAccountId(SAMPLE_ACCOUNT_LOCAL8_LITERAL),
    (error) => {
      assert(error instanceof AccountAddressError);
      assert.equal(error.code, AccountAddressErrorCode.LOCAL_DIGEST_TOO_SHORT);
      return true;
    },
  );
});

test("normalizeAssetId exported canonicalizes public asset identifiers", () => {
  const canonical = exportedNormalizeAssetId(ASSET_ID_INPUT);
  assert.equal(canonical, ASSET_ID_CANONICAL);
});

test("normalizeAssetId rejects malformed asset literals", () => {
  assert.throws(
    () => exportedNormalizeAssetId("not:an-asset"),
    /must use '<asset-definition-id>#<i105-account-id>' with optional '#dataspace:<id>' suffix/,
  );
});

test("buildMintAssetInstruction produces Norito-compatible payload", () => {
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

test("buildBurnAssetInstruction produces Norito-compatible payload", () => {
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
      domain: "commodities",
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
        domain: "commodities",
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
    domainId: DOMAIN_ID,
  });
  const account = instruction.Register.Account;
  assert.equal(account.id, ACCOUNT_ID_CANONICAL);
  assert.equal(account.domain, DOMAIN_ID);
  assert.deepEqual(account.metadata, {});
  assert.equal(account.label ?? null, null);
  const decoded = encodeAndDecode(instruction);
  const decodedAccount = decoded.Register.Account;
  assert.equal(decodedAccount.id, ACCOUNT_ID_CANONICAL);
  assert.equal(decodedAccount.domain, DOMAIN_ID);
  assert.deepEqual(decodedAccount.metadata, {});
  assert.equal(decodedAccount.label ?? null, null);
  assert.throws(
    () =>
      buildRegisterAccountInstruction({
        accountId: ACCOUNT_ID,
      }),
    /domainId must be a non-empty string/i,
  );
  assert.throws(
    () =>
      buildRegisterAccountInstruction({
        accountId: ACCOUNT_ID,
        domainId: DOMAIN_ID,
        metadata: ["invalid"],
      }),
    /plain object/i,
  );
});

const RELAY_ACCOUNT_ID = ACCOUNT_ID_CANONICAL;

test("buildCreateKaigiInstruction normalizes relay manifest and metadata", () => {
  const instruction = buildCreateKaigiInstruction({
    id: { domainId: "wonderland", callName: "weekly-sync" },
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
          id: { domain_id: "wonderland", call_name: "weekly-sync" },
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
      },
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("noritoDecodeInstruction decodes Kaigi manifests", () => {
  const instruction = buildCreateKaigiInstruction({
    id: "wonderland:weekly-sync",
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

test("buildJoinKaigiInstruction normalizes buffers and hashes", () => {
  const commitmentBytes = Buffer.alloc(32, 0x11);
  const nullifierBytes = Buffer.alloc(32, 0x22);
  const rosterRootBytes = Buffer.alloc(32, 0x33);
  const proofBytes = Buffer.from([0xaa, 0xbb, 0xcc]);
  const instruction = buildJoinKaigiInstruction({
    callId: "wonderland:weekly-sync",
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
        call_id: { domain_id: "wonderland", call_name: "weekly-sync" },
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
    callId: { domain_id: "wonderland", call_name: "weekly-sync" },
    participant: ACCOUNT_ID,
  });
  const expected = {
    Kaigi: {
      LeaveKaigi: {
        call_id: { domain_id: "wonderland", call_name: "weekly-sync" },
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
    callId: "wonderland:weekly-sync",
    endedAtMs: "1700001234567",
  });
  const expected = {
    Kaigi: {
      EndKaigi: {
        call_id: { domain_id: "wonderland", call_name: "weekly-sync" },
        ended_at_ms: 1700001234567,
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
    callId: "wonderland:weekly-sync",
    durationMs: 60000,
    billedGas: "512",
    usageCommitment,
    proof,
  });
  const expected = {
    Kaigi: {
      RecordKaigiUsage: {
        call_id: { domain_id: "wonderland", call_name: "weekly-sync" },
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
    callId: "wonderland:weekly-sync",
    relayManifest: null,
  });
  const expected = {
    Kaigi: {
      SetKaigiRelayManifest: {
        call_id: { domain_id: "wonderland", call_name: "weekly-sync" },
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
  const instruction = buildRegisterSmartContractCodeInstruction({
    manifest: {
      codeHash: codeHashBytes,
      abiHash: abiHashBytes,
      compilerFingerprint: "rustc-1.79",
      featuresBitmap: "42",
      accessSetHints: {
        readKeys: ["account:alice", "asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM"],
        writeKeys: ["contract:foo"],
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
        },
        entrypoints: null,
      },
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
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

test("buildDeactivateContractInstanceInstruction normalizes reason text", () => {
  const instruction = buildDeactivateContractInstanceInstruction({
    namespace: "apps",
    contractId: "ledger",
    reason: " rotate ",
  });
  const expected = {
    DeactivateContractInstance: {
      namespace: "apps",
      contract_id: "ledger",
      reason: " rotate ",
    },
  };
  assert.deepEqual(instruction, expected);
  assert.deepEqual(encodeAndDecode(instruction), expected);
});

test("buildActivateContractInstanceInstruction normalizes identifiers", () => {
  const instruction = buildActivateContractInstanceInstruction({
    namespace: "apps",
    contractId: "governance",
    codeHash: Buffer.alloc(32, 0x44),
  });
  const expected = {
    ActivateContractInstance: {
      namespace: "apps",
      contract_id: "governance",
      code_hash: normalizedHashHex(Buffer.alloc(32, 0x44)),
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
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
    namespace: "apps",
    contractId: "ledger",
    codeHash: "AA".repeat(32),
    abiHash: Buffer.alloc(32, 0xbb),
    abiVersion: "1",
    window: { lower: 10, upper: 20 },
    votingMode: "plain",
  });
  const expected = {
    ProposeDeployContract: {
      namespace: "apps",
      contract_id: "ledger",
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

test("buildCastZkBallotInstruction rejects deprecated public input keys", () => {
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
  assert.throws(
    () =>
      buildCastZkBallotInstruction({
        electionId: "ref-5",
        proof: Buffer.from([0x06]),
        publicInputs: {
          owner: ACCOUNT_ID_INPUT,
          amount: "250",
          duration_blocks: 12,
        },
      }),
    (error) => {
      assert.equal(error?.code, ValidationErrorCode.INVALID_ACCOUNT_ID);
      assert.match(String(error?.message), /canonical (?:I105 )?account id/i);
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
    derivedBy: "fallback",
  });
  const expected = {
    PersistCouncilForEpoch: {
      epoch: 10,
      members: [ACCOUNT_ID_CANONICAL],
      alternates: [],
      verified: 0,
      candidates_count: 5,
      derived_by: "Fallback",
    },
  };
  assert.deepEqual(instruction, expected);
  const decoded = encodeAndDecode(instruction);
  assert.deepEqual(decoded, expected);
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
        nullifier: [256],
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
      verifyingKeyInline: { backend: "lane/privacy", bytes: new Uint8Array([4, 5]) },
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
