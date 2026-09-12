//! Canonical registration and framing of native stream-token custody mutations.
use super::*;
use crate::{
    isi::sorafs::MutateSorafsStreamTokenCustody,
    sorafs::{capacity::ProviderId, stream_token_custody::SorafsStreamTokenCustodyActionV1},
};

#[test]
fn stream_token_custody_box_uses_one_canonical_wire_identity_and_roundtrips() {
    let instruction = MutateSorafsStreamTokenCustody {
        provider_id: ProviderId::new([3; 32]),
        expected_revision: 4,
        expected_digest: [5; 32],
        action: SorafsStreamTokenCustodyActionV1::Revoke {
            signer: true,
            attester: false,
        },
    };
    let boxed: InstructionBox = instruction.clone().into();
    let expected = "iroha.instruction.v1::sorafs::MutateSorafsStreamTokenCustody";
    let (wire_id, framed) =
        crate::isi::encoded_instruction_pair_payload(&boxed).expect("registered custody mutation");
    assert_eq!(wire_id, expected);
    let registry = default();
    assert_eq!(
        registry.wire_id(std::any::type_name::<MutateSorafsStreamTokenCustody>()),
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
        .decode(wire_id, &framed)
        .expect("known wire id")
        .expect("canonical mutation frame");
    let decoded = decoded
        .as_any()
        .downcast_ref::<MutateSorafsStreamTokenCustody>()
        .expect("decoded custody instruction");
    assert_eq!(decoded, &instruction);
    let reencoded =
        crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(decoded.clone()))
            .expect("reencoded frame");
    assert_eq!(reencoded, (wire_id, framed.clone()));
    assert!(
        registry
            .decode(
                std::any::type_name::<MutateSorafsStreamTokenCustody>(),
                &framed
            )
            .is_none(),
        "Rust type paths are not alternate wire IDs"
    );
    let foreign = crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(Log::new(
        Level::INFO,
        "foreign root".to_owned(),
    )))
    .expect("registered log frame");
    assert!(
        registry
            .decode(expected, &foreign.1)
            .expect("known custody id")
            .is_err(),
        "a different valid instruction frame cannot substitute for custody"
    );
    assert!(
        registry
            .decode(expected, &framed[..framed.len() - 1])
            .expect("known custody id")
            .is_err()
    );
}
