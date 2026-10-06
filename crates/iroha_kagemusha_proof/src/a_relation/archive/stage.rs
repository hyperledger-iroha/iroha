//! Complete fixed Archive owner dispatch and original-object schema.

use iroha_pasta::{Ep, Fp};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::bytes::tape::BytesChip;
use iroha_plonk_recursion::{obligation::ledger::Variant, verifier::VerifierChip};

use super::{
    authorization::ArchiveAuthorizationObjects,
    evidence::{self, ArchiveEvidenceSource},
    incoming::ArchiveIncomingObjects,
    maps::{self, ArchiveEffectsCells},
    proofs::{ArchiveProofInputs, ArchiveProofSources},
    results::{ArchiveResultClaims, ArchiveResultPlan},
    retained::ArchiveRetainedPayment,
};
use crate::{
    a_relation::{
        SigmaBindingCells, SignatureQCells,
        context::{ContextInputs, ContextObjectSpec, ContextPlan},
        own::OwnPolicy,
        schedule::OperationTask,
    },
    operation_relation::map_effects::ArchiveMapWitness,
    q_signature::QSignaturePlan,
};

/// Fixed complete Archive program. This checks source schema and dispatch,
/// not circuit capacity, actual Q/A/W execution or terminal artifact admission.
#[derive(Clone, Debug)]
pub struct ArchiveStagePlan {
    context: ContextPlan,
    policy: OwnPolicy,
    signatures: [QSignaturePlan; 2],
    results: ArchiveResultPlan,
}

/// Same original context and exact views required by this particular stage.
#[derive(Clone, Copy)]
pub struct ArchiveStageInputs<'a> {
    /// Complete source context, rebound by every recursive continuation.
    pub context: &'a ContextInputs<'a>,
    /// All three proposed results and original Status opening.
    pub results: &'a ArchiveResultClaims,
    /// Current credential/certificate/receipt only for their named owners.
    pub own: Option<&'a ArchiveAuthorizationObjects>,
    /// Exact historical Payment only for its mandatory hard owner.
    pub retained: Option<&'a ArchiveRetainedPayment>,
    /// Held Request/receiver and soft receipt, for Evidence or Signatures.
    pub incoming: Option<&'a ArchiveIncomingObjects>,
    /// Own sigma projection only for the own-proof owner.
    pub sigma: Option<&'a SigmaBindingCells>,
    /// Exact incoming proof carrier only for the Proofs owner.
    pub proof_source: Option<&'a ArchiveProofSources>,
}

/// Additional actual verifiers, original tapes and paths for the assigned tasks.
#[derive(Clone, Copy, Default)]
pub struct ArchiveStageWitness<'a> {
    /// Hard current authorization Q1; required only at Authorization.
    pub own_signatures: Option<&'a SignatureQCells>,
    /// Soft incoming receipt Q2, hard-verified at Signatures.
    pub incoming_signatures: Option<&'a SignatureQCells>,
    /// Original Receive sigma pair or Status trusted Omega key.
    pub proof: Option<ArchiveProofInputs<'a>>,
    /// Original incoming evidence tapes, only for Evidence.
    pub evidence: Option<ArchiveEvidenceSource<'a>>,
    /// Both authenticated pending removals, only for terminal Effects.
    pub maps: Option<&'a ArchiveMapWitness>,
}

impl ArchiveStagePlan {
    /// Own3, retained9, incoming receipt/raw proof, exact evidence and typed results.
    /// All capacities belong to this fixed key schema, never a witness choice.
    /// # Errors
    /// Wrong variant or empty/overflowing active-proof capacity.
    pub fn context_specs(
        variant: Variant,
        retained_omega: usize,
        retained_sigma: usize,
        incoming_proof: usize,
    ) -> Result<Vec<ContextObjectSpec>, Error> {
        super::require_variant(variant)?;
        if incoming_proof == 0 {
            return Err(Error::Synthesis);
        }
        let mut specs = ArchiveAuthorizationObjects::context_specs()?.to_vec();
        specs.extend(ArchiveRetainedPayment::context_specs(
            retained_omega,
            retained_sigma,
        )?);
        specs.push(ArchiveIncomingObjects::context_specs()?[2]);
        specs.push(ContextObjectSpec {
            tag: 14,
            capacity: u32::try_from(incoming_proof).map_err(|_| Error::BoundsFailure)?,
        });
        specs.extend(evidence::evidence_specs(variant)?);
        specs.push(ArchiveResultPlan::context_spec(
            variant,
            u32::try_from(specs.len() + 1).map_err(|_| Error::BoundsFailure)?,
        )?);
        Ok(specs)
    }

    /// Validate complete tasks, all original categories and the actual Q schemas.
    /// Q1/Q2 must execute with their exact authorization/signature owners.
    /// # Errors
    /// Wrong class/count/shape, missing or repeated owner, or misplaced Q.
    pub fn new(context: ContextPlan, policy: OwnPolicy) -> Result<Self, Error> {
        let variant = context.operation().frame().variant();
        super::require_variant(variant)?;
        let specs = context.object_specs();
        if context.operation().q_count() != 3
            || specs.len() < 14
            || context.operation().sigma.slot_count()
                != 1 + usize::from(variant == Variant::ArchiveReceive)
            || specs
                != Self::context_specs(
                    variant,
                    specs[9].capacity as usize,
                    specs[10].capacity as usize,
                    specs[13].capacity as usize,
                )?
        {
            return Err(Error::Synthesis);
        }
        let signatures = ArchiveAuthorizationObjects::signature_schemas(policy)?;
        for (i, schema) in signatures.iter().enumerate() {
            let descriptor = context
                .operation()
                .q(i + 1)
                .ok_or(Error::Synthesis)?
                .verifier()
                .binding()
                .descriptor();
            if descriptor.instance_lengths
                != [u32::try_from(schema.instance_length()).map_err(|_| Error::BoundsFailure)?]
                || descriptor.instance_types.as_deref() != Some(&QSignaturePlan::instance_types())
            {
                return Err(Error::Synthesis);
            }
        }
        let results = ArchiveResultPlan::new(&context, specs.len() - 1)?;
        for stage in 0..context.stage_count() {
            let tasks = context.operation_tasks(stage).ok_or(Error::Synthesis)?;
            let partition = context.q_partition(stage).ok_or(Error::Synthesis)?;
            for (task, q) in [
                (OperationTask::ArchiveAuthorization, 1),
                (OperationTask::ArchiveSignatures, 2),
            ] {
                if tasks.contains(&task) && !partition.contains(&q) {
                    return Err(Error::Synthesis);
                }
            }
        }
        Ok(Self {
            context,
            policy,
            signatures,
            results,
        })
    }

    /// Exact context used by every split A/W continuation.
    pub const fn context(&self) -> &ContextPlan {
        &self.context
    }
    /// Complete result plan used to assign the same proposal at every stage.
    pub const fn results(&self) -> ArchiveResultPlan {
        self.results
    }
    /// Fixed Q1 or Q2 signature schema. Q0 is the separate sigma program.
    pub fn signature_schema(&self, q: usize) -> Option<&QSignaturePlan> {
        q.checked_sub(1)
            .and_then(|index| self.signatures.get(index))
    }

    /// Execute every assigned task exactly once on the same original context.
    /// The recursive owner must still hard-verify Q/W/predecessor obligations,
    /// close every deferred claim, and admit only measured complete stage keys.
    /// # Errors
    /// Missing/extraneous task source, wrong context/schema, altered original
    /// bytes, discretionary no-op or failed hard state/path/authentication rule.
    pub fn constrain_stage(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        stage: u32,
        input: ArchiveStageInputs<'_>,
        witness: ArchiveStageWitness<'_>,
    ) -> Result<Option<ArchiveEffectsCells>, Error> {
        use OperationTask::*;
        let tasks = self
            .context
            .operation_tasks(usize::try_from(stage).map_err(|_| Error::BoundsFailure)?)
            .ok_or(Error::Synthesis)?;
        for (required, present) in [
            (
                tasks.contains(&ArchiveAuthorization) || tasks.contains(&ArchiveOwnProof),
                input.own.is_some(),
            ),
            (
                tasks.contains(&ArchiveRetainedPayment),
                input.retained.is_some(),
            ),
            (
                tasks.contains(&ArchiveEvidence) || tasks.contains(&ArchiveSignatures),
                input.incoming.is_some(),
            ),
            (tasks.contains(&ArchiveOwnProof), input.sigma.is_some()),
            (tasks.contains(&ArchiveProofs), input.proof_source.is_some()),
            (tasks.contains(&ArchiveProofs), witness.proof.is_some()),
            (
                tasks.contains(&ArchiveAuthorization),
                witness.own_signatures.is_some(),
            ),
            (
                tasks.contains(&ArchiveSignatures),
                witness.incoming_signatures.is_some(),
            ),
            (tasks.contains(&ArchiveEvidence), witness.evidence.is_some()),
            (tasks.contains(&ArchiveEffects), witness.maps.is_some()),
        ] {
            if required != present {
                return Err(Error::Synthesis);
            }
        }
        input
            .results
            .bind_context(region, &self.context, input.context)?;
        let mut effects = None;
        for task in tasks {
            match task {
                ArchiveRetainedPayment => input.retained.ok_or(Error::Synthesis)?.bind_context(
                    region,
                    &self.context,
                    stage,
                    input.context,
                )?,
                ArchiveAuthorization => input.own.ok_or(Error::Synthesis)?.constrain_current(
                    chip,
                    region,
                    &self.context,
                    stage,
                    input.context,
                    (self.policy, witness.own_signatures.ok_or(Error::Synthesis)?),
                )?,
                ArchiveOwnProof => input.own.ok_or(Error::Synthesis)?.constrain_own_proof(
                    chip,
                    region,
                    &self.context,
                    stage,
                    input.context,
                    input.sigma.ok_or(Error::Synthesis)?,
                )?,
                ArchiveSignatures => input
                    .incoming
                    .ok_or(Error::Synthesis)?
                    .constrain_signature(
                        region,
                        &self.context,
                        stage,
                        input.context,
                        input.results,
                        witness.incoming_signatures.ok_or(Error::Synthesis)?,
                    )?,
                ArchiveProofs => {
                    let source = input.proof_source.ok_or(Error::Synthesis)?;
                    source.require_context_slot(13)?;
                    source.constrain_proofs(
                        chip,
                        region,
                        &self.context,
                        stage,
                        input.context,
                        input.results,
                        witness.proof.ok_or(Error::Synthesis)?,
                    )?;
                }
                ArchiveEvidence => evidence::constrain_evidence(
                    chip,
                    bytes,
                    region,
                    &self.context,
                    stage,
                    input.context,
                    input.results,
                    self.policy,
                    input.incoming.ok_or(Error::Synthesis)?,
                    witness.evidence.ok_or(Error::Synthesis)?,
                )?,
                ArchiveEffects => {
                    effects = Some(maps::constrain_effects(
                        chip,
                        region,
                        &self.context,
                        stage,
                        input.context,
                        input.results,
                        witness.maps.ok_or(Error::Synthesis)?,
                    )?)
                }
                _ => return Err(Error::Synthesis),
            }
        }
        Ok(effects)
    }
}
