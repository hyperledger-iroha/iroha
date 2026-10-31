//! Authored source-only controls for borrowed lifecycle input capture.
//! These do not establish charged journal integration or authenticated effects.

use super::*;
use iroha_data_model::events::time::{ExecutionTime, Schedule};
use iroha_primitives::json::Json;
use std::num::NonZeroU32;

type Captured = (Vec<u8>, Vec<u8>, Option<(u32, u64)>);

fn payload(value: &dyn norito::core::SerializePayload) -> Vec<u8> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let mut bytes = Vec::new();
    norito::core::serialize_to_buffer(&WorldDeltaFieldRef(value), &mut bytes).unwrap();
    bytes
}

fn insert_time(tx: &mut SetTransaction<'_>, executable: Executable) -> TriggerId {
    let id: TriggerId = "lifecycle-source".parse().unwrap();
    let filter = TimeEventFilter(ExecutionTime::Schedule(Schedule::starting_at(
        std::time::Duration::from_millis(100),
    )));
    let mut action = SpecializedAction::new(
        executable,
        Repeats::Exactly(3),
        iroha_test_samples::ALICE_ID.clone(),
        filter,
    )
    .unwrap();
    action.retry_policy = Some(TimeTriggerRetryPolicy {
        max_retries: NonZeroU32::new(2).unwrap(),
        retry_after_ms: NonZeroU64::new(17).unwrap(),
    });
    action
        .metadata
        .insert("journal-marker".parse().unwrap(), Json::new("original"));
    assert!(
        tx.add_time_trigger(SpecializedTrigger::new(id.clone(), action))
            .unwrap()
    );
    id
}

fn capture_time(store: &impl BorrowedTriggerSourceReadOnly, id: &TriggerId) -> Option<Captured> {
    let action = store.time_triggers().get(id).unwrap();
    store
        .with_borrowed_trigger_source(
            id,
            BorrowedTriggerFilter::Time(&action.filter),
            action,
            |source| {
                Ok(source.map(|source| {
                    (
                        payload(source.persistent_action()),
                        payload(source),
                        source.retry_state(),
                    )
                }))
            },
        )
        .unwrap()
}

#[test]
fn persistent_action_payload_and_retry_tuple_preserve_distinct_originals() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert_time(
        &mut tx,
        Executable::Instructions(ConstVec::from(vec![InstructionBox::from(Log::new(
            Level::INFO,
            "retained original instruction".to_owned(),
        ))])),
    );
    let loaded = tx.time_triggers().get(&id).unwrap().clone();
    // Owned reconstruction is a test oracle only, never the producer's charge path.
    let original_action = tx.get_original_action(loaded).unwrap();
    let original_trigger = Trigger::new(id.clone(), original_action.clone());
    let before = capture_time(&tx, &id).unwrap();
    assert_eq!(before.0, payload(&original_action));
    assert_eq!(before.1, payload(&original_trigger));
    assert_eq!(before.2, None);

    assert!(tx.set_time_trigger_retry_state(
        &id,
        Some(TimeTriggerRetryState {
            retries_used: 1,
            next_retry_at_ms: 117
        }),
    ));
    let after = capture_time(&tx, &id).unwrap();
    assert_eq!(after.0, before.0);
    assert_eq!(after.1, before.1);
    assert_eq!(after.2, Some((1, 117)));
}

#[test]
fn borrowed_capture_observes_current_overlay_and_rollback() {
    let set = Set::default();
    let mut block = set.block();
    let id = {
        let mut tx = block.transaction();
        let id = insert_time(&mut tx, Executable::Instructions(ConstVec::new_empty()));
        tx.apply();
        id
    };
    let before = capture_time(&block, &id).unwrap();
    {
        let mut tx = block.transaction();
        tx.inspect_by_id_mut(&id, |action| {
            action
                .metadata_mut()
                .insert("journal-marker".parse().unwrap(), Json::new("overlay"));
        })
        .unwrap();
        assert!(tx.set_time_trigger_retry_state(
            &id,
            Some(TimeTriggerRetryState {
                retries_used: 2,
                next_retry_at_ms: 134
            }),
        ));
        let staged = capture_time(&tx, &id).unwrap();
        assert_ne!(staged.0, before.0);
        assert_ne!(staged.1, before.1);
        assert_eq!(staged.2, Some((2, 134)));
        // No apply: neither persistent Action nor retry tuple may escape the overlay.
    }
    assert_eq!(capture_time(&block, &id).unwrap(), before);
}

#[test]
fn borrowed_ivm_action_requires_complete_original_bytecode() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    // Stored source fixture only; this bytecode is not executed or called valid code.
    let code = IvmBytecode::from_compiled(vec![0xaa, 0xbb]);
    let hash = HashOf::new(&code);
    let id = insert_time(&mut tx, Executable::Ivm(code));
    let loaded = tx.time_triggers().get(&id).unwrap().clone();
    let original_action = tx.get_original_action(loaded).unwrap();
    assert_eq!(capture_time(&tx, &id).unwrap().0, payload(&original_action));
    assert!(tx.contracts.remove(hash).is_some());
    // Missing original bytes are not silently replaced by a loaded/cache hash.
    assert!(capture_time(&tx, &id).is_none());
}
