//! Performance regression test for the FASTPQ production prover.
//!
//! This is intentionally `#[ignore]` because the prover is CPU-heavy in debug
//! builds. Run the regression with
//! `cargo test -p fastpq_prover --test perf_production --release -- --ignored`
//! to obtain realistic timings.

use std::{env, time::Instant};

use fastpq_prover::{
    OperationKind, Prover, PublicInputs, StateTransition, TransitionBatch, verify,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    domain::DomainId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
use iroha_primitives::numeric::Numeric;
use norito::core::to_bytes;
use norito::to_bytes as norito_bytes;
use std::str::FromStr;

const PARAMETER_SET: &str = "fastpq-lane-balanced";
const DEFAULT_ROW_COUNT: usize = 20_000;
const DEFAULT_EXPECTED_MS: f64 = 1_000.0;
const DEFAULT_MAX_KIB: f64 = 512.0;

fn synthetic_batch(rows: usize) -> TransitionBatch {
    let mut batch = TransitionBatch::new(PARAMETER_SET, PublicInputs::default());
    let mut transcripts = Vec::new();
    let mut row_idx = 0usize;
    let mut transfer_idx = 0usize;
    while row_idx < rows {
        match row_idx % 5 {
            0 if row_idx + 1 < rows => {
                let (transcript, sender, receiver) = transfer_pair(transfer_idx);
                batch.push(sender);
                batch.push(receiver);
                transcripts.push(transcript);
                row_idx += 2;
                transfer_idx = transfer_idx.wrapping_add(1);
            }
            1 => {
                let asset_id = format!("asset/perf_mint_{row_idx:04}#perf");
                let key = format!("{asset_id}/balance/{row_idx:08}").into_bytes();
                let base = 50_000u64 + row_idx as u64;
                let delta = ((row_idx % 7) + 1) as u64;
                batch.push(StateTransition::new(
                    key,
                    base.to_le_bytes().to_vec(),
                    (base + delta).to_le_bytes().to_vec(),
                    OperationKind::Mint,
                ));
                row_idx += 1;
            }
            _ => {
                let asset_id = format!("asset/perf_burn_{row_idx:04}#perf");
                let key = format!("{asset_id}/balance/{row_idx:08}").into_bytes();
                let delta = ((row_idx % 7) + 1) as u64;
                let base = 60_000u64 + row_idx as u64 + delta;
                batch.push(StateTransition::new(
                    key,
                    base.to_le_bytes().to_vec(),
                    (base - delta).to_le_bytes().to_vec(),
                    OperationKind::Burn,
                ));
                row_idx += 1;
            }
        }
    }
    if !transcripts.is_empty() {
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            norito_bytes(&transcripts).expect("encode transcripts"),
        );
    }

    batch.sort();
    let mut dsid = [0u8; 16];
    for (idx, byte) in dsid.iter_mut().enumerate() {
        *byte = (idx as u8).wrapping_mul(11);
    }
    batch.public_inputs.dsid = dsid;
    batch.public_inputs.slot = 1_726_128_000_000_000;
    batch.public_inputs.old_root = [0x11; 32];
    batch.public_inputs.new_root = [0x22; 32];
    batch.public_inputs.perm_root = [0x33; 32];
    batch.public_inputs.tx_set_hash = [0x44; 32];

    batch
}

fn deterministic_account(label: &str, domain: &DomainId) -> AccountId {
    let seed: [u8; Hash::LENGTH] = Hash::new(format!("{label}@{domain}")).into();
    let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::default());
    AccountId::new(domain.clone(), keypair.public_key().clone())
}

fn transfer_pair(index: usize) -> (TransferTranscript, StateTransition, StateTransition) {
    let asset_definition = AssetDefinitionId::from_str("xor#perf").expect("asset definition");
    let domain = DomainId::from_str("perf").expect("domain id");
    let from_account = deterministic_account(&format!("sender_{index:04}"), &domain);
    let to_account = deterministic_account(&format!("receiver_{index:04}"), &domain);
    let amount = 1 + (index as u64 % 100);
    let from_pre = 200_000u64 + index as u64;
    let from_post = from_pre.saturating_sub(amount);
    let to_pre = 100_000u64 + index as u64;
    let to_post = to_pre.saturating_add(amount);
    let delta = TransferDeltaTranscript {
        from_account: from_account.clone(),
        to_account: to_account.clone(),
        asset_definition: asset_definition.clone(),
        amount: Numeric::from(amount),
        from_balance_before: Numeric::from(from_pre),
        from_balance_after: Numeric::from(from_post),
        to_balance_before: Numeric::from(to_pre),
        to_balance_after: Numeric::from(to_post),
        from_merkle_proof: None,
        to_merkle_proof: None,
    };
    let mut payload = Vec::with_capacity(32);
    payload.extend_from_slice(b"fastpq-perf");
    payload.extend_from_slice(&(index as u64).to_le_bytes());
    let batch_hash = Hash::new(payload);
    let digest = fastpq_prover::gadgets::transfer::compute_poseidon_digest(&delta, &batch_hash);
    let transcript = TransferTranscript {
        batch_hash,
        deltas: vec![delta],
        authority_digest: Hash::new(b"authority"),
        poseidon_preimage_digest: Some(digest),
    };
    let sender = StateTransition::new(
        format!("asset/{}/{}", asset_definition, from_account).into_bytes(),
        from_pre.to_le_bytes().to_vec(),
        from_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    let receiver = StateTransition::new(
        format!("asset/{}/{}", asset_definition, to_account).into_bytes(),
        to_pre.to_le_bytes().to_vec(),
        to_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    (transcript, sender, receiver)
}

fn configured_row_count() -> usize {
    env::var("FASTPQ_PROOF_ROWS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&rows| rows > 0)
        .unwrap_or(DEFAULT_ROW_COUNT)
}

fn configured_expected_ms() -> f64 {
    env::var("FASTPQ_EXPECTED_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&ms| ms > 0.0)
        .unwrap_or(DEFAULT_EXPECTED_MS)
}

fn configured_max_kib() -> f64 {
    env::var("FASTPQ_EXPECTED_KIB")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&kib| kib > 0.0)
        .unwrap_or(DEFAULT_MAX_KIB)
}

#[test]
#[ignore = "expensive production perf regression"]
fn production_prover_handles_large_batch_quickly() {
    let row_count = configured_row_count();
    let batch = synthetic_batch(row_count);
    let prover = Prover::canonical(PARAMETER_SET).expect("prover");

    let started = Instant::now();
    let proof = prover.prove(&batch).expect("prove synthetic batch");
    let elapsed = started.elapsed();

    let proof_bytes = to_bytes(&proof).expect("serialize proof");
    verify(&batch, &proof).expect("proof verifies");

    #[allow(clippy::cast_precision_loss)]
    let proof_kib = proof_bytes.len() as f64 / 1024.0;
    let duration_ms = elapsed.as_secs_f64() * 1000.0;
    eprintln!(
        "fastpq_production_prover: rows={row_count} duration_ms={duration_ms:.2} proof_kib={proof_kib:.1}"
    );

    if !cfg!(debug_assertions) {
        let expected_ms = configured_expected_ms();
        let max_kib = configured_max_kib();

        let elapsed_ms = duration_ms;
        assert!(
            elapsed_ms <= expected_ms,
            "placeholder backend took {elapsed_ms:.2} ms for {row_count} rows (threshold {expected_ms:.2} ms)"
        );

        assert!(
            proof_kib <= max_kib,
            "placeholder proof length {proof_kib:.1} KiB exceeds budget {max_kib:.1} KiB"
        );
    }
}
