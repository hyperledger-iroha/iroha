#![allow(
    clippy::mut_range_bound,
    clippy::cast_possible_truncation,
    clippy::useless_let_if_seq,
    clippy::cast_lossless
)]
//! Signature/SignatureOf Norito bare-codec layout sanity checks.
use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
use norito::{
    codec::Encode as _,
    core,
    core::{DecodeFromSlice, Header},
};
fn checked_ed25519_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate checked signature-layout Ed25519 keypair")
}
#[test]
fn signature_layout_fixture_uses_checked_ed25519_keypair() {
    let key_pair = checked_ed25519_keypair();
    assert_eq!(key_pair.public_key().algorithm(), Algorithm::Ed25519);
}
fn dump_header(label: &str, bytes: &[u8]) {
    use norito::core::{Header, header_flags};
    use std::fmt::Write as _;
    if bytes.len() < Header::SIZE {
        eprintln!("{label}: buffer shorter than header (len={})", bytes.len());
        return;
    }
    let header = &bytes[..Header::SIZE];
    let major = header[4];
    let minor = header[5];
    let mut len_bytes = [0u8; 8];
    len_bytes.copy_from_slice(&header[23..31]);
    let payload_len = u64::from_le_bytes(len_bytes);
    let flags = header[Header::SIZE - 1];
    let mut checksum_bytes = [0u8; 8];
    checksum_bytes.copy_from_slice(&header[31..39]);
    let checksum = u64::from_le_bytes(checksum_bytes);
    let mut flag_desc = String::new();
    let mut push_flag = |name| {
        if !flag_desc.is_empty() {
            let _ = write!(flag_desc, "|");
        }
        let _ = write!(flag_desc, "{name}");
    };
    if (flags & header_flags::COMPACT_LEN) != 0 {
        push_flag("COMPACT_LEN");
    }
    if flag_desc.is_empty() {
        flag_desc.push_str("<none>");
    }
    eprintln!(
        "{label}: len={} payload_len={} checksum=0x{checksum:016x} version={major}.{minor} flags=0x{flags:02x} ({flag_desc})",
        bytes.len(),
        payload_len
    );
    let body_prefix = &bytes[Header::SIZE..bytes.len().min(Header::SIZE + 16)];
    eprintln!("{label} body prefix={body_prefix:02x?}");
}
#[test]
fn signature_bare_default_layout_frames_each_payload_byte_compactly() {
    // The bare codec uses the default COMPACT_LEN layout: a fixed u64 sequence
    // count followed by one compact-length-framed byte per element.
    let sig = Signature::from_bytes(&[0xAA, 0xBB, 0xCC, 0xDD]);
    let bytes = sig.encode();
    let flags = core::default_encode_flags();
    assert_eq!(flags, core::header_flags::COMPACT_LEN);
    let mut expected = Vec::new();
    expected.extend_from_slice(&(sig.payload().len() as u64).to_le_bytes());
    for byte in sig.payload() {
        core::write_len_to_vec_with_flags(&mut expected, 1, flags);
        expected.push(*byte);
    }
    assert_eq!(bytes, expected);
    let (decoded, used) =
        Signature::decode_from_slice(&bytes).expect("decode bare signature payload");
    assert_eq!(used, bytes.len());
    assert_eq!(decoded, sig);
}
#[test]
fn signature_bare_unpacked_layout_frames_each_payload_byte() {
    // Select the fixed-width length layout (header flags 0).
    let sig = Signature::from_bytes(&[1, 2, 3]);
    let _fg = core::DecodeFlagsGuard::enter(0);
    let mut out = Vec::new();
    norito::core::serialize_to_buffer(&sig, &mut out).expect("serialize");
    assert!(out.len() > sig.payload().len());
    // The unpacked layout starts with a little-endian sequence count.
    let mut len_bytes = [0u8; 8];
    len_bytes.copy_from_slice(&out[..8]);
    assert_eq!(u64::from_le_bytes(len_bytes), sig.payload().len() as u64);
    let mut expected = Vec::new();
    expected.extend_from_slice(&(sig.payload().len() as u64).to_le_bytes());
    for byte in sig.payload() {
        expected.extend_from_slice(&1_u64.to_le_bytes());
        expected.push(*byte);
    }
    assert_eq!(out, expected);
    let (decoded, consumed) =
        Signature::decode_from_slice(out.as_slice()).expect("decode compat body");
    assert_eq!(consumed, out.len());
    assert_eq!(decoded, sig);
}
#[test]
fn signature_of_delegates_to_signature_layout() {
    // SignatureOf<T> should encode exactly like Signature (transparent newtype)
    // Build a real SignatureOf by signing the same message; then construct a Signature
    // from its inner payload and compare bare bytes.
    let key_pair = checked_ed25519_keypair();
    let msg = ();
    let wrapped: iroha_crypto::SignatureOf<()> =
        iroha_crypto::SignatureOf::try_new(key_pair.private_key(), &msg)
            .expect("fixture Ed25519 typed signature");
    let base = Signature::from_bytes(wrapped.payload());
    assert_eq!(
        base.encode(),
        wrapped.encode(),
        "SignatureOf must delegate to Signature encoding"
    );
    // The advertised unpacked path also has the same shape.
    let _fg = core::DecodeFlagsGuard::enter(0);
    let mut s1 = Vec::new();
    norito::core::serialize_to_buffer(&base, &mut s1).expect("serialize sig");
    let mut s2 = Vec::new();
    norito::core::serialize_to_buffer(&wrapped, &mut s2).expect("serialize sigof");
    assert_eq!(s1, s2);
}
#[test]
fn signature_large_payload_layout_debug() {
    let payload: Vec<u8> = (0..1235u16).map(|i| (i % 251) as u8).collect();
    let sig = Signature::from_bytes(&payload);
    let bytes = norito::to_bytes(&sig).expect("encode");
    println!(
        "large sig flags=0x{:02x} len={} prefix={:02x?}",
        bytes[norito::core::Header::SIZE - 1],
        bytes.len(),
        &bytes[norito::core::Header::SIZE..norito::core::Header::SIZE + 32]
    );
}
#[test]
#[ignore = "diagnostic output"]
fn signature_of_norito_payload_diagnostics() {
    let key_pair = checked_ed25519_keypair();
    let sig_of = SignatureOf::try_new(key_pair.private_key(), &())
        .expect("diagnostic fixture Ed25519 typed signature");
    let bytes = norito::to_bytes(&sig_of).expect("encode SignatureOf");
    dump_header("SignatureOf", &bytes);
    let body = &bytes[Header::SIZE..];
    let (count, prefix) =
        core::inspect_seq_len_slice(body).expect("SignatureOf payload sequence count");
    eprintln!(
        "SignatureOf payload count={count} (prefix bytes={prefix}) body len={}",
        body.len()
    );
}
