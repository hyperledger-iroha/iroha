//! Golden vectors for Jurisdiction Roots Merkle composition.
use iroha_crypto::Hash;

fn hex_of(h: &Hash) -> String {
    // Hash implements Display as lowercase hex; use that for stability.
    format!("{h}")
}

#[test]
fn jurisdiction_merkle_golden() {
    // Sample canonical IDs and roots
    let jur1 = b"JUR1";
    let jur2 = b"JUR2";
    let root1 = [0x11u8; Hash::LENGTH];
    let root2 = [0x22u8; Hash::LENGTH];

    // Leaves: H(0x00 || canonical(JurisdictionId) || jurisdiction_root)
    let leaf1 = Hash::new([&[0x00u8][..], jur1, &root1[..]].concat());
    let leaf2 = Hash::new([&[0x00u8][..], jur2, &root2[..]].concat());

    assert_eq!(
        hex_of(&leaf1),
        "c0584eb03a83a00557b4ba5d598ab0b9ce6903f5699fc6ae2c975380ada9ff2d"
    );
    assert_eq!(
        hex_of(&leaf2),
        "090e6f3d4de5829428772dd8808720b18f1cec051d4817f6b75b1265f94cb40f"
    );

    // Root: H(0x01 || leaf1 || leaf2) with non-commutative left||right
    let preimage = [&[0x01u8][..], leaf1.as_ref(), leaf2.as_ref()].concat();
    let root = Hash::new(preimage);
    assert_eq!(
        hex_of(&root),
        "521c0ec11759f7d86db193e57adff5f862efd55f20b25720b5c8697d97157cd7"
    );
}
