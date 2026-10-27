//! Authored controls only; this detached candidate has not been compiled or run.
use super::*;
use crate::{
    HasMetadata,
    account::AccountId,
    events::{
        EventFilterBox,
        data::DataEventFilter,
        execute_trigger::ExecuteTriggerEventFilter,
        time::{ExecutionTime, Schedule, TimeEventFilter},
    },
    transaction::{Executable, IvmBytecode},
    trigger::action::{Repeats, TimeTriggerRetryPolicy},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_model_base::{metadata::Metadata, name::Name};
use iroha_primitives::{const_vec::ConstVec, json::Json};
use std::{
    num::{NonZeroU32, NonZeroU64},
    time::Duration,
};

fn limits() -> ActionScannerLimitsV1 {
    ActionScannerLimitsV1 {
        max_frame_bytes: 65_536,
        max_total_frame_bytes: 131_072,
        max_frames: 2,
        max_work: 2_000_000,
        max_fields: 1000,
        max_sequence_elements: 65_536,
        max_depth: 32,
    }
}

fn field(bytes: &[u8]) -> Vec<u8> {
    let mut length = bytes.len() as u64;
    let mut output = Vec::new();
    loop {
        let next = (length & 0x7f) as u8;
        length >>= 7;
        output.push(next | if length == 0 { 0 } else { 0x80 });
        if length == 0 {
            break;
        }
    }
    output.extend_from_slice(bytes);
    output
}

fn payload<T: norito::SerializePayload>(value: &T) -> Vec<u8> {
    let mut output = Vec::new();
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    ncore::serialize_to_buffer(value, &mut output).unwrap();
    output
}

fn action() -> Action {
    use crate::{
        account::AccountId,
        events::time::{ExecutionTime, Schedule, TimeEventFilter},
        transaction::{Executable, IvmBytecode},
        trigger::action::Repeats,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    let key = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).unwrap();
    Action::new(
        Executable::Ivm(IvmBytecode::from_compiled(vec![0, 1, 255])),
        Repeats::Exactly(1),
        AccountId::new(key.public_key().clone()),
        TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
            std::time::Duration::from_millis(1),
        ))),
    )
    .unwrap()
}

#[test]
fn accepts_real_canonical_ivm_action_without_an_action_decoder() {
    let frame = norito::encode_canonical(&action()).unwrap();
    let pool = AllocationBudget::new(65_536);
    let mut scanner = BoundedActionFrameScannerV1::try_new(
        &pool,
        limits(),
        DecodeLimits::new(1000, 65_536, 1000, 65_536, 32),
    )
    .unwrap();
    let result = scanner.scan(&frame).unwrap();
    assert!(matches!(
        result.filter_kind,
        TriggerLifecycleStateKindV1::Time
    ));
    assert!(!result.has_retry_policy);
}

#[test]
fn first_refusal_is_latched_before_a_later_valid_frame() {
    let frame = norito::encode_canonical(&action()).unwrap();
    let pool = AllocationBudget::new(65_536);
    let mut scanner = BoundedActionFrameScannerV1::try_new(
        &pool,
        limits(),
        DecodeLimits::new(1000, 65_536, 1000, 65_536, 32),
    )
    .unwrap();
    let first = std::ptr::from_ref(scanner.scan(&[]).unwrap_err());
    let second = std::ptr::from_ref(scanner.scan(&frame).unwrap_err());
    assert_eq!(first, second);
}

#[test]
fn physical_control_admission_precedes_decode() {
    let pool = AllocationBudget::new(0);
    assert!(
        BoundedActionFrameScannerV1::try_new(
            &pool,
            limits(),
            DecodeLimits::new(1000, 65_536, 1000, 65_536, 32),
        )
        .is_err()
    );
}

#[test]
fn original_codec_ivm_payload_is_fully_consumed() {
    use crate::transaction::{Executable, IvmBytecode};
    let mut bytes = payload(&Executable::Ivm(IvmBytecode::from_compiled(vec![
        0, 1, 255,
    ])));
    let mut budget = Budget::new(limits());
    assert!(scan_executable(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget).is_ok());
    bytes.push(0);
    let mut budget = Budget::new(limits());
    assert_eq!(
        scan_executable(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget),
        Err(ScanError::Length)
    );
}

#[test]
fn nonempty_dynamic_instructions_are_never_skipped_by_length() {
    let mut bytes = 0u32.to_le_bytes().to_vec();
    bytes.extend(field(&1u64.to_le_bytes()));
    let mut budget = Budget::new(limits());
    assert_eq!(
        scan_executable(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget),
        Err(ScanError::UnsupportedExecutable)
    );
}

#[test]
fn compact_length_rejects_redundant_and_overflow_forms() {
    for bytes in [&[0x80, 0x00][..], &[0xff; 10][..], &[0x81][..]] {
        let mut budget = Budget::new(limits());
        let mut cursor = Cursor::root(bytes, &mut budget).unwrap();
        assert!(cursor.field(&mut budget).is_err());
    }
}

#[test]
fn nested_fields_consume_the_same_depth_and_field_counters() {
    let bytes = field(&field(&[]));
    let mut bounds = limits();
    bounds.max_depth = 2;
    let mut budget = Budget::new(bounds);
    let mut root = Cursor::root(&bytes, &mut budget).unwrap();
    let mut child = root.field(&mut budget).unwrap();
    assert_eq!(child.field(&mut budget).unwrap_err(), ScanError::Limit);
}

#[test]
fn frame_and_work_limits_are_cumulative() {
    let mut budget = Budget::new(limits());
    budget.begin_frame(10).unwrap();
    budget.begin_frame(10).unwrap();
    assert_eq!(budget.begin_frame(10), Err(ScanError::Limit));
    let mut bounds = limits();
    bounds.max_work = 9;
    assert_eq!(Budget::new(bounds).begin_frame(10), Err(ScanError::Limit));
}

#[test]
fn zero_retry_fields_are_rejected() {
    for (count, delay) in [(0u32, 1u64), (1, 0)] {
        let mut policy = field(&count.to_le_bytes());
        policy.extend(field(&delay.to_le_bytes()));
        let mut option = vec![1];
        option.extend(field(&policy));
        let mut budget = Budget::new(limits());
        assert_eq!(
            cursor::retry_policy(Cursor::root(&option, &mut budget).unwrap(), &mut budget),
            Err(ScanError::InvalidValue)
        );
    }
}

#[test]
fn by_call_missing_normalized_authority_is_not_canonical() {
    let mut inner = field(&[0]); // trigger_id None
    inner.extend(field(&[0])); // authority None would be normalized
    let mut bytes = 3u32.to_le_bytes().to_vec();
    bytes.extend(field(&inner));
    let mut budget = Budget::new(limits());
    assert!(matches!(
        scan_filter(Cursor::root(&bytes, &mut budget).unwrap(), &[], &mut budget),
        Err(ScanError::NonCanonical)
    ));
}

fn metadata(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut output = (entries.len() as u64).to_le_bytes().to_vec();
    for (name, json) in entries {
        let mut pair = field(&field(name.as_bytes()));
        pair.extend(field(&field(&field(json.as_bytes()))));
        output.extend(field(&pair));
    }
    output
}

#[test]
fn metadata_subset_checks_every_value_and_strict_key_order() {
    for entries in [vec![("a", "null"), ("b", "\"hello\"")], vec![]] {
        let bytes = metadata(&entries);
        let mut budget = Budget::new(limits());
        assert!(scan_metadata(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget).is_ok());
    }
    for entries in [
        vec![("a", "null"), ("a", "true")],
        vec![("b", "null"), ("a", "true")],
        vec![("a", "[1]")],
        vec![("a", "0")],
        vec![("a", "\"x\\ny\"")],
    ] {
        let bytes = metadata(&entries);
        let mut budget = Budget::new(limits());
        assert!(scan_metadata(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget).is_err());
    }
}

#[test]
fn name_subset_refuses_normalization_and_control_paths() {
    for name in ["", "bad name", "a@b", "a#b", "a$b", "é", "\u{202e}"] {
        let bytes = field(name.as_bytes());
        let mut budget = Budget::new(limits());
        assert!(scan_name(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget).is_err());
    }
}

// The controls below are authored against the retained production constructors,
// field types and canonical codec. Owned decode is a test oracle only; the
// production scanner never constructs or decodes an owned Action.
fn fixture_authority(seed: u8) -> AccountId {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap();
    AccountId::new(key.public_key().clone())
}

fn fixture_action(filter: impl Into<EventFilterBox>) -> Action {
    Action::new(
        Executable::Ivm(IvmBytecode::from_compiled(vec![0, 1, 255])),
        Repeats::Exactly(1),
        fixture_authority(7),
        filter,
    )
    .unwrap()
}

fn fixture_retry() -> TimeTriggerRetryPolicy {
    TimeTriggerRetryPolicy {
        max_retries: NonZeroU32::new(2).unwrap(),
        retry_after_ms: NonZeroU64::new(5).unwrap(),
    }
}

fn assert_real_canonical_action(value: &Action, kind: TriggerLifecycleStateKindV1, retry: bool) {
    let frame = norito::encode_canonical(value).unwrap();
    // A real normative decode must preserve the complete typed fixture and bytes.
    let decoded = norito::decode_canonical::<Action>(&frame).unwrap();
    assert_eq!(&decoded, value);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    let pool = AllocationBudget::new(65_536);
    let mut scanner = BoundedActionFrameScannerV1::try_new(
        &pool,
        limits(),
        DecodeLimits::new(1000, 65_536, 1000, 65_536, 32),
    )
    .unwrap();
    let actual = scanner.scan(&frame).unwrap();
    assert_eq!(actual.filter_kind, kind);
    assert_eq!(actual.has_retry_policy, retry);
}

#[test]
fn canonical_scheduled_actions_with_retry_cover_optional_period() {
    for schedule in [
        Schedule::starting_at(Duration::from_millis(17)),
        Schedule::starting_at(Duration::from_millis(17)).with_period(Duration::from_millis(1000)),
    ] {
        let value = fixture_action(TimeEventFilter::new(ExecutionTime::Schedule(schedule)))
            .with_retry_policy(fixture_retry())
            .unwrap();
        assert_real_canonical_action(&value, TriggerLifecycleStateKindV1::Time, true);
    }
}

#[test]
fn canonical_precommit_action_has_no_retry_policy() {
    let value = fixture_action(TimeEventFilter::new(ExecutionTime::PreCommit));
    assert_real_canonical_action(&value, TriggerLifecycleStateKindV1::Time, false);
}

#[test]
fn canonical_data_any_action_covers_indefinite_and_zero_repeats() {
    for repeats in [Repeats::Indefinitely, Repeats::Exactly(0)] {
        let value = Action::new(
            Executable::Ivm(IvmBytecode::from_compiled(vec![])),
            repeats,
            fixture_authority(7),
            DataEventFilter::Any,
        )
        .unwrap();
        assert_real_canonical_action(&value, TriggerLifecycleStateKindV1::Data, false);
    }
}

#[test]
fn canonical_by_call_actions_cover_optional_trigger_id() {
    for filter in [
        ExecuteTriggerEventFilter::new(),
        ExecuteTriggerEventFilter::new().for_trigger("specific_trigger".parse().unwrap()),
    ] {
        let value = fixture_action(filter);
        let EventFilterBox::ExecuteTrigger(filter) = value.filter() else {
            panic!("fixture kind");
        };
        // The real Action constructor fills the canonical authority, even when
        // the supplied filter omitted it. These bytes must be accepted.
        assert_eq!(filter.authority(), Some(value.authority()));
        assert_real_canonical_action(&value, TriggerLifecycleStateKindV1::ByCall, false);
    }
}

#[test]
fn canonical_explicit_by_call_authority_matches_the_owner() {
    let filter = ExecuteTriggerEventFilter::new()
        .for_trigger("explicit_trigger".parse().unwrap())
        .under_authority(fixture_authority(7));
    let value = fixture_action(filter);
    assert_real_canonical_action(&value, TriggerLifecycleStateKindV1::ByCall, false);
}

#[test]
fn canonical_nonempty_metadata_covers_every_supported_json_kind() {
    let mut metadata = Metadata::default();
    // Deliberately insert out of order: the real serializer must emit Name order.
    for (name, text) in [
        ("f-string", "\"visible / ASCII ~\""),
        ("d-array", "[]"),
        ("b-true", "true"),
        ("e-object", "{}"),
        ("a-null", "null"),
        ("c-false", "false"),
    ] {
        metadata.insert(
            name.parse().unwrap(),
            Json::from_raw_json(text.to_owned()).unwrap(),
        );
    }
    let value = fixture_action(DataEventFilter::Any).with_metadata(metadata);
    assert_real_canonical_action(&value, TriggerLifecycleStateKindV1::Data, false);
}

#[test]
fn canonical_empty_instructions_action_is_a_real_element_sequence() {
    let value = Action::new(
        Executable::Instructions(ConstVec::new_empty()),
        Repeats::Exactly(1),
        fixture_authority(7),
        DataEventFilter::Any,
    )
    .unwrap();
    assert_real_canonical_action(&value, TriggerLifecycleStateKindV1::Data, false);
}

#[test]
fn canonical_empty_batch_action_is_a_real_element_sequence() {
    let value = Action::new(
        Executable::Batch(ConstVec::new_empty()),
        Repeats::Exactly(1),
        fixture_authority(7),
        TimeEventFilter::new(ExecutionTime::PreCommit),
    )
    .unwrap();
    assert_real_canonical_action(&value, TriggerLifecycleStateKindV1::Time, false);
}

/// Test-only typed wire candidate. Safe Action constructors cannot produce the
/// invalid semantics needed below. This uses the actual six field types and
/// production derive encoder, never hand-built field lengths or raw byte edits.
/// Every negative starts by proving that its unmodified frame equals a real
/// canonical Action frame before changing exactly the named typed field.
#[derive(norito::derive::SerializePayload)]
struct ActionWireFixture {
    executable: Executable,
    repeats: Repeats,
    authority: AccountId,
    filter: EventFilterBox,
    retry_policy: Option<TimeTriggerRetryPolicy>,
    metadata: Metadata,
}

impl ActionWireFixture {
    fn from_action(value: &Action) -> Self {
        let fixture = Self {
            executable: value.executable().clone(),
            repeats: value.repeats(),
            authority: value.authority().clone(),
            filter: value.filter().clone(),
            retry_policy: value.retry_policy(),
            metadata: value.metadata().clone(),
        };
        assert_eq!(fixture.frame(), norito::encode_canonical(value).unwrap());
        fixture
    }

    fn frame(&self) -> Vec<u8> {
        ncore::frame_bare_with_header_flags::<Action>(&payload(self), SCANNER_FLAGS).unwrap()
    }
}

fn assert_typed_semantic_refusal(frame: &[u8], reason: ScanError) {
    // The original Action decoder must reject the typed mutation too. This
    // oracle stays test-only and grants no production validator authority.
    assert!(norito::decode_canonical::<Action>(frame).is_err());
    let pool = AllocationBudget::new(65_536);
    let mut scanner = BoundedActionFrameScannerV1::try_new(
        &pool,
        limits(),
        DecodeLimits::new(1000, 65_536, 1000, 65_536, 32),
    )
    .unwrap();
    match scanner.scan(frame).unwrap_err() {
        ActionFrameScanFailureV1::Structure(actual) => assert_eq!(*actual, reason.message()),
        other => panic!("wrong rejection point: {other:?}"),
    }
}

#[test]
fn typed_mutation_of_by_call_authority_is_rejected() {
    let value = fixture_action(ExecuteTriggerEventFilter::new());
    let mut changed = ActionWireFixture::from_action(&value);
    changed.filter = ExecuteTriggerEventFilter::new()
        .under_authority(fixture_authority(8))
        .into();
    assert_typed_semantic_refusal(&changed.frame(), ScanError::InvalidValue);
}

#[test]
fn typed_mutation_of_retry_on_precommit_is_rejected() {
    let value = fixture_action(TimeEventFilter::new(ExecutionTime::PreCommit));
    assert!(value.clone().with_retry_policy(fixture_retry()).is_err());
    let mut changed = ActionWireFixture::from_action(&value);
    changed.retry_policy = Some(fixture_retry());
    assert_typed_semantic_refusal(&changed.frame(), ScanError::InvalidValue);
}

#[test]
fn typed_mutation_of_retry_on_data_any_is_rejected() {
    let value = fixture_action(DataEventFilter::Any);
    assert!(value.clone().with_retry_policy(fixture_retry()).is_err());
    let mut changed = ActionWireFixture::from_action(&value);
    changed.retry_policy = Some(fixture_retry());
    assert_typed_semantic_refusal(&changed.frame(), ScanError::InvalidValue);
}

#[test]
fn typed_removal_of_canonical_by_call_authority_is_rejected() {
    let value = fixture_action(ExecuteTriggerEventFilter::new());
    let mut changed = ActionWireFixture::from_action(&value);
    changed.filter = ExecuteTriggerEventFilter::new().into();
    assert_typed_semantic_refusal(&changed.frame(), ScanError::NonCanonical);
}

/// Exact JsonWireOwned field type, used only to express protocol-invalid text
/// which Json's public constructors intentionally refuse to store.
#[derive(norito::derive::SerializePayload)]
struct RawJsonText(String);

#[test]
fn json_protocol_byte_ceiling_is_independent_of_larger_scan_limits() {
    let boundary = format!("\"{}\"", "x".repeat(MAX_JSON_BYTES - 2));
    let mut real = Metadata::default();
    let key: Name = "bounded".parse().unwrap();
    real.insert(key.clone(), Json::from_raw_json(boundary.clone()).unwrap());
    let typed_boundary = vec![(key.clone(), RawJsonText(boundary))];
    // The typed mutation fixture matches the real Metadata/Json codec at the
    // exact legal boundary; only one extra ASCII byte is added afterwards.
    assert_eq!(payload(&typed_boundary), payload(&real));

    let over = format!("\"{}\"", "x".repeat(MAX_JSON_BYTES - 1));
    assert!(Json::from_raw_json(over.clone()).is_err());
    let bytes = payload(&vec![(key, RawJsonText(over))]);
    let mut bounds = limits();
    bounds.max_frame_bytes = 4_194_304;
    bounds.max_total_frame_bytes = 4_194_304;
    bounds.max_work = 16_777_216;
    let boundary_bytes = payload(&real);
    let mut boundary_budget = Budget::new(bounds);
    scan_metadata(
        Cursor::root(&boundary_bytes, &mut boundary_budget).unwrap(),
        &mut boundary_budget,
    )
    .unwrap();
    let mut budget = Budget::new(bounds);
    assert_eq!(
        scan_metadata(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget),
        Err(ScanError::InvalidValue),
    );
    // Enough work for the framing, but not for the oversized text. The protocol
    // refusal must win before traversing or validating that text's UTF-8 bytes.
    bounds.max_work = 128;
    let mut budget = Budget::new(bounds);
    assert_eq!(
        scan_metadata(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget),
        Err(ScanError::InvalidValue),
    );
}

#[test]
fn original_codec_reconciles_time_data_by_call_and_metadata_payload_layouts() {
    let owner = fixture_authority(7);
    let filters: [(EventFilterBox, TriggerLifecycleStateKindV1, bool); 4] = [
        (
            TimeEventFilter::new(ExecutionTime::PreCommit).into(),
            TriggerLifecycleStateKindV1::Time,
            false,
        ),
        (
            TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
                Duration::from_millis(3),
            )))
            .into(),
            TriggerLifecycleStateKindV1::Time,
            true,
        ),
        (
            DataEventFilter::Any.into(),
            TriggerLifecycleStateKindV1::Data,
            false,
        ),
        (
            ExecuteTriggerEventFilter::new()
                .for_trigger("codec_trigger".parse().unwrap())
                .under_authority(owner.clone())
                .into(),
            TriggerLifecycleStateKindV1::ByCall,
            false,
        ),
    ];
    let owner_bytes = payload(&owner);
    for (filter, kind, scheduled) in filters {
        let bytes = payload(&filter);
        let mut budget = Budget::new(limits());
        let result = scan_filter(
            Cursor::root(&bytes, &mut budget).unwrap(),
            &owner_bytes,
            &mut budget,
        )
        .unwrap();
        assert_eq!(result, (kind, scheduled));
    }
    let mut metadata = Metadata::default();
    metadata.insert(
        "codec_name".parse().unwrap(),
        Json::from_raw_json("\"codec value\"".to_owned()).unwrap(),
    );
    let bytes = payload(&metadata);
    let mut budget = Budget::new(limits());
    scan_metadata(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget).unwrap();
}
