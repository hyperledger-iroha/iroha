//! Literal schema custody without inferring names or changing canonical bytes.

use super::*;
use crate::state::authority_registry::{
    schema,
    world::{
        musubi_availability_policy::MusubiAvailabilityAuthorityV1,
        musubi_universal_policy::{MusubiDirectoryAuthorityV1, MusubiResolverAuthorityV1},
    },
};
use iroha_allocation::{AllocationBudget, AllocationRefusal};
use iroha_data_model::musubi::{ArchiveId, MusubiPackageSelectorV1, MusubiReleaseIdV1};
use std::borrow::Cow;

#[test]
fn all_musubi_semantic_capture_keys_and_values_have_exact_borrowed_identities() {
    for (identity, expected) in [
        (
            norito::schema::identity::nominal_name::<ArchiveId>(),
            ArchiveId::nominal_name(),
        ),
        (
            norito::schema::identity::nominal_name::<MusubiReleaseIdV1>(),
            MusubiReleaseIdV1::nominal_name(),
        ),
        (
            norito::schema::identity::nominal_name::<MusubiPackageSelectorV1>(),
            MusubiPackageSelectorV1::nominal_name(),
        ),
        (
            norito::schema::identity::nominal_name::<MusubiAvailabilityAuthorityV1>(),
            MusubiAvailabilityAuthorityV1::nominal_name(),
        ),
        (
            norito::schema::identity::nominal_name::<MusubiResolverAuthorityV1>(),
            MusubiResolverAuthorityV1::nominal_name(),
        ),
        (
            norito::schema::identity::nominal_name::<MusubiDirectoryAuthorityV1>(),
            MusubiDirectoryAuthorityV1::nominal_name(),
        ),
    ] {
        assert!(matches!(identity, Cow::Borrowed(_)));
        assert_eq!(identity, expected);
    }
    for id in [
        "world.musubi_archive_availability",
        "world.musubi_resolver_index",
        "world.musubi_public_directory",
    ] {
        let field = declared_table(id).unwrap();
        let Role::Canonical(Canonical::Table { key, value }) = field.role else {
            panic!("canonical table declaration required")
        };
        assert!(matches!(schema_identity(key), Cow::Borrowed(_)));
        assert!(matches!(schema_identity(value), Cow::Borrowed(_)));
    }
}

#[test]
fn borrowed_identity_schema_and_payload_hashes_match_the_previous_owned_transcript() {
    let accumulator = Hash::new(SCHEMA_START);
    for id in [
        "world.musubi_archive_availability",
        "world.musubi_resolver_index",
        "world.musubi_public_directory",
    ] {
        let field = declared_table(id).unwrap();
        let Role::Canonical(Canonical::Table { key, value }) = field.role else {
            panic!("canonical table declaration required")
        };
        let key_name = schema_identity(key).into_owned();
        let value_name = schema_identity(value).into_owned();
        let expected = Hash::new_from_chunks(&[
            SCHEMA_FIELD,
            accumulator.as_ref(),
            &(field.id.len() as u64).to_le_bytes(),
            field.id.as_bytes(),
            schema_name(key).as_bytes(),
            &(key_name.len() as u64).to_le_bytes(),
            key_name.as_bytes(),
            schema_name(value).as_bytes(),
            &(value_name.len() as u64).to_le_bytes(),
            value_name.as_bytes(),
            &[V1_LAYOUT.major, V1_LAYOUT.minor, V1_LAYOUT.flags],
        ]);
        assert_eq!(fold_schema(accumulator, field, key, value), expected);
        let payload = b"exact retained payload";
        for (schema, name) in [(key, key_name), (value, value_name)] {
            let expected = Hash::new_from_chunks(&[
                KEY_PAYLOAD,
                &(name.len() as u64).to_le_bytes(),
                name.as_bytes(),
                &[V1_LAYOUT.major, V1_LAYOUT.minor, V1_LAYOUT.flags],
                payload,
                &(payload.len() as u64).to_le_bytes(),
            ]);
            assert_eq!(
                bare_payload_hash(id, schema, payload, KEY_PAYLOAD).unwrap(),
                expected
            );
        }
    }
}

struct LiteralU64(u64);
impl NoritoSchema for LiteralU64 {
    fn nominal_name() -> String {
        panic!("the owned schema constructor must never run for a literal identity")
    }
    fn static_nominal_name() -> Option<&'static str> {
        Some("u64")
    }
}
impl norito::SerializePayload for LiteralU64 {
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::SerializePayload::serialize(&self.0, encoder)
    }
}

#[test]
fn literal_type_checks_skip_owned_name_construction_and_keep_original_frame_funding() {
    let value = LiteralU64(7);
    let expected = norito::codec::encode_adaptive(&7_u64);
    let budget = AllocationBudget::new(expected.len());
    let frame = typed_bare_payload(
        "literal.owner",
        schema::<u64>(),
        &value,
        expected.len(),
        |length| frame::FundedFrame::new(length, &budget),
    )
    .unwrap()
    .into_buffer();
    assert_eq!(frame.as_slice(), expected);
    assert_eq!(budget.reserved_bytes(), expected.len());
    assert_eq!(
        typed_bare_payload_digests("literal.owner", schema::<u64>(), &value, expected.len())
            .unwrap(),
        typed_bare_payload_digests("literal.owner", schema::<u64>(), &7_u64, expected.len())
            .unwrap()
    );
    assert_eq!(
        typed_payload_hash(
            "literal.owner",
            schema::<u64>(),
            &value,
            KEY_PAYLOAD,
            expected.len()
        )
        .unwrap(),
        typed_payload_hash(
            "literal.owner",
            schema::<u64>(),
            &7_u64,
            KEY_PAYLOAD,
            expected.len()
        )
        .unwrap()
    );
    assert!(matches!(
        typed_bare_payload(
            "literal.owner",
            schema::<u32>(),
            &value,
            expected.len(),
            |length| { frame::FundedFrame::new(length, &budget) }
        ),
        Err(LeafError::TypeMismatch("literal.owner"))
    ));
    drop(frame);
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(0);
    assert!(matches!(
        typed_bare_payload(
            "literal.owner",
            schema::<u64>(),
            &value,
            expected.len(),
            |length| { frame::FundedFrame::new(length, &budget) }
        ),
        Err(LeafError::Admission(AllocationRefusal::ExceedsLimit { .. }))
    ));
    assert_eq!(budget.reserved_bytes(), 0);
}
