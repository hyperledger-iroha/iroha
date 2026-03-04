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

fn read_varint(bytes: &[u8]) -> (usize, usize) {
    let mut i = 0usize;
    let mut val: u64 = 0;
    let mut shift = 0u32;
    loop {
        let b = bytes[i];
        i += 1;
        val |= u64::from(b & 0x7F) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    (usize::try_from(val).unwrap_or(usize::MAX), i)
}

fn dump_header(label: &str, bytes: &[u8]) {
    use std::fmt::Write as _;

    use norito::core::{Header, header_flags};

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
    if (flags & header_flags::PACKED_STRUCT) != 0 {
        push_flag("PACKED_STRUCT");
    }
    if (flags & header_flags::FIELD_BITSET) != 0 {
        push_flag("FIELD_BITSET");
    }
    if (flags & header_flags::PACKED_SEQ) != 0 {
        push_flag("PACKED_SEQ");
    }
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
fn signature_bare_hybrid_is_bitset_plus_constvec() {
    // Build a tiny signature payload and encode via bare codec (hybrid packed-struct enabled by default)
    let sig = Signature::from_bytes(&[0xAA, 0xBB, 0xCC, 0xDD]);
    let bytes = sig.encode();

    assert!(!bytes.is_empty());
    let bitset = bytes[0];
    assert_eq!(
        bitset & 0x04,
        0x04,
        "packed-struct bitset should flag payload"
    );

    let (decoded, used) =
        Signature::decode_from_slice(&bytes).expect("decode bare signature payload");
    assert_eq!(used, bytes.len());
    assert_eq!(decoded, sig);
}

#[test]
fn signature_bare_compat_is_len_prefixed_then_payload() {
    // Force non-hybrid path by clearing flags and using core::NoritoSerialize
    let sig = Signature::from_bytes(&[1, 2, 3]);
    let _fg = core::DecodeFlagsGuard::enter(0);
    let mut out = Vec::new();
    norito::NoritoSerialize::serialize(&sig, &mut out).expect("serialize");

    assert!(out.len() > sig.payload().len());

    // The compat bare format still prefixes the payload length as a little-endian u64.
    let mut len_bytes = [0u8; 8];
    len_bytes.copy_from_slice(&out[..8]);
    assert_eq!(u64::from_le_bytes(len_bytes), sig.payload().len() as u64);

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
    let key_pair = iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::Ed25519);
    let msg = ();
    let wrapped: iroha_crypto::SignatureOf<()> =
        iroha_crypto::SignatureOf::new(key_pair.private_key(), &msg);
    let base = Signature::from_bytes(wrapped.payload());
    assert_eq!(
        base.encode(),
        wrapped.encode(),
        "SignatureOf must delegate to Signature encoding"
    );

    // Compat path also the same shape
    let _fg = core::DecodeFlagsGuard::enter(0);
    let mut s1 = Vec::new();
    norito::NoritoSerialize::serialize(&base, &mut s1).expect("serialize sig");
    let mut s2 = Vec::new();
    norito::NoritoSerialize::serialize(&wrapped, &mut s2).expect("serialize sigof");
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
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let sig_of = SignatureOf::new(key_pair.private_key(), &());
    let bytes = norito::to_bytes(&sig_of).expect("encode SignatureOf");

    dump_header("SignatureOf", &bytes);
    let mut offset = Header::SIZE;
    if bytes.len() > offset {
        let bitset = bytes[offset];
        offset += 1;
        eprintln!("SignatureOf field bitset=0b{bitset:08b}");
        if (bitset & 0x01) != 0 {
            let (declared_len, used) = read_varint(&bytes[offset..]);
            offset += used;
            eprintln!(
                "SignatureOf declared inner length via varint={declared_len} (bytes used={used})"
            );
        }
    }

    if bytes.len() > offset {
        let inner = &bytes[offset..];
        eprintln!("SignatureOf inner payload len={}", inner.len());
        dump_header("SignatureOf::inner Signature", inner);
        if inner.len() > Header::SIZE {
            let sig_bitset = inner[Header::SIZE];
            eprintln!("Signature inner field bitset=0b{sig_bitset:08b}");
        }
    }
}
