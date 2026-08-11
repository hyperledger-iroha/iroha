// Focused phase-ledger tests for application routed reads.

fn routed_read_working_set_for_phase(phase_bytes: usize) -> usize {
    query_fanout_fixed_overhead_bytes()
        .expect("fixed fanout overhead fits")
        .checked_add(
            phase_bytes
                .checked_mul(QUERY_FANOUT_PREBODY_UNITS)
                .expect("phase geometry fits"),
        )
        .expect("test working set fits")
}

#[test]
fn routed_read_body_limits_preserve_configured_and_envelope_caps() {
    let phase = 256 * 1024;
    let working_set = routed_read_working_set_for_phase(phase);
    let memory_limited = ToriiRoutedReadMemoryBudget::new(working_set, phase * 4)
        .expect("working set admits app routed reads");
    assert_eq!(memory_limited.route_body_limit(), phase);
    assert_eq!(memory_limited.final_body_limit(), phase * 2);

    let configured = ToriiRoutedReadMemoryBudget::new(working_set, phase / 2)
        .expect("working set admits app routed reads");
    assert_eq!(configured.route_body_limit(), phase / 2);
    assert_eq!(configured.final_body_limit(), phase / 2);
}

#[test]
fn routed_read_json_retains_measured_graph_and_exact_canonical_bytes() {
    let phase = 256 * 1024;
    let mut budget =
        ToriiRoutedReadMemoryBudget::new(routed_read_working_set_for_phase(phase), phase)
            .expect("working set admits app routed reads");
    let body = br#"{"items":[0,1,2],"nested":{"name":"alice"}}"#;
    let plan = budget.decode_plan(body.len()).expect("small body fits");
    let profile = budget
        .json_profile(body, plan)
        .expect("valid JSON preflights");
    let (value, usage) = norito::core::with_decode_limits_measured(plan.limits, || {
        norito::json::from_slice::<Value>(body)
    });
    let value = value.expect("valid JSON decodes");
    budget
        .verify_json_value_usage(profile, usage)
        .expect("parser charge covers the graph");
    let canonical = norito::json::to_json_bounded(&value, plan.canonical_limit_bytes)
        .expect("small canonical JSON fits");

    budget
        .retain_decode_usage(usage)
        .expect("measured graph fits retained phase");
    budget
        .retain_canonical_bytes(canonical.len())
        .expect("exact canonical bytes fit candidate phase");
    assert_eq!(budget.retained_decoded_bytes, usage.total_allocated_bytes());
    assert_eq!(budget.retained_canonical_bytes, canonical.len());
}

#[test]
fn routed_read_json_graph_uses_each_objects_exact_conservative_node_count() {
    let phase = 256 * 1024;
    let budget = ToriiRoutedReadMemoryBudget::new(routed_read_working_set_for_phase(phase), phase)
        .expect("working set admits app routed reads");
    let body = br#"{"full":{"a":0,"b":1,"c":2,"d":3,"e":4},"empty":{}}"#;
    let plan = budget.decode_plan(body.len()).expect("small body fits");
    let profile = budget
        .json_profile(body, plan)
        .expect("valid JSON preflights");
    assert_eq!(profile.object_btree_node_upper_bound(), 2);
    let (value, usage) = norito::core::with_decode_limits_measured(plan.limits, || {
        norito::json::from_slice::<Value>(body)
    });
    value.expect("valid JSON decodes");
    budget
        .verify_json_value_usage(profile, usage)
        .expect("exact per-object topology does not falsely reject the graph");
}

#[test]
fn routed_read_request_and_preflight_use_derived_phase_boundaries() {
    let phase = 64 * 1024;
    let budget =
        ToriiRoutedReadMemoryBudget::new(routed_read_working_set_for_phase(phase), phase * 2)
            .expect("working set admits app routed reads");
    budget
        .admit_request_bytes(phase)
        .expect("exact request phase fits");
    assert!(budget.admit_request_bytes(phase + 1).is_err());
    assert!(budget.decode_plan(phase + 1).is_err());

    let malformed = br#"{"unterminated": ["#;
    let plan = budget
        .decode_plan(malformed.len())
        .expect("small malformed body reaches lexical validation");
    assert!(budget.json_profile(malformed, plan).is_err());
}

#[test]
fn routed_read_decode_plan_uses_full_phase_and_retains_measured_usage() {
    let phase = 256 * 1024;
    let mut budget =
        ToriiRoutedReadMemoryBudget::new(routed_read_working_set_for_phase(phase), phase)
            .expect("working set admits app routed reads");
    let plan = budget.decode_plan(1024).expect("route body fits");
    assert_eq!(plan.limits.max_total_allocated_bytes(), phase);
    assert_eq!(plan.limits.max_total_elements(), phase);

    let (charged, usage) = norito::core::with_decode_limits_measured(plan.limits, || {
        norito::core::reserve_decode_allocation(phase / 4)
    });
    charged.expect("test allocation fits decode phase");
    budget
        .retain_decode_usage(usage)
        .expect("measured allocation fits accumulator");
    budget
        .retain_canonical_bytes(plan.canonical_limit_bytes / 4)
        .expect("candidate bytes fit");

    let next = budget.decode_plan(1024).expect("next route still decodes");
    assert_eq!(
        next.limits.max_total_allocated_bytes(),
        phase,
        "a later route receives the full transient phase, not an arbitrary fair split"
    );
    assert!(next.canonical_limit_bytes < plan.canonical_limit_bytes);
}

#[test]
fn routed_read_merge_tracks_only_canonical_keys_it_keeps() {
    let phase = 256 * 1024;
    let mut budget =
        ToriiRoutedReadMemoryBudget::new(routed_read_working_set_for_phase(phase), phase)
            .expect("working set admits app routed reads");
    let value = norito::json::json!({"id": "alice"});

    let candidate = budget
        .canonical_json_candidate(&value)
        .expect("small canonical candidate fits");
    assert_eq!(budget.retained_canonical_bytes, 0);
    budget
        .retain_canonical_bytes(candidate.len())
        .expect("retained key fits");
    assert_eq!(budget.retained_canonical_bytes, candidate.len());

    budget.begin_typed_merge();
    assert_eq!(budget.retained_canonical_bytes, candidate.len());
    budget.begin_json_merge();
    assert_eq!(budget.retained_canonical_bytes, 0);
}

#[test]
fn routed_read_retained_vector_charges_actual_capacity_growth() {
    let phase = 64 * 1024;
    let mut budget =
        ToriiRoutedReadMemoryBudget::new(routed_read_working_set_for_phase(phase), phase)
            .expect("working set admits app routed reads");
    let mut values = budget
        .try_retained_vec::<u64>(1)
        .expect("small retained vector fits");
    budget
        .push_retained(&mut values, 1)
        .expect("reserved slot accepts first value");
    budget
        .push_retained(&mut values, 2)
        .expect("growth remains inside retained phase");

    assert_eq!(values, [1, 2]);
    assert_eq!(
        budget.retained_decoded_bytes,
        values.capacity() * core::mem::size_of::<u64>()
    );
    assert!(budget.try_retained_vec::<u8>(usize::MAX).is_err());
}

#[test]
fn routed_read_merge_vector_charges_its_actual_capacity() {
    let phase = 64 * 1024;
    let mut budget =
        ToriiRoutedReadMemoryBudget::new(routed_read_working_set_for_phase(phase), phase)
            .expect("working set admits app routed reads");

    let values = budget
        .try_merge_vec::<u64>(17)
        .expect("small merge vector fits");
    let allocated_bytes = values
        .capacity()
        .checked_mul(core::mem::size_of::<u64>())
        .expect("test capacity fits");
    assert!(values.capacity() >= 17);
    assert_eq!(budget.merge_allocated_bytes, allocated_bytes);
    assert!(budget.try_merge_vec::<u8>(usize::MAX).is_err());
}

#[test]
fn routed_read_final_json_is_counted_before_allocation() {
    let phase = 64 * 1024;
    let budget = ToriiRoutedReadMemoryBudget::new(routed_read_working_set_for_phase(phase), 32)
        .expect("working set admits app routed reads");
    let oversized = Value::String("x".repeat(64));

    assert!(budget.json_response(&oversized).is_err());
}

#[test]
fn routed_read_source_keeps_multiroute_fanout_enabled() {
    let source = include_str!("../../lib.rs");
    assert!(!source.contains("multi-route application reads are unavailable"));
    for required in [
        "collect_torii_routed_list_json_payloads",
        "execute_torii_read_fanout_for_resolved_routes_admitted",
        "resolve_torii_proof_record_for_supported_routes(app, routes, proof_id).await",
        "collect_torii_alias_json_payloads",
        "collect_torii_alias_lookup_json_payloads",
    ] {
        assert!(
            source.contains(required),
            "bounded multi-route source `{required}` must remain wired"
        );
    }
}
