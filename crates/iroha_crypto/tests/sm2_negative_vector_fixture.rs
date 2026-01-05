//! Shared helpers for SM2 negative vector fixtures sourced from upstream suites.
#![cfg(feature = "sm")]

use hex::decode;
use hex_literal::hex;
use iroha_crypto::{Sm2PrivateKey, Sm2PublicKey};
use norito::json::{self, Value};

/// Canonical SM2 curve order (GM/T 0003-2012, Fp-256).
pub const ORDER: [u8; 32] =
    hex!("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFF7203DF6B21C6052B53BBF40939D54123");

/// Value just above the canonical order (used by upstream overflow tests).
pub const ORDER_PLUS_ONE: [u8; 32] =
    hex!("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFF7203DF6B21C6052B53BBF40939D54124");

/// Default SM2 distinguishing identifier used across the fixtures.
pub const DEFAULT_DISTID: &str = Sm2PublicKey::DEFAULT_DISTID;

/// Alternate distinguishing identifier used by `GmSSL` negative vectors.
const ALT_DISTID: &str = "gmssl:alternate-distid";

/// Parsed negative verification vector.
#[derive(Clone, Debug)]
pub struct NegativeVector {
    /// Human-readable vector identifier.
    pub label: String,
    /// Mutation applied to the deterministic signature/public key.
    pub mutation: String,
    /// Original message used when generating the signature.
    pub message: Vec<u8>,
    /// Message passed to verification (may differ for tamper cases).
    pub verify_message: Vec<u8>,
}

/// Result of applying a mutation to a freshly generated signature.
#[derive(Clone, Debug)]
pub struct MutationResult {
    /// Signature bytes after mutation (may be non-canonical length).
    pub signature_bytes: Vec<u8>,
    /// Message to verify against (mirrors [`NegativeVector::verify_message`]).
    pub verify_message: Vec<u8>,
    /// Public key used for verification (possibly mutated).
    pub public_key: Sm2PublicKey,
    /// Indicates the mutation should be rejected during signature parsing.
    pub expect_signature_parse_error: bool,
    /// Indicates public key parsing failed (e.g., tampered coordinates).
    pub public_parse_failed: bool,
}

/// Load the upstream negative verification fixtures.
pub fn load_negative_vectors() -> Vec<NegativeVector> {
    const RAW: &str = include_str!("fixtures/sm/sm2_negative_vectors.json");
    let raw_vectors: Vec<Value> = json::from_str(RAW).expect("negative vectors parse");
    raw_vectors
        .into_iter()
        .map(|raw| {
            let obj = raw
                .as_object()
                .expect("negative vector entry must be an object");
            let label = obj
                .get("label")
                .and_then(Value::as_str)
                .expect("negative vector label")
                .to_owned();
            let mutation = obj
                .get("mutation")
                .and_then(Value::as_str)
                .expect("negative vector mutation")
                .to_owned();
            let message_hex = obj
                .get("message_hex")
                .and_then(Value::as_str)
                .expect("negative vector message hex");
            let message = decode(message_hex).expect("hex decode message");
            let verify_message = obj
                .get("verify_message_hex")
                .and_then(Value::as_str)
                .map_or_else(
                    || message.clone(),
                    |hex| decode(hex).expect("hex decode verify message"),
                );

            NegativeVector {
                label,
                mutation,
                message,
                verify_message,
            }
        })
        .collect()
}

/// Apply the requested mutation, returning the mutated inputs for verification.
pub fn apply_mutation(vector: &NegativeVector, private: &Sm2PrivateKey) -> MutationResult {
    let base_signature = private.sign(&vector.message);
    let mut signature_bytes = base_signature.to_bytes().to_vec();
    let mut public = private.public_key();
    let mut verify_message = vector.verify_message.clone();
    let mut expect_signature_parse_error = false;
    let mut public_parse_failed = false;

    let half_len = signature_bytes.len() / 2;
    match vector.mutation.as_str() {
        "r_zero" => {
            signature_bytes[..half_len].fill(0);
            expect_signature_parse_error = true;
        }
        "s_zero" => {
            signature_bytes[half_len..].fill(0);
            expect_signature_parse_error = true;
        }
        "r_equals_order" => {
            signature_bytes[..half_len].copy_from_slice(&ORDER);
            expect_signature_parse_error = true;
        }
        "s_equals_order" => {
            signature_bytes[half_len..].copy_from_slice(&ORDER);
            expect_signature_parse_error = true;
        }
        "r_overflow" => {
            signature_bytes[..half_len].copy_from_slice(&ORDER_PLUS_ONE);
            expect_signature_parse_error = true;
        }
        "s_overflow" => {
            signature_bytes[half_len..].copy_from_slice(&ORDER_PLUS_ONE);
            expect_signature_parse_error = true;
        }
        "signature_zeroed" => {
            signature_bytes.fill(0);
            expect_signature_parse_error = true;
        }
        "distid_mismatch" => {
            let pk_bytes = public.to_sec1_bytes(false);
            public = Sm2PublicKey::from_sec1_bytes(ALT_DISTID, &pk_bytes)
                .expect("alternate distid should yield valid key");
        }
        "pubkey_flip" => {
            let mut pk_bytes = public.to_sec1_bytes(false);
            if pk_bytes.len() > 1 {
                pk_bytes[1] ^= 0x40;
            }
            match Sm2PublicKey::from_sec1_bytes(DEFAULT_DISTID, &pk_bytes) {
                Ok(pk) => public = pk,
                Err(_) => public_parse_failed = true,
            }
        }
        "signature_truncated" => {
            signature_bytes.pop();
            expect_signature_parse_error = true;
        }
        "signature_extended" => {
            signature_bytes.push(0);
            expect_signature_parse_error = true;
        }
        "message_tamper" => {
            // Keep the signature intact but alter the verification message.
            verify_message.clone_from(&vector.verify_message);
        }
        other => panic!("unsupported mutation: {other}"),
    }

    MutationResult {
        signature_bytes,
        verify_message,
        public_key: public,
        expect_signature_parse_error,
        public_parse_failed,
    }
}
