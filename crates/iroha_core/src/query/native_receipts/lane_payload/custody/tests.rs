//! Original sparse custody, canonical framing and separate-clock admission regressions.

use iroha_crypto::Hash;
use iroha_data_model::sumeragi_lanes::SumeragiLaneFrontier;
use iroha_model_base::topology::LaneId;
use norito::codec::Encode;

use super::*;

fn row() -> SumeragiLaneCustody {
    SumeragiLaneCustody {
        lane: LaneId::new(7),
        incarnation: [1; 32],
        instance: [2; 32],
        created_at: 20,
        merged: SumeragiLaneFrontier {
            height: 9_999,
            block_hash: [3; 32],
            result: [4; 32],
        },
        signer_count: 4,
        signers: vec![SumeragiLaneSignerCustody {
            signer: 2,
            binding: SumeragiLaneStakeBinding {
                owner_lane: LaneId::SINGLE,
                validator: Hash::new(b"original account"),
                activation_height: 17,
                tenure: Hash::new(b"original registration and escrow asset"),
            },
        }]
        .try_into()
        .unwrap(),
        evidence_horizon: 7,
        slashing_delay: 3,
        retired_at: Some(30),
    }
}

#[test]
fn borrowed_custody_keeps_original_bindings_and_separate_global_policy_fences() {
    let source = row();
    let bytes = source.encode();
    let view = LaneCustodyView::parse(&bytes).unwrap();
    assert_eq!(
        view.identity(),
        (
            source.lane,
            source.incarnation,
            source.instance,
            source.created_at
        )
    );
    assert_eq!(view.frontier(), source.merged);
    assert_eq!(
        view.binding(2).unwrap(),
        Some(source.signers.as_slice()[0].binding)
    );
    for absent in [0, 1, 3, 4, u32::MAX] {
        assert_eq!(view.binding(absent).unwrap(), None);
    }
    assert_eq!(view.policy(), (4, 7, 3, Some(30)));
    let first = bytes.as_ptr() as usize;
    assert!((view.signers.as_ptr() as usize) >= first);
    assert!((view.signers.as_ptr() as usize) + view.signers.len() <= first + bytes.len());
    assert_eq!(
        view.fences.signers.as_slice(),
        &[],
        "no copied signer backing"
    );

    let mut live = source;
    live.retired_at = None;
    let bytes = live.encode();
    let live_view = LaneCustodyView::parse(&bytes).unwrap();
    assert_eq!(live_view.policy(), (4, 7, 3, None));
    assert_eq!(live_view.binding(2).unwrap(), view.binding(2).unwrap());
}

#[test]
fn borrowed_custody_rejects_invalid_fences_signer_bounds_and_noncanonical_frames() {
    for case in 0..6 {
        let mut source = row();
        match case {
            0 => source.signer_count = 2,
            1 => source.evidence_horizon = 0,
            2 => source.retired_at = Some(source.created_at),
            3 => source.retired_at = Some(u64::MAX),
            4 | 5 => {
                let mut signer = source.signers.as_slice()[0];
                signer.binding.activation_height =
                    if case == 4 { 0 } else { source.created_at + 1 };
                source.signers = vec![signer].try_into().unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            LaneCustodyView::parse(&source.encode()).is_err(),
            "case {case}"
        );
    }
    let bytes = row().encode();
    for end in 0..bytes.len() {
        assert!(LaneCustodyView::parse(&bytes[..end]).is_err());
    }
    let mut suffix = bytes;
    suffix.push(0);
    assert!(LaneCustodyView::parse(&suffix).is_err());
}

#[test]
fn borrowed_custody_needs_no_decoder_alignment_or_signer_allocation() {
    let source = row();
    let mut unaligned = vec![0];
    unaligned.extend_from_slice(&source.encode());
    norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(1024, unaligned.len(), 4096, 0, 32),
        || {
            let view = LaneCustodyView::parse(&unaligned[1..]).unwrap();
            assert_eq!(
                view.binding(2).unwrap(),
                Some(source.signers.as_slice()[0].binding)
            );
            assert_eq!(view.frontier(), source.merged);
            assert_eq!(view.policy(), (4, 7, 3, Some(30)));
        },
    );
    assert!(optional_height(&[2]).is_err());
    assert!(optional_height(&[0, 0]).is_err());
    assert!(optional_height(&[1, 0]).is_err());
    assert!(super::hash(&[0; 32]).is_err());
    assert!(lane_id(&[0; 4]).is_err());
}

#[test]
fn complete_payload_selection_keeps_retired_incarnations_distinct_and_rejects_duplicates() {
    let row = row();
    let mut state = iroha_data_model::sumeragi_lanes::SumeragiLaneState {
        custody: vec![row.clone()],
        ..iroha_data_model::sumeragi_lanes::SumeragiLaneState::default()
    };
    let bytes = state.encode();
    let selected = super::super::select::custody(&bytes, &row.incarnation)
        .unwrap()
        .unwrap();
    assert_eq!(selected, row.encode());
    assert!(
        super::super::select::custody(&bytes, &[7; 32])
            .unwrap()
            .is_none()
    );
    assert!(
        super::super::select::lane(&bytes, &row.incarnation)
            .unwrap()
            .is_none()
    );
    state.custody.push(row.clone());
    assert!(super::super::select::custody(&state.encode(), &row.incarnation).is_err());
}
