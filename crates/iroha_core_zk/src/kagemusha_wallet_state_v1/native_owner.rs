//! One admitted native wallet owner for preparation, proof imports and folding.

use super::*;
mod bootstrap;
mod review;
mod sessions;
use crate::{
    kagemusha_wallet_advance_v1::{KagemushaWalletFsV1, KagemushaWalletPlatformV1},
    kagemusha_wallet_artifacts_v1::{
        InstalledVerifierPackV1,
        producer_inventory::{OriginalSourceV1, QualifiedWalletSourcesV1},
    },
    kagemusha_wallet_finality_v1::derive_history_anchor,
    kagemusha_wallet_intake_v1::AdmittedWalletV1,
    kagemusha_wallet_preparation_v1::{
        NativeAdvanceCheckV1, NativeChoicesV1, PreparationV1, PreparedOperationV1,
    },
    kagemusha_wallet_proofs_v1::Error as ProofError,
};
use iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier;
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::keys::pk::artifact::ReadConfig;
pub use review::{NativeOperationReviewV1, ReviewedOperationV1};
use std::sync::{Arc, Mutex};

/// Concrete native owner constructed only by consuming actual original admission.
/// The original proving store and custody provider each have one exclusive owner.
pub struct NativeWalletProofsV1<F: KagemushaWalletFsV1, P, S> {
    installed: Arc<InstalledVerifierPackV1>,
    sources: Arc<QualifiedWalletSourcesV1>,
    originals: Mutex<S>,
    observations: NativeObservationsV1<F, P>,
    worker: NativeFoldWorkerV1,
    read: ReadConfig,
    budget: MemoryBudget,
    chain: String,
    enrollment: KagemushaWalletCredentialV1,
    enrollment_certificates: Vec<u8>,
}

fn proof<T>(value: Result<T, ProofError>) -> Result<T, Error> {
    value.map_err(|error| match error {
        ProofError::Unavailable => Error::ArtifactsUnavailable("native operation artifact"),
        _ => Error::Proof("native operation source or proof"),
    })
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1> AdmittedWalletV1<F, P> {
    /// Consume original admission and bind the one complete producer grant to its native root.
    /// No production wallet can be created from a foreign verifier verdict or loose key list.
    ///
    /// # Errors
    /// Changed installation/genesis, unavailable custody or inconsistent native state.
    pub fn into_coordinator<S: OriginalSourceV1 + Send>(
        self,
        native_genesis: &SumeragiFinalityVerifier,
        originals: S,
        read: ReadConfig,
        budget: MemoryBudget,
    ) -> Result<
        Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>,
        Error,
    > {
        let (provider, installed, sources, slot, enrollment, _, _, _, certificates, _, _) =
            self.into_parts();
        let anchor = derive_history_anchor(native_genesis)
            .map_err(|_| Error::Proof("native Global root"))?;
        if &anchor != sources.finality().anchor() {
            return Err(Error::Proof("native finality source binding"));
        }
        let worker =
            NativeFoldWorkerV1::new(Arc::clone(&installed), Arc::clone(&sources), read, budget)?;
        let handle = AdvanceHandle::new(provider, slot);
        let archive = handle.archive(enrollment.body.scheme_id, enrollment.body.wallet_id);
        let owner = NativeWalletProofsV1 {
            installed,
            sources,
            originals: Mutex::new(originals),
            observations: handle.observations(),
            worker,
            read,
            budget,
            chain: native_genesis.chain_id().to_owned(),
            enrollment,
            enrollment_certificates: certificates,
        };
        Coordinator::new(
            handle,
            archive,
            owner,
            enrollment.body.scheme_id,
            enrollment.body.wallet_id,
        )
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    NativeWalletProofsV1<F, P, S>
{
    fn derive(
        &self,
        request: &NativeIntentV1,
        source: &PreparationSourceV1<'_>,
        custody: &mut PreparationCustodyV1<'_>,
        bytes: &[u8],
    ) -> Result<PreparedOperationV1, Error> {
        let preparation = proof(PreparationV1::new(&self.installed))?;
        let choices = NativeChoicesV1::decode(
            bytes,
            self.installed.verifier().manifest_digest(),
            request,
            source,
        )?;
        preparation.prepare_operation(
            request,
            source,
            custody,
            &self.sources,
            choices.nonce(),
            choices.time().as_ref(),
            self.budget,
        )
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    NativePreparation for NativeWalletProofsV1<F, P, S>
{
    fn plan_preparation(
        &self,
        request: &NativeIntentV1,
        source: &PreparationSourceV1<'_>,
        custody: &mut PreparationCustodyV1<'_>,
    ) -> Result<Vec<u8>, Error> {
        let choices = NativeChoicesV1::fresh(
            self.installed.verifier().manifest_digest(),
            request,
            source,
            &self.observations,
        )?;
        let bytes = choices.encode()?;
        self.derive(request, source, custody, &bytes)?;
        Ok(bytes)
    }
    fn validate_preparation(
        &self,
        request: &NativeIntentV1,
        source: &PreparationSourceV1<'_>,
        custody: &mut PreparationCustodyV1<'_>,
        plan: &[u8],
    ) -> Result<(), Error> {
        self.derive(request, source, custody, plan).map(|_| ())
    }
    fn prove_preparation(
        &self,
        request: &NativeIntentV1,
        source: &PreparationSourceV1<'_>,
        custody: &mut PreparationCustodyV1<'_>,
        plan: &[u8],
    ) -> Result<FrozenTransition, Error> {
        let prepared = self.derive(request, source, custody, plan)?;
        let mut originals = self
            .originals
            .lock()
            .map_err(|_| Error::ArtifactsUnavailable("proving original owner poisoned"))?;
        prepared.prove(
            &proof(PreparationV1::new(&self.installed))?,
            &self.sources,
            &mut *originals,
            self.read,
            self.budget,
        )
    }
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> NativeProofs
    for NativeWalletProofsV1<F, P, S>
{
    type AdvanceCheck = NativeAdvanceCheckV1;
    fn enrollment_certificates(
        &self,
        credential: &KagemushaWalletCredentialV1,
    ) -> Result<Vec<u8>, Error> {
        if credential != &self.enrollment {
            return Err(Error::Proof("enrollment original credential"));
        }
        Ok(self.enrollment_certificates.clone())
    }
    fn ledger_scope(&self) -> Result<(KagemushaWalletSchemeV1, String), Error> {
        Ok((*self.installed.verifier().scheme(), self.chain.clone()))
    }
    fn verify_transition(
        &self,
        next: &FrozenTransition,
        predecessor: Option<&ReleasedStep>,
        folded: Option<&KagemushaWalletFoldRecordV1>,
        custody: &mut TransitionCustodyV1<'_>,
    ) -> Result<Self::AdvanceCheck, Error> {
        next.validate()?;
        let preparation = proof(PreparationV1::new(&self.installed))?;
        if next.capsule.kind == KagemushaWalletOperationKindV1::Bootstrap {
            if predecessor.is_some()
                || folded.is_some()
                || custody.prepared().is_some()
                || next.credential != self.enrollment
            {
                return Err(Error::Proof("bootstrap native admission"));
            }
            let owner = preparation.authenticate_credential_set(
                &valid(next.credential.to_canonical_bytes())?,
                &self.enrollment_certificates,
            )?;
            let KagemushaWalletEffectV1::Bootstrap {
                enrollment_marker, ..
            } = next.capsule.statement.effect
            else {
                return Err(Error::Proof("Bootstrap effect"));
            };
            let expected = proof(preparation.prepare_bootstrap(
                &owner,
                enrollment_marker,
                next.capsule.successor_state.core.state_nonce,
            ))?;
            let frozen = proof(preparation.freeze_bootstrap(
                &owner,
                &expected,
                next.capsule.step_proof.clone(),
                self.budget,
            ))?;
            if &frozen != next {
                return Err(Error::Proof("Bootstrap derived capsule"));
            }
            return Ok(NativeAdvanceCheckV1::without_time());
        }
        let source = PreparationSourceV1 {
            released: predecessor.ok_or(Error::NoHead)?,
            folded,
        };
        let (request, plan, selected) = custody
            .prepared()
            .ok_or(Error::WitnessLost("ordinary native preparation"))?;
        let prepared = self.derive(request, &source, selected, plan)?;
        prepared.verify_frozen(&preparation, next, self.budget)?;
        prepared.advance_check(&preparation, selected)
    }
    fn check_advance(&self, check: Self::AdvanceCheck) -> Result<(), Error> {
        check.check(&self.observations)
    }
    fn verify_lineage(&self, lineage: &KagemushaWalletLineageV1) -> Result<(), Error> {
        proof(
            self.installed
                .verifier()
                .verify_lineage(lineage, self.budget),
        )
    }
    fn fold_schedule(
        &self,
        witness: &ReleasedStep,
        predecessor: Option<&KagemushaWalletFoldRecordV1>,
        custody: Option<&mut FoldCustodyV1<'_>>,
    ) -> Result<Vec<CheckpointLayout>, Error> {
        self.worker.schedule(
            witness,
            predecessor,
            custody.ok_or(Error::WitnessLost("native fold custody"))?,
        )
    }
    fn fold_next(
        &self,
        witness: &ReleasedStep,
        predecessor: Option<&KagemushaWalletFoldRecordV1>,
        checkpoints: &[Vec<u8>],
        custody: Option<&mut FoldCustodyV1<'_>>,
        cancellation: &Cancellation,
    ) -> Result<FoldProgress, Error> {
        let mut originals = self
            .originals
            .lock()
            .map_err(|_| Error::ArtifactsUnavailable("proving original owner poisoned"))?;
        self.worker.next(
            witness,
            predecessor,
            checkpoints,
            cancellation,
            &mut *originals,
            custody.ok_or(Error::WitnessLost("native fold custody"))?,
        )
    }
}
