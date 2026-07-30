/**
 * Private package capability used by the optional privacy-capabilities entry
 * point. A local symbol keeps the transport hook unavailable to ordinary
 * duck-typed objects without pulling the policy parser into base client graphs.
 */
export const privacyCapabilityTransportV1 = Symbol(
  "@iroha/iroha-js/privacy-capability-transport-v1",
);
