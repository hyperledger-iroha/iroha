#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
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
use iroha_data_model::{
    confidential::ConfidentialStatus,
    isi::verifying_keys,
    proof::{ProofAttachment, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_test_network::*;
use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID;
use tokio::{task::spawn_blocking, time::timeout};

const PROOF_VERIFY_TIMEOUT_MS: i64 = 600_000;
const CLIENT_STATUS_TIMEOUT: Duration = Duration::from_secs(600);
const PROOF_EVENT_TIMEOUT: Duration = Duration::from_secs(600);

fn active_vk_record(
    circuit_id: &str,
    vk_box: VerifyingKeyBox,
    public_inputs_schema_hash: [u8; 32],
    max_proof_bytes: usize,
) -> VerifyingKeyRecord {
    let mut record = VerifyingKeyRecord::new(
        1,
        circuit_id,
        BackendTag::Halo2IpaPasta,
        "pallas",
        public_inputs_schema_hash,
        iroha_core::zk::hash_vk(&vk_box),
    );
    record.vk_len =
        u32::try_from(vk_box.bytes.len()).expect("verifying key length should fit in u32");
    record.max_proof_bytes =
        u32::try_from(max_proof_bytes).expect("proof length should fit in u32");
    record.gas_schedule_id = Some("halo2_default".to_owned());
    record.key = Some(vk_box);
    record.status = ConfidentialStatus::Active;
    record
}

fn halo2_attachment_and_registration(
    vk_name: &str,
) -> (ProofAttachment, verifying_keys::RegisterVerifyingKey) {
    let seed = halo2_fixture_envelope("halo2/ipa:tiny-add", [0u8; 32]);
    let vk_hash = seed
        .vk_hash("halo2/ipa")
        .expect("fixture should include a verifying key");
    let fixture = halo2_fixture_envelope("halo2/ipa:tiny-add", vk_hash);
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture should include verifying key bytes");
    let proof_box = fixture.proof_box("halo2/ipa");
    let vk_id = VerifyingKeyId::new("halo2/ipa", vk_name);
    let record = active_vk_record(
        "halo2/ipa:tiny-add",
        vk_box,
        fixture.schema_hash,
        proof_box.bytes.len(),
    );
    let attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof_box, vk_id.clone());
    (
        attachment,
        verifying_keys::RegisterVerifyingKey { id: vk_id, record },
    )
}

fn rejected_halo2_attachment_and_registration()
-> (ProofAttachment, verifying_keys::RegisterVerifyingKey) {
    let circuit_id = "halo2/ipa:event-rejected";
    let public_inputs = vec![1, 2, 3, 4];
    let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![9, 8, 7, 6]);
    let vk_commitment = iroha_core::zk::hash_vk(&vk_box);
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: circuit_id.to_owned(),
        vk_hash: vk_commitment,
        public_inputs: public_inputs.clone(),
        proof_bytes: vec![0xaa, 0xbb, 0xcc],
        aux: Vec::new(),
    };
    let proof_box = iroha::data_model::proof::ProofBox::new(
        "halo2/ipa".into(),
        norito::to_bytes(&envelope).expect("OpenVerifyEnvelope should encode"),
    );
    let vk_id = VerifyingKeyId::new("halo2/ipa", "event_rejected_vk");
    let record = active_vk_record(
        circuit_id,
        vk_box,
        iroha_crypto::Hash::new(&public_inputs).into(),
        proof_box.bytes.len(),
    );
    let attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof_box, vk_id.clone());
    (
        attachment,
        verifying_keys::RegisterVerifyingKey { id: vk_id, record },
    )
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
    let (_, verified_vk) = halo2_attachment_and_registration("event_vk");
    let (_, rejected_vk) = rejected_halo2_attachment_and_registration();
    NetworkBuilder::new()
        .with_config_layer(|layer| {
            // Enable Halo2 verification explicitly; default configs keep it off so operators must opt in.
            layer.write(["zk", "halo2", "enabled"], true).write(
                ["confidential", "verify_timeout_ms"],
                PROOF_VERIFY_TIMEOUT_MS,
            );
        })
        .with_genesis_instruction(Grant::account_permission(
            Permission::new("CanManageVerifyingKeys".into(), Json::new(())),
            SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        ))
        .with_genesis_instruction(verified_vk)
        .with_genesis_instruction(rejected_vk)
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
            halo2_attachment_and_registration("event_vk").0,
            true,
        )
        .await?;

        verify_proof_emits_event(
            &network,
            stringify!(verify_proof_emits_rejected_event),
            rejected_halo2_attachment_and_registration().0,
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
