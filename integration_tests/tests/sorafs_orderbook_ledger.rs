//! Four-validator signed order admission, competing matches, custody settlement and restart.
//!
//! Provider ownership uses the documented production pre-genesis configuration. All subsequent
//! policies, reserve funding, orders, matching and settlement use signed native instructions.
//! Provider delivery, owner-governance transitions, partial fills and expiry remain separate tests.

use std::{
    sync::{Arc, Barrier},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::{Client, QueryError},
    crypto::{Algorithm, HashOf, KeyPair, Signature},
    data_model::{
        escrow::{AssetEscrowRecord, AssetEscrowStatus},
        events::data::sorafs::SorafsOrderbookLedgerEventKind,
        isi::{
            error::{InstructionExecutionError, InvalidParameterError},
            sorafs::{
                AdvanceSorafsReserveLifecycle, DecideSorafsReserveMovement, MatchSorafsOrderbook,
                RecordSorafsOrderbookSettlementReceipt, RegisterSorafsReserveAccount,
                RequestSorafsReserveMovement, SetSorafsOrderbookPolicy, SetSorafsReservePolicy,
                SubmitSorafsOrderbookOrder,
            },
        },
        prelude::*,
        query::{
            error::{FindError, QueryExecutionFail},
            escrow::prelude::FindAssetEscrowById,
        },
        sorafs::{
            capacity::ProviderId,
            orderbook::{
                ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyV1,
                OrderbookFinalizedEventPageV1, OrderbookLedgerStatusV1, OrderbookOrderPageV1,
                OrderbookOrderStatusV1, OrderbookSettlementChannelPageV1,
                OrderbookSettlementChannelStatusV1, OrderbookSettlementReceiptPageV1,
                OrderbookTradePageV1, orderbook_order_escrow_id, orderbook_settlement_escrow_id,
            },
            pin_registry::StorageClass,
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReserveDuration,
                ReserveLifecycleStage, ReserveMovementKindV1, ReservePolicyV1,
                ReserveProviderTermsV1, ReserveTier,
            },
        },
        transaction::{FeePaymentIntent, error::TransactionRejectionReason},
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanSetSorafsPricing, CanSetSorafsReservePolicy,
};
use iroha_test_network::{Network, NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use sorafs_manifest::{
    XorQuantity,
    orderbook::{
        ByteRangeV1, ORDERBOOK_ORDER_VERSION_V1, OrderRequestV1, OrderSideV1, OrderTierV1,
        OrderbookSignatureV1, SETTLEMENT_RECEIPT_VERSION_V1, SettlementReceiptV1, TradeEventV1,
        bid_order_escrow_requirement_v1, derive_orderbook_order_id_v1,
        deterministic_settlement_split_v1, order_request_signature_digest_v1,
        settlement_receipt_signature_digest_v1, trade_escrow_requirement_v1,
        trade_fee_requirement_v1,
    },
    provider_advert::SignatureAlgorithm,
};
use tokio::{
    task::JoinSet,
    time::{Instant, sleep, timeout, timeout_at},
};

const PROVIDER: ProviderId = ProviderId::new([0xB6; 32]);
const DEADLINE: Duration = Duration::from_secs(180);
const BYTES: u64 = 2 << 30;

fn no_fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}

fn reader(network: &Network, peer: usize, account: &AccountId, keys: &KeyPair) -> Client {
    let mut client = network.peers()[peer].client_for(account, keys.private_key().clone());
    client.transaction_status_timeout = DEADLINE;
    client.torii_request_timeout = Duration::from_secs(5);
    client.transaction_ttl = Some(Duration::from_secs(300));
    client.add_transaction_nonce = false;
    client
}

fn signing_material(keys: &KeyPair) -> Result<OrderbookSignatureV1> {
    let (_, public_key) = keys.public_key().try_to_bytes()?;
    Ok(OrderbookSignatureV1 {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: public_key.to_vec(),
        signature: Vec::new(),
    })
}

fn signed_order(keys: &KeyPair, side: OrderSideV1, expiry: u64) -> Result<OrderRequestV1> {
    let owner_account = AccountId::new(keys.public_key().clone())
        .to_string()
        .into_bytes();
    let mut order = OrderRequestV1 {
        version: ORDERBOOK_ORDER_VERSION_V1,
        order_id: derive_orderbook_order_id_v1(&owner_account, 1),
        side,
        tier: OrderTierV1::Hot,
        price_per_gib: XorQuantity::try_from_micro(if side == OrderSideV1::Bid {
            1_000_000
        } else {
            900_000
        })?,
        quantity_gib: 2,
        remaining_gib: 2,
        owner_account,
        provider_id: (side == OrderSideV1::Ask).then_some(*PROVIDER.as_bytes()),
        expiry_unix: expiry,
        nonce: 1,
        maker_fee_bps: 100,
        taker_fee_bps: 200,
        signature: signing_material(keys)?,
    };
    order.signature.signature = Signature::try_new(
        keys.private_key(),
        &order_request_signature_digest_v1(&order)?,
    )?
    .payload()
    .to_vec();
    Ok(order)
}

fn require_native_rejection(result: Result<HashOf<SignedTransaction>>, marker: &str) -> Result<()> {
    let error = result
        .err()
        .ok_or_else(|| eyre!("expected native rejection: {marker}"))?;
    let Some(TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)),
    ))) = error.downcast_ref::<TransactionRejectionReason>()
    else {
        return Err(eyre!(
            "transport, timeout or unrelated validation cannot prove {marker}: {error:?}"
        ));
    };
    ensure!(
        message.contains(marker),
        "unexpected native rejection for {marker}: {error:?}"
    );
    Ok(())
}

async fn race(
    left: Client,
    right: Client,
    instruction: InstructionBox,
) -> Result<[Result<HashOf<SignedTransaction>>; 2]> {
    let mut left_metadata = Metadata::default();
    left_metadata.insert("orderbook_race_route".parse()?, 0u32);
    let mut right_metadata = Metadata::default();
    right_metadata.insert("orderbook_race_route".parse()?, 1u32);
    let left_transaction =
        left.try_build_transaction([instruction.clone()], no_fee(), left_metadata)?;
    let right_transaction = right.try_build_transaction([instruction], no_fee(), right_metadata)?;
    ensure!(
        left_transaction.hash() != right_transaction.hash(),
        "race must not collapse into transaction deduplication"
    );
    let barrier = Arc::new(Barrier::new(2));
    let left_barrier = Arc::clone(&barrier);
    let (left, right) = timeout(DEADLINE + Duration::from_secs(10), async {
        tokio::try_join!(
            tokio::task::spawn_blocking(move || {
                left_barrier.wait();
                left.submit_transaction_blocking(&left_transaction)
            }),
            tokio::task::spawn_blocking(move || {
                barrier.wait();
                right.submit_transaction_blocking(&right_transaction)
            }),
        )
    })
    .await??;
    Ok([left, right])
}

fn asset_balance(
    client: &Client,
    definition: &AssetDefinitionId,
    account: &AccountId,
) -> Result<Quantity> {
    match client.query_single(FindAssetById::new(AssetId::of(
        definition.clone(),
        account.clone(),
    ))) {
        Ok(asset) => Ok(asset.value().clone()),
        Err(QueryError::Validation(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Asset(_),
        )))) => Ok(Quantity::zero()),
        Err(error) => Err(eyre!(error)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, norito::NoritoSerialize)]
struct Observation {
    orders: OrderbookOrderPageV1,
    trades: OrderbookTradePageV1,
    channels: OrderbookSettlementChannelPageV1,
    receipts: OrderbookSettlementReceiptPageV1,
    events: OrderbookFinalizedEventPageV1,
    status: OrderbookLedgerStatusV1,
    parent: AssetEscrowRecord,
    child: Option<AssetEscrowRecord>,
    // Alice, Bob, treasury, reserve custody, parent custody, channel custody.
    balances: Vec<Quantity>,
    provider_owner: AccountId,
}

impl Observation {
    fn business_bytes(&self) -> Result<Vec<u8>> {
        Ok(norito::encode_canonical(&(
            self.orders.orders.clone(),
            self.trades.trades.clone(),
            self.channels.channels.clone(),
            self.receipts.receipts.clone(),
            self.events.events.clone(),
            self.status.clone(),
            self.parent.clone(),
            self.child.clone(),
            self.balances.clone(),
            self.provider_owner.clone(),
        ))?)
    }
}

fn observe(
    client: &Client,
    definition: &AssetDefinitionId,
    manager: &AccountId,
    treasury: &AccountId,
    bid_id: [u8; 32],
) -> Result<Observation> {
    let orders = client.query_single(FindSorafsOrderbookOrders::new(None, None, None, 8))?;
    let anchor = Some(orders.finalized_cursor);
    let trades = client.query_single(FindSorafsOrderbookTrades::new(anchor, None, 8))?;
    let channels = client.query_single(FindSorafsOrderbookChannels::new(anchor, None, None, 8))?;
    let receipts = client.query_single(FindSorafsOrderbookReceipts::new(anchor, None, None, 8))?;
    let events = client.query_single(FindSorafsOrderbookEvents::new(anchor, None, 16))?;
    ensure!(
        trades.finalized_cursor == orders.finalized_cursor
            && channels.finalized_cursor == orders.finalized_cursor
            && receipts.finalized_cursor == orders.finalized_cursor
            && events.finalized_cursor == orders.finalized_cursor,
        "all orderbook pages must share one finalized anchor"
    );
    ensure!(
        !orders.has_more
            && !trades.has_more
            && !channels.has_more
            && !receipts.has_more
            && !events.has_more,
        "bounded scenario must retain complete pages"
    );
    let parent =
        client.query_single(FindAssetEscrowById::new(orderbook_order_escrow_id(bid_id)))?;
    let child = channels
        .channels
        .first()
        .map(|channel| {
            client.query_single(FindAssetEscrowById::new(orderbook_settlement_escrow_id(
                channel.channel_id,
            )))
        })
        .transpose()?;
    let mut balances = vec![];
    for account in [&*ALICE_ID, &*BOB_ID, treasury, manager, &parent.custody] {
        balances.push(asset_balance(client, definition, account)?);
    }
    balances.push(match child.as_ref() {
        Some(child) => asset_balance(client, definition, &child.custody)?,
        None => Quantity::zero(),
    });
    let status = client.query_single(FindSorafsOrderbookStatus)?;
    let provider_owner = client.query_single(FindSorafsProviderOwner::new(PROVIDER))?;
    let after = client.query_single(FindSorafsOrderbookEvents::new(None, None, 16))?;
    ensure!(
        after.finalized_cursor == orders.finalized_cursor,
        "finalized state advanced during custody observation"
    );
    Ok(Observation {
        orders,
        trades,
        channels,
        receipts,
        events,
        parent,
        child,
        balances,
        status,
        provider_owner,
    })
}

async fn converged(
    network: &Network,
    definition: &AssetDefinitionId,
    manager: &AccountId,
    treasury: &AccountId,
    bid_id: [u8; 32],
    counts: (usize, u64, u64, usize),
) -> Result<Observation> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        let mut tasks = JoinSet::new();
        for peer in 0..4 {
            let client = reader(network, peer, &ALICE_ID, &ALICE_KEYPAIR);
            let definition = definition.clone();
            let manager = manager.clone();
            let treasury = treasury.clone();
            tasks
                .spawn_blocking(move || observe(&client, &definition, &manager, &treasury, bid_id));
        }
        let mut observations = vec![];
        while let Some(result) = timeout_at(deadline, tasks.join_next()).await? {
            if let Ok(observation) = result? {
                observations.push(observation);
            }
        }
        if observations.len() == 4
            && observations.iter().all(|observation| {
                observation.orders.orders.len() == counts.0
                    && observation.status.book_revision == counts.1
                    && observation.status.settlement_receipts == counts.2
                    && observation.events.events.len() == counts.3
            })
            && observations.windows(2).all(|pair| pair[0] == pair[1])
        {
            return Ok(observations.remove(0));
        }
        ensure!(
            Instant::now() < deadline,
            "four peers did not converge to orderbook counts {counts:?}"
        );
        sleep(Duration::from_millis(100)).await;
    }
}

fn require_conservation(observation: &Observation) -> Result<()> {
    let total = observation
        .balances
        .iter()
        .try_fold(Quantity::zero(), |sum, value| sum.checked_add(value))?;
    ensure!(
        total == Quantity::from(307u32),
        "orderbook and reserve custody changed total minted value: {total}"
    );
    ensure!(
        observation.balances[4] == observation.parent.remaining_amount,
        "parent custody and native escrow diverged"
    );
    if let Some(child) = &observation.child {
        ensure!(
            observation.balances[5] == child.remaining_amount,
            "channel custody and native escrow diverged"
        );
    }
    ensure!(
        observation.provider_owner == *BOB_ID,
        "provider ownership changed outside governance"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn four_peer_orderbook_match_settlement_and_restart_are_authoritative() -> Result<()> {
    init_instruction_registry();
    let manager_keys = KeyPair::try_from_seed(vec![0xB7; 32], Algorithm::Ed25519)?;
    let manager = AccountId::new(manager_keys.public_key().clone());
    let treasury_keys = KeyPair::try_from_seed(vec![0xB8; 32], Algorithm::Ed25519)?;
    let treasury = AccountId::new(treasury_keys.public_key().clone());
    let domain = DomainId::try_new("sorafsorderbook", "universal")?;
    let definition = AssetDefinitionId::derive_from_components(domain.clone(), "xor".parse()?);
    let definition_literal = definition.to_string();
    let treasury_literal = treasury.to_string();
    let owner_literal = BOB_ID.to_string();
    let mut builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_block_cadence(Duration::from_secs(1))
        .with_npos_consensus()
        .with_config_layer(move |layer| {
            // This supported bootstrap seeds only an empty chain; restoration preserves ledger ownership.
            layer
                .write(
                    ["governance", "sorafs_provider_owners"],
                    toml::Value::Table(toml::Table::from_iter([(
                        hex::encode(PROVIDER.as_bytes()),
                        toml::Value::String(owner_literal.clone()),
                    )])),
                )
                .write(
                    ["governance", "sorafs_pin_fee_asset_id"],
                    definition_literal.clone(),
                )
                .write(
                    ["governance", "sorafs_pin_fee_treasury_account"],
                    treasury_literal.clone(),
                );
        })
        .with_genesis_instruction(Register::account(Account::new(manager.clone())))
        .with_genesis_instruction(Register::account(Account::new(treasury.clone())))
        .with_genesis_instruction(Register::domain(Domain::new(domain)))
        .with_genesis_instruction(Register::asset_definition(AssetDefinition::numeric(
            definition.clone(),
            "XOR".to_owned(),
            iroha::data_model::asset::AssetBalancePolicy::Global,
            None,
        )))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanSetSorafsPricing),
            manager.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanSetSorafsReservePolicy),
            manager.clone(),
        ));
    for (account, amount) in [
        (ALICE_ID.clone(), 100u32),
        (BOB_ID.clone(), 200),
        (treasury.clone(), 7),
    ] {
        builder = builder.with_genesis_instruction(Mint::asset_quantity(
            amount,
            AssetId::of(definition.clone(), account),
        ));
    }
    let context = stringify!(four_peer_orderbook_match_settlement_and_restart_are_authoritative);
    let network = sandbox::start_network_async_or_skip(builder, context).await?;
    let Some(network) = sandbox::enforce_network_start_requirement(network, context)? else {
        return Ok(());
    };
    ensure!(
        network.peers().len() == 4,
        "orderbook requires four validators"
    );
    for peer in 0..4 {
        ensure!(
            reader(&network, peer, &ALICE_ID, &ALICE_KEYPAIR)
                .query_single(FindSorafsProviderOwner::new(PROVIDER))?
                == *BOB_ID,
            "trusted genesis provider binding missing"
        );
    }
    let governor = reader(&network, 0, &manager, &manager_keys);
    // Sequential setup shares the submitting peer's Applied confirmation. The
    // deliberate cross-peer races begin only after all four query views agree.
    let buyer = reader(&network, 0, &ALICE_ID, &ALICE_KEYPAIR);
    let provider = reader(&network, 0, &BOB_ID, &BOB_KEYPAIR);
    let reserve_policy = ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: definition.clone(),
        custody_account: manager.clone(),
        treasury_account: treasury.clone(),
        operations_authority: manager.clone(),
        decision_authority: manager.clone(),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: "1000".parse()?,
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    };
    let reserve_digest = reserve_policy.digest()?;
    governor.submit_blocking(SetSorafsReservePolicy::new(reserve_policy), no_fee())?;
    governor.submit_blocking(
        RegisterSorafsReserveAccount::new(
            ReserveProviderTermsV1 {
                provider_id: PROVIDER,
                provider_account: BOB_ID.clone(),
                tier: ReserveTier::TierA,
                storage_class: StorageClass::Hot,
                duration: ReserveDuration::Monthly,
                capacity_gib: 2,
            },
            reserve_digest,
        ),
        no_fee(),
    )?;
    provider.submit_blocking(
        RequestSorafsReserveMovement::new(
            [0xB9; 32],
            PROVIDER,
            ReserveMovementKindV1::TopUp,
            "100".parse()?,
            1,
            reserve_digest,
        ),
        no_fee(),
    )?;
    governor.submit_blocking(
        DecideSorafsReserveMovement::new(
            [0xB9; 32],
            2,
            reserve_digest,
            true,
            "fund provider underwriting".to_owned(),
        ),
        no_fee(),
    )?;
    governor.submit_blocking(
        AdvanceSorafsReserveLifecycle::new(PROVIDER, 3, 0, reserve_digest),
        no_fee(),
    )?;
    let reserve = buyer.query_single(FindSorafsReserveProviderById::new(PROVIDER))?;
    ensure!(
        reserve.reserve_balance == "100".parse()?
            && reserve.lifecycle_stage == ReserveLifecycleStage::Active,
        "provider reserve must be genuinely funded and active before matching"
    );

    let policy = OrderbookAdmissionPolicyV1 {
        version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        market_id: [0xBA; 32],
        matcher_authority: manager.clone(),
        settlement_authority: manager.clone(),
        paused: false,
        min_order_gib: 2,
        max_order_gib: 1024,
        price_tick_micro_xor: 10,
        max_maker_fee_bps: 100,
        max_taker_fee_bps: 200,
        max_order_lifetime_secs: 3600,
        max_receipt_age_secs: 3600,
        max_clock_skew_secs: 5,
        max_receipt_bytes: BYTES,
        max_receipts_per_channel: 2,
    };
    let digest = policy.digest()?;
    require_native_rejection(
        provider.submit_blocking(SetSorafsOrderbookPolicy::new(policy.clone()), no_fee()),
        "CanSetSorafsPricing",
    )?;
    governor.submit_blocking(SetSorafsOrderbookPolicy::new(policy), no_fee())?;
    let expiry = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 1800;
    let bid = signed_order(&ALICE_KEYPAIR, OrderSideV1::Bid, expiry)?;
    let ask = signed_order(&BOB_KEYPAIR, OrderSideV1::Ask, expiry)?;
    let bid_instruction = SubmitSorafsOrderbookOrder::new(norito::encode_canonical(&bid)?, digest);
    require_native_rejection(
        provider.submit_blocking(bid_instruction.clone(), no_fee()),
        "does not match transaction authority",
    )?;
    buyer.submit_blocking(bid_instruction, no_fee())?;
    let admitted = converged(
        &network,
        &definition,
        &manager,
        &treasury,
        bid.order_id,
        (1, 1, 0, 2),
    )
    .await?;
    require_conservation(&admitted)?;
    let initial_lock = bid_order_escrow_requirement_v1(&bid, 100, 200)?.into_quantity();
    ensure!(
        admitted.parent.status == AssetEscrowStatus::Locked
            && admitted.parent.remaining_amount == initial_lock,
        "bid admission must atomically fund the conservative custody lock"
    );
    ensure!(
        admitted.balances[0] == Quantity::from(100u32).checked_sub(&initial_lock)?,
        "buyer balance did not fund the bid lock"
    );
    provider.submit_blocking(
        SubmitSorafsOrderbookOrder::new(norito::encode_canonical(&ask)?, digest),
        no_fee(),
    )?;
    let before_match = converged(
        &network,
        &definition,
        &manager,
        &treasury,
        bid.order_id,
        (2, 2, 0, 3),
    )
    .await?;
    let matching: InstructionBox =
        MatchSorafsOrderbook::new(digest, before_match.status.book_revision, 1).into();
    require_native_rejection(
        buyer.submit_blocking(matching.clone(), no_fee()),
        "governed authority",
    )?;
    let outcomes = race(
        reader(&network, 0, &manager, &manager_keys),
        reader(&network, 3, &manager, &manager_keys),
        matching,
    )
    .await?;
    ensure!(
        outcomes.iter().filter(|outcome| outcome.is_ok()).count() == 1,
        "exactly one competing match must commit"
    );
    for outcome in outcomes {
        if outcome.is_err() {
            require_native_rejection(outcome, "revision conflict")?;
        }
    }
    let matched = converged(
        &network,
        &definition,
        &manager,
        &treasury,
        bid.order_id,
        (2, 3, 0, 4),
    )
    .await?;
    require_conservation(&matched)?;
    ensure!(
        matched.status.trades == 1
            && matched.status.filled_orders == 2
            && matched.status.open_orders == 0
            && matched.status.open_settlement_channels == 1,
        "competing matches duplicated trade or channel counters"
    );
    ensure!(matched.orders.orders.iter().all(|order| order.status == OrderbookOrderStatusV1::Filled && order.remaining_gib == 0), "full match did not close both signed orders");
    ensure!(
        matched.trades.trades.len() == 1 && matched.channels.channels.len() == 1,
        "match must create one immutable trade and channel"
    );
    let trade_record = &matched.trades.trades[0];
    let trade: TradeEventV1 = norito::decode_canonical(&trade_record.canonical_trade)?;
    let channel = &matched.channels.channels[0];
    ensure!(
        trade.maker_order_id == bid.order_id
            && trade.taker_order_id == ask.order_id
            && trade.filled_gib == 2
            && trade.price_per_gib == bid.price_per_gib,
        "price-time matching selected the wrong signed orders or maker price"
    );
    ensure!(
        trade_record.channel_id == channel.channel_id
            && trade_record.book_revision == 3
            && channel.buyer == *ALICE_ID
            && channel.provider == *BOB_ID
            && channel.provider_id == PROVIDER
            && channel.settlement_authority == manager
            && channel.total_bytes == BYTES,
        "trade/channel authority bindings differ from admitted orders"
    );
    ensure!(
        channel.initial_xor_locked == trade_escrow_requirement_v1(&trade)?
            && channel.initial_fee_xor_locked == trade_fee_requirement_v1(&trade)?,
        "channel custody differs from immutable trade price and fees"
    );
    let child = matched
        .child
        .as_ref()
        .ok_or_else(|| eyre!("match omitted child escrow"))?;
    ensure!(
        matched.parent.status == AssetEscrowStatus::DrawnDown
            && matched.parent.remaining_amount.is_zero()
            && child.status == AssetEscrowStatus::Locked
            && child.remaining_amount == channel.remaining_xor_locked.clone().into_quantity()
            && child.release_authority.as_ref() == Some(&manager),
        "match did not partition exact native custody and refund the closed parent"
    );

    let split = deterministic_settlement_split_v1(
        &channel.remaining_xor_locked,
        &channel.remaining_fee_xor_locked,
        BYTES,
        BYTES,
    )?;
    let mut receipt = SettlementReceiptV1 {
        version: SETTLEMENT_RECEIPT_VERSION_V1,
        receipt_id: [0xBB; 32],
        channel_id: channel.channel_id,
        trade_id: trade.trade_id,
        range: ByteRangeV1 {
            start: 0,
            end: BYTES,
        },
        chunk_hash: [0xBC; 32],
        bytes_delivered: BYTES,
        xor_debited: split.xor_debited.clone(),
        provider_credit: split.provider_credit.clone(),
        fee_amount: split.fee_amount.clone(),
        issued_at_unix: channel.opened_at_unix,
        settlement_signature: signing_material(&BOB_KEYPAIR)?,
    };
    receipt.settlement_signature.signature = Signature::try_new(
        BOB_KEYPAIR.private_key(),
        &settlement_receipt_signature_digest_v1(&receipt)?,
    )?
    .payload()
    .to_vec();
    let settlement: InstructionBox =
        RecordSorafsOrderbookSettlementReceipt::new(norito::encode_canonical(&receipt)?, digest)
            .into();
    // Independent Alice relayers exercise the provider signature and immutable native release authority.
    let outcomes = race(
        reader(&network, 1, &ALICE_ID, &ALICE_KEYPAIR),
        reader(&network, 2, &ALICE_ID, &ALICE_KEYPAIR),
        settlement,
    )
    .await?;
    ensure!(
        outcomes.iter().filter(|outcome| outcome.is_ok()).count() == 1,
        "one receipt must cause exactly one custody release"
    );
    for outcome in outcomes {
        if outcome.is_err() {
            require_native_rejection(outcome, "channel is not open")?;
        }
    }
    let settled = converged(
        &network,
        &definition,
        &manager,
        &treasury,
        bid.order_id,
        (2, 3, 1, 5),
    )
    .await?;
    require_conservation(&settled)?;
    ensure!(
        settled.channels.channels[0].status == OrderbookSettlementChannelStatusV1::Closed
            && settled.channels.channels[0].remaining_bytes == 0
            && settled.channels.channels[0].remaining_xor_locked.is_zero()
            && settled.status.open_settlement_channels == 0,
        "receipt did not close byte and custody balances"
    );
    ensure!(
        settled
            .child
            .as_ref()
            .is_some_and(|child| child.status == AssetEscrowStatus::DrawnDown
                && child.remaining_amount.is_zero()),
        "settlement left native channel custody open"
    );
    ensure!(
        settled.balances[0]
            == Quantity::from(100u32).checked_sub(split.xor_debited.as_quantity())?
            && settled.balances[1]
                == Quantity::from(100u32).checked_add(split.provider_credit.as_quantity())?
            && settled.balances[2]
                == Quantity::from(7u32).checked_add(split.fee_amount.as_quantity())?
            && settled.balances[3] == Quantity::from(100u32),
        "provider/fee release or refund altered the wrong account"
    );
    ensure!(
        settled.receipts.receipts.len() == 1
            && settled.receipts.receipts[0].canonical_receipt
                == norito::encode_canonical(&receipt)?
            && settled.receipts.receipts[0].recorded_by == *ALICE_ID,
        "settlement replay replaced immutable receipt provenance"
    );
    ensure!(
        settled
            .events
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>()
            == vec![1, 2, 3, 4, 5],
        "committed journal duplicated or omitted a transition"
    );
    ensure!(
        settled
            .events
            .events
            .iter()
            .map(|event| event.event.kind)
            .collect::<Vec<_>>()
            == vec![
                SorafsOrderbookLedgerEventKind::PolicyActivated,
                SorafsOrderbookLedgerEventKind::OrderAdmitted,
                SorafsOrderbookLedgerEventKind::OrderAdmitted,
                SorafsOrderbookLedgerEventKind::TradeMatched,
                SorafsOrderbookLedgerEventKind::ReceiptRecorded
            ],
        "committed journal has unexpected native transition kinds"
    );

    let peer = network.peers()[3].clone();
    let config = network.config_layers().collect::<Vec<_>>();
    ensure!(
        peer.shutdown_if_started().await,
        "restart peer was not running"
    );
    timeout(
        network.peer_startup_timeout(),
        peer.start_checked(config.iter(), None),
    )
    .await??;
    timeout(
        network.sync_timeout(),
        peer.once_block(settled.orders.finalized_cursor.height),
    )
    .await?;
    let restored = converged(
        &network,
        &definition,
        &manager,
        &treasury,
        bid.order_id,
        (2, 3, 1, 5),
    )
    .await?;
    require_conservation(&restored)?;
    ensure!(
        restored.orders.finalized_cursor.height >= settled.orders.finalized_cursor.height
            && restored.business_bytes()? == settled.business_bytes()?,
        "cold peer recovery changed finalized orders, custody, balances or journal"
    );
    require_native_rejection(
        reader(&network, 3, &ALICE_ID, &ALICE_KEYPAIR).submit_blocking(
            RecordSorafsOrderbookSettlementReceipt::new(
                norito::encode_canonical(&receipt)?,
                digest,
            ),
            no_fee(),
        ),
        "channel is not open",
    )?;
    let after_replay = converged(
        &network,
        &definition,
        &manager,
        &treasury,
        bid.order_id,
        (2, 3, 1, 5),
    )
    .await?;
    require_conservation(&after_replay)?;
    ensure!(
        after_replay.business_bytes()? == restored.business_bytes()?,
        "receipt replay after restart changed custody or terminal journal state"
    );
    Ok(())
}

#[test]
fn orderbook_rejection_requires_the_exact_native_error() {
    let error = TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            "orderbook revision conflict".to_owned(),
        )),
    ));
    require_native_rejection(Err(eyre!(error.clone())), "revision conflict").unwrap();
    assert!(require_native_rejection(Err(eyre!(error)), "governed authority").is_err());
    assert!(
        require_native_rejection(
            Err(eyre!("transport revision conflict")),
            "revision conflict"
        )
        .is_err()
    );
}
