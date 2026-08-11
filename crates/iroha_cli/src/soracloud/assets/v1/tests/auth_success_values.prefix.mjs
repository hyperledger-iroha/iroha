
function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const principal = "1111111111111111111111111111111111111111111111111111111111111111";
const capabilityMap = parseCapabilityMap(JSON.stringify({
  [principal]: [
    "pii.records.delete",
    " pii.records.read ",
    "pii.consent.grant",
    "pii.records.retention.sweep",
    "pii.consent.revoke",
    "pii.records.read"
  ]
}), true);

const expectedCapabilities = [
  "pii.consent.grant",
  "pii.consent.revoke",
  "pii.records.delete",
  "pii.records.read",
  "pii.records.retention.sweep"
];
assert(
  JSON.stringify(capabilityMap.get(principal)) === JSON.stringify(expectedCapabilities),
  `capabilities must be trimmed, sorted, and deduplicated: ${JSON.stringify([...capabilityMap])}`
);

assert(parseBooleanEnv("FLAG", "yes", false) === true, "yes should parse true");
assert(parseBooleanEnv("FLAG", "on", false) === true, "on should parse true");
assert(parseBooleanEnv("FLAG", "off", true) === false, "off should parse false");
assert(parseBooleanEnv("FLAG", "", true) === true, "empty boolean should use fallback");
assert(parsePositiveIntEnv("SESSION", "86400", 900, 60, 86400) === 86400, "max session TTL should parse");
assert(parsePositiveIntEnv("CHALLENGE", "5", 120, 5, 900) === 5, "min challenge TTL should parse");
assert(parsePublicOrigin("") === "", "empty PUBLIC_BASE_URL should parse to empty origin");
assert(parsePublicOrigin("https://example.test/path?q=1") === "https://example.test", "public origin should canonicalize URL origin");
