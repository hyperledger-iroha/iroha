//! Stable V1 query wire-ID regression tests.
use super::*;
use crate::domain::Domain;
#[test]
fn builtin_query_wire_ids_match_v1_golden_inventory() {
    let mut actual = BUILTIN_QUERY_WIRE_ASSIGNMENTS
        .iter()
        .map(|(type_label, wire_id)| format!("{type_label}\t{wire_id}"))
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert!(
        BUILTIN_QUERY_WIRE_ASSIGNMENTS
            .iter()
            .map(|(_, wire_id)| wire_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == BUILTIN_QUERY_WIRE_ASSIGNMENTS.len(),
        "built-in query wire identifiers must be unique"
    );
    let expected = include_str!("../../../tests/fixtures/query_wire_ids_v1.txt")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}
#[test]
fn builtin_query_runtime_registry_has_no_type_name_aliases() {
    let registry = build_builtin_query_registry();
    for (type_name, wire_id) in builtin_query_runtime_assignments() {
        assert!(
            wire_id.starts_with("iroha.query.v1::iterable::"),
            "built-in wire identifier `{wire_id}` must use the canonical V1 iterable-query namespace"
        );
        assert_eq!(registry.wire_id(type_name), Some(wire_id));
        assert_ne!(
            type_name, wire_id,
            "built-in wire identifier must not collapse to its concrete Rust type name"
        );
        assert!(
            registry.decode(wire_id, &[]).is_some(),
            "canonical wire identifier `{wire_id}` must select its constructor"
        );
        assert!(
            registry.decode(type_name, &[]).is_none(),
            "concrete type name `{type_name}` must not remain a decode alias"
        );
    }
}
#[test]
fn builtin_query_decode_accepts_only_the_canonical_wire_id() {
    const WIRE_ID: &str = "iroha.query.v1::iterable::domain::Domain";
    const RETIRED_RUST_PATH_ID: &str =
        "iroha_data_model::query::ErasedIterQuery<iroha_data_model::domain::model::Domain>";
    type DomainQuery = ErasedIterQuery<Domain>;
    let builtin = QueryRegistry::new().register_with_id::<DomainQuery>(WIRE_ID);
    let query = DomainQuery::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        vec![0xA5, 0x5A],
    );
    let payload = norito::codec::Encode::encode(&query);
    let type_name = std::any::type_name::<DomainQuery>();
    assert_eq!(
        query_wire_id_from_registries(type_name, &builtin, None),
        Some(WIRE_ID)
    );
    let decoded = decode_query_from_registries(WIRE_ID, &payload, &builtin, None)
        .expect("canonical query wire identifier is registered")
        .expect("query payload decodes");
    assert_eq!(decoded.as_ref().type_name_key(), type_name);
    assert_eq!(decoded.as_ref().encode_bytes(), payload);
    assert!(
        decode_query_from_registries(type_name, &payload, &builtin, None).is_none(),
        "the concrete Rust type name must not remain a decode alias"
    );
    assert!(
        decode_query_from_registries(RETIRED_RUST_PATH_ID, &payload, &builtin, None).is_none(),
        "the retired Rust-path wire identifier must not remain a decode alias"
    );
}
#[test]
fn explicit_custom_query_wire_id_is_the_only_decode_key() {
    const WIRE_ID: &str = "custom.domain.v1";
    type DomainQuery = ErasedIterQuery<Domain>;
    let registry = QueryRegistry::new().register_with_id::<DomainQuery>(WIRE_ID);
    let query = DomainQuery::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        vec![0xA5, 0x5A],
    );
    let payload = norito::codec::Encode::encode(&query);
    let type_name = std::any::type_name::<DomainQuery>();
    assert_eq!(registry.wire_id(type_name), Some(WIRE_ID));
    assert!(registry.decode(WIRE_ID, &payload).is_some());
    assert!(registry.decode(type_name, &payload).is_none());
}
#[test]
fn unregistered_query_type_has_no_encoding_fallback() {
    type DomainQuery = ErasedIterQuery<Domain>;
    assert_eq!(
        query_wire_id_from_registries(
            std::any::type_name::<DomainQuery>(),
            &QueryRegistry::new(),
            None,
        ),
        None
    );
}
#[test]
#[should_panic(expected = "query registry key collision")]
fn query_registry_rejects_wire_id_collisions() {
    type DomainQuery = ErasedIterQuery<Domain>;
    type AccountQuery = ErasedIterQuery<crate::account::Account>;
    let _registry = QueryRegistry::new()
        .register_with_id::<DomainQuery>("query.collision")
        .register_with_id::<AccountQuery>("query.collision");
}
#[test]
#[should_panic(expected = "query registry key collision")]
fn query_registry_rejects_alternate_id_for_same_type() {
    type DomainQuery = ErasedIterQuery<Domain>;
    let _registry = QueryRegistry::new()
        .register_with_id::<DomainQuery>("query.domain.v1")
        .register_with_id::<DomainQuery>("query.domain.alias");
}

#[test]
#[should_panic(expected = "query registry key collision")]
fn query_registry_rejects_own_type_name_as_wire_id() {
    type DomainQuery = ErasedIterQuery<Domain>;
    let _registry =
        QueryRegistry::new().register_with_id::<DomainQuery>(std::any::type_name::<DomainQuery>());
}

#[test]
#[should_panic(expected = "query registry key collision")]
fn query_registry_rejects_wire_id_and_type_name_collisions() {
    type DomainQuery = ErasedIterQuery<Domain>;
    type AccountQuery = ErasedIterQuery<crate::account::Account>;
    let _registry = QueryRegistry::new()
        .register_with_id::<DomainQuery>("query.domain.v1")
        .register_with_id::<AccountQuery>(std::any::type_name::<DomainQuery>());
}
#[test]
#[should_panic(expected = "query registry key collision")]
fn installed_query_registry_rejects_builtin_wire_id_collision() {
    type DomainQuery = ErasedIterQuery<Domain>;
    type AccountQuery = ErasedIterQuery<crate::account::Account>;
    let builtin = QueryRegistry::new().register_with_id::<DomainQuery>("builtin.domain");
    let installed = QueryRegistry::new().register_with_id::<AccountQuery>("builtin.domain");
    builtin.assert_compatible_with(&installed);
}
#[test]
#[should_panic(expected = "query registry key collision")]
fn installed_query_registry_rejects_builtin_type_alias() {
    type DomainQuery = ErasedIterQuery<Domain>;
    type AccountQuery = ErasedIterQuery<crate::account::Account>;
    let builtin = QueryRegistry::new().register_with_id::<DomainQuery>("builtin.domain");
    let installed =
        QueryRegistry::new().register_with_id::<AccountQuery>(std::any::type_name::<DomainQuery>());
    builtin.assert_compatible_with(&installed);
}

#[test]
#[should_panic(expected = "query registry key collision")]
fn installed_query_registry_rejects_alternate_id_for_builtin_type() {
    type DomainQuery = ErasedIterQuery<Domain>;
    let builtin = QueryRegistry::new().register_with_id::<DomainQuery>("builtin.domain");
    let installed = QueryRegistry::new().register_with_id::<DomainQuery>("installed.domain");
    builtin.assert_compatible_with(&installed);
}

// A registered query is a bare payload inside the registry envelope. It need not
// expose a typed frame serializer or consult a frame schema during decoding.
#[derive(Debug, PartialEq)]
struct PayloadOnlyQuery(u32);

impl seal::Query for PayloadOnlyQuery {}

impl Query for PayloadOnlyQuery {
    type Item = QueryOutputBatchBox;
}

impl norito::core::SerializePayload for PayloadOnlyQuery {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::SerializePayload::serialize(&self.0, writer)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(4)
    }
}

impl norito::core::NoritoDeserialize<'_> for PayloadOnlyQuery {
    fn schema_hash() -> [u8; 16] {
        panic!("bare query decoding must not request a frame schema")
    }
}
impl<'de> norito::core::DeserializePayload<'de> for PayloadOnlyQuery {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("validated query payload")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        u32::try_deserialize(archived.cast()).map(Self)
    }
}

#[test]
fn registry_preserves_bare_query_encoding_without_a_frame_serializer() {
    const WIRE_ID: &str = "test.bare-query.v1";
    let query = PayloadOnlyQuery(0x1020_3040);
    let payload = query.encode();
    let registry = QueryRegistry::new().register_with_id::<PayloadOnlyQuery>(WIRE_ID);
    let decoded = registry.decode(WIRE_ID, &payload).unwrap().unwrap();
    assert_eq!(decoded.erased_as_any().downcast_ref(), Some(&query));
    assert_eq!(decoded.encode_bytes(), payload);
    assert_eq!(decoded.encoded_payload_len_exact(), Some(payload.len()));
    let mut streamed = Vec::new();
    let used = decoded
        .encode_payload_to(&mut norito::core::Encoder::new(&mut streamed))
        .unwrap();
    assert_eq!(used, payload.len());
    assert_eq!(streamed, payload);
    let mut malformed = payload;
    malformed.push(0xff);
    assert!(matches!(
        registry.decode(WIRE_ID, &malformed).unwrap(),
        Err(norito::Error::LengthMismatch)
    ));
}
