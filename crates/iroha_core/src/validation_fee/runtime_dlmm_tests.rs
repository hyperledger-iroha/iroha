//! Production DLMM math, authenticated precision queries and exact ledger transfers.
use super::*;
use crate::{
    executor::ContractEntrypointAuthorizationSnapshot,
    smartcontracts::ivm::host::{CoreHostImpl, HostExecutionArtifacts},
};
use iroha_data_model::{IntoKeyValue, smart_contract::ContractAddress};
use iroha_model_base::topology::DataSpaceId;

pub(super) fn execute_pool(
    stx: &mut StateTransaction<'_, '_>,
    pool: &ContractAddress,
    code: &[u8],
    authority: &AccountId,
    entrypoint: &str,
    arguments: Json,
) -> Result<HostExecutionArtifacts, ivm::VMError> {
    let parsed = ivm::ProgramMetadata::parse(code).unwrap();
    let descriptor = parsed
        .contract_interface
        .as_ref()
        .unwrap()
        .entrypoints
        .iter()
        .find(|entry| entry.name == entrypoint)
        .unwrap();
    let schema = descriptor.argument_schema.as_ref().unwrap();
    let encoded = ivm::encode_argument_record_from_json(schema, &arguments).unwrap();
    let arguments = ivm::prepare_argument_record_with_gas_limit(
        schema,
        std::sync::Arc::from(encoded),
        100_000_000,
    )
    .unwrap();
    let context = crate::executor::ContractRuntimeExecutionContext {
        contract_address: pool.clone(),
        contract_subject: pool.subject_id(),
        contract_alias: None,
        entrypoint: entrypoint.to_owned(),
    };
    let mut host = CoreHostImpl::new(authority.clone());
    host.set_query_state(stx);
    host.set_durable_state_snapshot_from_world(&stx.world);
    host.set_entrypoint_argument_record(Some(arguments.clone()));
    host.set_contract_runtime_context(Some(context.clone()));
    host.set_contract_entrypoint_authorization(Some(ContractEntrypointAuthorizationSnapshot::new(
        authority.clone(),
        entrypoint.to_owned(),
        descriptor.permission.clone(),
        &crate::smartcontracts::code::BoundContractIdentity {
            contract_address: pool.clone(),
            contract_alias: None,
            contract_alias_binding: None,
            code_hash: ivm::contract_code_hash(code),
        },
    )));
    let mut vm = ivm::IVM::new(100_000_000);
    vm.load_program(code).unwrap();
    vm.set_program_counter(parsed.prefix_len() as u64 + descriptor.entry_pc)
        .unwrap();
    arguments.precharge_vm(&mut vm).unwrap();
    vm.run_with_host(&mut host)?;
    Ok(host.into_execution_artifacts(Some(context)).unwrap())
}

fn balance(
    stx: &StateTransaction<'_, '_>,
    asset: &AssetDefinitionId,
    owner: &AccountId,
) -> Quantity {
    stx.world
        .assets
        .get(&AssetId::new(asset.clone(), owner.clone()))
        .map_or_else(Quantity::zero, |entry| entry.as_ref().clone())
}

fn pool_bin_quantity(
    stx: &StateTransaction<'_, '_>,
    pool: &ContractAddress,
    field: &str,
    bin: i128,
) -> Quantity {
    let scope = hex::encode(Hash::new(pool.to_string().as_bytes()).as_ref());
    let key =
        ivm::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(bin)).unwrap();
    let relative = ivm::host::canonical_state_map_path(&field.parse().unwrap(), &key).unwrap();
    let key: iroha_model_base::state_path::StatePath =
        format!("sc/{scope}/{relative}").parse().unwrap();
    let bytes = stx.world.smart_contract_state.get(&key).unwrap();
    let record: StateValueRecordV1 = norito::decode_from_bytes(bytes).unwrap();
    let StateValueAtomV1::Pointer(pointer) = &record.atoms[0] else {
        panic!("quantity atom")
    };
    let tlv = ivm::pointer_abi::validate_tlv_bytes(pointer).unwrap();
    iroha_primitives::numeric_abi::QuantityValueV1::decode_frame(tlv.payload)
        .unwrap()
        .into_quantity()
}

#[test]
fn production_dlmm_rounds_at_native_asset_precision_and_conserves_both_directions() {
    // Exact current production source. The cross-repository source guard checks
    // this copy byte-for-byte before the SoraSwap check/build/test workflows.
    let source = include_str!("fixtures/dlmm_pool.ko");
    let (code, _) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(source)
        .expect("compile complete production DLMM, including actual bin math");
    for (base_scale, bin, quote_input, input_amount, liquidity, expected_input, expected) in [
        (2u32, 1000i128, true, "10", "1000", "10", "9.96"),
        (2, 1000, false, "10", "1000", "10", "9.97"),
        (2, -1000, true, "10", "1000", "10", "9.97"),
        (2, -1000, false, "10", "1000", "10", "9.96"),
        (2, 1000, true, "100", "10.02", "10.07", "10.02"),
        (2, 1000, false, "100", "10.02", "10.05", "10.02"),
        // The launch pair has 18-place XOR and two-place SBD. These cases
        // exercise recurring output and input rounding at their distinct units.
        (18, 1000, true, "10", "1000", "10", "9.960039960039960039"),
        (18, 1000, false, "10", "1000", "10", "9.97"),
        (18, -1000, true, "10", "1000", "10", "9.97997"),
        (18, -1000, false, "10", "1000", "10", "9.96"),
        (18, 1000, true, "100", "10.02", "10.07", "10.02"),
        (
            18,
            1000,
            false,
            "100",
            "10.02",
            "10.040110341013049138",
            "10.02",
        ),
    ] {
        with_validation_fee_payout_state_at_height(10, |stx, deployer, _, _| {
            let pool =
                ContractAddress::derive(&stx.network_id, deployer, 220, DataSpaceId::UNIVERSAL)
                    .unwrap();
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
                .bind_inactive_contract_subject_for_testing(pool.clone(), deployer.clone());
            crate::smartcontracts::code::activate_instance(deployer, pool.clone(), 1, hash, stx)
                .unwrap();
            crate::smartcontracts::code::set_pending_contract_lifecycle(stx, &pool, None);
            let base = xor_asset();
            let quote = fee_asset();
            let definition = iroha_data_model::asset::AssetDefinition::new(
                base.clone(),
                "xor",
                NumericSpec::fractional(base_scale),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(deployer);
            stx.world.asset_definitions.insert(base.clone(), definition);
            let trader = account(2);
            for asset in [&base, &quote] {
                for (owner, amount) in [(deployer.clone(), 2000u64), (trader.clone(), 200u64)] {
                    let id = AssetId::new(asset.clone(), owner);
                    let (key, value) =
                        Asset::new(id.clone(), Quantity::from(amount)).into_key_value();
                    stx.world.assets.insert(key, value);
                    stx.world.add_account_permission(
                        &pool.subject_id(),
                        iroha_executor_data_model::permission::asset::CanTransferAsset {
                            asset: id,
                        }
                        .into(),
                    );
                }
            }
            for (who, entrypoint) in [
                (deployer, "seed_bin"),
                (&trader, "swap_exact_in_quote_public"),
            ] {
                stx.world.add_account_permission(who,
                    iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint {
                        contract: pool.clone(), entrypoint: entrypoint.into(),
                    }.into());
            }
            stx.world.add_account_permission(
                &trader,
                iroha_data_model::permission::Permission::new("AssetOps".into(), Json::new(())),
            );
            let initialize = Json::from(norito::json!({
                "base_asset": (base.to_string()), "quote_asset": (quote.to_string()),
                "vault_account": (pool.subject_id().to_string()), "fee_pips": "3000",
                "bin_step": "1", "active_bin": (bin.to_string()), "impact_cap_bps": "10000",
                "min_reserve_base": "0", "min_reserve_quote": "0", "max_bins_per_swap": "32",
                "bin_liquidity_cap": "0",
            }));
            execute_pool(stx, &pool, &code, deployer, "hajimari", initialize)
                .expect("production initialization")
                .apply_to_transaction(stx, deployer)
                .unwrap();
            execute_pool(
                stx,
                &pool,
                &code,
                deployer,
                "seed_bin",
                Json::from(norito::json!({
                    "position_id": "precision-liquidity", "bin_id": (bin.to_string()),
                    "base_amount": liquidity, "quote_amount": liquidity,
                })),
            )
            .expect("production liquidity seeding")
            .apply_to_transaction(stx, deployer)
            .unwrap();
            let (input, output, field, output_scale) = if quote_input {
                (&quote, &base, "BinReserveBase", base_scale)
            } else {
                (&base, &quote, "BinReserveQuote", 2)
            };
            let input_before = balance(stx, input, &trader);
            let output_before = balance(stx, output, &trader);
            let vault_input_before = balance(stx, input, &pool.subject_id());
            let vault_output_before = balance(stx, output, &pool.subject_id());
            let bin_before = pool_bin_quantity(stx, &pool, field, bin);
            if base_scale == 18 && bin == 1000 && quote_input {
                // Failure cannot leak contract reserve writes or queued input
                // transfers into the transaction, including a full-fill call
                // against the same partial liquidity used below.
                for (bad_input, bad_minimum) in [("10.001", "0"), (input_amount, "1000")] {
                    assert!(
                        execute_pool(
                            stx,
                            &pool,
                            &code,
                            &trader,
                            "swap_exact_in_quote_public",
                            Json::from(norito::json!({
                                "amount_in": bad_input,
                                "min_out": bad_minimum,
                            })),
                        )
                        .is_err(),
                        "unrepresentable input, unmet minimum, or partial full-fill must reject"
                    );
                    assert_eq!(balance(stx, input, &trader), input_before);
                    assert_eq!(balance(stx, output, &trader), output_before);
                    assert_eq!(balance(stx, input, &pool.subject_id()), vault_input_before);
                    assert_eq!(
                        balance(stx, output, &pool.subject_id()),
                        vault_output_before
                    );
                    assert_eq!(pool_bin_quantity(stx, &pool, field, bin), bin_before);
                }
            }
            let entrypoint = if quote_input {
                if input_amount == "100" {
                    "swap_exact_in_quote"
                } else {
                    "swap_exact_in_quote_public"
                }
            } else {
                "swap_exact_in_base"
            };
            let artifacts = execute_pool(
                stx,
                &pool,
                &code,
                &trader,
                entrypoint,
                Json::from(norito::json!({"amount_in": input_amount, "min_out": expected})),
            )
            .expect("full real pool execution at a non-integral bin price");
            let instructions = artifacts.queued_instructions_with_authority();
            assert_eq!(instructions.len(), 2);
            assert!(
                instructions
                    .iter()
                    .all(|(owner, _)| owner == &pool.subject_id())
            );
            let expected: Quantity = expected.parse().unwrap();
            let expected_input: Quantity = expected_input.parse().unwrap();
            let output_leg = direct_conversion_transfer(&instructions[1].1).unwrap();
            assert_eq!(&output_leg.object, &expected);
            assert_eq!(
                crate::validation_fee_rewards::minor_units(&output_leg.object, output_scale)
                    .unwrap(),
                crate::validation_fee_rewards::minor_units(&expected, output_scale).unwrap()
            );
            artifacts
                .apply_to_transaction(stx, &trader)
                .expect("native asset scale checks accept actual output");
            assert_eq!(
                balance(stx, input, &trader),
                input_before.checked_sub(&expected_input).unwrap()
            );
            assert_eq!(
                balance(stx, output, &trader),
                output_before.checked_add(&expected).unwrap()
            );
            assert_eq!(
                balance(stx, input, &pool.subject_id()),
                vault_input_before.checked_add(&expected_input).unwrap()
            );
            assert_eq!(
                balance(stx, output, &pool.subject_id()),
                vault_output_before.checked_sub(&expected).unwrap()
            );
            assert_eq!(
                pool_bin_quantity(stx, &pool, field, bin),
                bin_before.checked_sub(&expected).unwrap()
            );
            // Both the bin reserve and real custody retain the theoretical
            // fraction; neither state accounts for an untransferable amount.
            assert_eq!(
                pool_bin_quantity(stx, &pool, field, bin),
                balance(stx, output, &pool.subject_id())
            );
        });
    }
}
