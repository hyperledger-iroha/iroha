//! Sole canonical role-16 topology instruction identity and Norito framing.
use super::*;
use crate::{
    isi::sorafs::MutateSorafsTopologyAuthority,
    sorafs::topology_authority::{
        TopologyActionV1, TopologyHeadV1, TopologyRevocationV1, TopologyTransitionV1,
    },
};

fn transition() -> TopologyTransitionV1 {
    TopologyTransitionV1 {
        deployment_id: "sora-main".to_owned(),
        control: TopologyHeadV1::EMPTY,
        operations: TopologyHeadV1::EMPTY,
        action: TopologyActionV1::Revoke(TopologyRevocationV1 {
            signer: true,
            attester: false,
        }),
    }
}

#[test]
fn topology_instruction_roundtrips_under_one_v1_wire_identity() {
    let instruction = MutateSorafsTopologyAuthority {
        transition: transition(),
    };
    let boxed: InstructionBox = instruction.clone().into();
    let expected = "iroha.instruction.v1::sorafs::MutateSorafsTopologyAuthority";
    let (wire_id, frame) =
        crate::isi::encoded_instruction_pair_payload(&boxed).expect("registered topology ISI");
    assert_eq!(wire_id, expected);
    let registry = default();
    assert_eq!(
        registry.wire_id(std::any::type_name::<MutateSorafsTopologyAuthority>()),
        Some(expected)
    );
    assert_eq!(
        wire_ids::ALL
            .iter()
            .filter(|entry| entry.wire_id == expected)
            .count(),
        1
    );
    let decoded = registry
        .decode(expected, &frame)
        .expect("canonical V1 ID")
        .expect("canonical frame");
    assert_eq!(
        decoded
            .as_any()
            .downcast_ref::<MutateSorafsTopologyAuthority>(),
        Some(&instruction)
    );
    assert_eq!(
        crate::isi::encoded_instruction_pair_payload(&decoded).expect("canonical re-encoding"),
        (expected, frame.clone())
    );
    let boxed_frame = norito::encode_canonical(&boxed).expect("boxed Norito frame");
    assert_eq!(
        norito::decode_canonical::<InstructionBox>(&boxed_frame).expect("boxed roundtrip"),
        boxed
    );
}

#[test]
fn topology_instruction_rejects_wrong_root_truncation_and_type_path_alias() {
    let instruction = MutateSorafsTopologyAuthority {
        transition: transition(),
    };
    let expected = "iroha.instruction.v1::sorafs::MutateSorafsTopologyAuthority";
    let registry = default();
    let frame = norito::encode_canonical(&instruction).expect("instruction frame");
    let transition_frame =
        norito::encode_canonical(&instruction.transition).expect("transition frame");
    assert!(matches!(
        norito::decode_canonical::<MutateSorafsTopologyAuthority>(&transition_frame),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(matches!(
        norito::decode_canonical::<TopologyTransitionV1>(&frame),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(
        registry
            .decode(expected, &transition_frame)
            .unwrap()
            .is_err()
    );
    assert!(
        registry
            .decode(expected, &frame[..frame.len() - 1])
            .unwrap()
            .is_err()
    );
    assert!(
        registry
            .decode(
                std::any::type_name::<MutateSorafsTopologyAuthority>(),
                &frame
            )
            .is_none(),
        "Rust type paths are not alternate wire IDs"
    );
}
