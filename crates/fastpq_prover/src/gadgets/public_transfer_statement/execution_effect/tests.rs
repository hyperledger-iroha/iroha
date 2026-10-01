//! Complete execution-effect semantics, authority binding and generic SMT regression coverage.

use super::*;
use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::{AssetBalanceScope, AssetDefinitionId},
    fastpq::{
        FastpqExecutionBalanceV1, FastpqExecutionEffectContextV1, FastpqExecutionEffectV1,
        FastpqExecutionSupplyChangeV1, FastpqExecutionTransferV1, FastpqSourceExecutionEntryV1,
        FastpqSourceExecutionKindV1, FastpqSourceStatementContextV1,
        execution_effect_statement_digest_v1, execution_effects_digest_v1,
    },
    nexus::AxtAssetIncarnationV1,
};
use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
use iroha_test_samples::{ALICE_ID, BOB_ID};

/// Narrow one small fixture position or level to its `u32` wire field.
fn narrow_u32(value: usize) -> u32 {
    u32::try_from(value).expect("fixture position fits u32")
}

fn balance(account: &AccountId) -> FastpqExecutionBalanceV1 {
    FastpqExecutionBalanceV1 {
        asset: FastpqExecutionAssetV1 {
            definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            incarnation: AxtAssetIncarnationV1::try_from_bytes(
                Hash::new(b"actual registry fixture token").into(),
            )
            .unwrap(),
        },
        account: account.clone(),
        scope: AssetBalanceScope::Global,
    }
}
fn transfer(amount: u64, sender: u64, receiver: u64) -> FastpqExecutionEffectKindV1 {
    FastpqExecutionEffectKindV1::Transfer(FastpqExecutionTransferV1 {
        source: balance(&ALICE_ID),
        destination: balance(&BOB_ID),
        amount: amount.into(),
        source_before: sender.into(),
        source_after: (sender - amount).into(),
        destination_before: receiver.into(),
        destination_after: (receiver + amount).into(),
    })
}
fn supply(mint: bool, amount: u64, before: u64, total: u64) -> FastpqExecutionEffectKindV1 {
    let change = FastpqExecutionSupplyChangeV1 {
        balance: balance(&ALICE_ID),
        amount: amount.into(),
        balance_before: before.into(),
        balance_after: (if mint {
            before + amount
        } else {
            before - amount
        })
        .into(),
        supply_before: total.into(),
        supply_after: (if mint { total + amount } else { total - amount }).into(),
    };
    if mint {
        FastpqExecutionEffectKindV1::Mint(change)
    } else {
        FastpqExecutionEffectKindV1::Burn(change)
    }
}
fn tape(kinds: Vec<FastpqExecutionEffectKindV1>) -> FastpqExecutionEffectsV1 {
    FastpqExecutionEffectsV1 {
        context: FastpqExecutionEffectContextV1 {
            source: FastpqSourceStatementContextV1 {
                network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"network"),
                )),
                height: 7,
            },
            entry: FastpqSourceExecutionEntryV1 {
                entry_hash: Hash::new(b"source call"),
                execution_kind: FastpqSourceExecutionKindV1::ExecutionCall,
                route: FastpqSourceRouteV1::Unrouted,
                dataspace_id: DataSpaceId::UNIVERSAL,
            },
        },
        effects: kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| FastpqExecutionEffectV1 {
                ordinal: u32::try_from(index).unwrap(),
                authority_digest: Hash::new(b"actual authority"),
                authorization_context: Hash::new(b"actual operation authorization context"),
                kind,
            })
            .collect(),
    }
}
fn public_inputs() -> FastpqPublicInputs {
    let root = Hash::new(b"caller unchanged empty root").into();
    FastpqPublicInputs {
        dsid: [0; 16],
        slot: 99,
        old_root: root,
        new_root: root,
        perm_root: Hash::new(b"permissions").into(),
        tx_set_hash: Hash::new(b"transaction set").into(),
    }
}
fn trees() -> TransferSmtBuildLimits {
    TransferSmtBuildLimits::for_update_limit(32).unwrap()
}
fn materialize(effects: &FastpqExecutionEffectsV1) -> Result<ExecutionEffectMaterialization> {
    materialize_execution_effect_statement(
        effects,
        execution_effects_digest_v1(effects).unwrap(),
        public_inputs(),
        ExecutionEffectLimits::default(),
        trees(),
    )
}
fn expected(statement: &FastpqExecutionEffectStatementV1) -> ExecutionEffectExpectations {
    // Test-only trusted fixture owner. Production must derive these through authenticated source evidence.
    ExecutionEffectExpectations {
        effects_digest: execution_effects_digest_v1(&statement.effects).unwrap(),
        statement_digest: execution_effect_statement_digest_v1(statement).unwrap(),
        public_inputs: statement.public_inputs,
    }
}
fn check(statement: &FastpqExecutionEffectStatementV1) -> Result<PreparedExecutionEffects> {
    prepare_execution_effect_statement(
        statement,
        expected(statement),
        ExecutionEffectLimits::default(),
    )
}

#[test]
fn transfer_mint_transfer_and_transfer_burn_transfer_preserve_complete_chronology() {
    for mint in [true, false] {
        let after = if mint { 14 } else { 4 };
        let effects = tape(vec![
            transfer(1, 10, 0),
            supply(mint, 5, 9, 10),
            transfer(2, after, 1),
        ]);
        let built = materialize(&effects).unwrap();
        assert_eq!(built.statement.effects, effects);
        assert_eq!(built.statement.transitions.len(), 6);
        let prepared = check(&built.statement).unwrap();
        assert_eq!(prepared.keys().len(), 3);
        assert_eq!(prepared.rows().len(), 6);
        assert_eq!(
            prepared.build_smt_witnesses(trees()).unwrap(),
            built.witnesses
        );
        assert_eq!(built.witnesses.work().updates, 6);
        assert_eq!(built.witnesses.work().sibling_hashes, 192);
        let intermediate: Vec<_> = built.witnesses.intermediate_roots().collect();
        let statements = prepared.compact_statements(&intermediate).unwrap();
        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0].new_root, statements[1].old_root);
        assert_eq!(statements[1].new_root, statements[2].old_root);
        for (index, statement) in statements.iter().enumerate() {
            let pair = prepared.pair(index).unwrap();
            assert_eq!(pair.occurrence, [0, narrow_u32(index), narrow_u32(index)]);
            for leg in 0..2 {
                let row = &prepared.rows()[pair.row_indices[leg]];
                assert_eq!(row.effect_ordinal as usize, index);
                assert_eq!(row.leg, leg);
                assert_eq!(statement.updates[leg], row.update);
                let witness = &built.witnesses.pairs()[index][leg];
                assert_eq!(
                    witness.root_before,
                    if leg == 0 {
                        if index == 0 {
                            built.statement.public_inputs.old_root
                        } else {
                            intermediate[index - 1]
                        }
                    } else {
                        built.witnesses.pairs()[index][0].root_after
                    }
                );
            }
        }
        // Removing the intervening original operation cannot be repaired by a gap inference.
        let mut missing = effects.clone();
        missing.effects.remove(1);
        missing.effects[1].ordinal = 1;
        assert!(
            materialize(&missing)
                .unwrap_err()
                .to_string()
                .contains("do not chain")
        );
    }
}

#[test]
fn supply_chain_arithmetic_ordinals_and_rows_fail_closed() {
    let base = tape(vec![
        transfer(1, 10, 0),
        supply(true, 5, 9, 10),
        supply(false, 2, 14, 15),
        transfer(1, 12, 1),
    ]);
    materialize(&base).unwrap();
    for mutation in 0..7 {
        let mut changed = base.clone();
        match mutation {
            0 => {
                changed.effects.swap(1, 2);
                for (i, e) in changed.effects.iter_mut().enumerate() {
                    e.ordinal = narrow_u32(i);
                }
            }
            1 => changed.effects[1].ordinal = 0,
            2 => {
                if let FastpqExecutionEffectKindV1::Mint(m) = &mut changed.effects[1].kind {
                    m.supply_after = 16u32.into();
                }
            }
            3 => {
                if let FastpqExecutionEffectKindV1::Burn(b) = &mut changed.effects[2].kind {
                    b.supply_before = 17u32.into();
                    b.supply_after = 15u32.into();
                }
            }
            4 => {
                if let FastpqExecutionEffectKindV1::Transfer(t) = &mut changed.effects[3].kind {
                    t.source_before = 13u32.into();
                    t.source_after = 12u32.into();
                }
            }
            5 => {
                if let FastpqExecutionEffectKindV1::Transfer(t) = &mut changed.effects[0].kind {
                    t.destination.asset.incarnation = AxtAssetIncarnationV1::try_from_bytes(
                        Hash::new(b"other incarnation").into(),
                    )
                    .unwrap();
                }
            }
            _ => changed.context.source.height = 0,
        }
        assert!(materialize(&changed).is_err(), "mutation {mutation}");
    }
    let built = materialize(&base).unwrap();
    for mutation in 0..5 {
        let mut changed = built.statement.clone();
        match mutation {
            0 => {
                changed.transitions.pop();
            }
            1 => changed.transitions.swap(0, 1),
            2 => changed.transitions[0].operation = FastpqOperationKind::MetaSet,
            3 => changed.transitions[0].pre_value = changed.transitions[0].post_value.clone(),
            _ => changed.ordering_hash = Hash::new(b"wrong ordering").into(),
        }
        // Even granting a new expected outer digest cannot legitimize invalid semantics.
        assert!(check(&changed).is_err(), "row mutation {mutation}");
    }
}

#[test]
fn independent_facts_context_authority_and_final_inputs_are_required() {
    let base = tape(vec![transfer(1, 10, 0), supply(true, 5, 9, 10)]);
    let built = materialize(&base).unwrap();
    let external = expected(&built.statement);
    for mutation in 0..7 {
        let mut changed = built.statement.clone();
        match mutation {
            0 => changed.effects.effects[0].authority_digest = Hash::new(b"different signer set"),
            1 => {
                changed.effects.effects[1].authorization_context =
                    Hash::new(b"invented mint authority")
            }
            2 => changed.effects.context.source.height += 1,
            3 => changed.effects.context.entry.entry_hash = Hash::new(b"different execution"),
            4 => changed.effects.context.entry.dataspace_id = DataSpaceId::new(5),
            5 => changed.public_inputs.perm_root = Hash::new(b"different permission state").into(),
            _ => changed.public_inputs.new_root = Hash::new(b"different root").into(),
        }
        assert!(
            prepare_execution_effect_statement(
                &changed,
                external,
                ExecutionEffectLimits::default()
            )
            .is_err()
        );
    }
    let mut different = base.clone();
    different.effects[1].authorization_context = Hash::new(b"invented context");
    assert!(
        materialize_execution_effect_statement(
            &different,
            external.effects_digest,
            public_inputs(),
            ExecutionEffectLimits::default(),
            trees()
        )
        .is_err()
    );
    let mut wrong = external;
    wrong.effects_digest = Hash::new(b"wrong facts");
    assert!(
        prepare_execution_effect_statement(
            &built.statement,
            wrong,
            ExecutionEffectLimits::default()
        )
        .is_err()
    );
    let mut changed = built.statement.clone();
    changed.public_inputs.new_root = Hash::new(b"caller asserted unproved root").into();
    let prepared = check(&changed).unwrap();
    assert!(prepared.build_smt_witnesses(trees()).is_err());
}

#[test]
fn wide_fractional_quantities_zero_effects_and_self_transfer_remain_exact() {
    use super::super::quantity_tests::{maximum, tiny};
    let max = maximum();
    let tiny = tiny();
    let mut t = match transfer(0, 0, 0) {
        FastpqExecutionEffectKindV1::Transfer(t) => t,
        _ => unreachable!(),
    };
    t.amount = Quantity::one();
    t.source_before = max.clone();
    t.source_after = max.try_sub(&Quantity::one()).unwrap();
    t.destination_before = tiny.clone();
    t.destination_after = tiny.try_add(&Quantity::one()).unwrap();
    let effects = tape(vec![FastpqExecutionEffectKindV1::Transfer(t)]);
    let built = materialize(&effects).unwrap();
    let prepared = check(&built.statement).unwrap();
    assert!(prepared.rows().iter().all(|row| row.scale == 28));
    assert!(
        prepared
            .rows()
            .iter()
            .any(|row| row.before.try_to_u64().is_none())
    );
    assert_eq!(built.statement.effects, effects);
    let mut self_transfer = match transfer(2, 10, 8) {
        FastpqExecutionEffectKindV1::Transfer(t) => t,
        _ => unreachable!(),
    };
    self_transfer.destination = self_transfer.source.clone();
    let self_tape = tape(vec![FastpqExecutionEffectKindV1::Transfer(self_transfer)]);
    let built = materialize(&self_tape).unwrap();
    assert_eq!(
        built.statement.public_inputs.old_root,
        built.statement.public_inputs.new_root
    );
    assert_eq!(check(&built.statement).unwrap().keys().len(), 1);
    let zeros = tape(vec![
        transfer(0, 10, 0),
        transfer(0, 10, 0),
        supply(false, 0, 10, 10),
    ]);
    let built = materialize(&zeros).unwrap();
    assert_eq!(built.witnesses.work().updates, 6);
    assert_eq!(
        built.statement.public_inputs.old_root,
        built.statement.public_inputs.new_root
    );
    let mut wrong_tag = built.statement.clone();
    let zero_transfer = wrong_tag
        .transitions
        .iter_mut()
        .find(|row| row.operation == FastpqOperationKind::Transfer)
        .unwrap();
    zero_transfer.operation = FastpqOperationKind::Burn;
    assert!(check(&wrong_tag).is_err());
    assert!(materialize(&tape(vec![supply(true, 0, 10, 10)])).is_err());
    let mut invalid = self_tape;
    if let FastpqExecutionEffectKindV1::Transfer(t) = &mut invalid.effects[0].kind {
        t.destination_before = 9u32.into();
        t.destination_after = 11u32.into();
    }
    assert!(materialize(&invalid).is_err());
}

#[test]
fn exact_limits_empty_entries_and_duplicate_typed_keys() {
    let effects = tape(vec![transfer(1, 10, 0), supply(true, 5, 9, 10)]);
    let built = materialize(&effects).unwrap();
    let base = check(&built.statement).unwrap();
    let exact = ExecutionEffectLimits {
        max_effects: 2,
        max_rows: 4,
        max_public_bytes: norito::encode_canonical(&built.statement).unwrap().len(),
        max_unique_keys: 3,
        max_allocation_steps: base.work().allocation_steps,
    };
    let prepared =
        prepare_execution_effect_statement(&built.statement, expected(&built.statement), exact)
            .unwrap();
    assert_eq!(prepared.work().public_bytes, exact.max_public_bytes);
    for limit in 0..5 {
        let mut low = exact;
        match limit {
            0 => low.max_effects -= 1,
            1 => low.max_rows -= 1,
            2 => low.max_public_bytes -= 1,
            3 => low.max_unique_keys -= 1,
            _ => low.max_allocation_steps -= 1,
        }
        assert!(
            prepare_execution_effect_statement(&built.statement, expected(&built.statement), low)
                .is_err()
        );
    }
    let empty = tape(vec![]);
    let built = materialize(&empty).unwrap();
    let prepared = check(&built.statement).unwrap();
    assert!(prepared.rows().is_empty());
    assert!(prepared.compact_statements(&[]).unwrap().is_empty());
    assert_eq!(built.statement.public_inputs, public_inputs());
    let mut changed = built.statement;
    changed.public_inputs.new_root = Hash::new(b"illegal empty root change").into();
    assert!(check(&changed).is_err());
    // Same controller and amount in a different explicit scope/incarnation never chain as one key.
    let mut second = transfer(1, 10, 0);
    if let FastpqExecutionEffectKindV1::Transfer(t) = &mut second {
        t.source.scope = AssetBalanceScope::Dataspace(DataSpaceId::new(3));
        t.destination.scope = t.source.scope;
    }
    let scoped = materialize(&tape(vec![transfer(1, 10, 0), second])).unwrap();
    assert_eq!(check(&scoped.statement).unwrap().keys().len(), 4);
}

#[test]
#[ignore = "requires retained output of the genuine Core signed-effect producer"]
fn genuine_core_capture_preserves_all_effects_and_refuses_substitution() {
    use std::io::Read;
    let path = std::env::var_os("IROHA_FASTPQ_GENUINE_EFFECT_CAPTURE_PATH")
        .expect("explicit genuine Core capture path is required");
    let expected_frame_hash = std::env::var("IROHA_FASTPQ_GENUINE_EFFECT_CAPTURE_HASH")
        .expect("exact retained Core frame hash is required");
    assert_eq!(expected_frame_hash.len(), 64);
    assert!(
        expected_frame_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(
        bytes.len() <= 1_048_576,
        "capture exceeds its complete frame bound"
    );
    assert_eq!(hex::encode(Hash::new(&bytes).as_ref()), expected_frame_hash);
    let (effects, inputs): (FastpqExecutionEffectsV1, FastpqPublicInputs) =
        norito::decode_canonical_with_limits(
            &bytes,
            norito::DecodeLimits::new(4096, 1_048_576, 16384, 4_194_304, 64),
        )
        .unwrap();
    assert_eq!(
        norito::encode_canonical(&(effects.clone(), inputs)).unwrap(),
        bytes
    );
    assert_eq!(effects.context.source.height, 2);
    assert_eq!(effects.effects.len(), 4);
    assert_eq!(inputs.slot, 2_000_000);
    assert_eq!(inputs.old_root, [0; 32]);
    assert_eq!(inputs.new_root, [0; 32]);
    let [first, mint, burn, last] = effects.effects.as_slice() else {
        panic!("all four native effects are required")
    };
    let FastpqExecutionEffectKindV1::Transfer(first) = &first.kind else {
        panic!("first transfer")
    };
    let FastpqExecutionEffectKindV1::Mint(mint) = &mint.kind else {
        panic!("actual mint")
    };
    let FastpqExecutionEffectKindV1::Burn(burn) = &burn.kind else {
        panic!("actual burn")
    };
    let FastpqExecutionEffectKindV1::Transfer(last) = &last.kind else {
        panic!("last transfer")
    };
    assert_eq!(
        (&first.source.account, &first.destination.account),
        (&*ALICE_ID, &*BOB_ID)
    );
    assert_eq!(
        (&last.source, &last.destination),
        (&first.source, &first.destination)
    );
    assert_eq!(
        (&mint.balance, &burn.balance),
        (&first.source, &first.source)
    );
    assert_eq!(first.source.scope, AssetBalanceScope::Global);
    assert_eq!(
        (
            first.amount.clone(),
            mint.amount.clone(),
            burn.amount.clone(),
            last.amount.clone()
        ),
        (1_u32.into(), 5_u32.into(), 1_u32.into(), 2_u32.into())
    );
    assert_eq!(
        (
            first.source_before.clone(),
            first.source_after.clone(),
            mint.balance_after.clone(),
            burn.balance_after.clone(),
            last.source_after.clone()
        ),
        (
            10_u32.into(),
            9_u32.into(),
            14_u32.into(),
            13_u32.into(),
            11_u32.into()
        )
    );
    assert_eq!(
        (
            mint.supply_before.clone(),
            mint.supply_after.clone(),
            burn.supply_before.clone(),
            burn.supply_after.clone()
        ),
        (10_u32.into(), 15_u32.into(), 15_u32.into(), 14_u32.into())
    );
    assert_eq!(last.destination_after, Quantity::from(3_u32));

    // This expected tape is retained from the separately authenticated Core
    // producer. Re-hashing an offered mutant below cannot change that baseline.
    // No finality or production source authority is claimed by this test.
    let effects_digest = execution_effects_digest_v1(&effects).unwrap();
    let limits = ExecutionEffectLimits::default();
    let tree_limits = TransferSmtBuildLimits::for_update_limit(8).unwrap();
    let built = materialize_execution_effect_statement(
        &effects,
        effects_digest,
        inputs,
        limits,
        tree_limits,
    )
    .unwrap();
    assert_eq!(built.statement.effects, effects);
    let expected = expected(&built.statement);
    let prepared = prepare_execution_effect_statement(&built.statement, expected, limits).unwrap();
    assert_eq!(prepared.rows().len(), 8);
    assert_eq!(prepared.keys().len(), 3);
    assert_eq!(
        prepared.build_smt_witnesses(tree_limits).unwrap(),
        built.witnesses
    );
    let intermediate: Vec<_> = built.witnesses.intermediate_roots().collect();
    let compact = prepared.compact_statements(&intermediate).unwrap();
    assert_eq!(compact.len(), 4);
    for pair in compact.windows(2) {
        assert_eq!(pair[0].new_root, pair[1].old_root);
    }
    for ordinal in 0..4 {
        for leg in 0..2 {
            assert_eq!(
                prepared
                    .rows()
                    .iter()
                    .filter(|row| row.effect_ordinal == ordinal && row.leg == leg)
                    .count(),
                1
            );
        }
    }

    for mutation in 0..6 {
        let mut offered = built.statement.clone();
        match mutation {
            0 => {
                offered.effects.effects.remove(1);
            }
            1 => offered.effects.effects.swap(1, 2),
            2 => offered.effects.effects[0].authority_digest = Hash::new(b"substituted authority"),
            3 => {
                offered.effects.effects[1].authorization_context =
                    Hash::new(b"substituted mint owner")
            }
            4 => {
                offered.effects.context.entry.entry_hash =
                    Hash::new(b"substituted signed invocation")
            }
            5 => offered.public_inputs.tx_set_hash = Hash::new(b"substituted source wires").into(),
            _ => unreachable!(),
        }
        assert!(
            prepare_execution_effect_statement(&offered, expected, limits).is_err(),
            "mutation {mutation}"
        );
    }
    // Recomputed diagnostic tape digests must not hide a chronology or supply
    // error, independently of the fixed producer commitment checks above.
    for mutation in 0..3 {
        let mut changed = effects.clone();
        match mutation {
            0 => {
                changed.effects.remove(1);
            }
            1 => changed.effects.swap(1, 2),
            2 => {
                let FastpqExecutionEffectKindV1::Burn(burn) = &mut changed.effects[2].kind else {
                    unreachable!()
                };
                burn.supply_before = Quantity::from(16_u32);
                burn.supply_after = Quantity::from(15_u32);
            }
            _ => unreachable!(),
        }
        for (index, effect) in changed.effects.iter_mut().enumerate() {
            effect.ordinal = u32::try_from(index).unwrap();
        }
        assert!(
            matches!(materialize_execution_effect_statement(&changed, execution_effects_digest_v1(&changed).unwrap(), inputs, limits, tree_limits), Err(crate::Error::TransferInvariant { details }) if details.contains("repeated-key")),
            "semantic mutation {mutation}"
        );
    }
    assert_eq!(norito::encode_canonical(&(effects, inputs)).unwrap(), bytes);
}
