#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for ZK proof events over the Torii event stream.
use std::time::Duration;

use assert_matches::assert_matches;
use eyre::{Result, eyre};
use futures_util::StreamExt;
use integration_tests::sandbox;
use iroha::client::Client;
use iroha::data_model::prelude::*;
use iroha_core::zk::{hash_vk, test_utils::halo2_fixture_envelope};
use iroha_data_model::events::data::prelude::ProofEventFilter;
use iroha_data_model::{
    confidential::ConfidentialStatus,
    isi::verifying_keys,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_test_network::*;
use tokio::{task::spawn_blocking, time::timeout};

const PROOF_VERIFY_TIMEOUT_MS: i64 = 600_000;
const CLIENT_STATUS_TIMEOUT: Duration = Duration::from_secs(600);
const PROOF_EVENT_TIMEOUT: Duration = Duration::from_secs(600);

fn halo2_attachment() -> iroha::data_model::proof::ProofAttachment {
    let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add", [0u8; 32]);
    let proof_box = fixture.proof_box("halo2/ipa");
    iroha::data_model::proof::ProofAttachment::new_ref(
        "halo2/ipa".into(),
        proof_box,
        VerifyingKeyId::new("halo2/ipa", "event_vk"),
    )
}

fn vk_record(
    circuit_id: &str,
    backend: BackendTag,
    curve: &str,
    vk_box: VerifyingKeyBox,
    schema_hash: [u8; 32],
) -> VerifyingKeyRecord {
    let mut record = VerifyingKeyRecord::new(
        1,
        circuit_id.to_owned(),
        backend,
        curve,
        schema_hash,
        hash_vk(&vk_box),
    );
    record.vk_len = u32::try_from(vk_box.bytes.len()).expect("verifying key length fits u32");
    record.max_proof_bytes = 1_048_576;
    record.gas_schedule_id = Some("event_test".to_owned());
    record.key = Some(vk_box);
    record.status = ConfidentialStatus::Active;
    record
}

fn halo2_verifying_key_registration() -> verifying_keys::RegisterVerifyingKey {
    let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add", [0u8; 32]);
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("proof event fixture embeds verifier key bytes");
    verifying_keys::RegisterVerifyingKey {
        id: VerifyingKeyId::new("halo2/ipa", "event_vk"),
        record: vk_record(
            "halo2/ipa:tiny-add",
            BackendTag::Halo2IpaPasta,
            "pallas",
            vk_box,
            fixture.schema_hash,
        ),
    }
}

fn rejected_verifying_key_registration() -> verifying_keys::RegisterVerifyingKey {
    let vk_box = VerifyingKeyBox::new("groth16/bn254".into(), vec![7, 7, 7]);
    verifying_keys::RegisterVerifyingKey {
        id: VerifyingKeyId::new("groth16/bn254", "event_rejected_vk"),
        record: vk_record(
            "groth16/bn254:unsupported",
            BackendTag::Groth16,
            "bn254",
            vk_box,
            [0u8; 32],
        ),
    }
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
    NetworkBuilder::new()
        .with_genesis_instruction(halo2_verifying_key_registration())
        .with_genesis_instruction(rejected_verifying_key_registration())
        .with_config_layer(|layer| {
            // Enable Halo2 verification explicitly; default configs keep it off so operators must opt in.
            layer.write(["zk", "halo2", "enabled"], true).write(
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

async fn verify_proof_emits_event(
    network: &Network,
    context: &'static str,
    attachment: iroha::data_model::proof::ProofAttachment,
    expect_verified: bool,
) -> Result<()> {
    network.ensure_blocks(1).await?;
    let client = client_with_timeout(network);
    let mut events = tokio::time::timeout(
        proof_event_timeout(network),
        client.listen_for_events_async([DataEventFilter::Proof(ProofEventFilter::new())]),
    )
    .await
    .map_err(|_| eyre!("{context}: timed out opening proof event stream"))??;

    let verify: InstructionBox = iroha::data_model::isi::zk::VerifyProof::new(attachment).into();
    {
        let submit_client = client.clone();
        let submit_result =
            spawn_blocking(move || submit_client.submit_all_blocking([verify])).await?;
        if let Err(err) = submit_result {
            if is_tx_confirmation_timeout(&err) {
                eprintln!(
                    "warning: {context} confirmation timed out; continuing to wait for events"
                );
            } else {
                return Err(err);
            }
        }
    }
    network.ensure_blocks(2).await?;

    let result = async {
        let proof_event = timeout(proof_event_timeout(network), async {
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
        if expect_verified {
            assert_matches!(
                proof_event,
                iroha::data_model::events::data::proof::ProofEvent::Verified(_)
            );
        } else {
            assert_matches!(
                proof_event,
                iroha::data_model::events::data::proof::ProofEvent::Rejected(_)
            );
        }

        Ok(())
    }
    .await;

    events.close().await;
    result
}

#[tokio::test]
async fn proof_event_scenarios() -> Result<()> {
    let _override_guard = sandbox::override_network_parallelism(Some(true), None);
    let Some(network) =
        sandbox::start_network_async_or_skip(proof_network_builder(), "proof_event_scenarios")
            .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        verify_proof_emits_event(
            &network,
            stringify!(verify_proof_emits_verified_event),
            halo2_attachment(),
            true,
        )
        .await?;

        let rejected_attachment = iroha::data_model::proof::ProofAttachment::new_ref(
            "groth16/bn254".into(),
            iroha::data_model::proof::ProofBox::new("groth16/bn254".into(), vec![0xaa]),
            VerifyingKeyId::new("groth16/bn254", "event_rejected_vk"),
        );
        verify_proof_emits_event(
            &network,
            stringify!(verify_proof_emits_rejected_event),
            rejected_attachment,
            false,
        )
        .await?;

        Ok(())
    }
    .await;

    if sandbox::handle_result(result, stringify!(proof_event_scenarios))?.is_none() {
        return Ok(());
    }

    Ok(())
}
