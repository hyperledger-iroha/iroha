#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for proof queries.
use std::{thread::sleep, time::Duration};

use eyre::Result;
use integration_tests::sandbox;
use iroha::data_model::{
    confidential::ConfidentialStatus,
    isi::verifying_keys,
    prelude::*,
    proof::{ProofAttachment, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    query::proof::prelude::{FindProofRecords, FindProofRecordsByStatus},
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_core::zk::test_utils::halo2_fixture_envelope;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID;

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
    circuit_id: &str,
    vk_name: &str,
) -> (ProofAttachment, verifying_keys::RegisterVerifyingKey) {
    let seed = halo2_fixture_envelope(circuit_id, [0u8; 32]);
    let vk_hash = seed
        .vk_hash("halo2/ipa")
        .expect("fixture must include a verifying key");
    let fixture = halo2_fixture_envelope(circuit_id, vk_hash);
    let proof_box = fixture.proof_box("halo2/ipa");
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture must include verifying key bytes");
    let vk_id = VerifyingKeyId::new("halo2/ipa", vk_name);
    let record = active_vk_record(
        circuit_id,
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
    let circuit_id = "halo2/ipa:query-rejected";
    let public_inputs = vec![1, 2, 3, 4];
    let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![7, 7, 7, 7]);
    let vk_commitment = iroha_core::zk::hash_vk(&vk_box);
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: circuit_id.to_owned(),
        vk_hash: vk_commitment,
        public_inputs: public_inputs.clone(),
        proof_bytes: vec![0x20, 0x21, 0x22],
        aux: Vec::new(),
    };
    let proof_box = iroha::data_model::proof::ProofBox::new(
        "halo2/ipa".into(),
        norito::to_bytes(&envelope).expect("OpenVerifyEnvelope should encode"),
    );
    let vk_id = VerifyingKeyId::new("halo2/ipa", "query_bad_vk");
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

fn proof_query_network_builder(
    registrations: impl IntoIterator<Item = verifying_keys::RegisterVerifyingKey>,
) -> NetworkBuilder {
    let mut builder = NetworkBuilder::new()
        .with_genesis_instruction(Grant::account_permission(
            Permission::new("CanManageVerifyingKeys".into(), Json::new(())),
            SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        ))
        .with_config_layer(|layer| {
            layer.write(["zk", "halo2", "enabled"], true);
        });
    for registration in registrations {
        builder = builder.with_genesis_instruction(registration);
    }
    builder
}

fn halo2_attachment(circuit_id: &str) -> ProofAttachment {
    halo2_attachment_and_registration(
        circuit_id,
        &format!("hash_only_{}", circuit_id.replace(['/', ':'], "_")),
    )
    .0
}

#[test]
fn proof_query_scenarios() -> Result<()> {
    use iroha::data_model::query::proof::prelude::FindProofRecordsByBackend;

    let (find_attachment, find_vk) =
        halo2_attachment_and_registration("halo2/ipa:tiny-add", "query_vk_find");
    let (backend_attachment, backend_vk) =
        halo2_attachment_and_registration("halo2/ipa:tiny-add-public", "query_vk_backend");
    let (verified_attachment, verified_vk) =
        halo2_attachment_and_registration("halo2/ipa:tiny-add2inst-public", "query_vk_status");
    let (rejected_attachment, rejected_vk) = rejected_halo2_attachment_and_registration();

    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        proof_query_network_builder([find_vk, backend_vk, verified_vk, rejected_vk]),
        stringify!(proof_query_scenarios),
    )?
    else {
        return Ok(());
    };
    let client = network.client();

    // find_proof_records_lists_after_verify
    {
        client.submit_blocking(iroha::data_model::isi::zk::VerifyProof::new(
            find_attachment,
        ))?;
        rt.block_on(async { network.ensure_blocks(1).await })?;

        let recs = client.query(FindProofRecords).execute_all()?;
        assert!(
            !recs.is_empty(),
            "expected at least one proof record after VerifyProof"
        );
    }

    // find_proof_records_by_backend_filters
    {
        client.submit_all_blocking([iroha::data_model::isi::zk::VerifyProof::new(
            backend_attachment,
        )])?;
        rt.block_on(async { network.ensure_blocks(1).await })?;

        let halo2 = client
            .query(FindProofRecordsByBackend::new("halo2/ipa".into()))
            .execute_all()?;
        assert!(
            !halo2.is_empty(),
            "expected at least one halo2/ipa proof record"
        );
        assert!(
            halo2
                .iter()
                .all(|record| record.id.backend.as_str() == "halo2/ipa"),
            "backend query should only return halo2/ipa proof records"
        );

        let nonexistent = client
            .query(FindProofRecordsByBackend::new("nonexistent".into()))
            .execute_all()?;
        assert!(
            nonexistent.is_empty(),
            "nonexistent backend should be empty"
        );
    }

    // find_proof_records_by_status_filters
    {
        client.submit_all_blocking([iroha::data_model::isi::zk::VerifyProof::new(
            verified_attachment,
        )])?;
        client.submit_all_blocking([iroha::data_model::isi::zk::VerifyProof::new(
            rejected_attachment,
        )])?;
        rt.block_on(async { network.ensure_blocks(1).await })?;

        let verified =
            retry_records_by_status(&client, iroha::data_model::proof::ProofStatus::Verified)?;
        let rejected =
            retry_records_by_status(&client, iroha::data_model::proof::ProofStatus::Rejected)?;

        assert!(
            !verified.is_empty(),
            "expected at least one verified proof record"
        );
        assert!(
            !rejected.is_empty(),
            "expected at least one rejected proof record"
        );
    }

    Ok(())
}

fn retry_records_by_status(
    client: &iroha::client::Client,
    status: iroha::data_model::proof::ProofStatus,
) -> Result<Vec<iroha::data_model::proof::ProofRecord>> {
    const RETRIES: usize = 5;
    const DELAY: Duration = Duration::from_millis(200);

    for attempt in 0..RETRIES {
        match client
            .query(FindProofRecordsByStatus::new(status))
            .execute_all()
        {
            Ok(records) if !records.is_empty() => return Ok(records),
            Ok(records) if attempt + 1 < RETRIES => {
                let _ = records;
                sleep(DELAY);
                // Continue retrying if empty.
            }
            Ok(records) => return Ok(records),
            Err(_) if attempt + 1 < RETRIES => {
                sleep(DELAY);
                // Retry on transient errors.
            }
            Err(err) => return Err(err.into()),
        }
    }
    unreachable!()
}

#[test]
fn halo2_attachment_circuit_changes_proof_hash() {
    let a = halo2_attachment("halo2/ipa:tiny-add");
    let b = halo2_attachment("halo2/ipa:tiny-add-public");
    let hash_a = iroha_core::zk::hash_proof(&a.proof);
    let hash_b = iroha_core::zk::hash_proof(&b.proof);
    assert_ne!(hash_a, hash_b, "fixture circuit should change proof hash");
}
