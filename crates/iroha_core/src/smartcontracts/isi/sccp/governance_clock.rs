//! SCCP's items of the one block-start pass: the rank-three items of G2a and the same-pass
//! emergency enactments of G3 (`specs/sccp.md` §4.7.2).
//!
//! [`SccpPassOwnerV1`] lists them into the cursor the Parliament owner begins
//! (`parliament_iface`), so they share its budget and positions with every other item. It
//! holds no budget or position of its own.

use core::marker::PhantomData;

use iroha_data_model::{
    bridge::SccpNetworkV1, governance::GovernanceAnchorV1, sccp::fast_pause::SccpPanelRoleV1,
};
use mv::storage::StorageReadOnly as _;

use super::{
    fast_pause::certification::{self, CertifiedPause},
    keepalive::DueWorkUnavailable,
    parliament_iface::{ForeignPassOwnerV1, GovernancePassItemV1},
    reconciliation_store, store,
};
use crate::state::{StateBlock, StateBlockStartError, WorldReadOnly as _};

/// At most two panels for each of the three external networks.
const PANELS: usize = 6;

/// SCCP's owner of its items in one block-start pass.
pub(crate) struct SccpPassOwnerV1<E> {
    /// Panels this pass certified in G2a, awaiting their enactment in G3 of the same pass.
    /// No certificate survives its pass.
    certified: [Option<((SccpNetworkV1, SccpPanelRoleV1), CertifiedPause)>; PANELS],
    stage: PhantomData<fn() -> E>,
}

impl<E> Default for SccpPassOwnerV1<E> {
    fn default() -> Self {
        Self {
            certified: std::array::from_fn(|_| None),
            stage: PhantomData,
        }
    }
}

/// An item this owner does not own, or an occurrence that left its source.
fn unavailable<T, E: std::fmt::Debug>() -> Result<T, StateBlockStartError<E>> {
    Err(StateBlockStartError::GovernanceDueUnavailable(
        DueWorkUnavailable,
    ))
}

impl<E: std::fmt::Debug> ForeignPassOwnerV1 for SccpPassOwnerV1<E> {
    type Error = StateBlockStartError<E>;

    fn has_due_work(&self, state: &StateBlock<'_>) -> bool {
        store::fast_pause_rounds::iter(&state.world).any(|(_, round)| round.certify_due)
            || reconciliation_store::earliest_due(&state.world, state._curr_block.creation_time_ms)
                .is_some()
    }

    /// Rank three: a pause conclusion is due at the block time, and a reconciliation end
    /// keeps its original `closes_at_ms`, so overdue windows precede newly due conclusions.
    fn g2a_items(
        &mut self,
        state: &StateBlock<'_>,
        limit: usize,
    ) -> Result<Vec<GovernancePassItemV1>, Self::Error> {
        let time = state._curr_block.creation_time_ms;
        let mut items = Vec::new();
        for ((network, role), round) in store::fast_pause_rounds::iter(&state.world) {
            if round.certify_due {
                if items.len() == PANELS {
                    return Err(StateBlockStartError::Policy(
                        "SCCP panel round key bound exceeded".into(),
                    ));
                }
                items.push(GovernancePassItemV1::PauseConclusion {
                    due_ms: time,
                    network: *network,
                    role: *role,
                });
            }
        }
        // The ordered deadline index: only the due prefix is read, never the records.
        items.extend(
            state
                .world
                .sccp_reconciliation_due()
                .iter()
                .take_while(|(key, ())| key.0 <= time)
                .take(limit)
                .map(
                    |(&(due_ms, network, revision), ())| GovernancePassItemV1::Reconciliation {
                        due_ms,
                        network,
                        revision,
                    },
                ),
        );
        Ok(items)
    }

    fn g3_items(&mut self, _: &StateBlock<'_>) -> Result<Vec<GovernancePassItemV1>, Self::Error> {
        Ok(self
            .certified
            .iter()
            .flatten()
            .map(|((network, role), _)| GovernancePassItemV1::EnactPause {
                network: *network,
                role: *role,
            })
            .collect())
    }

    fn is_live(
        &self,
        state: &StateBlock<'_>,
        item: GovernancePassItemV1,
    ) -> Result<bool, Self::Error> {
        match item {
            GovernancePassItemV1::PauseConclusion { network, role, .. } => Ok(
                store::fast_pause_rounds::get(&state.world, &(network, role))
                    .is_some_and(|round| round.certify_due),
            ),
            GovernancePassItemV1::Reconciliation {
                due_ms,
                network,
                revision,
            } => Ok(state
                .world
                .sccp_reconciliation_due()
                .get(&(due_ms, network, revision))
                .is_some()),
            GovernancePassItemV1::EnactPause { network, role } => Ok(self
                .certified
                .iter()
                .flatten()
                .any(|(key, _)| *key == (network, role))),
            _ => unavailable(),
        }
    }

    fn apply(
        &mut self,
        state: &mut StateBlock<'_>,
        item: GovernancePassItemV1,
        at: GovernanceAnchorV1,
    ) -> Result<(), Self::Error> {
        match item {
            GovernancePassItemV1::PauseConclusion { network, role, .. } => {
                if let Some(certified) = certification::certify(state, network, role)? {
                    let Some(slot) = self.certified.iter_mut().find(|slot| slot.is_none()) else {
                        return Err(StateBlockStartError::Policy(
                            "SCCP panel round key bound exceeded".into(),
                        ));
                    };
                    *slot = Some(((network, role), certified));
                }
            }
            GovernancePassItemV1::Reconciliation {
                network, revision, ..
            } => {
                let mut tx = state
                    .try_transaction()
                    .map_err(StateBlockStartError::Storage)?;
                let result = super::reconciliation::end_one(&mut tx, network, revision);
                tx.require_storage_admission()
                    .map_err(StateBlockStartError::Storage)?;
                match result {
                    Ok(true) => tx.apply(),
                    Ok(false) => return unavailable(),
                    Err(error) => return Err(StateBlockStartError::Policy(error.to_string())),
                }
            }
            GovernancePassItemV1::EnactPause { network, role } => {
                let Some((_, certified)) = self
                    .certified
                    .iter_mut()
                    .find(|slot| {
                        slot.as_ref()
                            .is_some_and(|(key, _)| *key == (network, role))
                    })
                    .and_then(Option::take)
                else {
                    return unavailable();
                };
                certification::enact(state, certified, at.position)?;
            }
            _ => return unavailable(),
        }
        state
            .require_storage_admission()
            .map_err(StateBlockStartError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::{
        parliament_iface::{NoParliamentV1, run_governance_pass},
        test_support::{blank_state, header},
    };

    type Owner = SccpPassOwnerV1<()>;

    #[test]
    fn an_idle_chain_lists_no_sccp_item_and_begins_no_cursor() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut owner = Owner::default();
        assert!(!owner.has_due_work(&block));
        assert!(owner.g2a_items(&block, 8).unwrap().is_empty());
        assert!(owner.g3_items(&block).unwrap().is_empty());
        let mut reached = false;
        let report = run_governance_pass(&mut block, None::<NoParliamentV1>, &mut owner, |_| {
            reached = true;
            Ok(())
        })
        .unwrap();
        assert!(
            reached,
            "the step before the enactments runs on every block"
        );
        assert!(!report.governance_due_applied());
        assert!(report.rows.rows().is_empty());
    }

    #[test]
    fn an_item_of_another_owner_or_a_lost_occurrence_is_never_applied() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut owner = Owner::default();
        let network = SccpNetworkV1::EthereumMainnet;
        let role = SccpPanelRoleV1::Primary;
        let at = GovernanceAnchorV1 {
            height: 2,
            position: 0,
            timestamp_ms: block._curr_block.creation_time_ms,
        };
        // Occurrences that are not in their source lapse.
        for item in [
            GovernancePassItemV1::PauseConclusion {
                due_ms: 0,
                network,
                role,
            },
            GovernancePassItemV1::Reconciliation {
                due_ms: 0,
                network,
                revision: 1,
            },
            GovernancePassItemV1::EnactPause { network, role },
        ] {
            assert!(!owner.is_live(&block, item).unwrap());
        }
        // No certificate of this pass exists: nothing is enacted.
        assert!(matches!(
            owner.apply(
                &mut block,
                GovernancePassItemV1::EnactPause { network, role },
                at
            ),
            Err(StateBlockStartError::GovernanceDueUnavailable(_))
        ));
        // Parliament's items never reach SCCP.
        let foreign = GovernancePassItemV1::HeightProgress;
        assert!(matches!(
            owner.is_live(&block, foreign),
            Err(StateBlockStartError::GovernanceDueUnavailable(_))
        ));
        assert!(matches!(
            owner.apply(&mut block, foreign, at),
            Err(StateBlockStartError::GovernanceDueUnavailable(_))
        ));
    }

    #[test]
    fn sccp_rank_three_union_orders_due_time_network_kind_then_role_or_revision() {
        // The cursor's order key is the union key: time, rank three, network, kind (a pause
        // conclusion before a reconciliation end), then role or revision.
        use crate::governance::parliament_anonymous::pass::GovernancePassSegmentV1;
        let state = blank_state();
        let mut header = header(2);
        header.creation_time_ms = 10_000;
        let mut block = state.block(header);
        let time = block._curr_block.creation_time_ms;
        let network = SccpNetworkV1::EthereumMainnet;
        let primary = GovernancePassItemV1::PauseConclusion {
            due_ms: time,
            network,
            role: SccpPanelRoleV1::Primary,
        };
        let backup = GovernancePassItemV1::PauseConclusion {
            due_ms: time,
            network,
            role: SccpPanelRoleV1::Backup,
        };
        let overdue = GovernancePassItemV1::Reconciliation {
            due_ms: time - 1,
            network,
            revision: 9,
        };
        let now = GovernancePassItemV1::Reconciliation {
            due_ms: time,
            network,
            revision: 1,
        };
        let other = GovernancePassItemV1::PauseConclusion {
            due_ms: time,
            network: SccpNetworkV1::BscMainnet,
            role: SccpPanelRoleV1::Primary,
        };
        let mut seen = Vec::new();
        crate::governance::parliament_anonymous::pass::order_for_test(
            &mut block,
            GovernancePassSegmentV1::G2a,
            vec![other, now, backup, overdue, primary],
            &mut seen,
        );
        let expected = if SccpNetworkV1::EthereumMainnet < SccpNetworkV1::BscMainnet {
            vec![overdue, primary, backup, now, other]
        } else {
            vec![overdue, other, primary, backup, now]
        };
        assert_eq!(seen, expected);
    }
}
