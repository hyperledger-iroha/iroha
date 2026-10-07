//! Complete recursive Load finality and current ledger instruction preparation.
use super::*;
mod load_session;
mod unload_session;
use crate::kagemusha_wallet_artifacts_v1::producer_inventory::FinalityProducerErrorV1;
use crate::kagemusha_wallet_finality_v1::{block_witness, load_witness, retain_load_finality};
use iroha_crypto::MerkleProof;
use iroha_data_model::isi::kagemusha_wallet::load_finality::KAGEMUSHA_WALLET_LOAD_RECEIPT_MAX_BYTES_V1;
use iroha_data_model::{
    events::EventBox,
    isi::kagemusha_wallet::{
        KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1, KagemushaWalletLoadReceiptV1,
    },
    sumeragi_finality::MAX_FINALITY_CHECKPOINT_BYTES,
};
use iroha_kagemusha_proof::finality::native::HistoryPrefix;
pub use load_session::LoadProofProgressV1;

/// Complete counted Load event path, including canonical Norito framing.
pub const LOAD_EVENT_PROOF_MAX_BYTES_V1: usize = 8192;
/// One complete current ledger instruction, including its original signed transport.
pub const LEDGER_INSTRUCTION_MAX_BYTES_V1: usize = 64 * 1024;
use crate::kagemusha_wallet_finality_v1::{HISTORY_ORIGINAL_MAX_BYTES_V1, HistoryOriginalV1};

pub(super) fn history_error(
    error: crate::kagemusha_wallet_finality_v1::HistoryOriginalErrorV1,
) -> Error {
    match error {
        crate::kagemusha_wallet_finality_v1::HistoryOriginalErrorV1::Encoding => {
            Error::WitnessLost("recursive prefix original")
        }
        crate::kagemusha_wallet_finality_v1::HistoryOriginalErrorV1::Proof(error)
            if error.is_cancelled() =>
        {
            Error::Cancelled
        }
        _ => Error::Proof("selected recursive prefix"),
    }
}
fn producer<T>(result: Result<T, FinalityProducerErrorV1>) -> Result<T, Error> {
    result.map_err(|error| match error {
        FinalityProducerErrorV1::Original(
            crate::kagemusha_wallet_proofs_v1::Error::Unavailable,
        ) => Error::ArtifactsUnavailable("recursive finality original"),
        FinalityProducerErrorV1::Original(crate::kagemusha_wallet_proofs_v1::Error::Cancelled) => {
            Error::Cancelled
        }
        FinalityProducerErrorV1::Source(error) if error.is_cancelled() => Error::Cancelled,
        _ => Error::Proof("complete recursive finality producer"),
    })
}
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::LedgerLoadPlanV1")]
struct LoadPlan {
    request: [u8; 32],
    amount: u128,
    ordinal: u128,
    instruction: Vec<u8>,
}

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::UnloadConfirmationV1")]
struct UnloadConfirmation {
    original_digest: [u8; 32],
    height: u64,
    block_hash: [u8; 32],
}

// Inclusion is checked while each certified original is selected, before its cursor can move.
// Absence is ordinary history; a located but foreign or unsuccessful input is an error.
fn contains_successful_unload(
    verified: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
    network: &iroha_data_model::NetworkId,
    scheme: [u8; 32],
    account: &iroha_data_model::account::AccountId,
    transaction: &[u8; 32],
    original: &[u8],
) -> Result<bool, Error> {
    use iroha_crypto::HashOf;
    use iroha_data_model::{query::CommittedTransaction, transaction::TransactionEntrypoint};
    let block = verified.block();
    let Some(index) = block
        .network_entrypoints()
        .position(|input| input.hash().as_ref() == transaction)
    else {
        return Ok(false);
    };
    let input_index = u32::try_from(index).map_err(|_| Error::Invalid("Unload input index"))?;
    let entrypoint = block
        .network_entrypoint_at(index)
        .ok_or(Error::Proof("Unload input"))?
        .clone();
    let TransactionEntrypoint::External(signed) = &entrypoint else {
        return Err(Error::Invalid("Unload external input"));
    };
    if signed.authority() != account
        || !signed
            .instructions()
            .explicit_instructions()
            .any(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<KagemushaWalletLedgerV1>()
                    .is_some_and(|instruction| {
                        instruction.scheme == scheme
                            && matches!(&instruction.action,
                    KagemushaWalletLedgerActionV1::Unload(bytes) if bytes == original)
                    })
            })
    {
        return Err(Error::Invalid("Unload exact original instruction"));
    }
    let (output_index, _) = block
        .network_output_at(input_index)
        .ok_or(Error::Proof("Unload execution output"))?;
    let output = block
        .execution_outputs()
        .get(output_index as usize)
        .ok_or(Error::Proof("Unload output index"))?
        .clone();
    let committed = CommittedTransaction {
        block_hash: block.hash(),
        entrypoint_hash: entrypoint.hash(),
        entrypoint_proof: block
            .network_input_proof(input_index)
            .ok_or(Error::Proof("Unload input membership"))?,
        entrypoint,
        output_hash: HashOf::new(&output),
        output_proof: block
            .output_proof(output_index)
            .ok_or(Error::Proof("Unload output membership"))?,
        output,
    };
    verified
        .verify_committed_transaction(network, &committed)
        .map_err(|_| Error::Proof("Unload successful authenticated execution"))?;
    Ok(true)
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    Coordinator<AdvanceHandle<F, P>, ProviderArchive<F, P>, NativeWalletProofsV1<F, P, S>>
{
    pub(super) fn prove_recursive_successor(
        &mut self,
        prefix: Option<HistoryPrefix>,
        block: &iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
    ) -> Result<HistoryPrefix, Error> {
        let sources = Arc::clone(&self.proofs.sources);
        let graph = sources.finality_producer();
        let next = {
            let mut originals = self
                .proofs
                .originals
                .lock()
                .map_err(|_| Error::ArtifactsUnavailable("recursive original owner"))?;
            match prefix {
                None => producer(graph.genesis(&mut *originals, self.proofs.budget))?,
                Some(prefix) => {
                    let input =
                        block_witness(graph.installed().anchor(), &self.proofs.chain, block)
                            .map_err(|_| Error::Proof("native recursive block witness"))?;
                    producer(graph.append(&mut *originals, &prefix, &input, self.proofs.budget))?
                }
            }
        };
        Ok(next)
    }
    pub(super) fn recursive_prefix(
        &mut self,
        manifest: &manifest::Manifest,
    ) -> Result<Option<HistoryPrefix>, Error> {
        let Some(address) = manifest.recursive_checkpoint else {
            if manifest.ledger_checkpoint.is_some() {
                return Err(Error::WitnessLost("recursive ledger prefix missing"));
            }
            return Ok(None);
        };
        if manifest.recursive_retired == Some(address) || manifest.ledger_checkpoint.is_none() {
            return Err(Error::WitnessLost("recursive ledger selection"));
        }
        let bytes = self
            .archive
            .read_object(&address, HISTORY_ORIGINAL_MAX_BYTES_V1)?;
        let original = HistoryOriginalV1::decode_canonical(&bytes)
            .map_err(|_| Error::WitnessLost("recursive prefix original"))?;
        original
            .restore_producer(
                self.proofs.sources.finality_producer().installed(),
                self.proofs.budget,
            )
            .map(Some)
            .map_err(history_error)
    }
    pub(super) fn ingest_recursive_ledger(
        &mut self,
        bytes: &[u8],
    ) -> Result<LedgerProgressV1, Error> {
        let genesis = Arc::clone(&self.proofs.genesis);
        self.ingest_ledger_transition(
            &genesis,
            bytes,
            |this, manifest| this.recursive_prefix(manifest),
            |prefix| prefix.state().next_height,
            |this, prefix, block| {
                let prefix = this.prove_recursive_successor(prefix, block)?;
                Ok((
                    prefix.state().next_height,
                    archive::encode(&HistoryOriginalV1::from_prefix(&prefix))?,
                ))
            },
        )
    }

    /// Produce actual compact finality for an original Load and counted event path at the selected tip.
    /// # Errors
    /// Foreign receipt, changed/native prefix, invalid event inclusion, missing originals or actual proof failure.
    pub fn prove_load_finality(
        &mut self,
        receipt_original: &[u8],
        event_original: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.prove_load_session(receipt_original, event_original)
    }

    /// Freeze the actual next native Load ordinal and instruction under a stable caller request.
    /// Retrying the same request returns its original instruction, even after credit changes the head.
    /// # Errors
    /// Missing/replaced custody, changed request amount, invalid lifecycle or durable publication failure.
    pub fn prepare_ledger_load(
        &mut self,
        request: [u8; 32],
        amount: u128,
    ) -> Result<Vec<u8>, Error> {
        let _payment = self.scheduler.payment();
        if request == [0; 32] || amount == 0 {
            return Err(Error::Invalid("ledger Load request"));
        }
        let (root, mut manifest) = self.sync_manifest()?;
        self.require_ledger_activation(&manifest)?;
        if let Some(address) = manifest
            .ledger_load_plans
            .get(&mut self.archive, &request)?
        {
            let address: [u8; 32] = address
                .try_into()
                .map_err(|_| Error::WitnessLost("ledger Load plan address"))?;
            let bytes = self
                .archive
                .read_object(&address, LEDGER_INSTRUCTION_MAX_BYTES_V1)?;
            let plan: LoadPlan = archive::decode(&bytes)?;
            if plan.request != request || plan.amount != amount {
                return Err(Error::Invalid("changed ledger Load request"));
            }
            return Ok(plan.instruction);
        }
        let released = self.indexed_step(&manifest, manifest.indexed.ok_or(Error::NoHead)?)?;
        let core = &released.frozen.capsule.successor_state.core;
        if core.lifecycle != KagemushaWalletLifecycleV1::Active {
            return Err(Error::Invalid("ledger Load lifecycle"));
        }
        let instruction = KagemushaWalletLedgerV1 {
            scheme: self.scheme_id,
            action: KagemushaWalletLedgerActionV1::IssueLoad {
                wallet: self.wallet_id,
                asset: self.proofs.asset.asset_digest(),
                ordinal: core.next_load,
                request_id: request,
                amount,
                charge: None,
            },
        };
        let plan = LoadPlan {
            request,
            amount,
            ordinal: core.next_load,
            instruction: archive::encode(&instruction)?,
        };
        let address = self
            .archive
            .write_object(&archive::encode(&plan)?, LEDGER_INSTRUCTION_MAX_BYTES_V1)?;
        manifest.ledger_load_plans =
            manifest
                .ledger_load_plans
                .set(&mut self.archive, request, &address)?;
        self.publish_manifest(root, &manifest)?;
        Ok(plan.instruction)
    }
    /// Wrap a complete canonical Native transport in the current closed ledger instruction.
    /// This conversion supplies no ledger acceptance or proof authority.
    /// # Errors
    /// Wrong transport kind, malformed original or another wallet incarnation.
    pub fn ledger_instruction(&self, kind: u64, original: &[u8]) -> Result<Vec<u8>, Error> {
        if original.is_empty() || original.len() > LEDGER_INSTRUCTION_MAX_BYTES_V1 {
            return Err(Error::Invalid("ledger transport bound"));
        }
        let action = match kind {
            1 => {
                let frame = valid(KagemushaWalletActivationV1::decode_canonical(
                    original,
                    &self.scheme_id,
                ))?;
                if frame.credential.body.wallet_id != self.wallet_id {
                    return Err(Error::Invalid("activation wallet"));
                }
                KagemushaWalletLedgerActionV1::Activate(original.to_vec())
            }
            2 => {
                let frame = valid(KagemushaWalletUnloadClaimV1::decode_canonical(
                    original,
                    &self.scheme_id,
                ))?;
                if frame.credential.body.wallet_id != self.wallet_id {
                    return Err(Error::Invalid("Unload wallet"));
                }
                KagemushaWalletLedgerActionV1::Unload(original.to_vec())
            }
            3 => {
                let frame = valid(KagemushaWalletCloseLoadsV1::decode_canonical(
                    original,
                    &self.scheme_id,
                ))?;
                if frame.credential.body.wallet_id != self.wallet_id {
                    return Err(Error::Invalid("CloseLoads wallet"));
                }
                KagemushaWalletLedgerActionV1::CloseLoads(original.to_vec())
            }
            _ => return Err(Error::Invalid("ledger transport kind")),
        };
        archive::encode(&KagemushaWalletLedgerV1 {
            scheme: self.scheme_id,
            action,
        })
    }
    /// Return the durably retained exact successful Unload input/output inclusion.
    /// The transaction hash is a locator. The complete retained block, signature, execution
    /// result and original Unload claim independently authenticate the returned confirmation.
    /// # Errors
    /// Absent/rejected transaction, another claim or wallet, invalid inclusion or changed source.
    pub fn confirm_ledger_unload(
        &mut self,
        transaction: [u8; 32],
        original: &[u8],
    ) -> Result<LedgerProgressV1, Error> {
        let claim = valid(KagemushaWalletUnloadClaimV1::decode_canonical(
            original,
            &self.scheme_id,
        ))?;
        if transaction == [0; 32]
            || claim.credential.body.wallet_id != self.wallet_id
            || claim.credential.body.account_digest != self.proofs.enrollment.body.account_digest
        {
            return Err(Error::Invalid("Unload confirmation scope"));
        }
        let original_digest = self.unload_original_digest(&transaction, original)?;
        let (selected, mut manifest) = self.sync_manifest()?;
        if let Some(bytes) = manifest
            .ledger_unload_confirmations
            .get(&mut self.archive, &transaction)?
        {
            let confirmed: UnloadConfirmation = archive::decode(&bytes)?;
            if confirmed.original_digest != original_digest
                || confirmed.height < 2
                || confirmed.block_hash == [0; 32]
            {
                return Err(Error::WitnessLost("retained Unload confirmation"));
            }
            return Ok(LedgerProgressV1 {
                height: confirmed.height,
                block_hash: confirmed.block_hash,
            });
        }
        let genesis = Arc::clone(&self.proofs.genesis);
        let (verifier, checkpoint) =
            self.selected_unload_cursor(&manifest, &transaction, &original_digest)?;
        let checkpoint = checkpoint.ok_or(Error::Invalid("Unload confirmation ledger absent"))?;
        let verified = verifier
            .verify_retained_decision(checkpoint.tip())
            .map_err(|_| Error::Proof("Unload selected native decision"))?;
        if !contains_successful_unload(
            &verified,
            &genesis.initial_epoch().network_id,
            self.scheme_id,
            &claim.account,
            &transaction,
            original,
        )? {
            return Err(Error::Invalid(
                "Unload transaction absent from selected block",
            ));
        }
        if self.manifest()?.0 != selected {
            return Err(Error::WitnessLost("Unload confirmation source changed"));
        }
        let progress = ledger::progress(&checkpoint);
        let confirmation = UnloadConfirmation {
            original_digest,
            height: progress.height,
            block_hash: progress.block_hash,
        };
        manifest.ledger_unload_confirmations = manifest.ledger_unload_confirmations.set(
            &mut self.archive,
            transaction,
            &archive::encode(&confirmation)?,
        )?;
        self.publish_manifest(selected, &manifest)?;
        Ok(progress)
    }
}
