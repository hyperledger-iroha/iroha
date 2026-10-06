//! Fixed Receive soft-result ownership, prior to its circuit composition.
//!
//! A result plan describes which stage must derive each predicate. Its five
//! boolean claims and fixed owner schema enter the original context, but this
//! does not certify execution and exposes no acceptance verdict. The Objects
//! producer binds its derived predicate and exact context inputs. TODO: compose
//! the remaining typed owners and consume all five in the terminal iff rule
//! before admitting a Receive terminal key.

use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{Bit, GlueChip, Word};
use iroha_plonk_recursion::obligation::ledger::Variant;

use super::{context::ContextPlan, schedule::OperationTask};

/// Namespace framing the fixed Receive result count, tags, owners and values.
pub const RECEIVE_RESULTS_DOMAIN: [u8; 8] = *b"kgwrslt1";

/// Exactly one of Receive's five required total predicate groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ReceiveResultTag {
    /// Incoming recursive proofs, key continuity and canonical decoders.
    Proofs = 1,
    /// Same-tape Payment, Request, payer credential and receipt semantics.
    Objects = 2,
    /// Exact incoming signature slots and quoted receiver authorization.
    Signatures = 3,
    /// Unique authenticated consumed-credit search proves absence.
    Nonmembership = 4,
    /// Recorded blacklist pair, history search and exact sigma selector.
    Blacklist = 5,
}
impl ReceiveResultTag {
    /// Canonical complete terminal set; it is never selected by witness data.
    pub const ALL: [Self; 5] = [
        Self::Proofs,
        Self::Objects,
        Self::Signatures,
        Self::Nonmembership,
        Self::Blacklist,
    ];

    /// Stable code within a separately framed result namespace.
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Constraint task that alone can own this result.
    pub const fn task(self) -> OperationTask {
        match self {
            Self::Proofs => OperationTask::ReceiveProofs,
            Self::Objects => OperationTask::ReceiveObjects,
            Self::Signatures => OperationTask::ReceiveSignatures,
            Self::Nonmembership => OperationTask::ReceiveNonmembership,
            Self::Blacklist => OperationTask::ReceiveBlacklist,
        }
    }
}

/// The complete fixed result set and its unique stage owners.
///
/// Private fixed-size storage prevents empty or shortened terminal policies.
/// Construction derives ownership from the complete operation task schedule;
/// callers cannot independently relabel a result's owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiveResultPlan {
    owners: [u32; 5],
}
impl ReceiveResultPlan {
    /// Derive all five result owners from a complete Receive task schedule.
    ///
    /// # Errors
    /// Wrong variant, missing/duplicate tasks, or malformed stage ordering.
    pub fn new(context: &ContextPlan) -> Result<Self, Error> {
        let groups = (0..context.stage_count())
            .map(|stage| {
                context
                    .operation_tasks(stage)
                    .map(<[OperationTask]>::to_vec)
                    .ok_or(Error::Synthesis)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_tasks(context.operation().frame().variant(), &groups)
    }

    pub(super) fn from_tasks(
        variant: Variant,
        groups: &[Vec<OperationTask>],
    ) -> Result<Self, Error> {
        if !matches!(variant, Variant::Receive | Variant::ReceiveRenewed) || groups.len() < 2 {
            return Err(Error::Synthesis);
        }
        OperationTask::validate(variant, groups)?;
        let mut owners = [0; 5];
        for (owner, tag) in owners.iter_mut().zip(ReceiveResultTag::ALL) {
            *owner = u32::try_from(
                groups
                    .iter()
                    .position(|group| group.contains(&tag.task()))
                    .ok_or(Error::Synthesis)?,
            )
            .map_err(|_| Error::BoundsFailure)?;
        }
        Ok(Self { owners })
    }

    /// Exact stage that must constrain the named task's complete result.
    pub const fn owner(self, tag: ReceiveResultTag) -> u32 {
        self.owners[tag as usize - 1]
    }

    /// Canonical `(tag, owner stage)` pairs to frame before the five values.
    /// Stage ordinals are zero-based checked u32 values. The context must frame
    /// this list with [`RECEIVE_RESULTS_DOMAIN`] and its fixed count of five;
    /// platform-sized integer encodings are not part of the schema.
    pub fn schema(self) -> [(u8, u32); 5] {
        ReceiveResultTag::ALL.map(|tag| (tag.code(), self.owner(tag)))
    }
}

/// Five boolean claims retained by the original split context.
///
/// This object is a commitment to proposed results, never evidence that any
/// predicate holds. It exposes no acceptance verdict. Every typed owning task
/// must constrain its complete predicate to the corresponding committed bit
/// before the fixed stage keys may be admitted.
#[derive(Clone, Debug)]
pub struct ReceiveResultClaims {
    plan: ReceiveResultPlan,
    values: [Bit<Fp>; 5],
}
impl ReceiveResultClaims {
    pub(super) fn bind_derived(
        &self,
        region: &mut Region<'_, Fp>,
        plan: ReceiveResultPlan,
        stage: u32,
        tag: ReceiveResultTag,
        derived: &Bit<Fp>,
    ) -> Result<(), Error> {
        if self.plan != plan || stage != plan.owner(tag) {
            return Err(Error::Synthesis);
        }
        GlueChip::assert_equal(region, self.values[tag as usize - 1].word(), derived.word())
    }
    /// Commit exactly five proposed results in the plan's fixed canonical order.
    /// # Errors
    /// Layout failure. This constructor does not validate the proposed results.
    pub fn assign(
        glue: &mut GlueChip<Fp>,
        region: &mut Region<'_, Fp>,
        plan: ReceiveResultPlan,
        values: [Value<bool>; 5],
    ) -> Result<Self, Error> {
        let values = values
            .iter()
            .map(|value| glue.boolean(region, *value))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        Ok(Self { plan, values })
    }

    pub(super) fn context_words(&self, plan: ReceiveResultPlan) -> Result<[Word<Fp>; 5], Error> {
        if self.plan != plan {
            return Err(Error::Synthesis);
        }
        Ok(self.values.each_ref().map(|value| value.word().clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field;
    use iroha_plonk::{
        check::{CheckMode, check_circuit},
        cs::{Column, ConstraintSystem, Instance},
        frontend::{Circuit, Layouter, SimpleFloorPlanner, synthesize},
    };
    use iroha_plonk_gadgets::GlueConfig;

    #[derive(Clone, Debug)]
    struct Config {
        glue: GlueConfig,
        public: Column<Instance>,
    }
    #[derive(Clone)]
    struct Claims {
        mask: u8,
        known: bool,
        wrong_plan: bool,
    }
    impl Circuit<Fp> for Claims {
        type Config = Config;
        type Params = ();
        type FloorPlanner = SimpleFloorPlanner;
        fn without_witnesses(&self) -> Self {
            Self {
                known: false,
                ..self.clone()
            }
        }
        fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
            let columns = core::array::from_fn(|_| meta.advice_column());
            let constant = meta.fixed_column();
            let glue = GlueConfig::configure(meta, columns, constant);
            let public = meta.instance_column(5);
            meta.enable_equality(public);
            Config { glue, public }
        }
        fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
            let mut glue = GlueChip::new(config.glue);
            let out = layouter.assign_region(
                || "fixed result claims",
                |mut region| {
                    let plan = ReceiveResultPlan::from_tasks(Variant::Receive, &groups())?;
                    let values = core::array::from_fn(|i| {
                        if self.known {
                            Value::known(self.mask & (1 << i) != 0)
                        } else {
                            Value::unknown()
                        }
                    });
                    let claims = ReceiveResultClaims::assign(&mut glue, &mut region, plan, values)?;
                    let expected = if self.wrong_plan {
                        let mut changed = groups();
                        changed[1].clear();
                        changed[0].push(OperationTask::ReceiveProofs);
                        changed[0].sort_unstable();
                        ReceiveResultPlan::from_tasks(Variant::Receive, &changed)?
                    } else {
                        plan
                    };
                    claims.context_words(expected)
                },
            )?;
            for (i, word) in out.iter().enumerate() {
                layouter.constrain_instance(word.cell(), config.public, i)?;
            }
            Ok(())
        }
    }

    fn groups() -> Vec<Vec<OperationTask>> {
        use OperationTask::{
            ReceiveAuthorization, ReceiveBlacklist, ReceiveEffects, ReceiveNonmembership,
            ReceiveObjects, ReceiveOwnProof, ReceiveProofs, ReceiveSignatures,
        };
        vec![
            vec![ReceiveOwnProof],
            vec![ReceiveProofs],
            vec![ReceiveObjects, ReceiveNonmembership],
            vec![ReceiveBlacklist],
            vec![ReceiveSignatures, ReceiveAuthorization, ReceiveEffects],
        ]
    }

    #[test]
    fn complete_result_set_has_exact_named_owners_for_both_receive_variants() {
        for variant in [Variant::Receive, Variant::ReceiveRenewed] {
            let plan = ReceiveResultPlan::from_tasks(variant, &groups()).unwrap();
            assert_eq!(plan.schema(), [(1, 1), (2, 2), (3, 4), (4, 2), (5, 3)]);
            assert_eq!(ReceiveResultTag::ALL.len(), 5);
        }
        assert!(ReceiveResultPlan::from_tasks(Variant::Send, &groups()).is_err());
        assert!(ReceiveResultPlan::from_tasks(Variant::Receive, &[]).is_err());
    }

    #[test]
    fn no_missing_doubled_or_relabelled_result_or_hard_task_is_accepted() {
        let honest = groups();
        for (stage, group) in honest.iter().enumerate() {
            for (index, task) in group.iter().copied().enumerate() {
                let mut missing = honest.clone();
                missing[stage].remove(index);
                assert!(ReceiveResultPlan::from_tasks(Variant::Receive, &missing).is_err());
                let mut duplicate = honest.clone();
                duplicate[0].push(task);
                duplicate[0].sort_unstable();
                assert!(ReceiveResultPlan::from_tasks(Variant::Receive, &duplicate).is_err());
                let mut relabelled = honest.clone();
                relabelled[stage][index] = OperationTask::SendProof;
                relabelled[stage].sort_unstable();
                assert!(ReceiveResultPlan::from_tasks(Variant::Receive, &relabelled).is_err());
            }
        }
    }

    #[test]
    fn moved_owner_changes_context_schema_without_changing_result_order() {
        let mut moved = groups();
        let original = ReceiveResultPlan::from_tasks(Variant::Receive, &moved).unwrap();
        moved[1].clear();
        moved[0].push(OperationTask::ReceiveProofs);
        moved[0].sort_unstable();
        let changed = ReceiveResultPlan::from_tasks(Variant::Receive, &moved).unwrap();
        assert_ne!(original.schema(), changed.schema());
        assert_eq!(changed.owner(ReceiveResultTag::Proofs), 0);
        for tag in ReceiveResultTag::ALL.into_iter().skip(1) {
            assert_eq!(original.owner(tag), changed.owner(tag));
        }
    }

    #[test]
    fn every_committed_result_is_boolean_bound_and_cannot_change_owner() {
        for mask in 0..32 {
            let circuit = Claims {
                mask,
                known: true,
                wrong_plan: false,
            };
            let public = vec![
                (0..5)
                    .map(|i| Fp::from(u64::from(mask & (1 << i) != 0)))
                    .collect::<Vec<_>>(),
            ];
            assert!(
                check_circuit(&circuit, 6, &public, CheckMode::Strict)
                    .unwrap()
                    .is_satisfied()
            );
            for index in 0..5 {
                let mut forged = public.clone();
                forged[0][index] = Fp::ONE - forged[0][index];
                assert!(
                    !check_circuit(&circuit, 6, &forged, CheckMode::Strict)
                        .unwrap()
                        .is_satisfied()
                );
            }
            let mut wrong = circuit.clone();
            wrong.wrong_plan = true;
            assert!(synthesize(&wrong, 6, None).is_err());
        }
        let circuit = Claims {
            mask: 21,
            known: true,
            wrong_plan: false,
        };
        let known = synthesize(&circuit, 6, None).unwrap();
        let unknown = synthesize(&circuit.without_witnesses(), 6, None).unwrap();
        assert_eq!(known.tables.fixed(), unknown.tables.fixed());
        assert_eq!(known.tables.permutation(), unknown.tables.permutation());
        let public = vec![vec![Fp::ONE, Fp::ZERO, Fp::ONE, Fp::ZERO, Fp::ONE]];
        assert!(
            iroha_plonk_gadgets::tamper::undetected_tampers(&circuit, 6, &public)
                .unwrap()
                .is_empty()
        );
    }
}
