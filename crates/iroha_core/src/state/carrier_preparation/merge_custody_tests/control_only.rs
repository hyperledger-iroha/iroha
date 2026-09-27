//! Actual drain-certificate custody without an autonomous execution prefix.

use super::*;

#[test]
fn control_only_merge_moves_exact_certificate_after_metadata_world_and_events() {
    on_stack(|| {
        let (state, entry, carrier, topology) =
            crate::state::tests::unpersisted_control_only_merge_custody_fixture();
        assert!(entry.execution_batch.is_none());
        assert!(entry.lane_snapshots.is_empty());
        assert_eq!(entry.lane_drain_certificates.len(), 1);
        let before = state.committed_height();
        let (valid, staged) = recorded(&state, &entry, carrier);
        assert!(staged.merge_prefix_seal().is_none());
        assert!(staged.canonical_wsv_merge_commit_authorization.is_none());
        assert!(
            staged
                .exec_witness
                .as_ref()
                .unwrap()
                .fastpq_transcripts
                .is_empty()
        );
        let inventory = Arc::clone(
            staged
                .fastpq_source_inventory
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap(),
        );
        let (mut preparation, _, _) = PrefixPreparation::capture(staged, &valid, None).unwrap();
        assert!(preparation.prefix.retains_closed_state(&preparation.state));
        assert!(preparation.prefix.sources().merge_prefix().is_none());
        assert!(preparation.prefix.merge_entry().is_none());
        assert!(
            preparation.finish_merge_source(valid.as_ref()).is_err(),
            "certificate capture alone cannot skip the deterministic tail"
        );
        preparation
            .state
            .prepare_deterministic_carrier_metadata(
                valid.as_ref(),
                topology,
                ApplyTopologyAuthority::V2Finality,
            )
            .unwrap();
        let _world = preparation.prepare_world_effects().unwrap();
        let events = preparation
            .state
            .prepare_carrier_publication_events(valid.as_ref().header())
            .unwrap();
        assert!(!events.is_empty());
        preparation.finish_merge_source(valid.as_ref()).unwrap();
        assert!(preparation.prefix.retains_closed_state(&preparation.state));
        assert!(Arc::ptr_eq(preparation.prefix.inventory(), &inventory));
        assert_eq!(preparation.prefix.merge_entry(), Some(&entry));
        assert!(preparation.prefix.sources().merge_prefix().is_none());
        assert!(preparation.state.staged_merge_entry.is_none());
        assert!(
            preparation
                .state
                .canonical_wsv_merge_commit_authorization
                .is_none()
        );
        let PrefixSourceAuthority::Merge(custody) = &preparation.prefix.authority else {
            panic!("exact certificate moves into completed private custody")
        };
        assert!(custody.retains_carrier(valid.as_ref(), preparation.prefix.sources()));
        let mut foreign = valid.as_ref().clone();
        foreign.set_execution_context(None);
        assert!(!custody.retains_carrier(&foreign, preparation.prefix.sources()));
        let PrefixPreparation {
            state: staged,
            prefix,
        } = preparation;
        assert!(
            staged.commit().is_err(),
            "private custody grants no raw State publication authority"
        );
        assert_eq!(prefix.merge_entry(), Some(&entry));
        assert_eq!(state.committed_height(), before);
    });
}
