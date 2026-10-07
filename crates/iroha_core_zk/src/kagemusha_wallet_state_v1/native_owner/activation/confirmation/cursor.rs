//! Activation-bound recursive history: later global ledger activity cannot skip Activate.
use super::super::super::ledger_producer::history_error;
use super::*;
use crate::kagemusha_wallet_finality_v1::{HISTORY_ORIGINAL_MAX_BYTES_V1, HistoryOriginalV1};
use iroha_data_model::sumeragi_finality::{
    MAX_FINALITY_CHECKPOINT_BYTES, SumeragiFinalityCheckpoint,
};
use iroha_kagemusha_proof::finality::native::HistoryPrefix;

const CURSOR_MAX: usize =
    TRANSACTION_MAX + MAX_FINALITY_CHECKPOINT_BYTES + HISTORY_ORIGINAL_MAX_BYTES_V1 + 4096;

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::ActivationFinalityCursorV1")]
pub(super) struct Cursor {
    activation: [u8; 32],
    signed_transaction: Vec<u8>,
    // Owned complete originals, never aliases to a global checkpoint which cleanup can retire.
    checkpoint: Vec<u8>,
    prefix: HistoryOriginalV1,
}
impl Cursor {
    fn require(&self, plan: &Plan, signed_wire: &[u8]) -> Result<(), Error> {
        if Some(self.activation) != plan.output
            || self.activation == [0; 32]
            || self.signed_transaction.is_empty()
            || self.signed_transaction.len() > TRANSACTION_MAX
            || self.checkpoint.is_empty()
            || self.checkpoint.len() > MAX_FINALITY_CHECKPOINT_BYTES
        {
            return Err(Error::WitnessLost("activation cursor binding"));
        }
        if self.signed_transaction != signed_wire {
            return Err(Error::OperationConflict);
        }
        Ok(())
    }
}
impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    pub(super) fn retained_activation_cursor(
        &mut self,
        plan: &Plan,
        signed_wire: &[u8],
    ) -> Result<Option<Cursor>, Error> {
        let Some(address) = plan.cursor else {
            return Ok(None);
        };
        if plan.retired_cursor == Some(address) || plan.confirmation.is_some() {
            return Err(Error::WitnessLost("activation cursor selection"));
        }
        let cursor: Cursor = archive::decode(&self.archive.read_object(&address, CURSOR_MAX)?)?;
        cursor.require(plan, signed_wire)?;
        Ok(Some(cursor))
    }
    pub(super) fn clean_activation_cursor(&mut self) -> Result<(), Error> {
        let (root, mut manifest) = self.sync_manifest()?;
        let Some(address) = manifest.activation else {
            return Ok(());
        };
        let mut plan: Plan = archive::decode(&self.archive.read_object(&address, PLAN_MAX)?)?;
        plan.require(&self.scheme_id, &self.wallet_id, &plan.asset)?;
        if let Some(retired) = plan.retired_cursor {
            if plan.cursor == Some(retired)
                || plan.confirmation == Some(retired)
                || plan.output == Some(retired)
            {
                return Err(Error::WitnessLost(
                    "activation cleanup selects current original",
                ));
            }
            self.archive.remove(ArchiveKey::Object(retired))?;
            plan.retired_cursor = None;
            manifest.activation = Some(
                self.archive
                    .write_object(&archive::encode(&plan)?, PLAN_MAX)?,
            );
            self.publish_manifest(root, &manifest)?;
        }
        Ok(())
    }
    fn publish_activation_cursor(
        &mut self,
        root: [u8; 32],
        mut manifest: manifest::Manifest,
        plan: &Plan,
        cursor: &Cursor,
    ) -> Result<(), Error> {
        cursor.require(plan, &cursor.signed_transaction)?;
        let selected = manifest
            .activation
            .ok_or(Error::WitnessLost("activation cursor plan"))?;
        if self.manifest()?.0 != root
            || plan.confirmation.is_some()
            || plan.retired_cursor.is_some()
            || self.archive.read_object(&selected, PLAN_MAX)? != archive::encode(plan)?
        {
            return Err(Error::WitnessLost("activation cursor source changed"));
        }
        let mut next = plan.clone();
        let address = self
            .archive
            .write_object(&archive::encode(cursor)?, CURSOR_MAX)?;
        next.retired_cursor = next.cursor.filter(|previous| *previous != address);
        next.cursor = Some(address);
        manifest.activation = Some(
            self.archive
                .write_object(&archive::encode(&next)?, PLAN_MAX)?,
        );
        self.publish_manifest(root, &manifest)?;
        self.clean_activation_cursor()
    }
}
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>
{
    fn activation_cursor_root(&self) -> Result<(), Error> {
        let genesis = &self.proofs.genesis;
        let (scheme, chain) = self.proofs.ledger_scope()?;
        if scheme.scheme_id() != self.scheme_id
            || chain != genesis.chain_id()
            || genesis.initial_epoch().network_id.as_bytes() != &scheme.network_id
            || !matches!(
                genesis.root_scope(),
                Ok(iroha_data_model::block::consensus::SumeragiRootScope::Global)
            )
        {
            return Err(Error::Proof("activation cursor root scope"));
        }
        Ok(())
    }
    pub(super) fn restore_activation_cursor(
        &self,
        cursor: &Cursor,
    ) -> Result<
        (
            SumeragiFinalityVerifier,
            SumeragiFinalityCheckpoint,
            HistoryPrefix,
        ),
        Error,
    > {
        self.activation_cursor_root()?;
        let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&cursor.checkpoint)
            .map_err(|_| Error::WitnessLost("activation cursor checkpoint"))?;
        let genesis = &self.proofs.genesis;
        let verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &checkpoint,
            &genesis.initial_epoch().network_id,
            &self.proofs.chain,
        )
        .map_err(|_| Error::WitnessLost("activation cursor checkpoint root"))?;
        if verifier.initial_epoch() != genesis.initial_epoch()
            || verifier.instance() != genesis.instance()
        {
            return Err(Error::WitnessLost("activation cursor genesis binding"));
        }
        let prefix = cursor
            .prefix
            .clone()
            .restore_producer(
                self.proofs.sources.finality_producer().installed(),
                self.proofs.budget,
            )
            .map_err(history_error)?;
        if checkpoint.height().checked_add(1) != Some(prefix.state().next_height) {
            return Err(Error::WitnessLost("activation cursor recursive height"));
        }
        Ok((verifier, checkpoint, prefix))
    }
    fn activation_cursor_intake(
        &mut self,
        plan: &Plan,
        signed_wire: &[u8],
    ) -> Result<SignedTransaction, Error> {
        plan.require(&self.scheme_id, &self.wallet_id, &self.proofs.asset)?;
        self.authenticate_activation_plan(plan)?;
        self.activation_cursor_root()?;
        let original = self
            .retained_activation(plan)?
            .ok_or(Error::Invalid("activation output absent"))?;
        signed_activate(
            signed_wire,
            &self.proofs.genesis.initial_epoch().network_id,
            &self.scheme_id,
            &plan.credential.body.account_digest,
            &original,
        )
    }
    /// Read durable progress for this exact signed Activate. No HTTP status supplies confirmation.
    /// # Errors
    /// Changed transaction, missing selected original, corrupt cursor or untrusted installation.
    pub fn activation_finality_progress(
        &mut self,
        signed_wire: &[u8],
    ) -> Result<ActivationFinalityProgressV1, Error> {
        let (_, manifest) = self.sync_manifest()?;
        let address = manifest
            .activation
            .ok_or(Error::Invalid("activation plan absent"))?;
        let plan: Plan = archive::decode(&self.archive.read_object(&address, PLAN_MAX)?)?;
        self.activation_cursor_intake(&plan, signed_wire)?;
        if let Some(confirmed) = self.retained_activation_confirmation(&plan)? {
            if confirmed.signed_transaction != signed_wire {
                return Err(Error::OperationConflict);
            }
            return Ok(ActivationFinalityProgressV1::Confirmed(
                confirmed.progress(),
            ));
        }
        let Some(cursor) = self.retained_activation_cursor(&plan, signed_wire)? else {
            return Ok(ActivationFinalityProgressV1::NotStarted);
        };
        let (_, checkpoint, _) = self.restore_activation_cursor(&cursor)?;
        Ok(ActivationFinalityProgressV1::Verifying(ledger::progress(
            &checkpoint,
        )))
    }
    /// Verify and prove one exact next block, then atomically retain either progress or successful
    /// Activate inclusion. This cursor never advances past the matching transaction unconfirmed.
    /// # Errors
    /// Gaps, invalid finality, changed transaction, failed execution, missing proving originals,
    /// failed recursive proof, or uncertain publication leave first Load unavailable.
    pub fn ingest_activation_finality(
        &mut self,
        signed_wire: &[u8],
        original: &[u8],
    ) -> Result<ActivationFinalityProgressV1, Error> {
        let _payment = self.scheduler.payment();
        let candidate = ledger::proof_original(original)?;
        self.clean_activation_cursor()?;
        let (root, manifest) = self.sync_manifest()?;
        let address = manifest
            .activation
            .ok_or(Error::Invalid("activation plan absent"))?;
        let plan: Plan = archive::decode(&self.archive.read_object(&address, PLAN_MAX)?)?;
        let signed = self.activation_cursor_intake(&plan, signed_wire)?;
        if let Some(confirmed) = self.retained_activation_confirmation(&plan)? {
            if confirmed.signed_transaction != signed_wire {
                return Err(Error::OperationConflict);
            }
            return Ok(ActivationFinalityProgressV1::Confirmed(
                confirmed.progress(),
            ));
        }
        let selected = self.retained_activation_cursor(&plan, signed_wire)?;
        let (mut verifier, previous, prefix) = if let Some(cursor) = &selected {
            let (verifier, checkpoint, prefix) = self.restore_activation_cursor(cursor)?;
            (verifier, Some(checkpoint), Some(prefix))
        } else {
            (self.proofs.genesis.as_ref().clone(), None, None)
        };
        if let Some(previous) = &previous {
            if candidate.height() == previous.height() {
                verifier
                    .verify_retained_decision(&candidate)
                    .map_err(|_| Error::Proof("activation cursor retry"))?;
                return Ok(ActivationFinalityProgressV1::Verifying(ledger::progress(
                    previous,
                )));
            }
            if previous.height().checked_add(1) != Some(candidate.height()) {
                return Err(Error::Invalid("activation cursor exact next height"));
            }
        } else if candidate.height() != 1 {
            return Err(Error::Invalid("activation cursor starts at genesis"));
        }
        let verified = if previous.is_none() {
            verifier
                .verify_retained_decision(&candidate)
                .or_else(|_| verifier.verify(&candidate))
                .map_err(|_| Error::Proof("activation cursor genesis"))?
        } else {
            verifier
                .verify(&candidate)
                .map_err(|_| Error::Proof("activation cursor continuity"))?
        };
        let expected = TransactionEntrypoint::External(signed.clone());
        let included = verified
            .block()
            .network_entrypoints()
            .any(|entry| entry.hash() == expected.hash());
        // Rejected or substituted execution is not permission to skip the exact target block.
        let confirmation = if included {
            Some(successful_inclusion(
                &verified,
                &self.proofs.genesis.initial_epoch().network_id,
                signed,
            )?)
        } else {
            None
        };
        let prefix = self.prove_recursive_successor(prefix, &verified)?;
        if let Some(progress) = confirmation {
            let confirmation = Confirmation {
                version: 1,
                activation: plan
                    .output
                    .ok_or(Error::WitnessLost("activation output absent"))?,
                signed_transaction: signed_wire.to_vec(),
                height: progress.height,
                block_hash: progress.block_hash,
            };
            self.publish_activation_confirmation(root, manifest, &plan, &confirmation)?;
            return Ok(ActivationFinalityProgressV1::Confirmed(progress));
        }
        let checkpoint = verifier
            .export_checkpoint(&candidate)
            .map_err(|_| Error::Proof("activation cursor checkpoint"))?;
        let cursor = Cursor {
            activation: plan
                .output
                .ok_or(Error::WitnessLost("activation output absent"))?,
            signed_transaction: signed_wire.to_vec(),
            checkpoint: checkpoint
                .encode_canonical()
                .map_err(|_| Error::Proof("activation cursor checkpoint encoding"))?,
            prefix: HistoryOriginalV1::from_prefix(&prefix),
        };
        self.publish_activation_cursor(root, manifest, &plan, &cursor)?;
        Ok(ActivationFinalityProgressV1::Verifying(ledger::progress(
            &checkpoint,
        )))
    }
}
