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
use sha2::{Digest, Sha256};

use crate::connect::{
    ConnectCiphertextV1, ConnectFrameV1, ConnectPayloadV1, ConnectRelayEnvelopeV1, Dir, EnvelopeV1,
    FrameKind, PermissionsV1, Role, SignInProofV1, decode_connect_envelope_framed,
    encode_connect_envelope_framed,
};

/// Connect bearer-token class used for domain-separated token hashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// One-time application role token.
    App,
    /// One-time wallet role token.
    Wallet,
    /// Stable session management token.
    Management,
}

impl TokenKind {
    const fn label(self) -> &'static [u8] {
        match self {
            Self::App => b"app",
            Self::Wallet => b"wallet",
            Self::Management => b"management",
        }
    }
}

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

fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    const BLOCK_LEN: usize = 64;
    let mut key_block = [0u8; BLOCK_LEN];
    if key.len() > BLOCK_LEN {
        key_block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; BLOCK_LEN];
    let mut opad = [0x5cu8; BLOCK_LEN];
    for idx in 0..BLOCK_LEN {
        ipad[idx] ^= key_block[idx];
        opad[idx] ^= key_block[idx];
    }

    let mut inner = Sha256::new();
    Digest::update(&mut inner, ipad);
    for part in parts {
        Digest::update(&mut inner, part);
    }
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    Digest::update(&mut outer, opad);
    Digest::update(&mut outer, inner_digest);
    outer.finalize().into()
}

/// Derive the Connect P2P relay MAC key from a session id and relay token.
#[must_use]
pub fn derive_relay_mac_key(sid: &[u8; 32], relay_token: &str) -> [u8; 32] {
    let mut salt_input = Vec::with_capacity(25 + sid.len());
    salt_input.extend_from_slice(b"iroha-connect|relay|salt|");
    salt_input.extend_from_slice(sid);
    let salt = Sha256::digest(&salt_input);
    let hkdf = Hkdf::<Sha256>::new(Some(&salt), relay_token.as_bytes());
    let mut out = [0u8; 32];
    hkdf.expand(b"iroha-connect|relay-mac-key|v1", &mut out)
        .expect("hkdf expansion to 32 bytes must succeed");
    out
}

/// Compute the relay auth hash bound into wallet approval signatures.
#[must_use]
pub fn relay_auth_hash(sid: &[u8; 32], relay_token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    Digest::update(&mut hasher, b"iroha-connect|relay-auth|v1");
    Digest::update(&mut hasher, sid);
    Digest::update(&mut hasher, relay_token.as_bytes());
    hasher.finalize().into()
}

/// Compute a domain-separated authentication hash for a Connect bearer token.
#[must_use]
pub fn token_auth_hash(kind: TokenKind, sid: &[u8; 32], token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    Digest::update(&mut hasher, b"iroha-connect|token-auth|v1");
    Digest::update(&mut hasher, kind.label());
    Digest::update(&mut hasher, sid);
    Digest::update(&mut hasher, token.as_bytes());
    hasher.finalize().into()
}

/// Compute the relay MAC for a Connect frame and remaining relay TTL.
///
/// # Errors
/// Returns [`norito::core::Error`] when the frame cannot be encoded using the
/// canonical Connect bare layout.
pub fn compute_relay_mac(
    relay_key: &[u8; 32],
    frame: &ConnectFrameV1,
    ttl: u8,
) -> Result<[u8; 32], norito::core::Error> {
    let frame_bytes = crate::connect::encode_connect_frame_bare(frame)?;
    let ttl_bytes = [ttl];
    Ok(hmac_sha256(
        relay_key,
        &[
            b"iroha-connect|relay-frame|v1",
            &frame.sid,
            &[frame.dir as u8],
            &frame.seq.to_le_bytes(),
            &ttl_bytes,
            &frame_bytes,
        ],
    ))
}

/// Build an authenticated relay envelope for P2P forwarding.
///
/// # Errors
/// Returns [`norito::core::Error`] if the frame cannot be encoded.
pub fn seal_relay_envelope(
    relay_key: &[u8; 32],
    frame: ConnectFrameV1,
    ttl: u8,
) -> Result<ConnectRelayEnvelopeV1, norito::core::Error> {
    let mac = compute_relay_mac(relay_key, &frame, ttl)?;
    Ok(ConnectRelayEnvelopeV1 { frame, ttl, mac })
}

/// Verify an authenticated relay envelope.
///
/// # Errors
/// Returns [`norito::core::Error`] if the inner frame cannot be encoded.
pub fn verify_relay_envelope(
    relay_key: &[u8; 32],
    envelope: &ConnectRelayEnvelopeV1,
) -> Result<bool, norito::core::Error> {
    let expected = compute_relay_mac(relay_key, &envelope.frame, envelope.ttl)?;
    Ok(constant_time_eq(&expected, &envelope.mac))
}

/// Compare two byte slices without data-dependent early exit.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in a.iter().zip(b) {
        diff |= left ^ right;
    }
    diff == 0
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
    let pt = encode_connect_envelope_framed(&env).expect("encode envelope");
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
    let env = decode_connect_envelope_framed(&pt).map_err(|_| "decode")?;
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
/// Layout: length-delimited tagged fields under
/// `iroha-connect|approve|v1`, without relay binding.
pub fn build_approve_preimage(
    sid: &[u8; 32],
    app_pk: &[u8; 32],
    wallet_pk: &[u8; 32],
    account_id: &str,
    perms: Option<&PermissionsV1>,
    proof: Option<&SignInProofV1>,
) -> Vec<u8> {
    build_approve_preimage_with_relay(sid, app_pk, wallet_pk, account_id, perms, proof, None)
}

/// Build the canonical approval preimage and bind optional relay auth material.
#[must_use]
pub fn build_approve_preimage_with_relay(
    sid: &[u8; 32],
    app_pk: &[u8; 32],
    wallet_pk: &[u8; 32],
    account_id: &str,
    perms: Option<&PermissionsV1>,
    proof: Option<&SignInProofV1>,
    relay_auth: Option<&[u8; 32]>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 32 + 32 + 32 + account_id.len() + 64);
    push_tagged(&mut out, b"domain", b"iroha-connect|approve|v1");
    push_tagged(&mut out, b"sid", sid);
    push_tagged(&mut out, b"app_pk", app_pk);
    push_tagged(&mut out, b"wallet_pk", wallet_pk);
    push_tagged(&mut out, b"account_id", account_id.as_bytes());
    if let Some(p) = perms {
        push_tagged(&mut out, b"permissions", &hash_permissions_current(p));
    }
    if let Some(pf) = proof {
        push_tagged(&mut out, b"proof", &hash_signin_proof_current(pf));
    }
    if let Some(relay) = relay_auth {
        push_tagged(&mut out, b"relay_auth", relay);
    }
    out
}

fn push_tagged(out: &mut Vec<u8>, tag: &[u8], value: &[u8]) {
    let tag_len = u16::try_from(tag.len()).expect("connect SDK tags are bounded");
    out.extend_from_slice(&tag_len.to_le_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value);
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

    #[test]
    fn sealed_envelope_frame_encodes() {
        let key = [0x11_u8; 32];
        let sid = [0x22_u8; 32];
        let frame = seal_envelope_current(
            &key,
            &sid,
            Dir::AppToWallet,
            7,
            ConnectPayloadV1::Control(crate::connect::ControlAfterKeyV1::Close {
                who: Role::App,
                code: 1,
                reason: "bye".to_owned(),
                retryable: false,
            }),
        );
        let bytes =
            crate::connect::encode_connect_frame_bare(&frame).expect("sealed frame must encode");
        let decoded =
            crate::connect::decode_connect_frame_bare(&bytes).expect("sealed frame must decode");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn relay_mac_authenticates_frame_and_ttl() {
        let sid = [0x22_u8; 32];
        let frame = ConnectFrameV1 {
            sid,
            dir: Dir::AppToWallet,
            seq: 1,
            kind: FrameKind::Control(crate::connect::ConnectControlV1::Ping { nonce: 9 }),
        };
        let key = derive_relay_mac_key(&sid, "relay-token");
        let envelope = seal_relay_envelope(&key, frame.clone(), 3).expect("relay envelope");
        assert!(verify_relay_envelope(&key, &envelope).expect("verify relay envelope"));

        let mut bad = envelope.clone();
        bad.ttl = 2;
        assert!(!verify_relay_envelope(&key, &bad).expect("verify relay envelope"));

        let wrong_key = derive_relay_mac_key(&sid, "wrong-token");
        assert!(!verify_relay_envelope(&wrong_key, &envelope).expect("verify relay envelope"));
    }

    #[test]
    fn token_auth_hash_is_domain_separated() {
        let sid = [0x33_u8; 32];
        let app = token_auth_hash(TokenKind::App, &sid, "token");
        let wallet = token_auth_hash(TokenKind::Wallet, &sid, "token");
        let management = token_auth_hash(TokenKind::Management, &sid, "token");

        assert_ne!(app, wallet);
        assert_ne!(app, management);
        assert!(constant_time_eq(
            &app,
            &token_auth_hash(TokenKind::App, &sid, "token")
        ));
        assert!(!constant_time_eq(
            &app,
            &token_auth_hash(TokenKind::App, &sid, "other-token")
        ));
    }

    #[test]
    fn connect_session_vectors_match_fixture() {
        let fixture: norito::json::Value = norito::json::from_str(include_str!(
            "../../../fixtures/connect/session_vectors.json"
        ))
        .expect("connect session vectors parse");
        let sid_hex = fixture
            .get("sid_hex")
            .and_then(norito::json::Value::as_str)
            .expect("sid_hex");
        let sid_vec = hex::decode(sid_hex).expect("sid hex");
        let sid: [u8; 32] = sid_vec.try_into().expect("sid length");
        let tokens = fixture
            .get("tokens")
            .and_then(norito::json::Value::as_object)
            .expect("tokens");
        let hashes = fixture
            .get("token_hashes")
            .and_then(norito::json::Value::as_object)
            .expect("token_hashes");
        let token = |name: &str| {
            tokens
                .get(name)
                .and_then(norito::json::Value::as_str)
                .expect("token")
        };
        let hash = |name: &str| {
            hashes
                .get(name)
                .and_then(norito::json::Value::as_str)
                .expect("hash")
        };

        assert_eq!(
            hex::encode(token_auth_hash(TokenKind::App, &sid, token("app"))),
            hash("app")
        );
        assert_eq!(
            hex::encode(token_auth_hash(TokenKind::Wallet, &sid, token("wallet"))),
            hash("wallet")
        );
        assert_eq!(
            hex::encode(token_auth_hash(
                TokenKind::Management,
                &sid,
                token("management")
            )),
            hash("management")
        );
        assert_eq!(
            hex::encode(derive_relay_mac_key(&sid, token("relay"))),
            fixture
                .get("relay_mac_key_hex")
                .and_then(norito::json::Value::as_str)
                .expect("relay_mac_key_hex")
        );
        assert_eq!(
            hex::encode(relay_auth_hash(&sid, token("relay"))),
            fixture
                .get("relay_auth_hash_hex")
                .and_then(norito::json::Value::as_str)
                .expect("relay_auth_hash_hex")
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
        let domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain parses");
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
        let relay = relay_auth_hash(&sid, "relay-token");
        let img = build_approve_preimage_with_relay(
            &sid,
            &app,
            &wal,
            &acc,
            Some(&perms),
            Some(&proof),
            Some(&relay),
        );
        assert!(img.windows(24).any(|w| w == b"iroha-connect|approve|v1"));
        assert!(img.windows(32).any(|w| w == sid));
        assert!(img.windows(32).any(|w| w == app));
        assert!(img.windows(32).any(|w| w == wal));
        assert!(img.windows(32).any(|w| w == relay));
        assert!(std::str::from_utf8(&img).is_err(), "binary tail included");
    }
}
