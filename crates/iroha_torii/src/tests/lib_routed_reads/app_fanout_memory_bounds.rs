// Focused generic application-API fanout memory bounds.

#[derive(
    Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
struct ToriiAppFanoutNoritoTestDto {
    nested: Vec<Vec<Vec<u8>>>,
}

impl torii_app_fanout_norito_dto_sealed::Sealed for ToriiAppFanoutNoritoTestDto {}
impl ToriiAppFanoutNoritoDto for ToriiAppFanoutNoritoTestDto {}

fn generous_json_limits() -> ToriiFanoutJsonLimits {
    ToriiFanoutJsonLimits {
        raw_bytes: 1 << 20,
        encoded_string_bytes: 1 << 20,
        decoded_string_bytes: 1 << 20,
        values: 1 << 16,
        array_entries: 1 << 16,
        object_entries: 1 << 16,
        nesting_depth: norito::json::MAX_JSON_VALUE_NESTING_DEPTH,
        decoded_graph_bytes: 64 << 20,
    }
}

#[test]
fn app_fanout_json_preflight_counts_tiny_token_amplification() {
    let body = b"[0,0,0,0,0]";
    let mut limits = generous_json_limits();
    limits.values = 6;
    limits.array_entries = 5;
    let profile = preflight_torii_fanout_json(body, limits)
        .expect("exact tiny-token value and array-entry bounds must fit");
    assert_eq!(profile.values, 6);
    assert_eq!(profile.array_entries, 5);
    assert!(profile.decoded_graph_bytes > body.len());

    limits.values = 5;
    assert!(matches!(
        preflight_torii_fanout_json(body, limits),
        Err(ToriiAppFanoutMemoryError {
            resource: ToriiAppFanoutResource::Values,
            attempted: 6,
            limit: 5,
            ..
        })
    ));

    limits.values = 6;
    limits.array_entries = 4;
    assert!(matches!(
        preflight_torii_fanout_json(body, limits),
        Err(ToriiAppFanoutMemoryError {
            resource: ToriiAppFanoutResource::ArrayEntries,
            attempted: 5,
            limit: 4,
            ..
        })
    ));
}

#[test]
fn app_fanout_json_preflight_counts_object_entries_and_depth_exactly() {
    let body = br#"{"a":{"b":0},"c":1}"#;
    let mut limits = generous_json_limits();
    limits.object_entries = 3;
    limits.nesting_depth = 3;
    let profile = preflight_torii_fanout_json(body, limits)
        .expect("exact object-entry and nesting bounds must fit");
    assert_eq!(profile.objects, 2);
    assert_eq!(profile.object_entries, 3);
    assert_eq!(profile.max_nesting_depth, 3);

    limits.object_entries = 2;
    assert!(matches!(
        preflight_torii_fanout_json(body, limits),
        Err(ToriiAppFanoutMemoryError {
            resource: ToriiAppFanoutResource::ObjectEntries,
            attempted: 3,
            limit: 2,
            ..
        })
    ));

    limits.object_entries = 3;
    limits.nesting_depth = 2;
    assert!(matches!(
        preflight_torii_fanout_json(body, limits),
        Err(ToriiAppFanoutMemoryError {
            resource: ToriiAppFanoutResource::NestingDepth,
            attempted: 3,
            limit: 2,
            ..
        })
    ));
}

#[test]
fn app_fanout_json_preflight_charges_encoded_escaped_string_capacity() {
    let body = br#"{"k":"\u0041\n"}"#;
    let mut limits = generous_json_limits();
    let profile =
        preflight_torii_fanout_json(body, limits).expect("valid escaped strings must preflight");
    assert_eq!(profile.decoded_string_bytes, 3, "key plus decoded value");
    assert_eq!(profile.encoded_string_bytes, 13, "both quoted tokens");
    assert!(
        profile.string_capacity_bytes >= 33,
        "the one-byte key plus escaped string's geometric capacity must be charged"
    );
    assert!(profile.max_escaped_string_capacity_bytes >= 32);

    limits.encoded_string_bytes = 13;
    preflight_torii_fanout_json(body, limits)
        .expect("exact aggregate encoded string-token limit must fit");
    limits.encoded_string_bytes = 12;
    assert!(matches!(
        preflight_torii_fanout_json(body, limits),
        Err(ToriiAppFanoutMemoryError {
            resource: ToriiAppFanoutResource::EncodedStringBytes,
            attempted: 13,
            limit: 12,
            ..
        })
    ));

    limits.encoded_string_bytes = 13;
    limits.decoded_string_bytes = 3;
    preflight_torii_fanout_json(body, limits)
        .expect("exact aggregate decoded string-byte limit must fit");
    limits.decoded_string_bytes = 2;
    assert!(matches!(
        preflight_torii_fanout_json(body, limits),
        Err(ToriiAppFanoutMemoryError {
            resource: ToriiAppFanoutResource::DecodedStringBytes,
            attempted: 3,
            limit: 2,
            ..
        })
    ));
}

#[test]
fn app_fanout_json_preflight_rejects_invalid_input_before_value_decode() {
    for body in [
        &b"[0,]"[..],
        &b"{\"a\":1,}"[..],
        &b"\"\\x\""[..],
        &b"01"[..],
        &b"1."[..],
        &b"1e+"[..],
        &b"true false"[..],
        &b"{a:0}"[..],
    ] {
        assert!(
            preflight_torii_fanout_json(body, generous_json_limits()).is_err(),
            "invalid JSON unexpectedly passed: {:?}",
            String::from_utf8_lossy(body)
        );
    }
    assert!(matches!(
        preflight_torii_fanout_json(&[b'"', 0xff, b'"'], generous_json_limits()),
        Err(ToriiAppFanoutMemoryError {
            detail: "invalid UTF-8",
            ..
        })
    ));
}

#[test]
fn app_fanout_decoder_diagnostics_do_not_reflect_hostile_payloads() {
    let hostile_key = "private-key-material".repeat(64);
    let body = format!(r#"{{"{hostile_key}":0,"{hostile_key}":1}}"#);
    let profile = preflight_torii_fanout_json(body.as_bytes(), generous_json_limits())
        .expect("duplicate detection is deferred only after its full graph is admitted");
    assert_eq!(profile.object_entries, 2);
    assert_eq!(profile.values, 3);
    assert_eq!(profile.decoded_string_bytes, hostile_key.len() * 2);
    let json_error = norito::json::from_slice::<norito::json::Value>(body.as_bytes())
        .expect_err("duplicate key must be rejected by the real parser");
    let sanitized = sanitize_torii_app_fanout_json_error(json_error).to_string();
    assert_eq!(sanitized, "proxied JSON response failed bounded decoding");
    assert!(!sanitized.contains("private-key-material"));

    let norito_error = norito::decode_from_bytes_with_limits::<Vec<u8>>(
        b"hostile-norito-body",
        norito::DecodeLimits::new(1, 1, 1, 1, 1),
    )
    .expect_err("invalid Norito frame must fail");
    let sanitized = sanitize_torii_app_fanout_norito_error(norito_error).to_string();
    assert_eq!(sanitized, "proxied Norito response failed bounded decoding");
    assert!(!sanitized.contains("hostile-norito-body"));
}

#[test]
fn app_fanout_norito_default_layout_decodes_once_under_explicit_limits() {
    let budget = ToriiAppFanoutMemoryBudget::new(1024 * 1024).expect("test budget");
    let expected = ToriiAppFanoutNoritoTestDto {
        nested: vec![vec![vec![1_u8, 2, 3]]],
    };
    let bytes = norito::to_bytes(&expected).expect("encode default-layout test DTO");
    let header =
        norito::core::Header::read(std::io::Cursor::new(&bytes)).expect("read test DTO header");
    assert_eq!(header.compression, norito::Compression::None);
    assert_eq!(header.flags, norito::default_encode_flags());
    let plan = budget
        .norito_decode_plan::<ToriiAppFanoutNoritoTestDto>(bytes.len(), 1, usize::MAX)
        .expect("default-layout raw frame fits");
    let decoded = decode_torii_app_fanout_norito::<ToriiAppFanoutNoritoTestDto>(&bytes, plan)
        .expect("sealed DTO decodes sequentially under explicit limits");
    assert_eq!(decoded, expected);
}

#[test]
fn app_fanout_json_raw_and_decode_graph_overlap_has_exact_boundary() {
    let body = b"[0]";
    let mut limits = generous_json_limits();
    limits.raw_bytes = body.len();
    let profile = preflight_torii_fanout_json(body, limits).expect("small JSON must preflight");
    limits.raw_bytes = body.len() - 1;
    assert!(matches!(
        preflight_torii_fanout_json(body, limits),
        Err(ToriiAppFanoutMemoryError {
            resource: ToriiAppFanoutResource::RawBytes,
            attempted: 3,
            limit: 2,
            ..
        })
    ));
    let peak = profile
        .decode_peak_bytes()
        .expect("profile peak must fit usize");
    let exact = ToriiAppFanoutMemoryBudget::new(peak).expect("non-zero budget");
    exact
        .admit_json_decode(profile)
        .expect("raw plus decode graph must fit exact boundary");

    let short = ToriiAppFanoutMemoryBudget::new(peak - 1).expect("non-zero budget");
    assert!(matches!(
        short.admit_json_decode(profile),
        Err(ToriiAppFanoutMemoryError {
            resource: ToriiAppFanoutResource::WorkingSetBytes,
            attempted,
            limit,
            ..
        }) if attempted == peak && limit == peak - 1
    ));
}

#[test]
fn app_fanout_aggregate_merge_candidate_and_final_phases_are_bounded() {
    let mut budget = ToriiAppFanoutMemoryBudget::new(100).expect("test budget");
    budget.retain(40).expect("decoded route payloads");
    budget
        .admit_temporary(60)
        .expect("merge graph may overlap decoded inputs at the exact bound");
    assert!(budget.admit_temporary(61).is_err());

    budget
        .retain(60)
        .expect("retain merged graph after admission");
    budget.release(40).expect("release decoded route payloads");
    assert_eq!(budget.retained_bytes(), 60);
    budget
        .admit_temporary(40)
        .expect("candidate key plus final response fit the exact remainder");
    assert!(budget.admit_temporary(41).is_err());
    assert!(budget.release(61).is_err());
    assert_eq!(
        budget.retained_bytes(),
        60,
        "an invalid release must leave the ledger unchanged"
    );
}

#[test]
fn app_fanout_budget_borrows_the_existing_shared_reservation() {
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
    let permit = semaphore
        .clone()
        .try_acquire_owned()
        .expect("test owns the shared fanout slot");
    let reservation = QueryFanoutMemoryReservation::new(permit);
    let budget =
        ToriiAppFanoutMemoryBudget::from_shared_query_fanout_reservation(&reservation, 8 * 1024)
            .expect("shared reservation creates a request ledger");
    assert_eq!(budget.remaining_bytes().expect("valid ledger"), 8 * 1024);
    assert_eq!(semaphore.available_permits(), 0);

    drop(budget);
    assert_eq!(
        semaphore.available_permits(),
        0,
        "the ledger borrows rather than replacing the shared ownership token"
    );
    drop(reservation);
    assert_eq!(semaphore.available_permits(), 1);
}

#[test]
fn app_fanout_norito_plan_keeps_raw_bytes_live_and_splits_routes() {
    let capacity = 64 * 1024;
    let prior = 4 * 1024;
    let raw = 4 * 1024;
    let mut budget = ToriiAppFanoutMemoryBudget::new(capacity).expect("test budget");
    budget.retain(prior).expect("prior decoded route");
    let plan = budget
        .norito_decode_plan::<ToriiAppFanoutNoritoTestDto>(raw, 2, usize::MAX)
        .expect("raw body plus split allocation should fit");
    let route_slice = (capacity - prior - raw) / 2;
    let variable = route_slice - TORII_APP_FANOUT_NORITO_FIXED_BYTES;
    let tracked_physical = variable / 2;
    let tracked_logical = tracked_physical / TORII_APP_FANOUT_NORITO_TRACKED_ALLOCATION_FACTOR;
    let max_elements =
        (variable - tracked_physical) / TORII_APP_FANOUT_NORITO_UNTRACKED_ELEMENT_BYTES;
    assert_eq!(plan.retained_charge_bytes, route_slice);
    assert_eq!(plan.temporary_charge_bytes, route_slice);
    assert_eq!(plan.limits.max_sequence_elements(), max_elements);
    assert_eq!(plan.limits.max_field_bytes(), tracked_logical);
    assert_eq!(plan.limits.max_total_elements(), max_elements);
    assert_eq!(plan.limits.max_total_allocated_bytes(), tracked_logical);
    assert_eq!(
        plan.limits.max_nesting_depth(),
        norito::core::MAX_OWNED_VALUE_DECODE_DEPTH
    );
    budget
        .admit_temporary(raw + plan.temporary_charge_bytes)
        .expect("raw and decoder allocation overlap at plan high water");
    budget
        .retain_norito_decode(plan)
        .expect("successful decoded graph retains its complete allocation slice");
    assert_eq!(budget.retained_bytes(), prior + route_slice);
}

#[test]
fn app_fanout_norito_limits_reject_nested_and_packed_amplification() {
    let budget = ToriiAppFanoutMemoryBudget::new(1024 * 1024).expect("test budget");
    let nested = ToriiAppFanoutNoritoTestDto {
        nested: vec![vec![vec![1_u8]]],
    };
    let nested_bytes = norito::to_bytes(&nested).expect("encode nested sequence");
    let nested_plan = budget
        .norito_decode_plan::<ToriiAppFanoutNoritoTestDto>(nested_bytes.len(), 1, 1)
        .expect("nested raw frame fits");
    let error =
        decode_torii_app_fanout_norito::<ToriiAppFanoutNoritoTestDto>(&nested_bytes, nested_plan)
            .expect_err("nested DTO must obey the explicit depth limit");
    assert_eq!(
        error.to_string(),
        "proxied Norito response failed bounded decoding"
    );

    let mut packed_bytes = norito::to_bytes(&nested).expect("encode test DTO");
    packed_bytes[norito::core::Header::SIZE - 1] |= norito::core::header_flags::PACKED_SEQ;
    let packed_plan = budget
        .norito_decode_plan::<ToriiAppFanoutNoritoTestDto>(packed_bytes.len(), 1, 8)
        .expect("packed raw frame fits");
    let error =
        decode_torii_app_fanout_norito::<ToriiAppFanoutNoritoTestDto>(&packed_bytes, packed_plan)
            .expect_err("non-default packed layout must fail before sequential decode");
    assert_eq!(
        error.to_string(),
        "proxied Norito response failed bounded decoding"
    );
}

#[test]
fn app_fanout_norito_limits_reject_hostile_declared_lengths_before_allocation() {
    let declared = 1_u64 << 30;
    let error =
        norito::with_decode_limits(norito::DecodeLimits::new(16, 1024, 16, 1024, 8), || {
            norito::core::read_seq_len_slice(&declared.to_le_bytes()).map(|_| ())
        })
        .expect_err("hostile declared sequence length must fail closed");
    assert!(matches!(
        error,
        norito::Error::SequenceLengthExceeded {
            length,
            limit: 16,
        } if length == declared
    ));
}
