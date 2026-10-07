//! Transaction-bound ordinary finality custody, independent of the wallet's global tip.
use super::*;
use iroha_data_model::sumeragi_finality::SumeragiFinalityCheckpoint;
const SESSION_MAX: usize = MAX_FINALITY_CHECKPOINT_BYTES + 1024;

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::UnloadProofSessionV1")]
struct Session {
    transaction: [u8; 32],
    original_digest: [u8; 32],
    checkpoint: Vec<u8>,
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>
{
    pub(super) fn unload_original_digest(
        &self,
        transaction: &[u8; 32],
        original: &[u8],
    ) -> Result<[u8; 32], Error> {
        if *transaction == [0; 32]
            || original.is_empty()
            || original.len() > LEDGER_INSTRUCTION_MAX_BYTES_V1
        {
            return Err(Error::Invalid("Unload cursor input bound"));
        }
        let claim = valid(KagemushaWalletUnloadClaimV1::decode_canonical(
            original,
            &self.scheme_id,
        ))?;
        if claim.credential.body.wallet_id != self.wallet_id
            || claim.credential.body.account_digest != self.proofs.enrollment.body.account_digest
        {
            return Err(Error::Invalid("Unload cursor wallet scope"));
        }
        Ok(*iroha_crypto::Hash::new(original).as_ref())
    }
    pub(super) fn selected_unload_cursor(
        &mut self,
        manifest: &manifest::Manifest,
        transaction: &[u8; 32],
        original_digest: &[u8; 32],
    ) -> Result<(SumeragiFinalityVerifier, Option<SumeragiFinalityCheckpoint>), Error> {
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
            return Err(Error::Proof("Unload cursor root scope"));
        }
        let Some(address) = manifest
            .ledger_unload_proofs
            .get(&mut self.archive, transaction)?
        else {
            return Ok(((**genesis).clone(), None));
        };
        let address = address
            .try_into()
            .map_err(|_| Error::WitnessLost("Unload cursor index"))?;
        if manifest.ledger_unload_retired == Some((*transaction, address)) {
            return Err(Error::WitnessLost("Unload cursor selected retired object"));
        }
        let session: Session = archive::decode(&self.archive.read_object(&address, SESSION_MAX)?)?;
        if session.transaction != *transaction || session.original_digest != *original_digest {
            return Err(Error::OperationConflict);
        }
        let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(&session.checkpoint)
            .map_err(|_| Error::WitnessLost("Unload cursor checkpoint"))?;
        if checkpoint.network_id().as_bytes() != &scheme.network_id
            || checkpoint.chain_id() != chain
        {
            return Err(Error::WitnessLost("Unload cursor checkpoint scope"));
        }
        let verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &checkpoint,
            &genesis.initial_epoch().network_id,
            &chain,
        )
        .map_err(|_| Error::WitnessLost("Unload cursor checkpoint verification"))?;
        if verifier.initial_epoch() != genesis.initial_epoch()
            || verifier.instance() != genesis.instance()
        {
            return Err(Error::WitnessLost("Unload cursor genesis binding"));
        }
        Ok((verifier, Some(checkpoint)))
    }
    fn clean_unload_cursor(&mut self) -> Result<(), Error> {
        let (selected, mut manifest) = self.sync_manifest()?;
        if let Some((transaction, address)) = manifest.ledger_unload_retired {
            if manifest
                .ledger_unload_proofs
                .get(&mut self.archive, &transaction)?
                .as_deref()
                == Some(address.as_slice())
            {
                return Err(Error::WitnessLost("Unload cleanup selects current cursor"));
            }
            self.archive.remove(ArchiveKey::Object(address))?;
            manifest.ledger_unload_retired = None;
            self.publish_manifest(selected, &manifest)?;
        }
        Ok(())
    }
    /// Read this exact transaction and claim's independently retained ordinary finality cursor.
    /// # Errors
    /// Foreign/changed claim, missing checkpoint custody or invalid selected genesis.
    pub fn unload_finality_progress(
        &mut self,
        transaction: [u8; 32],
        original: &[u8],
    ) -> Result<Option<LedgerProgressV1>, Error> {
        let digest = self.unload_original_digest(&transaction, original)?;
        let (_, manifest) = self.sync_manifest()?;
        let (_, checkpoint) = self.selected_unload_cursor(&manifest, &transaction, &digest)?;
        Ok(checkpoint.as_ref().map(ledger::progress))
    }
    /// Verify and persist one exact next ordinary block for this Unload's own cursor.
    /// No caller height, HTTP status or proposed transaction supplies inclusion authority.
    /// # Errors
    /// Invalid prefix, changed original, unavailable custody or failed durable publication.
    pub fn ingest_unload_finality(
        &mut self,
        transaction: [u8; 32],
        original: &[u8],
        finality: &[u8],
    ) -> Result<LedgerProgressV1, Error> {
        let _payment = self.scheduler.payment();
        let digest = self.unload_original_digest(&transaction, original)?;
        let candidate = ledger::proof_original(finality)?;
        self.clean_unload_cursor()?;
        let (selected, mut manifest) = self.sync_manifest()?;
        let (mut verifier, previous) =
            self.selected_unload_cursor(&manifest, &transaction, &digest)?;
        if let Some(previous) = &previous {
            if candidate.height() == previous.height() {
                verifier
                    .verify_retained_decision(&candidate)
                    .map_err(|_| Error::Proof("Unload cursor retry decision"))?;
                return Ok(ledger::progress(previous));
            }
            if previous.height().checked_add(1) != Some(candidate.height()) {
                return Err(Error::Invalid("Unload cursor next height"));
            }
        } else if candidate.height() != 1 {
            return Err(Error::Invalid("Unload cursor requires genesis"));
        }
        if previous.is_some() || verifier.verify_retained_decision(&candidate).is_err() {
            verifier
                .verify(&candidate)
                .map_err(|_| Error::Proof("Unload cursor continuity"))?;
        }
        let checkpoint = verifier
            .export_checkpoint(&candidate)
            .map_err(|_| Error::Proof("Unload cursor checkpoint"))?;
        let session = Session {
            transaction,
            original_digest: digest,
            checkpoint: checkpoint
                .encode_canonical()
                .map_err(|_| Error::Proof("Unload checkpoint encoding"))?,
        };
        let bytes = archive::encode(&session)?;
        let address = self.archive.write_object(&bytes, SESSION_MAX)?;
        let old = manifest
            .ledger_unload_proofs
            .get(&mut self.archive, &transaction)?
            .map(|bytes| {
                bytes
                    .try_into()
                    .map_err(|_| Error::WitnessLost("Unload previous cursor"))
            })
            .transpose()?;
        manifest.ledger_unload_proofs =
            manifest
                .ledger_unload_proofs
                .set(&mut self.archive, transaction, &address)?;
        manifest.ledger_unload_retired = old
            .filter(|old| *old != address)
            .map(|old| (transaction, old));
        self.publish_manifest(selected, &manifest)?;
        self.clean_unload_cursor()?;
        Ok(ledger::progress(&checkpoint))
    }
}
