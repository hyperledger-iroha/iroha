//! Iroha Connect SDK helpers: key derivation, AAD, sealing/opening frames.
use hkdf::Hkdf;
use iroha_crypto::{
    SessionKey,
    blake2::{
        Blake2bVar,
        digest::{Update, VariableOutput},
    },
    encryption::{ChaCha20Poly1305, SymmetricEncryptor},
    kex::{KeyExchangeScheme as _, X25519Sha256},
};
use norito::codec::Encode;
use sha2::Sha256;

use crate::connect::{
    ConnectCiphertextV1, ConnectFrameV1, ConnectPayloadV1, Dir, EnvelopeV1, FrameKind,
    PermissionsV1, Role, SignInProofV1,
};

/// Derive per-direction keys from a `SessionKey` using HKDF-SHA256 with a BLAKE2b(sid) salt.
pub fn derive_direction_keys(session_key: &SessionKey, sid: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let mut salt = [0u8; 32];
    let mut b2 = Blake2bVar::new(32).expect("ok");
    b2.update(b"iroha-connect|salt|");
    b2.update(sid);
    b2.finalize_variable(&mut salt).expect("ok");
    let h = Hkdf::<Sha256>::new(Some(&salt), session_key.payload());
    let mut k_app = [0u8; 32];
    let mut k_wallet = [0u8; 32];
    h.expand(b"iroha-connect|k_app", &mut k_app).expect("ok");
    h.expand(b"iroha-connect|k_wallet", &mut k_wallet)
        .expect("ok");
    (k_app, k_wallet)
}

/// Compute X25519 shared secret and derive direction keys.
///
/// # Errors
///
/// Returns an error when the peer public key is invalid or the shared secret
/// fails contributory checks.
pub fn x25519_derive_keys(
    local_sk: &[u8; 32],
    peer_pk: &[u8; 32],
    sid: &[u8; 32],
) -> Result<([u8; 32], [u8; 32]), iroha_crypto::Error> {
    let x = X25519Sha256::new();
    let peer = X25519Sha256::decode_public_key(peer_pk)?;
    let (_pk, sk) = x.keypair(iroha_crypto::KeyGenOption::FromPrivateKey(
        (*local_sk).into(),
    ));
    let sess = x.compute_shared_secret(&sk, &peer)?;
    Ok(derive_direction_keys(&sess, sid))
}

/// Build v1 AAD: "connect:v1" || sid || dir || seq || kind=1
pub fn aad_current(sid: &[u8; 32], dir: Dir, seq: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(8 + 32 + 1 + 8 + 1);
    aad.extend_from_slice(b"connect:v1");
    aad.extend_from_slice(sid);
    aad.push(match dir {
        Dir::AppToWallet => 0u8,
        Dir::WalletToApp => 1u8,
    });
    aad.extend_from_slice(&seq.to_le_bytes());
    aad.push(1u8); // kind = Ciphertext
    aad
}

/// Build canonical AAD for the current Connect envelope format.
pub fn aad(sid: &[u8; 32], dir: Dir, seq: u64) -> Vec<u8> {
    aad_current(sid, dir, seq)
}

/// Derive a 96-bit ChaCha20-Poly1305 nonce from sequence: 0x00000000 || `seq_le`.
pub fn nonce_from_seq(seq: u64) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[4..].copy_from_slice(&seq.to_le_bytes());
    n
}

/// Seal an envelope (payload+seq) into a ciphertext frame using the provided key bytes.
pub fn seal_envelope_current(
    key: &[u8; 32],
    sid: &[u8; 32],
    dir: Dir,
    seq: u64,
    payload: ConnectPayloadV1,
) -> ConnectFrameV1 {
    let env = EnvelopeV1 { seq, payload };
    let (payload, flags) = norito::codec::encode_with_header_flags(&env);
    let pt = norito::core::frame_bare_with_header_flags::<EnvelopeV1>(&payload, flags)
        .expect("encode envelope");
    let aad = aad_current(sid, dir, seq);
    let nonce = nonce_from_seq(seq);
    let encryptor =
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(key).expect("valid key length");
    let ct_bytes = encryptor
        .encrypt(&nonce[..], aad.as_slice(), pt.as_slice())
        .expect("encrypt");
    ConnectFrameV1 {
        sid: *sid,
        dir,
        seq,
        kind: FrameKind::Ciphertext(ConnectCiphertextV1 {
            dir,
            aead: ct_bytes,
        }),
    }
}

/// Seal an envelope using the canonical Connect envelope format.
pub fn seal_envelope(
    key: &[u8; 32],
    sid: &[u8; 32],
    dir: Dir,
    seq: u64,
    payload: ConnectPayloadV1,
) -> ConnectFrameV1 {
    seal_envelope_current(key, sid, dir, seq, payload)
}

/// Open a ciphertext frame and return the decrypted envelope. Enforces Envelope.seq == frame.seq.
///
/// # Errors
///
/// Returns an error when the frame is not ciphertext, decryption fails,
/// decoding fails, or when the decrypted envelope sequence does not match
/// the frame sequence.
pub fn open_envelope_current(
    key: &[u8; 32],
    frame: &ConnectFrameV1,
) -> Result<EnvelopeV1, &'static str> {
    let FrameKind::Ciphertext(ct) = &frame.kind else {
        return Err("not ciphertext");
    };
    let aad = aad_current(&frame.sid, ct.dir, frame.seq);
    let nonce = nonce_from_seq(frame.seq);
    let encryptor =
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(key).map_err(|_| "decrypt")?;
    let pt = encryptor
        .decrypt(&nonce[..], aad.as_slice(), ct.aead.as_slice())
        .map_err(|_| "decrypt")?;
    let view = norito::core::from_bytes_view(&pt).map_err(|_| "decode")?;
    let env = view.decode::<EnvelopeV1>().map_err(|_| "decode")?;
    if env.seq != frame.seq {
        return Err("seq_mismatch");
    }
    Ok(env)
}

/// Open a ciphertext frame using the canonical Connect envelope format.
///
/// # Errors
///
/// Returns an error when the frame is not ciphertext, decryption fails,
/// decoding fails, or when the decrypted envelope sequence does not match
/// the frame sequence.
pub fn open_envelope(key: &[u8; 32], frame: &ConnectFrameV1) -> Result<EnvelopeV1, &'static str> {
    open_envelope_current(key, frame)
}

/// Deterministic Norito-encoded BLAKE2b-256 hash of permissions.
pub fn hash_permissions_current(perms: &PermissionsV1) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2bVar, digest::Update};
    let buf = perms.encode();
    let mut out = [0u8; 32];
    let mut b2 = Blake2bVar::new(32).expect("ok");
    b2.update(&buf);
    b2.finalize_variable(&mut out).expect("ok");
    out
}

/// Deterministic Norito-encoded BLAKE2b-256 hash of sign-in proof.
pub fn hash_signin_proof_current(proof: &SignInProofV1) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2bVar, digest::Update};
    let buf = proof.encode();
    let mut out = [0u8; 32];
    let mut b2 = Blake2bVar::new(32).expect("ok");
    b2.update(&buf);
    b2.finalize_variable(&mut out).expect("ok");
    out
}

/// Build the canonical approval preimage for wallet signature.
///
/// Layout: "iroha-connect|approve|" || sid || `app_pk` || `wallet_pk` || `account_id` || hash(permissions?) || hash(proof?)
pub fn build_approve_preimage(
    sid: &[u8; 32],
    app_pk: &[u8; 32],
    wallet_pk: &[u8; 32],
    account_id: &str,
    perms: Option<&PermissionsV1>,
    proof: Option<&SignInProofV1>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 32 + 32 + 32 + account_id.len() + 64);
    out.extend_from_slice(b"iroha-connect|approve|");
    out.extend_from_slice(sid);
    out.extend_from_slice(app_pk);
    out.extend_from_slice(wallet_pk);
    out.extend_from_slice(account_id.as_bytes());
    if let Some(p) = perms {
        out.extend_from_slice(&hash_permissions_current(p));
    }
    if let Some(pf) = proof {
        out.extend_from_slice(&hash_signin_proof_current(pf));
    }
    out
}

/// Convenience: encrypt a Close control as an encrypted payload.
#[allow(clippy::too_many_arguments)]
pub fn encrypt_close_current(
    key: &[u8; 32],
    sid: &[u8; 32],
    dir: Dir,
    seq: u64,
    who: Role,
    code: u16,
    reason: String,
    retryable: bool,
) -> ConnectFrameV1 {
    let payload = ConnectPayloadV1::Control(crate::connect::ControlAfterKeyV1::Close {
        who,
        code,
        reason,
        retryable,
    });
    seal_envelope_current(key, sid, dir, seq, payload)
}

/// Convenience: encrypt a Reject control as an encrypted payload.
pub fn encrypt_reject_current(
    key: &[u8; 32],
    sid: &[u8; 32],
    dir: Dir,
    seq: u64,
    code: u16,
    code_id: String,
    reason: String,
) -> ConnectFrameV1 {
    let payload = ConnectPayloadV1::Control(crate::connect::ControlAfterKeyV1::Reject {
        code,
        code_id,
        reason,
    });
    seal_envelope_current(key, sid, dir, seq, payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aead_bind_header_and_seq() {
        let key = [7u8; 32];
        let sid = [9u8; 32];
        let dir = Dir::AppToWallet;
        let seq = 1u64;
        let frame = seal_envelope_current(
            &key,
            &sid,
            dir,
            seq,
            ConnectPayloadV1::SignRequestRaw {
                domain_tag: "iroha-connect/v1/test".into(),
                bytes: vec![1, 2, 3],
            },
        );
        let env = open_envelope_current(&key, &frame).expect("open");
        assert_eq!(env.seq, seq);
        // Tamper header: change seq → expect failure
        let tampered = ConnectFrameV1 {
            seq: 2,
            ..frame.clone()
        };
        assert_eq!(
            open_envelope_current(&key, &tampered).err(),
            Some("decrypt")
        );
    }
}

#[cfg(test)]
mod approve_preimage_tests {
    use super::*;
    use iroha_crypto::PublicKey;
    use iroha_data_model::{account::AccountId, domain::DomainId};
    #[test]
    fn preimage_contains_prefix_and_keys() {
        let sid = [1u8; 32];
        let app = [2u8; 32];
        let wal = [3u8; 32];
        let domain: DomainId = "wonderland".parse().expect("domain parses");
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key parses");
        let _ = domain;
        let acc = AccountId::new(public_key).to_string();
        let perms = PermissionsV1 {
            methods: vec!["SIGN_REQUEST_RAW".into()],
            events: vec![],
            resources: None,
        };
        let proof = SignInProofV1 {
            domain: "example.org".into(),
            uri: "https://example.org".into(),
            statement: "Sign in".into(),
            issued_at: "2025-01-01T00:00:00Z".into(),
            nonce: "abc".into(),
        };
        let img = build_approve_preimage(&sid, &app, &wal, &acc, Some(&perms), Some(&proof));
        assert!(img.starts_with(b"iroha-connect|approve|"));
        assert!(img.windows(32).any(|w| w == sid));
        assert!(img.windows(32).any(|w| w == app));
        assert!(img.windows(32).any(|w| w == wal));
        assert!(std::str::from_utf8(&img).is_err(), "binary tail included");
    }
}
