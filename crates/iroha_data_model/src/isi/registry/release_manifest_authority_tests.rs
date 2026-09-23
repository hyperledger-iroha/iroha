//! One canonical role-13 instruction wire identity; no alternate role or type-path decoder.
use super::*;
use crate::{
    isi::sorafs::MutateSorafsReleaseManifestAuthority,
    sorafs::release_manifest_authority::{ReleaseManifestActionV1, ReleaseManifestRevocationV1},
};

#[test]
fn release_manifest_authority_instruction_has_one_canonical_wire_identity() {
    let instruction = MutateSorafsReleaseManifestAuthority {
        deployment_id: "release-primary".into(),
        expected_control_revision: 7,
        expected_control_digest: [9; 32],
        action: ReleaseManifestActionV1::Revoke(ReleaseManifestRevocationV1 {
            signer: true,
            attester: false,
        }),
    };
    let boxed: InstructionBox = instruction.clone().into();
    let expected = "iroha.instruction.v1::sorafs::MutateSorafsReleaseManifestAuthority";
    let (wire_id, frame) =
        crate::isi::encoded_instruction_pair_payload(&boxed).expect("registered instruction");
    assert_eq!(wire_id, expected);
    let registry = default();
    assert_eq!(
        registry.wire_id(std::any::type_name::<MutateSorafsReleaseManifestAuthority>()),
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
        .decode(wire_id, &frame)
        .expect("registered ID")
        .expect("canonical frame");
    let decoded = decoded
        .as_any()
        .downcast_ref::<MutateSorafsReleaseManifestAuthority>()
        .expect("role-13 instruction");
    assert_eq!(decoded, &instruction);
    assert_eq!(
        crate::isi::encoded_instruction_pair_payload(&decoded.clone().into())
            .expect("re-encoded instruction"),
        (wire_id, frame.clone())
    );
    assert!(
        registry
            .decode(
                std::any::type_name::<MutateSorafsReleaseManifestAuthority>(),
                &frame
            )
            .is_none(),
        "Rust type paths are not alternate wire IDs"
    );
    let foreign = crate::isi::encoded_instruction_pair_payload(&InstructionBox::from(Log::new(
        Level::INFO,
        "foreign instruction".into(),
    )))
    .expect("foreign canonical frame");
    assert!(registry.decode(expected, &foreign.1).unwrap().is_err());
    assert!(
        registry
            .decode(expected, &frame[..frame.len() - 1])
            .unwrap()
            .is_err()
    );
}
