//! Non-authorizing app boundary for one testnet lane's actual paired State proofs.
//!
//! Only a Rust host that has loaded the release-authenticated native verifier may
//! install the observation owner. The C caller supplies canonical State inputs and
//! a canonical paired proof, but cannot supply a verifier, success callback, release
//! pin, or hardware qualification. This diagnostic boundary does not open the
//! production monetary coordinator.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    ptr, slice,
    sync::{Arc, Mutex, OnceLock},
};

use iroha_core::zk::kagemusha_v1_recursion::{
    KagemushaAuthenticatedArtifactSetV1, KagemushaAuthenticatedRecursiveVerifierV1,
    KagemushaDirectoryArtifactResolverV1, KagemushaOperationV1,
    KagemushaRecursiveVerifierProfileV1, KagemushaStateRelationPublicInputsV1,
    KagemushaTestnetProofObservationOwnerV1, KagemushaTestnetStateObservationScopeV1,
};
use iroha_core::zk::kagemusha_v1_state::KagemushaStateProofReleaseV1;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1, KagemushaInternalValidationReceiptV1,
    KagemushaPairedProofV1, KagemushaReleaseAttestationV1, KagemushaReleaseAuthorityPolicyV1,
    KagemushaReleaseManifestV1,
};
use libc::{c_int, c_uchar};

use crate::{
    ERR_BUFFER_TOO_SMALL, ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1, ERR_KAGEMUSHA_V1, ERR_NULL_PTR,
};

/// Maximum canonical Norito archive for one complete public State statement.
pub const KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1: usize = 4 * 1024;
/// Maximum canonical Norito diagnostic response length.
pub const KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1: usize = 256;

/// Unsigned, non-authorizing canonical diagnostic data returned after native verification.
///
/// Private fields and no monetary trait prevent this record from being accepted as
/// an admission, payment, terminal, or hardware-capability result. The caller must
/// check the FFI status; the archive can be copied or forged and is for inspection.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "connect_norito_bridge::KagemushaTestnetStateObservationArchiveV1")]
pub struct KagemushaTestnetStateObservationArchiveV1 {
    version: u16,
    operation: KagemushaOperationV1,
    hardware_qualified: bool,
    network_id: [u8; 32],
    release_id: [u8; 32],
    release_attestation_digest: [u8; 32],
    candidate_envelope_digest: [u8; 32],
    successor_state_commitment: [u8; 32],
}

static TESTNET_STATE_OBSERVATION_OWNER_V1: OnceLock<
    Mutex<KagemushaTestnetProofObservationOwnerV1>,
> = OnceLock::new();

/// Installation failure for the single native diagnostic owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaTestnetObservationInstallErrorV1 {
    /// This process already owns a testnet observation verifier and lane.
    AlreadyInstalled,
}

/// Install an already authenticated and operator-pinned testnet proof owner once.
///
/// The Rust caller must first load the concrete native verifier and construct
/// `KagemushaTestnetProofObservationOwnerV1`. There is intentionally no C/JNI
/// installer, uninstall, callback hook, or test-only proof success switch.
///
/// # Errors
///
/// Rejects replacement of the process's existing diagnostic verifier and lane.
pub fn install_kagemusha_testnet_state_observation_owner_v1(
    owner: KagemushaTestnetProofObservationOwnerV1,
) -> Result<(), KagemushaTestnetObservationInstallErrorV1> {
    TESTNET_STATE_OBSERVATION_OWNER_V1
        .set(Mutex::new(owner))
        .map_err(|_| KagemushaTestnetObservationInstallErrorV1::AlreadyInstalled)
}

fn require_scope_release_pins(
    scope: KagemushaTestnetStateObservationScopeV1,
    authenticated_release_id: [u8; 32],
    authenticated_attestation_digest: [u8; 32],
) -> Result<(), String> {
    if authenticated_release_id != scope.release_id()
        || authenticated_attestation_digest != scope.release_attestation_digest()
    {
        return Err("KAGEMUSHA release differs from operator-pinned testnet scope".to_owned());
    }
    Ok(())
}

/// Authenticate one operator-pinned testnet release and install its native proof verifier.
///
/// `trusted_authority_policy`, `scope`, and `profile` must come from an independent
/// operator-controlled Rust configuration. Only the manifest, validation receipt,
/// threshold attestation, and content-addressed artifact directory are untrusted
/// package inputs. In particular, never read the authority policy or expected
/// release/network pins from the submitted wallet proof or from that package.
///
/// This Rust-only entrypoint does not make a stock mobile binary usable by itself:
/// TODO: package an approved signed testnet release, its 50 exact artifacts and
/// independently pinned app configuration, then invoke this at app startup.
///
/// # Errors
///
/// Rejects malformed or unsigned release inputs, mismatched operator pins,
/// substituted artifact bytes, an invalid recursive profile, or a second owner.
pub fn load_and_install_kagemusha_testnet_state_observation_owner_v1(
    manifest_archive: &[u8],
    validation_receipt_archive: &[u8],
    release_attestation_archive: &[u8],
    trusted_authority_policy: &KagemushaReleaseAuthorityPolicyV1,
    scope: KagemushaTestnetStateObservationScopeV1,
    profile: KagemushaRecursiveVerifierProfileV1,
    artifact_root: impl AsRef<Path>,
) -> Result<(), String> {
    trusted_authority_policy
        .validate()
        .map_err(|error| format!("invalid operator-pinned KAGEMUSHA authority policy: {error}"))?;
    let manifest = KagemushaReleaseManifestV1::decode_canonical_exact(manifest_archive)
        .map_err(|error| format!("invalid KAGEMUSHA release manifest: {error}"))?;
    let receipt =
        KagemushaInternalValidationReceiptV1::decode_canonical_exact(validation_receipt_archive)
            .map_err(|error| format!("invalid KAGEMUSHA release validation receipt: {error}"))?;
    let attestation =
        KagemushaReleaseAttestationV1::decode_canonical_exact(release_attestation_archive)
            .map_err(|error| format!("invalid KAGEMUSHA release attestation: {error}"))?;
    let release = manifest
        .authenticate(&receipt, trusted_authority_policy, &attestation)
        .map_err(|error| format!("unauthenticated KAGEMUSHA release: {error}"))?;
    require_scope_release_pins(scope, release.release_id(), release.attestation_digest())?;
    let state_release = KagemushaStateProofReleaseV1::from_authenticated_release(&release)
        .map_err(|error| format!("invalid KAGEMUSHA State proof release: {error}"))?;
    let resolver = KagemushaDirectoryArtifactResolverV1::new(artifact_root)
        .map_err(|error| format!("cannot open KAGEMUSHA artifact directory: {error}"))?;
    let artifacts = KagemushaAuthenticatedArtifactSetV1::new(
        &release,
        state_release.canonical_empty_effect_digest(),
        resolver,
    )
    .map_err(|error| format!("invalid KAGEMUSHA artifact set: {error}"))?;
    let mut verifier = KagemushaAuthenticatedRecursiveVerifierV1::load(&artifacts, profile)
        .map_err(|error| format!("cannot load KAGEMUSHA native verifier: {error}"))?;
    verifier
        .authorize_monetary_release(Arc::new(release))
        .map_err(|error| format!("cannot authorize KAGEMUSHA proof release: {error}"))?;
    let owner = KagemushaTestnetProofObservationOwnerV1::new(verifier, scope)
        .map_err(|error| format!("invalid KAGEMUSHA testnet observation owner: {error}"))?;
    install_kagemusha_testnet_state_observation_owner_v1(owner)
        .map_err(|_| "KAGEMUSHA testnet observation owner is already installed".to_owned())
}

/// Verify and append one real paired State proof to the installed testnet trial.
///
/// `public_inputs_archive` and `paired_proof_archive` are independently bounded,
/// exact canonical Norito frames. On success, `output` receives a bounded canonical
/// Norito `KagemushaTestnetStateObservationArchiveV1`: version 1, operation,
/// hardware-qualified false, operator-pinned network/release/attestation digests,
/// candidate envelope digest, and successor State commitment. It is an unsigned
/// observation, never an admission, payment, redemption, terminal, or hardware capability.
///
/// A missing Rust-installed owner returns device-unavailable. Any malformed,
/// substituted, forked, or invalid proof returns the KAGEMUSHA rejection code and
/// leaves the trial head unchanged. The caller must provide the full documented
/// maximum output capacity before verification can advance the trial. `output_len`
/// is zero on failure and the actual canonical archive length on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_testnet_state_proof_observe_v1(
    public_inputs_archive_ptr: *const c_uchar,
    public_inputs_archive_len: usize,
    paired_proof_archive_ptr: *const c_uchar,
    paired_proof_archive_len: usize,
    output_ptr: *mut c_uchar,
    output_capacity: usize,
    output_len: *mut usize,
) -> c_int {
    if output_len.is_null() {
        return ERR_NULL_PTR;
    }
    unsafe { *output_len = 0 };
    if public_inputs_archive_ptr.is_null()
        || paired_proof_archive_ptr.is_null()
        || output_ptr.is_null()
    {
        return ERR_NULL_PTR;
    }
    if public_inputs_archive_len == 0
        || public_inputs_archive_len > KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1
        || paired_proof_archive_len == 0
        || paired_proof_archive_len > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
    {
        return ERR_KAGEMUSHA_V1;
    }
    if output_capacity < KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1 {
        return ERR_BUFFER_TOO_SMALL;
    }
    // Snapshot both bounded foreign buffers before decoding or writing output;
    // even an aliasing caller cannot change the verified bytes mid-dispatch.
    let public_archive =
        unsafe { slice::from_raw_parts(public_inputs_archive_ptr, public_inputs_archive_len) }
            .to_vec();
    let proof_archive =
        unsafe { slice::from_raw_parts(paired_proof_archive_ptr, paired_proof_archive_len) }
            .to_vec();
    let Some(owner) = TESTNET_STATE_OBSERVATION_OWNER_V1.get() else {
        return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1;
    };
    let archive = catch_unwind(AssertUnwindSafe(|| {
        let public: KagemushaStateRelationPublicInputsV1 = norito::decode_canonical_with_limits(
            &public_archive,
            norito::canonical_decode_limits(public_archive.len()),
        )
        .map_err(|_| ())?;
        let proof: KagemushaPairedProofV1 = norito::decode_canonical_with_limits(
            &proof_archive,
            norito::canonical_decode_limits(proof_archive.len()),
        )
        .map_err(|_| ())?;
        let mut owner = owner.lock().map_err(|_| ())?;
        let scope = owner.scope();
        let record = KagemushaTestnetStateObservationArchiveV1 {
            version: 1,
            operation: public.operation,
            hardware_qualified: false,
            network_id: scope.network_id(),
            release_id: scope.release_id(),
            release_attestation_digest: scope.release_attestation_digest(),
            candidate_envelope_digest:
                KagemushaTestnetProofObservationOwnerV1::candidate_envelope_digest(&public)
                    .map_err(|_| ())?,
            successor_state_commitment: public.successor.state_commitment,
        };
        let archive = norito::encode_canonical(&record).map_err(|_| ())?;
        if archive.len() > KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1 {
            return Err(());
        }
        // Core's record_verified checks the verified observation against these
        // same pinned scope and public-input fields before it advances the head.
        owner.observe_and_advance(&public, &proof).map_err(|_| ())?;
        Ok(archive)
    }));
    let Ok(Ok(archive)) = archive else {
        return ERR_KAGEMUSHA_V1;
    };
    unsafe {
        ptr::copy_nonoverlapping(archive.as_ptr(), output_ptr, archive.len());
        *output_len = archive.len();
    };
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_entry_rejects_invalid_buffers_without_observing() {
        let input = [1_u8];
        let mut output = [0x5a_u8; KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1];
        let mut output_len = 123;
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_state_proof_observe_v1(
                    ptr::null(),
                    1,
                    input.as_ptr(),
                    1,
                    output.as_mut_ptr(),
                    output.len(),
                    &mut output_len,
                )
            },
            ERR_NULL_PTR
        );
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_state_proof_observe_v1(
                    input.as_ptr(),
                    KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1 + 1,
                    input.as_ptr(),
                    1,
                    output.as_mut_ptr(),
                    output.len(),
                    &mut output_len,
                )
            },
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_state_proof_observe_v1(
                    input.as_ptr(),
                    1,
                    input.as_ptr(),
                    1,
                    output.as_mut_ptr(),
                    output.len() - 1,
                    &mut output_len,
                )
            },
            ERR_BUFFER_TOO_SMALL
        );
        assert_eq!(
            output,
            [0x5a; KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1]
        );
        assert_eq!(output_len, 0);
    }

    #[test]
    fn stock_bridge_has_no_testnet_proof_owner_or_monetary_result() {
        let input = [1_u8];
        let mut output = [0x5a_u8; KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1];
        let mut output_len = 123;
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_state_proof_observe_v1(
                    input.as_ptr(),
                    input.len(),
                    input.as_ptr(),
                    input.len(),
                    output.as_mut_ptr(),
                    output.len(),
                    &mut output_len,
                )
            },
            ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1
        );
        assert_eq!(
            output,
            [0x5a; KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1]
        );
        assert_eq!(output_len, 0);
    }

    #[test]
    fn diagnostic_archive_is_canonical_and_declares_no_hardware_authority() {
        let record = KagemushaTestnetStateObservationArchiveV1 {
            version: 1,
            operation: KagemushaOperationV1::MintFold,
            hardware_qualified: false,
            network_id: [1; 32],
            release_id: [2; 32],
            release_attestation_digest: [3; 32],
            candidate_envelope_digest: [4; 32],
            successor_state_commitment: [5; 32],
        };
        let encoded = norito::encode_canonical(&record).expect("canonical diagnostic");
        assert!(encoded.len() <= KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1);
        let decoded: KagemushaTestnetStateObservationArchiveV1 =
            norito::decode_canonical(&encoded).expect("canonical diagnostic roundtrip");
        assert_eq!(decoded, record);
        assert!(!decoded.hardware_qualified);
    }

    #[test]
    fn release_scope_requires_both_independently_pinned_identities() {
        let scope = KagemushaTestnetStateObservationScopeV1::new([1; 32], [2; 32], [3; 32])
            .expect("distinct operator pins");
        assert!(require_scope_release_pins(scope, [2; 32], [3; 32]).is_ok());
        assert!(require_scope_release_pins(scope, [4; 32], [3; 32]).is_err());
        assert!(require_scope_release_pins(scope, [2; 32], [4; 32]).is_err());
    }
}
