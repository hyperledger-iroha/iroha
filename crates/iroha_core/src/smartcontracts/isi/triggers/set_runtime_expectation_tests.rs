// Runtime-consent snapshot regressions, included in the trigger-set DTO test module.

fn runtime_snapshot_set() -> (Set, Hash) {
    let state = crate::state::State::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    let header = dm::BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, 0, 0);
    let mut state_block = state.block(header);
    let transaction = state_block.transaction();
    let executable = dm::Executable::Instructions(ConstVec::new_empty());
    let expectation =
        crate::runtime::expectation::capture_executable(&executable, &transaction.world).unwrap();
    let digest =
        crate::runtime::expectation::digest_owned(&expectation, &transaction.world).unwrap();
    let authority = iroha_test_samples::ALICE_ID.clone();
    let mut metadata = global_data_trigger_scope_metadata_for_testing(&authority);
    metadata.insert(
        super::super::runtime_expectation::metadata_key().clone(),
        Json::new(digest),
    );
    let set = Set::default();
    {
        let mut block = set.block();
        let mut tx = block.transaction();
        macro_rules! add {
            ($method:ident, $id:literal, $filter:expr) => {{
                let mut action = SpecializedAction::new(
                    executable.clone(),
                    dm::Repeats::Indefinitely,
                    authority.clone(),
                    $filter,
                )
                .unwrap();
                action.metadata = metadata.clone();
                tx.$method(SpecializedTrigger::new($id.parse().unwrap(), action))
                    .unwrap();
            }};
        }
        add!(add_data_trigger, "runtime_data", dm::DataEventFilter::Any);
        add!(
            add_pipeline_trigger,
            "runtime_pipeline",
            dm::PipelineEventFilterBox::from(pipeline::BlockEventFilter::new())
        );
        add!(
            add_time_trigger,
            "runtime_time",
            dm::TimeEventFilter(ExecutionTime::PreCommit)
        );
        add!(
            add_by_call_trigger,
            "runtime_call",
            dm::ExecuteTriggerEventFilter::new()
        );
        tx.apply();
        block.commit();
    }
    // Retain real predecessor actions as well as current actions in the snapshot.
    {
        let mut block = set.block();
        let mut tx = block.transaction();
        for id in [
            "runtime_data",
            "runtime_pipeline",
            "runtime_time",
            "runtime_call",
        ] {
            tx.inspect_by_id_mut(&id.parse().unwrap(), |action| {
                action
                    .metadata_mut()
                    .insert("user_note".parse().unwrap(), Json::new(1_u64));
            })
            .unwrap();
        }
        tx.apply();
        block.commit();
    }
    (set, digest)
}

#[test]
fn runtime_capture_snapshot_restores_all_action_kinds_and_predecessor_images() {
    let (set, expected) = runtime_snapshot_set();
    let encoded = json::to_json(&set).unwrap();
    let restored: Set = json::from_json(&encoded).expect("restore the trigger snapshot");
    assert_eq!(json::to_json(&restored).unwrap(), encoded);
    let key = super::super::runtime_expectation::metadata_key();
    for id in [
        "runtime_data",
        "runtime_pipeline",
        "runtime_time",
        "runtime_call",
    ] {
        let id = id.parse().unwrap();
        let view = restored.view();
        let current = view
            .inspect_by_id(&id, |action| {
                action
                    .metadata()
                    .get(key)
                    .unwrap()
                    .try_into_any_norito::<Hash>()
                    .unwrap()
            })
            .unwrap();
        assert_eq!(current, expected);
        drop(view);
        let predecessor = restored.block_and_revert();
        let prior = predecessor
            .inspect_by_id(&id, |action| {
                action
                    .metadata()
                    .get(key)
                    .unwrap()
                    .try_into_any_norito::<Hash>()
                    .unwrap()
            })
            .unwrap();
        assert_eq!(prior, expected);
        assert!(
            predecessor
                .inspect_by_id(&id, |action| {
                    action.metadata().get("user_note").is_none()
                })
                .unwrap()
        );
    }
    assert_eq!(json::to_json(&restored).unwrap(), encoded);
}

fn tamper_runtime_snapshot_capture<F: mv::Value>(
    store: &mut Storage<TriggerId, LoadedAction<F>>,
    previous: bool,
    replacement: Option<&Json>,
) {
    let mut current = store
        .view()
        .iter()
        .map(|(id, action)| (id.clone(), action.clone()))
        .collect::<BTreeMap<_, _>>();
    let (id, original) = current
        .iter()
        .next()
        .map(|(id, action)| (id.clone(), action.clone()))
        .unwrap();
    let mut altered = original.clone();
    let key = super::super::runtime_expectation::metadata_key();
    if let Some(replacement) = replacement {
        altered.metadata.insert(key.clone(), replacement.clone());
    } else {
        altered.metadata.remove(key);
    }
    let prior = if previous {
        altered
    } else {
        current.insert(id.clone(), altered);
        original
    };
    *store = Storage::from_snapshot_parts(current, BTreeMap::from([(id, Some(prior))]));
}

#[test]
fn runtime_capture_snapshot_refuses_missing_or_malformed_current_and_predecessor() {
    let (set, expected) = runtime_snapshot_set();
    let valid = json::to_json(&set).unwrap();
    let prefixed = Json::new(norito::literal::format(
        "hash",
        &format!("0X{}", hex::encode_upper(expected.as_ref())),
    ));
    assert_eq!(
        prefixed.try_into_any_norito::<Hash>().unwrap(),
        expected,
        "the generic Hash decoder accepts this noncanonical equivalent spelling"
    );
    let malformed = Json::new(false);
    for field in [
        "data_triggers",
        "pipeline_triggers",
        "time_triggers",
        "by_call_triggers",
    ] {
        for previous in [false, true] {
            for replacement in [None, Some(&malformed), Some(&prefixed)] {
                let mut candidate: Set = json::from_json(&valid).unwrap();
                match field {
                    "data_triggers" => tamper_runtime_snapshot_capture(
                        &mut candidate.data_triggers,
                        previous,
                        replacement,
                    ),
                    "pipeline_triggers" => tamper_runtime_snapshot_capture(
                        &mut candidate.pipeline_triggers,
                        previous,
                        replacement,
                    ),
                    "time_triggers" => tamper_runtime_snapshot_capture(
                        &mut candidate.time_triggers,
                        previous,
                        replacement,
                    ),
                    _ => tamper_runtime_snapshot_capture(
                        &mut candidate.by_call_triggers,
                        previous,
                        replacement,
                    ),
                }
                let encoded = json::to_json(&candidate).unwrap();
                let Err(error) = json::from_json::<Set>(&encoded) else {
                    panic!("accepted {field} with invalid runtime consent");
                };
                let diagnostic = error.to_string();
                assert!(diagnostic.contains(field), "{diagnostic}");
                assert!(diagnostic.contains("runtime expectation"), "{diagnostic}");
                assert!(
                    diagnostic.contains(if previous { "predecessor" } else { "current" }),
                    "{diagnostic}"
                );
                assert_eq!(
                    json::to_json(&candidate).unwrap(),
                    encoded,
                    "restore never repairs missing consent"
                );
            }
        }
    }
}

#[test]
fn runtime_capture_restore_preserves_local_decoder_refusal() {
    let (mut set, _) = runtime_snapshot_set();
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
    let refusal = norito::with_decode_limits_scope(limits, || {
        norito::core::classify_decode_attempt(|| {
            validate_runtime_expectation_history("data_triggers", &mut set.data_triggers)
                .map_err(json::Error::into_core_error)
        })
    })
    .expect_err("the original enclosing allocation budget refuses digest decoding");
    assert_eq!(
        refusal.kind(),
        norito::core::DecodeAttemptErrorKind::EnclosingLimit
    );
    assert!(refusal.local_refusal().is_some());
    validate_runtime_expectation_history("data_triggers", &mut set.data_triggers)
        .expect("the same snapshot validates outside the retired resource scope");
}

#[test]
fn runtime_capture_binary_snapshot_refuses_missing_and_malformed_consent() {
    let (set, expected) = runtime_snapshot_set();
    let valid = SetDto::from(&set).encode().unwrap();
    let restored = Set::try_from(SetDto::decode(&valid).unwrap()).expect("valid binary snapshot");
    for id in [
        "runtime_data",
        "runtime_pipeline",
        "runtime_time",
        "runtime_call",
    ] {
        let digest = restored
            .view()
            .inspect_by_id(&id.parse().unwrap(), |action| {
                action
                    .metadata()
                    .get(super::super::runtime_expectation::metadata_key())
                    .unwrap()
                    .try_into_any_norito::<Hash>()
                    .unwrap()
            })
            .unwrap();
        assert_eq!(digest, expected);
    }
    for field in ["data", "pipeline", "time", "by_call"] {
        for invalid in [
            None,
            Some(Json::new(false)),
            Some(Json::new(
                "hash:0000000000000000000000000000000000000000000000000000000000000001#0000",
            )),
        ] {
            let mut dto = SetDto::decode(&valid).unwrap();
            let metadata = match field {
                "data" => &mut dto.data[0].1.metadata,
                "pipeline" => &mut dto.pipeline[0].1.metadata,
                "time" => &mut dto.time[0].1.metadata,
                _ => &mut dto.by_call[0].1.metadata,
            };
            let key = super::super::runtime_expectation::metadata_key();
            match invalid {
                Some(value) => {
                    metadata.insert(key.clone(), value);
                }
                None => {
                    metadata.remove(key);
                }
            }
            let bytes = dto.encode().unwrap();
            let decoded = SetDto::decode(&bytes).expect("the binary framing is valid");
            let Err(error) = Set::try_from(decoded) else {
                panic!("accepted {field} without valid runtime consent");
            };
            assert!(error.contains(field), "{error}");
            assert!(error.contains("runtime expectation"), "{error}");
        }
    }
}

#[test]
fn runtime_capture_dto_validation_needs_no_decoder_allocation() {
    let (_, digest) = runtime_snapshot_set();
    let valid = Json::new(digest);
    let invalid = Json::new(false);
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
    norito::with_decode_limits_scope(limits, || {
        assert!(canonical_runtime_capture(&valid));
        assert!(!canonical_runtime_capture(&invalid));
    });
}
