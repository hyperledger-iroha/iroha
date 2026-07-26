//! Ensure Norito decode rejects invalid NFT payloads gracefully.

use std::io::Cursor;

use iroha_data_model::prelude::NftId;
use norito::codec::Decode as NoritoDecode;

#[test]
fn nft_decode_rejects_plain_string_payload() {
    let payload = b"rose:uuid:0000$domain";
    let mut cursor = Cursor::new(&payload[..]);
    assert!(NftId::decode(&mut cursor).is_err());
}
