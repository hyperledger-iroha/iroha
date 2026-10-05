//! Original semantic error order and genuine History parity, without another validator.
use super::*;
use iroha_test_samples::ALICE_ID;
use mv::storage::Storage;

pub(super) fn action(hash: HashOf<IvmBytecode>) -> LoadedAction<DataEventFilter> {
    LoadedAction {
        executable: ExecutableRef::Ivm(hash),
        repeats: Repeats::Indefinitely,
        authority: ALICE_ID.clone(),
        filter: DataEventFilter::Any,
        retry_policy: None,
        retry_state: None,
        metadata: Metadata::default(),
    }
}
pub(super) fn fixture() -> (Set, HashOf<IvmBytecode>) {
    let mut set = Set::default();
    let bytes = IvmBytecode::from_compiled(vec![1, 2, 3]);
    let key = HashOf::new(&bytes);
    set.data_triggers = [("a".parse().unwrap(), action(key))].into_iter().collect();
    set.contracts = [(
        key,
        IvmBytecodeEntry {
            code_hash: ivm::contract_code_hash(bytes.as_ref()),
            original_contract: bytes,
            count: NonZeroU64::MIN,
        },
    )]
    .into_iter()
    .collect();
    (set, key)
}
fn checked(set: &Set) -> Result<(), TriggerContractError> {
    let pool = AllocationBudget::new(16 * 1024 * 1024);
    let result = super::super::CheckedContracts::capture(set, u64::MAX, &pool);
    let outcome = result.map(|_| ());
    assert_eq!(pool.reserved_bytes(), 0);
    outcome
}
#[test]
fn original_lookup_code_count_and_missing_diagnostics_remain_exact() {
    let (mut set, key) = fixture();
    assert_eq!(set.view().validate_world_contract_rows(), Ok(()));
    assert_eq!(checked(&set), Ok(()));
    let mut entry = set.contracts.view().get(&key).unwrap().clone();
    entry.count = NonZeroU64::new(2).unwrap();
    set.contracts = [(key, entry.clone())].into_iter().collect();
    assert_eq!(
        set.view().validate_world_contract_rows(),
        Err(SemanticFailure::Count.original_message())
    );
    assert_eq!(
        checked(&set),
        Err(TriggerContractError::Semantic(SemanticFailure::Count))
    );
    entry.code_hash = Hash::new(b"wrong");
    set.contracts = [(key, entry.clone())].into_iter().collect();
    assert_eq!(
        set.view().validate_world_contract_rows(),
        Err(SemanticFailure::Code.original_message())
    );
    assert_eq!(
        checked(&set),
        Err(TriggerContractError::Semantic(SemanticFailure::Code))
    );
    entry.original_contract = IvmBytecode::from_compiled(vec![4]);
    set.contracts = [(key, entry)].into_iter().collect();
    assert_eq!(
        set.view().validate_world_contract_rows(),
        Err(SemanticFailure::Lookup.original_message())
    );
    assert_eq!(
        checked(&set),
        Err(TriggerContractError::Semantic(SemanticFailure::Lookup))
    );
    set.contracts = Storage::default();
    assert_eq!(
        set.view().validate_world_contract_rows(),
        Err(SemanticFailure::Missing.original_message())
    );
    assert_eq!(
        checked(&set),
        Err(TriggerContractError::Semantic(SemanticFailure::Missing))
    );
}
#[test]
fn checked_and_public_overflow_keep_the_original_message() {
    let (_, hash) = fixture();
    let mut ordinary = OrdinaryStrategy([(hash, u64::MAX)].into_iter().collect());
    assert_eq!(
        ordinary.increment(&hash),
        Err(SemanticFailure::Overflow.original_message())
    );
    let pool = AllocationBudget::new(4096);
    let mut strategy = CheckedStrategy {
        work: Work::new(u64::MAX),
        counts: ChargedBuffer::new(1, &pool).unwrap(),
    };
    strategy.counts.push_reserved(CounterCell {
        hash,
        count: u64::MAX,
    });
    assert_eq!(
        strategy.increment(&hash),
        Err(TriggerContractError::Semantic(SemanticFailure::Overflow))
    );
    assert_eq!(strategy.counts.as_slice()[0].count, u64::MAX);
    drop(strategy);
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn one_engine_keeps_action_phase_then_sorted_contract_first_fault() {
    let (mut set, key) = fixture();
    let other = IvmBytecode::from_compiled(vec![9, 8]);
    let other_key = HashOf::new(&other);
    let mut entries = vec![
        (key, set.contracts.view().get(&key).unwrap().clone()),
        (
            other_key,
            IvmBytecodeEntry {
                code_hash: ivm::contract_code_hash(other.as_ref()),
                original_contract: other,
                count: NonZeroU64::MIN,
            },
        ),
    ];
    entries.sort_by_key(|(key, _)| *key);
    entries[0].1.count = NonZeroU64::new(2).unwrap();
    entries[1].1.original_contract = IvmBytecode::from_compiled(vec![0xff]);
    set.data_triggers = [
        ("a".parse().unwrap(), action(entries[0].0)),
        ("b".parse().unwrap(), action(entries[1].0)),
    ]
    .into_iter()
    .collect();
    set.contracts = entries.into_iter().collect();
    assert_eq!(
        set.view().validate_world_contract_rows(),
        Err(SemanticFailure::Count.original_message())
    );
    assert_eq!(
        checked(&set),
        Err(TriggerContractError::Semantic(SemanticFailure::Count))
    );
}
#[test]
fn canonical_predecessor_merge_matches_real_history_with_all_mask_shapes() {
    let mut rows: Storage<TriggerId, u32> = [
        ("a".parse().unwrap(), 1),
        ("c".parse().unwrap(), 3),
        ("e".parse().unwrap(), 5),
    ]
    .into_iter()
    .collect();
    let mut block = rows.block();
    block.insert("a".parse().unwrap(), 10);
    block.remove("c".parse().unwrap());
    block.insert("b".parse().unwrap(), 2);
    block.insert("e".parse().unwrap(), 5);
    block.remove("d".parse().unwrap());
    block.commit();
    let expected: Vec<_> = rows
        .history()
        .iter_before_block()
        .map(|(key, value)| (key.clone(), *value))
        .collect();
    let pool = AllocationBudget::new(0);
    let view = rows.try_committed_view_nonblocking().unwrap();
    assert!(view.undo().iter().any(|(_, value)| value.is_none()));
    let mut strategy = CheckedStrategy {
        work: Work::new(u64::MAX),
        counts: ChargedBuffer::new(0, &pool).unwrap(),
    };
    let mut actual = Vec::new();
    visit_original(&view, Image::Predecessor, &mut strategy, |key, value, _| {
        actual.push((key.clone(), *value));
        Ok(())
    })
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        actual
            .iter()
            .map(|(key, _)| key.name().as_ref())
            .collect::<Vec<&str>>(),
        vec!["a", "c", "e"]
    );
    assert_eq!(strategy.work.observed.setup, 2);
    assert_eq!(
        strategy.work.observed.next,
        view.current().len() + view.undo().len() + 2
    );
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(pool.peak_reserved_bytes(), 0);
}
#[test]
fn current_image_is_checked_before_an_invalid_retained_predecessor() {
    let (set, key) = fixture();
    let mut block = set.block();
    block.contracts.get_mut(&key).unwrap().count = NonZeroU64::new(2).unwrap();
    block.commit();
    assert_eq!(
        checked(&set),
        Err(TriggerContractError::Semantic(SemanticFailure::Count))
    );

    // Both images now fail differently: reversing their order would return Count.
    let (mut set, key) = fixture();
    let mut predecessor = set.contracts.view().get(&key).unwrap().clone();
    predecessor.count = NonZeroU64::new(2).unwrap();
    set.contracts = [(key, predecessor)].into_iter().collect();
    let mut block = set.block();
    let current = block.contracts.get_mut(&key).unwrap();
    current.count = NonZeroU64::MIN;
    current.code_hash = Hash::new(b"current code mismatch");
    block.commit();
    let originals = set.contracts.try_committed_view_nonblocking().unwrap();
    let current = originals.current().get(&key).unwrap();
    let predecessor = originals.undo().get(&key).unwrap().as_ref().unwrap();
    assert_eq!(current.count, NonZeroU64::MIN);
    assert_ne!(
        current.code_hash,
        ivm::contract_code_hash(current.original_contract.as_ref())
    );
    assert_eq!(predecessor.count, NonZeroU64::new(2).unwrap());
    assert_eq!(
        predecessor.code_hash,
        ivm::contract_code_hash(predecessor.original_contract.as_ref())
    );
    assert_eq!(
        set.view().validate_world_contract_rows(),
        Err(SemanticFailure::Code.original_message())
    );
    assert_eq!(
        checked(&set),
        Err(TriggerContractError::Semantic(SemanticFailure::Code))
    );
}

#[test]
fn valid_current_image_still_rejects_its_invalid_retained_predecessor() {
    let (mut set, key) = fixture();
    let mut predecessor = set.contracts.view().get(&key).unwrap().clone();
    predecessor.count = NonZeroU64::new(2).unwrap();
    set.contracts = [(key, predecessor)].into_iter().collect();
    let mut block = set.block();
    block.contracts.get_mut(&key).unwrap().count = NonZeroU64::MIN;
    block.commit();
    let originals = set.contracts.try_committed_view_nonblocking().unwrap();
    assert_eq!(
        originals.current().get(&key).unwrap().count,
        NonZeroU64::MIN
    );
    assert_eq!(
        originals.undo().get(&key).unwrap().as_ref().unwrap().count,
        NonZeroU64::new(2).unwrap()
    );
    assert_eq!(set.view().validate_world_contract_rows(), Ok(()));
    assert_eq!(
        checked(&set),
        Err(TriggerContractError::Semantic(SemanticFailure::Count))
    );
}
