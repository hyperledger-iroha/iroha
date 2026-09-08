//! Independent native check of browser NFT sale bytes and canonical content/offer commitments.
#![cfg(feature = "json")]
use iroha_crypto::Hash;
use iroha_data_model::{
    isi::{InstructionBox, nft_market::*},
    metadata::Metadata,
    nft_market::*,
};
use norito::{
    codec::Encode,
    json::{self, JsonDeserialize, Value},
};

#[derive(norito::derive::JsonDeserialize)]
struct Fixtures {
    version: u16,
    metadata_hash: Hash,
    offer_hash: Hash,
    vectors: Vec<Vector>,
}
#[derive(norito::derive::JsonDeserialize)]
struct Vector {
    name: String,
    value: Value,
    encoded_hex: String,
    instruction_hex: Option<String>,
}
fn check<T: Encode + JsonDeserialize>(vector: &Vector) -> T {
    let value: T = json::from_value(vector.value.clone()).expect(&vector.name);
    assert_eq!(
        hex::encode_upper(value.encode()),
        vector.encoded_hex,
        "{} native bytes",
        vector.name
    );
    value
}
fn instruction<T: Encode + JsonDeserialize + Into<InstructionBox>>(vector: &Vector) {
    let value: T = check(vector);
    let instruction: InstructionBox = value.into();
    assert_eq!(
        hex::encode_upper(instruction.encode()),
        vector.instruction_hex.as_deref().unwrap(),
        "{} native instruction bytes",
        vector.name
    );
}
#[test]
fn browser_nft_market_values_and_commitments_match_native() {
    let fixtures: Fixtures = json::from_str(include_str!(
        "../../../javascript/iroha_js/test/fixtures/nft-market-v1-codec.json"
    ))
    .unwrap();
    assert_eq!(fixtures.version, 1);
    assert_eq!(fixtures.vectors.len(), 8);
    for vector in &fixtures.vectors {
        match vector.name.as_str() {
            "Metadata" => {
                let metadata: Metadata = check(vector);
                assert_eq!(Hash::new(metadata.encode()), fixtures.metadata_hash);
            }
            "NftSaleOfferV1" => {
                let offer: NftSaleOfferV1 = check(vector);
                assert_eq!(offer.commitment(), fixtures.offer_hash);
            }
            "NftSaleRecordV1" => {
                let _: NftSaleRecordV1 = check(vector);
            }
            "OfferNftV1" => instruction::<OfferNftV1>(vector),
            "BuyNftV1" => instruction::<BuyNftV1>(vector),
            "CancelNftOfferV1" => instruction::<CancelNftOfferV1>(vector),
            _ => panic!("unknown NFT fixture"),
        }
    }
}
