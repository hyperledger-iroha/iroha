//! Verify that Norito roundtrips for `SignedBlock` even when signatures carry
//! packed `ConstVec<u8>` payloads. Reproduces the regression reported for
//! `SignedBlock` decoding failing on transaction signatures.

use iroha_crypto::{KeyPair, MerkleTree};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::{SignedBlock, decode_framed_signed_block},
    isi::InstructionBox,
    transaction::signed::{SignedTransaction, TransactionBuilder},
};

fn sample_signed_block_with_empty_instructions() -> (SignedBlock, Vec<SignedTransaction>) {
    let keypair = KeyPair::random();
    let chain: ChainId = "constvec-roundtrip-chain"
        .parse()
        .expect("chain id for sample block");
    let authority: AccountId =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245@wonderland"
            .parse()
            .expect("account id for sample block");

    let txs = vec![
        TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions(core::iter::empty::<InstructionBox>())
            .sign(keypair.private_key()),
        TransactionBuilder::new(chain, authority)
            .with_instructions(core::iter::empty::<InstructionBox>())
            .sign(keypair.private_key()),
    ];
    let block = SignedBlock::genesis(txs.clone(), keypair.private_key(), None, None);
    (block, txs)
}

#[test]
fn signed_block_roundtrip_via_norito() {
    let (block, txs) = sample_signed_block_with_empty_instructions();

    for (idx, tx) in txs.iter().enumerate() {
        let mut buf = Vec::new();
        norito::core::NoritoSerialize::serialize(tx, &mut buf)
            .expect("serialize signed transaction");
        println!(
            "tx {} encoded len {} first bytes {:?}",
            idx,
            buf.len(),
            &buf[..32.min(buf.len())]
        );
        let (decoded_tx, used) =
            <SignedTransaction as norito::core::DecodeFromSlice>::decode_from_slice(&buf)
                .expect("decode signed tx");
        println!("tx {idx} decode used {used}");
        assert_eq!(decoded_tx, *tx);
        let mut cursor = buf.as_slice();
        let codec_decoded = <SignedTransaction as norito::codec::Decode>::decode(&mut cursor)
            .expect("codec decode signed tx");
        assert_eq!(codec_decoded, *tx);
    }

    // Audit the constructed block structure before serialization.
    assert_eq!(
        block.signatures().count(),
        1,
        "genesis must have one signature"
    );
    assert_eq!(
        block.transactions_vec().len(),
        txs.len(),
        "payload should retain all genesis transactions"
    );
    assert!(
        block.has_results(),
        "genesis blocks now carry an empty results envelope for deterministic hashing"
    );

    let expected_hashes: Vec<_> = txs
        .iter()
        .map(SignedTransaction::hash_as_entrypoint)
        .collect();
    let mut merkle = MerkleTree::default();
    for hash in &expected_hashes {
        merkle.add(*hash);
    }
    assert_eq!(
        block.header().merkle_root(),
        merkle.root(),
        "header merkle root must match recomputed transaction merkle tree"
    );
    assert!(
        block.header().result_merkle_root().is_none(),
        "result merkle root remains unset for empty results"
    );

    let tx_sig_len = {
        let tx = block.transactions_vec().first().expect("tx");
        let mut buf = Vec::new();
        norito::core::NoritoSerialize::serialize(tx.signature(), &mut buf)
            .expect("serialize signature");
        buf.len()
    };
    println!("transaction signature serialized len {tx_sig_len}");

    let bytes = norito::codec::Encode::encode(&block);
    let mut cursor = bytes.as_slice();
    println!("signed block encoded len {}", bytes.len());
    println!("first 64 bytes {:?}", &bytes[..64.min(bytes.len())]);
    let decoded =
        <SignedBlock as norito::codec::Decode>::decode(&mut cursor).expect("decode block");

    assert_eq!(decoded, block);
}

#[test]
fn canonical_signed_block_wire_roundtrip_is_sequential() {
    let (block, _) = sample_signed_block_with_empty_instructions();
    let wire = block.canonical_wire().expect("canonical wire encoding");
    let framed = wire.as_framed();
    assert!(
        framed.len() > norito::core::Header::SIZE,
        "canonical frame must include Norito header"
    );
    let header_flags = framed[1 + norito::core::Header::SIZE - 1];
    assert_eq!(
        header_flags, 0,
        "canonical header must not advertise layout flags"
    );
    let decoded = decode_framed_signed_block(framed).expect("decode framed block");
    assert_eq!(
        decoded, block,
        "framed canonical bytes should round-trip through Norito decode"
    );
}
