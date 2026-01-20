//! Integration tests for proof queries.
use std::{thread::sleep, time::Duration};

use eyre::Result;
use integration_tests::sandbox;
use iroha::data_model::{
    prelude::*,
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox},
    query::proof::prelude::{FindProofRecords, FindProofRecordsByStatus},
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_test_network::NetworkBuilder;

fn halo2_attachment() -> ProofAttachment {
    // Minimal Halo2 OpenVerifyEnvelope to satisfy verification invariants.
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: "integration-test".to_string(),
        vk_hash: [0u8; 32],
        public_inputs: vec![0xde, 0xad],
        proof_bytes: vec![0xbe, 0xef],
        aux: Vec::new(),
    };
    let proof_payload =
        norito::to_bytes(&envelope).expect("OpenVerifyEnvelope serialization must succeed");
    let proof_box = ProofBox::new("halo2/ipa".into(), proof_payload);
    let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), vec![9, 9, 9]);
    ProofAttachment::new_inline("halo2/ipa".into(), proof_box, vk_box)
}

fn debug_ok_attachment() -> ProofAttachment {
    // Debug backend still expects a verifying key to satisfy attachment validation, even
    // though the key bytes are opaque to the backend.
    ProofAttachment::new_inline(
        "debug/ok".into(),
        ProofBox::new("debug/ok".into(), vec![0x01]),
        VerifyingKeyBox::new("debug/ok".into(), vec![0x02]),
    )
}

#[test]
fn proof_query_scenarios() -> Result<()> {
    use iroha::data_model::query::proof::prelude::FindProofRecordsByBackend;

    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new(),
        stringify!(proof_query_scenarios),
    )?
    else {
        return Ok(());
    };
    let client = network.client();

    // find_proof_records_lists_after_verify
    {
        let attachment = halo2_attachment();
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
        let att1 = halo2_attachment();
        let att2 = iroha::data_model::proof::ProofAttachment::new_inline(
            "groth16/bn254".into(),
            iroha::data_model::proof::ProofBox::new("groth16/bn254".into(), vec![0x03]),
            iroha::data_model::proof::VerifyingKeyBox::new("groth16/bn254".into(), vec![0x04]),
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
        let att_ok = debug_ok_attachment();
        let att_bad = iroha::data_model::proof::ProofAttachment::new_inline(
            "debug/reject".into(),
            iroha::data_model::proof::ProofBox::new("debug/reject".into(), vec![0x20]),
            iroha::data_model::proof::VerifyingKeyBox::new("debug/reject".into(), vec![0x21]),
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
