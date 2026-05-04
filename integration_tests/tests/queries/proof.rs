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
use iroha_core::zk::test_utils::halo2_fixture_envelope;
use iroha_test_network::NetworkBuilder;

fn halo2_attachment(circuit_id: &str) -> ProofAttachment {
    let seed = halo2_fixture_envelope(circuit_id, [0u8; 32]);
    let vk_hash = seed
        .vk_hash("halo2/ipa")
        .expect("fixture must include a verifying key");
    let fixture = halo2_fixture_envelope(circuit_id, vk_hash);
    let proof_box = fixture.proof_box("halo2/ipa");
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture must include a verifying key");
    ProofAttachment::new_inline("halo2/ipa".into(), proof_box, vk_box)
}

#[test]
fn proof_query_scenarios() -> Result<()> {
    use iroha::data_model::query::proof::prelude::FindProofRecordsByBackend;

    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        NetworkBuilder::new().with_config_layer(|layer| {
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
        let attachment = halo2_attachment("halo2/ipa:tiny-add");
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
        let att1 = halo2_attachment("halo2/ipa:tiny-add-public");
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
        let att_ok = halo2_attachment("halo2/ipa:tiny-add2inst-public");
        let att_bad = iroha::data_model::proof::ProofAttachment::new_inline(
            "groth16/bn254".into(),
            iroha::data_model::proof::ProofBox::new("groth16/bn254".into(), vec![0x20]),
            iroha::data_model::proof::VerifyingKeyBox::new("groth16/bn254".into(), vec![0x21]),
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
    let a = halo2_attachment("halo2/ipa:tiny-add");
    let b = halo2_attachment("halo2/ipa:tiny-add-public");
    let hash_a = iroha_core::zk::hash_proof(&a.proof);
    let hash_b = iroha_core::zk::hash_proof(&b.proof);
    assert_ne!(hash_a, hash_b, "fixture circuit should change proof hash");
}
