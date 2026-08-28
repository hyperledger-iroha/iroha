//! Shared safeguards for explicit fixture regeneration.
#[cfg(feature = "dev-tools")]
use fastpq_prover::{
    OperationKind, PublicInputs, StateTransition, TransitionBatch,
    gadgets::transfer::attach_transfer_smt_witnesses,
};
#[cfg(feature = "dev-tools")]
use iroha_crypto::{Algorithm, Hash, KeyPair};
#[cfg(feature = "dev-tools")]
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    domain::DomainId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
#[cfg(feature = "dev-tools")]
use iroha_primitives::numeric::Quantity;
#[cfg(feature = "dev-tools")]
use norito::to_bytes;
use std::ffi::OsStr;

pub(super) fn fixture_update_requested_from(value: Option<&OsStr>) -> Result<bool, &'static str> {
    match value {
        None => Ok(false),
        Some(value) if value == OsStr::new("1") => Ok(true),
        Some(_) => Err("FASTPQ_UPDATE_FIXTURES must be absent or have the exact value `1`"),
    }
}
pub(super) fn fixture_update_requested() -> bool {
    fixture_update_requested_from(std::env::var_os("FASTPQ_UPDATE_FIXTURES").as_deref())
        .unwrap_or_else(|message| panic!("{message}"))
}

/// Construct the deterministic mixed-operation batch shared by V1 proof fixtures.
#[cfg(feature = "dev-tools")]
pub(super) fn v1_fixture_batch(rows: usize, public_inputs: PublicInputs) -> TransitionBatch {
    let mut batch = TransitionBatch::new("fastpq-lane-balanced", public_inputs);
    let mut transcripts = Vec::new();
    let mut row_idx = 0usize;
    let mut transfer_idx = 0usize;
    while row_idx < rows {
        if row_idx % 3 == 0 && rows - row_idx >= 2 {
            let (transcript, sender, receiver) = transfer_pair(transfer_idx);
            batch.push(sender);
            batch.push(receiver);
            transcripts.push(transcript);
            row_idx += 2;
            transfer_idx += 1;
        } else {
            let key = format!("metadata/fixture/{row_idx:08}").into_bytes();
            let pre = (row_idx as u64 + 2).to_le_bytes().to_vec();
            let post = (row_idx as u64 + 1).to_le_bytes().to_vec();
            batch.push(StateTransition::new(key, pre, post, OperationKind::MetaSet));
            row_idx += 1;
        }
    }
    let (old_root, new_root) = if transcripts.is_empty() {
        ([0u8; 32], [0u8; 32])
    } else {
        attach_transfer_smt_witnesses(&mut transcripts).expect("attach transfer SMT witnesses")
    };
    if !transcripts.is_empty() {
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&transcripts).expect("encode transcripts"),
        );
    }
    batch.sort();
    batch.public_inputs.old_root = old_root;
    batch.public_inputs.new_root = new_root;
    batch
}

#[cfg(feature = "dev-tools")]
fn transfer_pair(index: usize) -> (TransferTranscript, StateTransition, StateTransition) {
    let domain = DomainId::try_new("fixture", "universal").expect("domain id");
    let asset_definition =
        AssetDefinitionId::derive_from_components(domain.clone(), "xor".parse().unwrap());
    let from_account = deterministic_account(&format!("sender_{index:08}"), &domain);
    let to_account = deterministic_account(&format!("receiver_{index:08}"), &domain);
    let amount = 1 + (index as u64 % 100);
    let from_pre = 1_000_000u64 + index as u64;
    let from_post = from_pre.saturating_sub(amount);
    let to_pre = 500_000u64 + index as u64;
    let to_post = to_pre.saturating_add(amount);
    let delta = TransferDeltaTranscript {
        from_account: from_account.clone(),
        to_account: to_account.clone(),
        asset_definition: asset_definition.clone(),
        amount: Quantity::from(amount),
        from_balance_before: Quantity::from(from_pre),
        from_balance_after: Quantity::from(from_post),
        to_balance_before: Quantity::from(to_pre),
        to_balance_after: Quantity::from(to_post),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    };
    let mut payload = Vec::with_capacity(32);
    payload.extend_from_slice(b"fastpq-v1-fixture");
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
        format!("asset/{asset_definition}/{from_account}").into_bytes(),
        from_pre.to_le_bytes().to_vec(),
        from_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    let receiver = StateTransition::new(
        format!("asset/{asset_definition}/{to_account}").into_bytes(),
        to_pre.to_le_bytes().to_vec(),
        to_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    (transcript, sender, receiver)
}

#[cfg(feature = "dev-tools")]
fn deterministic_account(label: &str, domain: &DomainId) -> AccountId {
    let seed: [u8; Hash::LENGTH] = Hash::new(format!("{label}@{domain}")).into();
    let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::default())
        .expect("fixture FASTPQ account key");
    AccountId::new(keypair.public_key().clone())
}
