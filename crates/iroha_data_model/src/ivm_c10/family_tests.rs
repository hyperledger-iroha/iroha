//! Roundtrips, layout parity and schema identities of the C10 event, filter, manifest,
//! trigger, result and registration families.

#[cfg(feature = "transparent_api")]
mod event_matching;

use std::{boxed::Box, num::NonZeroU64, vec, vec::Vec};

use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_model_base::{
    metadata::Metadata,
    topology::{DataSpaceId, LaneId},
};
use norito::codec::{DecodeAll, Encode};

use super::*;
use crate::{
    account::AccountId,
    events::{
        EventFilterBox,
        execute_trigger::ExecuteTriggerEventFilter,
        pipeline::{TransactionEvent, TransactionEventFilter, TransactionStatus},
        time::{ExecutionTime, Schedule, TimeEventFilter},
        trigger_completed::TriggerCompletedEventFilter,
    },
    smart_contract::{
        ContractArtifactId,
        event::ContractEmissionsV1,
        manifest::{ContractManifest, EntryPointKind, EntrypointAuthorizationV1, TriggerCallback},
    },
    transaction::{Executable, IvmBytecode, SignedTransaction, TransactionResult},
    trigger::action::{ActionValidationError, Repeats, TimeTriggerRetryPolicy},
};

/// The framed and bare Norito encodings and the JSON form all decode to `value`.
fn roundtrip_all<T>(value: &T)
where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + Encode
        + DecodeAll
        + norito::json::JsonSerialize
        + norito::json::JsonDeserialize
        + PartialEq
        + core::fmt::Debug,
{
    let framed = norito::to_bytes(value).expect("encode");
    assert_eq!(
        &norito::decode_from_bytes::<T>(&framed).expect("decode"),
        value
    );
    let bare = value.encode();
    assert_eq!(
        &T::decode_all(&mut bare.as_slice()).expect("decode the bare encoding"),
        value
    );
    let json = norito::json::to_json(value).expect("json");
    assert_eq!(
        &norito::json::from_json::<T>(&json).expect("from json"),
        value
    );
}

fn tx_hash(byte: u8) -> HashOf<SignedTransaction> {
    HashOf::from_untyped_unchecked(Hash::prehashed([byte; Hash::LENGTH]))
}

fn account(seed: u8) -> AccountId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
        .expect("fixed key seed");
    AccountId::new(key_pair.public_key().clone())
}

/// A rejection that only the C10 family can carry.
fn c10_rejection() -> TransactionStatusC10V1 {
    TransactionStatusC10V1::Rejected(Box::new(TransactionRejectionReasonC10V1::Validation(
        ValidationFailC10V1::IvmAdmission(IvmAdmissionErrorC10V1::MaxCyclesWithoutZk(7)),
    )))
}

fn statuses() -> Vec<TransactionStatusC10V1> {
    vec![
        TransactionStatusC10V1::Queued,
        TransactionStatusC10V1::Expired,
        TransactionStatusC10V1::Approved,
        c10_rejection(),
    ]
}

fn event(status: TransactionStatusC10V1) -> TransactionEventC10V1 {
    TransactionEventC10V1 {
        hash: tx_hash(1),
        block_height: NonZeroU64::new(7),
        lane_id: LaneId::new(2),
        dataspace_id: DataSpaceId::new(3),
        status,
    }
}

fn schedule() -> TimeEventFilter {
    TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
        core::time::Duration::from_millis(5),
    )))
}

fn rejected_filter() -> EventFilterBoxC10V1 {
    TransactionEventFilterC10V1::default()
        .for_status(c10_rejection())
        .into()
}

#[cfg(feature = "http")]
#[test]
fn opted_in_stream_uses_distinct_subscription_and_message_frames() {
    use super::events::{EventMessageC10V1, EventSubscriptionC10V1};
    use crate::events::{
        EventBox,
        prelude::{EventMessage, EventSubscriptionRequest},
    };

    let baseline = EventSubscriptionRequest::new(vec![]);
    let request = EventSubscriptionC10V1 {
        filters: vec![TransactionEventFilterC10V1::default().for_status(c10_rejection())],
    };
    let bytes = norito::encode_canonical(&request).unwrap();
    assert_eq!(
        norito::decode_canonical::<EventSubscriptionC10V1>(&bytes).unwrap(),
        request
    );
    assert!(norito::decode_canonical::<EventSubscriptionRequest>(&bytes).is_err());
    assert!(
        norito::decode_canonical::<EventSubscriptionC10V1>(
            &norito::encode_canonical(&baseline).unwrap()
        )
        .is_err()
    );

    let baseline = EventMessage(EventBox::PipelineBatch(vec![]));
    let message = EventMessageC10V1(event(c10_rejection()).into());
    let bytes = norito::encode_canonical(&message).unwrap();
    assert_eq!(
        norito::decode_canonical::<EventMessageC10V1>(&bytes).unwrap(),
        message
    );
    assert!(norito::decode_canonical::<EventMessage>(&bytes).is_err());
    assert!(
        norito::decode_canonical::<EventMessageC10V1>(
            &norito::encode_canonical(&baseline).unwrap()
        )
        .is_err()
    );
}

#[test]
fn events_and_filters_roundtrip_framed_bare_and_json() {
    for status in statuses() {
        roundtrip_all(&status);
        if let TransactionStatusC10V1::Rejected(reason) = &status {
            roundtrip_all(reason.as_ref());
        }
        let transaction = event(status.clone());
        roundtrip_all(&transaction);
        let pipeline = PipelineEventBoxC10V1::Transaction(transaction.clone());
        roundtrip_all(&pipeline);
        roundtrip_all(&EventBoxC10V1::Pipeline(pipeline.clone()));
        roundtrip_all(&EventBoxC10V1::PipelineBatch(vec![
            pipeline.clone(),
            pipeline,
        ]));
        let filter = TransactionEventFilterC10V1::default()
            .for_hash(tx_hash(1))
            .for_block_height(NonZeroU64::new(7))
            .for_lane_id(LaneId::new(2))
            .for_dataspace_id(DataSpaceId::new(3))
            .for_status(status);
        roundtrip_all(&filter);
        let pipeline = PipelineEventFilterBoxC10V1::Transaction(filter);
        roundtrip_all(&pipeline);
        roundtrip_all(&EventFilterBoxC10V1::Pipeline(pipeline));
    }
    for filter in [
        EventFilterBoxC10V1::Time(schedule()),
        EventFilterBoxC10V1::ExecuteTrigger(ExecuteTriggerEventFilter::new()),
        EventFilterBoxC10V1::TriggerCompleted(TriggerCompletedEventFilter::new()),
        EventFilterBoxC10V1::Pipeline(PipelineEventFilterBoxC10V1::Transaction(
            TransactionEventFilterC10V1::default(),
        )),
    ] {
        roundtrip_all(&filter);
    }
}

#[test]
fn c10_parents_keep_the_baseline_layout_except_the_status() {
    // Every unchanged member and tag encodes exactly as the baseline parent encodes it.
    let time = schedule();
    assert_eq!(
        EventFilterBoxC10V1::Time(time.clone()).encode(),
        EventFilterBox::Time(time).encode()
    );
    assert_eq!(
        EventFilterBoxC10V1::from(TransactionEventFilterC10V1::default().for_hash(tx_hash(4)))
            .encode(),
        EventFilterBox::from(TransactionEventFilter::default().for_hash(tx_hash(4))).encode()
    );
    for (c10, baseline) in [
        (TransactionStatusC10V1::Queued, TransactionStatus::Queued),
        (TransactionStatusC10V1::Expired, TransactionStatus::Expired),
        (
            TransactionStatusC10V1::Approved,
            TransactionStatus::Approved,
        ),
    ] {
        let c10 = event(c10);
        let baseline = TransactionEvent {
            hash: c10.hash,
            block_height: c10.block_height,
            lane_id: c10.lane_id,
            dataspace_id: c10.dataspace_id,
            status: baseline,
        };
        assert_eq!(c10.encode(), baseline.encode());
    }
    let ok = TransactionResultC10V1(
        Ok(Vec::new()),
        Vec::new(),
        None,
        ContractEmissionsV1::default(),
    );
    let baseline_ok = TransactionResult(
        Ok(Vec::new()),
        Vec::new(),
        None,
        ContractEmissionsV1::default(),
    );
    assert_eq!(ok.encode(), baseline_ok.encode());
    let manifest = manifest(None);
    assert_eq!(manifest.encode(), baseline_manifest().encode());
}

#[test]
fn transaction_filter_matches_every_present_field() {
    let rejected = event(c10_rejection());
    assert!(TransactionEventFilterC10V1::default().matches(&rejected));
    assert!(
        TransactionEventFilterC10V1::default()
            .for_status(c10_rejection())
            .for_hash(tx_hash(1))
            .for_block_height(NonZeroU64::new(7))
            .matches(&rejected)
    );
    for filter in [
        TransactionEventFilterC10V1::default().for_status(TransactionStatusC10V1::Approved),
        TransactionEventFilterC10V1::default().for_hash(tx_hash(2)),
        TransactionEventFilterC10V1::default().for_block_height(None),
        TransactionEventFilterC10V1::default().for_lane_id(LaneId::new(9)),
        TransactionEventFilterC10V1::default().for_dataspace_id(DataSpaceId::new(9)),
    ] {
        assert!(!filter.matches(&rejected), "{filter:?}");
    }
}

fn trigger_descriptor() -> TriggerDescriptorC10V1 {
    TriggerDescriptorC10V1 {
        id: "on_c10_rejection".parse().expect("trigger id"),
        repeats: Repeats::Indefinitely,
        filter: rejected_filter(),
        authority: Some(account(3)),
        metadata: Metadata::default(),
        callback: TriggerCallback {
            namespace: None,
            entrypoint: "handle".into(),
        },
    }
}

fn manifest(entrypoints: Option<Vec<EntrypointDescriptorC10V1>>) -> ContractManifestC10V1 {
    ContractManifestC10V1 {
        seiyaku_name: Some("Seiyaku".into()),
        code_hash: Some(Hash::new(b"code")),
        abi_hash: Some(Hash::new(b"abi")),
        compiler_fingerprint: Some("koto".into()),
        features_bitmap: Some(0),
        access_set_hints: None,
        permissions: Vec::new(),
        events: Vec::new(),
        entrypoints,
        states: None,
        error_types: None,
        enum_types: Vec::new(),
        error_messages: None,
        kotoba: None,
        provenance: None,
    }
}

fn baseline_manifest() -> ContractManifest {
    ContractManifest {
        seiyaku_name: Some("Seiyaku".into()),
        code_hash: Some(Hash::new(b"code")),
        abi_hash: Some(Hash::new(b"abi")),
        compiler_fingerprint: Some("koto".into()),
        features_bitmap: Some(0),
        access_set_hints: None,
        permissions: Vec::new(),
        events: Vec::new(),
        entrypoints: None,
        states: None,
        error_types: None,
        enum_types: Vec::new(),
        error_messages: None,
        kotoba: None,
        provenance: None,
    }
}

fn entrypoint() -> EntrypointDescriptorC10V1 {
    EntrypointDescriptorC10V1 {
        name: "handle".into(),
        kind: EntryPointKind::Kotoage,
        params: Vec::new(),
        argument_schema: None,
        return_type: Some("()".into()),
        return_schema: None,
        authorization: EntrypointAuthorizationV1::Anyone,
        read_keys: Vec::new(),
        write_keys: Vec::new(),
        access_hints_complete: Some(true),
        access_hints_skipped: Vec::new(),
        triggers: vec![trigger_descriptor()],
    }
}

#[test]
fn manifests_and_trigger_descriptors_roundtrip_framed_bare_and_json() {
    roundtrip_all(&trigger_descriptor());
    roundtrip_all(&entrypoint());
    roundtrip_all(&manifest(None));
    roundtrip_all(&manifest(Some(vec![entrypoint()])));
}

fn action() -> ActionC10V1 {
    ActionC10V1::new(
        Executable::Ivm(IvmBytecode::from_compiled(vec![1, 2, 3])),
        Repeats::Exactly(2),
        account(1),
        rejected_filter(),
    )
    .expect("a pipeline filter is a valid trigger filter")
}

#[test]
fn actions_and_triggers_roundtrip_and_keep_the_baseline_invariants() {
    let action = action();
    roundtrip_all(&action);
    let trigger = TriggerC10V1 {
        id: "c10_trigger".parse().expect("trigger id"),
        action: action.clone(),
    };
    roundtrip_all(&trigger);
    let retry = TimeTriggerRetryPolicy {
        max_retries: core::num::NonZeroU32::new(2).expect("nonzero"),
        retry_after_ms: NonZeroU64::new(5).expect("nonzero"),
    };
    let scheduled = ActionC10V1::new(
        Executable::Ivm(IvmBytecode::from_compiled(Vec::new())),
        Repeats::Indefinitely,
        account(1),
        EventFilterBoxC10V1::Time(schedule()),
    )
    .expect("time filter")
    .with_retry_policy(retry)
    .expect("a scheduled time trigger accepts a retry policy");
    let mut stored = StoredC10TriggerV1::new(TriggerC10V1 {
        id: trigger.id.clone(),
        action: scheduled.clone(),
    });
    roundtrip_all(&stored);
    assert!(stored.retry_state().is_none());
    let cursor = crate::block::execution_output::TriggerLifecycleRetryStateV1 {
        attempt: 2,
        next_time_ms: 15,
    };
    stored.set_retry_state(Some(cursor.clone())).unwrap();
    roundtrip_all(&stored);
    assert_eq!(stored.retry_state(), Some(&cursor));
    let invalid = crate::block::execution_output::TriggerLifecycleRetryStateV1 {
        attempt: 3,
        next_time_ms: 20,
    };
    assert!(stored.set_retry_state(Some(invalid)).is_err());
    assert_eq!(stored.retry_state(), Some(&cursor));
    assert!(
        StoredC10TriggerV1::new(trigger.clone())
            .set_retry_state(Some(cursor))
            .is_err()
    );
    roundtrip_all(&scheduled);
    assert_eq!(
        action.clone().with_retry_policy(retry),
        Err(ActionValidationError::RetryPolicyRequiresScheduledTimeTrigger)
    );
    assert_eq!(
        ActionC10V1::new(
            Executable::Ivm(IvmBytecode::from_compiled(Vec::new())),
            Repeats::Indefinitely,
            account(1),
            EventFilterBoxC10V1::TriggerCompleted(TriggerCompletedEventFilter::new()),
        ),
        Err(ActionValidationError::TriggerCompletedFilter)
    );
    assert_eq!(
        ActionC10V1::new(
            Executable::Ivm(IvmBytecode::from_compiled(Vec::new())),
            Repeats::Indefinitely,
            account(1),
            EventFilterBoxC10V1::ExecuteTrigger(
                ExecuteTriggerEventFilter::new().under_authority(account(2))
            ),
        ),
        Err(ActionValidationError::ExecuteTriggerAuthorityMismatch)
    );
    let bound = ActionC10V1::new(
        Executable::Ivm(IvmBytecode::from_compiled(Vec::new())),
        Repeats::Indefinitely,
        account(1),
        EventFilterBoxC10V1::ExecuteTrigger(ExecuteTriggerEventFilter::new()),
    )
    .expect("an unbound execute-trigger filter binds to the authority");
    let EventFilterBoxC10V1::ExecuteTrigger(filter) = bound.filter() else {
        panic!("execute-trigger filter");
    };
    assert_eq!(filter.authority(), Some(&account(1)));
}

#[test]
fn action_encoding_is_its_fields_and_decoding_refuses_an_invalid_action() {
    let action = action();
    assert_eq!(
        action.encode(),
        super::trigger::tests_support::candidate_bytes(&action),
        "the borrowed encoding is the owned fields' encoding"
    );
    let invalid = super::trigger::tests_support::invalid_candidate_bytes(&action);
    assert!(ActionC10V1::decode_all(&mut invalid.as_slice()).is_err());
}

#[test]
fn results_and_registrations_roundtrip_framed_bare_and_json() {
    roundtrip_all(&TransactionResultC10V1(
        Ok(Vec::new()),
        Vec::new(),
        None,
        ContractEmissionsV1::default(),
    ));
    let TransactionStatusC10V1::Rejected(reason) = c10_rejection() else {
        unreachable!()
    };
    roundtrip_all(&TransactionResultC10V1(
        Err(*reason),
        Vec::new(),
        None,
        ContractEmissionsV1::default(),
    ));
    roundtrip_all(&RegisterSmartContractCodeC10V1 {
        artifact_id: ContractArtifactId::new(DataSpaceId::new(0), Hash::new(b"code")),
        manifest: manifest(Some(vec![entrypoint()])),
    });
    roundtrip_all(&RegisterTriggerC10V1 {
        object: TriggerC10V1 {
            id: "c10_trigger".parse().expect("trigger id"),
            action: action(),
        },
    });
}

#[test]
fn c10_registration_wire_ids_are_unique_and_decodable() {
    let registered: std::collections::BTreeSet<&str> =
        crate::isi::registry::built_in_instruction_schemas()
            .into_iter()
            .map(|instruction| instruction.wire_id)
            .collect();
    for wire_id in [
        RegisterSmartContractCodeC10V1::WIRE_ID,
        RegisterTriggerC10V1::WIRE_ID,
    ] {
        assert!(
            registered.contains(wire_id),
            "missing C10 registration decoder for {wire_id}"
        );
    }
    assert_ne!(
        RegisterSmartContractCodeC10V1::WIRE_ID,
        RegisterTriggerC10V1::WIRE_ID
    );
}

#[test]
fn every_new_family_has_its_own_schema_identity() {
    use norito::schema::identity::frame_hash;
    let pairs = [
        (
            frame_hash::<TransactionEventC10V1>(),
            frame_hash::<TransactionEvent>(),
        ),
        (
            frame_hash::<PipelineEventBoxC10V1>(),
            frame_hash::<crate::events::pipeline::PipelineEventBox>(),
        ),
        (
            frame_hash::<EventBoxC10V1>(),
            frame_hash::<crate::events::EventBox>(),
        ),
        (
            frame_hash::<TransactionEventFilterC10V1>(),
            frame_hash::<TransactionEventFilter>(),
        ),
        (
            frame_hash::<PipelineEventFilterBoxC10V1>(),
            frame_hash::<crate::events::pipeline::PipelineEventFilterBox>(),
        ),
        (
            frame_hash::<EventFilterBoxC10V1>(),
            frame_hash::<EventFilterBox>(),
        ),
        (
            frame_hash::<ContractManifestC10V1>(),
            frame_hash::<ContractManifest>(),
        ),
        (
            frame_hash::<ActionC10V1>(),
            frame_hash::<crate::trigger::action::Action>(),
        ),
        (
            frame_hash::<TriggerC10V1>(),
            frame_hash::<crate::trigger::Trigger>(),
        ),
        (
            frame_hash::<TransactionResultC10V1>(),
            frame_hash::<TransactionResult>(),
        ),
        (
            frame_hash::<RegisterSmartContractCodeC10V1>(),
            frame_hash::<crate::isi::smart_contract_code::RegisterSmartContractCode>(),
        ),
    ];
    let mut identities = std::collections::BTreeSet::new();
    for (c10, baseline) in pairs {
        assert_ne!(
            c10, baseline,
            "a C10 family never reuses a baseline identity"
        );
        assert!(identities.insert(c10), "C10 identities are distinct");
    }
    // The baseline descriptors have no schema identity of their own; the C10 ones do.
    for c10 in [
        frame_hash::<TriggerDescriptorC10V1>(),
        frame_hash::<EntrypointDescriptorC10V1>(),
        frame_hash::<RegisterTriggerC10V1>(),
    ] {
        assert!(identities.insert(c10), "C10 identities are distinct");
    }
}

#[test]
fn c10_provenance_signs_every_field_and_requires_its_own_frame() {
    let owner = iroha_allocation::AllocationBudget::new(1 << 20);
    let grant = owner.try_reserve_bytes(1 << 18).unwrap();
    let ctx = norito::core::DecodeBudgetContext::try_new_owned(
        norito::core::DecodeLimits::new(65536, 65536, 65536, grant.remaining_bytes(), 256),
        &owner,
    )
    .unwrap();
    let keys = iroha_crypto::KeyPair::try_random().unwrap();
    let baseline = baseline_manifest();
    let old_frame = baseline.signature_payload_bytes(&ctx, 65536).unwrap();
    let compiled = ContractManifestC10V1::try_from(baseline.clone()).unwrap();
    let frame = compiled.signature_payload_bytes(&ctx, 65536).unwrap();
    assert_ne!(old_frame, frame);
    assert!(compiled.signature_payload_bytes(&ctx, 1).is_err());
    let signed = compiled.clone().try_signed(&ctx, 65536, &keys).unwrap();
    let provenance = signed.provenance.as_ref().unwrap();
    assert!(
        provenance
            .signature
            .verify(keys.public_key(), &frame)
            .is_ok()
    );
    assert!(
        provenance
            .signature
            .verify(keys.public_key(), &old_frame)
            .is_err()
    );
    assert!(signed.same_signed_content(&compiled));
    let mut modified = compiled;
    modified.seiyaku_name = Some("changed".into());
    assert!(!signed.same_signed_content(&modified));
    assert!(
        ContractManifestC10V1::try_from(baseline.try_signed(&ctx, 65536, &keys).unwrap()).is_err()
    );
}

#[test]
fn c10_stored_action_rekey_preserves_retry_and_rebinds_exact_filter_authority() {
    use crate::events::{
        data::prelude::AccountEventFilter, execute_trigger::ExecuteTriggerEventFilter,
    };
    let old = account(71);
    let new = account(72);
    let id = "owned_c10".parse::<crate::trigger::TriggerId>().unwrap();
    for filter in [
        EventFilterBoxC10V1::Data(AccountEventFilter::new().for_account(old.clone()).into()),
        EventFilterBoxC10V1::ExecuteTrigger(
            ExecuteTriggerEventFilter::new().for_trigger(id.clone()),
        ),
    ] {
        let mut row = StoredC10TriggerV1::new(TriggerC10V1 {
            id: id.clone(),
            action: ActionC10V1::new(
                crate::transaction::Executable::Instructions(Vec::new().into()),
                crate::trigger::action::Repeats::Exactly(3),
                old.clone(),
                filter.clone(),
            )
            .unwrap(),
        });
        let before = row.action_hash().unwrap();
        assert_eq!(
            row.replace_account_id(&old, &new),
            matches!(filter, EventFilterBoxC10V1::Data(_))
        );
        assert_ne!(row.action_hash().unwrap(), before);
        assert_eq!(row.trigger().action.authority(), &new);
        match row.trigger().action.filter() {
            EventFilterBoxC10V1::Data(f) => {
                assert!(f.references_account_id(&new));
                assert!(!f.references_account_id(&old));
            }
            EventFilterBoxC10V1::ExecuteTrigger(f) => assert_eq!(f.authority(), Some(&new)),
            _ => unreachable!(),
        }
        assert_eq!(
            row.trigger().action.repeats(),
            crate::trigger::action::Repeats::Exactly(3)
        );
        assert!(row.retry_state().is_none());
        roundtrip_all(&row);
    }
}
