//! Readiness smoke tests against the in-process mock Torii service.
#[path = "supervisor.rs"]
mod supervisor;
use color_eyre::Result;
use iroha_data_model::{
    NetworkId,
    events::{
        EventBox,
        pipeline::{PipelineEventBox, TransactionEvent, TransactionStatus},
        stream::EventMessage,
    },
};
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use mochi_core::{
    ReadinessOptions, ReadinessSmokePlan, SmokeCommitOptions, ToriiClient, ToriiError,
    development_signing_authorities,
};
use mochi_integration::{MockToriiBuilder, MockToriiFrame};
use std::{
    net::{SocketAddr, TcpListener},
    num::NonZeroU64,
    time::Duration,
};
use tokio::time::sleep;
fn test_network_id() -> NetworkId {
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        .parse()
        .expect("test network id")
}
fn stream_reader(addr: SocketAddr) -> iroha::client::AccountClient {
    stream_reader_for_network(addr, test_network_id())
}
fn stream_reader_for_network(
    addr: SocketAddr,
    network_id: NetworkId,
) -> iroha::client::AccountClient {
    let signer = &development_signing_authorities()[0];
    iroha::client::Client::builder(iroha::config::Config {
        chain: "mochi-local".into(),
        network_id,
        account: signer.account_id().clone(),
        key_pair: signer.key_pair().clone(),
        account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
        basic_auth: None,
        torii_api_url: format!("http://{addr}/").parse().unwrap(),
        torii_request_timeout: Duration::from_secs(5),
        transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
        transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
        transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
        sorafs_alias_cache: iroha::config::AliasCache::default().into_policy(),
        sorafs_anonymity_policy: Default::default(),
        sorafs_rollout_phase: Default::default(),
    })
    .build()
    .unwrap()
    .account_client()
    .unwrap()
}
fn reserve_port() -> std::io::Result<u16> {
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
}
#[tokio::test(flavor = "multi_thread")]
async fn readiness_smoke_succeeds_on_pipeline_event() -> Result<()> {
    let port = match reserve_port() {
        Ok(port) => port,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping readiness_smoke_succeeds_on_pipeline_event: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let reader = stream_reader(addr);
    let mock = MockToriiBuilder::new(addr)
        .stream_reader(&reader)
        .block_frame(Vec::new())
        .event_frame(Vec::new())
        .spawn()
        .await?;
    let network_id = test_network_id();
    let client = ToriiClient::new_for_network(format!("http://{}", mock.addr()), network_id)?;
    let signer = &development_signing_authorities()[0];
    let mut plan = ReadinessSmokePlan::for_signer_with_attempts(network_id, signer, 1)?;
    plan.commit_options = SmokeCommitOptions::new(Duration::from_millis(800));
    plan.status_options = ReadinessOptions::new(Duration::from_millis(500))
        .with_poll_interval(Duration::from_millis(50));
    plan.backoff = Duration::from_millis(50);
    let tx_hash = plan.tx_hashes().next().expect("hash present");
    let event = EventMessage::new(EventBox::Pipeline(PipelineEventBox::Transaction(
        TransactionEvent {
            hash: tx_hash,
            block_height: Some(NonZeroU64::new(7).expect("non-zero height")),
            lane_id: LaneId::new(0),
            dataspace_id: DataSpaceId::new(0),
            status: TransactionStatus::Approved,
        },
    )));
    let event_bytes = norito::to_bytes(&event)?;
    let readiness = client.wait_for_readiness_smoke(&reader, plan);
    let sender = async {
        sleep(Duration::from_millis(50)).await;
        mock.broadcast_event(MockToriiFrame::Binary(event_bytes));
    };
    let (outcome, _) = tokio::join!(readiness, sender);
    let outcome = outcome.expect("smoke readiness should succeed");
    assert_eq!(outcome.attempt, 1);
    assert_eq!(outcome.commit.block_height, 7);
    assert_eq!(outcome.commit.tx_hash, tx_hash);
    mock.shutdown().await?;
    Ok(())
}
#[tokio::test(flavor = "multi_thread")]
async fn readiness_smoke_times_out_without_commit() -> Result<()> {
    let port = match reserve_port() {
        Ok(port) => port,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping readiness_smoke_times_out_without_commit: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let reader = stream_reader(addr);
    let mock = MockToriiBuilder::new(addr)
        .stream_reader(&reader)
        .block_frame(Vec::new())
        .event_frame(Vec::new())
        .spawn()
        .await?;
    let network_id = test_network_id();
    let client = ToriiClient::new_for_network(format!("http://{}", mock.addr()), network_id)?;
    let signer = &development_signing_authorities()[0];
    let mut plan = ReadinessSmokePlan::for_signer_with_attempts(network_id, signer, 1)?;
    plan.commit_options = SmokeCommitOptions::new(Duration::from_millis(150));
    plan.status_options = ReadinessOptions::new(Duration::from_millis(200))
        .with_poll_interval(Duration::from_millis(30));
    let outcome = client.wait_for_readiness_smoke(&reader, plan).await;
    match outcome {
        Err(ToriiError::Timeout { .. }) => {}
        other => panic!("expected timeout error, got {other:?}"),
    }
    mock.shutdown().await?;
    Ok(())
}
#[tokio::test(flavor = "multi_thread")]
async fn readiness_smoke_surfaces_stream_decode_errors() -> Result<()> {
    let port = match reserve_port() {
        Ok(port) => port,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skipping readiness_smoke_surfaces_stream_decode_errors: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let reader = stream_reader(addr);
    let mock = MockToriiBuilder::new(addr)
        .stream_reader(&reader)
        .block_frame(vec![0xFF])
        .event_frame(Vec::new())
        .spawn()
        .await?;
    let network_id = test_network_id();
    let client = ToriiClient::new_for_network(format!("http://{}", mock.addr()), network_id)?;
    let signer = &development_signing_authorities()[0];
    let mut plan = ReadinessSmokePlan::for_signer_with_attempts(network_id, signer, 1)?;
    plan.commit_options = SmokeCommitOptions::new(Duration::from_millis(300));
    plan.status_options = ReadinessOptions::new(Duration::from_millis(200))
        .with_poll_interval(Duration::from_millis(30));
    let outcome = client.wait_for_readiness_smoke(&reader, plan).await;
    match outcome {
        Err(ToriiError::Sdk(error)) if matches!(error.as_ref(), iroha::Error::Decode { .. }) => {}
        other => panic!("expected decode error, got {other:?}"),
    }
    mock.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn streams_reject_another_network_and_an_ungranted_reader() -> Result<()> {
    let expected = stream_reader(SocketAddr::from(([127, 0, 0, 1], 0)));
    let mock = MockToriiBuilder::new(SocketAddr::from(([127, 0, 0, 1], 0)))
        .stream_reader(&expected)
        .spawn()
        .await?;
    let foreign_network = NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"another-genesis")),
    );
    let foreign = stream_reader_for_network(mock.addr(), foreign_network);
    let result = foreign.blocks().subscribe(NonZeroU64::MIN).await;
    assert!(matches!(
        result,
        Err(iroha::Error::Http { status: 401, .. })
    ));
    assert_eq!(mock.authenticated_stream_count(), 0);
    mock.shutdown().await?;

    let ungranted = MockToriiBuilder::new(SocketAddr::from(([127, 0, 0, 1], 0)))
        .spawn()
        .await?;
    let reader = stream_reader(ungranted.addr());
    let result = reader.blocks().subscribe(NonZeroU64::MIN).await;
    assert!(matches!(
        result,
        Err(iroha::Error::Http { status: 403, .. })
    ));
    assert_eq!(ungranted.authenticated_stream_count(), 0);
    ungranted.shutdown().await?;
    Ok(())
}
