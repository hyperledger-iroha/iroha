// Wire-schema regression tests included in the parent consensus test module.
#[test]
fn cert_phase_schema_matches_canonical_wire_discriminants() {
    use iroha_schema::{IntoSchema as _, Metadata};
    use norito::codec::{DecodeAll as _, Encode as _};
    let cases = [
        (CertPhase::Prepare, 1_u32),
        (CertPhase::Commit, 2),
        (CertPhase::NewView, 3),
    ];
    for (phase, expected) in cases {
        let encoded = phase.encode();
        assert_eq!(
            u32::from_le_bytes(encoded[..4].try_into().expect("phase tag")),
            expected
        );
        assert_eq!(
            CertPhase::decode_all(&mut encoded.as_slice()).expect("phase roundtrip"),
            phase
        );
    }
    let schema = CertPhase::schema();
    let Metadata::Enum(metadata) = schema.get::<CertPhase>().expect("phase schema") else {
        panic!("CertPhase schema must be an enum");
    };
    assert_eq!(
        metadata
            .variants
            .iter()
            .map(|variant| variant.discriminant)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
}
