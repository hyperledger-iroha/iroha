//! Actual certified merge custody through capture and the deterministic tail.

use super::*;

fn on_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("merge-carrier-custody".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn recorded<'state>(
    state: &'state State,
    entry: &MergeLedgerEntry,
    mut carrier: SignedBlock,
) -> (ValidBlock, Box<StateBlock<'state>>) {
    let (mut staged, recording) = state
        .block_with_owned_start_stages(
            carrier.header(),
            |block| {
                let recording = crate::exec_witness::begin_exec_witness_capture()
                    .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;
                block.stage_certified_merge_entry(entry, ConsensusMode::Permissioned)?;
                Ok::<_, MergeLedgerCommitError>(recording)
            },
            |_, recording| Ok(recording),
        )
        .unwrap();
    staged
        .execute_and_seal_ordinary_outputs(&mut carrier, None, |block, source, _| {
            block.stage_canonical_carrier_membership(
                Vec::new(),
                source.header().height().try_into().unwrap(),
            )?;
            Ok::<_, MergeLedgerCommitError>(ExecutionOutputSealMetadata {
                committed_fragment_count: block.committed_fragment_count().try_into().unwrap(),
                lane_finality_statements: Vec::new(),
            })
        })
        .unwrap();
    staged
        .finalize_lane_consensus_contexts(&carrier, None)
        .unwrap();
    staged.capture_exec_witness().unwrap();
    drop(recording);
    (ValidBlock::new_unverified_for_tests(carrier), staged)
}

#[test]
fn merge_capture_retains_original_sources_and_moves_certified_authority_after_tail() {
    on_stack(|| {
        for sealed in [false, true] {
            let (state, entry, carrier, topology) =
                crate::state::tests::unpersisted_merge_custody_fixture(sealed);
            let before = state.committed_height();
            let (valid, staged) = recorded(&state, &entry, carrier);
            let original = Arc::clone(staged.merge_prefix_seal().unwrap());
            let inventory = Arc::clone(
                staged
                    .fastpq_source_inventory
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .unwrap(),
            );
            let writes = staged.exec_witness.as_ref().unwrap().writes.as_ptr();
            let (mut preparation, _, _) = PrefixPreparation::capture(staged, &valid, None).unwrap();
            assert!(preparation.prefix.retains_closed_state(&preparation.state));
            assert!(
                preparation.prefix.merge_entry().is_none(),
                "preparing source cannot escape as completed custody"
            );
            assert!(
                preparation.finish_merge_source(valid.as_ref()).is_err(),
                "capture alone cannot skip the metadata tail"
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
            preparation.state.pending_da_commitments = Some(PendingDaCommitmentBundle {
                block_height: valid.as_ref().header().height().get(),
                bundle: iroha_data_model::da::commitment::DaCommitmentBundle::new(Vec::new()),
            });
            let refused = preparation.finish_merge_source(valid.as_ref()).unwrap_err();
            assert!(refused.to_string().contains("incompatible runtime effect"));
            assert!(preparation.state.staged_merge_entry.is_some());
            assert!(
                preparation
                    .state
                    .canonical_wsv_merge_commit_authorization
                    .is_some()
            );
            preparation.state.pending_da_commitments = None;
            preparation.finish_merge_source(valid.as_ref()).unwrap();
            assert!(!events.is_empty());
            assert!(preparation.prefix.retains_closed_state(&preparation.state));
            assert!(Arc::ptr_eq(
                preparation.prefix.sources().merge_prefix().unwrap(),
                &original
            ));
            assert!(Arc::ptr_eq(preparation.prefix.inventory(), &inventory));
            assert_eq!(preparation.prefix.witness().writes.as_ptr(), writes);
            assert_eq!(preparation.prefix.merge_entry(), Some(&entry));
            assert!(preparation.state.staged_merge_entry.is_none());
            assert!(
                preparation
                    .state
                    .canonical_wsv_merge_commit_authorization
                    .is_none()
            );
            let PrefixSourceAuthority::Merge(merge) = &preparation.prefix.authority else {
                panic!("completed original custody")
            };
            assert!(merge.retains_carrier(valid.as_ref(), preparation.prefix.sources()));
            let mut foreign = valid.as_ref().clone();
            foreign.set_execution_context(None);
            assert!(!merge.retains_carrier(&foreign, preparation.prefix.sources()));
            let PrefixPreparation {
                state: staged,
                prefix,
            } = preparation;
            assert!(
                staged.commit().is_err(),
                "moving source authority cannot unlock raw State publication"
            );
            assert_eq!(prefix.merge_entry(), Some(&entry));
            drop(prefix);
            assert_eq!(state.committed_height(), before);
        }
    });
}

#[test]
fn merge_capture_rejects_lost_prefix_inventory_and_witness_before_metadata() {
    on_stack(|| {
        for substitution in 0..3 {
            let (state, entry, carrier, _) =
                crate::state::tests::unpersisted_merge_custody_fixture(false);
            let (valid, mut staged) = recorded(&state, &entry, carrier);
            match substitution {
                0 => staged.merge_execution_prefix = None,
                1 => {
                    let original = staged
                        .fastpq_source_inventory
                        .as_ref()
                        .unwrap()
                        .as_ref()
                        .unwrap();
                    staged.fastpq_source_inventory = Some(Ok(Arc::new((**original).clone())));
                }
                2 => staged
                    .exec_witness
                    .as_mut()
                    .unwrap()
                    .fastpq_transcripts
                    .clear(),
                _ => unreachable!(),
            }
            assert!(PrefixPreparation::capture(staged, &valid, None).is_err());
            assert_eq!(
                state.committed_height() + 1,
                valid.as_ref().header().height().get() as usize
            );
        }
    });
}

#[path = "merge_custody_tests/control_only.rs"]
mod control_only;
