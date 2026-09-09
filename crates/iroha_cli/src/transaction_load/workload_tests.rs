//! Independent effect, complete-cohort bounds and exact postcondition regression cases.

use super::*;
use iroha::data_model::{
    account::AccountAlias,
    isi::SetKeyValueBox,
    nexus::DataSpaceId,
    parameter::{CustomParameter, Parameter},
};
use std::sync::Mutex;

#[derive(Default)]
struct Events(Mutex<Vec<Value>>);
impl Recorder for Events {
    fn record(&self, event: Value) -> Result<()> {
        self.0.lock().expect("events").push(event);
        Ok(())
    }
}
fn account(index: u8) -> Account {
    let keys = KeyPair::try_from_seed(vec![index; 32], Algorithm::Ed25519).expect("fixture key");
    let id = AccountId::new(keys.public_key().clone());
    // Register the canonical identity before adding any optional metadata or alias state.
    Account::new(id.clone()).build(&id)
}
fn schedule(rate: u128, warmup_seconds: i64, measurement_seconds: i64) -> Schedule {
    Schedule {
        rate_numerator: rate,
        rate_denominator: 1,
        warmup_ns: warmup_seconds * NS,
        measurement_ns: measurement_seconds * NS,
        drain_ns: NS,
        lag_ns: 0,
    }
}
fn rows(schedule: &Schedule, accounts: usize) -> Vec<Record> {
    schedule
        .plan(&"a".repeat(64), accounts)
        .expect("planned rows")
}
fn settle(records: &mut [Record]) {
    for record in records {
        record.submission_finished = true;
        record.applied = Some((0, 3));
    }
}
fn accounts() -> Vec<Account> {
    (1..=4).map(account).collect()
}

#[test]
fn round_robin_preserves_logical_schedule_and_exact_per_cohort_route_quotas() {
    for count in [4, 8, 64] {
        let schedule = schedule(count as u128, 1, 2);
        validate_schedule(&schedule, count).expect("complete pool rounds");
        let planned = rows(&schedule, count);
        let mut warmup = vec![0; count];
        let mut measurement = vec![0; count];
        for row in &planned {
            let phase = if row.plan.cohort == Cohort::Warmup {
                &mut warmup
            } else {
                &mut measurement
            };
            phase[row.plan.account_index] += 1;
            let logical = hex::encode(Sha256::digest(
                format!(
                    "{}:{}:{}",
                    "a".repeat(64),
                    row.plan.cohort.text(),
                    row.plan.sequence
                )
                .as_bytes(),
            ));
            assert_eq!(row.plan.logical_id, logical);
        }
        assert_eq!(warmup, vec![1; count]);
        assert_eq!(measurement, vec![2; count]);
        let mut lanes = [0_usize; 4];
        for row in &planned {
            lanes[row.plan.account_index % 4] += 1;
        }
        assert_eq!(lanes, [planned.len() / 4; 4]);
        let paired = rows(&schedule, count);
        assert!(
            planned
                .iter()
                .zip(paired)
                .all(
                    |(one, four)| one.plan.account_index == four.plan.account_index
                        && one.plan.scheduled_offset_ns == four.plan.scheduled_offset_ns
                )
        );
    }
}

#[test]
fn complete_cohort_cap_includes_warmup_and_preserves_zero_warmup() {
    validate_schedule(&schedule(4, 0, 1), 4).expect("zero warmup remains valid");
    validate_schedule(&schedule(4, 512, 512), 4).expect("exact whole-trial per-account cap");
    assert!(validate_schedule(&schedule(4, 513, 512), 4).is_err());
    assert!(validate_schedule(&schedule(4, 0, 0), 4).is_err());
    assert!(validate_schedule(&schedule(3, 1, 4), 4).is_err());
    assert!(validate_schedule(&schedule(4, 1, 1), 8).is_err());
    for invalid in [0, 1, 3, 65, 68] {
        assert!(validate_schedule(&schedule(4, 1, 1), invalid).is_err());
    }
    assert!(account_offset("seed", 0).is_err());
    assert!(account_offset("seed", 65).is_err());
}

#[test]
fn workload_executable_is_one_self_owned_real_metadata_write() {
    let owner = account(1);
    let plans = rows(&schedule(4, 0, 1), 4);
    let Executable::Instructions(instructions) =
        executable(owner.id(), &plans[0].plan).expect("executable")
    else {
        panic!("expected native instructions")
    };
    assert_eq!(instructions.len(), 1);
    let set = instructions[0]
        .as_any()
        .downcast_ref::<SetKeyValueBox>()
        .expect("metadata instruction");
    let SetKeyValueBox::Account(set) = set else {
        panic!("account target required")
    };
    assert_eq!(&set.object, owner.id());
    assert_eq!(
        set.key.as_ref(),
        format!("gscale_{}", plans[0].plan.logical_id)
    );
    assert_eq!(set.key.as_ref().len(), 71);
    assert_eq!(set.value.as_ref().len(), 64);
    assert_eq!(
        norito::json::to_json(&set.value)
            .expect("canonical JSON")
            .len(),
        66
    );
    assert_eq!(
        set.value
            .try_into_any_norito::<String>()
            .expect("string value"),
        plans[0].plan.logical_id
    );
    assert_ne!(
        effect(&plans[0].plan).unwrap().0,
        effect(&plans[1].plan).unwrap().0
    );
}

#[test]
fn effect_identity_and_live_metadata_value_bound_fail_before_collection() {
    let mut plan = rows(&schedule(4, 0, 1), 4).remove(0).plan;
    for invalid in [
        "A".repeat(64),
        "g".repeat(64),
        "0".repeat(63),
        "0".repeat(65),
    ] {
        plan.logical_id = invalid;
        assert!(effect(&plan).is_err());
    }
    let mut parameters = Parameters::default();
    validate_value_limit(&parameters).expect("instruction default");
    let key = CustomParameterId("max_metadata_value_bytes".parse().unwrap());
    parameters.set_parameter(Parameter::Custom(CustomParameter::new(
        key.clone(),
        Json::new(63_u64),
    )));
    assert!(validate_value_limit(&parameters).is_err());
    parameters.set_parameter(Parameter::Custom(CustomParameter::new(
        key.clone(),
        Json::new(64_u64),
    )));
    validate_value_limit(&parameters).expect("exact value boundary");
    parameters.set_parameter(Parameter::Custom(CustomParameter::new(
        key,
        Json::new("malformed numeric custom parameter"),
    )));
    validate_value_limit(&parameters).expect("same instruction fallback semantics");
}

#[test]
fn baseline_rejects_existing_workload_keys_aliases_wrong_identity_and_large_maps() {
    let owner = account(1);
    require_baseline(&owner, owner.id()).expect("fresh universal account");
    assert!(require_baseline(&owner, account(2).id()).is_err());
    let mut reused = owner.clone();
    reused
        .metadata
        .insert("gscale_old".parse().unwrap(), Json::new("old"));
    assert!(require_baseline(&reused, owner.id()).is_err());
    let mut aliased = owner.clone();
    aliased.label = Some(AccountAlias::domainless(
        "alice".parse().expect("alias label"),
        DataSpaceId::new(1),
    ));
    assert!(require_baseline(&aliased, owner.id()).is_err());
    let mut oversized = owner.clone();
    oversized.metadata.insert(
        "baseline".parse().unwrap(),
        Json::new("x".repeat(MAX_BASELINE_FRAME_BYTES)),
    );
    assert!(require_baseline(&oversized, owner.id()).is_err());
}

#[test]
fn maximum_complete_post_account_passes_real_codec_and_allocation_preflight() {
    let owner = account(1);
    let planned = rows(&schedule(4, 512, 512), 4);
    let (expected, count) =
        expected_account(&owner, &planned, 0).expect("maximum actual post-account");
    assert_eq!(count, MAX_EFFECTS_PER_ACCOUNT);
    assert_eq!(expected.metadata().iter().len(), MAX_EFFECTS_PER_ACCOUNT);
    let frame = checked_account(&expected, MAX_ACCOUNT_FRAME_BYTES).expect("bounded actual codec");
    assert!(frame.len() < MAX_ACCOUNT_FRAME_BYTES);
    assert!(checked_account(&expected, frame.len() - 1).is_err());
    let mut duplicate = rows(&schedule(4, 0, 2), 4);
    duplicate[4].plan.logical_id = duplicate[0].plan.logical_id.clone();
    assert!(expected_account(&owner, &duplicate, duplicate[0].plan.account_index).is_err());
}

#[test]
fn complete_effect_check_rejects_missing_wrong_extra_and_final_overwrite_states() {
    let mut owner = account(1);
    owner
        .metadata
        .insert("preexisting".parse().unwrap(), Json::new(7_u64));
    let planned = rows(&schedule(4, 1, 2), 4);
    let (expected, count) = expected_account(&owner, &planned, 0).expect("complete state");
    assert_eq!(count, 3);
    let digest = verify_account(&expected, &expected).expect("exact complete state");
    assert_eq!(digest.len(), 64);
    // Account equality alone would wrongly accept every one of these states.
    assert_eq!(&expected, &owner);
    assert!(verify_account(&expected, &owner).is_err());
    let key = planned
        .iter()
        .find(|row| row.plan.account_index == 0)
        .map(|row| effect(&row.plan).unwrap().0)
        .unwrap();
    let mut wrong = expected.clone();
    wrong.metadata.insert(key, Json::new("wrong"));
    assert!(verify_account(&expected, &wrong).is_err());
    let mut extra = expected.clone();
    extra
        .metadata
        .insert("gscale_unexpected".parse().unwrap(), Json::new("extra"));
    assert!(verify_account(&expected, &extra).is_err());
    let mut changed_baseline = expected.clone();
    changed_baseline
        .metadata
        .insert("preexisting".parse().unwrap(), Json::new(8_u64));
    assert!(verify_account(&expected, &changed_baseline).is_err());
    let last = planned
        .iter()
        .rev()
        .find(|row| row.plan.account_index == 0)
        .unwrap();
    let (last_key, last_value) = effect(&last.plan).unwrap();
    let mut final_only = owner.clone();
    final_only.metadata.insert(last_key, last_value);
    assert!(verify_account(&expected, &final_only).is_err());
}

#[test]
fn account_preflight_and_postreads_cover_every_warmup_and_measurement_effect() {
    let pool = accounts();
    let authorities: Vec<_> = pool.iter().map(Account::id).collect();
    let mut planned = rows(&schedule(4, 1, 2), 4);
    let events = Events::default();
    let mut reads = Vec::new();
    let baselines = preflight_accounts(
        &authorities,
        &planned,
        &Parameters::default(),
        &events,
        |index, id| {
            reads.push(index);
            assert_eq!(pool[index].id(), id);
            Ok(pool[index].clone())
        },
    )
    .expect("preflight");
    assert_eq!(reads, vec![0, 1, 2, 3]);
    let mut called = false;
    assert!(
        verify_accounts(&authorities, &planned, &baselines, &events, |_, _| {
            called = true;
            unreachable!("unsettled cannot read")
        })
        .is_err()
    );
    assert!(!called);
    settle(&mut planned);
    reads.clear();
    verify_accounts(&authorities, &planned, &baselines, &events, |index, _| {
        reads.push(index);
        Ok(expected_account(&pool[index], &planned, index)?.0)
    })
    .expect("all individual effects verified");
    assert_eq!(reads, vec![0, 1, 2, 3]);
    assert_eq!(events.0.lock().unwrap().len(), 8);
}

#[test]
fn verification_rejects_pool_changes_duplicate_logical_rows_and_partial_read_failures() {
    let pool = accounts();
    let authorities: Vec<_> = pool.iter().map(Account::id).collect();
    let mut planned = rows(&schedule(4, 0, 1), 4);
    let events = Events::default();
    let baselines = preflight_accounts(
        &authorities,
        &planned,
        &Parameters::default(),
        &events,
        |index, _| Ok(pool[index].clone()),
    )
    .unwrap();
    assert!(validate_pool(&[authorities[0], authorities[0]], &planned).is_err());
    assert!(
        validate_pool(
            &[
                authorities[0],
                authorities[0],
                authorities[2],
                authorities[3]
            ],
            &planned
        )
        .is_err()
    );
    planned[1].plan.logical_id = planned[0].plan.logical_id.clone();
    assert!(validate_pool(&authorities, &planned).is_err());
    planned = rows(&schedule(4, 0, 1), 4);
    planned[0].plan.account_index = pool.len();
    assert!(validate_pool(&authorities, &planned).is_err());
    planned = rows(&schedule(4, 0, 1), 4);
    settle(&mut planned);
    let mut read_indices = Vec::new();
    assert!(
        verify_accounts(&authorities, &planned, &baselines, &events, |index, _| {
            read_indices.push(index);
            if index == 1 {
                bail!("read failed");
            }
            Ok(expected_account(&pool[index], &planned, index)?.0)
        })
        .is_err()
    );
    assert_eq!(read_indices, vec![0, 1]);
    let mut reversed = authorities.clone();
    reversed.reverse();
    assert!(
        verify_accounts(
            &reversed,
            &planned,
            &baselines,
            &events,
            |_, _| unreachable!("changed pool")
        )
        .is_err()
    );
}

#[test]
fn workload_metadata_limit_uses_actual_instruction_view_at_both_boundaries() {
    let plan = rows(&schedule(4, 0, 1), 4).remove(0).plan;
    let (_, value) = effect(&plan).expect("actual workload value");
    let measured = u64::try_from(value.as_ref().len()).expect("bounded value");
    assert_eq!(measured, 64);
    assert_eq!(
        norito::json::to_json(&value).expect("canonical serialized value"),
        format!("\"{}\"", plan.logical_id)
    );
    for limit in [0, measured - 1, measured, measured + 1, u64::MAX] {
        let mut parameters = Parameters::default();
        parameters.set_parameter(Parameter::Custom(CustomParameter::new(
            CustomParameterId("max_metadata_value_bytes".parse().expect("parameter name")),
            Json::new(limit),
        )));
        // The instruction compares value.as_ref().len(), not the quoted JSON text.
        assert_eq!(validate_value_limit(&parameters).is_ok(), measured <= limit);
    }
}

#[test]
fn workload_decode_policy_is_bounded_by_actual_frame_geometry() {
    for length in [
        1,
        MAX_BASELINE_FRAME_BYTES,
        MAX_ACCOUNT_FRAME_BYTES,
        MAX_QUERY_RESPONSE_BYTES,
    ] {
        let limits = workload_decode_limits(length).expect("admitted frame length");
        let sdk = norito::canonical_decode_limits(length);
        assert_eq!(limits.max_sequence_elements(), length * 8);
        assert_eq!(limits.max_field_bytes(), length);
        assert_eq!(limits.max_total_elements(), length * 8);
        assert_eq!(limits.max_nesting_depth(), 64);
        assert_eq!(
            limits.max_total_allocated_bytes(),
            sdk.max_total_allocated_bytes()
        );
        assert_eq!(limits.max_total_allocated_bytes(), length * 64 + 64 * 1024);
    }
    assert_eq!(
        workload_decode_limits(MAX_QUERY_RESPONSE_BYTES)
            .unwrap()
            .max_total_allocated_bytes(),
        32 * 1024 * 1024 + 64 * 1024
    );
    assert!(workload_decode_limits(0).is_err());
    assert!(workload_decode_limits(MAX_QUERY_RESPONSE_BYTES + 1).is_err());
    assert!(workload_decode_limits(usize::MAX).is_err());
    assert!(checked_frame(&account(1), 0).is_err());
    assert!(checked_frame(&account(1), MAX_QUERY_RESPONSE_BYTES + 1).is_err());
}

#[test]
fn full_workload_account_query_response_uses_real_sdk_decoder_and_keeps_effect_cap() {
    let mut owner = account(1);
    owner.metadata.insert(
        "retained_baseline".parse().unwrap(),
        Json::new("x".repeat(4096)),
    );
    require_baseline(&owner, owner.id()).expect("bounded nonempty baseline");
    let planned = rows(&schedule(4, 512, 512), 4);
    let (expected, count) = expected_account(&owner, &planned, 0).expect("all 1024 effects");
    assert_eq!(count, MAX_EFFECTS_PER_ACCOUNT);
    assert_eq!(
        expected.metadata().iter().len(),
        MAX_EFFECTS_PER_ACCOUNT + 1
    );
    let expected_frame = checked_account(&expected, MAX_ACCOUNT_FRAME_BYTES).expect("full account");
    let response = QueryResponse::Singular(SingularQueryOutputBox::Account(expected));
    let frame = checked_frame(&response, MAX_QUERY_RESPONSE_BYTES).expect("full query response");
    // This is the decoder invoked by iroha::query::decode_query_response_body.
    let decoded =
        norito::decode_from_bytes::<QueryResponse>(&frame).expect("actual SDK decode policy");
    let QueryResponse::Singular(SingularQueryOutputBox::Account(observed)) = decoded else {
        panic!("exact account response required")
    };
    assert_eq!(
        checked_account(&observed, MAX_ACCOUNT_FRAME_BYTES).unwrap(),
        expected_frame
    );
    assert!(frame.len() <= MAX_QUERY_RESPONSE_BYTES);
    assert!(checked_frame(&response, frame.len() - 1).is_err());
    let too_many = rows(&schedule(4, 512, 513), 4);
    assert!(expected_account(&owner, &too_many, 0).is_err());
}

#[test]
fn real_account_decoder_still_enforces_allocation_field_element_and_depth_limits() {
    let owner = account(1);
    let frame = checked_account(&owner, MAX_ACCOUNT_FRAME_BYTES).expect("valid account frame");
    let valid = workload_decode_limits(frame.len()).unwrap();
    let decoded =
        norito::decode_from_bytes_with_limits::<Account>(&frame, valid).expect("valid decode");
    assert_eq!(
        checked_account(&decoded, MAX_ACCOUNT_FRAME_BYTES).unwrap(),
        frame
    );
    let allocation_zero = norito::DecodeLimits::new(
        valid.max_sequence_elements(),
        valid.max_field_bytes(),
        valid.max_total_elements(),
        0,
        64,
    );
    assert!(matches!(
        norito::decode_from_bytes_with_limits::<Account>(&frame, allocation_zero),
        Err(norito::Error::TotalAllocationExceeded { limit: 0, .. })
    ));
    for restricted in [
        norito::DecodeLimits::new(
            valid.max_sequence_elements(),
            1,
            valid.max_total_elements(),
            valid.max_total_allocated_bytes(),
            64,
        ),
        norito::DecodeLimits::new(
            0,
            valid.max_field_bytes(),
            0,
            valid.max_total_allocated_bytes(),
            64,
        ),
        norito::DecodeLimits::new(
            valid.max_sequence_elements(),
            valid.max_field_bytes(),
            valid.max_total_elements(),
            valid.max_total_allocated_bytes(),
            0,
        ),
    ] {
        assert!(norito::decode_from_bytes_with_limits::<Account>(&frame, restricted).is_err());
    }
}
