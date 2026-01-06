//! Module with multihash implementation

use std::{
    format,
    string::{String, ToString as _},
    vec,
    vec::Vec,
};

use derive_more::Display;

use crate::{Algorithm, ParseError, hex_decode, varint};

pub fn decode_public_key(bytes: &[u8]) -> Result<(Algorithm, Vec<u8>), ParseError> {
    let (digest_function, payload) = decode_multihash(bytes)?;
    let algorithm = digest_function_public::decode(digest_function)?;
    Ok((algorithm, payload))
}

pub fn encode_public_key(
    algorithm: Algorithm,
    payload: &[u8],
) -> Result<Vec<u8>, MultihashConvertError> {
    let digest_function = digest_function_public::encode(algorithm);
    encode_multihash(digest_function, payload)
}

pub fn decode_private_key(bytes: &[u8]) -> Result<(Algorithm, Vec<u8>), ParseError> {
    let (digest_function, payload) = decode_multihash(bytes)?;
    let algorithm = digest_function_private::decode(digest_function)?;
    Ok((algorithm, payload))
}

pub fn encode_private_key(
    algorithm: Algorithm,
    payload: &[u8],
) -> Result<Vec<u8>, MultihashConvertError> {
    let digest_function = digest_function_private::encode(algorithm);
    encode_multihash(digest_function, payload)
}

/// Format a multihash as a canonical hex string.
///
/// # Errors
///
/// Returns an error when the input is not a canonical multihash payload.
pub fn multihash_to_hex_string(bytes: &[u8]) -> Result<String, ParseError> {
    // Format as: <varint fn-code lower><varint len lower><payload upper>
    // Length may span multiple varint bytes for larger keys (e.g., ML‑DSA).
    let (digest_function, payload) = decode_multihash(bytes)?;
    Ok(format_multihash_hex(digest_function, &payload))
}

fn format_multihash_hex(digest_function: DigestFunction, payload: &[u8]) -> String {
    let df_varint: varint::VarUint = digest_function.into();
    let df_bytes: Vec<u8> = df_varint.into();
    let len_varint: varint::VarUint = (payload.len() as u64).into();
    let len_bytes: Vec<u8> = len_varint.into();

    let fn_code = hex::encode(df_bytes);
    let dig_size = hex::encode(len_bytes);
    let key = hex::encode_upper(payload);

    format!("{fn_code}{dig_size}{key}")
}

/// Encode a public key into an algorithm-prefixed multihash hex string, e.g. "ed25519:...".
#[cfg(not(feature = "ffi_import"))]
pub fn encode_public_key_prefixed(
    algorithm: Algorithm,
    payload: &[u8],
) -> Result<String, MultihashConvertError> {
    let mh = encode_public_key(algorithm, payload)?;
    Ok(format!(
        "{}:{}",
        algorithm.as_static_str(),
        multihash_to_hex_string(&mh).map_err(|err| MultihashConvertError::new(err.to_string()))?
    ))
}

/// Encode a private key into an algorithm-prefixed multihash hex string, e.g. "ml-dsa:...".
#[cfg(not(feature = "ffi_import"))]
pub fn encode_private_key_prefixed(
    algorithm: Algorithm,
    payload: &[u8],
) -> Result<String, MultihashConvertError> {
    let mh = encode_private_key(algorithm, payload)?;
    Ok(format!(
        "{}:{}",
        algorithm.as_static_str(),
        multihash_to_hex_string(&mh).map_err(|err| MultihashConvertError::new(err.to_string()))?
    ))
}

/// Decode a public key from either a bare multihash hex string or an
/// algorithm-prefixed form like "ed25519:<multihash-hex>".
/// Input must be canonical multihash hex (varint bytes lowercase, payload uppercase);
/// `0x` prefixes are rejected.
#[cfg(not(feature = "ffi_import"))]
pub fn decode_public_key_str(s: &str) -> Result<(Algorithm, Vec<u8>), ParseError> {
    if let Some((alg_str, rest)) = s.split_once(':') {
        let algorithm = alg_str
            .parse::<Algorithm>()
            .map_err(|_| ParseError(format!("Unknown algorithm prefix: {alg_str}")))?;
        let bytes = decode_multihash_hex_bytes(rest)?;
        let (alg_from_mh, payload) = decode_public_key(&bytes)?;
        if alg_from_mh != algorithm {
            return Err(ParseError(
                "Algorithm prefix does not match multihash".to_string(),
            ));
        }
        Ok((algorithm, payload))
    } else {
        let bytes = decode_multihash_hex_bytes(s)?;
        decode_public_key(&bytes)
    }
}

/// Decode a private key from either a bare multihash hex string or an
/// algorithm-prefixed form like "ml-dsa:<multihash-hex>".
/// Input must be canonical multihash hex (varint bytes lowercase, payload uppercase);
/// `0x` prefixes are rejected.
#[cfg(not(feature = "ffi_import"))]
pub fn decode_private_key_str(s: &str) -> Result<(Algorithm, Vec<u8>), ParseError> {
    if let Some((alg_str, rest)) = s.split_once(':') {
        let algorithm = alg_str
            .parse::<Algorithm>()
            .map_err(|_| ParseError(format!("Unknown algorithm prefix: {alg_str}")))?;
        let bytes = decode_multihash_hex_bytes(rest)?;
        let (alg_from_mh, payload) = decode_private_key(&bytes)?;
        if alg_from_mh != algorithm {
            return Err(ParseError(
                "Algorithm prefix does not match multihash".to_string(),
            ));
        }
        Ok((algorithm, payload))
    } else {
        let bytes = decode_multihash_hex_bytes(s)?;
        decode_private_key(&bytes)
    }
}

#[cfg(not(feature = "ffi_import"))]
fn decode_multihash_hex_bytes(s: &str) -> Result<Vec<u8>, ParseError> {
    let bytes = hex_decode(s)?;
    let (digest_function, payload) = decode_multihash(&bytes)?;
    let canonical = format_multihash_hex(digest_function, &payload);
    if s != canonical {
        return Err(ParseError("Non-canonical multihash hex".to_string()));
    }
    Ok(bytes)
}

/// Value of byte code corresponding to algorithm.
/// See [official multihash table](https://github.com/multiformats/multicodec/blob/master/table.csv)
type DigestFunction = u64;

mod digest_function_public {
    use std::string::String;

    use crate::{Algorithm, error::ParseError, multihash::DigestFunction};

    const ED_25519: DigestFunction = 0xed;
    const SECP_256_K1: DigestFunction = 0xe7;
    #[cfg(feature = "ml-dsa")]
    // Provisional multicodec for ML‑DSA (Dilithium3) public keys; align with upstream when assigned.
    const ML_DSA_DILITHIUM3_PK: DigestFunction = 0xee;
    #[cfg(feature = "gost")]
    // Provisional multicodec assignments for TC26 parameter sets; replace with canonical codes once allocated.
    const GOST_3410_2012_256_A: DigestFunction = 0x1200;
    #[cfg(feature = "gost")]
    const GOST_3410_2012_256_B: DigestFunction = 0x1201;
    #[cfg(feature = "gost")]
    const GOST_3410_2012_256_C: DigestFunction = 0x1202;
    #[cfg(feature = "gost")]
    const GOST_3410_2012_512_A: DigestFunction = 0x1203;
    #[cfg(feature = "gost")]
    const GOST_3410_2012_512_B: DigestFunction = 0x1204;
    #[cfg(feature = "bls")]
    const BLS12_381_G1: DigestFunction = 0xea;
    #[cfg(feature = "bls")]
    const BLS12_381_G2: DigestFunction = 0xeb;
    #[cfg(feature = "sm")]
    // Provisional multicodec assignment; replace once canonical code is allocated.
    const SM2_PUB: DigestFunction = 0x1306;

    pub fn decode(digest_function: DigestFunction) -> Result<Algorithm, ParseError> {
        let algorithm = match digest_function {
            ED_25519 => Algorithm::Ed25519,
            SECP_256_K1 => Algorithm::Secp256k1,
            #[cfg(feature = "bls")]
            BLS12_381_G1 => Algorithm::BlsNormal,
            #[cfg(feature = "bls")]
            BLS12_381_G2 => Algorithm::BlsSmall,
            #[cfg(feature = "ml-dsa")]
            ML_DSA_DILITHIUM3_PK => Algorithm::MlDsa,
            #[cfg(feature = "gost")]
            GOST_3410_2012_256_A => Algorithm::Gost3410_2012_256ParamSetA,
            #[cfg(feature = "gost")]
            GOST_3410_2012_256_B => Algorithm::Gost3410_2012_256ParamSetB,
            #[cfg(feature = "gost")]
            GOST_3410_2012_256_C => Algorithm::Gost3410_2012_256ParamSetC,
            #[cfg(feature = "gost")]
            GOST_3410_2012_512_A => Algorithm::Gost3410_2012_512ParamSetA,
            #[cfg(feature = "gost")]
            GOST_3410_2012_512_B => Algorithm::Gost3410_2012_512ParamSetB,
            #[cfg(feature = "sm")]
            SM2_PUB => Algorithm::Sm2,
            _ => return Err(ParseError(String::from("No such algorithm"))),
        };
        Ok(algorithm)
    }

    pub fn encode(algorithm: Algorithm) -> u64 {
        match algorithm {
            Algorithm::Ed25519 => ED_25519,
            Algorithm::Secp256k1 => SECP_256_K1,
            #[cfg(feature = "ml-dsa")]
            Algorithm::MlDsa => ML_DSA_DILITHIUM3_PK,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetA => GOST_3410_2012_256_A,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetB => GOST_3410_2012_256_B,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetC => GOST_3410_2012_256_C,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_512ParamSetA => GOST_3410_2012_512_A,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_512ParamSetB => GOST_3410_2012_512_B,
            #[cfg(feature = "bls")]
            Algorithm::BlsNormal => BLS12_381_G1,
            #[cfg(feature = "bls")]
            Algorithm::BlsSmall => BLS12_381_G2,
            #[cfg(feature = "sm")]
            Algorithm::Sm2 => SM2_PUB,
        }
    }
}

mod digest_function_private {
    use std::string::String;

    use crate::{Algorithm, error::ParseError, multihash::DigestFunction};

    const ED_25519: DigestFunction = 0x1300;
    const SECP_256_K1: DigestFunction = 0x1301;
    #[cfg(feature = "ml-dsa")]
    // Provisional multicodec for ML‑DSA (Dilithium3) private keys; align with upstream when assigned.
    const ML_DSA_DILITHIUM3_SK: DigestFunction = 0x130b;
    #[cfg(feature = "gost")]
    // Provisional multicodec assignments for TC26 private keys.
    const GOST_3410_2012_256_A: DigestFunction = 0x130c;
    #[cfg(feature = "gost")]
    const GOST_3410_2012_256_B: DigestFunction = 0x130d;
    #[cfg(feature = "gost")]
    const GOST_3410_2012_256_C: DigestFunction = 0x130e;
    #[cfg(feature = "gost")]
    const GOST_3410_2012_512_A: DigestFunction = 0x130f;
    #[cfg(feature = "gost")]
    const GOST_3410_2012_512_B: DigestFunction = 0x1310;
    #[cfg(feature = "bls")]
    const BLS12_381_G1: DigestFunction = 0x1309;
    #[cfg(feature = "bls")]
    const BLS12_381_G2: DigestFunction = 0x130a;
    #[cfg(feature = "sm")]
    // Provisional multicodec assignment; replace once canonical code is allocated.
    const SM2_PRIV: DigestFunction = 0x1311;

    pub fn decode(digest_function: DigestFunction) -> Result<Algorithm, ParseError> {
        let algorithm = match digest_function {
            ED_25519 => Algorithm::Ed25519,
            SECP_256_K1 => Algorithm::Secp256k1,
            #[cfg(feature = "bls")]
            BLS12_381_G1 => Algorithm::BlsNormal,
            #[cfg(feature = "bls")]
            BLS12_381_G2 => Algorithm::BlsSmall,
            #[cfg(feature = "ml-dsa")]
            ML_DSA_DILITHIUM3_SK => Algorithm::MlDsa,
            #[cfg(feature = "gost")]
            GOST_3410_2012_256_A => Algorithm::Gost3410_2012_256ParamSetA,
            #[cfg(feature = "gost")]
            GOST_3410_2012_256_B => Algorithm::Gost3410_2012_256ParamSetB,
            #[cfg(feature = "gost")]
            GOST_3410_2012_256_C => Algorithm::Gost3410_2012_256ParamSetC,
            #[cfg(feature = "gost")]
            GOST_3410_2012_512_A => Algorithm::Gost3410_2012_512ParamSetA,
            #[cfg(feature = "gost")]
            GOST_3410_2012_512_B => Algorithm::Gost3410_2012_512ParamSetB,
            #[cfg(feature = "sm")]
            SM2_PRIV => Algorithm::Sm2,
            _ => return Err(ParseError(String::from("No such algorithm"))),
        };
        Ok(algorithm)
    }

    pub fn encode(algorithm: Algorithm) -> u64 {
        match algorithm {
            Algorithm::Ed25519 => ED_25519,
            Algorithm::Secp256k1 => SECP_256_K1,
            #[cfg(feature = "ml-dsa")]
            Algorithm::MlDsa => ML_DSA_DILITHIUM3_SK,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetA => GOST_3410_2012_256_A,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetB => GOST_3410_2012_256_B,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetC => GOST_3410_2012_256_C,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_512ParamSetA => GOST_3410_2012_512_A,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_512ParamSetB => GOST_3410_2012_512_B,
            #[cfg(feature = "bls")]
            Algorithm::BlsNormal => BLS12_381_G1,
            #[cfg(feature = "bls")]
            Algorithm::BlsSmall => BLS12_381_G2,
            #[cfg(feature = "sm")]
            Algorithm::Sm2 => SM2_PRIV,
        }
    }
}

fn decode_multihash(bytes: &[u8]) -> Result<(DigestFunction, Vec<u8>), ParseError> {
    // Parse varint-encoded function code
    let idx_fn_end = bytes
        .iter()
        .enumerate()
        .find(|&(_, &byte)| (byte & 0b1000_0000) == 0)
        .ok_or_else(|| ParseError(String::from("Failed to find end of function code varint")))?
        .0;

    let (fn_varint, rest) = bytes.split_at(idx_fn_end + 1);

    let digest_function: u64 = varint::VarUint::new(fn_varint)
        .map_err(|err| ParseError(err.to_string()))?
        .try_into()
        .map_err(|err: varint::ConvertError| ParseError(err.to_string()))?;

    // Parse varint-encoded digest length
    let idx_len_end = rest
        .iter()
        .enumerate()
        .find(|&(_, &byte)| (byte & 0b1000_0000) == 0)
        .ok_or_else(|| ParseError(String::from("Digest size not found")))?
        .0;
    let (len_varint, payload) = rest.split_at(idx_len_end + 1);
    let digest_size: u64 = varint::VarUint::new(len_varint)
        .map_err(|err| ParseError(err.to_string()))?
        .try_into()
        .map_err(|err: varint::ConvertError| ParseError(err.to_string()))?;

    if payload.len() as u64 != digest_size {
        return Err(ParseError(String::from(
            "Digest size not equal to actual length",
        )));
    }
    Ok((digest_function, payload.to_vec()))
}

#[allow(clippy::unnecessary_wraps)]
fn encode_multihash(
    digest_function: DigestFunction,
    payload: &[u8],
) -> Result<Vec<u8>, MultihashConvertError> {
    let mut out = vec![];

    // varint-encode function code
    let df_varint: varint::VarUint = digest_function.into();
    let mut df_bytes: Vec<u8> = df_varint.into();
    out.append(&mut df_bytes);

    // varint-encode payload length (supports large keys, e.g., ML‑DSA)
    let len_u64 = payload.len() as u64;
    let len_varint: varint::VarUint = len_u64.into();
    let len_bytes: Vec<u8> = len_varint.into();
    out.extend_from_slice(&len_bytes);

    // payload
    out.extend_from_slice(payload);
    Ok(out)
}

/// Error which occurs when converting to/from `Multihash`
#[derive(Debug, Clone, Display)]
pub struct MultihashConvertError {
    reason: String,
}

impl MultihashConvertError {
    #[allow(dead_code)]
    pub(crate) const fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl std::error::Error for MultihashConvertError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex_decode;
    #[cfg(feature = "sm")]
    use crate::sm::encode_sm2_public_key_payload;

    #[test]
    fn test_encode_public_key() {
        let algorithm = Algorithm::Ed25519;
        let payload =
            hex_decode("1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4").unwrap();
        let multihash =
            hex_decode("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4")
                .unwrap();
        assert_eq!(encode_public_key(algorithm, &payload).unwrap(), multihash);
    }

    #[cfg(feature = "sm")]
    #[test]
    fn test_encode_sm2_public_key() {
        let algorithm = Algorithm::Sm2;
        let distid = "ALICE123@YAHOO.COM";
        let sec1 = hex_decode("040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857").unwrap();
        let payload = encode_sm2_public_key_payload(distid, &sec1).expect("sm2 payload");
        let multihash = hex_decode("8626550012414C494345313233405941484F4F2E434F4D040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857").unwrap();
        assert_eq!(encode_public_key(algorithm, &payload).unwrap(), multihash);
    }

    #[test]
    fn test_decode_public_key() {
        let algorithm = Algorithm::Ed25519;
        let payload =
            hex_decode("1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4").unwrap();
        let multihash =
            hex_decode("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4")
                .unwrap();
        assert_eq!(decode_public_key(&multihash).unwrap(), (algorithm, payload));
    }

    #[test]
    fn multihash_to_hex_string_formats_canonical() {
        let multihash =
            hex_decode("ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4")
                .unwrap();
        let formatted = multihash_to_hex_string(&multihash).expect("format");
        assert_eq!(
            formatted,
            "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        );
    }

    #[test]
    fn multihash_to_hex_string_rejects_truncated_input() {
        assert!(multihash_to_hex_string(&[]).is_err());
        assert!(multihash_to_hex_string(&[0x01]).is_err());
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn decode_public_key_str_rejects_non_canonical_hex() {
        let canonical = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4";
        let lower = canonical.to_lowercase();
        assert!(decode_public_key_str(&lower).is_err());
        let upper = canonical.to_uppercase();
        assert!(decode_public_key_str(&upper).is_err());
        let prefixed = format!("0x{canonical}");
        assert!(decode_public_key_str(&prefixed).is_err());
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn decode_private_key_str_rejects_non_canonical_hex() {
        let canonical = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F";
        let lower = canonical.to_lowercase();
        assert!(decode_private_key_str(&lower).is_err());
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn decode_public_key_str_rejects_non_canonical_varint() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xed, 0x01]);
        bytes.extend_from_slice(&[0xa0, 0x00]);
        bytes.extend_from_slice(&[0u8; 32]);
        let input = hex::encode(bytes);
        assert!(decode_public_key_str(&input).is_err());
    }

    #[test]
    fn test_encode_private_key() {
        let algorithm = Algorithm::Ed25519;
        let payload =
            hex_decode("8F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F").unwrap();
        let multihash =
            hex_decode("8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F")
                .unwrap();
        assert_eq!(encode_private_key(algorithm, &payload).unwrap(), multihash);
    }

    #[test]
    fn test_decode_private_key() {
        let algorithm = Algorithm::Ed25519;
        let payload =
            hex_decode("8F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F").unwrap();
        let multihash =
            hex_decode("8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F")
                .unwrap();
        assert_eq!(
            decode_private_key(&multihash).unwrap(),
            (algorithm, payload)
        );
    }

    #[cfg(feature = "gost")]
    #[test]
    fn test_gost_public_key_multihash_roundtrip() {
        let cases = [
            (Algorithm::Gost3410_2012_256ParamSetA, 32usize),
            (Algorithm::Gost3410_2012_256ParamSetB, 32),
            (Algorithm::Gost3410_2012_256ParamSetC, 32),
            (Algorithm::Gost3410_2012_512ParamSetA, 64),
            (Algorithm::Gost3410_2012_512ParamSetB, 64),
        ];

        for (index, (algorithm, len)) in cases.into_iter().enumerate() {
            let mut payload = vec![0u8; len];
            for (offset, byte) in payload.iter_mut().enumerate() {
                *byte = u8::try_from((index + offset) % 256).expect("value fits into u8");
            }
            let encoded = encode_public_key(algorithm, &payload).expect("encode");
            let (decoded_alg, decoded_payload) =
                decode_public_key(&encoded).expect("decode public key");
            assert_eq!(algorithm, decoded_alg);
            assert_eq!(payload, decoded_payload);
        }
    }

    #[cfg(feature = "gost")]
    #[test]
    fn test_gost_private_key_multihash_roundtrip() {
        let cases = [
            (Algorithm::Gost3410_2012_256ParamSetA, 32usize),
            (Algorithm::Gost3410_2012_256ParamSetB, 32),
            (Algorithm::Gost3410_2012_256ParamSetC, 32),
            (Algorithm::Gost3410_2012_512ParamSetA, 64),
            (Algorithm::Gost3410_2012_512ParamSetB, 64),
        ];

        for (index, (algorithm, len)) in cases.into_iter().enumerate() {
            let mut payload = vec![0u8; len];
            for (offset, byte) in payload.iter_mut().enumerate() {
                *byte = u8::try_from((index * 3 + offset) % 256).expect("value fits into u8");
            }
            let encoded = encode_private_key(algorithm, &payload).expect("encode");
            let (decoded_alg, decoded_payload) =
                decode_private_key(&encoded).expect("decode private key");
            assert_eq!(algorithm, decoded_alg);
            assert_eq!(payload, decoded_payload);
        }
    }

    #[cfg(feature = "sm")]
    #[test]
    fn test_sm2_public_key_multihash_roundtrip() {
        let algorithm = Algorithm::Sm2;
        let distid = "sm2-multihash-test";
        let mut sec1 = vec![0u8; 65];
        for (idx, byte) in sec1.iter_mut().enumerate() {
            *byte = u8::try_from(idx % 256).expect("value fits into u8");
        }
        let payload =
            crate::sm::encode_sm2_public_key_payload(distid, &sec1).expect("encode SM2 payload");
        let encoded = encode_public_key(algorithm, &payload).expect("encode sm2 pk");
        let (decoded_alg, decoded_payload) = decode_public_key(&encoded).expect("decode sm2 pk");
        assert_eq!(decoded_alg, algorithm);
        assert_eq!(decoded_payload, payload);
    }
}
