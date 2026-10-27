//! Authored source controls only; no compilation or execution has occurred.
use super::*;
use crate::{
    HasMetadata,
    account::AccountId,
    events::time::{ExecutionTime, Schedule, TimeEventFilter},
    smart_contract::{
        ContractAddress,
        entrypoint::{EntrypointArgumentRecordV1, EntrypointValueAtomV1},
    },
    transaction::{
        Executable, ExecutableBatchItem,
        executable::{ContractArgumentRecord, ContractInvocation},
    },
    trigger::action::{Repeats, TimeTriggerRetryPolicy},
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_model_base::{metadata::Metadata, name::Name};
use iroha_primitives::{const_vec::ConstVec, json::Json};
use std::{
    num::{NonZeroU32, NonZeroU64},
    time::Duration,
};

const ADDRESS: &str = "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh";

fn limits() -> ActionScannerLimitsV1 {
    ActionScannerLimitsV1 {
        max_frame_bytes: 2_000_000,
        max_total_frame_bytes: 4_000_000,
        max_frames: 2,
        max_work: 20_000_000,
        max_fields: 1000,
        max_sequence_elements: 2_000_000,
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
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let mut output = Vec::new();
    ncore::serialize_to_buffer(value, &mut output).unwrap();
    output
}

fn metadata_wire(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut output = (entries.len() as u64).to_le_bytes().to_vec();
    for (name, text) in entries {
        let name: Name = name.parse().unwrap();
        let mut entry = field(&payload(&name));
        entry.extend(field(&field(&payload(text))));
        output.extend(field(&entry));
    }
    output
}

fn scan_metadata_bytes(bytes: &[u8]) -> Result<(), ScanError> {
    let mut budget = Budget::new(limits());
    scan_metadata(Cursor::root(bytes, &mut budget)?, &mut budget)
}

fn invocation_parts(address: &[u8], hash: &[u8], entrypoint: &[u8], arguments: &[u8]) -> Vec<u8> {
    [
        field(address),
        field(hash),
        field(entrypoint),
        field(arguments),
    ]
    .concat()
}

fn invocation_bytes(address: &str, hash: &[u8], name: &str, arguments: Option<Vec<u8>>) -> Vec<u8> {
    invocation_parts(
        &payload(&address),
        hash,
        &payload(&name),
        &payload(&arguments),
    )
}

fn scan_invocation_bytes(bytes: &[u8]) -> Result<(), ScanError> {
    let mut budget = Budget::new(limits());
    scan_contract_invocation(Cursor::root(bytes, &mut budget)?, &mut budget)
}

fn invocation() -> ContractInvocation {
    // Exact constructor shape retained from scheduled_contract_trigger.rs:123–129.
    let record = norito::to_bytes(&EntrypointArgumentRecordV1 {
        schema_hash: [7; 32],
        atoms: vec![EntrypointValueAtomV1::Unit],
    })
    .unwrap();
    ContractInvocation {
        contract_address: ADDRESS.parse().unwrap(),
        expected_code_hash: Hash::prehashed([1; 32]),
        entrypoint: "scheduled_payment".to_owned(),
        arguments: Some(ContractArgumentRecord::try_new(record).unwrap()),
    }
}

#[test]
fn original_numeric_json_boundaries_have_exact_decimal_wire_and_scan() {
    for value in [0, 1, u64::MAX] {
        let json = Json::from(value);
        assert_eq!(json.get(), &value.to_string());
        let mut metadata = Metadata::default();
        for name in ["__registered_at_ms", "__registered_block_height"] {
            metadata.insert(name.parse().unwrap(), json.clone());
        }
        assert!(scan_metadata_bytes(&payload(&metadata)).is_ok());
    }
}

#[test]
fn registration_numbers_refuse_noncanonical_spellings_and_other_keys() {
    for text in [
        "",
        "00",
        "01",
        "-0",
        "-1",
        "+1",
        "1.0",
        "1e0",
        " 1",
        "1 ",
        "18446744073709551616",
        "100000000000000000000",
        "１",
    ] {
        for name in ["__registered_at_ms", "__registered_block_height"] {
            assert_eq!(
                scan_metadata_bytes(&metadata_wire(&[(name, text)])),
                Err(ScanError::UnsupportedMetadata)
            );
        }
    }
    assert_eq!(
        scan_metadata_bytes(&metadata_wire(&[("other", "1")])),
        Err(ScanError::UnsupportedMetadata)
    );
    // This preexisting scanner subset is intentionally preserved even at reserved
    // names. The typed registration producer separately refuses quoted numbers.
    for text in ["\"1\"", "null", "true", "false", "[]", "{}"] {
        assert!(scan_metadata_bytes(&metadata_wire(&[("__registered_at_ms", text)])).is_ok());
    }
}

#[test]
fn registration_metadata_retains_order_and_full_consumption_checks() {
    for entries in [
        [
            ("__registered_block_height", "1"),
            ("__registered_at_ms", "2"),
        ],
        [("__registered_at_ms", "1"), ("__registered_at_ms", "2")],
    ] {
        assert_eq!(
            scan_metadata_bytes(&metadata_wire(&entries)),
            Err(ScanError::NonCanonical)
        );
    }
    let mut bytes = metadata_wire(&[("__registered_at_ms", "18446744073709551615")]);
    bytes.push(0);
    assert_eq!(scan_metadata_bytes(&bytes), Err(ScanError::Length));
}

#[test]
fn numeric_digit_scan_is_precharged_at_exact_boundary() {
    let mut bounds = limits();
    bounds.max_work = 19;
    assert_eq!(
        canonical_registration_u64("18446744073709551615", &mut Budget::new(bounds)),
        Err(ScanError::Limit)
    );
    // The malformed overflow spelling must not reach the digit predicate when
    // work is short: otherwise it would report Ok(false), not the resource error.
    assert_eq!(
        canonical_registration_u64("18446744073709551616", &mut Budget::new(bounds)),
        Err(ScanError::Limit)
    );
    bounds.max_work = 20;
    assert_eq!(
        canonical_registration_u64("18446744073709551615", &mut Budget::new(bounds)),
        Ok(true)
    );
    assert_eq!(
        canonical_registration_u64("18446744073709551616", &mut Budget::new(bounds)),
        Ok(false)
    );
}

#[test]
fn original_one_call_batch_and_registration_metadata_match_canonical_action() {
    for period in [false, true] {
        for retry in [false, true] {
            let key = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).unwrap();
            let mut schedule = Schedule::starting_at(Duration::from_millis(1));
            if period {
                schedule = schedule.with_period(Duration::from_millis(10));
            }
            let mut value = Action::new(
                Executable::Batch(ConstVec::from(vec![ExecutableBatchItem::ContractCall(
                    invocation(),
                )])),
                Repeats::Exactly(1),
                AccountId::new(key.public_key().clone()),
                TimeEventFilter::new(ExecutionTime::Schedule(schedule)),
            )
            .unwrap();
            if retry {
                value = value
                    .with_retry_policy(TimeTriggerRetryPolicy {
                        max_retries: NonZeroU32::new(2).unwrap(),
                        retry_after_ms: NonZeroU64::new(5).unwrap(),
                    })
                    .unwrap();
            }
            let mut metadata = Metadata::default();
            metadata.insert("__registered_at_ms".parse().unwrap(), Json::from(u64::MAX));
            metadata.insert(
                "__registered_block_height".parse().unwrap(),
                Json::from(0u64),
            );
            value = value.with_metadata(metadata);
            let frame = norito::encode_canonical(&value).unwrap();
            let decoded = norito::decode_canonical::<Action>(&frame).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
            let pool = AllocationBudget::new(65_536);
            let mut scanner = BoundedActionFrameScannerV1::try_new(
                &pool,
                limits(),
                DecodeLimits::new(1000, 65_536, 1000, 65_536, 32),
            )
            .unwrap();
            let result = scanner.scan(&frame).unwrap();
            assert_eq!(result.filter_kind, TriggerLifecycleStateKindV1::Time);
            assert_eq!(result.has_retry_policy, retry);
        }
    }
}

#[test]
fn batch_count_and_variant_boundaries_remain_closed() {
    let original = invocation();
    for value in [
        Executable::ContractCall(original.clone()),
        Executable::Batch(ConstVec::from(vec![
            ExecutableBatchItem::ContractCall(original.clone()),
            ExecutableBatchItem::ContractCall(original),
        ])),
    ] {
        let bytes = payload(&value);
        let mut budget = Budget::new(limits());
        assert_eq!(
            scan_executable(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget),
            Err(ScanError::UnsupportedExecutable)
        );
    }
    // A dynamic InstructionBox body is not skipped even when its enclosing item
    // has valid framing. The registry path remains unsupported.
    let mut item = 0u32.to_le_bytes().to_vec();
    item.extend(field(&[]));
    let mut sequence = 1u64.to_le_bytes().to_vec();
    sequence.extend(field(&item));
    let mut bytes = 4u32.to_le_bytes().to_vec();
    bytes.extend(field(&sequence));
    let mut budget = Budget::new(limits());
    assert_eq!(
        scan_executable(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget),
        Err(ScanError::UnsupportedExecutable)
    );
}

#[test]
fn contract_hash_marker_and_every_outer_field_are_consumed() {
    let typed = payload(&invocation());
    assert!(scan_invocation_bytes(&typed).is_ok());
    for hash in [&[0u8; 32][..], &[1u8; 31][..], &[1u8; 33][..]] {
        assert!(
            scan_invocation_bytes(&invocation_bytes(ADDRESS, hash, "entry", Some(vec![1])))
                .is_err()
        );
    }
    let mut trailing = typed;
    trailing.push(0);
    assert_eq!(scan_invocation_bytes(&trailing), Err(ScanError::Length));
    let mut record = payload(&Some(vec![1u8]));
    record.push(0);
    assert_eq!(
        scan_invocation_bytes(&invocation_parts(
            &payload(&ADDRESS),
            &[1; 32],
            &payload(&"entry"),
            &record
        )),
        Err(ScanError::Length)
    );
}

#[test]
fn argument_presence_wire_cap_and_selector_bounds_are_enforced() {
    use crate::transaction::executable::MAX_CONTRACT_ARGUMENT_RECORD_BYTES;
    for arguments in [None, Some(vec![])] {
        assert_eq!(
            scan_invocation_bytes(&invocation_bytes(ADDRESS, &[1; 32], "entry", arguments)),
            Err(ScanError::UnsupportedExecutable)
        );
    }
    for name in ["", " entry", "entry ", "\u{2003}entry"] {
        assert_eq!(
            scan_invocation_bytes(&invocation_bytes(ADDRESS, &[1; 32], name, Some(vec![1]))),
            Err(ScanError::UnsupportedExecutable)
        );
    }
    assert!(
        scan_invocation_bytes(&invocation_bytes(
            ADDRESS,
            &[1; 32],
            &"x".repeat(255),
            Some(vec![1])
        ))
        .is_ok()
    );
    assert!(
        scan_invocation_bytes(&invocation_bytes(
            ADDRESS,
            &[1; 32],
            &"x".repeat(256),
            Some(vec![1])
        ))
        .is_err()
    );
    let maximum = vec![0u8; MAX_CONTRACT_ARGUMENT_RECORD_BYTES];
    // These opaque zero bytes deliberately demonstrate outer framing scope only;
    // nested EntrypointArgumentRecord semantics are not claimed by this scanner.
    assert!(
        scan_invocation_bytes(&invocation_bytes(ADDRESS, &[1; 32], "entry", Some(maximum))).is_ok()
    );
    let mut oversized = vec![1];
    oversized.extend(field(
        &((MAX_CONTRACT_ARGUMENT_RECORD_BYTES as u64) + 1).to_le_bytes(),
    ));
    assert_eq!(
        scan_invocation_bytes(&invocation_parts(
            &payload(&ADDRESS),
            &[1; 32],
            &payload(&"entry"),
            &oversized
        )),
        Err(ScanError::UnsupportedExecutable)
    );
}

#[test]
fn borrowed_address_validator_preserves_original_checksum_variants_and_refusals() {
    use bech32::{Bech32, Bech32m, Hrp};
    let mut decoded = [0u8; 29];
    decoded[0] = 1;
    let hrp = Hrp::parse("irohac").unwrap();
    for text in [
        bech32::encode::<Bech32m>(hrp, &decoded).unwrap(),
        bech32::encode::<Bech32>(hrp, &decoded).unwrap(),
    ] {
        let _: ContractAddress = text.parse().unwrap();
        assert!(
            scan_invocation_bytes(&invocation_bytes(&text, &[1; 32], "entry", Some(vec![1])))
                .is_ok()
        );
        let upper = text.to_uppercase();
        assert!(upper.parse::<ContractAddress>().is_err());
        assert!(
            scan_invocation_bytes(&invocation_bytes(&upper, &[1; 32], "entry", Some(vec![1])))
                .is_err()
        );
    }
    let wrong_hrp = bech32::encode::<Bech32m>(Hrp::parse("wrongx").unwrap(), &decoded).unwrap();
    decoded[0] = 2;
    let wrong_version = bech32::encode::<Bech32m>(hrp, &decoded).unwrap();
    let mut wrong_checksum = ADDRESS.as_bytes().to_vec();
    let last = wrong_checksum.last_mut().unwrap();
    *last = if *last == b'q' { b'p' } else { b'q' };
    for text in [
        wrong_hrp,
        wrong_version,
        String::from_utf8(wrong_checksum).unwrap(),
    ] {
        assert!(text.parse::<ContractAddress>().is_err());
        assert!(
            scan_invocation_bytes(&invocation_bytes(&text, &[1; 32], "entry", Some(vec![1])))
                .is_err()
        );
    }
}

#[test]
fn address_validation_is_precharged_before_checksum_work() {
    let bytes = payload(&ADDRESS);
    // root 1 + compact length 1 + bytes 60 + UTF8 pass 60 precede the fixed debit.
    let mut bounds = limits();
    bounds.max_work = 122 + 4095;
    let mut budget = Budget::new(bounds);
    assert_eq!(
        scan_contract_address(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget),
        Err(ScanError::Limit)
    );
    let malformed = payload(&"!".repeat(60));
    let mut budget = Budget::new(bounds);
    assert_eq!(
        scan_contract_address(Cursor::root(&malformed, &mut budget).unwrap(), &mut budget),
        Err(ScanError::Limit)
    );
    bounds.max_work += 1;
    let mut budget = Budget::new(bounds);
    assert_eq!(
        scan_contract_address(Cursor::root(&malformed, &mut budget).unwrap(), &mut budget),
        Err(ScanError::InvalidValue)
    );
    let mut budget = Budget::new(bounds);
    assert!(scan_contract_address(Cursor::root(&bytes, &mut budget).unwrap(), &mut budget).is_ok());
}
