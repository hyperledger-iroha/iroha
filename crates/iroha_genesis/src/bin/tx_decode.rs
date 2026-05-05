//! Decode transaction payloads from stdin for diagnostics.

use std::io::{self, Read};

use iroha_data_model::transaction::{SignedTransaction, TransactionEntrypoint};
use iroha_version::codec::DecodeVersioned as _;
use norito::core::DecodeFromSlice as _;

fn try_decode_signed(bytes: &[u8]) -> String {
    if SignedTransaction::decode_all_versioned(bytes).is_ok() {
        return "signed:versioned".to_string();
    }
    if let Ok((_, used)) = SignedTransaction::decode_from_slice(bytes)
        && used == bytes.len()
    {
        return "signed:adaptive".to_string();
    }
    if norito::decode_from_bytes::<SignedTransaction>(bytes).is_ok() {
        return "signed:framed".to_string();
    }
    "signed:no".to_string()
}

fn try_decode_entrypoint(bytes: &[u8]) -> String {
    if let Ok(TransactionEntrypoint::External(_)) =
        TransactionEntrypoint::decode_all_versioned(bytes)
    {
        return "entrypoint:versioned-external".to_string();
    }
    if let Ok((TransactionEntrypoint::External(_), used)) =
        TransactionEntrypoint::decode_from_slice(bytes)
        && used == bytes.len()
    {
        return "entrypoint:adaptive-external".to_string();
    }
    if let Ok(TransactionEntrypoint::External(_)) =
        norito::decode_from_bytes::<TransactionEntrypoint>(bytes)
    {
        return "entrypoint:framed-external".to_string();
    }
    "entrypoint:no".to_string()
}

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).expect("read stdin");
    let trimmed = input.trim().trim_start_matches("0x");
    let bytes = hex::decode(trimmed).expect("valid hex input");
    println!("{}", try_decode_signed(&bytes));
    println!("{}", try_decode_entrypoint(&bytes));
}
