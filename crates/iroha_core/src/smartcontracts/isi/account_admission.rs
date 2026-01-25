//! Helpers for domain-scoped account admission and implicit account creation.
//!
//! This module provides a deterministic implementation of "implicit accounts":
//! when a domain opts in via metadata policy, receipt-like operations (asset mint/transfer, NFT
//! transfer) may create the destination `Account` object automatically if it does not exist yet.

use std::sync::LazyLock;

use iroha_data_model::{
    HasMetadata as _, IntoKeyValue,
    account::{
        ACCOUNT_ADMISSION_POLICY_METADATA_KEY, AccountAdmissionMode, AccountAdmissionPolicy,
        admission::{ImplicitAccountCreationFee, ImplicitAccountFeeDestination},
        curve::CurveId,
    },
    asset::{AssetDefinitionId, AssetId},
    isi::error::{
        AccountAdmissionDefaultRoleError, AccountAdmissionError, AccountAdmissionFeeUnsatisfied,
        AccountAdmissionInvalidPolicy, AccountAdmissionMinInitialAmountUnsatisfied,
        AccountAdmissionQuotaExceeded, AccountAdmissionQuotaScope, InstructionExecutionError,
    },
    name::Name,
    prelude::*,
};
use iroha_primitives::{json::Json, numeric::Numeric};

use crate::{
    role::RoleIdWithOwner,
    state::{StateTransaction, WorldReadOnly},
};

static POLICY_METADATA_KEY: LazyLock<Name> = LazyLock::new(|| {
    ACCOUNT_ADMISSION_POLICY_METADATA_KEY
        .parse()
        .expect("account admission policy metadata key must be a valid Name")
});

static IMPLICIT_CREATED_VIA_KEY: LazyLock<Name> = LazyLock::new(|| {
    "iroha:created_via"
        .parse()
        .expect("implicit created_via metadata key must be a valid Name")
});

fn first_disallowed_algorithm(
    controller: &AccountController,
    allowed: &[iroha_crypto::Algorithm],
) -> Option<iroha_crypto::Algorithm> {
    match controller {
        AccountController::Single(signatory) => {
            algorithm_if_disallowed(signatory.algorithm(), allowed)
        }
        AccountController::Multisig(policy) => policy
            .members()
            .iter()
            .find_map(|member| algorithm_if_disallowed(member.algorithm(), allowed)),
    }
}

fn algorithm_if_disallowed(
    algorithm: iroha_crypto::Algorithm,
    allowed: &[iroha_crypto::Algorithm],
) -> Option<iroha_crypto::Algorithm> {
    if allowed.contains(&algorithm) {
        None
    } else {
        Some(algorithm)
    }
}

fn ensure_controller_allowed_for_implicit_admission(
    controller: &AccountController,
    allowed_algorithms: &[iroha_crypto::Algorithm],
    allowed_curve_ids: &[u8],
) -> Result<(), AccountAdmissionError> {
    if let Some(disallowed) = first_disallowed_algorithm(controller, allowed_algorithms) {
        return Err(AccountAdmissionError::AlgorithmNotAllowed(disallowed));
    }

    let validate_curve = |algorithm| {
        let Ok(curve) = CurveId::try_from_algorithm(algorithm) else {
            return Err(AccountAdmissionError::AlgorithmNotAllowed(algorithm));
        };
        let curve_code: u8 = curve.into();
        if allowed_curve_ids.contains(&curve_code) {
            Ok(())
        } else {
            Err(AccountAdmissionError::AlgorithmNotAllowed(algorithm))
        }
    };

    match controller {
        AccountController::Single(signatory) => validate_curve(signatory.algorithm())?,
        AccountController::Multisig(policy) => {
            for member in policy.members() {
                validate_curve(member.algorithm())?;
            }
        }
    }

    Ok(())
}

fn load_account_admission_policy(
    destination: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<AccountAdmissionPolicy, InstructionExecutionError> {
    let domain = state_transaction.world.domain(destination.domain())?;
    domain
        .metadata()
        .get(&*POLICY_METADATA_KEY)
        .map_or_else(
            || {
                state_transaction
                    .world
                    .parameters()
                    .custom()
                    .get(&AccountAdmissionPolicy::parameter_id())
                    .map_or_else(
                        || Ok(AccountAdmissionPolicy::default()),
                        |custom| {
                            custom
                                .payload()
                                .try_into_any_norito::<AccountAdmissionPolicy>()
                                .map_err(|err| {
                                    AccountAdmissionError::InvalidPolicy(
                                        AccountAdmissionInvalidPolicy {
                                            domain: destination.domain().clone(),
                                            reason: format!(
                                                "chain parameter `{}` decode error: {err}",
                                                AccountAdmissionPolicy::PARAMETER_ID_STR
                                            ),
                                        },
                                    )
                                    .into()
                                })
                        },
                    )
            },
            |policy_json| {
                policy_json
                    .try_into_any_norito::<AccountAdmissionPolicy>()
                    .map_err(|err| {
                        AccountAdmissionError::InvalidPolicy(AccountAdmissionInvalidPolicy {
                            domain: destination.domain().clone(),
                            reason: format!(
                                "domain metadata key `{ACCOUNT_ADMISSION_POLICY_METADATA_KEY}` decode error: {err}"
                            ),
                        })
                        .into()
                    })
            },
        )
}

fn apply_implicit_creation_fee(
    authority: &AccountId,
    fee: &ImplicitAccountCreationFee,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let payer_asset_id = AssetId::new(fee.asset_definition_id.clone(), authority.clone());
    let available = state_transaction.world.asset(&payer_asset_id).map_or_else(
        |_| Numeric::zero(),
        |asset| asset.value().clone().into_inner(),
    );

    if available < fee.amount {
        return Err(
            AccountAdmissionError::FeeUnsatisfied(AccountAdmissionFeeUnsatisfied {
                asset_definition: fee.asset_definition_id.clone(),
                required: fee.amount.clone(),
                available,
            })
            .into(),
        );
    }

    state_transaction
        .world
        .withdraw_numeric_asset(&payer_asset_id, &fee.amount)?;

    match &fee.destination {
        ImplicitAccountFeeDestination::Burn => {
            state_transaction
                .world
                .decrease_asset_total_amount(&fee.asset_definition_id, &fee.amount)?;
        }
        ImplicitAccountFeeDestination::Account(sink) => {
            let sink_asset_id = AssetId::new(fee.asset_definition_id.clone(), sink.clone());
            state_transaction
                .world
                .deposit_numeric_asset(&sink_asset_id, &fee.amount)?;
        }
    }

    Ok(())
}

fn create_implicit_account(
    destination: &AccountId,
    default_role_on_create: Option<&RoleId>,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    let mut metadata = Metadata::default();
    metadata.insert(IMPLICIT_CREATED_VIA_KEY.clone(), Json::new("implicit"));
    let account = Account {
        id: destination.clone(),
        metadata,
        label: None,
        uaid: None,
        opaque_ids: Vec::new(),
    };
    let (account_id, account_value) = account.clone().into_key_value();
    state_transaction
        .world
        .accounts
        .insert(account_id, account_value);

    let mut default_role_granted = None;
    if let Some(role) = default_role_on_create {
        let role_key = RoleIdWithOwner::new(destination.clone(), role.clone());
        if state_transaction
            .world
            .account_roles
            .insert(role_key.clone(), ())
            .is_some()
        {
            state_transaction.world.account_roles.remove(role_key);
            state_transaction.world.accounts.remove(destination.clone());
            return Err(AccountAdmissionError::DefaultRoleError(
                AccountAdmissionDefaultRoleError {
                    role: role.clone(),
                    reason: "duplicate role assignment".to_owned(),
                },
            )
            .into());
        }
        default_role_granted = Some(role.clone());
        state_transaction.invalidate_permission_cache_for_account(destination);
    }

    state_transaction
        .world
        .emit_events(Some(DomainEvent::Account(AccountEvent::Created(account))));

    if let Some(role) = default_role_granted {
        state_transaction
            .world
            .emit_events(Some(AccountEvent::RoleGranted(AccountRoleChanged {
                account: destination.clone(),
                role,
            })));
    }

    Ok(())
}

/// Ensure `destination` account exists, creating it implicitly when allowed by policy.
///
/// Returns `Ok(true)` when the account was created, `Ok(false)` when it already existed.
pub(super) fn ensure_receiving_account(
    authority: &AccountId,
    destination: &AccountId,
    value_hint: Option<(&AssetDefinitionId, &Numeric)>,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<bool, InstructionExecutionError> {
    if state_transaction.world.account(destination).is_ok() {
        return Ok(false);
    }

    if *destination.domain() == *iroha_genesis::GENESIS_DOMAIN_ID {
        return Err(AccountAdmissionError::GenesisDomainForbidden.into());
    }

    let policy = load_account_admission_policy(destination, state_transaction)?;

    if policy.mode != AccountAdmissionMode::ImplicitReceive {
        return Err(AccountAdmissionError::ImplicitAccountCreationDisabled(
            destination.domain().clone(),
        )
        .into());
    }

    if let Some((asset_def_id, amount)) = value_hint {
        if let Some(required) = policy.min_initial_amount_for(asset_def_id) {
            if amount < required {
                return Err(AccountAdmissionError::MinInitialAmountUnsatisfied(
                    AccountAdmissionMinInitialAmountUnsatisfied {
                        asset_definition: asset_def_id.clone(),
                        required: required.clone(),
                        provided: amount.clone(),
                    },
                )
                .into());
            }
        }
    }

    let max_creations = policy.max_implicit_creations_per_tx();
    if state_transaction.implicit_account_creations_in_tx >= max_creations {
        return Err(
            AccountAdmissionError::QuotaExceeded(AccountAdmissionQuotaExceeded {
                scope: AccountAdmissionQuotaScope::Transaction,
                created: state_transaction.implicit_account_creations_in_tx,
                cap: max_creations,
            })
            .into(),
        );
    }

    if let Some(max_creations) = policy.max_implicit_creations_per_block() {
        let created_so_far = state_transaction
            .implicit_account_creations_in_block_so_far
            .saturating_add(state_transaction.implicit_account_creations_in_tx);
        if created_so_far >= max_creations {
            return Err(
                AccountAdmissionError::QuotaExceeded(AccountAdmissionQuotaExceeded {
                    scope: AccountAdmissionQuotaScope::Block,
                    created: created_so_far,
                    cap: max_creations,
                })
                .into(),
            );
        }
    }

    let default_role_on_create = policy.default_role_on_create().cloned();

    if let Some(default_role) = &default_role_on_create {
        state_transaction.world.role(default_role).map_err(|err| {
            AccountAdmissionError::DefaultRoleError(AccountAdmissionDefaultRoleError {
                role: default_role.clone(),
                reason: err.to_string(),
            })
        })?;
    }

    ensure_controller_allowed_for_implicit_admission(
        destination.controller(),
        &state_transaction.crypto.allowed_signing,
        &state_transaction.crypto.allowed_curve_ids,
    )?;

    if let Some(fee) = policy.implicit_creation_fee() {
        apply_implicit_creation_fee(authority, fee, state_transaction)?;
    }

    create_implicit_account(
        destination,
        default_role_on_create.as_ref(),
        state_transaction,
    )?;

    state_transaction.implicit_account_creations_in_tx += 1;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, LazyLock, Mutex, MutexGuard},
    };

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::{
            AccountDomainSelector, admission::ImplicitAccountCreationFee,
            clear_account_domain_selector_resolver, set_account_domain_selector_resolver,
        },
        permission::Permissions,
    };
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, World},
    };

    struct DomainSelectorGuard {
        _lock: MutexGuard<'static, ()>,
    }

    impl Drop for DomainSelectorGuard {
        fn drop(&mut self) {
            clear_account_domain_selector_resolver();
        }
    }

    fn guard_domain_selector_resolver(domain: DomainId) -> DomainSelectorGuard {
        static DOMAIN_SELECTOR_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        let lock = DOMAIN_SELECTOR_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let selector = AccountDomainSelector::from_domain(&domain).expect("domain selector");
        let resolver_domain = domain.clone();
        set_account_domain_selector_resolver(Arc::new(move |candidate| {
            if candidate == &selector {
                Some(resolver_domain.clone())
            } else {
                None
            }
        }));
        DomainSelectorGuard { _lock: lock }
    }

    fn open_domain(domain_id: DomainId, policy: AccountAdmissionPolicy) -> Domain {
        let mut metadata = Metadata::default();
        let key: Name = ACCOUNT_ADMISSION_POLICY_METADATA_KEY
            .parse()
            .expect("policy metadata key");
        metadata.insert(key, Json::new(policy));
        Domain::new(domain_id)
            .with_metadata(metadata)
            .build(&ALICE_ID)
    }

    fn test_state(world: World) -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(world, kura, query)
    }

    #[test]
    fn transfer_asset_creates_destination_account_in_open_domain() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: None,
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
        let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
        let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(100, 0));

        let world = World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        Transfer::asset_numeric(alice_asset_id.clone(), 10_u32, dest.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("transfer succeeds");

        stx.world
            .account(&dest)
            .expect("destination account should be created");

        let dest_asset_id = AssetId::new(asset_def_id.clone(), dest.clone());
        let alice_balance = stx
            .world
            .asset_mut(&alice_asset_id)
            .expect("alice asset exists")
            .clone()
            .into_inner();
        let dest_balance = stx
            .world
            .asset_mut(&dest_asset_id)
            .expect("destination asset exists")
            .clone()
            .into_inner();
        assert_eq!(alice_balance, Numeric::new(90, 0));
        assert_eq!(dest_balance, Numeric::new(10, 0));

        // AccountCreated must come before any receipt events.
        let events = &stx.world.internal_event_buf;
        assert!(!events.is_empty(), "events must be emitted");
        assert!(
            matches!(
                events[0].as_ref(),
                DataEvent::Domain(DomainEvent::Account(AccountEvent::Created(_)))
            ),
            "first event should be destination AccountCreated, got {:?}",
            events[0]
        );

        // The created account must advertise its origin in metadata.
        let account = stx.world.account(&dest).expect("account");
        let account_details = account.value();
        let created_via_key: Name = "iroha:created_via".parse().expect("created_via key");
        assert_eq!(
            account_details.metadata().get(&created_via_key),
            Some(&Json::new("implicit"))
        );
    }

    #[test]
    fn transfer_asset_batch_creates_destination_account_in_open_domain() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: None,
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
        let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
        let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(100, 0));

        let world = World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        let entry = TransferAssetBatchEntry::new(
            ALICE_ID.clone(),
            dest.clone(),
            asset_def_id.clone(),
            5_u32,
        );
        TransferAssetBatch::new(vec![entry])
            .execute(&ALICE_ID, &mut stx)
            .expect("batch transfer succeeds");

        stx.world
            .account(&dest)
            .expect("destination account should be created");

        let dest_asset_id = AssetId::new(asset_def_id, dest);
        let alice_balance = stx
            .world
            .asset_mut(&alice_asset_id)
            .expect("alice asset exists")
            .clone()
            .into_inner();
        let dest_balance = stx
            .world
            .asset_mut(&dest_asset_id)
            .expect("destination asset exists")
            .clone()
            .into_inner();
        assert_eq!(alice_balance, Numeric::new(95, 0));
        assert_eq!(dest_balance, Numeric::new(5, 0));
    }

    #[test]
    fn mint_asset_creates_destination_account_in_open_domain() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: None,
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);

        let world = World::with([domain], [alice_account], [asset_def]);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        let dest_asset_id = AssetId::new(asset_def_id, dest.clone());
        Mint::asset_numeric(7_u32, dest_asset_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("mint succeeds");

        stx.world
            .account(&dest)
            .expect("destination account should be created");
        let dest_balance = stx
            .world
            .asset_mut(&dest_asset_id)
            .expect("destination asset exists")
            .clone()
            .into_inner();
        assert_eq!(dest_balance, Numeric::new(7, 0));

        let events = &stx.world.internal_event_buf;
        assert!(
            matches!(
                events[0].as_ref(),
                DataEvent::Domain(DomainEvent::Account(AccountEvent::Created(_)))
            ),
            "first event should be destination AccountCreated"
        );
    }

    #[test]
    fn transfer_nft_creates_destination_account_in_open_domain() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: None,
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let nft_id: NftId = "n0$wonderland".parse().expect("nft id");
        let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&ALICE_ID);

        let world = World::with_assets([domain], [alice_account], [], [], [nft]);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        Transfer::nft(ALICE_ID.clone(), nft_id.clone(), dest.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("nft transfer succeeds");

        stx.world
            .account(&dest)
            .expect("destination account should be created");
        let nft_entry = stx.world.nft(&nft_id).expect("nft exists");
        assert_eq!(nft_entry.value().owned_by, dest);

        let events = &stx.world.internal_event_buf;
        assert_eq!(events.len(), 2, "expected account + nft events");
        assert!(
            matches!(
                events[0].as_ref(),
                DataEvent::Domain(DomainEvent::Account(AccountEvent::Created(_)))
            ),
            "first event should be destination AccountCreated"
        );
        assert!(
            matches!(
                events[1].as_ref(),
                DataEvent::Domain(DomainEvent::Nft(NftEvent::OwnerChanged(_)))
            ),
            "second event should be NFT owner change"
        );
    }

    #[test]
    fn transfer_asset_rejects_missing_destination_in_explicit_domain() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ExplicitOnly,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: None,
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
        let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
        let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(100, 0));

        let world = World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        let err = Transfer::asset_numeric(alice_asset_id, 10_u32, dest)
            .execute(&ALICE_ID, &mut stx)
            .expect_err("transfer should fail in ExplicitOnly domain");
        assert!(
            matches!(
                err,
                InstructionExecutionError::AccountAdmission(
                    AccountAdmissionError::ImplicitAccountCreationDisabled(_)
                )
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn chain_default_policy_disables_implicit_receive_without_domain_metadata() {
        use iroha_data_model::parameter::Parameters;

        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
        let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
        let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(100, 0));

        let mut world =
            World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);
        let policy = AccountAdmissionPolicy {
            mode: AccountAdmissionMode::ExplicitOnly,
            max_implicit_creations_per_tx: None,
            max_implicit_creations_per_block: None,
            implicit_creation_fee: None,
            min_initial_amounts: BTreeMap::new(),
            default_role_on_create: None,
        };
        let custom = policy.into_custom_parameter();
        let mut params = Parameters::default();
        params.custom.insert(custom.id.clone(), custom);
        world.parameters = mv::cell::Cell::new(params);

        let state = test_state(world);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        let err = Transfer::asset_numeric(alice_asset_id, 10_u32, dest.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect_err("transfer should be rejected via chain default explicit policy");

        assert!(
            matches!(
                err,
                InstructionExecutionError::AccountAdmission(
                    AccountAdmissionError::ImplicitAccountCreationDisabled(_)
                )
            ),
            "unexpected error: {err:?}"
        );
        assert!(
            stx.world.account(&dest).is_err(),
            "destination must not exist"
        );
    }

    #[test]
    fn implicit_creation_cap_is_enforced_per_transaction() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: Some(1),
                max_implicit_creations_per_block: None,
                implicit_creation_fee: None,
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);

        let world = World::with([domain], [alice_account], [asset_def]);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest1_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest1 = AccountId::new(domain_id.clone(), dest1_kp.public_key().clone());
        let dest1_asset_id = AssetId::new(asset_def_id.clone(), dest1.clone());
        Mint::asset_numeric(1_u32, dest1_asset_id)
            .execute(&ALICE_ID, &mut stx)
            .expect("first mint creates account");

        let dest2_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest2 = AccountId::new(domain_id, dest2_kp.public_key().clone());
        let dest2_asset_id = AssetId::new(asset_def_id, dest2.clone());
        let err = Mint::asset_numeric(1_u32, dest2_asset_id)
            .execute(&ALICE_ID, &mut stx)
            .expect_err("second mint exceeds cap");
        assert!(
            matches!(
                err,
                InstructionExecutionError::AccountAdmission(AccountAdmissionError::QuotaExceeded(
                    AccountAdmissionQuotaExceeded {
                        scope: AccountAdmissionQuotaScope::Transaction,
                        ..
                    }
                ))
            ),
            "unexpected error: {err:?}"
        );
        assert!(stx.world.account(&dest2).is_err(), "dest2 must not exist");
    }

    #[test]
    fn implicit_creation_cap_is_enforced_per_block_across_transactions() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: Some(1),
                implicit_creation_fee: None,
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);

        let world = World::with([domain], [alice_account], [asset_def]);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);

        let dest1_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest1 = AccountId::new(domain_id.clone(), dest1_kp.public_key().clone());
        let dest1_asset_id = AssetId::new(asset_def_id.clone(), dest1.clone());
        {
            let mut stx = block.transaction();
            Mint::asset_numeric(1_u32, dest1_asset_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("first mint creates account");
            stx.apply();
        }

        let dest2_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest2 = AccountId::new(domain_id, dest2_kp.public_key().clone());
        let dest2_asset_id = AssetId::new(asset_def_id, dest2.clone());
        let mut stx = block.transaction();
        let err = Mint::asset_numeric(1_u32, dest2_asset_id)
            .execute(&ALICE_ID, &mut stx)
            .expect_err("second mint exceeds block cap");
        assert!(
            matches!(
                err,
                InstructionExecutionError::AccountAdmission(AccountAdmissionError::QuotaExceeded(
                    AccountAdmissionQuotaExceeded {
                        scope: AccountAdmissionQuotaScope::Block,
                        ..
                    }
                ))
            ),
            "unexpected error: {err:?}"
        );
        assert!(stx.world.account(&dest2).is_err(), "dest2 must not exist");
    }

    #[test]
    fn implicit_creation_fee_is_enforced_and_charged() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let _resolver_guard = guard_domain_selector_resolver(domain_id.clone());
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let fee_sink_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let fee_sink = AccountId::new(domain_id.clone(), fee_sink_kp.public_key().clone());
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: Some(ImplicitAccountCreationFee {
                    asset_definition_id: asset_def_id.clone(),
                    amount: Numeric::new(5, 0),
                    destination: ImplicitAccountFeeDestination::Account(fee_sink.clone()),
                }),
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let fee_sink_account = Account::new(fee_sink.clone()).build(&fee_sink);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
        let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
        let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(20, 0));

        let world = World::with_assets(
            [domain],
            [alice_account, fee_sink_account],
            [asset_def],
            [alice_asset],
            [],
        );
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        let dest_asset_id = AssetId::new(asset_def_id.clone(), dest.clone());
        Mint::asset_numeric(10_u32, dest_asset_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("mint should succeed with fee payment");

        stx.world.account(&dest).expect("destination exists");
        let alice_balance = stx
            .world
            .asset_mut(&alice_asset_id)
            .expect("alice asset exists")
            .clone()
            .into_inner();
        assert_eq!(alice_balance, Numeric::new(15, 0));
        let sink_asset_id = AssetId::new(asset_def_id.clone(), fee_sink.clone());
        let sink_balance = stx
            .world
            .asset_mut(&sink_asset_id)
            .expect("fee sink asset exists")
            .clone()
            .into_inner();
        assert_eq!(sink_balance, Numeric::new(5, 0));
        let dest_balance = stx
            .world
            .asset_mut(&dest_asset_id)
            .expect("destination asset exists")
            .clone()
            .into_inner();
        assert_eq!(dest_balance, Numeric::new(10, 0));
    }

    #[test]
    fn implicit_creation_fee_rejects_when_insufficient_balance() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: Some(ImplicitAccountCreationFee {
                    asset_definition_id: asset_def_id.clone(),
                    amount: Numeric::new(5, 0),
                    destination: ImplicitAccountFeeDestination::Burn,
                }),
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
        let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
        let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(3, 0));

        let world = World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        let dest_asset_id = AssetId::new(asset_def_id.clone(), dest.clone());
        let err = Mint::asset_numeric(1_u32, dest_asset_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect_err("fee should prevent implicit creation");
        assert!(
            matches!(
                err,
                InstructionExecutionError::AccountAdmission(AccountAdmissionError::FeeUnsatisfied(
                    AccountAdmissionFeeUnsatisfied { .. }
                ))
            ),
            "unexpected error: {err:?}"
        );
        assert!(
            stx.world.account(&dest).is_err(),
            "destination must not exist"
        );
        let alice_balance = stx
            .world
            .asset_mut(&alice_asset_id)
            .expect("alice asset exists")
            .clone()
            .into_inner();
        assert_eq!(alice_balance, Numeric::new(3, 0));
    }

    #[test]
    fn min_initial_amount_is_enforced() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let mut min_initial_amounts = BTreeMap::new();
        min_initial_amounts.insert(asset_def_id.clone(), Numeric::new(10, 0));
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: None,
                min_initial_amounts,
                default_role_on_create: None,
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);

        let world = World::with([domain], [alice_account], [asset_def]);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        let dest_asset_id = AssetId::new(asset_def_id.clone(), dest.clone());
        let err = Mint::asset_numeric(5_u32, dest_asset_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect_err("mint should fail when below min initial amount");
        assert!(
            matches!(
                err,
                InstructionExecutionError::AccountAdmission(
                    AccountAdmissionError::MinInitialAmountUnsatisfied(
                        AccountAdmissionMinInitialAmountUnsatisfied { .. }
                    )
                )
            ),
            "unexpected error: {err:?}"
        );
        assert!(
            stx.world.account(&dest).is_err(),
            "destination must not exist"
        );
        assert!(
            stx.world.asset_mut(&dest_asset_id).is_err(),
            "destination asset must not exist"
        );
    }

    #[test]
    fn default_role_is_assigned_on_implicit_creation() {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let role_id: RoleId = "baseline_user".parse().expect("role id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: None,
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: Some(role_id.clone()),
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);

        let mut world = World::with([domain], [alice_account], [asset_def]);
        let role = iroha_data_model::role::Role {
            id: role_id.clone(),
            permissions: Permissions::new(),
        };
        world.roles.insert(role_id.clone(), role);

        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        let dest_asset_id = AssetId::new(asset_def_id.clone(), dest.clone());
        Mint::asset_numeric(1_u32, dest_asset_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("mint should create account and assign role");

        let mut roles = stx.world.account_roles_iter(&dest);
        assert_eq!(
            roles.next(),
            Some(&role_id),
            "implicit account should receive default role"
        );

        let events = &stx.world.internal_event_buf;
        assert!(
            matches!(
                events[0].as_ref(),
                DataEvent::Domain(DomainEvent::Account(AccountEvent::Created(_)))
            ),
            "first event should be account creation"
        );
        assert!(
            matches!(
                events[1].as_ref(),
                DataEvent::Domain(DomainEvent::Account(AccountEvent::RoleGranted(_)))
            ),
            "second event should be default role grant"
        );
    }

    #[test]
    fn default_role_missing_rejects_implicit_creation() {
        let domain_id: DomainId = "missing-role.world".parse().expect("domain id");
        let role_id: RoleId = "starter".parse().expect("role id");
        let domain = open_domain(
            domain_id.clone(),
            AccountAdmissionPolicy {
                mode: AccountAdmissionMode::ImplicitReceive,
                max_implicit_creations_per_tx: None,
                max_implicit_creations_per_block: None,
                implicit_creation_fee: None,
                min_initial_amounts: BTreeMap::new(),
                default_role_on_create: Some(role_id.clone()),
            },
        );
        let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);

        let world = World::with([domain], [alice_account], []);
        let state = test_state(world);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let dest_kp = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let dest = AccountId::new(domain_id, dest_kp.public_key().clone());
        let err = ensure_receiving_account(&ALICE_ID, &dest, None, &mut stx)
            .expect_err("implicit creation should fail when role is absent");

        assert!(
            matches!(
                err,
                InstructionExecutionError::AccountAdmission(
                    AccountAdmissionError::DefaultRoleError(AccountAdmissionDefaultRoleError {
                        ref role,
                        ..
                    })
                ) if role == &role_id
            ),
            "unexpected error: {err:?}"
        );
        assert!(
            stx.world.account(&dest).is_err(),
            "account must not exist on failure"
        );
        assert!(
            stx.world.internal_event_buf.is_empty(),
            "no events should be emitted when role is missing"
        );
    }
}
