// Typed byte-vector accounting preserves the snapshot's canonical schema and custody.

fn scan_snapshot_accounting(input: &str) -> SnapshotJsonBudgetScanner<'_> {
    let mut scanner = SnapshotJsonBudgetScanner::new(
        input,
        resource_policy(
            norito::core::MAX_VALUE_NESTING_DEPTH,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ),
    );
    scanner
        .parse_value(1, false, SnapshotJsonContext::Root)
        .unwrap();
    assert_eq!(scanner.cursor, input.len());
    scanner
}

fn byte_store_snapshot(field: &str, value: &str, previous: bool) -> String {
    let storage = if previous {
        format!(r#"{{"revert":{{"key":{{"value":{value}}}}},"blocks":{{}}}}"#)
    } else {
        format!(r#"{{"revert":{{}},"blocks":{{"key":{value}}}}}"#)
    };
    format!(r#"{{"world":{{"{field}":{storage}}}}}"#)
}

#[test]
fn snapshot_byte_accounting_covers_only_exact_typed_paths_and_both_images() {
    for field in [
        "contract_code",
        "contract_code_upload_chunks",
        "smart_contract_state",
        "content_chunks",
    ] {
        for previous in [false, true] {
            let wrap = |bytes: &str| {
                if field == "content_chunks" {
                    format!(r#"{{"data":{bytes},"refcount":1}}"#)
                } else {
                    bytes.to_owned()
                }
            };
            let empty = byte_store_snapshot(field, &wrap("[]"), previous);
            let full = byte_store_snapshot(field, &wrap("[0,1,255]"), previous);
            let empty_scan = scan_snapshot_accounting(&empty);
            let full_scan = scan_snapshot_accounting(&full);
            assert_eq!(
                empty_scan.items, full_scan.items,
                "{field}, undo={previous}"
            );
            assert_eq!(
                full_scan.transient_bytes - empty_scan.transient_bytes,
                4 * (full.len() - empty.len()) + 3 * 3
            );
            let exact = SnapshotResourcePolicy {
                max_decode_items: NonZeroUsize::new(full_scan.items).unwrap(),
                max_transient_bytes: NonZeroUsize::new(full_scan.transient_bytes).unwrap(),
                ..SnapshotResourcePolicy::default()
            };
            validate_snapshot_json_resources(full.as_bytes(), exact).unwrap();
            for policy in [
                SnapshotResourcePolicy {
                    max_decode_items: NonZeroUsize::new(full_scan.items - 1).unwrap(),
                    ..exact
                },
                SnapshotResourcePolicy {
                    max_transient_bytes: NonZeroUsize::new(full_scan.transient_bytes - 1).unwrap(),
                    ..exact
                },
                SnapshotResourcePolicy {
                    max_blob_bytes: nonzero!(2_usize),
                    ..exact
                },
            ] {
                assert!(matches!(
                    validate_snapshot_json_resources(full.as_bytes(), policy),
                    Err(TryReadError::SnapshotResourceLimit(_))
                ));
            }
        }
    }
    // A field name alone, or an unknown storage member, must never confer a discount.
    for payload in [
        r#"{"contract_code":{"blocks":{"key":[0,1,255]}}}"#,
        r#"{"world":{"unclassified":{"blocks":{"key":[0,1,255]}}}}"#,
        r#"{"world":{"contract_code":{"unknown":{"key":[0,1,255]}}}}"#,
        r#"{"world":{"accounts":{"blocks":{"key":{"contract_code":[0,1,255]}}}}}"#,
        r#"{"world":{"content_chunks":{"blocks":{"key":{"unknown":[0,1,255]}}}}}"#,
        r#"{"World":{"contract_code":{"blocks":{"key":[0,1,255]}}}}"#,
    ] {
        let full = scan_snapshot_accounting(payload);
        let empty_payload = payload.replace("[0,1,255]", "[]");
        assert_eq!(
            full.items,
            scan_snapshot_accounting(&empty_payload).items + 3
        );
    }
    validate_snapshot_json_resources(
        br#"{"world":{"contract_code":{"revert":{"key":null},"blocks":{}}}}"#,
        SnapshotResourcePolicy::default(),
    )
    .expect("an absent predecessor remains valid");
    let all_bytes = json::to_json(&(0_u8..=255).collect::<Vec<_>>()).unwrap();
    validate_snapshot_json_resources(
        byte_store_snapshot("contract_code", &all_bytes, false).as_bytes(),
        SnapshotResourcePolicy::default(),
    )
    .expect("every canonical u8 is accepted");
}

#[test]
fn snapshot_byte_accounting_rejects_nonbyte_and_noncanonical_values() {
    for previous in [false, true] {
        for value in [
            "[256]",
            "[-1]",
            "[-0]",
            "[01]",
            "[1.0]",
            "[1e0]",
            "[true]",
            "[null]",
            "[[0]]",
            "[{}]",
            "[ 0]",
            "[0,]",
            "\"bytes\"",
            "null",
            "{}",
        ] {
            let payload = byte_store_snapshot("contract_code", value, previous);
            assert!(
                matches!(
                    validate_snapshot_json_resources(
                        payload.as_bytes(),
                        SnapshotResourcePolicy::default()
                    ),
                    Err(TryReadError::NonCanonicalSnapshotPayload)
                ),
                "accepted {payload}"
            );
        }
    }
    for payload in [
        r#"{"world":{"contract_code":{"revert":{"key":{}}}}}"#,
        r#"{"world":{"contract_code":{"revert":{"key":{"value":[],"value":[]}}}}}"#,
        r#"{"world":{"contract_code":{"revert":{"key":[0]}}}}"#,
        r#"{"world":{"contract_cod\u0065":{"blocks":{"key":[0]}}}}"#,
    ] {
        assert!(
            validate_snapshot_json_resources(payload.as_bytes(), SnapshotResourcePolicy::default())
                .is_err()
        );
    }
    let payload = byte_store_snapshot("contract_code", "[255]", false);
    assert!(matches!(
        validate_snapshot_json_resources(
            payload.as_bytes(),
            SnapshotResourcePolicy {
                max_decode_depth: nonzero!(5_usize),
                ..SnapshotResourcePolicy::default()
            }
        ),
        Err(TryReadError::SnapshotResourceLimit(_))
    ));
}

#[tokio::test]
async fn snapshot_over_ten_million_byte_items_publishes_restarts_and_preserves_undo() {
    let mut state = state_factory();
    // A future World schema change must not silently reuse these discounts for
    // a more expensive element type under the same JSON field name.
    let _: &mv::storage::Storage<_, Vec<u8>> = &state.world.contract_code;
    let _: &mv::storage::Storage<_, Vec<u8>> = &state.world.contract_code_upload_chunks;
    let _: &mv::storage::Storage<_, Vec<u8>> = &state.world.smart_contract_state;
    let _: &mv::storage::Storage<_, iroha_data_model::content::ContentChunk> =
        &state.world.content_chunks;
    let _: Vec<u8> = iroha_data_model::content::ContentChunk::new(Vec::new()).data;
    let key: iroha_model_base::state_path::StatePath = "snapshot/accounting".parse().unwrap();
    // Both MV images are required on restart, including the full predecessor.
    const BYTES: usize = 6 * 1024 * 1024;
    state
        .world
        .smart_contract_state
        .insert(key.clone(), vec![0; BYTES]);
    let mut block = state.world.smart_contract_state.block();
    block.insert(key, vec![255; BYTES]);
    block.commit();
    let before = exact_snapshot_payload_bytes(&state);
    let input = std::str::from_utf8(&before).unwrap();
    let accounting = scan_snapshot_accounting(input);
    let byte_items = 2 * BYTES;
    assert!(byte_items > defaults::snapshot::MAX_DECODE_ITEMS.get());
    // The old scanner treats this identical shape as ordinary arrays. It must
    // still refuse; only the explicit typed paths earn byte accounting.
    let old_shape = input.replacen("\"smart_contract_state\":", "\"unclassified_state\":", 1);
    assert!(
        matches!(validate_snapshot_json_resources(old_shape.as_bytes(), SnapshotResourcePolicy::default()),
        Err(TryReadError::SnapshotResourceLimit(reason)) if reason.contains("aggregate items"))
    );
    drop(old_shape);
    let directory = tempdir().unwrap();
    let signing_key = checked_random_snapshot_keypair();
    try_write_snapshot(
        &state,
        directory.path(),
        &signing_key,
        defaults::snapshot::MERKLE_CHUNK_SIZE_BYTES,
    )
    .unwrap();
    let restored = try_read_snapshot(
        &state.ivm_execution_budget(),
        directory.path(),
        &state.kura_handle(),
        &state.lane_manifests.read().clone(),
        &state.nexus_snapshot(),
        LiveQueryStore::start_test,
        BlockCount(state.view().height()),
        defaults::snapshot::MERKLE_CHUNK_SIZE_BYTES,
        signing_key.public_key(),
        &state.chain_id,
        &state.network_id,
        &crate::state::default_zk_config(),
        #[cfg(feature = "telemetry")]
        StateTelemetry::new(<_>::default(), true),
        &snapshot_read_budget_for_testing(),
    )
    .expect("the published snapshot must pass the ordinary signed restart reader");
    assert_eq!(
        exact_snapshot_payload_bytes(&restored),
        before,
        "every World field and its predecessor survives restart byte-for-byte"
    );
    eprintln!(
        "snapshot-byte-accounting: byte_items={byte_items}, structural_items={}, payload_bytes={}, transient_bytes={}, largest_field_bytes={}",
        accounting.items,
        input.len(),
        accounting.transient_bytes,
        accounting.largest_typed_field_bytes
    );
}

#[test]
fn snapshot_accounting_keeps_release_wire_and_abi_pins() {
    let wire = hex::encode(crate::release_identity::wire_schema_hash());
    let abi = hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1));
    assert!(wire.starts_with("8c6e5380"), "wire={wire}");
    assert!(abi.starts_with("7531b45d"), "abi={abi}");
    eprintln!("snapshot-byte-accounting: wire={wire} abi={abi}");
}

/// Local diagnostic only: no signatures are trusted and no State is installed.
#[test]
#[ignore = "requires SNAPSHOT_CENSUS_PATH pointing to a local snapshot.data"]
fn snapshot_resource_census_local_file() {
    let path = std::env::var_os("SNAPSHOT_CENSUS_PATH").expect("local snapshot path");
    let input = std::fs::read_to_string(path).unwrap();
    let total = scan_snapshot_accounting(&input);
    eprintln!(
        "default_resource_admission={:?}",
        validate_snapshot_json_resources(input.as_bytes(), SnapshotResourcePolicy::default())
    );
    eprintln!(
        "total: structural_items={} payload_bytes={} transient_bytes={} largest_field_bytes={}",
        total.items,
        input.len(),
        total.transient_bytes,
        total.largest_typed_field_bytes
    );
    let world = snapshot_object_field_raw(&input, "world").unwrap().unwrap();
    let mut rows = Vec::new();
    for field in borrowed_json_object_members(world).unwrap() {
        let wrapped = format!(r#"{{"world":{{{}:{}}}}}"#, field.encoded_key, field.value);
        let current = scan_snapshot_accounting(&wrapped);
        let ordinary = wrapped.replacen("\"world\":", "\"ordinary\":", 1);
        let old = scan_snapshot_accounting(&ordinary);
        rows.push((
            old.items - 1,
            field.key,
            current.items - 1,
            old.items - current.items,
            field.value.len(),
        ));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    eprintln!("old_items\tfield\tstructural_items\tbyte_items\tjson_bytes");
    for (old, name, items, bytes, encoded) in rows {
        eprintln!("{old}\t{name}\t{items}\t{bytes}\t{encoded}");
    }
}
