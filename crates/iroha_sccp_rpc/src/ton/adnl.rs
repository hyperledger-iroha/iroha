//! ADNL-TCP handshake and framing for TON liteservers (spec §8).
//!
//! x25519 key agreement against the server's Ed25519 identity, AES-256-CTR stream ciphers and
//! SHA-256 framed packets.

// TODO(ws25): implement the ADNL handshake, framing and zeroization of session keys.
