// P.3: the ledger's capacity, identity and transaction boundaries remain observable
// when a test-only faulty verifier supplies an arbitrary allocation record. These
// controls do not establish FASTPQ soundness or authenticate the synthetic claim.

fn fee_sponsor_faulty_effect_fixture() -> (State, VerifiedFeeSponsorVaultAllocation) {
    use iroha_data_model::nexus::{
        FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramLifecycle, FeeSponsorVault,
        FeeSponsorVaultKey,
    };
    let program_id = FeeSponsorProgramId::new(
        ALICE_ID.clone(),
        "faulty-verifier-capacity".parse().expect("program name"),
    );
    let definition: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
        .parse()
        .expect("canonical fee asset");
    let record = fee_sponsor_relay_allocation_fixture(
        program_id.clone(),
        definition.clone(),
        Quantity::from(6_u32),
        DataSpaceId::new(1),
        Hash::new(b"faulty-verifier-first-lease"),
        10,
    );
    let mut world = World::default();
    let mut program = FeeSponsorProgram::new(program_id.clone(), ALICE_ID.clone());
    program.lifecycle = FeeSponsorProgramLifecycle::Active;
    program.active_revision = Some(1);
    world
        .fee_sponsor_programs
        .insert(program_id.clone(), program);
    let key = FeeSponsorVaultKey {
        program_id,
        asset_definition_id: definition,
    };
    world.fee_sponsor_vaults.insert(
        key.clone(),
        FeeSponsorVault {
            key,
            balance: Quantity::from(10_u32),
        },
    );
    (
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ),
        record,
    )
}

fn fee_sponsor_faulty_effect_rows(
    transaction: &StateTransaction<'_, '_>,
) -> std::collections::BTreeMap<StatePath, Vec<u8>> {
    transaction
        .world
        .smart_contract_state
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

// State initialization seeds authenticated SNS and namespace rows. Preserve every
// original key/value, and add exactly the declared lease row when an effect succeeds.
fn fee_sponsor_expected_rows_with_allocation(
    mut rows: std::collections::BTreeMap<StatePath, Vec<u8>>,
    record: &VerifiedFeeSponsorVaultAllocation,
) -> std::collections::BTreeMap<StatePath, Vec<u8>> {
    let key = super::verified_fee_sponsor_vault_allocation_state_key(
        &record.program_id,
        &record.asset_definition_id,
        &record.lease_id,
    )
    .expect("canonical lease key");
    let encoded = super::encode_verified_fee_sponsor_vault_allocation_state(record)
        .expect("complete canonical allocation record");
    assert!(
        rows.insert(key, encoded).is_none(),
        "a new lease has exactly one new row"
    );
    rows
}

#[test]
fn faulty_verifier_fee_sponsor_capacity_is_exact_and_identical_replay_adds_no_lock() {
    let (state, first) = fee_sponsor_faulty_effect_fixture();
    let mut block = state.block(BlockHeader::new(
        iroha_primitives::nonzero!(1_u64),
        None,
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    let baseline = fee_sponsor_faulty_effect_rows(&transaction);
    super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, first.clone())
        .expect("six units fit the authoritative ten-unit vault");
    let first_rows = fee_sponsor_faulty_effect_rows(&transaction);
    assert_eq!(
        first_rows,
        fee_sponsor_expected_rows_with_allocation(baseline.clone(), &first)
    );
    super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, first.clone())
        .expect("an identical replay is an idempotent no-op");
    assert_eq!(fee_sponsor_faulty_effect_rows(&transaction), first_rows);
    let mut second = first.clone();
    second.lease_id = Hash::new(b"faulty-verifier-second-lease");
    second.source_dataspace_id = DataSpaceId::new(2);
    second.verified_allocation = Quantity::from(4_u32);
    let expected_full = fee_sponsor_expected_rows_with_allocation(first_rows, &second);
    super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, second)
        .expect("the second route may lock exactly the four remaining units");
    assert_eq!(
        super::locked_fee_sponsor_vault_capacity(
            &transaction.world,
            &first.program_id,
            &first.asset_definition_id,
            1,
        )
        .unwrap(),
        Quantity::from(10_u32),
    );
    let full_rows = fee_sponsor_faulty_effect_rows(&transaction);
    let mut excess = first.clone();
    excess.lease_id = Hash::new(b"faulty-verifier-excess-lease");
    excess.source_dataspace_id = DataSpaceId::new(3);
    excess.verified_allocation = Quantity::one();
    let error = super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, excess)
        .expect_err("even accepted verification cannot allocate an eleventh unit");
    assert!(
        matches!(&error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) if message.contains("exceed source capacity")),
        "{error:?}"
    );
    assert_eq!(fee_sponsor_faulty_effect_rows(&transaction), full_rows);
    assert_eq!(full_rows, expected_full);
    assert_eq!(full_rows.len(), baseline.len() + 2);
    assert_eq!(
        transaction
            .world
            .fee_sponsor_vaults
            .get(&iroha_data_model::nexus::FeeSponsorVaultKey {
                program_id: first.program_id,
                asset_definition_id: first.asset_definition_id,
            })
            .unwrap()
            .balance,
        Quantity::from(10_u32),
        "allocating spend capacity neither mints nor debits the vault",
    );
}

#[test]
fn faulty_verifier_fee_sponsor_cannot_rebind_a_lease_or_overlap_a_live_route() {
    let (state, first) = fee_sponsor_faulty_effect_fixture();
    let mut block = state.block(BlockHeader::new(
        iroha_primitives::nonzero!(1_u64),
        None,
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, first.clone()).unwrap();
    let before = fee_sponsor_faulty_effect_rows(&transaction);
    let mut same_lease = first.clone();
    same_lease.verified_allocation = Quantity::one();
    let error = super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, same_lease)
        .expect_err("a valid-looking second amount cannot reuse the original lease id");
    assert!(
        matches!(&error, InstructionExecutionError::InvariantViolation(message) if message.contains("already bound to another allocation")),
        "{error:?}"
    );
    let mut same_route = first;
    same_route.lease_id = Hash::new(b"faulty-verifier-overlapping-route");
    same_route.verified_allocation = Quantity::one();
    let error = super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, same_route)
        .expect_err("available capacity does not allow two live leases on one route");
    assert!(
        matches!(&error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) if message.contains("already has an unexpired spend lease")),
        "{error:?}"
    );
    assert_eq!(fee_sponsor_faulty_effect_rows(&transaction), before);
}

#[test]
fn faulty_verifier_fee_sponsor_respects_current_height_and_scheduled_revision() {
    let (state, mut record) = fee_sponsor_faulty_effect_fixture();
    let mut block = state.block(BlockHeader::new(
        iroha_primitives::nonzero!(1_u64),
        None,
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    let baseline = fee_sponsor_faulty_effect_rows(&transaction);
    record.verified_at_height = 2;
    let error =
        super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, record.clone())
            .expect_err("a verified record belongs to one execution height");
    assert!(
        matches!(&error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) if message.contains("another execution height")),
        "{error:?}"
    );
    record.verified_at_height = 1;
    let mut program = transaction
        .world
        .fee_sponsor_programs
        .get(&record.program_id)
        .unwrap()
        .clone();
    program.scheduled_activation = Some(iroha_data_model::nexus::FeeSponsorProgramActivation {
        revision: 2,
        activate_at_height: 10,
    });
    transaction
        .world
        .fee_sponsor_programs
        .insert(record.program_id.clone(), program);
    let error =
        super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, record.clone())
            .expect_err("a lease must end strictly before scheduled revision activation");
    assert!(
        matches!(&error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) if message.contains("must expire before scheduled revision")),
        "{error:?}"
    );
    assert_eq!(fee_sponsor_faulty_effect_rows(&transaction), baseline);
    record.expires_at_height = 9;
    let expected = fee_sponsor_expected_rows_with_allocation(baseline, &record);
    super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, record)
        .expect("the preceding height remains admissible");
    assert_eq!(fee_sponsor_faulty_effect_rows(&transaction), expected);
}

#[test]
fn faulty_verifier_fee_sponsor_failed_batch_discards_earlier_allocation() {
    let (state, first) = fee_sponsor_faulty_effect_fixture();
    let mut block = state.block(BlockHeader::new(
        iroha_primitives::nonzero!(1_u64),
        None,
        None,
        0,
        0,
    ));
    let baseline = fee_sponsor_faulty_effect_rows(&block.transaction());
    let expected_partial = fee_sponsor_expected_rows_with_allocation(baseline.clone(), &first);
    {
        let mut transaction = block.transaction();
        super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, first.clone())
            .unwrap();
        let mut excess = first;
        excess.lease_id = Hash::new(b"faulty-verifier-batch-excess");
        excess.source_dataspace_id = DataSpaceId::new(2);
        excess.verified_allocation = Quantity::from(5_u32);
        let error = super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, excess)
            .expect_err("the second instruction exceeds remaining capacity");
        assert!(
            matches!(&error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) if message.contains("exceed source capacity")),
            "{error:?}"
        );
        assert_eq!(
            fee_sponsor_faulty_effect_rows(&transaction),
            expected_partial
        );
        // The failed transaction is dropped, exactly as an unsuccessful instruction batch.
    }
    assert_eq!(
        fee_sponsor_faulty_effect_rows(&block.transaction()),
        baseline
    );
}

// Registration applies the ledger rules before it verifies the proof, so admission
// sees the candidate without its verifier digests; these controls
// zero them to show that no ledger rule reads them.
fn fee_sponsor_unverified_candidate(
    record: &VerifiedFeeSponsorVaultAllocation,
) -> VerifiedFeeSponsorVaultAllocation {
    VerifiedFeeSponsorVaultAllocation {
        fastpq_statement_digest: [0; 32],
        fastpq_proof_digest: Hash::prehashed([0; 32]),
        ..record.clone()
    }
}

#[test]
fn fee_sponsor_admission_finds_the_stored_allocation_without_verifier_digests() {
    let (state, first) = fee_sponsor_faulty_effect_fixture();
    let mut block = state.block(BlockHeader::new(
        iroha_primitives::nonzero!(1_u64),
        None,
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    let baseline = fee_sponsor_faulty_effect_rows(&transaction);
    let candidate = fee_sponsor_unverified_candidate(&first);
    let key = match super::fee_sponsor_vault_allocation_admission(&transaction, &candidate)
        .expect("the empty ledger admits the lease")
    {
        super::FeeSponsorVaultAllocationAdmission::Admissible(key) => key,
        super::FeeSponsorVaultAllocationAdmission::Recorded => {
            panic!("nothing is stored before the first allocation")
        }
    };
    assert_eq!(
        key,
        super::verified_fee_sponsor_vault_allocation_state_key(
            &first.program_id,
            &first.asset_definition_id,
            &first.lease_id,
        )
        .expect("canonical lease key"),
    );
    assert_eq!(
        fee_sponsor_faulty_effect_rows(&transaction),
        baseline,
        "admission alone writes nothing",
    );
    super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, first.clone())
        .expect("six units fit the ten-unit vault");
    let stored = fee_sponsor_faulty_effect_rows(&transaction);
    assert_eq!(
        stored,
        fee_sponsor_expected_rows_with_allocation(baseline, &first)
    );
    assert!(
        matches!(
            super::fee_sponsor_vault_allocation_admission(&transaction, &candidate)
                .expect("the same payload at the same height is the stored allocation"),
            super::FeeSponsorVaultAllocationAdmission::Recorded
        ),
        "a stored lease is never admitted twice"
    );
    assert_eq!(fee_sponsor_faulty_effect_rows(&transaction), stored);
}

#[test]
fn fee_sponsor_admission_refuses_replays_and_conflicts_without_verifier_digests() {
    let (state, first) = fee_sponsor_faulty_effect_fixture();
    let mut block = state.block(BlockHeader::new(
        iroha_primitives::nonzero!(1_u64),
        None,
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, first.clone()).unwrap();
    // A row that another block verified and stored records that block's height.
    let mut earlier = first.clone();
    earlier.lease_id = Hash::new(b"lease-stored-by-another-block");
    earlier.source_dataspace_id = DataSpaceId::new(2);
    earlier.verified_allocation = Quantity::one();
    earlier.verified_at_height = 7;
    let earlier_key = super::verified_fee_sponsor_vault_allocation_state_key(
        &earlier.program_id,
        &earlier.asset_definition_id,
        &earlier.lease_id,
    )
    .unwrap();
    super::store_fee_sponsor_vault_allocation(&mut transaction, earlier_key, &earlier).unwrap();
    let rows = fee_sponsor_faulty_effect_rows(&transaction);
    let lease_rebound = |error: &InstructionExecutionError| matches!(error, InstructionExecutionError::InvariantViolation(message) if message.contains("already bound to another allocation"));
    // Resubmitting that exact payload in this block is another allocation for the
    // lease, refused without verifying the proof again.
    let mut replay = fee_sponsor_unverified_candidate(&earlier);
    replay.verified_at_height = 1;
    let error = super::fee_sponsor_vault_allocation_admission(&transaction, &replay)
        .err()
        .expect("a replay at another height is refused");
    assert!(lease_rebound(&error), "{error:?}");
    // Another payload under a stored lease id.
    let mut other_payload = fee_sponsor_unverified_candidate(&first);
    other_payload.proof_payload_hash = Hash::new(b"another-proof-payload");
    let error = super::fee_sponsor_vault_allocation_admission(&transaction, &other_payload)
        .err()
        .expect("a lease id binds one payload");
    assert!(lease_rebound(&error), "{error:?}");
    // A fresh lease on a route that already has a live lease.
    let mut same_route = fee_sponsor_unverified_candidate(&first);
    same_route.lease_id = Hash::new(b"fresh-lease-on-a-live-route");
    same_route.verified_allocation = Quantity::one();
    let error = super::fee_sponsor_vault_allocation_admission(&transaction, &same_route)
        .err()
        .expect("one live lease per route");
    assert!(
        matches!(&error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) if message.contains("already has an unexpired spend lease")),
        "{error:?}"
    );
    // A fresh lease on a free route that the vault cannot cover: seven of ten units
    // are locked, so four more do not fit.
    let mut excess = fee_sponsor_unverified_candidate(&first);
    excess.lease_id = Hash::new(b"fresh-lease-beyond-capacity");
    excess.source_dataspace_id = DataSpaceId::new(3);
    excess.verified_allocation = Quantity::from(4_u32);
    let error = super::fee_sponsor_vault_allocation_admission(&transaction, &excess)
        .err()
        .expect("capacity is checked before verification");
    assert!(
        matches!(&error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) if message.contains("exceed source capacity")),
        "{error:?}"
    );
    // A lease that would outlive a scheduled revision activation.
    let mut program = transaction
        .world
        .fee_sponsor_programs
        .get(&first.program_id)
        .unwrap()
        .clone();
    program.scheduled_activation = Some(iroha_data_model::nexus::FeeSponsorProgramActivation {
        revision: 2,
        activate_at_height: 10,
    });
    transaction
        .world
        .fee_sponsor_programs
        .insert(first.program_id.clone(), program);
    let mut outliving = fee_sponsor_unverified_candidate(&first);
    outliving.lease_id = Hash::new(b"fresh-lease-past-activation");
    outliving.source_dataspace_id = DataSpaceId::new(3);
    outliving.verified_allocation = Quantity::one();
    let error = super::fee_sponsor_vault_allocation_admission(&transaction, &outliving)
        .err()
        .expect("a lease ends before the scheduled activation");
    assert!(
        matches!(&error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) if message.contains("must expire before scheduled revision")),
        "{error:?}"
    );
    // A verified record that differs from the stored row only in its digests is a
    // conflict for a faulty verifier, never a silent no-op.
    let mut redigested = first.clone();
    redigested.fastpq_proof_digest = Hash::new(b"another-proof-digest");
    let error = super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, redigested)
        .expect_err("a faulty verifier cannot overwrite stored digests");
    assert!(
        matches!(&error, InstructionExecutionError::InvariantViolation(message) if message.contains("conflicting verified fee sponsor vault allocation")),
        "{error:?}"
    );
    assert_eq!(fee_sponsor_faulty_effect_rows(&transaction), rows);
}

#[test]
fn fee_sponsor_allocation_rows_are_exactly_the_prefixed_contract_state() {
    let (state, first) = fee_sponsor_faulty_effect_fixture();
    let mut block = state.block(BlockHeader::new(
        iroha_primitives::nonzero!(1_u64),
        None,
        None,
        0,
        0,
    ));
    let mut transaction = block.transaction();
    super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, first.clone()).unwrap();
    let mut second = first.clone();
    second.lease_id = Hash::new(b"prefix-walk-second-lease");
    second.source_dataspace_id = DataSpaceId::new(2);
    second.verified_allocation = Quantity::from(4_u32);
    super::apply_verified_fee_sponsor_vault_allocation(&mut transaction, second).unwrap();
    // Neighbours on both sides of the prefix: usage rows and keys that sort just
    // before and just after every allocation key.
    for neighbour in [
        VerifiedFeeSponsorVaultAllocation::usage_state_key_for(&first.lease_id),
        "pkdeploy_verified_fee_sponsor_vault_allocatioM".to_owned(),
        "pkdeploy_verified_fee_sponsor_vault_allocatioo".to_owned(),
        "pkdeploy_verified_fee_sponsor_vault_allocatiop_after".to_owned(),
    ] {
        transaction
            .world
            .smart_contract_state
            .insert(StatePath::from_str(&neighbour).unwrap(), vec![1]);
    }
    let walked: Vec<StatePath> =
        crate::state::verified_fee_sponsor_vault_allocation_rows(&transaction.world)
            .map(|(key, _)| key.clone())
            .collect();
    let filtered: Vec<StatePath> = transaction
        .world
        .smart_contract_state
        .iter()
        .filter(|(key, _)| {
            key.as_ref().starts_with(
                iroha_data_model::nexus::VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX,
            )
        })
        .map(|(key, _)| key.clone())
        .collect();
    assert_eq!(walked.len(), 2);
    assert_eq!(walked, filtered);
}
