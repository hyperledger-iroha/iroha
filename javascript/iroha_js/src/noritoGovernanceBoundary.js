import { parseCanonicalContractAddress } from "./contractAddress.js";
import { isCanonicalGovernanceSelectorV1 } from "./governanceSelector.js";
import { ensureCanonicalAccountId } from "./normalizers.js";
import { NumericV1 } from "./numericV1.js";
import {
  parseStrictLosslessIntegerJson,
  stringifyStrictLosslessIntegerJson,
} from "./strictLosslessJson.js";

const UINT64_MASK = 0xffff_ffff_ffff_ffffn;
const GOVERNANCE_PRIVATE_KEY_FIELDS = new Set([
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
]);
const GOVERNANCE_ZK_PUBLIC_INPUT_FIELDS = Object.freeze([
  "root_hint",
  "owner",
  "amount",
  "duration_blocks",
  "direction",
  "nullifier",
]);

/**
 * Bind strict governance validation to the low-level Norito codecs without
 * introducing a cycle back to the public codec module.
 */
export function createNoritoGovernanceInstructionBoundary({
  assertExactNonEmptyString,
  assertOnlyObjectKeys,
  decodeExactStandardBase64,
  decodeManifestProvenanceValue,
  encodeManifestProvenanceValue,
  encodeVotingModeValue,
  isPlainObject,
}) {
  function isStrictGovernanceInstructionCandidate(value) {
    return (
      isPlainObject(value) &&
      (
        Object.prototype.hasOwnProperty.call(value, "ProposeDeployContract") ||
        Object.prototype.hasOwnProperty.call(value, "CastZkBallot")
      )
    );
  }

  function assertCanonicalGovernanceSelectorV1(value, context) {
    if (!isCanonicalGovernanceSelectorV1(value)) {
      throw new TypeError(
        `${context} must be 1-128 RFC 3986 unreserved ASCII characters and must not start with a dot`,
      );
    }
    return value;
  }

  function validateGovernanceSelectorPayload(payload, field, context) {
    if (
      !isPlainObject(payload) ||
      !Object.prototype.hasOwnProperty.call(payload, field)
    ) {
      return;
    }
    assertCanonicalGovernanceSelectorV1(payload[field], `${context}.${field}`);
  }

  function validateGovernanceInstructionSelectors(instruction) {
    if (!isPlainObject(instruction)) {
      return;
    }
    for (const [variant, field] of [
      ["CastZkBallot", "election_id"],
      ["CastPlainBallot", "referendum_id"],
      ["FinalizeReferendum", "referendum_id"],
    ]) {
      validateGovernanceSelectorPayload(instruction[variant], field, variant);
    }
    for (const zkKey of ["zk", "Zk", "ZK"]) {
      const zk = instruction[zkKey];
      if (!isPlainObject(zk)) {
        continue;
      }
      for (const variant of [
        "CreateElection",
        "SubmitBallot",
        "FinalizeElection",
      ]) {
        validateGovernanceSelectorPayload(
          zk[variant],
          "election_id",
          `${zkKey}.${variant}`,
        );
      }
    }
  }

  function rejectGovernancePrivateKeyFieldsDeep(value, context) {
    const pending = [{ value, path: context }];
    const visited = new WeakSet();
    while (pending.length > 0) {
      const { value: candidate, path } = pending.pop();
      if (candidate === null || typeof candidate !== "object") {
        continue;
      }
      if (visited.has(candidate)) {
        continue;
      }
      visited.add(candidate);
      if (!Array.isArray(candidate) && !isPlainObject(candidate)) {
        continue;
      }
      for (const key of Object.keys(candidate)) {
        if (GOVERNANCE_PRIVATE_KEY_FIELDS.has(key)) {
          throw new TypeError(
            `${path} does not accept private-key field ${key}; sign the transaction locally`,
          );
        }
        pending.push({ value: candidate[key], path: `${path}.${key}` });
      }
    }
  }

  function assertExactGovernanceObjectKeys(value, allowed, required, context) {
    if (!isPlainObject(value)) {
      throw new TypeError(`${context} must be an object`);
    }
    assertOnlyObjectKeys(value, allowed, context);
    for (const field of required) {
      if (!Object.prototype.hasOwnProperty.call(value, field)) {
        throw new TypeError(`${context}.${field} is required`);
      }
    }
  }

  function normalizeGovernanceHex32(value, context) {
    if (typeof value !== "string" || value.length === 0) {
      throw new TypeError(`${context} must be exactly 32-byte hexadecimal`);
    }
    let body = value;
    const separator = value.indexOf(":");
    if (separator !== -1) {
      const scheme = value.slice(0, separator);
      if (scheme.length === 0 || scheme.toLowerCase() !== "blake2b32") {
        throw new TypeError(`${context} must use the optional blake2b32: scheme`);
      }
      body = value.slice(separator + 1);
    }
    if (body.startsWith("0x") || body.startsWith("0X")) {
      body = body.slice(2);
    }
    if (body.length !== 64 || !/^[0-9A-Fa-f]{64}$/u.test(body)) {
      throw new TypeError(
        `${context} must be exactly 32-byte hexadecimal with no whitespace`,
      );
    }
    return body.toLowerCase();
  }

  function normalizeGovernanceU64(value, context) {
    let integer;
    if (typeof value === "bigint") {
      integer = value;
    } else if (typeof value === "number") {
      if (!Number.isSafeInteger(value) || value < 0) {
        throw new TypeError(`${context} must be a lossless unsigned 64-bit integer`);
      }
      integer = BigInt(value);
    } else if (typeof value === "string") {
      if (!/^(?:0|[1-9][0-9]*)$/u.test(value)) {
        throw new TypeError(`${context} must be a canonical unsigned 64-bit integer`);
      }
      integer = BigInt(value);
    } else {
      throw new TypeError(`${context} must be a lossless unsigned 64-bit integer`);
    }
    if (integer < 0n || integer > UINT64_MASK) {
      throw new RangeError(`${context} must fit in an unsigned 64-bit integer`);
    }
    return integer;
  }

  function normalizeGovernanceWindowValue(value, context) {
    assertExactGovernanceObjectKeys(
      value,
      ["lower", "upper"],
      ["lower", "upper"],
      context,
    );
    const lower = normalizeGovernanceU64(value.lower, `${context}.lower`);
    const upper = normalizeGovernanceU64(value.upper, `${context}.upper`);
    if (upper < lower) {
      throw new RangeError(`${context}.upper must be greater than or equal to lower`);
    }
    return { lower: lower.toString(10), upper: upper.toString(10) };
  }

  function normalizeGovernanceQuantity(value, context) {
    if (typeof value !== "string") {
      throw new TypeError(`${context} must be a canonical Kotodama V1 quantity string`);
    }
    try {
      return NumericV1.decodeQuantityJson(value).toString();
    } catch {
      throw new TypeError(`${context} must be a canonical non-negative Kotodama V1 quantity`);
    }
  }

  function normalizeGovernanceBallotDirection(value, context) {
    if (value === "Aye" || value === "Nay" || value === "Abstain") {
      return value;
    }
    throw new TypeError(`${context} must be exactly Aye, Nay, or Abstain`);
  }

  function normalizeGovernanceZkPublicInputsJson(value, context) {
    if (typeof value !== "string" || value.length === 0) {
      throw new TypeError(`${context} must be a non-empty JSON object string`);
    }
    const parsed = parseStrictLosslessIntegerJson(value, context);
    if (!isPlainObject(parsed)) {
      throw new TypeError(`${context} must encode a JSON object`);
    }
    rejectGovernancePrivateKeyFieldsDeep(parsed, context);
    assertOnlyObjectKeys(parsed, GOVERNANCE_ZK_PUBLIC_INPUT_FIELDS, context);

    const normalized = {};
    for (const field of GOVERNANCE_ZK_PUBLIC_INPUT_FIELDS) {
      if (!Object.prototype.hasOwnProperty.call(parsed, field)) {
        continue;
      }
      const entry = parsed[field];
      if (entry === null) {
        normalized[field] = null;
        continue;
      }
      switch (field) {
        case "root_hint":
        case "nullifier":
          normalized[field] = normalizeGovernanceHex32(entry, `${context}.${field}`);
          break;
        case "owner":
          normalized.owner = ensureCanonicalAccountId(entry, `${context}.owner`);
          break;
        case "amount":
          normalized.amount = normalizeGovernanceQuantity(entry, `${context}.amount`);
          break;
        case "duration_blocks":
          normalized.duration_blocks = normalizeGovernanceU64(
            entry,
            `${context}.duration_blocks`,
          );
          break;
        case "direction":
          normalized.direction = normalizeGovernanceBallotDirection(
            entry,
            `${context}.direction`,
          );
          break;
        default:
          throw new Error(`unhandled governance public input ${field}`);
      }
    }

    const hasOwner = normalized.owner !== undefined && normalized.owner !== null;
    const hasAmount = normalized.amount !== undefined && normalized.amount !== null;
    const hasDuration =
      normalized.duration_blocks !== undefined && normalized.duration_blocks !== null;
    if ((hasOwner || hasAmount || hasDuration) && !(hasOwner && hasAmount && hasDuration)) {
      throw new TypeError(
        `${context} must include owner, amount, and duration_blocks when providing lock hints`,
      );
    }
    return stringifyStrictLosslessIntegerJson(normalized, context);
  }

  function validateProposeDeployContractPayload(value) {
    const context = "ProposeDeployContract";
    assertExactGovernanceObjectKeys(
      value,
      [
        "contract_address",
        "code_hash_hex",
        "abi_hash_hex",
        "abi_version",
        "window",
        "mode",
        "manifest_provenance",
      ],
      ["contract_address", "code_hash_hex", "abi_hash_hex", "abi_version"],
      context,
    );
    const contractAddress = assertExactNonEmptyString(
      value.contract_address,
      `${context}.contract_address`,
    );
    parseCanonicalContractAddress(contractAddress, `${context}.contract_address`);
    value.code_hash_hex = normalizeGovernanceHex32(
      value.code_hash_hex,
      `${context}.code_hash_hex`,
    );
    value.abi_hash_hex = normalizeGovernanceHex32(
      value.abi_hash_hex,
      `${context}.abi_hash_hex`,
    );
    if (value.abi_version !== "1") {
      throw new TypeError(`${context}.abi_version must be exactly '1'`);
    }
    if (value.window !== undefined && value.window !== null) {
      value.window = normalizeGovernanceWindowValue(value.window, `${context}.window`);
    }
    if (value.mode !== undefined && value.mode !== null) {
      encodeVotingModeValue(value.mode, `${context}.mode`);
    }
    if (value.manifest_provenance !== undefined && value.manifest_provenance !== null) {
      value.manifest_provenance = decodeManifestProvenanceValue(
        encodeManifestProvenanceValue(
          value.manifest_provenance,
          `${context}.manifest_provenance`,
        ),
        `${context}.manifest_provenance`,
      );
    }
    return value;
  }

  function validateCastZkBallotPayload(value) {
    const context = "CastZkBallot";
    assertExactGovernanceObjectKeys(
      value,
      ["election_id", "proof_b64", "public_inputs_json"],
      ["election_id", "proof_b64", "public_inputs_json"],
      context,
    );
    assertCanonicalGovernanceSelectorV1(
      value.election_id,
      `${context}.election_id`,
    );
    decodeExactStandardBase64(value.proof_b64, `${context}.proof_b64`);
    value.public_inputs_json = normalizeGovernanceZkPublicInputsJson(
      value.public_inputs_json,
      `${context}.public_inputs_json`,
    );
    return value;
  }

  function validateGovernanceInstructionBoundary(instruction) {
    validateGovernanceInstructionSelectors(instruction);
    if (!isStrictGovernanceInstructionCandidate(instruction)) {
      return;
    }
    rejectGovernancePrivateKeyFieldsDeep(instruction, "governance instruction");
    if (Object.prototype.hasOwnProperty.call(instruction, "ProposeDeployContract")) {
      assertOnlyObjectKeys(instruction, ["ProposeDeployContract"], "governance instruction");
      validateProposeDeployContractPayload(instruction.ProposeDeployContract);
      return;
    }
    assertOnlyObjectKeys(instruction, ["CastZkBallot"], "governance instruction");
    validateCastZkBallotPayload(instruction.CastZkBallot);
  }

  return validateGovernanceInstructionBoundary;
}
