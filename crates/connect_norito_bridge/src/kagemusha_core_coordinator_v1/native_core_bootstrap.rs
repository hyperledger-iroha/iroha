//! Concrete native fresh-enrollment bootstrap and exclusive exact publication retry.
//! The platform supplies physical custody and originals, never a software Core backend.

use super::{
    FreshIssuerAdmissionV1, KagemushaAuthenticatedRecoveredCoordinatorV1,
    KagemushaCoreCoordinatorBackendErrorV1 as Error, KagemushaNativeCoreAuthorizationSignerV1,
    kagemusha_core_coordinator_validate_storage_path_v1,
};
use iroha_core_zk::{
    kagemusha_v1_recursion::{
        KagemushaArtifactByteResolverV1, KagemushaAuthenticatedGuardBundleVerifierV1,
        KagemushaAuthenticatedRecursiveVerifierV1, KagemushaHardwareTransactionV1,
        KagemushaHardwareTransactionVerifierV1, KagemushaProductionProverV1,
        KagemushaRecursiveVerifierProfileV1,
    },
    kagemusha_v1_state::{
        BootstrapAuthorizationV1, KagemushaAuthenticatedBootstrapProvingSelectionV1,
        KagemushaAuthenticatedCoreOwnerV1, KagemushaBootstrapCheckpointV1,
        KagemushaDiskAuthenticatedHistoryStoreV1, KagemushaDurableCapacityV1,
        KagemushaHardwareTransactionJournalV1, KagemushaHardwareTransactionTransportV1,
    },
};
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

type Checkpoint = KagemushaBootstrapCheckpointV1<
    Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    KagemushaAuthenticatedGuardBundleVerifierV1,
    KagemushaDiskAuthenticatedHistoryStoreV1,
>;

/// Native-only originals for this exact issuer admission. Merely constructing these values
/// grants no authority: the concrete Core reauthenticates both real proof relations and Guard.
/// The independently installed source owns their path, time, key, policy and physical custody.
pub struct KagemushaNativeCoreBootstrapInputsV1 {
    /// Newly authenticated native service/hardware time; never a mobile timestamp.
    pub trusted_current_time_ms: u64,
    /// Original genuine hardware bootstrap Guard, not a claimed proof or software owner.
    pub original_guard_bundle: Vec<u8>,
    /// Independently selected native profile and original signed-content-addressed PK/VK source.
    pub prover_profile: KagemushaRecursiveVerifierProfileV1,
    pub artifact_resolver: Arc<dyn KagemushaArtifactByteResolverV1>,
    /// Original private state nonce commitment bound by that proof.
    pub state_nonce_commitment: [u8; 32],
    /// Original selected physical storage policy.
    pub durable_capacity: KagemushaDurableCapacityV1,
    /// Actual production verifier retaining the threshold-authenticated release catalog.
    pub recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    /// Actual governed hardware verifier and physical nonforking transport.
    pub hardware_verifier: KagemushaHardwareTransactionVerifierV1,
    pub transaction_transport: Arc<dyn KagemushaHardwareTransactionTransportV1>,
    /// Original independently installed Core private-key custody.
    pub signer: Arc<dyn KagemushaNativeCoreAuthorizationSignerV1>,
    /// Exact private child paths, selected outside the C/JNI request.
    pub history_directory: PathBuf,
    pub journal_bundle_directory: PathBuf,
    pub transaction_directory: PathBuf,
    pub overlay_capacity_bytes: u64,
    pub coordinator_capacity_bytes: u64,
    /// Original nonzero hardware INITIAL checkpoint operation identity.
    pub checkpoint_operation_id: [u8; 32],
}

/// Independently installed immutable Rust physical/original source. No C/JNI registration,
/// accepting verifier, app-selected root or serialized owner is accepted through this interface.
pub trait KagemushaNativeCoreBootstrapSourceV1: Send + Sync + 'static {
    fn recheck_originals(&self) -> Result<(), Error>;
    /// Produce exact original physical bootstrap material for the retained issuer admission.
    /// A missing signer, real paired proof, Guard, current ledger/service authority or qualified
    /// device must fail. This callback does not implement software Core initialization/dispatch.
    fn inputs_for_admission(
        &self,
        native_storage_path: &str,
        admission: &FreshIssuerAdmissionV1,
    ) -> Result<KagemushaNativeCoreBootstrapInputsV1, Error>;
}

enum Stage {
    Admission(FreshIssuerAdmissionV1),
    Checkpoint {
        original: Box<Checkpoint>,
        transactions: KagemushaHardwareTransactionJournalV1,
        native_key: iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1,
        signer: Arc<dyn KagemushaNativeCoreAuthorizationSignerV1>,
    },
    Complete(Arc<KagemushaAuthenticatedRecoveredCoordinatorV1>),
    Frozen,
}

/// Actual software bootstrap owner. It retains issuer admission before any physical call,
/// then only an exclusive initialized checkpoint until the original hardware CAS is verified.
pub struct KagemushaNativeFreshCoreBootstrapV1 {
    path: String,
    source: Arc<dyn KagemushaNativeCoreBootstrapSourceV1>,
    stage: Stage,
}
impl KagemushaNativeFreshCoreBootstrapV1 {
    /// Retain the consuming phase-5 handoff; this step creates no files or monetary owner.
    pub fn retain(
        path: String,
        admission: FreshIssuerAdmissionV1,
        source: Arc<dyn KagemushaNativeCoreBootstrapSourceV1>,
    ) -> Result<Self, Error> {
        kagemusha_core_coordinator_validate_storage_path_v1(path.as_bytes())
            .map_err(|_| Error::Rejected)?;
        admission.require_live().map_err(|_| Error::Rejected)?;
        source.recheck_originals()?;
        admission.require_live().map_err(|_| Error::Rejected)?;
        Ok(Self {
            path,
            source,
            stage: Stage::Admission(admission),
        })
    }

    /// Advance the same original bootstrap, never replace existing files or reopen issuer work.
    /// Failed INITIAL hardware publication retains its same initialized checkpoint and WAL.
    pub fn advance(&mut self) -> Result<Arc<KagemushaAuthenticatedRecoveredCoordinatorV1>, Error> {
        self.source.recheck_originals()?;
        let previous = std::mem::replace(&mut self.stage, Stage::Frozen);
        match previous {
            Stage::Admission(admission) => {
                let input = match self.source.inputs_for_admission(&self.path, &admission) {
                    Ok(input) => input,
                    Err(error) => {
                        self.stage = Stage::Admission(admission);
                        return Err(error);
                    }
                };
                self.source.recheck_originals()?;
                validate_input_paths(
                    Path::new(&self.path),
                    [
                        &input.history_directory,
                        &input.journal_bundle_directory,
                        &input.transaction_directory,
                    ],
                )?;
                let native_key = *admission.native_authorization_public_key();
                input.signer.recheck_originals()?;
                if input.signer.original_public_key()? != native_key
                    || input.trusted_current_time_ms == 0
                    || input.state_nonce_commitment == [0; 32]
                    || input.checkpoint_operation_id == [0; 32]
                {
                    return Err(Error::Rejected);
                }
                input.signer.recheck_originals()?;
                let (enrollment, possession) = admission
                    .into_current_bootstrap_evidence(input.trusted_current_time_ms)
                    .map_err(|_| Error::Rejected)?;
                let authorization = {
                    let selection = KagemushaAuthenticatedBootstrapProvingSelectionV1::from_verified_enrollment(
                        &enrollment, &possession, input.recursive_verifier.clone(),
                        input.state_nonce_commitment, input.durable_capacity, input.trusted_current_time_ms,
                    ).map_err(|_| Error::Rejected)?;
                    let prover = KagemushaProductionProverV1::load_bootstrap(
                        &selection,
                        input.prover_profile,
                        super::native_core_work::Resolver(input.artifact_resolver),
                    )
                    .map_err(|_| Error::Rejected)?;
                    let hash_claim = prover
                        .prove_bootstrap_state_hash_claim(&selection)
                        .map_err(|_| Error::Rejected)?;
                    let proof = prover
                        .prove_bootstrap_state(&selection, &hash_claim)
                        .map_err(|_| Error::Rejected)?;
                    self.source.recheck_originals()?;
                    BootstrapAuthorizationV1 {
                        proof: proof.proof,
                        guard_bundle: input.original_guard_bundle,
                    }
                };
                let transactions = KagemushaHardwareTransactionJournalV1::create_new(
                    &input.transaction_directory,
                    input.hardware_verifier.clone(),
                    input.transaction_transport,
                )
                .map_err(|_| Error::Rejected)?;
                let stage = KagemushaAuthenticatedCoreOwnerV1::stage_enrolled_bootstrap(
                    enrollment,
                    possession,
                    input.state_nonce_commitment,
                    input.durable_capacity,
                    authorization,
                    input.recursive_verifier,
                    input.hardware_verifier,
                    &input.history_directory,
                    input.overlay_capacity_bytes,
                )
                .map_err(|_| Error::Rejected)?;
                let original = stage
                    .initialize_journals(
                        &input.journal_bundle_directory,
                        input.coordinator_capacity_bytes,
                        input.checkpoint_operation_id,
                    )
                    .map_err(|_| Error::Rejected)?;
                self.stage = Stage::Checkpoint {
                    original: Box::new(original),
                    transactions,
                    native_key,
                    signer: input.signer,
                };
                self.source.recheck_originals()?;
                self.advance()
            }
            Stage::Checkpoint {
                original,
                mut transactions,
                native_key,
                signer,
            } => {
                let statement = original.statement().clone();
                let guard = match transactions.commit_or_recover(
                    statement.operation_id,
                    KagemushaHardwareTransactionV1::RecoveryCheckpoint(statement),
                ) {
                    Ok(guard) => guard,
                    Err(_) => {
                        self.stage = Stage::Checkpoint {
                            original,
                            transactions,
                            native_key,
                            signer,
                        };
                        return Err(Error::Unavailable);
                    }
                };
                if let Err(error) = self.source.recheck_originals() {
                    self.stage = Stage::Checkpoint {
                        original,
                        transactions,
                        native_key,
                        signer,
                    };
                    return Err(error);
                }
                let wallet = match original.finish_or_retain(guard) {
                    Ok(wallet) => wallet,
                    Err((original, _)) => {
                        self.stage = Stage::Checkpoint {
                            original,
                            transactions,
                            native_key,
                            signer,
                        };
                        return Err(Error::Rejected);
                    }
                };
                let owner = KagemushaAuthenticatedCoreOwnerV1::from_bootstrapped_wallet(
                    wallet,
                    transactions,
                )
                .map_err(|_| Error::Rejected)?;
                self.source.recheck_originals()?;
                let backend = Arc::new(
                    KagemushaAuthenticatedRecoveredCoordinatorV1::from_native_owner(
                        self.path.clone(),
                        owner,
                        native_key,
                        signer,
                    )?,
                );
                self.source.recheck_originals()?;
                self.stage = Stage::Complete(backend.clone());
                Ok(backend)
            }
            Stage::Complete(backend) => {
                self.stage = Stage::Complete(backend.clone());
                Ok(backend)
            }
            Stage::Frozen => Err(Error::Rejected),
        }
    }
}

fn validate_input_paths(base: &Path, paths: [&PathBuf; 3]) -> Result<(), Error> {
    for (index, path) in paths.iter().enumerate() {
        if !path.is_absolute()
            || *path == base
            || !path.starts_with(base)
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
            || paths[..index].contains(path)
        {
            return Err(Error::Rejected);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bootstrap_paths_require_distinct_native_selected_children() {
        let base = Path::new("/private/native-wallet");
        let valid = [
            base.join("history"),
            base.join("snapshot"),
            base.join("transactions"),
        ];
        assert_eq!(
            validate_input_paths(base, [&valid[0], &valid[1], &valid[2]]),
            Ok(())
        );
        for invalid in [
            PathBuf::from("relative"),
            base.to_path_buf(),
            PathBuf::from("/another-owner/journal"),
            base.join("../escaped"),
            valid[0].clone(),
        ] {
            assert_eq!(
                validate_input_paths(base, [&valid[0], &valid[1], &invalid]),
                Err(Error::Rejected)
            );
        }
    }
}
