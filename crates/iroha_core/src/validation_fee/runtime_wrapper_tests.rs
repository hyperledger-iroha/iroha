//! Execute the SoraSwap wrapper through the real host and production DLMM bin math.
use super::*;
use crate::{
    executor::ContractEntrypointAuthorizationSnapshot, smartcontracts::ivm::host::CoreHostImpl,
};
use iroha_data_model::{IntoKeyValue, smart_contract::ContractAddress};
use iroha_model_base::{name::Name, topology::DataSpaceId};

#[test]
fn compiled_fee_wrapper_preserves_nested_pool_authorities_and_actual_output() {
    for production_pool in [false, true] {
        with_validation_fee_payout_state_at_height(10, |stx, deployer, _, _| {
            let derive = |nonce| {
                ContractAddress::derive(&stx.network_id, deployer, nonce, DataSpaceId::UNIVERSAL)
                    .unwrap()
            };
            let pool = derive(200);
            let wrapper = derive(201);
            let reward_pool = account(8);
            let sbd = fee_asset();
            let xor = xor_asset();
            let expected_output: Quantity = if production_pool {
                "9.960039960039960039".parse().unwrap()
            } else {
                Quantity::from(20u64)
            };
            let minimum_output = if production_pool { "9.9" } else { "19.8" };
            if production_pool {
                stx.world.asset_definitions.insert(
                    xor.clone(),
                    iroha_data_model::asset::AssetDefinition::new(
                        xor.clone(),
                        "xor",
                        NumericSpec::fractional(18),
                        iroha_data_model::asset::AssetBalancePolicy::Global,
                        None,
                    )
                    .build(deployer),
                );
            }
            // The first case isolates the effect plan with a fixed liquid quote.
            // The second uses the entire current production DLMM unchanged. Real nested
            // frames, argument decoding, output validation and transfers run unchanged.
            let pool_source = r#"
seiyaku FullFillPool {
  error enum PoolError { Minimum = 1, }
  kotoage fn swap_exact_in_quote_public(quantity amount_in, quantity min_out) -> quantity authorize("CanInvokeContractEntrypoint") {
    let quantity output = 20;
    require(output >= min_out, PoolError::Minimum);
    ledger::asset::transfer(source: context::authority(), destination: context::seiyaku_subject(), asset_definition: AssetDefinitionId::parse("@@SBD@@"), amount: amount_in, dataspace: DataSpaceId::parse("0"));
    ledger::asset::transfer(source: context::seiyaku_subject(), destination: context::authority(), asset_definition: AssetDefinitionId::parse("@@XOR@@"), amount: output, dataspace: DataSpaceId::parse("0"));
    return output;
  }
}
"#.replace("@@SBD@@", &sbd.to_string()).replace("@@XOR@@", &xor.to_string());
            let pool_source = if production_pool {
                include_str!("fixtures/dlmm_pool.ko").to_owned()
            } else {
                pool_source
            };
            // Exact production SoraSwap template; only its reviewed custody inputs
            // and the two asset identities are rendered for this isolated ledger.
            let wrapper_source = include_str!("fixtures/autonomous_payout.ko.template")
                .replace(
                    "@@PAYOUT_VAULT_ACCOUNT_ID@@",
                    &wrapper.subject_id().to_string(),
                )
                .replace("@@POOL_VAULT_ACCOUNT_ID@@", &pool.subject_id().to_string())
                .replace("@@REWARD_POOL_ACCOUNT_ID@@", &reward_pool.to_string())
                .replace("@@POOL_CONTRACT_ADDRESS@@", &pool.to_string())
                .replace("7ZepsJTHCVLKsrFFNZGSRGZgvBhv", &sbd.to_string())
                .replace("6TEAJqbb8oEPmLncoNiMRbLEK6tw", &xor.to_string());
            let install = |stx: &mut StateTransaction<'_, '_>,
                           address: &ContractAddress,
                           source: &str| {
                let (code, _) = kotodama_lang::compiler::Compiler::new()
                    .compile_source_with_manifest(source)
                    .expect("compile real contract frames");
                let verified = ivm::verify_contract_artifact(&code).unwrap();
                let hash = crate::smartcontracts::code::register_code_bytes(
                    deployer,
                    DataSpaceId::UNIVERSAL,
                    code.clone(),
                    stx,
                )
                .unwrap();
                crate::smartcontracts::code::register_manifest(
                    deployer,
                    DataSpaceId::UNIVERSAL,
                    verified.manifest.signed(&key_pair(55)),
                    stx,
                )
                .unwrap();
                stx.world
                    .bind_inactive_contract_subject_for_testing(address.clone(), deployer.clone());
                crate::smartcontracts::code::activate_instance(
                    deployer,
                    address.clone(),
                    1,
                    hash,
                    stx,
                )
                .unwrap();
                crate::smartcontracts::code::set_pending_contract_lifecycle(stx, address, None);
                (code, hash)
            };
            let (pool_code, pool_hash) = install(stx, &pool, &pool_source);
            let (wrapper_code, wrapper_hash) = install(stx, &wrapper, &wrapper_source);
            for address in [&pool, &wrapper] {
                stx.world.add_account_permission(&wrapper.subject_id(), iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                contract: address.clone(),
                entrypoint: if address == &pool { "swap_exact_in_quote_public" } else { "autonomous_validation_fee_tick" }.to_owned(),
            }.into());
            }
            let treasury_sbd = AssetId::new(sbd.clone(), wrapper.subject_id());
            stx.world.add_account_permission(
                &pool.subject_id(),
                iroha_executor_data_model::permission::asset::CanTransferAsset {
                    asset: treasury_sbd.clone(),
                }
                .into(),
            );
            let pool_funding = if production_pool {
                vec![
                    (AssetId::new(xor.clone(), deployer.clone()), 1000u64),
                    (AssetId::new(sbd.clone(), deployer.clone()), 1000u64),
                ]
            } else {
                vec![(AssetId::new(xor.clone(), pool.subject_id()), 20u64)]
            };
            for (id, amount) in std::iter::once((treasury_sbd.clone(), 10u64)).chain(pool_funding) {
                let (id, value) = Asset::new(id, Quantity::from(amount)).into_key_value();
                stx.world.assets.insert(id, value);
            }
            if production_pool {
                stx.world.add_account_permission(deployer,
                iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                    contract: pool.clone(), entrypoint: "seed_bin".to_owned(),
                }.into());
                for asset in [&xor, &sbd] {
                    stx.world.add_account_permission(
                        &pool.subject_id(),
                        iroha_executor_data_model::permission::asset::CanTransferAsset {
                            asset: AssetId::new(asset.clone(), deployer.clone()),
                        }
                        .into(),
                    );
                }
                super::runtime_dlmm_tests::execute_pool(
                stx, &pool, &pool_code, deployer, "hajimari", Json::from(norito::json!({
                    "base_asset": (xor.to_string()), "quote_asset": (sbd.to_string()),
                    "vault_account": (pool.subject_id().to_string()), "fee_pips": "3000",
                    "bin_step": "1", "active_bin": "1000", "impact_cap_bps": "10000",
                    "min_reserve_base": "0", "min_reserve_quote": "0", "max_bins_per_swap": "32",
                    "bin_liquidity_cap": "0",
                })),
            ).expect("initialize actual nested production pool").apply_to_transaction(stx, deployer).unwrap();
                super::runtime_dlmm_tests::execute_pool(
                    stx,
                    &pool,
                    &pool_code,
                    deployer,
                    "seed_bin",
                    Json::from(norito::json!({
                        "position_id": "wrapper-liquidity", "bin_id": "1000",
                        "base_amount": "1000", "quote_amount": "1000",
                    })),
                )
                .expect("fund actual nested production pool")
                .apply_to_transaction(stx, deployer)
                .unwrap();
            }
            let treasury_xor = AssetId::new(xor.clone(), wrapper.subject_id());
            assert!(
                stx.world.assets.get(&treasury_xor).is_none(),
                "zero initial XOR uses the canonical absent-asset representation"
            );
            let scope = hex::encode(Hash::new(wrapper.to_string().as_bytes()).as_ref());
            for (index, amount) in [(0i128, "10"), (1, minimum_output)] {
                let base: Name = "ValidationFeeConversion".parse().unwrap();
                let encoded = ivm::numeric_tlv::encode_int(
                    &iroha_primitives::bigint::BigInt::from_i128(index),
                )
                .unwrap();
                let relative = ivm::host::canonical_state_map_path(&base, &encoded).unwrap();
                stx.world.smart_contract_state.insert(
                    format!("sc/{scope}/{relative}").parse().unwrap(),
                    encode_conversion_quantity_state_value(&amount.parse().unwrap()).unwrap(),
                );
            }
            let context = crate::executor::ContractRuntimeExecutionContext {
                contract_address: wrapper.clone(),
                contract_subject: wrapper.subject_id(),
                contract_alias: None,
                entrypoint: "autonomous_validation_fee_tick".to_owned(),
            };
            let prepared =
                ivm::prepare_contract(std::sync::Arc::<[u8]>::from(wrapper_code.clone())).unwrap();
            let mut host = CoreHostImpl::new(wrapper.subject_id());
            host.set_query_state(stx);
            host.set_durable_state_snapshot_from_world(&stx.world);
            host.set_contract_runtime_context(Some(context.clone()));
            host.set_contract_entrypoint_authorization(Some(
                ContractEntrypointAuthorizationSnapshot::new(
                    wrapper.subject_id(),
                    context.entrypoint.clone(),
                    Some("CanInvokeContractEntrypoint".to_owned()),
                    &crate::smartcontracts::code::BoundContractIdentity {
                        contract_address: wrapper.clone(),
                        contract_alias: None,
                        contract_alias_binding: None,
                        code_hash: wrapper_hash,
                    },
                ),
            ));
            let mut vm = ivm::IVM::new(50_000_000);
            vm.load_program(&wrapper_code).unwrap();
            vm.set_program_counter(prepared.entrypoint_pc(&context.entrypoint).unwrap())
                .unwrap();
            vm.run_with_host(&mut host)
                .expect("real wrapper and nested pool execute");
            let artifacts = host.into_execution_artifacts(Some(context)).unwrap();
            let ordered = artifacts.queued_instructions_with_authority();
            assert_eq!(
                ordered
                    .iter()
                    .map(|(authority, _)| authority.clone())
                    .collect::<Vec<_>>(),
                vec![pool.subject_id(), pool.subject_id(), wrapper.subject_id()]
            );
            let mut binding = treasury_payout_binding(wrapper.clone(), &wrapper_code);
            binding.pool_contract_address = pool.clone();
            binding.pool_vault_account_id = pool.subject_id();
            binding.pool_code_hash = pool_hash.into();
            binding.reward_pool_account_id = reward_pool.clone();
            let terms = ValidationFeePayoutTerms {
                debit_ds: "10".parse().unwrap(),
                min_xor_out: minimum_output.parse().unwrap(),
                xor_scale: if production_pool { 18 } else { 2 },
            };
            assert!(
                validate_treasury_payout_effect_plan(
                    &artifacts.queued_instructions_by_authority(),
                    &ordered,
                    &binding,
                    &terms
                )
                .unwrap()
            );
            artifacts
                .apply_to_transaction(stx, &wrapper.subject_id())
                .expect("exact typed frame authorizations apply");
            let paid = AssetId::new(xor, reward_pool);
            assert_eq!(
                stx.world.assets.get(&paid).unwrap().as_ref(),
                &expected_output
            );
            assert!(stx.world.assets.get(&treasury_sbd).is_none());
            assert!(
                stx.world.assets.get(&treasury_xor).is_none(),
                "all actual pool output is atomically forwarded to reward custody"
            );
        });
    }
}
