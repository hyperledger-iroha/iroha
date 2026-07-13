import { ed25519 } from "@noble/curves/ed25519";
import { blake2b } from "@noble/hashes/blake2b";
import { sha256 } from "@noble/hashes/sha2";
import {
  normalizeAssetDefinitionId,
  normalizeI105AccountId,
} from "./normalizers.js";
import { verifyEd25519Strict } from "./ed25519Strict.js";

export const VALIDATION_FEE_POLICY_SCHEMA_VERSION = 1;
export const VALIDATION_FEE_DS_SCALE = 2;
export const VALIDATION_FEE_INITIAL_MINOR_UNITS = 10n;
export const VALIDATION_FEE_POLICY_HASH_DOMAIN =
  "iroha.validation_fee.policy.v1";
export const VALIDATION_FEE_POLICY_SIGNATURE_DOMAIN =
  "iroha.validation_fee.policy.signature.v1";
export const VALIDATION_FEE_POLICY_TYPE_NAME =
  "iroha_data_model::validation_fee::ValidationFeePolicyV1";
export const VALIDATION_FEE_CHARGING_MODE =
  "PER_QUALIFYING_TRANSFER_INSTRUCTION";
export const VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS =
  "TREASURY_PAYOUT";

const NORITO_COMPACT_LEN_FLAG = 0x02;
const UINT64_MASK = 0xffff_ffff_ffff_ffffn;
const UINT16_MAX = 0xffffn;
const CRC64_REFLECTED_POLY = 0xc96c_5795_d787_0f42n;
const HEX_RE = /^[0-9a-fA-F]+$/u;
const ED25519_MULTIHASH_RE = /^(?:ed25519:)?ed0120([0-9a-fA-F]{64})$/u;
const textEncoder = new TextEncoder();
const MAX_VALIDATION_FEE_KEYSETS = 64;
const MAX_VALIDATION_FEE_KEYS_PER_KEYSET = 256;
const MAX_VALIDATION_FEE_REGISTRY_ENTRIES = 4096;
const MAX_VALIDATION_FEE_SIGNATURES = 256;
const MAX_VALIDATION_FEE_STRING_CODE_UNITS = 1024;
const MAX_VALIDATION_FEE_STRING_BYTES = 4096;
const MAX_VALIDATION_FEE_BYTE_SOURCE_LENGTH = 64;

export class ValidationFeePolicyError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "ValidationFeePolicyError";
    this.code = code;
  }
}

function readExclusiveAlias(record, aliases, label) {
  const supplied = [];
  for (const alias of aliases) {
    if (!Object.prototype.hasOwnProperty.call(record, alias)) continue;
    const value = record[alias];
    if (value !== undefined) supplied.push({ alias, value });
  }
  if (supplied.length > 1) {
    fail(
      "CONFLICTING_ALIASES",
      `${label} must use exactly one of ${aliases.join(", ")}`,
    );
  }
  return supplied.length === 0 ? undefined : supplied[0].value;
}

function cloneByteSource(value) {
  if (
    typeof value === "string" &&
    value.length > MAX_VALIDATION_FEE_STRING_CODE_UNITS
  ) {
    fail("INPUT_TOO_LARGE", "validation fee byte source is too large");
  }
  const byteLength =
    value instanceof Uint8Array || ArrayBuffer.isView(value)
      ? value.byteLength
      : value instanceof ArrayBuffer
        ? value.byteLength
        : Array.isArray(value)
          ? value.length
          : null;
  if (
    byteLength !== null &&
    byteLength > MAX_VALIDATION_FEE_BYTE_SOURCE_LENGTH
  ) {
    fail("INPUT_TOO_LARGE", "validation fee byte source is too large");
  }
  if (value instanceof Uint8Array) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength).slice();
  }
  if (value instanceof ArrayBuffer) return value.slice(0);
  if (Array.isArray(value)) return Object.freeze([...value]);
  return value;
}

function immutableByteSource(value, label) {
  return typeof value === "string" ? value : bytesToHex(bytes(value, label));
}

function snapshotPolicy(value) {
  const policy = normalizeObject(value, "policy");
  if (
    Array.isArray(policy.exemption_classes) &&
    policy.exemption_classes.length > 1
  ) {
    fail(
      "INVALID_EXEMPTION_CLASSES",
      "validation fee exemption classes exceed the supported release set",
    );
  }
  const exemptionClasses = Array.isArray(policy.exemption_classes)
    ? Object.freeze([...policy.exemption_classes])
    : policy.exemption_classes;
  return Object.freeze({
    schema_version: policy.schema_version,
    network_id: policy.network_id,
    genesis_hash: bytesToHex(bytes32(policy.genesis_hash, "policy.genesis_hash")),
    policy_version: policy.policy_version,
    previous_policy_hash:
      policy.previous_policy_hash === null ||
      policy.previous_policy_hash === undefined
        ? null
        : bytesToHex(
            bytes32(policy.previous_policy_hash, "policy.previous_policy_hash"),
          ),
    ds_asset_id: policy.ds_asset_id,
    ds_scale: policy.ds_scale,
    fee_minor_units: policy.fee_minor_units,
    treasury_account_id: policy.treasury_account_id,
    charging_mode: policy.charging_mode,
    effective_from_height: policy.effective_from_height,
    expires_after_height:
      policy.expires_after_height === undefined
        ? null
        : policy.expires_after_height,
    governance_keyset_id: policy.governance_keyset_id,
    exemption_classes: exemptionClasses,
  });
}

function snapshotRegistry(value) {
  const registry = normalizeObject(value, "policyRegistry");
  if (!Array.isArray(registry.registered_policies)) {
    fail(
      "EMPTY_POLICY_REGISTRY",
      "validation fee policy registry must contain registered_policies",
    );
  }
  if (
    registry.registered_policies.length >
    MAX_VALIDATION_FEE_REGISTRY_ENTRIES
  ) {
    fail(
      "INPUT_TOO_LARGE",
      "validation fee policy registry contains too many entries",
    );
  }
  const registeredPolicies = registry.registered_policies.map((value, index) => {
    const entry = normalizeObject(
      value,
      `policyRegistry.registered_policies[${index}]`,
    );
    return Object.freeze({
      policy_version: entry.policy_version,
      policy_hash: bytesToHex(
        bytes32(
          entry.policy_hash,
          `policyRegistry.registered_policies[${index}].policy_hash`,
        ),
      ),
      previous_policy_hash:
        entry.previous_policy_hash === null ||
        entry.previous_policy_hash === undefined
          ? null
          : bytesToHex(
              bytes32(
                entry.previous_policy_hash,
                `policyRegistry.registered_policies[${index}].previous_policy_hash`,
              ),
            ),
    });
  });
  return Object.freeze({
    active_policy_hash: bytesToHex(
      bytes32(registry.active_policy_hash, "policyRegistry.active_policy_hash"),
    ),
    active_policy_version: registry.active_policy_version,
    registered_policies: Object.freeze(registeredPolicies),
  });
}

function snapshotKeyset(value, index) {
  const keyset = normalizeObject(value, `context.governanceKeysets[${index}]`);
  const publicKeys = readExclusiveAlias(
    keyset,
    ["public_keys", "public_keys_hex"],
    `context.governanceKeysets[${index}].public_keys`,
  );
  const weightedKeys = keyset.keys;
  if (weightedKeys !== undefined && publicKeys !== undefined) {
    fail(
      "INVALID_GOVERNANCE_KEYSET",
      "validation fee governance keyset must use either keys or public_keys",
    );
  }
  const snapshot = {
    keyset_id: keyset.keyset_id,
    threshold: keyset.threshold,
  };
  if (weightedKeys !== undefined) {
    if (!Array.isArray(weightedKeys)) {
      fail(
        "INVALID_GOVERNANCE_KEYSET",
        "validation fee governance keyset keys must be an array",
      );
    }
    if (weightedKeys.length > MAX_VALIDATION_FEE_KEYS_PER_KEYSET) {
      fail(
        "INPUT_TOO_LARGE",
        "validation fee governance keyset contains too many keys",
      );
    }
    snapshot.keys = Object.freeze(
      weightedKeys.map((value, keyIndex) => {
        const entry = normalizeObject(
          value,
          `context.governanceKeysets[${index}].keys[${keyIndex}]`,
        );
        return Object.freeze({
          public_key: immutableByteSource(
            entry.public_key,
            `context.governanceKeysets[${index}].keys[${keyIndex}].public_key`,
          ),
          weight: entry.weight,
        });
      }),
    );
  } else if (publicKeys !== undefined) {
    if (!Array.isArray(publicKeys)) {
      fail(
        "INVALID_GOVERNANCE_KEYSET",
        "validation fee governance keyset public_keys must be an array",
      );
    }
    if (publicKeys.length > MAX_VALIDATION_FEE_KEYS_PER_KEYSET) {
      fail(
        "INPUT_TOO_LARGE",
        "validation fee governance keyset contains too many keys",
      );
    }
    snapshot.public_keys = Object.freeze(
      publicKeys.map((publicKey, keyIndex) =>
        immutableByteSource(
          publicKey,
          `context.governanceKeysets[${index}].public_keys[${keyIndex}]`,
        ),
      ),
    );
  }
  return Object.freeze(snapshot);
}

/**
 * Copy and validate a trusted policy-verification anchor for long-lived use.
 * The returned graph shares no mutable buffers or arrays with the caller.
 */
export function snapshotValidationFeePolicyVerificationContext(value) {
  const context = normalizeObject(value, "context");
  const requireActive = readExclusiveAlias(
    context,
    ["requireActive", "require_active"],
    "context.requireActive",
  );
  if (requireActive !== undefined && requireActive !== true) {
    fail(
      "ACTIVE_POLICY_REQUIRED",
      "validation fee submission verification cannot disable the active-policy check",
    );
  }
  const plural = readExclusiveAlias(
    context,
    ["governanceKeysets", "governance_keysets"],
    "context.governanceKeysets",
  );
  const singular = readExclusiveAlias(
    context,
    ["governanceKeyset", "governance_keyset"],
    "context.governanceKeyset",
  );
  if (plural !== undefined && singular !== undefined) {
    fail(
      "INVALID_GOVERNANCE_KEYSET",
      "context must not provide both governanceKeyset and governanceKeysets",
    );
  }
  const keysets = plural ?? (singular === undefined ? null : [singular]);
  if (!Array.isArray(keysets) || keysets.length === 0) {
    fail(
      "UNKNOWN_GOVERNANCE_KEYSET",
      "validation fee policy verification requires a governance keyset",
    );
  }
  if (keysets.length > MAX_VALIDATION_FEE_KEYSETS) {
    fail(
      "INPUT_TOO_LARGE",
      "validation fee verification context contains too many governance keysets",
    );
  }
  const seenKeysetIds = new Set();
  for (let index = 0; index < keysets.length; index += 1) {
    const keyset = normalizeObject(
      keysets[index],
      `context.governanceKeysets[${index}]`,
    );
    const keysetId = nonEmptyTrimmedString(
      keyset.keyset_id,
      `context.governanceKeysets[${index}].keyset_id`,
    );
    if (seenKeysetIds.has(keysetId)) {
      fail(
        "DUPLICATE_GOVERNANCE_KEYSET_ID",
        `duplicate validation fee governance keyset id ${keysetId}`,
      );
    }
    seenKeysetIds.add(keysetId);
  }
  return Object.freeze({
    networkId: readExclusiveAlias(
      context,
      ["networkId", "network_id"],
      "context.networkId",
    ),
    genesisHash: bytesToHex(
      bytes32(
        readExclusiveAlias(
          context,
          ["genesisHash", "genesis_hash"],
          "context.genesisHash",
        ),
        "context.genesisHash",
      ),
    ),
    currentHeight: readExclusiveAlias(
      context,
      ["currentHeight", "current_height"],
      "context.currentHeight",
    ),
    governanceKeysets: Object.freeze(
      keysets.map((keyset, index) => snapshotKeyset(keyset, index)),
    ),
    policyRegistry: snapshotRegistry(
      readExclusiveAlias(
        context,
        ["policyRegistry", "policy_registry"],
        "context.policyRegistry",
      ),
    ),
  });
}

/** Encode `ValidationFeePolicyV1` exactly as `norito::to_bytes(policy)`. */
export function encodeValidationFeePolicyNorito(policy) {
  const body = snapshotPolicy(policy);
  return frameNoritoPayload(
    encodeValidationFeePolicyBare(body),
    schemaHashForTypeName(VALIDATION_FEE_POLICY_TYPE_NAME),
    NORITO_COMPACT_LEN_FLAG,
  );
}

/** Return the ledger's domain-separated `ValidationFeePolicyV1::policy_hash()`. */
export function validationFeePolicyHash(policy) {
  const body = snapshotPolicy(policy);
  const payload = concatBytes(
    textEncoder.encode(VALIDATION_FEE_POLICY_HASH_DOMAIN),
    Uint8Array.of(0),
    encodeValidationFeePolicyNorito(body),
  );
  return bytesToHex(irohaHash(payload));
}

/** Return the 32-byte message verified by Ed25519 policy signatures. */
export function validationFeePolicyLedgerSignaturePayload(policy) {
  const body = snapshotPolicy(policy);
  const signingPayload = concatBytes(
    encodeField(encodeString(VALIDATION_FEE_POLICY_SIGNATURE_DOMAIN)),
    encodeField(encodeValidationFeePolicyBare(body)),
  );
  return irohaHash(signingPayload);
}

/**
 * Validate a contiguous on-ledger policy registry and prove that `policy` is
 * the registry's active tip.
 */
export function verifyValidationFeePolicyRegistry(registry, policy) {
  const body = snapshotPolicy(policy);
  const record = snapshotRegistry(registry);
  const entries = record.registered_policies;
  if (!Array.isArray(entries) || entries.length === 0) {
    fail("EMPTY_POLICY_REGISTRY", "validation fee policy registry is empty");
  }

  let expectedVersion = 1n;
  let previousHash = null;
  const seenHashes = new Set();
  let latest = null;
  for (let index = 0; index < entries.length; index += 1) {
    const entry = normalizeObject(
      entries[index],
      `policyRegistry.registered_policies[${index}]`,
    );
    const version = toU64(
      entry.policy_version,
      `policyRegistry.registered_policies[${index}].policy_version`,
    );
    if (version !== expectedVersion) {
      fail(
        "UNEXPECTED_POLICY_VERSION",
        `validation fee policy registry expected version ${expectedVersion} but found ${version}`,
      );
    }
    const policyHash = bytes32(
      entry.policy_hash,
      `policyRegistry.registered_policies[${index}].policy_hash`,
    );
    const policyHashHex = bytesToHex(policyHash);
    if (seenHashes.has(policyHashHex)) {
      fail(
        "DUPLICATE_POLICY_HASH",
        `validation fee policy registry duplicates the hash at version ${version}`,
      );
    }
    seenHashes.add(policyHashHex);

    const entryPreviousHash = optionalBytes32(
      entry.previous_policy_hash,
      `policyRegistry.registered_policies[${index}].previous_policy_hash`,
    );
    if (
      (previousHash === null && entryPreviousHash !== null) ||
      (previousHash !== null &&
        (entryPreviousHash === null ||
          !bytesEqual(entryPreviousHash, previousHash)))
    ) {
      fail(
        "BROKEN_PREVIOUS_POLICY_HASH",
        `validation fee policy registry previous hash is broken at version ${version}`,
      );
    }
    latest = {
      version,
      policyHash,
      previousPolicyHash: entryPreviousHash,
    };
    previousHash = policyHash;
    expectedVersion += 1n;
  }

  const activeVersion = toU64(
    record.active_policy_version,
    "policyRegistry.active_policy_version",
  );
  const activeHash = bytes32(
    record.active_policy_hash,
    "policyRegistry.active_policy_hash",
  );
  if (activeVersion !== latest.version) {
    fail(
      "ACTIVE_POLICY_VERSION_MISMATCH",
      `validation fee registry active version ${activeVersion} does not match its latest version ${latest.version}`,
    );
  }
  if (!bytesEqual(activeHash, latest.policyHash)) {
    fail(
      "ACTIVE_POLICY_HASH_MISMATCH",
      "validation fee registry active hash does not match its latest entry",
    );
  }

  const policyVersion = toU64(body.policy_version, "policy.policy_version");
  if (activeVersion !== policyVersion) {
    fail(
      "ACTIVE_POLICY_VERSION_MISMATCH",
      `validation fee registry active version ${activeVersion} does not match signed policy version ${policyVersion}`,
    );
  }
  const computedPolicyHash = hexToBytes(validationFeePolicyHash(body));
  if (!bytesEqual(activeHash, computedPolicyHash)) {
    fail(
      "ACTIVE_POLICY_HASH_MISMATCH",
      "validation fee registry active hash does not match the signed policy",
    );
  }
  const policyPreviousHash = optionalBytes32(
    body.previous_policy_hash,
    "policy.previous_policy_hash",
  );
  if (!optionalBytesEqual(latest.previousPolicyHash, policyPreviousHash)) {
    fail(
      "ACTIVE_PREVIOUS_POLICY_HASH_MISMATCH",
      "validation fee active policy previous hash does not match its registry entry",
    );
  }

  return {
    activePolicyVersion: activeVersion,
    activePolicyHashHex: bytesToHex(activeHash),
    registeredPolicyCount: entries.length,
  };
}

/**
 * Independently verify policy invariants, canonical Norito hash, active
 * registry membership, governance keyset, and weighted Ed25519 threshold.
 */
export function verifySignedValidationFeePolicy(signedPolicy, context) {
  const signed = normalizeObject(signedPolicy, "signedPolicy");
  const verification = snapshotValidationFeePolicyVerificationContext(context);
  const policy = snapshotPolicy(signed.policy);
  validatePolicyBody(policy, verification);

  const policyHashHex = validationFeePolicyHash(policy);
  const registryResult = verifyValidationFeePolicyRegistry(
    verification.policyRegistry,
    policy,
  );
  if (registryResult.activePolicyHashHex !== policyHashHex) {
    fail(
      "ACTIVE_POLICY_HASH_MISMATCH",
      "validation fee policy hash does not match the active registry hash",
    );
  }

  const keysets = normalizeGovernanceKeysets(verification);
  const keyset = keysets.find(
    (candidate) => candidate.keyset_id === policy.governance_keyset_id,
  );
  if (!keyset) {
    fail(
      "UNKNOWN_GOVERNANCE_KEYSET",
      `unknown validation fee governance keyset ${policy.governance_keyset_id}`,
    );
  }
  const { allowedKeys, threshold } = validateKeyset(keyset);
  if (!Array.isArray(signed.signatures) || signed.signatures.length === 0) {
    fail("NO_SIGNATURES", "validation fee policy has no signatures");
  }
  if (signed.signatures.length > MAX_VALIDATION_FEE_SIGNATURES) {
    fail(
      "INPUT_TOO_LARGE",
      "validation fee policy contains too many signatures",
    );
  }
  if (signed.signatures.length > allowedKeys.size) {
    fail(
      "TOO_MANY_SIGNATURES",
      "validation fee policy contains more signatures than governance keys",
    );
  }
  const signatures = Object.freeze(
    signed.signatures.map((value, index) => {
      const signature = normalizeObject(
        value,
        `signedPolicy.signatures[${index}]`,
      );
      return Object.freeze({
        publicKey: cloneByteSource(
          readExclusiveAlias(
            signature,
            ["signer_public_key", "public_key"],
            `signedPolicy.signatures[${index}].public_key`,
          ),
        ),
        signature: bytesToHex(
          bytes(
            normalizeSignatureValue(
              signature.signature,
              `signedPolicy.signatures[${index}].signature`,
            ),
            `signedPolicy.signatures[${index}].signature`,
          ),
        ),
      });
    }),
  );

  const signingPayload = validationFeePolicyLedgerSignaturePayload(policy);
  const seenSigners = new Set();
  let validSignatureWeight = 0n;
  for (let index = 0; index < signatures.length; index += 1) {
    const signerPublicKey = ed25519PublicKeyBytes(
      signatures[index].publicKey,
      `signedPolicy.signatures[${index}].public_key`,
    );
    const signerHex = bytesToHex(signerPublicKey);
    const key = allowedKeys.get(signerHex);
    if (!key) {
      fail(
        "UNKNOWN_SIGNER",
        "validation fee policy signer is not in the active keyset",
      );
    }
    if (seenSigners.has(signerHex)) {
      fail("DUPLICATE_SIGNER", "duplicate validation fee policy signer");
    }
    const signature = bytes(
      signatures[index].signature,
      `signedPolicy.signatures[${index}].signature`,
    );
    if (signature.length !== 64) {
      fail(
        "MALFORMED_SIGNATURE",
        "Ed25519 validation fee policy signature must be 64 bytes",
      );
    }
    validateEd25519SignatureEncoding(
      signature,
      `signedPolicy.signatures[${index}].signature`,
    );
    let valid = false;
    try {
      valid = verifyEd25519Strict(
        signingPayload,
        signature,
        signerPublicKey,
      );
    } catch {
      fail(
        "MALFORMED_PUBLIC_KEY",
        "malformed Ed25519 validation fee policy public key",
      );
    }
    if (!valid) {
      fail("INVALID_SIGNATURE", "invalid validation fee policy signature");
    }
    seenSigners.add(signerHex);
    validSignatureWeight += key.weight;
  }

  if (validSignatureWeight < threshold) {
    fail(
      "INSUFFICIENT_SIGNATURE_THRESHOLD",
      `validation fee policy collected weight ${validSignatureWeight} but requires ${threshold}`,
    );
  }

  return {
    policy,
    policyHashHex,
    policyVersion: toU64(policy.policy_version, "policy.policy_version"),
    validSignatureCount: seenSigners.size,
    validSignatureWeight,
    registry: registryResult,
  };
}

/** Return the canonical NumericV1 quantity for `qualifyingTransferCount`. */
export function validationFeeQuantity(policy, qualifyingTransferCount) {
  const body = snapshotPolicy(policy);
  if (body.ds_scale !== VALIDATION_FEE_DS_SCALE) {
    fail("INVALID_DS_SCALE", "validation fee policy asset scale must be 2");
  }
  const perTransfer = toU64(body.fee_minor_units, "policy.fee_minor_units");
  if (perTransfer !== VALIDATION_FEE_INITIAL_MINOR_UNITS) {
    fail(
      "INVALID_INITIAL_FEE_MINOR_UNITS",
      "validation fee policy amount must be 10 minor units",
    );
  }
  const count = toU64(
    qualifyingTransferCount,
    "qualifyingTransferCount",
  );
  const minorUnits = perTransfer * count;
  if (minorUnits > UINT64_MASK) {
    fail("REQUIRED_FEE_OVERFLOW", "required validation fee exceeds u64");
  }
  return canonicalQuantity(minorUnits, body.ds_scale);
}

function validatePolicyBody(policy, context) {
  if (policy.schema_version !== VALIDATION_FEE_POLICY_SCHEMA_VERSION) {
    fail(
      "UNSUPPORTED_SCHEMA_VERSION",
      `unsupported validation fee policy schema version ${policy.schema_version}`,
    );
  }
  const policyVersion = toU64(policy.policy_version, "policy.policy_version");
  if (policyVersion === 0n) {
    fail("INVALID_POLICY_VERSION", "validation fee policy version must be positive");
  }
  const previousPolicyHash = optionalBytes32(
    policy.previous_policy_hash,
    "policy.previous_policy_hash",
  );
  if (policyVersion === 1n && previousPolicyHash !== null) {
    fail(
      "UNEXPECTED_PREVIOUS_POLICY_HASH",
      "initial validation fee policy must not carry a previous policy hash",
    );
  }
  if (policyVersion > 1n && previousPolicyHash === null) {
    fail(
      "MISSING_PREVIOUS_POLICY_HASH",
      "non-initial validation fee policy must carry a previous policy hash",
    );
  }

  const expectedNetworkId = nonEmptyTrimmedString(
    context.networkId,
    "context.networkId",
  );
  const policyNetworkId = nonEmptyTrimmedString(
    policy.network_id,
    "policy.network_id",
  );
  if (policyNetworkId !== expectedNetworkId) {
    fail(
      "WRONG_NETWORK",
      `validation fee policy network mismatch: expected ${expectedNetworkId}, found ${policyNetworkId}`,
    );
  }
  if (
    !bytesEqual(
      bytes32(policy.genesis_hash, "policy.genesis_hash"),
      bytes32(context.genesisHash, "context.genesisHash"),
    )
  ) {
    fail("WRONG_GENESIS", "validation fee policy genesis hash mismatch");
  }

  const dsAssetId = nonEmptyTrimmedString(
    policy.ds_asset_id,
    "policy.ds_asset_id",
  );
  let canonicalDsAssetId;
  try {
    canonicalDsAssetId = normalizeAssetDefinitionId(
      dsAssetId,
      "policy.ds_asset_id",
    );
  } catch {
    fail(
      "INVALID_DS_ASSET_ID",
      "validation fee policy DS asset id must be canonical",
    );
  }
  if (canonicalDsAssetId !== dsAssetId) {
    fail(
      "INVALID_DS_ASSET_ID",
      "validation fee policy DS asset id must be canonical",
    );
  }
  if (policy.ds_scale !== VALIDATION_FEE_DS_SCALE) {
    fail("INVALID_DS_SCALE", "validation fee policy asset scale must be 2");
  }
  if (
    toU64(policy.fee_minor_units, "policy.fee_minor_units") !==
    VALIDATION_FEE_INITIAL_MINOR_UNITS
  ) {
    fail(
      "INVALID_INITIAL_FEE_MINOR_UNITS",
      "validation fee policy amount must be 10 minor units",
    );
  }

  const treasuryAccountId = nonEmptyTrimmedString(
    policy.treasury_account_id,
    "policy.treasury_account_id",
  );
  let canonicalTreasuryAccountId;
  try {
    canonicalTreasuryAccountId = normalizeI105AccountId(
      treasuryAccountId,
      "policy.treasury_account_id",
    );
  } catch {
    fail(
      "INVALID_TREASURY_ACCOUNT_ID",
      "validation fee policy treasury account id must be canonical I105",
    );
  }
  if (canonicalTreasuryAccountId !== treasuryAccountId) {
    fail(
      "INVALID_TREASURY_ACCOUNT_ID",
      "validation fee policy treasury account id must be canonical I105",
    );
  }
  if (policy.charging_mode !== VALIDATION_FEE_CHARGING_MODE) {
    fail("INVALID_CHARGING_MODE", "unsupported validation fee charging mode");
  }
  nonEmptyTrimmedString(
    policy.governance_keyset_id,
    "policy.governance_keyset_id",
  );
  validateExemptionClasses(policy.exemption_classes);

  const effectiveFromHeight = toU64(
    policy.effective_from_height,
    "policy.effective_from_height",
  );
  const expiresAfterHeight = optionalU64(
    policy.expires_after_height,
    "policy.expires_after_height",
  );
  if (
    expiresAfterHeight !== null &&
    expiresAfterHeight <= effectiveFromHeight
  ) {
    fail(
      "INVALID_POLICY_VALIDITY_WINDOW",
      "validation fee policy validity window is invalid",
    );
  }
  const currentHeight = toU64(context.currentHeight, "context.currentHeight");
  if (currentHeight < effectiveFromHeight) {
    fail("FUTURE_POLICY", "validation fee policy is not active yet");
  }
  if (expiresAfterHeight !== null && currentHeight >= expiresAfterHeight) {
    fail("EXPIRED_POLICY", "validation fee policy is expired");
  }
}

function normalizeGovernanceKeysets(context) {
  const values = context.governanceKeysets;
  if (!Array.isArray(values) || values.length === 0) {
    fail(
      "UNKNOWN_GOVERNANCE_KEYSET",
      "validation fee policy verification requires a governance keyset",
    );
  }
  return values.map((value, index) =>
    normalizeObject(value, `context.governanceKeysets[${index}]`),
  );
}

function validateKeyset(keyset) {
  const keysetId = nonEmptyTrimmedString(
    keyset.keyset_id,
    "governanceKeyset.keyset_id",
  );
  const threshold = toU16(keyset.threshold, "governanceKeyset.threshold");
  if (threshold === 0n) {
    fail(
      "INVALID_GOVERNANCE_THRESHOLD",
      `validation fee governance keyset ${keysetId} threshold must be positive`,
    );
  }

  const weightedKeys = keyset.keys;
  const publicKeys = keyset.public_keys;
  if (weightedKeys !== undefined && publicKeys !== undefined) {
    fail(
      "INVALID_GOVERNANCE_KEYSET",
      "validation fee governance keyset must use either keys or public_keys",
    );
  }
  let entries;
  if (Array.isArray(weightedKeys)) {
    if (weightedKeys.length > MAX_VALIDATION_FEE_KEYS_PER_KEYSET) {
      fail(
        "INPUT_TOO_LARGE",
        "validation fee governance keyset contains too many keys",
      );
    }
    entries = weightedKeys.map((value, index) => {
      const entry = normalizeObject(value, `governanceKeyset.keys[${index}]`);
      const weight = toU16(
        entry.weight,
        `governanceKeyset.keys[${index}].weight`,
      );
      if (weight === 0n) {
        fail(
          "INVALID_GOVERNANCE_WEIGHT",
          "validation fee governance key weight must be positive",
        );
      }
      return { publicKey: entry.public_key, weight };
    });
  } else if (Array.isArray(publicKeys)) {
    if (publicKeys.length > MAX_VALIDATION_FEE_KEYS_PER_KEYSET) {
      fail(
        "INPUT_TOO_LARGE",
        "validation fee governance keyset contains too many keys",
      );
    }
    entries = publicKeys.map((publicKey) => ({ publicKey, weight: 1n }));
  } else {
    fail(
      "INVALID_GOVERNANCE_KEYSET",
      "validation fee governance keyset must contain at least one key",
    );
  }
  if (entries.length === 0) {
    fail(
      "INVALID_GOVERNANCE_KEYSET",
      "validation fee governance keyset must contain at least one key",
    );
  }

  const allowedKeys = new Map();
  let totalWeight = 0n;
  for (let index = 0; index < entries.length; index += 1) {
    const publicKey = ed25519PublicKeyBytes(
      entries[index].publicKey,
      `governanceKeyset.keys[${index}].public_key`,
    );
    const keyHex = bytesToHex(publicKey);
    if (allowedKeys.has(keyHex)) {
      fail(
        "DUPLICATE_GOVERNANCE_KEY",
        "duplicate public key in validation fee governance keyset",
      );
    }
    const entry = { publicKey, weight: entries[index].weight };
    allowedKeys.set(keyHex, entry);
    totalWeight += entry.weight;
  }
  if (totalWeight < threshold) {
    fail(
      "INVALID_GOVERNANCE_THRESHOLD",
      "validation fee governance threshold exceeds total key weight",
    );
  }
  return { allowedKeys, threshold };
}

function validateExemptionClasses(value) {
  if (!Array.isArray(value)) {
    fail(
      "INVALID_EXEMPTION_CLASSES",
      "validation fee exemption classes must be an array",
    );
  }
  const seen = new Set();
  for (const exemptionClass of value) {
    if (
      exemptionClass !== VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS ||
      seen.has(exemptionClass)
    ) {
      fail(
        "INVALID_EXEMPTION_CLASSES",
        "validation fee exemption classes must be unique approved release classes: TREASURY_PAYOUT",
      );
    }
    seen.add(exemptionClass);
  }
}

function encodeValidationFeePolicyBare(policy) {
  return concatBytes(
    encodeField(encodeU16(policy.schema_version, "policy.schema_version")),
    encodeField(encodeString(policy.network_id)),
    encodeArray32Field(policy.genesis_hash, "policy.genesis_hash"),
    encodeField(encodeU64(policy.policy_version, "policy.policy_version")),
    encodeField(
      encodeOptionalBytes32(
        policy.previous_policy_hash,
        "policy.previous_policy_hash",
      ),
    ),
    encodeField(encodeString(policy.ds_asset_id)),
    encodeField(encodeU8(policy.ds_scale, "policy.ds_scale")),
    encodeField(encodeU64(policy.fee_minor_units, "policy.fee_minor_units")),
    encodeField(encodeString(policy.treasury_account_id)),
    encodeField(encodeChargingMode(policy.charging_mode)),
    encodeField(
      encodeU64(
        policy.effective_from_height,
        "policy.effective_from_height",
      ),
    ),
    encodeField(
      encodeOptionalU64(
        policy.expires_after_height,
        "policy.expires_after_height",
      ),
    ),
    encodeField(encodeString(policy.governance_keyset_id)),
    encodeField(encodeStringVec(policy.exemption_classes)),
  );
}

function encodeField(payload) {
  return concatBytes(encodeCompactLen(BigInt(payload.length)), payload);
}

function encodeArray32Field(value, label) {
  return concatBytes(encodeCompactLen(32n), bytes32(value, label));
}

function encodeString(value) {
  if (typeof value !== "string") {
    fail("INVALID_STRING", "Norito string value must be a string");
  }
  const encoded = textEncoder.encode(value);
  if (encoded.length > MAX_VALIDATION_FEE_STRING_BYTES) {
    fail("INPUT_TOO_LARGE", "validation fee string is too large");
  }
  return concatBytes(encodeCompactLen(BigInt(encoded.length)), encoded);
}

function encodeStringVec(values) {
  if (!Array.isArray(values)) {
    fail("INVALID_EXEMPTION_CLASSES", "policy.exemption_classes must be an array");
  }
  return concatBytes(
    encodeU64(values.length, "policy.exemption_classes.length"),
    ...values.map((value) => encodeField(encodeString(value))),
  );
}

function encodeOptionalBytes32(value, label) {
  const parsed = optionalBytes32(value, label);
  return parsed === null
    ? Uint8Array.of(0)
    : concatBytes(Uint8Array.of(1), encodeField(parsed));
}

function encodeOptionalU64(value, label) {
  const parsed = optionalU64(value, label);
  return parsed === null
    ? Uint8Array.of(0)
    : concatBytes(Uint8Array.of(1), encodeField(encodeU64(parsed, label)));
}

function encodeChargingMode(value) {
  if (value !== VALIDATION_FEE_CHARGING_MODE) {
    fail("INVALID_CHARGING_MODE", "unsupported validation fee charging mode");
  }
  return encodeU32(0);
}

function encodeU8(value, label) {
  if (!Number.isInteger(value) || value < 0 || value > 0xff) {
    fail("INVALID_U64", `${label} must fit in u8`);
  }
  return Uint8Array.of(value);
}

function encodeU16(value, label) {
  const parsed = toU16(value, label);
  const out = new Uint8Array(2);
  new DataView(out.buffer).setUint16(0, Number(parsed), true);
  return out;
}

function encodeU32(value) {
  const out = new Uint8Array(4);
  new DataView(out.buffer).setUint32(0, value, true);
  return out;
}

function encodeU64(value, label) {
  let remaining = toU64(value, label);
  const out = new Uint8Array(8);
  for (let index = 0; index < out.length; index += 1) {
    out[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return out;
}

function encodeCompactLen(value) {
  if (value < 0n) {
    fail("INVALID_U64", "Norito compact length cannot be negative");
  }
  const out = [];
  let remaining = value;
  do {
    let byte = Number(remaining & 0x7fn);
    remaining >>= 7n;
    if (remaining !== 0n) byte |= 0x80;
    out.push(byte);
  } while (remaining !== 0n);
  return Uint8Array.from(out);
}

function frameNoritoPayload(payload, schemaHash, flags) {
  const header = concatBytes(
    textEncoder.encode("NRT0"),
    Uint8Array.of(0, 0),
    schemaHash,
    Uint8Array.of(0),
    encodeU64(payload.length, "norito.payloadLength"),
    encodeU64(crc64(payload), "norito.payloadCrc"),
    Uint8Array.of(flags & 0xff),
  );
  return concatBytes(header, payload);
}

function schemaHashForTypeName(typeName) {
  const digest = sha256(
    concatBytes(
      textEncoder.encode("norito:v1:type-name\0"),
      textEncoder.encode(typeName),
    ),
  );
  return digest.subarray(0, 16);
}

function irohaHash(payload) {
  const digest = blake2b(payload, { dkLen: 32 });
  digest[digest.length - 1] |= 1;
  return digest;
}

const CRC64_TABLE = Array.from({ length: 256 }, (_, index) => {
  let crc = BigInt(index);
  for (let bit = 0; bit < 8; bit += 1) {
    crc =
      (crc & 1n) !== 0n
        ? (crc >> 1n) ^ CRC64_REFLECTED_POLY
        : crc >> 1n;
  }
  return crc;
});

function crc64(payload) {
  let crc = UINT64_MASK;
  for (const byte of payload) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = CRC64_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ UINT64_MASK);
}

function normalizeObject(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail("INVALID_OBJECT", `${label} must be an object`);
  }
  return value;
}

function nonEmptyTrimmedString(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.trim() !== value
  ) {
    fail("INVALID_STRING", `${label} must be a non-empty trimmed string`);
  }
  if (value.length > MAX_VALIDATION_FEE_STRING_CODE_UNITS) {
    fail("INPUT_TOO_LARGE", `${label} is too large`);
  }
  return value;
}

function toU16(value, label) {
  const parsed = toU64(value, label);
  if (parsed > UINT16_MAX) {
    fail("INVALID_U64", `${label} must fit in u16`);
  }
  return parsed;
}

function toU64(value, label) {
  let parsed;
  if (typeof value === "bigint") {
    parsed = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      fail("INVALID_U64", `${label} must be a safe integer`);
    }
    parsed = BigInt(value);
  } else if (typeof value === "string" && /^\d+$/u.test(value)) {
    parsed = BigInt(value);
  } else {
    fail("INVALID_U64", `${label} must be an unsigned integer`);
  }
  if (parsed < 0n || parsed > UINT64_MASK) {
    fail("INVALID_U64", `${label} must fit in u64`);
  }
  return parsed;
}

function optionalU64(value, label) {
  return value === null || value === undefined ? null : toU64(value, label);
}

function optionalBytes32(value, label) {
  return value === null || value === undefined ? null : bytes32(value, label);
}

function bytes32(value, label) {
  const parsed = bytes(value, label);
  if (parsed.length !== 32) {
    fail("INVALID_BYTES", `${label} must contain 32 bytes`);
  }
  return parsed;
}

function bytes(value, label) {
  if (value instanceof Uint8Array) {
    if (value.byteLength > MAX_VALIDATION_FEE_BYTE_SOURCE_LENGTH) {
      fail("INPUT_TOO_LARGE", `${label} is too large`);
    }
    return new Uint8Array(value);
  }
  if (ArrayBuffer.isView(value)) {
    if (value.byteLength > MAX_VALIDATION_FEE_BYTE_SOURCE_LENGTH) {
      fail("INPUT_TOO_LARGE", `${label} is too large`);
    }
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength).slice();
  }
  if (value instanceof ArrayBuffer) {
    if (value.byteLength > MAX_VALIDATION_FEE_BYTE_SOURCE_LENGTH) {
      fail("INPUT_TOO_LARGE", `${label} is too large`);
    }
    return new Uint8Array(value.slice(0));
  }
  if (Array.isArray(value)) {
    if (value.length > MAX_VALIDATION_FEE_BYTE_SOURCE_LENGTH) {
      fail("INPUT_TOO_LARGE", `${label} is too large`);
    }
    const out = new Uint8Array(value.length);
    for (let index = 0; index < value.length; index += 1) {
      const byte = value[index];
      if (!Number.isInteger(byte) || byte < 0 || byte > 0xff) {
        fail("INVALID_BYTES", `${label} contains an invalid byte`);
      }
      out[index] = byte;
    }
    return out;
  }
  if (typeof value !== "string") {
    fail("INVALID_BYTES", `${label} must be bytes or hex-encoded bytes`);
  }
  if (value.length > MAX_VALIDATION_FEE_BYTE_SOURCE_LENGTH * 2) {
    fail("INPUT_TOO_LARGE", `${label} is too large`);
  }
  const normalized = value.trim();
  if (
    normalized.length === 0 ||
    normalized.length % 2 !== 0 ||
    !HEX_RE.test(normalized)
  ) {
    fail("INVALID_BYTES", `${label} must be hex-encoded bytes`);
  }
  return hexToBytes(normalized);
}

function ed25519PublicKeyBytes(value, label) {
  let parsed;
  if (typeof value === "string") {
    const match = ED25519_MULTIHASH_RE.exec(value.trim());
    if (match) parsed = hexToBytes(match[1]);
  }
  parsed ??= bytes32(value, label);
  let point;
  try {
    point = ed25519.Point.fromHex(parsed);
  } catch {
    fail("MALFORMED_PUBLIC_KEY", `${label} is not a valid Ed25519 public key`);
  }
  if (point.isSmallOrder()) {
    fail(
      "MALFORMED_PUBLIC_KEY",
      `${label} is a small-order Ed25519 public key`,
    );
  }
  return parsed;
}

function normalizeSignatureValue(value, label) {
  if (
    value &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    !ArrayBuffer.isView(value) &&
    !(value instanceof ArrayBuffer)
  ) {
    return readExclusiveAlias(
      value,
      ["payload", "bytes", "signature"],
      label,
    );
  }
  return value;
}

function validateEd25519SignatureEncoding(signature, label) {
  if (signature.every((byte) => byte === 0)) {
    fail("MALFORMED_SIGNATURE", `${label} must not be all zero`);
  }
  let commitment;
  try {
    commitment = ed25519.Point.fromHex(signature.subarray(0, 32));
  } catch {
    fail(
      "MALFORMED_SIGNATURE",
      `${label} has a non-canonical Ed25519 R component`,
    );
  }
  if (commitment.isSmallOrder()) {
    fail(
      "MALFORMED_SIGNATURE",
      `${label} has a small-order Ed25519 R component`,
    );
  }
  let scalar = 0n;
  for (let index = 63; index >= 32; index -= 1) {
    scalar = (scalar << 8n) | BigInt(signature[index]);
  }
  if (scalar >= ed25519.CURVE.n) {
    fail(
      "MALFORMED_SIGNATURE",
      `${label} has a non-canonical Ed25519 scalar`,
    );
  }
}

function hexToBytes(value) {
  const out = new Uint8Array(value.length / 2);
  for (let index = 0; index < out.length; index += 1) {
    out[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  }
  return out;
}

function bytesToHex(value) {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}

function bytesEqual(left, right) {
  if (left.length !== right.length) return false;
  let different = 0;
  for (let index = 0; index < left.length; index += 1) {
    different |= left[index] ^ right[index];
  }
  return different === 0;
}

function optionalBytesEqual(left, right) {
  if (left === null || right === null) return left === right;
  return bytesEqual(left, right);
}

function concatBytes(...chunks) {
  const length = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const out = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

function canonicalQuantity(minorUnits, scale) {
  const denominator = 10n ** BigInt(scale);
  const whole = minorUnits / denominator;
  const fractional = (minorUnits % denominator)
    .toString()
    .padStart(scale, "0");
  const canonicalFractional = fractional.replace(/0+$/u, "");
  return canonicalFractional.length === 0
    ? whole.toString()
    : `${whole}.${canonicalFractional}`;
}

function fail(code, message) {
  throw new ValidationFeePolicyError(code, message);
}
