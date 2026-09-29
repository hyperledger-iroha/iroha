//! Lane routing (`specs/sumeragi_lanes.md` §5.1): the lane a transaction belongs to at a global
//! height, from committed state only.
//!
//! The policy's explicit routes come first and send a matching transaction to their lane (lane
//! `0` or a fixed lane) when that lane is admitted at the height. Every other transaction takes
//! the default route: lane `0` (sequenced by the global chain itself) and the admitted elastic
//! lanes, sharded by `H(authority)` so that one account's transactions stay in one lane while the
//! lane set is unchanged. The global chain re-evaluates the route at merge, so routing is a
//! single authority.

use iroha_crypto::Hash;
use iroha_data_model::{
    nexus::DataSpaceCatalog,
    sumeragi_lanes::{SumeragiLanePolicy, SumeragiLaneState},
};
use iroha_model_base::topology::LaneId;
use norito::codec::Encode as _;

use crate::{
    queue::{TransactionRoutingView, matchers_match_with_world},
    state::{StateReadOnly, WorldReadOnly},
};

/// The lane of the global chain itself.
pub const GLOBAL_LANE: LaneId = LaneId::new(0);

/// Routing inputs taken from committed state.
pub struct RoutingInputs<'a, W> {
    /// The committed lane policy (`None`: the chain has only lane `0`).
    pub policy: Option<&'a SumeragiLanePolicy>,
    /// The committed lane set.
    pub lanes: &'a SumeragiLaneState,
    /// The committed dataspace catalog (for account matchers).
    pub dataspaces: &'a DataSpaceCatalog,
    /// The world state the matchers read.
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

impl<W: WorldReadOnly> RoutingInputs<'_, W> {
    /// Whether `lane` receives transactions at global height `height`: lane `0` always, any
    /// other lane while its record admits blocks anchored at `height - 1`, the tip a block at
    /// `height` is built on (a lane carries nothing before the global chain has applied its
    /// activation height, and nothing anchored from its closing height).
    #[must_use]
    pub fn admitted(&self, lane: LaneId, height: u64) -> bool {
        lane == GLOBAL_LANE
            || self
                .lanes
                .lane(lane)
                .is_some_and(|record| record.admits_anchor(height.saturating_sub(1)))
    }

    /// The sole execution route of a transaction at the next global height.
    /// The global lane owns the universal dataspace; another lane keeps its original record's
    /// dataspace even when the global chain rescues the transaction directly.
    pub fn execution_route(
        &self,
        tx: &dyn TransactionRoutingView,
        height: u64,
    ) -> Option<crate::queue::RoutingDecision> {
        let lane = self.route(tx, height);
        let dataspace = if lane == GLOBAL_LANE {
            iroha_model_base::topology::DataSpaceId::UNIVERSAL
        } else {
            self.lanes.lane(lane)?.dataspace
        };
        Some(crate::queue::RoutingDecision::new(lane, dataspace))
    }

    /// The default-route lanes at `height`: lane `0` and the admitted elastic lanes, ascending.
    #[must_use]
    pub fn shards(&self, height: u64) -> Vec<LaneId> {
        let mut shards = vec![GLOBAL_LANE];
        if let Some(policy) = self.policy {
            shards.extend(
                self.lanes
                    .lanes
                    .iter()
                    .filter(|record| {
                        policy.is_elastic(record.lane)
                            && record.admits_anchor(height.saturating_sub(1))
                    })
                    .map(|record| record.lane),
            );
        }
        shards
    }

    /// The lane of `tx` at global height `height`.
    #[must_use]
    pub fn route(&self, tx: &dyn TransactionRoutingView, height: u64) -> LaneId {
        let Some(policy) = self.policy else {
            return GLOBAL_LANE;
        };
        for route in &policy.routes {
            if matchers_match_with_world(
                route.account.as_deref(),
                route.instruction.as_deref(),
                tx,
                self.dataspaces,
                self.world,
                Some(self.ledger_time_ms),
            ) {
                return if self.admitted(route.lane, height) {
                    route.lane
                } else {
                    GLOBAL_LANE
                };
            }
        }
        let shards = self.shards(height);
        let Some(authority) = tx.authority_opt() else {
            return GLOBAL_LANE;
        };
        shards[default_shard(authority, shards.len())]
    }
}

/// Routing inputs owned for one pass over many transactions: read once from a state view.
#[derive(Clone, Debug)]
pub struct RoutingSnapshot {
    policy: Option<SumeragiLanePolicy>,
    lanes: SumeragiLaneState,
    dataspaces: DataSpaceCatalog,
    ledger_time_ms: u64,
}

impl RoutingSnapshot {
    /// The routing inputs of the committed state `view` (for the height after its tip).
    #[must_use]
    pub fn of(view: &impl StateReadOnly) -> Self {
        Self {
            policy: super::lane_policy(view.world()),
            lanes: view.world().sumeragi_lanes().clone(),
            dataspaces: view.nexus().dataspace_catalog.clone(),
            ledger_time_ms: view
                .latest_block()
                .and_then(|block| u64::try_from(block.header().creation_time().as_millis()).ok())
                .unwrap_or(0),
        }
    }

    /// The committed lane policy.
    #[must_use]
    pub fn policy(&self) -> Option<&SumeragiLanePolicy> {
        self.policy.as_ref()
    }

    /// Whether the chain routes anything off lane `0`.
    #[must_use]
    pub fn has_lanes(&self) -> bool {
        self.policy.is_some() && !self.lanes.lanes.is_empty()
    }

    /// The borrowed inputs over `world`.
    #[must_use]
    pub fn inputs<'a, W>(&'a self, world: &'a W) -> RoutingInputs<'a, W> {
        RoutingInputs {
            policy: self.policy.as_ref(),
            lanes: &self.lanes,
            dataspaces: &self.dataspaces,
            world,
            ledger_time_ms: self.ledger_time_ms,
        }
    }
}

/// The default-route shard of `authority` among `count` shards: `H(authority) mod count` over
/// the first eight digest bytes.
#[must_use]
pub fn default_shard(authority: &iroha_data_model::account::AccountId, count: usize) -> usize {
    let digest: [u8; 32] = Hash::new(authority.encode()).into();
    let mut prefix = [0u8; 8];
    prefix.copy_from_slice(&digest[..8]);
    let count = u64::try_from(count.max(1)).unwrap_or(u64::MAX);
    usize::try_from(u64::from_be_bytes(prefix) % count).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        isi::{InstructionBox, Log},
        parameter::system::SumeragiParameters,
        sumeragi_lanes::{
            SumeragiFixedLane, SumeragiLaneAutoscale, SumeragiLaneFrontier, SumeragiLaneRecord,
            SumeragiLaneRoute,
        },
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
            anchor_freshness: 16,
            merged: SumeragiLaneFrontier::default(),
            merged_at: active_from,
            rescued: 0,
        }
    }

    fn lanes(records: Vec<SumeragiLaneRecord>) -> SumeragiLaneState {
        let mut state = SumeragiLaneState::default();
        for record in records {
            state.upsert(record);
        }
        state
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

    fn policy(routes: Vec<SumeragiLaneRoute>) -> SumeragiLanePolicy {
        SumeragiLanePolicy {
            anchor_freshness: 16,
            max_merge_blocks: 32,
            stall_window: 256,
            lane_params: SumeragiParameters::default(),
            fixed: vec![SumeragiFixedLane {
                lane: LaneId::new(3),
                dataspace: DataSpaceId::new(0),
                committee: Vec::new(),
            }],
            routes,
            autoscale: Some(SumeragiLaneAutoscale {
                min_lane: LaneId::new(16),
                max_lane_exclusive: LaneId::new(32),
                dataspace: DataSpaceId::new(0),
                committee_size: 4,
                per_lane_target_tps: 100,
                window: 8,
                scale_out_permille: 750,
                scale_in_permille: 250,
                cooldown: 8,
            }),
        }
    }

    #[test]
    fn default_route_shards_by_authority_over_admitted_elastic_lanes() {
        let world = World::default();
        let dataspaces = DataSpaceCatalog::default();
        let policy = policy(Vec::new());
        let open = lanes(vec![record(16, 10, None), record(17, 10, None)]);
        let transactions = (1u8..41).map(tx).collect::<Vec<_>>();
        let view = world.view();
        let inputs = RoutingInputs {
            policy: Some(&policy),
            lanes: &open,
            dataspaces: &dataspaces,
            world: &view,
            ledger_time_ms: 0,
        };
        let routed = transactions
            .iter()
            .map(|tx| inputs.route(tx, 20))
            .collect::<Vec<_>>();
        for lane in [0, 16, 17] {
            assert!(
                routed.contains(&LaneId::new(lane)),
                "lane {lane} receives traffic"
            );
        }
        // The same account always routes to the same lane.
        assert_eq!(
            inputs.route(&transactions[2], 20),
            inputs.route(&transactions[2], 20)
        );
        // Until the global chain has applied the activation height, and from the height after
        // closing, a lane receives nothing.
        assert!(
            transactions
                .iter()
                .all(|tx| inputs.route(tx, 10) == GLOBAL_LANE)
        );
        assert!(
            transactions
                .iter()
                .any(|tx| inputs.route(tx, 11) != GLOBAL_LANE)
        );
        let closing = lanes(vec![record(16, 10, Some(15)), record(17, 10, Some(15))]);
        let inputs = RoutingInputs {
            lanes: &closing,
            ..inputs
        };
        assert!(
            transactions
                .iter()
                .any(|tx| inputs.route(tx, 15) != GLOBAL_LANE)
        );
        assert!(
            transactions
                .iter()
                .all(|tx| inputs.route(tx, 16) == GLOBAL_LANE)
        );
        // Without a policy every transaction belongs to lane 0.
        let inputs = RoutingInputs {
            policy: None,
            lanes: &open,
            ..inputs
        };
        assert!(
            transactions
                .iter()
                .all(|tx| inputs.route(tx, 20) == GLOBAL_LANE)
        );
    }

    #[test]
    fn explicit_routes_target_admitted_lanes_only() {
        let world = World::default();
        let dataspaces = DataSpaceCatalog::default();
        let fixed = lanes(vec![record(3, 5, None)]);
        let transaction = tx(1);
        let view = world.view();
        let with_policy = |policy: &SumeragiLanePolicy, height: u64| {
            RoutingInputs {
                policy: Some(policy),
                lanes: &fixed,
                dataspaces: &dataspaces,
                world: &view,
                ledger_time_ms: 0,
            }
            .route(&transaction, height)
        };
        let to = |lane: u32, instruction: &str| SumeragiLaneRoute {
            lane: LaneId::new(lane),
            account: None,
            instruction: Some(instruction.to_owned()),
        };
        assert_eq!(with_policy(&policy(vec![to(3, "Log")]), 6), LaneId::new(3));
        // A fixed lane that is not admitted at the height falls back to the global lane.
        assert_eq!(with_policy(&policy(vec![to(3, "Log")]), 5), GLOBAL_LANE);
        // A route whose matcher does not match leaves the default route (lane 0 only here).
        assert_eq!(with_policy(&policy(vec![to(3, "Mint")]), 6), GLOBAL_LANE);
    }
    #[test]
    fn execution_route_preserves_actual_pinned_dataspace_and_closing_boundary() {
        let world = World::default();
        let view = world.view();
        let dataspaces = DataSpaceCatalog::default();
        let mut pinned = record(3, 5, Some(8));
        pinned.dataspace = DataSpaceId::new(7);
        let lanes = lanes(vec![pinned]);
        let tx = tx(1);
        let policy = policy(vec![SumeragiLaneRoute {
            lane: LaneId::new(3),
            account: None,
            instruction: Some("Log".into()),
        }]);
        let inputs = RoutingInputs {
            policy: Some(&policy),
            lanes: &lanes,
            dataspaces: &dataspaces,
            world: &view,
            ledger_time_ms: 0,
        };
        assert_eq!(
            inputs.execution_route(&tx, 6),
            Some(crate::queue::RoutingDecision::new(
                LaneId::new(3),
                DataSpaceId::new(7)
            ))
        );
        assert_eq!(
            inputs.execution_route(&tx, 8),
            Some(crate::queue::RoutingDecision::new(
                LaneId::new(3),
                DataSpaceId::new(7)
            ))
        );
        assert_eq!(
            inputs.execution_route(&tx, 9),
            Some(crate::queue::RoutingDecision::new(
                GLOBAL_LANE,
                DataSpaceId::UNIVERSAL
            ))
        );
    }
}
