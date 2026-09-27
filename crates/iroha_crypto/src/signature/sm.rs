use sm2::dsa::Signature as Sm2Signature;
/// Parse a canonical SM2 signature (r∥s) and surface the underlying error type.
pub(crate) fn parse_signature(bytes: &[u8; 64]) -> Result<Sm2Signature, signature::Error> {
    Sm2Signature::from_bytes(bytes)
}
#[cfg(test)]
mod tests {
    use sm4::cipher::{Block, BlockEncrypt, KeyInit};
    #[test]
    fn sm4_zero_key_zero_block_matches_known_answer_vector() {
        let key = [0u8; 16];
        let cipher = sm4::Sm4::new((&key).into());
        let mut block = Block::<sm4::Sm4>::default();
        cipher.encrypt_block(&mut block);
        assert_eq!(
            block.as_slice(),
            [
                0x9f, 0x1f, 0x7b, 0xff, 0x6f, 0x55, 0x11, 0x38, 0x4d, 0x94, 0x30, 0x53, 0x1e, 0x53,
                0x8f, 0xd3,
            ]
        );
    }
}
