import {
  createValidationError,
  ValidationErrorCode,
} from "./validationError.js";

export function assertNonBlankString(value, context) {
  if (typeof value !== "string" || value.trim() === "") {
    throw createValidationError(
      ValidationErrorCode.INVALID_OBJECT,
      `${context} must be a non-empty string`,
      context,
    );
  }
  return value.trim();
}

export function normalizeTransactionStatusScope(value, context) {
  if (value === undefined) {
    return "global";
  }
  if (value === "local" || value === "global") {
    return value;
  }
  throw createValidationError(
    ValidationErrorCode.INVALID_OBJECT,
    `${context} must be one of: local, global`,
    context,
  );
}

export function readHeaderValue(headers, name) {
  if (!headers) {
    return null;
  }
  if (typeof headers.get === "function") {
    return headers.get(name) ?? headers.get(name.toLowerCase());
  }
  const lower = name.toLowerCase();
  if (headers instanceof Map) {
    return headers.get(name) ?? headers.get(lower);
  }
  if (typeof headers === "object") {
    const direct = headers[name];
    if (typeof direct === "string") {
      return direct;
    }
    const lowerValue = headers[lower];
    if (typeof lowerValue === "string") {
      return lowerValue;
    }
  }
  return null;
}
