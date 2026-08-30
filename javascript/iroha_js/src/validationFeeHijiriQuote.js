import { Buffer } from "node:buffer";

import {
  defaultNativeRuntime,
  resolveNativeRuntimeBinding,
} from "./nativeRuntime.js";
import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";

export const VALIDATION_FEE_HIJIRI_QUOTE_PATH =
  "/v1/validation-fee/hijiri/quote";
export const VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA =
  "iroha.torii.v1.validation_fee.hijiri_quote.response";
export const VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE =
  "EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED";
export const VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES = 4 * 1024;
export const VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES = 64 * 1024;
export const VALIDATION_FEE_HIJIRI_QUOTE_MAX_TRANSFERS = 100_000;
export const VALIDATION_FEE_HIJIRI_QUOTE_REQUIRED_BRIDGE_ABI_VERSION = 23;

const PROJECTION_KEYS = Object.freeze([
  "accountId",
  "accountRiskDigest",
  "accountRiskRevision",
  "activePolicyHash",
  "activePolicyVersion",
  "adjustedPerTransferFeeMinorUnits",
  "aggregateAdjustedFeeMinorUnits",
  "aggregateBaseFeeMinorUnits",
  "assurance",
  "basePerTransferFeeMinorUnits",
  "defaultAccountRiskQ16",
  "effectiveAccountRiskQ16",
  "evaluatedStateHeight",
  "feeAssetDefinitionId",
  "feeMultiplierQ16",
  "feeScale",
  "hijiriFeeQuoteHash",
  "hijiriParametersDigest",
  "hijiriParametersRevision",
  "hijiriParametersVersion",
  "qualifyingTransferCount",
  "quotedExecutionHeight",
  "schema",
  "treasuryAccountId",
  "version",
]);
const LOWER_HEX_32 = /^[0-9a-f]{64}$/u;
const CANONICAL_POSITIVE_DECIMAL = /^[1-9][0-9]*$/u;
const TYPED_ARRAY_BYTE_LENGTH = Object.getOwnPropertyDescriptor(
  Object.getPrototypeOf(Uint8Array.prototype),
  "byteLength",
).get;

function record(value, label) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    (Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null)
  ) {
    throw new TypeError(`${label} must be a plain object`);
  }
  return value;
}

function exactKeys(value, expected, label) {
  const keys = Object.keys(value).sort();
  if (
    keys.length !== expected.length ||
    keys.some((key, index) => key !== expected[index])
  ) {
    throw new TypeError(`${label} has an unexpected field set`);
  }
}

function requireU32(value, label) {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new TypeError(`${label} must be an unsigned 32-bit integer`);
  }
  return value;
}

function requireBoundedBytes(value, maximum, label) {
  if (!(value instanceof Uint8Array)) {
    throw new TypeError(`${label} must be a Uint8Array`);
  }
  const length = TYPED_ARRAY_BYTE_LENGTH.call(value);
  if (length === 0 || length > maximum) {
    throw new TypeError(`${label} must contain 1..${maximum} bytes`);
  }
}

function nativeBinding(nativeRuntime) {
  const native = resolveNativeRuntimeBinding(nativeRuntime);
  if (
    typeof native?.connectNoritoBridgeAbiVersion !== "function" ||
    native.connectNoritoBridgeAbiVersion() !==
      VALIDATION_FEE_HIJIRI_QUOTE_REQUIRED_BRIDGE_ABI_VERSION ||
    typeof native?.validationFeeHijiriQuoteRequestV1 !== "function" ||
    typeof native?.validationFeeVerifyHijiriQuoteResponseV1 !== "function"
  ) {
    throw new Error(
      `native binding lacks the ABI ${VALIDATION_FEE_HIJIRI_QUOTE_REQUIRED_BRIDGE_ABI_VERSION} Hijiri validation-fee quote codec`,
    );
  }
  return native;
}

/** Encode one exact V1 native-Norito Hijiri quote request. */
function encodeValidationFeeHijiriQuoteRequestV1WithRuntime(
  nativeRuntime,
  accountId,
  qualifyingTransferCount,
) {
  if (typeof accountId !== "string" || accountId.length === 0) {
    throw new TypeError("accountId must be a non-empty canonical I105 account id");
  }
  const count = requireU32(
    qualifyingTransferCount,
    "qualifyingTransferCount",
  );
  if (count === 0 || count > VALIDATION_FEE_HIJIRI_QUOTE_MAX_TRANSFERS) {
    throw new TypeError(
      `qualifyingTransferCount must be in 1..${VALIDATION_FEE_HIJIRI_QUOTE_MAX_TRANSFERS}`,
    );
  }
  const native = nativeBinding(nativeRuntime);
  const encoded = native.validationFeeHijiriQuoteRequestV1(
    accountId,
    count,
  );
  const request = Buffer.from(encoded ?? []);
  if (
    request.length === 0 ||
    request.length > VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES
  ) {
    throw new Error("native Hijiri quote request encoder returned invalid bytes");
  }
  return request;
}

/**
 * Canonically decode, validate, and bind one native-Norito response to the
 * exact request archive. All structural and arithmetic checks run natively.
 */
function verifyValidationFeeHijiriQuoteResponseV1WithRuntime(
  nativeRuntime,
  responseNorito,
  requestNorito,
) {
  requireBoundedBytes(
    responseNorito,
    VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES,
    "responseNorito",
  );
  requireBoundedBytes(
    requestNorito,
    VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES,
    "requestNorito",
  );
  const response = Buffer.from(responseNorito);
  const request = Buffer.from(requestNorito);
  const native = nativeBinding(nativeRuntime);
  const json = native.validationFeeVerifyHijiriQuoteResponseV1(
    response,
    request,
  );
  if (typeof json !== "string" || json.length === 0) {
    throw new Error("native Hijiri quote verifier returned no projection");
  }
  if (
    Buffer.byteLength(json, "utf8") >
    VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES
  ) {
    throw new Error("native Hijiri quote projection exceeds its byte bound");
  }
  const projection = record(
    parseStrictLosslessIntegerJson(json, "Hijiri validation-fee quote projection"),
    "Hijiri validation-fee quote projection",
  );
  exactKeys(
    projection,
    PROJECTION_KEYS,
    "Hijiri validation-fee quote projection",
  );
  if (
    projection.schema !== VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA ||
    projection.version !== 1 ||
    projection.assurance !== VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE
  ) {
    throw new TypeError("Hijiri quote projection markers are invalid");
  }
  for (const [field, value] of [
    ["evaluatedStateHeight", projection.evaluatedStateHeight],
    ["quotedExecutionHeight", projection.quotedExecutionHeight],
    ["activePolicyVersion", projection.activePolicyVersion],
    ["hijiriParametersRevision", projection.hijiriParametersRevision],
    ["basePerTransferFeeMinorUnits", projection.basePerTransferFeeMinorUnits],
    [
      "adjustedPerTransferFeeMinorUnits",
      projection.adjustedPerTransferFeeMinorUnits,
    ],
    ["aggregateBaseFeeMinorUnits", projection.aggregateBaseFeeMinorUnits],
    [
      "aggregateAdjustedFeeMinorUnits",
      projection.aggregateAdjustedFeeMinorUnits,
    ],
  ]) {
    if (typeof value !== "string" || !CANONICAL_POSITIVE_DECIMAL.test(value)) {
      throw new TypeError(`${field} must be a canonical positive decimal string`);
    }
  }
  for (const [field, value] of [
    ["activePolicyHash", projection.activePolicyHash],
    ["hijiriParametersDigest", projection.hijiriParametersDigest],
    ["hijiriFeeQuoteHash", projection.hijiriFeeQuoteHash],
  ]) {
    if (typeof value !== "string" || !LOWER_HEX_32.test(value)) {
      throw new TypeError(`${field} must be one lowercase 32-byte hash`);
    }
  }
  if (
    (projection.accountRiskRevision === null) !==
    (projection.accountRiskDigest === null)
  ) {
    throw new TypeError(
      "accountRiskRevision and accountRiskDigest must be present together",
    );
  }
  if (projection.accountRiskRevision !== null) {
    if (
      typeof projection.accountRiskRevision !== "string" ||
      !CANONICAL_POSITIVE_DECIMAL.test(projection.accountRiskRevision)
    ) {
      throw new TypeError(
        "accountRiskRevision must be a canonical positive decimal string",
      );
    }
    if (
      typeof projection.accountRiskDigest !== "string" ||
      !LOWER_HEX_32.test(projection.accountRiskDigest)
    ) {
      throw new TypeError(
        "accountRiskDigest must be one lowercase 32-byte hash",
      );
    }
  }
  requireU32(projection.defaultAccountRiskQ16, "defaultAccountRiskQ16");
  requireU32(projection.effectiveAccountRiskQ16, "effectiveAccountRiskQ16");
  requireU32(projection.feeMultiplierQ16, "feeMultiplierQ16");
  requireU32(projection.qualifyingTransferCount, "qualifyingTransferCount");
  if (
    requireU32(
      projection.hijiriParametersVersion,
      "hijiriParametersVersion",
    ) !== 1
  ) {
    throw new TypeError("hijiriParametersVersion must be exactly 1");
  }
  requireU32(projection.feeScale, "feeScale");
  return Object.freeze({ ...projection });
}

/** @internal Create Hijiri validation-fee codecs for one immutable runtime. */
export function createValidationFeeHijiriQuoteApi(nativeRuntime) {
  return Object.freeze({
    encodeValidationFeeHijiriQuoteRequestV1: (
      accountId,
      qualifyingTransferCount,
    ) =>
      encodeValidationFeeHijiriQuoteRequestV1WithRuntime(
        nativeRuntime,
        accountId,
        qualifyingTransferCount,
      ),
    verifyValidationFeeHijiriQuoteResponseV1: (
      responseNorito,
      requestNorito,
    ) =>
      verifyValidationFeeHijiriQuoteResponseV1WithRuntime(
        nativeRuntime,
        responseNorito,
        requestNorito,
      ),
  });
}

const DEFAULT_VALIDATION_FEE_HIJIRI_QUOTE_API =
  createValidationFeeHijiriQuoteApi(defaultNativeRuntime);

/** Encode one exact V1 native-Norito Hijiri quote request. */
export function encodeValidationFeeHijiriQuoteRequestV1(
  accountId,
  qualifyingTransferCount,
) {
  return DEFAULT_VALIDATION_FEE_HIJIRI_QUOTE_API
    .encodeValidationFeeHijiriQuoteRequestV1(
      accountId,
      qualifyingTransferCount,
    );
}

/**
 * Canonically decode, validate, and bind one native-Norito response to the
 * exact request archive. All structural and arithmetic checks run natively.
 */
export function verifyValidationFeeHijiriQuoteResponseV1(
  responseNorito,
  requestNorito,
) {
  return DEFAULT_VALIDATION_FEE_HIJIRI_QUOTE_API
    .verifyValidationFeeHijiriQuoteResponseV1(
      responseNorito,
      requestNorito,
    );
}
