export const VALIDATION_FEE_DS_SCALE = 2;
export const VALIDATION_FEE_POLICY_SCHEMA_VERSION = 1;

const LOWER_HEX_32 = /^[0-9a-f]{64}$/u;
const CANONICAL_QUANTITY = /^(?:0|[1-9][0-9]*)(?:\.[0-9]*[1-9])?$/u;
const CANONICAL_U64 = /^(?:0|[1-9][0-9]*)$/u;
const U64_MAX = 0xffff_ffff_ffff_ffffn;
const POLICY_KEYS = Object.freeze([
  "chain_id",
  "charging_mode",
  "ds_asset_id",
  "ds_scale",
  "effective_from_height",
  "exemption_classes",
  "expires_after_height",
  "fee",
  "genesis_hash",
  "policy_version",
  "previous_policy_hash",
  "schema_version",
  "treasury_account_id",
  "treasury_payout_binding",
]);
const CHARGING_MODE_KEYS = Object.freeze(["charging_mode", "value"]);
const CHARGING_MODES = new Set([
  "DISABLED",
  "PER_QUALIFYING_TRANSFER_INSTRUCTION",
]);

function plainRecord(value, label) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.getPrototypeOf(value) !== Object.prototype
  ) {
    throw new TypeError(`${label} must be a plain object`);
  }
  return value;
}

function requireExactKeys(value, expected, label) {
  const actual = Object.keys(value).sort();
  if (
    actual.length !== expected.length ||
    actual.some((key, index) => key !== expected[index])
  ) {
    throw new TypeError(`${label} must contain exactly ${expected.join(", ")}`);
  }
}

function canonicalText(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 1024 ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new TypeError(`${label} must be non-empty canonical text`);
  }
  return value;
}

function hash32(value, label) {
  if (typeof value !== "string" || !LOWER_HEX_32.test(value) || /^0+$/u.test(value)) {
    throw new TypeError(`${label} must be a non-zero lowercase 32-byte hash`);
  }
  return value;
}

function u64Text(value, label, { positive = false } = {}) {
  let text;
  if (typeof value === "bigint") {
    text = value.toString();
  } else if (typeof value === "number" && Number.isSafeInteger(value)) {
    text = String(value);
  } else if (typeof value === "string") {
    text = value;
  } else {
    throw new TypeError(`${label} must be a uint64`);
  }
  if (!CANONICAL_U64.test(text)) {
    throw new TypeError(`${label} must use canonical uint64 text`);
  }
  const parsed = BigInt(text);
  if (parsed > U64_MAX || (positive && parsed === 0n)) {
    throw new TypeError(`${label} is outside its uint64 range`);
  }
  return text;
}

function normalizeChargingMode(value) {
  const mode = plainRecord(value, "validation-fee policy.charging_mode");
  requireExactKeys(
    mode,
    CHARGING_MODE_KEYS,
    "validation-fee policy.charging_mode",
  );
  if (!CHARGING_MODES.has(mode.charging_mode) || mode.value !== null) {
    throw new TypeError("validation-fee policy.charging_mode is unsupported");
  }
  return Object.freeze({
    charging_mode: mode.charging_mode,
    value: null,
  });
}

function normalizeExemptionClasses(value) {
  if (!Array.isArray(value)) {
    throw new TypeError("validation-fee policy.exemption_classes must be an array");
  }
  const seen = new Set();
  const normalized = value.map((entry) => {
    const exemption = canonicalText(
      entry,
      "validation-fee policy.exemption_classes entry",
    );
    if (exemption !== "TREASURY_PAYOUT" || seen.has(exemption)) {
      throw new TypeError(
        "validation-fee policy.exemption_classes contains an unsupported or duplicate entry",
      );
    }
    seen.add(exemption);
    return exemption;
  });
  return Object.freeze(normalized);
}

function cloneAndFreezeJson(value, label, depth = 0, counter = { value: 0 }) {
  counter.value += 1;
  if (counter.value > 10_000 || depth > 32) {
    throw new TypeError(`${label} exceeds the bounded JSON object graph`);
  }
  if (
    value === null ||
    typeof value === "boolean" ||
    typeof value === "string" ||
    (typeof value === "number" && Number.isFinite(value))
  ) {
    return value;
  }
  if (Array.isArray(value)) {
    return Object.freeze(
      value.map((entry) => cloneAndFreezeJson(entry, label, depth + 1, counter)),
    );
  }
  const record = plainRecord(value, label);
  const copy = {};
  for (const key of Object.keys(record).sort()) {
    copy[key] = cloneAndFreezeJson(record[key], label, depth + 1, counter);
  }
  return Object.freeze(copy);
}

/**
 * Normalize the exact native `ValidationFeePolicyV1` JSON contract.
 *
 * Unknown and legacy fields fail closed. Integer values are projected as
 * canonical decimal strings so the result remains lossless across JS runtimes.
 */
export function normalizeValidationFeePolicyV1(value) {
  const policy = plainRecord(value, "validation-fee policy");
  requireExactKeys(policy, POLICY_KEYS, "validation-fee policy");
  if (policy.schema_version !== VALIDATION_FEE_POLICY_SCHEMA_VERSION) {
    throw new TypeError("validation-fee policy.schema_version is unsupported");
  }
  if (policy.ds_scale !== VALIDATION_FEE_DS_SCALE) {
    throw new TypeError(
      `validation-fee policy.ds_scale must be ${VALIDATION_FEE_DS_SCALE}`,
    );
  }
  canonicalText(policy.ds_asset_id, "validation-fee policy.ds_asset_id");
  if (
    typeof policy.fee !== "string" ||
    !CANONICAL_QUANTITY.test(policy.fee)
  ) {
    throw new TypeError("validation-fee policy.fee must be a canonical quantity");
  }
  const policyVersion = u64Text(
    policy.policy_version,
    "validation-fee policy.policy_version",
    { positive: true },
  );
  const previousPolicyHash =
    policy.previous_policy_hash === null
      ? null
      : hash32(
          policy.previous_policy_hash,
          "validation-fee policy.previous_policy_hash",
        );
  if (
    (policyVersion === "1" && previousPolicyHash !== null) ||
    (policyVersion !== "1" && previousPolicyHash === null)
  ) {
    throw new TypeError(
      "validation-fee policy.previous_policy_hash does not match policy_version",
    );
  }
  const effectiveFromHeight = u64Text(
    policy.effective_from_height,
    "validation-fee policy.effective_from_height",
  );
  const expiresAfterHeight =
    policy.expires_after_height === null
      ? null
      : u64Text(
          policy.expires_after_height,
          "validation-fee policy.expires_after_height",
        );
  if (
    expiresAfterHeight !== null &&
    BigInt(expiresAfterHeight) <= BigInt(effectiveFromHeight)
  ) {
    throw new TypeError("validation-fee policy validity window is invalid");
  }
  const chargingMode = normalizeChargingMode(policy.charging_mode);
  const exemptionClasses = normalizeExemptionClasses(policy.exemption_classes);
  const treasuryPayoutBinding =
    policy.treasury_payout_binding === null
      ? null
      : cloneAndFreezeJson(
          policy.treasury_payout_binding,
          "validation-fee policy.treasury_payout_binding",
        );
  if (
    (chargingMode.charging_mode === "DISABLED" &&
      (policy.fee !== "0" ||
        exemptionClasses.length !== 0 ||
        treasuryPayoutBinding !== null)) ||
    (exemptionClasses.includes("TREASURY_PAYOUT") !==
      (treasuryPayoutBinding !== null))
  ) {
    throw new TypeError("validation-fee policy payout and charging mode are inconsistent");
  }
  return Object.freeze({
    schema_version: policy.schema_version,
    chain_id: canonicalText(policy.chain_id, "validation-fee policy.chain_id"),
    genesis_hash: hash32(policy.genesis_hash, "validation-fee policy.genesis_hash"),
    policy_version: policyVersion,
    previous_policy_hash: previousPolicyHash,
    ds_asset_id: policy.ds_asset_id,
    ds_scale: policy.ds_scale,
    fee: policy.fee,
    treasury_account_id: canonicalText(
      policy.treasury_account_id,
      "validation-fee policy.treasury_account_id",
    ),
    charging_mode: chargingMode,
    effective_from_height: effectiveFromHeight,
    expires_after_height: expiresAfterHeight,
    exemption_classes: exemptionClasses,
    treasury_payout_binding: treasuryPayoutBinding,
  });
}
