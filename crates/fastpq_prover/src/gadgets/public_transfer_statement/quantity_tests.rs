//! Full-domain public transfer preparation without private witnesses or traces.

use super::*;
use crate::gadgets::transfer;
use iroha_data_model::{DomainId, fastpq::TransferSmtWitness};
use iroha_primitives::{bigint::BigInt, numeric::Numeric};
use iroha_test_samples::{ALICE_ID, BOB_ID};

pub(super) fn maximum() -> Quantity {
    let mut bytes = [0xff; 64];
    bytes[63] = 0x7f;
    Quantity::from_canonical_numeric(
        Numeric::try_new(BigInt::from_twos_bytes(&bytes).unwrap(), 0).unwrap(),
    )
    .unwrap()
}

pub(super) fn tiny() -> Quantity {
    Quantity::from_canonical_numeric(Numeric::try_new(1_u32, 28).unwrap()).unwrap()
}

pub(super) fn delta(sender: Quantity, receiver: Quantity, amount: Quantity) -> PublicTransferDelta {
    PublicTransferDelta {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        from_balance_after: sender.try_sub(&amount).unwrap(),
        to_balance_after: receiver.try_add(&amount).unwrap(),
        from_balance_before: sender,
        to_balance_before: receiver,
        amount,
    }
}

fn set_digest(claim: &mut PublicTransferTranscript) {
    claim.poseidon_preimage_digest = match claim.deltas.as_slice() {
        [d] => Some(transfer::compute_poseidon_digest(
            &TransferDeltaTranscript {
                from_account: d.from_account.clone(),
                to_account: d.to_account.clone(),
                asset_definition: d.asset_definition.clone(),
                amount: d.amount.clone(),
                from_balance_before: d.from_balance_before.clone(),
                from_balance_after: d.from_balance_after.clone(),
                to_balance_before: d.to_balance_before.clone(),
                to_balance_after: d.to_balance_after.clone(),
                from_smt_witness: TransferSmtWitness::default(),
                to_smt_witness: TransferSmtWitness::default(),
            },
            &claim.batch_hash,
        )),
        _ => None,
    };
}

pub(super) fn fixture(
    deltas: Vec<PublicTransferDelta>,
) -> (
    Vec<PublicTransferTranscript>,
    Vec<StateTransition>,
    PublicInputs,
) {
    let mut claim = PublicTransferTranscript {
        batch_hash: Hash::new(b"quantity public call"),
        authority_digest: Hash::new(b"quantity public authority"),
        deltas,
        poseidon_preimage_digest: None,
    };
    set_digest(&mut claim);
    let claims = vec![claim];
    let scales = asset_scales(&claims);
    let mut rows = Vec::new();
    for d in &claims[0].deltas {
        let scale = scales[&d.asset_definition];
        for (account, before, after) in [
            (
                &d.from_account,
                &d.from_balance_before,
                &d.from_balance_after,
            ),
            (&d.to_account, &d.to_balance_before, &d.to_balance_after),
        ] {
            let encode = |q| {
                encode_quantity_units_v1(&FastpqQuantityUnits::from_quantity(q, scale).unwrap())
                    .unwrap()
            };
            rows.push(StateTransition::new(
                balance_key(&d.asset_definition, account).unwrap(),
                encode(before),
                encode(after),
                OperationKind::Transfer,
            ));
        }
    }
    rows.sort_by(|a, b| (&a.key, a.operation_rank()).cmp(&(&b.key, b.operation_rank())));
    let inputs = PublicInputs {
        old_root: Hash::new(b"caller expected source root").into(),
        new_root: Hash::new(b"caller expected destination root").into(),
        slot: 17,
        dsid: [0x45; 16],
        perm_root: Hash::new(b"permissions").into(),
        tx_set_hash: Hash::new(b"transaction set").into(),
    };
    (claims, rows, inputs)
}

fn prepare<'a>(
    claims: &'a [PublicTransferTranscript],
    rows: &'a [StateTransition],
    inputs: PublicInputs,
) -> Result<PreparedPublicTransfers<'a, FastpqQuantityUnits>> {
    prepare_quantity_public_transfers(
        rows,
        claims,
        inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
}

#[test]
fn complete_domain_quantities_derive_exact_public_ports() {
    for d in [
        delta(Quantity::from(u128::MAX), Quantity::zero(), Quantity::one()),
        delta(maximum(), Quantity::zero(), maximum()),
        delta(
            maximum().try_sub(&Quantity::one()).unwrap(),
            tiny(),
            Quantity::one(),
        ),
        delta(Quantity::from(2_u32), Quantity::zero(), tiny()),
    ] {
        let expected = d.clone();
        let (claims, rows, inputs) = fixture(vec![d]);
        let table = prepare(&claims, &rows, inputs).unwrap();
        assert_eq!(table.claims(), claims);
        assert_eq!(table.transitions(), rows);
        assert_eq!(*table.public_inputs(), inputs);
        assert_eq!(table.semantics(), ProofSemantics::StateTransition);
        assert_eq!(table.rows().len(), 2);
        assert_eq!(table.pairs().len(), 1);
        let scale = asset_scales(&claims)[&expected.asset_definition];
        let pair = table.pairs()[0];
        for (leg, index) in pair.row_indices.into_iter().enumerate() {
            let row = table.rows()[index];
            assert_eq!(row.asset_scale, scale);
            assert_eq!(row.before.scale(), scale);
            assert_eq!(row.amount.to_quantity().unwrap(), expected.amount);
            assert_eq!(row.update, pair.updates[leg]);
            assert_eq!(
                row.before,
                decode_quantity_units_v1(&rows[index].pre_value).unwrap()
            );
            assert_eq!(
                row.after,
                decode_quantity_units_v1(&rows[index].post_value).unwrap()
            );
            if leg == 0 {
                assert_eq!(
                    row.before.to_quantity().unwrap(),
                    expected.from_balance_before
                );
                assert_eq!(row.before.checked_sub(&row.amount), Some(row.after));
                assert!(row.before.try_to_u64().is_none());
            } else {
                assert_eq!(
                    row.before.to_quantity().unwrap(),
                    expected.to_balance_before
                );
                assert_eq!(row.before.checked_add(&row.amount), Some(row.after));
            }
            let key = &table.keys()[row.key_index];
            let frame = &rows[index].pre_value;
            let value_hash =
                Hash::new([b"fastpq:quantity:v1:smt:value|".as_slice(), frame].concat());
            let leaf = Hash::new(
                [
                    b"fastpq:v1:smt:leaf|".as_slice(),
                    &key.key_hash,
                    value_hash.as_ref(),
                ]
                .concat(),
            );
            assert_eq!(row.update.old_leaf, digest_limbs(leaf.into()));
        }
        if scale == 28 && expected.from_balance_before > Quantity::from(u128::MAX) {
            assert_ne!(table.rows()[pair.row_indices[0]].before.limbs()[18], 0);
        }
        let statements = table.compact_statements(&[]).unwrap();
        assert_eq!(statements[0].updates, pair.updates);
        assert_eq!(statements[0].old_root, digest_limbs(inputs.old_root));
        assert_eq!(statements[0].new_root, digest_limbs(inputs.new_root));
        assert!(table.compact_statements(&[[1; 32]]).is_err());
    }
}

#[test]
fn wide_and_narrow_value_encodings_cannot_be_interchanged() {
    let (claims, rows, inputs) = fixture(vec![delta(
        Quantity::from(20_u32),
        Quantity::zero(),
        Quantity::one(),
    )]);
    assert!(
        prepare_public_transfers(
            &rows,
            &claims,
            inputs,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default()
        )
        .is_err()
    );
    let mut narrow = rows.clone();
    for row in &mut narrow {
        row.pre_value = decode_quantity_units_v1(&row.pre_value)
            .unwrap()
            .try_to_u64()
            .unwrap()
            .to_le_bytes()
            .to_vec();
        row.post_value = decode_quantity_units_v1(&row.post_value)
            .unwrap()
            .try_to_u64()
            .unwrap()
            .to_le_bytes()
            .to_vec();
    }
    assert!(prepare(&claims, &narrow, inputs).is_err());
    let old = prepare_public_transfers(
        &narrow,
        &claims,
        inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let new = prepare(&claims, &rows, inputs).unwrap();
    assert_ne!(old.ordering_hash(), new.ordering_hash());
    assert_ne!(old.pairs()[0].updates, new.pairs()[0].updates);
    assert_eq!(old.keys(), new.keys());
    let receiver = new.pairs()[0].row_indices[1];
    let original = decode_quantity_units_v1(&rows[receiver].pre_value).unwrap();
    for scale in [1, 28] {
        let wrong_scale = FastpqQuantityUnits::from_quantity(&Quantity::zero(), scale).unwrap();
        assert_eq!(original.limbs(), wrong_scale.limbs());
        let mut changed = rows.clone();
        changed[receiver].pre_value = encode_quantity_units_v1(&wrong_scale).unwrap();
        assert!(matches!(
            prepare(&claims, &changed, inputs),
            Err(Error::TransferInvariant { details }) if details.contains("occurrence")
        ));
    }
}

#[test]
fn full_width_rows_reject_changed_scales_high_values_and_occurrences() {
    let (claims, rows, inputs) = fixture(vec![delta(
        maximum().try_sub(&Quantity::one()).unwrap(),
        tiny(),
        Quantity::one(),
    )]);
    let sender = prepare(&claims, &rows, inputs).unwrap().pairs()[0].row_indices[0];
    for mutation in 0..9 {
        let mut changed_claims = claims.clone();
        let mut changed = rows.clone();
        match mutation {
            0 => {
                let q = decode_quantity_units_v1(&changed[sender].pre_value)
                    .unwrap()
                    .to_quantity()
                    .unwrap();
                changed[sender].pre_value =
                    encode_quantity_units_v1(&FastpqQuantityUnits::from_quantity(&q, 27).unwrap())
                        .unwrap();
            }
            1 => {
                let new_before = Quantity::from(u128::MAX);
                changed_claims[0].deltas[0].from_balance_after =
                    new_before.try_sub(&Quantity::one()).unwrap();
                changed_claims[0].deltas[0].from_balance_before = new_before;
                set_digest(&mut changed_claims[0]);
            }
            2 => changed[1] = changed[0].clone(),
            3 => {
                changed.pop();
            }
            4 => changed[sender].key.push(0),
            5 => changed[sender].operation = OperationKind::MetaSet,
            6 => changed[sender].pre_value.push(0),
            7 => changed_claims[0].poseidon_preimage_digest = None,
            _ => changed.reverse(),
        }
        assert!(
            prepare(&changed_claims, &changed, inputs).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn full_quantity_arithmetic_rejects_underflow_nonrepresentable_results_and_false_self_legs() {
    let original = delta(maximum(), Quantity::zero(), Quantity::one());
    for mutation in 0..4 {
        let mut d = original.clone();
        match mutation {
            0 => {
                d.amount = maximum();
                d.from_balance_before = Quantity::one();
            }
            1 => {
                d.to_balance_before = maximum();
                d.to_balance_after = maximum();
            }
            2 => {
                d.amount = tiny();
                d.from_balance_after = maximum();
            }
            _ => d.to_account = d.from_account.clone(),
        }
        let scale = if mutation == 2 { 28 } else { 0 };
        assert!(normalized_values_for::<FastpqQuantityUnits>(&d, scale).is_err());
    }
}

#[test]
fn full_quantity_repeated_keys_and_zero_self_occurrences_remain_exact() {
    let first = delta(Quantity::from(u128::MAX), Quantity::zero(), Quantity::one());
    let second = delta(
        first.from_balance_after.clone(),
        first.to_balance_after.clone(),
        tiny(),
    );
    let (claims, rows, inputs) = fixture(vec![first, second]);
    let table = prepare(&claims, &rows, inputs).unwrap();
    assert_eq!(table.pairs().len(), 2);
    assert!(table.rows().iter().all(|row| row.asset_scale == 28));
    let middle: [u8; 32] = Hash::new(b"committed intermediate").into();
    let statements = table.compact_statements(&[middle]).unwrap();
    assert_eq!(statements[0].new_root, statements[1].old_root);
    assert!(table.compact_statements(&[[0; 32]]).is_err());
    let mut broken = claims.clone();
    broken[0].deltas[1].from_balance_before = Quantity::from(20_u32);
    broken[0].deltas[1].from_balance_after = Quantity::from(20_u32).try_sub(&tiny()).unwrap();
    assert!(prepare(&broken, &rows, inputs).is_err());
    let mut d = delta(maximum(), maximum(), Quantity::zero());
    d.to_account = d.from_account.clone();
    let (claims, rows, inputs) = fixture(vec![d.clone(), d]);
    assert!(rows.windows(2).all(|pair| pair[0] == pair[1]));
    let table = prepare(&claims, &rows, inputs).unwrap();
    assert_eq!(table.pairs()[0].row_indices, [0, 1]);
    assert_eq!(table.pairs()[1].row_indices, [2, 3]);
    assert_ne!(table.pairs()[0].occurrence, table.pairs()[1].occurrence);
}

#[test]
fn full_quantity_preparation_enforces_public_limits_and_profile_rules() {
    let (claims, rows, inputs) = fixture(vec![delta(maximum(), Quantity::zero(), Quantity::one())]);
    let exact = prepare(&claims, &rows, inputs).unwrap().work().public_bytes;
    let limits = PublicTransferLimits {
        max_public_bytes: exact,
        ..PublicTransferLimits::default()
    };
    assert!(
        prepare_quantity_public_transfers(
            &rows,
            &claims,
            inputs,
            ProofSemantics::AxtTransferClaim,
            limits
        )
        .is_ok()
    );
    for limited in [
        PublicTransferLimits {
            max_public_bytes: exact - 1,
            ..limits
        },
        PublicTransferLimits {
            max_rows: 1,
            ..limits
        },
        PublicTransferLimits {
            max_transcripts: 0,
            ..limits
        },
        PublicTransferLimits {
            max_deltas: 0,
            ..limits
        },
        PublicTransferLimits {
            max_unique_keys: 1,
            ..limits
        },
        PublicTransferLimits {
            max_allocation_steps: 0,
            ..limits
        },
    ] {
        assert!(
            prepare_quantity_public_transfers(
                &rows,
                &claims,
                inputs,
                ProofSemantics::StateTransition,
                limited
            )
            .is_err()
        );
    }
    assert!(
        prepare_quantity_public_transfers(
            &rows,
            &claims,
            inputs,
            ProofSemantics::AxtOpaqueEffect,
            limits
        )
        .is_err()
    );
    let mut unmarked = inputs;
    unmarked.old_root[31] &= !1;
    assert!(prepare(&claims, &rows, unmarked).is_err());
    let empty = prepare(&[], &[], PublicInputs::default()).unwrap();
    assert!(empty.rows().is_empty());
    assert!(empty.compact_statements(&[]).unwrap().is_empty());
    assert!(
        prepare_quantity_public_transfers(
            &[],
            &[],
            PublicInputs::default(),
            ProofSemantics::AxtTransferClaim,
            limits
        )
        .is_err()
    );
}
