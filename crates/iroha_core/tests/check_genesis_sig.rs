use iroha_crypto::{PublicKey, SignatureOf};
use iroha_data_model::block::{
    BlockHeader, decode_framed_signed_block, decode_versioned_signed_block,
};
use std::fs::File;
use std::io::Read;
use std::str::FromStr;

#[test]
fn check_genesis_signature() {
    let genesis_path = "/tmp/iroha-localnet-7peer/genesis.signed.nrt";
    let pub_key_str = "ed0120EEF765223920C4D7D7ED4E204DCBDF3DAFE37F53B11F155D78206F24BC232646";

    let mut file = File::open(genesis_path).expect("open genesis");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("read genesis");

    let block = decode_framed_signed_block(&bytes)
        .or_else(|_| decode_versioned_signed_block(&bytes))
        .expect("decode genesis block");

    let pub_key = PublicKey::from_str(pub_key_str).expect("parse pub key");

    println!("Genesis hash: {:?}", block.hash());
    println!("Signatures: {:?}", block.signatures().count());

    for sig in block.signatures() {
        println!("Verifying signature against {:?}", pub_key);
        let signature: &SignatureOf<BlockHeader> = sig.signature();
        signature
            .verify_hash(&pub_key, block.hash())
            .expect("verify signature");
    }
}
