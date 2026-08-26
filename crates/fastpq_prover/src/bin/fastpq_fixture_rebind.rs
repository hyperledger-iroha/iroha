//! One-shot local helper for refreshing checked-in FASTPQ measurement fixtures.
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use fastpq_prover::{
    TransitionBatch,
    gadgets::transfer::{
        attach_transfer_smt_witnesses, compute_poseidon_digest, decode_transcripts,
        verify_transcripts,
    },
    transition_batch_from_model, transition_batch_to_model,
};
use iroha_data_model::fastpq::{FastpqTransitionBatch, TRANSFER_TRANSCRIPTS_METADATA_KEY};
use std::{env, fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("missing input batch path")?);
    let dsid: u64 = args
        .next()
        .ok_or("missing source dataspace id")?
        .into_string()
        .map_err(|_| "source dataspace id is not UTF-8")?
        .parse()?;
    let entry_hash = args
        .next()
        .ok_or("missing entry hash")?
        .into_string()
        .map_err(|_| "entry hash is not UTF-8")?;
    if args.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    let entry_hash = hex::decode(entry_hash)?;
    if entry_hash.len() != 32 {
        return Err("entry hash must be exactly 32 bytes".into());
    }
    let encoded = fs::read(input)?;
    let mut batch = match norito::decode_from_bytes::<FastpqTransitionBatch>(&encoded) {
        Ok(model) => transition_batch_from_model(&model),
        Err(_) => norito::decode_from_bytes::<TransitionBatch>(&encoded)?,
    };
    let mut dsid_bytes = [0_u8; 16];
    dsid_bytes[..8].copy_from_slice(&dsid.to_le_bytes());
    batch.public_inputs.dsid = dsid_bytes;
    batch.metadata.insert("entry_hash".into(), entry_hash);
    if let Some(mut transcripts) = decode_transcripts(&batch.metadata)? {
        for transcript in &mut transcripts {
            transcript.poseidon_preimage_digest = match transcript.deltas.as_slice() {
                [delta] => Some(compute_poseidon_digest(delta, &transcript.batch_hash)),
                _ => None,
            };
        }
        let (old_root, new_root) = attach_transfer_smt_witnesses(&mut transcripts)?;
        verify_transcripts(&batch.transitions, &transcripts)?;
        batch.public_inputs.old_root = old_root;
        batch.public_inputs.new_root = new_root;
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            norito::to_bytes(&transcripts)?,
        );
    }
    let rebound = transition_batch_to_model(&batch);
    let canonical = norito::to_bytes(&rebound)?;
    println!("{}", BASE64_STANDARD.encode(canonical));
    Ok(())
}
