//! Lane routing (`specs/sumeragi_lanes.md` §5.1): the lane a transaction belongs to at a global
//! height, from committed state only.
//!
//! Explicit routing rules come first and send a matching transaction to their fixed lane when
//! that lane is admitted at the height. Every other transaction takes the default route: lane `0`
//! (sequenced by the global chain itself) and the admitted elastic lanes, sharded by
//! `H(authority)` so that one account's transactions stay in one lane while the lane set is
//! unchanged. The global chain re-evaluates the route at merge, so routing is a single authority.

use iroha_config::parameters::actual::LaneRoutingPolicy;
use iroha_crypto::Hash;
use iroha_data_model::{nexus::DataSpaceCatalog, sumeragi_lanes::SumeragiLaneRecord};
use iroha_model_base::topology::LaneId;
use norito::codec::Encode as _;

use crate::{
    queue::{TransactionRoutingView, rule_matches_with_world},
    state::WorldReadOnly,
};

/// The lane of the global chain itself.
pub const GLOBAL_LANE: LaneId = LaneId::new(0);

/// Routing inputs taken from committed state.
pub struct RoutingInputs<'a, W> {
    /// The committed routing policy.
    pub policy: &'a LaneRoutingPolicy,
    /// The committed dataspace catalog (for account matchers).
    pub dataspaces: &'a DataSpaceCatalog,
    /// The world state the rules read.
    pub world: &'a W,
    /// Deterministic ledger time of the height (for time-dependent matchers).
    pub ledger_time_ms: u64,
}

impl<W> Clone for RoutingInputs<'_, W> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<W> Copy for RoutingInputs<'_, W> {}

/// The lane of `tx` at global height `height` given the lane records of committed state.
///
/// `elastic` tells which lanes are elastic (autoscaled): they are default-route shards and never
/// explicit-rule targets.
pub fn route<W: WorldReadOnly>(
    inputs: RoutingInputs<'_, W>,
    records: &[&SumeragiLaneRecord],
    elastic: impl Fn(LaneId) -> bool,
    tx: &dyn TransactionRoutingView,
    height: u64,
) -> LaneId {
    let admitted = |lane: LaneId| {
        records
            .iter()
            .any(|record| record.lane == lane && record.admits_anchor(height))
    };
    for rule in &inputs.policy.rules {
        if elastic(rule.lane) {
            continue;
        }
        if rule_matches_with_world(
            rule,
            tx,
            inputs.dataspaces,
            inputs.world,
            Some(inputs.ledger_time_ms),
        ) {
            return if rule.lane == GLOBAL_LANE || admitted(rule.lane) {
                rule.lane
            } else {
                GLOBAL_LANE
            };
        }
    }
    let mut shards = vec![GLOBAL_LANE];
    shards.extend(
        records
            .iter()
            .filter(|record| elastic(record.lane) && record.admits_anchor(height))
            .map(|record| record.lane),
    );
    shards.sort_unstable();
    shards.dedup();
    let Some(authority) = tx.authority_opt() else {
        return GLOBAL_LANE;
    };
    let digest = Hash::new(authority.encode());
    let bytes: [u8; 32] = digest.into();
    let mut prefix = [0u8; 8];
    prefix.copy_from_slice(&bytes[..8]);
    let index = u64::from_be_bytes(prefix) % (shards.len() as u64);
    shards[usize::try_from(index).unwrap_or(0)]
}

#[cfg(test)]
mod tests {
    use iroha_config::parameters::actual::{LaneRoutingMatcher, LaneRoutingRule};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        isi::{InstructionBox, Log},
        parameter::system::SumeragiParameters,
        sumeragi_lanes::SumeragiLaneFrontier,
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use iroha_model_base::topology::DataSpaceId;

    use super::*;
    use crate::{state::World, tx::AcceptedTransaction};

    fn record(lane: u32, active_from: u64, closing: Option<u64>) -> SumeragiLaneRecord {
        SumeragiLaneRecord {
            lane: LaneId::new(lane),
            dataspace: DataSpaceId::new(0),
            incarnation: [u8::try_from(lane).unwrap(); 32],
            params: SumeragiParameters::default(),
            committee: Vec::new(),
            created_at: active_from.saturating_sub(2),
            active_from,
            closing,
            merged: SumeragiLaneFrontier::default(),
        }
    }

    fn tx(seed: u8) -> AcceptedTransaction<'static> {
        let pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let signed = TransactionBuilder::new(
            NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                b"routing-test",
            ))),
            AccountId::new(pair.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "routed".to_owned(),
        ))])
        .sign(pair.private_key());
        AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(signed))
    }

    fn policy(rules: Vec<LaneRoutingRule>) -> LaneRoutingPolicy {
        LaneRoutingPolicy {
            default_lane: GLOBAL_LANE,
            default_dataspace: DataSpaceId::new(0),
            rules,
        }
    }

    #[test]
    fn default_route_shards_by_authority_over_admitted_elastic_lanes() {
        let world = World::default();
        let dataspaces = DataSpaceCatalog::default();
        let policy = policy(Vec::new());
        let inputs = RoutingInputs {
            policy: &policy,
            dataspaces: &dataspaces,
            world: &world.view(),
            ledger_time_ms: 0,
        };
        let elastic = |lane: LaneId| lane.as_u32() >= 16;
        let open = [record(16, 10, None), record(17, 10, None)];
        let refs = open.iter().collect::<Vec<_>>();
        let lanes = (1u8..41)
            .map(|seed| route(inputs, &refs, elastic, &tx(seed), 20))
            .collect::<Vec<_>>();
        for lane in [0, 16, 17] {
            assert!(
                lanes.contains(&LaneId::new(lane)),
                "lane {lane} receives traffic"
            );
        }
        // The same account always routes to the same lane.
        assert_eq!(
            route(inputs, &refs, elastic, &tx(3), 20),
            route(inputs, &refs, elastic, &tx(3), 20)
        );
        // Before activation and from closing, a lane receives nothing.
        assert!((1u8..41).all(|seed| route(inputs, &refs, elastic, &tx(seed), 9) == GLOBAL_LANE));
        let closing = [record(16, 10, Some(15)), record(17, 10, Some(15))];
        let refs = closing.iter().collect::<Vec<_>>();
        assert!((1u8..41).all(|seed| route(inputs, &refs, elastic, &tx(seed), 15) == GLOBAL_LANE));
    }

    #[test]
    fn explicit_rules_target_admitted_fixed_lanes_only() {
        let world = World::default();
        let dataspaces = DataSpaceCatalog::default();
        let rule = |lane: u32| LaneRoutingRule {
            lane: LaneId::new(lane),
            dataspace: None,
            matcher: LaneRoutingMatcher {
                instruction: Some("Log".to_owned()),
                ..LaneRoutingMatcher::default()
            },
        };
        let fixed = [record(3, 5, None)];
        let refs = fixed.iter().collect::<Vec<_>>();
        let elastic = |lane: LaneId| lane.as_u32() >= 16;
        let with_policy = |policy: &LaneRoutingPolicy, height: u64| {
            let inputs = RoutingInputs {
                policy,
                dataspaces: &dataspaces,
                world: &world.view(),
                ledger_time_ms: 0,
            };
            route(inputs, &refs, elastic, &tx(1), height)
        };
        assert_eq!(with_policy(&policy(vec![rule(3)]), 6), LaneId::new(3));
        // A fixed lane that is not admitted at the height falls back to the global lane.
        assert_eq!(with_policy(&policy(vec![rule(3)]), 4), GLOBAL_LANE);
        assert_eq!(with_policy(&policy(vec![rule(4)]), 6), GLOBAL_LANE);
        // Elastic lanes are never explicit targets.
        assert_eq!(with_policy(&policy(vec![rule(16)]), 6), GLOBAL_LANE);
    }
}
