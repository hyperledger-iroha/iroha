import {
  createValidationError,
  ValidationErrorCode,
} from "./validationError.js";

function normalizeErrorPath(context) {
  return typeof context === "string" ? context.replace(/\s+/gu, ".") : context;
}

export function normalizeIsoWeekLabel(input, name, Client) {
  const path = normalizeErrorPath(name);
  if (typeof input === "string") {
    const trimmed = input.trim();
    if (!trimmed) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must be a non-empty ISO week string`,
        path,
      );
    }
    if (!/^\d{4}-W\d{2}$/u.test(trimmed)) {
      throw createValidationError(
        ValidationErrorCode.INVALID_STRING,
        `${name} must match YYYY-Www format`,
        path,
      );
    }
    return trimmed;
  }
  if (input && typeof input === "object") {
    const year = Client._normalizeUnsignedInteger(input.year, `${name}.year`, {
      allowZero: false,
    });
    const week = Client._normalizeUnsignedInteger(input.week, `${name}.week`, {
      allowZero: false,
    });
    if (week < 1 || week > 53) {
      throw createValidationError(
        ValidationErrorCode.VALUE_OUT_OF_RANGE,
        `${name}.week must be between 1 and 53`,
        normalizeErrorPath(`${name}.week`),
      );
    }
    const weekLabel = week.toString().padStart(2, "0");
    const yearLabel = year.toString().padStart(4, "0");
    return `${yearLabel}-W${weekLabel}`;
  }
  throw createValidationError(
    ValidationErrorCode.INVALID_OBJECT,
    `${name} must be an ISO week string or {year, week} object`,
    path,
  );
}
