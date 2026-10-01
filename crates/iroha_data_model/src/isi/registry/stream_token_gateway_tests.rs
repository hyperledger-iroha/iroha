//! Sole canonical native gateway instruction ID and exact framing regressions.

use super::*;
use crate::{
    NetworkId,
    isi::sorafs::MutateSorafsStreamTokenGateway,
    sorafs::stream_token_gateway::native::{
        StreamTokenGatewayActionV1, StreamTokenGatewayRequestV1,
    },
};
use iroha_crypto::{Hash, HashOf};

#[test]
fn stream_token_gateway_has_one_wire_id_and_rejects_alternate_frames() {
    let instruction = MutateSorafsStreamTokenGateway {
        request: StreamTokenGatewayRequestV1 {
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"gateway-registry",
            ))),
            gateway_id: [2; 32],
            expected_policy_revision: 3,
            expected_policy_digest: [4; 32],
            action: StreamTokenGatewayActionV1::Expire { max_items: 256 },
        },
    };
    instruction.request.validate().unwrap();
    let expected = "iroha.instruction.v1::sorafs::MutateSorafsStreamTokenGateway";
    let boxed: InstructionBox = instruction.clone().into();
    let (wire_id, frame) = crate::isi::encoded_instruction_pair_payload(&boxed).unwrap();
    assert_eq!(wire_id, expected);
    let registry = default();
    assert_eq!(
        registry.wire_id(std::any::type_name::<MutateSorafsStreamTokenGateway>()),
        Some(expected)
    );
    assert_eq!(
        wire_ids::ALL
            .iter()
            .filter(|entry| entry.wire_id == expected)
            .count(),
        1
    );
    let decoded = registry.decode(expected, &frame).unwrap().unwrap();
    assert_eq!(
        decoded
            .as_any()
            .downcast_ref::<MutateSorafsStreamTokenGateway>(),
        Some(&instruction)
    );
    assert_eq!(
        crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(instruction.clone()))
            .unwrap(),
        (wire_id, frame.clone())
    );
    assert!(
        registry
            .decode(
                std::any::type_name::<MutateSorafsStreamTokenGateway>(),
                &frame
            )
            .is_none()
    );
    assert!(
        registry
            .decode(expected, &frame[..frame.len() - 1])
            .unwrap()
            .is_err()
    );
    let mut trailing = frame;
    trailing.push(0);
    assert!(registry.decode(expected, &trailing).unwrap().is_err());
}
