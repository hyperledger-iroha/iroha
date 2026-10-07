//! Online native finality progress and payout evidence under the selected wallet archive.
use super::*;
use iroha_data_model::sumeragi_finality::{
    MAX_FINALITY_BLOCK_BYTES, MAX_FINALITY_CHECKPOINT_BYTES, SumeragiFinalityCheckpoint,
    SumeragiFinalityProof, WorldStateSnapshotV1,
};

/// One bounded original proof: maximum block plus bounded committee and framing overhead.
/// This online control surface is separate from the 10,000-byte Payment envelope.
pub const LEDGER_PROOF_MAX_BYTES_V1: usize = MAX_FINALITY_BLOCK_BYTES + 4 * 1024 * 1024;
/// Maximum original immutable payout row, including canonical Norito framing.
pub const PAYOUT_RECORD_MAX_BYTES_V1: usize = 1024;
/// Selected native finality progress; this alone grants no fee-payout acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerProgressV1 {
    /// Last durably selected authenticated height.
    pub height: u64,
    /// Exact certified block header hash at that height.
    pub block_hash: [u8; 32],
}
pub(super) fn progress(checkpoint: &SumeragiFinalityCheckpoint) -> LedgerProgressV1 {
    LedgerProgressV1 {
        height: checkpoint.height(),
        block_hash: *checkpoint.block_hash().as_ref(),
    }
}
pub(super) fn proof_original(bytes: &[u8]) -> Result<SumeragiFinalityProof, Error> {
    if bytes.is_empty() || bytes.len() > LEDGER_PROOF_MAX_BYTES_V1 {
        return Err(Error::Invalid("ledger proof original bound"));
    }
    let proof: SumeragiFinalityProof =
        archive::decode(bytes).map_err(|_| Error::Invalid("ledger proof canonical original"))?;
    if proof.committee.len() > 31 || proof.block_wire.len() > MAX_FINALITY_BLOCK_BYTES {
        return Err(Error::Invalid("ledger proof decoded bound"));
    }
    Ok(proof)
}
fn payout_original(bytes: &[u8]) -> Result<KagemushaWalletPayoutRecordV1, Error> {
    if bytes.is_empty() || bytes.len() > PAYOUT_RECORD_MAX_BYTES_V1 {
        return Err(Error::Invalid("payout original bound"));
    }
    archive::decode(bytes).map_err(|_| Error::Invalid("payout canonical original"))
}
impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    pub(super) fn selected_ledger(
        &mut self,
        manifest: &manifest::Manifest,
        genesis: &SumeragiFinalityVerifier,
    ) -> Result<(SumeragiFinalityVerifier, Option<SumeragiFinalityCheckpoint>), Error> {
        let (scheme, chain) = self.proofs.ledger_scope()?;
        if scheme.scheme_id() != self.scheme_id
            || chain != genesis.chain_id()
            || genesis.initial_epoch().network_id.as_bytes() != &scheme.network_id
            || !matches!(
                genesis.root_scope(),
                Ok(iroha_data_model::block::consensus::SumeragiRootScope::Global)
            )
        {
            return Err(Error::Proof("native ledger root scope"));
        }
        let Some(address) = manifest.ledger_checkpoint else {
            return Ok((genesis.clone(), None));
        };
        let bytes = self
            .archive
            .read_object(&address, MAX_FINALITY_CHECKPOINT_BYTES)?;
        let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&bytes)
            .map_err(|_| Error::WitnessLost("selected ledger checkpoint original"))?;
        if checkpoint.network_id().as_bytes() != &scheme.network_id
            || checkpoint.chain_id() != chain
            || manifest.ledger_retired == Some(address)
        {
            return Err(Error::WitnessLost("selected ledger checkpoint scope"));
        }
        // This checkpoint came only from a verified local prefix and is selected by Advance's
        // rollback-sensitive manifest. No incoming checkpoint or peer verdict reaches here.
        let verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &checkpoint,
            &genesis.initial_epoch().network_id,
            &chain,
        )
        .map_err(|_| Error::WitnessLost("selected ledger checkpoint verification"))?;
        if verifier.initial_epoch() != genesis.initial_epoch()
            || verifier.instance() != genesis.instance()
        {
            return Err(Error::WitnessLost("selected ledger genesis binding"));
        }
        Ok((verifier, Some(checkpoint)))
    }
    pub(super) fn clean_retired_ledger(&mut self) -> Result<(), Error> {
        let (root, mut manifest) = self.sync_manifest()?;
        let Some(address) = manifest.ledger_retired else {
            return Ok(());
        };
        if manifest.ledger_checkpoint == Some(address) {
            return Err(Error::WitnessLost(
                "ledger cleanup selects current original",
            ));
        }
        self.archive.remove(ArchiveKey::Object(address))?;
        manifest.ledger_retired = None;
        self.publish_manifest(root, &manifest).map(|_| ())
    }
    pub(super) fn clean_recursive_retired(&mut self) -> Result<(), Error> {
        let (root, mut manifest) = self.sync_manifest()?;
        if let Some(address) = manifest.recursive_retired {
            if manifest.recursive_checkpoint == Some(address) {
                return Err(Error::WitnessLost("recursive cleanup selected prefix"));
            }
            self.archive.remove(ArchiveKey::Object(address))?;
            manifest.recursive_retired = None;
            self.publish_manifest(root, &manifest)?;
        }
        self.clean_retired_ledger()
    }
    // Single native verification and atomic paired-publication path. Production supplies the
    // installed recursive prover; custody tests explicitly replace only that expensive proof step.
    pub(super) fn ingest_ledger_transition<R>(
        &mut self,
        genesis: &SumeragiFinalityVerifier,
        bytes: &[u8],
        restore: impl FnOnce(&mut Self, &manifest::Manifest) -> Result<Option<R>, Error>,
        next_height: impl Fn(&R) -> u64,
        prove: impl FnOnce(
            &mut Self,
            Option<R>,
            &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
        ) -> Result<(u64, Vec<u8>), Error>,
    ) -> Result<LedgerProgressV1, Error> {
        if !matches!(self.status()?, SlotStatus::Released(_)) {
            return Err(Error::NoHead);
        }
        let candidate = proof_original(bytes)?;
        self.clean_recursive_retired()?;
        let (root, mut manifest) = self.sync_manifest()?;
        let (mut verifier, previous) = self.selected_ledger(&manifest, genesis)?;
        if manifest.ledger_checkpoint.is_some() != manifest.recursive_checkpoint.is_some()
            || manifest
                .recursive_checkpoint
                .is_some_and(|address| manifest.recursive_retired == Some(address))
        {
            return Err(Error::WitnessLost("paired ledger prefix selection"));
        }
        let prefix = restore(self, &manifest)?;
        if previous.is_some() != prefix.is_some() {
            return Err(Error::WitnessLost("paired ledger prefix restoration"));
        }
        if let Some(previous) = previous.as_ref() {
            let prefix = prefix
                .as_ref()
                .ok_or(Error::WitnessLost("recursive selected prefix"))?;
            if next_height(prefix)
                != previous
                    .height()
                    .checked_add(1)
                    .ok_or(Error::Invalid("ledger height overflow"))?
            {
                return Err(Error::WitnessLost("recursive native height binding"));
            }
            if candidate.height() == previous.height() {
                verifier
                    .verify_retained_decision(&candidate)
                    .map_err(|_| Error::Proof("recursive ledger retry decision"))?;
                return Ok(progress(previous));
            }
            if candidate.height() != next_height(prefix) {
                return Err(Error::Invalid("recursive ledger requires next height"));
            }
        }
        let block = if previous.is_none() {
            if candidate.height() != 1 {
                return Err(Error::Invalid("recursive ledger starts at genesis"));
            }
            verifier
                .verify_retained_decision(&candidate)
                .or_else(|_| verifier.verify(&candidate))
                .map_err(|_| Error::Proof("recursive ledger genesis"))?
        } else {
            verifier
                .verify(&candidate)
                .map_err(|_| Error::Proof("recursive ledger continuity"))?
        };
        let (proved_next, prefix_original) = prove(self, prefix, &block)?;
        if proved_next
            != candidate
                .height()
                .checked_add(1)
                .ok_or(Error::Invalid("ledger height overflow"))?
        {
            return Err(Error::Proof("recursive successor height"));
        }
        let checkpoint = verifier
            .export_checkpoint(&candidate)
            .map_err(|_| Error::Proof("recursive native checkpoint"))?;
        let checkpoint_bytes = checkpoint
            .encode_canonical()
            .map_err(|_| Error::Proof("recursive native checkpoint encoding"))?;
        let checkpoint_address = self
            .archive
            .write_object(&checkpoint_bytes, MAX_FINALITY_CHECKPOINT_BYTES)?;
        let prefix_address = self.archive.write_object(
            &prefix_original,
            crate::kagemusha_wallet_finality_v1::HISTORY_ORIGINAL_MAX_BYTES_V1,
        )?;
        manifest.ledger_retired = manifest.ledger_checkpoint;
        manifest.ledger_checkpoint = Some(checkpoint_address);
        manifest.recursive_retired = manifest.recursive_checkpoint;
        manifest.recursive_checkpoint = Some(prefix_address);
        self.publish_manifest(root, &manifest)?;
        self.clean_recursive_retired()?;
        Ok(progress(&checkpoint))
    }

    fn ledger_progress_selected(
        &mut self,
        genesis: &SumeragiFinalityVerifier,
    ) -> Result<Option<LedgerProgressV1>, Error> {
        let (_, manifest) = self.sync_manifest()?;
        let (_, checkpoint) = self.selected_ledger(&manifest, genesis)?;
        Ok(checkpoint.as_ref().map(progress))
    }
    fn acknowledge_payout_originals(
        &mut self,
        genesis: &SumeragiFinalityVerifier,
        credit: [u8; 32],
        world: &[u8],
        payout: &[u8],
    ) -> Result<(), Error> {
        let payout = payout_original(payout)?;
        let world = WorldStateSnapshotV1::decode_bounded_canonical(world)
            .map_err(|_| Error::Invalid("payout World original"))?;
        let (selected, manifest) = self.sync_manifest()?;
        let (verifier, checkpoint) = self.selected_ledger(&manifest, genesis)?;
        let checkpoint = checkpoint.ok_or(Error::Invalid("no selected ledger tip"))?;
        let block = verifier
            .verify_retained_decision(checkpoint.tip())
            .map_err(|_| Error::Proof("payout selected decision"))?;
        let world = world
            .authenticate(&block)
            .map_err(|_| Error::Proof("payout World root"))?;
        // Reconcile the selected manifest immediately before the existing atomic publisher.
        if self.manifest()?.0 != selected {
            return Err(Error::WitnessLost("payout source changed"));
        }
        self.acknowledge_fee_payout(
            credit,
            &FinalizedPayoutEvidence {
                block: &block,
                world: &world,
                payout,
            },
        )
    }
}
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>
{
    /// Verify exactly one next native finality original and durably select its compact prefix.
    /// Retries of the same last decision reverify it; earlier decisions, gaps and foreign roots fail.
    /// # Errors
    /// Bounded canonical decoding, certificate/continuity, custody and publication failures.
    pub fn ingest_ledger_finality(&mut self, original: &[u8]) -> Result<LedgerProgressV1, Error> {
        let _payment = self.scheduler.payment();
        self.ingest_recursive_ledger(original)
    }
    /// Read only the authoritative selected native prefix, not an uncommitted in-memory tip.
    /// # Errors
    /// Lost/substituted checkpoint originals, invalid scope or unavailable custody.
    pub fn ledger_progress(&mut self) -> Result<Option<LedgerProgressV1>, Error> {
        let genesis = Arc::clone(&self.proofs.genesis);
        self.ledger_progress_selected(&genesis)
    }
    /// Authenticate a World snapshot and payout row against the selected native tip before
    /// acknowledging the exact fee payout and releasing its separately retained originals.
    /// # Errors
    /// Unauthenticated/foreign/stale evidence, changed source, lost claim or storage uncertainty.
    pub fn acknowledge_fee_payout_originals(
        &mut self,
        credit: [u8; 32],
        world: &[u8],
        payout: &[u8],
    ) -> Result<(), Error> {
        let _payment = self.scheduler.payment();
        let genesis = Arc::clone(&self.proofs.genesis);
        self.acknowledge_payout_originals(&genesis, credit, world, payout)
    }
}

#[cfg(test)]
mod tests;
