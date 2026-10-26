//! Authored controls for the additional inner-wire seam; not an execution claim.
use super::*;
use crate::{
    account::AccountId,
    events::time::{ExecutionTime, Schedule, TimeEventFilter},
    smart_contract::entrypoint::{
        EntrypointArgumentRecordV1, EntrypointValueAtomV1 as Atom,
        MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES,
    },
    transaction::{
        Executable, ExecutableBatchItem,
        executable::{ContractArgumentRecord, ContractInvocation},
    },
    trigger::action::Repeats,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_primitives::const_vec::ConstVec;
use std::time::Duration;

fn limits() -> ActionScannerLimitsV1 {
    ActionScannerLimitsV1 {
        max_frame_bytes: 2_000_000,
        max_total_frame_bytes: 32_000_000,
        max_frames: 32,
        max_work: 100_000_000,
        max_fields: 100_000,
        max_sequence_elements: 16_000_000,
        max_depth: 32,
    }
}

fn scanner(pool: &AllocationBudget) -> BoundedActionFrameScannerV1<'_> {
    scanner_with_limits(pool, limits())
}

fn scanner_with_limits(
    pool: &AllocationBudget,
    work: ActionScannerLimitsV1,
) -> BoundedActionFrameScannerV1<'_> {
    BoundedActionFrameScannerV1::try_new(
        pool,
        work,
        DecodeLimits::new(100_000, 2_000_000, 100_000, 2_000_000, 32),
    )
    .unwrap()
}

fn record(atoms: Vec<Atom>) -> EntrypointArgumentRecordV1 {
    EntrypointArgumentRecordV1 {
        schema_hash: [7; 32],
        atoms,
    }
}

fn payload<T: norito::SerializePayload>(value: &T) -> Vec<u8> {
    let _flags = ncore::DecodeFlagsGuard::enter(SCANNER_FLAGS);
    let mut bytes = Vec::new();
    ncore::serialize_to_buffer(value, &mut bytes).unwrap();
    bytes
}

fn field(bytes: &[u8]) -> Vec<u8> {
    let mut count = bytes.len() as u64;
    let mut out = Vec::new();
    loop {
        let byte = (count & 0x7f) as u8;
        count >>= 7;
        out.push(byte | if count == 0 { 0 } else { 0x80 });
        if count == 0 {
            break;
        }
    }
    out.extend_from_slice(bytes);
    out
}

fn record_payload(schema_hash: &[u8], atoms: &[Vec<u8>]) -> Vec<u8> {
    let mut sequence = (atoms.len() as u64).to_le_bytes().to_vec();
    for atom in atoms {
        sequence.extend(field(atom));
    }
    let mut out = field(schema_hash);
    out.extend(field(&sequence));
    out
}

fn frame(raw: &[u8]) -> Vec<u8> {
    ncore::frame_bare_with_header_flags::<EntrypointArgumentRecordV1>(raw, SCANNER_FLAGS).unwrap()
}

fn assert_bad_payload(raw: &[u8]) {
    let bytes = frame(raw);
    assert!(norito::decode_canonical::<EntrypointArgumentRecordV1>(&bytes).is_err());
    let pool = AllocationBudget::new(65_536);
    let mut scanner = scanner(&pool);
    let baseline = pool.reserved_bytes();
    assert!(scanner.validate_argument_record_wire(&bytes).is_err());
    assert_eq!(pool.reserved_bytes(), baseline);
}

fn action_frame(arguments: Vec<u8>) -> Vec<u8> {
    let key = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).unwrap();
    let invocation = ContractInvocation {
        contract_address: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
            .parse()
            .unwrap(),
        expected_code_hash: Hash::prehashed([1; 32]),
        entrypoint: "scheduled_payment".to_owned(),
        arguments: Some(ContractArgumentRecord::try_new(arguments).unwrap()),
    };
    let action = Action::new(
        Executable::Batch(ConstVec::from(vec![ExecutableBatchItem::ContractCall(
            invocation,
        )])),
        Repeats::Exactly(1),
        AccountId::new(key.public_key().clone()),
        TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
            Duration::from_millis(10),
        ))),
    )
    .unwrap();
    norito::encode_canonical(&action).unwrap()
}

#[test]
fn every_atom_wire_variant_preserves_original_bytes_without_graph_admission() {
    let cases = [
        Vec::new(),
        vec![
            Atom::Tag(false),
            Atom::Tag(true),
            Atom::Bool(false),
            Atom::Bool(true),
        ],
        vec![
            Atom::Unit,
            Atom::ErrorCode(0),
            Atom::ErrorCode(u32::MAX),
            Atom::EnumCode(u32::MAX),
        ],
        vec![Atom::List(0), Atom::List(64), Atom::List(u8::MAX)],
        vec![
            Atom::Pointer(Vec::new()),
            Atom::Pointer(vec![0xa5; 127]),
            Atom::Pointer(vec![0x5a; 128]),
        ],
    ];
    for atoms in cases {
        let value = record(atoms);
        let bytes = norito::encode_canonical(&value).unwrap();
        let raw_atoms: Vec<_> = value.atoms.iter().map(payload).collect();
        assert_eq!(
            frame(&record_payload(&value.schema_hash, &raw_atoms)),
            bytes
        );
        assert_eq!(
            norito::decode_canonical::<EntrypointArgumentRecordV1>(&bytes).unwrap(),
            value
        );
        let pool = AllocationBudget::new(65_536);
        let mut scanner = scanner(&pool);
        let baseline = (pool.reserved_bytes(), pool.peak_reserved_bytes());
        let borrowed = scanner.validate_argument_record_wire(&bytes).unwrap();
        assert_eq!(borrowed.as_ptr(), bytes.as_ptr());
        assert_eq!(borrowed, bytes);
        assert_eq!(
            (pool.reserved_bytes(), pool.peak_reserved_bytes()),
            baseline
        );
    }
    // Arbitrary schema hashes, opaque Pointer bytes and List(255) are canonical
    // wire, not successful callable/schema/active-atom semantic authorization.
}

#[test]
fn unknown_tags_bad_scalars_and_extra_variant_payload_refuse() {
    let unit = payload(&Atom::Unit);
    assert_eq!(unit, 4_u32.to_le_bytes());
    let valid = record_payload(&[7; 32], &[unit.clone()]);
    assert_eq!(
        frame(&valid),
        norito::encode_canonical(&record(vec![Atom::Unit])).unwrap()
    );
    let mut unit_with_extra = unit;
    unit_with_extra.push(0);
    for invalid in [
        7_u32.to_le_bytes().to_vec(),
        [0_u32.to_le_bytes().to_vec(), field(&[2])].concat(),
        [1_u32.to_le_bytes().to_vec(), field(&[0, 0])].concat(),
        [3_u32.to_le_bytes().to_vec(), field(&[])].concat(),
        [5_u32.to_le_bytes().to_vec(), field(&[0; 3])].concat(),
        [6_u32.to_le_bytes().to_vec(), field(&[0; 5])].concat(),
        unit_with_extra,
    ] {
        assert_bad_payload(&record_payload(&[7; 32], &[invalid]));
    }
    let pointer = [2_u32.to_le_bytes().to_vec(), field(&2_u64.to_le_bytes())].concat();
    assert_bad_payload(&record_payload(&[7; 32], &[pointer]));
}

#[test]
fn every_record_field_and_sequence_count_is_exact_and_canonical() {
    let atoms = [payload(&Atom::Unit)];
    assert_bad_payload(&record_payload(&[7; 31], &atoms));
    assert_bad_payload(&record_payload(&[7; 33], &atoms));
    let mut suffix = record_payload(&[7; 32], &atoms);
    suffix.push(0);
    assert_bad_payload(&suffix);
    let mut count = field(&[7; 32]);
    count.extend(field(&u64::MAX.to_le_bytes()));
    assert_bad_payload(&count);
    let canonical = record_payload(&[7; 32], &atoms);
    assert_eq!(canonical[0], 32);
    let overlong_prefix = [&[0xa0, 0][..], &canonical[1..]].concat();
    assert_bad_payload(&overlong_prefix);
}

#[test]
fn envelope_truncation_trailing_bytes_and_alternate_layout_refuse() {
    let value = record(vec![Atom::Bool(true)]);
    let bytes = norito::encode_canonical(&value).unwrap();
    for end in 0..bytes.len() {
        let pool = AllocationBudget::new(65_536);
        assert!(
            scanner(&pool)
                .validate_argument_record_wire(&bytes[..end])
                .is_err()
        );
    }
    let mut suffix = bytes.clone();
    suffix.push(0);
    let mut corrupt = bytes.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    let original_payload =
        norito::canonical_frame_payload::<EntrypointArgumentRecordV1>(&bytes, SCANNER_FLAGS)
            .unwrap();
    let alternate_flags = SCANNER_FLAGS ^ ncore::header_flags::COMPACT_LEN;
    assert_ne!(alternate_flags, SCANNER_FLAGS);
    let alternate = ncore::frame_bare_with_header_flags::<EntrypointArgumentRecordV1>(
        original_payload,
        alternate_flags,
    )
    .unwrap();
    assert_eq!(
        ncore::Header::read(&alternate[..]).unwrap().flags,
        alternate_flags
    );
    assert_ne!(alternate, bytes);
    for invalid in [suffix, corrupt, alternate] {
        let pool = AllocationBudget::new(65_536);
        assert!(
            scanner(&pool)
                .validate_argument_record_wire(&invalid)
                .is_err()
        );
    }
}

#[test]
fn record_wire_protocol_cap_is_independent_of_looser_caller_limits() {
    let value = record(vec![Atom::Pointer(
        vec![0; MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES],
    )]);
    let too_large = norito::encode_canonical(&value).unwrap();
    assert!(too_large.len() > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES);
    let pool = AllocationBudget::new(65_536);
    assert!(
        scanner(&pool)
            .validate_argument_record_wire(&too_large)
            .is_err()
    );
    let mut length = MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES - 256;
    let initial = norito::encode_canonical(&record(vec![Atom::Pointer(vec![0; length])])).unwrap();
    length += MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES - initial.len();
    let exact = norito::encode_canonical(&record(vec![Atom::Pointer(vec![0; length])])).unwrap();
    assert_eq!(exact.len(), MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES);
    assert!(scanner(&pool).validate_argument_record_wire(&exact).is_ok());
}

#[test]
fn inner_wire_limits_accumulate_and_preserve_first_original_failure() {
    let bytes = norito::encode_canonical(&record(vec![Atom::Unit])).unwrap();
    let pool = AllocationBudget::new(65_536);
    let mut work = limits();
    work.max_frames = 1;
    let mut scanner = scanner_with_limits(&pool, work);
    scanner.validate_argument_record_wire(&bytes).unwrap();
    let first = std::ptr::from_ref(scanner.validate_argument_record_wire(&bytes).unwrap_err());
    let baseline = pool.reserved_bytes();
    let later = std::ptr::from_ref(scanner.validate_argument_record_wire(&[]).unwrap_err());
    assert_eq!(first, later);
    assert_eq!(pool.reserved_bytes(), baseline);
    assert_eq!(scanner.failure().map(std::ptr::from_ref), Some(first));
}

#[test]
fn strict_owned_action_returns_same_original_owner_and_charge() {
    let inner = norito::encode_canonical(&record(vec![Atom::Unit])).unwrap();
    let bytes = action_frame(inner);
    let pool = AllocationBudget::new(65_536);
    let mut scanner = scanner(&pool);
    let baseline = pool.reserved_bytes();
    let owner = RetainedActionFrameBytesV1::try_copy(&bytes, &pool).unwrap();
    let pointer = owner.as_bytes().as_ptr();
    let backing = owner.backing_bytes();
    let (owner, semantics) = scanner.scan_owned_with_canonical_arguments(owner).unwrap();
    assert_eq!(owner.as_bytes().as_ptr(), pointer);
    assert_eq!(owner.as_bytes(), bytes);
    assert_eq!(semantics.filter_kind, TriggerLifecycleStateKindV1::Time);
    assert_eq!(pool.reserved_bytes(), baseline + backing);
    drop(owner);
    assert_eq!(pool.reserved_bytes(), baseline);
}

#[test]
fn strict_owned_refusal_keeps_owner_and_legacy_outer_only_behavior() {
    let bytes = action_frame(vec![0xa5; 32]);
    let pool = AllocationBudget::new(65_536);
    {
        let mut old = scanner(&pool);
        let owner = RetainedActionFrameBytesV1::try_copy(&bytes, &pool).unwrap();
        assert!(old.scan_owned(owner).is_ok());
    }
    let mut scanner = scanner(&pool);
    let baseline = pool.reserved_bytes();
    let owner = RetainedActionFrameBytesV1::try_copy(&bytes, &pool).unwrap();
    let pointer = owner.as_bytes().as_ptr();
    let backing = owner.backing_bytes();
    let refused = match scanner.scan_owned_with_canonical_arguments(owner) {
        Err(refused) => refused,
        Ok(_) => panic!("opaque record cannot pass the explicit stronger seam"),
    };
    let first = std::ptr::from_ref(refused.failure());
    assert_eq!(refused.frame().unwrap().as_bytes().as_ptr(), pointer);
    assert_eq!(refused.frame().unwrap().as_bytes(), bytes);
    assert_eq!(pool.reserved_bytes(), baseline + backing);
    let owner = refused.into_frame().unwrap();
    assert_eq!(scanner.failure().map(std::ptr::from_ref), Some(first));
    assert_eq!(std::ptr::from_ref(scanner.scan(&bytes).unwrap_err()), first);
    drop(owner);
    assert_eq!(pool.reserved_bytes(), baseline);
}

#[test]
fn owned_action_and_nested_record_share_one_frame_limit() {
    let bytes = action_frame(norito::encode_canonical(&record(vec![Atom::Unit])).unwrap());
    let pool = AllocationBudget::new(65_536);
    let mut work = limits();
    work.max_frames = 1;
    let mut scanner = scanner_with_limits(&pool, work);
    let owner = RetainedActionFrameBytesV1::try_copy(&bytes, &pool).unwrap();
    let pointer = owner.as_bytes().as_ptr();
    let refused = match scanner.scan_owned_with_canonical_arguments(owner) {
        Err(refused) => refused,
        Ok(_) => panic!("the real nested frame must consume the same cumulative limit"),
    };
    assert_eq!(refused.frame().unwrap().as_bytes().as_ptr(), pointer);
    assert!(matches!(
        refused.failure(),
        ActionFrameScanFailureV1::Structure(_)
    ));
}

#[test]
fn nested_record_retains_outer_cursor_depth_at_the_real_boundary() {
    let inner = norito::encode_canonical(&record(vec![Atom::Bool(true)])).unwrap();
    let bytes = action_frame(inner.clone());
    let pool = AllocationBudget::new(65_536);
    let mut work = limits();
    work.max_depth = 4;
    assert!(
        scanner_with_limits(&pool, work)
            .validate_argument_record_wire(&inner)
            .is_ok()
    );
    // Action root=1; Batch invocation argument field=7; framed record root=8;
    // atom sequence=9; atom=10; Bool payload=11. The old outer-only seam fits 10.
    work.max_depth = 10;
    let owner = RetainedActionFrameBytesV1::try_copy(&bytes, &pool).unwrap();
    assert!(scanner_with_limits(&pool, work).scan_owned(owner).is_ok());
    let mut strict = scanner_with_limits(&pool, work);
    let owner = RetainedActionFrameBytesV1::try_copy(&bytes, &pool).unwrap();
    let pointer = owner.as_bytes().as_ptr();
    let refused = match strict.scan_owned_with_canonical_arguments(owner) {
        Err(refused) => refused,
        Ok(_) => panic!("nested root must not reset its actual enclosing depth"),
    };
    assert_eq!(refused.frame().unwrap().as_bytes().as_ptr(), pointer);
    drop(refused);
    work.max_depth = 11;
    let mut strict = scanner_with_limits(&pool, work);
    let owner = RetainedActionFrameBytesV1::try_copy(&bytes, &pool).unwrap();
    assert!(strict.scan_owned_with_canonical_arguments(owner).is_ok());
}
