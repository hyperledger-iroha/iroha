/**
 * Private package capability used only by the legacy JSON inspection entry
 * point. A local symbol keeps the transport hook unavailable to ordinary
 * duck-typed objects without pulling the inspection parser into base clients.
 */
export const legacyPrivacyCapabilityInspectionTransportV1 = Symbol(
  "@iroha/iroha-js/legacy-privacy-capability-inspection-transport-v1",
);

/** N-API-only transport for Torii's canonical Exact12 manifest archive. */
export const privacyExact12CapabilityManifestTransportV1 = Symbol(
  "@iroha/iroha-js/privacy-exact12-capability-manifest-transport-v1",
);
