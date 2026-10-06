//! Real WSV/canonical asset-owner tests; native fixture verification remains explicitly local.
use super::*;
use crate::{
    smartcontracts::Execute as _,
    state::{State, StateTransaction, World, WorldReadOnly},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    Registrable,
    account::Account,
    asset::{Asset, AssetBalancePolicy, AssetDefinition, AssetId},
    block::BlockHeader,
    isi::{Burn, Transfer, Unregister},
    nexus::AxtAssetIncarnationV1,
    permission::Permissions,
};
use iroha_executor_data_model::permission::asset_definition::CanManageKagemushaWallet;
use iroha_primitives::numeric::{Numeric, NumericSpec, Quantity};
use mv::storage::StorageReadOnly as _;

fn world_state(memory: &Memory, permission: bool) -> State {
    let r = &memory.registration;
    let owner = &memory.authority;
    let definition = AssetDefinition::new(
        r.asset.asset.clone(),
        "offline test",
        NumericSpec::fractional(r.asset.scale),
        AssetBalancePolicy::Global,
        None,
    )
    .build(owner);
    let assets = memory
        .balances
        .iter()
        .map(|(owner, value)| {
            Asset::new(
                AssetId::of(r.asset.asset.clone(), owner.clone()),
                Quantity::from_canonical_numeric(Numeric::new(*value, r.asset.scale)).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let mut world = World::with_assets(
        [],
        memory
            .balances
            .keys()
            .map(|account| Account::new(account.clone()).build(account)),
        [definition],
        assets,
        [],
    );
    world.axt_asset_incarnations.insert(
        r.asset.asset.clone(),
        AxtAssetIncarnationV1::try_from_bytes(r.asset.asset_incarnation).unwrap(),
    );
    if permission {
        world.account_permissions.insert(
            r.reserve.clone(),
            Permissions::from_iter([CanManageKagemushaWallet {
                asset_definition: r.asset.asset.clone(),
            }
            .into()]),
        );
    }
    let mut state = State::new_for_testing(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    state.network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::from_untyped_unchecked(Hash::prehashed(r.scheme.network_id)),
    );
    state
}
fn transact<T>(
    state: &mut State,
    height: u64,
    action: impl FnOnce(&mut StateTransaction<'_, '_>) -> Result<T>,
) -> Result<T> {
    let header = BlockHeader::new(height.try_into().unwrap(), None, None, height * 1_000, 0);
    let mut block = state.block(header.clone());
    let invocation = Hash::new(height.to_le_bytes());
    let mut tx = block.transaction_for_fastpq_testing(invocation);
    // Explicit local fixture binding: this is not network/finality qualification.
    tx.current_network_entrypoint_hash = Some(HashOf::from_untyped_unchecked(invocation));
    let result = action(&mut tx)?;
    tx.apply();
    block.commit_world_overlay_for_testing().unwrap();
    state.push_block_hash_for_testing(header.hash());
    Ok(result)
}
fn register(state: &mut State, memory: &Memory, height: u64) -> Result<()> {
    transact(state, height, |tx| {
        wsv::WsvLedger::new(tx, &memory.registration.reserve)?.register(memory.registration.clone())
    })
}
fn balance(state: &State, memory: &Memory, owner: &AccountId) -> Quantity {
    state
        .view()
        .world
        .assets()
        .get(&AssetId::of(
            memory.registration.asset.asset.clone(),
            owner.clone(),
        ))
        .map(|v| v.as_ref().clone())
        .unwrap_or_else(Quantity::zero)
}
#[test]
fn registration_requires_dedicated_asset_permission_and_reserve_consent() {
    let memory = Memory::new();
    let mut state = world_state(&memory, false);
    assert!(register(&mut state, &memory, 1).is_err());
    assert!(
        state
            .view()
            .world
            .kagemusha_wallet_ledger()
            .iter()
            .next()
            .is_none()
    );
    // Owning the asset alone cannot capture another account's reserve bucket.
    assert!(
        transact(&mut state, 1, |tx| wsv::WsvLedger::new(
            tx,
            &memory.authority
        )?
        .register(memory.registration.clone()))
        .is_err()
    );
    transact(&mut state, 1, |tx| {
        tx.world.account_permissions.insert(
            memory.registration.reserve.clone(),
            Permissions::from_iter([CanManageKagemushaWallet {
                asset_definition:
                    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                        iroha_model_base::domain::DomainId::try_new("wrong", "universal").unwrap(),
                        "coin".parse().unwrap(),
                    ),
            }
            .into()]),
        );
        Ok(())
    })
    .unwrap();
    assert!(register(&mut state, &memory, 2).is_err());
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    register(&mut state, &memory, 2).unwrap();
    let view = state.view();
    assert!(custody::is_reserve_account(&view.world, &memory.registration.reserve).unwrap());
    assert!(custody::is_reserve_definition(&view.world, &memory.registration.asset.asset).unwrap());
    let refs: u64 = storage::decode(
        view.world
            .kagemusha_wallet_ledger()
            .get(&storage::reserve_account_key(&memory.registration.reserve).unwrap())
            .unwrap(),
        128,
    )
    .unwrap();
    assert_eq!(
        refs, 1,
        "exact retry must not increment permanent references"
    );
}
#[test]
fn registered_reserve_rejects_ordinary_transfer_burn_and_teardown() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let reserve = custody::reserve_id(&memory.registration);
    let before = balance(&state, &memory, &memory.registration.reserve);
    let amount =
        Quantity::from_canonical_numeric(Numeric::new(1_u32, memory.registration.asset.scale))
            .unwrap();
    assert!(
        transact(&mut state, 2, |tx| Transfer::asset_quantity(
            reserve.clone(),
            amount.clone(),
            memory.authority.clone()
        )
        .execute(&memory.registration.reserve, tx)
        .map_err(Error::from))
        .is_err()
    );
    assert!(
        transact(&mut state, 2, |tx| Burn::asset_quantity(
            amount,
            reserve.clone()
        )
        .execute(&memory.registration.reserve, tx)
        .map_err(Error::from))
        .is_err()
    );
    assert!(
        transact(&mut state, 2, |tx| Unregister::account(
            memory.registration.reserve.clone()
        )
        .execute(&memory.registration.reserve, tx)
        .map_err(Error::from))
        .is_err()
    );
    assert!(
        transact(&mut state, 2, |tx| Unregister::asset_definition(
            memory.registration.asset.asset.clone()
        )
        .execute(&memory.authority, tx)
        .map_err(Error::from))
        .is_err()
    );
    assert_eq!(
        balance(&state, &memory, &memory.registration.reserve),
        before
    );
}
#[test]
fn native_rejection_and_missing_source_anchor_cannot_activate() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let activation: KagemushaWalletActivationV1 = fixture("KagemushaWalletActivationV1");
    let before = state.view().world.kagemusha_wallet_ledger().iter().count();
    assert!(matches!(
        transact(&mut state, 2, |tx| activate(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &Verifier::new(false),
            &activation
        )),
        Err(Error::Proof)
    ));
    assert_eq!(
        state.view().world.kagemusha_wallet_ledger().iter().count(),
        before
    );
    assert!(matches!(
        transact(&mut state, 2, |tx| {
            tx.current_network_entrypoint_hash = None;
            wsv::WsvLedger::new(tx, &memory.authority).map(|_| ())
        }),
        Err(Error::Binding)
    ));
}
#[test]
fn real_asset_batch_loads_once_and_failed_debit_rolls_back_ordinal() {
    let mut memory = Memory::new();
    let command = memory.active();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let activation = fixture("KagemushaWalletActivationV1");
    transact(&mut state, 2, |tx| {
        activate(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &Verifier::new(true),
            &activation,
        )
    })
    .unwrap();
    let prior = balance(&state, &memory, &memory.registration.reserve);
    let first = transact(&mut state, 3, |tx| {
        issue_load(&mut wsv::WsvLedger::new(tx, &memory.authority)?, &command)
    })
    .unwrap();
    let funded = balance(&state, &memory, &memory.registration.reserve);
    assert_eq!(
        funded,
        prior
            .checked_add(
                &Quantity::from_canonical_numeric(Numeric::new(
                    command.amount,
                    memory.registration.asset.scale
                ))
                .unwrap()
            )
            .unwrap()
    );
    assert_eq!(
        transact(&mut state, 4, |tx| issue_load(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &command
        ))
        .unwrap(),
        first
    );
    assert_eq!(
        balance(&state, &memory, &memory.registration.reserve),
        funded
    );
    let mut overdrawn = command.clone();
    overdrawn.request_id = [0x79; 32];
    overdrawn.amount = 10_000;
    assert!(
        transact(&mut state, 5, |tx| issue_load(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &overdrawn
        ))
        .is_err()
    );
    assert_eq!(
        balance(&state, &memory, &memory.registration.reserve),
        funded
    );
    transact(&mut state, 5, |tx| {
        let ledger = wsv::WsvLedger::new(tx, &memory.authority)?;
        assert_eq!(
            ledger
                .wallet(&command.scheme, &command.wallet)?
                .unwrap()
                .next_load,
            1
        );
        assert!(
            ledger
                .issuance(&command.scheme, &command.wallet, &overdrawn.request_id)?
                .is_none()
        );
        Ok(())
    })
    .unwrap();
    let rows: BTreeMap<_, _> = state
        .view()
        .world
        .kagemusha_wallet_ledger()
        .iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect();
    let check = |rows: &BTreeMap<KagemushaWalletLedgerKeyV1, Vec<u8>>| {
        storage::validate_snapshot(rows.iter(), |key| rows.get(key).map(Vec::as_slice))
    };
    check(&rows).unwrap();
    let pending = PendingPublication::from_issuance(&first);
    let pending_key = pending.key(command.scheme, first.body.authorizer_certificate);
    assert!(rows.contains_key(&pending_key));
    for missing_key in [
        pending_key,
        storage::issuance_key(command.scheme, command.wallet, command.request_id),
    ] {
        let mut missing = rows.clone();
        missing.remove(&missing_key);
        assert!(
            check(&missing).is_err(),
            "pending and original issuance must survive in the same snapshot generation"
        );
    }
    let mut wrong_certificate = rows;
    let bytes = wrong_certificate.remove(&pending_key).unwrap();
    wrong_certificate.insert(pending.key(command.scheme, [0x88; 32]), bytes);
    assert!(check(&wrong_certificate).is_err());
}
#[test]
fn finalized_reader_requires_real_current_qc_and_bounded_history() {
    use crate::sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig};
    use iroha_data_model::sumeragi::finality::NativeFinalityLimits;
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let limits = NativeFinalityLimits {
        block_bytes: 2_000_000,
        journal_bytes: 8_000_000,
        block_count: 8,
        allocated_bytes: 16_000_000,
    };
    assert!(matches!(
        FinalizedLedger::new(&chain.state().view(), limits),
        Err(Error::NotFinalized)
    ));
    chain.commit_at(2_000, Vec::new());
    {
        let view = chain.state().view();
        let reader = FinalizedLedger::new(&view, limits).unwrap();
        assert!(matches!(
            reader.issuance(&[1; 32], &[2; 32], &[3; 32]),
            Err(Error::Unavailable)
        ));
        let tiny = NativeFinalityLimits {
            block_bytes: 1,
            journal_bytes: 1,
            block_count: 1,
            allocated_bytes: 1,
        };
        assert!(FinalizedLedger::new(&view, tiny).is_err());
    }
    chain.commit_at(3_000, Vec::new());
    assert!(FinalizedLedger::new(&chain.state().view(), limits).is_ok());
    chain.corrupt_local_quorum_for_test(3, Signers::BelowQuorum);
    assert!(matches!(
        FinalizedLedger::new(&chain.state().view(), limits),
        Err(Error::NotFinalized)
    ));
}

#[test]
fn routing_uses_permanent_scope_and_refuses_missing_wallet_records() {
    use iroha_data_model::isi::kagemusha_wallet::{
        KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1,
    };
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let activation: KagemushaWalletActivationV1 = fixture("KagemushaWalletActivationV1");
    let instruction = KagemushaWalletLedgerV1::new(
        memory.registration.scheme.scheme_id(),
        KagemushaWalletLedgerActionV1::Activate(activation.to_canonical_bytes().unwrap()),
    );
    assert_eq!(
        routing::dataspace(&state.view().world, &instruction).unwrap(),
        iroha_model_base::topology::DataSpaceId::UNIVERSAL
    );
    let missing = KagemushaWalletLedgerV1::new(
        instruction.scheme,
        KagemushaWalletLedgerActionV1::IssueLoad {
            wallet: [3; 32],
            request_id: [4; 32],
            amount: 1,
            charge: None,
        },
    );
    assert!(matches!(
        routing::dataspace(&state.view().world, &missing),
        Err(Error::Unavailable)
    ));
}

#[test]
fn verifier_install_routing_uses_only_the_registered_asset_scope() {
    use iroha_data_model::isi::kagemusha_wallet::{
        KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1,
    };
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    // Deliberately unadmitted DATA: routing reads the permanent asset registration;
    // it neither mounts these empty originals nor grants install authority.
    let instruction = |asset| {
        KagemushaWalletLedgerV1::new(
            memory.registration.scheme.scheme_id(),
            KagemushaWalletLedgerActionV1::InstallVerifierPack {
                asset,
                manifest_digest: [9; 32],
                pack: Vec::new(),
            },
        )
    };
    assert_eq!(
        routing::dataspace(
            &state.view().world,
            &instruction(memory.registration.asset.asset_digest())
        )
        .unwrap(),
        iroha_model_base::topology::DataSpaceId::UNIVERSAL
    );
    assert!(matches!(
        routing::dataspace(&state.view().world, &instruction([7; 32])),
        Err(Error::Unavailable)
    ));
}

#[test]
fn package_quota_charges_exact_native_calls_and_one_operation_at_the_boundary() {
    let memory = Memory::new();
    let activation: KagemushaWalletActivationV1 = fixture("KagemushaWalletActivationV1");
    let unload: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    for (package, calls) in [(&activation.bootstrap, 1), (&unload.package, 2)] {
        assert_eq!(package.lineage.lineage().is_some(), calls == 2);
        let bytes = package.step_proof.bytes.len()
            + package
                .lineage
                .lineage()
                .map_or(0, |value| value.proof.len());
        assert!(bytes > 0);
        let mut state = world_state(&memory, true);
        transact(&mut state, 1, |tx| {
            tx.zk.max_proof_size_bytes = u32::try_from(bytes).unwrap();
            tx.zk.max_verify_calls_per_tx = calls;
            tx.zk.max_verify_calls_per_block = calls;
            tx.zk.max_confidential_ops_per_block = 1;
            tx.zk.max_proof_bytes_block = u64::try_from(bytes).unwrap();
            let rows = tx.world.kagemusha_wallet_ledger.iter().count();
            // Existing model fixtures are used only to count retained transport
            // work. No fixture proof enters the native verifier or gets a verdict.
            wsv::WsvLedger::new(tx, &memory.authority)?.reserve_package_proof(package)?;
            assert_eq!(tx.zk_confidential_ops_in_tx, 1);
            assert_eq!(tx.zk_verify_calls_in_tx, calls);
            assert_eq!(tx.zk_proof_bytes_in_tx, u64::try_from(bytes).unwrap());
            assert_eq!(tx.world.kagemusha_wallet_ledger.iter().count(), rows);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn package_quota_refusal_is_atomic_for_sigma_and_lineage_limits_and_overflow() {
    let memory = Memory::new();
    let activation: KagemushaWalletActivationV1 = fixture("KagemushaWalletActivationV1");
    let unload: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    for (package, calls) in [(&activation.bootstrap, 1), (&unload.package, 2)] {
        let bytes = package.step_proof.bytes.len()
            + package
                .lineage
                .lineage()
                .map_or(0, |value| value.proof.len());
        assert!(bytes > 1);
        for refusal in 0..10 {
            let mut state = world_state(&memory, true);
            transact(&mut state, 1, |tx| {
                tx.zk_confidential_ops_in_tx = 1;
                tx.zk_verify_calls_in_tx = 1;
                tx.zk_proof_bytes_in_tx = 13;
                tx.zk.max_proof_size_bytes = u32::try_from(bytes).unwrap();
                tx.zk.max_verify_calls_per_tx = calls + 1;
                tx.zk.max_verify_calls_per_block = calls + 1;
                tx.zk.max_confidential_ops_per_block = 2;
                tx.zk.max_proof_bytes_block = 13 + u64::try_from(bytes).unwrap();
                match refusal {
                    0 => tx.zk.max_verify_calls_per_tx = calls,
                    1 => tx.zk.max_verify_calls_per_block = calls,
                    2 => tx.zk.max_proof_bytes_block -= 1,
                    3 => tx.zk.max_proof_size_bytes -= 1,
                    4 => tx.zk.max_confidential_ops_per_block = 1,
                    5 => tx.zk_verify_calls_in_tx = u32::MAX,
                    6 => tx.zk_proof_bytes_in_tx = u64::MAX,
                    7 => tx.zk_confidential_ops_in_tx = u32::MAX,
                    8 => tx.zk_verify_calls_in_block_so_far = u32::MAX,
                    _ => tx.zk_proof_bytes_in_block_so_far = u64::MAX,
                }
                let before = (
                    tx.zk_confidential_ops_in_tx,
                    tx.zk_verify_calls_in_tx,
                    tx.zk_proof_bytes_in_tx,
                    tx.zk_confidential_ops_in_block_so_far,
                    tx.zk_verify_calls_in_block_so_far,
                    tx.zk_proof_bytes_in_block_so_far,
                );
                let rows = tx.world.kagemusha_wallet_ledger.iter().count();
                assert!(
                    wsv::WsvLedger::new(tx, &memory.authority)?
                        .reserve_package_proof(package)
                        .is_err(),
                    "native call count {calls}, refusal {refusal}"
                );
                assert_eq!(
                    (
                        tx.zk_confidential_ops_in_tx,
                        tx.zk_verify_calls_in_tx,
                        tx.zk_proof_bytes_in_tx,
                        tx.zk_confidential_ops_in_block_so_far,
                        tx.zk_verify_calls_in_block_so_far,
                        tx.zk_proof_bytes_in_block_so_far,
                    ),
                    before,
                    "refused whole-package reservation must leave no partial charge"
                );
                assert_eq!(tx.world.kagemusha_wallet_ledger.iter().count(), rows);
                Ok(())
            })
            .unwrap();
        }
    }
}

#[test]
fn lineage_call_quota_refuses_before_native_verifier_or_payout() {
    use iroha_data_model::isi::kagemusha_wallet::{
        KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1,
    };
    let memory = Memory::new();
    let claim: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    assert!(claim.package.lineage.lineage().is_some());
    claim.verify(&memory.registration.scheme).unwrap();
    for transaction_limit in [true, false] {
        let mut state = world_state(&memory, true);
        register(&mut state, &memory, 1).unwrap();
        let verifier = Verifier::new(true);
        let instruction = KagemushaWalletLedgerV1::new(
            memory.registration.scheme.scheme_id(),
            KagemushaWalletLedgerActionV1::Unload(claim.to_canonical_bytes().unwrap()),
        );
        let result: Result<()> = transact(&mut state, 2, |tx| {
            tx.zk.max_verify_calls_per_tx = if transaction_limit { 1 } else { 2 };
            tx.zk.max_verify_calls_per_block = if transaction_limit { 2 } else { 1 };
            let rows = tx.world.kagemusha_wallet_ledger.iter().count();
            let result = crate::smartcontracts::isi::kagemusha_wallet::execute_with_verifier(
                instruction,
                &memory.authority,
                tx,
                &verifier,
            );
            assert!(matches!(result, Err(Error::Execution(_))));
            assert_eq!(verifier.calls.get(), 0);
            assert_eq!(tx.zk_confidential_ops_in_tx, 0);
            assert_eq!(tx.zk_verify_calls_in_tx, 0);
            assert_eq!(tx.zk_proof_bytes_in_tx, 0);
            assert_eq!(tx.world.kagemusha_wallet_ledger.iter().count(), rows);
            result
        });
        assert!(matches!(result, Err(Error::Execution(_))));
        assert_eq!(verifier.calls.get(), 0);
    }
}

#[test]
fn unavailable_production_artifacts_retain_local_deferral_and_no_activation() {
    use iroha_data_model::isi::kagemusha_wallet::{
        KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1,
    };
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let activation: KagemushaWalletActivationV1 = fixture("KagemushaWalletActivationV1");
    let instruction = KagemushaWalletLedgerV1::new(
        memory.registration.scheme.scheme_id(),
        KagemushaWalletLedgerActionV1::Activate(activation.to_canonical_bytes().unwrap()),
    );
    let result: Result<()> = transact(&mut state, 2, |tx| {
        let rows = tx.world.kagemusha_wallet_ledger.iter().count();
        assert!(instruction.execute(&memory.authority, tx).is_err());
        assert_eq!(
            tx.execution_deferral().unwrap().reason(),
            ivm::error::ExecutionDeferral::VerifierArtifactsUnavailable
        );
        assert_eq!(tx.world.kagemusha_wallet_ledger.iter().count(), rows);
        Err(Error::ArtifactsUnavailable)
    });
    assert!(matches!(result, Err(Error::ArtifactsUnavailable)));
}

#[test]
fn verifier_install_requires_actual_reserve_owner_before_artifact_parsing() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let result: Result<()> = transact(&mut state, 2, |tx| {
        let rows = tx.world.kagemusha_wallet_ledger.iter().count();
        let result = wsv::WsvLedger::new(tx, &memory.authority)?.install_verifier_pack(
            memory.registration.scheme.scheme_id(),
            memory.registration.asset.asset_digest(),
            [3; 32],
            vec![1],
        );
        assert!(matches!(result, Err(Error::Execution(_))));
        assert_eq!(tx.world.kagemusha_wallet_ledger.iter().count(), rows);
        result
    });
    assert!(matches!(result, Err(Error::Execution(_))));
}

#[test]
#[ignore = "genuine complete signed native inventory; run optimized"]
fn genuine_verifier_install_is_immutable_exact_retry_and_same_overlay_native_owner() {
    use iroha_core_zk::kagemusha_wallet_artifacts_v1::{
        InstalledVerifierPackV1, engineering_fixture,
    };
    let (pack, installation) = engineering_fixture::signed_inventory();
    let mut memory = Memory::new();
    memory.registration.scheme =
        KagemushaWalletSchemeV1::decode_canonical(&pack.scheme, &installation.scheme_id).unwrap();
    memory.registration.load_authorizer = certificate(
        &memory.registration.scheme,
        KagemushaWalletSignerRoleV1::LoadAuthorization,
        0x34,
    );
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let original = pack.to_canonical_bytes().unwrap();
    InstalledVerifierPackV1::load(&original, installation).unwrap();
    let row_key = super::super::artifacts::key(installation.scheme_id);
    let install = |tx: &mut StateTransaction<'_, '_>, pin, original| {
        wsv::WsvLedger::new(tx, &memory.registration.reserve)?.install_verifier_pack(
            installation.scheme_id,
            memory.registration.asset.asset_digest(),
            pin,
            original,
        )
    };
    let before_rows = state.view().world.kagemusha_wallet_ledger().iter().count();
    transact(&mut state, 2, |tx| {
        install(tx, installation.manifest_digest, original.clone())
    })
    .unwrap();
    let retained = state
        .view()
        .world
        .kagemusha_wallet_ledger()
        .get(&row_key)
        .unwrap()
        .clone();
    assert_eq!(
        state.view().world.kagemusha_wallet_ledger().iter().count(),
        before_rows + 1
    );
    let (alternate, alternate_installation) =
        engineering_fixture::alternate_authority(&pack, installation);
    let alternate_original = alternate.to_canonical_bytes().unwrap();
    InstalledVerifierPackV1::load(&alternate_original, alternate_installation).unwrap();
    assert_ne!(alternate_original, original);
    assert!(matches!(
        transact(&mut state, 4, |tx| install(
            tx,
            alternate_installation.manifest_digest,
            alternate_original
        )),
        Err(Error::Conflict)
    ));
    assert_eq!(
        state.view().world.kagemusha_wallet_ledger().get(&row_key),
        Some(&retained)
    );
    transact(&mut state, 3, |tx| {
        install(tx, installation.manifest_digest, original.clone())
    })
    .unwrap();
    assert_eq!(
        state.view().world.kagemusha_wallet_ledger().get(&row_key),
        Some(&retained)
    );
    assert_eq!(
        state.view().world.kagemusha_wallet_ledger().iter().count(),
        before_rows + 1
    );
    for changed_member in 0..5 {
        let mut changed = pack.clone();
        let member = match changed_member {
            0 => &mut changed.signer_certificate,
            1 => &mut changed.manifest,
            2 => &mut changed.steps[0].artifact.descriptor,
            3 => &mut changed.steps[0].artifact.verifying_key,
            _ => &mut changed.lineage.verifying_key,
        };
        *member.last_mut().unwrap() ^= 1;
        assert!(
            transact(&mut state, 4, |tx| {
                install(
                    tx,
                    installation.manifest_digest,
                    changed.to_canonical_bytes().unwrap(),
                )
            })
            .is_err()
        );
        assert_eq!(
            state.view().world.kagemusha_wallet_ledger().get(&row_key),
            Some(&retained)
        );
    }
    assert!(
        transact(&mut state, 4, |tx| install(
            tx,
            [0x52; 32],
            original.clone()
        ))
        .is_err()
    );
    assert!(
        transact(&mut state, 4, |tx| {
            wsv::WsvLedger::new(tx, &memory.registration.reserve)?.install_verifier_pack(
                installation.scheme_id,
                [0x53; 32],
                installation.manifest_digest,
                original.clone(),
            )
        })
        .is_err()
    );
    assert_eq!(
        state.view().world.kagemusha_wallet_ledger().get(&row_key),
        Some(&retained)
    );
    // The real same-overlay verifier exists, but unrelated fixture credentials/packages
    // cannot obtain a verdict from this engineering-only installation.
    transact(&mut state, 4, |tx| {
        let verifier =
            super::super::artifacts::LedgerVerifier::from_state(tx, installation.scheme_id);
        let activation: KagemushaWalletActivationV1 = fixture("KagemushaWalletActivationV1");
        assert!(
            verifier
                .verify(
                    &memory.registration.scheme,
                    &activation.credential,
                    &activation.bootstrap
                )
                .is_err()
        );
        let value: super::super::artifacts::VerifierInstallation = storage::decode(
            tx.world.kagemusha_wallet_ledger.get(&row_key).unwrap(),
            super::super::artifacts::CAP,
        )?;
        value.require_registration(&memory.registration)?;
        let mut changed_registration = memory.registration.clone();
        changed_registration.asset.asset_incarnation[0] ^= 1;
        assert!(value.require_registration(&changed_registration).is_err());
        Ok(())
    })
    .unwrap();
}

#[test]
fn verifier_install_rechecks_live_asset_permission_and_registered_scope() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    transact(&mut state, 2, |tx| {
        tx.world
            .account_permissions
            .remove(memory.registration.reserve.clone());
        Ok(())
    })
    .unwrap();
    let result: Result<()> = transact(&mut state, 3, |tx| {
        let rows = tx.world.kagemusha_wallet_ledger.iter().count();
        let result = wsv::WsvLedger::new(tx, &memory.registration.reserve)?.install_verifier_pack(
            memory.registration.scheme.scheme_id(),
            memory.registration.asset.asset_digest(),
            [3; 32],
            vec![1],
        );
        assert!(matches!(result, Err(Error::Execution(_))));
        assert_eq!(tx.world.kagemusha_wallet_ledger.iter().count(), rows);
        result
    });
    assert!(matches!(result, Err(Error::Execution(_))));
    assert!(
        transact(&mut state, 3, |tx| {
            wsv::WsvLedger::new(tx, &memory.registration.reserve)?.install_verifier_pack(
                memory.registration.scheme.scheme_id(),
                [7; 32],
                [3; 32],
                vec![1],
            )
        })
        .is_err()
    );
}

#[test]
fn authorized_install_cannot_turn_missing_originals_into_a_verifier() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    for (manifest, original) in [
        ([0; 32], vec![1]),
        ([3; 32], Vec::new()),
        ([3; 32], vec![1]),
    ] {
        assert!(
            transact(&mut state, 2, |tx| {
                let rows = tx.world.kagemusha_wallet_ledger.iter().count();
                let result = wsv::WsvLedger::new(tx, &memory.registration.reserve)?
                    .install_verifier_pack(
                        memory.registration.scheme.scheme_id(),
                        memory.registration.asset.asset_digest(),
                        manifest,
                        original,
                    );
                assert!(result.is_err());
                assert_eq!(tx.world.kagemusha_wallet_ledger.iter().count(), rows);
                assert!(
                    tx.world
                        .kagemusha_wallet_ledger
                        .get(&super::super::artifacts::key(
                            memory.registration.scheme.scheme_id()
                        ),)
                        .is_none()
                );
                result
            })
            .is_err()
        );
    }
}

#[test]
fn package_quota_refuses_before_native_verifier_and_ledger_activation() {
    use iroha_data_model::isi::kagemusha_wallet::{
        KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1,
    };
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let activation: KagemushaWalletActivationV1 = fixture("KagemushaWalletActivationV1");
    let verifier = Verifier::new(true);
    let instruction = KagemushaWalletLedgerV1::new(
        memory.registration.scheme.scheme_id(),
        KagemushaWalletLedgerActionV1::Activate(activation.to_canonical_bytes().unwrap()),
    );
    assert!(
        transact(&mut state, 2, |tx| {
            tx.zk.max_proof_size_bytes = 1;
            let rows = tx.world.kagemusha_wallet_ledger.iter().count();
            let result = crate::smartcontracts::isi::kagemusha_wallet::execute_with_verifier(
                instruction,
                &memory.authority,
                tx,
                &verifier,
            );
            assert!(matches!(result, Err(Error::Execution(_))));
            assert_eq!(verifier.calls.get(), 0);
            assert_eq!(tx.world.kagemusha_wallet_ledger.iter().count(), rows);
            result
        })
        .is_err()
    );
}

#[test]
fn real_reserve_pays_unload_once_and_online_controls_defer_without_consuming_it() {
    use iroha_data_model::{asset::AssetTransferAvailability, isi::SetAssetTransferAvailability};
    let mut memory = Memory::new();
    let claim: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    if let KagemushaWalletUnloadChargeV1::Quoted { beneficiary, .. } = &claim.charge {
        memory.balances.insert(beneficiary.clone(), 0);
    }
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let key = KagemushaWalletPayoutKeyV1::Unload(
        claim.verify(&memory.registration.scheme).unwrap().nullifier,
    );
    let before = balance(&state, &memory, &memory.registration.reserve);
    transact(&mut state, 2, |tx| {
        SetAssetTransferAvailability::new(
            memory.registration.reserve.clone(),
            memory.registration.asset.asset.clone(),
            0,
            AssetTransferAvailability::Enabled,
            AssetTransferAvailability::Disabled,
            None,
        )
        .execute(&memory.authority, tx)
        .map_err(Error::from)
    })
    .unwrap();
    assert!(
        transact(&mut state, 3, |tx| pay_unload(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &Verifier::new(true),
            &claim,
        ))
        .is_err()
    );
    assert_eq!(
        balance(&state, &memory, &memory.registration.reserve),
        before
    );
    transact(&mut state, 3, |tx| {
        assert!(
            wsv::WsvLedger::new(tx, &memory.authority)?
                .payout(&memory.registration.scheme.scheme_id(), key)?
                .is_none()
        );
        SetAssetTransferAvailability::new(
            memory.registration.reserve.clone(),
            memory.registration.asset.asset.clone(),
            1,
            AssetTransferAvailability::Enabled,
            AssetTransferAvailability::Enabled,
            None,
        )
        .execute(&memory.authority, tx)
        .map_err(Error::from)
    })
    .unwrap();
    let verifier = Verifier::new(true);
    let paid = transact(&mut state, 4, |tx| {
        pay_unload(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &verifier,
            &claim,
        )
    })
    .unwrap();
    let expected = before
        .checked_sub(
            &Quantity::from_canonical_numeric(Numeric::new(
                paid.amount,
                memory.registration.asset.scale,
            ))
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        balance(&state, &memory, &memory.registration.reserve),
        expected
    );
    assert_eq!(
        transact(&mut state, 5, |tx| pay_unload(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &verifier,
            &claim,
        ))
        .unwrap(),
        paid
    );
    assert_eq!(
        balance(&state, &memory, &memory.registration.reserve),
        expected
    );
    assert_eq!(
        verifier.calls.get(),
        2,
        "retry re-verifies the retained claim"
    );
}

#[test]
fn real_fee_payout_requires_retained_history_and_never_requires_receive() {
    let mut memory = Memory::new();
    memory.history = true;
    let claim: KagemushaWalletFeeClaimV1 = fixture("KagemushaWalletFeeClaimV1");
    memory.balances.insert(claim.beneficiary.clone(), 0);
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    assert!(matches!(
        transact(&mut state, 2, |tx| pay_fee(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &Verifier::new(true),
            &claim,
        )),
        Err(Error::Unavailable)
    ));
    let history = memory.fee_inputs(&claim).unwrap();
    transact(&mut state, 2, |tx| {
        let mut ledger = wsv::WsvLedger::new(tx, &memory.authority)?;
        ledger.retain_credential(CredentialRecord {
            credential: history.payer,
            certificates: history.certificates.clone(),
        })?;
        ledger.retain_request(history.request.clone())
    })
    .unwrap();
    let paid = transact(&mut state, 3, |tx| {
        pay_fee(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &Verifier::new(true),
            &claim,
        )
    })
    .unwrap();
    assert_eq!(
        balance(&state, &memory, &claim.beneficiary),
        Quantity::from_canonical_numeric(Numeric::new(
            paid.amount,
            memory.registration.asset.scale
        ))
        .unwrap()
    );
    assert_eq!(
        transact(&mut state, 4, |tx| pay_fee(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &Verifier::new(true),
            &claim,
        ))
        .unwrap(),
        paid
    );
    assert_eq!(
        balance(&state, &memory, &claim.beneficiary),
        Quantity::from_canonical_numeric(Numeric::new(
            paid.amount,
            memory.registration.asset.scale
        ))
        .unwrap()
    );
}

#[test]
fn corrupted_reserve_owner_and_mutated_registration_are_rejected() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let mut changed = memory.registration.clone();
    changed.balance_scope =
        AssetBalanceScope::Dataspace(iroha_model_base::topology::DataSpaceId::new(3));
    assert!(
        transact(&mut state, 2, |tx| wsv::WsvLedger::new(
            tx,
            &memory.registration.reserve
        )?
        .register(changed))
        .is_err()
    );
    transact(&mut state, 2, |tx| {
        let reserve = custody::reserve_id(&memory.registration);
        let key = storage::reserve_key(&reserve)?;
        tx.world.kagemusha_wallet_ledger.insert(key, vec![0; 32]);
        assert!(custody::reserve_registration(tx.world(), &reserve).is_err());
        Ok(())
    })
    .unwrap();
    assert!(register(&mut state, &memory, 3).is_err());
}

#[test]
fn snapshot_requires_exact_reserve_indexes_and_reference_counts() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let rows: BTreeMap<_, _> = state
        .view()
        .world
        .kagemusha_wallet_ledger()
        .iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect();
    let check = |rows: &BTreeMap<KagemushaWalletLedgerKeyV1, Vec<u8>>| {
        storage::validate_snapshot(rows.iter(), |key| rows.get(key).map(Vec::as_slice))
    };
    check(&rows).unwrap();
    for key in [
        storage::reserve_key(&custody::reserve_id(&memory.registration)).unwrap(),
        storage::reserve_account_key(&memory.registration.reserve).unwrap(),
        storage::reserve_definition_key(&memory.registration.asset.asset).unwrap(),
        storage::key(
            storage::REGISTRATION,
            memory.registration.scheme.scheme_id(),
            memory.registration.asset.asset_digest(),
        ),
    ] {
        let mut missing = rows.clone();
        missing.remove(&key);
        assert!(
            check(&missing).is_err(),
            "every original binding is required"
        );
    }
    let mut inflated = rows.clone();
    inflated.insert(
        storage::reserve_account_key(&memory.registration.reserve).unwrap(),
        storage::encode(&2_u64).unwrap(),
    );
    assert!(check(&inflated).is_err());
    let mut orphan = rows;
    orphan.insert(
        storage::reserve_account_key(&memory.authority).unwrap(),
        storage::encode(&1_u64).unwrap(),
    );
    assert!(check(&orphan).is_err());
}

#[test]
fn publication_requires_exact_submission_scope_and_retains_historical_signer_after_rotation() {
    use iroha_executor_data_model::permission::asset_definition::CanPublishKagemushaLoadVoucher;
    let mut memory = Memory::new();
    let command = memory.active();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    transact(&mut state, 2, |tx| {
        activate(
            &mut wsv::WsvLedger::new(tx, &memory.authority)?,
            &Verifier::new(true),
            &fixture("KagemushaWalletActivationV1"),
        )
    })
    .unwrap();
    let first = transact(&mut state, 3, |tx| {
        issue_load(&mut wsv::WsvLedger::new(tx, &memory.authority)?, &command)
    })
    .unwrap();
    let signed = voucher(&first, &memory);
    let token = CanPublishKagemushaLoadVoucher {
        asset_definition: memory.registration.asset.asset.clone(),
        scheme: command.scheme,
        authorizer_certificate: signed.body.authorizer_certificate,
    };
    let mut wrong_asset = token.clone();
    wrong_asset.asset_definition =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::try_new("other", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
    for wrong in [
        None,
        Some(wrong_asset),
        Some(CanPublishKagemushaLoadVoucher {
            scheme: [0x51; 32],
            ..token.clone()
        }),
        Some(CanPublishKagemushaLoadVoucher {
            authorizer_certificate: [0x52; 32],
            ..token.clone()
        }),
    ] {
        assert!(
            transact(&mut state, 4, |tx| {
                tx.world.account_permissions.insert(
                    memory.authority.clone(),
                    Permissions::from_iter(wrong.map(Into::into)),
                );
                wsv::WsvLedger::new(tx, &memory.authority)?
                    .publish_voucher(command.request_id, &signed)
            })
            .is_err()
        );
    }
    let next = certificate(
        &memory.registration.scheme,
        KagemushaWalletSignerRoleV1::LoadAuthorization,
        0x35,
    );
    assert!(
        transact(&mut state, 4, |tx| wsv::WsvLedger::new(
            tx,
            &memory.authority
        )?
        .rotate_load_authorizer(
            memory.registration.asset.asset_digest(),
            next
        ))
        .is_err()
    );
    transact(&mut state, 4, |tx| {
        wsv::WsvLedger::new(tx, &memory.registration.reserve)?
            .rotate_load_authorizer(memory.registration.asset.asset_digest(), next)
    })
    .unwrap();
    let balances = (
        balance(&state, &memory, &memory.authority),
        balance(&state, &memory, &memory.registration.reserve),
    );
    transact(&mut state, 5, |tx| {
        tx.world.account_permissions.insert(
            memory.authority.clone(),
            Permissions::from_iter([token.into()]),
        );
        let mut ledger = wsv::WsvLedger::new(tx, &memory.authority)?;
        ledger.publish_voucher(command.request_id, &signed)?;
        ledger.publish_voucher(command.request_id, &signed)?;
        assert!(matches!(
            ledger.publish_voucher(command.request_id, &alternative_voucher(&first, &memory)),
            Err(Error::Conflict)
        ));
        assert_eq!(
            ledger
                .issuance(&command.scheme, &command.wallet, &command.request_id)?
                .unwrap()
                .voucher,
            Some(signed.to_canonical_bytes().unwrap())
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(
        (
            balance(&state, &memory, &memory.authority),
            balance(&state, &memory, &memory.registration.reserve)
        ),
        balances
    );
    let mut later = command.clone();
    later.request_id = [0x55; 32];
    let issued = transact(&mut state, 6, |tx| {
        issue_load(&mut wsv::WsvLedger::new(tx, &memory.authority)?, &later)
    })
    .unwrap();
    assert_eq!(
        issued.body.authorizer_certificate,
        next.certificate_digest()
    );
    assert_ne!(
        issued.body.authorizer_certificate,
        first.body.authorizer_certificate
    );
    let view = state.view();
    let rows = view.world.kagemusha_wallet_ledger();
    storage::validate_snapshot(rows.iter(), |key| rows.get(key).map(Vec::as_slice)).unwrap();
}

#[test]
fn certified_chain_issues_publishes_and_retrieves_original_voucher_after_growth() {
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_data_model::{
        isi::kagemusha_wallet::{KagemushaWalletLedgerActionV1 as Action, KagemushaWalletLedgerV1},
        sumeragi::finality::NativeFinalityLimits,
    };
    use iroha_executor_data_model::permission::asset_definition::CanPublishKagemushaLoadVoucher;
    let mut memory = Memory::new();
    let payer_key =
        iroha_crypto::KeyPair::from_seed(vec![0x66; 32], iroha_crypto::Algorithm::Ed25519);
    memory.balances.remove(&memory.authority);
    memory.authority = AccountId::new(payer_key.public_key().clone());
    memory.balances.insert(memory.authority.clone(), 1000);
    let world = world_state(&memory, true).world;
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(world, 1_000)).unwrap();
    memory.registration.scheme.network_id = *chain.network_id().as_bytes();
    memory.registration.load_authorizer = certificate(
        &memory.registration.scheme,
        KagemushaWalletSignerRoleV1::LoadAuthorization,
        0x34,
    );
    let scheme = memory.registration.scheme.scheme_id();
    let wallet = [0x91; 32];
    let request = [0x92; 32];
    // Only enrollment/Bootstrap admission is a fixture. Load issuance, publication, exact
    // transaction membership and CommitQC verification below use the actual chain owners.
    chain.setup_world_at(2_000, |tx| {
        tx.current_network_entrypoint_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"registration fixture",
        )));
        wsv::WsvLedger::new(tx, &memory.registration.reserve)
            .unwrap()
            .register(memory.registration.clone())
            .unwrap();
        tx.world.kagemusha_wallet_ledger.insert(
            storage::key(storage::WALLET, scheme, wallet),
            storage::encode(&WalletRecord {
                asset: memory.registration.asset.asset_digest(),
                phase: Phase::Active,
                activation: [0x93; 32],
                next_load: 0,
            })
            .unwrap(),
        );
        tx.world.account_permissions.insert(
            memory.authority.clone(),
            Permissions::from_iter([
                CanPublishKagemushaLoadVoucher {
                    asset_definition: memory.registration.asset.asset.clone(),
                    scheme,
                    authorizer_certificate: memory
                        .registration
                        .load_authorizer
                        .certificate_digest(),
                }
                .into(),
                CanManageKagemushaWallet {
                    asset_definition: memory.registration.asset.asset.clone(),
                }
                .into(),
            ]),
        );
    });
    let issue = KagemushaWalletLedgerV1::new(
        scheme,
        Action::IssueLoad {
            wallet,
            request_id: request,
            amount: 100,
            charge: None,
        },
    );
    let issue_tx = chain.sign(&payer_key, [issue.into()], 1_999);
    assert_eq!(chain.commit_at(2_000, vec![issue_tx]), vec![true]);
    let replacement = certificate(
        &memory.registration.scheme,
        KagemushaWalletSignerRoleV1::LoadAuthorization,
        0x35,
    );
    let rotate = KagemushaWalletLedgerV1::new(
        scheme,
        Action::RotateLoadAuthorizer {
            asset: memory.registration.asset.asset_digest(),
            certificate: replacement.to_canonical_bytes().unwrap(),
        },
    );
    let rotate_tx = chain.sign(&payer_key, [rotate.into()], 2_999);
    assert_eq!(chain.commit_at(3_000, vec![rotate_tx]), vec![true]);
    let limits = NativeFinalityLimits {
        block_bytes: 2_000_000,
        journal_bytes: 8_000_000,
        block_count: 8,
        allocated_bytes: 16_000_000,
    };
    let (original, publish) = {
        let view = chain.state().view();
        let source = FinalizedLedger::new(&view, limits).unwrap();
        let original = source
            .issuance_for(&memory.authority, &scheme, &wallet, &request, 2_000_000)
            .unwrap();
        assert_eq!(original.body.block_height, 2);
        assert!(original.voucher.is_none());
        assert!(
            source
                .issuance_for(
                    &memory.registration.reserve,
                    &scheme,
                    &wallet,
                    &request,
                    2_000_000
                )
                .is_err()
        );
        assert!(
            source
                .issuance_for(&memory.authority, &scheme, &wallet, &request, 1)
                .is_err()
        );
        let keyring = LoadAuthorizerKeyringV1 {
            version: 1,
            keys: vec![LoadAuthorizerKeyV1 {
                scheme: memory.registration.scheme,
                certificate: memory.registration.load_authorizer,
                secret: [0x34; 32],
            }],
        };
        let encoded = storage::encode(&keyring).unwrap();
        let mut before_outage = PublicationWorker::from_canonical_keyring(&encoded).unwrap();
        let prepared = before_outage
            .prepare_page(&source, 1)
            .unwrap()
            .pop()
            .unwrap()
            .into_publication_instruction()
            .unwrap();
        // Simulate a dropped/unknown submission result and restart with no local journal.
        // The original finalized issuance recovers exactly the same voucher bytes.
        drop(before_outage);
        let mut restarted = PublicationWorker::from_canonical_keyring(&encoded).unwrap();
        let retry = restarted
            .prepare_page(&source, 1)
            .unwrap()
            .pop()
            .unwrap()
            .into_publication_instruction()
            .unwrap();
        assert_eq!(
            storage::encode(&prepared).unwrap(),
            storage::encode(&retry).unwrap()
        );
        let wrong = LoadAuthorizerKeyringV1 {
            version: 1,
            keys: vec![LoadAuthorizerKeyV1 {
                scheme: memory.registration.scheme,
                certificate: certificate(
                    &memory.registration.scheme,
                    KagemushaWalletSignerRoleV1::LoadAuthorization,
                    0x35,
                ),
                secret: [0x35; 32],
            }],
        };
        let mut wrong_worker =
            PublicationWorker::from_canonical_keyring(&storage::encode(&wrong).unwrap()).unwrap();
        assert!(wrong_worker.prepare_page(&source, 1).unwrap().is_empty());
        assert!(restarted.prepare_page(&source, 0).is_err());
        assert!(
            restarted
                .prepare_page(&source, MAX_PENDING_PAGE + 1)
                .is_err()
        );
        (original, prepared)
    };
    let publish_tx = chain.sign(&payer_key, [publish.into()], 3_999);
    assert_eq!(chain.commit_at(4_000, vec![publish_tx]), vec![true]);
    chain.commit_at(5_000, Vec::new());
    let view = chain.state().view();
    let source = FinalizedLedger::new(&view, limits).unwrap();
    let retained = source
        .issuance_for(&memory.authority, &scheme, &wallet, &request, 2_000_000)
        .unwrap();
    assert_eq!(retained.body, original.body);
    assert_eq!(retained.command, original.command);
    let signed =
        KagemushaWalletLoadVoucherV1::decode_canonical(retained.voucher.as_ref().unwrap(), &scheme)
            .unwrap();
    signed
        .verify(
            &memory.registration.scheme,
            &memory.registration.load_authorizer,
        )
        .unwrap();
    assert_eq!(signed.body, original.body);
    assert!(
        source
            .pending_publications(
                scheme,
                memory.registration.load_authorizer.certificate_digest(),
                None,
                1
            )
            .unwrap()
            .is_empty()
    );
    let rows = view.world.kagemusha_wallet_ledger();
    storage::validate_snapshot(rows.iter(), |key| rows.get(key).map(Vec::as_slice)).unwrap();
}
