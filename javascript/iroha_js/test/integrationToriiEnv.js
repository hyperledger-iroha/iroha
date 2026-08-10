/** Parse the permissive boolean literals used by the live Torii suite. */
export function parseBooleanEnv(value) {
  if (!value) {
    return false;
  }
  const normalized = value.trim().toLowerCase();
  return normalized !== "0" && normalized !== "false";
}

/** Normalize optional string settings without accepting blank values. */
export function normalizeIntegrationString(value) {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}
