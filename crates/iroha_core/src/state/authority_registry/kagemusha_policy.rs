//! Typed semantic preimage for the configured Kagemusha verifier runtime.
//!
//! This is a checked operational projection. Daemon startup currently loads
//! release artifacts from configured local files; snapshot recovery starts with
//! a RejectAll verifier. The typed World registry now persists expected release
//! identities and lifecycle. Installation, startup, and ordinary State commit
//! check an authenticated runtime against that registry. The built-in reject-all
//! runtime remains safe across certified signer-policy installation, standby
//! installation, and first activation. Rotation, retirement, multi-release
//! reload, and a complete root publisher still must exist before this
//! projection can enter a root.

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
