//! Optional keyless sponsor-rule probe scoped to the discarded activation rehearsal.

use super::*;
use iroha_data_model::{
    NetworkId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        nexus::StageFeeSponsorProgramRevision,
    },
    nexus::{FeeSponsorProgramRevisionKey, FeeSponsorRuleEffect, FeeSponsorRuleSelector},
    runtime::definition::BindingV1,
};
use norito::derive::JsonDeserialize;

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Rule {
    id: String,
    wire_id: String,
}

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct Input {
    network_id: NetworkId,
    authority: AccountId,
    stage: InstructionBox,
    retained: Rule,
    added: Rule,
}

#[derive(Debug, JsonSerialize)]
pub(super) struct Observation {
    selection: String,
    retained_binding: BindingV1,
    added_binding: Option<BindingV1>,
    active_revision: Option<u64>,
    proposal_revision: u64,
    staged_in_transaction: bool,
    refusal: Option<String>,
}

#[derive(Debug, JsonSerialize)]
pub(super) struct Comparison {
    pub(super) predecessor: Observation,
    pub(super) successor: Observation,
}

impl Input {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > 65_536 {
            return Err("sponsor stage input exceeds the finite input bound".into());
        }
        let case: Self = norito::json::from_slice(bytes).map_err(|error| error.to_string())?;
        let revision = case.stage()?.revision();
        if case.authority != revision.program_id.sponsor
            || case.retained.id == case.added.id
            || case.retained.wire_id == case.added.wire_id
        {
            return Err("sponsor owner or two-rule selection differs".into());
        }
        for expected in [&case.retained, &case.added] {
            let mut matches = revision
                .rules
                .iter()
                .filter(|rule| rule.id.as_ref() == expected.id.as_str());
            let rule = matches.next().ok_or("selected sponsor rule missing")?;
            if matches.next().is_some()
                || rule.effect != FeeSponsorRuleEffect::Allow
                || !matches!(rule.selectors.as_slice(), [FeeSponsorRuleSelector::NativeInstruction(s)]
                     if s.wire_id == expected.wire_id && s.asset_definition_id.is_none())
            {
                return Err(
                    "selected sponsor rule differs from exact unfiltered native allow".into(),
                );
            }
        }
        Ok(case)
    }

    fn stage(&self) -> Result<&StageFeeSponsorProgramRevision, String> {
        self.stage
            .as_any()
            .downcast_ref::<StageFeeSponsorProgramRevision>()
            .ok_or_else(|| "expected one native StageFeeSponsorProgramRevision".into())
    }

    pub(super) fn observe(
        &self,
        block: &mut crate::state::StateBlock<'_>,
        after: bool,
    ) -> Result<Observation, String> {
        let stage = self.stage()?;
        let revision = stage.revision();
        let id = &revision.program_id;
        let key = FeeSponsorProgramRevisionKey::new(id.clone(), revision.revision);
        let original = block
            .world
            .fee_sponsor_programs()
            .get(id)
            .cloned()
            .ok_or("original fee sponsor program missing")?;
        if original.staged_revision.is_some()
            || original.scheduled_activation.is_some()
            || block
                .world
                .fee_sponsor_program_revisions()
                .get(&key)
                .is_some()
        {
            return Err("requested revision is already staged, scheduled or installed".into());
        }
        let selection = block.world.runtime_selection();
        let selected_id = selection
            .id()
            .map_err(|error| error.to_string())?
            .to_string();
        let binding = |wire_id: &str| {
            selection
                .bindings
                .binary_search_by(|b| b.wire_id.as_str().cmp(wire_id))
                .ok()
                .map(|i| selection.bindings[i].clone())
        };
        let retained_binding =
            binding(&self.retained.wire_id).ok_or("retained instruction is absent")?;
        let added_binding = binding(&self.added.wire_id);
        if added_binding.is_some() != after {
            return Err("actual selected State has the wrong added binding for this phase".into());
        }
        let mut tx = block
            .try_transaction()
            .map_err(|error| format!("sponsor overlay refused: {error:?}"))?;
        if tx.network_id != self.network_id || crate::executor::is_initial_genesis_context(&tx) {
            return Err(
                "sponsor check requires the selected network and non-genesis authority checks"
                    .into(),
            );
        }
        let result = stage.clone().execute(&self.authority, &mut tx);
        let expected = format!(
            "fee sponsor rule `{}` references unknown instruction wire id `{}`",
            self.added.id, self.added.wire_id
        );
        let refusal = match (after, result) {
            (
                false,
                Err(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(reason),
                )),
            ) if reason == expected => Some(reason),
            (true, Ok(())) => None,
            (_, other) => return Err(format!("unexpected native sponsor stage result: {other:?}")),
        };
        let mut expected_program = original.clone();
        if after {
            expected_program.staged_revision = Some(revision.revision);
        }
        if tx.world.fee_sponsor_programs.get(id) != Some(&expected_program)
            || tx.world.fee_sponsor_program_revisions.get(&key) != after.then_some(revision)
        {
            return Err(
                "native sponsor stage did not leave the exact expected program and revision".into(),
            );
        }
        drop(tx);
        if block.world.fee_sponsor_programs().get(id) != Some(&original)
            || block
                .world
                .fee_sponsor_program_revisions()
                .get(&key)
                .is_some()
        {
            return Err("discarded sponsor stage changed its parent".into());
        }
        Ok(Observation {
            selection: selected_id,
            retained_binding,
            added_binding,
            active_revision: original.active_revision,
            proposal_revision: revision.revision,
            staged_in_transaction: after,
            refusal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        smartcontracts::isi::sorafs_bootstrap::tests::{Fixture, account},
        state::World,
    };
    use iroha_data_model::{
        Registrable as _,
        asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetDefinitionId},
        domain::Domain,
        nexus::{
            FeeSponsorAssetBudget, FeeSponsorEligibility, FeeSponsorNativeInstructionSelector,
            FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramRevision, FeeSponsorRule,
        },
        privacy::PrivacyPoolIdV1,
    };
    use iroha_model_base::domain::DomainId;
    use iroha_primitives::numeric::Quantity;

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("privacy", "universal").unwrap(),
            "sponsor_rehearsal".parse().unwrap(),
        )
    }

    fn revision() -> FeeSponsorProgramRevision {
        FeeSponsorProgramRevision {
            program_id: FeeSponsorProgramId::new(account(1), "rehearsal".parse().unwrap()),
            revision: 6,
            eligibility: FeeSponsorEligibility::EnrolledOnly,
            rules: [
                ("scheduled_registration", "iroha.register"),
                ("privacy_proofs", "iroha.privacy.submit_ironwood_proof.v1"),
            ]
            .into_iter()
            .map(|(id, wire_id)| FeeSponsorRule {
                id: id.parse().unwrap(),
                effect: FeeSponsorRuleEffect::Allow,
                selectors: vec![FeeSponsorRuleSelector::NativeInstruction(
                    FeeSponsorNativeInstructionSelector {
                        wire_id: wire_id.into(),
                        asset_definition_id: None,
                    },
                )],
            })
            .collect(),
            asset_budgets: vec![FeeSponsorAssetBudget {
                asset_definition_id: asset(),
                per_transaction: Quantity::from(1_u32),
                per_block: Quantity::from(10_u32),
                per_program_epoch: Quantity::from(100_u32),
                per_beneficiary_epoch: Quantity::from(5_u32),
                reserve_floor: Quantity::zero(),
                epoch_length_blocks: NonZeroU64::new(100).unwrap(),
            }],
        }
    }

    fn input(network: NetworkId, revision: FeeSponsorProgramRevision) -> norito::json::Value {
        let stage: InstructionBox = StageFeeSponsorProgramRevision { revision }.into();
        norito::json!({
            "network_id": (network),
            "authority": (account(1)),
            "stage": (stage),
            "retained": {"id": "scheduled_registration", "wire_id": "iroha.register"},
            "added": {"id": "privacy_proofs", "wire_id": "iroha.privacy.submit_ironwood_proof.v1"}
        })
    }

    fn bytes(value: &norito::json::Value) -> Vec<u8> {
        norito::json::to_vec(value).unwrap()
    }

    fn fixture() -> Fixture {
        let domain =
            Domain::new(DomainId::try_new("privacy", "universal").unwrap()).build(&account(1));
        let definition = AssetDefinition::numeric(
            asset(),
            "Sponsor rehearsal fixture",
            AssetBalancePolicy::Global,
            None,
        )
        .build(&account(1));
        let mut world = World::with([domain], [], [definition]);
        let id = revision().program_id;
        world
            .fee_sponsor_programs
            .insert(id.clone(), FeeSponsorProgram::new(id, account(1)));
        let mut fixture = Fixture::with_world(world);
        let payload = fixture.payload();
        let certificate = fixture.certificate(&payload);
        assert_eq!(fixture.submit(certificate), None);
        fixture
    }

    fn pool() -> PrivacyIronwoodPoolBootstrapV1 {
        PrivacyIronwoodPoolBootstrapV1::new(
            PrivacyPoolIdV1::new([0x41; 32]),
            asset(),
            AssetBalanceScope::Global,
            account(1),
        )
        .unwrap()
    }

    #[test]
    fn stage_input_requires_one_owned_exact_unfiltered_allow_pair() {
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([4; 32]),
        ));
        let valid = input(network, revision());
        let parsed = Input::parse(&bytes(&valid)).unwrap();
        assert_eq!(parsed.stage().unwrap().revision(), &revision());
        assert!(Input::parse(&[]).is_err());
        assert!(Input::parse(&vec![b' '; 65_537]).is_err());
        assert!(Input::parse(b"{").is_err());
        for (field, value) in [
            ("authority", norito::json::to_value(&account(2)).unwrap()),
            ("extra", norito::json::Value::Bool(true)),
            (
                "stage",
                norito::json::to_value(&InstructionBox::from(Register::account(
                    iroha_data_model::account::Account::new(account(1)),
                )))
                .unwrap(),
            ),
        ] {
            let mut changed = valid.clone();
            changed.as_object_mut().unwrap().insert(field.into(), value);
            assert!(Input::parse(&bytes(&changed)).is_err(), "{field}");
        }
        for mutation in 0..5 {
            let mut changed = revision();
            match mutation {
                0 => changed.rules[1].effect = FeeSponsorRuleEffect::Deny,
                1 => changed.rules.push(changed.rules[1].clone()),
                2 => {
                    let extra = changed.rules[0].selectors[0].clone();
                    changed.rules[1].selectors.push(extra);
                }
                3 => {
                    let FeeSponsorRuleSelector::NativeInstruction(selector) =
                        &mut changed.rules[1].selectors[0]
                    else {
                        unreachable!()
                    };
                    selector.asset_definition_id = Some(asset());
                }
                _ => {
                    changed.rules.pop();
                }
            }
            assert!(Input::parse(&bytes(&input(network, changed))).is_err());
        }
    }

    #[test]
    fn native_stage_observes_both_selections_and_discards_the_exact_revision() {
        let fixture = fixture();
        let state = fixture.chain.state();
        let before = state.verify_world_state_accumulator().unwrap();
        let tip = state.view().native_execution_tip();
        let request = bytes(&input(fixture.chain.network_id(), revision()));
        let report = state
            .rehearse_taira_activation_with_sponsor_stage(pool(), Some(&request))
            .expect("actual Stage on both sides of the real discarded activation");
        assert!(report.activated, "{}", report.status);
        assert!(!report.state_published);
        let comparison = report.sponsor_stage.unwrap();
        assert_eq!(comparison.predecessor.selection, report.predecessor);
        assert_eq!(comparison.successor.selection, report.successor);
        assert_eq!(
            comparison.predecessor.retained_binding.wire_id,
            "iroha.register"
        );
        assert_eq!(
            comparison.successor.retained_binding.wire_id,
            "iroha.register"
        );
        assert!(comparison.predecessor.added_binding.is_none());
        assert_eq!(
            comparison.successor.added_binding.unwrap().wire_id,
            "iroha.privacy.submit_ironwood_proof.v1"
        );
        assert_eq!(
            comparison.predecessor.refusal.as_deref(),
            Some(
                "fee sponsor rule `privacy_proofs` references unknown instruction wire id `iroha.privacy.submit_ironwood_proof.v1`"
            )
        );
        assert_eq!(comparison.successor.refusal, None);
        assert!(!comparison.predecessor.staged_in_transaction);
        assert!(comparison.successor.staged_in_transaction);
        assert_eq!(comparison.successor.proposal_revision, 6);
        assert_eq!(report.committed_world_before, report.committed_world_after);
        assert_eq!(state.verify_world_state_accumulator().unwrap(), before);
        assert_eq!(state.view().native_execution_tip(), tip);
        let view = state.view();
        let key = FeeSponsorProgramRevisionKey::new(revision().program_id, 6);
        assert!(
            view.world()
                .fee_sponsor_program_revisions()
                .get(&key)
                .is_none()
        );
        assert_eq!(
            view.world()
                .fee_sponsor_programs()
                .get(&key.program_id)
                .unwrap()
                .staged_revision,
            None
        );
    }

    #[test]
    fn sponsor_rehearsal_refuses_wrong_network_and_unrelated_native_failure_without_publication() {
        let fixture = fixture();
        let state = fixture.chain.state();
        let before = state.verify_world_state_accumulator().unwrap();
        let tip = state.view().native_execution_tip();
        let foreign = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xF1; 32]),
        ));
        let error = state
            .rehearse_taira_activation_with_sponsor_stage(
                pool(),
                Some(&bytes(&input(foreign, revision()))),
            )
            .unwrap_err();
        assert!(error.contains("selected network"), "{error}");
        let mut invalid = revision();
        invalid.revision = 0;
        let error = state
            .rehearse_taira_activation_with_sponsor_stage(
                pool(),
                Some(&bytes(&input(fixture.chain.network_id(), invalid))),
            )
            .unwrap_err();
        assert!(
            error.contains("unexpected native sponsor stage result"),
            "{error}"
        );
        assert!(error.contains("revision must be nonzero"), "{error}");
        assert_eq!(state.verify_world_state_accumulator().unwrap(), before);
        assert_eq!(state.view().native_execution_tip(), tip);
    }
}
