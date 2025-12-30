//! Stress block production with randomized mint bursts.

use std::{
    num::NonZero,
    time::{Duration, Instant},
};

use eyre::{Result, eyre};
use futures_util::StreamExt;
use integration_tests::sandbox;
use iroha::data_model::{
    events::pipeline::{BlockEventFilter, TransactionEventFilter},
    parameter::BlockParameter,
    prelude::*,
};
use iroha_test_network::*;
use iroha_test_samples::gen_account_in;
use rand::{SeedableRng, prelude::IteratorRandom};
use rand_chacha::ChaCha8Rng;
use tokio::{
    sync::{mpsc, watch},
    task::{JoinSet, spawn_blocking},
    time::{sleep, timeout},
};

/// Bombard random peers with random mints in multiple rounds, ensuring they all have
/// a consistent total amount in the end.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn multiple_blocks_created() -> Result<()> {
    const N_ROUNDS: u64 = 5;
    const N_MAX_TXS_PER_BLOCK: u64 = 10;

    // Given
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(NonZero::new(N_MAX_TXS_PER_BLOCK).expect("valid")),
        )))
        .with_pipeline_time(Duration::from_secs(1));
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(multiple_blocks_created)).await?
    else {
        return Ok(());
    };
    let leader = network
        .peers()
        .iter()
        .min_by_key(|peer| peer.public_key().to_string())
        .expect("at least one peer");

    let create_domain = Register::domain(Domain::new("domain".parse()?));
    let (account_id, _account_keypair) = gen_account_in("domain");
    let create_account = Register::account(Account::new(account_id.clone()));
    let asset_definition_id: AssetDefinitionId = "xor#domain".parse()?;
    let create_asset =
        Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));

    {
        let client = leader.client();
        let tx = client.clone().build_transaction(
            [
                InstructionBox::from(create_domain),
                InstructionBox::from(create_account),
                InstructionBox::from(create_asset),
            ],
            <_>::default(),
        );
        let submit_res: eyre::Result<()> = spawn_blocking(move || client.submit_transaction(&tx))
            .await
            .map_err(eyre::Report::from)?
            .map(|_| ());
        if sandbox::handle_result(submit_res, stringify!(multiple_blocks_created))?.is_none() {
            return Ok(());
        }
    }

    if sandbox::handle_result(
        network.ensure_blocks(2).await,
        stringify!(multiple_blocks_created),
    )?
    .is_none()
    {
        return Ok(());
    }

    let blocks = BlocksTracker::start(&network);

    // When
    let mut rng = ChaCha8Rng::seed_from_u64(0x4d55_4c54);
    let mut total: u128 = 0;
    for _ in 1..=N_ROUNDS {
        let txs = (1..=N_MAX_TXS_PER_BLOCK)
            .choose(&mut rng)
            .expect("there is a room to choose from");
        println!("submitting {txs} transactions to random peers");
        for _ in 0..txs {
            let value = (0..999_999)
                .choose(&mut rng)
                .expect("there is quite a room to choose from");
            total += value;

            let client = leader.client();
            let tx = client.build_transaction(
                [Mint::asset_numeric(
                    Numeric::new(value, 0),
                    AssetId::new(asset_definition_id.clone(), account_id.clone()),
                )],
                <_>::default(),
            );
            let submit_res: eyre::Result<()> =
                spawn_blocking(move || client.submit_transaction(&tx))
                    .await
                    .map_err(eyre::Report::from)?
                    .map(|_| ());
            if sandbox::handle_result(submit_res, stringify!(multiple_blocks_created))?.is_none() {
                return Ok(());
            }
        }

        let sync_res = timeout(network.sync_timeout(), blocks.sync())
            .await
            .map_err(eyre::Report::new);
        if sandbox::handle_result(sync_res, stringify!(multiple_blocks_created))?.is_none() {
            return Ok(());
        }
    }

    // ensuring all have the same total
    println!("all peers should have total={total}");
    let expected_value = Numeric::new(total, 0);

    // Give the network a chance to flush any straggling transactions before asserting.
    let deadline = Instant::now() + network.sync_timeout();
    loop {
        let mut all_ok = true;
        for peer in network.peers() {
            let client = peer.client();
            let account_id = account_id.clone();
            let definition = asset_definition_id.clone();
            let assets: Vec<Asset> = match sandbox::handle_result(
                spawn_blocking(move || client.query(FindAssets::new()).execute_all())
                    .await?
                    .map_err(eyre::Report::from),
                stringify!(multiple_blocks_created),
            )? {
                Some(v) => v,
                None => return Ok(()),
            };
            let asset = assets
                .into_iter()
                .find(|asset| {
                    *asset.id().account() == account_id && *asset.id().definition() == definition
                })
                .expect("asset not found");
            if *asset.value() != expected_value {
                all_ok = false;
                break;
            }
        }

        if all_ok {
            break;
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "asset totals did not converge to expected value {expected_value}"
            ));
        }

        sleep(Duration::from_millis(200)).await;
    }

    Ok(())
}

struct BlocksTracker {
    sync_tx: watch::Sender<bool>,
    _children: JoinSet<()>,
}

impl BlocksTracker {
    fn start(network: &Network) -> Self {
        enum PeerEvent {
            Block(u64),
            Transaction,
        }

        let mut children = JoinSet::new();

        let (block_tx, mut block_rx) = mpsc::channel::<(PeerEvent, usize)>(10);
        for (i, peer) in network.peers().iter().cloned().enumerate() {
            let tx = block_tx.clone();
            children.spawn(async move {
                let mut events = peer
                    .client()
                    .listen_for_events_async([
                        EventFilterBox::from(BlockEventFilter::default()),
                        TransactionEventFilter::default().into(),
                    ])
                    .await
                    .expect("peer should be up");
                while let Some(Ok(event)) = events.next().await {
                    match event {
                        EventBox::Pipeline(PipelineEventBox::Block(x))
                            if matches!(*x.status(), BlockStatus::Applied) =>
                        {
                            let _ = tx
                                .send((PeerEvent::Block(x.header().height().get()), i))
                                .await;
                        }
                        EventBox::Pipeline(PipelineEventBox::Transaction(x))
                            if matches!(*x.status(), TransactionStatus::Queued) =>
                        {
                            let _ = tx.send((PeerEvent::Transaction, i)).await;
                        }
                        _ => {}
                    }
                }
            });
        }

        let peers_count = network.peers().len();
        let (sync_tx, _sync_rx) = watch::channel(false);
        let sync_clone = sync_tx.clone();
        children.spawn(async move {
            #[derive(Copy, Clone)]
            struct PeerState {
                height: u64,
                mutated: bool,
            }

            let mut blocks = vec![
                PeerState {
                    height: 0,
                    mutated: false
                };
                peers_count
            ];
            loop {
                tokio::select! {
                    Some((event, i)) = block_rx.recv() => {
                        let state = blocks.get_mut(i).unwrap();
                        match event {
                            PeerEvent::Block(height) => {
                                state.height = height;
                                state.mutated = false;
                            }
                            PeerEvent::Transaction => {
                                state.mutated = true;
                            }
                        }

                        let max_height = blocks.iter().map(|x| x.height).max().expect("there is at least 1");
                        let is_sync = blocks.iter().all(|x| x.height == max_height && !x.mutated);
                        sync_tx.send_modify(|flag| *flag = is_sync);
                    }
                }
            }
        });

        Self {
            sync_tx: sync_clone,
            _children: children,
        }
    }

    async fn sync(&self) {
        let mut recv = self.sync_tx.subscribe();
        loop {
            if *recv.borrow_and_update() {
                return;
            }
            recv.changed().await.unwrap()
        }
    }
}
