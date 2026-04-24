//! SM norito roundtrip tests covering signature and public key encoding.

#[cfg(feature = "sm")]
mod tests {
    use iroha_crypto::{Algorithm, PublicKey, Signature, Sm3Digest};
    use iroha_data_model::account::AccountId;
    use norito::NoritoDeserialize;

    const SM2_PUB: &str = "86265300103132333435363738313233343536373804361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F";
    const SM2_SIG: &str = "1877845D5FFE0305946EEA3046D0279BE886B866EF620B7325413602CAD17C7FF72EBF26C29E77AAAB2226EDFBEE2D6D6ABC0D6C9B2C9A2248E2BD9324A12268";

    #[test]
    fn sm2_public_key_multihash_parse_roundtrip() {
        let pk: PublicKey = SM2_PUB.parse().expect("parse SM2 multihash");
        assert_eq!(pk.algorithm(), Algorithm::Sm2);
        let encoded = pk.to_string();
        assert_eq!(encoded, SM2_PUB);
    }

    #[test]
    fn sm2_public_key_norito_roundtrip() {
        let pk: PublicKey = SM2_PUB.parse().expect("parse SM2 multihash");
        let mut buf = Vec::new();
        norito::core::NoritoSerialize::serialize(&pk, &mut buf).expect("serialize sm2 pk");
        let (decoded, used) = <PublicKey as norito::core::DecodeFromSlice>::decode_from_slice(&buf)
            .expect("decode sm2 pk");
        assert_eq!(used, buf.len());
        assert_eq!(decoded, pk);
    }

    #[test]
    fn sm2_signature_norito_roundtrip() {
        let sig_bytes = hex::decode(SM2_SIG).expect("hex signature");
        let signature = Signature::from_bytes(&sig_bytes);
        let mut buf = Vec::new();
        norito::core::NoritoSerialize::serialize(&signature, &mut buf).expect("serialize sm2 sig");
        let (decoded, used) = <Signature as norito::core::DecodeFromSlice>::decode_from_slice(&buf)
            .expect("decode sm2 sig");
        assert_eq!(used, buf.len());
        assert_eq!(decoded.payload(), signature.payload());
    }

    #[test]
    fn sm3_digest_norito_roundtrip() {
        let digest = Sm3Digest::hash(b"iroha-sm3");
        let mut bare = Vec::new();
        norito::core::NoritoSerialize::serialize(&digest, &mut bare).expect("serialize sm3 digest");
        let (decoded_bare, used) =
            <Sm3Digest as norito::core::DecodeFromSlice>::decode_from_slice(&bare)
                .expect("decode bare sm3 digest");
        assert_eq!(used, bare.len());
        assert_eq!(decoded_bare.as_bytes(), digest.as_bytes());
        let framed = norito::core::frame_bare_with_header_flags::<Sm3Digest>(&bare, 0)
            .expect("frame sm3 digest with header");
        let archived = norito::core::from_bytes::<Sm3Digest>(&framed).expect("archived sm3 digest");
        let decoded = Sm3Digest::deserialize(archived);
        assert_eq!(decoded.as_bytes(), digest.as_bytes());
    }

    #[test]
    fn account_id_with_sm2_signatory_roundtrip() {
        let pk: PublicKey = SM2_PUB.parse().expect("parse sm2 pk");
        let account_id = AccountId::new(pk.clone());
        let mut buf = Vec::new();
        norito::core::NoritoSerialize::serialize(&account_id, &mut buf)
            .expect("serialize AccountId");
        let (decoded, used) = <AccountId as norito::core::DecodeFromSlice>::decode_from_slice(&buf)
            .expect("decode AccountId");
        assert_eq!(used, buf.len());
        assert_eq!(decoded.signatory().to_string(), pk.to_string());
    }
}
