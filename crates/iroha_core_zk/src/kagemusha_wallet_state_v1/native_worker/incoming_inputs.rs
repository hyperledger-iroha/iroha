//! Source-selected Receive/Archive proposals and exact adjusted-map effects.
//!
//! All result bits are derived from original circuit predicates or the total
//! native verifier. The final Q/A owners rederive them; no caller supplies verdicts.

use super::*;
use crate::kagemusha_wallet_preparation_v1::{
    ArchiveFoldFieldsV1, ArchiveFoldWitnessV1, ArchiveIncomingWitnessV1, FoldedStateV1,
    PreparationV1, PreparedOperationV1, ReceiveFoldFieldsV1,
};
use crate::kagemusha_wallet_state_v1::{FoldCustodyV1, ReleasedStep};
use iroha_data_model::kagemusha::kagemusha_wallet_v1::*;
use iroha_kagemusha_proof::{
    operation_relation::objects::ObjectKind,
    q_sigma::{
        SigmaSlotWitness,
        native::{IncomingMode, IncomingSigma},
    },
};
use iroha_pasta::{Ep, Eq, poseidon::hash_with_domain};
use iroha_plonk::Protocol;
use iroha_plonk_gadgets::statement::STATEMENT_DOMAIN;
use iroha_plonk_recursion::{AccumulatorT, obligation::ledger::Variant};

fn predecessor_claims(
    predecessor: &FoldedStateV1,
) -> Result<(AccumulatorT<Ep>, AccumulatorT<Eq>), Error> {
    let (_, p, v) = predecessor.transport();
    Ok((
        proof(AccumulatorT::from_bytes(p))?,
        proof(AccumulatorT::from_bytes(v))?,
    ))
}
// Q exports the original length independently of its fixed verifier view.
// Complete original bytes remain in the A object tapes and their context digest.
fn sigma_view(original: &[u8], expected: usize) -> Result<(Vec<u8>, u32), Error> {
    if expected == 0 || !expected.is_multiple_of(32) {
        return Err(Error::Proof("incoming sigma descriptor"));
    }
    let length =
        u32::try_from(original.len()).map_err(|_| Error::Proof("incoming sigma length"))?;
    let mut view = vec![0; expected];
    let count = original.len().min(expected);
    view[..count].copy_from_slice(&original[..count]);
    Ok((view, length))
}

fn sigma_slot(
    sources: &QualifiedWalletSourcesV1,
    selector: u8,
    statement: &[Fp; 26],
    original: &[u8],
) -> Result<SigmaSlotWitness, Error> {
    let key = sources
        .sigmas()
        .key(selector)
        .ok_or(Error::Proof("incoming sigma source"))?;
    let expected = proof(Protocol::new(key.binding().descriptor()))?.proof_length();
    let (bytes, length) = sigma_view(original, expected)?;
    Ok(SigmaSlotWitness {
        key: key.key().clone(),
        statement: hash_with_domain(STATEMENT_DOMAIN, statement),
        proof: bytes,
        length,
    })
}

/// `Λ_recv`'s four incoming results and their modes, exactly as the fixed Receive owners
/// derive them: the Omega transport, the incoming Q-σ, the three nonproof predicates and
/// every decide. The fold binds them into its proposal; Receive admission refuses, before
/// Advance, every Payment whose verdict burns.
struct ReceiveVerdictV1 {
    incoming: super::incoming::TransportV1,
    slot: SigmaSlotWitness,
    results: [bool; 4],
    modes: [IncomingMode; 4],
    pallas_corrections: [iroha_pasta::EpAffine; 2],
    vesta_correction: iroha_pasta::EqAffine,
}

impl ReceiveVerdictV1 {
    /// The burn branch: a false result or a corrected claim.
    fn burns(&self) -> bool {
        self.modes.contains(&IncomingMode::Corrected) || !self.results.iter().all(|v| *v)
    }
}

/// Derive the verdict from exact predicate inputs, whose incoming selector must be the
/// route's. The fold supplies its own source; admission supplies its projection.
fn receive_verdict(
    installed: &InstalledVerifierPackV1,
    sources: &QualifiedWalletSourcesV1,
    budget: MemoryBudget,
    route: OperationRoute,
    input: receive::PredicateInputs,
    cancellation: Option<&iroha_pasta::CancellationToken>,
) -> Result<ReceiveVerdictV1, Error> {
    iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(|_| Error::Cancelled)?;
    let pallas_params = installed.verifier().pallas_parameters();
    let vesta_params = proof(installed.verifier().vesta_parameters(16))?;
    let fixed = proof(sources.route(route))?;
    let QualifiedOperationOwnerV1::Receive(prover) = fixed.owner() else {
        return Err(Error::Proof("Receive source route"));
    };
    let omega = fixed
        .candidate_omega()
        .ok_or(Error::Proof("Receive Omega source"))?;
    let incoming = super::incoming::transport(
        pallas_params,
        vesta_params,
        omega,
        &input.objects[4],
        budget,
        cancellation,
    )?;
    let selector = route
        .incoming
        .filter(|selector| *selector == input.incoming_selector)
        .ok_or(Error::Proof("Receive incoming selector"))?;
    let slot = sigma_slot(
        sources,
        selector,
        &input.incoming_statement,
        &input.objects[5],
    )?;
    let q = proof(sources.q(route))?;
    let sigma = proof(
        q.sigma()
            .ok_or(Error::Proof("incoming sigma source"))?
            .incoming_original_cancellable(&slot, selector, budget, cancellation),
    )?;
    let bits = proof(prover.propose_nonproof(input))?;
    let results = [incoming.valid && sigma.is_some(), bits[0], bits[1], bits[2]];
    let (modes, pallas_corrections, vesta_correction) = super::incoming::modes(
        pallas_params,
        vesta_params,
        results.iter().all(|v| *v),
        &incoming,
        sigma.as_ref(),
        budget,
        cancellation,
    )?;
    Ok(ReceiveVerdictV1 {
        incoming,
        slot,
        results,
        modes,
        pallas_corrections,
        vesta_correction,
    })
}

impl NativeFoldWorkerV1 {
    /// Receive admission (§3.2): before a new Receive plan can reach Advance, derive the
    /// verdict its fold will derive and refuse the Payment if that verdict burns it, so an
    /// accepted Payment is never burned. The predicates read the receiver lineage only
    /// through its stable identity and never read the own receipt, so the admission tapes
    /// (`PreparationV1::receive_admission_fields`) give the fold's exact results and modes.
    /// # Errors
    /// [`Error::Invalid`] with [`super::RECEIVE_ADMISSION_REFUSAL`] for a Payment the fold
    /// would burn, a final verdict on these exact bytes; otherwise the verdict's own errors.
    pub(crate) fn receive_admission(
        &self,
        preparation: &PreparationV1<'_>,
        prepared: &PreparedOperationV1,
        source: &ReleasedStep,
    ) -> Result<(), Error> {
        let owner = prepared.owner();
        let monetary = prepared
            .monetary()
            .ok_or(Error::Proof("Receive preparation"))?;
        let fields = proof(preparation.receive_admission_fields(owner, source, monetary))?;
        let route = super::inputs::receive_route(fields.renewed, &fields.payment)?;
        let selector = route
            .incoming
            .ok_or(Error::Proof("Receive incoming selector"))?;
        let root = self.sources.scope().root();
        let signatures = super::inputs::receive_signatures_under(
            &fields.objects,
            super::inputs::key(&fields.own_key, 0)?,
            fields.renewed,
            [root.x, root.y],
        )?;
        let verdict = receive_verdict(
            &self.installed,
            &self.sources,
            self.budget,
            route,
            receive::PredicateInputs {
                before: fields.before,
                after: fields.after,
                statement: fields.statement,
                consumed: fields.consumed,
                incoming_statement: fields.incoming_statement,
                objects: fields.objects,
                signatures,
                incoming_selector: selector,
            },
            None,
        )?;
        if verdict.burns() {
            return Err(Error::Invalid(super::RECEIVE_ADMISSION_REFUSAL));
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn receive(
    worker: &NativeFoldWorkerV1,
    preparation: &PreparationV1<'_>,
    prepared: &PreparedOperationV1,
    step: &ReleasedStep,
    predecessor: &FoldedStateV1,
    public: &mut KagemushaWalletLineagePublicV1,
    custody: &mut FoldCustodyV1<'_>,
    route: OperationRoute,
    cancellation: &Cancellation,
) -> Result<(ReceiveFoldFieldsV1, Option<IncomingSigma>), Error> {
    cancellation.check()?;
    let owner = prepared.owner();
    let monetary = prepared
        .monetary()
        .ok_or(Error::Proof("Receive preparation"))?;
    let sources = proof(preparation.receive_source_fields(
        owner,
        step,
        predecessor,
        monetary,
        public,
        worker.budget,
    ))?;
    let selector = route
        .incoming
        .ok_or(Error::Proof("Receive incoming selector"))?;
    let root = worker.sources.scope().root();
    let signatures = super::inputs::receive_signatures(
        &sources.objects,
        &sources.credential,
        route.variant == Variant::ReceiveRenewed,
        [root.x, root.y],
    )?;
    let verdict = receive_verdict(
        &worker.installed,
        &worker.sources,
        worker.budget,
        route,
        receive::PredicateInputs {
            before: sources.before,
            after: sources.after,
            statement: sources.statement,
            consumed: sources.consumed,
            incoming_statement: sources.incoming_statement,
            objects: sources.objects,
            signatures,
            incoming_selector: selector,
        },
        Some(cancellation.prover_token()),
    )?;
    let burned = verdict.burns();
    let KagemushaWalletEffectV1::Receive { amount, .. } =
        step.frozen.capsule.body().statement.effect
    else {
        return Err(Error::Proof("Receive amount"));
    };
    public.burned_total = predecessor
        .lineage()
        .public
        .burned_total
        .checked_add(if burned { amount } else { 0 })
        .ok_or(Error::Proof("Receive burn overflow"))?;
    let credit = custody.credit_record(burned)?;
    public.credit_digest_root = custody.credit_root();
    let ReceiveVerdictV1 {
        incoming,
        slot,
        results,
        modes,
        pallas_corrections,
        vesta_correction,
    } = verdict;
    let proposal = receive::IncomingWitness {
        public: incoming.public,
        public_valid: incoming.public_valid,
        pallas: incoming.pallas,
        vesta: incoming.vesta,
        opening: incoming.opening,
        results,
        modes,
        pallas_corrections,
        vesta_correction,
    };
    let fields = proof(preparation.receive_fold_fields(
        owner,
        step,
        predecessor,
        monetary,
        public,
        &credit,
        proposal,
        worker.budget,
    ))?;
    Ok((
        fields,
        Some(IncomingSigma {
            sigma: slot,
            mode: modes[3],
        }),
    ))
}

fn retained(step: &ReleasedStep, role: KagemushaWalletRetainedInputRoleV1) -> Result<&[u8], Error> {
    let mut found = step
        .frozen
        .capsule
        .body()
        .retained_inputs
        .iter()
        .filter(|i| i.role == role);
    let original = found.next().ok_or(Error::WitnessLost("Archive original"))?;
    if found.next().is_some() {
        return Err(Error::WitnessLost("duplicate Archive original"));
    }
    Ok(&original.bytes)
}
fn status_witness(
    transport: &super::incoming::TransportV1,
    modes: [IncomingMode; 3],
    pc: [iroha_pasta::EpAffine; 2],
    vc: iroha_pasta::EqAffine,
) -> archive::StatusWitness {
    archive::StatusWitness {
        public: transport.public,
        public_valid: transport.public_valid,
        pallas: transport.pallas.clone(),
        vesta: transport.vesta.clone(),
        opening: transport.opening.clone(),
        modes,
        pallas_corrections: pc,
        vesta_correction: vc,
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn archive(
    worker: &NativeFoldWorkerV1,
    preparation: &PreparationV1<'_>,
    prepared: &PreparedOperationV1,
    step: &ReleasedStep,
    predecessor: &FoldedStateV1,
    public: &mut KagemushaWalletLineagePublicV1,
    custody: &mut FoldCustodyV1<'_>,
    route: OperationRoute,
    cancellation: &Cancellation,
) -> Result<(ArchiveFoldFieldsV1, Option<IncomingSigma>), Error> {
    cancellation.check()?;
    let pallas_params = worker.installed.verifier().pallas_parameters();
    let vesta_params = proof(worker.installed.verifier().vesta_parameters(16))?;
    let fixed = proof(worker.sources.route(route))?;
    let QualifiedOperationOwnerV1::Archive(prover) = fixed.owner() else {
        return Err(Error::Proof("Archive source route"));
    };
    let original = retained(step, KagemushaWalletRetainedInputRoleV1::Credited)?;
    if original.is_empty() || original.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 {
        return Err(Error::Proof("Archive evidence bound"));
    }
    let credited: KagemushaWalletCreditedV1 = proof(norito::decode_canonical_with_limits(
        original,
        norito::canonical_decode_limits(original.len()),
    ))?;
    let transport = match &credited.evidence {
        KagemushaWalletCreditedEvidenceV1::Status { status } => Some(super::incoming::transport(
            pallas_params,
            vesta_params,
            fixed
                .candidate_omega()
                .ok_or(Error::Proof("Archive Omega source"))?,
            &status.lineage.bytes(),
            worker.budget,
            Some(cancellation.prover_token()),
        )?),
        KagemushaWalletCreditedEvidenceV1::Receive { .. } => None,
    };
    let initial = if let Some(t) = &transport {
        let (m, p, v) = super::incoming::status_modes(
            pallas_params,
            vesta_params,
            false,
            t,
            worker.budget,
            Some(cancellation.prover_token()),
        )?;
        ArchiveIncomingWitnessV1::Status(Box::new(status_witness(t, m, p, v)))
    } else {
        ArchiveIncomingWitnessV1::Receive(Box::new(IncomingMode::Trivial))
    };
    let adjusted = custody.pending_remove()?;
    let mut fields = proof(preparation.archive_fold_fields(
        prepared.owner(),
        step,
        predecessor,
        public,
        ArchiveFoldWitnessV1 {
            adjusted_pending: adjusted,
            incoming: initial,
            results: [false; 3],
        },
        worker.budget,
    ))?;
    let (receipt, sigma_slot_value, sigma_claim) = match &fields.evidence {
        archive::Evidence::Receive {
            statement,
            receipt,
            sigma,
            ..
        } => {
            let selector = route
                .incoming
                .ok_or(Error::Proof("Archive Receive selector"))?;
            let slot = sigma_slot(&worker.sources, selector, statement, sigma)?;
            let q = proof(worker.sources.q(route))?;
            let claim = proof(
                q.sigma()
                    .ok_or(Error::Proof("incoming sigma source"))?
                    .incoming_original_cancellable(
                        &slot,
                        selector,
                        worker.budget,
                        Some(cancellation.prover_token()),
                    ),
            )?;
            (receipt, Some(slot), claim)
        }
        archive::Evidence::Status { receipt, .. } => (receipt, None, None),
    };
    // The merged leaf's witnesses in slot order: the own receipt (hard) under the
    // payment key its current credential opens, then the incoming receipt (soft).
    let signatures = [
        super::inputs::signature(
            ObjectKind::Receipt,
            &fields.own,
            super::inputs::key(&fields.credential, 130)?,
        )?,
        super::inputs::signature(
            ObjectKind::Receipt,
            receipt,
            super::inputs::key(&fields.retained.signed[3], 130)?,
        )?,
    ];
    let (pp, pv) = predecessor_claims(predecessor)?;
    let bits = proof(prover.propose_nonproof(archive::PredicateInputs {
        state: fields.state,
        own: fields.own.clone(),
        retained: fields.retained.clone(),
        evidence: fields.evidence.clone(),
        signatures,
        predecessor_pallas: pp,
        predecessor_vesta: pv,
        incoming_selector: route.incoming,
    }))?;
    let results = [
        transport
            .as_ref()
            .map_or_else(|| sigma_claim.is_some(), |t| t.valid),
        bits[0],
        bits[1],
    ];
    let soft = results.iter().all(|v| *v);
    let (proposal, q_incoming, accepted) = if let Some(t) = transport {
        let (m, p, v) = super::incoming::status_modes(
            pallas_params,
            vesta_params,
            soft,
            &t,
            worker.budget,
            Some(cancellation.prover_token()),
        )?;
        (
            ArchiveIncomingWitnessV1::Status(Box::new(status_witness(&t, m, p, v))),
            None,
            soft && m == [IncomingMode::Accept; 3],
        )
    } else {
        let mode = super::incoming::sigma_mode(
            vesta_params,
            soft,
            sigma_claim.as_ref(),
            worker.budget,
            Some(cancellation.prover_token()),
        )?;
        (
            ArchiveIncomingWitnessV1::Receive(Box::new(mode)),
            Some(IncomingSigma {
                sigma: sigma_slot_value.ok_or(Error::Proof("Archive incoming sigma"))?,
                mode,
            }),
            soft && mode == IncomingMode::Accept,
        )
    };
    let KagemushaWalletEffectV1::ArchiveSent { credit_id, .. } =
        step.frozen.capsule.body().statement.effect
    else {
        return Err(Error::Proof("Archive credit"));
    };
    public.pending_outgoing_root = if accepted {
        proof(adjusted.verify(
            &predecessor.lineage().public.pending_outgoing_root,
            &credit_id,
        ))?
    } else {
        predecessor.lineage().public.pending_outgoing_root
    };
    fields = proof(preparation.archive_fold_fields(
        prepared.owner(),
        step,
        predecessor,
        public,
        ArchiveFoldWitnessV1 {
            adjusted_pending: adjusted,
            incoming: proposal,
            results,
        },
        worker.budget,
    ))?;
    Ok((fields, q_incoming))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incoming_sigma_fixed_view_keeps_original_length_and_zero_padding() {
        let original = (0_u8..96).collect::<Vec<_>>();
        for length in [0, 1, 31, 32, 63, 64, 65, 96] {
            let (view, declared) = sigma_view(&original[..length], 64).unwrap();
            assert_eq!(declared as usize, length);
            assert_eq!(&view[..length.min(64)], &original[..length.min(64)]);
            assert!(view[length.min(64)..].iter().all(|v| *v == 0));
        }
        assert!(sigma_view(&original, 0).is_err());
        assert!(sigma_view(&original, 63).is_err());
    }
}
