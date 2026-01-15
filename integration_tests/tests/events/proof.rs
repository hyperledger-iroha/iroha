//! Integration tests for ZK proof events over the Torii event stream.
use std::time::Duration;

use assert_matches::assert_matches;
use eyre::{Result, eyre};
use futures_util::StreamExt;
use integration_tests::sandbox;
use iroha::client::Client;
use iroha::data_model::prelude::*;
use iroha_core::zk::test_utils::halo2_fixture_envelope;
use iroha_data_model::events::data::prelude::ProofEventFilter;
use iroha_test_network::*;
use tokio::{task::spawn_blocking, time::timeout};

const PROOF_VERIFY_TIMEOUT_MS: i64 = 600_000;
const CLIENT_STATUS_TIMEOUT: Duration = Duration::from_secs(600);
const PROOF_EVENT_TIMEOUT: Duration = Duration::from_secs(600);

fn halo2_attachment() -> iroha::data_model::proof::ProofAttachment {
    let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add-v1", [0u8; 32]);
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture must include a verifying key");
    let proof_box = fixture.proof_box("halo2/ipa");
    iroha::data_model::proof::ProofAttachment::new_inline("halo2/ipa".into(), proof_box, vk_box)
}

fn client_with_timeout(network: &Network) -> Client {
    let mut client = network.client();
    client.transaction_status_timeout = CLIENT_STATUS_TIMEOUT;
    client.transaction_ttl = Some(CLIENT_STATUS_TIMEOUT + Duration::from_secs(5));
    client
}

fn proof_event_timeout(network: &Network) -> Duration {
    network.sync_timeout().max(PROOF_EVENT_TIMEOUT)
}

fn proof_network_builder() -> NetworkBuilder {
    NetworkBuilder::new().with_config_layer(|layer| {
        layer.write(
            ["confidential", "verify_timeout_ms"],
            PROOF_VERIFY_TIMEOUT_MS,
        );
    })
}

fn is_tx_confirmation_timeout(err: &eyre::Report) -> bool {
    const NEEDLES: [&str; 3] = [
        "haven't got tx confirmation within",
        "transaction queued for too long",
        "Connection dropped without `Committed/Applied` or `Rejected` event",
    ];
    err.chain().any(|cause| {
        let text = cause.to_string();
        NEEDLES.iter().any(|needle| text.contains(needle))
    })
}

#[tokio::test]
async fn verify_proof_emits_verified_event() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        proof_network_builder(),
        stringify!(verify_proof_emits_verified_event),
    )
    .await?
    else {
        return Ok(());
    };
    let result: Result<()> = async {
        network.ensure_blocks(1).await?;
        let client = client_with_timeout(&network);
        let mut events = tokio::time::timeout(
            proof_event_timeout(&network),
            client.listen_for_events_async([DataEventFilter::Proof(ProofEventFilter::new())]),
        )
        .await
        .map_err(|_| {
            eyre!("verify_proof_emits_verified_event: timed out opening proof event stream")
        })??;

        // Build a VerifyProof ISI with inline VK on an accepted backend
        let attachment = halo2_attachment();
        let verify: InstructionBox =
            iroha::data_model::isi::zk::VerifyProof::new(attachment).into();

        {
            let submit_client = client.clone();
            let submit_result =
                spawn_blocking(move || submit_client.submit_all_blocking([verify])).await?;
            if let Err(err) = submit_result {
                if is_tx_confirmation_timeout(&err) {
                    eprintln!(
                        "warning: verify_proof_emits_verified_event confirmation timed out; continuing to wait for events"
                    );
                } else {
                    return Err(err);
                }
            }
        }
        network.ensure_blocks(2).await?;

        let result = async {
            // Wait for the proof event and assert it is Verified
            let proof_event = timeout(proof_event_timeout(&network), async {
                loop {
                    let ev = events.next().await.expect("event stream open")?;
                    if let EventBox::Data(event) = ev
                        && let DataEvent::Proof(pe) = event.as_ref()
                    {
                        break Ok::<_, eyre::Report>(pe.clone());
                    }
                }
            })
            .await??;
            assert_matches!(
                proof_event,
                iroha::data_model::events::data::proof::ProofEvent::Verified(_)
            );

            Ok(())
        }
        .await;

        events.close().await;
        result
    }
    .await;
    if sandbox::handle_result(result, stringify!(verify_proof_emits_verified_event))?.is_none() {
        return Ok(());
    }

    Ok(())
}

#[tokio::test]
async fn verify_proof_emits_rejected_event() -> Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        proof_network_builder(),
        stringify!(verify_proof_emits_rejected_event),
    )
    .await?
    else {
        return Ok(());
    };
    let result: Result<()> = async {
        network.ensure_blocks(1).await?;
        let client = client_with_timeout(&network);
        let mut events = tokio::time::timeout(
            proof_event_timeout(&network),
            client.listen_for_events_async([DataEventFilter::Proof(ProofEventFilter::new())]),
        )
        .await
        .map_err(|_| {
            eyre!("verify_proof_emits_rejected_event: timed out opening proof event stream")
        })??;

        // Build a VerifyProof ISI that deterministically rejects
        let attachment = iroha::data_model::proof::ProofAttachment::new_inline(
            "debug/reject".into(),
            iroha::data_model::proof::ProofBox::new("debug/reject".into(), vec![0xaa]),
            iroha::data_model::proof::VerifyingKeyBox::new("debug/reject".into(), vec![0xbb]),
        );
        let verify: InstructionBox =
            iroha::data_model::isi::zk::VerifyProof::new(attachment).into();

        {
            let submit_client = client.clone();
            let submit_result =
                spawn_blocking(move || submit_client.submit_all_blocking([verify])).await?;
            if let Err(err) = submit_result {
                if is_tx_confirmation_timeout(&err) {
                    eprintln!(
                        "warning: verify_proof_emits_rejected_event confirmation timed out; continuing to wait for events"
                    );
                } else {
                    return Err(err);
                }
            }
        }
        network.ensure_blocks(2).await?;

        let result = async {
            let proof_event = timeout(proof_event_timeout(&network), async {
                loop {
                    let ev = events.next().await.expect("event stream open")?;
                    if let EventBox::Data(event) = ev
                        && let DataEvent::Proof(pe) = event.as_ref()
                    {
                        break Ok::<_, eyre::Report>(pe.clone());
                    }
                }
            })
            .await??;
            assert_matches!(
                proof_event,
                iroha::data_model::events::data::proof::ProofEvent::Rejected(_)
            );

            Ok(())
        }
        .await;

        events.close().await;
        result
    }
    .await;
    if sandbox::handle_result(result, stringify!(verify_proof_emits_rejected_event))?.is_none() {
        return Ok(());
    }

    Ok(())
}
