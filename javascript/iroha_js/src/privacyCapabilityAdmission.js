const exact12CapabilityAdmissions = new WeakMap();

/** @internal Bind one native-validated manifest to its closed admission callback. */
export function bindPrivacyExact12CapabilityAdmissionV1(manifest, admit) {
  if (
    (typeof manifest !== "object" && typeof manifest !== "function")
    || manifest === null
    || typeof admit !== "function"
    || exact12CapabilityAdmissions.has(manifest)
  ) {
    throw new TypeError("invalid Exact12 capability admission binding");
  }
  exact12CapabilityAdmissions.set(manifest, admit);
}

/** Require a tuple from a native-validated canonical Exact12 manifest. */
export function requirePrivacyExact12CapabilityTupleV1(manifest, protocolId) {
  const admit = exact12CapabilityAdmissions.get(manifest);
  if (!admit) {
    throw new TypeError(
      "Exact12 capability admission requires a native-validated PrivacyExact12CapabilityManifestV1",
    );
  }
  return admit(protocolId);
}

/** Explicit transaction-construction admission guard. */
export function requirePrivacyExact12CapabilityAdmissionV1(manifest, protocolId) {
  return requirePrivacyExact12CapabilityTupleV1(manifest, protocolId);
}
