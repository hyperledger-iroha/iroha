//! Receipt-bound recursive cursor, independent of later ordinary ledger activity.
use super::*;
use iroha_data_model::sumeragi_finality::SumeragiFinalityCheckpoint;
const SESSION_MAX: usize =
    MAX_FINALITY_CHECKPOINT_BYTES + HISTORY_ORIGINAL_MAX_BYTES_V1 + 32 * 1024;

/// Receipt DATA target and actual durably verified height of its own recursive cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadProofProgressV1 {
    /// Original receipt's proposed height. Finality is established only by proof completion.
    pub receipt_height: u64,
    /// Selected verified height; zero means this receipt's cursor has not started.
    pub verified_height: u64,
}
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::LoadProofSessionV1")]
struct Session {
    receipt: KagemushaWalletLoadReceiptV1,
    // Enclosed in this request-bound object, never aliased with a global checkpoint's address.
    checkpoint: Option<Vec<u8>>,
    prefix: Option<HistoryOriginalV1>,
    finality: Option<Vec<u8>>,
}
impl Session {
    fn empty(receipt: KagemushaWalletLoadReceiptV1) -> Self {
        Self {
            receipt,
            checkpoint: None,
            prefix: None,
            finality: None,
        }
    }
    fn progress(&self) -> Result<LoadProofProgressV1, Error> {
        let height = if self.finality.is_some() {
            self.receipt.block_height
        } else {
            match (&self.checkpoint, &self.prefix) {
                (None, None) => 0,
                (Some(bytes), Some(prefix)) => {
                    let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(bytes)
                        .map_err(|_| Error::WitnessLost("Load cursor checkpoint"))?;
                    if checkpoint.height().checked_add(1) != Some(prefix.next_height()) {
                        return Err(Error::WitnessLost("Load cursor height binding"));
                    }
                    checkpoint.height()
                }
                _ => return Err(Error::WitnessLost("partial Load cursor")),
            }
        };
        if height > self.receipt.block_height {
            return Err(Error::WitnessLost("Load cursor passed receipt"));
        }
        Ok(LoadProofProgressV1 {
            receipt_height: self.receipt.block_height,
            verified_height: height,
        })
    }
}
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>
{
    fn selected_load_receipt(
        &mut self,
        bytes: &[u8],
        manifest: &manifest::Manifest,
    ) -> Result<KagemushaWalletLoadReceiptV1, Error> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_WALLET_LOAD_RECEIPT_MAX_BYTES_V1 {
            return Err(Error::Invalid("Load receipt bound"));
        }
        let receipt: KagemushaWalletLoadReceiptV1 = archive::decode(bytes)?;
        valid(receipt.validate())?;
        if receipt.scheme_id != self.scheme_id
            || receipt.wallet_id != self.wallet_id
            || receipt.asset_digest != self.proofs.asset.asset_digest()
            || receipt.payer_account_digest != self.proofs.enrollment.body.account_digest
        {
            return Err(Error::Invalid("Load receipt wallet scope"));
        }
        let address = manifest
            .ledger_load_plans
            .get(&mut self.archive, &receipt.request_id)?
            .ok_or(Error::Invalid("Load has no prepared ledger request"))?;
        let address = address
            .try_into()
            .map_err(|_| Error::WitnessLost("Load request index"))?;
        let plan: LoadPlan = archive::decode(
            &self
                .archive
                .read_object(&address, LEDGER_INSTRUCTION_MAX_BYTES_V1)?,
        )?;
        if plan.request != receipt.request_id
            || plan.amount != receipt.amount
            || plan.ordinal != receipt.ordinal
        {
            return Err(Error::Invalid("Load receipt selected terms"));
        }
        Ok(receipt)
    }
    fn load_session(
        &mut self,
        manifest: &manifest::Manifest,
        receipt: KagemushaWalletLoadReceiptV1,
    ) -> Result<Session, Error> {
        let Some(address) = manifest
            .ledger_load_proofs
            .get(&mut self.archive, &receipt.request_id)?
        else {
            return Ok(Session::empty(receipt));
        };
        let address = address
            .try_into()
            .map_err(|_| Error::WitnessLost("Load cursor index"))?;
        let session: Session = archive::decode(&self.archive.read_object(&address, SESSION_MAX)?)?;
        if session.receipt != receipt {
            // An unproved HTTP proposal may be replaced. A complete authenticated result
            // is immutable; a conflicting new server proposal cannot erase it.
            if session.finality.is_some() {
                return Err(Error::Invalid("Load receipt differs from completed proof"));
            }
            return Ok(Session::empty(receipt));
        }
        session.progress()?;
        Ok(session)
    }
    fn clean_load_session(&mut self) -> Result<(), Error> {
        let (root, mut manifest) = self.sync_manifest()?;
        if let Some((request, address)) = manifest.ledger_load_retired {
            if manifest
                .ledger_load_proofs
                .get(&mut self.archive, &request)?
                .as_deref()
                == Some(address.as_slice())
            {
                return Err(Error::WitnessLost("Load cleanup selects current session"));
            }
            self.archive.remove(ArchiveKey::Object(address))?;
            manifest.ledger_load_retired = None;
            self.publish_manifest(root, &manifest)?;
        }
        Ok(())
    }
    fn publish_load_session(
        &mut self,
        root: [u8; 32],
        mut manifest: manifest::Manifest,
        session: &Session,
    ) -> Result<(), Error> {
        let request = session.receipt.request_id;
        if manifest.ledger_load_retired.is_some() {
            return Err(Error::WitnessLost("Load cleanup pending"));
        }
        let previous = manifest
            .ledger_load_proofs
            .get(&mut self.archive, &request)?
            .map(|bytes| {
                bytes
                    .try_into()
                    .map_err(|_| Error::WitnessLost("Load prior index"))
            })
            .transpose()?;
        let address = self
            .archive
            .write_object(&archive::encode(session)?, SESSION_MAX)?;
        manifest.ledger_load_proofs =
            manifest
                .ledger_load_proofs
                .set(&mut self.archive, request, &address)?;
        manifest.ledger_load_retired = previous
            .filter(|previous| previous != &address)
            .map(|previous| (request, previous));
        self.publish_manifest(root, &manifest)?;
        self.clean_load_session()
    }
    fn load_cursor(
        &self,
        session: &Session,
    ) -> Result<(SumeragiFinalityVerifier, Option<HistoryPrefix>), Error> {
        let Some(bytes) = &session.checkpoint else {
            if session.prefix.is_some() {
                return Err(Error::WitnessLost("Load partial prefix"));
            }
            return Ok((self.proofs.genesis.as_ref().clone(), None));
        };
        let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(bytes)
            .map_err(|_| Error::WitnessLost("Load checkpoint original"))?;
        let verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &checkpoint,
            &self.proofs.genesis.initial_epoch().network_id,
            &self.proofs.chain,
        )
        .map_err(|_| Error::WitnessLost("Load checkpoint root"))?;
        if verifier.initial_epoch() != self.proofs.genesis.initial_epoch()
            || verifier.instance() != self.proofs.genesis.instance()
        {
            return Err(Error::WitnessLost("Load checkpoint installation"));
        }
        let prefix = session
            .prefix
            .clone()
            .ok_or(Error::WitnessLost("Load prefix missing"))?
            .restore_producer(
                self.proofs.sources.finality_producer().installed(),
                self.proofs.budget,
            )
            .map_err(history_error)?;
        if checkpoint.height().checked_add(1) != Some(prefix.state().next_height) {
            return Err(Error::WitnessLost("Load original prefix height"));
        }
        Ok((verifier, Some(prefix)))
    }
    /// Read the receipt-bound proof cursor. This DATA projection never proves a receipt.
    /// # Errors
    /// Malformed/foreign receipt, changed prepared terms or lost selected state.
    pub fn load_finality_progress(&mut self, receipt: &[u8]) -> Result<LoadProofProgressV1, Error> {
        let (_, manifest) = self.sync_manifest()?;
        let receipt = self.selected_load_receipt(receipt, &manifest)?;
        self.load_session(&manifest, receipt)?.progress()
    }
    /// Verify and prove one exact next block in this receipt's independent native-rooted cursor.
    /// # Errors
    /// Changed receipt/terms, gaps, another genesis, proof failure or uncertain durable publication.
    pub fn ingest_load_finality(
        &mut self,
        receipt: &[u8],
        original: &[u8],
    ) -> Result<LoadProofProgressV1, Error> {
        let _payment = self.scheduler.payment();
        self.clean_load_session()?;
        let candidate = ledger::proof_original(original)?;
        let (root, manifest) = self.sync_manifest()?;
        let receipt = self.selected_load_receipt(receipt, &manifest)?;
        let mut session = self.load_session(&manifest, receipt)?;
        if session.finality.is_some() {
            return session.progress();
        }
        let progress = session.progress()?;
        let (mut verifier, prefix) = self.load_cursor(&session)?;
        if candidate.height() == progress.verified_height && progress.verified_height > 0 {
            verifier
                .verify_retained_decision(&candidate)
                .map_err(|_| Error::Proof("Load cursor retry"))?;
            return Ok(progress);
        }
        if candidate.height()
            != progress
                .verified_height
                .checked_add(1)
                .ok_or(Error::Invalid("Load cursor overflow"))?
            || candidate.height() > receipt.block_height
        {
            return Err(Error::Invalid("Load cursor exact next height"));
        }
        let block = if prefix.is_none() {
            verifier
                .verify_retained_decision(&candidate)
                .or_else(|_| verifier.verify(&candidate))
                .map_err(|_| Error::Proof("Load cursor genesis"))?
        } else {
            verifier
                .verify(&candidate)
                .map_err(|_| Error::Proof("Load cursor continuity"))?
        };
        let next = self.prove_recursive_successor(prefix, &block)?;
        session.checkpoint = Some(
            verifier
                .export_checkpoint(&candidate)
                .and_then(|checkpoint| checkpoint.encode_canonical())
                .map_err(|_| Error::Proof("Load verified checkpoint"))?,
        );
        session.prefix = Some(HistoryOriginalV1::from_prefix(&next));
        let progress = session.progress()?;
        self.publish_load_session(root, manifest, &session)?;
        Ok(progress)
    }
    pub(super) fn prove_load_session(
        &mut self,
        receipt: &[u8],
        event: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let _payment = self.scheduler.payment();
        self.clean_load_session()?;
        if event.is_empty() || event.len() > LOAD_EVENT_PROOF_MAX_BYTES_V1 {
            return Err(Error::Invalid("Load event path bound"));
        }
        let event: MerkleProof<EventBox> = archive::decode(event)?;
        let (root, manifest) = self.sync_manifest()?;
        let receipt = self.selected_load_receipt(receipt, &manifest)?;
        let mut session = self.load_session(&manifest, receipt)?;
        if let Some(bytes) = session.finality {
            let finality = valid(KagemushaWalletLoadFinalityV1::decode_canonical(&bytes))?;
            if finality.receipt_digest != valid(receipt.receipt_digest())? {
                return Err(Error::WitnessLost("Load final proof binding"));
            }
            return Ok(bytes);
        }
        let progress = session.progress()?;
        if progress.verified_height != receipt.block_height {
            return Err(Error::Invalid("Load cursor incomplete"));
        }
        let (verifier, prefix) = self.load_cursor(&session)?;
        let prefix = prefix.ok_or(Error::WitnessLost("Load complete prefix"))?;
        let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(
            session
                .checkpoint
                .as_ref()
                .ok_or(Error::WitnessLost("Load selected checkpoint"))?,
        )
        .map_err(|_| Error::WitnessLost("Load checkpoint original"))?;
        let block = verifier
            .verify_retained_decision(checkpoint.tip())
            .map_err(|_| Error::Proof("Load selected decision"))?;
        let sources = Arc::clone(&self.proofs.sources);
        let graph = sources.finality_producer();
        let input = load_witness(
            graph.installed().anchor(),
            &self.proofs.chain,
            &block,
            &receipt,
            &event,
        )
        .map_err(|_| Error::Proof("Load original event inclusion"))?;
        let evidence = {
            let mut originals = self
                .proofs
                .originals
                .lock()
                .map_err(|_| Error::ArtifactsUnavailable("Load proving originals"))?;
            producer(graph.receipt(&mut *originals, &prefix, &input, self.proofs.budget))?
        };
        let finality =
            retain_load_finality(graph.installed(), &receipt, evidence, self.proofs.budget)
                .map_err(|_| Error::Proof("Load complete proof retention"))?;
        let bytes = valid(finality.to_canonical_bytes())?;
        session.finality = Some(bytes.clone());
        session.checkpoint = None;
        session.prefix = None;
        self.publish_load_session(root, manifest, &session)?;
        Ok(bytes)
    }
}
