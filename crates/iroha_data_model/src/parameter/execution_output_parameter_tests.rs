//! Complete output-capacity parameter codecs; no State admission claims.

use std::num::{NonZeroU32, NonZeroU64};

use norito::{
    codec::{DecodeAll as _, Encode as _},
    json::{self, BoundedJsonError, JsonDeserialize, JsonSerialize, Value},
};

use crate::parameter::{
    BlockParameter, BlockParameters, ExecutionOutputPolicyV1, Parameter, Parameters,
};

fn policy() -> ExecutionOutputPolicyV1 {
    // All seven fields differ from bootstrap, but remain a valid logical plan.
    // This fixture does not establish complete source/metadata/host feasibility.
    ExecutionOutputPolicyV1 {
        max_outputs: 128,
        max_output_bytes: 32_768,
        max_total_output_bytes: 2 * 1024 * 1024,
        max_executed_wire_bytes: 8 * 1024 * 1024,
        max_pipeline_triggers: 3,
        max_time_triggers: 19,
        max_time_invocations: 7,
    }
}

fn parameters() -> Parameters {
    let mut parameters = Parameters::default();
    for value in [
        BlockParameter::MaxTransactions(NonZeroU64::new(11).unwrap()),
        BlockParameter::MaxTimeTriggerInvocations(NonZeroU32::new(5).unwrap()),
        BlockParameter::ExecutionOutput(policy()),
    ] {
        parameters.set_parameter(Parameter::Block(value));
    }
    parameters
}

fn assert_complete_codecs<T>(value: &T)
where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize
        + std::fmt::Debug
        + PartialEq,
{
    let raw = value.encode();
    assert_eq!(T::decode_all(&mut raw.as_slice()).unwrap(), *value);
    let frame = norito::encode_canonical(value).unwrap();
    let decoded: T = norito::decode_canonical(&frame).unwrap();
    assert_eq!(decoded, *value);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    let ordinary_json = json::to_json(value).unwrap();
    assert_eq!(json::from_str::<T>(&ordinary_json).unwrap(), *value);
    assert_eq!(
        json::to_json_bounded(value, ordinary_json.len()).unwrap(),
        ordinary_json
    );
    assert_eq!(
        json::to_json_bounded_boxed(value, ordinary_json.len())
            .unwrap()
            .as_ref(),
        ordinary_json.as_bytes()
    );
    assert!(matches!(
        json::to_json_bounded(value, ordinary_json.len() - 1),
        Err(BoundedJsonError::BodyTooLarge)
    ));
    assert!(matches!(
        json::to_json_bounded(value, 0),
        Err(BoundedJsonError::BodyTooLarge)
    ));
}

#[test]
fn block_parameters_bootstrap_time_is_independent_of_network_count() {
    let expected = ExecutionOutputPolicyV1::bootstrap();
    expected.validate().unwrap();
    for network in [1, 17, u64::MAX] {
        let block = BlockParameters::new(NonZeroU64::new(network).unwrap());
        assert_eq!(block.max_transactions().get(), network);
        assert_eq!(block.max_time_trigger_invocations().get(), 512);
        assert_eq!(block.execution_output(), expected);
        expected
            .validate_time_invocations(block.max_time_trigger_invocations().get())
            .unwrap();
    }
    let default = BlockParameters::default();
    assert_eq!(default.execution_output(), expected);
    assert_eq!(default.max_time_trigger_invocations().get(), 512);
}

#[test]
fn block_parameters_roundtrip_complete_policy_in_all_codec_writers() {
    let parameters = parameters();
    let block = parameters.block();
    assert_eq!(block.max_transactions().get(), 11);
    assert_eq!(block.max_time_trigger_invocations().get(), 5);
    assert_eq!(block.execution_output(), policy());
    policy().validate().unwrap();
    assert_complete_codecs(&policy());
    assert_complete_codecs(&block);
    assert_complete_codecs(&parameters);
    let json = json::to_value(&block).unwrap();
    let mut keys: Vec<_> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "execution_output",
            "max_time_trigger_invocations",
            "max_transactions"
        ]
    );
}

#[test]
fn block_parameters_json_requires_each_new_field_and_rejects_unknowns() {
    let current = json::to_value(&parameters().block()).unwrap();
    for missing in ["max_time_trigger_invocations", "execution_output"] {
        let mut absent = current.clone();
        absent.as_object_mut().unwrap().remove(missing);
        assert!(json::from_value::<BlockParameters>(absent).is_err());
        let mut null = current.clone();
        null.as_object_mut()
            .unwrap()
            .insert(missing.into(), Value::Null);
        assert!(json::from_value::<BlockParameters>(null).is_err());
    }
    for invalid in [0_u64, u64::from(u32::MAX) + 1] {
        let mut changed = current.clone();
        changed
            .as_object_mut()
            .unwrap()
            .insert("max_time_trigger_invocations".into(), Value::from(invalid));
        assert!(json::from_value::<BlockParameters>(changed).is_err());
    }
    let mut unknown = current.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("local_memory_override".into(), Value::from(1_u64));
    assert!(json::from_value::<BlockParameters>(unknown).is_err());

    // Preserve the pre-existing optional Network cap JSON behavior. Both new
    // fields are still present, explicit and fully decoded in this case.
    let mut no_network_cap = current;
    no_network_cap
        .as_object_mut()
        .unwrap()
        .remove("max_transactions");
    let decoded = json::from_value::<BlockParameters>(no_network_cap).unwrap();
    assert_eq!(
        decoded.max_transactions(),
        BlockParameters::default().max_transactions()
    );
    assert_eq!(decoded.max_time_trigger_invocations().get(), 5);
    assert_eq!(decoded.execution_output(), policy());
}

#[test]
fn retired_one_field_block_parameters_fail_json_raw_and_canonical_frames() {
    #[derive(norito::Encode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::parameter::system::model::BlockParameters")]
    struct RetiredBlockParameters {
        max_transactions: NonZeroU64,
    }

    // The nominal identity did not change. This must reject the obsolete payload
    // itself, not pass merely because the frame advertises another type.
    assert_eq!(
        norito::schema::identity::frame_hash::<RetiredBlockParameters>(),
        norito::schema::identity::frame_hash::<BlockParameters>()
    );
    for maximum in [1, 512, u64::MAX] {
        let retired = RetiredBlockParameters {
            max_transactions: NonZeroU64::new(maximum).unwrap(),
        };
        let raw = retired.encode();
        assert!(BlockParameters::decode_all(&mut raw.as_slice()).is_err());
        assert!(
            norito::decode_canonical::<BlockParameters>(
                &norito::encode_canonical(&retired).unwrap()
            )
            .is_err()
        );
        let old_json = format!(r#"{{"max_transactions":{maximum}}}"#);
        assert!(json::from_str::<BlockParameters>(&old_json).is_err());
        let mut parent = json::to_value(&parameters()).unwrap();
        parent
            .as_object_mut()
            .unwrap()
            .insert("block".into(), json::from_str::<Value>(&old_json).unwrap());
        assert!(json::from_value::<Parameters>(parent).is_err());
    }
}

#[test]
fn block_parameter_variants_roundtrip_and_reject_mixed_or_unknown_tags() {
    let variants = [
        BlockParameter::MaxTransactions(NonZeroU64::new(11).unwrap()),
        BlockParameter::MaxTimeTriggerInvocations(NonZeroU32::new(5).unwrap()),
        BlockParameter::ExecutionOutput(policy()),
    ];
    for (expected_tag, value) in [
        "MaxTransactions",
        "MaxTimeTriggerInvocations",
        "ExecutionOutput",
    ]
    .into_iter()
    .zip(variants)
    {
        assert_complete_codecs(&value);
        assert_complete_codecs(&Parameter::Block(value));
        let encoded_json = json::to_value(&value).unwrap();
        let object = encoded_json.as_object().unwrap();
        assert_eq!(object.len(), 1);
        assert!(object.contains_key(expected_tag));
        let mut unknown_variant = value.encode();
        unknown_variant[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(BlockParameter::decode_all(&mut unknown_variant.as_slice()).is_err());
    }
    for invalid in [
        r#"{}"#,
        r#"{"MaxTimeTriggerInvocations":0}"#,
        r#"{"MaxTimeTriggerInvocations":4294967296}"#,
        r#"{"MaxTimeTriggerInvocations":1,"MaxTransactions":1}"#,
        r#"{"ExecutionOutput":null}"#,
        r#"{"ExecutionOutput":{}}"#,
        r#"{"LocalOutputOverride":1}"#,
    ] {
        assert!(json::from_str::<BlockParameter>(invalid).is_err());
    }
}

#[test]
fn parameters_getter_setter_and_enumeration_retain_all_three_block_values() {
    let parameters = parameters();
    let expected = vec![
        BlockParameter::MaxTransactions(NonZeroU64::new(11).unwrap()),
        BlockParameter::MaxTimeTriggerInvocations(NonZeroU32::new(5).unwrap()),
        BlockParameter::ExecutionOutput(policy()),
    ];
    assert_eq!(
        parameters.block().parameters().collect::<Vec<_>>(),
        expected
    );
    let all: Vec<_> = parameters.parameters().collect();
    let block: Vec<_> = all
        .iter()
        .filter_map(|value| match value {
            Parameter::Block(value) => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(block, expected);
    let restored: Parameters = all.into_iter().collect();
    assert_eq!(restored, parameters);
    assert_complete_codecs(&restored);
}

#[test]
fn network_time_and_envelope_setters_do_not_reinterpret_each_other() {
    let mut parameters = parameters();
    let capacity = parameters.block().execution_output();
    for network in [1, u64::MAX] {
        parameters.set_parameter(Parameter::Block(BlockParameter::MaxTransactions(
            NonZeroU64::new(network).unwrap(),
        )));
        assert_eq!(parameters.block().max_transactions().get(), network);
        assert_eq!(parameters.block().max_time_trigger_invocations().get(), 5);
        assert_eq!(parameters.block().execution_output(), capacity);
    }
    for time in [1, capacity.max_time_invocations] {
        parameters.set_parameter(Parameter::Block(BlockParameter::MaxTimeTriggerInvocations(
            NonZeroU32::new(time).unwrap(),
        )));
        assert_eq!(
            parameters.block().max_time_trigger_invocations().get(),
            time
        );
        assert_eq!(parameters.block().max_transactions().get(), u64::MAX);
        assert_eq!(parameters.block().execution_output(), capacity);
        capacity.validate_time_invocations(time).unwrap();
    }
    assert!(capacity.validate_time_invocations(0).is_err());
    assert!(
        capacity
            .validate_time_invocations(capacity.max_time_invocations + 1)
            .is_err()
    );
    let mut changed = capacity;
    changed.max_time_invocations += 1;
    changed.validate().unwrap();
    // Pure Parameters mutation transports an atomic value. Post-genesis refusal
    // and actual State authority belong to Core, not this infallible model setter.
    parameters.set_parameter(Parameter::Block(BlockParameter::ExecutionOutput(changed)));
    assert_eq!(parameters.block().execution_output(), changed);
    assert_eq!(parameters.block().max_time_trigger_invocations().get(), 7);
    assert_eq!(parameters.block().max_transactions().get(), u64::MAX);
}

#[test]
fn atomic_output_policy_requires_every_field_and_has_one_declared_identity() {
    assert_eq!(
        <ExecutionOutputPolicyV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_data_model::parameter::execution_output::ExecutionOutputPolicyV1"
    );
    let current = json::to_value(&policy()).unwrap();
    let expected = [
        "max_outputs",
        "max_output_bytes",
        "max_total_output_bytes",
        "max_executed_wire_bytes",
        "max_pipeline_triggers",
        "max_time_triggers",
        "max_time_invocations",
    ];
    assert_eq!(current.as_object().unwrap().len(), expected.len());
    for missing in expected {
        let mut absent = current.clone();
        assert!(absent.as_object_mut().unwrap().remove(missing).is_some());
        assert!(json::from_value::<ExecutionOutputPolicyV1>(absent).is_err());
        let mut null = current.clone();
        null.as_object_mut()
            .unwrap()
            .insert(missing.into(), Value::Null);
        assert!(json::from_value::<ExecutionOutputPolicyV1>(null).is_err());
    }
    assert_complete_codecs(&policy());
}
