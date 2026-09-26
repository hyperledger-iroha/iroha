//! Non-authorizing app boundary for one testnet lane's actual paired State proofs.
//!
//! Only a Rust host that has loaded the release-authenticated native verifier may
//! install the observation owner. The C caller supplies canonical State inputs and
//! a canonical paired proof, but cannot supply a verifier, success callback, release
//! pin, or hardware qualification. This diagnostic boundary does not open the
//! production monetary coordinator.

#[cfg(unix)]
use std::collections::BTreeMap;
use std::{
    mem::{align_of, size_of},
    path::Path,
    ptr, slice,
    sync::{Arc, Mutex, OnceLock},
};

use iroha_core::zk::kagemusha_v1_recursion::{
    KagemushaAuthenticatedArtifactSetV1, KagemushaAuthenticatedRecursiveVerifierV1,
    KagemushaDirectoryArtifactResolverV1, KagemushaOperationV1,
    KagemushaRecursiveVerifierProfileV1, KagemushaStateRelationPublicInputsV1,
    KagemushaTestnetProofObservationOwnerV1, KagemushaTestnetStateObservationScopeV1,
    KagemushaTestnetValueAdmissionV1, KagemushaVerifiedFinalityChainV1,
};
use iroha_core::zk::kagemusha_v1_state::KagemushaStateProofReleaseV1;
#[cfg(unix)]
use iroha_core::zk::kagemusha_v1_state::MintInboxReservationV1;
#[cfg(unix)]
use iroha_data_model::isi::kagemusha_v1::KagemushaOperationStatusV1;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1, KagemushaInternalValidationReceiptV1,
    KagemushaPairedProofV1, KagemushaReleaseAttestationV1, KagemushaReleaseAuthorityPolicyV1,
    KagemushaReleaseManifestV1, KagemushaReleasePurposeV1,
};
use iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1;
use libc::{c_int, c_uchar};

#[cfg(unix)]
use crate::kagemusha_mobile_bootstrap_v1::KagemushaVerifiedMobileBootstrapV1;
#[cfg(unix)]
use crate::kagemusha_reserve_finality_v1::trusted_anchor;
use crate::{
    ERR_BUFFER_TOO_SMALL, ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1, ERR_KAGEMUSHA_V1, ERR_NULL_PTR,
    kagemusha_testnet_publication_v1::{
        TestnetPublicationPermitV1, TestnetPublicationStateV1, catch_testnet_dispatch_panic_v1,
        testnet_publication_gate_v1,
    },
};

/// Maximum canonical Norito archive for one complete public State statement.
pub const KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1: usize = 4 * 1024;
/// Maximum canonical Norito diagnostic response length.
pub const KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1: usize = 256;
/// Maximum original Torii JSON Applied top-up operation status at this diagnostic boundary.
pub const KAGEMUSHA_TESTNET_MINT_STATUS_JSON_MAX_BYTES_V1: usize =
    KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1;
/// Exact length of each independently supplied finality network/context identifier.
pub const KAGEMUSHA_TESTNET_MINT_ANCHOR_ID_BYTES_V1: usize = 32;
/// Maximum canonical Norito diagnostic finalized-mint observation response.
pub const KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1: usize = 512;
/// Maximum canonical Norito archive for one explicitly experimental value admission.
pub const KAGEMUSHA_TESTNET_VALUE_ADMISSION_MAX_BYTES_V1: usize = 768;

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

/// Unsigned diagnostic result of an exact Applied top-up and paired MintFold proof.
///
/// This copied archive is inspectable data, not a monetary or hardware capability.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "connect_norito_bridge::KagemushaTestnetFinalizedMintObservationArchiveV1")]
pub struct KagemushaTestnetFinalizedMintObservationArchiveV1 {
    version: u16,
    hardware_qualified: bool,
    network_id: [u8; 32],
    release_id: [u8; 32],
    release_attestation_digest: [u8; 32],
    candidate_envelope_digest: [u8; 32],
    successor_state_commitment: [u8; 32],
    operation_id: [u8; 32],
    credit_id: [u8; 32],
    mint_envelope_digest: [u8; 32],
}

/// Copyable testnet value-admission evidence returned by the durable native owner.
///
/// A successful FFI call confirms that native memory holds the exact Applied top-up, paired
/// MintFold proof, pre-submission reservation, and independent finality pin. This archive is
/// inspectable and forgeable after copying; testnet ledgers must deduplicate its operation and
/// credit IDs and must not accept the archive itself as a production spend capability.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "connect_norito_bridge::KagemushaTestnetValueAdmissionArchiveV1")]
pub struct KagemushaTestnetValueAdmissionArchiveV1 {
    version: u16,
    hardware_qualified: bool,
    network_id: [u8; 32],
    release_id: [u8; 32],
    release_attestation_digest: [u8; 32],
    asset_identity_digest: [u8; 32],
    asset_incarnation: [u8; 32],
    asset_scale: u32,
    liability_pool_id: [u8; 32],
    operation_id: [u8; 32],
    credit_id: [u8; 32],
    amount: u128,
    mint_envelope_digest: [u8; 32],
    candidate_envelope_digest: [u8; 32],
    successor_state_commitment: [u8; 32],
    finality_block_height: u64,
    finality_height_context_id: [u8; 32],
}

struct KagemushaTestnetObservationInstallationV1 {
    owner: KagemushaTestnetProofObservationOwnerV1,
    durable: bool,
}

static TESTNET_STATE_OBSERVATION_OWNER_V1: OnceLock<
    Mutex<KagemushaTestnetObservationInstallationV1>,
> = OnceLock::new();
static TESTNET_STATE_OBSERVATION_INSTALL_LOCK_V1: Mutex<()> = Mutex::new(());

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
    let publication = testnet_publication_gate_v1()
        .exclusive()
        .map_err(|_| KagemushaTestnetObservationInstallErrorV1::AlreadyInstalled)?;
    if *publication != TestnetPublicationStateV1::Standalone {
        return Err(KagemushaTestnetObservationInstallErrorV1::AlreadyInstalled);
    }
    let _installation_guard = TESTNET_STATE_OBSERVATION_INSTALL_LOCK_V1
        .lock()
        .map_err(|_| KagemushaTestnetObservationInstallErrorV1::AlreadyInstalled)?;
    install_testnet_observation_owner_unlocked(&publication.permit(), owner, false)
}

fn install_testnet_observation_owner_unlocked(
    publication: &TestnetPublicationPermitV1<'_>,
    owner: KagemushaTestnetProofObservationOwnerV1,
    durable: bool,
) -> Result<(), KagemushaTestnetObservationInstallErrorV1> {
    if publication.require_valid().is_err() {
        return Err(KagemushaTestnetObservationInstallErrorV1::AlreadyInstalled);
    }
    TESTNET_STATE_OBSERVATION_OWNER_V1
        .set(Mutex::new(KagemushaTestnetObservationInstallationV1 {
            owner,
            durable,
        }))
        .map_err(|_| KagemushaTestnetObservationInstallErrorV1::AlreadyInstalled)
}

fn require_scope_release_pins(
    scope: KagemushaTestnetStateObservationScopeV1,
    authenticated_network_id: [u8; 32],
    authenticated_release_id: [u8; 32],
    authenticated_attestation_digest: [u8; 32],
    authenticated_purpose: KagemushaReleasePurposeV1,
) -> Result<(), String> {
    let purpose_matches = matches!(
        authenticated_purpose,
        KagemushaReleasePurposeV1::TestnetExperiment(experimental)
            if experimental.asset_identity_digest == scope.asset_identity_digest()
                && experimental.asset_incarnation == scope.asset_incarnation()
                && experimental.asset_scale == scope.asset_scale()
                && experimental.liability_pool_id == scope.liability_pool_id()
    );
    if authenticated_network_id != scope.network_id()
        || authenticated_release_id != scope.release_id()
        || authenticated_attestation_digest != scope.release_attestation_digest()
        || !purpose_matches
    {
        return Err("KAGEMUSHA release differs from operator-pinned testnet scope".to_owned());
    }
    Ok(())
}

/// Authenticate one operator-pinned testnet release and install its native proof verifier.
///
/// `trusted_authority_policy`, the complete network/asset/incarnation/scale/pool/release
/// `scope`, and `profile` must come from an independent operator-controlled Rust
/// configuration. Only the manifest, validation receipt,
/// threshold attestation, and content-addressed artifact directory are untrusted
/// package inputs. In particular, never read the authority policy or expected
/// release, network, or reserve pins from the submitted wallet proof or from that package.
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
    let publication = testnet_publication_gate_v1().exclusive()?;
    if *publication != TestnetPublicationStateV1::Standalone {
        return Err("testnet diagnostic installation requires standalone publication".to_owned());
    }
    let _installation_guard = TESTNET_STATE_OBSERVATION_INSTALL_LOCK_V1
        .lock()
        .map_err(|_| "KAGEMUSHA testnet owner installation lock is poisoned".to_owned())?;
    if TESTNET_STATE_OBSERVATION_OWNER_V1.get().is_some() {
        return Err("KAGEMUSHA testnet observation owner is already installed".to_owned());
    }
    let verifier = load_authenticated_testnet_verifier(
        manifest_archive,
        validation_receipt_archive,
        release_attestation_archive,
        trusted_authority_policy,
        scope,
        profile,
        artifact_root,
    )?;
    let owner = KagemushaTestnetProofObservationOwnerV1::new(verifier, scope)
        .map_err(|error| format!("invalid KAGEMUSHA testnet observation owner: {error}"))?;
    install_testnet_observation_owner_unlocked(&publication.permit(), owner, false)
        .map_err(|_| "KAGEMUSHA testnet observation owner is already installed".to_owned())
}

/// Whether to create a fresh native testnet trial or replay its exact private journal.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaTestnetDurableObservationModeV1 {
    /// Create a new journal, rejecting an existing path.
    Create,
    /// Recover an existing journal with its original release and independent scope pins.
    Recover,
}

#[cfg(unix)]
pub(crate) fn authenticated_observation_scope(
    bootstrap: &KagemushaVerifiedMobileBootstrapV1,
) -> Result<KagemushaTestnetStateObservationScopeV1, String> {
    let asset = bootstrap.scope();
    KagemushaTestnetStateObservationScopeV1::new(
        *bootstrap.network_id().as_bytes(),
        asset.asset_identity_digest,
        asset.asset_incarnation,
        asset.asset_scale,
        asset.liability_pool_id,
        bootstrap.release_id(),
        bootstrap.release_attestation_digest(),
    )
    .map_err(|error| format!("invalid authenticated testnet bootstrap scope: {error}"))
}

/// Install an authenticated native verifier with one private, durable testnet lineage trial.
///
/// The journal path and all release pins must come from trusted native configuration. The
/// private mint reservation, including the credit opening, is never accepted over C or JNI.
/// A caller must reserve it natively before sending the exact top-up request. Neither this
/// installation nor its observation result qualifies hardware or opens the production Guard.
///
/// # Errors
///
/// Rejects unauthenticated artifacts, a changed scope/release, an absent or conflicting
/// journal, missing independent finality anchors, failed replay, or an already installed owner.
#[cfg(unix)]
pub(crate) fn load_and_install_kagemusha_testnet_durable_state_observation_owner_v1(
    publication: &TestnetPublicationPermitV1<'_>,
    manifest_archive: &[u8],
    validation_receipt_archive: &[u8],
    release_attestation_archive: &[u8],
    bootstrap: &KagemushaVerifiedMobileBootstrapV1,
    profile: KagemushaRecursiveVerifierProfileV1,
    artifact_root: impl AsRef<Path>,
    journal_path: impl AsRef<Path>,
    mode: KagemushaTestnetDurableObservationModeV1,
    independent_anchors: &BTreeMap<[u8; 32], KagemushaVerifiedFinalityChainV1>,
) -> Result<(), String> {
    publication.require_valid()?;
    bootstrap.require_unexpired()?;
    let scope = authenticated_observation_scope(bootstrap)?;
    // Serialize journal creation with every installation path. A duplicate caller must
    // not initialize an orphan journal before OnceLock rejects its owner.
    let _installation_guard = TESTNET_STATE_OBSERVATION_INSTALL_LOCK_V1
        .lock()
        .map_err(|_| "KAGEMUSHA testnet owner installation lock is poisoned".to_owned())?;
    if TESTNET_STATE_OBSERVATION_OWNER_V1.get().is_some() {
        return Err("KAGEMUSHA testnet observation owner is already installed".to_owned());
    }
    let verifier = load_authenticated_testnet_verifier(
        manifest_archive,
        validation_receipt_archive,
        release_attestation_archive,
        bootstrap.trusted_authority_policy(),
        scope,
        profile,
        artifact_root,
    )?;
    bootstrap.require_unexpired()?;
    let owner = match mode {
        KagemushaTestnetDurableObservationModeV1::Create => {
            KagemushaTestnetProofObservationOwnerV1::create_durable(
                verifier,
                scope,
                journal_path.as_ref(),
            )
        }
        KagemushaTestnetDurableObservationModeV1::Recover => {
            KagemushaTestnetProofObservationOwnerV1::recover_durable(
                verifier,
                scope,
                journal_path.as_ref(),
                independent_anchors,
            )
        }
    }
    .map_err(|error| format!("cannot open KAGEMUSHA private testnet journal: {error}"))?;
    // Artifact loading and journal replay may be lengthy. A checkpoint checked only
    // at entry must not authorize publication after its installation lease expires.
    bootstrap.require_unexpired()?;
    install_testnet_observation_owner_unlocked(publication, owner, true)
        .map_err(|_| "KAGEMUSHA testnet observation owner is already installed".to_owned())
}

fn load_authenticated_testnet_verifier(
    manifest_archive: &[u8],
    validation_receipt_archive: &[u8],
    release_attestation_archive: &[u8],
    trusted_authority_policy: &KagemushaReleaseAuthorityPolicyV1,
    scope: KagemushaTestnetStateObservationScopeV1,
    profile: KagemushaRecursiveVerifierProfileV1,
    artifact_root: impl AsRef<Path>,
) -> Result<KagemushaAuthenticatedRecursiveVerifierV1, String> {
    trusted_authority_policy
        .validate()
        .map_err(|error| format!("invalid operator-pinned KAGEMUSHA authority policy: {error}"))?;
    let manifest = KagemushaReleaseManifestV1::decode_canonical_exact(manifest_archive)
        .map_err(|error| format!("invalid KAGEMUSHA release manifest: {error}"))?;
    let receipt = KagemushaInternalValidationReceiptV1::decode_canonical_experimental_exact(
        validation_receipt_archive,
    )
    .map_err(|error| format!("invalid KAGEMUSHA release validation receipt: {error}"))?;
    let attestation =
        KagemushaReleaseAttestationV1::decode_canonical_exact(release_attestation_archive)
            .map_err(|error| format!("invalid KAGEMUSHA release attestation: {error}"))?;
    let release = manifest
        .authenticate_experimental(&receipt, trusted_authority_policy, &attestation)
        .map_err(|error| format!("unauthenticated KAGEMUSHA release: {error}"))?;
    require_scope_release_pins(
        scope,
        *release.network_id().as_bytes(),
        release.release_id(),
        release.attestation_digest(),
        release.purpose(),
    )?;
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
        .authorize_experimental_proof_release(Arc::new(release))
        .map_err(|error| {
            format!("cannot authorize KAGEMUSHA experimental proof release: {error}")
        })?;
    Ok(verifier)
}

/// Durably reserve an exact confidential mint before submitting its top-up request.
///
/// This Rust-only API is for a trusted native provider that owns the private credit opening.
/// It returns `true` for a new fsynced reservation and `false` for its exact retry. No C/JNI
/// entrypoint transports a `MintInboxReservationV1` from application memory.
///
/// # Errors
///
/// Rejects an absent or process-only owner, invalid reservation, changed-byte retry,
/// duplicate credit, or journal write failure.
#[cfg(unix)]
pub fn reserve_kagemusha_testnet_mint_before_submission_v1(
    reservation: &MintInboxReservationV1,
) -> Result<bool, String> {
    testnet_publication_gate_v1().with_dispatch(|publication| {
        reserve_kagemusha_testnet_mint_under_publication_v1(publication, reservation)
    })
}

#[cfg(unix)]
pub(crate) fn reserve_kagemusha_testnet_mint_under_publication_v1(
    publication: &TestnetPublicationPermitV1<'_>,
    reservation: &MintInboxReservationV1,
) -> Result<bool, String> {
    publication.require_valid()?;
    let installed = TESTNET_STATE_OBSERVATION_OWNER_V1
        .get()
        .ok_or_else(|| "KAGEMUSHA native testnet observation owner is unavailable".to_owned())?;
    let mut installed = installed
        .lock()
        .map_err(|_| "KAGEMUSHA native testnet observation owner is poisoned".to_owned())?;
    if !installed.durable {
        return Err("KAGEMUSHA durable testnet observation owner is unavailable".to_owned());
    }
    installed
        .owner
        .reserve_mint_before_submission(reservation)
        .map_err(|error| format!("KAGEMUSHA testnet mint reservation rejected: {error}"))
}

/// Pin a verified signed finality chain to one durably reserved testnet top-up.
///
/// This Rust-only call must be made by a native finality source that has verified the actual
/// chain height context independently of the submitted Torii operation response. The bridge
/// cannot infer that provenance from caller coordinates, a status hint, or a certificate whose
/// roster is embedded in the response. C/JNI observation succeeds only for this exact pin.
/// An exact retry returns `false`; a replacement pin fails closed.
///
/// # Errors
///
/// Rejects a missing durable owner or reservation, wrong network, malformed anchor,
/// replacement pin, or poisoned owner.
#[cfg(unix)]
pub(crate) fn pin_kagemusha_testnet_authenticated_finality_anchor_v1(
    publication: &TestnetPublicationPermitV1<'_>,
    operation_id: [u8; 32],
    verified_chain: &KagemushaVerifiedFinalityChainV1,
) -> Result<bool, String> {
    publication.require_valid()?;
    let installed = TESTNET_STATE_OBSERVATION_OWNER_V1
        .get()
        .ok_or_else(|| "KAGEMUSHA native testnet observation owner is unavailable".to_owned())?;
    let mut installed = installed
        .lock()
        .map_err(|_| "KAGEMUSHA native testnet observation owner is poisoned".to_owned())?;
    if !installed.durable {
        return Err("KAGEMUSHA durable testnet observation owner is unavailable".to_owned());
    }
    installed
        .owner
        .pin_authenticated_finality_anchor(operation_id, verified_chain)
        .map_err(|error| format!("KAGEMUSHA testnet finality anchor rejected: {error}"))
}

/// Run one native-only mint-credit operation while the durable proof owner is held.
///
/// No C/JNI caller can supply or serialize the opaque value-admission token. Holding
/// the owner lock through the consumer prevents an observation/recovery race between
/// rederiving the Applied proof and durably counting its credit.
#[cfg(unix)]
pub(crate) fn with_kagemusha_testnet_durable_credit_owner_v1<T>(
    publication: &TestnetPublicationPermitV1<'_>,
    consume: impl FnOnce(&KagemushaTestnetProofObservationOwnerV1) -> Result<T, String>,
) -> Result<T, String> {
    publication.require_valid()?;
    let owner = TESTNET_STATE_OBSERVATION_OWNER_V1
        .get()
        .ok_or_else(|| "KAGEMUSHA durable testnet observation owner is unavailable".to_owned())?;
    let installed = owner
        .lock()
        .map_err(|_| "KAGEMUSHA durable testnet observation owner is poisoned".to_owned())?;
    require_durable_credit_owner_v1(installed.durable)?;
    consume(&installed.owner)
}

#[cfg(unix)]
fn require_durable_credit_owner_v1(durable: bool) -> Result<(), String> {
    if durable {
        Ok(())
    } else {
        Err("KAGEMUSHA durable testnet observation owner is unavailable".to_owned())
    }
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
/// maximum output capacity before verification can advance the trial. For valid,
/// disjoint buffers, `output_len` is zero on failure and the actual canonical
/// archive length on success. An aliased or misaligned `output_len` is rejected
/// without writing through it, so it cannot corrupt an input or successful archive.
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
    let length_start = output_len as usize;
    let Some(length_end) = length_start.checked_add(size_of::<usize>()) else {
        return ERR_KAGEMUSHA_V1;
    };
    if !length_start.is_multiple_of(align_of::<usize>())
        || [
            (
                public_inputs_archive_ptr as usize,
                public_inputs_archive_len,
            ),
            (paired_proof_archive_ptr as usize, paired_proof_archive_len),
            (output_ptr as usize, output_capacity),
        ]
        .into_iter()
        .any(|(start, length)| {
            start != 0
                && start
                    .checked_add(length)
                    .is_none_or(|end| start < length_end && length_start < end)
        })
    {
        return ERR_KAGEMUSHA_V1;
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
    // Keep the publication dispatch guard through verification, journal mutation and output.
    // A partially installed owner is never usable, even if its OnceLock is already populated.
    let Ok(_publication) = testnet_publication_gate_v1().dispatch() else {
        return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1;
    };
    let Some(owner) = TESTNET_STATE_OBSERVATION_OWNER_V1.get() else {
        return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1;
    };
    let archive = catch_testnet_dispatch_panic_v1(&_publication, || {
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
        let mut installed = owner.lock().map_err(|_| ())?;
        let scope = installed.owner.scope();
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
        installed
            .owner
            .observe_and_advance(&public, &proof)
            .map_err(|_| ())?;
        Ok(archive)
    });
    let Ok(Ok(archive)) = archive else {
        return ERR_KAGEMUSHA_V1;
    };
    unsafe {
        ptr::copy_nonoverlapping(archive.as_ptr(), output_ptr, archive.len());
        *output_len = archive.len();
    };
    0
}

/// Observe an Applied top-up and real paired MintFold proof after a native pre-send reservation.
///
/// `operation_id` names the exact private reservation already fsynced by the Rust owner. Its
/// confidential opening never crosses this ABI. The caller supplies the original Torii status
/// JSON, finality network/height/context that must match a Rust-only authenticated native pin,
/// and canonical State inputs and proof. The
/// response is unsigned, non-authorizing diagnostic data with `hardware_qualified=false`.
/// A missing durable native owner or reservation fails closed. As with the State observer,
/// supply the full output capacity before a proof can advance the trial. `output_length` must
/// be naturally aligned and disjoint from every input and output span.
///
/// # Safety
///
/// Each non-null pointer must reference its declared number of accessible bytes, and the
/// output span must be writable for its declared capacity.
#[unsafe(no_mangle)]
#[cfg(unix)]
pub unsafe extern "C" fn connect_norito_kagemusha_testnet_finalized_mint_observe_v1(
    operation_id_ptr: *const c_uchar,
    operation_id_len: usize,
    status_json_ptr: *const c_uchar,
    status_json_len: usize,
    anchor_network_id_ptr: *const c_uchar,
    anchor_network_id_len: usize,
    anchor_height: u64,
    anchor_context_id_ptr: *const c_uchar,
    anchor_context_id_len: usize,
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
    let length_start = output_len as usize;
    let Some(length_end) = length_start.checked_add(size_of::<usize>()) else {
        return ERR_KAGEMUSHA_V1;
    };
    if !length_start.is_multiple_of(align_of::<usize>())
        || [
            (operation_id_ptr as usize, operation_id_len),
            (status_json_ptr as usize, status_json_len),
            (anchor_network_id_ptr as usize, anchor_network_id_len),
            (anchor_context_id_ptr as usize, anchor_context_id_len),
            (
                public_inputs_archive_ptr as usize,
                public_inputs_archive_len,
            ),
            (paired_proof_archive_ptr as usize, paired_proof_archive_len),
            (output_ptr as usize, output_capacity),
        ]
        .into_iter()
        .any(|(start, length)| {
            start != 0
                && start
                    .checked_add(length)
                    .is_none_or(|end| start < length_end && length_start < end)
        })
    {
        return ERR_KAGEMUSHA_V1;
    }
    unsafe { *output_len = 0 };
    if operation_id_ptr.is_null()
        || status_json_ptr.is_null()
        || anchor_network_id_ptr.is_null()
        || anchor_context_id_ptr.is_null()
        || public_inputs_archive_ptr.is_null()
        || paired_proof_archive_ptr.is_null()
        || output_ptr.is_null()
    {
        return ERR_NULL_PTR;
    }
    if operation_id_len != 32
        || status_json_len == 0
        || status_json_len > KAGEMUSHA_TESTNET_MINT_STATUS_JSON_MAX_BYTES_V1
        || anchor_network_id_len != KAGEMUSHA_TESTNET_MINT_ANCHOR_ID_BYTES_V1
        || anchor_context_id_len != KAGEMUSHA_TESTNET_MINT_ANCHOR_ID_BYTES_V1
        || anchor_height == 0
        || public_inputs_archive_len == 0
        || public_inputs_archive_len > KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1
        || paired_proof_archive_len == 0
        || paired_proof_archive_len > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
    {
        return ERR_KAGEMUSHA_V1;
    }
    if output_capacity < KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1 {
        return ERR_BUFFER_TOO_SMALL;
    }
    // Keep the publication dispatch guard through verification, journal mutation and output.
    // A partially installed owner is never usable, even if its OnceLock is already populated.
    let Ok(_publication) = testnet_publication_gate_v1().dispatch() else {
        return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1;
    };
    let Some(owner) = TESTNET_STATE_OBSERVATION_OWNER_V1.get() else {
        return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1;
    };
    match owner.lock() {
        Ok(installed) if installed.durable => {}
        Ok(_) => return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1,
        Err(_) => return ERR_KAGEMUSHA_V1,
    }
    // Foreign buffers may alias one another or the output. Snapshot every bounded input
    // before canonical decoding or acquiring the mutable trial owner.
    let operation_id: [u8; 32] = unsafe { slice::from_raw_parts(operation_id_ptr, 32) }
        .try_into()
        .expect("fixed operation ID length");
    let status_json = unsafe { slice::from_raw_parts(status_json_ptr, status_json_len) }.to_vec();
    let anchor_network_id: [u8; 32] = unsafe { slice::from_raw_parts(anchor_network_id_ptr, 32) }
        .try_into()
        .expect("fixed network ID length");
    let anchor_context_id: [u8; 32] = unsafe { slice::from_raw_parts(anchor_context_id_ptr, 32) }
        .try_into()
        .expect("fixed context ID length");
    let public_archive =
        unsafe { slice::from_raw_parts(public_inputs_archive_ptr, public_inputs_archive_len) }
            .to_vec();
    let proof_archive =
        unsafe { slice::from_raw_parts(paired_proof_archive_ptr, paired_proof_archive_len) }
            .to_vec();
    let archive = catch_testnet_dispatch_panic_v1(&_publication, || {
        let trust_anchor =
            trusted_anchor(anchor_network_id, anchor_height, anchor_context_id).map_err(|_| ())?;
        // Core rejects an absent native reservation or independently pinned finality
        // context before it validates the status certificate and its paired proofs.
        let status: KagemushaOperationStatusV1 =
            norito::json::from_slice(&status_json).map_err(|_| ())?;
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
        let mut installed = owner.lock().map_err(|_| ())?;
        if !installed.durable {
            return Err(());
        }
        let scope = installed.owner.scope();
        let observation = installed
            .owner
            .observe_retained_finalized_mint_and_advance(
                operation_id,
                &status,
                &trust_anchor,
                &public,
                &proof,
            )
            .map_err(|_| ())?;
        let record = KagemushaTestnetFinalizedMintObservationArchiveV1 {
            version: 1,
            hardware_qualified: false,
            network_id: scope.network_id(),
            release_id: scope.release_id(),
            release_attestation_digest: scope.release_attestation_digest(),
            candidate_envelope_digest:
                KagemushaTestnetProofObservationOwnerV1::candidate_envelope_digest(&public)
                    .map_err(|_| ())?,
            successor_state_commitment: public.successor.state_commitment,
            operation_id: observation.operation_id(),
            credit_id: observation.credit_id(),
            mint_envelope_digest: observation.mint_envelope_digest(),
        };
        let archive = norito::encode_canonical(&record).map_err(|_| ())?;
        if archive.len() > KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1 {
            return Err(());
        }
        Ok(archive)
    });
    let Ok(Ok(archive)) = archive else {
        return ERR_KAGEMUSHA_V1;
    };
    unsafe {
        ptr::copy_nonoverlapping(archive.as_ptr(), output_ptr, archive.len());
        *output_len = archive.len();
    }
    0
}

/// Admit one previously verified Applied mint as scoped value in the experimental testnet lane.
///
/// The caller supplies only the operation ID. The durable native owner retrieves the exact
/// proof, reservation, signed release, and independently pinned finality context retained in
/// its journal. A copied output archive is inspection data; a testnet ledger must call this
/// boundary and deduplicate operation/credit IDs instead of trusting submitted archive bytes.
///
/// # Safety
/// Non-null pointers must reference their declared accessible spans; the output span must be
/// writable, and `output_len` must be naturally aligned and disjoint from input and output.
#[unsafe(no_mangle)]
#[cfg(unix)]
pub unsafe extern "C" fn connect_norito_kagemusha_testnet_value_admit_v1(
    operation_id_ptr: *const c_uchar,
    operation_id_len: usize,
    output_ptr: *mut c_uchar,
    output_capacity: usize,
    output_len: *mut usize,
) -> c_int {
    if output_len.is_null() {
        return ERR_NULL_PTR;
    }
    let length_start = output_len as usize;
    let Some(length_end) = length_start.checked_add(size_of::<usize>()) else {
        return ERR_KAGEMUSHA_V1;
    };
    let input_start = operation_id_ptr as usize;
    let output_start = output_ptr as usize;
    let Some(input_end) = input_start.checked_add(operation_id_len) else {
        return ERR_KAGEMUSHA_V1;
    };
    let Some(output_end) = output_start.checked_add(output_capacity) else {
        return ERR_KAGEMUSHA_V1;
    };
    if !length_start.is_multiple_of(align_of::<usize>())
        || (input_start < length_end && length_start < input_end)
        || (output_start < length_end && length_start < output_end)
        || (input_start < output_end && output_start < input_end)
    {
        return ERR_KAGEMUSHA_V1;
    }
    unsafe { *output_len = 0 };
    if operation_id_ptr.is_null() || output_ptr.is_null() {
        return ERR_NULL_PTR;
    }
    if operation_id_len != 32 {
        return ERR_KAGEMUSHA_V1;
    }
    if output_capacity < KAGEMUSHA_TESTNET_VALUE_ADMISSION_MAX_BYTES_V1 {
        return ERR_BUFFER_TOO_SMALL;
    }
    // Keep the publication dispatch guard through verification, journal mutation and output.
    // A partially installed owner is never usable, even if its OnceLock is already populated.
    let Ok(_publication) = testnet_publication_gate_v1().dispatch() else {
        return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1;
    };
    let Some(owner) = TESTNET_STATE_OBSERVATION_OWNER_V1.get() else {
        return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1;
    };
    let operation_id: [u8; 32] = unsafe { slice::from_raw_parts(operation_id_ptr, 32) }
        .try_into()
        .expect("fixed operation ID length");
    let archive = catch_testnet_dispatch_panic_v1(&_publication, || {
        let installed = owner.lock().map_err(|_| ())?;
        if !installed.durable {
            return Err(());
        }
        let admission: KagemushaTestnetValueAdmissionV1 = installed
            .owner
            .admit_finalized_testnet_value(operation_id)
            .map_err(|_| ())?;
        let scope = admission.scope();
        let anchor = admission.finality_anchor();
        let context_id: [u8; 32] = *anchor.height_context_id.0.as_ref();
        let record = KagemushaTestnetValueAdmissionArchiveV1 {
            version: 1,
            hardware_qualified: false,
            network_id: scope.network_id(),
            release_id: scope.release_id(),
            release_attestation_digest: scope.release_attestation_digest(),
            asset_identity_digest: scope.asset_identity_digest(),
            asset_incarnation: scope.asset_incarnation(),
            asset_scale: scope.asset_scale(),
            liability_pool_id: scope.liability_pool_id(),
            operation_id: admission.operation_id(),
            credit_id: admission.credit_id(),
            amount: admission.amount(),
            mint_envelope_digest: admission.mint_envelope_digest(),
            candidate_envelope_digest: admission.candidate_envelope_digest(),
            successor_state_commitment: admission.successor_state_commitment(),
            finality_block_height: anchor.block_height,
            finality_height_context_id: context_id,
        };
        let archive = norito::encode_canonical(&record).map_err(|_| ())?;
        if archive.len() > KAGEMUSHA_TESTNET_VALUE_ADMISSION_MAX_BYTES_V1 {
            return Err(());
        }
        Ok(archive)
    });
    let Ok(Ok(archive)) = archive else {
        return ERR_KAGEMUSHA_V1;
    };
    unsafe {
        ptr::copy_nonoverlapping(archive.as_ptr(), output_ptr, archive.len());
        *output_len = archive.len();
    }
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

    #[cfg(unix)]
    #[test]
    fn finalized_mint_entry_requires_complete_bounded_inputs_and_native_owner() {
        let input = [1_u8; 32];
        let mut output = [0x5a_u8; KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1];
        let output_ptr = output.as_mut_ptr();
        let mut output_len = 123;
        let call = |operation_id_ptr,
                    operation_id_len,
                    status_json_len,
                    output_capacity,
                    output_len: &mut usize| unsafe {
            connect_norito_kagemusha_testnet_finalized_mint_observe_v1(
                operation_id_ptr,
                operation_id_len,
                input.as_ptr(),
                status_json_len,
                input.as_ptr(),
                32,
                1,
                input.as_ptr(),
                32,
                input.as_ptr(),
                1,
                input.as_ptr(),
                1,
                output_ptr,
                output_capacity,
                output_len,
            )
        };
        assert_eq!(
            call(
                ptr::null(),
                32,
                1,
                KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1,
                &mut output_len
            ),
            ERR_NULL_PTR
        );
        assert_eq!(output_len, 0);
        output_len = 123;
        assert_eq!(
            call(
                input.as_ptr(),
                31,
                1,
                KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1,
                &mut output_len
            ),
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(output_len, 0);
        output_len = 123;
        let oversized_status = vec![1_u8; KAGEMUSHA_TESTNET_MINT_STATUS_JSON_MAX_BYTES_V1 + 1];
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_finalized_mint_observe_v1(
                    input.as_ptr(),
                    input.len(),
                    oversized_status.as_ptr(),
                    oversized_status.len(),
                    input.as_ptr(),
                    32,
                    1,
                    input.as_ptr(),
                    32,
                    input.as_ptr(),
                    1,
                    input.as_ptr(),
                    1,
                    output_ptr,
                    KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1,
                    &mut output_len,
                )
            },
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(output_len, 0);
        output_len = 123;
        assert_eq!(
            call(
                input.as_ptr(),
                32,
                1,
                KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1 - 1,
                &mut output_len
            ),
            ERR_BUFFER_TOO_SMALL
        );
        assert_eq!(output_len, 0);
        output_len = 123;
        assert_eq!(
            call(
                input.as_ptr(),
                32,
                1,
                KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1,
                &mut output_len
            ),
            ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1
        );
        assert_eq!(output_len, 0);
        assert_eq!(
            output,
            [0x5a; KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1]
        );
    }

    #[cfg(unix)]
    #[test]
    fn finalized_mint_entry_rejects_aliased_length_before_any_write() {
        let input = [1_u8; 32];
        let mut output = [usize::MAX; KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1];
        let output_ptr = output.as_mut_ptr().cast::<u8>();
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_finalized_mint_observe_v1(
                    input.as_ptr(),
                    input.len(),
                    input.as_ptr(),
                    1,
                    input.as_ptr(),
                    32,
                    1,
                    input.as_ptr(),
                    32,
                    input.as_ptr(),
                    1,
                    input.as_ptr(),
                    1,
                    output_ptr,
                    KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1,
                    output.as_mut_ptr(),
                )
            },
            ERR_KAGEMUSHA_V1
        );
        assert!(output.iter().all(|word| *word == usize::MAX));
    }

    #[test]
    fn diagnostic_entry_rejects_aliased_or_misaligned_length_before_writing() {
        let input = [1_u8];
        let mut output = [usize::MAX; KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1];
        let output_ptr = output.as_mut_ptr().cast::<u8>();
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_state_proof_observe_v1(
                    input.as_ptr(),
                    input.len(),
                    input.as_ptr(),
                    input.len(),
                    output_ptr,
                    KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1,
                    output.as_mut_ptr(),
                )
            },
            ERR_KAGEMUSHA_V1
        );
        assert!(output.iter().all(|word| *word == usize::MAX));

        let mut input_word = usize::MAX;
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_state_proof_observe_v1(
                    (&raw const input_word).cast::<u8>(),
                    size_of::<usize>(),
                    input.as_ptr(),
                    input.len(),
                    output_ptr,
                    KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1,
                    &raw mut input_word,
                )
            },
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(input_word, usize::MAX);

        let mut length_words = [usize::MAX; 2];
        let misaligned = unsafe {
            length_words
                .as_mut_ptr()
                .cast::<u8>()
                .add(1)
                .cast::<usize>()
        };
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_state_proof_observe_v1(
                    input.as_ptr(),
                    input.len(),
                    input.as_ptr(),
                    input.len(),
                    output_ptr,
                    KAGEMUSHA_TESTNET_STATE_OBSERVATION_MAX_BYTES_V1,
                    misaligned,
                )
            },
            ERR_KAGEMUSHA_V1
        );
        assert!(length_words.iter().all(|word| *word == usize::MAX));
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
        // Struct fields, including fixed arrays, carry compact length prefixes.
        // The mobile observer accepts this exact canonical archive layout.
        assert!(encoded.len() > 40);
        assert_eq!(encoded[39], norito::core::header_flags::COMPACT_LEN);
        let decoded: KagemushaTestnetStateObservationArchiveV1 =
            norito::decode_canonical(&encoded).expect("canonical diagnostic roundtrip");
        assert_eq!(decoded, record);
        assert!(!decoded.hardware_qualified);
    }

    #[test]
    fn finalized_mint_archive_is_canonical_and_declares_no_hardware_authority() {
        let record = KagemushaTestnetFinalizedMintObservationArchiveV1 {
            version: 1,
            hardware_qualified: false,
            network_id: [1; 32],
            release_id: [2; 32],
            release_attestation_digest: [3; 32],
            candidate_envelope_digest: [4; 32],
            successor_state_commitment: [5; 32],
            operation_id: [6; 32],
            credit_id: [7; 32],
            mint_envelope_digest: [8; 32],
        };
        let encoded = norito::encode_canonical(&record).expect("canonical diagnostic");
        assert!(encoded.len() <= KAGEMUSHA_TESTNET_MINT_OBSERVATION_MAX_BYTES_V1);
        let decoded: KagemushaTestnetFinalizedMintObservationArchiveV1 =
            norito::decode_canonical(&encoded).expect("canonical diagnostic roundtrip");
        assert_eq!(decoded, record);
        assert!(!decoded.hardware_qualified);
    }

    #[cfg(unix)]
    #[test]
    fn testnet_value_admission_archive_is_scoped_and_requires_native_owner() {
        let record = KagemushaTestnetValueAdmissionArchiveV1 {
            version: 1,
            hardware_qualified: false,
            network_id: [1; 32],
            release_id: [2; 32],
            release_attestation_digest: [3; 32],
            asset_identity_digest: [4; 32],
            asset_incarnation: [5; 32],
            asset_scale: 2,
            liability_pool_id: [6; 32],
            operation_id: [7; 32],
            credit_id: [8; 32],
            amount: 9,
            mint_envelope_digest: [10; 32],
            candidate_envelope_digest: [11; 32],
            successor_state_commitment: [12; 32],
            finality_block_height: 13,
            finality_height_context_id: [14; 32],
        };
        let encoded = norito::encode_canonical(&record).expect("canonical value archive");
        assert_eq!(encoded.len(), 480);
        assert_eq!(&encoded[40..48], &[0; 8]);
        assert!(encoded.len() <= KAGEMUSHA_TESTNET_VALUE_ADMISSION_MAX_BYTES_V1);
        let decoded: KagemushaTestnetValueAdmissionArchiveV1 =
            norito::decode_canonical(&encoded).expect("canonical value archive roundtrip");
        assert_eq!(decoded, record);
        assert!(!decoded.hardware_qualified);

        let mut output = [0x5a_u8; KAGEMUSHA_TESTNET_VALUE_ADMISSION_MAX_BYTES_V1];
        let mut output_len = 123;
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_testnet_value_admit_v1(
                    record.operation_id.as_ptr(),
                    31,
                    output.as_mut_ptr(),
                    output.len(),
                    &mut output_len,
                )
            },
            ERR_KAGEMUSHA_V1
        );
        assert_eq!(output_len, 0);
        assert!(output.iter().all(|byte| *byte == 0x5a));
        assert_ne!(
            unsafe {
                connect_norito_kagemusha_testnet_value_admit_v1(
                    record.operation_id.as_ptr(),
                    32,
                    output.as_mut_ptr(),
                    output.len(),
                    &mut output_len,
                )
            },
            0,
            "no retained Applied mint may be admitted from an uninstalled owner"
        );
    }

    #[test]
    fn release_scope_requires_all_independently_pinned_identities() {
        let scope = KagemushaTestnetStateObservationScopeV1::new(
            [1; 32], [4; 32], [5; 32], 2, [6; 32], [2; 32], [3; 32],
        )
        .expect("distinct operator pins");
        let purpose = KagemushaReleasePurposeV1::TestnetExperiment(
            iroha_data_model::kagemusha::KagemushaTestnetExperimentScopeV1 {
                asset_identity_digest: [4; 32],
                asset_incarnation: [5; 32],
                asset_scale: 2,
                liability_pool_id: [6; 32],
            },
        );
        assert!(require_scope_release_pins(scope, [1; 32], [2; 32], [3; 32], purpose).is_ok());
        assert!(require_scope_release_pins(scope, [4; 32], [2; 32], [3; 32], purpose).is_err());
        assert!(require_scope_release_pins(scope, [1; 32], [4; 32], [3; 32], purpose).is_err());
        assert!(require_scope_release_pins(scope, [1; 32], [2; 32], [4; 32], purpose).is_err());
        assert!(
            require_scope_release_pins(
                scope,
                [1; 32],
                [2; 32],
                [3; 32],
                KagemushaReleasePurposeV1::Production
            )
            .is_err()
        );
        assert!(
            require_scope_release_pins(
                scope,
                [1; 32],
                [2; 32],
                [3; 32],
                KagemushaReleasePurposeV1::TestnetExperiment(
                    iroha_data_model::kagemusha::KagemushaTestnetExperimentScopeV1 {
                        asset_identity_digest: [7; 32],
                        asset_incarnation: [5; 32],
                        asset_scale: 2,
                        liability_pool_id: [6; 32],
                    }
                )
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn testnet_value_credit_owner_rejects_process_only_installation() {
        assert!(require_durable_credit_owner_v1(false).is_err());
        assert!(require_durable_credit_owner_v1(true).is_ok());
        let gate = crate::kagemusha_testnet_publication_v1::TestnetPublicationGateV1::for_test();
        let publication = gate.dispatch().unwrap();
        assert!(
            with_kagemusha_testnet_durable_credit_owner_v1(&publication.permit(), |_| Ok(()))
                .is_err()
        );
    }
}
