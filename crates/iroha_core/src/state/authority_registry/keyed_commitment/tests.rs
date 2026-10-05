//! Contract tests of the keyed State commitment interface.

use super::conformance::{RULES, Violation, check_contract};
use super::reference::{Reference, fault};
use super::*;
use crate::state::authority_registry::{STATE_FIELDS, V1_LAYOUT, schema};

#[test]
fn reference_oracle_satisfies_every_contract_rule() {
    assert_eq!(check_contract::<Reference>(), Ok(()));
    assert!(Reference::<{ fault::NONE }>::CONSTRUCTION.contains("test only"));
}

/// The suite has teeth: each deliberate violation is reported under its own rule, and
/// every rule of the specification has at least one such negative control.
#[test]
fn each_contract_rule_reports_its_deliberate_violation() {
    use std::{cell::RefCell, collections::BTreeSet};

    thread_local! {
        static REPORTED: RefCell<BTreeSet<&'static str>> = const { RefCell::new(BTreeSet::new()) };
    }
    fn rule<C: KeyedStateCommitment>() -> &'static str {
        let rule = match check_contract::<C>() {
            Err(Violation { rule, .. }) => rule,
            Ok(()) => "conforming",
        };
        REPORTED.with(|reported| reported.borrow_mut().insert(rule));
        rule
    }
    assert_eq!(rule::<Reference<{ fault::SCHEMA_UNBOUND }>>(), "K1");
    assert_eq!(rule::<Reference<{ fault::HISTORY_DEPENDENT }>>(), "K2");
    assert_eq!(rule::<Reference<{ fault::VALUE_NOT_IN_ROOT }>>(), "K3");
    assert_eq!(rule::<Reference<{ fault::VALUE_UNBOUND }>>(), "K4");
    assert_eq!(rule::<Reference<{ fault::EMPTY_VALUE_IS_ABSENT }>>(), "K5");
    assert_eq!(rule::<Reference<{ fault::RANGE_INCOMPLETE }>>(), "K6");
    assert_eq!(rule::<Reference<{ fault::RANGE_UPPER_INCLUSIVE }>>(), "K6");
    assert_eq!(
        rule::<Reference<{ fault::RANGE_OUTSIDE_CLAIM_IGNORED }>>(),
        "K6"
    );
    assert_eq!(rule::<Reference<{ fault::TABLE_UNBOUND }>>(), "K7");
    assert_eq!(rule::<Reference<{ fault::CELL_KEY_UNCHECKED }>>(), "K8");
    assert_eq!(rule::<Reference<{ fault::TRAILING_BYTES }>>(), "K9");
    assert_eq!(rule::<Reference<{ fault::NON_ATOMIC_APPLY }>>(), "K10");
    assert_eq!(rule::<Reference<{ fault::LARGE_STATE_DRIFT }>>(), "K11");
    assert_eq!(rule::<Reference<{ fault::ROOT_UNBOUND }>>(), "K12");
    REPORTED.with(|reported| {
        assert_eq!(
            *reported.borrow(),
            RULES.iter().copied().collect::<BTreeSet<_>>(),
            "a rule of the specification has no faulty oracle variant"
        );
    });
}

/// The identifiers the suite can report are exactly the rules of the specification's
/// table, in its order: `specs/sumeragi.md` §16.4.
#[test]
fn suite_reports_exactly_the_specified_rules() {
    use std::collections::BTreeSet;

    let source = include_str!("conformance.rs");
    let (head, tail) = source
        .split_once("pub(crate) const RULES")
        .expect("the suite declares its rules");
    let (_, tail) = tail.split_once("];").expect("the rule list ends");
    let mut reported = BTreeSet::new();
    for part in [head, tail] {
        for literal in part.split('"').skip(1).step_by(2) {
            let digits = literal.strip_prefix('K').unwrap_or_default();
            if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) {
                reported.insert(literal);
            }
        }
    }
    assert_eq!(reported, RULES.iter().copied().collect::<BTreeSet<_>>());

    let spec = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/sumeragi.md"),
    )
    .expect("read specs/sumeragi.md");
    let (_, section) = spec
        .split_once("\n### 16.4 Rules every construction satisfies\n")
        .expect("the specification has section 16.4");
    let (section, _) = section
        .split_once("\n### 16.5 ")
        .expect("section 16.5 follows section 16.4");
    let rows: Vec<&str> = section
        .lines()
        .filter_map(|line| line.strip_prefix("| K"))
        .map(|rest| {
            let (digits, requirement) = rest.split_once(" | **").expect("a rule row has a title");
            assert!(digits.bytes().all(|byte| byte.is_ascii_digit()), "{rest}");
            assert!(requirement.ends_with(" |"), "{rest}");
            &rest[..digits.len()]
        })
        .collect();
    let specified: Vec<String> = rows.iter().map(|digits| format!("K{digits}")).collect();
    assert_eq!(specified, RULES);
}

/// The wire contract of a keyed State root field (`specs/sumeragi.md` §16.6): exactly 32
/// raw bytes, every bit preserved, for both values of the `iroha_crypto::Hash` marker
/// bit, and no alternate framing.
#[test]
fn root_payload_is_exactly_32_raw_bytes_without_a_hash_marker() {
    let mut unmarked = [0xa5_u8; ROOT_BYTES];
    unmarked[ROOT_BYTES - 1] = 0xfe;
    let mut marked = unmarked;
    marked[ROOT_BYTES - 1] = 0xff;
    for bytes in [unmarked, marked, [0; ROOT_BYTES], [0xff; ROOT_BYTES]] {
        let root = KeyedStateRoot(bytes);
        assert_eq!(root.payload(), &bytes);
        assert_eq!(KeyedStateRoot::from_payload(root.payload()), Some(root));
    }
    assert_ne!(KeyedStateRoot(unmarked), KeyedStateRoot(marked));
    // A marker-setting `Hash` conversion would change an unmarked root.
    let through_hash = <[u8; ROOT_BYTES]>::from(iroha_crypto::Hash::prehashed(unmarked));
    assert_eq!(through_hash, marked);
    assert_ne!(through_hash, unmarked);

    // Ordinary array serialization frames each element; a byte string adds a length.
    let framed_array = norito::codec::encode_adaptive(&unmarked);
    let framed_bytes = norito::codec::encode_adaptive(&unmarked.to_vec());
    assert_ne!(framed_array.len(), ROOT_BYTES);
    assert_ne!(framed_bytes.len(), ROOT_BYTES);
    let extended = [unmarked.as_slice(), &[0]].concat();
    let alternates: [&[u8]; 5] = [
        framed_array.as_slice(),
        framed_bytes.as_slice(),
        &unmarked[..ROOT_BYTES - 1],
        extended.as_slice(),
        &[],
    ];
    for alternate in alternates {
        assert_eq!(KeyedStateRoot::from_payload(alternate), None);
    }
}

#[test]
fn schema_orders_tables_by_identity_bytes_and_binds_every_descriptor_field() {
    let schema = StateSchema::new(vec![
        TableDescriptor::table("world.b", "norito:K@0.0.2", "norito:V@0.0.2"),
        TableDescriptor::cell("world.a", "semantic:C@0.0.2"),
        TableDescriptor::table("state.z", "norito:K@0.0.2", "norito:V@0.0.2"),
    ])
    .unwrap();
    let ids: Vec<_> = schema.tables().iter().map(TableDescriptor::id).collect();
    assert_eq!(ids, ["state.z", "world.a", "world.b"]);
    assert_eq!(schema.table("world.a").unwrap().0, 1);
    assert_eq!(schema.table("world.a").unwrap().1.shape(), TableShape::Cell);
    assert_eq!(schema.table("world.a").unwrap().1.key_schema(), "");
    assert_eq!(
        schema.table("world.b").unwrap().1.value_schema(),
        "norito:V@0.0.2"
    );
    assert!(schema.table("world").is_none());

    let mut expected = b"iroha:state-keyed-commitment:schema:v1\0".to_vec();
    expected.extend_from_slice(&3_u32.to_le_bytes());
    for (shape, id, key, value) in [
        (0_u8, "state.z", "norito:K@0.0.2", "norito:V@0.0.2"),
        (1, "world.a", "", "semantic:C@0.0.2"),
        (0, "world.b", "norito:K@0.0.2", "norito:V@0.0.2"),
    ] {
        expected.push(shape);
        for text in [id, key, value] {
            expected.extend_from_slice(&u32::try_from(text.len()).unwrap().to_le_bytes());
            expected.extend_from_slice(text.as_bytes());
        }
    }
    assert_eq!(schema.canonical_bytes(), expected);
    assert_eq!(SCHEMA_DOMAIN, b"iroha:state-keyed-commitment:schema:v1\0");
    assert_eq!(ROOT_BYTES, 32);
}

#[test]
fn schema_rejects_ambiguous_or_incomplete_descriptors() {
    let table = |id: &str| TableDescriptor::table(id, "norito:K@0.0.2", "norito:V@0.0.2");
    assert_eq!(
        StateSchema::new(vec![table("")]),
        Err(SchemaError::EmptyIdentity)
    );
    assert_eq!(
        StateSchema::new(vec![
            table("t"),
            TableDescriptor::cell("t", "norito:V@0.0.2")
        ]),
        Err(SchemaError::DuplicateTable("t".into()))
    );
    assert_eq!(
        StateSchema::new(vec![TableDescriptor::table("t", "", "norito:V@0.0.2")]),
        Err(SchemaError::KeyCodec("t".into()))
    );
    assert_eq!(
        StateSchema::new(vec![TableDescriptor::table("t", "norito:K@0.0.2", "")]),
        Err(SchemaError::ValueCodec("t".into()))
    );
    assert_eq!(
        StateSchema::new(vec![TableDescriptor::cell("c", "")]),
        Err(SchemaError::ValueCodec("c".into()))
    );
    assert!(StateSchema::new(Vec::new()).unwrap().tables().is_empty());
}

#[test]
fn statements_are_checked_before_any_commitment_is_consulted() {
    let schema = StateSchema::new(vec![
        TableDescriptor::table("t", "norito:K@0.0.2", "norito:V@0.0.2"),
        TableDescriptor::cell("c", "norito:V@0.0.2"),
    ])
    .unwrap();
    assert_eq!(schema.check_key("t", b"any"), Ok(1));
    assert_eq!(schema.check_key("c", CELL_KEY), Ok(0));
    assert_eq!(schema.check_key("c", b"x"), Err(StatementError::CellKey));
    assert_eq!(
        schema.check_key("missing", b""),
        Err(StatementError::UnknownTable)
    );

    let entry = |key: &'static [u8]| Entry { key, value: b"v" };
    let range = KeyRange::new(b"b", Some(b"d"));
    assert_eq!(
        schema.check_range("t", range, &[entry(b"b"), entry(b"c")]),
        Ok(1)
    );
    assert_eq!(
        schema.check_range("t", range, &[entry(b"d")]),
        Err(StatementError::EntryOutsideRange)
    );
    assert_eq!(
        schema.check_range("t", range, &[entry(b"a")]),
        Err(StatementError::EntryOutsideRange)
    );
    assert_eq!(
        schema.check_range("t", range, &[entry(b"c"), entry(b"b")]),
        Err(StatementError::EntryOrder)
    );
    assert_eq!(
        schema.check_range("t", range, &[entry(b"c"), entry(b"c")]),
        Err(StatementError::EntryOrder)
    );
    assert_eq!(
        schema.check_range("t", KeyRange::new(b"d", Some(b"b")), &[]),
        Err(StatementError::EmptyRange)
    );
    assert_eq!(
        schema.check_range("missing", range, &[]),
        Err(StatementError::UnknownTable)
    );
    assert_eq!(
        schema.check_range("c", KeyRange::FULL, &[entry(CELL_KEY)]),
        Ok(0)
    );
    assert_eq!(
        schema.check_range("c", KeyRange::FULL, &[entry(b"x")]),
        Err(StatementError::CellKey)
    );
}

#[test]
fn key_ranges_are_half_open_byte_intervals() {
    let range = KeyRange::new(b"ab", Some(b"b"));
    assert_eq!(range.validate(), Ok(()));
    assert!(range.contains(b"ab"));
    assert!(range.contains(b"ab\0"));
    assert!(range.contains(b"a\xff"));
    assert!(!range.contains(b"a"));
    assert!(!range.contains(b"b"));
    assert!(KeyRange::FULL.contains(b""));
    assert!(KeyRange::FULL.contains(b"\xff\xff"));
    assert_eq!(KeyRange::FULL.validate(), Ok(()));
    assert_eq!(
        KeyRange::new(b"a", Some(b"a")).validate(),
        Err(StatementError::EmptyRange)
    );
    assert_eq!(
        KeyRange::new(b"", Some(b"")).validate(),
        Err(StatementError::EmptyRange)
    );
    // Little-endian integers do not sort numerically: the commitment order is byte order.
    let one = norito::codec::encode_adaptive(&1_u64);
    let two_fifty_six = norito::codec::encode_adaptive(&256_u64);
    assert!(two_fifty_six < one);
}

/// A tuple key frames each component, so a component prefix is the framed bytes the key
/// encoding emits, never the bare encoding of the component.
#[test]
fn tuple_key_prefixes_include_the_norito_field_framing() {
    let encode = |first: u64, second: u64| norito::codec::encode_adaptive(&(first, second));
    let (same, sibling, other) = (encode(7, 9), encode(7, 10), encode(8, 9));
    let bare = norito::codec::encode_adaptive(&7_u64);
    // Both components are framed alike, so the first one is the leading half.
    let prefix = &same[..same.len() / 2];
    assert!(prefix.len() > bare.len() && prefix.ends_with(&bare));
    assert!(
        !same.starts_with(&bare),
        "the bare component is not the prefix"
    );
    assert!(sibling.starts_with(prefix) && !other.starts_with(prefix));
    let upper = prefix_upper_bound(prefix).unwrap();
    let range = KeyRange::new(prefix, Some(&upper));
    assert!(range.contains(&same) && range.contains(&sibling));
    assert!(!range.contains(&other));
}

#[test]
fn prefix_upper_bound_covers_exactly_the_keys_with_that_prefix() {
    assert_eq!(prefix_upper_bound(b"a"), Some(b"b".to_vec()));
    assert_eq!(prefix_upper_bound(b"ab\xff"), Some(b"ac".to_vec()));
    assert_eq!(prefix_upper_bound(b"\xff\xff"), None);
    assert_eq!(prefix_upper_bound(b""), None);
    let prefix = b"a\xff";
    let upper = prefix_upper_bound(prefix).unwrap();
    let range = KeyRange::new(prefix, Some(&upper));
    assert_eq!(range.validate(), Ok(()));
    for key in [&b"a\xff"[..], b"a\xff\0", b"a\xff\xff\xff"] {
        assert!(range.contains(key), "{key:?}");
    }
    for key in [&b"a\xfe\xff"[..], b"a", b"b", b"b\0"] {
        assert!(!range.contains(key), "{key:?}");
    }
}

#[test]
fn registry_schema_commits_every_canonical_table_and_cell_once() {
    fn count(fields: &'static [Field], tables: &mut usize, cells: &mut usize) {
        for field in fields {
            match field.role {
                Role::Canonical(Canonical::Table { .. }) => *tables += 1,
                Role::Canonical(Canonical::Cell(_)) => *cells += 1,
                Role::Canonical(Canonical::Owner(children)) => count(children, tables, cells),
                Role::Derived { .. } | Role::History { .. } | Role::Local(_) => {}
            }
        }
    }
    let (mut tables, mut cells) = (0, 0);
    count(STATE_FIELDS, &mut tables, &mut cells);
    let schema = StateSchema::from_registry(STATE_FIELDS).unwrap();
    assert_eq!(schema.tables().len(), tables + cells);
    assert_eq!(
        schema
            .tables()
            .iter()
            .filter(|table| table.shape() == TableShape::Table)
            .count(),
        tables
    );
    assert!(
        schema
            .tables()
            .windows(2)
            .all(|pair| pair[0].id().as_bytes() < pair[1].id().as_bytes())
    );
    let layout = format!(
        "@{}.{}.{}",
        V1_LAYOUT.major, V1_LAYOUT.minor, V1_LAYOUT.flags
    );
    for table in schema.tables() {
        assert!(table.value_schema().ends_with(&layout), "{}", table.id());
        assert_eq!(
            table.key_schema().is_empty(),
            table.shape() == TableShape::Cell,
            "{}",
            table.id()
        );
    }
    let (_, accounts) = schema.table("world.accounts").unwrap();
    assert_eq!(accounts.shape(), TableShape::Table);
    assert!(accounts.key_schema().starts_with("norito:"));
    assert!(accounts.key_schema().contains("AccountId"));
    let (_, executor) = schema.table("world.executor").unwrap();
    assert_eq!(executor.shape(), TableShape::Cell);
    assert_eq!(
        executor.value_schema(),
        format!("semantic:iroha:state:executor-semantic:v1{layout}")
    );
    // State-level, runtime and trigger owners are part of the one committed schema.
    for id in [
        "state.transactions.current",
        "state.commit_topology",
        "runtime.lanes",
        "triggers.data",
    ] {
        assert!(schema.table(id).is_some(), "{id}");
    }
    // Owners, derived indexes, history and local fields are not committed entries.
    for id in [
        "state.world",
        "world.domains_by_owner",
        "world.state_accumulator",
        "state.block_hashes",
        "state.kura",
        "world.external_event_buf",
    ] {
        assert!(schema.table(id).is_none(), "{id}");
    }
    // The canonical descriptor is deterministic.
    assert_eq!(
        schema.canonical_bytes(),
        StateSchema::from_registry(STATE_FIELDS)
            .unwrap()
            .canonical_bytes()
    );
}

#[test]
fn registry_schema_refuses_an_unresolved_projection() {
    const UNRESOLVED: &[Field] = &[Field::new(
        "test.required",
        Role::Canonical(Canonical::Cell(Schema::Required {
            identity: "iroha:state:test-required:v1",
            obligation: "TODO: implement the canonical projection",
        })),
    )];
    assert_eq!(
        StateSchema::from_registry(UNRESOLVED),
        Err(SchemaError::Unresolved("test.required"))
    );
    assert_eq!(
        codec_identity("test.u64", schema::<u64>()).unwrap(),
        format!(
            "norito:{}@{}.{}.{}",
            norito::schema::identity::nominal_name::<u64>(),
            V1_LAYOUT.major,
            V1_LAYOUT.minor,
            V1_LAYOUT.flags
        )
    );
    const REPEATED: &[Field] = &[
        Field::new(
            "test.same",
            Role::Canonical(Canonical::Cell(schema::<u64>())),
        ),
        Field::new(
            "test.same",
            Role::Canonical(Canonical::Cell(schema::<u64>())),
        ),
    ];
    assert_eq!(
        StateSchema::from_registry(REPEATED),
        Err(SchemaError::DuplicateTable("test.same".into()))
    );
}

/// Actual World rows under the registry schema and the per-table encodings of the
/// contract: bare Norito V1 key and value bytes, cells at the empty key.
#[test]
fn actual_world_rows_verify_under_the_registry_schema_and_canonical_encodings() {
    use crate::{kura::Kura, query::store::LiveQueryStore, state::State, state::World};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        Registrable,
        account::{Account, AccountId, rekey::AccountAlias},
    };
    use iroha_model_base::topology::DataSpaceId;
    use mv::storage::StorageReadOnly;

    const ALIASES: &str = "world.account_aliases";
    const WATERMARK: &str = "world.soracloud_sequence_watermark";
    let account = |seed: &[u8]| {
        AccountId::new(
            KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    };
    let alias = |label: &str| {
        AccountAlias::domainless(label.parse().expect("alias label"), DataSpaceId::UNIVERSAL)
    };
    let (first, second) = (account(b"keyed-first"), account(b"keyed-second"));
    let mut world = World::with(
        [],
        [
            Account::new(first.clone()).build(&first),
            Account::new(second.clone()).build(&second),
        ],
        [],
    );
    for (label, owner) in [("first", &first), ("second", &second), ("third", &first)] {
        world.account_aliases.insert(alias(label), owner.clone());
    }
    world.rebuild_account_alias_index().unwrap();
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );

    let schema = StateSchema::from_registry(STATE_FIELDS).unwrap();
    let view = state.world.account_aliases.view();
    let mut rows: Vec<(Vec<u8>, Vec<u8>)> = view
        .iter()
        .map(|(key, value)| {
            (
                norito::codec::encode_adaptive(key),
                norito::codec::encode_adaptive(value),
            )
        })
        .collect();
    drop(view);
    // The commitment order is the byte order of the canonical keys.
    rows.sort();
    assert_eq!(rows.len(), 3);
    let watermark =
        norito::codec::encode_adaptive(&*state.world.soracloud_sequence_watermark.view());
    let mut changes: Vec<Change<'_>> = rows
        .iter()
        .map(|(key, value)| Change {
            table: ALIASES,
            key,
            value: Some(value),
        })
        .collect();
    changes.push(Change {
        table: WATERMARK,
        key: CELL_KEY,
        value: Some(&watermark),
    });
    let mut commitment = Reference::<{ fault::NONE }>::empty(&schema);
    commitment.apply(&changes).unwrap();
    let root = commitment.root();
    assert_eq!(commitment.entries(), 4);

    let key = norito::codec::encode_adaptive(&alias("second"));
    let value = norito::codec::encode_adaptive(&second);
    let included = commitment.prove_inclusion(ALIASES, &key).unwrap();
    assert_eq!(
        Reference::<{ fault::NONE }>::verify_inclusion(
            &schema, &root, ALIASES, &key, &value, &included
        ),
        Ok(())
    );
    let other = norito::codec::encode_adaptive(&first);
    assert_eq!(
        Reference::<{ fault::NONE }>::verify_inclusion(
            &schema, &root, ALIASES, &key, &other, &included
        ),
        Err(Rejection::NotProven)
    );
    let missing = norito::codec::encode_adaptive(&alias("missing"));
    let absent = commitment.prove_absence(ALIASES, &missing).unwrap();
    assert_eq!(
        Reference::<{ fault::NONE }>::verify_absence(&schema, &root, ALIASES, &missing, &absent),
        Ok(())
    );
    let entries: Vec<Entry<'_>> = rows
        .iter()
        .map(|(key, value)| Entry { key, value })
        .collect();
    let complete = commitment.prove_range(ALIASES, KeyRange::FULL).unwrap();
    assert_eq!(
        Reference::<{ fault::NONE }>::verify_range(
            &schema,
            &root,
            ALIASES,
            KeyRange::FULL,
            &entries,
            &complete
        ),
        Ok(())
    );
    assert_eq!(
        Reference::<{ fault::NONE }>::verify_range(
            &schema,
            &root,
            ALIASES,
            KeyRange::FULL,
            &entries[1..],
            &complete
        ),
        Err(Rejection::NotProven),
        "an omitted authoritative row"
    );
    let cell = commitment.prove_inclusion(WATERMARK, CELL_KEY).unwrap();
    assert_eq!(
        Reference::<{ fault::NONE }>::verify_inclusion(
            &schema, &root, WATERMARK, CELL_KEY, &watermark, &cell
        ),
        Ok(())
    );
    assert_eq!(
        commitment.prove_inclusion(WATERMARK, b"not-the-cell-key"),
        Err(ProveError::Statement(StatementError::CellKey))
    );
    assert_eq!(
        commitment.prove_inclusion("world.account_aliases_by_account", &key),
        Err(ProveError::Statement(StatementError::UnknownTable)),
        "a derived index is not a committed table"
    );
}

#[test]
fn local_failures_are_distinct_from_verdicts() {
    let update = UpdateError::Local(LocalFailure::Resources);
    let prove = ProveError::Local(LocalFailure::Storage);
    assert!(update.to_string().contains("locally"));
    assert!(prove.to_string().contains("locally"));
    assert_ne!(update, UpdateError::Statement(StatementError::UnknownTable));
    assert_eq!(
        ProveError::from(StatementError::CellKey),
        ProveError::Statement(StatementError::CellKey)
    );
    assert_eq!(
        Rejection::from(StatementError::EmptyRange),
        Rejection::Statement(StatementError::EmptyRange)
    );
    assert_eq!(Witness::new(vec![1, 2]).as_bytes(), [1, 2]);
}
