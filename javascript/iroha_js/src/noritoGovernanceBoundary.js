import {
  JS_TYPE_BIGINT,
  JS_TYPE_NUMBER,
  JS_TYPE_OBJECT,
  JS_TYPE_STRING,
} from "./commonLiterals.js";
import { parseCanonicalContractAddress } from "./contractAddress.js";
import { isCanonicalGovernanceSelectorV1 } from "./governanceSelector.js";
import { ensureCanonicalAccountId } from "./normalizers.js";
import { NumericV1 } from "./numericV1.js";
import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";

const UINT64_MASK = 0xffff_ffff_ffff_ffffn;
const hasOwn = (value, key) => Object.prototype.hasOwnProperty.call(value, key);
const PROPOSE_DEPLOY_CONTRACT = "ProposeDeployContract";
const CAST_ZK_BALLOT = "CastZkBallot";
const GOVERNANCE_INSTRUCTION_CONTEXT = "governance instruction";
const ELECTION_ID_FIELD = "election_id";
const PUBLIC_INPUTS_JSON_FIELD = "public_inputs_json";
const CONTRACT_ADDRESS_FIELD = "contract_address";
const DURATION_BLOCKS_FIELD = "duration_blocks";
const CODE_HASH_FIELD = "code_hash";
const REFERENDUM_ID_FIELD = "referendum_id";
const ABI_HASH_FIELD = "abi_hash";
const ABI_VERSION_FIELD = "abi_version";
const PROOF_BASE64_FIELD = "proof_b64";
const GOVERNANCE_PRIVATE_KEY_FIELD_RE =
  /^private(?:_key(?:_(?:hex|bytes|seed|multihash|algorithm))?|Key(?:Hex|Bytes|Seed|Multihash|Algorithm)?)$/u;
const GOVERNANCE_ZK_PUBLIC_INPUT_FIELDS = Object.freeze([
  "root_hint",
  "owner",
  "amount",
  DURATION_BLOCKS_FIELD,
  "direction",
  "nullifier",
]);

/**
 * Parse the closed governance JSON profile without rounding integer tokens.
 * Safe integers remain numbers and larger integers remain bigint values.
 */
export function parseStrictGovernanceInstructionJson(text, context) {
  let parsed;
  try {
    parsed = parseStrictLosslessIntegerJson(text, context);
  } catch (error) {
    const remapped = error?.message?.replace(
      /(?:high surrogate must be followed[^:]*|unterminated high surrogate)/u,
      "unpaired high surrogate",
    );
    if (remapped !== error?.message) {
      throw new error.constructor(remapped);
    }
    throw error;
  }
  return parsed;
}

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
  isPlainObject,
}) {
  function isStrictGovernanceInstructionCandidate(value) {
    return (
      isPlainObject(value) &&
      (
        hasOwn(value, PROPOSE_DEPLOY_CONTRACT) ||
        hasOwn(value, CAST_ZK_BALLOT)
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
      !hasOwn(payload, field)
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
      [CAST_ZK_BALLOT, ELECTION_ID_FIELD],
      ["CastPlainBallot", REFERENDUM_ID_FIELD],
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
          ELECTION_ID_FIELD,
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
      if (candidate === null || typeof candidate !== JS_TYPE_OBJECT) {
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
        if (GOVERNANCE_PRIVATE_KEY_FIELD_RE.test(key)) {
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
      if (!hasOwn(value, field)) {
        throw new TypeError(`${context}.${field} is required`);
      }
    }
  }

  function normalizeGovernanceHex32(value, context) {
    if (typeof value !== JS_TYPE_STRING || value.length === 0) {
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
    if (typeof value === JS_TYPE_BIGINT) {
      integer = value;
    } else if (typeof value === JS_TYPE_NUMBER) {
      if (!Number.isSafeInteger(value) || value < 0) {
        throw new TypeError(`${context} must be a lossless unsigned 64-bit integer`);
      }
      integer = BigInt(value);
    } else if (typeof value === JS_TYPE_STRING) {
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

  function normalizeGovernanceQuantity(value, context) {
    if (typeof value !== JS_TYPE_STRING) {
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
    if (typeof value !== JS_TYPE_STRING || value.length === 0) {
      throw new TypeError(`${context} must be a non-empty JSON object string`);
    }
    const parsed = parseStrictGovernanceInstructionJson(value, context);
    if (!isPlainObject(parsed)) {
      throw new TypeError(`${context} must encode a JSON object`);
    }
    rejectGovernancePrivateKeyFieldsDeep(parsed, context);
    assertOnlyObjectKeys(parsed, GOVERNANCE_ZK_PUBLIC_INPUT_FIELDS, context);

    const normalized = {};
    for (const field of GOVERNANCE_ZK_PUBLIC_INPUT_FIELDS) {
      if (!hasOwn(parsed, field)) {
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
        case DURATION_BLOCKS_FIELD:
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
    const fields = [];
    for (const field of GOVERNANCE_ZK_PUBLIC_INPUT_FIELDS) {
      if (!hasOwn(normalized, field)) {
        continue;
      }
      const entry = normalized[field];
      fields.push(
        `${JSON.stringify(field)}:${
          typeof entry === JS_TYPE_BIGINT ? entry.toString(10) : JSON.stringify(entry)
        }`,
      );
    }
    return `{${fields.join(",")}}`;
  }

  function validateProposeDeployContractPayload(value) {
    const context = PROPOSE_DEPLOY_CONTRACT;
    assertExactGovernanceObjectKeys(
      value,
      [
        CONTRACT_ADDRESS_FIELD,
        CODE_HASH_FIELD,
        ABI_HASH_FIELD,
        ABI_VERSION_FIELD,
        "manifest_provenance",
      ],
      [
        CONTRACT_ADDRESS_FIELD,
        CODE_HASH_FIELD,
        ABI_HASH_FIELD,
        ABI_VERSION_FIELD,
      ],
      context,
    );
    const contractAddress = assertExactNonEmptyString(
      value.contract_address,
      `${context}.contract_address`,
    );
    parseCanonicalContractAddress(contractAddress, `${context}.contract_address`);
    value.code_hash = normalizeGovernanceHex32(
      value.code_hash,
      `${context}.code_hash`,
    );
    value.abi_hash = normalizeGovernanceHex32(
      value.abi_hash,
      `${context}.abi_hash`,
    );
    if (value.abi_version !== 1) {
      throw new TypeError(`${context}.abi_version must be exactly 1`);
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
    const context = CAST_ZK_BALLOT;
    assertExactGovernanceObjectKeys(
      value,
      [ELECTION_ID_FIELD, PROOF_BASE64_FIELD, PUBLIC_INPUTS_JSON_FIELD],
      [ELECTION_ID_FIELD, PROOF_BASE64_FIELD, PUBLIC_INPUTS_JSON_FIELD],
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
    rejectGovernancePrivateKeyFieldsDeep(instruction, GOVERNANCE_INSTRUCTION_CONTEXT);
    if (hasOwn(instruction, PROPOSE_DEPLOY_CONTRACT)) {
      assertOnlyObjectKeys(instruction, [PROPOSE_DEPLOY_CONTRACT], GOVERNANCE_INSTRUCTION_CONTEXT);
      validateProposeDeployContractPayload(instruction.ProposeDeployContract);
      return;
    }
    assertOnlyObjectKeys(instruction, [CAST_ZK_BALLOT], GOVERNANCE_INSTRUCTION_CONTEXT);
    validateCastZkBallotPayload(instruction.CastZkBallot);
  }

  return Object.freeze({
    assertCanonicalGovernanceSelectorV1,
    isStrictGovernanceInstructionCandidate,
    validateCastZkBallotPayload,
    validateGovernanceInstructionBoundary,
    validateProposeDeployContractPayload,
  });
}
