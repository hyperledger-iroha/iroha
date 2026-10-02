import { normalizeAccountId } from "./normalizers.js";

const FIELDS = ["account_id", "retail_enrolled", "billing_month_start_ms", "policy_revision",
  "payments_used_before", "qualifying_payments", "fee_minor", "state_commitment", "intent_hash", "expires_at_ms"];
const U64_MAX = 0xffff_ffff_ffff_ffffn;

/** One exact native assessment, defensively snapshotted before asynchronous signing or transport. */
export function normalizeRetailFeeAssessment(value) {
  if (!value || typeof value !== "object" || Array.isArray(value) ||
      Object.keys(value).sort().join("\0") !== [...FIELDS].sort().join("\0")) {
    throw new TypeError("retail fee assessment must contain exactly the native fields");
  }
  const result = {};
  result.account_id = normalizeAccountId(value.account_id, "retail fee assessment.account_id");
  if (result.account_id !== value.account_id) throw new TypeError("retail fee assessment requires canonical account identity");
  if (typeof value.retail_enrolled !== "boolean") throw new TypeError("retail_enrolled must be boolean");
  result.retail_enrolled = value.retail_enrolled;
  for (const key of ["billing_month_start_ms", "policy_revision", "payments_used_before",
    "qualifying_payments", "fee_minor", "expires_at_ms"]) {
    const input = value[key];
    if (!(typeof input === "bigint" || typeof input === "number" && Number.isSafeInteger(input))) {
      throw new TypeError(`retail fee assessment.${key} must be a lossless unsigned integer`);
    }
    const integer = BigInt(input);
    if (integer < 0n || integer > U64_MAX) throw new TypeError(`retail fee assessment.${key} exceeds u64`);
    result[key] = integer <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(integer) : integer;
  }
  if (BigInt(result.policy_revision) === 0n ||
      BigInt(result.qualifying_payments) > 1_000n ||
      BigInt(result.payments_used_before) + BigInt(result.qualifying_payments) > U64_MAX ||
      BigInt(result.qualifying_payments) === 0n && BigInt(result.fee_minor) !== 0n) {
    throw new TypeError("retail fee assessment has an invalid revision, payment count, or charge");
  }
  const month = new Date(Number(BigInt(result.billing_month_start_ms) + 39_600_000n));
  const expires = BigInt(result.expires_at_ms);
  const next = new Date(month.getTime());
  next.setUTCMonth(next.getUTCMonth() + 1);
  if (!Number.isFinite(month.getTime()) || !Number.isFinite(next.getTime()) ||
      month.getUTCDate() !== 1 || month.getUTCHours() !== 0 || month.getUTCMinutes() !== 0 ||
      month.getUTCSeconds() !== 0 || month.getUTCMilliseconds() !== 0 ||
      expires <= BigInt(result.billing_month_start_ms) || expires > BigInt(next.getTime()) - 39_600_000n) {
    throw new TypeError("retail fee assessment must expire within its Honiara billing month");
  }
  for (const key of ["state_commitment", "intent_hash"]) {
    const hex = value[key];
    if (typeof hex !== "string" || !/^[0-9A-F]{64}$/.test(hex) || /^0+$/.test(hex) ||
        (Number.parseInt(hex.slice(-2), 16) & 1) === 0) {
      throw new TypeError(`retail fee assessment.${key} must be a nonzero canonical uppercase Iroha hash`);
    }
    result[key] = hex;
  }
  return Object.freeze(result);
}

/** Reject superseded fee fields instead of silently dropping signature-bound information. */
export function rejectUnknownRetailFeeFields(source, context) {
  for (const key of Object.keys(source)) {
    if ((key.startsWith("validationFee") || key.startsWith("validation_fee")) &&
        key !== "validation_fee_assessment") {
      throw new TypeError(`${context} contains unsupported fee field ${key}`);
    }
  }
}
