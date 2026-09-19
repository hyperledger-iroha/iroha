// Actual native Decisions, complete common execution and genuine durable finality.

fn native_publication_carrier_for_test(
    fixture: &NativeEconomicFixture,
) -> (SignedBlock, HeightContext) {
    let carrier = native_consumer_stage_carrier(fixture);
    let parent = fixture
        .native
        .state
        .kura
        .v2_finality_artifact(fixture.native.block.header().height().get())
        .expect("read authenticated applying parent")
        .expect("fixture parent has genuine finality");
    let context = crate::sumeragi::v2_context::build_successor_height_context(
        &parent,
        crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(&fixture.native.state)
            .expect("derive Native applying policy from its exact committed predecessor"),
        None,
    )
    .expect("derive exact native applying authority");
    (carrier, context)
}

fn native_publication_fixture_for_test(
    cases: &[NativeEconomicCase],
) -> (Box<NativeEconomicFixture>, SignedBlock, HeightContext) {
    let fixture = native_economic_fixture_with_genesis_layout(
        cases,
        false,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
    );
    let (carrier, context) = native_publication_carrier_for_test(&fixture);
    (fixture, carrier, context)
}

fn prepared_native_publication_for_test<'state>(
    state: &'state State,
    carrier: &SignedBlock,
    context: HeightContext,
) -> (Box<StateBlock<'state>>, crate::block::CommittedBlock) {
    let mut executed = carrier.clone();
    let mut overlay =
        ValidBlock::execute_native_block_and_capture_for_test(&mut executed, state, &context)
            .expect("execute exact native Decisions and capture the complete common witness");
    let (committed, witness) =
        finalize_native_execution_for_test(state, executed, &mut overlay, context);
    overlay
        .authorize_execution_output_publication(&committed, &witness)
        .expect("bind the actual witness to exact durable finality");
    overlay
        .apply_without_execution_with_verified_v2_finality(&committed)
        .expect("consume authorization and prepare exact carrier metadata");
    assert!(
        overlay
            .native_output_publication_identity()
            .unwrap()
            .is_some()
    );
    assert!(matches!(
        overlay.execution_output_plan,
        Some(super::output_capacity::ExecutionOutputPlanState::Finalized(
            _
        ))
    ));
    (overlay, committed)
}

// Keep restore failures actionable without dumping the complete State or
// changing the exact canonical commitment asserted by the callers.
fn canonical_native_restore_difference_for_test(before: &State, after: &State) -> String {
    fn collect(
        path: &str,
        before: &json::Value,
        after: &json::Value,
        differences: &mut Vec<String>,
    ) {
        if before == after || differences.len() == 12 {
            return;
        }
        if let (Some(left), Some(right)) = (before.as_object(), after.as_object()) {
            let keys = left.keys().chain(right.keys()).collect::<BTreeSet<_>>();
            for key in keys {
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        collect(&format!("{path}.{key}"), left, right, differences)
                    }
                    _ if differences.len() < 12 => {
                        differences.push(format!("{path}.{key}: field presence changed"))
                    }
                    _ => {}
                }
            }
        } else if let (Some(left), Some(right)) = (before.as_array(), after.as_array()) {
            if left.len() != right.len() {
                differences.push(format!("{path}: length {} -> {}", left.len(), right.len()));
            }
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                collect(&format!("{path}[{index}]"), left, right, differences);
            }
        } else {
            let brief = |value: &json::Value| {
                json::to_json(value)
                    .unwrap()
                    .chars()
                    .take(240)
                    .collect::<String>()
            };
            differences.push(format!("{path}: {} -> {}", brief(before), brief(after)));
        }
    }
    let before: json::Value =
        json::from_slice(&crate::snapshot::canonical_state_snapshot_bytes(before)).unwrap();
    let after: json::Value =
        json::from_slice(&crate::snapshot::canonical_state_snapshot_bytes(after)).unwrap();
    let mut differences = Vec::new();
    collect("state", &before, &after, &mut differences);
    differences.join("\n")
}
