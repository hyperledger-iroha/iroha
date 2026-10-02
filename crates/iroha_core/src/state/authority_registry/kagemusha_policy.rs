//! Typed semantic preimage for the configured Kagemusha verifier runtime.
//!
//! This is a diagnostic projection, never a complete-State commitment. Local
//! RejectAll versus loaded/stale artifacts is availability, not canonical authority.
//! The complete governed release registry is already an original World value.
//! Reload authenticates the exact governed set and original State head; monetary
//! execution checks that same registry and locally defers missing/stale artifacts
//! before effects. Publication checks local cache integrity independently of the
//! certified registry transition, so a successor does not depend on local preload.
//! TODO: permissioned rotation/retirement and original complete-State capture,
//! derived checks, history publication and recovery remain before Required closes.

use std::{any::Any, sync::Arc};

use crate::smartcontracts::isi::kagemusha::{
    KagemushaV1RuntimeVerifier, KagemushaVerifierAuthorityV1, runtime_verifier_authority,
};

fn from_runtime(
    value: &Arc<dyn KagemushaV1RuntimeVerifier>,
) -> Result<KagemushaVerifierAuthorityV1, String> {
    let verifier: &dyn Any = value.as_ref();
    runtime_verifier_authority(verifier)
}

pub(super) fn canonical_preimage(
    value: &Arc<dyn KagemushaV1RuntimeVerifier>,
) -> Result<Vec<u8>, String> {
    norito::encode_canonical(&from_runtime(value)?)
        .map_err(|error| format!("failed to encode Kagemusha verifier authority: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::kagemusha::RejectAllKagemushaV1RuntimeVerifier;

    #[test]
    fn reject_all_preimage_is_stable_and_does_not_include_pointer_identity() {
        let first: Arc<dyn KagemushaV1RuntimeVerifier> =
            Arc::new(RejectAllKagemushaV1RuntimeVerifier);
        let second: Arc<dyn KagemushaV1RuntimeVerifier> =
            Arc::new(RejectAllKagemushaV1RuntimeVerifier);
        assert_eq!(
            canonical_preimage(&first).unwrap(),
            canonical_preimage(&second).unwrap()
        );
    }
}
