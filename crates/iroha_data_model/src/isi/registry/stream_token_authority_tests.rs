//! Sole canonical native role-11 instruction ID and exact framing regressions.

use super::*;
use crate::{
    isi::sorafs::MutateSorafsStreamTokenAuthority,
    sorafs::{
        capacity::ProviderId,
        stream_token_authority::{
            StreamTokenAuthorityActionV1, StreamTokenAuthorityRequestV1, StreamTokenExpireV1,
        },
    },
};
use sorafs_manifest::signer::protocol::SignerOperationReservationV1;

#[test]
fn stream_token_authority_has_one_wire_id_and_rejects_alternate_frames() {
    let instruction = MutateSorafsStreamTokenAuthority {
        request: StreamTokenAuthorityRequestV1 {
            network_id: [1; 32],
            provider_id: ProviderId::new([2; 32]),
            expected_control_revision: 3,
            expected_control_digest: [4; 32],
            action: StreamTokenAuthorityActionV1::Expire(StreamTokenExpireV1 {
                operation_id: [5; 32],
                reservation: SignerOperationReservationV1 {
                    reservation_id: [6; 32],
                    fence: 7,
                    expires_at_unix_ms: 8,
                },
            }),
        },
    };
    let expected = "iroha.instruction.v1::sorafs::MutateSorafsStreamTokenAuthority";
    let boxed: InstructionBox = instruction.clone().into();
    let (wire_id, frame) = crate::isi::encoded_instruction_pair_payload(&boxed).unwrap();
    assert_eq!(wire_id, expected);
    let registry = default();
    assert_eq!(
        registry.wire_id(std::any::type_name::<MutateSorafsStreamTokenAuthority>()),
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
            .downcast_ref::<MutateSorafsStreamTokenAuthority>(),
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
                std::any::type_name::<MutateSorafsStreamTokenAuthority>(),
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
