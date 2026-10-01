//! Signed private-root fee currency and schedule; the parent currency is never a fallback.

use super::*;
use iroha_data_model::block::consensus::{PrivateRootFeePolicy, SumeragiRootScope};

fn invalid(reason: impl Into<String>) -> NexusFeeAdmissionError {
    NexusFeeAdmissionError::ConfigInvalid(reason.into())
}

/// Read the immutable root's complete fee policy and validate its local monetary namespace.
pub(crate) fn policy(
    world: &impl WorldReadOnly,
) -> Result<Option<(DataSpaceId, PrivateRootFeePolicy)>, NexusFeeAdmissionError> {
    let scope = crate::sumeragi::lanes::routing::committed_root_scope(world)
        .ok_or_else(|| invalid("fee admission requires immutable root scope"))?;
    let SumeragiRootScope::Dataspace { dataspace_id, .. } = scope else {
        return Ok(None);
    };
    let policy = PrivateRootFeePolicy::from_parameters(world.parameters())
        .map_err(|error| invalid(format!("invalid signed private fee policy: {error}")))?
        .ok_or_else(|| invalid("signed private root fee policy is absent"))?;
    let definition = world
        .asset_definition(&policy.asset_definition_id)
        .map_err(|_| invalid("private fee asset is not registered"))?;
    let domain = definition
        .owning_domain
        .as_ref()
        .ok_or_else(|| invalid("private fee asset requires an owning domain"))?;
    if definition.balance_scope_policy() != AssetBalancePolicy::DataspaceRestricted
        || world
            .dataspace_catalog()
            .by_alias(domain.dataspace().as_ref())
            .map(|entry| entry.id)
            != Some(dataspace_id)
        || world.domain(domain).is_err()
    {
        return Err(invalid(
            "private fee asset must belong to the exact signed root dataspace",
        ));
    }
    Ok(Some((dataspace_id, policy)))
}

pub(super) fn effective(
    world: &impl WorldReadOnly,
    configured: &NexusFees,
) -> Result<NexusFees, NexusFeeAdmissionError> {
    let mut fees = configured.clone();
    if let Some((_, policy)) = policy(world)? {
        fees.fee_asset_id = policy.asset_definition_id.canonical_address();
        fees.base_fee = policy.base_fee;
        fees.per_byte_fee = policy.per_byte_fee;
        fees.per_instruction_fee = policy.per_instruction_fee;
        fees.per_gas_unit_fee = policy.per_gas_unit_fee;
        fees.successful_claim_fee_exempt_authorities.clear();
    }
    Ok(fees)
}

pub(super) fn currency(
    world: &impl WorldReadOnly,
    fees: &NexusFees,
    observation_time_ms: u64,
) -> Result<AssetDefinitionId, NexusFeeAdmissionError> {
    if let Some((_, policy)) = policy(world)? {
        return Ok(policy.asset_definition_id);
    }
    crate::block::resolve_network_xor_asset_definition(
        world,
        &fees.fee_asset_id,
        observation_time_ms,
    )
    .ok_or_else(|| invalid("invalid Nexus fee asset; expected the committed global XOR identity"))
}

pub(super) fn private_scope(
    world: &impl WorldReadOnly,
    route: Option<DataSpaceId>,
) -> Result<Option<DataSpaceId>, NexusFeeAdmissionError> {
    let Some((dataspace, _)) = policy(world)? else {
        return Ok(None);
    };
    if route != Some(dataspace) {
        return Err(invalid(
            "private fee debit requires the exact captured root dataspace",
        ));
    }
    Ok(Some(dataspace))
}

pub(super) fn permits_public_exemption(world: &impl WorldReadOnly) -> bool {
    crate::sumeragi::lanes::routing::committed_root_scope(world) == Some(SumeragiRootScope::Global)
}

/// Select exact protocol vault custody. A private root has its own World and sole committed
/// fee currency, so its program/vault keys cannot address a parent World or another root.
/// Restricted currencies on a global root still require a separate scoped-vault protocol.
pub(crate) fn sponsor_asset_scope(
    world: &impl WorldReadOnly,
    asset: &AssetDefinitionId,
    route: Option<DataSpaceId>,
) -> Result<AssetBalanceScope, NexusFeeAdmissionError> {
    if matches!(
        crate::sumeragi::lanes::routing::committed_root_scope(world),
        Some(SumeragiRootScope::Dataspace { .. })
    ) {
        let (dataspace, policy) =
            policy(world)?.ok_or_else(|| invalid("private fee policy is absent"))?;
        if route != Some(dataspace) || asset != &policy.asset_definition_id {
            return Err(invalid(
                "private sponsor vault requires the exact root route and committed local fee currency",
            ));
        }
        return Ok(AssetBalanceScope::Dataspace(dataspace));
    }
    let definition = world
        .asset_definition(asset)
        .map_err(|_| invalid("sponsor vault currency is not registered"))?;
    if definition.balance_scope_policy() != AssetBalancePolicy::Global {
        return Err(invalid(format!(
            "fee sponsor asset `{asset}` must use Global balance scope"
        )));
    }
    Ok(AssetBalanceScope::Global)
}

pub(super) fn validate_sponsor_custody(
    world: &impl WorldReadOnly,
    custody: &AccountId,
    route: Option<DataSpaceId>,
    charges: &[FeeChargeBound],
) -> Result<(), NexusFeeAdmissionError> {
    let Some((dataspace, policy)) = policy(world)? else {
        return Ok(());
    };
    if world.account(custody).is_err() {
        return Err(invalid(
            "private sponsor vault custody account is not registered in this root",
        ));
    }
    let mut required = Quantity::zero();
    for charge in charges {
        sponsor_asset_scope(world, &charge.asset_definition_id, route)?;
        required = checked_quantity_add(&required, &charge.max_bound, "private vault custody")?;
    }
    let asset = AssetId::with_scope(
        policy.asset_definition_id,
        custody.clone(),
        AssetBalanceScope::Dataspace(dataspace),
    );
    let available = world
        .assets()
        .get(&asset)
        .map_or_else(Quantity::zero, |balance| balance.as_ref().clone());
    if available < required {
        return Err(NexusFeeAdmissionError::sponsor(
            FeeRejectionCode::VaultInsufficient,
            "private sponsor custody has insufficient exact scoped currency",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        query::store::LiveQueryStore,
        state::{State, StateReadOnly as _, World},
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        asset::{Asset, AssetDefinition},
        block::BlockHeader,
        domain::Domain,
        nexus::{DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig, LaneVisibility},
        parameter::{CustomParameter, Parameter},
        transaction::TransactionBuilder,
    };
    use iroha_model_base::domain::DomainId;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};

    fn fixture(with_policy: bool, restricted: bool) -> (State, DataSpaceId, AssetDefinitionId) {
        let ds = DataSpaceId::new(u64::MAX - 19);
        let domain = DomainId::parse_fully_qualified("app.privatefees").unwrap();
        let asset =
            AssetDefinitionId::derive_from_components(domain.clone(), "gas".parse().unwrap());
        let mut definition = AssetDefinition::numeric(
            asset.clone(),
            "Private gas",
            if restricted {
                AssetBalancePolicy::DataspaceRestricted
            } else {
                AssetBalancePolicy::Global
            },
            Some(domain.clone()),
        )
        .build(&ALICE_ID);
        definition.total_quantity = Quantity::from(1000_u32);
        let world = World::with_assets(
            [Domain::new(domain).build(&ALICE_ID)],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&ALICE_ID),
            ],
            [definition],
            [Asset::new(
                AssetId::with_scope(
                    asset.clone(),
                    ALICE_ID.clone(),
                    if restricted {
                        AssetBalanceScope::Dataspace(ds)
                    } else {
                        AssetBalanceScope::Global
                    },
                ),
                Quantity::from(1000_u32),
            )],
            [],
        );
        let scope = SumeragiRootScope::Dataspace {
            parent_network_id: iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"fee parent")),
            ),
            dataspace_id: ds,
        };
        let mut parameters = world.parameters.block();
        parameters.set_parameter(crate::sumeragi::lanes::routing::test_support::metadata(
            scope,
        ));
        if with_policy {
            parameters.set_parameter(Parameter::Custom(
                PrivateRootFeePolicy {
                    asset_definition_id: asset.clone(),
                    base_fee: 2_u32.into(),
                    per_byte_fee: Quantity::zero(),
                    per_instruction_fee: 3_u32.into(),
                    per_gas_unit_fee: 4_u32.into(),
                }
                .into_custom_parameter()
                .unwrap(),
            ));
        }
        parameters.commit();
        let mut nexus = iroha_config::parameters::actual::Nexus::default();
        nexus.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: ds,
            alias: "privatefees".into(),
            description: None,
            fault_tolerance: 1,
        }])
        .unwrap();
        nexus.configured_dataspace_catalog = nexus.dataspace_catalog.clone();
        nexus.lane_catalog = LaneCatalog::new(
            std::num::NonZeroU32::new(1).unwrap(),
            vec![LaneConfig {
                dataspace_id: ds,
                visibility: LaneVisibility::Restricted,
                ..LaneConfig::default()
            }],
        )
        .unwrap();
        nexus.configured_lane_catalog = nexus.lane_catalog.clone();
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
        nexus.routing_policy.default_dataspace = ds;
        nexus.fees.fee_asset_id = asset.canonical_address();
        nexus.fees.sponsor_vault_custody_account_id = BOB_ID.clone();
        (
            State::new_with_nexus_for_testing(world, nexus, LiveQueryStore::start_test()),
            ds,
            asset,
        )
    }

    fn bind_signed_source(
        tx: &mut StateTransaction<'_, '_>,
        signed: &iroha_data_model::transaction::SignedTransaction,
        ds: DataSpaceId,
    ) {
        // Direct component fixtures own the exact signed execution that the
        // Network producer normally captures before any balance mutation.
        tx.current_entrypoint_index = Some(0);
        tx.current_network_entrypoint_hash = Some(signed.hash_as_entrypoint());
        tx.current_tx_hash = Some(signed.hash());
        tx.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
        tx.current_lane_id = Some(iroha_model_base::topology::LaneId::SINGLE);
        tx.current_dataspace_id = Some(ds);
        tx.world.current_dataspace_id = Some(ds);
    }

    #[test]
    fn committed_private_schedule_ignores_local_zero_rates_and_quotes_exact_scoped_asset() {
        let (state, ds, asset) = fixture(true, true);
        let view = state.view();
        let mut nexus = view.nexus().clone();
        nexus.fees.base_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = "xor#universal".into();
        let transaction = TransactionBuilder::new(
            *view.network_id(),
            ALICE_ID.clone(),
            FeePaymentIntent::authority(vec![], None),
        )
        .with_instructions([iroha_data_model::isi::Log::new(
            iroha_logger::Level::INFO,
            "private paid work".into(),
        )])
        .sign(ALICE_KEYPAIR.private_key());
        let quote = quote_nexus_fee_admission_draft(
            view.world(),
            &nexus,
            &Pipeline::default(),
            transaction.payload(),
            0,
            2,
            Some(ds),
        )
        .unwrap();
        assert_eq!(quote.quote.charges.len(), 1);
        assert_eq!(quote.quote.charges[0].asset_definition_id, asset);
        assert!(!quote.quote.charges[0].max_bound.is_zero());
        assert_eq!(
            quote.quote.authority_charge_assets[&FeeChargeKind::Nexus].scope,
            AssetBalanceScope::Dataspace(ds)
        );
        assert!(private_scope(view.world(), None).is_err());
        assert!(private_scope(view.world(), Some(DataSpaceId::UNIVERSAL)).is_err());
        assert!(!permits_public_exemption(view.world()));
    }

    #[test]
    fn private_fee_burn_debits_local_balance_and_supply_with_signed_limits() {
        let (state, ds, asset) = fixture(true, true);
        let transaction = TransactionBuilder::new(
            *state.view().network_id(),
            ALICE_ID.clone(),
            FeePaymentIntent::authority(
                vec![FeeChargeLimit::new(
                    FeeChargeKind::Nexus,
                    asset.clone(),
                    Quantity::from(9_u32),
                )],
                None,
            ),
        )
        .with_instructions([iroha_data_model::isi::Log::new(
            iroha_logger::Level::INFO,
            "charge".into(),
        )])
        .sign(ALICE_KEYPAIR.private_key());
        let mut block = state.block(BlockHeader::new(2.try_into().unwrap(), None, None, 1000, 0));
        let mut tx =
            block.transaction_for_fastpq_testing(Hash::from(transaction.hash_as_entrypoint()));
        bind_signed_source(&mut tx, &transaction, ds);
        Executor::charge_nexus_fees(
            &mut tx,
            &ALICE_ID,
            &transaction,
            None,
            norito::canonical_frame_len(transaction.payload()).unwrap(),
            1,
            1,
        )
        .unwrap();
        let scoped = AssetId::with_scope(
            asset.clone(),
            ALICE_ID.clone(),
            AssetBalanceScope::Dataspace(ds),
        );
        assert_eq!(
            tx.world.assets().get(&scoped).unwrap().as_ref(),
            &Quantity::from(991_u32)
        );
        assert_eq!(
            tx.world.asset_definition(&asset).unwrap().total_quantity(),
            &Quantity::from(991_u32)
        );
        assert!(
            tx.world
                .assets()
                .get(&AssetId::new(asset, ALICE_ID.clone()))
                .is_none()
        );
        assert_eq!(tx.current_dataspace_id, Some(ds));
        assert_eq!(tx.world.current_dataspace_id, Some(ds));
    }

    #[test]
    fn private_fee_limits_and_funding_reject_before_any_balance_or_supply_change() {
        for (limit, gas) in [(8_u32, 1_u64), (2000, 300)] {
            let (state, ds, asset) = fixture(true, true);
            let signed = TransactionBuilder::new(
                *state.view().network_id(),
                ALICE_ID.clone(),
                FeePaymentIntent::authority(
                    vec![FeeChargeLimit::new(
                        FeeChargeKind::Nexus,
                        asset.clone(),
                        limit.into(),
                    )],
                    None,
                ),
            )
            .with_instructions([iroha_data_model::isi::Log::new(
                iroha_logger::Level::INFO,
                "bounded paid work".into(),
            )])
            .sign(ALICE_KEYPAIR.private_key());
            let mut block =
                state.block(BlockHeader::new(2.try_into().unwrap(), None, None, 1000, 0));
            let mut tx =
                block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
            bind_signed_source(&mut tx, &signed, ds);
            assert!(
                Executor::charge_nexus_fees(
                    &mut tx,
                    &ALICE_ID,
                    &signed,
                    None,
                    norito::canonical_frame_len(signed.payload()).unwrap(),
                    1,
                    gas
                )
                .is_err()
            );
            let scoped = AssetId::with_scope(
                asset.clone(),
                ALICE_ID.clone(),
                AssetBalanceScope::Dataspace(ds),
            );
            assert_eq!(
                tx.world.assets().get(&scoped).unwrap().as_ref(),
                &Quantity::from(1000_u32)
            );
            assert_eq!(
                tx.world.asset_definition(&asset).unwrap().total_quantity(),
                &Quantity::from(1000_u32)
            );
            assert_eq!(tx.current_dataspace_id, Some(ds));
            assert_eq!(tx.world.current_dataspace_id, Some(ds));
        }
    }

    #[test]
    fn private_fee_sponsor_cannot_select_an_unscoped_public_vault() {
        let (state, ds, _) = fixture(true, true);
        let view = state.view();
        let transaction = TransactionBuilder::new(
            *view.network_id(),
            ALICE_ID.clone(),
            FeePaymentIntent::sponsor(
                FeeSponsorProgramId::new(ALICE_ID.clone(), "public_vault".parse().unwrap()),
                1,
                vec![],
                None,
            ),
        )
        .with_instructions([iroha_data_model::isi::Log::new(
            iroha_logger::Level::INFO,
            "sponsored work".into(),
        )])
        .sign(ALICE_KEYPAIR.private_key());
        let error = quote_nexus_fee_admission_draft(
            view.world(),
            view.nexus(),
            &Pipeline::default(),
            transaction.payload(),
            0,
            2,
            Some(ds),
        )
        .unwrap_err();
        assert_eq!(
            error,
            NexusFeeAdmissionError::sponsor(
                FeeRejectionCode::VaultInsufficient,
                "private sponsor custody has insufficient exact scoped currency"
            )
        );
    }

    #[test]
    fn private_vault_funding_paid_execution_and_withdrawal_preserve_exact_local_scope() {
        use iroha_data_model::{isi::nexus::*, nexus::*};
        let (state, ds, asset) = fixture(true, true);
        let network_id = *state.view().network_id();
        let mut block = state.block(BlockHeader::new(2.try_into().unwrap(), None, None, 1000, 0));
        let program_id = FeeSponsorProgramId::new(ALICE_ID.clone(), "local".parse().unwrap());
        let fund = FundFeeSponsorProgram {
            program_id: program_id.clone(),
            asset_definition_id: asset.clone(),
            amount: 200_u32.into(),
        };
        let setup = TransactionBuilder::new(
            network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(vec![], None),
        )
        .with_instructions([fund.clone()])
        .sign(ALICE_KEYPAIR.private_key());
        let mut tx = block.transaction_for_fastpq_testing(Hash::from(setup.hash_as_entrypoint()));
        bind_signed_source(&mut tx, &setup, ds);
        CreateFeeSponsorProgram {
            program: FeeSponsorProgram::new(program_id.clone(), ALICE_ID.clone()),
        }
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        let instruction =
            iroha_data_model::isi::InstructionBox::from(iroha_data_model::isi::Log::new(
                iroha_logger::Level::INFO,
                "scoped sponsor work".into(),
            ));
        let revision = FeeSponsorProgramRevision {
            program_id: program_id.clone(),
            revision: 1,
            eligibility: FeeSponsorEligibility::EnrolledOnly,
            rules: vec![FeeSponsorRule {
                id: "log".parse().unwrap(),
                effect: FeeSponsorRuleEffect::Allow,
                selectors: vec![FeeSponsorRuleSelector::NativeInstruction(
                    FeeSponsorNativeInstructionSelector {
                        wire_id: iroha_data_model::isi::instruction_wire_id(&instruction)
                            .unwrap()
                            .into(),
                        asset_definition_id: None,
                    },
                )],
            }],
            asset_budgets: vec![FeeSponsorAssetBudget {
                asset_definition_id: asset.clone(),
                per_transaction: 150_u32.into(),
                per_block: 300_u32.into(),
                per_program_epoch: 600_u32.into(),
                per_beneficiary_epoch: 300_u32.into(),
                reserve_floor: 10_u32.into(),
                epoch_length_blocks: std::num::NonZeroU64::new(10).unwrap(),
            }],
        };
        let mut zero = revision.clone();
        zero.asset_budgets[0].per_transaction = Quantity::zero();
        assert!(
            StageFeeSponsorProgramRevision { revision: zero }
                .execute(&ALICE_ID, &mut tx)
                .is_err()
        );
        StageFeeSponsorProgramRevision { revision }
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
        EnrollFeeSponsorBeneficiary {
            program_id: program_id.clone(),
            beneficiary: ALICE_ID.clone(),
        }
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        fund.execute(&ALICE_ID, &mut tx).unwrap();
        ActivateFeeSponsorProgramRevision {
            program_id: program_id.clone(),
            revision: 1,
            activate_at_height: 2,
        }
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        tx.apply();
        let signed = TransactionBuilder::new(
            network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::sponsor(
                program_id.clone(),
                1,
                vec![FeeChargeLimit::new(
                    FeeChargeKind::Nexus,
                    asset.clone(),
                    150_u32.into(),
                )],
                None,
            ),
        )
        .with_instructions([instruction])
        .sign(ALICE_KEYPAIR.private_key());
        let mut tx = block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
        bind_signed_source(&mut tx, &signed, ds);
        let quoted = quote_nexus_fee_admission_draft(
            &tx.world,
            &tx.nexus,
            &Pipeline::default(),
            signed.payload(),
            1000,
            2,
            Some(ds),
        )
        .unwrap();
        assert_eq!(
            quoted.quote.debit_source,
            FeeDebitSource::SponsorProgram(program_id.clone())
        );
        assert!(
            quote_nexus_fee_admission_draft(
                &tx.world,
                &tx.nexus,
                &Pipeline::default(),
                signed.payload(),
                1000,
                2,
                Some(DataSpaceId::UNIVERSAL)
            )
            .is_err()
        );
        Executor::charge_nexus_fees(
            &mut tx,
            &ALICE_ID,
            &signed,
            Some(program_id.clone()),
            norito::canonical_frame_len(signed.payload()).unwrap(),
            1,
            1,
        )
        .unwrap();
        let owner_balance = AssetId::with_scope(
            asset.clone(),
            ALICE_ID.clone(),
            AssetBalanceScope::Dataspace(ds),
        );
        let custody_balance = AssetId::with_scope(
            asset.clone(),
            BOB_ID.clone(),
            AssetBalanceScope::Dataspace(ds),
        );
        assert_eq!(
            tx.world.assets().get(&owner_balance).unwrap().as_ref(),
            &Quantity::from(800_u32)
        );
        assert_eq!(
            tx.world.assets().get(&custody_balance).unwrap().as_ref(),
            &Quantity::from(191_u32)
        );
        let vault_key = FeeSponsorVaultKey {
            program_id: program_id.clone(),
            asset_definition_id: asset.clone(),
        };
        assert_eq!(
            tx.world
                .fee_sponsor_vaults()
                .get(&vault_key)
                .unwrap()
                .balance,
            Quantity::from(191_u32)
        );
        tx.apply();
        let pause = PauseFeeSponsorProgram {
            program_id: program_id.clone(),
        };
        let withdraw = WithdrawFeeSponsorProgram {
            program_id,
            asset_definition_id: asset.clone(),
            amount: 20_u32.into(),
        };
        let withdrawal = TransactionBuilder::new(
            network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(vec![], None),
        )
        .with_instructions([
            iroha_data_model::isi::InstructionBox::from(pause.clone()),
            iroha_data_model::isi::InstructionBox::from(withdraw.clone()),
        ])
        .sign(ALICE_KEYPAIR.private_key());
        let mut tx =
            block.transaction_for_fastpq_testing(Hash::from(withdrawal.hash_as_entrypoint()));
        bind_signed_source(&mut tx, &withdrawal, ds);
        pause.execute(&ALICE_ID, &mut tx).unwrap();
        withdraw.execute(&ALICE_ID, &mut tx).unwrap();
        assert_eq!(
            tx.world.assets().get(&owner_balance).unwrap().as_ref(),
            &Quantity::from(820_u32)
        );
        assert_eq!(
            tx.world.assets().get(&custody_balance).unwrap().as_ref(),
            &Quantity::from(171_u32)
        );
        assert_eq!(
            tx.world.asset_definition(&asset).unwrap().total_quantity(),
            &Quantity::from(991_u32)
        );
        assert!(
            tx.world
                .assets()
                .get(&AssetId::new(asset.clone(), BOB_ID.clone()))
                .is_none()
        );
        assert!(sponsor_asset_scope(&tx.world, &asset, Some(DataSpaceId::UNIVERSAL)).is_err());
        assert!(
            sponsor_asset_scope(
                &tx.world,
                &AssetDefinitionId::derive_from_components(
                    DomainId::parse_fully_qualified("app.universal").unwrap(),
                    "gas".parse().unwrap()
                ),
                Some(ds)
            )
            .is_err()
        );
    }

    #[test]
    fn private_policy_refuses_missing_malformed_global_assets_and_unbound_roots() {
        let (missing, _, _) = fixture(false, true);
        assert!(policy(missing.view().world()).is_err());
        let (global_asset, _, _) = fixture(true, false);
        assert!(policy(global_asset.view().world()).is_err());
        assert!(policy(&World::new().view()).is_err());
        let (state, _, _) = fixture(true, true);
        let mut block = state.block(BlockHeader::new(2.try_into().unwrap(), None, None, 1000, 0));
        let mut tx = block.transaction();
        tx.world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(CustomParameter::new(
                PrivateRootFeePolicy::parameter_id(),
                iroha_primitives::json::Json::new(false),
            )));
        assert!(policy(&tx.world).is_err());
    }
}
