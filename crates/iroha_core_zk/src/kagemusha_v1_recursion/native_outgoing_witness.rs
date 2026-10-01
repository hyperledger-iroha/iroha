//! Rust-only intake for independently installed qualified native witness custody.
//!
//! Registration is a trusted OEM composition action, never a C/JNI/SDK input. The
//! implementation must authenticate physical unsealing under the original native
//! operation, preparation, credential, checkpoint and held lease. Returning public
//! sender replies, ciphertexts or deterministic public hashes is not an implementation.
//! The prover independently checks the exact Core claim and verifies its actual proof.

use super::{
    KagemushaGeneratedMintHashClaimV1, KagemushaRecoverySeedV1,
    KagemushaRecursiveStateGenerationWitnessV1, KagemushaTerminalAuthorizationGenerationWitnessV1,
    KagemushaTerminalAuthorizationHashClaimGenerationWitnessV1,
};
use crate::kagemusha_v1_state::{
    KagemushaAuthenticatedBootstrapProvingSelectionV1,
    KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1,
    KagemushaAuthenticatedIncomingProvingSelectionV1,
    KagemushaAuthenticatedOutgoingProvingSelectionV1,
};
use std::sync::{Arc, OnceLock};

/// Borrowed private witness consumer. No witness or seed can be returned through it.
/// The seed is the zeroizing secret unsealed by the qualified physical owner, not
/// software entropy, a password, a public digest or an opaque encrypted blob.
pub type KagemushaNativeStateWitnessConsumerV1<'a> = dyn for<'w> FnMut(
        KagemushaRecursiveStateGenerationWitnessV1<'w>,
        &'w KagemushaRecoverySeedV1,
    ) -> Result<(), String>
    + 'a;

/// Borrowed genuine postcommit terminal witness, independently bound to the retained Core.
/// Private material and the zeroizing original seed cannot escape the consumer callback.
pub type KagemushaNativeTerminalWitnessConsumerV1<'a> = dyn for<'w> FnMut(
        KagemushaTerminalAuthorizationGenerationWitnessV1<'w>,
        &'w KagemushaRecoverySeedV1,
    ) -> Result<(), String>
    + 'a;
/// Borrowed semantic witness before its exact ordered SHA claim exists.
pub type KagemushaNativeTerminalHashWitnessConsumerV1<'a> = dyn for<'w> FnMut(
        KagemushaTerminalAuthorizationHashClaimGenerationWitnessV1<'w>,
        &'w KagemushaRecoverySeedV1,
    ) -> Result<(), String>
    + 'a;

/// Independently provisioned original hardware-unseal/witness owner.
///
/// This trusted Rust implementation is installed together with the native provisioner.
/// It must retain and recheck the actual qualified release/provider originals, live
/// device owner, enrollment/key reference, original operation/preparation and exact
/// sealed streams before and after a callback. The Core selection alone cannot unseal
/// or authenticate a recovery seed. There is no wire or decoded-witness registration.
///
/// The callback must run exactly once with the immutable original witness and seed
/// under retained physical custody. It must not expose private witness copies after
/// the callback, renew an old deadline, redispatch preparation, or substitute software
/// randomness. An implementation is independently required; this trait supplies none.
pub trait KagemushaNativeOutgoingWitnessSourceV1: Send + Sync {
    /// Recheck actual qualified native originals and same selected live custody.
    fn recheck_originals(
        &self,
        selection: &KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>,
    ) -> Result<(), String>;
    /// Authenticate original unsealing and consume borrowed private witness material.
    fn with_borrowed_state_witness(
        &self,
        selection: &KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>,
        hash_claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        consume: &mut KagemushaNativeStateWitnessConsumerV1<'_>,
    ) -> Result<(), String>;
    /// Reauthenticate irreversible original op7, exact certificate/evidence/nonce and current
    /// committed Core under this original qualified physical owner. No decoded certificate suffices.
    fn recheck_committed_originals(
        &self,
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
    ) -> Result<(), String>;
    /// Consume the exact native terminal semantic witness before its SHA claim is generated.
    fn with_borrowed_terminal_hash_witness(
        &self,
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
        consume: &mut KagemushaNativeTerminalHashWitnessConsumerV1<'_>,
    ) -> Result<(), String>;
    /// Consume the actual original private terminal witness and genuine generated SHA claim.
    /// Candidate proof and complete Terminal Guard relation are separately authenticated;
    /// an ordinary precommit State Guard or paired State proof is not a Terminal Guard proof.
    fn with_borrowed_terminal_witness(
        &self,
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
        hash_claim: &KagemushaGeneratedMintHashClaimV1,
        consume: &mut KagemushaNativeTerminalWitnessConsumerV1<'_>,
    ) -> Result<(), String>;
    /// Recheck the original exclusive incoming fold, actual credential/lease, staged decryption
    /// and authenticated State/replay/terminal history custody. Public reply bytes cannot do so.
    fn recheck_incoming_originals(
        &self,
        selection: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
    ) -> Result<(), String>;
    /// Consume the real native incoming witness and zeroizing seed exactly once under physical
    /// custody. A supplied SHA claim must open the same original; no external proof pair fallback.
    fn with_borrowed_incoming_witness(
        &self,
        selection: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
        hash_claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        consume: &mut KagemushaNativeStateWitnessConsumerV1<'_>,
    ) -> Result<(), String>;
    /// Authoritative current native time retained by the independently installed physical owner.
    /// It cannot use ambient wall-clock time or renew enrollment; proving requires both
    /// original intervals still live. The prover rechecks this immediately before and after work.
    fn trusted_bootstrap_now_ms(
        &self,
        selection: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
    ) -> Result<u64, String>;
    /// Reauthenticate original enrollment/possession, qualified physical zero checkpoint,
    /// native credential/key custody and the original unsealed seed under this same source.
    fn recheck_bootstrap_originals(
        &self,
        selection: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
    ) -> Result<(), String>;
    /// Supply only the actual original zero-state witness. A generated SHA claim, if supplied,
    /// is recursively verified by the circuit and must open this same immutable witness.
    /// No software-generated zero-state proof or accepting padding helper is a substitute.
    fn with_borrowed_bootstrap_witness(
        &self,
        selection: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
        hash_claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        consume: &mut KagemushaNativeStateWitnessConsumerV1<'_>,
    ) -> Result<(), String>;
}

static SOURCE: OnceLock<Arc<dyn KagemushaNativeOutgoingWitnessSourceV1>> = OnceLock::new();

/// Install the immutable original native witness owner once from trusted Rust provisioning.
/// This neither opens a physical session nor grants a monetary lease. Registration cannot
/// replace an existing source; exact same-Arc retry is idempotent. No native wire export exists.
///
/// # Errors
/// Refuses any different source after the first successful installation.
pub fn register_kagemusha_native_outgoing_witness_source_v1(
    source: Arc<dyn KagemushaNativeOutgoingWitnessSourceV1>,
) -> Result<(), String> {
    register_in(&SOURCE, source)
}

fn register_in(
    cell: &OnceLock<Arc<dyn KagemushaNativeOutgoingWitnessSourceV1>>,
    source: Arc<dyn KagemushaNativeOutgoingWitnessSourceV1>,
) -> Result<(), String> {
    if let Some(installed) = cell.get() {
        return if Arc::ptr_eq(installed, &source) {
            Ok(())
        } else {
            Err("native witness source is already installed".to_owned())
        };
    }
    match cell.set(Arc::clone(&source)) {
        Ok(()) => Ok(()),
        Err(_)
            if cell
                .get()
                .is_some_and(|installed| Arc::ptr_eq(installed, &source)) =>
        {
            Ok(())
        }
        Err(_) => Err("native witness source installation conflicted".to_owned()),
    }
}

pub(super) fn installed_source()
-> Result<&'static Arc<dyn KagemushaNativeOutgoingWitnessSourceV1>, String> {
    SOURCE
        .get()
        .ok_or_else(|| "qualified native outgoing witness source is not installed".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct UnavailableSource;
    impl KagemushaNativeOutgoingWitnessSourceV1 for UnavailableSource {
        fn recheck_originals(
            &self,
            _: &KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>,
        ) -> Result<(), String> {
            Err("no physical custody".to_owned())
        }
        fn with_borrowed_state_witness(
            &self,
            _: &KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>,
            _: Option<&KagemushaGeneratedMintHashClaimV1>,
            _: &mut KagemushaNativeStateWitnessConsumerV1<'_>,
        ) -> Result<(), String> {
            Err("no physical custody".to_owned())
        }
        fn recheck_committed_originals(
            &self,
            _: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
        ) -> Result<(), String> {
            Err("no physical custody".to_owned())
        }
        fn with_borrowed_terminal_hash_witness(
            &self,
            _: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
            _: &mut KagemushaNativeTerminalHashWitnessConsumerV1<'_>,
        ) -> Result<(), String> {
            Err("no physical custody".to_owned())
        }
        fn with_borrowed_terminal_witness(
            &self,
            _: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
            _: &KagemushaGeneratedMintHashClaimV1,
            _: &mut KagemushaNativeTerminalWitnessConsumerV1<'_>,
        ) -> Result<(), String> {
            Err("no physical custody".to_owned())
        }
        fn recheck_incoming_originals(
            &self,
            _: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
        ) -> Result<(), String> {
            Err("original incoming custody unavailable".into())
        }
        fn with_borrowed_incoming_witness(
            &self,
            _: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
            _: Option<&KagemushaGeneratedMintHashClaimV1>,
            _: &mut KagemushaNativeStateWitnessConsumerV1<'_>,
        ) -> Result<(), String> {
            Err("original incoming witness unavailable".into())
        }
        fn trusted_bootstrap_now_ms(
            &self,
            _: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
        ) -> Result<u64, String> {
            Err("no physical custody".to_owned())
        }
        fn recheck_bootstrap_originals(
            &self,
            _: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
        ) -> Result<(), String> {
            Err("no physical custody".to_owned())
        }
        fn with_borrowed_bootstrap_witness(
            &self,
            _: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
            _: Option<&KagemushaGeneratedMintHashClaimV1>,
            _: &mut KagemushaNativeStateWitnessConsumerV1<'_>,
        ) -> Result<(), String> {
            Err("no physical custody".to_owned())
        }
    }
    #[test]
    fn registration_cannot_replace_original_source() {
        let cell = OnceLock::new();
        let first: Arc<dyn KagemushaNativeOutgoingWitnessSourceV1> = Arc::new(UnavailableSource);
        register_in(&cell, Arc::clone(&first)).unwrap();
        register_in(&cell, Arc::clone(&first)).unwrap();
        assert!(register_in(&cell, Arc::new(UnavailableSource)).is_err());
        assert!(Arc::ptr_eq(cell.get().unwrap(), &first));
    }
    #[test]
    fn concurrent_different_registration_retains_exactly_one_original() {
        let cell = Arc::new(OnceLock::new());
        let sources: Vec<Arc<dyn KagemushaNativeOutgoingWitnessSourceV1>> = (0..8)
            .map(|_| {
                let source: Arc<dyn KagemushaNativeOutgoingWitnessSourceV1> =
                    Arc::new(UnavailableSource);
                source
            })
            .collect();
        let handles: Vec<_> = sources
            .iter()
            .map(|source| {
                let cell = Arc::clone(&cell);
                let source = Arc::clone(source);
                std::thread::spawn(move || register_in(&cell, source).is_ok())
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(|installed| *installed)
                .count(),
            1
        );
        let installed = cell.get().unwrap();
        assert_eq!(
            sources
                .iter()
                .filter(|source| Arc::ptr_eq(installed, source))
                .count(),
            1
        );
    }
}
