//! Load's signed authorization, receipt and recovery-map composition.
//!
//! These constraints consume the exact state and Q outputs retained by the A
//! frame. The `LoadAuthorization` issuer attests finalized ledger funding; this
//! relation does not accept a caller-supplied finality height or boolean.

use ff::Field;
use iroha_pasta::{Ep, Fp};
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{GlueChip, UintChip, bytes::tape::BytesChip};
use iroha_plonk_recursion::{obligation::ledger::Variant, verifier::VerifierChip};

use super::{
    SigmaBindingCells, SignatureProofCells,
    context::{ContextObjectCells, ContextObjectSpec},
    own::{CurrentAuthorization, OwnPolicy, authenticate_current},
};
use crate::{
    operation_relation::{
        map_effects::{InsertCells, MapEffectsChip, MapState, MapTransition},
        objects::{
            ObjectKind, SignedObjectCells,
            issuer::{self, IssuerAuthorization},
            policy::PolicyCells,
            receipt::{self, ReceiptContext},
        },
    },
    q_signature::SignatureKey,
};

/// Load certificate, finalized voucher, own receipt, Enrollment certificate and current credential.
#[derive(Clone, Debug)]
pub struct LoadObjects {
    objects: [SignedObjectCells; 5],
    context: [ContextObjectCells; 5],
}
/// State and authenticated Q outputs consumed by Load's signed-object relation.
#[derive(Clone, Copy)]
pub struct LoadInputs<'a> {
    /// Canonical state and public prefix authenticated by the hard predecessor.
    pub predecessor: MapState<'a>,
    /// Canonical successor state and public prefix retained by A.
    pub successor: MapState<'a>,
    /// Own exact statement and sigma-only proof digest from the Q-bound tape.
    pub sigma: &'a SigmaBindingCells,
    /// Slots for the owning task: receipt/voucher/Load certificate, or current credential/Enrollment certificate.
    pub signatures: &'a [SignatureProofCells],
}
impl LoadObjects {
    /// Fixed ordered context object schema, including raw signatures.
    ///
    /// # Errors
    /// An impossible size conversion in the fixed schema.
    pub fn context_specs() -> Result<[ContextObjectSpec; 5], Error> {
        let mut out = Vec::new();
        for (tag, kind) in [
            (1, ObjectKind::Certificate),
            (2, ObjectKind::Voucher),
            (3, ObjectKind::Receipt),
            (4, ObjectKind::Certificate),
            (5, ObjectKind::Credential),
        ] {
            out.push(ContextObjectSpec {
                tag,
                capacity: u32::try_from(kind.body_len() + 64).map_err(|_| Error::BoundsFailure)?,
            });
        }
        out.try_into().map_err(|_| Error::Synthesis)
    }
    /// Parse each hard own object and commit the identical canonical byte tape.
    ///
    /// # Errors
    /// A wrong fixed shape or layout error; malformed own objects are unsatisfiable.
    pub fn decode(
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        source: [&[Value<u8>]; 5],
    ) -> Result<Self, Error> {
        let mut objects = Vec::new();
        let mut context = Vec::new();
        for ((kind, source), spec) in [
            ObjectKind::Certificate,
            ObjectKind::Voucher,
            ObjectKind::Receipt,
            ObjectKind::Certificate,
            ObjectKind::Credential,
        ]
        .into_iter()
        .zip(source)
        .zip(Self::context_specs()?)
        {
            let run = bytes.run(
                region,
                source,
                &kind.primary_segments(),
                &kind.secondary_segments(),
            )?;
            let lanes = chip.operation_lanes()?;
            let object = SignedObjectCells::from_run(
                &mut UintChip::new(lanes.glue, lanes.range),
                lanes.hash,
                region,
                kind,
                &run,
            )?;
            context.push(ContextObjectCells::from_exact_run(
                chip,
                region,
                spec,
                object.digest(),
                &run,
            )?);
            objects.push(object);
        }
        Ok(Self {
            objects: objects.try_into().map_err(|_| Error::Synthesis)?,
            context: context.try_into().map_err(|_| Error::Synthesis)?,
        })
    }
    /// Same-tape object commitments rebound by every split stage.
    pub const fn context(&self) -> &[ContextObjectCells; 5] {
        &self.context
    }

    /// Hard-authenticate the finalized voucher and exact own receipt.
    ///
    /// The separate recovery relation must also run in the composed A circuit.
    /// The predecessor payment key is already authenticated by the hard lineage
    /// proof; the voucher's delegated key is authorized only for role2.
    /// `ChargeQuote` signatures are outside this adopted Load relation.
    ///
    /// # Errors
    /// Wrong variant, slot/root policy, missing same-tape digest or layout error.
    /// Invalid own signatures, scope, amounts and receipt bindings are unsatisfiable.
    pub fn authenticate(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        policy: OwnPolicy,
        input: LoadInputs<'_>,
    ) -> Result<(), Error> {
        let statement = input.sigma.hard_statement()?;
        let slots = input.signatures;
        if statement.variant() != Variant::Load
            || slots.len() != 3
            || slots[2].key_policy() != SignatureKey::Fixed(policy.root)
        {
            return Err(Error::Synthesis);
        }
        let mut uint = chip.uint();
        let scheme = policy
            .scheme
            .map(|v| uint.constant::<128>(region, v).map(|v| v.word().clone()))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let provider = policy
            .provider
            .map(|v| uint.constant::<128>(region, v).map(|v| v.word().clone()))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let valid = issuer::authenticate(
            &mut uint,
            region,
            &self.objects[1],
            &IssuerAuthorization {
                certificate: &self.objects[0],
                certificate_proof: &slots[2],
                object_proof: &slots[1],
                root_key: slots[2].key(),
                scheme: &scheme,
            },
        )?;
        GlueChip::assert_constant(region, valid.word(), Fp::ONE)?;
        let wallet = [
            input.successor.lineage.fields()[6].clone(),
            input.successor.lineage.fields()[7].clone(),
        ];
        let policy = PolicyCells::check(&mut uint, region, &self.objects[1])?;
        let valid = policy.bind_load_voucher(&mut uint, region, statement, &wallet)?;
        GlueChip::assert_constant(region, valid.word(), Fp::ONE)?;
        let payment_key =
            core::array::from_fn(|i| input.predecessor.lineage.fields()[9 + i].clone());
        let valid = self.objects[2].bind_signature(region, &slots[0], &payment_key)?;
        GlueChip::assert_constant(region, valid.word(), Fp::ONE)?;
        let zero = uint.glue().constant(region, Fp::ZERO)?;
        let lanes = chip.operation_lanes()?;
        let valid = receipt::bind(
            &mut UintChip::new(lanes.glue, lanes.range),
            lanes.hash,
            region,
            &self.objects[2],
            &ReceiptContext {
                wallet: &wallet,
                provider: &provider,
                statement,
                proof_digest: input.sigma.step_digest()?,
                payment_digest: &zero,
            },
        )?;
        GlueChip::assert_constant(region, valid.word(), Fp::ONE)
    }
    /// Reverify the current credential and its Enrollment certificate for this step.
    /// # Errors
    /// Wrong signature count/root/scope, wrong own state, or layout failure.
    pub fn authenticate_current(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        policy: OwnPolicy,
        input: LoadInputs<'_>,
    ) -> Result<(), Error> {
        if input.signatures.len() != 2 || input.sigma.hard_statement()?.variant() != Variant::Load {
            return Err(Error::Synthesis);
        }
        authenticate_current(
            chip,
            region,
            policy,
            CurrentAuthorization {
                credential: &self.objects[4],
                certificate: &self.objects[3],
                credential_proof: &input.signatures[0],
                certificate_proof: &input.signatures[1],
                current: input.predecessor,
                statement: input.sigma.hard_statement()?,
            },
        )
    }
    /// Prove the exact recovery insertion, arithmetic and unchanged-field rules.
    ///
    /// This hard map obligation can occupy a separate fixed split stage from
    /// object authentication, provided its state/statement is rebound by context.
    ///
    /// # Errors
    /// Wrong statement/variant or layout error; incorrect paths are unsatisfiable.
    pub fn recovery(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: LoadInputs<'_>,
        insertion: &InsertCells,
    ) -> Result<(), Error> {
        let statement = input.sigma.hard_statement()?;
        if statement.variant() != Variant::Load {
            return Err(Error::Synthesis);
        }
        let lanes = chip.operation_lanes()?;
        MapEffectsChip::new(lanes.glue, lanes.range, lanes.hash).recovery(
            region,
            &MapTransition {
                statement,
                predecessor: input.predecessor,
                successor: input.successor,
            },
            insertion,
        )
    }
}

/// Fixed Load task ownership, coupled to the same Q/key/context schema.
#[derive(Clone, Debug)]
pub struct LoadStagePlan {
    context: super::context::ContextPlan,
}
impl LoadStagePlan {
    /// Require recovery, voucher/receipt authorization and current-credential
    /// authorization exactly once, with hard signature Q slots1 and2 respectively.
    /// # Errors
    /// Wrong variant/task set, signature-Q count or stage assignment.
    pub fn new(context: super::context::ContextPlan) -> Result<Self, Error> {
        use super::schedule::OperationTask;
        if context.operation().frame().variant() != Variant::Load
            || context.operation().q_count() != 3
            || context.object_specs() != LoadObjects::context_specs()?
        {
            return Err(Error::Synthesis);
        }
        let groups = (0..context.stage_count())
            .map(|i| context.operation_tasks(i).unwrap_or_default().to_vec())
            .collect::<Vec<_>>();
        OperationTask::validate(Variant::Load, &groups)?;
        for (stage, tasks) in groups.iter().enumerate() {
            for (task, q) in [
                (OperationTask::LoadAuthorization, 1),
                (OperationTask::LoadCurrentAuthorization, 2),
            ] {
                if tasks.contains(&task)
                    && !context
                        .q_partition(stage)
                        .ok_or(Error::Synthesis)?
                        .contains(&q)
                {
                    return Err(Error::Synthesis);
                }
            }
        }
        Ok(Self { context })
    }
    /// Context committed by every stage and its pinned W wrapper.
    pub const fn context(&self) -> &super::context::ContextPlan {
        &self.context
    }
    /// Execute precisely the fixed stage's operation tasks on context-bound cells.
    /// # Errors
    /// Wrong stage, missing/extraneous recovery path, or any failed constraint.
    #[allow(clippy::too_many_arguments)] // One fixed stage and its same-cell operation inputs.
    pub fn constrain_stage(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        stage: usize,
        objects: &LoadObjects,
        policy: OwnPolicy,
        input: LoadInputs<'_>,
        insertion: Option<&InsertCells>,
        signature: Option<super::SignatureQContext<'_>>,
    ) -> Result<(), Error> {
        use super::schedule::OperationTask;
        let tasks = self
            .context
            .operation_tasks(stage)
            .ok_or(Error::Synthesis)?;
        if tasks.contains(&OperationTask::LoadRecovery) != insertion.is_some() {
            return Err(Error::Synthesis);
        }
        let owning_q = if tasks.contains(&OperationTask::LoadAuthorization) {
            Some(1)
        } else if tasks.contains(&OperationTask::LoadCurrentAuthorization) {
            Some(2)
        } else {
            None
        };
        if owning_q.is_some() != signature.is_some() {
            return Err(Error::Synthesis);
        }
        let input = if let Some(q) = signature {
            q.bundle.bind_context(
                region,
                self.context.operation(),
                owning_q.ok_or(Error::Synthesis)?,
                q.instances,
            )?;
            LoadInputs {
                signatures: q.bundle.slots(),
                ..input
            }
        } else {
            input
        };
        for task in tasks {
            match task {
                OperationTask::LoadRecovery => {
                    LoadObjects::recovery(chip, region, input, insertion.ok_or(Error::Synthesis)?)?
                }
                OperationTask::LoadAuthorization => {
                    objects.authenticate(chip, region, policy, input)?
                }
                OperationTask::LoadCurrentAuthorization => {
                    objects.authenticate_current(chip, region, policy, input)?
                }
                _ => return Err(Error::Synthesis),
            }
        }
        Ok(())
    }
}
