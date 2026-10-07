//! Contiguous real proof production from bounded untrusted native originals.

use super::*;

#[path = "context_cache.rs"]
mod context_cache;
use crate::finality::{
    LoadReceiptCells,
    aggregate::{prepare_aggregation, propose_aggregate_key},
    bls::prepare_bls_batches,
    certificate::{CertificateCircuit, CertificateContext},
    certified_result::{CertifiedResultCircuit, CertifiedResultContext},
    history::{self, HistoryStepCircuit, HistoryStepInput},
    load_source::prepare_load_source,
    receipt_finality::{ReceiptFinalityCircuit, ReceiptFinalityInput},
    result::MAX_RESULT_BYTES,
    result_scan::prepare_result_batches,
    schedule::{
        complete::ScheduleCircuit,
        context_hash::prepare_context_batches,
        source::{ScheduleSourceInput, prepare_schedule_source},
    },
    scheduled_result::{ScheduledResultCircuit, ScheduledResultInput},
};
use context_cache::ProvedContext;

/// Original native block fields used only as untrusted circuit witness proposals.
/// The native adapter must preserve canonical original bytes and independently
/// perform normal finality/custody checks; the graph proves every authorization link.
#[derive(Clone, Debug)]
pub struct BlockWitnessInput {
    /// Exact original canonical R preimage, excluding its external hash domain.
    pub result_frame: Vec<u8>,
    /// Exact ordinary Commit vote preimage.
    pub message: [u8; 165],
    /// Original current committee keys in authenticated order.
    pub roster: Vec<[u8; 48]>,
    /// Original compact LSB-first signer bitmap.
    pub bitmap: Vec<u8>,
    /// Original native aggregate signature.
    pub signature: [u8; 96],
    /// Exact original current epoch-context ID.
    pub current_context: [u8; 32],
    /// Exact authorized successor epoch ID, equal to current without a boundary.
    pub authorized_context: [u8; 32],
}
impl BlockWitnessInput {
    fn check_bounds(&self) -> Result<(), Error> {
        if self.result_frame.is_empty()
            || self.result_frame.len()
                > usize::try_from(MAX_RESULT_BYTES).map_err(|_| Error::Input)?
            || !(4..=31).contains(&self.roster.len())
            || self.bitmap.len() > 4
        {
            return Err(Error::Input);
        }
        Ok(())
    }
}

/// Original receipt and counted event path, without a host acceptance verdict.
#[derive(Clone, Debug)]
pub struct LoadWitnessInput {
    /// Exact original canonical R preimage at the receipt's successful height.
    pub result_frame: Vec<u8>,
    /// Exact fixed receipt transcript, including the payer account digest.
    pub receipt: [u8; LoadReceiptCells::BYTES],
    /// Original counted event root.
    pub event_root: [u8; 32],
    /// Original nonzero number of events.
    pub event_count: u64,
    /// Exact index of this receipt's typed successful-Load event.
    pub event_index: u32,
    /// Original sibling hashes, with absent and exhausted positions set to zero.
    pub siblings: [[u8; 32]; 32],
}

/// One complete original history proof and its exact authenticated terminal state.
/// Fields are private; restoration always re-verifies the installed closed prefix.
#[derive(Clone, Debug)]
pub struct HistoryPrefix {
    state: HistoryState,
    evidence: SourceNodeEvidence,
}
impl HistoryPrefix {
    /// State opened by the exact verified complete prefix.
    pub const fn state(&self) -> &HistoryState {
        &self.state
    }
    /// Original complete prefix proof and both retained curve claims.
    pub const fn evidence(&self) -> &SourceNodeEvidence {
        &self.evidence
    }
}

impl InstalledFinality {
    /// Produce a genuine fixed genesis proof under the finite shared wrapper.
    /// # Errors
    /// Changed original artifacts, invalid proof or resource refusal.
    pub fn genesis(&self, context: &mut ProvingContext<'_, '_>) -> Result<HistoryPrefix, Error> {
        let state = HistoryState::genesis(&self.anchor);
        if let Some(evidence) = context.restore_source(
            &self.history.source,
            self.history_endpoints(&state)?,
            &self.params.vesta,
        )? {
            return self.restore_history_cancellable(
                &state,
                evidence,
                context.proof.msm_budget,
                context.proof.cancellation,
            );
        }
        let entropy = context.entropy(NodeId::Genesis)?;
        let producer = self.history.reload(
            self.anchor,
            context.artifacts,
            &self.params,
            self.limits,
            context.proof.cancellation,
        )?;
        let evidence = producer.genesis(entropy, context.proof)?;
        context.retain_source(&self.history.source, &evidence, &self.params.vesta)?;
        self.restore_history_cancellable(
            &state,
            evidence,
            context.proof.msm_budget,
            context.proof.cancellation,
        )
    }

    /// Restore only a fully verified complete prefix under this exact installation.
    /// Serialized checkpoints or native state openings alone carry no authority.
    /// # Errors
    /// Noncanonical state, changed anchor/key/boundaries or failed proof/claim decision.
    pub fn restore_history(
        &self,
        state: &HistoryState,
        evidence: SourceNodeEvidence,
        budget: MemoryBudget,
    ) -> Result<HistoryPrefix, Error> {
        self.restore_history_cancellable(state, evidence, budget, None)
    }
    fn restore_history_cancellable(
        &self,
        state: &HistoryState,
        evidence: SourceNodeEvidence,
        budget: MemoryBudget,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<HistoryPrefix, Error> {
        self.verify_history(state, &evidence, budget, cancellation)?;
        Ok(HistoryPrefix {
            state: *state,
            evidence,
        })
    }
    fn verify_history(
        &self,
        state: &HistoryState,
        evidence: &SourceNodeEvidence,
        budget: MemoryBudget,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<(), Error> {
        iroha_pasta::CancellationToken::checkpoint(cancellation)?;
        if !state.is_canonical()
            || state.next_height < 2
            || evidence.endpoints != self.history_endpoints(state)?
        {
            return Err(Error::Input);
        }
        let _opening = self
            .history
            .source
            .verify_native_cancellable(evidence, &self.params.vesta, budget, cancellation)
            .map_err(|error| {
                if matches!(error, iroha_plonk::frontend::Error::Cancelled) {
                    Error::Cancelled
                } else {
                    Error::Proof
                }
            })?;
        Ok(())
    }
    fn history_endpoints(&self, state: &HistoryState) -> Result<[Fp; 6], Error> {
        let anchor = self.anchor.digest();
        let key = self
            .history
            .source
            .key_digest()
            .map_err(|_| Error::Artifact)?;
        Ok([
            Fp::from(history::PREFIX_PROGRAM_ID),
            history::prefix_context(anchor, key),
            Fp::from(0),
            Fp::from(1),
            Fp::from(0),
            state.digest(anchor),
        ])
    }

    /// Prove exactly the next ordinary block, preserving the prior prefix on failure.
    /// No requested/latest-height jump substitutes for the intervening block proofs.
    /// # Errors
    /// Skipped/repeated height, malformed native proposals, changed installed
    /// originals, failed source constraints/proofs/folds or finite resource refusal.
    pub fn append_block(
        &self,
        previous: &HistoryPrefix,
        input: &BlockWitnessInput,
        context: &mut ProvingContext<'_, '_>,
    ) -> Result<HistoryPrefix, Error> {
        input.check_bounds()?;
        self.verify_history(
            &previous.state,
            &previous.evidence,
            context.proof.msm_budget,
            context.proof.cancellation,
        )?;
        let height = u64::from_be_bytes(core::array::from_fn(|i| input.message[85 + i]));
        if height != previous.state.next_height {
            return Err(Error::Input);
        }
        height.checked_add(1).ok_or(Error::Input)?;
        let (certified_input, certified) = self.certify_result(input, context)?;
        let (current_input, current, current_hash) = self.prove_schedule(
            &input.result_frame,
            false,
            input.current_context,
            None,
            context,
        )?;
        let scheduled_input = ScheduledResultInput {
            certified: certified_input,
            schedule: current_input,
        };
        let scheduled = self.scheduled.node.prepare_and_prove(
            &self.params,
            self.limits,
            context,
            |salt, fold| {
                ScheduledResultCircuit::prepare(
                    self.scheduled.plan.clone(),
                    &scheduled_input,
                    [certified, current],
                    &self.params.vesta,
                    salt,
                    fold,
                )
            },
        )?;
        let (authorized, authorized_proof, _authorized_hash) = self.prove_schedule(
            &input.result_frame,
            true,
            input.authorized_context,
            Some(&current_hash),
            context,
        )?;
        let mut step_input = HistoryStepInput {
            before: previous.state,
            after: previous.state,
            scheduled: scheduled_input.statement(),
            authorized,
        };
        step_input.after = circuit(step_input.expected_after())?;
        let step = self.step.node.prepare_and_prove(
            &self.params,
            self.limits,
            context,
            |salt, fold| {
                HistoryStepCircuit::prepare(
                    self.anchor,
                    self.step.plan.clone(),
                    &step_input,
                    [scheduled, authorized_proof],
                    &self.params.vesta,
                    salt,
                    fold,
                )
            },
        )?;
        if let Some(evidence) = context.restore_source(
            &self.history.source,
            self.history_endpoints(&step_input.after)?,
            &self.params.vesta,
        )? {
            return self.restore_history_cancellable(
                &step_input.after,
                evidence,
                context.proof.msm_budget,
                context.proof.cancellation,
            );
        }
        let entropy = context.entropy(NodeId::Append)?;
        let producer = self.history.reload(
            self.anchor,
            context.artifacts,
            &self.params,
            self.limits,
            context.proof.cancellation,
        )?;
        let evidence = producer.append(
            previous.evidence.clone(),
            step,
            entropy,
            context.proof,
            context.fold,
        )?;
        context.retain_source(&self.history.source, &evidence, &self.params.vesta)?;
        self.restore_history_cancellable(
            &step_input.after,
            evidence,
            context.proof.msm_budget,
            context.proof.cancellation,
        )
    }

    fn certify_result(
        &self,
        input: &BlockWitnessInput,
        context: &mut ProvingContext<'_, '_>,
    ) -> Result<(CertifiedResultContext, SourceNodeEvidence), Error> {
        let aggregate_key = circuit(propose_aggregate_key(&input.roster, &input.bitmap))?;
        let aggregation = circuit(prepare_aggregation(
            &input.roster,
            &input.bitmap,
            aggregate_key,
        ))?;
        let aggregation_context = *aggregation.first().ok_or(Error::Input)?.context();
        let signature = prepare_bls_batches(input.message, aggregate_key, input.signature)
            .map_err(|_| Error::Input)?;
        let expected = core::array::from_fn(|i| input.message[133 + i]);
        let scan = circuit(prepare_result_batches(&input.result_frame, expected))?;
        if circuit(scan.last().ok_or(Error::Input)?.proposed_digest())? != expected {
            return Err(Error::Input);
        }
        let scan_context = *scan.first().ok_or(Error::Input)?.context();
        let aggregate =
            self.programs
                .aggregation
                .prove(aggregation, &self.params, self.limits, context)?;
        let signature = self
            .programs
            .bls
            .prove(signature, &self.params, self.limits, context)?;
        let certificate_context = CertificateContext {
            aggregation: aggregation_context,
            message: input.message,
            signature: input.signature,
        };
        let certificate = self.certificate.node.prepare_and_prove(
            &self.params,
            self.limits,
            context,
            |salt, fold| {
                CertificateCircuit::prepare(
                    self.certificate.plan.clone(),
                    &certificate_context,
                    [aggregate, signature],
                    &self.params.vesta,
                    salt,
                    fold,
                )
            },
        )?;
        let scan = self
            .programs
            .result
            .prove(scan, &self.params, self.limits, context)?;
        let certified_input = CertifiedResultContext {
            certificate: certificate_context.statement(),
            root: scan_context.root,
            frame_len: scan_context.frame_len,
        };
        let certified = self.certified.node.prepare_and_prove(
            &self.params,
            self.limits,
            context,
            |salt, fold| {
                CertifiedResultCircuit::prepare(
                    self.certified.plan.clone(),
                    certified_input,
                    [certificate, scan],
                    &self.params.vesta,
                    salt,
                    fold,
                )
            },
        )?;
        Ok((certified_input, certified))
    }

    fn prove_schedule(
        &self,
        frame: &[u8],
        authorized: bool,
        id: [u8; 32],
        reuse: Option<&ProvedContext>,
        context: &mut ProvingContext<'_, '_>,
    ) -> Result<(ScheduleSourceInput, SourceNodeEvidence, ProvedContext), Error> {
        let parser = circuit(prepare_schedule_source(frame.to_vec(), authorized, id))?;
        let input = *parser.first().ok_or(Error::Input)?.input();
        let parser = self
            .programs
            .schedule
            .prove(parser, &self.params, self.limits, context)?;
        let hash = circuit(prepare_context_batches(
            frame.to_vec(),
            input.epoch_hash.payload_start,
            input.epoch_hash.payload_len,
            id,
        ))?;
        let hash_input = *hash.first().ok_or(Error::Input)?.input();
        let cached = reuse
            .map(|cached| {
                cached.reuse(
                    &hash_input,
                    &self.programs.context.source(),
                    &self.params.vesta,
                    context.proof.msm_budget,
                    context.proof.cancellation,
                )
            })
            .transpose()?
            .flatten();
        let hash = match cached {
            Some(evidence) => evidence,
            None => self
                .programs
                .context
                .prove(hash, &self.params, self.limits, context)?,
        };
        let retained_hash = ProvedContext::new(hash_input, hash.clone())?;
        let proof = self.schedule.node.prepare_and_prove(
            &self.params,
            self.limits,
            context,
            |salt, fold| {
                ScheduleCircuit::prepare(
                    self.schedule.plan.clone(),
                    &input,
                    [parser, hash],
                    &self.params.vesta,
                    salt,
                    fold,
                )
            },
        )?;
        Ok((input, proof, retained_hash))
    }

    /// Prove one exact receipt at this prefix's terminal successful block.
    /// The original complete R is required again and copy-bound by its tape root;
    /// no historical native checkpoint, row, DTO or inclusion path replaces history.
    /// # Errors
    /// Wrong receipt height/result/event, partial sources, changed originals,
    /// failed constraints/proofs/claims or bounded resource refusal.
    pub fn prove_receipt(
        &self,
        prefix: &HistoryPrefix,
        input: &LoadWitnessInput,
        context: &mut ProvingContext<'_, '_>,
    ) -> Result<SourceNodeEvidence, Error> {
        if input.result_frame.is_empty()
            || input.result_frame.len()
                > usize::try_from(MAX_RESULT_BYTES).map_err(|_| Error::Input)?
            || input.event_count == 0
            || input.event_count > 1_u64 << 32
            || u64::from(input.event_index) >= input.event_count
        {
            return Err(Error::Input);
        }
        self.verify_history(
            &prefix.state,
            &prefix.evidence,
            context.proof.msm_budget,
            context.proof.cancellation,
        )?;
        let height = u64::from_le_bytes(core::array::from_fn(|i| input.receipt[242 + i]));
        if height < 2 || height.checked_add(1) != Some(prefix.state.next_height) {
            return Err(Error::Input);
        }
        let load = circuit(prepare_load_source(
            &input.result_frame,
            &input.receipt,
            input.event_root,
            input.event_count,
            input.event_index,
            &input.siblings,
        ))?;
        let terminal_input = ReceiptFinalityInput {
            terminal: prefix.state,
            load: *load.first().ok_or(Error::Input)?.context(),
            receipt: input.receipt,
        };
        let load = self
            .programs
            .load
            .prove(load, &self.params, self.limits, context)?;
        let evidence = self.receipt.node.prepare_and_prove(
            &self.params,
            self.limits,
            context,
            |salt, fold| {
                ReceiptFinalityCircuit::prepare(
                    self.anchor,
                    self.receipt.plan.clone(),
                    &terminal_input,
                    [prefix.evidence.clone(), load],
                    &self.params.vesta,
                    salt,
                    fold,
                )
            },
        )?;
        self.verify_receipt_evidence(
            terminal_input.receipt_digest(),
            &evidence,
            context.proof.msm_budget,
        )?;
        Ok(evidence)
    }
}
