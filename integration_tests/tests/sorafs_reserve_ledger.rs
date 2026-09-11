//! Four-validator reserve authority, competing custody decisions and restart convergence.
//!
//! Provider ownership uses the production trusted pre-genesis configuration. Runtime mutations
//! use signed transactions through the Initial executor. Rent coverage rejects premature charging;
//! elapsed monthly rent settlement and hardware service qualification are separate scenarios.

use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use std::time::Duration;

use super::sorafs_network::{prepare_transaction, submit_instruction};
use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    crypto::{Algorithm, HashOf, KeyPair},
    data_model::{
        events::data::sorafs::SorafsReserveLedgerEventKind,
        isi::{
            error::{InstructionExecutionError, InvalidParameterError},
            sorafs::{
                ChargeSorafsReserveRent, DecideSorafsReserveMovement, RegisterSorafsReserveAccount,
                RequestSorafsReserveMovement, SetSorafsReservePolicy,
            },
        },
        prelude::*,
        query::sorafs::prelude::{
            FindSorafsProviderOwner, FindSorafsReserveEvents, FindSorafsReserveMovements,
            FindSorafsReserveProviders,
        },
        sorafs::{
            capacity::ProviderId,
            pin_registry::StorageClass,
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReserveDuration,
                ReserveFinalizedEventPageV1, ReserveMovementKindV1, ReserveMovementRecordV1,
                ReserveMovementStatusV1, ReservePolicyV1, ReserveProviderAccountV1,
                ReserveProviderTermsV1, ReserveTier,
            },
        },
        transaction::error::TransactionRejectionReason,
    },
};
use iroha_executor_data_model::permission::sorafs::CanSetSorafsReservePolicy;
use iroha_test_network::read_on_dedicated_thread;
use iroha_test_network::{Network, NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use sorafs_manifest::XorQuantity;
use tokio::{
    task::JoinSet,
    time::{Instant, sleep, timeout, timeout_at},
};

const PROVIDER: ProviderId = ProviderId::new([0xB7; 32]);
const TOP_UP: [u8; 32] = [0xB8; 32];
const WITHDRAWAL: [u8; 32] = [0xB9; 32];
const DEADLINE: Duration = Duration::from_secs(180);

fn xor(value: u64) -> XorQuantity {
    value.to_string().parse().expect("canonical integer XOR")
}
fn client(network: &Network, peer: usize, provider: bool) -> Client {
    let (account, keys) = if provider {
        (&*BOB_ID, &*BOB_KEYPAIR)
    } else {
        (&*ALICE_ID, &*ALICE_KEYPAIR)
    };
    let client = network.peers()[peer].client_for(account, keys.private_key().clone());
    integration_tests::sync::rebind_blocking_client(&client, |client| {
        client.transaction_status_timeout = DEADLINE;
        client.torii_request_timeout = Duration::from_secs(10);
        client.transaction_ttl = Some(Duration::from_secs(300));
        client.add_transaction_nonce = false;
    })
}

fn configured_policy(custody: AccountId, asset: AssetDefinitionId) -> ReserveAuthorityPolicyV1 {
    ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: asset,
        custody_account: custody,
        treasury_account: ALICE_ID.clone(),
        operations_authority: ALICE_ID.clone(),
        decision_authority: ALICE_ID.clone(),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: xor(1_000),
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    }
}
fn request(kind: ReserveMovementKindV1, revision: u64, digest: [u8; 32]) -> InstructionBox {
    let (id, amount) = match kind {
        ReserveMovementKindV1::TopUp => (TOP_UP, 100),
        ReserveMovementKindV1::Withdrawal => (WITHDRAWAL, 25),
    };
    RequestSorafsReserveMovement::new(id, PROVIDER, kind, xor(amount), revision, digest).into()
}
fn decision(id: [u8; 32], revision: u64, digest: [u8; 32]) -> InstructionBox {
    DecideSorafsReserveMovement::new(
        id,
        revision,
        digest,
        true,
        "exact reserve custody decision".to_owned(),
    )
    .into()
}

fn require_native_rejection(result: Result<HashOf<SignedTransaction>>, marker: &str) -> Result<()> {
    let error = result
        .err()
        .ok_or_else(|| eyre!("invalid reserve mutation unexpectedly committed: {marker}"))?;
    let Some(TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)),
    ))) = error.downcast_ref::<TransactionRejectionReason>()
    else {
        return Err(eyre!(
            "transport, timeout or unrelated validation cannot prove reserve rejection: {error:?}"
        ));
    };
    ensure!(
        message.contains(marker),
        "expected native {marker:?}, got {error:?}"
    );
    Ok(())
}

#[test]
fn reserve_rejection_requires_the_exact_native_error() {
    let rejected = TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            "reserve movement is already decided".to_owned(),
        )),
    ));
    require_native_rejection(Err(eyre!(rejected.clone())), "already decided").unwrap();
    assert!(require_native_rejection(Err(eyre!(rejected)), "revision conflict").is_err());
    assert!(
        require_native_rejection(Err(eyre!("transport already decided")), "already decided")
            .is_err()
    );
    assert!(
        require_native_rejection(
            Err(eyre!(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("already decided".to_owned()),
            ))),
            "already decided"
        )
        .is_err()
    );
}

async fn competing_decisions(network: &Network, instruction: InstructionBox) -> Result<()> {
    let left = client(network, 0, false);
    let right = client(network, 1, false);
    let mut left_metadata = Metadata::default();
    left_metadata.insert("reserve_decision_route".parse()?, 0u32);
    let mut right_metadata = Metadata::default();
    right_metadata.insert("reserve_decision_route".parse()?, 1u32);
    let left_transaction = prepare_transaction(&left, [instruction.clone()], left_metadata).await?;
    let right_transaction = prepare_transaction(&right, [instruction], right_metadata).await?;
    ensure!(
        left_transaction.hash() != right_transaction.hash(),
        "decisions must exercise instruction replay, not transaction deduplication"
    );
    let barrier = tokio::sync::Barrier::new(2);
    let outcomes = timeout(DEADLINE + Duration::from_secs(10), async {
        tokio::join!(
            async {
                barrier.wait().await;
                left.account_client()
                    .submit_transaction_and_wait(&left_transaction)
                    .await
            },
            async {
                barrier.wait().await;
                right
                    .account_client()
                    .submit_transaction_and_wait(&right_transaction)
                    .await
            },
        )
    })
    .await?;
    let outcomes = [outcomes.0, outcomes.1];
    ensure!(
        outcomes.iter().filter(|result| result.is_ok()).count() == 1,
        "exactly one competing decision must commit"
    );
    for outcome in outcomes {
        if outcome.is_err() {
            require_native_rejection(outcome, "already decided")?;
        }
    }
    Ok(())
}

type Observation = (
    ReserveProviderAccountV1,
    Vec<ReserveMovementRecordV1>,
    ReserveFinalizedEventPageV1,
    [Quantity; 3],
    AccountId,
);

fn observe(reader: &Client, policy: &ReserveAuthorityPolicyV1) -> Result<Observation> {
    let events = reader
        .client()
        .query_single(FindSorafsReserveEvents::new(None, None, 16))?;
    let anchor = Some(events.finalized_cursor);
    let providers = reader
        .client()
        .query_single(FindSorafsReserveProviders::new(anchor, None, 16))?;
    let movements = reader
        .client()
        .query_single(FindSorafsReserveMovements::new(anchor, None, 16))?;
    ensure!(
        providers.finalized_cursor == events.finalized_cursor
            && movements.finalized_cursor == events.finalized_cursor,
        "reserve queries must share one finalized anchor"
    );
    ensure!(
        !providers.has_more && providers.next_after.is_none() && providers.accounts.len() == 1,
        "expected exactly one complete provider partition"
    );
    ensure!(
        !movements.has_more
            && movements.next_after.is_none()
            && !events.has_more
            && events.next_after.is_none(),
        "bounded reserve query must not omit state"
    );
    let owner = reader
        .client()
        .query_single(FindSorafsProviderOwner::new(PROVIDER))?;
    let ids = [&*BOB_ID, &policy.custody_account, &*ALICE_ID]
        .map(|account| AssetId::of(policy.asset_definition.clone(), account.clone()));
    let mut balances = std::array::from_fn(|_| Quantity::zero());
    let mut found = [false; 3];
    // A completed authenticated asset query establishes zero for an absent asset;
    // an arbitrary missing/failed singular query never becomes a zero balance.
    for asset in reader.client().query(FindAssets::new()).execute_all()? {
        if let Some(index) = ids.iter().position(|id| id == asset.id()) {
            ensure!(
                !found[index],
                "asset query repeated a reserve custody partition"
            );
            found[index] = true;
            balances[index] = asset.value().clone();
        }
    }
    let after = reader
        .client()
        .query_single(FindSorafsReserveEvents::new(None, None, 16))?;
    ensure!(
        after.finalized_cursor == events.finalized_cursor,
        "finalized state advanced during reserve observation"
    );
    Ok((
        providers
            .accounts
            .into_iter()
            .next()
            .expect("one provider checked"),
        movements.movements,
        events,
        balances,
        owner,
    ))
}

async fn converged(
    network: &Network,
    policy: &ReserveAuthorityPolicyV1,
    revision: u64,
    event_count: usize,
    expected_balances: [u64; 3],
) -> Result<Observation> {
    let deadline = Instant::now() + DEADLINE;
    let expected = expected_balances.map(|amount| xor(amount).into_quantity());
    let digest = policy.digest()?;
    loop {
        let mut readers = JoinSet::new();
        for peer in 0..4 {
            let reader = client(network, peer, false);
            let policy = policy.clone();
            readers.spawn(async move {
                read_on_dedicated_thread(move || observe(&reader, &policy)).await
            });
        }
        let mut observations = Vec::new();
        while let Some(result) = timeout_at(deadline, readers.join_next()).await? {
            if let Ok(observation) = result? {
                observations.push(observation);
            }
        }
        if observations.len() == 4
            && observations.iter().all(|row| {
                row.0.revision == revision
                    && row.0.terms.provider_id == PROVIDER
                    && row.0.terms.provider_account == *BOB_ID
                    && row.0.policy_digest == digest
                    && row.2.events.len() == event_count
                    && row.3 == expected
                    && row.4 == *BOB_ID
                    && row.0.reserve_balance.clone().into_quantity() == row.3[1]
            })
        {
            let bytes = observations
                .iter()
                .map(norito::to_bytes)
                .collect::<Result<Vec<_>, _>>()?;
            if bytes.windows(2).all(|pair| pair[0] == pair[1]) {
                let observation = observations.remove(0);
                ensure!(
                    observation.3[0]
                        .checked_add(&observation.3[1])?
                        .checked_add(&observation.3[2])?
                        == xor(1_010).into_quantity(),
                    "reserve custody transfer created or destroyed value"
                );
                return Ok(observation);
            }
        }
        ensure!(
            Instant::now() < deadline,
            "four-validator reserve state did not converge at revision {revision}"
        );
        // Polling delay is never the success condition: exact query bytes and balances are.
        sleep(Duration::from_millis(250)).await;
    }
}

fn require_unchanged(before: &Observation, after: &Observation) -> Result<()> {
    ensure!(
        before.0 == after.0
            && before.1 == after.1
            && before.2.events == after.2.events
            && before.3 == after.3
            && before.4 == after.4,
        "rejection or restart changed canonical reserve state, journal, owner or balances"
    );
    ensure!(
        after.2.finalized_cursor.height >= before.2.finalized_cursor.height,
        "finalized reserve query anchor rolled back"
    );
    Ok(())
}

#[test]
fn four_peer_reserve_decisions_conserve_custody_and_survive_restart() -> Result<()> {
    super::sorafs_network::run(
        stringify!(four_peer_reserve_decisions_conserve_custody_and_survive_restart),
        four_peer_reserve_decisions_conserve_custody_and_survive_restart_impl,
    )
}

async fn four_peer_reserve_decisions_conserve_custody_and_survive_restart_impl() -> Result<()> {
    init_instruction_registry();
    let custody_keys = KeyPair::try_from_seed(
        b"sorafs-reserve-native-custody".to_vec(),
        Algorithm::Ed25519,
    )?;
    let custody = AccountId::new(custody_keys.public_key().clone());
    let domain = DomainId::try_new("reserve", "universal")?;
    let asset_id = AssetDefinitionId::derive_from_components(domain.clone(), "xor".parse()?);
    let policy = configured_policy(custody.clone(), asset_id.clone());
    policy.validate()?;
    let digest = policy.digest()?;
    let owner = BOB_ID.canonical_i105()?;
    let mut owners = toml::map::Map::new();
    owners.insert(hex::encode(PROVIDER.as_bytes()), toml::Value::String(owner));
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(1))
        .with_npos_consensus()
        .with_config_layer(move |layer| {
            layer.write(
                ["governance", "sorafs_provider_owners"],
                toml::Value::Table(owners.clone()),
            );
        })
        .with_genesis_instruction(Register::account(Account::new(custody)))
        .with_genesis_instruction(Register::domain(Domain::new(domain)))
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            asset_id.clone(),
            "Reserve XOR".to_owned(),
            iroha::data_model::asset::AssetBalancePolicy::Global,
            None,
        )))
        .with_genesis_instruction(Mint::asset_quantity(
            xor(1_000).into_quantity(),
            AssetId::of(asset_id.clone(), BOB_ID.clone()),
        ))
        .with_genesis_instruction(Mint::asset_quantity(
            xor(10).into_quantity(),
            AssetId::of(asset_id, ALICE_ID.clone()),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanSetSorafsReservePolicy),
            ALICE_ID.clone(),
        ));
    let context = stringify!(four_peer_reserve_decisions_conserve_custody_and_survive_restart);
    let builder = super::sorafs_network::bounded_storage(builder);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    ensure!(
        network.peers().len() == 4,
        "reserve qualification requires four validators"
    );
    for peer in 0..4 {
        let reader = client(&network, peer, false);
        let owner = read_on_dedicated_thread(move || {
            Ok(reader
                .client()
                .query_single(FindSorafsProviderOwner::new(PROVIDER))?)
        })
        .await?;
        ensure!(
            owner == *BOB_ID,
            "canonical genesis provider owner differs across validators"
        );
    }
    let policy_instruction: InstructionBox = SetSorafsReservePolicy::new(policy.clone()).into();
    require_native_rejection(
        submit_instruction(&client(&network, 2, true), policy_instruction.clone()).await,
        "CanSetSorafsReservePolicy",
    )?;
    submit_instruction(&client(&network, 0, false), policy_instruction).await?;
    let registration: InstructionBox = RegisterSorafsReserveAccount::new(
        ReserveProviderTermsV1 {
            provider_id: PROVIDER,
            provider_account: BOB_ID.clone(),
            tier: ReserveTier::TierA,
            storage_class: StorageClass::Hot,
            duration: ReserveDuration::Monthly,
            capacity_gib: 1,
        },
        digest,
    )
    .into();
    require_native_rejection(
        submit_instruction(&client(&network, 0, true), registration.clone()).await,
        "governed operations account",
    )?;
    submit_instruction(&client(&network, 0, false), registration).await?;
    let before = converged(&network, &policy, 1, 2, [1_000, 0, 10]).await?;
    let top_up = request(ReserveMovementKindV1::TopUp, 1, digest);
    require_native_rejection(
        submit_instruction(&client(&network, 0, false), top_up.clone()).await,
        "not the provider account",
    )?;
    require_unchanged(
        &before,
        &converged(&network, &policy, 1, 2, [1_000, 0, 10]).await?,
    )?;
    submit_instruction(&client(&network, 0, true), top_up.clone()).await?;
    let pending = converged(&network, &policy, 2, 3, [1_000, 0, 10]).await?;
    ensure!(
        pending.0.pending_movements == 1
            && pending.1.len() == 1
            && pending.1[0].status == ReserveMovementStatusV1::Pending,
        "request must reserve one decision without moving custody"
    );
    require_native_rejection(
        submit_instruction(&client(&network, 2, true), top_up).await,
        "already recorded",
    )?;
    require_native_rejection(
        submit_instruction(&client(&network, 2, true), decision(TOP_UP, 2, digest)).await,
        "governed decision account",
    )?;
    require_unchanged(
        &pending,
        &converged(&network, &policy, 2, 3, [1_000, 0, 10]).await?,
    )?;
    competing_decisions(&network, decision(TOP_UP, 2, digest)).await?;
    let funded = converged(&network, &policy, 3, 4, [900, 100, 10]).await?;
    require_native_rejection(
        submit_instruction(&client(&network, 2, false), decision(TOP_UP, 3, digest)).await,
        "already decided",
    )?;
    require_native_rejection(
        submit_instruction(
            &client(&network, 2, true),
            ChargeSorafsReserveRent::new(PROVIDER, 3, 1, digest),
        )
        .await,
        "governed operations account",
    )?;
    require_native_rejection(
        submit_instruction(
            &client(&network, 2, false),
            ChargeSorafsReserveRent::new(PROVIDER, 3, 1, digest),
        )
        .await,
        "only 0 are due",
    )?;
    require_unchanged(
        &funded,
        &converged(&network, &policy, 3, 4, [900, 100, 10]).await?,
    )?;
    submit_instruction(
        &client(&network, 0, true),
        request(ReserveMovementKindV1::Withdrawal, 3, digest),
    )
    .await?;
    let pending = converged(&network, &policy, 4, 5, [900, 100, 10]).await?;
    require_native_rejection(
        submit_instruction(&client(&network, 2, false), decision(WITHDRAWAL, 3, digest)).await,
        "revision conflict",
    )?;
    require_unchanged(
        &pending,
        &converged(&network, &policy, 4, 5, [900, 100, 10]).await?,
    )?;
    competing_decisions(&network, decision(WITHDRAWAL, 4, digest)).await?;
    let finished = converged(&network, &policy, 5, 6, [925, 75, 10]).await?;
    ensure!(
        finished.0.pending_movements == 0
            && finished.0.debt_principal.is_zero()
            && finished.0.accrued_interest.is_zero(),
        "decisions changed reserve debt or retained pending custody"
    );
    ensure!(
        finished.1.len() == 2
            && finished.1.iter().all(|movement| movement.status
                == ReserveMovementStatusV1::Approved
                && movement.requested_by == *BOB_ID
                && movement.decided_by.as_ref() == Some(&*ALICE_ID)
                && movement.policy_digest == digest),
        "movement authority or exact terminal decisions differ"
    );
    for (id, kind, amount, revision) in [
        (TOP_UP, ReserveMovementKindV1::TopUp, 100, 1),
        (WITHDRAWAL, ReserveMovementKindV1::Withdrawal, 25, 3),
    ] {
        let movement = finished
            .1
            .iter()
            .find(|movement| movement.movement_id == id)
            .ok_or_else(|| eyre!("missing approved reserve movement {}", hex::encode(id)))?;
        ensure!(
            movement.provider_id == PROVIDER
                && movement.kind == kind
                && movement.amount == xor(amount)
                && movement.expected_provider_revision == revision
                && movement
                    .decided_at_unix
                    .is_some_and(|decided| decided >= movement.requested_at_unix),
            "terminal movement differs from the exact requested custody transfer"
        );
    }
    ensure!(
        finished
            .2
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>()
            == vec![1, 2, 3, 4, 5, 6],
        "reserve journal contains repeated or missing transitions"
    );
    ensure!(
        finished
            .2
            .events
            .iter()
            .map(|event| event.event.kind)
            .collect::<Vec<_>>()
            == vec![
                SorafsReserveLedgerEventKind::PolicyActivated,
                SorafsReserveLedgerEventKind::ProviderRegistered,
                SorafsReserveLedgerEventKind::MovementRequested,
                SorafsReserveLedgerEventKind::MovementApproved,
                SorafsReserveLedgerEventKind::MovementRequested,
                SorafsReserveLedgerEventKind::MovementApproved,
            ],
        "reserve journal contains an unauthorized custody or rent event"
    );
    require_native_rejection(
        submit_instruction(&client(&network, 2, false), decision(WITHDRAWAL, 5, digest)).await,
        "already decided",
    )?;
    let before_restart = converged(&network, &policy, 5, 6, [925, 75, 10]).await?;
    require_unchanged(&finished, &before_restart)?;
    let peer = network.peers()[3].clone();
    let config = network.config_layers().collect::<Vec<_>>();
    ensure!(
        peer.shutdown_if_started().await,
        "same-peer restart requires a running validator"
    );
    timeout(
        network.peer_startup_timeout(),
        peer.start_checked(config.iter(), None),
    )
    .await??;
    timeout(
        network.sync_timeout(),
        peer.once_block(before_restart.2.finalized_cursor.height),
    )
    .await?;
    let after_restart = converged(&network, &policy, 5, 6, [925, 75, 10]).await?;
    require_unchanged(&before_restart, &after_restart)?;
    Ok(())
}
