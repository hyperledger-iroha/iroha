//! Current-source frames for the endorsed runtime-upgrade release addition.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    isi::governance::{ProposeEndorsedRuntimeUpgradeProposal, ProposeRuntimeUpgradeProposal},
    runtime::{
        definition::SelectionId,
        lifecycle::{ChangeSetEndorsementV1, ChangeSetV1, RootScopeV1, RuntimeUpgradeActionV1},
    },
};
use norito::json::Value;

use crate::frame_identity_test_support::record_binary;

fn proposal_values() -> Vec<ProposeEndorsedRuntimeUpgradeProposal> {
    let change_set = ChangeSetV1 {
        network: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [0x11; 32],
        ))),
        root: RootScopeV1::Universal,
        predecessor: SelectionId([1; 32]),
        successor: SelectionId([2; 32]),
        effective_height: 100,
        publish: Vec::new(),
        transitions: Vec::new(),
        qualification: [3; 32],
    };
    let change_set_id = change_set.id().expect("fixture change-set identity");
    // A public deterministic test seed, never a deployment signing key.
    let key = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
    let endorsement =
        ChangeSetEndorsementV1::sign(change_set_id, key.public_key().clone(), key.private_key())
            .expect("sign fixture endorsement");
    endorsement.verify().expect("valid fixture endorsement");
    vec![
        ProposeEndorsedRuntimeUpgradeProposal {
            action: RuntimeUpgradeActionV1::Schedule(change_set.clone()),
            provenance: Vec::new(),
        },
        ProposeEndorsedRuntimeUpgradeProposal {
            action: RuntimeUpgradeActionV1::Schedule(change_set),
            provenance: vec![endorsement],
        },
        ProposeEndorsedRuntimeUpgradeProposal {
            action: RuntimeUpgradeActionV1::Cancel(change_set_id),
            provenance: Vec::new(),
        },
    ]
}

fn frame_evidence() -> Value {
    let values = proposal_values();
    let mut rows = Vec::new();
    for (case, value) in ["schedule", "endorsed_schedule", "cancel"]
        .into_iter()
        .zip(&values)
    {
        record_binary(&mut rows, case, value);
        record_binary(&mut rows, &format!("{case}/option"), &Some(value.clone()));
    }
    record_binary(
        &mut rows,
        "option_none",
        &None::<ProposeEndorsedRuntimeUpgradeProposal>,
    );
    record_binary(
        &mut rows,
        "vec_empty",
        &Vec::<ProposeEndorsedRuntimeUpgradeProposal>::new(),
    );
    record_binary(&mut rows, "vec_all", &values);
    assert_eq!(rows.len(), 9);
    norito::json!({
        "format_version": 1,
        "purpose": "current endorsed runtime-upgrade release-addition frames; not pre-cutover observations",
        "default_encode_flags": (norito::core::default_encode_flags()),
        "rows": rows,
    })
}

#[test]
fn endorsed_runtime_upgrade_frames_match_capture() {
    let expected: Value = norito::json::from_json(include_str!(
        "../fixtures/endorsed_runtime_upgrade_identity_frames.json"
    ))
    .expect("current-source endorsed proposal capture");
    assert_eq!(frame_evidence(), expected);
}

#[test]
fn endorsed_and_baseline_proposal_frames_are_distinct() {
    for value in proposal_values() {
        let baseline = ProposeRuntimeUpgradeProposal {
            action: value.action.clone(),
        };
        let endorsed_frame = norito::encode_canonical(&value).expect("endorsed frame");
        let baseline_frame = norito::encode_canonical(&baseline).expect("baseline frame");
        assert_ne!(endorsed_frame, baseline_frame);
        assert!(matches!(
            norito::decode_canonical::<ProposeRuntimeUpgradeProposal>(&endorsed_frame),
            Err(norito::Error::SchemaMismatch)
        ));
        assert!(matches!(
            norito::decode_canonical::<ProposeEndorsedRuntimeUpgradeProposal>(&baseline_frame),
            Err(norito::Error::SchemaMismatch)
        ));
    }
}

#[test]
#[ignore = "explicit maintenance capture of current endorsed runtime-upgrade frames"]
fn capture_endorsed_runtime_upgrade_frames() {
    println!(
        "ENDORSED_RUNTIME_UPGRADE_IDENTITY_FRAMES={}",
        norito::json::to_json(&frame_evidence()).expect("serialize current-source capture")
    );
}
