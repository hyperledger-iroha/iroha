//! Genuine lane admission, native quorum and global execution controls.
use super::*;
use crate::kura::scaling_evidence::fixture::producer::{LaneProducer, resign_lane};
use iroha_core::state::StateReadOnly as _;
#[test]
fn actual_lane_execution_and_quorum_feed_original_global_worker_merge() {
    for lanes in [1, 4] {
        let mut fixture = LaneProducer::start(lanes);
        let mut owner = fixture.proof_owner();
        let mut frames = Vec::new();
        let direct = fixture.request(0, &format!("{:064x}", 1));
        for lane in 1..lanes {
            frames.push(fixture.certify(
                lane,
                vec![fixture.request(lane, &format!("{:064x}", lane + 1))],
            ));
        }
        assert_eq!(fixture.chain.commit(vec![direct]), vec![true]);
        fixture.capture();
        let (block, state) = fixture.prefix.last().unwrap();
        let sources = owner
            .verify(
                block,
                state,
                &frames,
                &fixture.policy,
                fixture.chain.network_id(),
                fixture.chain.state().view().chain_id(),
            )
            .unwrap();
        assert_eq!(sources.len(), lanes - 1);
        for (index, source) in sources {
            assert_eq!(source.source.height, 1);
            assert_eq!(source.source.anchor_height, 3);
            assert_eq!(source.source.batch_index, 0);
            assert_ne!(source.source.block_hash, [0; 32]);
            assert!(
                block
                    .network_output_at(u32::try_from(index).unwrap())
                    .unwrap()
                    .1
                    .result
                    .is_ok()
            );
            let context = block
                .execution_context()
                .unwrap()
                .external
                .iter()
                .find(|context| {
                    context.entrypoint_hash == block.network_entrypoint_at(index).unwrap().hash()
                })
                .unwrap();
            assert_eq!(
                context,
                &iroha_data_model::block::ExternalExecutionContext::new(
                    context.entrypoint_hash,
                    source.lane,
                    source.dataspace
                )
            );
        }
    }
}

#[test]
fn original_lane_evidence_rejects_crypto_subject_parent_result_and_order_changes() {
    let mut fixture = LaneProducer::start(4);
    let frames = (1..4)
        .map(|lane| fixture.certify(lane, vec![fixture.request(lane, &format!("{lane:064x}"))]))
        .collect::<Vec<_>>();
    fixture.chain.commit(Vec::new());
    fixture.capture();
    let (block, state) = fixture.prefix.last().unwrap();
    for control in 0..9 {
        let mut changed = frames.clone();
        if control < 6 {
            let mut entry = decode_frame(&changed[0].frame).unwrap();
            match control {
                0 => entry.commit_qc.agg_sig.0[0] ^= 1,
                1 => {
                    entry.block.header.parent_result = Hash32([0xFF; 32]);
                    resign_lane(&mut entry, &fixture.keys);
                }
                2 => {
                    entry.commit_qc.result = Hash32([0xFF; 32]);
                    resign_lane(&mut entry, &fixture.keys);
                }
                3 => {
                    entry.block.header.instance = Hash32([0xFF; 32]);
                    resign_lane(&mut entry, &fixture.keys);
                }
                4 => {
                    entry.block.header.epoch.epoch += 1;
                    resign_lane(&mut entry, &fixture.keys);
                }
                5 => {
                    entry.block.payload.push(0);
                    resign_lane(&mut entry, &fixture.keys);
                }
                _ => unreachable!(),
            }
            changed[0].frame = entry.encode();
        } else {
            match control {
                6 => {
                    changed.pop();
                }
                7 => changed.swap(0, 1),
                8 => changed.push(frames[0].clone()),
                _ => unreachable!(),
            }
        }
        let mut owner = LaneProofState::default();
        owner
            .anchor_genesis(&fixture.prefix[0].0, fixture.prefix[0].1.clone())
            .unwrap();
        for (prefix, lanes) in &fixture.prefix[1..3] {
            owner
                .verify(
                    prefix,
                    lanes,
                    &[],
                    &fixture.policy,
                    fixture.chain.network_id(),
                    fixture.chain.state().view().chain_id(),
                )
                .unwrap();
        }
        assert!(
            owner
                .verify(
                    block,
                    state,
                    &changed,
                    &fixture.policy,
                    fixture.chain.network_id(),
                    fixture.chain.state().view().chain_id()
                )
                .is_err(),
            "control {control}"
        );
    }
}
