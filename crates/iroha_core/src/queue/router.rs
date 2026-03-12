//! Lane and dataspace routing utilities for the transaction queue.
//!
//! These helpers translate pending transactions into the lane/dataspace
//! identifiers that the Nexus scheduler expects, based on the runtime
//! configuration. The router abstraction keeps the queue decoupled from the
//! exact routing policy while allowing metrics to reflect the real
//! assignments instead of single-lane placeholders.

use std::sync::Arc;

use iroha_config::parameters::actual::{LaneRoutingPolicy, LaneRoutingRule};
use iroha_data_model::{
    isi::{
        BurnBox, GrantBox, Instruction, MintBox, RegisterBox, RemoveKeyValueBox, RevokeBox,
        SetKeyValueBox, TransferBox, UnregisterBox,
        smart_contract_code::{RegisterSmartContractBytes, RegisterSmartContractCode},
    },
    nexus::{DataSpaceCatalog, DataSpaceId, LaneCatalog, LaneId},
    transaction::Executable,
};

use crate::{
    state::{State, StateView},
    tx::AcceptedTransaction,
};

/// Routing decision returned by a [`LaneRouter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutingDecision {
    /// Lane assigned to the transaction.
    pub lane_id: LaneId,
    /// Dataspace assigned to the transaction.
    pub dataspace_id: DataSpaceId,
}

impl RoutingDecision {
    /// Create a new routing decision.
    #[must_use]
    pub const fn new(lane_id: LaneId, dataspace_id: DataSpaceId) -> Self {
        Self {
            lane_id,
            dataspace_id,
        }
    }
}

impl Default for RoutingDecision {
    fn default() -> Self {
        Self::new(LaneId::SINGLE, DataSpaceId::GLOBAL)
    }
}

/// Evaluate the configured routing policy for a transaction, returning the lane and dataspace.
///
/// This does not validate the decision against the lane or dataspace catalogs. Use
/// [`evaluate_policy_with_catalog`] when catalog alignment is required.
pub fn evaluate_policy(
    policy: &LaneRoutingPolicy,
    tx: &AcceptedTransaction<'_>,
) -> RoutingDecision {
    let matched_rule = policy.rules.iter().find(|rule| rule_matches(rule, tx));
    let lane_id = matched_rule.map_or(policy.default_lane, |rule| rule.lane);
    let dataspace_id = matched_rule
        .and_then(|rule| rule.dataspace)
        .unwrap_or(policy.default_dataspace);
    RoutingDecision::new(lane_id, dataspace_id)
}

/// Evaluate the routing policy and align the decision to the configured catalogs.
pub fn evaluate_policy_with_catalog(
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
    tx: &AcceptedTransaction<'_>,
) -> RoutingDecision {
    let decision = evaluate_policy(policy, tx);
    normalize_routing_decision(decision, policy, lane_catalog, dataspace_catalog)
}

fn normalize_routing_decision(
    decision: RoutingDecision,
    policy: &LaneRoutingPolicy,
    lane_catalog: &LaneCatalog,
    dataspace_catalog: &DataSpaceCatalog,
) -> RoutingDecision {
    let lane_id = if lane_catalog
        .lanes()
        .iter()
        .any(|lane| lane.id == decision.lane_id)
    {
        decision.lane_id
    } else if lane_catalog
        .lanes()
        .iter()
        .any(|lane| lane.id == policy.default_lane)
    {
        policy.default_lane
    } else {
        lane_catalog
            .lanes()
            .iter()
            .min_by_key(|lane| lane.id.as_u32())
            .map_or(policy.default_lane, |lane| lane.id)
    };

    let dataspace_known = |dataspace_id: DataSpaceId| {
        dataspace_catalog
            .entries()
            .iter()
            .any(|entry| entry.id == dataspace_id)
    };
    let fallback_dataspace = dataspace_catalog
        .entries()
        .iter()
        .find(|entry| entry.id == DataSpaceId::GLOBAL)
        .map(|entry| entry.id)
        .or_else(|| {
            dataspace_catalog
                .entries()
                .iter()
                .min_by_key(|entry| entry.id.as_u64())
                .map(|entry| entry.id)
        })
        .unwrap_or(policy.default_dataspace);
    let default_dataspace = if dataspace_known(policy.default_dataspace) {
        policy.default_dataspace
    } else {
        fallback_dataspace
    };

    let dataspace_id = if dataspace_known(decision.dataspace_id) {
        decision.dataspace_id
    } else {
        default_dataspace
    };

    let lane_dataspace = lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == lane_id)
        .map(|lane| lane.dataspace_id)
        .filter(|dataspace_id| dataspace_known(*dataspace_id))
        .unwrap_or(default_dataspace);

    let dataspace_id = if dataspace_id == lane_dataspace {
        dataspace_id
    } else {
        lane_dataspace
    };

    RoutingDecision::new(lane_id, dataspace_id)
}

fn rule_matches(rule: &LaneRoutingRule, tx: &AcceptedTransaction<'_>) -> bool {
    let matcher = &rule.matcher;

    if let Some(account) = matcher.account.as_deref()
        && !account_matches(account, tx.as_ref().authority())
    {
        return false;
    }

    if let Some(instruction) = matcher.instruction.as_deref()
        && !instructions_match(instruction, tx)
    {
        return false;
    }

    true
}

fn account_matches(pattern: &str, authority: &iroha_data_model::account::AccountId) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }

    if authority.to_string() == pattern {
        return true;
    }

    let wildcard_domain = pattern
        .strip_prefix("*@")
        .or_else(|| pattern.strip_prefix("domain:"))
        .or_else(|| pattern.strip_prefix("authority_domain:"));

    wildcard_domain.is_some_and(|domain| !domain.trim().is_empty() && false)
}

fn instructions_match(matcher: &str, tx: &AcceptedTransaction<'_>) -> bool {
    let matcher_norm = matcher.trim().to_ascii_lowercase();
    if matcher_norm.is_empty() {
        return false;
    }
    let (matcher_label, destination_domain) = split_instruction_matcher(&matcher_norm);
    if matcher_label.is_empty() {
        return false;
    }

    let executable = tx.as_ref().instructions();
    let Executable::Instructions(batch) = executable else {
        return false;
    };

    batch
        .iter()
        .any(|instruction| instruction_matches(matcher_label, destination_domain, &**instruction))
}

fn split_instruction_matcher(matcher: &str) -> (&str, Option<&str>) {
    if let Some((label, domain)) = matcher.rsplit_once('@')
        && label.starts_with("transfer")
    {
        let label = label.trim();
        let domain = domain.trim();
        if !label.is_empty() && !domain.is_empty() {
            return (label, Some(domain));
        }
    }

    (matcher, None)
}

fn instruction_matches(
    matcher: &str,
    destination_domain: Option<&str>,
    instruction: &dyn Instruction,
) -> bool {
    if destination_domain
        .is_some_and(|domain| !transfer_destination_matches_domain(instruction, domain))
    {
        return false;
    }

    if instruction_label_matches(matcher, instruction) {
        return true;
    }

    let id = Instruction::id(instruction).to_ascii_lowercase();
    if matches_label(matcher, &id) {
        return true;
    }

    id.split("::").any(|segment| {
        matches_label(matcher, segment)
            || segment
                .strip_suffix("box")
                .is_some_and(|trimmed| !trimmed.is_empty() && matches_label(matcher, trimmed))
    })
}

fn transfer_destination_matches_domain(instruction: &dyn Instruction, domain: &str) -> bool {
    let domain = domain.trim();
    if domain.is_empty() {
        return false;
    }

    let any = instruction.as_any();
    let Some(transfer) = any.downcast_ref::<TransferBox>() else {
        return false;
    };

    let _ = transfer;
    let _ = domain;
    false
}

fn instruction_label_matches(matcher: &str, instruction: &dyn Instruction) -> bool {
    let any = instruction.as_any();

    if let Some(register) = any.downcast_ref::<RegisterBox>() {
        let variant = match register {
            RegisterBox::Peer(_) => "register::peer",
            RegisterBox::Domain(_) => "register::domain",
            RegisterBox::Account(_) => "register::account",
            RegisterBox::AssetDefinition(_) => "register::asset_definition",
            RegisterBox::Nft(_) => "register::nft",
            RegisterBox::Role(_) => "register::role",
            RegisterBox::Trigger(_) => "register::trigger",
        };
        return matches_box_variant(matcher, "register", variant);
    }

    if let Some(unregister) = any.downcast_ref::<UnregisterBox>() {
        let variant = match unregister {
            UnregisterBox::Peer(_) => "unregister::peer",
            UnregisterBox::Domain(_) => "unregister::domain",
            UnregisterBox::Account(_) => "unregister::account",
            UnregisterBox::AssetDefinition(_) => "unregister::asset_definition",
            UnregisterBox::Nft(_) => "unregister::nft",
            UnregisterBox::Role(_) => "unregister::role",
            UnregisterBox::Trigger(_) => "unregister::trigger",
        };
        return matches_box_variant(matcher, "unregister", variant);
    }

    if let Some(mint) = any.downcast_ref::<MintBox>() {
        let variant = match mint {
            MintBox::Asset(_) => "mint::asset",
            MintBox::TriggerRepetitions(_) => "mint::trigger_repetitions",
        };
        return matches_box_variant(matcher, "mint", variant);
    }

    if let Some(burn) = any.downcast_ref::<BurnBox>() {
        let variant = match burn {
            BurnBox::Asset(_) => "burn::asset",
            BurnBox::TriggerRepetitions(_) => "burn::trigger_repetitions",
        };
        return matches_box_variant(matcher, "burn", variant);
    }

    if let Some(transfer) = any.downcast_ref::<TransferBox>() {
        let variant = match transfer {
            TransferBox::Domain(_) => "transfer::domain",
            TransferBox::AssetDefinition(_) => "transfer::asset_definition",
            TransferBox::Asset(_) => "transfer::asset",
            TransferBox::Nft(_) => "transfer::nft",
        };
        return matches_box_variant(matcher, "transfer", variant);
    }

    if let Some(set_key_value) = any.downcast_ref::<SetKeyValueBox>() {
        let variant = match set_key_value {
            SetKeyValueBox::Domain(_) => "set_key_value::domain",
            SetKeyValueBox::Account(_) => "set_key_value::account",
            SetKeyValueBox::AssetDefinition(_) => "set_key_value::asset_definition",
            SetKeyValueBox::Nft(_) => "set_key_value::nft",
            SetKeyValueBox::Trigger(_) => "set_key_value::trigger",
        };
        return matches_box_variant(matcher, "set_key_value", variant);
    }

    if let Some(remove_key_value) = any.downcast_ref::<RemoveKeyValueBox>() {
        let variant = match remove_key_value {
            RemoveKeyValueBox::Domain(_) => "remove_key_value::domain",
            RemoveKeyValueBox::Account(_) => "remove_key_value::account",
            RemoveKeyValueBox::AssetDefinition(_) => "remove_key_value::asset_definition",
            RemoveKeyValueBox::Nft(_) => "remove_key_value::nft",
            RemoveKeyValueBox::Trigger(_) => "remove_key_value::trigger",
        };
        return matches_box_variant(matcher, "remove_key_value", variant);
    }

    if let Some(grant) = any.downcast_ref::<GrantBox>() {
        let variant = match grant {
            GrantBox::Permission(_) => "grant::permission",
            GrantBox::Role(_) => "grant::role",
            GrantBox::RolePermission(_) => "grant::role_permission",
        };
        return matches_box_variant(matcher, "grant", variant);
    }

    if let Some(revoke) = any.downcast_ref::<RevokeBox>() {
        let variant = match revoke {
            RevokeBox::Permission(_) => "revoke::permission",
            RevokeBox::Role(_) => "revoke::role",
            RevokeBox::RolePermission(_) => "revoke::role_permission",
        };
        return matches_box_variant(matcher, "revoke", variant);
    }

    if any.is::<RegisterSmartContractCode>() || any.is::<RegisterSmartContractBytes>() {
        return matches_label(matcher, "smartcontract::deploy")
            || matches_label(matcher, "smart_contract::deploy");
    }

    false
}

fn matches_box_variant(matcher: &str, base: &str, variant: &str) -> bool {
    matches_label(matcher, base) || matches_label(matcher, variant)
}

fn matches_label(matcher: &str, label: &str) -> bool {
    label == matcher || eq_ignoring_underscores(label, matcher)
}

fn eq_ignoring_underscores(left: &str, right: &str) -> bool {
    let mut left_iter = left.bytes().filter(|byte| *byte != b'_');
    let mut right_iter = right.bytes().filter(|byte| *byte != b'_');
    loop {
        match (left_iter.next(), right_iter.next()) {
            (None, None) => return true,
            (Some(left_byte), Some(right_byte)) if left_byte == right_byte => {}
            _ => return false,
        }
    }
}

/// Strategy object that derives lane/dataspace assignments for queued transactions.
pub trait LaneRouter: Send + Sync + 'static {
    /// Route the given transaction without requiring a state snapshot.
    fn route(&self, tx: &AcceptedTransaction<'_>) -> RoutingDecision;

    /// Route the given transaction using an already acquired state view.
    ///
    /// Routers that require dynamic world-state can override this method and
    /// [`LaneRouter::route_without_state`].
    fn route_with_view(
        &self,
        tx: &AcceptedTransaction<'_>,
        _state_view: &StateView<'_>,
    ) -> RoutingDecision {
        self.route(tx)
    }

    /// Route the given transaction with narrow state access when possible.
    ///
    /// The default implementation prefers [`LaneRouter::route_without_state`]
    /// and only falls back to taking a short-lived [`StateView`] when needed.
    fn route_with_state(&self, tx: &AcceptedTransaction<'_>, state: &State) -> RoutingDecision {
        if let Some(decision) = self.route_without_state(tx) {
            return decision;
        }
        let state_view = state.view();
        self.route_with_view(tx, &state_view)
    }

    /// Route the given transaction without a state snapshot when possible.
    ///
    /// Routers that do not depend on dynamic world-state can override this to
    /// avoid taking a full [`StateView`] in hot requeue paths.
    fn route_without_state(&self, tx: &AcceptedTransaction<'_>) -> Option<RoutingDecision> {
        Some(self.route(tx))
    }
}

/// Trivial router that keeps the single-lane/global-dataspace behaviour.
#[derive(Copy, Clone, Debug, Default)]
pub struct SingleLaneRouter;

impl SingleLaneRouter {
    /// Create a router that always selects the default single lane/global dataspace.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl LaneRouter for SingleLaneRouter {
    fn route(&self, _tx: &AcceptedTransaction<'_>) -> RoutingDecision {
        RoutingDecision::new(LaneId::SINGLE, DataSpaceId::GLOBAL)
    }
}

/// Router that applies the declarative policy derived from configuration.
#[derive(Debug, Clone)]
pub struct ConfigLaneRouter {
    policy: Arc<LaneRoutingPolicy>,
    dataspace_catalog: Arc<DataSpaceCatalog>,
    lane_catalog: Arc<LaneCatalog>,
}

impl ConfigLaneRouter {
    /// Build a router from the validated runtime configuration.
    #[must_use]
    pub fn new(
        policy: LaneRoutingPolicy,
        dataspace_catalog: DataSpaceCatalog,
        lane_catalog: LaneCatalog,
    ) -> Self {
        Self {
            policy: Arc::new(policy),
            dataspace_catalog: Arc::new(dataspace_catalog),
            lane_catalog: Arc::new(lane_catalog),
        }
    }
}

impl LaneRouter for ConfigLaneRouter {
    fn route(&self, tx: &AcceptedTransaction<'_>) -> RoutingDecision {
        let decision = evaluate_policy(&self.policy, tx);
        normalize_routing_decision(
            decision,
            self.policy.as_ref(),
            self.lane_catalog.as_ref(),
            self.dataspace_catalog.as_ref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use iroha_config::parameters::actual::{LaneRoutingMatcher, LaneRoutingRule};
    use iroha_crypto::Hash;
    use iroha_data_model::{
        isi::{
            prelude::{Mint, Register, Transfer},
            smart_contract_code::RegisterSmartContractBytes,
        },
        metadata::Metadata,
        nexus::LaneConfig,
        prelude::*,
        transaction::TransactionBuilder,
    };
    use iroha_test_samples::gen_account_in;
    use nonzero_ext::nonzero;

    use super::*;

    fn sample_transaction(
        authority: &AccountId,
        signer: &iroha_crypto::PrivateKey,
        instructions: Vec<InstructionBox>,
    ) -> AcceptedTransaction<'static> {
        let chain_id = ChainId::from("chain");
        let tx = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_instructions(instructions)
            .with_metadata(Metadata::default())
            .sign(signer);
        let default_limits = TransactionParameters::default();
        let params = TransactionParameters::with_max_signatures(
            nonzero!(16_u64),
            nonzero!(4096_u64),
            nonzero!(4096_u64),
            default_limits.max_tx_bytes(),
            default_limits.max_decompressed_bytes(),
            default_limits.max_metadata_depth(),
        );
        let crypto_cfg = iroha_config::parameters::actual::Crypto::default();
        AcceptedTransaction::accept(
            tx,
            &chain_id,
            core::time::Duration::from_secs(30),
            params,
            &crypto_cfg,
        )
        .expect("tx should be accepted")
    }

    fn catalog_with_lane_dataspaces(entries: &[(LaneId, DataSpaceId)]) -> LaneCatalog {
        let max_lane = entries
            .iter()
            .map(|(lane, _)| lane.as_u32())
            .max()
            .unwrap_or(0);
        let lane_count =
            std::num::NonZeroU32::new(max_lane + 1).expect("catalog requires nonzero lanes");
        let lanes = entries
            .iter()
            .map(|(lane_id, dataspace_id)| LaneConfig {
                id: *lane_id,
                dataspace_id: *dataspace_id,
                alias: format!("lane-{}", lane_id.as_u32()),
                ..LaneConfig::default()
            })
            .collect();
        LaneCatalog::new(lane_count, lanes).expect("valid lane catalog")
    }

    fn catalog_with_lanes(lanes: &[LaneId]) -> LaneCatalog {
        let entries: Vec<(LaneId, DataSpaceId)> = lanes
            .iter()
            .map(|lane_id| (*lane_id, DataSpaceId::GLOBAL))
            .collect();
        catalog_with_lane_dataspaces(&entries)
    }

    fn blank_state() -> crate::state::State {
        let world = crate::state::World::default();
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        #[cfg(feature = "telemetry")]
        let telemetry = crate::telemetry::StateTelemetry::default();
        #[cfg(feature = "telemetry")]
        return crate::state::State::with_telemetry(world, kura, query, telemetry);
        #[cfg(not(feature = "telemetry"))]
        crate::state::State::new(world, kura, query)
    }

    #[test]
    fn applies_account_and_instruction_rules() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let (_bob_id, _) = gen_account_in("wonderland");

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(0),
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![
                LaneRoutingRule {
                    lane: LaneId::new(1),
                    dataspace: None,
                    matcher: LaneRoutingMatcher {
                        account: Some(alice_id.to_string()),
                        instruction: Some("Mint".into()),
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: None,
                    matcher: LaneRoutingMatcher {
                        account: Some(
                            "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp".into(),
                        ),
                        instruction: None,
                        description: None,
                    },
                },
            ],
        };
        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1), LaneId::new(2)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);

        let asset_definition: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "xor".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_definition.clone(), alice_id.clone());
        let mint = Mint::asset_numeric(1u32, asset_id);
        let register = Register::asset_definition(
            AssetDefinition::numeric(asset_definition.clone())
                .with_name(asset_definition.name().to_string()),
        );

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(mint), InstructionBox::from(register)],
        );

        let state = blank_state();
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id.as_u32(), 1);
        assert_eq!(decision.dataspace_id, DataSpaceId::GLOBAL);

        // Non-matching instruction should fall back to default lane.
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "fallback".parse().expect("domain"),
            )))],
        );
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id.as_u32(), 0);
    }

    #[test]
    fn single_lane_router_supports_state_free_routing() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "single".parse().expect("domain"),
            )))],
        );
        let state = blank_state();
        let router = SingleLaneRouter::new();
        let with_view = router.route_with_view(&tx, &state.view());
        let without_view = router.route_without_state(&tx);
        assert_eq!(without_view, Some(with_view));
    }

    #[test]
    fn config_lane_router_state_free_path_matches_view_path() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(3),
                dataspace: Some(DataSpaceId::new(7)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: Some("register::domain".to_string()),
                    description: None,
                },
            }],
        };
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::GLOBAL),
            (LaneId::new(3), DataSpaceId::new(7)),
        ]);
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "alpha".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");
        let router = ConfigLaneRouter::new(policy, dataspace_catalog, lane_catalog);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "statefree".parse().expect("domain"),
            )))],
        );
        let state = blank_state();
        let with_view = router.route_with_view(&tx, &state.view());
        let without_view = router.route_without_state(&tx);
        assert_eq!(without_view, Some(with_view));
    }

    #[test]
    fn rule_dataspace_override_is_used() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(5),
                dataspace: Some(DataSpaceId::new(7)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "alpha".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");

        let policy_for_helper = policy.clone();
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::GLOBAL),
            (LaneId::new(5), DataSpaceId::new(7)),
        ]);
        let router = ConfigLaneRouter::new(policy, catalog, lane_catalog);

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "override".parse().expect("domain"),
            )))],
        );
        let state = blank_state();
        let decision = router.route_with_view(&tx, &state.view());
        assert_eq!(decision.lane_id, LaneId::new(5));
        assert_eq!(decision.dataspace_id, DataSpaceId::new(7));

        let helper_decision = evaluate_policy(&policy_for_helper, &tx);
        assert_eq!(helper_decision, decision);
    }

    #[test]
    fn aligns_dataspace_with_lane_binding_on_mismatch() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(4),
                dataspace: Some(DataSpaceId::new(9)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "beta".to_string(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: DataSpaceId::new(9),
                alias: "gamma".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");

        let policy_for_helper = policy.clone();
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::SINGLE, DataSpaceId::GLOBAL),
            (LaneId::new(4), DataSpaceId::new(7)),
        ]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog.clone());

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "override".parse().expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::new(4));
        assert_eq!(decision.dataspace_id, DataSpaceId::new(7));

        let helper_decision =
            evaluate_policy_with_catalog(&policy_for_helper, &lane_catalog, &catalog, &tx);
        assert_eq!(helper_decision, decision);
    }

    #[test]
    fn unknown_lane_rule_falls_back_to_default_dataspace() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(9),
                dataspace: Some(DataSpaceId::new(7)),
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "alpha".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");

        let policy_for_helper = policy.clone();
        let lane_catalog = catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::GLOBAL)]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog.clone());

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "fallback".parse().expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::SINGLE);
        assert_eq!(decision.dataspace_id, DataSpaceId::GLOBAL);

        let helper_decision =
            evaluate_policy_with_catalog(&policy_for_helper, &lane_catalog, &catalog, &tx);
        assert_eq!(helper_decision, decision);
    }

    #[test]
    fn missing_default_lane_falls_back_to_lowest_lane() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::new(9),
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(11),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "alpha".to_string(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: DataSpaceId::new(9),
                alias: "beta".to_string(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("valid dataspace catalog");

        let policy_for_helper = policy.clone();
        let lane_catalog = catalog_with_lane_dataspaces(&[
            (LaneId::new(2), DataSpaceId::new(7)),
            (LaneId::new(4), DataSpaceId::new(9)),
        ]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog.clone());

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "fallback".parse().expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::new(2));
        assert_eq!(decision.dataspace_id, DataSpaceId::new(7));

        let helper_decision =
            evaluate_policy_with_catalog(&policy_for_helper, &lane_catalog, &catalog, &tx);
        assert_eq!(helper_decision, decision);
    }

    #[test]
    fn missing_default_dataspace_falls_back_to_catalog() {
        use iroha_data_model::nexus::DataSpaceMetadata;

        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::new(11),
            rules: vec![LaneRoutingRule {
                lane: LaneId::SINGLE,
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: Some(alice_id.to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: DataSpaceId::new(7),
            alias: "alpha".to_string(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("valid dataspace catalog");

        let policy_for_helper = policy.clone();
        let lane_catalog = catalog_with_lane_dataspaces(&[(LaneId::SINGLE, DataSpaceId::new(9))]);
        let router = ConfigLaneRouter::new(policy, catalog.clone(), lane_catalog.clone());

        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "fallback".parse().expect("domain"),
            )))],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::SINGLE);
        assert_eq!(decision.dataspace_id, DataSpaceId::new(7));

        let helper_decision =
            evaluate_policy_with_catalog(&policy_for_helper, &lane_catalog, &catalog, &tx);
        assert_eq!(helper_decision, decision);
    }

    #[test]
    fn matches_register_domain_rule() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("register::domain".to_string()),
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "castle".parse().expect("domain id"),
            )))],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn matches_smartcontract_deploy_rule() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("smartcontract::deploy".to_string()),
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let code = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let register = RegisterSmartContractBytes {
            code_hash: Hash::new(&code),
            code,
        };
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(register)],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn matches_set_key_value_rule_without_underscores() {
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("setkeyvalue::account".to_string()),
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);
        let instruction = SetKeyValue::account(
            alice_id.clone(),
            "flag".parse().expect("metadata key"),
            Json::new("on"),
        );
        let tx = sample_transaction(
            &alice_id,
            alice_keypair.private_key(),
            vec![InstructionBox::from(instruction)],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn matches_account_domain_wildcard_rule() {
        let (uae_id, uae_keypair) = gen_account_in("uae");
        let (bank_id, bank_keypair) = gen_account_in("hbl");

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: Some("*@uae".to_string()),
                    instruction: None,
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);

        let uae_tx = sample_transaction(
            &uae_id,
            uae_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "uae-match".parse().expect("domain id"),
            )))],
        );
        let bank_tx = sample_transaction(
            &bank_id,
            bank_keypair.private_key(),
            vec![InstructionBox::from(Register::domain(Domain::new(
                "bank-no-match".parse().expect("domain id"),
            )))],
        );

        let state = blank_state();
        let uae_decision = router.route_with_view(&uae_tx, &state.view());
        let bank_decision = router.route_with_view(&bank_tx, &state.view());

        assert_eq!(uae_decision.lane_id, LaneId::new(1));
        assert_eq!(bank_decision.lane_id, LaneId::SINGLE);
    }

    #[test]
    fn matches_transfer_destination_domain_rule() {
        let (sender_id, sender_keypair) = gen_account_in("hbl");
        let (receiver_id, _) = gen_account_in("acme");

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![LaneRoutingRule {
                lane: LaneId::new(1),
                dataspace: None,
                matcher: LaneRoutingMatcher {
                    account: None,
                    instruction: Some("transfer@acme".to_string()),
                    description: None,
                },
            }],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);

        let asset_definition: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "uae".parse().unwrap(),
            "aed".parse().unwrap(),
        );
        let asset_id = AssetId::of(asset_definition, sender_id.clone());
        let transfer = Transfer::asset_numeric(asset_id, 1_u32, receiver_id);
        let tx = sample_transaction(
            &sender_id,
            sender_keypair.private_key(),
            vec![InstructionBox::from(transfer)],
        );

        let decision = router.route_with_view(&tx, &blank_state().view());
        assert_eq!(decision.lane_id, LaneId::new(1));
    }

    #[test]
    fn account_rule_takes_precedence_over_transfer_destination_rule() {
        let (uae_sender_id, uae_sender_keypair) = gen_account_in("uae");
        let (bank_sender_id, bank_sender_keypair) = gen_account_in("hbl");
        let (acme_receiver_id, _) = gen_account_in("acme");

        let policy = LaneRoutingPolicy {
            default_lane: LaneId::SINGLE,
            default_dataspace: DataSpaceId::GLOBAL,
            rules: vec![
                LaneRoutingRule {
                    lane: LaneId::new(2),
                    dataspace: None,
                    matcher: LaneRoutingMatcher {
                        account: Some("*@uae".to_string()),
                        instruction: Some("transfer".to_string()),
                        description: None,
                    },
                },
                LaneRoutingRule {
                    lane: LaneId::new(1),
                    dataspace: None,
                    matcher: LaneRoutingMatcher {
                        account: None,
                        instruction: Some("transfer@acme".to_string()),
                        description: None,
                    },
                },
            ],
        };

        let lane_catalog = catalog_with_lanes(&[LaneId::SINGLE, LaneId::new(1), LaneId::new(2)]);
        let router = ConfigLaneRouter::new(policy, DataSpaceCatalog::default(), lane_catalog);

        let asset_definition: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "uae".parse().unwrap(),
            "aed".parse().unwrap(),
        );
        let uae_transfer = Transfer::asset_numeric(
            AssetId::of(asset_definition.clone(), uae_sender_id.clone()),
            1_u32,
            acme_receiver_id.clone(),
        );
        let bank_transfer = Transfer::asset_numeric(
            AssetId::of(asset_definition, bank_sender_id.clone()),
            1_u32,
            acme_receiver_id,
        );

        let uae_tx = sample_transaction(
            &uae_sender_id,
            uae_sender_keypair.private_key(),
            vec![InstructionBox::from(uae_transfer)],
        );
        let bank_tx = sample_transaction(
            &bank_sender_id,
            bank_sender_keypair.private_key(),
            vec![InstructionBox::from(bank_transfer)],
        );

        let state = blank_state();
        let uae_decision = router.route_with_view(&uae_tx, &state.view());
        let bank_decision = router.route_with_view(&bank_tx, &state.view());
        assert_eq!(uae_decision.lane_id, LaneId::new(2));
        assert_eq!(bank_decision.lane_id, LaneId::new(1));
    }
}
