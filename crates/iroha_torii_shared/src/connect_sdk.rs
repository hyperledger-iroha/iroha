//! Iroha Connect SDK helpers: key derivation, AAD, sealing/opening frames.
use hkdf::Hkdf;
use iroha_crypto::{
    PublicKey, SessionKey,
    blake2::{
        Blake2bVar,
        digest::{Update, VariableOutput},
    },
    encryption::{ChaCha20Poly1305, SymmetricEncryptor},
    kex::{KeyExchangeScheme as _, X25519Sha256},
};
use iroha_data_model::NetworkId;
use norito::codec::Encode;
use sha2::{Digest, Sha256};
use crate::connect::{
    ConnectCiphertextV1, ConnectFrameV1, ConnectPayloadV1, ConnectRelayEnvelopeV1, Constraints,
    Dir, EnvelopeV1, FrameKind, PermissionsV1, Role, SignInProofV1, WalletSignatureV1,
    decode_connect_envelope_framed, encode_connect_envelope_framed,
};
/// Derive the canonical Connect session identifier for one exact deployment.
#[must_use]
pub fn derive_session_id(network_id: &NetworkId, app_pk: &[u8; 32], nonce: &[u8; 16]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut b2 = Blake2bVar::new(32).expect("32-byte BLAKE2b output is valid");
    b2.update(b"iroha-connect|sid|");
    b2.update(network_id.as_bytes());
    b2.update(app_pk);
    b2.update(nonce);
    b2.finalize_variable(&mut out)
        .expect("fixed BLAKE2b output buffer has the requested length");
    out
}
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
/// Returns an error when the frame is not ciphertext, the redundant directions
/// disagree, decryption or decoding fails, or the decrypted envelope sequence
/// does not match the frame sequence.
pub fn open_envelope_current(
    key: &[u8; 32],
    frame: &ConnectFrameV1,
) -> Result<EnvelopeV1, &'static str> {
    let FrameKind::Ciphertext(ct) = &frame.kind else {
        return Err("not ciphertext");
    };
    if ct.dir != frame.dir {
        return Err("dir_mismatch");
    }
    let aad = aad_current(&frame.sid, frame.dir, frame.seq);
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
/// Returns an error when the frame is not ciphertext, the redundant directions
/// disagree, decryption or decoding fails, or the decrypted envelope sequence
/// does not match the frame sequence.
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
/// Deterministic Norito-encoded BLAKE2b-256 hash of session constraints.
#[must_use]
pub fn hash_constraints_current(constraints: &Constraints) -> [u8; 32] {
    let buf = constraints.encode();
    let mut out = [0u8; 32];
    let mut b2 = Blake2bVar::new(32).expect("32-byte BLAKE2b output is valid");
    b2.update(&buf);
    b2.finalize_variable(&mut out)
        .expect("fixed BLAKE2b output buffer has the requested length");
    out
}
/// Build the canonical approval preimage for wallet signature.
///
/// The preimage binds the exact deployment, all application constraints, and
/// the relay authorization established when the one-shot session was created.
#[must_use]
pub fn build_approve_preimage(
    constraints: &Constraints,
    sid: &[u8; 32],
    app_pk: &[u8; 32],
    wallet_pk: &[u8; 32],
    account_id: &str,
    perms: Option<&PermissionsV1>,
    proof: Option<&SignInProofV1>,
    relay_auth: &[u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 32 * 7 + account_id.len());
    push_tagged(&mut out, b"domain", b"iroha-connect|approve|v1");
    push_tagged(&mut out, b"network_id", constraints.network_id.as_bytes());
    push_tagged(
        &mut out,
        b"constraints",
        &hash_constraints_current(constraints),
    );
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
    push_tagged(&mut out, b"relay_auth", relay_auth);
    out
}
/// Verify a wallet's approval signature against the canonical session preimage.
///
/// # Errors
///
/// Returns a stable error when the redundant signature algorithm disagrees
/// with the account key or when cryptographic verification fails.
pub fn verify_wallet_approval_signature(
    account_signatory: &PublicKey,
    constraints: &Constraints,
    sid: &[u8; 32],
    app_pk: &[u8; 32],
    wallet_pk: &[u8; 32],
    account_id: &str,
    perms: Option<&PermissionsV1>,
    proof: Option<&SignInProofV1>,
    relay_auth: &[u8; 32],
    signature: &WalletSignatureV1,
) -> Result<(), &'static str> {
    if account_signatory
        .try_algorithm()
        .map_err(|_| "connect_wallet_account_key_invalid")?
        != signature.algorithm
    {
        return Err("connect_wallet_signature_algorithm_mismatch");
    }
    let preimage = build_approve_preimage(
        constraints,
        sid,
        app_pk,
        wallet_pk,
        account_id,
        perms,
        proof,
        relay_auth,
    );
    signature
        .signature
        .verify(account_signatory, &preimage)
        .map_err(|_| "connect_wallet_signature_invalid")
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
        // Both redundant direction fields must agree, and the outer direction
        // is part of the authenticated header.
        let mut tampered_outer_dir = frame.clone();
        tampered_outer_dir.dir = Dir::WalletToApp;
        assert_eq!(
            open_envelope_current(&key, &tampered_outer_dir).err(),
            Some("dir_mismatch")
        );
        let mut tampered_ciphertext_dir = frame;
        let FrameKind::Ciphertext(ciphertext) = &mut tampered_ciphertext_dir.kind else {
            unreachable!("sealed envelope is ciphertext");
        };
        ciphertext.dir = Dir::WalletToApp;
        assert_eq!(
            open_envelope_current(&key, &tampered_ciphertext_dir).err(),
            Some("dir_mismatch")
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
        let network_id: NetworkId = fixture
            .get("network_id")
            .and_then(norito::json::Value::as_str)
            .expect("network_id")
            .parse()
            .expect("canonical fixture NetworkId");
        assert_eq!(
            hex::encode(network_id.as_bytes()),
            fixture
                .get("network_id_hex")
                .and_then(norito::json::Value::as_str)
                .expect("network_id_hex")
        );
        let app_pk: [u8; 32] = hex::decode(
            fixture
                .get("app_pk_hex")
                .and_then(norito::json::Value::as_str)
                .expect("app_pk_hex"),
        )
        .expect("app key hex")
        .try_into()
        .expect("app key length");
        let nonce: [u8; 16] = hex::decode(
            fixture
                .get("nonce_hex")
                .and_then(norito::json::Value::as_str)
                .expect("nonce_hex"),
        )
        .expect("nonce hex")
        .try_into()
        .expect("nonce length");
        let sid_hex = fixture
            .get("sid_hex")
            .and_then(norito::json::Value::as_str)
            .expect("sid_hex");
        let sid_vec = hex::decode(sid_hex).expect("sid hex");
        let sid: [u8; 32] = sid_vec.try_into().expect("sid length");
        assert_eq!(derive_session_id(&network_id, &app_pk, &nonce), sid);
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
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{account::AccountId, block::BlockHeader};
    fn network_id(label: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            label,
        )))
    }
    #[test]
    fn canonical_approval_fixture_verifies_end_to_end() {
        let fixture: norito::json::Value = norito::json::from_str(include_str!(
            "../../../fixtures/connect/session_vectors.json"
        ))
        .expect("connect session vectors parse");
        let approval = fixture
            .get("approval")
            .and_then(norito::json::Value::as_object)
            .expect("approval fixture");
        let decode = |object: &norito::json::Map, field: &str| {
            hex::decode(
                object
                    .get(field)
                    .and_then(norito::json::Value::as_str)
                    .unwrap_or_else(|| panic!("missing {field}")),
            )
            .unwrap_or_else(|_| panic!("invalid {field}"))
        };
        let network_id: NetworkId = fixture
            .get("network_id")
            .and_then(norito::json::Value::as_str)
            .expect("network_id")
            .parse()
            .expect("canonical fixture NetworkId");
        let constraints = Constraints { network_id };
        assert_eq!(
            hex::encode(hash_constraints_current(&constraints)),
            approval
                .get("constraints_hash_hex")
                .and_then(norito::json::Value::as_str)
                .expect("constraints_hash_hex")
        );
        let sid: [u8; 32] = decode(fixture.as_object().expect("fixture object"), "sid_hex")
            .try_into()
            .expect("sid length");
        let app_pk: [u8; 32] = decode(fixture.as_object().expect("fixture object"), "app_pk_hex")
            .try_into()
            .expect("app key length");
        let wallet_pk: [u8; 32] = decode(approval, "wallet_pk_hex")
            .try_into()
            .expect("wallet key length");
        let relay_auth: [u8; 32] = decode(
            fixture.as_object().expect("fixture object"),
            "relay_auth_hash_hex",
        )
        .try_into()
        .expect("relay auth length");
        let account_id = approval
            .get("account_id")
            .and_then(norito::json::Value::as_str)
            .expect("account_id");
        let preimage = build_approve_preimage(
            &constraints,
            &sid,
            &app_pk,
            &wallet_pk,
            account_id,
            None,
            None,
            &relay_auth,
        );
        assert_eq!(
            hex::encode(&preimage),
            approval
                .get("approve_preimage_hex")
                .and_then(norito::json::Value::as_str)
                .expect("approve_preimage_hex")
        );
        let key_pair = KeyPair::try_from_seed(
            decode(approval, "account_private_key_seed_hex"),
            Algorithm::Ed25519,
        )
        .expect("fixture approval keypair");
        assert_eq!(
            hex::encode(key_pair.public_key().payload()),
            approval
                .get("account_public_key_hex")
                .and_then(norito::json::Value::as_str)
                .expect("account_public_key_hex")
        );
        assert_eq!(
            AccountId::new(key_pair.public_key().clone()).to_string(),
            account_id
        );
        let signature = Signature::try_new(key_pair.private_key(), &preimage)
            .expect("fixture approval signature");
        assert_eq!(
            hex::encode(signature.payload()),
            approval
                .get("signature_hex")
                .and_then(norito::json::Value::as_str)
                .expect("signature_hex")
        );
        verify_wallet_approval_signature(
            key_pair.public_key(),
            &constraints,
            &sid,
            &app_pk,
            &wallet_pk,
            account_id,
            None,
            None,
            &relay_auth,
            &WalletSignatureV1::new(Algorithm::Ed25519, signature),
        )
        .expect("canonical fixture approval verifies");
    }
    #[test]
    fn approval_signature_binds_exact_network_constraints_and_relay() {
        let sid = [1u8; 32];
        let app = [2u8; 32];
        let wal = [3u8; 32];
        let key_pair = KeyPair::try_from_seed(vec![0x44; 32], Algorithm::Ed25519)
            .expect("approval fixture keypair");
        let acc = AccountId::new(key_pair.public_key().clone()).to_string();
        let constraints = Constraints {
            network_id: network_id(b"connect-approval-genesis-a"),
        };
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
        let img = build_approve_preimage(
            &constraints,
            &sid,
            &app,
            &wal,
            &acc,
            Some(&perms),
            Some(&proof),
            &relay,
        );
        let signature = WalletSignatureV1::new(
            Algorithm::Ed25519,
            Signature::try_new(key_pair.private_key(), &img).expect("approval fixture signs"),
        );
        verify_wallet_approval_signature(
            key_pair.public_key(),
            &constraints,
            &sid,
            &app,
            &wal,
            &acc,
            Some(&perms),
            Some(&proof),
            &relay,
            &signature,
        )
        .expect("exact approval verifies");
        let wrong_constraints = Constraints {
            network_id: network_id(b"connect-approval-genesis-b"),
        };
        assert_eq!(
            verify_wallet_approval_signature(
                key_pair.public_key(),
                &wrong_constraints,
                &sid,
                &app,
                &wal,
                &acc,
                Some(&perms),
                Some(&proof),
                &relay,
                &signature,
            ),
            Err("connect_wallet_signature_invalid")
        );
        let wrong_relay = relay_auth_hash(&sid, "other-relay-token");
        assert_eq!(
            verify_wallet_approval_signature(
                key_pair.public_key(),
                &constraints,
                &sid,
                &app,
                &wal,
                &acc,
                Some(&perms),
                Some(&proof),
                &wrong_relay,
                &signature,
            ),
            Err("connect_wallet_signature_invalid")
        );
        assert!(img.windows(24).any(|w| w == b"iroha-connect|approve|v1"));
        assert!(
            img.windows(32)
                .any(|w| w == constraints.network_id.as_bytes())
        );
        assert!(img.windows(32).any(|w| w == sid));
        assert!(img.windows(32).any(|w| w == app));
        assert!(img.windows(32).any(|w| w == wal));
        assert!(img.windows(32).any(|w| w == relay));
        assert!(std::str::from_utf8(&img).is_err(), "binary tail included");
    }
    #[test]
    fn session_id_rejects_same_label_different_genesis() {
        let app = [7u8; 32];
        let nonce = [8u8; 16];
        assert_ne!(
            derive_session_id(&network_id(b"same-label-genesis-a"), &app, &nonce),
            derive_session_id(&network_id(b"same-label-genesis-b"), &app, &nonce)
        );
    }
}
