//! Complete registry selection, lexical commitment and admission-order controls.

use super::*;

fn limits(max_tables: usize) -> LeafLimits {
    LeafLimits {
        max_tables,
        max_rows: 8,
        max_payload_bytes: 4096,
        max_ordered_table_bytes: 4096,
        max_streamed_value_bytes: 8192,
    }
}

fn declared_tables(fields: &'static [Field], output: &mut Vec<&'static Field>) {
    for field in fields {
        match field.role {
            Role::Canonical(Canonical::Table { .. }) => output.push(field),
            Role::Canonical(Canonical::Owner(children)) => declared_tables(children, output),
            _ => {}
        }
    }
}

#[test]
fn static_catalog_covers_every_nested_table_once_in_lexical_order() {
    let mut expected = Vec::new();
    declared_tables(STATE_FIELDS, &mut expected);
    expected.sort_by_key(|field| field.id);
    assert_eq!(expected.len(), TABLES.len());
    assert!(
        expected.len() > u64::BITS as usize,
        "exercise multiple words"
    );
    for (actual, expected) in TABLES.iter().zip(&expected) {
        assert_eq!(actual.id, expected.id);
        assert!(matches!(
            actual.role,
            Role::Canonical(Canonical::Table { .. })
        ));
    }
    for pair in TABLES.windows(2) {
        assert!(pair[0].id < pair[1].id);
    }
}

#[test]
fn selections_bind_lexical_schemas_and_outlive_caller_identity_buffers() {
    let accepted: Vec<_> = TABLES
        .iter()
        .filter(|field| declared_table(field.id).is_ok())
        .collect();
    let mut owned_ids: Vec<_> = accepted
        .iter()
        .rev()
        .map(|field| field.id.to_owned())
        .collect();
    let borrowed: Vec<_> = owned_ids.iter().map(String::as_str).collect();
    let selection = TableSelection::new(&borrowed, limits(accepted.len())).unwrap();
    drop(borrowed);
    for id in &mut owned_ids {
        id.clear();
        id.push_str("replaced caller identity");
    }
    drop(owned_ids);

    assert_eq!(selection.len(), accepted.len());
    assert_eq!(
        selection.first_table_id(),
        accepted.first().map(|field| field.id)
    );
    let expected = accepted
        .iter()
        .fold(Hash::new(SCHEMA_START), |root, field| {
            let Role::Canonical(Canonical::Table { key, value }) = field.role else {
                unreachable!("test selected a table")
            };
            fold_schema(root, field, key, value)
        });
    assert_eq!(selection.schema, expected);
    for field in &TABLES {
        assert_eq!(
            selection.table(field.id).is_some(),
            declared_table(field.id).is_ok()
        );
    }
    assert!(selection.table("unknown").is_none());
    let empty = TableSelection::new(&[], limits(0)).unwrap();
    assert_eq!(empty.len(), 0);
    assert_eq!(empty.first_table_id(), None);
    assert_eq!(empty.schema, Hash::new(SCHEMA_START));
    assert!(empty.table(accepted[0].id).is_none());
    assert_ne!(empty.schema, selection.schema);
}

#[test]
fn selection_refusals_preserve_count_duplicate_and_schema_precedence() {
    assert!(matches!(
        TableSelection::new(&["unknown"], limits(0)),
        Err(LeafError::TableLimit)
    ));
    assert!(matches!(
        TableSelection::new(&["unknown"], limits(1)),
        Err(LeafError::UnknownField)
    ));
    let table = TABLES
        .iter()
        .find(|field| declared_table(field.id).is_ok())
        .unwrap()
        .id;
    let duplicate = table.to_owned();
    assert!(matches!(
        TableSelection::new(&[table, duplicate.as_str()], limits(2)),
        Err(LeafError::DuplicateTable(id)) if id == table
    ));
    assert!(matches!(
        TableSelection::new(&[table, duplicate.as_str()], limits(1)),
        Err(LeafError::TableLimit)
    ));
    for field in &TABLES {
        if let Err(expected) = declared_table(field.id) {
            let Err(actual) = TableSelection::new(&[field.id], limits(1)) else {
                panic!("unresolved table admitted: {}", field.id)
            };
            assert_eq!(actual, expected);
        }
    }
    let selection = TableSelection::new(&[table], limits(1)).unwrap();
    assert_eq!(
        selection.key_path("omitted", &0_u64),
        Err(LeafError::TableNotSelected)
    );
}

#[test]
fn caller_owned_identity_refusals_do_not_retain_or_encode_the_input() {
    let empty = TableSelection::new(&[], limits(0)).unwrap();
    let original_schema = empty.schema;
    let (unknown, omitted, capped) = {
        // The invalid descriptor is caller-owned before admission. Neither refusal
        // needs its contents or lifetime after the call returns.
        let identity = "不存在".repeat(32 * 1024);
        let unknown = TableSelection::new(&[&identity], limits(1)).err().unwrap();
        let omitted = empty.key_path(&identity, &0_u64).unwrap_err();
        let capped = TableSelection::new(&[&identity], limits(0)).err().unwrap();
        (unknown, omitted, capped)
    };
    assert_eq!(unknown, LeafError::UnknownField);
    assert_eq!(omitted, LeafError::TableNotSelected);
    assert_eq!(capped, LeafError::TableLimit);
    assert_eq!(unknown.to_string(), "unknown State authority field");
    assert_eq!(
        omitted.to_string(),
        "State authority table was not selected"
    );
    assert_eq!(empty.len(), 0);
    assert_eq!(empty.schema, original_schema);
}
