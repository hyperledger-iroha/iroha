//! One-shot local helper for refreshing checked-in FASTPQ measurement fixtures.
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use fastpq_prover::{TransitionBatch, transition_batch_from_model, transition_batch_to_model};
use iroha_data_model::fastpq::FastpqTransitionBatch;
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
    let rebound = transition_batch_to_model(&batch);
    let canonical = norito::to_bytes(&rebound)?;
    println!("{}", BASE64_STANDARD.encode(canonical));
    Ok(())
}
