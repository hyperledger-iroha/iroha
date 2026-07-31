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
fn builtin_query_wire_ids_override_installed_registry_and_decode_falls_back() {
    const WIRE_ID: &str =
        "iroha_data_model::query::ErasedIterQuery<iroha_data_model::domain::model::Domain>";
    const CUSTOM_WIRE_ID: &str = "custom.domain.current-path";
    type DomainQuery = ErasedIterQuery<Domain>;

    let builtin = QueryRegistry::new().register_with_id::<DomainQuery>(WIRE_ID);
    let installed = QueryRegistry::new().register_with_id::<DomainQuery>(CUSTOM_WIRE_ID);
    let query = DomainQuery::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        vec![0xA5, 0x5A],
    );
    let payload = norito::codec::Encode::encode(&query);
    let type_name = std::any::type_name::<DomainQuery>();

    assert_eq!(
        query_wire_id_from_registries(type_name, &builtin, Some(&installed)),
        WIRE_ID
    );
    for lookup_key in [type_name, WIRE_ID, CUSTOM_WIRE_ID] {
        let decoded =
            decode_query_from_registries(lookup_key, &payload, &builtin, Some(&installed))
                .expect("query lookup key is registered")
                .expect("query payload decodes");
        assert_eq!(decoded.as_ref().type_name_key(), type_name);
        assert_eq!(decoded.as_ref().encode_bytes(), payload);
    }
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
fn query_registry_rejects_wire_id_and_type_name_collisions() {
    type DomainQuery = ErasedIterQuery<Domain>;
    type AccountQuery = ErasedIterQuery<crate::account::Account>;

    let _registry = QueryRegistry::new()
        .register_with_id::<DomainQuery>(std::any::type_name::<AccountQuery>())
        .register::<AccountQuery>();
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
fn installed_query_registry_rejects_builtin_type_name_collision() {
    type DomainQuery = ErasedIterQuery<Domain>;
    type AccountQuery = ErasedIterQuery<crate::account::Account>;

    let builtin = QueryRegistry::new().register_with_id::<DomainQuery>("builtin.domain");
    let installed =
        QueryRegistry::new().register_with_id::<AccountQuery>(std::any::type_name::<DomainQuery>());
    builtin.assert_compatible_with(&installed);
}
