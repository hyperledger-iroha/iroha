//! Iroha Connect protocol wire types (scaffold).
#![allow(clippy::size_of_ref)]
//!
//! Minimal, versioned Norito types for a WalletConnect‑style pairing and
//! message exchange between a dApp and a wallet over Torii WebSockets and the
//! Iroha P2P network. This module defines data only; transport/handlers live
//! in server/client crates.

use iroha_crypto::{Algorithm, Signature};
use norito::{
    codec::{Decode, Encode},
    core::{DecodeFlagsGuard, Error, Header},
};

/// Wallet signature used across Connect control and payload messages.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
pub struct WalletSignatureV1 {
    /// Signature scheme used to produce the signature bytes.
    pub algorithm: Algorithm,
    /// Raw signature payload.
    pub signature: Signature,
}

impl WalletSignatureV1 {
    /// Construct a new wallet signature from the given parts.
    #[must_use]
    pub fn new(algorithm: Algorithm, signature: Signature) -> Self {
        Self {
            algorithm,
            signature,
        }
    }

    /// Borrow the raw signature bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.signature.payload()
    }

    /// Decompose the signature into its parts.
    #[must_use]
    pub fn into_parts(self) -> (Algorithm, Signature) {
        (self.algorithm, self.signature)
    }

    /// Construct an Ed25519 signature from raw bytes.
    #[must_use]
    pub fn from_ed25519_bytes(bytes: &[u8]) -> Option<Self> {
        (bytes.len() == 64).then(|| Self::new(Algorithm::Ed25519, Signature::from_bytes(bytes)))
    }
}

fn ensure_connect_layout(flags: u8) -> Result<(), Error> {
    if flags == CONNECT_LAYOUT_FLAGS {
        Ok(())
    } else {
        Err(Error::UnsupportedFeature("connect layout flags"))
    }
}

fn encode_field<T: Encode>(value: &T) -> Result<Vec<u8>, Error> {
    let (payload, flags) = norito::codec::encode_with_header_flags(value);
    ensure_connect_layout(flags)?;
    Ok(payload)
}

#[allow(clippy::too_many_lines)]
fn encode_connect_control_payload(control: &ConnectControlV1) -> Result<Vec<u8>, Error> {
    let (tag, body) = match control {
        ConnectControlV1::Open {
            app_pk,
            app_meta,
            constraints,
            permissions,
        } => {
            let mut body = Vec::new();
            let app_pk = encode_field(app_pk)?;
            body.extend(&(app_pk.len() as u64).to_le_bytes());
            body.extend(app_pk);
            let app_meta = encode_field(app_meta)?;
            body.extend(&(app_meta.len() as u64).to_le_bytes());
            body.extend(app_meta);
            let constraints = encode_field(constraints)?;
            body.extend(&(constraints.len() as u64).to_le_bytes());
            body.extend(constraints);
            let permissions = encode_field(permissions)?;
            body.extend(&(permissions.len() as u64).to_le_bytes());
            body.extend(permissions);
            (0u32, body)
        }
        ConnectControlV1::Approve {
            wallet_pk,
            account_id,
            permissions,
            proof,
            sig_wallet,
        } => {
            let mut body = Vec::new();
            let wallet_pk = encode_field(wallet_pk)?;
            body.extend(&(wallet_pk.len() as u64).to_le_bytes());
            body.extend(wallet_pk);
            let account_id = encode_field(account_id)?;
            body.extend(&(account_id.len() as u64).to_le_bytes());
            body.extend(account_id);
            let permissions = encode_field(permissions)?;
            body.extend(&(permissions.len() as u64).to_le_bytes());
            body.extend(permissions);
            let proof = encode_field(proof)?;
            body.extend(&(proof.len() as u64).to_le_bytes());
            body.extend(proof);
            let sig_wallet = encode_field(sig_wallet)?;
            body.extend(&(sig_wallet.len() as u64).to_le_bytes());
            body.extend(sig_wallet);
            (1u32, body)
        }
        ConnectControlV1::Reject {
            code,
            code_id,
            reason,
        } => {
            let mut body = Vec::new();
            let code = encode_field(code)?;
            body.extend(&(code.len() as u64).to_le_bytes());
            body.extend(code);
            let code_id = encode_field(code_id)?;
            body.extend(&(code_id.len() as u64).to_le_bytes());
            body.extend(code_id);
            let reason = encode_field(reason)?;
            body.extend(&(reason.len() as u64).to_le_bytes());
            body.extend(reason);
            (2u32, body)
        }
        ConnectControlV1::Close {
            who,
            code,
            reason,
            retryable,
        } => {
            let mut body = Vec::new();
            let who = encode_field(who)?;
            body.extend(&(who.len() as u64).to_le_bytes());
            body.extend(who);
            let code = encode_field(code)?;
            body.extend(&(code.len() as u64).to_le_bytes());
            body.extend(code);
            let reason = encode_field(reason)?;
            body.extend(&(reason.len() as u64).to_le_bytes());
            body.extend(reason);
            let retryable = encode_field(retryable)?;
            body.extend(&(retryable.len() as u64).to_le_bytes());
            body.extend(retryable);
            (3u32, body)
        }
        ConnectControlV1::Ping { nonce } => {
            let mut body = Vec::new();
            let nonce = encode_field(nonce)?;
            body.extend(&(nonce.len() as u64).to_le_bytes());
            body.extend(nonce);
            (4u32, body)
        }
        ConnectControlV1::Pong { nonce } => {
            let mut body = Vec::new();
            let nonce = encode_field(nonce)?;
            body.extend(&(nonce.len() as u64).to_le_bytes());
            body.extend(nonce);
            (5u32, body)
        }
        ConnectControlV1::ServerEvent { event } => {
            let mut body = Vec::new();
            let event = encode_field(event)?;
            body.extend(&(event.len() as u64).to_le_bytes());
            body.extend(event);
            (6u32, body)
        }
    };

    let mut out = Vec::with_capacity(4 + 8 + body.len());
    out.extend(&tag.to_le_bytes());
    out.extend(&(body.len() as u64).to_le_bytes());
    out.extend(body);
    Ok(out)
}

fn encode_frame_kind_payload(kind: &FrameKind) -> Result<Vec<u8>, Error> {
    match kind {
        FrameKind::Control(ctrl) => {
            let control = encode_connect_control_payload(ctrl)?;
            let mut out = Vec::with_capacity(4 + 8 + control.len());
            out.extend(&0u32.to_le_bytes());
            out.extend(&(control.len() as u64).to_le_bytes());
            out.extend(control);
            Ok(out)
        }
        FrameKind::Ciphertext(ct) => {
            let cipher = encode_field(ct)?;
            let mut out = Vec::with_capacity(4 + 8 + cipher.len());
            out.extend(&1u32.to_le_bytes());
            out.extend(&(cipher.len() as u64).to_le_bytes());
            out.extend(cipher);
            Ok(out)
        }
    }
}

fn encode_connect_frame_payload(frame: &ConnectFrameV1) -> Result<Vec<u8>, Error> {
    let mut payload = Vec::new();
    let sid = encode_field(&frame.sid)?;
    payload.extend(&(sid.len() as u64).to_le_bytes());
    payload.extend(sid);
    let dir = encode_field(&frame.dir)?;
    payload.extend(&(dir.len() as u64).to_le_bytes());
    payload.extend(dir);
    let seq = encode_field(&frame.seq)?;
    payload.extend(&(seq.len() as u64).to_le_bytes());
    payload.extend(seq);
    let kind = encode_frame_kind_payload(&frame.kind)?;
    payload.extend(&(kind.len() as u64).to_le_bytes());
    payload.extend(kind);
    Ok(payload)
}

#[inline]
fn disable_packed_struct_layout_for_connect() {
    #[cfg(debug_assertions)]
    {
        norito::disable_packed_struct_layout();
    }
}

/// Encode a Connect frame without a Norito header using the canonical layout flags.
///
/// # Errors
/// Returns [`Error`] if serialization fails or the layout flags are invalid.
pub fn encode_connect_frame_bare(frame: &ConnectFrameV1) -> Result<Vec<u8>, Error> {
    disable_packed_struct_layout_for_connect();
    encode_connect_frame_payload(frame)
}

/// Encode a Connect frame with a Norito header using the canonical layout flags.
///
/// # Errors
/// Returns [`Error`] if serialization fails or header framing cannot be generated.
pub fn encode_connect_frame_framed(frame: &ConnectFrameV1) -> Result<Vec<u8>, Error> {
    disable_packed_struct_layout_for_connect();
    let payload = encode_connect_frame_payload(frame)?;
    norito::core::frame_bare_with_header_flags::<ConnectFrameV1>(&payload, CONNECT_LAYOUT_FLAGS)
}

/// Decode a bare (headerless) Connect frame using the canonical layout flags.
///
/// # Errors
/// Returns [`Error`] if decoding fails or the flags do not match the canonical layout.
pub fn decode_connect_frame_bare(bytes: &[u8]) -> Result<ConnectFrameV1, Error> {
    disable_packed_struct_layout_for_connect();
    let _flags = DecodeFlagsGuard::enter_with_hint(CONNECT_LAYOUT_FLAGS, CONNECT_LAYOUT_FLAGS);
    decode_connect_frame_payload(bytes)
}

/// Decode a header-framed Connect frame using the canonical layout flags.
///
/// # Errors
/// Returns [`Error`] if decoding fails or the length prefix is inconsistent.
pub fn decode_connect_frame_framed(bytes: &[u8]) -> Result<ConnectFrameV1, Error> {
    disable_packed_struct_layout_for_connect();
    let header_flags = *bytes.get(Header::SIZE - 1).ok_or(Error::LengthMismatch)?;
    ensure_connect_layout(header_flags)?;
    let view = norito::core::from_bytes_view(bytes)?;
    let payload = view.as_bytes();
    let _flags = DecodeFlagsGuard::enter_with_hint(CONNECT_LAYOUT_FLAGS, CONNECT_LAYOUT_FLAGS);
    decode_connect_frame_payload(payload)
}

#[cfg(test)]
mod signature_tests {
    use super::*;

    #[test]
    fn ed25519_helper_maps_bytes() {
        let raw = [0xAAu8; 64];
        let sig = WalletSignatureV1::from_ed25519_bytes(&raw).expect("ed25519 helper");
        assert_eq!(sig.algorithm, Algorithm::Ed25519);
        assert_eq!(sig.bytes(), raw.as_slice());
    }
}

/// Message direction between roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Hash)]
#[norito(decode_from_slice)]
pub enum Dir {
    /// From application to wallet.
    AppToWallet,
    /// From wallet to application.
    WalletToApp,
}

/// Role of a WebSocket endpoint in a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
pub enum Role {
    /// Decentralized application (dApp) role.
    App,
    /// Wallet role.
    Wallet,
}

/// Canonical layout flags for Connect Norito payloads.
///
/// Connect sticks to the AoS/default Norito layout (no packed structs, packed
/// sequences, or compact lengths). Enum tags stay on the default 32‑bit
/// little‑endian discriminant emitted by `norito::derive` (see `Dir`/`Role`
/// optimisations in the derive helper).
pub const CONNECT_LAYOUT_FLAGS: u8 = 0;

fn read_len(body: &[u8], offset: &mut usize) -> Result<usize, Error> {
    let start = *offset;
    let end = start.checked_add(8).ok_or(Error::LengthMismatch)?;
    if end > body.len() {
        return Err(Error::LengthMismatch);
    }
    let len = usize::try_from(u64::from_le_bytes(body[start..end].try_into().unwrap()))
        .expect("connect frame length fits in usize");
    *offset = end;
    Ok(len)
}

fn read_slice(body: &[u8], offset: usize, len: usize) -> Result<&[u8], Error> {
    let end = offset.checked_add(len).ok_or(Error::LengthMismatch)?;
    if end > body.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(&body[offset..end])
}

fn decode_field_with_len<T: Decode + Encode>(
    body: &[u8],
    offset: &mut usize,
    len: usize,
    label: &'static str,
) -> Result<T, Error> {
    let slice = read_slice(body, *offset, len)?;
    let (value, used) = norito::core::decode_field_canonical::<T>(slice)
        .map_err(|err| Error::Message(format!("{label} decode failed: {err}")))?;
    if used != len {
        return Err(Error::Message(format!(
            "{label} length mismatch: expected {len} used {used}"
        )));
    }
    *offset = offset.checked_add(used).ok_or(Error::LengthMismatch)?;
    Ok(value)
}

#[allow(clippy::too_many_lines)]
fn decode_connect_control_payload(bytes: &[u8]) -> Result<(ConnectControlV1, usize), Error> {
    if bytes.len() < 12 {
        return Err(Error::LengthMismatch);
    }
    let tag = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let payload_len = usize::try_from(u64::from_le_bytes(bytes[4..12].try_into().unwrap()))
        .expect("connect payload length fits in usize");
    #[cfg(test)]
    eprintln!(
        "decode_connect_control_payload tag={tag} payload_len={payload_len} total_bytes={}",
        bytes.len()
    );
    let body_start = 12usize;
    let body_end = body_start
        .checked_add(payload_len)
        .ok_or(Error::LengthMismatch)?;
    if body_end > bytes.len() {
        return Err(Error::LengthMismatch);
    }
    let body = &bytes[body_start..body_end];
    let mut offset = 0usize;
    let control = match tag {
        0 => {
            let app_pk_len = read_len(body, &mut offset)?;
            let app_pk =
                decode_field_with_len::<[u8; 32]>(body, &mut offset, app_pk_len, "app_pk")?;
            let app_meta_len = read_len(body, &mut offset)?;
            let app_meta = decode_field_with_len::<Option<AppMeta>>(
                body,
                &mut offset,
                app_meta_len,
                "app_meta",
            )?;
            let constraints_len = read_len(body, &mut offset)?;
            let constraints = decode_field_with_len::<Constraints>(
                body,
                &mut offset,
                constraints_len,
                "constraints",
            )?;
            let permissions_len = read_len(body, &mut offset)?;
            let permissions = decode_field_with_len::<Option<PermissionsV1>>(
                body,
                &mut offset,
                permissions_len,
                "permissions",
            )?;
            ConnectControlV1::Open {
                app_pk,
                app_meta,
                constraints,
                permissions,
            }
        }
        1 => {
            let wallet_pk_len = read_len(body, &mut offset)?;
            #[cfg(test)]
            eprintln!(
                "approve decode wallet_pk_len={wallet_pk_len} body_len={}",
                body.len()
            );
            let wallet_pk =
                decode_field_with_len::<[u8; 32]>(body, &mut offset, wallet_pk_len, "wallet_pk")?;
            let account_id_len = read_len(body, &mut offset)?;
            let account_id =
                decode_field_with_len::<String>(body, &mut offset, account_id_len, "account_id")?;
            let permissions_len = read_len(body, &mut offset)?;
            let permissions = decode_field_with_len::<Option<PermissionsV1>>(
                body,
                &mut offset,
                permissions_len,
                "permissions",
            )?;
            let proof_len = read_len(body, &mut offset)?;
            let proof = decode_field_with_len::<Option<SignInProofV1>>(
                body,
                &mut offset,
                proof_len,
                "proof",
            )?;
            let sig_wallet_len = read_len(body, &mut offset)?;
            let sig_wallet = decode_field_with_len::<WalletSignatureV1>(
                body,
                &mut offset,
                sig_wallet_len,
                "sig_wallet",
            )?;
            ConnectControlV1::Approve {
                wallet_pk,
                account_id,
                permissions,
                proof,
                sig_wallet,
            }
        }
        2 => {
            let code_len = read_len(body, &mut offset)?;
            let code = decode_field_with_len::<u16>(body, &mut offset, code_len, "code")?;
            let code_id_len = read_len(body, &mut offset)?;
            let code_id =
                decode_field_with_len::<String>(body, &mut offset, code_id_len, "code_id")?;
            let reason_len = read_len(body, &mut offset)?;
            let reason = decode_field_with_len::<String>(body, &mut offset, reason_len, "reason")?;
            ConnectControlV1::Reject {
                code,
                code_id,
                reason,
            }
        }
        3 => {
            let who_len = read_len(body, &mut offset)?;
            let who = decode_field_with_len::<Role>(body, &mut offset, who_len, "who")?;
            let code_len = read_len(body, &mut offset)?;
            let code = decode_field_with_len::<u16>(body, &mut offset, code_len, "code")?;
            let reason_len = read_len(body, &mut offset)?;
            let reason = decode_field_with_len::<String>(body, &mut offset, reason_len, "reason")?;
            let retryable_len = read_len(body, &mut offset)?;
            let retryable =
                decode_field_with_len::<bool>(body, &mut offset, retryable_len, "retryable")?;
            ConnectControlV1::Close {
                who,
                code,
                reason,
                retryable,
            }
        }
        4 => {
            let nonce_len = read_len(body, &mut offset)?;
            let nonce = decode_field_with_len::<u64>(body, &mut offset, nonce_len, "nonce")?;
            ConnectControlV1::Ping { nonce }
        }
        5 => {
            let nonce_len = read_len(body, &mut offset)?;
            let nonce = decode_field_with_len::<u64>(body, &mut offset, nonce_len, "nonce")?;
            ConnectControlV1::Pong { nonce }
        }
        6 => {
            let event_len = read_len(body, &mut offset)?;
            let event =
                decode_field_with_len::<ServerEventV1>(body, &mut offset, event_len, "event")?;
            ConnectControlV1::ServerEvent { event }
        }
        _ => {
            let tag_u8 = u8::try_from(tag).unwrap_or(u8::MAX);
            return Err(Error::invalid_tag("ConnectControlV1", tag_u8));
        }
    };
    if offset != payload_len {
        #[cfg(test)]
        eprintln!(
            "ConnectControlV1 decode mismatch: tag={tag} expected={payload_len} used={offset}"
        );
        return Err(Error::Message(format!(
            "ConnectControlV1 length mismatch: expected {payload_len} used {offset}"
        )));
    }
    Ok((control, body_end))
}

fn decode_frame_kind_payload(bytes: &[u8]) -> Result<(FrameKind, usize), Error> {
    if bytes.len() < 4 {
        return Err(Error::LengthMismatch);
    }
    let tag = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if tag == 0 {
        if bytes.len() < 12 {
            return Err(Error::LengthMismatch);
        }
        let len = usize::try_from(u64::from_le_bytes(bytes[4..12].try_into().unwrap()))
            .expect("connect payload length fits in usize");
        let body_start = 12usize;
        let body_end = body_start.checked_add(len).ok_or(Error::LengthMismatch)?;
        if body_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let (ctrl, used) = decode_connect_control_payload(&bytes[body_start..body_end])?;
        if used != len {
            return Err(Error::Message(format!(
                "FrameKind::Control length mismatch: expected {len} used {used}"
            )));
        }
        return Ok((FrameKind::Control(ctrl), body_end));
    } else if tag == 1 {
        if bytes.len() < 12 {
            return Err(Error::LengthMismatch);
        }
        let len = usize::try_from(u64::from_le_bytes(bytes[4..12].try_into().unwrap()))
            .expect("connect payload length fits in usize");
        let body_start = 12usize;
        let body_end = body_start.checked_add(len).ok_or(Error::LengthMismatch)?;
        if body_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let (cipher, used) = norito::core::decode_field_canonical::<ConnectCiphertextV1>(
            &bytes[body_start..body_end],
        )?;
        if used != len {
            return Err(Error::Message(format!(
                "FrameKind::Ciphertext length mismatch: expected {len} used {used}"
            )));
        }
        return Ok((FrameKind::Ciphertext(cipher), body_end));
    }
    let tag_u8 = u8::try_from(tag).unwrap_or(u8::MAX);
    Err(Error::invalid_tag("FrameKind", tag_u8))
}

fn decode_connect_frame_payload(bytes: &[u8]) -> Result<ConnectFrameV1, Error> {
    let mut offset = 0usize;
    let sid_len = read_len(bytes, &mut offset)?;
    let sid = decode_field_with_len::<[u8; 32]>(bytes, &mut offset, sid_len, "sid")?;
    let dir_len = read_len(bytes, &mut offset)?;
    let dir = decode_field_with_len::<Dir>(bytes, &mut offset, dir_len, "dir")?;
    let seq_len = read_len(bytes, &mut offset)?;
    let seq = decode_field_with_len::<u64>(bytes, &mut offset, seq_len, "seq")?;
    let kind_len = read_len(bytes, &mut offset)?;
    let kind_slice = read_slice(bytes, offset, kind_len)?;
    let (kind, used_kind) = decode_frame_kind_payload(kind_slice)?;
    if used_kind != kind_len {
        return Err(Error::Message(format!(
            "FrameKind length mismatch: expected {kind_len} used {used_kind}"
        )));
    }
    offset = offset.checked_add(used_kind).ok_or(Error::LengthMismatch)?;
    if offset != bytes.len() {
        return Err(Error::Message(format!(
            "ConnectFrameV1 length mismatch: used={offset} available={}",
            bytes.len()
        )));
    }
    Ok(ConnectFrameV1 {
        sid,
        dir,
        seq,
        kind,
    })
}

/// Top‑level frame routed over P2P and delivered over WS.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ConnectFrameV1 {
    /// 32‑byte session identifier.
    pub sid: [u8; 32],
    /// Logical direction of the message.
    pub dir: Dir,
    /// Monotonic per‑direction sequence number starting from 1 for each sender.
    /// Server events use a separate server-side sequence and do not advance
    /// app/wallet ciphertext sequencing.
    pub seq: u64,
    /// Frame payload: control or encrypted content.
    pub kind: FrameKind,
}

/// Payload of a [`ConnectFrameV1`].
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::large_enum_variant)]
pub enum FrameKind {
    /// Unencrypted control messages needed to establish a secure channel.
    Control(ConnectControlV1),
    /// Encrypted application payload.
    Ciphertext(ConnectCiphertextV1),
}

/// Unencrypted control messages exchanged during session setup and lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub enum ConnectControlV1 {
    /// Open a session from the application side (minimal plaintext).
    Open {
        /// Application ephemeral X25519 public key (32 bytes).
        app_pk: [u8; 32],
        /// Optional application metadata for display.
        app_meta: Option<AppMeta>,
        /// Constraints such as chain id.
        constraints: Constraints,
        /// Requested permissions/namespaces. Wallet may narrow in Approve.
        permissions: Option<PermissionsV1>,
    },
    /// Approve the session from the wallet side.
    Approve {
        /// Wallet ephemeral X25519 public key (32 bytes).
        wallet_pk: [u8; 32],
        /// Wallet account identifier (canonical i105 string).
        account_id: String,
        /// Accepted (possibly narrowed) permissions/namespaces.
        permissions: Option<PermissionsV1>,
        /// Optional sign‑in proof fields (SIWE‑like) bound into the signature.
        proof: Option<SignInProofV1>,
        /// Detached signature by the wallet's account key over the approval message.
        /// Carries the algorithm so verifiers can interpret the payload.
        sig_wallet: WalletSignatureV1,
    },
    /// Reject the session from the wallet side.
    Reject {
        /// Stable numeric code.
        code: u16,
        /// Stable identifier (`UPPER_SNAKE`); e.g., "UNAUTHORIZED", "`USER_DENIED`".
        code_id: String,
        /// Human‑readable reason string.
        reason: String,
    },
    /// Close the session (either side).
    Close {
        /// Who initiated the close.
        who: Role,
        /// Implementation‑defined close code.
        code: u16,
        /// Human‑readable reason string.
        reason: String,
        /// Whether the client may retry automatically.
        retryable: bool,
    },
    /// Liveness probe.
    Ping {
        /// Opaque nonce used to correlate Ping/Pong pairs.
        nonce: u64,
    },
    /// Liveness reply.
    Pong {
        /// Opaque nonce echoed back from the corresponding Ping.
        nonce: u64,
    },
    /// Server-initiated event delivered over the plaintext control channel.
    ServerEvent {
        /// Event payload carrying ledger updates or diagnostics.
        event: ServerEventV1,
    },
}

/// AEAD ciphertext envelope (end‑to‑end encrypted between app and wallet).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub struct ConnectCiphertextV1 {
    /// Direction this ciphertext is intended for (binds to per‑direction key/nonce).
    pub dir: Dir,
    /// AEAD payload (ciphertext + tag), format defined by the chosen cipher.
    ///
    /// Note: v0 enforces `Envelope.seq == frame.seq`; nonces are derived from
    /// the same monotonic sequence (dir‑specific). Implementations must bind the
    /// outer header (version, sid, dir, seq, kind) via AEAD AAD.
    pub aead: Vec<u8>,
}

/// Plaintext envelope carried inside the AEAD for replay protection and routing.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub struct EnvelopeV1 {
    /// Monotonic sequence matching the outer frame `seq`.
    pub seq: u64,
    /// Application payload.
    pub payload: ConnectPayloadV1,
}

// Strict bare decode is provided by norito_derive when annotated with
// `#[norito(decode_from_slice)]` on the container types above.

/// Minimal encrypted payload set for v1.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub enum ConnectPayloadV1 {
    /// Encrypted control messages post-approval (e.g., Close/Reject).
    Control(ControlAfterKeyV1),
    /// Request the wallet to sign opaque bytes under a domain separation tag.
    SignRequestRaw {
        /// Domain separation tag for the signature prompt.
        domain_tag: String,
        /// Opaque bytes to sign.
        bytes: Vec<u8>,
    },
    /// Request the wallet to sign a canonical Iroha transaction payload.
    SignRequestTx {
        /// Canonical transaction payload bytes to be signed by the wallet.
        tx_bytes: Vec<u8>,
    },
    /// Successful signing result.
    SignResultOk {
        /// Detached signature along with its algorithm identifier.
        signature: WalletSignatureV1,
    },
    /// Error during signing or user rejection.
    SignResultErr {
        /// Stable error code string.
        code: String,
        /// Human‑readable message.
        message: String,
    },
    /// Optional UX prompt for display on the wallet.
    DisplayRequest {
        /// Title text.
        title: String,
        /// Body/description text.
        body: String,
    },
}

/// Control messages that must be encrypted after key establishment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub enum ControlAfterKeyV1 {
    /// Close the established session (encrypted control).
    ///
    /// Mirrors [`ConnectControlV1::Close`] but must be sent over the encrypted channel
    /// after key establishment to avoid leaking metadata over plaintext.
    Close {
        /// Who initiated the close (app or wallet).
        who: Role,
        /// Implementation‑defined close code for diagnostics/analytics.
        code: u16,
        /// Human‑readable reason string suitable for logs or UI.
        reason: String,
        /// Whether the client may attempt an automatic retry/reconnect.
        retryable: bool,
    },
    /// Reject a request or abort a session (encrypted control).
    ///
    /// Used for post‑approval errors, policy violations, or user actions that
    /// should be communicated securely after the channel is established.
    Reject {
        /// Stable numeric code describing the class of rejection.
        code: u16,
        /// Stable identifier (`UPPER_SNAKE`), e.g., "UNAUTHORIZED", "`USER_DENIED`".
        code_id: String,
        /// Human‑readable reason text for display or logs.
        reason: String,
    },
}

/// Optional application metadata for display.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub struct AppMeta {
    /// Application display name.
    pub name: String,
    /// Optional homepage URL.
    pub url: Option<String>,
    /// Optional icon hash (algorithm‑qualified, e.g., blake2b:... ).
    pub icon_hash: Option<String>,
}

/// Optional wallet metadata for display.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub struct WalletMeta {
    /// Wallet display name.
    pub name: String,
    /// Optional homepage URL.
    pub url: Option<String>,
    /// Optional icon hash (algorithm‑qualified, e.g., blake2b:... ).
    pub icon_hash: Option<String>,
}

/// Server-to-client events that do not require encryption.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub enum ServerEventV1 {
    /// Deterministic block proof payload for light clients.
    BlockProofs {
        /// Block height.
        height: u64,
        /// Hex-encoded transaction entry hash.
        entry_hash: String,
        /// Norito JSON representation of `BlockProofs`.
        proofs_json: String,
    },
}

/// Session constraints advertised by the application.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub struct Constraints {
    /// Target chain identifier.
    pub chain_id: String,
}

/// Requested/accepted permissions (WalletConnect‑style namespaces).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub struct PermissionsV1 {
    /// Allowed methods/payload kinds (upper‑snake or dotted paths).
    /// Examples: `SIGN_REQUEST_RAW`, `SIGN_REQUEST_TX`, `DISPLAY_REQUEST`.
    pub methods: Vec<String>,
    /// Allowed unsolicited events the app may emit for display.
    pub events: Vec<String>,
    /// Optional resource identifiers for scoping (e.g., accounts, domains).
    pub resources: Option<Vec<String>>,
}

/// Optional sign‑in proof (akin to SIWE), carried in Approve.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[allow(clippy::size_of_ref)]
pub struct SignInProofV1 {
    /// App domain (host) expected by the wallet.
    pub domain: String,
    /// URI of the requesting dApp.
    pub uri: String,
    /// Human‑readable statement/purpose.
    pub statement: String,
    /// RFC 3339/ISO‑8601 issued‑at timestamp.
    pub issued_at: String,
    /// Unique nonce provided by the dApp to prevent replay.
    pub nonce: String,
}

/// Canonical aliases for the current Connect wire format.
/// Wallet signature used in control and payload messages.
pub type WalletSignature = WalletSignatureV1;
/// Top-level Connect frame routed over transport channels.
pub type ConnectFrame = ConnectFrameV1;
/// Plaintext control channel payload used during session lifecycle.
pub type ConnectControl = ConnectControlV1;
/// Ciphertext envelope metadata carried in a frame.
pub type ConnectCiphertext = ConnectCiphertextV1;
/// Decrypted envelope containing sequence and payload.
pub type Envelope = EnvelopeV1;
/// Encrypted payload variants carried inside an envelope.
pub type ConnectPayload = ConnectPayloadV1;
/// Encrypted control variants permitted after key establishment.
pub type ControlAfterKey = ControlAfterKeyV1;
/// Server-generated event payload delivered on the control channel.
pub type ServerEvent = ServerEventV1;
/// Permission request/approval surface.
pub type Permissions = PermissionsV1;
/// Optional sign-in proof payload.
pub type SignInProof = SignInProofV1;

#[cfg(test)]
mod tests {
    use norito::core::{Error, header_flags};
    use rand::{Rng, SeedableRng};

    use super::*;
    use iroha_crypto::PublicKey;
    use iroha_data_model::{account::AccountId, domain::DomainId};

    fn sample_open_frame() -> ConnectFrameV1 {
        let sid = [0xAB; 32];
        ConnectFrameV1 {
            sid,
            dir: Dir::AppToWallet,
            seq: 1,
            kind: FrameKind::Control(ConnectControlV1::Open {
                app_pk: [1u8; 32],
                app_meta: Some(AppMeta {
                    name: "DemoApp".into(),
                    url: Some("https://example.org".into()),
                    icon_hash: None,
                }),
                constraints: Constraints {
                    chain_id: "testnet".into(),
                },
                permissions: Some(PermissionsV1 {
                    methods: vec!["SIGN_REQUEST_RAW".into(), "SIGN_REQUEST_TX".into()],
                    events: vec!["DISPLAY_REQUEST".into()],
                    resources: None,
                }),
            }),
        }
    }

    fn sample_approve_frame() -> ConnectFrameV1 {
        let sid = [0xCD; 32];
        ConnectFrameV1 {
            sid,
            dir: Dir::WalletToApp,
            seq: 2,
            kind: FrameKind::Control(sample_approve_control()),
        }
    }

    #[test]
    fn read_slice_bounds_check() {
        let body = [10u8, 20, 30, 40];
        let slice = read_slice(&body, 1, 2).expect("slice");
        assert_eq!(slice, &[20, 30]);
        assert!(matches!(
            read_slice(&body, 3, 2),
            Err(Error::LengthMismatch)
        ));
    }

    fn sample_reject_frame() -> ConnectFrameV1 {
        let sid = [0xEF; 32];
        ConnectFrameV1 {
            sid,
            dir: Dir::AppToWallet,
            seq: 3,
            kind: FrameKind::Control(ConnectControlV1::Reject {
                code: 401,
                code_id: "UNAUTHORIZED".into(),
                reason: "bad key".into(),
            }),
        }
    }

    fn sample_approve_control() -> ConnectControlV1 {
        let domain: DomainId = "wonderland".parse().expect("domain parses");
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key parses");
        let _ = domain;
        let account_id = AccountId::new(public_key).to_string();
        ConnectControlV1::Approve {
            wallet_pk: [0x42; 32],
            account_id,
            permissions: None,
            proof: Some(SignInProofV1 {
                domain: "example.org".into(),
                uri: "https://example.org".into(),
                statement: "Sign in".into(),
                issued_at: "2025-01-01T00:00:00Z".into(),
                nonce: "abc".into(),
            }),
            sig_wallet: WalletSignatureV1::new(
                Algorithm::Ed25519,
                Signature::from_bytes(&[9u8; 64]),
            ),
        }
    }

    #[test]
    fn archived_alignments_are_natural() {
        assert!(std::mem::align_of::<norito::core::Archived<ConnectFrameV1>>() >= 8);
        assert!(std::mem::align_of::<norito::core::Archived<FrameKind>>() >= 4);
        assert!(std::mem::align_of::<norito::core::Archived<ConnectControlV1>>() >= 4);
    }

    #[test]
    fn frame_kind_roundtrip_header_framed() -> Result<(), Error> {
        norito::disable_packed_struct_layout();
        let fk = FrameKind::Control(sample_approve_control());
        let payload = encode_frame_kind_payload(&fk)?;
        let framed = norito::core::frame_bare_with_header_flags::<FrameKind>(
            &payload,
            CONNECT_LAYOUT_FLAGS,
        )?;
        let decoded_payload = norito::core::from_bytes_view(&framed)?;
        let decoded = decode_frame_kind_payload(decoded_payload.as_bytes())?.0;
        assert_eq!(decoded, fk);
        Ok(())
    }

    #[test]
    fn frame_kind_roundtrip_debug_payload_decode() {
        norito::disable_packed_struct_layout();
        let fk = FrameKind::Control(ConnectControlV1::Ping { nonce: 7 });
        let payload = encode_frame_kind_payload(&fk).expect("encode");
        eprintln!(
            "[debug] payload_len={} tag_le={} head={:02x?}",
            payload.len(),
            u32::from_le_bytes(payload[0..4].try_into().unwrap()),
            &payload[..payload.len().min(32)]
        );
        let framed =
            norito::core::frame_bare_with_header_flags::<FrameKind>(&payload, CONNECT_LAYOUT_FLAGS)
                .expect("frame payload");
        assert_eq!(framed[Header::SIZE - 1], CONNECT_LAYOUT_FLAGS);
        let (decoded, used) = decode_frame_kind_payload(&payload).expect("decode payload");
        assert_eq!(used, payload.len());
        assert_eq!(decoded, fk);
    }

    #[test]
    fn connect_control_roundtrip_header() {
        norito::disable_packed_struct_layout();
        let ctrl = sample_approve_control();
        if let ConnectControlV1::Approve { wallet_pk, .. } = &ctrl {
            let enc = encode_field(wallet_pk).expect("encode wallet_pk");
            eprintln!("wallet_pk encoded len={}", enc.len());
        }
        let payload = encode_connect_control_payload(&ctrl).expect("encode control");
        let framed = norito::core::frame_bare_with_header_flags::<ConnectControlV1>(
            &payload,
            CONNECT_LAYOUT_FLAGS,
        )
        .expect("frame control");
        let view = norito::core::from_bytes_view(&framed).expect("view");
        let decoded = decode_connect_control_payload(view.as_bytes());
        if let Err(err) = &decoded {
            eprintln!("connect_control_roundtrip_header decode error: {err}");
        }
        let decoded = decoded.expect("decode control").0;
        assert_eq!(decoded, ctrl);
    }

    #[test]
    fn array_encoding_layout() {
        norito::disable_packed_struct_layout();
        let arr = [0xABu8; 32];
        let framed = norito::to_bytes(&arr).expect("encode array");
        let payload = norito::core::from_bytes_view(&framed)
            .expect("view")
            .as_bytes()
            .to_vec();
        eprintln!(
            "array payload len={} head={:02x?}",
            payload.len(),
            &payload[..payload.len().min(64)]
        );
        let decoded_field = norito::core::decode_field_canonical::<[u8; 32]>(&payload);
        eprintln!("array decode_field_canonical: {decoded_field:?}");
        let (decoded_field, used) = decoded_field.expect("field decode");
        assert_eq!(used, payload.len());
        assert_eq!(decoded_field, arr);
        let decoded: [u8; 32] = norito::decode_from_bytes(&framed).expect("decode");
        assert_eq!(decoded, arr);
    }

    #[test]
    fn open_frame_roundtrip_framed() {
        let frame = sample_open_frame();
        let bytes = encode_connect_frame_framed(&frame).expect("encode framed");
        assert_eq!(bytes[Header::SIZE - 1], CONNECT_LAYOUT_FLAGS);
        let decoded = decode_connect_frame_framed(&bytes).expect("decode framed");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn ciphertext_envelope_roundtrip() {
        let env = EnvelopeV1 {
            seq: 42,
            payload: ConnectPayloadV1::SignRequestRaw {
                domain_tag: "iroha-connect|test".into(),
                bytes: vec![1, 2, 3, 4],
            },
        };
        let buf = norito::to_bytes(&env).expect("encode");
        let view = norito::core::from_bytes_view(&buf).expect("decode view");
        let back: EnvelopeV1 = view.decode().expect("decode framed");
        assert_eq!(env, back);
    }

    #[test]
    fn approve_frame_roundtrip_bare() {
        let frame = sample_approve_frame();
        let bytes = encode_connect_frame_bare(&frame).expect("encode bare");
        let decoded = decode_connect_frame_bare(&bytes).expect("decode bare");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn reject_frame_roundtrip_bare() {
        let frame = sample_reject_frame();
        let bytes = encode_connect_frame_bare(&frame).expect("encode bare");
        let decoded = decode_connect_frame_bare(&bytes).expect("decode bare");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn server_event_block_proof_roundtrip() {
        let frame = ConnectFrameV1 {
            sid: [9u8; 32],
            dir: Dir::AppToWallet,
            seq: 7,
            kind: FrameKind::Control(ConnectControlV1::ServerEvent {
                event: ServerEventV1::BlockProofs {
                    height: 5,
                    entry_hash: "ab12".into(),
                    proofs_json: r#"{"foo":"bar"}"#.into(),
                },
            }),
        };
        let bytes = encode_connect_frame_framed(&frame).expect("encode framed");
        let decoded = decode_connect_frame_framed(&bytes).expect("decode framed");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn connect_frame_truncated_payload_returns_err() {
        let frame = sample_open_frame();
        let mut payload = encode_connect_frame_framed(&frame).expect("encode framed");
        payload.pop();
        let result = decode_connect_frame_framed(&payload);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[test]
    fn connect_frame_bare_invalid_field_length_returns_err() {
        let mut payload = Vec::new();
        payload.extend(&300u64.to_le_bytes());
        let result = decode_connect_frame_bare(&payload);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[test]
    fn frame_kind_invalid_tag_returns_err() {
        let mut payload = encode_frame_kind_payload(&FrameKind::Control(ConnectControlV1::Close {
            who: Role::Wallet,
            code: 900,
            reason: "shutdown".into(),
            retryable: true,
        }))
        .expect("encode");
        if let Some(byte) = payload.first_mut() {
            *byte = 0xFF;
        }
        let result = decode_frame_kind_payload(&payload);
        assert!(matches!(result, Err(Error::InvalidTag { .. })));
    }

    #[test]
    fn connect_control_invalid_tag_returns_err() {
        let mut payload =
            encode_connect_control_payload(&ConnectControlV1::Ping { nonce: 7 }).expect("encode");
        if let Some(byte) = payload.first_mut() {
            *byte = 0xFF;
        }
        let result = decode_connect_control_payload(&payload);
        assert!(matches!(result, Err(Error::InvalidTag { .. })));
    }

    #[test]
    fn framed_checksum_mismatch_returns_err() {
        let frame = ConnectFrameV1 {
            sid: [0x11; 32],
            dir: Dir::AppToWallet,
            seq: 9,
            kind: FrameKind::Control(ConnectControlV1::Ping { nonce: 7 }),
        };
        let mut bytes = encode_connect_frame_framed(&frame).expect("encode framed");
        if let Some(last) = bytes.last_mut() {
            *last ^= 0xFF;
        }
        let result = decode_connect_frame_framed(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn framed_invalid_flags_rejected() {
        let frame = sample_open_frame();
        let mut bytes = encode_connect_frame_framed(&frame).expect("encode framed");
        let idx = Header::SIZE - 1;
        bytes[idx] = header_flags::PACKED_SEQ;
        let result = decode_connect_frame_framed(&bytes);
        assert!(matches!(result, Err(Error::UnsupportedFeature(_))));
    }

    #[test]
    fn truncated_headers_and_noise_fail_cleanly() {
        let frame = sample_open_frame();
        let bytes = encode_connect_frame_framed(&frame).expect("encode framed");
        for cut in 0..Header::SIZE {
            assert!(decode_connect_frame_framed(&bytes[..cut]).is_err());
        }
        let mut rng = rand::rngs::StdRng::seed_from_u64(0xC0DE_C0DE);
        for _ in 0..32 {
            let len = rng.random_range(0..Header::SIZE + 8);
            let mut buf = vec![0u8; len];
            rng.fill(buf.as_mut_slice());
            let _ = decode_connect_frame_framed(&buf);
        }
    }

    #[test]
    fn framed_oversized_length_rejected() {
        const LENGTH_OFFSET: usize = 4 + 1 + 1 + 16 + 1;
        let frame = sample_open_frame();
        let mut bytes = encode_connect_frame_framed(&frame).expect("encode framed");
        bytes[LENGTH_OFFSET..LENGTH_OFFSET + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        let err = decode_connect_frame_framed(&bytes).expect_err("should reject oversize len");
        assert!(matches!(
            err,
            Error::ArchiveLengthExceeded { .. } | Error::LengthMismatch
        ));
    }
}
