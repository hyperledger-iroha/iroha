/// Canonical public-input schema hash for the ABI-21/V4 `StepEq` verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_step_eq_public_inputs_schema_hash_v4() -> [u8; 32] {
    Hash::new(KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PUBLIC_INPUTS_SCHEMA_V4).into()
}

/// Canonical public-input schema hash for the ABI-21/V4 `StepEp` verifier record.
#[must_use]
pub fn kagemusha_recursive_spend_step_ep_public_inputs_schema_hash_v4() -> [u8; 32] {
    Hash::new(KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PUBLIC_INPUTS_SCHEMA_V4).into()
}

/// Compute the SHA-256 content identifier used by Kagemusha release files.
#[must_use]
pub fn kagemusha_recursive_spend_release_sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
