//! Readiness smoke tests against the in-process mock Torii service.

use std::{
    net::{SocketAddr, TcpListener},
    num::NonZeroU64,
    time::Duration,
};

use color_eyre::Result;
use iroha_data_model::{
    NetworkId,
    events::{
        EventBox,
        pipeline::{PipelineEventBox, TransactionEvent, TransactionStatus},
        stream::EventMessage,
    },
    nexus::{DataSpaceId, LaneId},
};
use mochi_core::{
    ReadinessOptions, ReadinessSmokePlan, SmokeCommitOptions, ToriiClient, ToriiError,
    development_signing_authorities,
};
use mochi_integration::{MockToriiBuilder, MockToriiFrame};
use tokio::time::sleep;

fn test_network_id() -> NetworkId {
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        .parse()
        .expect("test network id")
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
    let mock = MockToriiBuilder::new(addr)
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

    let readiness = client.wait_for_readiness_smoke(plan);
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
    let mock = MockToriiBuilder::new(addr)
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

    let outcome = client.wait_for_readiness_smoke(plan).await;
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
    let mock = MockToriiBuilder::new(addr)
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

    let outcome = client.wait_for_readiness_smoke(plan).await;
    match outcome {
        Err(ToriiError::Decode(_)) => {}
        other => panic!("expected decode error, got {other:?}"),
    }

    mock.shutdown().await?;
    Ok(())
}
