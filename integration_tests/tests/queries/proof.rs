#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests for proof queries.
use std::{thread::sleep, time::Duration};

use eyre::Result;
use integration_tests::sandbox;
use iroha::data_model::{
    prelude::*,
    proof::ProofAttachment,
    query::proof::prelude::{FindProofRecords, FindProofRecordsByStatus},
};
use iroha_core::zk::{hash_vk, test_utils::halo2_fixture_envelope};
use iroha_data_model::{
    confidential::ConfidentialStatus,
    isi::verifying_keys,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_test_network::NetworkBuilder;

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
    record.gas_schedule_id = Some("query_test".to_owned());
    record.key = Some(vk_box);
    record.status = ConfidentialStatus::Active;
    record
}

fn halo2_verifying_key_registration(
    circuit_id: &str,
    label: &str,
) -> verifying_keys::RegisterVerifyingKey {
    let fixture = halo2_fixture_envelope(circuit_id, [0u8; 32]);
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("proof query fixture embeds verifier key bytes");
    verifying_keys::RegisterVerifyingKey {
        id: VerifyingKeyId::new("halo2/ipa", label),
        record: vk_record(
            circuit_id,
            BackendTag::Halo2IpaPasta,
            "pallas",
            vk_box,
            fixture.schema_hash,
        ),
    }
}

fn groth_verifying_key_registration(label: &str) -> verifying_keys::RegisterVerifyingKey {
    let vk_box = VerifyingKeyBox::new("groth16/bn254".into(), vec![7, 7, 7]);
    verifying_keys::RegisterVerifyingKey {
        id: VerifyingKeyId::new("groth16/bn254", label),
        record: vk_record(
            "groth16/bn254:unsupported",
            BackendTag::Groth16,
            "bn254",
            vk_box,
            [0u8; 32],
        ),
    }
}

fn halo2_attachment(circuit_id: &str, label: &str) -> ProofAttachment {
    let seed = halo2_fixture_envelope(circuit_id, [0u8; 32]);
    let vk_hash = seed
        .vk_hash("halo2/ipa")
        .expect("fixture must include a verifying key");
    let fixture = halo2_fixture_envelope(circuit_id, vk_hash);
    let proof_box = fixture.proof_box("halo2/ipa");
    ProofAttachment::new_ref(
        "halo2/ipa".into(),
        proof_box,
        VerifyingKeyId::new("halo2/ipa", label),
    )
}

#[test]
fn proof_query_scenarios() -> Result<()> {
    use iroha::data_model::query::proof::prelude::FindProofRecordsByBackend;

    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new()
            .with_genesis_instruction(halo2_verifying_key_registration(
                "halo2/ipa:tiny-add",
                "query_tiny_add_vk",
            ))
            .with_genesis_instruction(halo2_verifying_key_registration(
                "halo2/ipa:tiny-add-public",
                "query_tiny_add_public_vk",
            ))
            .with_genesis_instruction(halo2_verifying_key_registration(
                "halo2/ipa:tiny-add2inst-public",
                "query_tiny_add2inst_public_vk",
            ))
            .with_genesis_instruction(groth_verifying_key_registration("query_groth_vk"))
            .with_genesis_instruction(groth_verifying_key_registration("query_bad_vk"))
            .with_config_layer(|layer| {
                layer.write(["zk", "halo2", "enabled"], true);
            }),
        stringify!(proof_query_scenarios),
    )?
    else {
        return Ok(());
    };
    let client = network.client();

    // find_proof_records_lists_after_verify
    {
        let attachment = halo2_attachment("halo2/ipa:tiny-add", "query_tiny_add_vk");
        client.submit_blocking(iroha::data_model::isi::zk::VerifyProof::new(attachment))?;
        rt.block_on(async { network.ensure_blocks(1).await })?;

        let recs = client.query(FindProofRecords).execute_all()?;
        assert!(
            !recs.is_empty(),
            "expected at least one proof record after VerifyProof"
        );
    }

    // find_proof_records_by_backend_filters
    {
        let att1 = halo2_attachment("halo2/ipa:tiny-add-public", "query_tiny_add_public_vk");
        let att2 = iroha::data_model::proof::ProofAttachment::new_ref(
            "groth16/bn254".into(),
            iroha::data_model::proof::ProofBox::new("groth16/bn254".into(), vec![0x03]),
            iroha::data_model::proof::VerifyingKeyId::new("groth16/bn254", "query_groth_vk"),
        );
        client.submit_all_blocking([iroha::data_model::isi::zk::VerifyProof::new(att1)])?;
        client.submit_all_blocking([iroha::data_model::isi::zk::VerifyProof::new(att2)])?;
        rt.block_on(async { network.ensure_blocks(1).await })?;

        let halo2 = client
            .query(FindProofRecordsByBackend::new("halo2/ipa".into()))
            .execute_all()?;
        let groth = client
            .query(FindProofRecordsByBackend::new("groth16/bn254".into()))
            .execute_all()?;

        assert!(
            !halo2.is_empty(),
            "expected at least one halo2/ipa proof record"
        );
        assert!(
            !groth.is_empty(),
            "expected at least one groth16/bn254 proof record"
        );
    }

    // find_proof_records_by_status_filters
    {
        let att_ok = halo2_attachment(
            "halo2/ipa:tiny-add2inst-public",
            "query_tiny_add2inst_public_vk",
        );
        let att_bad = iroha::data_model::proof::ProofAttachment::new_ref(
            "groth16/bn254".into(),
            iroha::data_model::proof::ProofBox::new("groth16/bn254".into(), vec![0x20]),
            iroha::data_model::proof::VerifyingKeyId::new("groth16/bn254", "query_bad_vk"),
        );
        client.submit_all_blocking([iroha::data_model::isi::zk::VerifyProof::new(att_ok)])?;
        client.submit_all_blocking([iroha::data_model::isi::zk::VerifyProof::new(att_bad)])?;
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
    let a = halo2_attachment("halo2/ipa:tiny-add", "query_tiny_add_vk");
    let b = halo2_attachment("halo2/ipa:tiny-add-public", "query_tiny_add_public_vk");
    let hash_a = iroha_core::zk::hash_proof(&a.proof);
    let hash_b = iroha_core::zk::hash_proof(&b.proof);
    assert_ne!(hash_a, hash_b, "fixture circuit should change proof hash");
}
