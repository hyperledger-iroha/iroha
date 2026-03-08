/**
 * Structured validation errors used by the JS SDK. These extend {@link TypeError}
 * while exposing a stable code + path field for diagnostics.
 */
export const ValidationErrorCode = Object.freeze({
  INVALID_STRING: "ERR_INVALID_STRING",
  INVALID_HEX: "ERR_INVALID_HEX",
  INVALID_MULTIHASH: "ERR_INVALID_MULTIHASH",
  INVALID_ACCOUNT_ID: "ERR_INVALID_ACCOUNT_ID",
  INVALID_ASSET_ID: "ERR_INVALID_ASSET_ID",
  INVALID_IBAN: "ERR_INVALID_IBAN",
  INVALID_OBJECT: "ERR_INVALID_OBJECT",
  INVALID_METADATA: "ERR_INVALID_METADATA",
  INVALID_JSON_VALUE: "ERR_INVALID_JSON_VALUE",
  INVALID_NUMERIC: "ERR_INVALID_NUMERIC",
  VALUE_OUT_OF_RANGE: "ERR_VALUE_OUT_OF_RANGE",
});

export class ValidationError extends TypeError {
  constructor(code, message, options = {}) {
    super(message);
    this.name = "ValidationError";
    this.code = code;
    this.path = options.path ?? null;
    if (options.cause !== undefined) {
      this.cause = options.cause;
    }
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, ValidationError);
    }
  }
}

export function createValidationError(code, message, path, cause) {
  return new ValidationError(code, message, { path, cause });
}
