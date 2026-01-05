//! SM norito roundtrip tests covering signature and public key encoding.

#[cfg(feature = "sm")]
mod tests {
    use iroha_crypto::{Algorithm, PublicKey, Signature, Sm3Digest};
    use iroha_data_model::account::AccountId;
    use norito::NoritoDeserialize;

    const SM2_PUB: &str = "8626550012414C494345313233405941484F4F2E434F4D040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857";
    const SM2_SIG: &str = "40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D16FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7";

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
        let domain = "sm2domain".parse().expect("domain");
        let pk: PublicKey = SM2_PUB.parse().expect("parse sm2 pk");
        let account_id = AccountId::new(domain, pk.clone());
        let mut buf = Vec::new();
        norito::core::NoritoSerialize::serialize(&account_id, &mut buf)
            .expect("serialize AccountId");
        let (decoded, used) = <AccountId as norito::core::DecodeFromSlice>::decode_from_slice(&buf)
            .expect("decode AccountId");
        assert_eq!(used, buf.len());
        assert_eq!(decoded.signatory().to_string(), pk.to_string());
    }
}
