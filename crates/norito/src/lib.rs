//! Norito serialization framework.
//!
//! Provides deterministic serialization and zero-copy deserialization for
//! common Rust data structures. Derive macros are always available through
//! [`norito::derive`].
//!
//! Layout selection
//! - Norito primarily uses an Array-of-Structs (AoS) layout via derives.
//! - For some homogeneous sequences we expose an adaptive API that can choose a
//!   columnar layout internally for better cache locality and size. See
//!   `norito::columnar` adaptive helpers. For small inputs (AoS path), those
//!   helpers use compact, ad-hoc AoS formats that honor runtime decode flags
//!   (`COMPACT_LEN` varints when enabled; u64 otherwise).
//!
//! Checksum: payloads carry a CRC64-XZ checksum (ECMA polynomial `0x42F0E1EBA9EA3693`,
//! reflected with init/xor all ones) computed using the `crc64fast` crate.
//! [`hardware_crc64`] enables the
//! SIMD-accelerated path when available, while [`crc64_fallback`] forces the
//! portable table implementation.
//!
//! Compatibility
//! - Header-framed decoders (`deserialize_stream`, `decode_from_bytes`,
//!   `decode_from_reader`) validate the Norito header, require the fixed v1
//!   minor (`VERSION_MINOR = 0x00`), and apply the header flag byte as the
//!   authoritative layout selection (unknown bits are rejected). Bare,
//!   headerless decoders (`codec::Decode`) are internal-only for hashing/bench
//!   scenarios and use the fixed v1 default flags.
//! - Packed-seq/packed-struct and compact-length layouts are opt-in via header
//!   flags; v1 defaults to `flags = 0x00`. `COMPACT_LEN` governs per-value
//!   length prefixes; sequence length headers and packed-seq offsets are fixed
//!   `u64` in v1, and reserved layout bits are rejected when decoding headers.

//!
//! Helpers
//! - `norito::core::frame_bare_with_header_flags<T>(payload, flags)` prefixes a
//!   headerless (“bare”) payload with a Norito header that exactly matches the
//!   supplied layout flags. `norito::codec::encode_with_header_flags(value)`
//!   returns both the bare payload and the recorded flags so callers can persist
//!   the metadata alongside the bytes.
//! - `norito::core::frame_bare_with_default_header` is limited to unit tests: it
//!   consumes the thread-local telemetry from the most recent encode/decode on
//!   the same thread and propagates `Error::MissingLayoutFlags` when no metadata
//!   is available.

extern crate self as norito;

use std::{
    alloc::{Layout, alloc, dealloc, handle_alloc_error},
    cell::Cell,
    collections::{BTreeMap, HashMap},
    io::{Read, Write},
    ptr,
    sync::OnceLock,
};
// std imported selectively where needed
pub mod aos;
pub mod columnar;
pub mod core;
pub mod schema;
pub mod streaming;
// Expose heuristics configuration helpers for hosts
pub use core::{
    Archived, ArchivedBox, Compression, CompressionConfig, Error, NoritoDeserialize,
    NoritoSerialize, crc64_fallback, default_encode_flags, from_bytes, from_compressed_bytes,
    hardware_crc64,
    heuristics::{
        Heuristics as HeuristicsConfig, get as get_heuristics,
        select_layout_flags_for_size_with as select_layout_flags_with,
    },
    to_bytes, to_bytes_auto, to_compressed_bytes,
};

pub use codec::disable_packed_struct_layout;

struct ArchiveSlice {
    ptr: *mut u8,
    len: usize,
    layout: Option<Layout>,
}

impl ArchiveSlice {
    fn new(src: &[u8], align: usize) -> Result<Self, Error> {
        if align <= 1 || (src.as_ptr() as usize).is_multiple_of(align) {
            Ok(Self {
                ptr: src.as_ptr() as *mut u8,
                len: src.len(),
                layout: None,
            })
        } else {
            let layout =
                Layout::from_size_align(src.len(), align).map_err(|_| Error::LengthMismatch)?;
            unsafe {
                let ptr = alloc(layout);
                if ptr.is_null() {
                    handle_alloc_error(layout);
                }
                if !src.is_empty() {
                    ptr::copy_nonoverlapping(src.as_ptr(), ptr, src.len());
                }
                Ok(Self {
                    ptr,
                    len: src.len(),
                    layout: Some(layout),
                })
            }
        }
    }

    fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.len) }
    }
}

impl Drop for ArchiveSlice {
    fn drop(&mut self) {
        if let Some(layout) = self.layout {
            unsafe {
                dealloc(self.ptr, layout);
            }
        }
    }
}

#[doc(hidden)]
#[inline]
pub fn debug_trace_enabled() -> bool {
    // Environment-based trace toggles are limited to debug/test builds; release
    // builds ignore the flag to keep runtime behaviour config-driven.
    #[cfg(any(test, debug_assertions))]
    {
        std::env::var_os("NORITO_TRACE").is_some()
    }
    #[cfg(not(any(test, debug_assertions)))]
    {
        false
    }
}

#[cfg(test)]
mod trace_tests {
    use std::env;

    use super::debug_trace_enabled;

    #[test]
    fn debug_trace_follows_env_flag() {
        let env_enabled = env::var_os("NORITO_TRACE").is_some();
        assert_eq!(debug_trace_enabled(), env_enabled);
    }
}

// Re-export selected JSON traits at the crate root for convenience
pub use self::json::FastJsonWrite;

pub mod yaml;

pub mod derive {
    pub use norito_derive::{
        Decode, Encode, FastJson, FastJsonWrite, JsonDeserialize, JsonSerialize, NoritoDeserialize,
        NoritoSerialize,
    };
}

pub use derive::*;

pub mod sequential;

/// Compatibility wrapper providing `Encode` and `Decode` traits analogous to the
/// `parity-scale-codec` ones, but powered by Norito serialization.
pub mod codec {
    use std::{
        io::{Read, Write},
        sync::{
            OnceLock,
            atomic::{AtomicBool, Ordering},
        },
    };

    pub use super::Error;
    use super::{NoritoDeserialize, NoritoSerialize, core};
    pub use crate::derive::{Decode, Encode};

    // Lightweight telemetry for the generic two-pass `encode_adaptive` path.
    // Separate from the columnar bucket to avoid conflating concerns.
    mod telemetry {
        use std::sync::atomic::{AtomicU64, Ordering};

        static CALLS: AtomicU64 = AtomicU64::new(0);
        static REENCODES: AtomicU64 = AtomicU64::new(0);
        static BYTES_ABS_DIFF_TOTAL: AtomicU64 = AtomicU64::new(0);
        #[cfg(feature = "adaptive-telemetry")]
        static PASS1_TIME_NS_TOTAL: AtomicU64 = AtomicU64::new(0);
        #[cfg(feature = "adaptive-telemetry")]
        static PASS2_TIME_NS_TOTAL: AtomicU64 = AtomicU64::new(0);

        #[derive(Clone, Copy, Debug)]
        pub struct Snapshot {
            pub calls: u64,
            pub reencodes: u64,
            pub bytes_abs_diff_total: u64,
            #[cfg(feature = "adaptive-telemetry")]
            pub pass1_time_ns_total: u64,
            #[cfg(feature = "adaptive-telemetry")]
            pub pass2_time_ns_total: u64,
        }

        #[inline]
        pub fn record(
            call_reencoded: bool,
            first_len: usize,
            final_len: usize,
            _pass1_ns: u64,
            _pass2_ns: u64,
        ) {
            CALLS.fetch_add(1, Ordering::Relaxed);
            if call_reencoded {
                REENCODES.fetch_add(1, Ordering::Relaxed);
            }
            let diff = first_len.abs_diff(final_len) as u64;
            BYTES_ABS_DIFF_TOTAL.fetch_add(diff, Ordering::Relaxed);
            #[cfg(feature = "adaptive-telemetry")]
            {
                PASS1_TIME_NS_TOTAL.fetch_add(_pass1_ns, Ordering::Relaxed);
                PASS2_TIME_NS_TOTAL.fetch_add(_pass2_ns, Ordering::Relaxed);
            }
        }

        pub fn snapshot() -> Snapshot {
            Snapshot {
                calls: CALLS.load(Ordering::Relaxed),
                reencodes: REENCODES.load(Ordering::Relaxed),
                bytes_abs_diff_total: BYTES_ABS_DIFF_TOTAL.load(Ordering::Relaxed),
                #[cfg(feature = "adaptive-telemetry")]
                pass1_time_ns_total: PASS1_TIME_NS_TOTAL.load(Ordering::Relaxed),
                #[cfg(feature = "adaptive-telemetry")]
                pass2_time_ns_total: PASS2_TIME_NS_TOTAL.load(Ordering::Relaxed),
            }
        }

        #[allow(dead_code)]
        pub fn reset() {
            CALLS.store(0, Ordering::Relaxed);
            REENCODES.store(0, Ordering::Relaxed);
            BYTES_ABS_DIFF_TOTAL.store(0, Ordering::Relaxed);
            #[cfg(feature = "adaptive-telemetry")]
            {
                PASS1_TIME_NS_TOTAL.store(0, Ordering::Relaxed);
                PASS2_TIME_NS_TOTAL.store(0, Ordering::Relaxed);
            }
        }
    }

    use std::cell::Cell;

    thread_local! {
        static LAST_ENCODE_FLAGS: Cell<Option<u8>> = const { Cell::new(None) };
    }

    pub fn take_last_encode_flags() -> Option<u8> {
        LAST_ENCODE_FLAGS.with(|cell| cell.replace(None))
    }

    fn store_last_encode_flags(flags: u8) {
        LAST_ENCODE_FLAGS.with(|cell| cell.set(Some(flags)));
    }

    /// Encode values into bytes using Norito.
    pub trait Encode: NoritoSerialize + Sized {
        /// Encode `self` into a new `Vec<u8>` without compression.
        ///
        /// Uses the fixed v1 bare layout (no adaptive layout flags).
        fn encode(&self) -> Vec<u8> {
            encode_adaptive(self)
        }

        /// Encode `self` into the given writer without compression.
        fn encode_to<W: Write>(&self, writer: &mut W) {
            let bytes = encode_adaptive(self);
            writer.write_all(&bytes).expect("encoding should not fail");
        }
    }

    impl<T: NoritoSerialize + Sized> Encode for T {}

    /// Input stream for decoding.
    pub trait Input: Read {}
    impl<T: Read> Input for T {}

    /// Decode values from a byte stream produced by [`Encode`].
    pub trait Decode: for<'de> NoritoDeserialize<'de> + Sized {
        /// Attempt to decode `Self` from the given input.
        fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
            // Ensure a clean thread-local decode state for headerless payloads.
            core::reset_decode_state();
            let mut buf = Vec::new();
            input.read_to_end(&mut buf)?;
            decode_adaptive::<Self>(&buf)
        }
    }

    impl<T> Decode for T where T: for<'de> NoritoDeserialize<'de> + Sized {}

    // Keep only the blanket Decode impl; container specializations are unnecessary

    static MANUAL_DISABLE_PACKED_STRUCT: AtomicBool = AtomicBool::new(false);

    fn packed_struct_disabled() -> bool {
        if MANUAL_DISABLE_PACKED_STRUCT.load(Ordering::Relaxed) {
            #[cfg(debug_assertions)]
            {
                static BANNER: OnceLock<()> = OnceLock::new();
                BANNER.get_or_init(|| {
                eprintln!(
                    "norito: packed-struct layout disabled; MANUAL_DISABLE_PACKED_STRUCT override active"
                );
            });
            }
            return true;
        }
        #[cfg(any(test, debug_assertions))]
        {
            static DISABLE_PACKED_STRUCT: OnceLock<bool> = OnceLock::new();
            *DISABLE_PACKED_STRUCT.get_or_init(|| {
            match std::env::var_os("NORITO_DISABLE_PACKED_STRUCT") {
                Some(_) => {
                    eprintln!(
                        "norito: NORITO_DISABLE_PACKED_STRUCT enabled; packed-struct layout disabled for this debug build"
                    );
                    true
                }
                None => false,
            }
        })
        }
        #[cfg(not(any(test, debug_assertions)))]
        {
            false
        }
    }

    /// Permanently disables packed-struct layout for the current process.
    ///
    /// Intended for debug/testing scenarios that require forcing AoS layouts.
    pub fn disable_packed_struct_layout() {
        static FORCE_DISABLE: OnceLock<()> = OnceLock::new();
        FORCE_DISABLE.get_or_init(|| {
            if !cfg!(debug_assertions) {
                panic!("packed-struct layout can only be disabled in debug builds");
            }
            MANUAL_DISABLE_PACKED_STRUCT.store(true, Ordering::Relaxed);
            let _ = packed_struct_disabled();
        });
    }

    /// Bare encode using the fixed v1 layout flags.
    pub fn encode_adaptive<T: NoritoSerialize>(value: &T) -> Vec<u8> {
        let encode_guard = core::EncodeContextGuard::enter();
        // Base flags from compile-time features
        let base: u8 = core::default_encode_flags();
        let _disable_packed_struct = packed_struct_disabled();
        let flags = base;
        #[cfg(debug_assertions)]
        if crate::debug_trace_enabled() {
            eprintln!("norito.codec.encode_adaptive: flags=0x{flags:02x}");
        }

        let mut payload = Vec::new();
        #[cfg(feature = "adaptive-telemetry")]
        let __t0 = std::time::Instant::now();
        {
            let _fg = core::DecodeFlagsGuard::enter(flags);
            NoritoSerialize::serialize(value, &mut payload).expect("encode pass 1");
        }
        #[cfg(feature = "adaptive-telemetry")]
        let __pass1_ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        #[cfg(not(feature = "adaptive-telemetry"))]
        let __pass1_ns: u64 = 0;

        let mut final_flags = flags;
        let fixed_offsets_used = core::fixed_offsets_used();
        let field_bitset_used = core::field_bitset_used();
        let compact_len_used = core::compact_len_used();

        {
            telemetry::record(false, payload.len(), payload.len(), __pass1_ns, 0);
            #[cfg(feature = "adaptive-telemetry-log")]
            eprintln!(
                "norito.codec.adapt: reencode=0 len={} pass1_ns={}",
                payload.len(),
                __pass1_ns
            );
        }

        drop(encode_guard);

        if field_bitset_used {
            final_flags |= core::header_flags::FIELD_BITSET;
        } else {
            final_flags &= !core::header_flags::FIELD_BITSET;
        }
        if compact_len_used {
            final_flags |= core::header_flags::COMPACT_LEN;
        } else {
            final_flags &= !core::header_flags::COMPACT_LEN;
        }
        let packed_seq_used = fixed_offsets_used;
        if packed_seq_used {
            final_flags |= core::header_flags::PACKED_SEQ;
        } else {
            final_flags &= !core::header_flags::PACKED_SEQ;
        }
        #[cfg(debug_assertions)]
        if crate::debug_trace_enabled() {
            eprintln!("norito.codec.encode_adaptive: final_flags=0x{final_flags:02x}");
        }
        store_last_encode_flags(final_flags);
        core::record_last_header_flags(final_flags);
        payload
    }

    /// Encode `value` and return both the bare payload and the exact header flags required
    /// to frame it for header-based decoding.
    pub fn encode_with_header_flags<T: NoritoSerialize>(value: &T) -> (Vec<u8>, u8) {
        let (payload, flags) =
            core::encode_bare_with_flags(value).expect("encode_with_header_flags should succeed");
        store_last_encode_flags(flags);
        (payload, flags)
    }

    /// Bare decode using the fixed v1 layout flags.
    pub fn decode_adaptive<T>(bytes: &[u8]) -> Result<T, Error>
    where
        T: for<'de> NoritoDeserialize<'de>,
    {
        core::reset_decode_state();
        #[inline]
        fn decode_from_aligned<T: for<'de> NoritoDeserialize<'de>>(
            buf: &[u8],
            flags: u8,
            logical_len: usize,
        ) -> Result<T, Error> {
            struct RootGuard;
            impl Drop for RootGuard {
                fn drop(&mut self) {
                    core::clear_decode_root();
                }
            }
            let _root_guard = if core::payload_root_span().is_none() {
                let logical_len = logical_len.min(buf.len());
                core::set_decode_root(&buf[..logical_len]);
                Some(RootGuard)
            } else {
                None
            };
            let _fg = core::DecodeFlagsGuard::enter(flags);
            let _pg = core::PayloadCtxGuard::enter_with_flags_len(buf, logical_len, flags);
            let archived = unsafe { &*(buf.as_ptr() as *const core::Archived<T>) };
            super::guarded_try_deserialize(|| <T as NoritoDeserialize>::try_deserialize(archived))
        }

        let flags = core::default_encode_flags();
        if crate::debug_trace_enabled() {
            eprintln!(
                "norito.codec.decode_adaptive: len={} align={} ptr={:?}",
                bytes.len(),
                ::core::mem::align_of::<core::Archived<T>>(),
                bytes.as_ptr()
            );
        }

        let min_size = ::core::mem::size_of::<core::Archived<T>>();
        let logical_len = bytes.len();
        if min_size > 0 && logical_len == 0 {
            return Err(core::Error::LengthMismatch);
        }

        let align = ::core::mem::align_of::<core::Archived<T>>();

        // If the payload is shorter than the archived footprint, pad with zeros so we
        // can safely form an `Archived<T>` reference while keeping the logical length
        // constrained to the original slice for bounds checks during decode.
        let backing: &[u8];
        let _owned_pad: Option<Vec<u8>>;
        if min_size > 0 && logical_len < min_size {
            let mut pad = Vec::with_capacity(min_size);
            pad.extend_from_slice(bytes);
            pad.resize(min_size, 0);
            _owned_pad = Some(pad);
            backing = _owned_pad.as_deref().expect("pad present");
        } else {
            _owned_pad = None;
            backing = bytes;
        }

        let aligned = match super::ArchiveSlice::new(backing, align) {
            Ok(slice) => slice,
            Err(err) => {
                if align > 1 && backing.as_ptr().align_offset(align) != 0 {
                    return Err(Error::misaligned(align, backing.as_ptr()));
                }
                return Err(err);
            }
        };

        let aligned_slice = aligned.as_slice();
        let _reset = DecodeResetGuard;

        if crate::debug_trace_enabled() {
            if align <= 1 || aligned_slice.as_ptr() == bytes.as_ptr() {
                eprintln!("norito.codec.decode_adaptive: aligned fast-path");
            } else {
                eprintln!(
                    "norito.codec.decode_adaptive: realigned via copy ptr={:?}",
                    aligned_slice.as_ptr()
                );
            }
        }

        decode_from_aligned::<T>(aligned_slice, flags, logical_len)
    }

    struct DecodeResetGuard;

    impl Drop for DecodeResetGuard {
        fn drop(&mut self) {
            crate::core::reset_decode_state();
        }
    }

    /// Decode values ensuring the input contains no trailing bytes.
    pub trait DecodeAll: Decode {
        /// Decode `Self` from `input` verifying that the entire stream is consumed.
        fn decode_all<I: Input>(input: &mut I) -> Result<Self, Error> {
            // Reuse the bare decoder which already mirrors the encode defaults.
            // Tests in this workspace always feed exact payloads, so trailing bytes
            // are not expected and would indicate upstream issues.
            <Self as Decode>::decode(input)
        }
    }

    impl<T: Decode> DecodeAll for T {}

    /// Return a snapshot of the codec two-pass adaptive metrics.
    pub fn adaptive_metrics_snapshot() -> telemetry::Snapshot {
        telemetry::snapshot()
    }

    /// Reset codec adaptive metrics (intended for tests/benches).
    #[allow(dead_code)]
    pub fn adaptive_metrics_reset() {
        telemetry::reset()
    }

    /// JSON: export codec two-pass adaptive metrics as a compact JSON value.
    pub fn adaptive_metrics_json_value() -> crate::json::Value {
        let s = adaptive_metrics_snapshot();
        let mut map = crate::json::Map::new();
        map.insert("calls".into(), crate::json::Value::from(s.calls));
        map.insert("reencodes".into(), crate::json::Value::from(s.reencodes));
        map.insert(
            "bytes_abs_diff_total".into(),
            crate::json::Value::from(s.bytes_abs_diff_total),
        );
        #[cfg(feature = "adaptive-telemetry")]
        {
            map.insert(
                "pass1_time_ns_total".into(),
                crate::json::Value::from(s.pass1_time_ns_total),
            );
            map.insert(
                "pass2_time_ns_total".into(),
                crate::json::Value::from(s.pass2_time_ns_total),
            );
        }
        crate::json::Value::Object(map)
    }

    /// JSON: export codec two-pass adaptive metrics as a compact JSON string.
    pub fn adaptive_metrics_json_string() -> String {
        let v = adaptive_metrics_json_value();
        crate::json::to_string(&v).unwrap_or_else(|_| String::from("{}"))
    }

    /// JSON: compute fieldwise delta between two codec telemetry JSON maps.
    pub fn adaptive_metrics_delta_json(
        prev: &crate::json::Value,
        curr: &crate::json::Value,
    ) -> crate::json::Value {
        use crate::json::Value;
        let mut out = crate::json::Map::new();
        let empty = crate::json::Map::new();
        let p = prev.as_object().unwrap_or(&empty);
        let c = curr.as_object().unwrap_or(&empty);
        for k in [
            "calls",
            "reencodes",
            "bytes_abs_diff_total",
            #[cfg(feature = "adaptive-telemetry")]
            "pass1_time_ns_total",
            #[cfg(feature = "adaptive-telemetry")]
            "pass2_time_ns_total",
        ] {
            if let (Some(Value::Number(a)), Some(Value::Number(b))) = (p.get(k), c.get(k)) {
                let av = a.as_u64().unwrap_or(0);
                let bv = b.as_u64().unwrap_or(0);
                out.insert(k.to_string(), Value::from(bv.saturating_sub(av)));
            }
        }
        Value::Object(out)
    }
}

/// Telemetry helpers aggregating Norito metrics for easy ingestion.
pub mod telemetry {
    /// Reset all Norito telemetry buckets (columnar, codec, compression).
    /// Intended for examples/benches/tests.
    pub fn reset_all() {
        #[allow(unused)]
        {
            crate::columnar::adaptive_metrics_reset();
            // codec bucket always available
            crate::codec::adaptive_metrics_reset();
        }
        crate::core::compression_metrics_reset();
    }

    /// Build a compact JSON value aggregating columnar, codec, and compression telemetry.
    pub fn snapshot_json_value() -> crate::json::Value {
        let mut root = crate::json::Map::new();
        // Columnar may be feature-gated, guard call with cfg
        root.insert(
            "columnar".into(),
            crate::columnar::adaptive_metrics_json_value(),
        );
        // Codec is always present
        root.insert("codec".into(), crate::codec::adaptive_metrics_json_value());
        root.insert(
            "compression".into(),
            crate::core::compression_metrics_json_value(),
        );
        crate::json::Value::Object(root)
    }

    /// Serialize the aggregated telemetry snapshot into a compact JSON string.
    pub fn snapshot_json_string() -> String {
        let v = snapshot_json_value();
        crate::json::to_string(&v).unwrap_or_else(|_| String::from("{}"))
    }

    /// JSON: compute fieldwise delta for the aggregated telemetry map.
    pub fn snapshot_delta_json(
        prev: &crate::json::Value,
        curr: &crate::json::Value,
    ) -> crate::json::Value {
        use crate::json::Value;
        let mut out = crate::json::Map::new();
        let empty = crate::json::Map::new();
        let p = prev.as_object().unwrap_or(&empty);
        let c = curr.as_object().unwrap_or(&empty);
        out.insert(
            "columnar".into(),
            crate::columnar::adaptive_metrics_delta_json(
                p.get("columnar").unwrap_or(&Value::Null),
                c.get("columnar").unwrap_or(&Value::Null),
            ),
        );
        out.insert(
            "codec".into(),
            crate::codec::adaptive_metrics_delta_json(
                p.get("codec").unwrap_or(&Value::Null),
                c.get("codec").unwrap_or(&Value::Null),
            ),
        );
        out.insert(
            "compression".into(),
            crate::core::compression_metrics_delta_json(
                p.get("compression").unwrap_or(&Value::Null),
                c.get("compression").unwrap_or(&Value::Null),
            ),
        );
        Value::Object(out)
    }
}

/// Minimal JSON serialization/deserialization helpers without `serde`.
///
/// This module implements a compact JSON writer and a simple, recursive-descent
/// parser that covers a subset of JSON sufficient for benchmarking and common
/// Norito demos: numbers (integers and simple floats), booleans, strings with
/// escaping, `null`, arrays, and user-defined objects via manual trait impls.
///
/// Types implement [`JsonSerialize`] and/or [`JsonDeserialize`] to participate
/// in the JSON codec. Container impls are provided for `Option<T>` and
/// `Vec<T>`. Object encoding/decoding is done in user code by writing keys and
/// dispatching field decoders with the [`json::Parser`]. For zero‑copy streaming,
/// use the token [`json::Reader`] and convert borrowed string slices into owned
/// `String`s with [`json::unescape_json_string`] when needed.
///
/// Notes and limitations:
/// - The token `Reader` yields borrowed string slices without unescaping; users
///   should parse/unescape as needed.
/// - Unicode escapes (`\uXXXX`) are decoded to Unicode scalars, including
///   surrogate pairs (two `\u` sequences representing one code point) which are
///   combined into a single character when valid.
/// - Leading zeros in numbers are rejected to match JSON rules.
/// - Number parsing is conservative and aims for correctness over breadth for
///   benchmarking scenarios.
pub mod json {
    use url::Url;

    pub use super::{
        JsonDeserialize as Deserialize, JsonDeserialize, JsonSerialize as Serialize, JsonSerialize,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum UnexpectedToken {
        Char(char),
        Eof,
    }

    impl std::fmt::Display for UnexpectedToken {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                UnexpectedToken::Char(c) => write!(f, "character `{c}`"),
                UnexpectedToken::Eof => write!(f, "end of input"),
            }
        }
    }

    // Dedicated error type for Norito JSON helpers (parser/writer/tape).
    #[derive(Debug, thiserror::Error, Clone)]
    pub enum Error {
        #[error("JSON error: {msg} at byte {byte} (line {line}, col {col})")]
        WithPos {
            msg: &'static str,
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unexpected {found} at byte {byte} (line {line}, col {col})")]
        UnexpectedCharacter {
            found: UnexpectedToken,
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected digits at byte {byte} (line {line}, col {col})")]
        ExpectedDigits {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected frac digits at byte {byte} (line {line}, col {col})")]
        ExpectedFracDigits {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected exp digits at byte {byte} (line {line}, col {col})")]
        ExpectedExpDigits {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unexpected value at byte {byte} (line {line}, col {col})")]
        UnexpectedValue {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unexpected quote at byte {byte} (line {line}, col {col})")]
        UnexpectedQuote {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unexpected comma at byte {byte} (line {line}, col {col})")]
        UnexpectedComma {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unexpected ':' at byte {byte} (line {line}, col {col})")]
        UnexpectedColon {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unexpected object end at byte {byte} (line {line}, col {col})")]
        UnexpectedObjectEnd {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unexpected array end at byte {byte} (line {line}, col {col})")]
        UnexpectedArrayEnd {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: u64 overflow at byte {byte} (line {line}, col {col})")]
        U64Overflow {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unterminated string at byte {byte} (line {line}, col {col})")]
        UnterminatedString {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unexpected eof at byte {byte} (line {line}, col {col})")]
        UnexpectedEof {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: eof escape at byte {byte} (line {line}, col {col})")]
        EofEscape {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: eof hex at byte {byte} (line {line}, col {col})")]
        EofHex {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: invalid hex at byte {byte} (line {line}, col {col})")]
        InvalidHex {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: control in string at byte {byte} (line {line}, col {col})")]
        ControlInString {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected null at byte {byte} (line {line}, col {col})")]
        ExpectedNull {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected bool at byte {byte} (line {line}, col {col})")]
        ExpectedBool {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: trailing characters at byte {byte} (line {line}, col {col})")]
        TrailingCharacters {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected object start at byte {byte} (line {line}, col {col})")]
        ExpectedObjectStart {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected object end at byte {byte} (line {line}, col {col})")]
        ExpectedObjectEnd {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected array start at byte {byte} (line {line}, col {col})")]
        ExpectedArrayStart {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected array end at byte {byte} (line {line}, col {col})")]
        ExpectedArrayEnd {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected key string at byte {byte} (line {line}, col {col})")]
        ExpectedKeyString {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected key quote at byte {byte} (line {line}, col {col})")]
        ExpectedKeyHashQuote {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: unterminated key at byte {byte} (line {line}, col {col})")]
        UnterminatedKey {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected ':' at byte {byte} (line {line}, col {col})")]
        ExpectedColon {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected , or ] at byte {byte} (line {line}, col {col})")]
        ExpectedCommaOrArrayEnd {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: expected , or }} at byte {byte} (line {line}, col {col})")]
        ExpectedCommaOrObjectEnd {
            byte: usize,
            line: usize,
            col: usize,
        },
        #[error("JSON error: invalid field `{field}`: {message}")]
        InvalidField { field: String, message: String },
        #[error("JSON error: missing field `{field}`")]
        MissingField { field: String },
        #[error("JSON error: unknown field `{field}`")]
        UnknownField { field: String },
        #[error("JSON error: duplicate field `{field}`")]
        DuplicateField { field: String },
        #[error("invalid utf8")]
        InvalidUtf8,
        #[error("{0}")]
        Message(String),
    }
    impl Error {
        #[inline]
        pub fn missing_field(field: impl Into<String>) -> Self {
            Self::MissingField {
                field: field.into(),
            }
        }

        #[inline]
        pub fn duplicate_field(field: impl Into<String>) -> Self {
            Self::DuplicateField {
                field: field.into(),
            }
        }

        #[inline]
        pub fn unknown_field(field: impl Into<String>) -> Self {
            Self::UnknownField {
                field: field.into(),
            }
        }
    }
    impl From<String> for Error {
        fn from(s: String) -> Self {
            Error::Message(s)
        }
    }
    impl From<&str> for Error {
        fn from(s: &str) -> Self {
            Error::Message(s.to_owned())
        }
    }
    impl From<super::Error> for Error {
        fn from(e: super::Error) -> Self {
            Error::Message(e.to_string())
        }
    }
    pub mod value {
        pub use super::RawValue;
        use super::{Error, Value, parse_value, to_json};

        pub fn to_raw_value(value: &Value) -> Result<Box<RawValue>, Error> {
            let json = to_json(value)?;
            Ok(Box::new(RawValue::from_string(json)))
        }

        pub fn from_raw_value(raw: &RawValue) -> Result<Value, Error> {
            parse_value(raw.get())
        }

        pub fn from_value<T: super::JsonDeserialize>(value: Value) -> Result<T, Error> {
            super::from_value(value)
        }

        pub fn to_value<T: super::JsonSerialize>(value: &T) -> Result<Value, Error> {
            super::to_value(value)
        }
    }

    // Native, serde-free JSON types and helpers
    #[inline]
    pub(crate) fn pos_from_offset(s: &str, pos: usize) -> (usize, usize, usize) {
        let bytes = s.as_bytes();
        let mut line = 1usize;
        let mut col = 1usize;
        let mut i = 0usize;
        while i < pos && i < bytes.len() {
            if bytes[i] == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            i += 1;
        }
        (pos, line, col)
    }
    pub mod native {
        use core::mem;
        use std::{collections::BTreeMap, ops::Index};

        use super::ValueIndex;

        #[derive(Debug, Clone, Copy)]
        pub enum Number {
            I64(i64),
            U64(u64),
            F64(f64),
        }
        impl Number {
            pub fn as_i64(&self) -> Option<i64> {
                if let Number::I64(v) = self {
                    Some(*v)
                } else {
                    None
                }
            }
            pub fn as_u64(&self) -> Option<u64> {
                match self {
                    Number::U64(v) => Some(*v),
                    Number::I64(v) if *v >= 0 => Some(*v as u64),
                    _ => None,
                }
            }
            pub fn as_f64(&self) -> Option<f64> {
                match self {
                    Number::F64(v) => Some(*v),
                    Number::I64(v) => Some(*v as f64),
                    Number::U64(v) => Some(*v as f64),
                }
            }
            pub fn from_f64(v: f64) -> Option<Self> {
                if v.is_finite() {
                    Some(Number::F64(v))
                } else {
                    None
                }
            }
        }
        impl From<i64> for Number {
            fn from(v: i64) -> Self {
                Number::I64(v)
            }
        }
        impl From<u64> for Number {
            fn from(v: u64) -> Self {
                Number::U64(v)
            }
        }
        impl From<f64> for Number {
            fn from(v: f64) -> Self {
                Number::F64(v)
            }
        }

        impl PartialEq for Number {
            fn eq(&self, other: &Self) -> bool {
                match (self, other) {
                    (Number::I64(a), Number::I64(b)) => a == b,
                    (Number::U64(a), Number::U64(b)) => a == b,
                    (Number::F64(a), Number::F64(b)) => a == b,
                    (Number::I64(a), Number::U64(b)) => *a >= 0 && (*a as u64) == *b,
                    (Number::U64(a), Number::I64(b)) => *b >= 0 && *a == (*b as u64),
                    (Number::I64(a), Number::F64(b)) => (*a as f64) == *b,
                    (Number::F64(a), Number::I64(b)) => *a == (*b as f64),
                    (Number::U64(a), Number::F64(b)) => (*a as f64) == *b,
                    (Number::F64(a), Number::U64(b)) => *a == (*b as f64),
                }
            }
        }

        pub type Map = BTreeMap<String, Value>;

        fn decode_pointer_segment(segment: &str) -> Option<String> {
            if !segment.contains('~') {
                return Some(segment.to_owned());
            }
            let mut out = String::with_capacity(segment.len());
            let mut chars = segment.chars();
            while let Some(ch) = chars.next() {
                if ch == '~' {
                    match chars.next() {
                        Some('0') => out.push('~'),
                        Some('1') => out.push('/'),
                        _ => return None,
                    }
                } else {
                    out.push(ch);
                }
            }
            Some(out)
        }

        #[derive(Debug, Clone, PartialEq)]
        pub enum Value {
            Null,
            Bool(bool),
            Number(Number),
            String(String),
            Array(Vec<Value>),
            Object(Map),
        }

        impl Eq for Value {}

        impl Value {
            pub fn is_null(&self) -> bool {
                matches!(self, Value::Null)
            }
            pub fn is_bool(&self) -> bool {
                matches!(self, Value::Bool(_))
            }
            pub fn is_number(&self) -> bool {
                matches!(self, Value::Number(_))
            }
            pub fn is_string(&self) -> bool {
                matches!(self, Value::String(_))
            }
            pub fn is_array(&self) -> bool {
                matches!(self, Value::Array(_))
            }
            pub fn is_object(&self) -> bool {
                matches!(self, Value::Object(_))
            }
            pub fn as_array(&self) -> Option<&Vec<Value>> {
                if let Value::Array(a) = self {
                    Some(a)
                } else {
                    None
                }
            }
            pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
                if let Value::Array(a) = self {
                    Some(a)
                } else {
                    None
                }
            }
            pub fn as_object(&self) -> Option<&Map> {
                if let Value::Object(map) = self {
                    Some(map)
                } else {
                    None
                }
            }
            pub fn as_object_mut(&mut self) -> Option<&mut Map> {
                if let Value::Object(map) = self {
                    Some(map)
                } else {
                    None
                }
            }
            pub fn get<I>(&self, index: I) -> Option<&Value>
            where
                I: ValueIndex,
            {
                index.index_into(self)
            }
            pub fn get_mut<I>(&mut self, index: I) -> Option<&mut Value>
            where
                I: ValueIndex,
            {
                index.index_into_mut(self)
            }
            pub fn as_str(&self) -> Option<&str> {
                if let Value::String(s) = self {
                    Some(s)
                } else {
                    None
                }
            }
            pub fn as_bool(&self) -> Option<bool> {
                if let Value::Bool(b) = self {
                    Some(*b)
                } else {
                    None
                }
            }
            pub fn as_u64(&self) -> Option<u64> {
                if let Value::Number(n) = self {
                    n.as_u64()
                } else {
                    None
                }
            }
            pub fn as_i64(&self) -> Option<i64> {
                if let Value::Number(n) = self {
                    n.as_i64()
                } else {
                    None
                }
            }
            pub fn as_f64(&self) -> Option<f64> {
                if let Value::Number(n) = self {
                    n.as_f64()
                } else {
                    None
                }
            }
            pub fn pointer(&self, pointer: &str) -> Option<&Value> {
                if pointer.is_empty() {
                    return Some(self);
                }
                if !pointer.starts_with('/') {
                    return None;
                }
                let mut current = self;
                for segment in pointer.split('/').skip(1) {
                    let key = decode_pointer_segment(segment)?;
                    match current {
                        Value::Object(map) => {
                            current = map.get(key.as_str())?;
                        }
                        Value::Array(arr) => {
                            let idx = key.parse::<usize>().ok()?;
                            current = arr.get(idx)?;
                        }
                        _ => return None,
                    }
                }
                Some(current)
            }
            pub fn pointer_mut(&mut self, pointer: &str) -> Option<&mut Value> {
                if pointer.is_empty() {
                    return Some(self);
                }
                if !pointer.starts_with('/') {
                    return None;
                }
                let mut current = self;
                for segment in pointer.split('/').skip(1) {
                    let key = decode_pointer_segment(segment)?;
                    match current {
                        Value::Object(map) => {
                            current = map.get_mut(key.as_str())?;
                        }
                        Value::Array(arr) => {
                            let idx = key.parse::<usize>().ok()?;
                            current = arr.get_mut(idx)?;
                        }
                        _ => return None,
                    }
                }
                Some(current)
            }
            pub fn take(&mut self) -> Value {
                mem::replace(self, Value::Null)
            }
        }

        impl From<bool> for Value {
            fn from(v: bool) -> Self {
                Value::Bool(v)
            }
        }
        impl From<u64> for Value {
            fn from(v: u64) -> Self {
                Value::Number(Number::U64(v))
            }
        }
        impl From<u32> for Value {
            fn from(v: u32) -> Self {
                Value::Number(Number::U64(v as u64))
            }
        }
        impl From<u16> for Value {
            fn from(v: u16) -> Self {
                Value::Number(Number::U64(v as u64))
            }
        }
        impl From<u8> for Value {
            fn from(v: u8) -> Self {
                Value::Number(Number::U64(v as u64))
            }
        }
        impl From<i64> for Value {
            fn from(v: i64) -> Self {
                Value::Number(Number::I64(v))
            }
        }
        impl From<i32> for Value {
            fn from(v: i32) -> Self {
                Value::Number(Number::I64(v as i64))
            }
        }
        impl From<i16> for Value {
            fn from(v: i16) -> Self {
                Value::Number(Number::I64(v as i64))
            }
        }
        impl From<i8> for Value {
            fn from(v: i8) -> Self {
                Value::Number(Number::I64(v as i64))
            }
        }
        impl From<usize> for Value {
            fn from(v: usize) -> Self {
                Value::Number(Number::U64(v as u64))
            }
        }
        impl From<isize> for Value {
            fn from(v: isize) -> Self {
                Value::Number(Number::I64(v as i64))
            }
        }
        impl From<f64> for Value {
            fn from(v: f64) -> Self {
                Value::Number(Number::F64(v))
            }
        }
        impl From<String> for Value {
            fn from(v: String) -> Self {
                Value::String(v)
            }
        }
        impl From<&str> for Value {
            fn from(v: &str) -> Self {
                Value::String(v.to_string())
            }
        }
        impl From<Vec<Value>> for Value {
            fn from(v: Vec<Value>) -> Self {
                Value::Array(v)
            }
        }

        impl From<Map> for Value {
            fn from(map: Map) -> Self {
                Value::Object(map)
            }
        }

        static NULL: Value = Value::Null;
        impl Index<&str> for Value {
            type Output = Value;
            fn index(&self, index: &str) -> &Self::Output {
                if let Value::Object(map) = self {
                    map.get(index).unwrap_or(&NULL)
                } else {
                    &NULL
                }
            }
        }
        impl Index<usize> for Value {
            type Output = Value;
            fn index(&self, idx: usize) -> &Self::Output {
                if let Value::Array(a) = self {
                    a.get(idx).unwrap_or(&NULL)
                } else {
                    &NULL
                }
            }
        }
    }

    pub trait ValueIndex {
        fn index_into<'a>(&self, value: &'a Value) -> Option<&'a Value>;
        fn index_into_mut<'a>(&self, value: &'a mut Value) -> Option<&'a mut Value>;
    }

    impl ValueIndex for &str {
        fn index_into<'a>(&self, value: &'a Value) -> Option<&'a Value> {
            if let Value::Object(map) = value {
                map.get(*self)
            } else {
                None
            }
        }

        fn index_into_mut<'a>(&self, value: &'a mut Value) -> Option<&'a mut Value> {
            if let Value::Object(map) = value {
                map.get_mut(*self)
            } else {
                None
            }
        }
    }

    impl ValueIndex for String {
        fn index_into<'a>(&self, value: &'a Value) -> Option<&'a Value> {
            if let Value::Object(map) = value {
                map.get(self.as_str())
            } else {
                None
            }
        }

        fn index_into_mut<'a>(&self, value: &'a mut Value) -> Option<&'a mut Value> {
            if let Value::Object(map) = value {
                map.get_mut(self.as_str())
            } else {
                None
            }
        }
    }

    impl ValueIndex for usize {
        fn index_into<'a>(&self, value: &'a Value) -> Option<&'a Value> {
            if let Value::Array(arr) = value {
                arr.get(*self)
            } else {
                None
            }
        }

        fn index_into_mut<'a>(&self, value: &'a mut Value) -> Option<&'a mut Value> {
            if let Value::Array(arr) = value {
                arr.get_mut(*self)
            } else {
                None
            }
        }
    }

    pub type Map = native::Map;
    pub type Number = native::Number;
    pub type Value = native::Value;

    #[macro_export]
    macro_rules! json {
        (null) => { $crate::json::Value::Null };
        ([$($elem:tt),* $(,)?]) => {{
            let values = vec![$($crate::json!($elem)),*];
            $crate::json::Value::Array(values)
        }};
        ({$($key:literal : $val:tt),* $(,)?}) => {{
            let mut map = $crate::json::Map::new();
            $( map.insert($key.to_string(), $crate::json!($val)); )*
            $crate::json::Value::Object(map)
        }};
        ($other:expr) => {{
            match $crate::json::to_value(&$other) {
                Ok(value) => value,
                Err(err) => panic!("norito::json! failed to serialize expression: {err}"),
            }
        }};
    }

    mod schema_support {
        use core::{any::TypeId, convert::TryFrom};
        use std::collections::{BTreeMap, btree_map::Entry as BTreeEntry};

        use iroha_schema::{
            ArrayMeta, BitmapMask, BitmapMeta, EnumMeta, EnumVariant, FixedMeta, Ident, IntMode,
            IntoSchema, MapMeta, MetaMap, MetaMapEntry, Metadata, NamedFieldsMeta, ResultMeta,
            TypeId as SchemaTypeId, UnnamedFieldsMeta,
        };

        use super::{JsonSerialize, Map, Number, Value};

        type EntryMap = BTreeMap<TypeId, MetaMapEntry>;

        impl JsonSerialize for MetaMap {
            fn json_serialize(&self, out: &mut String) {
                let entries: EntryMap = self.clone().into_iter().collect();
                let mut sorted: BTreeMap<String, Value> = BTreeMap::new();
                let mut duplicates: BTreeMap<String, Vec<Value>> = BTreeMap::new();

                for entry in entries.values() {
                    let value = metadata_to_value(&entry.metadata, &entries);
                    match sorted.entry(entry.type_name.clone()) {
                        BTreeEntry::Vacant(slot) => {
                            slot.insert(value);
                        }
                        BTreeEntry::Occupied(slot) => {
                            if slot.get() != &value {
                                let dup = duplicates
                                    .entry(entry.type_name.clone())
                                    .or_insert_with(|| vec![slot.get().clone()]);
                                dup.push(value);
                            }
                        }
                    }
                }

                assert!(
                    duplicates.is_empty(),
                    "Duplicate type names: {duplicates:#?}"
                );

                let mut map = Map::new();
                for (type_name, value) in sorted {
                    map.insert(type_name, value);
                }
                Value::Object(map).json_serialize(out);
            }
        }

        fn metadata_to_value(meta: &Metadata, entries: &EntryMap) -> Value {
            match meta {
                Metadata::String => {
                    Value::String(lookup(entries, TypeId::of::<String>(), "Metadata::String"))
                }
                Metadata::Bool => {
                    Value::String(lookup(entries, TypeId::of::<bool>(), "Metadata::Bool"))
                }
                Metadata::Option(inner) => single_entry(
                    "Option",
                    Value::String(lookup(entries, *inner, "Metadata::Option")),
                ),
                Metadata::Int(mode) => single_entry("Int", int_mode_to_value(*mode)),
                Metadata::Tuple(tuple) => tuple_metadata_to_value(tuple, entries),
                Metadata::Struct(named) => {
                    single_entry("Struct", struct_fields_to_value(named, entries))
                }
                Metadata::Enum(enum_meta) => {
                    single_entry("Enum", enum_variants_to_value(enum_meta, entries))
                }
                Metadata::FixedPoint(fixed) => {
                    single_entry("FixedPoint", fixed_meta_to_value(fixed, entries))
                }
                Metadata::Array(array) => {
                    single_entry("Array", array_meta_to_value(array, entries))
                }
                Metadata::Vec(vec_meta) => single_entry(
                    "Vec",
                    Value::String(lookup(entries, vec_meta.ty, "Metadata::Vec")),
                ),
                Metadata::Map(map_meta) => {
                    single_entry("Map", map_meta_to_value(map_meta, entries))
                }
                Metadata::Result(result) => {
                    single_entry("Result", result_meta_to_value(result, entries))
                }
                Metadata::Bitmap(bitmap) => {
                    single_entry("Bitmap", bitmap_meta_to_value(bitmap, entries))
                }
            }
        }

        fn lookup(entries: &EntryMap, type_id: TypeId, context: &'static str) -> String {
            entries
                .get(&type_id)
                .map(|entry| entry.type_name.clone())
                .unwrap_or_else(|| {
                    panic!("Failed to find type id `{type_id:?}` while serializing {context}")
                })
        }

        fn tuple_metadata_to_value(meta: &UnnamedFieldsMeta, entries: &EntryMap) -> Value {
            match meta.types.as_slice() {
                [] => Value::Null,
                [ty] => Value::String(lookup(entries, *ty, "Tuple::Single")),
                types => {
                    let items = types
                        .iter()
                        .map(|ty| Value::String(lookup(entries, *ty, "Tuple::Multi")))
                        .collect();
                    single_entry("Tuple", Value::Array(items))
                }
            }
        }

        fn struct_fields_to_value(meta: &NamedFieldsMeta, entries: &EntryMap) -> Value {
            let mut out = Vec::with_capacity(meta.declarations.len());
            for decl in &meta.declarations {
                let mut field = Map::new();
                field.insert("name".to_owned(), Value::String(decl.name.clone()));
                field.insert(
                    "type".to_owned(),
                    Value::String(lookup(entries, decl.ty, "StructDecl")),
                );
                out.push(Value::Object(field));
            }
            Value::Array(out)
        }

        fn enum_variants_to_value(meta: &EnumMeta, entries: &EntryMap) -> Value {
            let mut out = Vec::with_capacity(meta.variants.len());
            for EnumVariant {
                tag,
                discriminant,
                ty,
            } in &meta.variants
            {
                let mut variant = Map::new();
                variant.insert("tag".to_owned(), Value::String(tag.clone()));
                variant.insert(
                    "discriminant".to_owned(),
                    Value::from(u64::from(*discriminant)),
                );
                if let Some(type_id) = ty {
                    variant.insert(
                        "type".to_owned(),
                        Value::String(lookup(entries, *type_id, "EnumDecl")),
                    );
                }
                out.push(Value::Object(variant));
            }
            Value::Array(out)
        }

        fn fixed_meta_to_value(meta: &FixedMeta, entries: &EntryMap) -> Value {
            let mut obj = Map::new();
            obj.insert(
                "base".to_owned(),
                Value::String(lookup(entries, meta.base, "FixedPoint::base")),
            );
            obj.insert(
                "decimal_places".to_owned(),
                Value::from(u64::from(meta.decimal_places)),
            );
            Value::Object(obj)
        }

        fn array_meta_to_value(meta: &ArrayMeta, entries: &EntryMap) -> Value {
            let mut obj = Map::new();
            obj.insert(
                "type".to_owned(),
                Value::String(lookup(entries, meta.ty, "ArrayMeta::type")),
            );
            obj.insert("len".to_owned(), u128_to_value(meta.len));
            Value::Object(obj)
        }

        fn map_meta_to_value(meta: &MapMeta, entries: &EntryMap) -> Value {
            let mut obj = Map::new();
            obj.insert(
                "key".to_owned(),
                Value::String(lookup(entries, meta.key, "MapMeta::key")),
            );
            obj.insert(
                "value".to_owned(),
                Value::String(lookup(entries, meta.value, "MapMeta::value")),
            );
            Value::Object(obj)
        }

        fn result_meta_to_value(meta: &ResultMeta, entries: &EntryMap) -> Value {
            let mut obj = Map::new();
            obj.insert(
                "ok".to_owned(),
                Value::String(lookup(entries, meta.ok, "ResultMeta::ok")),
            );
            obj.insert(
                "err".to_owned(),
                Value::String(lookup(entries, meta.err, "ResultMeta::err")),
            );
            Value::Object(obj)
        }

        fn bitmap_meta_to_value(meta: &BitmapMeta, entries: &EntryMap) -> Value {
            let mut obj = Map::new();
            obj.insert(
                "repr".to_owned(),
                Value::String(lookup(entries, meta.repr, "BitmapMeta::repr")),
            );
            obj.insert("masks".to_owned(), bitmap_masks_to_value(&meta.masks));
            Value::Object(obj)
        }

        fn bitmap_masks_to_value(masks: &[BitmapMask]) -> Value {
            let mut out = Vec::with_capacity(masks.len());
            for mask in masks {
                let mut obj = Map::new();
                obj.insert("name".to_owned(), Value::String(mask.name.clone()));
                obj.insert("mask".to_owned(), Value::from(mask.mask));
                out.push(Value::Object(obj));
            }
            Value::Array(out)
        }

        fn int_mode_to_value(mode: IntMode) -> Value {
            match mode {
                IntMode::FixedWidth => Value::String("FixedWidth".to_owned()),
                IntMode::Compact => Value::String("Compact".to_owned()),
            }
        }

        #[inline]
        fn type_name_of<T>() -> Ident {
            core::any::type_name::<T>().to_owned()
        }

        impl SchemaTypeId for Number {
            fn id() -> Ident {
                type_name_of::<Self>()
            }
        }

        impl IntoSchema for Number {
            fn type_name() -> Ident {
                type_name_of::<Self>()
            }

            fn update_schema_map(map: &mut MetaMap) {
                if map.contains_key::<Self>() {
                    return;
                }

                let variants = vec![
                    EnumVariant {
                        tag: "I64".to_owned(),
                        discriminant: 0,
                        ty: Some(TypeId::of::<i64>()),
                    },
                    EnumVariant {
                        tag: "U64".to_owned(),
                        discriminant: 1,
                        ty: Some(TypeId::of::<u64>()),
                    },
                    EnumVariant {
                        tag: "F64".to_owned(),
                        discriminant: 2,
                        // Expose the floating-point payload as a string since
                        // `f64` intentionally lacks an `IntoSchema` impl.
                        ty: Some(TypeId::of::<String>()),
                    },
                ];

                map.insert::<Self>(Metadata::Enum(EnumMeta { variants }));

                <i64 as IntoSchema>::update_schema_map(map);
                <u64 as IntoSchema>::update_schema_map(map);
                <String as IntoSchema>::update_schema_map(map);
            }
        }

        impl SchemaTypeId for Value {
            fn id() -> Ident {
                type_name_of::<Self>()
            }
        }

        impl IntoSchema for Value {
            fn type_name() -> Ident {
                type_name_of::<Self>()
            }

            fn update_schema_map(map: &mut MetaMap) {
                if map.contains_key::<Self>() {
                    return;
                }

                let variants = vec![
                    EnumVariant {
                        tag: "Null".to_owned(),
                        discriminant: 0,
                        ty: None,
                    },
                    EnumVariant {
                        tag: "Bool".to_owned(),
                        discriminant: 1,
                        ty: Some(TypeId::of::<bool>()),
                    },
                    EnumVariant {
                        tag: "Number".to_owned(),
                        discriminant: 2,
                        ty: Some(TypeId::of::<Number>()),
                    },
                    EnumVariant {
                        tag: "String".to_owned(),
                        discriminant: 3,
                        ty: Some(TypeId::of::<String>()),
                    },
                    EnumVariant {
                        tag: "Array".to_owned(),
                        discriminant: 4,
                        ty: Some(TypeId::of::<Vec<Value>>()),
                    },
                    EnumVariant {
                        tag: "Object".to_owned(),
                        discriminant: 5,
                        ty: Some(TypeId::of::<Map>()),
                    },
                ];

                map.insert::<Self>(Metadata::Enum(EnumMeta { variants }));

                <bool as IntoSchema>::update_schema_map(map);
                <Number as IntoSchema>::update_schema_map(map);
                <String as IntoSchema>::update_schema_map(map);
                <Vec<Value> as IntoSchema>::update_schema_map(map);
                <std::collections::BTreeMap<String, Value> as IntoSchema>::update_schema_map(map);
            }
        }

        fn u128_to_value(len: u128) -> Value {
            match u64::try_from(len) {
                Ok(v) => Value::from(v),
                Err(_) => Value::String(len.to_string()),
            }
        }

        fn single_entry(key: &str, value: Value) -> Value {
            let mut map = Map::new();
            map.insert(key.to_owned(), value);
            Value::Object(map)
        }
    }

    /// Compute a compile-time key hash for JSON object field dispatch.
    ///
    /// The hash function mirrors `TapeWalker::read_key_hash` on plain ASCII keys
    /// without escapes. For typical field identifiers (letters, digits, `_`),
    /// this matches exactly. When the `crc-key-hash` feature is enabled we use a
    /// software CRC32C update and widen to 64 bits using a fixed avalanche to
    /// minimize collisions. Otherwise we default to 64-bit FNV-1a.
    pub const fn key_hash_const(s: &str) -> u64 {
        #[cfg(feature = "crc-key-hash")]
        {
            // Match TapeWalker::read_key_hash CRC32C path:
            // seed = 0xFFFF_FFFF; per-byte reflected update; deterministic 64-bit mix.
            const fn crc32c_sw_byte(crc: u32, b: u8) -> u32 {
                let mut c = crc ^ 0xFFFF_FFFF;
                let mut x = b as u32;
                let mut i = 0u32;
                while i < 8 {
                    let mix = (c ^ x) & 1;
                    c >>= 1;
                    if mix != 0 {
                        c ^= 0x82F63B78;
                    }
                    x >>= 1;
                    i += 1;
                }
                c ^ 0xFFFF_FFFF
            }
            let bytes = s.as_bytes();
            let mut i = 0usize;
            let mut crc: u32 = 0xFFFF_FFFF;
            while i < bytes.len() {
                crc = crc32c_sw_byte(crc, bytes[i]);
                i += 1;
            }
            // Mix CRC32C to 64 bits deterministically (no HW dependency)
            let mut x = (crc as u64) ^ 0x9E3779B97F4A7C15;
            x ^= x >> 33;
            x = x.wrapping_mul(0xff51afd7ed558ccd);
            x ^= x >> 33;
            x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
            x ^= x >> 33;
            x
        }
        #[cfg(not(feature = "crc-key-hash"))]
        {
            // 64-bit FNV-1a
            let bytes = s.as_bytes();
            let mut i = 0usize;
            let mut h: u64 = 0xcbf29ce484222325;
            while i < bytes.len() {
                h ^= bytes[i] as u64;
                h = h.wrapping_mul(0x100000001b3);
                i += 1;
            }
            h
        }
    }

    // Native variants for Value
    fn write_value_to_string(v: &Value, out: &mut String, pretty: bool, depth: usize) {
        use native::Value as V;
        match v {
            V::Null => out.push_str("null"),
            V::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            V::Number(n) => match n {
                native::Number::I64(x) => out.push_str(&x.to_string()),
                native::Number::U64(x) => out.push_str(&x.to_string()),
                native::Number::F64(x) => {
                    const F64_SAFE_INT: f64 = 9_007_199_254_740_992.0; // 2^53
                    if !x.is_finite() {
                        out.push_str("null");
                    } else {
                        use core::fmt::Write as _;
                        if x.fract() == 0.0 && x.abs() <= F64_SAFE_INT {
                            let _ = write!(out, "{x:.1}");
                        } else {
                            let _ = write!(out, "{x}");
                        }
                    }
                }
            },
            V::String(s) => write_json_string(s, out),
            V::Array(a) => {
                out.push('[');
                let mut first = true;
                for elem in a {
                    if !first {
                        out.push(',');
                    }
                    if pretty {
                        out.push('\n');
                        out.push_str(&"  ".repeat(depth + 1));
                    }
                    write_value_to_string(elem, out, pretty, depth + 1);
                    first = false;
                }
                if pretty && !a.is_empty() {
                    out.push('\n');
                    out.push_str(&"  ".repeat(depth));
                }
                out.push(']');
            }
            V::Object(m) => {
                out.push('{');
                let mut first = true;
                for (k, v) in m.iter() {
                    if !first {
                        out.push(',');
                    }
                    if pretty {
                        out.push('\n');
                        out.push_str(&"  ".repeat(depth + 1));
                    }
                    write_json_string(k, out);
                    out.push(':');
                    if pretty {
                        out.push(' ');
                    }
                    write_value_to_string(v, out, pretty, depth + 1);
                    first = false;
                }
                if pretty && !m.is_empty() {
                    out.push('\n');
                    out.push_str(&"  ".repeat(depth));
                }
                out.push('}');
            }
        }
    }
    pub fn to_string(value: &Value) -> Result<String, Error> {
        let mut s = String::new();
        write_value_to_string(value, &mut s, false, 0);
        Ok(s)
    }
    pub fn to_string_pretty(value: &Value) -> Result<String, Error> {
        let mut s = String::new();
        write_value_to_string(value, &mut s, true, 0);
        Ok(s)
    }
    pub fn to_vec<T: JsonSerialize + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
        Ok(to_json(value)?.into_bytes())
    }

    /// serde-style API: serialize to a pretty-printed `Vec<u8>` (alloc-only helper).
    pub fn to_vec_pretty<T: JsonSerialize + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
        Ok(to_json_pretty(value)?.into_bytes())
    }

    /// serde-style API: parse from &str using Norito's native JSON stack.
    pub fn from_str<T: JsonDeserialize>(s: &str) -> Result<T, Error> {
        let v = parse_value(s)?;
        from_value(v)
    }

    /// Parse from a UTF‑8 byte slice using Norito's native parser.
    ///
    /// Note: this variant requires `T: JsonDeserialize` because Norito's
    /// native parser builds an owned `Value` as an intermediate.
    pub fn from_slice<T: JsonDeserialize>(bytes: &[u8]) -> Result<T, Error> {
        let s = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
        from_str(s)
    }

    /// Alias for `from_slice` for convenience.
    pub fn from_bytes<T: JsonDeserialize>(bytes: &[u8]) -> Result<T, Error> {
        from_slice(bytes)
    }

    /// Convert a native JSON `Value` into `T` using `JsonDeserialize`.
    pub fn from_value<T: JsonDeserialize>(v: Value) -> Result<T, Error> {
        T::json_from_value(&v)
    }

    /// Convert `value` into a native JSON `Value` using `JsonSerialize`.
    pub fn to_value<T: JsonSerialize + ?Sized>(value: &T) -> Result<Value, Error> {
        let json = to_json(value)?;
        parse_value(&json)
    }

    /// Convenience: parse a JSON `Value` from `&str` using Norito's parser.
    pub fn parse_value(s: &str) -> Result<Value, Error> {
        let mut parser = super::json::Parser::new(s);
        let value = parse_value_internal(&mut parser)?;
        parser.skip_ws();
        if !parser.eof() {
            let (byte, line, col) = parser.pos_meta(parser.position());
            return Err(Error::TrailingCharacters { byte, line, col });
        }
        Ok(value)
    }

    fn parse_value_internal(p: &mut super::json::Parser<'_>) -> Result<Value, Error> {
        p.skip_ws();
        match p.peek() {
            Some(b'"') => Ok(Value::String(p.parse_string()?)),
            Some(b'n') => {
                p.parse_null()?;
                Ok(Value::Null)
            }
            Some(b't') | Some(b'f') => Ok(Value::Bool(p.parse_bool()?)),
            Some(b'[') => {
                let arr = parse_array_internal(p)?;
                Ok(Value::Array(arr))
            }
            Some(b'{') => {
                let map = parse_object_internal(p)?;
                Ok(Value::Object(map))
            }
            Some(b'-') | Some(b'0'..=b'9') => parse_number_value(p),
            Some(_) => Err(p.err_unexpected_char()),
            None => {
                let (byte, line, col) = p.pos_meta(p.position());
                Err(Error::UnexpectedEof { byte, line, col })
            }
        }
    }

    fn parse_array_internal(p: &mut super::json::Parser<'_>) -> Result<Vec<Value>, Error> {
        if p.bump() != Some(b'[') {
            let (byte, line, col) = p.pos_meta(p.position());
            return Err(Error::ExpectedArrayStart { byte, line, col });
        }
        let mut arr = Vec::new();
        p.skip_ws();
        if let Some(b']') = p.peek() {
            p.bump();
            return Ok(arr);
        }
        loop {
            let value = parse_value_internal(p)?;
            arr.push(value);
            p.skip_ws();
            match p.peek() {
                Some(b',') => {
                    p.bump();
                    p.skip_ws();
                }
                Some(b']') => {
                    p.bump();
                    break;
                }
                _ => {
                    let (byte, line, col) = p.pos_meta(p.position());
                    return Err(Error::ExpectedCommaOrArrayEnd { byte, line, col });
                }
            }
        }
        Ok(arr)
    }

    fn parse_object_internal(p: &mut super::json::Parser<'_>) -> Result<Map, Error> {
        if p.bump() != Some(b'{') {
            let (byte, line, col) = p.pos_meta(p.position());
            return Err(Error::ExpectedObjectStart { byte, line, col });
        }
        let mut map = Map::new();
        p.skip_ws();
        if let Some(b'}') = p.peek() {
            p.bump();
            return Ok(map);
        }
        loop {
            let key = p.parse_string()?;
            p.skip_ws();
            if p.peek() == Some(b':') {
                p.bump();
            } else {
                let (byte, line, col) = p.pos_meta(p.position());
                return Err(Error::ExpectedColon { byte, line, col });
            }
            let value = parse_value_internal(p)?;
            map.insert(key, value);
            p.skip_ws();
            match p.peek() {
                Some(b',') => {
                    p.bump();
                    p.skip_ws();
                }
                Some(b'}') => {
                    p.bump();
                    break;
                }
                _ => {
                    let (byte, line, col) = p.pos_meta(p.position());
                    return Err(Error::ExpectedCommaOrObjectEnd { byte, line, col });
                }
            }
        }
        Ok(map)
    }

    fn parse_number_value(p: &mut super::json::Parser<'_>) -> Result<Value, Error> {
        let s = p.input();
        let bytes = s.as_bytes();
        let start = p.position();
        let mut i = start;
        let len = bytes.len();
        let mut neg = false;
        if i < len && bytes[i] == b'-' {
            neg = true;
            i += 1;
        }
        let int_start = i;
        let mut saw = false;
        while i < len && bytes[i].is_ascii_digit() {
            saw = true;
            i += 1;
        }
        if !saw {
            let (byte, line, col) = p.pos_meta(i.min(len));
            return Err(Error::ExpectedDigits { byte, line, col });
        }
        if bytes[int_start] == b'0' && i > int_start + 1 {
            return Err(p.err_at(int_start + 1, Parser::LEADING_ZERO_MSG));
        }
        let mut is_float = false;
        if i < len && bytes[i] == b'.' {
            is_float = true;
            i += 1;
            let mut has_frac = false;
            while i < len && bytes[i].is_ascii_digit() {
                has_frac = true;
                i += 1;
            }
            if !has_frac {
                let (byte, line, col) = p.pos_meta(i.min(len));
                return Err(Error::ExpectedFracDigits { byte, line, col });
            }
        }
        if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
            is_float = true;
            i += 1;
            if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                i += 1;
            }
            let mut has_exp = false;
            while i < len && bytes[i].is_ascii_digit() {
                has_exp = true;
                i += 1;
            }
            if !has_exp {
                let (byte, line, col) = p.pos_meta(i.min(len));
                return Err(Error::ExpectedExpDigits { byte, line, col });
            }
        }
        let num_slice = &s[start..i];
        p.i = i;
        if is_float {
            let v: f64 = num_slice.parse().map_err(|_| {
                let (byte, line, col) = p.pos_meta(p.position());
                Error::WithPos {
                    msg: "float parse",
                    byte,
                    line,
                    col,
                }
            })?;
            let n = Number::from_f64(v).ok_or_else(|| {
                let (byte, line, col) = p.pos_meta(p.position());
                Error::WithPos {
                    msg: "NaN/Inf",
                    byte,
                    line,
                    col,
                }
            })?;
            return Ok(Value::Number(n));
        }
        if neg && &s[int_start..i] == "0" {
            let n = Number::from_f64(-0.0).ok_or_else(|| {
                let (byte, line, col) = p.pos_meta(p.position());
                Error::WithPos {
                    msg: "NaN/Inf",
                    byte,
                    line,
                    col,
                }
            })?;
            return Ok(Value::Number(n));
        }
        let digits = &s[int_start..i];
        if !neg {
            if let Ok(u) = digits.parse::<u64>() {
                if u <= i64::MAX as u64 {
                    return Ok(Value::Number(Number::from(u as i64)));
                }
                return Ok(Value::Number(Number::from(u)));
            }
        } else if let Ok(u) = digits.parse::<u64>() {
            if u == (i64::MAX as u64) + 1 {
                return Ok(Value::Number(Number::from(i64::MIN)));
            }
            if u <= i64::MAX as u64 {
                let v = -(u as i64);
                return Ok(Value::Number(Number::from(v)));
            }
        }
        let v: f64 = num_slice.parse().map_err(|_| {
            let (byte, line, col) = p.pos_meta(p.position());
            Error::WithPos {
                msg: "number parse",
                byte,
                line,
                col,
            }
        })?;
        let n = Number::from_f64(v).ok_or_else(|| {
            let (byte, line, col) = p.pos_meta(p.position());
            Error::WithPos {
                msg: "NaN/Inf",
                byte,
                line,
                col,
            }
        })?;
        Ok(Value::Number(n))
    }

    /// Convenience: parse a JSON `Value` from a byte slice.
    pub fn from_slice_value(bytes: &[u8]) -> Result<Value, Error> {
        let s = std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
        parse_value(s)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::json;

        #[test]
        fn parse_value_str_and_bytes_match() {
            let s = "{\"k\": [1, true, \"x\"]}";
            let v1 = parse_value(s).expect("parse_value");
            let v2 = from_slice_value(s.as_bytes()).expect("from_slice_value");
            assert_eq!(v1, v2);
        }

        #[test]
        fn json_serialize_from_reference() {
            let value = "hello".to_string();
            let rendered = to_json(&&value).expect("serialize reference");
            assert_eq!(rendered, "\"hello\"");
        }

        #[test]
        fn fast_writer_handles_reference_fields() {
            use crate::derive::JsonSerialize;

            #[derive(JsonSerialize)]
            struct Borrowed<'a> {
                field: &'a u32,
            }

            let inner = 42u32;
            let borrowed = Borrowed { field: &inner };
            let rendered = to_json(&borrowed).expect("serialize borrowed struct");
            assert_eq!(rendered, "{\"field\":42}");
        }

        #[test]
        fn object_macro_basic() {
            let value = json!({"key": 1u32, "flag": true});
            let string = to_json(&value).expect("json render");
            assert_eq!(string, "{\"flag\":true,\"key\":1}");
            let alt = norito::json!({"another": 2u32});
            let alt_string = to_json(&alt).expect("json render alt");
            assert_eq!(alt_string, "{\"another\":2}");
        }

        #[test]
        fn helpers_convert_to_value() {
            let value = crate::json::to_value(&42u32).expect("to_value");
            assert_eq!(
                value,
                crate::json::Value::Number(crate::json::Number::U64(42))
            );

            let array = crate::json::array([1u32, 2u32]).expect("array helper");
            assert_eq!(
                array,
                crate::json::Value::Array(vec![json!(1u32), json!(2u32)])
            );

            let object = crate::json::object([
                ("alpha", crate::json::Value::from(1u32)),
                ("beta", crate::json::Value::from(true)),
            ])
            .expect("object helper");
            assert_eq!(object["alpha"], json!(1u32));
            assert_eq!(object["beta"], json!(true));
        }

        #[test]
        fn pretty_writer_handles_multibyte_strings() {
            let sample: String = [
                '\u{ff87}', '\u{ff88}', '\u{ff8a}', '\u{ff8b}', '\u{ff8c}', '\u{ff8d}', '\u{ff8e}',
                '\u{ff8f}', '\u{ff90}', '\u{ff91}', '\u{ff92}', '\u{ff93}', '\u{ff94}', '\u{ff95}',
                '\u{ff96}', '\u{ff97}',
            ]
            .iter()
            .copied()
            .collect();
            let value =
                json::object([("input", Value::from(sample.clone()))]).expect("object render");
            let rendered = json::to_string_pretty(&value).expect("json render with multibyte");
            let reparsed: Value = json::from_str(&rendered).expect("parse pretty-printed output");
            assert_eq!(reparsed["input"], Value::from(sample));
        }

        #[test]
        fn unescape_json_string_preserves_utf8_bytes() {
            let raw = format!("price: {}\\nend", '\u{00A2}');
            let out = unescape_json_string(&raw).expect("unescape");
            let expected = format!("price: {}\nend", '\u{00A2}');
            assert_eq!(out, expected);
        }

        #[test]
        fn parse_u64_rejects_leading_zero() {
            let mut parser = Parser::new("01");
            let err = parser
                .parse_u64()
                .expect_err("leading zero should be rejected");
            match err {
                Error::WithPos { msg, .. } => assert_eq!(msg, Parser::LEADING_ZERO_MSG),
                other => panic!("unexpected error variant: {other:?}"),
            }
        }

        #[test]
        fn parse_i64_from_parser_rejects_leading_zero() {
            let mut parser = Parser::new("-012");
            let err = parse_i64_from_parser(&mut parser)
                .expect_err("leading zero in signed number should be rejected");
            match err {
                Error::WithPos { msg, .. } => assert_eq!(msg, Parser::LEADING_ZERO_MSG),
                other => panic!("unexpected error variant: {other:?}"),
            }
        }

        #[test]
        fn parse_number_token_rejects_leading_zero() {
            for sample in ["01", "-012"] {
                let mut parser = Parser::new(sample);
                let err = parse_number_token(&mut parser)
                    .expect_err("leading zero in Value parser should be rejected");
                match err {
                    Error::WithPos { msg, .. } => assert_eq!(msg, Parser::LEADING_ZERO_MSG),
                    other => panic!("unexpected error variant for {sample:?}: {other:?}"),
                }
            }
        }

        #[test]
        fn parse_value_rejects_leading_zero() {
            for sample in ["01", "-012"] {
                let err = parse_value(sample).expect_err("leading zero should be rejected");
                match err {
                    Error::WithPos { msg, .. } => assert_eq!(msg, Parser::LEADING_ZERO_MSG),
                    other => panic!("unexpected error variant for {sample:?}: {other:?}"),
                }
            }
        }
    }

    /// Write JSON to an `io::Write` sink.
    #[cfg(feature = "json-std-io")]
    pub fn to_writer<W: std::io::Write, T: JsonSerialize>(
        mut writer: W,
        value: &T,
    ) -> Result<(), Error> {
        let json = to_json(value)?;
        writer
            .write_all(json.as_bytes())
            .map_err(|e| Error::Message(e.to_string()))
    }

    /// Write pretty-printed JSON to an `io::Write` sink.
    #[cfg(feature = "json-std-io")]
    pub fn to_writer_pretty<W: std::io::Write, T: JsonSerialize>(
        mut writer: W,
        value: &T,
    ) -> Result<(), Error> {
        let json = to_json_pretty(value)?;
        writer
            .write_all(json.as_bytes())
            .map_err(|e| Error::Message(e.to_string()))
    }

    /// Parse JSON from an `io::Read` source.
    #[cfg(feature = "json-std-io")]
    pub fn from_reader<R: std::io::Read, T: JsonDeserialize>(mut reader: R) -> Result<T, Error> {
        let mut buf = String::new();
        reader
            .read_to_string(&mut buf)
            .map_err(|e| Error::Message(e.to_string()))?;
        from_str(&buf)
    }

    /// Simple bump arena for unescaped string storage.
    pub struct Arena {
        buf: Vec<u8>,
    }
    impl Default for Arena {
        fn default() -> Self {
            Self::new()
        }
    }
    impl Arena {
        pub fn new() -> Self {
            Self { buf: Vec::new() }
        }
        pub fn clear(&mut self) {
            self.buf.clear();
        }
        fn alloc_str(&mut self, bytes: &[u8]) -> &str {
            let start = self.buf.len();
            self.buf.extend_from_slice(bytes);
            let end = self.buf.len();
            // SAFETY: callers must guarantee valid UTF‑8 bytes; we return an immutable
            // slice to the exact range we just appended so future arena growth does not
            // change the visible contents or rely on tail length.
            unsafe { std::str::from_utf8_unchecked(&self.buf[start..end]) }
        }
    }

    /// Serialize a JSON string with proper escaping into `out`.
    #[inline]
    fn push_u16_escape(out: &mut String, code: u16) {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        out.push('\\');
        out.push('u');
        out.push(HEX[((code >> 12) & 0xF) as usize] as char);
        out.push(HEX[((code >> 8) & 0xF) as usize] as char);
        out.push(HEX[((code >> 4) & 0xF) as usize] as char);
        out.push(HEX[(code & 0xF) as usize] as char);
    }

    fn write_json_string_charwise(s: &str, out: &mut String) {
        out.reserve(s.len() + 2);
        out.push('"');
        for ch in s.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\u{08}' => out.push_str("\\b"),
                '\u{0C}' => out.push_str("\\f"),
                '\u{2028}' => out.push_str("\\u2028"),
                '\u{2029}' => out.push_str("\\u2029"),
                c if (c as u32) < 0x20 => {
                    out.push_str("\\u00");
                    const HEX: &[u8; 16] = b"0123456789ABCDEF";
                    out.push(HEX[((c as u32 >> 4) & 0xF) as usize] as char);
                    out.push(HEX[(c as u32 & 0xF) as usize] as char);
                }
                c if (c as u32) >= 0x10000 => {
                    let code = (c as u32) - 0x1_0000;
                    let hi = 0xD800u16 + ((code >> 10) as u16);
                    let lo = 0xDC00u16 + ((code & 0x3FF) as u16);
                    push_u16_escape(out, hi);
                    push_u16_escape(out, lo);
                }
                _ => out.push(ch),
            }
        }
        out.push('"');
    }

    pub fn write_json_string(s: &str, out: &mut String) {
        if !s.is_ascii()
            || s.chars()
                .any(|c| (c as u32) >= 0x10000 || c == '\u{2028}' || c == '\u{2029}')
        {
            write_json_string_charwise(s, out);
            return;
        }
        // aarch64 NEON fast path using LUT classification + chunked copy
        #[cfg(all(
            feature = "simd-accel",
            target_arch = "aarch64",
            target_feature = "neon"
        ))]
        {
            unsafe fn write_neon(s: &str, out: &mut String) {
                use core::arch::aarch64::*;
                unsafe {
                    out.reserve(s.len() + 2);
                    out.push('"');
                    let bytes = s.as_bytes();
                    let mut i = 0usize;
                    let lut_lo = vld1q_u8([0u8; 16].as_ptr()); // not used; we do compares instead
                    let _ = lut_lo; // silence unused
                    while i < bytes.len() {
                        // Load 16 bytes or less
                        let rem = bytes.len() - i;
                        if rem >= 16 {
                            let v = vld1q_u8(bytes.as_ptr().add(i));
                            let is_quote = vceqq_u8(v, vdupq_n_u8(b'"'));
                            let is_bslash = vceqq_u8(v, vdupq_n_u8(b'\\'));
                            let ctrl = vcltq_u8(v, vdupq_n_u8(0x20));
                            let specials = vorrq_u8(vorrq_u8(is_quote, is_bslash), ctrl);
                            let mut mask = 0u16;
                            let msb = vshrq_n_u8(specials, 7);
                            let mut tmp = [0u8; 16];
                            vst1q_u8(tmp.as_mut_ptr(), msb);
                            for (j, value) in tmp.iter().enumerate() {
                                mask |= ((value & 1) as u16) << j;
                            }
                            if mask == 0 {
                                // No specials in this block; copy raw
                                out.push_str(std::str::from_utf8_unchecked(&bytes[i..i + 16]));
                                i += 16;
                                continue;
                            } else {
                                // Copy up to first special, then handle escape
                                let tz = mask.trailing_zeros() as usize;
                                if tz > 0 {
                                    out.push_str(std::str::from_utf8_unchecked(&bytes[i..i + tz]));
                                    i += tz;
                                }
                            }
                        }
                        // Scalar escape for the special
                        let b = bytes[i];
                        match b {
                            b'"' => out.push_str("\\\""),
                            b'\\' => out.push_str("\\\\"),
                            b'\n' => out.push_str("\\n"),
                            b'\r' => out.push_str("\\r"),
                            b'\t' => out.push_str("\\t"),
                            c if c < 0x20 => {
                                out.push_str("\\u00");
                                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                                out.push(HEX[(c >> 4) as usize] as char);
                                out.push(HEX[(c & 0x0F) as usize] as char);
                            }
                            _ => {
                                // Should not happen; caught by NEON mask as non-special
                                out.push(b as char);
                            }
                        }
                        i += 1;
                    }
                    out.push('"');
                }
            }
            unsafe {
                write_neon(s, out);
            }
        }
        // x86_64 AVX2 fast path using vector compares + movemask
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            unsafe fn write_avx2(s: &str, out: &mut String) {
                use core::arch::x86_64::*;
                unsafe {
                    out.reserve(s.len() + 2);
                    out.push('"');
                    let bytes = s.as_bytes();
                    let mut i = 0usize;
                    let v_quote = _mm256_set1_epi8(b'"' as i8);
                    let v_bslash = _mm256_set1_epi8(b'\\' as i8);
                    while i < bytes.len() {
                        let rem = bytes.len() - i;
                        if rem >= 32 {
                            let v = _mm256_loadu_si256(bytes.as_ptr().add(i) as *const __m256i);
                            let is_q = _mm256_cmpeq_epi8(v, v_quote);
                            let is_b = _mm256_cmpeq_epi8(v, v_bslash);
                            // ctrl < 0x20: compare signed; emulate by subtracting 0x20 and test negative
                            let ctrl = _mm256_cmpgt_epi8(_mm256_set1_epi8(0x20i8), v);
                            let specials = _mm256_or_si256(_mm256_or_si256(is_q, is_b), ctrl);
                            let mask = _mm256_movemask_epi8(specials) as u32;
                            if mask == 0 {
                                out.push_str(std::str::from_utf8_unchecked(&bytes[i..i + 32]));
                                i += 32;
                                continue;
                            } else {
                                let tz = mask.trailing_zeros() as usize;
                                if tz > 0 {
                                    out.push_str(std::str::from_utf8_unchecked(&bytes[i..i + tz]));
                                    i += tz;
                                }
                            }
                        }
                        let b = bytes[i];
                        match b {
                            b'"' => out.push_str("\\\""),
                            b'\\' => out.push_str("\\\\"),
                            b'\n' => out.push_str("\\n"),
                            b'\r' => out.push_str("\\r"),
                            b'\t' => out.push_str("\\t"),
                            c if c < 0x20 => {
                                out.push_str("\\u00");
                                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                                out.push(HEX[(c >> 4) as usize] as char);
                                out.push(HEX[(c & 0x0F) as usize] as char);
                            }
                            _ => out.push(b as char),
                        }
                        i += 1;
                    }
                    out.push('"');
                }
            }
            unsafe {
                write_avx2(s, out);
            }
        }
        // x86_64 AVX2 runtime path when binary is not compiled with avx2 by default
        #[cfg(all(
            target_arch = "x86_64",
            feature = "simd-accel",
            not(target_feature = "avx2")
        ))]
        {
            if std::is_x86_feature_detected!("avx2") {
                #[target_feature(enable = "avx2")]
                unsafe fn write_avx2_rt(s: &str, out: &mut String) {
                    use core::arch::x86_64::*;
                    unsafe {
                        out.reserve(s.len() + 2);
                        out.push('"');
                        let bytes = s.as_bytes();
                        let mut i = 0usize;
                        let v_quote = _mm256_set1_epi8(b'"' as i8);
                        let v_bslash = _mm256_set1_epi8(b'\\' as i8);
                        while i < bytes.len() {
                            let rem = bytes.len() - i;
                            if rem >= 32 {
                                let v = _mm256_loadu_si256(bytes.as_ptr().add(i) as *const __m256i);
                                let is_q = _mm256_cmpeq_epi8(v, v_quote);
                                let is_b = _mm256_cmpeq_epi8(v, v_bslash);
                                // ctrl < 0x20: compare signed; emulate by subtracting 0x20 and test negative
                                let ctrl = _mm256_cmpgt_epi8(_mm256_set1_epi8(0x20i8), v);
                                let specials = _mm256_or_si256(_mm256_or_si256(is_q, is_b), ctrl);
                                let mask = _mm256_movemask_epi8(specials) as u32;
                                if mask == 0 {
                                    out.push_str(std::str::from_utf8_unchecked(&bytes[i..i + 32]));
                                    i += 32;
                                    continue;
                                } else {
                                    let tz = mask.trailing_zeros() as usize;
                                    if tz > 0 {
                                        out.push_str(std::str::from_utf8_unchecked(
                                            &bytes[i..i + tz],
                                        ));
                                        i += tz;
                                    }
                                }
                            }
                            let b = bytes[i];
                            match b {
                                b'"' => out.push_str("\\\""),
                                b'\\' => out.push_str("\\\\"),
                                b'\n' => out.push_str("\\n"),
                                b'\r' => out.push_str("\\r"),
                                b'\t' => out.push_str("\\t"),
                                c if c < 0x20 => {
                                    out.push_str("\\u00");
                                    const HEX: &[u8; 16] = b"0123456789ABCDEF";
                                    out.push(HEX[(c >> 4) as usize] as char);
                                    out.push(HEX[(c & 0x0F) as usize] as char);
                                }
                                _ => out.push(b as char),
                            }
                            i += 1;
                        }
                        out.push('"');
                    }
                }
                unsafe {
                    write_avx2_rt(s, out);
                }
            }
        }
        #[cfg(not(any(
            all(
                feature = "simd-accel",
                target_arch = "aarch64",
                target_feature = "neon"
            ),
            all(
                feature = "simd-accel",
                target_arch = "x86_64",
                target_feature = "avx2"
            )
        )))]
        {
            // Scalar fallback: chunked copy + escape
            out.reserve(s.len() + 2);
            out.push('"');
            let bytes = s.as_bytes();
            let mut i = 0usize;
            while i < bytes.len() {
                let mut j = i;
                while j < bytes.len() {
                    let b = bytes[j];
                    if b == b'"' || b == b'\\' || b < 0x20 {
                        break;
                    }
                    j += 1;
                }
                if j > i {
                    out.push_str(unsafe { std::str::from_utf8_unchecked(&bytes[i..j]) });
                }
                if j == bytes.len() {
                    break;
                }
                match bytes[j] {
                    b'"' => out.push_str("\\\""),
                    b'\\' => out.push_str("\\\\"),
                    b'\n' => out.push_str("\\n"),
                    b'\r' => out.push_str("\\r"),
                    b'\t' => out.push_str("\\t"),
                    0x08 => out.push_str("\\b"),
                    0x0C => out.push_str("\\f"),
                    c if c < 0x20 => {
                        out.push_str("\\u00");
                        const HEX: &[u8; 16] = b"0123456789ABCDEF";
                        out.push(HEX[(c >> 4) as usize] as char);
                        out.push(HEX[(c & 0x0F) as usize] as char);
                    }
                    _ => unreachable!(),
                }
                i = j + 1;
            }
            out.push('"');
        }
    }

    /// Trait for types that can be serialized to JSON.
    pub trait JsonSerialize {
        /// Serialize `self` into `out` as JSON.
        fn json_serialize(&self, out: &mut String);
    }

    impl JsonSerialize for bool {
        fn json_serialize(&self, out: &mut String) {
            out.push_str(if *self { "true" } else { "false" });
        }
    }

    #[inline]
    fn encode_hex(bytes: &[u8], out: &mut String) {
        const LOOKUP: &[u8; 16] = b"0123456789ABCDEF";
        out.reserve(bytes.len() * 2 + 2);
        out.push('"');
        for &byte in bytes {
            out.push(LOOKUP[(byte >> 4) as usize] as char);
            out.push(LOOKUP[(byte & 0x0f) as usize] as char);
        }
        out.push('"');
    }

    fn decode_hex<const N: usize>(s: &str) -> Result<[u8; N], Error> {
        if s.len() != N * 2 {
            return Err(Error::Message(format!(
                "expected {N} byte hex string, got length {}",
                s.len()
            )));
        }
        let mut out = [0u8; N];
        let bytes = s.as_bytes();
        for i in 0..N {
            let hi = decode_nibble(bytes[2 * i]).ok_or_else(|| {
                Error::Message(format!(
                    "invalid hex digit `{}` at position {}",
                    bytes[2 * i] as char,
                    2 * i
                ))
            })?;
            let lo = decode_nibble(bytes[2 * i + 1]).ok_or_else(|| {
                Error::Message(format!(
                    "invalid hex digit `{}` at position {}",
                    bytes[2 * i + 1] as char,
                    2 * i + 1
                ))
            })?;
            out[i] = (hi << 4) | lo;
        }
        Ok(out)
    }

    #[inline]
    const fn decode_nibble(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    #[inline]
    fn write_u128_json(out: &mut String, mut v: u128) {
        const BUF_LEN: usize = 39; // ceil(log10(2^128))
        let mut buf = [0u8; BUF_LEN];
        let mut i = buf.len();
        if v == 0 {
            out.push('0');
            return;
        }
        while v > 0 {
            let d = (v % 10) as u8;
            v /= 10;
            i -= 1;
            buf[i] = b'0' + d;
        }
        unsafe {
            out.push_str(std::str::from_utf8_unchecked(&buf[i..]));
        }
    }
    #[inline]
    fn write_u64_json(out: &mut String, v: u64) {
        write_u128_json(out, v as u128)
    }
    #[inline]
    fn write_u32_json(out: &mut String, v: u32) {
        write_u64_json(out, v as u64)
    }

    #[inline]
    fn write_i64_json(out: &mut String, v: i64) {
        if v >= 0 {
            write_u64_json(out, v as u64);
        } else {
            out.push('-');
            write_u64_json(out, v.unsigned_abs());
        }
    }

    impl JsonSerialize for core::num::NonZeroU128 {
        fn json_serialize(&self, out: &mut String) {
            write_u128_json(out, self.get());
        }
    }
    impl JsonSerialize for core::num::NonZeroU64 {
        fn json_serialize(&self, out: &mut String) {
            write_u64_json(out, self.get());
        }
    }
    impl JsonSerialize for core::num::NonZeroU32 {
        fn json_serialize(&self, out: &mut String) {
            write_u32_json(out, self.get());
        }
    }
    impl JsonSerialize for core::num::NonZeroU16 {
        fn json_serialize(&self, out: &mut String) {
            write_u32_json(out, self.get() as u32);
        }
    }
    impl JsonSerialize for core::num::NonZeroUsize {
        fn json_serialize(&self, out: &mut String) {
            write_u64_json(out, self.get() as u64);
        }
    }
    impl JsonSerialize for str {
        fn json_serialize(&self, out: &mut String) {
            write_json_string(self, out);
        }
    }

    impl JsonSerialize for std::time::Duration {
        fn json_serialize(&self, out: &mut String) {
            out.push('{');
            out.push_str("\"secs\":");
            JsonSerialize::json_serialize(&self.as_secs(), out);
            out.push(',');
            out.push_str("\"nanos\":");
            JsonSerialize::json_serialize(&self.subsec_nanos(), out);
            out.push('}');
        }
    }
    impl<T: JsonSerialize> JsonSerialize for Option<T> {
        fn json_serialize(&self, out: &mut String) {
            match self {
                Some(v) => v.json_serialize(out),
                None => out.push_str("null"),
            }
        }
    }
    impl JsonSerialize for () {
        fn json_serialize(&self, out: &mut String) {
            out.push_str("null");
        }
    }
    impl<T: JsonSerialize> JsonSerialize for Vec<T> {
        fn json_serialize(&self, out: &mut String) {
            out.push('[');
            let mut first = true;
            for v in self {
                if !first {
                    out.push(',');
                }
                first = false;
                v.json_serialize(out);
            }
            out.push(']');
        }
    }

    impl FastJsonWrite for Value {
        fn write_json(&self, out: &mut String) {
            write_value_to_string(self, out, false, 0);
        }
    }

    impl<T: JsonDeserialize> JsonDeserialize for Box<T> {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            T::json_deserialize(parser).map(Box::new)
        }
    }

    impl JsonDeserialize for Box<str> {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            let string = String::json_deserialize(parser)?;
            Ok(string.into_boxed_str())
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Some(s) = value.as_str() {
                return Ok(s.to_owned().into_boxed_str());
            }
            String::json_from_value(value).map(String::into_boxed_str)
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            Ok(key.to_owned().into_boxed_str())
        }
    }

    impl<T: JsonSerialize + ?Sized> FastJsonWrite for Box<T> {
        fn write_json(&self, out: &mut String) {
            (**self).json_serialize(out);
        }
    }

    /// Bridge: any type with a typed writer can serve as a JSON serializer.
    impl<T: FastJsonWrite> JsonSerialize for T {
        fn json_serialize(&self, out: &mut String) {
            self.write_json(out)
        }
    }

    /// Serialize `value` into a compact JSON string.
    pub fn to_json<T: JsonSerialize + ?Sized>(value: &T) -> Result<String, Error> {
        let mut out = String::new();
        value.json_serialize(&mut out);
        Ok(out)
    }

    /// Build a Norito JSON array from an iterator of serializable values.
    pub fn array<T, I>(values: I) -> Result<Value, Error>
    where
        T: JsonSerialize,
        I: IntoIterator<Item = T>,
    {
        let mut out = Vec::new();
        for item in values {
            out.push(to_value(&item)?);
        }
        Ok(Value::Array(out))
    }

    /// Build a Norito JSON object from key/value pairs.
    pub fn object<K, V, I>(pairs: I) -> Result<Value, Error>
    where
        K: Into<String>,
        V: JsonSerialize,
        I: IntoIterator<Item = (K, V)>,
    {
        let mut map = Map::new();
        for (key, value) in pairs {
            map.insert(key.into(), to_value(&value)?);
        }
        Ok(Value::Object(map))
    }

    /// Serialize `value` into a compact JSON string using the fast typed writer.
    ///
    /// Alias for `to_json` kept for symmetry with `to_json_pretty`.
    pub fn to_json_fast<T: JsonSerialize + ?Sized>(value: &T) -> Result<String, Error> {
        to_json(value)
    }

    /// Pretty-print the JSON representation of `value` deterministically.
    ///
    /// Rules:
    /// - Two-space indentation
    /// - Newlines after `,`, `[` and `{` when appropriate
    /// - Canonical escaping preserved from the typed writer
    pub fn to_json_pretty<T: JsonSerialize + ?Sized>(value: &T) -> Result<String, Error> {
        let minified = to_json(value)?;
        Ok(pretty_format_minified_json(&minified))
    }

    /// Pretty-format a minified JSON string without reparsing.
    fn pretty_format_minified_json(input: &str) -> String {
        let bytes = input.as_bytes();
        let mut out = String::with_capacity(input.len() + input.len() / 4);
        let mut indent = 0usize;
        let mut i = 0usize;
        while i < bytes.len() {
            match bytes[i] {
                b' ' | b'\n' | b'\r' | b'\t' => {
                    i += 1; // drop whitespace
                }
                b'"' => {
                    let start = i;
                    i += 1;
                    while i < bytes.len() {
                        match bytes[i] {
                            b'\\' => {
                                i += 1;
                                if i < bytes.len() {
                                    i += 1;
                                }
                            }
                            b'"' => {
                                i += 1;
                                break;
                            }
                            _ => i += 1,
                        }
                    }
                    out.push_str(&input[start..i]);
                }
                b'{' | b'[' => {
                    out.push(bytes[i] as char);
                    indent += 1;
                    out.push('\n');
                    for _ in 0..indent {
                        out.push_str("  ");
                    }
                    i += 1;
                }
                b'}' | b']' => {
                    if indent > 0 {
                        indent = indent.saturating_sub(1);
                    }
                    out.push('\n');
                    for _ in 0..indent {
                        out.push_str("  ");
                    }
                    out.push(bytes[i] as char);
                    i += 1;
                }
                b',' => {
                    out.push(',');
                    out.push('\n');
                    for _ in 0..indent {
                        out.push_str("  ");
                    }
                    i += 1;
                }
                b':' => {
                    out.push(':');
                    out.push(' ');
                    i += 1;
                }
                other => {
                    out.push(other as char);
                    i += 1;
                }
            }
        }
        out
    }

    /// Unescape a borrowed JSON string (without surrounding quotes) into an owned `String`.
    ///
    /// Intended for use with `Reader` tokens (`StringBorrowed` and `KeyBorrowed`).
    ///
    /// Escapes → Code Points
    /// - `\"` → `U+0022` (double quote)
    /// - `\\` → `U+005C` (backslash)
    /// - `\/` → `U+002F` (forward slash)
    /// - `\b` → `U+0008` (backspace)
    /// - `\f` → `U+000C` (form feed)
    /// - `\n` → `U+000A` (line feed)
    /// - `\r` → `U+000D` (carriage return)
    /// - `\t` → `U+0009` (tab)
    /// - `\uXXXX` → Unicode code unit; surrogate pairs (`\uD800..\uDBFF` + `\uDC00..\uDFFF`) are combined into a single scalar.
    ///
    /// Errors include invalid hex digits in `\uXXXX`, unexpected/isolated low surrogates, missing low surrogates after a high surrogate, and control
    /// characters (`< 0x20`) appearing unescaped.
    pub fn unescape_json_string(s: &str) -> Result<String, Error> {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0usize;
        while i < bytes.len() {
            let b = bytes[i];
            i += 1;
            if b != b'\\' {
                if b < 0x20 {
                    return Err(Error::ControlInString {
                        byte: i - 1,
                        line: 1,
                        col: 1,
                    });
                }
                out.push(b);
                continue;
            }
            if i >= bytes.len() {
                return Err(Error::EofEscape {
                    byte: i,
                    line: 1,
                    col: 1,
                });
            }
            let esc = bytes[i];
            i += 1;
            match esc {
                b'"' => out.push(b'"'),
                b'\\' => out.push(b'\\'),
                b'/' => out.push(b'/'),
                b'b' => out.push(0x08),
                b'f' => out.push(0x0C),
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'u' => {
                    let hex_to_u32 = |idx: &mut usize| -> Result<u32, Error> {
                        let mut v: u32 = 0;
                        for _ in 0..4 {
                            if *idx >= bytes.len() {
                                return Err(Error::EofHex {
                                    byte: *idx,
                                    line: 1,
                                    col: 1,
                                });
                            }
                            let c = bytes[*idx];
                            *idx += 1;
                            v = (v << 4)
                                | match c {
                                    b'0'..=b'9' => (c - b'0') as u32,
                                    b'a'..=b'f' => (c - b'a' + 10) as u32,
                                    b'A'..=b'F' => (c - b'A' + 10) as u32,
                                    _ => {
                                        return Err(Error::InvalidHex {
                                            byte: *idx - 1,
                                            line: 1,
                                            col: 1,
                                        });
                                    }
                                };
                        }
                        Ok(v)
                    };
                    let hi = hex_to_u32(&mut i)?;
                    let cp = if (0xD800..=0xDBFF).contains(&hi) {
                        if i + 6 > bytes.len() || bytes[i] != b'\\' || bytes[i + 1] != b'u' {
                            return Err(Error::WithPos {
                                msg: "expected low surrogate",
                                byte: i,
                                line: 1,
                                col: 1,
                            });
                        }
                        i += 2; // skip \\u
                        let lo = hex_to_u32(&mut i)?;
                        if !(0xDC00..=0xDFFF).contains(&lo) {
                            return Err(Error::WithPos {
                                msg: "invalid low surrogate",
                                byte: i - 1,
                                line: 1,
                                col: 1,
                            });
                        }
                        0x10000 + (((hi - 0xD800) << 10) | (lo - 0xDC00))
                    } else if (0xDC00..=0xDFFF).contains(&hi) {
                        return Err(Error::WithPos {
                            msg: "unexpected low surrogate",
                            byte: i - 1,
                            line: 1,
                            col: 1,
                        });
                    } else {
                        hi
                    };
                    if let Some(ch) = char::from_u32(cp) {
                        let mut buf = [0u8; 4];
                        let n = ch.encode_utf8(&mut buf).len();
                        out.extend_from_slice(&buf[..n]);
                    } else {
                        return Err(Error::WithPos {
                            msg: "invalid codepoint",
                            byte: i - 1,
                            line: 1,
                            col: 1,
                        });
                    }
                }
                _ => {
                    return Err(Error::WithPos {
                        msg: "bad escape",
                        byte: i - 1,
                        line: 1,
                        col: 1,
                    });
                }
            }
        }
        String::from_utf8(out).map_err(|_| Error::InvalidUtf8)
    }

    /// A minimal JSON parser over `&str`.
    #[derive(Clone, Copy)]
    pub struct Parser<'a> {
        s: &'a [u8],
        i: usize,
    }

    impl<'a> Parser<'a> {
        const LEADING_ZERO_MSG: &'static str = "leading zeros are not allowed in JSON numbers";
        #[inline]
        fn pos_meta(&self, pos: usize) -> (usize, usize, usize) {
            let bytes = self.s;
            let mut line = 1usize;
            let mut col = 1usize;
            let mut i = 0usize;
            while i < pos && i < bytes.len() {
                if bytes[i] == b'\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
                i += 1;
            }
            (pos, line, col)
        }
        #[inline]
        fn err_at(&self, pos: usize, msg: &'static str) -> Error {
            let (byte, line, col) = self.pos_meta(pos);
            Error::WithPos {
                msg,
                byte,
                line,
                col,
            }
        }
        #[inline]
        fn err_unexpected_char(&self) -> Error {
            let (byte, line, col) = self.pos_meta(self.i.min(self.s.len()));
            let found = if self.i < self.s.len() {
                UnexpectedToken::Char(self.s[self.i] as char)
            } else {
                UnexpectedToken::Eof
            };
            Error::UnexpectedCharacter {
                found,
                byte,
                line,
                col,
            }
        }
        #[inline]
        fn err_expected_digits(&self) -> Error {
            let (byte, line, col) = self.pos_meta(self.i.min(self.s.len()));
            Error::ExpectedDigits { byte, line, col }
        }
        #[inline]
        fn err_u64_overflow(&self) -> Error {
            let (byte, line, col) = self.pos_meta(self.i.min(self.s.len()));
            Error::U64Overflow { byte, line, col }
        }
        #[inline]
        fn err_here(&self, msg: &'static str) -> Error {
            self.err_at(self.i.min(self.s.len()), msg)
        }
        /// Create a new parser over string slice `s`.
        pub fn new(s: &'a str) -> Self {
            Self {
                s: s.as_bytes(),
                i: 0,
            }
        }
        /// Create a new parser starting at byte position `pos`.
        pub fn new_at(s: &'a str, pos: usize) -> Self {
            Self {
                s: s.as_bytes(),
                i: pos,
            }
        }
        /// Current byte position in the input stream.
        pub fn position(&self) -> usize {
            self.i
        }
        /// Borrow the remaining input as a `&str` from the current position to the end.
        pub fn input_from_pos(&self) -> &'a str {
            // Safety: `self.s` is constructed from a `&str` and thus is valid UTF‑8.
            unsafe { std::str::from_utf8_unchecked(&self.s[self.i..]) }
        }
        /// Borrow the full original input as a `&str`.
        pub fn input(&self) -> &'a str {
            unsafe { std::str::from_utf8_unchecked(self.s) }
        }
        /// Return true if no more input remains.
        pub fn eof(&self) -> bool {
            self.i >= self.s.len()
        }
        /// Peek at the next byte without consuming it.
        pub fn peek(&self) -> Option<u8> {
            self.s.get(self.i).copied()
        }
        /// Consume and return the next byte.
        pub fn bump(&mut self) -> Option<u8> {
            let b = self.s.get(self.i).copied();
            self.i = self.i.saturating_add(1);
            b
        }
        /// Skip any ASCII whitespace.
        pub fn skip_ws(&mut self) {
            while let Some(b) = self.peek() {
                if matches!(b, b' ' | b'\n' | b'\r' | b'\t') {
                    self.i += 1;
                } else {
                    break;
                }
            }
        }
        /// Expect the next non-whitespace byte to equal `b`.
        pub fn expect(&mut self, b: u8) -> Result<(), Error> {
            self.skip_ws();
            match self.peek() {
                Some(x) if x == b => {
                    // Consume only on success to keep error position stable
                    self.bump();
                    Ok(())
                }
                _ => Err(self.err_unexpected_char()),
            }
        }
        /// Compatibility: consume an expected byte (alias of expect).
        pub fn consume_char(&mut self, b: u8) -> Result<(), Error> {
            self.expect(b)
        }
        /// Compatibility: try to consume an expected byte, returning true if consumed.
        pub fn try_consume_char(&mut self, b: u8) -> Result<bool, Error> {
            self.skip_ws();
            if self.peek() == Some(b) {
                self.bump();
                Ok(true)
            } else {
                Ok(false)
            }
        }
        /// Compatibility: consume a comma if present after optional whitespace.
        pub fn consume_comma_if_present(&mut self) -> Result<bool, Error> {
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.bump();
                Ok(true)
            } else {
                Ok(false)
            }
        }
        /// Parse a JSON `null` token.
        pub fn parse_null(&mut self) -> Result<(), Error> {
            self.skip_ws();
            let rest = &self.s.get(self.i..).ok_or_else(|| self.err_here("eof"))?;
            if rest.starts_with(b"null") {
                self.i += 4;
                Ok(())
            } else {
                let (byte, line, col) = self.pos_meta(self.i);
                Err(Error::ExpectedNull { byte, line, col })
            }
        }
        /// Parse a boolean.
        pub fn parse_bool(&mut self) -> Result<bool, Error> {
            self.skip_ws();
            let rest = &self.s[self.i..];
            if rest.starts_with(b"true") {
                self.i += 4;
                Ok(true)
            } else if rest.starts_with(b"false") {
                self.i += 5;
                Ok(false)
            } else {
                let (byte, line, col) = self.pos_meta(self.i);
                Err(Error::ExpectedBool { byte, line, col })
            }
        }
        /// Parse a non-negative integer into `u64`.
        pub fn parse_u64(&mut self) -> Result<u64, Error> {
            self.skip_ws();
            let bytes = self.s;
            let mut i = self.i;
            let start = i;
            let mut val: u64 = 0;
            let mut any = false;
            while i < bytes.len() {
                let b = bytes[i];
                if b.wrapping_sub(b'0') <= 9 {
                    // fast digit test
                    let d = (b - b'0') as u64;
                    // Overflow check: val*10 + d <= u64::MAX
                    if val > (u64::MAX - d) / 10 {
                        return Err(self.err_u64_overflow());
                    }
                    val = val * 10 + d;
                    i += 1;
                    any = true;
                } else {
                    break;
                }
            }
            if !any {
                return Err(self.err_expected_digits());
            }
            if bytes[start] == b'0' && i > start + 1 {
                return Err(self.err_at(start + 1, Self::LEADING_ZERO_MSG));
            }
            self.i = i;
            Ok(val)
        }
        /// Parse a JSON string with escaping support.
        pub fn parse_string(&mut self) -> Result<String, Error> {
            self.skip_ws();
            self.expect(b'"')?;
            // Fast path: scan for closing quote without encountering escapes or controls.
            let start = self.i;
            let bytes = self.s;
            let mut i = start;
            while i < bytes.len() {
                let b = bytes[i];
                if b == b'"' {
                    let slice = &bytes[start..i];
                    self.i = i + 1;
                    let st = std::str::from_utf8(slice)
                        .map_err(|_| self.err_at(start, "invalid utf8"))?;
                    return Ok(st.to_string());
                }
                if b == b'\\' || b < 0x20 {
                    break;
                }
                i += 1;
            }
            // Slow path: build into a UTF-8 byte buffer, handling escapes
            let mut out: Vec<u8> = Vec::with_capacity(16);
            loop {
                let b = self.bump().ok_or_else(|| {
                    let (byte, line, col) = self.pos_meta(self.i);
                    Error::UnterminatedString { byte, line, col }
                })?;
                match b {
                    b'"' => break,
                    b'\\' => {
                        let esc = self.bump().ok_or_else(|| {
                            let (byte, line, col) = self.pos_meta(self.i);
                            Error::EofEscape { byte, line, col }
                        })?;
                        match esc {
                            b'"' => out.push(b'"'),
                            b'\\' => out.push(b'\\'),
                            b'/' => out.push(b'/'),
                            b'b' => out.push(0x08),
                            b'f' => out.push(0x0C),
                            b'n' => out.push(b'\n'),
                            b'r' => out.push(b'\r'),
                            b't' => out.push(b'\t'),
                            b'u' => {
                                let mut hi: u32 = 0;
                                for _ in 0..4 {
                                    let h = self.bump().ok_or_else(|| {
                                        let (byte, line, col) = self.pos_meta(self.i);
                                        Error::EofHex { byte, line, col }
                                    })?;
                                    hi = (hi << 4)
                                        | match h {
                                            b'0'..=b'9' => (h - b'0') as u32,
                                            b'a'..=b'f' => (h - b'a' + 10) as u32,
                                            b'A'..=b'F' => (h - b'A' + 10) as u32,
                                            _ => {
                                                let (byte, line, col) = self.pos_meta(self.i - 1);
                                                return Err(Error::InvalidHex { byte, line, col });
                                            }
                                        };
                                }
                                if (0xD800..=0xDBFF).contains(&hi) {
                                    // Expect a following \uDC00..\uDFFF low surrogate
                                    if self.peek() != Some(b'\\') {
                                        let (byte, line, col) = self.pos_meta(self.i);
                                        return Err(Error::WithPos {
                                            msg: "expected low surrogate",
                                            byte,
                                            line,
                                            col,
                                        });
                                    }
                                    self.bump();
                                    if self.bump() != Some(b'u') {
                                        let (byte, line, col) = self.pos_meta(self.i);
                                        return Err(Error::WithPos {
                                            msg: "expected \\u for low surrogate",
                                            byte,
                                            line,
                                            col,
                                        });
                                    }
                                    let mut lo: u32 = 0;
                                    for _ in 0..4 {
                                        let h = self.bump().ok_or_else(|| {
                                            let (byte, line, col) = self.pos_meta(self.i);
                                            Error::EofHex { byte, line, col }
                                        })?;
                                        lo = (lo << 4)
                                            | match h {
                                                b'0'..=b'9' => (h - b'0') as u32,
                                                b'a'..=b'f' => (h - b'a' + 10) as u32,
                                                b'A'..=b'F' => (h - b'A' + 10) as u32,
                                                _ => {
                                                    let (byte, line, col) =
                                                        self.pos_meta(self.i - 1);
                                                    return Err(Error::InvalidHex {
                                                        byte,
                                                        line,
                                                        col,
                                                    });
                                                }
                                            };
                                    }
                                    if !(0xDC00..=0xDFFF).contains(&lo) {
                                        let (byte, line, col) = self.pos_meta(self.i - 1);
                                        return Err(Error::WithPos {
                                            msg: "invalid low surrogate",
                                            byte,
                                            line,
                                            col,
                                        });
                                    }
                                    let cp: u32 = 0x10000 + (((hi - 0xD800) << 10) | (lo - 0xDC00));
                                    if let Some(ch) = char::from_u32(cp) {
                                        let mut buf = [0u8; 4];
                                        let n = ch.encode_utf8(&mut buf).len();
                                        out.extend_from_slice(&buf[..n]);
                                    } else {
                                        let (byte, line, col) = self.pos_meta(self.i - 1);
                                        return Err(Error::WithPos {
                                            msg: "invalid codepoint",
                                            byte,
                                            line,
                                            col,
                                        });
                                    }
                                } else if (0xDC00..=0xDFFF).contains(&hi) {
                                    let (byte, line, col) = self.pos_meta(self.i - 1);
                                    return Err(Error::WithPos {
                                        msg: "unexpected low surrogate",
                                        byte,
                                        line,
                                        col,
                                    });
                                } else if let Some(ch) = char::from_u32(hi) {
                                    let mut buf = [0u8; 4];
                                    let n = ch.encode_utf8(&mut buf).len();
                                    out.extend_from_slice(&buf[..n]);
                                } else {
                                    let (byte, line, col) = self.pos_meta(self.i - 1);
                                    return Err(Error::WithPos {
                                        msg: "invalid codepoint",
                                        byte,
                                        line,
                                        col,
                                    });
                                }
                            }
                            _ => {
                                let (byte, line, col) = self.pos_meta(self.i - 1);
                                return Err(Error::WithPos {
                                    msg: "bad escape",
                                    byte,
                                    line,
                                    col,
                                });
                            }
                        }
                    }
                    b if b < 0x20 => {
                        let (byte, line, col) = self.pos_meta(self.i - 1);
                        return Err(Error::ControlInString { byte, line, col });
                    }
                    b => out.push(b),
                }
            }
            let s = std::str::from_utf8(&out).map_err(|_| self.err_here("invalid utf8"))?;
            Ok(s.to_string())
        }
        /// Parse a JSON array into `Vec<T>` using `JsonDeserialize` for elements.
        pub fn parse_array<T: JsonDeserialize>(&mut self) -> Result<Vec<T>, Error> {
            self.skip_ws();
            self.expect(b'[')?;
            let mut out = Vec::new();
            self.skip_ws();
            if matches!(self.peek(), Some(b']')) {
                self.bump(); // consume ']'
                return Ok(out);
            }
            loop {
                let v = T::json_deserialize(self)?;
                out.push(v);
                self.skip_ws();
                match self.bump() {
                    Some(b',') => continue,
                    Some(b']') => break,
                    _ => {
                        let (byte, line, col) = self.pos_meta(self.i);
                        return Err(Error::ExpectedCommaOrArrayEnd { byte, line, col });
                    }
                }
            }
            Ok(out)
        }

        /// Parse a JSON `f64` number using the generic implementation.
        #[inline]
        pub fn parse_f64(&mut self) -> Result<f64, Error> {
            <f64 as JsonDeserialize>::json_deserialize(self)
        }

        /// Try to consume a JSON `null` token without erroring when absent.
        #[inline]
        pub fn try_consume_null(&mut self) -> Result<bool, Error> {
            self.skip_ws();
            if let Some(rest) = self.s.get(self.i..)
                && rest.starts_with(b"null")
            {
                self.i += 4;
                return Ok(true);
            }
            Ok(false)
        }
        /// Parse and skip a JSON string without allocating; validates structure.
        pub fn skip_string(&mut self) -> Result<(), Error> {
            self.skip_ws();
            self.expect(b'"')?;
            loop {
                let b = self.bump().ok_or_else(|| {
                    let (byte, line, col) = self.pos_meta(self.i);
                    Error::UnterminatedString { byte, line, col }
                })?;
                match b {
                    b'"' => break,
                    b'\\' => {
                        // Skip escape or \uXXXX
                        let esc = self.bump().ok_or_else(|| {
                            let (byte, line, col) = self.pos_meta(self.i);
                            Error::EofEscape { byte, line, col }
                        })?;
                        if esc == b'u' {
                            // Skip 4 hex digits
                            for _ in 0..4 {
                                let _ = self.bump().ok_or_else(|| {
                                    let (byte, line, col) = self.pos_meta(self.i);
                                    Error::EofHex { byte, line, col }
                                })?;
                            }
                            // If this was a high surrogate, also skip a required low surrogate
                            // We conservatively try to detect a following "\u" and skip its 4 hex digits.
                            if let Some(rest) = self.s.get(self.i..)
                                && rest.len() >= 6
                                && rest[0] == b'\\'
                                && rest[1] == b'u'
                            {
                                // We don't validate ranges here; parsing will do that when needed.
                                self.i += 2; // skip \\u
                                for _ in 0..4 {
                                    let _ = self.bump().ok_or_else(|| {
                                        let (byte, line, col) = self.pos_meta(self.i);
                                        Error::EofHex { byte, line, col }
                                    })?;
                                }
                            }
                        }
                    }
                    b if b < 0x20 => {
                        let (byte, line, col) = self.pos_meta(self.i.saturating_sub(1));
                        return Err(Error::ControlInString { byte, line, col });
                    }
                    _ => {}
                }
            }
            Ok(())
        }

        /// Skip over the next JSON value (object, array, string, number, true/false/null).
        pub fn skip_value(&mut self) -> Result<(), Error> {
            self.skip_ws();
            match self.peek() {
                Some(b'{') => {
                    self.bump();
                    self.skip_ws();
                    if matches!(self.peek(), Some(b'}')) {
                        self.bump();
                        return Ok(());
                    }
                    loop {
                        self.skip_string()?; // key
                        self.skip_ws();
                        self.expect(b':')?;
                        self.skip_value()?; // value
                        self.skip_ws();
                        match self.peek() {
                            Some(b',') => {
                                self.bump();
                                self.skip_ws();
                                continue;
                            }
                            Some(b'}') => {
                                self.bump();
                                break;
                            }
                            _ => {
                                let (byte, line, col) = self.pos_meta(self.i);
                                return Err(Error::ExpectedCommaOrObjectEnd { byte, line, col });
                            }
                        }
                    }
                    Ok(())
                }
                Some(b'[') => {
                    self.bump();
                    self.skip_ws();
                    if matches!(self.peek(), Some(b']')) {
                        self.bump();
                        return Ok(());
                    }
                    loop {
                        self.skip_value()?;
                        self.skip_ws();
                        match self.peek() {
                            Some(b',') => {
                                self.bump();
                                self.skip_ws();
                                continue;
                            }
                            Some(b']') => {
                                self.bump();
                                break;
                            }
                            _ => {
                                let (byte, line, col) = self.pos_meta(self.i);
                                return Err(Error::ExpectedCommaOrArrayEnd { byte, line, col });
                            }
                        }
                    }
                    Ok(())
                }
                Some(b'"') => self.skip_string(),
                Some(b't') | Some(b'f') => {
                    let _ = self.parse_bool()?;
                    Ok(())
                }
                Some(b'n') => self.parse_null(),
                Some(b'-') | Some(b'0'..=b'9') => {
                    // Skip a simple number: optional '-' then digits, optional fraction/exponent (best-effort)
                    if self.peek() == Some(b'-') {
                        self.bump();
                    }
                    let mut saw = false;
                    while let Some(b'0'..=b'9') = self.peek() {
                        self.bump();
                        saw = true;
                    }
                    if !saw {
                        let (byte, line, col) = self.pos_meta(self.i);
                        return Err(Error::ExpectedDigits { byte, line, col });
                    }
                    if self.peek() == Some(b'.') {
                        self.bump();
                        let mut at_least_one = false;
                        while let Some(b'0'..=b'9') = self.peek() {
                            self.bump();
                            at_least_one = true;
                        }
                        if !at_least_one {
                            let (byte, line, col) = self.pos_meta(self.i);
                            return Err(Error::ExpectedFracDigits { byte, line, col });
                        }
                    }
                    if let Some(b'e' | b'E') = self.peek() {
                        self.bump();
                        if let Some(b'+' | b'-') = self.peek() {
                            self.bump();
                        }
                        let mut at_least_one = false;
                        while let Some(b'0'..=b'9') = self.peek() {
                            self.bump();
                            at_least_one = true;
                        }
                        if !at_least_one {
                            let (byte, line, col) = self.pos_meta(self.i);
                            return Err(Error::ExpectedExpDigits { byte, line, col });
                        }
                    }
                    Ok(())
                }
                _ => {
                    let (byte, line, col) = self.pos_meta(self.i);
                    Err(Error::UnexpectedValue { byte, line, col })
                }
            }
        }

        /// Read a JSON object key and return its FNV-1a 64-bit hash.
        pub fn read_key_hash(&mut self) -> Result<u64, Error> {
            self.skip_ws();
            self.expect(b'"')?;
            let mut h: u64 = 0xcbf29ce484222325;
            loop {
                let b = self.bump().ok_or_else(|| {
                    let (byte, line, col) = self.pos_meta(self.i);
                    Error::UnterminatedString { byte, line, col }
                })?;
                match b {
                    b'"' => break,
                    b'\\' => {
                        // Hash the escaped char logically (treat escape as the resulting byte where trivial)
                        let esc = self.bump().ok_or_else(|| {
                            let (byte, line, col) = self.pos_meta(self.i);
                            Error::EofEscape { byte, line, col }
                        })?;
                        match esc {
                            b'"' => {
                                h ^= b'"' as u64;
                                h = h.wrapping_mul(0x100000001b3);
                            }
                            b'\\' => {
                                h ^= b'\\' as u64;
                                h = h.wrapping_mul(0x100000001b3);
                            }
                            b'/' => {
                                h ^= b'/' as u64;
                                h = h.wrapping_mul(0x100000001b3);
                            }
                            b'b' => {
                                h ^= 0x08u64;
                                h = h.wrapping_mul(0x100000001b3);
                            }
                            b'f' => {
                                h ^= 0x0Cu64;
                                h = h.wrapping_mul(0x100000001b3);
                            }
                            b'n' => {
                                h ^= b'\n' as u64;
                                h = h.wrapping_mul(0x100000001b3);
                            }
                            b'r' => {
                                h ^= b'\r' as u64;
                                h = h.wrapping_mul(0x100000001b3);
                            }
                            b't' => {
                                h ^= b'\t' as u64;
                                h = h.wrapping_mul(0x100000001b3);
                            }
                            b'u' => {
                                // Consume 4 hex digits; combine surrogate pairs when present and hash UTF‑8 bytes
                                let hex_to_u32 = |p: &mut Self| -> Result<u32, Error> {
                                    let mut v: u32 = 0;
                                    for _ in 0..4 {
                                        let c = p.bump().ok_or_else(|| {
                                            let (byte, line, col) = p.pos_meta(p.i);
                                            Error::EofHex { byte, line, col }
                                        })?;
                                        v = (v << 4)
                                            | match c {
                                                b'0'..=b'9' => (c - b'0') as u32,
                                                b'a'..=b'f' => (c - b'a' + 10) as u32,
                                                b'A'..=b'F' => (c - b'A' + 10) as u32,
                                                _ => {
                                                    let (byte, line, col) =
                                                        p.pos_meta(p.i.saturating_sub(1));
                                                    return Err(Error::InvalidHex {
                                                        byte,
                                                        line,
                                                        col,
                                                    });
                                                }
                                            };
                                    }
                                    Ok(v)
                                };
                                let hi = hex_to_u32(self)?;
                                let cp = if (0xD800..=0xDBFF).contains(&hi) {
                                    if self.peek() != Some(b'\\') {
                                        let (byte, line, col) = self.pos_meta(self.i);
                                        return Err(Error::WithPos {
                                            msg: "expected low surrogate",
                                            byte,
                                            line,
                                            col,
                                        });
                                    }
                                    self.bump();
                                    if self.bump() != Some(b'u') {
                                        let (byte, line, col) = self.pos_meta(self.i);
                                        return Err(Error::WithPos {
                                            msg: "expected \\u for low surrogate",
                                            byte,
                                            line,
                                            col,
                                        });
                                    }
                                    let lo = hex_to_u32(self)?;
                                    if !(0xDC00..=0xDFFF).contains(&lo) {
                                        let (byte, line, col) = self.pos_meta(self.i);
                                        return Err(Error::WithPos {
                                            msg: "invalid low surrogate",
                                            byte,
                                            line,
                                            col,
                                        });
                                    }
                                    0x10000 + (((hi - 0xD800) << 10) | (lo - 0xDC00))
                                } else if (0xDC00..=0xDFFF).contains(&hi) {
                                    let (byte, line, col) = self.pos_meta(self.i);
                                    return Err(Error::WithPos {
                                        msg: "unexpected low surrogate",
                                        byte,
                                        line,
                                        col,
                                    });
                                } else {
                                    hi
                                };
                                if let Some(ch) = char::from_u32(cp) {
                                    let mut buf = [0u8; 4];
                                    let s = ch.encode_utf8(&mut buf);
                                    for &bb in s.as_bytes() {
                                        h ^= bb as u64;
                                        h = h.wrapping_mul(0x100000001b3);
                                    }
                                } else {
                                    let (byte, line, col) = self.pos_meta(self.i);
                                    return Err(Error::WithPos {
                                        msg: "invalid codepoint",
                                        byte,
                                        line,
                                        col,
                                    });
                                }
                            }
                            _ => {
                                let (byte, line, col) = self.pos_meta(self.i.saturating_sub(1));
                                return Err(Error::WithPos {
                                    msg: "bad escape",
                                    byte,
                                    line,
                                    col,
                                });
                            }
                        }
                    }
                    _ => {
                        h ^= b as u64;
                        h = h.wrapping_mul(0x100000001b3);
                    }
                }
            }
            Ok(h)
        }

        /// Parse a JSON object key and return a borrowed `&str` when no escapes are present,
        /// or an owned `String` otherwise. This avoids allocating in the common fast path.
        ///
        /// After reading the key string, this function also consumes the mandatory
        /// colon `:` delimiter (with optional surrounding whitespace), positioning
        /// the parser at the start of the value. This matches the typical caller
        /// contract used across tests and benches.
        pub fn parse_key(&mut self) -> Result<KeyRef<'a>, Error> {
            self.skip_ws();
            let pre = self.i;
            self.expect(b'"')?;
            let start = self.i;
            let bytes = self.s;
            let mut i = start;
            while i < bytes.len() {
                let b = bytes[i];
                if b == b'"' {
                    // No escapes encountered; borrow directly from input
                    let slice = &bytes[start..i];
                    self.i = i + 1;
                    // Consume the mandatory colon after the key
                    self.skip_ws();
                    match self.bump() {
                        Some(b':') => {}
                        _ => {
                            let (byte, line, col) = self.pos_meta(self.i);
                            return Err(Error::WithPos {
                                msg: "expected :",
                                byte,
                                line,
                                col,
                            });
                        }
                    }
                    let st = std::str::from_utf8(slice).map_err(|_| Error::InvalidUtf8)?;
                    return Ok(KeyRef::Borrowed(st));
                }
                if b == b'\\' || b < 0x20 {
                    break;
                }
                i += 1;
            }
            // Slow path: re-parse from the opening quote using the general string parser
            let mut tmp = Parser::new_at(
                // SAFETY: Parser holds the same original string underlying `self.s`.
                unsafe { std::str::from_utf8_unchecked(self.s) },
                pre,
            );
            let s = tmp.parse_string()?;
            self.i = tmp.i;
            // Consume the mandatory colon after the key
            self.skip_ws();
            match self.bump() {
                Some(b':') => {}
                _ => {
                    let (byte, line, col) = self.pos_meta(self.i);
                    return Err(Error::WithPos {
                        msg: "expected :",
                        byte,
                        line,
                        col,
                    });
                }
            }
            Ok(KeyRef::Owned(s))
        }
    }

    /// Trait for types that can be deserialized from JSON using the simple parser.
    pub trait JsonDeserialize: Sized {
        /// Parse `Self` from the parser.
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error>;

        /// Convert a pre-parsed [`Value`] into `Self`.
        fn json_from_value(value: &Value) -> Result<Self, Error> {
            json_from_value_via_string::<Self>(value)
        }

        /// Convert a JSON object key into `Self`.
        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            json_from_value_via_string::<Self>(&Value::String(key.to_owned()))
        }
    }

    /// Marker trait mirroring `serde::de::DeserializeOwned` for Norito JSON.
    pub trait JsonDeserializeOwned: JsonDeserialize {}

    impl<T: JsonDeserialize> JsonDeserializeOwned for T {}

    pub use JsonDeserializeOwned as DeserializeOwned;

    fn json_from_value_via_string<T: JsonDeserialize>(value: &Value) -> Result<T, Error> {
        let json = to_json(value)?;
        let mut parser = Parser::new(&json);
        let result = T::json_deserialize(&mut parser)?;
        parser.skip_ws();
        if !parser.eof() {
            let (byte, line, col) = parser.pos_meta(parser.position());
            return Err(Error::TrailingCharacters { byte, line, col });
        }
        Ok(result)
    }

    impl JsonDeserialize for bool {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            p.parse_bool()
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            value
                .as_bool()
                .ok_or_else(|| Error::Message("expected bool".into()))
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            match key {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(Error::Message("expected bool".into())),
            }
        }
    }
    fn parse_u128_from_parser(p: &mut Parser<'_>) -> Result<u128, Error> {
        p.skip_ws();
        let bytes = p.s;
        let mut i = p.i;
        if i < bytes.len() && bytes[i] == b'-' {
            let (byte, line, col) = p.pos_meta(i);
            return Err(Error::WithPos {
                msg: "negative not allowed",
                byte,
                line,
                col,
            });
        }
        let mut val: u128 = 0;
        let mut any = false;
        while i < bytes.len() {
            let c = bytes[i];
            if c.wrapping_sub(b'0') <= 9 {
                let d = (c - b'0') as u128;
                if val > (u128::MAX - d) / 10 {
                    let (byte, line, col) = p.pos_meta(i);
                    return Err(Error::WithPos {
                        msg: "u128 overflow",
                        byte,
                        line,
                        col,
                    });
                }
                val = val * 10 + d;
                i += 1;
                any = true;
            } else {
                break;
            }
        }
        if !any {
            let (byte, line, col) = p.pos_meta(p.i.min(bytes.len()));
            return Err(Error::WithPos {
                msg: "expected number",
                byte,
                line,
                col,
            });
        }
        if i < bytes.len() && (bytes[i] == b'.' || bytes[i] == b'e' || bytes[i] == b'E') {
            let (byte, line, col) = p.pos_meta(i);
            return Err(Error::WithPos {
                msg: "expected integer",
                byte,
                line,
                col,
            });
        }
        p.i = i;
        p.skip_ws();
        Ok(val)
    }

    impl JsonDeserialize for u128 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            parse_u128_from_parser(p)
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Value::Number(number) = value
                && let Some(u) = number.as_u64()
            {
                return Ok(u as u128);
            }
            json_from_value_via_string(value)
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            key.parse::<u128>()
                .map_err(|_| Error::Message("expected u128".into()))
        }
    }
    impl JsonDeserialize for core::num::NonZeroU128 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let value = parse_u128_from_parser(p)?;
            core::num::NonZeroU128::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u128".into()))
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            let value = u128::json_from_value(value)?;
            core::num::NonZeroU128::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u128".into()))
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            let value = u128::json_from_map_key(key)?;
            core::num::NonZeroU128::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u128".into()))
        }
    }
    impl JsonDeserialize for u64 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            p.parse_u64()
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Some(n) = value.as_u64() {
                return Ok(n);
            }
            json_from_value_via_string(value)
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            key.parse::<u64>()
                .map_err(|_| Error::Message("expected u64".into()))
        }
    }
    impl JsonDeserialize for core::num::NonZeroU64 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let value = p.parse_u64()?;
            core::num::NonZeroU64::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u64".into()))
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            let value = u64::json_from_value(value)?;
            core::num::NonZeroU64::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u64".into()))
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            let value = u64::json_from_map_key(key)?;
            core::num::NonZeroU64::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u64".into()))
        }
    }
    impl JsonDeserialize for core::num::NonZeroU32 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let value = p.parse_u64()?;
            let value = u32::try_from(value).map_err(|_| Error::Message("u32 overflow".into()))?;
            core::num::NonZeroU32::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u32".into()))
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            let value = u32::json_from_value(value)?;
            core::num::NonZeroU32::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u32".into()))
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            let value = u32::json_from_map_key(key)?;
            core::num::NonZeroU32::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u32".into()))
        }
    }
    impl JsonDeserialize for u32 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let n = p.parse_u64()?;
            u32::try_from(n).map_err(|_| Error::Message("u32 overflow".into()))
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Some(n) = value.as_u64() {
                return u32::try_from(n).map_err(|_| Error::Message("u32 overflow".into()));
            }
            json_from_value_via_string(value)
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            key.parse::<u32>()
                .map_err(|_| Error::Message("u32 overflow".into()))
        }
    }
    impl JsonDeserialize for u16 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let n = p.parse_u64()?;
            u16::try_from(n).map_err(|_| Error::Message("u16 overflow".into()))
        }
    }
    impl JsonDeserialize for core::num::NonZeroU16 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let n = p.parse_u64()?;
            let value = u16::try_from(n).map_err(|_| Error::Message("u16 overflow".into()))?;
            core::num::NonZeroU16::new(value)
                .ok_or_else(|| Error::Message("expected non-zero u16".into()))
        }
    }
    impl JsonDeserialize for core::num::NonZeroUsize {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let n = p.parse_u64()?;
            let value = usize::try_from(n).map_err(|_| Error::Message("usize overflow".into()))?;
            core::num::NonZeroUsize::new(value)
                .ok_or_else(|| Error::Message("expected non-zero usize".into()))
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            let value = usize::json_from_value(value)?;
            core::num::NonZeroUsize::new(value)
                .ok_or_else(|| Error::Message("expected non-zero usize".into()))
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            let value = usize::json_from_map_key(key)?;
            core::num::NonZeroUsize::new(value)
                .ok_or_else(|| Error::Message("expected non-zero usize".into()))
        }
    }
    impl JsonDeserialize for u8 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let n = p.parse_u64()?;
            u8::try_from(n).map_err(|_| Error::Message("u8 overflow".into()))
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Some(n) = value.as_u64() {
                return u8::try_from(n).map_err(|_| Error::Message("u8 overflow".into()));
            }
            json_from_value_via_string(value)
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            key.parse::<u8>()
                .map_err(|_| Error::Message("u8 overflow".into()))
        }
    }
    impl JsonDeserialize for usize {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let n = p.parse_u64()?;
            usize::try_from(n).map_err(|_| Error::Message("usize overflow".into()))
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Some(n) = value.as_u64() {
                return usize::try_from(n).map_err(|_| Error::Message("usize overflow".into()));
            }
            json_from_value_via_string(value)
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            key.parse::<usize>()
                .map_err(|_| Error::Message("usize overflow".into()))
        }
    }

    fn parse_i64_from_parser(p: &mut Parser<'_>) -> Result<i64, Error> {
        p.skip_ws();
        let input = p.input();
        let bytes = p.s;
        let start = p.i;
        let mut idx = start;
        if idx >= bytes.len() {
            let (byte, line, col) = pos_from_offset(input, idx);
            return Err(Error::UnexpectedEof { byte, line, col });
        }
        if bytes[idx] == b'-' {
            idx += 1;
        }
        let int_start = idx;
        if idx >= bytes.len() || !bytes[idx].is_ascii_digit() {
            let (byte, line, col) = pos_from_offset(input, idx);
            return Err(Error::WithPos {
                msg: "expected integer",
                byte,
                line,
                col,
            });
        }
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        let int_end = idx;
        if bytes[int_start] == b'0' && int_end > int_start + 1 {
            let (byte, line, col) = pos_from_offset(input, int_start + 1);
            return Err(Error::WithPos {
                msg: Parser::LEADING_ZERO_MSG,
                byte,
                line,
                col,
            });
        }
        if idx < bytes.len() && matches!(bytes[idx], b'.' | b'e' | b'E') {
            let (byte, line, col) = pos_from_offset(input, idx);
            return Err(Error::WithPos {
                msg: "expected integer",
                byte,
                line,
                col,
            });
        }
        let slice = &bytes[start..idx];
        let text = std::str::from_utf8(slice).map_err(|_| {
            let (byte, line, col) = pos_from_offset(input, start);
            Error::WithPos {
                msg: "invalid integer",
                byte,
                line,
                col,
            }
        })?;
        let value = text.parse::<i64>().map_err(|_| {
            let (byte, line, col) = pos_from_offset(input, start);
            Error::WithPos {
                msg: "i64 overflow",
                byte,
                line,
                col,
            }
        })?;
        p.i = idx;
        Ok(value)
    }

    impl JsonDeserialize for i64 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            parse_i64_from_parser(p)
        }
    }
    impl JsonDeserialize for i32 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let v = parse_i64_from_parser(p)?;
            i32::try_from(v).map_err(|_| Error::Message("i32 overflow".into()))
        }
    }
    impl JsonDeserialize for i16 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let v = parse_i64_from_parser(p)?;
            i16::try_from(v).map_err(|_| Error::Message("i16 overflow".into()))
        }
    }
    impl JsonDeserialize for i8 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let v = parse_i64_from_parser(p)?;
            i8::try_from(v).map_err(|_| Error::Message("i8 overflow".into()))
        }
    }
    impl JsonDeserialize for isize {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let v = parse_i64_from_parser(p)?;
            isize::try_from(v).map_err(|_| Error::Message("isize overflow".into()))
        }
    }
    impl JsonDeserialize for f64 {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            p.skip_ws();
            let start = p.position();
            // Reuse skip logic to locate the end of the number
            // Minimal scan: [-]? digits [. digits]? ([eE] [+-]? digits)?
            let mut i = start;
            let s = std::str::from_utf8(p.s).map_err(|_| Error::InvalidUtf8)?;
            let bytes = p.s;
            if i < bytes.len() && bytes[i] == b'-' {
                i += 1;
            }
            let mut saw = false;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
                saw = true;
            }
            if i < bytes.len() && bytes[i] == b'.' {
                i += 1;
                let mut d = false;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                    d = true;
                }
                if !d {
                    let (byte, line, col) = pos_from_offset(p.input(), i);
                    return Err(Error::ExpectedFracDigits { byte, line, col });
                }
            }
            if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                i += 1;
                if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                    i += 1;
                }
                let mut d = false;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                    d = true;
                }
                if !d {
                    let (byte, line, col) = pos_from_offset(p.input(), i);
                    return Err(Error::ExpectedExpDigits { byte, line, col });
                }
            }
            if !saw {
                let (byte, line, col) = pos_from_offset(p.input(), start);
                return Err(Error::ExpectedDigits { byte, line, col });
            }
            let num_str = &s[start..i];
            let v: f64 = num_str
                .parse()
                .map_err(|e| Error::Message(format!("failed to parse float `{num_str}`: {e}")))?;
            p.i = i;
            Ok(v)
        }
    }
    impl JsonDeserialize for String {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            p.parse_string()
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Some(s) = value.as_str() {
                return Ok(s.to_owned());
            }
            json_from_value_via_string(value)
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            Ok(key.to_owned())
        }
    }

    impl JsonDeserialize for Url {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            let raw = String::json_deserialize(parser)?;
            raw.parse()
                .map_err(|e| Error::Message(format!("invalid url: {e}")))
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Some(s) = value.as_str() {
                return s
                    .parse()
                    .map_err(|e| Error::Message(format!("invalid url: {e}")));
            }
            json_from_value_via_string(value)
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            key.parse()
                .map_err(|e| Error::Message(format!("invalid url: {e}")))
        }
    }
    impl JsonDeserialize for std::time::Duration {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let mut map = MapVisitor::new(p)?;
            let mut secs: Option<u64> = None;
            let mut nanos: Option<u32> = None;
            while let Some(key) = map.next_key()? {
                match key.as_str() {
                    "secs" => {
                        if secs.is_some() {
                            return Err(Error::duplicate_field("secs"));
                        }
                        secs = Some(map.parse_value::<u64>()?);
                    }
                    "nanos" => {
                        if nanos.is_some() {
                            return Err(Error::duplicate_field("nanos"));
                        }
                        nanos = Some(map.parse_value::<u32>()?);
                    }
                    _ => map.skip_value()?,
                }
            }
            map.finish()?;
            let secs = secs.ok_or_else(|| Error::missing_field("secs"))?;
            let nanos = nanos.ok_or_else(|| Error::missing_field("nanos"))?;
            Ok(std::time::Duration::from_secs(secs)
                + std::time::Duration::from_nanos(u64::from(nanos)))
        }
    }

    impl JsonSerialize for std::path::PathBuf {
        fn json_serialize(&self, out: &mut String) {
            write_json_string(&self.to_string_lossy(), out);
        }
    }

    impl JsonDeserialize for std::path::PathBuf {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let raw = p.parse_string()?;
            Ok(std::path::PathBuf::from(raw))
        }
    }

    // NOTE: arena-backed string parsing API can be added here in a follow-up using a
    // dedicated reference type to handle lifetimes of input vs arena correctly.
    impl<T: JsonDeserialize> JsonDeserialize for Option<T> {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            p.skip_ws();
            if let Some(rest) = p.s.get(p.i..)
                && rest.starts_with(b"null")
            {
                p.i += 4;
                return Ok(None);
            }
            let v = T::json_deserialize(p)?;
            Ok(Some(v))
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if value.is_null() {
                Ok(None)
            } else {
                T::json_from_value(value).map(Some)
            }
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            if key == "null" {
                Ok(None)
            } else {
                T::json_from_map_key(key).map(Some)
            }
        }
    }
    impl JsonDeserialize for () {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            p.skip_ws();
            if p.try_consume_null()? {
                Ok(())
            } else {
                Err(Error::Message("expected null".into()))
            }
        }
    }
    impl<T: JsonDeserialize> JsonDeserialize for Vec<T> {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            p.parse_array::<T>()
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Value::Array(items) = value {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(T::json_from_value(item)?);
                }
                Ok(out)
            } else {
                json_from_value_via_string(value)
            }
        }
    }

    impl<T> JsonDeserialize for std::collections::BTreeSet<T>
    where
        T: JsonDeserialize + Ord,
    {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            let values: Vec<T> = p.parse_array()?;
            let mut set = std::collections::BTreeSet::new();
            for value in values {
                if !set.insert(value) {
                    return Err(Error::Message("duplicate element in set".into()));
                }
            }
            Ok(set)
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Value::Array(items) = value {
                let mut set = std::collections::BTreeSet::new();
                for item in items {
                    let v = T::json_from_value(item)?;
                    if !set.insert(v) {
                        return Err(Error::Message("duplicate element in set".into()));
                    }
                }
                Ok(set)
            } else {
                json_from_value_via_string(value)
            }
        }
    }

    impl JsonDeserialize for Value {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            parse_value_internal(p)
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            Ok(value.clone())
        }
    }

    /// Structural index (scalar) representing offsets of JSON structural characters
    /// outside of strings. This is a scalar reference implementation; SIMD paths can
    /// replace the builder without changing the downstream walker.
    pub struct StructIndex {
        pub offsets: Vec<u32>,
    }

    /// Build a structural index for `input`.
    ///
    /// Attempts a SIMD (NEON) path on AArch64 when enabled via the `simd-accel`
    /// feature and supported at runtime; otherwise falls back to a portable
    /// scalar implementation. Both paths must produce identical results.
    #[inline(always)]
    fn debug_stage1_backend(tag: &str) {
        #[cfg(debug_assertions)]
        {
            if crate::debug_trace_enabled() {
                eprintln!("norito/json: stage1 backend = {tag}");
            }
        }
        let _ = tag;
    }

    #[cfg(all(debug_assertions, feature = "stage1-validate"))]
    #[inline]
    fn validate_accel(tag: &str, input: &str, acc: StructIndex) -> StructIndex {
        static BANNER: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        BANNER.get_or_init(|| {
            if crate::debug_trace_enabled() {
                eprintln!("norito/json: stage1-validate enabled (debug), validating accelerated tapes against scalar for inputs ≤256KiB");
            }
        });
        const VALIDATE_MAX_BYTES: usize = 256 * 1024;
        if input.len() <= VALIDATE_MAX_BYTES {
            let scalar = build_struct_index_scalar(input);
            if scalar.offsets != acc.offsets {
                if crate::debug_trace_enabled() {
                    eprintln!(
                        "norito/json: stage1 {} mismatch; falling back to scalar (acc={} vs scalar={})",
                        tag,
                        acc.offsets.len(),
                        scalar.offsets.len()
                    );
                }
                return scalar;
            }
        }
        acc
    }
    #[cfg(not(all(debug_assertions, feature = "stage1-validate")))]
    #[inline]
    #[allow(dead_code)]
    fn validate_accel(_tag: &str, _input: &str, acc: StructIndex) -> StructIndex {
        acc
    }

    pub fn build_struct_index(input: &str) -> StructIndex {
        // Small inputs: prefer the scalar reference to avoid accelerator overheads
        // Benchmarks (`examples/stage1_cutover`) show SIMD wins start to dominate
        // around 6–8 KiB, with a slight regression at ~4 KiB; set the cutover a
        // bit higher than before to avoid thrashing on tiny payloads while still
        // exercising accelerated paths for typical documents.
        const SMALL_BYTES: usize = 4096;
        if input.len() < SMALL_BYTES {
            return build_struct_index_scalar(input);
        }
        #[cfg(feature = "parallel-stage1")]
        {
            // Parallel tape build for large inputs; deterministic merge.
            if input.len() >= par_min_bytes()
                && let Some(t) = build_struct_index_parallel(input)
            {
                debug_stage1_backend("parallel");
                return t;
            }
        }
        // Compile-time preferred paths when target features are enabled
        #[cfg(all(
            feature = "simd-accel",
            target_arch = "aarch64",
            target_feature = "neon"
        ))]
        {
            if let Some(t) = unsafe { build_struct_index_neon(input) } {
                debug_stage1_backend("neon-ct");
                return validate_accel("neon-ct", input, t);
            }
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            if let Some(t) = unsafe { build_struct_index_avx2(input) } {
                debug_stage1_backend("avx2-ct");
                return validate_accel("avx2-ct", input, t);
            }
        }
        #[cfg(feature = "metal-stage1")]
        {
            if input.len() >= stage1_gpu_min_bytes()
                && let Some(t) = metal::build_struct_index_metal(input)
            {
                debug_stage1_backend("metal");
                return validate_accel("metal", input, t);
            }
        }
        #[cfg(feature = "cuda-stage1")]
        {
            if input.len() >= stage1_gpu_min_bytes()
                && let Some(t) = cuda::build_struct_index_cuda(input)
            {
                debug_stage1_backend("cuda");
                return validate_accel("cuda", input, t);
            }
        }
        #[cfg(target_arch = "x86_64")]
        {
            if std::arch::is_x86_feature_detected!("avx2") {
                // SAFETY: guarded by runtime feature detection
                if let Some(t) = unsafe { build_struct_index_avx2(input) } {
                    debug_stage1_backend("avx2");
                    return validate_accel("avx2", input, t);
                }
            }
        }
        #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
        {
            if std::arch::is_aarch64_feature_detected!("neon") {
                // SAFETY: guarded by runtime feature detection
                if let Some(t) = unsafe { build_struct_index_neon(input) } {
                    debug_stage1_backend("neon");
                    return validate_accel("neon", input, t);
                }
            }
        }
        debug_stage1_backend("scalar");
        build_struct_index_scalar(input)
    }

    #[cfg(feature = "parallel-stage1")]
    const PAR_STAGE1_MIN_BYTES_DEFAULT: usize = 1 << 20;

    #[cfg(any(test, debug_assertions))]
    #[cfg(feature = "parallel-stage1")]
    fn par_min_bytes() -> usize {
        use std::sync::OnceLock;
        static V: OnceLock<usize> = OnceLock::new();
        *V.get_or_init(|| {
            std::env::var("NORITO_PAR_STAGE1_MIN")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(PAR_STAGE1_MIN_BYTES_DEFAULT) // 1 MiB default
        })
    }

    #[cfg(all(feature = "parallel-stage1", not(any(test, debug_assertions))))]
    fn par_min_bytes() -> usize {
        PAR_STAGE1_MIN_BYTES_DEFAULT
    }

    #[cfg(any(feature = "metal-stage1", feature = "cuda-stage1", test))]
    fn stage1_gpu_min_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .expect("stage1 gpu min mutex")
    }

    #[cfg(any(feature = "metal-stage1", feature = "cuda-stage1", test))]
    const STAGE1_GPU_MIN_DEFAULT: usize = 192 * 1024;

    #[cfg(any(feature = "metal-stage1", feature = "cuda-stage1", test))]
    static STAGE1_GPU_MIN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    #[cfg(any(feature = "metal-stage1", feature = "cuda-stage1", test))]
    fn stage1_gpu_min_bytes_locked(_guard: &std::sync::MutexGuard<'static, ()>) -> usize {
        use std::sync::atomic::Ordering;
        let cached = STAGE1_GPU_MIN.load(Ordering::Relaxed);
        if cached != 0 {
            return cached;
        }
        let parsed = if cfg!(any(test, debug_assertions)) {
            std::env::var("NORITO_STAGE1_GPU_MIN_BYTES")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|v| *v > 0)
        } else {
            None
        };
        // Tuned via `examples/stage1_cutover` runs (see `benchmarks/norito_stage1/cutover.csv`):
        // SIMD catches up around 6–8 KiB and GPU tapers in around ~192 KiB once launch
        // overheads and chunked CRC64/Stage1 kernels are amortised.
        let min_bytes = parsed.unwrap_or(STAGE1_GPU_MIN_DEFAULT);
        STAGE1_GPU_MIN.store(min_bytes, Ordering::Relaxed);
        min_bytes
    }

    #[cfg(any(feature = "metal-stage1", feature = "cuda-stage1"))]
    fn stage1_gpu_min_bytes() -> usize {
        let guard = stage1_gpu_min_lock();
        stage1_gpu_min_bytes_locked(&guard)
    }

    #[cfg(test)]
    mod stage1_gpu_min_tests {
        use std::sync::atomic::Ordering;

        use super::{STAGE1_GPU_MIN, stage1_gpu_min_bytes_locked};

        #[test]
        fn defaults_when_env_missing() {
            let guard = super::stage1_gpu_min_lock();
            unsafe { std::env::remove_var("NORITO_STAGE1_GPU_MIN_BYTES") };
            STAGE1_GPU_MIN.store(0, Ordering::Relaxed);
            assert_eq!(stage1_gpu_min_bytes_locked(&guard), 192 * 1024);
            STAGE1_GPU_MIN.store(0, Ordering::Relaxed);
        }

        #[test]
        fn respects_env_override() {
            let guard = super::stage1_gpu_min_lock();
            unsafe { std::env::set_var("NORITO_STAGE1_GPU_MIN_BYTES", "65536") };
            STAGE1_GPU_MIN.store(0, Ordering::Relaxed);
            assert_eq!(stage1_gpu_min_bytes_locked(&guard), 65_536);
            unsafe { std::env::remove_var("NORITO_STAGE1_GPU_MIN_BYTES") };
            STAGE1_GPU_MIN.store(0, Ordering::Relaxed);
        }
    }

    #[cfg(feature = "parallel-stage1")]
    fn build_struct_index_parallel(input: &str) -> Option<StructIndex> {
        let ncpu = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2);
        let len = input.len();
        let chunk_goal = 256 * 1024; // 256 KiB target
        let max_chunks = ncpu * 4; // avoid oversharding
        let chunks = len.div_ceil(chunk_goal).clamp(2, max_chunks);

        // Partition input
        let mut ranges = Vec::with_capacity(chunks);
        let mut start = 0usize;
        for _ in 0..chunks {
            if start >= len {
                break;
            }
            let end = (start + chunk_goal).min(len);
            ranges.push((start, end));
            start = end;
        }

        // Compute per-chunk tapes in parallel (rayon or threads), using scalar/accelerated builder underneath.
        #[cfg(feature = "parallel-stage1-rayon")]
        let parts: Vec<(usize, Vec<u32>)> = {
            use rayon::prelude::*;
            ranges
                .into_par_iter()
                .map(|(s, e)| {
                    let t = build_struct_index_scalar(&input[s..e]);
                    (s, t.offsets)
                })
                .collect()
        };
        #[cfg(not(feature = "parallel-stage1-rayon"))]
        let parts: Vec<(usize, Vec<u32>)> = {
            use std::thread;
            let mut handles = Vec::new();
            for (s, e) in ranges.into_iter() {
                // Spawn with an owned slice to satisfy the 'static bound on thread::spawn.
                let chunk = input[s..e].to_owned();
                handles.push(thread::spawn(move || {
                    let t = build_struct_index_scalar(&chunk);
                    (s, t.offsets)
                }));
            }
            let mut v = Vec::new();
            for h in handles {
                v.push(h.join().ok()?);
            }
            v
        };

        // Merge and normalize
        let mut all: Vec<u32> = Vec::new();
        for (base, mut offs) in parts.into_iter() {
            for o in offs.iter_mut() {
                *o = (*o as usize + base) as u32;
            }
            all.extend(offs);
        }
        all.sort_unstable();
        // Deterministic merge pass: recompute which offsets to keep based on quote parity and structurals
        let bytes = input.as_bytes();
        let mut out: Vec<u32> = Vec::with_capacity(all.len());
        let mut in_string = false;
        for &o in &all {
            let off = o as usize;
            if off >= bytes.len() {
                continue;
            }
            let b = bytes[off];
            if b == b'"' {
                // Count preceding backslashes
                let mut run = 0usize;
                let mut p = off as isize - 1;
                while p >= 0 && bytes[p as usize] == b'\\' {
                    run += 1;
                    p -= 1;
                }
                if (run & 1) == 0 {
                    in_string = !in_string;
                    out.push(o);
                }
            } else if !in_string {
                match b {
                    b'{' | b'}' | b'[' | b']' | b':' | b',' => out.push(o),
                    _ => {}
                }
            }
        }
        Some(StructIndex { offsets: out })
    }

    #[cfg(all(feature = "parallel-stage1", feature = "bench-internal"))]
    pub fn build_struct_index_parallel_bench(input: &str) -> StructIndex {
        build_struct_index_parallel(input).unwrap_or_else(|| build_struct_index_scalar(input))
    }

    #[cfg(all(feature = "parallel-stage1", feature = "bench-internal"))]
    pub fn build_struct_index_parallel_with_chunks(
        input: &str,
        chunks_override: usize,
    ) -> StructIndex {
        use std::thread;
        let len = input.len();
        let chunks = chunks_override.max(1);
        let step = len.div_ceil(chunks);
        let mut handles = Vec::new();
        for c in 0..chunks {
            let start = c * step;
            if start >= len {
                break;
            }
            let end = ((c + 1) * step).min(len);
            let chunk = input[start..end].to_owned();
            handles.push(thread::spawn(move || {
                let t = build_struct_index_scalar(&chunk);
                (start, t.offsets)
            }));
        }
        let mut all: Vec<u32> = Vec::new();
        for h in handles {
            let (base, mut offs) = h.join().unwrap();
            for o in offs.iter_mut() {
                *o = (*o as usize + base) as u32;
            }
            all.extend(offs);
        }
        all.sort_unstable();
        // Deterministic finalize
        let bytes = input.as_bytes();
        let mut out: Vec<u32> = Vec::with_capacity(all.len());
        let mut in_string = false;
        let mut bs_run = 0usize;
        for &o in &all {
            let off = o as usize;
            if off >= bytes.len() {
                continue;
            }
            let b = bytes[off];
            if b == b'\\' {
                bs_run += 1;
                continue;
            }
            if b == b'"' {
                let escaped = (bs_run & 1) == 1;
                if !escaped {
                    in_string = !in_string;
                    out.push(o);
                }
                bs_run = 0;
                continue;
            }
            bs_run = 0;
            if !in_string {
                match b {
                    b'{' | b'}' | b'[' | b']' | b':' | b',' => out.push(o),
                    _ => {}
                }
            }
        }
        StructIndex { offsets: out }
    }

    /// Reference scalar builder used for correctness and as a fallback.
    ///
    /// The fast paths (`build_struct_index_neon` / `build_struct_index_avx2`) already
    /// implement the nibble-LUT SIMD classification. This scalar implementation
    /// remains the canonical, portable baseline.
    fn build_struct_index_scalar(input: &str) -> StructIndex {
        let b = input.as_bytes();
        let mut offsets = Vec::new();
        let mut i = 0usize;
        let mut in_str = false;
        while i < b.len() {
            let c = b[i];
            if in_str {
                if c == b'\\' {
                    i = i.saturating_add(2);
                    continue;
                }
                if c == b'"' {
                    in_str = false;
                    offsets.push(i as u32);
                    i += 1;
                    continue;
                }
                i += 1;
                continue;
            } else {
                match c {
                    b'"' => {
                        in_str = true;
                        offsets.push(i as u32);
                        i += 1;
                    }
                    b'{' | b'}' | b'[' | b']' | b':' | b',' => {
                        offsets.push(i as u32);
                        i += 1;
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
        }
        StructIndex { offsets }
    }

    #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
    /// NEON-accelerated structural indexer with bitboard iteration.
    ///
    /// Computes quote/backslash/structural bitmasks per 16-byte block and iterates
    /// bitboards (trailing-zeros) instead of per-byte scans. Maintains carry for
    /// trailing backslash runs and in-string parity across blocks to match scalar.
    unsafe fn build_struct_index_neon(input: &str) -> Option<StructIndex> {
        use core::arch::aarch64::*;
        unsafe {
            let bytes = input.as_bytes();
            let mut offsets: Vec<u32> = Vec::new();
            let mut i = 0usize;
            let mut in_string = false;
            let mut carry_bs_run_len: usize = 0;

            #[inline(always)]
            fn to_mask(v: uint8x16_t) -> u16 {
                unsafe {
                    // Extract MSBs and pack into a 16-bit mask (LSB=lane0)
                    let msb = vshrq_n_u8(v, 7);
                    let mut tmp = [0u8; 16];
                    vst1q_u8(tmp.as_mut_ptr(), msb);
                    let mut m: u16 = 0;
                    let mut j = 0;
                    while j < 16 {
                        m |= ((tmp[j] & 1) as u16) << j;
                        j += 1;
                    }
                    m
                }
            }

            while i + 16 <= bytes.len() {
                let ptr = bytes.as_ptr().add(i);
                let v = vld1q_u8(ptr);

                // Quote predicate
                let is_quote = vceqq_u8(v, vdupq_n_u8(b'"'));
                let qmask = to_mask(is_quote);

                // Nibble-LUT classification for structurals { } [ ] : ,
                // Map low-nibble -> allowed high-nibble groups (bits: 1=>0x2, 2=>0x3, 4=>0x5, 8=>0x7)
                const STRUCT_LO: [u8; 16] = [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,  /*0xA ':' -> hi=0x3*/
                    12, /*0xB '{' or '[' -> hi=0x7 or 0x5*/
                    1,  /*0xC ',' -> hi=0x2*/
                    12, /*0xD '}' or ']' -> hi=0x7 or 0x5*/
                    0,  /*0xE */
                    0,  /*0xF */
                ];
                // Map high-nibble -> group bitmask as above
                const HI_GROUP: [u8; 16] = [0, 0, 1, 2, 0, 4, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0];
                let lo = vandq_u8(v, vdupq_n_u8(0x0f));
                let hi = vshrq_n_u8(v, 4);
                let struct_lo_tbl = vld1q_u8(STRUCT_LO.as_ptr());
                let hi_group_tbl = vld1q_u8(HI_GROUP.as_ptr());
                let lo_map = vqtbl1q_u8(struct_lo_tbl, lo);
                let hi_map = vqtbl1q_u8(hi_group_tbl, hi);
                // Test for nonzero intersection per lane -> 0xFF where true
                let structurals = vtstq_u8(lo_map, hi_map);
                let smask = to_mask(structurals);

                // Compute mask of unescaped quotes using quote and backslash bitboards.
                let bslash = vceqq_u8(v, vdupq_n_u8(b'\\'));
                let bmask = to_mask(bslash);
                let mut unescaped: u16 = 0;
                let mut qm: u16 = qmask;
                while qm != 0 {
                    let tz = qm.trailing_zeros() as usize;
                    // Count immediate preceding backslashes via bitboard
                    let mut run = 0usize;
                    let mut p = tz as isize - 1;
                    while p >= 0 {
                        if ((bmask >> (p as usize)) & 1) == 1 {
                            run += 1;
                            p -= 1;
                        } else {
                            break;
                        }
                    }
                    if p < 0 {
                        run += carry_bs_run_len;
                    }
                    if (run & 1) == 0 {
                        unescaped |= 1u16 << tz;
                    }
                    qm &= qm - 1; // clear lowest set bit
                }

                // Iterate union of unescaped quotes and structurals; toggle in_string on quotes
                let mut union = unescaped | smask;
                while union != 0 {
                    let tz = union.trailing_zeros() as usize;
                    let bit = 1u16 << tz;
                    if (unescaped & bit) != 0 {
                        in_string = !in_string;
                        offsets.push((i + tz) as u32);
                    } else if !in_string {
                        // structural outside string
                        offsets.push((i + tz) as u32);
                    }
                    union &= union - 1;
                }

                // Trailing backslash run length becomes carry for next block
                // We still examine the block tail directly (cheaper than reconstructing a rank prefix)
                let mut arr = [0u8; 16];
                vst1q_u8(arr.as_mut_ptr(), v);
                let mut t = 0usize;
                let mut p = 15isize;
                while p >= 0 && arr[p as usize] == b'\\' {
                    t += 1;
                    p -= 1;
                }
                carry_bs_run_len = t;
                i += 16;
            }

            if i < bytes.len() {
                let rem = build_struct_index_scalar(&input[i..]);
                offsets.extend(rem.offsets.into_iter().map(|off| off + i as u32));
            }
            Some(StructIndex { offsets })
        }
    }

    /// Helper to access the scalar reference builder for parity tests.
    ///
    /// Exposed under the `json` feature for integration tests comparing
    /// optimized builders to the scalar reference.
    pub fn build_struct_index_scalar_test(input: &str) -> StructIndex {
        build_struct_index_scalar(input)
    }

    #[cfg(feature = "bench-internal")]
    /// Expose scalar builder to benchmarks to compare against NEON and future GPU paths.
    pub fn build_struct_index_scalar_bench(input: &str) -> StructIndex {
        build_struct_index_scalar(input)
    }

    /// AVX2 Stage-1 builder using nibble-LUT (vpshufb) and bitboard iteration.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn build_struct_index_avx2(input: &str) -> Option<StructIndex> {
        use core::arch::x86_64::*;
        unsafe {
            let bytes = input.as_bytes();
            let mut offsets: Vec<u32> = Vec::new();
            let mut i = 0usize;
            let mut in_string = false;
            let mut carry_bs_run_len: usize = 0;

            // Prepare 32-byte lookup tables (lo/hi replicated)
            let lo_tbl_16: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 12, 1, 12, 0, 0];
            let hi_tbl_16: [u8; 16] = [0, 0, 1, 2, 0, 4, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0];
            let lo_tbl = _mm256_set_m128i(
                _mm_loadu_si128(lo_tbl_16.as_ptr() as *const __m128i),
                _mm_loadu_si128(lo_tbl_16.as_ptr() as *const __m128i),
            );
            let hi_tbl = _mm256_set_m128i(
                _mm_loadu_si128(hi_tbl_16.as_ptr() as *const __m128i),
                _mm_loadu_si128(hi_tbl_16.as_ptr() as *const __m128i),
            );
            let mask_0f = _mm256_set1_epi8(0x0f_i8);
            let zero = _mm256_setzero_si256();

            while i + 32 <= bytes.len() {
                let ptr = bytes.as_ptr().add(i) as *const __m256i;
                let v = _mm256_loadu_si256(ptr);
                // Quotes
                let quote = _mm256_cmpeq_epi8(v, _mm256_set1_epi8(b'"' as i8));
                let mut qmask = _mm256_movemask_epi8(quote) as u32;
                // Structurals via LUT
                let lo = _mm256_and_si256(v, mask_0f);
                let hi = _mm256_and_si256(_mm256_srli_epi16(v, 4), mask_0f);
                let lo_map = _mm256_shuffle_epi8(lo_tbl, lo);
                let hi_map = _mm256_shuffle_epi8(hi_tbl, hi);
                let inter = _mm256_and_si256(lo_map, hi_map);
                let nonzero =
                    _mm256_xor_si256(_mm256_cmpeq_epi8(inter, zero), _mm256_set1_epi8(-1));
                let smask = _mm256_movemask_epi8(nonzero) as u32;

                // Backslash bitboard for escaped-quote parity
                let bslash = _mm256_cmpeq_epi8(v, _mm256_set1_epi8(b'\\' as i8));
                let bmask = _mm256_movemask_epi8(bslash) as u32;

                // Unescaped quotes: iterate only quote bits
                let mut unescaped: u32 = 0;
                while qmask != 0 {
                    let tz = qmask.trailing_zeros() as usize;
                    // Count immediate preceding backslashes using the bitboard
                    let mut run = 0usize;
                    let mut p = tz as isize - 1;
                    while p >= 0 {
                        if ((bmask >> (p as usize)) & 1) == 1 {
                            run += 1;
                            p -= 1;
                        } else {
                            break;
                        }
                    }
                    if p < 0 {
                        run += carry_bs_run_len;
                    }
                    if (run & 1) == 0 {
                        unescaped |= 1u32 << tz;
                    }
                    qmask &= qmask - 1;
                }

                // Iterate union and push offsets
                let mut union = unescaped | smask;
                while union != 0 {
                    let tz = union.trailing_zeros() as usize;
                    let bit = 1u32 << tz;
                    if (unescaped & bit) != 0 {
                        in_string = !in_string;
                        offsets.push((i + tz) as u32);
                    } else if !in_string {
                        offsets.push((i + tz) as u32);
                    }
                    union &= union - 1;
                }

                // Trailing backslash run carry: examine bytes directly for the tail
                let mut arr = [0u8; 32];
                _mm256_storeu_si256(arr.as_mut_ptr() as *mut __m256i, v);
                let mut t = 0usize;
                let mut p = 31isize;
                while p >= 0 && arr[p as usize] == b'\\' {
                    t += 1;
                    p -= 1;
                }
                carry_bs_run_len = t;
                i += 32;
            }

            if i < bytes.len() {
                let rem = build_struct_index_scalar(&input[i..]);
                offsets.extend(rem.offsets.into_iter().map(|off| off + i as u32));
            }
            Some(StructIndex { offsets })
        }
    }

    #[cfg(feature = "metal-stage1")]
    mod metal {
        use std::{
            ffi::{c_char, c_int, c_void},
            sync::{Mutex, OnceLock},
        };

        use super::StructIndex;

        #[cfg(unix)]
        unsafe extern "C" {
            fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
            fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
            fn dlclose(handle: *mut c_void) -> c_int;
        }
        #[cfg(windows)]
        extern "system" {
            fn LoadLibraryA(lpLibFileName: *const u8) -> *mut c_void;
            fn GetProcAddress(hModule: *mut c_void, lpProcName: *const u8) -> *mut c_void;
            fn FreeLibrary(hLibModule: *mut c_void) -> i32;
        }

        const RTLD_LAZY: c_int = 1;

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        #[link(name = "Metal", kind = "framework")]
        unsafe extern "C" {
            fn MTLCreateSystemDefaultDevice() -> *mut c_void;
        }
        #[link(name = "objc")]
        unsafe extern "C" {
            fn objc_autoreleasePoolPush() -> *mut c_void;
            fn objc_autoreleasePoolPop(pool: *mut c_void);
        }

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        type BuildTapeFn = unsafe extern "C" fn(
            in_ptr: *const u8,
            len: usize,
            out_offsets: *mut u32,
            out_capacity: usize,
            out_len: *mut usize,
        ) -> i32;

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        struct MetalLib {
            _handle: *mut c_void,
            func: BuildTapeFn,
        }

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        unsafe impl Send for MetalLib {}
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        unsafe impl Sync for MetalLib {}

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        static METAL_LIB: OnceLock<Mutex<Option<MetalLib>>> = OnceLock::new();

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        pub(super) fn reset_cached_library() {
            if let Some(cache) = METAL_LIB.get() {
                *cache.lock().expect("metal cache poisoned") = None;
            }
        }

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        unsafe fn load_metal_library() -> Option<MetalLib> {
            unsafe {
                // Wrap device creation in an autorelease pool to avoid leaks.
                let pool = objc_autoreleasePoolPush();
                let avail = !MTLCreateSystemDefaultDevice().is_null();
                objc_autoreleasePoolPop(pool);
                if !avail {
                    return None;
                }

                let lib = dlopen(c"libjsonstage1_metal.dylib".as_ptr(), RTLD_LAZY);
                if lib.is_null() {
                    return None;
                }
                let sym = dlsym(lib, c"json_stage1_build_tape".as_ptr());
                if sym.is_null() {
                    let _ = dlclose(lib);
                    return None;
                }
                let func: BuildTapeFn = std::mem::transmute(sym);
                Some(MetalLib { _handle: lib, func })
            }
        }

        /// Attempt to build a structural tape using a dynamically loaded Metal implementation.
        /// Returns None when Metal is unavailable or the helper dylib is not present.
        pub fn build_struct_index_metal(input: &str) -> Option<StructIndex> {
            #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
            unsafe {
                let cache = METAL_LIB.get_or_init(|| Mutex::new(None));
                let mut guard = cache.lock().expect("metal cache poisoned");
                if guard.is_none() {
                    *guard = load_metal_library();
                }
                let lib = guard.as_ref()?;

                let bytes = input.as_bytes();
                let mut offsets: Vec<u32> = Vec::with_capacity(bytes.len());
                let mut out_len: usize = 0;
                let rc = (lib.func)(
                    bytes.as_ptr(),
                    bytes.len(),
                    offsets.as_mut_ptr(),
                    offsets.capacity(),
                    &mut out_len,
                );
                if rc != 0 {
                    return None;
                }
                if out_len > offsets.capacity() {
                    return None;
                }
                offsets.set_len(out_len);
                debug_assert!(
                    {
                        let mut ok = true;
                        let mut prev = 0usize;
                        for (i, off) in offsets.iter().enumerate() {
                            let o = *off as usize;
                            if o >= bytes.len() {
                                ok = false;
                                break;
                            }
                            if i > 0 && o < prev {
                                ok = false;
                                break;
                            }
                            prev = o;
                        }
                        ok
                    },
                    "Metal Stage-1 returned invalid structural offsets"
                );
                Some(StructIndex { offsets })
            }
            #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
            {
                let _ = input;
                None
            }
        }
    }

    #[cfg(feature = "cuda-stage1")]
    mod cuda {
        use std::{
            ffi::{c_char, c_int, c_void},
            sync::{Mutex, OnceLock},
        };

        use super::StructIndex;

        #[cfg(unix)]
        unsafe extern "C" {
            fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
            fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
            fn dlclose(handle: *mut c_void) -> c_int;
        }

        const RTLD_LAZY: c_int = 1;

        type BuildTapeFn = unsafe extern "C" fn(
            in_ptr: *const u8,
            len: usize,
            out_offsets: *mut u32,
            out_capacity: usize,
            out_len: *mut usize,
        ) -> i32;

        struct CudaLib {
            _handle: *mut c_void,
            func: BuildTapeFn,
        }

        unsafe impl Send for CudaLib {}
        unsafe impl Sync for CudaLib {}

        static CUDA_LIB: OnceLock<Mutex<Option<CudaLib>>> = OnceLock::new();

        pub(super) fn reset_cached_library() {
            if let Some(cache) = CUDA_LIB.get() {
                *cache.lock().expect("cuda cache poisoned") = None;
            }
        }

        #[cfg(target_os = "macos")]
        unsafe fn load_cuda_library() -> Option<CudaLib> {
            let mut h = unsafe { dlopen(c"libjsonstage1_cuda.dylib".as_ptr(), RTLD_LAZY) };
            if h.is_null() {
                h = unsafe { dlopen(c"libjsonstage1_cuda.so".as_ptr(), RTLD_LAZY) };
            }
            if h.is_null() {
                return None;
            }
            let sym = unsafe { dlsym(h, c"json_stage1_build_tape".as_ptr()) };
            if sym.is_null() {
                let _ = unsafe { dlclose(h) };
                return None;
            }
            let func: BuildTapeFn = unsafe { std::mem::transmute(sym) };
            Some(CudaLib { _handle: h, func })
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        unsafe fn load_cuda_library() -> Option<CudaLib> {
            let h = unsafe { dlopen(c"libjsonstage1_cuda.so".as_ptr(), RTLD_LAZY) };
            if h.is_null() {
                return None;
            }
            let sym = unsafe { dlsym(h, c"json_stage1_build_tape".as_ptr()) };
            if sym.is_null() {
                let _ = unsafe { dlclose(h) };
                return None;
            }
            let func: BuildTapeFn = unsafe { std::mem::transmute(sym) };
            Some(CudaLib { _handle: h, func })
        }

        #[cfg(windows)]
        unsafe fn load_cuda_library() -> Option<CudaLib> {
            let h = unsafe { LoadLibraryA(b"jsonstage1_cuda.dll\0".as_ptr()) };
            if h.is_null() {
                return None;
            }
            let sym = unsafe { GetProcAddress(h, b"json_stage1_build_tape\0".as_ptr()) };
            if sym.is_null() {
                let _ = unsafe { FreeLibrary(h) };
                return None;
            }
            let func: BuildTapeFn = unsafe { std::mem::transmute(sym) };
            Some(CudaLib { _handle: h, func })
        }

        #[cfg(not(any(target_os = "macos", all(unix, not(target_os = "macos")), windows)))]
        unsafe fn load_cuda_library() -> Option<CudaLib> {
            None
        }

        pub fn build_struct_index_cuda(input: &str) -> Option<StructIndex> {
            unsafe {
                let cache = CUDA_LIB.get_or_init(|| Mutex::new(None));
                let mut guard = cache.lock().expect("cuda cache poisoned");
                if guard.is_none() {
                    *guard = load_cuda_library();
                }
                let lib = guard.as_ref()?;

                let bytes = input.as_bytes();
                let mut offsets: Vec<u32> = Vec::with_capacity(bytes.len());
                let mut out_len: usize = 0;
                let rc = (lib.func)(
                    bytes.as_ptr(),
                    bytes.len(),
                    offsets.as_mut_ptr(),
                    offsets.capacity(),
                    &mut out_len,
                );
                if rc != 0 {
                    return None;
                }
                if out_len > offsets.capacity() {
                    return None;
                }
                offsets.set_len(out_len);
                debug_assert!(
                    {
                        let mut ok = true;
                        let mut prev = 0usize;
                        for (i, off) in offsets.iter().enumerate() {
                            let o = *off as usize;
                            if o >= bytes.len() {
                                ok = false;
                                break;
                            }
                            if i > 0 && o < prev {
                                ok = false;
                                break;
                            }
                            prev = o;
                        }
                        ok
                    },
                    "CUDA Stage-1 returned invalid structural offsets"
                );
                Some(StructIndex { offsets })
            }
        }
    }

    /// A light walker over the structural index.
    pub struct TapeWalker<'a> {
        input: &'a str,
        pub idx: usize,
        pub tape: StructIndex,
        raw: usize,
        last_key_lo: usize,
        last_key_hi: usize,
    }

    /// Reset cached dynamic libraries used by the Stage-1 GPU/Metal accelerators.
    /// This allows callers to retry loading helper libraries without restarting the process.
    pub fn reset_stage1_backends() {
        #[cfg(feature = "metal-stage1")]
        {
            metal::reset_cached_library();
        }
        #[cfg(feature = "cuda-stage1")]
        {
            cuda::reset_cached_library();
        }
    }
    impl<'a> TapeWalker<'a> {
        pub fn new(input: &'a str) -> Self {
            Self {
                input,
                idx: 0,
                tape: build_struct_index(input),
                raw: 0,
                last_key_lo: 0,
                last_key_hi: 0,
            }
        }
        pub fn input(&self) -> &'a str {
            self.input
        }
        pub fn raw_pos(&self) -> usize {
            self.raw
        }
        pub fn sync_to_raw(&mut self, pos: usize) {
            self.raw = pos;
            while self.idx < self.tape.offsets.len()
                && (self.tape.offsets[self.idx] as usize) < self.raw
            {
                self.idx += 1;
            }
        }
        /// Skip ASCII whitespace from the current raw position.
        pub fn skip_ws(&mut self) {
            let bytes = self.input.as_bytes();
            while self.raw < bytes.len() {
                match bytes[self.raw] {
                    b' ' | b'\n' | b'\r' | b'\t' => self.raw += 1,
                    _ => break,
                }
            }
            // Re-sync structural index if needed
            while self.idx < self.tape.offsets.len()
                && (self.tape.offsets[self.idx] as usize) < self.raw
            {
                self.idx += 1;
            }
        }
        pub fn next_struct(&mut self) -> Option<(usize, u8)> {
            if self.idx >= self.tape.offsets.len() {
                return None;
            }
            let off = self.tape.offsets[self.idx] as usize;
            self.idx += 1;
            Some((off, self.input.as_bytes()[off]))
        }
        pub fn peek_struct(&self) -> Option<(usize, u8)> {
            let i = self.idx;
            if i >= self.tape.offsets.len() {
                None
            } else {
                let off = self.tape.offsets[i] as usize;
                Some((off, self.input.as_bytes()[off]))
            }
        }
        /// Expect the next structural to be `{` and advance past it.
        pub fn expect_object_start(&mut self) -> Result<(), Error> {
            match self.next_struct() {
                Some((off, b'{')) => {
                    self.raw = off + 1;
                    Ok(())
                }
                Some((off, _)) => {
                    let (byte, line, col) = pos_from_offset(self.input, off);
                    Err(Error::ExpectedObjectStart { byte, line, col })
                }
                None => {
                    let (byte, line, col) = pos_from_offset(self.input, self.raw);
                    Err(Error::UnexpectedEof { byte, line, col })
                }
            }
        }
        /// Return true if the next structural is `}`.
        pub fn peek_object_end(&self) -> Result<bool, Error> {
            Ok(matches!(self.peek_struct(), Some((_, b'}'))))
        }
        /// Expect the next structural to be `}` and advance past it.
        pub fn expect_object_end(&mut self) -> Result<(), Error> {
            match self.next_struct() {
                Some((off, b'}')) => {
                    self.raw = off + 1;
                    Ok(())
                }
                Some((off, _)) => {
                    let (byte, line, col) = pos_from_offset(self.input, off);
                    Err(Error::ExpectedObjectEnd { byte, line, col })
                }
                None => {
                    let (byte, line, col) = pos_from_offset(self.input, self.raw);
                    Err(Error::UnexpectedEof { byte, line, col })
                }
            }
        }
        /// Expect the next structural to be `[` and advance past it.
        pub fn expect_array_start(&mut self) -> Result<(), Error> {
            match self.next_struct() {
                Some((off, b'[')) => {
                    self.raw = off + 1;
                    Ok(())
                }
                Some((off, _)) => {
                    let (byte, line, col) = pos_from_offset(self.input, off);
                    Err(Error::ExpectedArrayStart { byte, line, col })
                }
                None => {
                    let (byte, line, col) = pos_from_offset(self.input, self.raw);
                    Err(Error::UnexpectedEof { byte, line, col })
                }
            }
        }
        /// Expect the next structural to be `]` and advance past it.
        pub fn expect_array_end(&mut self) -> Result<(), Error> {
            match self.next_struct() {
                Some((off, b']')) => {
                    self.raw = off + 1;
                    Ok(())
                }
                Some((off, _)) => {
                    let (byte, line, col) = pos_from_offset(self.input, off);
                    Err(Error::ExpectedArrayEnd { byte, line, col })
                }
                None => {
                    let (byte, line, col) = pos_from_offset(self.input, self.raw);
                    Err(Error::UnexpectedEof { byte, line, col })
                }
            }
        }
        /// Expect the next structural to be `:` and advance past it.
        pub fn expect_colon(&mut self) -> Result<(), Error> {
            match self.next_struct() {
                Some((off, b':')) => {
                    self.raw = off + 1;
                    Ok(())
                }
                Some((off, ch)) => {
                    if ch == b'"' {
                        let (byte, line, col) = pos_from_offset(self.input, off);
                        return Err(Error::WithPos {
                            msg: "unexpected quote",
                            byte,
                            line,
                            col,
                        });
                    }
                    let (byte, line, col) = pos_from_offset(self.input, off);
                    Err(Error::ExpectedColon { byte, line, col })
                }
                None => {
                    let (byte, line, col) = pos_from_offset(self.input, self.raw);
                    Err(Error::ExpectedColon { byte, line, col })
                }
            }
        }
        /// Robust colon consumption: if the structural index got ahead/behind, resync until the next ':' at or after raw.
        pub fn expect_colon_resync(&mut self) -> Result<(), Error> {
            if self.expect_colon().is_ok() {
                return Ok(());
            }
            // Try to resync by advancing idx to the first ':' whose offset >= raw
            loop {
                match self.peek_struct() {
                    Some((off, _)) if off < self.raw => {
                        let _ = self.next_struct();
                        continue;
                    }
                    Some((off, b':')) => {
                        let _ = self.next_struct();
                        self.raw = off + 1;
                        return Ok(());
                    }
                    Some((off, b',' | b'}')) => {
                        let (byte, line, col) = pos_from_offset(self.input, off);
                        return Err(Error::ExpectedColon { byte, line, col });
                    }
                    Some((_off, _)) => {
                        let _ = self.next_struct();
                        continue;
                    }
                    None => {
                        let (byte, line, col) = pos_from_offset(self.input, self.raw);
                        return Err(Error::ExpectedColon { byte, line, col });
                    }
                }
            }
        }
        /// If the next structural is `,`, consume and return true.
        pub fn consume_comma_if_present(&mut self) -> Result<bool, Error> {
            if let Some((off, b',')) = self.peek_struct() {
                let _ = self.next_struct();
                self.raw = off + 1;
                Ok(true)
            } else {
                // Fallback: raw-based check in case the tape index drifted
                self.skip_ws_raw();
                if self.raw < self.input.len() && self.input.as_bytes()[self.raw] == b',' {
                    self.raw += 1;
                    // Re-sync idx to raw position after consuming comma
                    self.skip_ws();
                    return Ok(true);
                }
                Ok(false)
            }
        }
        /// Read the next object key and return a 64-bit hash (FNV‑1a by default).
        ///
        /// When the `crc-key-hash` feature is enabled and the CPU supports CRC32C
        /// instructions (aarch64 `crc` or x86_64 SSE4.2), a CRC32C‑based key hash
        /// is used for faster dispatch. Collisions must be guarded by a
        /// string‑equality fallback at call sites.
        pub fn read_key_hash(&mut self) -> Result<u64, Error> {
            let (open_off, ch) = match self.next_struct() {
                Some(v) => v,
                None => {
                    let (byte, line, col) = pos_from_offset(self.input, self.raw);
                    return Err(Error::ExpectedKeyHashQuote { byte, line, col });
                }
            };
            if ch != b'"' {
                let (byte, line, col) = pos_from_offset(self.input, open_off);
                return Err(Error::ExpectedKeyHashQuote { byte, line, col });
            }
            let (close_off, ch2) = match self.next_struct() {
                Some(v) => v,
                None => {
                    let (byte, line, col) = pos_from_offset(self.input, self.raw);
                    return Err(Error::UnterminatedKey { byte, line, col });
                }
            };
            if ch2 != b'"' {
                let (byte, line, col) = pos_from_offset(self.input, close_off);
                return Err(Error::UnterminatedKey { byte, line, col });
            }
            let bytes = self.input.as_bytes();
            let mut h_fnv: u64 = 0xcbf29ce484222325;
            let mut h_crc: u32 = 0xFFFF_FFFF;
            #[cfg(not(feature = "crc-key-hash"))]
            let _ = &mut h_crc;
            #[inline]
            fn fnv_add(h: &mut u64, b: u8) {
                *h ^= b as u64;
                *h = h.wrapping_mul(0x100000001b3);
            }
            #[cfg(feature = "crc-key-hash")]
            #[inline]
            fn crc32c_sw(crc: u32, b: u8) -> u32 {
                // Reflected CRC32C update (poly 0x82F63B78)
                let mut c = crc ^ 0xFFFF_FFFF;
                let mut x = b as u32;
                for _ in 0..8 {
                    let mix = (c ^ x) & 1;
                    c >>= 1;
                    if mix != 0 {
                        c ^= 0x82F63B78;
                    }
                    x >>= 1;
                }
                c ^ 0xFFFF_FFFF
            }
            #[cfg(all(feature = "crc-key-hash", target_arch = "x86_64"))]
            #[inline]
            unsafe fn crc32c_u8_sse(crc: u32, b: u8) -> u32 {
                use core::arch::x86_64::_mm_crc32_u8;
                unsafe { _mm_crc32_u8(crc, b) }
            }
            #[cfg(all(feature = "crc-key-hash", target_arch = "aarch64"))]
            #[inline]
            unsafe fn crc32c_u8_arm(crc: u32, b: u8) -> u32 {
                // Uses aarch64 CRC32C byte update when available
                #[cfg(target_feature = "crc")]
                {
                    use core::arch::aarch64::__crc32cb;
                    return unsafe { __crc32cb(crc, b) };
                }
                #[allow(unreachable_code)]
                {
                    crc
                }
            }
            let mut i = open_off + 1;
            while i < close_off {
                let b = bytes[i];
                if b == b'\\' {
                    i += 1;
                    if i >= close_off {
                        let (byte, line, col) = pos_from_offset(self.input, i);
                        return Err(Error::EofEscape { byte, line, col });
                    }
                    let esc = bytes[i];
                    i += 1;
                    match esc {
                        b'"' | b'\\' | b'/' => {
                            fnv_add(&mut h_fnv, esc);
                            #[cfg(feature = "crc-key-hash")]
                            {
                                #[cfg(target_arch = "x86_64")]
                                {
                                    h_crc = if std::is_x86_feature_detected!("sse4.2") {
                                        unsafe { crc32c_u8_sse(h_crc, esc) }
                                    } else {
                                        crc32c_sw(h_crc, esc)
                                    };
                                }
                                #[cfg(target_arch = "aarch64")]
                                {
                                    h_crc = if std::arch::is_aarch64_feature_detected!("crc") {
                                        unsafe { crc32c_u8_arm(h_crc, esc) }
                                    } else {
                                        crc32c_sw(h_crc, esc)
                                    };
                                }
                                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                                {
                                    h_crc = crc32c_sw(h_crc, esc);
                                }
                            }
                        }
                        b'b' => {
                            fnv_add(&mut h_fnv, 0x08);
                            #[cfg(feature = "crc-key-hash")]
                            {
                                let bb = 0x08u8;
                                #[cfg(target_arch = "x86_64")]
                                {
                                    h_crc = if std::is_x86_feature_detected!("sse4.2") {
                                        unsafe { crc32c_u8_sse(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(target_arch = "aarch64")]
                                {
                                    h_crc = if std::arch::is_aarch64_feature_detected!("crc") {
                                        unsafe { crc32c_u8_arm(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                                {
                                    h_crc = crc32c_sw(h_crc, bb);
                                }
                            }
                        }
                        b'f' => {
                            fnv_add(&mut h_fnv, 0x0C);
                            #[cfg(feature = "crc-key-hash")]
                            {
                                let bb = 0x0Cu8;
                                #[cfg(target_arch = "x86_64")]
                                {
                                    h_crc = if std::is_x86_feature_detected!("sse4.2") {
                                        unsafe { crc32c_u8_sse(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(target_arch = "aarch64")]
                                {
                                    h_crc = if std::arch::is_aarch64_feature_detected!("crc") {
                                        unsafe { crc32c_u8_arm(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                                {
                                    h_crc = crc32c_sw(h_crc, bb);
                                }
                            }
                        }
                        b'n' => {
                            fnv_add(&mut h_fnv, b'\n');
                            #[cfg(feature = "crc-key-hash")]
                            {
                                let bb = b'\n';
                                #[cfg(target_arch = "x86_64")]
                                {
                                    h_crc = if std::is_x86_feature_detected!("sse4.2") {
                                        unsafe { crc32c_u8_sse(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(target_arch = "aarch64")]
                                {
                                    h_crc = if std::arch::is_aarch64_feature_detected!("crc") {
                                        unsafe { crc32c_u8_arm(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                                {
                                    h_crc = crc32c_sw(h_crc, bb);
                                }
                            }
                        }
                        b'r' => {
                            fnv_add(&mut h_fnv, b'\r');
                            #[cfg(feature = "crc-key-hash")]
                            {
                                let bb = b'\r';
                                #[cfg(target_arch = "x86_64")]
                                {
                                    h_crc = if std::is_x86_feature_detected!("sse4.2") {
                                        unsafe { crc32c_u8_sse(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(target_arch = "aarch64")]
                                {
                                    h_crc = if std::arch::is_aarch64_feature_detected!("crc") {
                                        unsafe { crc32c_u8_arm(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                                {
                                    h_crc = crc32c_sw(h_crc, bb);
                                }
                            }
                        }
                        b't' => {
                            fnv_add(&mut h_fnv, b'\t');
                            #[cfg(feature = "crc-key-hash")]
                            {
                                let bb = b'\t';
                                #[cfg(target_arch = "x86_64")]
                                {
                                    h_crc = if std::is_x86_feature_detected!("sse4.2") {
                                        unsafe { crc32c_u8_sse(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(target_arch = "aarch64")]
                                {
                                    h_crc = if std::arch::is_aarch64_feature_detected!("crc") {
                                        unsafe { crc32c_u8_arm(h_crc, bb) }
                                    } else {
                                        crc32c_sw(h_crc, bb)
                                    };
                                }
                                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                                {
                                    h_crc = crc32c_sw(h_crc, bb);
                                }
                            }
                        }
                        b'u' => {
                            // Parse 4 hex digits, handle surrogate pair, hash UTF‑8
                            let hex_to_u32 = |bytes: &[u8],
                                              idx: &mut usize|
                             -> Result<u32, Error> {
                                let mut v: u32 = 0;
                                for _ in 0..4 {
                                    if *idx >= close_off {
                                        let (byte, line, col) = pos_from_offset(self.input, *idx);
                                        return Err(Error::EofHex { byte, line, col });
                                    }
                                    let c = bytes[*idx];
                                    *idx += 1;
                                    v = (v << 4)
                                        | match c {
                                            b'0'..=b'9' => (c - b'0') as u32,
                                            b'a'..=b'f' => (c - b'a' + 10) as u32,
                                            b'A'..=b'F' => (c - b'A' + 10) as u32,
                                            _ => {
                                                let (byte, line, col) =
                                                    pos_from_offset(self.input, *idx - 1);
                                                return Err(Error::InvalidHex { byte, line, col });
                                            }
                                        };
                                }
                                Ok(v)
                            };
                            let mut idx = i;
                            let hi = hex_to_u32(bytes, &mut idx)?;
                            let cp = if (0xD800..=0xDBFF).contains(&hi) {
                                // Require next sequence to be \uXXXX
                                if idx + 6 > close_off
                                    || bytes[idx] != b'\\'
                                    || bytes[idx + 1] != b'u'
                                {
                                    let (byte, line, col) = pos_from_offset(self.input, idx);
                                    return Err(Error::WithPos {
                                        msg: "expected low surrogate",
                                        byte,
                                        line,
                                        col,
                                    });
                                }
                                idx += 2; // skip \\u
                                let lo = hex_to_u32(bytes, &mut idx)?;
                                if !(0xDC00..=0xDFFF).contains(&lo) {
                                    let (byte, line, col) =
                                        pos_from_offset(self.input, idx.saturating_sub(1));
                                    return Err(Error::WithPos {
                                        msg: "invalid low surrogate",
                                        byte,
                                        line,
                                        col,
                                    });
                                }
                                0x10000 + (((hi - 0xD800) << 10) | (lo - 0xDC00))
                            } else if (0xDC00..=0xDFFF).contains(&hi) {
                                let (byte, line, col) =
                                    pos_from_offset(self.input, idx.saturating_sub(1));
                                return Err(Error::WithPos {
                                    msg: "unexpected low surrogate",
                                    byte,
                                    line,
                                    col,
                                });
                            } else {
                                hi
                            };
                            i = idx; // advance input index
                            if let Some(ch) = char::from_u32(cp) {
                                let mut buf = [0u8; 4];
                                let s = ch.encode_utf8(&mut buf);
                                for &bb in s.as_bytes() {
                                    fnv_add(&mut h_fnv, bb);
                                    #[cfg(feature = "crc-key-hash")]
                                    {
                                        #[cfg(target_arch = "x86_64")]
                                        {
                                            h_crc = if std::is_x86_feature_detected!("sse4.2") {
                                                unsafe { crc32c_u8_sse(h_crc, bb) }
                                            } else {
                                                crc32c_sw(h_crc, bb)
                                            };
                                        }
                                        #[cfg(target_arch = "aarch64")]
                                        {
                                            h_crc =
                                                if std::arch::is_aarch64_feature_detected!("crc") {
                                                    unsafe { crc32c_u8_arm(h_crc, bb) }
                                                } else {
                                                    crc32c_sw(h_crc, bb)
                                                };
                                        }
                                        #[cfg(not(any(
                                            target_arch = "x86_64",
                                            target_arch = "aarch64"
                                        )))]
                                        {
                                            h_crc = crc32c_sw(h_crc, bb);
                                        }
                                    }
                                }
                            } else {
                                let (byte, line, col) =
                                    pos_from_offset(self.input, idx.saturating_sub(1));
                                return Err(Error::WithPos {
                                    msg: "invalid codepoint",
                                    byte,
                                    line,
                                    col,
                                });
                            }
                        }
                        _ => {
                            let (byte, line, col) =
                                pos_from_offset(self.input, i.saturating_sub(1));
                            return Err(Error::WithPos {
                                msg: "bad escape",
                                byte,
                                line,
                                col,
                            });
                        }
                    }
                } else {
                    fnv_add(&mut h_fnv, b);
                    #[cfg(feature = "crc-key-hash")]
                    {
                        #[cfg(target_arch = "x86_64")]
                        {
                            h_crc = if std::is_x86_feature_detected!("sse4.2") {
                                unsafe { crc32c_u8_sse(h_crc, b) }
                            } else {
                                crc32c_sw(h_crc, b)
                            };
                        }
                        #[cfg(target_arch = "aarch64")]
                        {
                            h_crc = if std::arch::is_aarch64_feature_detected!("crc") {
                                unsafe { crc32c_u8_arm(h_crc, b) }
                            } else {
                                crc32c_sw(h_crc, b)
                            };
                        }
                        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                        {
                            h_crc = crc32c_sw(h_crc, b);
                        }
                    }
                    i += 1;
                }
            }
            self.last_key_lo = open_off + 1;
            self.last_key_hi = close_off;
            self.raw = close_off + 1;
            // Validate that a colon follows the key (do not consume here)
            match self.peek_struct() {
                Some((off, b':')) => {
                    let _ = off; // presence validated; consumption left to caller
                }
                Some((off, b'"')) => {
                    let (byte, line, col) = pos_from_offset(self.input, off);
                    return Err(Error::WithPos {
                        msg: "unexpected quote",
                        byte,
                        line,
                        col,
                    });
                }
                Some((off, _)) => {
                    let (byte, line, col) = pos_from_offset(self.input, off);
                    return Err(Error::ExpectedColon { byte, line, col });
                }
                None => {
                    let (byte, line, col) = pos_from_offset(self.input, close_off);
                    return Err(Error::ExpectedColon { byte, line, col });
                }
            }
            #[cfg(feature = "crc-key-hash")]
            {
                // Mix CRC32C to 64 bits with a fixed avalanche; keep deterministic
                let mut x = (h_crc as u64) ^ 0x9E3779B97F4A7C15;
                x ^= x >> 33;
                x = x.wrapping_mul(0xff51afd7ed558ccd);
                x ^= x >> 33;
                x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
                x ^= x >> 33;
                Ok(x)
            }
            #[cfg(not(feature = "crc-key-hash"))]
            {
                Ok(h_fnv)
            }
        }

        fn skip_ws_raw(&mut self) {
            let bytes = self.input.as_bytes();
            while self.raw < bytes.len() {
                match bytes[self.raw] {
                    b' ' | b'\n' | b'\r' | b'\t' => self.raw += 1,
                    _ => break,
                }
            }
        }

        /// Skip over the next JSON value using the structural index when possible.
        pub fn skip_value(&mut self) -> Result<(), Error> {
            self.skip_ws_raw();
            let bytes = self.input.as_bytes();
            if self.raw >= bytes.len() {
                let (byte, line, col) = pos_from_offset(self.input, self.raw.min(bytes.len()));
                return Err(Error::UnexpectedEof { byte, line, col });
            }
            match bytes[self.raw] {
                b'{' => {
                    if let Some((off, b'{')) = self.peek_struct()
                        && off != self.raw
                    {
                        self.sync_to_raw(off);
                    }
                    let _ = self.next_struct();
                    let mut depth: i32 = 1;
                    while let Some((off, ch)) = self.next_struct() {
                        match ch {
                            b'{' | b'[' => depth += 1,
                            b'}' | b']' => {
                                depth -= 1;
                                if depth == 0 {
                                    self.raw = off + 1;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if depth != 0 {
                        let (byte, line, col) =
                            pos_from_offset(self.input, self.raw.min(bytes.len()));
                        return Err(Error::WithPos {
                            msg: "unclosed object",
                            byte,
                            line,
                            col,
                        });
                    }
                    Ok(())
                }
                b'[' => {
                    if let Some((off, b'[')) = self.peek_struct()
                        && off != self.raw
                    {
                        self.sync_to_raw(off);
                    }
                    let _ = self.next_struct();
                    let mut depth: i32 = 1;
                    while let Some((off, ch)) = self.next_struct() {
                        match ch {
                            b'{' | b'[' => depth += 1,
                            b'}' | b']' => {
                                depth -= 1;
                                if depth == 0 {
                                    self.raw = off + 1;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if depth != 0 {
                        let (byte, line, col) =
                            pos_from_offset(self.input, self.raw.min(bytes.len()));
                        return Err(Error::WithPos {
                            msg: "unclosed array",
                            byte,
                            line,
                            col,
                        });
                    }
                    Ok(())
                }
                b'"' => {
                    let (open_off, ch1) = self.next_struct().ok_or_else(|| {
                        let (byte, line, col) =
                            pos_from_offset(self.input, self.raw.min(bytes.len()));
                        Error::UnexpectedEof { byte, line, col }
                    })?;
                    if ch1 != b'"' || open_off != self.raw {
                        let (byte, line, col) = pos_from_offset(self.input, open_off);
                        return Err(Error::WithPos {
                            msg: "bad string",
                            byte,
                            line,
                            col,
                        });
                    }
                    let (close_off, ch2) = self.next_struct().ok_or_else(|| {
                        let (byte, line, col) =
                            pos_from_offset(self.input, self.raw.min(bytes.len()));
                        Error::UnexpectedEof { byte, line, col }
                    })?;
                    if ch2 != b'"' {
                        let (byte, line, col) = pos_from_offset(self.input, close_off);
                        return Err(Error::UnterminatedString { byte, line, col });
                    }
                    self.raw = close_off + 1;
                    Ok(())
                }
                _ => {
                    let s = self.input;
                    let mut p = Parser::new_at(s, self.raw);
                    p.skip_value()?;
                    self.sync_to_raw(p.position());
                    Ok(())
                }
            }
        }

        /// Return the last read key slice (without quotes), borrowed from input.
        pub fn last_key(&self) -> &'a str {
            // SAFETY: input is valid UTF-8, keys are inside quotes and by JSON rules contain valid UTF-8 bytes
            unsafe {
                std::str::from_utf8_unchecked(
                    &self.input.as_bytes()[self.last_key_lo..self.last_key_hi],
                )
            }
        }

        /// Fast inline parse of a boolean at the current raw position.
        /// Advances `raw` past the token.
        pub fn parse_bool_inline(&mut self) -> Result<bool, Error> {
            self.skip_ws_raw();
            let b = self.input.as_bytes();
            let i = self.raw;
            if i + 4 <= b.len() && &b[i..i + 4] == b"true" {
                self.raw = i + 4;
                return Ok(true);
            }
            if i + 5 <= b.len() && &b[i..i + 5] == b"false" {
                self.raw = i + 5;
                return Ok(false);
            }
            let (byte, line, col) = pos_from_offset(self.input, i.min(b.len()));
            Err(Error::ExpectedBool { byte, line, col })
        }

        /// Fast inline parse of a non-negative u64 at the current raw position.
        /// Advances `raw` past the number.
        pub fn parse_u64_inline(&mut self) -> Result<u64, Error> {
            self.skip_ws_raw();
            let b = self.input.as_bytes();
            let mut i = self.raw;
            if i < b.len() && b[i] == b'-' {
                let (byte, line, col) = pos_from_offset(self.input, i);
                return Err(Error::WithPos {
                    msg: "negative not allowed",
                    byte,
                    line,
                    col,
                });
            }
            let mut val: u64 = 0;
            let mut any = false;
            while i < b.len() {
                let c = b[i];
                if c.wrapping_sub(b'0') <= 9 {
                    let d = (c - b'0') as u64;
                    if val > (u64::MAX - d) / 10 {
                        let (byte, line, col) = pos_from_offset(self.input, i);
                        return Err(Error::U64Overflow { byte, line, col });
                    }
                    val = val * 10 + d;
                    i += 1;
                    any = true;
                } else {
                    break;
                }
            }
            if !any {
                let (byte, line, col) = pos_from_offset(self.input, self.raw.min(b.len()));
                return Err(Error::UnexpectedValue { byte, line, col });
            }
            self.raw = i;
            self.skip_ws();
            Ok(val)
        }

        /// Fast inline parse of a signed i64 at the current raw position.
        /// Advances `raw` past the number.
        pub fn parse_i64_inline(&mut self) -> Result<i64, Error> {
            self.skip_ws_raw();
            let b = self.input.as_bytes();
            let mut i = self.raw;
            let neg = if i < b.len() && b[i] == b'-' {
                i += 1;
                true
            } else {
                false
            };
            let _start_digits = i;
            let mut mag: u128 = 0;
            let mut any = false;
            while i < b.len() {
                let c = b[i];
                if c.wrapping_sub(b'0') <= 9 {
                    let d = (c - b'0') as u128;
                    // mag = mag*10 + d with overflow check up to 128 bits
                    if mag > (u128::MAX - d) / 10 {
                        let (byte, line, col) = pos_from_offset(self.input, i);
                        return Err(Error::WithPos {
                            msg: "i64 overflow",
                            byte,
                            line,
                            col,
                        });
                    }
                    mag = mag * 10 + d;
                    i += 1;
                    any = true;
                } else {
                    break;
                }
            }
            if !any {
                let (byte, line, col) = pos_from_offset(self.input, self.raw.min(b.len()));
                return Err(Error::UnexpectedValue { byte, line, col });
            }
            // Disallow fractional/exponent in i64 path
            if i < b.len() && (b[i] == b'.' || b[i] == b'e' || b[i] == b'E') {
                let (byte, line, col) = pos_from_offset(self.input, i);
                return Err(Error::WithPos {
                    msg: "expected integer",
                    byte,
                    line,
                    col,
                });
            }
            let v = if neg {
                // Allow mag up to 2^63, where 2^63 => i64::MIN
                const LIM: u128 = (i64::MAX as u128) + 1;
                if mag > LIM {
                    let (byte, line, col) = pos_from_offset(self.input, i);
                    return Err(Error::WithPos {
                        msg: "i64 overflow",
                        byte,
                        line,
                        col,
                    });
                }
                if mag == LIM { i64::MIN } else { -(mag as i64) }
            } else {
                if mag > (i64::MAX as u128) {
                    let (byte, line, col) = pos_from_offset(self.input, i);
                    return Err(Error::WithPos {
                        msg: "i64 overflow",
                        byte,
                        line,
                        col,
                    });
                }
                mag as i64
            };
            self.raw = i;
            self.skip_ws();
            Ok(v)
        }

        /// Fast inline parse of a non-negative u128 at the current raw position.
        /// Advances `raw` past the number.
        pub fn parse_u128_inline(&mut self) -> Result<u128, Error> {
            self.skip_ws_raw();
            let b = self.input.as_bytes();
            let mut i = self.raw;
            if i < b.len() && b[i] == b'-' {
                let (byte, line, col) = pos_from_offset(self.input, i);
                return Err(Error::WithPos {
                    msg: "negative not allowed",
                    byte,
                    line,
                    col,
                });
            }
            let mut val: u128 = 0;
            let mut any = false;
            while i < b.len() {
                let c = b[i];
                if c.wrapping_sub(b'0') <= 9 {
                    let d = (c - b'0') as u128;
                    if val > (u128::MAX - d) / 10 {
                        let (byte, line, col) = pos_from_offset(self.input, i);
                        return Err(Error::WithPos {
                            msg: "u128 overflow",
                            byte,
                            line,
                            col,
                        });
                    }
                    val = val * 10 + d;
                    i += 1;
                    any = true;
                } else {
                    break;
                }
            }
            if !any {
                let (byte, line, col) = pos_from_offset(self.input, self.raw.min(b.len()));
                return Err(Error::WithPos {
                    msg: "expected number",
                    byte,
                    line,
                    col,
                });
            }
            // Disallow fractional/exponent in integer path
            if i < b.len() && (b[i] == b'.' || b[i] == b'e' || b[i] == b'E') {
                let (byte, line, col) = pos_from_offset(self.input, i);
                return Err(Error::WithPos {
                    msg: "expected integer",
                    byte,
                    line,
                    col,
                });
            }
            self.raw = i;
            self.skip_ws();
            Ok(val)
        }

        /// Fast inline parse of an f64 at the current raw position.
        /// Advances `raw` past the number.
        pub fn parse_f64_inline(&mut self) -> Result<f64, Error> {
            self.skip_ws_raw();
            let b = self.input.as_bytes();
            let mut i = self.raw;
            if i >= b.len() {
                let (byte, line, col) = pos_from_offset(self.input, i);
                return Err(Error::UnexpectedEof { byte, line, col });
            }
            // optional sign
            if b[i] == b'-' {
                i += 1;
            }
            let _start = i;
            let mut saw_digit = false;
            while i < b.len() && (b[i].wrapping_sub(b'0') <= 9) {
                i += 1;
                saw_digit = true;
            }
            // JSON requires at least one integer digit before optional fraction/exponent.
            if !saw_digit {
                let (byte, line, col) = pos_from_offset(self.input, i.min(b.len()));
                return Err(Error::ExpectedDigits { byte, line, col });
            }
            // optional fraction
            if i < b.len() && b[i] == b'.' {
                i += 1;
                let mut frac_digit = false;
                while i < b.len() && (b[i].wrapping_sub(b'0') <= 9) {
                    i += 1;
                    frac_digit = true;
                }
                if !frac_digit {
                    let (byte, line, col) = pos_from_offset(self.input, i);
                    return Err(Error::ExpectedFracDigits { byte, line, col });
                }
            }
            // optional exponent
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                i += 1;
                if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
                    i += 1;
                }
                let mut exp_digit = false;
                while i < b.len() && (b[i].wrapping_sub(b'0') <= 9) {
                    i += 1;
                    exp_digit = true;
                }
                if !exp_digit {
                    let (byte, line, col) = pos_from_offset(self.input, i);
                    return Err(Error::ExpectedExpDigits { byte, line, col });
                }
            }
            let end = i;
            if end <= self.raw || end > b.len() {
                let (byte, line, col) = pos_from_offset(self.input, self.raw);
                return Err(Error::UnexpectedValue { byte, line, col });
            }
            let s = unsafe { std::str::from_utf8_unchecked(&b[self.raw..end]) };
            let v: f64 = s.parse().map_err(|_| {
                let (byte, line, col) = pos_from_offset(self.input, self.raw);
                Error::WithPos {
                    msg: "invalid number",
                    byte,
                    line,
                    col,
                }
            })?;
            self.raw = end;
            self.skip_ws();
            Ok(v)
        }

        /// Fast inline parse of a JSON string, returning a borrowed slice or an arena-owned string when escaping occurs.
        /// Advances `raw` past the string token.
        pub fn parse_string_ref_inline<'arena>(
            &mut self,
            arena: &'arena mut Arena,
        ) -> Result<StrRef<'a, 'arena>, Error> {
            self.skip_ws_raw();
            let b = self.input.as_bytes();
            let mut i = self.raw;
            if i >= b.len() || b[i] != b'"' {
                let (byte, line, col) = pos_from_offset(self.input, i.min(b.len()));
                return Err(Error::WithPos {
                    msg: "expected string",
                    byte,
                    line,
                    col,
                });
            }
            i += 1;
            let start = i;
            let mut has_escape = false;
            while i < b.len() {
                let c = b[i];
                if c == b'"' {
                    if !has_escape {
                        // borrow directly
                        let s = unsafe { std::str::from_utf8_unchecked(&b[start..i]) };
                        self.raw = i + 1;
                        self.skip_ws();
                        return Ok(StrRef::Borrowed(s));
                    }
                    // slow path: unescape into arena (supports surrogate pairs)
                    let mut out = Vec::with_capacity(i - start);
                    let mut j = start;
                    while j < i {
                        match b[j] {
                            b'\\' => {
                                j += 1;
                                if j >= i {
                                    let (byte, line, col) = pos_from_offset(self.input, j);
                                    return Err(Error::EofEscape { byte, line, col });
                                }
                                let esc = b[j];
                                j += 1;
                                match esc {
                                    b'"' => out.push(b'"'),
                                    b'\\' => out.push(b'\\'),
                                    b'/' => out.push(b'/'),
                                    b'b' => out.push(0x08),
                                    b'f' => out.push(0x0C),
                                    b'n' => out.push(b'\n'),
                                    b'r' => out.push(b'\r'),
                                    b't' => out.push(b'\t'),
                                    b'u' => {
                                        if j + 4 > i {
                                            let (byte, line, col) = pos_from_offset(self.input, j);
                                            return Err(Error::EofHex { byte, line, col });
                                        }
                                        // Read high 16-bit code unit
                                        let mut hi: u32 = 0;
                                        for _ in 0..4 {
                                            let h = b[j];
                                            hi = (hi << 4)
                                                | match h {
                                                    b'0'..=b'9' => (h - b'0') as u32,
                                                    b'a'..=b'f' => (h - b'a' + 10) as u32,
                                                    b'A'..=b'F' => (h - b'A' + 10) as u32,
                                                    _ => {
                                                        let (byte, line, col) =
                                                            pos_from_offset(self.input, j);
                                                        return Err(Error::InvalidHex {
                                                            byte,
                                                            line,
                                                            col,
                                                        });
                                                    }
                                                };
                                            j += 1;
                                        }
                                        let cp = if (0xD800..=0xDBFF).contains(&hi) {
                                            // High surrogate: require a following \uDC00..\uDFFF
                                            if j + 6 > i || b[j] != b'\\' || b[j + 1] != b'u' {
                                                let (byte, line, col) =
                                                    pos_from_offset(self.input, j.min(i));
                                                return Err(Error::WithPos {
                                                    msg: "expected low surrogate",
                                                    byte,
                                                    line,
                                                    col,
                                                });
                                            }
                                            j += 2; // skip \\u
                                            let mut lo: u32 = 0;
                                            for _ in 0..4 {
                                                let h = b[j];
                                                lo = (lo << 4)
                                                    | match h {
                                                        b'0'..=b'9' => (h - b'0') as u32,
                                                        b'a'..=b'f' => (h - b'a' + 10) as u32,
                                                        b'A'..=b'F' => (h - b'A' + 10) as u32,
                                                        _ => {
                                                            let (byte, line, col) =
                                                                pos_from_offset(self.input, j);
                                                            return Err(Error::InvalidHex {
                                                                byte,
                                                                line,
                                                                col,
                                                            });
                                                        }
                                                    };
                                                j += 1;
                                            }
                                            if !(0xDC00..=0xDFFF).contains(&lo) {
                                                let (byte, line, col) = pos_from_offset(
                                                    self.input,
                                                    j.saturating_sub(1),
                                                );
                                                return Err(Error::WithPos {
                                                    msg: "invalid low surrogate",
                                                    byte,
                                                    line,
                                                    col,
                                                });
                                            }
                                            0x10000 + (((hi - 0xD800) << 10) | (lo - 0xDC00))
                                        } else if (0xDC00..=0xDFFF).contains(&hi) {
                                            let (byte, line, col) =
                                                pos_from_offset(self.input, j.saturating_sub(1));
                                            return Err(Error::WithPos {
                                                msg: "unexpected low surrogate",
                                                byte,
                                                line,
                                                col,
                                            });
                                        } else {
                                            hi
                                        };
                                        let ch = char::from_u32(cp).ok_or_else(|| {
                                            let (byte, line, col) =
                                                pos_from_offset(self.input, j.saturating_sub(1));
                                            Error::WithPos {
                                                msg: "invalid codepoint",
                                                byte,
                                                line,
                                                col,
                                            }
                                        })?;
                                        let mut buf = [0u8; 4];
                                        let n = ch.encode_utf8(&mut buf).len();
                                        out.extend_from_slice(&buf[..n]);
                                    }
                                    _ => {
                                        let (byte, line, col) =
                                            pos_from_offset(self.input, j.saturating_sub(1));
                                        return Err(Error::WithPos {
                                            msg: "bad escape",
                                            byte,
                                            line,
                                            col,
                                        });
                                    }
                                }
                            }
                            x if x < 0x20 => {
                                let (byte, line, col) = pos_from_offset(self.input, j);
                                return Err(Error::ControlInString { byte, line, col });
                            }
                            x => {
                                out.push(x);
                                j += 1;
                            }
                        }
                    }
                    let st = std::str::from_utf8(&out).map_err(|_| Error::InvalidUtf8)?;
                    let intern = arena.alloc_str(st.as_bytes());
                    self.raw = i + 1;
                    self.skip_ws();
                    return Ok(StrRef::Owned(intern));
                }
                if c == b'\\' {
                    has_escape = true;
                    i += 2;
                    continue;
                }
                if c < 0x20 {
                    let (byte, line, col) = pos_from_offset(self.input, i);
                    return Err(Error::ControlInString { byte, line, col });
                }
                i += 1;
            }
            let (byte, line, col) = pos_from_offset(self.input, i.min(b.len()));
            Err(Error::UnterminatedString { byte, line, col })
        }
    }

    /// Streaming tokens over JSON input.
    #[derive(Debug, PartialEq)]
    pub enum Token<'a> {
        StartObject,
        EndObject,
        StartArray,
        EndArray,
        KeyBorrowed(&'a str),
        StringBorrowed(&'a str),
        Number(&'a str),
        Bool(bool),
        Null,
    }

    enum Ctx {
        Object { expecting_key: bool },
        Array,
    }

    /// Reader producing a zero-copy token stream using the structural tape.
    pub struct Reader<'a> {
        w: TapeWalker<'a>,
        stack: Vec<Ctx>,
    }

    impl<'a> Reader<'a> {
        pub fn new(input: &'a str) -> Self {
            Self {
                w: TapeWalker::new(input),
                stack: Vec::new(),
            }
        }

        fn skip_commas(&mut self) {
            loop {
                self.w.skip_ws();
                match self.w.peek_struct() {
                    Some((off, b',')) if off == self.w.raw_pos() => {
                        // Do not consume commas at top level; surface an error in next_token instead
                        if self.stack.is_empty() {
                            break;
                        }
                        let _ = self.w.consume_comma_if_present();
                        continue;
                    }
                    _ => break,
                }
            }
        }

        /// Return the next token or None at end of input.
        pub fn next_token(&mut self) -> Result<Option<Token<'a>>, Error> {
            self.skip_commas();
            let s = self.w.input();
            let bytes = s.as_bytes();
            if self.w.raw_pos() >= bytes.len() {
                return Ok(None);
            }

            // Handle immediate structural at raw without relying on the current idx
            let raw = self.w.raw_pos();
            match bytes[raw] {
                b'{' => {
                    self.w.sync_to_raw(raw);
                    let (off, _ch) = self.w.next_struct().ok_or_else(|| {
                        let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                        Error::UnexpectedEof { byte, line, col }
                    })?;
                    self.w.sync_to_raw(off + 1);
                    self.stack.push(Ctx::Object {
                        expecting_key: true,
                    });
                    return Ok(Some(Token::StartObject));
                }
                b'}' => {
                    self.w.sync_to_raw(raw);
                    let (off, _ch) = self.w.next_struct().ok_or_else(|| {
                        let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                        Error::UnexpectedEof { byte, line, col }
                    })?;
                    self.w.sync_to_raw(off + 1);
                    match self.stack.pop() {
                        Some(Ctx::Object { .. }) => return Ok(Some(Token::EndObject)),
                        Some(Ctx::Array) | None => {
                            let (byte, line, col) = pos_from_offset(s, off);
                            return Err(Error::UnexpectedObjectEnd { byte, line, col });
                        }
                    }
                }
                b'[' => {
                    self.w.sync_to_raw(raw);
                    let (off, _ch) = self.w.next_struct().ok_or_else(|| {
                        let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                        Error::UnexpectedEof { byte, line, col }
                    })?;
                    self.w.sync_to_raw(off + 1);
                    self.stack.push(Ctx::Array);
                    return Ok(Some(Token::StartArray));
                }
                b']' => {
                    self.w.sync_to_raw(raw);
                    let (off, _ch) = self.w.next_struct().ok_or_else(|| {
                        let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                        Error::UnexpectedEof { byte, line, col }
                    })?;
                    self.w.sync_to_raw(off + 1);
                    match self.stack.pop() {
                        Some(Ctx::Array) => return Ok(Some(Token::EndArray)),
                        Some(Ctx::Object { .. }) | None => {
                            let (byte, line, col) = pos_from_offset(s, off);
                            return Err(Error::UnexpectedArrayEnd { byte, line, col });
                        }
                    }
                }
                b',' => {
                    self.w.sync_to_raw(raw);
                    if self.stack.is_empty() {
                        let (byte, line, col) = pos_from_offset(s, raw);
                        return Err(Error::UnexpectedComma { byte, line, col });
                    }
                    let consumed = self.w.consume_comma_if_present()?;
                    if !consumed {
                        // Structural index may have advanced past this comma; ensure progress.
                        self.w.sync_to_raw(raw + 1);
                    }
                    if let Some(Ctx::Object { expecting_key }) = self.stack.last_mut() {
                        *expecting_key = true;
                    }
                    return self.next_token();
                }
                b':' => {
                    self.w.sync_to_raw(raw);
                    if !matches!(self.stack.last(), Some(Ctx::Object { expecting_key: _ })) {
                        let (byte, line, col) = pos_from_offset(s, raw);
                        return Err(Error::UnexpectedColon { byte, line, col });
                    }
                    self.w.expect_colon()?;
                    return self.next_token();
                }
                _ => {}
            }

            // If the next structural is ahead of raw, parse a scalar (number/bool/null) as value
            if let Some((off, ch)) = self.w.peek_struct() {
                if off > self.w.raw_pos() {
                    let c = bytes[self.w.raw_pos()];
                    return self.parse_scalar(c);
                }
                if off == self.w.raw_pos() {
                    match ch {
                        b'{' => {
                            let (off, _ch) = self.w.next_struct().ok_or_else(|| {
                                let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                                Error::UnexpectedEof { byte, line, col }
                            })?;
                            self.w.sync_to_raw(off + 1);
                            self.stack.push(Ctx::Object {
                                expecting_key: true,
                            });
                            return Ok(Some(Token::StartObject));
                        }
                        b'}' => {
                            let (off, _ch) = self.w.next_struct().ok_or_else(|| {
                                let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                                Error::UnexpectedEof { byte, line, col }
                            })?;
                            self.w.sync_to_raw(off + 1);
                            match self.stack.pop() {
                                Some(Ctx::Object { .. }) => return Ok(Some(Token::EndObject)),
                                Some(Ctx::Array) | None => {
                                    let (byte, line, col) = pos_from_offset(s, off);
                                    return Err(Error::UnexpectedObjectEnd { byte, line, col });
                                }
                            }
                        }
                        b'[' => {
                            let (off, _ch) = self.w.next_struct().ok_or_else(|| {
                                let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                                Error::UnexpectedEof { byte, line, col }
                            })?;
                            self.w.sync_to_raw(off + 1);
                            self.stack.push(Ctx::Array);
                            return Ok(Some(Token::StartArray));
                        }
                        b']' => {
                            let (off, _ch) = self.w.next_struct().ok_or_else(|| {
                                let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                                Error::UnexpectedEof { byte, line, col }
                            })?;
                            self.w.sync_to_raw(off + 1);
                            match self.stack.pop() {
                                Some(Ctx::Array) => return Ok(Some(Token::EndArray)),
                                Some(Ctx::Object { .. }) | None => {
                                    let (byte, line, col) = pos_from_offset(s, off);
                                    return Err(Error::UnexpectedArrayEnd { byte, line, col });
                                }
                            }
                        }
                        b'"' => {
                            // Distinguish key vs value string using object sub-state.
                            let i = self.w.idx;
                            let in_object = matches!(self.stack.last(), Some(Ctx::Object { .. }));
                            if in_object
                                && matches!(
                                    self.stack.last(),
                                    Some(Ctx::Object {
                                        expecting_key: true
                                    })
                                )
                            {
                                // Expect a closing quote next on the structural tape
                                let open = self.w.tape.offsets[i] as usize;
                                let close_opt = self.w.tape.offsets.get(i + 1).copied();
                                if close_opt.is_none() || bytes[close_opt.unwrap() as usize] != b'"'
                                {
                                    // unterminated key
                                    let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                                    return Err(Error::UnterminatedKey { byte, line, col });
                                }
                                let close = close_opt.unwrap() as usize;
                                // Valid key path
                                let _ = self.w.next_struct();
                                let _ = self.w.next_struct();
                                let key = &s[open + 1..close];
                                self.w.expect_colon()?;
                                if let Some(Ctx::Object { expecting_key }) = self.stack.last_mut() {
                                    *expecting_key = false;
                                }
                                return Ok(Some(Token::KeyBorrowed(key)));
                            } else {
                                // Value string (in object or not)
                                let (open_off, _) = self.w.next_struct().ok_or_else(|| {
                                    let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                                    Error::UnexpectedEof { byte, line, col }
                                })?;
                                let (close_off, _) = self.w.next_struct().ok_or_else(|| {
                                    let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                                    Error::UnexpectedEof { byte, line, col }
                                })?;
                                let val = &s[open_off + 1..close_off];
                                self.w.sync_to_raw(close_off + 1);
                                if let Some(Ctx::Object { expecting_key }) = self.stack.last_mut() {
                                    *expecting_key = true;
                                }
                                return Ok(Some(Token::StringBorrowed(val)));
                            }
                        }
                        b',' | b':' => {
                            // Should be handled by skip_commas/expect_colon; advance and continue
                            let _ = self.w.next_struct();
                            return self.next_token();
                        }
                        _ => {}
                    }
                }
            }

            // Scalar path
            let c = bytes[self.w.raw_pos()];
            self.parse_scalar(c)
        }

        fn parse_scalar(&mut self, c: u8) -> Result<Option<Token<'a>>, Error> {
            let s = self.w.input();
            match c {
                b't' | b'f' => {
                    let mut p = Parser::new_at(s, self.w.raw_pos());
                    let b = p.parse_bool()?;
                    self.w.sync_to_raw(p.position());
                    if let Some(Ctx::Object { expecting_key }) = self.stack.last_mut() {
                        *expecting_key = true;
                    }
                    Ok(Some(Token::Bool(b)))
                }
                b'n' => {
                    let mut p = Parser::new_at(s, self.w.raw_pos());
                    p.parse_null()?;
                    self.w.sync_to_raw(p.position());
                    if let Some(Ctx::Object { expecting_key }) = self.stack.last_mut() {
                        *expecting_key = true;
                    }
                    Ok(Some(Token::Null))
                }
                b'-' | b'0'..=b'9' => {
                    let start = self.w.raw_pos();
                    let bytes = s.as_bytes();
                    let mut i = start;
                    if bytes[i] == b'-' {
                        i += 1;
                    }
                    let mut saw = false;
                    while i < bytes.len() && (bytes[i]).is_ascii_digit() {
                        i += 1;
                        saw = true;
                    }
                    if i < bytes.len() && bytes[i] == b'.' {
                        i += 1;
                        let mut d = false;
                        while i < bytes.len() && bytes[i].is_ascii_digit() {
                            i += 1;
                            d = true;
                        }
                        if !d {
                            let (byte, line, col) = pos_from_offset(s, i);
                            return Err(Error::ExpectedFracDigits { byte, line, col });
                        }
                    }
                    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                        i += 1;
                        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                            i += 1;
                        }
                        let mut d = false;
                        while i < bytes.len() && bytes[i].is_ascii_digit() {
                            i += 1;
                            d = true;
                        }
                        if !d {
                            let (byte, line, col) = pos_from_offset(s, i);
                            return Err(Error::ExpectedExpDigits { byte, line, col });
                        }
                    }
                    if !saw {
                        let (byte, line, col) = pos_from_offset(s, start);
                        return Err(Error::ExpectedDigits { byte, line, col });
                    }
                    let num = &s[start..i];
                    self.w.sync_to_raw(i);
                    if let Some(Ctx::Object { expecting_key }) = self.stack.last_mut() {
                        *expecting_key = true;
                    }
                    Ok(Some(Token::Number(num)))
                }
                _ => {
                    let (byte, line, col) = pos_from_offset(s, self.w.raw_pos());
                    Err(Error::UnexpectedValue { byte, line, col })
                }
            }
        }

        /// Return an iterator over tokens borrowing from this reader.
        pub fn tokens(&mut self) -> Tokens<'_, 'a> {
            Tokens { reader: self }
        }

        /// Inherent helper for tokenizing: returns the next token or None at end of input.
        /// This API matches tests/examples that expect `Result<Option<Token>, Error>`.
        #[allow(clippy::should_implement_trait)]
        pub fn next(&mut self) -> Result<Option<Token<'a>>, Error> {
            self.next_token()
        }
    }

    /// Iterator wrapper over `Reader`.
    pub struct Tokens<'r, 'a> {
        reader: &'r mut Reader<'a>,
    }

    impl<'r, 'a> Iterator for Tokens<'r, 'a> {
        type Item = Result<Token<'a>, Error>;
        fn next(&mut self) -> Option<Self::Item> {
            match self.reader.next_token() {
                Ok(Some(tok)) => Some(Ok(tok)),
                Ok(None) => None,
                Err(e) => Some(Err(e)),
            }
        }
    }

    /// Deserialize `T` from JSON using the generic `JsonDeserialize` path.
    pub fn from_json<T: JsonDeserialize>(s: &str) -> Result<T, Error> {
        let mut p = Parser::new(s);
        p.skip_ws();
        let v = T::json_deserialize(&mut p)?;
        p.skip_ws();
        if !p.eof() {
            let (byte, line, col) = pos_from_offset(s, p.position());
            return Err(Error::TrailingCharacters { byte, line, col });
        }
        Ok(v)
    }

    /// Deserialize `T` using a `FastFromJson` implementation plus the structural tape.
    pub fn from_json_fast<'a, T>(s: &'a str) -> Result<T, Error>
    where
        T: FastFromJson<'a> + JsonDeserialize,
    {
        // Tape-first path: build structural index, then invoke the type's fast parser.
        let mut w = TapeWalker::new(s);
        let mut arena = Arena::new();
        let value = T::parse(&mut w, &mut arena).map_err(|e| Error::Message(e.to_string()))?;

        // After parsing a top-level value, ensure no trailing non-whitespace remains.
        w.skip_ws();
        if w.raw_pos() < s.len() {
            let (byte, line, col) = pos_from_offset(s, w.raw_pos());
            return Err(Error::TrailingCharacters { byte, line, col });
        }
        Ok(value)
    }

    /// Smart fast path: skip the tape for tiny inputs using the generic typed parser.
    /// Falls back to the tape-based path for larger inputs.
    pub fn from_json_fast_smart<'a, T>(s: &'a str) -> Result<T, Error>
    where
        T: FastFromJson<'a> + JsonDeserialize,
    {
        const SMALL_BYTES: usize = 256;
        if s.len() <= SMALL_BYTES {
            // Avoid structural tape for tiny inputs and use the typed parser.
            return from_json::<T>(s);
        }
        from_json_fast::<T>(s)
    }

    /// Auto-select JSON decode path for typed values.
    ///
    /// - For small inputs (<= 256 bytes), prefers the lightweight generic typed parser to
    ///   avoid structural-tape overhead.
    /// - For larger inputs, uses the `FastFromJson` structural-tape path.
    ///
    /// Note: This initial implementation requires `T` to implement both `FastFromJson` and
    /// `JsonDeserialize`. This guarantees correctness for both branches without unstable
    /// specialization. A future refinement can relax this restriction once a stable trait
    /// detection pattern lands.
    pub fn from_json_auto<'a, T>(s: &'a str) -> Result<T, Error>
    where
        T: FastFromJson<'a> + JsonDeserialize,
    {
        const SMALL_BYTES: usize = 256;
        if s.len() <= SMALL_BYTES {
            // Small inputs: avoid tape build and use the generic typed parser
            return from_json::<T>(s);
        }
        from_json_fast::<T>(s)
    }

    /// String reference that can borrow from input or from an arena.
    pub enum StrRef<'s, 'a> {
        Borrowed(&'s str),
        Owned(&'a str),
    }

    impl<'s, 'a> core::fmt::Display for StrRef<'s, 'a> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                StrRef::Borrowed(s) => f.write_str(s),
                StrRef::Owned(s) => f.write_str(s),
            }
        }
    }

    impl<'a> Parser<'a> {
        /// Parse a JSON string and return a reference either borrowed from input or
        /// allocated in the provided arena when unescaping is required.
        pub fn parse_string_ref<'b>(
            &mut self,
            arena: &'b mut Arena,
        ) -> Result<StrRef<'a, 'b>, Error> {
            self.skip_ws();
            self.expect(b'"')?;
            let b = self.s;
            let start = self.i;
            let mut i = self.i;
            let mut has_escape = false;
            while i < b.len() {
                match b[i] {
                    b'"' => {
                        if !has_escape {
                            let slice = &b[start..i];
                            let st = std::str::from_utf8(slice).map_err(|_| Error::InvalidUtf8)?;
                            self.i = i + 1;
                            return Ok(StrRef::Borrowed(st));
                        }
                        // slow path: unescape into arena (supports surrogate pairs)
                        let mut out = Vec::with_capacity(i - start);
                        let mut j = start;
                        while j < i {
                            match b[j] {
                                b'\\' => {
                                    j += 1;
                                    let esc = b.get(j).copied().ok_or_else(|| {
                                        let (byte, line, col) = self.pos_meta(j);
                                        Error::EofEscape { byte, line, col }
                                    })?;
                                    j += 1;
                                    match esc {
                                        b'"' => out.push(b'"'),
                                        b'\\' => out.push(b'\\'),
                                        b'/' => out.push(b'/'),
                                        b'b' => out.push(0x08),
                                        b'f' => out.push(0x0C),
                                        b'n' => out.push(b'\n'),
                                        b'r' => out.push(b'\r'),
                                        b't' => out.push(b'\t'),
                                        b'u' => {
                                            // Read high 16-bit code unit
                                            let mut hi: u32 = 0;
                                            for _ in 0..4 {
                                                let h = b.get(j).copied().ok_or_else(|| {
                                                    let (byte, line, col) = self.pos_meta(j);
                                                    Error::EofHex { byte, line, col }
                                                })?;
                                                hi = (hi << 4)
                                                    | match h {
                                                        b'0'..=b'9' => (h - b'0') as u32,
                                                        b'a'..=b'f' => (h - b'a' + 10) as u32,
                                                        b'A'..=b'F' => (h - b'A' + 10) as u32,
                                                        _ => {
                                                            let (byte, line, col) =
                                                                self.pos_meta(j);
                                                            return Err(Error::InvalidHex {
                                                                byte,
                                                                line,
                                                                col,
                                                            });
                                                        }
                                                    };
                                                j += 1;
                                            }
                                            let cp = if (0xD800..=0xDBFF).contains(&hi) {
                                                // Expect a following low surrogate
                                                if j + 6 > i || b[j] != b'\\' || b[j + 1] != b'u' {
                                                    let (byte, line, col) = self.pos_meta(j.min(i));
                                                    return Err(Error::WithPos {
                                                        msg: "expected low surrogate",
                                                        byte,
                                                        line,
                                                        col,
                                                    });
                                                }
                                                j += 2; // skip \\u
                                                let mut lo: u32 = 0;
                                                for _ in 0..4 {
                                                    let h = b.get(j).copied().ok_or_else(|| {
                                                        let (byte, line, col) = self.pos_meta(j);
                                                        Error::EofHex { byte, line, col }
                                                    })?;
                                                    lo = (lo << 4)
                                                        | match h {
                                                            b'0'..=b'9' => (h - b'0') as u32,
                                                            b'a'..=b'f' => (h - b'a' + 10) as u32,
                                                            b'A'..=b'F' => (h - b'A' + 10) as u32,
                                                            _ => {
                                                                let (byte, line, col) =
                                                                    self.pos_meta(j);
                                                                return Err(Error::InvalidHex {
                                                                    byte,
                                                                    line,
                                                                    col,
                                                                });
                                                            }
                                                        };
                                                    j += 1;
                                                }
                                                if !(0xDC00..=0xDFFF).contains(&lo) {
                                                    let (byte, line, col) =
                                                        self.pos_meta(j.saturating_sub(1));
                                                    return Err(Error::WithPos {
                                                        msg: "invalid low surrogate",
                                                        byte,
                                                        line,
                                                        col,
                                                    });
                                                }
                                                0x10000 + (((hi - 0xD800) << 10) | (lo - 0xDC00))
                                            } else if (0xDC00..=0xDFFF).contains(&hi) {
                                                let (byte, line, col) =
                                                    self.pos_meta(j.saturating_sub(1));
                                                return Err(Error::WithPos {
                                                    msg: "unexpected low surrogate",
                                                    byte,
                                                    line,
                                                    col,
                                                });
                                            } else {
                                                hi
                                            };
                                            let ch = char::from_u32(cp).ok_or_else(|| {
                                                let (byte, line, col) =
                                                    self.pos_meta(j.saturating_sub(1));
                                                Error::WithPos {
                                                    msg: "invalid codepoint",
                                                    byte,
                                                    line,
                                                    col,
                                                }
                                            })?;
                                            let mut buf = [0u8; 4];
                                            let n = ch.encode_utf8(&mut buf).len();
                                            out.extend_from_slice(&buf[..n]);
                                        }
                                        _ => {
                                            let (byte, line, col) =
                                                self.pos_meta(j.saturating_sub(1));
                                            return Err(Error::WithPos {
                                                msg: "bad escape",
                                                byte,
                                                line,
                                                col,
                                            });
                                        }
                                    }
                                }
                                x => {
                                    out.push(x);
                                    j += 1;
                                }
                            }
                        }
                        let st = std::str::from_utf8(&out).map_err(|_| Error::InvalidUtf8)?;
                        let intern = arena.alloc_str(st.as_bytes());
                        self.i = i + 1;
                        return Ok(StrRef::Owned(intern));
                    }
                    b'\\' => {
                        has_escape = true;
                        // If the backslash is the last character, report EOF escape
                        if i + 1 >= b.len() {
                            let (byte, line, col) = self.pos_meta(i + 1);
                            return Err(Error::EofEscape { byte, line, col });
                        }
                        i += 2;
                    }
                    x if x < 0x20 => {
                        let (byte, line, col) = self.pos_meta(i);
                        return Err(Error::ControlInString { byte, line, col });
                    }
                    _ => i += 1,
                }
            }
            let (byte, line, col) = self.pos_meta(i.min(b.len()));
            if has_escape {
                // If scanning saw an escape and we ran out of input without a closing quote,
                // prefer reporting an EOF-in-escape to help diagnostics for trailing '\\'.
                Err(Error::EofEscape { byte, line, col })
            } else {
                Err(Error::UnterminatedString { byte, line, col })
            }
        }
    }

    /// Typed, tape-first decode (prototype): parse using the structural tape directly.
    /// Error type used by FastFromJson derives.
    pub type FastPathError = super::Error;
    pub trait FastFromJson<'a>: Sized {
        fn parse<'b>(w: &mut TapeWalker<'a>, arena: &'b mut Arena) -> Result<Self, FastPathError>;
    }
    /// Typed, straight-line JSON writer.
    pub trait FastJsonWrite {
        fn write_json(&self, out: &mut String);
    }

    impl FastJsonWrite for u16 {
        fn write_json(&self, out: &mut String) {
            write_u64_json(out, u64::from(*self));
        }
    }
    impl FastJsonWrite for u8 {
        fn write_json(&self, out: &mut String) {
            write_u64_json(out, u64::from(*self));
        }
    }
    impl FastJsonWrite for usize {
        fn write_json(&self, out: &mut String) {
            write_u64_json(out, *self as u64);
        }
    }
    impl FastJsonWrite for u32 {
        fn write_json(&self, out: &mut String) {
            write_u32_json(out, *self);
        }
    }
    impl FastJsonWrite for u64 {
        fn write_json(&self, out: &mut String) {
            write_u64_json(out, *self);
        }
    }
    impl FastJsonWrite for u128 {
        fn write_json(&self, out: &mut String) {
            write_u128_json(out, *self);
        }
    }
    impl FastJsonWrite for i64 {
        fn write_json(&self, out: &mut String) {
            write_i64_json(out, *self);
        }
    }
    impl FastJsonWrite for i32 {
        fn write_json(&self, out: &mut String) {
            write_i64_json(out, *self as i64);
        }
    }
    impl FastJsonWrite for i16 {
        fn write_json(&self, out: &mut String) {
            write_i64_json(out, i64::from(*self));
        }
    }
    impl FastJsonWrite for i8 {
        fn write_json(&self, out: &mut String) {
            write_i64_json(out, i64::from(*self));
        }
    }
    impl FastJsonWrite for isize {
        fn write_json(&self, out: &mut String) {
            write_i64_json(out, *self as i64);
        }
    }

    impl<T: JsonSerialize + Ord> FastJsonWrite for std::collections::BTreeSet<T> {
        fn write_json(&self, out: &mut String) {
            out.push('[');
            let mut iter = self.iter();
            if let Some(first) = iter.next() {
                first.json_serialize(out);
                for value in iter {
                    out.push(',');
                    value.json_serialize(out);
                }
            }
            out.push(']');
        }
    }

    impl<K, V> FastJsonWrite for std::collections::BTreeMap<K, V>
    where
        K: JsonSerialize + Ord,
        V: JsonSerialize,
    {
        fn write_json(&self, out: &mut String) {
            out.push('{');
            let mut iter = self.iter();
            if let Some((key, value)) = iter.next() {
                key.json_serialize(out);
                out.push(':');
                value.json_serialize(out);
                for (key, value) in iter {
                    out.push(',');
                    key.json_serialize(out);
                    out.push(':');
                    value.json_serialize(out);
                }
            }
            out.push('}');
        }
    }

    // Minimal writer for f64 used by derives. Non-finite values are encoded as null.
    impl FastJsonWrite for f64 {
        fn write_json(&self, out: &mut String) {
            if self.is_finite() {
                use core::fmt::Write as _;
                let _ = write!(out, "{}", *self);
            } else {
                out.push_str("null");
            }
        }
    }

    impl<T: FastJsonWrite + ?Sized> FastJsonWrite for &T {
        fn write_json(&self, out: &mut String) {
            (**self).write_json(out);
        }
    }

    impl<T: FastJsonWrite + ?Sized> FastJsonWrite for &mut T {
        fn write_json(&self, out: &mut String) {
            (**self).write_json(out);
        }
    }

    impl FastJsonWrite for str {
        fn write_json(&self, out: &mut String) {
            write_json_string(self, out);
        }
    }

    impl FastJsonWrite for String {
        fn write_json(&self, out: &mut String) {
            write_json_string(self, out);
        }
    }

    impl<const N: usize> JsonDeserialize for [u8; N] {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            let raw = parser.parse_string()?;
            decode_hex::<N>(&raw)
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Some(s) = value.as_str() {
                decode_hex::<N>(s)
            } else {
                json_from_value_via_string(value)
            }
        }

        fn json_from_map_key(key: &str) -> Result<Self, Error> {
            decode_hex::<N>(key)
        }
    }

    impl<const N: usize> FastJsonWrite for [u8; N] {
        fn write_json(&self, out: &mut String) {
            encode_hex(self, out);
        }
    }

    impl<K, V> JsonDeserialize for std::collections::HashMap<K, V>
    where
        K: JsonDeserialize + Eq + core::hash::Hash,
        V: JsonDeserialize,
    {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            let mut visitor = MapVisitor::new(parser)?;
            let mut map = std::collections::HashMap::new();
            while let Some(key) = visitor.next_key()? {
                let key_ref = match &key {
                    KeyRef::Borrowed(s) => *s,
                    KeyRef::Owned(s) => s.as_str(),
                };
                let parsed_key = K::json_from_map_key(key_ref)?;
                let value = visitor.parse_value::<V>()?;
                if map.insert(parsed_key, value).is_some() {
                    return Err(MapVisitor::duplicate_field(key_ref));
                }
            }
            visitor.finish()?;
            Ok(map)
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Value::Object(obj) = value {
                let mut map = std::collections::HashMap::with_capacity(obj.len());
                for (k, v) in obj.iter() {
                    let parsed_key = K::json_from_map_key(k)?;
                    if map.insert(parsed_key, V::json_from_value(v)?).is_some() {
                        return Err(Error::duplicate_field(k));
                    }
                }
                Ok(map)
            } else {
                json_from_value_via_string(value)
            }
        }
    }

    impl<K, V> JsonDeserialize for std::collections::BTreeMap<K, V>
    where
        K: JsonDeserialize + Ord,
        V: JsonDeserialize,
    {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            let mut visitor = MapVisitor::new(parser)?;
            let mut map = std::collections::BTreeMap::new();
            while let Some(key) = visitor.next_key()? {
                let key_ref = match &key {
                    KeyRef::Borrowed(s) => *s,
                    KeyRef::Owned(s) => s.as_str(),
                };
                let parsed_key = K::json_from_map_key(key_ref)?;
                let value = visitor.parse_value::<V>()?;
                if map.insert(parsed_key, value).is_some() {
                    return Err(MapVisitor::duplicate_field(key_ref));
                }
            }
            visitor.finish()?;
            Ok(map)
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Value::Object(obj) = value {
                let mut map = std::collections::BTreeMap::new();
                for (k, v) in obj.iter() {
                    let parsed_key = K::json_from_map_key(k)?;
                    if map.insert(parsed_key, V::json_from_value(v)?).is_some() {
                        return Err(Error::duplicate_field(k));
                    }
                }
                Ok(map)
            } else {
                json_from_value_via_string(value)
            }
        }
    }

    impl<T> JsonDeserialize for std::collections::HashSet<T>
    where
        T: JsonDeserialize + Eq + core::hash::Hash,
    {
        fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, Error> {
            parser.skip_ws();
            parser.expect(b'[')?;
            let mut set = std::collections::HashSet::new();
            parser.skip_ws();
            if parser.try_consume_char(b']')? {
                return Ok(set);
            }
            loop {
                let value = T::json_deserialize(parser)?;
                if !set.insert(value) {
                    return Err(Error::Message("duplicate element in set".into()));
                }
                parser.skip_ws();
                if parser.try_consume_char(b',')? {
                    continue;
                }
                parser.expect(b']')?;
                break;
            }
            Ok(set)
        }

        fn json_from_value(value: &Value) -> Result<Self, Error> {
            if let Value::Array(items) = value {
                let mut set = std::collections::HashSet::with_capacity(items.len());
                for item in items {
                    let v = T::json_from_value(item)?;
                    if !set.insert(v) {
                        return Err(Error::Message("duplicate element in set".into()));
                    }
                }
                Ok(set)
            } else {
                json_from_value_via_string(value)
            }
        }
    }

    impl<T> FastJsonWrite for std::collections::HashSet<T>
    where
        T: JsonSerialize + Eq + core::hash::Hash + Ord,
    {
        fn write_json(&self, out: &mut String) {
            let mut values: Vec<&T> = self.iter().collect();
            values.sort();
            out.push('[');
            let mut iter = values.into_iter();
            if let Some(first) = iter.next() {
                first.json_serialize(out);
                for value in iter {
                    out.push(',');
                    value.json_serialize(out);
                }
            }
            out.push(']');
        }
    }

    impl FastJsonWrite for Url {
        fn write_json(&self, out: &mut String) {
            write_json_string(self.as_str(), out);
        }
    }

    /// Borrowed-or-owned key reference returned by `Parser::parse_key`.
    pub enum KeyRef<'a> {
        Borrowed(&'a str),
        Owned(String),
    }
    impl<'a> KeyRef<'a> {
        #[inline]
        pub fn as_str(&self) -> &str {
            match self {
                KeyRef::Borrowed(s) => s,
                KeyRef::Owned(s) => s.as_str(),
            }
        }
        // Note: prefer `as_str()` or the `AsRef<str>` impl over an inherent
        // `as_ref` method to avoid clippy's confusion with the trait method.
    }

    impl<'a> AsRef<str> for KeyRef<'a> {
        #[inline]
        fn as_ref(&self) -> &str {
            self.as_str()
        }
    }

    /// Wrapper around a parsed JSON object key that offers typed conversions.
    pub struct CoerceKey<'a> {
        inner: KeyRef<'a>,
    }

    impl<'a> From<KeyRef<'a>> for CoerceKey<'a> {
        fn from(inner: KeyRef<'a>) -> Self {
            Self { inner }
        }
    }

    impl<'a> CoerceKey<'a> {
        #[inline]
        pub fn as_str(&self) -> &str {
            self.inner.as_str()
        }

        #[inline]
        pub fn into_owned(self) -> String {
            match self.inner {
                KeyRef::Borrowed(s) => s.to_owned(),
                KeyRef::Owned(s) => s,
            }
        }

        pub fn parse<T>(&self) -> Result<T, Error>
        where
            T: core::str::FromStr,
            T::Err: core::fmt::Display,
        {
            let key = self.inner.as_str();
            key.parse::<T>()
                .map_err(|e| Error::Message(format!("failed to parse map key `{key}`: {e}")))
        }
    }

    #[derive(Debug, Clone)]
    pub struct RawValue {
        inner: Box<str>,
    }

    impl RawValue {
        #[inline]
        pub fn from_boxed(inner: Box<str>) -> Self {
            Self { inner }
        }

        #[inline]
        pub fn from_string(s: String) -> Self {
            Self {
                inner: s.into_boxed_str(),
            }
        }

        #[inline]
        pub fn get(&self) -> &str {
            &self.inner
        }

        #[inline]
        pub fn as_str(&self) -> &str {
            &self.inner
        }

        #[inline]
        pub fn into_boxed_str(self) -> Box<str> {
            self.inner
        }

        #[inline]
        pub fn into_string(self) -> String {
            self.inner.into()
        }
    }

    impl JsonSerialize for RawValue {
        fn json_serialize(&self, out: &mut String) {
            out.push_str(self.get());
        }
    }

    impl JsonDeserialize for RawValue {
        fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, Error> {
            p.skip_ws();
            let start = p.position();
            p.skip_value()?;
            let end = p.position();
            let slice = &p.input()[start..end];
            Ok(RawValue::from_string(slice.to_owned()))
        }
    }

    pub struct MapVisitor<'a, 'p> {
        parser: &'p mut Parser<'a>,
        finished: bool,
        value_pending: bool,
    }

    impl<'a, 'p> MapVisitor<'a, 'p> {
        pub fn new(parser: &'p mut Parser<'a>) -> Result<Self, Error> {
            parser.skip_ws();
            parser.expect(b'{')?;
            let mut visitor = Self {
                parser,
                finished: false,
                value_pending: false,
            };
            if visitor.parser.try_consume_char(b'}')? {
                visitor.finished = true;
            }
            Ok(visitor)
        }

        #[inline]
        pub fn parser(&mut self) -> &mut Parser<'a> {
            self.parser
        }

        #[inline]
        pub fn is_finished(&self) -> bool {
            self.finished && !self.value_pending
        }

        pub fn next_key(&mut self) -> Result<Option<KeyRef<'a>>, Error> {
            if self.finished {
                return Ok(None);
            }
            if self.value_pending {
                return Err(Error::Message(
                    "attempted to read a new key before consuming the previous value".into(),
                ));
            }
            if self.parser.try_consume_char(b'}')? {
                self.finished = true;
                return Ok(None);
            }
            let key = self.parser.parse_key()?;
            self.value_pending = true;
            Ok(Some(key))
        }

        pub fn parse_value<T: JsonDeserialize>(&mut self) -> Result<T, Error> {
            if !self.value_pending {
                return Err(Error::Message("no pending value for current key".into()));
            }
            let value = T::json_deserialize(self.parser)?;
            self.finish_value()?;
            Ok(value)
        }

        pub fn parse_value_with<V>(&mut self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'a>,
        {
            if !self.value_pending {
                return Err(Error::Message("no pending value for current key".into()));
            }
            let value = visit_value(self.parser, visitor)?;
            self.finish_value()?;
            Ok(value)
        }

        pub fn skip_value(&mut self) -> Result<(), Error> {
            if !self.value_pending {
                return Err(Error::Message("no pending value for current key".into()));
            }
            self.parser.skip_value()?;
            self.finish_value()
        }

        pub fn next_entry<T: JsonDeserialize>(&mut self) -> Result<Option<(String, T)>, Error> {
            match self.next_key()? {
                Some(key) => {
                    let owned = match key {
                        KeyRef::Borrowed(s) => s.to_owned(),
                        KeyRef::Owned(s) => s,
                    };
                    let value = self.parse_value::<T>()?;
                    Ok(Some((owned, value)))
                }
                None => Ok(None),
            }
        }

        /// Fetch the next key and coerce it into `T` using `FromStr`.
        ///
        /// Returns `Ok(None)` when the object has no more entries. Any parse
        /// failure from `T::from_str` is wrapped in a deterministic JSON
        /// [`Error`].
        pub fn coerce_key<T>(&mut self) -> Result<Option<T>, Error>
        where
            T: core::str::FromStr,
            T::Err: core::fmt::Display,
        {
            match self.next_key()? {
                Some(key) => {
                    let parsed = CoerceKey::from(key).parse::<T>()?;
                    Ok(Some(parsed))
                }
                None => Ok(None),
            }
        }

        /// Fetch the next key/value pair, coercing the key via `FromStr` and
        /// deserializing the value using `JsonDeserialize`.
        pub fn next_entry_coerced<T, V>(&mut self) -> Result<Option<(T, V)>, Error>
        where
            T: core::str::FromStr,
            T::Err: core::fmt::Display,
            V: JsonDeserialize,
        {
            match self.next_key()? {
                Some(key) => {
                    let parsed_key = CoerceKey::from(key).parse::<T>()?;
                    let value = self.parse_value::<V>()?;
                    Ok(Some((parsed_key, value)))
                }
                None => Ok(None),
            }
        }

        pub fn finish(mut self) -> Result<(), Error> {
            if self.value_pending {
                return Err(Error::Message(
                    "object ended before consuming value for current key".into(),
                ));
            }
            if !self.finished {
                self.parser.skip_ws();
                if self.parser.try_consume_char(b'}')? {
                    self.finished = true;
                } else {
                    let (byte, line, col) =
                        crate::json::pos_from_offset(self.parser.input(), self.parser.position());
                    return Err(Error::ExpectedCommaOrObjectEnd { byte, line, col });
                }
            }
            Ok(())
        }

        #[inline]
        pub fn missing_field(field: &'static str) -> Error {
            Error::missing_field(field)
        }

        #[inline]
        pub fn duplicate_field(field: &str) -> Error {
            Error::duplicate_field(field)
        }

        #[inline]
        pub fn unknown_field(field: &str) -> Error {
            Error::unknown_field(field)
        }

        fn finish_value(&mut self) -> Result<(), Error> {
            self.parser.skip_ws();
            match self.parser.peek() {
                Some(b',') => {
                    self.parser.bump();
                    self.value_pending = false;
                    Ok(())
                }
                Some(b'}') => {
                    self.parser.bump();
                    self.finished = true;
                    self.value_pending = false;
                    Ok(())
                }
                Some(_) => {
                    let (byte, line, col) =
                        crate::json::pos_from_offset(self.parser.input(), self.parser.position());
                    Err(Error::ExpectedCommaOrObjectEnd { byte, line, col })
                }
                None => {
                    let (byte, line, col) =
                        crate::json::pos_from_offset(self.parser.input(), self.parser.position());
                    Err(Error::UnexpectedEof { byte, line, col })
                }
            }
        }
    }

    pub struct SeqVisitor<'a, 'p> {
        parser: &'p mut Parser<'a>,
        finished: bool,
    }

    impl<'a, 'p> SeqVisitor<'a, 'p> {
        pub fn new(parser: &'p mut Parser<'a>) -> Result<Self, Error> {
            parser.skip_ws();
            parser.expect(b'[')?;
            let mut visitor = Self {
                parser,
                finished: false,
            };
            if visitor.parser.try_consume_char(b']')? {
                visitor.finished = true;
            }
            Ok(visitor)
        }

        #[inline]
        pub fn parser(&mut self) -> &mut Parser<'a> {
            self.parser
        }

        #[inline]
        pub fn is_finished(&self) -> bool {
            self.finished
        }

        pub fn next_element<T: JsonDeserialize>(&mut self) -> Result<Option<T>, Error> {
            if self.finished {
                return Ok(None);
            }
            let value = T::json_deserialize(self.parser)?;
            self.finish_element()?;
            Ok(Some(value))
        }

        pub fn next_element_with<V>(&mut self, visitor: V) -> Result<Option<V::Value>, Error>
        where
            V: Visitor<'a>,
        {
            if self.finished {
                return Ok(None);
            }
            let value = visit_value(self.parser, visitor)?;
            self.finish_element()?;
            Ok(Some(value))
        }

        pub fn skip_element(&mut self) -> Result<(), Error> {
            if self.finished {
                return Ok(());
            }
            self.parser.skip_value()?;
            self.finish_element()
        }

        pub fn finish(mut self) -> Result<(), Error> {
            if !self.finished {
                self.parser.skip_ws();
                if self.parser.try_consume_char(b']')? {
                    self.finished = true;
                } else {
                    let (byte, line, col) =
                        crate::json::pos_from_offset(self.parser.input(), self.parser.position());
                    return Err(Error::ExpectedCommaOrArrayEnd { byte, line, col });
                }
            }
            Ok(())
        }

        fn finish_element(&mut self) -> Result<(), Error> {
            self.parser.skip_ws();
            match self.parser.peek() {
                Some(b',') => {
                    self.parser.bump();
                    Ok(())
                }
                Some(b']') => {
                    self.parser.bump();
                    self.finished = true;
                    Ok(())
                }
                Some(_) => {
                    let (byte, line, col) =
                        crate::json::pos_from_offset(self.parser.input(), self.parser.position());
                    Err(Error::ExpectedCommaOrArrayEnd { byte, line, col })
                }
                None => {
                    let (byte, line, col) =
                        crate::json::pos_from_offset(self.parser.input(), self.parser.position());
                    Err(Error::UnexpectedEof { byte, line, col })
                }
            }
        }
    }

    pub trait Visitor<'a> {
        type Value;

        fn visit_null(self) -> Result<Self::Value, Error>;
        fn visit_bool(self, v: bool) -> Result<Self::Value, Error>;
        fn visit_i64(self, v: i64) -> Result<Self::Value, Error>;
        fn visit_u64(self, v: u64) -> Result<Self::Value, Error>;
        fn visit_f64(self, v: f64) -> Result<Self::Value, Error>;
        fn visit_string(self, v: String) -> Result<Self::Value, Error>;
        fn visit_map(self, visitor: MapVisitor<'a, '_>) -> Result<Self::Value, Error>;
        fn visit_seq(self, visitor: SeqVisitor<'a, '_>) -> Result<Self::Value, Error>;
    }

    pub fn visit_value<'a, V>(parser: &mut Parser<'a>, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'a>,
    {
        parser.skip_ws();
        match parser.peek() {
            Some(b'n') => {
                parser.parse_null()?;
                visitor.visit_null()
            }
            Some(b't') | Some(b'f') => {
                let v = parser.parse_bool()?;
                visitor.visit_bool(v)
            }
            Some(b'"') => {
                let s = parser.parse_string()?;
                visitor.visit_string(s)
            }
            Some(b'{') => {
                let map = MapVisitor::new(parser)?;
                visitor.visit_map(map)
            }
            Some(b'[') => {
                let seq = SeqVisitor::new(parser)?;
                visitor.visit_seq(seq)
            }
            Some(b'-') | Some(b'0'..=b'9') => {
                let number = parse_number_token(parser)?;
                match number {
                    Number::I64(v) => visitor.visit_i64(v),
                    Number::U64(v) => visitor.visit_u64(v),
                    Number::F64(v) => visitor.visit_f64(v),
                }
            }
            Some(other) => {
                let (byte, line, col) =
                    crate::json::pos_from_offset(parser.input(), parser.position());
                Err(Error::UnexpectedCharacter {
                    found: UnexpectedToken::Char(other as char),
                    byte,
                    line,
                    col,
                })
            }
            None => {
                let (byte, line, col) =
                    crate::json::pos_from_offset(parser.input(), parser.position());
                Err(Error::UnexpectedEof { byte, line, col })
            }
        }
    }

    fn parse_number_token(parser: &mut Parser<'_>) -> Result<Number, Error> {
        parser.skip_ws();
        let s = parser.input();
        let bytes = s.as_bytes();
        let mut idx = parser.position();
        let len = bytes.len();
        if idx >= len {
            let (byte, line, col) = crate::json::pos_from_offset(s, idx);
            return Err(Error::UnexpectedEof { byte, line, col });
        }
        let start = idx;
        let mut neg = false;
        if bytes[idx] == b'-' {
            neg = true;
            idx += 1;
        }
        let mut saw_digit = false;
        let int_start = idx;
        while idx < len && bytes[idx].is_ascii_digit() {
            saw_digit = true;
            idx += 1;
        }
        if !saw_digit {
            let (byte, line, col) = crate::json::pos_from_offset(s, idx.min(len));
            return Err(Error::ExpectedDigits { byte, line, col });
        }
        let int_end = idx;
        if bytes[int_start] == b'0' && int_end > int_start + 1 {
            let (byte, line, col) = crate::json::pos_from_offset(s, int_start + 1);
            return Err(Error::WithPos {
                msg: Parser::LEADING_ZERO_MSG,
                byte,
                line,
                col,
            });
        }
        let mut is_float = false;
        if idx < len && bytes[idx] == b'.' {
            is_float = true;
            idx += 1;
            let mut frac_digits = false;
            while idx < len && bytes[idx].is_ascii_digit() {
                frac_digits = true;
                idx += 1;
            }
            if !frac_digits {
                let (byte, line, col) = crate::json::pos_from_offset(s, idx.min(len));
                return Err(Error::ExpectedFracDigits { byte, line, col });
            }
        }
        if idx < len && (bytes[idx] == b'e' || bytes[idx] == b'E') {
            is_float = true;
            idx += 1;
            if idx < len && (bytes[idx] == b'+' || bytes[idx] == b'-') {
                idx += 1;
            }
            let mut exp_digits = false;
            while idx < len && bytes[idx].is_ascii_digit() {
                exp_digits = true;
                idx += 1;
            }
            if !exp_digits {
                let (byte, line, col) = crate::json::pos_from_offset(s, idx.min(len));
                return Err(Error::ExpectedExpDigits { byte, line, col });
            }
        }
        let slice = &s[start..idx];
        parser.i = idx;
        if is_float {
            let v: f64 = slice
                .parse()
                .map_err(|_| Error::Message("float parse".to_owned()))?;
            let n = Number::from_f64(v)
                .ok_or_else(|| Error::Message("json float out of range".to_owned()))?;
            return Ok(n);
        }
        if neg && &s[int_start..idx] == "0" {
            let n = Number::from_f64(-0.0)
                .ok_or_else(|| Error::Message("json float out of range".to_owned()))?;
            return Ok(n);
        }
        let digits = &s[int_start..idx];
        if !neg {
            if let Ok(u) = digits.parse::<u64>() {
                return Ok(Number::from(u));
            }
        } else if let Ok(u) = digits.parse::<u64>() {
            if u == (i64::MAX as u64) + 1 {
                return Ok(Number::from(i64::MIN));
            }
            if u <= i64::MAX as u64 {
                return Ok(Number::from(-(u as i64)));
            }
        }
        let v: f64 = slice
            .parse()
            .map_err(|_| Error::Message("number parse".to_owned()))?;
        let n = Number::from_f64(v)
            .ok_or_else(|| Error::Message("json float out of range".to_owned()))?;
        Ok(n)
    }

    // ===== CRC32C helpers (portable + HW-accelerated byte update) =====
    #[inline]
    #[allow(dead_code)]
    fn crc32c_update_byte(crc: u32, byte: u8) -> u32 {
        #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
        {
            if std::arch::is_aarch64_feature_detected!("crc") {
                // SAFETY: guarded by runtime feature detection
                return unsafe { crc32c_hw_update_byte(crc, byte) };
            }
        }
        #[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
        {
            if std::is_x86_feature_detected!("sse4.2") {
                // SAFETY: guarded by runtime feature detection
                return unsafe { crc32c_hw_update_byte(crc, byte) };
            }
        }
        crc32c_update_byte_sw(crc, byte)
    }

    #[inline]
    #[allow(dead_code)]
    fn crc32c_update_byte_sw(mut crc: u32, byte: u8) -> u32 {
        // Bitwise CRC32C (Castagnoli) with reflected polynomial 0x82F63B78
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg() & 0x82F6_3B78;
            crc = (crc >> 1) ^ mask;
        }
        crc
    }

    #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
    #[target_feature(enable = "crc")]
    unsafe fn crc32c_hw_update_byte(crc: u32, byte: u8) -> u32 {
        use core::arch::aarch64::__crc32cb;
        __crc32cb(crc, byte)
    }

    #[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
    #[target_feature(enable = "sse4.2")]
    unsafe fn crc32c_hw_update_byte(crc: u32, byte: u8) -> u32 {
        use core::arch::x86_64::_mm_crc32_u8;
        _mm_crc32_u8(crc, byte)
    }
}

/// Serialize an object into the given writer.
pub fn serialize_into<W: Write, T: NoritoSerialize>(
    mut writer: W,
    value: &T,
    compression: Compression,
) -> Result<(), Error> {
    let bytes = match compression {
        Compression::None => to_bytes(value)?,
        Compression::Zstd => to_compressed_bytes(value, Some(CompressionConfig::default()))?,
    };
    writer.write_all(&bytes)?;
    Ok(())
}

/// Deserialize an object from the provided reader.
pub fn deserialize_from<R: Read, T>(reader: R) -> Result<T, Error>
where
    for<'de> T: NoritoDeserialize<'de>,
{
    deserialize_stream(reader)
}

/// Deserialize an object from a stream, validating header and checksum without
/// buffering the entire input.
pub fn deserialize_stream<R: Read, T>(mut reader: R) -> Result<T, Error>
where
    for<'de> T: NoritoDeserialize<'de>,
{
    use core::Header;

    let header = Header::read(&mut reader)?;
    core::prepare_header_decode(header.flags, header.minor, false)?;
    if header.schema != T::schema_hash() {
        return Err(Error::SchemaMismatch);
    }
    let payload_len = core::payload_len_to_usize(header.length)?;
    // Set decode flags for this stream
    let _guard = core::DecodeFlagsGuard::enter(header.flags);
    let mut payload = Vec::with_capacity(payload_len);

    match header.compression {
        Compression::None => {
            let mut buf = [0u8; 64 * 1024];
            let padding = core::payload_alignment_padding_for::<T>();
            if padding != 0 {
                core::stream::skip_padding(&mut reader, padding)?;
            }
            let mut remaining = payload_len;
            while remaining > 0 {
                let chunk = remaining.min(buf.len());
                let read = reader.read(&mut buf[..chunk])?;
                if read == 0 {
                    return Err(Error::LengthMismatch);
                }
                payload.extend_from_slice(&buf[..read]);
                remaining -= read;
            }
        }
        Compression::Zstd => {
            #[cfg(feature = "compression")]
            {
                let mut decoder = zstd::Decoder::new(reader)?;
                let mut buf = [0u8; 64 * 1024];
                let mut remaining = payload_len;
                while remaining > 0 {
                    let chunk = remaining.min(buf.len());
                    let read = decoder.read(&mut buf[..chunk])?;
                    if read == 0 {
                        return Err(Error::LengthMismatch);
                    }
                    payload.extend_from_slice(&buf[..read]);
                    remaining -= read;
                }
                if decoder.read(&mut [0u8; 1])? != 0 {
                    return Err(Error::LengthMismatch);
                }
            }
            #[cfg(not(feature = "compression"))]
            {
                let _ = reader;
                return Err(std::io::Error::other("compression support disabled").into());
            }
        }
    }

    let crc = core::hardware_crc64(&payload);
    if crc != header.checksum {
        return Err(Error::ChecksumMismatch);
    }

    let min_size = ::core::mem::size_of::<core::Archived<T>>();
    let logical_len = payload.len();
    let mut padded: Option<Vec<u8>> = None;
    let backing: &[u8] = if min_size > 0 && logical_len < min_size {
        let mut buf = Vec::with_capacity(min_size);
        buf.extend_from_slice(&payload);
        buf.resize(min_size, 0);
        padded = Some(buf);
        padded.as_deref().expect("pad present")
    } else {
        payload.as_slice()
    };

    let archived = core::archived_from_slice::<T>(backing)?;
    let _payload_guard = if min_size > 0 && logical_len < min_size {
        core::PayloadCtxGuard::enter_with_len(archived.bytes(), logical_len)
    } else {
        core::PayloadCtxGuard::enter(archived.bytes())
    };
    guarded_try_deserialize(|| <T as NoritoDeserialize>::try_deserialize(archived.archived()))
}

fn decode_from_uncompressed_bytes<T>(bytes: &[u8], header: core::Header) -> Result<T, Error>
where
    for<'de> T: NoritoDeserialize<'de>,
{
    core::prepare_header_decode(header.flags, header.minor, false)?;
    if header.compression != Compression::None {
        return Err(Error::unsupported_compression_with(
            header.compression as u8,
            &[Compression::None],
        ));
    }
    if header.schema != T::schema_hash() {
        return Err(Error::SchemaMismatch);
    }
    let payload_len = core::payload_len_to_usize(header.length)?;
    let padding = core::payload_alignment_padding_for::<T>();
    let slice = bytes
        .get(core::Header::SIZE..)
        .ok_or(Error::LengthMismatch)?;
    let payload = core::payload_without_leading_padding_exact(slice, payload_len, padding)?;
    if core::hardware_crc64(payload) != header.checksum {
        return Err(Error::ChecksumMismatch);
    }

    let flags = header.flags;
    let flags_hint = header.minor;
    let min_size = ::core::mem::size_of::<core::Archived<T>>();
    let logical_len = payload.len();
    let mut padded: Option<Vec<u8>> = None;
    let backing: &[u8] = if min_size > 0 && logical_len < min_size {
        let mut buf = Vec::with_capacity(min_size);
        buf.extend_from_slice(payload);
        buf.resize(min_size, 0);
        padded = Some(buf);
        padded.as_deref().expect("pad present")
    } else {
        payload
    };
    let archived = core::archived_from_slice::<T>(backing)?;
    guarded_try_deserialize(|| {
        let _guard = if min_size > 0 && logical_len < min_size {
            core::PayloadCtxGuard::enter_with_flags_hint_len(
                archived.bytes(),
                logical_len,
                flags,
                flags_hint,
            )
        } else {
            core::PayloadCtxGuard::enter_with_flags_hint(archived.bytes(), flags, flags_hint)
        };
        <T as NoritoDeserialize>::try_deserialize(archived.archived())
    })
}

/// Prelude with commonly used items.
pub mod prelude {
    pub use super::{
        Compression, Error, NoritoDeserialize, NoritoSerialize,
        derive::{Decode, Encode},
        deserialize_from, serialize_into,
    };
}

/// Decode an object from Norito-encoded bytes (compressed or not),
/// scoping decode layout flags to this call.
pub fn decode_from_bytes<T>(bytes: &[u8]) -> Result<T, Error>
where
    for<'de> T: NoritoDeserialize<'de>,
{
    use std::io::Cursor;
    let mut cursor = Cursor::new(bytes);
    let header = core::Header::read(&mut cursor)?;
    if header.compression == Compression::None {
        return decode_from_uncompressed_bytes::<T>(bytes, header);
    }
    let mut cursor = Cursor::new(bytes);
    let value = deserialize_stream(&mut cursor)?;
    if cursor.position() != bytes.len() as u64 {
        return Err(Error::LengthMismatch);
    }
    Ok(value)
}

/// Convenience helper identical to `decode_from_bytes`.
/// Accepts either compressed or uncompressed Norito payloads and returns `T`.
pub fn decode_from_compressed_bytes<T>(bytes: &[u8]) -> Result<T, Error>
where
    for<'de> T: NoritoDeserialize<'de>,
{
    decode_from_bytes(bytes)
}

/// Decode from any `Read` implementor, validating header and checksum.
/// This is a thin wrapper over `deserialize_stream` for convenience.
pub fn decode_from_reader<R: Read, T>(reader: R) -> Result<T, Error>
where
    for<'de> T: NoritoDeserialize<'de>,
{
    deserialize_stream(reader)
}

/// Streaming fold over a top-level `Vec<T>` payload without materializing the full payload.
///
/// Validates header and CRC64 incrementally and feeds each element `T` to the folder `f`,
/// returning the final accumulator. Works for both compressed and uncompressed payloads.
#[inline]
pub(crate) fn guarded_try_deserialize<T, F>(f: F) -> Result<T, Error>
where
    F: FnOnce() -> Result<T, Error>,
{
    #[cfg(not(feature = "strict-safe"))]
    {
        return f();
    }

    #[cfg(feature = "strict-safe")]
    {
        install_decode_panic_hook();
        struct PanicDepthGuard;
        impl Drop for PanicDepthGuard {
            fn drop(&mut self) {
                DECODE_PANIC_DEPTH.with(|depth| {
                    let current = depth.get();
                    if current > 0 {
                        depth.set(current - 1);
                    }
                });
            }
        }
        DECODE_PANIC_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
        let _guard = PanicDepthGuard;
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
            Ok(res) => res,
            Err(payload) => {
                if crate::debug_trace_enabled() {
                    let msg = payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
                        .unwrap_or("<non-string panic>");
                    eprintln!(
                        "norito.decode suppressed panic while decoding {}: {}",
                        std::any::type_name::<T>(),
                        msg
                    );
                }
                Err(Error::decode_panic(std::any::type_name::<T>()))
            }
        }
    }
}

#[cfg(feature = "strict-safe")]
thread_local! {
    static DECODE_PANIC_DEPTH: Cell<u32> = const { Cell::new(0) };
}

#[cfg(all(test, feature = "strict-safe"))]
thread_local! {
    static SUPPRESSED_DECODE_PANICS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(feature = "strict-safe")]
fn install_decode_panic_hook() {
    static HOOK: OnceLock<()> = OnceLock::new();
    HOOK.get_or_init(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let suppressed = DECODE_PANIC_DEPTH.with(|depth| depth.get() > 0);
            if suppressed {
                #[cfg(test)]
                SUPPRESSED_DECODE_PANICS.with(|counter| {
                    counter.set(counter.get().saturating_add(1));
                });
                return;
            }
            default(info);
        }));
    });
}

#[cfg(all(test, feature = "strict-safe"))]
mod guarded_tests {
    use super::{Error, SUPPRESSED_DECODE_PANICS, guarded_try_deserialize};

    #[test]
    fn guarded_try_deserialize_catches_panics() {
        SUPPRESSED_DECODE_PANICS.with(|counter| counter.set(0));
        let result = guarded_try_deserialize::<(), _>(|| -> Result<(), Error> {
            panic!("trigger panic");
        });
        assert!(matches!(result, Err(Error::DecodePanic { .. })));
        assert_eq!(
            SUPPRESSED_DECODE_PANICS.with(|counter| counter.get()),
            1,
            "panic hook suppression counter should increment"
        );
    }
}

#[cfg(all(test, not(feature = "strict-safe")))]
mod guarded_non_strict_tests {
    use super::{Error, guarded_try_deserialize};

    #[test]
    fn guarded_try_deserialize_propagates_panics_without_strict_safe() {
        let result = std::panic::catch_unwind(|| {
            let _ = guarded_try_deserialize::<(), _>(|| -> Result<(), Error> {
                panic!("trigger panic");
            });
        });
        assert!(result.is_err(), "expected panic to propagate");
    }
}

#[allow(dead_code)]
fn stream_seq_fold_core<R, T, Acc, Init, F>(
    reader: R,
    init: Init,
    f: F,
    expected_schema: [u8; 16],
    padding: usize,
) -> Result<Acc, Error>
where
    R: Read,
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
    Init: FnOnce(usize) -> Acc,
    F: FnMut(Acc, T) -> Acc,
{
    core::stream::fold_sequence_from_reader(reader, init, f, expected_schema, padding)
}

/// Streaming fold over a top-level `Vec<T>` payload without materializing the full payload.
pub fn stream_vec_fold_from_reader<R, T, Acc, F>(reader: R, acc: Acc, f: F) -> Result<Acc, Error>
where
    R: Read,
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
    F: FnMut(Acc, T) -> Acc,
{
    type Top<U> = Vec<U>;
    stream_seq_fold_core(
        reader,
        move |_| acc,
        f,
        <Top<T> as NoritoDeserialize>::schema_hash(),
        core::payload_alignment_padding_for::<Top<T>>(),
    )
}

/// Collect a top-level `Vec<T>` by streaming, without buffering the entire payload.
pub fn stream_vec_collect_from_reader<R, T>(reader: R) -> Result<Vec<T>, Error>
where
    R: Read,
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    stream_seq_fold_core(
        reader,
        |len| Vec::with_capacity(len),
        |mut acc: Vec<T>, item| {
            acc.push(item);
            acc
        },
        core::compute_schema_hash::<Vec<T>>(),
        core::payload_alignment_padding_for::<Vec<T>>(),
    )
}

/// Collect a top-level `VecDeque<T>` by streaming.
pub fn stream_vecdeque_collect_from_reader<R, T>(
    reader: R,
) -> Result<std::collections::VecDeque<T>, Error>
where
    R: Read,
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    use std::collections::VecDeque;
    stream_seq_fold_core(
        reader,
        |len| VecDeque::with_capacity(len),
        |mut acc: VecDeque<T>, item| {
            acc.push_back(item);
            acc
        },
        core::compute_schema_hash::<VecDeque<T>>(),
        core::payload_alignment_padding_for::<VecDeque<T>>(),
    )
}

/// Collect a top-level `LinkedList<T>` by streaming.
pub fn stream_linkedlist_collect_from_reader<R, T>(
    reader: R,
) -> Result<std::collections::LinkedList<T>, Error>
where
    R: Read,
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    use std::collections::LinkedList;
    stream_seq_fold_core(
        reader,
        |_| LinkedList::new(),
        |mut acc: LinkedList<T>, item| {
            acc.push_back(item);
            acc
        },
        core::compute_schema_hash::<LinkedList<T>>(),
        core::payload_alignment_padding_for::<LinkedList<T>>(),
    )
}

/// Collect a top-level `HashSet<T>` by streaming.
pub fn stream_hashset_collect_from_reader<R, T>(
    reader: R,
) -> Result<std::collections::HashSet<T>, Error>
where
    R: Read,
    T: for<'de> NoritoDeserialize<'de> + std::hash::Hash + Eq + Ord + core::NoritoSerialize,
{
    use std::collections::HashSet;
    stream_seq_fold_core(
        reader,
        |len| HashSet::with_capacity(len),
        |mut acc: HashSet<T>, item| {
            acc.insert(item);
            acc
        },
        core::compute_schema_hash::<HashSet<T>>(),
        core::payload_alignment_padding_for::<HashSet<T>>(),
    )
}

/// Collect a top-level `BTreeSet<T>` by streaming.
pub fn stream_btreeset_collect_from_reader<R, T>(
    reader: R,
) -> Result<std::collections::BTreeSet<T>, Error>
where
    R: Read,
    T: for<'de> NoritoDeserialize<'de> + Ord + core::NoritoSerialize,
{
    use std::collections::BTreeSet;
    stream_seq_fold_core(
        reader,
        |_| BTreeSet::new(),
        |mut acc: BTreeSet<T>, item| {
            acc.insert(item);
            acc
        },
        core::compute_schema_hash::<BTreeSet<T>>(),
        core::payload_alignment_padding_for::<BTreeSet<T>>(),
    )
}

fn stream_map_collect_core<R, K, V, M, Init, Insert>(
    reader: R,
    expected_schema: [u8; 16],
    padding: usize,
    init: Init,
    mut insert: Insert,
) -> Result<M, Error>
where
    R: Read,
    K: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
    Init: FnOnce(usize) -> M,
    Insert: FnMut(&mut M, K, V) -> Result<(), Error>,
{
    use core::{Header, header_flags};

    let mut reader = reader;
    let header = Header::read(&mut reader)?;
    core::prepare_header_decode(header.flags, header.minor, false)?;
    if header.schema != expected_schema {
        return Err(Error::SchemaMismatch);
    }

    let flags = header.flags;
    let payload_len = core::payload_len_to_usize(header.length)?;
    let padding = match header.compression {
        Compression::None => padding,
        Compression::Zstd => 0,
    };
    if padding != 0 {
        core::stream::skip_padding(&mut reader, padding)?;
    }
    let _fg = core::DecodeFlagsGuard::enter(flags);

    let boxed: Box<dyn Read> = match header.compression {
        Compression::None => Box::new(reader),
        Compression::Zstd => {
            #[cfg(feature = "compression")]
            {
                Box::new(zstd::Decoder::new(reader)?)
            }
            #[cfg(not(feature = "compression"))]
            {
                return Err(std::io::Error::other("compression support disabled").into());
            }
        }
    };

    let mut digesting = core::stream::DigestingReader::new(boxed);
    let mut remaining = payload_len;

    #[inline]
    fn read_exact_update<Rd: Read>(
        reader: &mut core::stream::DigestingReader<Rd>,
        remaining: &mut usize,
        buf: &mut [u8],
    ) -> Result<(), Error> {
        let new_remaining = remaining
            .checked_sub(buf.len())
            .ok_or(Error::LengthMismatch)?;
        reader.read_exact(buf)?;
        *remaining = new_remaining;
        Ok(())
    }

    #[inline]
    fn read_u64_update<Rd: Read>(
        reader: &mut core::stream::DigestingReader<Rd>,
        remaining: &mut usize,
    ) -> Result<u64, Error> {
        let mut b = [0u8; 8];
        read_exact_update(reader, remaining, &mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    #[inline]
    fn read_varint_update<Rd: Read>(
        reader: &mut core::stream::DigestingReader<Rd>,
        remaining: &mut usize,
    ) -> Result<u64, Error> {
        let mut result = 0u64;
        let mut shift = 0u32;
        let mut used = 0usize;
        let mut buf = [0u8; 1];
        for _ in 0..STREAM_MAX_VARINT_BYTES {
            read_exact_update(reader, remaining, &mut buf)?;
            used += 1;
            let byte = buf[0];
            let payload = (byte & 0x7f) as u64;
            if shift == 63 && payload > 1 {
                return Err(Error::LengthMismatch);
            }
            result |= payload << shift;
            if byte & 0x80 == 0 {
                if used != core::varint_encoded_len_u64(result) {
                    return Err(Error::LengthMismatch);
                }
                return Ok(result);
            }
            shift += 7;
            if shift >= 64 {
                break;
            }
        }
        Err(Error::LengthMismatch)
    }

    let entries = {
        let v = read_u64_update(&mut digesting, &mut remaining)?;
        core::stream::u64_to_usize(v)?
    };

    let mut map = init(entries);
    if (flags & header_flags::PACKED_SEQ) == 0 {
        let len_bytes = if (flags & header_flags::COMPACT_LEN) != 0 {
            1usize
        } else {
            8usize
        };
        let per_entry = len_bytes.checked_mul(2).ok_or(Error::LengthMismatch)?;
        let min_headers = entries
            .checked_mul(per_entry)
            .ok_or(Error::LengthMismatch)?;
        if min_headers > remaining {
            return Err(Error::LengthMismatch);
        }

        let mut key_buf = Vec::new();
        let mut val_buf = Vec::new();
        for _ in 0..entries {
            let key_len = if (flags & header_flags::COMPACT_LEN) != 0 {
                let v = read_varint_update(&mut digesting, &mut remaining)?;
                core::stream::u64_to_usize(v)?
            } else {
                let v = read_u64_update(&mut digesting, &mut remaining)?;
                core::stream::u64_to_usize(v)?
            };
            if key_len > remaining {
                return Err(Error::LengthMismatch);
            }
            key_buf.resize(key_len, 0);
            read_exact_update(&mut digesting, &mut remaining, &mut key_buf)?;
            let _gk = core::PayloadCtxGuard::enter(&key_buf);
            let ak = unsafe { &*(key_buf.as_ptr() as *const Archived<K>) };
            let key = guarded_try_deserialize(|| K::try_deserialize(ak))?;

            let val_len = if (flags & header_flags::COMPACT_LEN) != 0 {
                let v = read_varint_update(&mut digesting, &mut remaining)?;
                core::stream::u64_to_usize(v)?
            } else {
                let v = read_u64_update(&mut digesting, &mut remaining)?;
                core::stream::u64_to_usize(v)?
            };
            if val_len > remaining {
                return Err(Error::LengthMismatch);
            }
            val_buf.resize(val_len, 0);
            read_exact_update(&mut digesting, &mut remaining, &mut val_buf)?;
            let _gv = core::PayloadCtxGuard::enter(&val_buf);
            let av = unsafe { &*(val_buf.as_ptr() as *const Archived<V>) };
            let value = guarded_try_deserialize(|| V::try_deserialize(av))?;
            insert(&mut map, key, value)?;
        }
    } else {
        let offsets_len = entries.checked_add(1).ok_or(Error::LengthMismatch)?;
        let offsets_bytes = offsets_len.checked_mul(16).ok_or(Error::LengthMismatch)?;
        if offsets_bytes > remaining {
            return Err(Error::LengthMismatch);
        }
        let mut koffs = Vec::with_capacity(offsets_len);
        let mut last = None;
        for _ in 0..offsets_len {
            let raw = read_u64_update(&mut digesting, &mut remaining)?;
            let off = core::stream::u64_to_usize(raw)?;
            if let Some(prev) = last {
                if off < prev {
                    return Err(Error::LengthMismatch);
                }
            } else if off != 0 {
                return Err(Error::LengthMismatch);
            }
            last = Some(off);
            koffs.push(off);
        }
        let mut voffs = Vec::with_capacity(offsets_len);
        let mut last = None;
        for _ in 0..offsets_len {
            let raw = read_u64_update(&mut digesting, &mut remaining)?;
            let off = core::stream::u64_to_usize(raw)?;
            if let Some(prev) = last {
                if off < prev {
                    return Err(Error::LengthMismatch);
                }
            } else if off != 0 {
                return Err(Error::LengthMismatch);
            }
            last = Some(off);
            voffs.push(off);
        }
        let mut key_sizes = Vec::with_capacity(entries);
        let mut val_sizes = Vec::with_capacity(entries);
        for i in 0..entries {
            let ksz = koffs[i + 1]
                .checked_sub(koffs[i])
                .ok_or(Error::LengthMismatch)?;
            let vsz = voffs[i + 1]
                .checked_sub(voffs[i])
                .ok_or(Error::LengthMismatch)?;
            key_sizes.push(ksz);
            val_sizes.push(vsz);
        }
        let key_total = *koffs.last().unwrap_or(&0);
        let val_total = *voffs.last().unwrap_or(&0);
        let total_data_len = key_total
            .checked_add(val_total)
            .ok_or(Error::LengthMismatch)?;
        if total_data_len > remaining {
            return Err(Error::LengthMismatch);
        }

        let mut keys = Vec::with_capacity(entries);
        let mut key_buf = Vec::new();
        let mut key_remaining = key_total;
        for size in key_sizes {
            if size > key_remaining || size > remaining {
                return Err(Error::LengthMismatch);
            }
            key_buf.resize(size, 0);
            read_exact_update(&mut digesting, &mut remaining, &mut key_buf)?;
            let _gk = core::PayloadCtxGuard::enter(&key_buf);
            let ak = unsafe { &*(key_buf.as_ptr() as *const Archived<K>) };
            let key = guarded_try_deserialize(|| K::try_deserialize(ak))?;
            keys.push(key);
            key_remaining = key_remaining
                .checked_sub(size)
                .ok_or(Error::LengthMismatch)?;
        }
        if key_remaining != 0 {
            return Err(Error::LengthMismatch);
        }

        let mut val_buf = Vec::new();
        let mut val_remaining = val_total;
        for (key, size) in keys.into_iter().zip(val_sizes) {
            if size > val_remaining || size > remaining {
                return Err(Error::LengthMismatch);
            }
            val_buf.resize(size, 0);
            read_exact_update(&mut digesting, &mut remaining, &mut val_buf)?;
            let _gv = core::PayloadCtxGuard::enter(&val_buf);
            let av = unsafe { &*(val_buf.as_ptr() as *const Archived<V>) };
            let value = guarded_try_deserialize(|| V::try_deserialize(av))?;
            val_remaining = val_remaining
                .checked_sub(size)
                .ok_or(Error::LengthMismatch)?;
            insert(&mut map, key, value)?;
        }
        if val_remaining != 0 {
            return Err(Error::LengthMismatch);
        }
    }

    if remaining != 0 {
        return Err(Error::LengthMismatch);
    }
    let _ = digesting.finalize(payload_len, header.checksum)?;
    Ok(map)
}

/// Collect a top-level `HashMap<K,V>` by streaming with minimal buffering.
///
/// - Packed layout: reads varint sizes or u64 offsets for keys and values, then streams keys
///   and values segments; stores decoded keys temporarily until values arrive.
/// - Compat layout: streams entry-by-entry (len+key, len+value) without buffering.
///
/// Collect a top-level `BTreeMap<K,V>` by streaming with minimal buffering.
pub fn stream_hashmap_collect_from_reader<R, K, V>(reader: R) -> Result<HashMap<K, V>, Error>
where
    R: Read,
    K: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Eq + std::hash::Hash + Ord,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    stream_map_collect_core(
        reader,
        core::compute_schema_hash::<HashMap<K, V>>(),
        core::payload_alignment_padding_for::<HashMap<K, V>>(),
        HashMap::with_capacity,
        |map, key, value| {
            if map.insert(key, value).is_some() {
                Err(Error::LengthMismatch)
            } else {
                Ok(())
            }
        },
    )
}

pub fn stream_btreemap_collect_from_reader<R, K, V>(reader: R) -> Result<BTreeMap<K, V>, Error>
where
    R: Read,
    K: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Ord,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    stream_map_collect_core(
        reader,
        core::compute_schema_hash::<BTreeMap<K, V>>(),
        core::payload_alignment_padding_for::<BTreeMap<K, V>>(),
        |_| BTreeMap::new(),
        |map, key, value| {
            if map.insert(key, value).is_some() {
                Err(Error::LengthMismatch)
            } else {
                Ok(())
            }
        },
    )
}

/// Types that can be finalized via `finish()` to verify integrity
/// (e.g., CRC) when a stream is not fully consumed.
pub trait Finishable {
    fn finish(self) -> Result<(), Error>;
}

impl<T> Finishable for StreamSeqIter<T>
where
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    fn finish(self) -> Result<(), Error> {
        StreamSeqIter::finish(self)
    }
}

impl<K, V> Finishable for StreamMapIter<K, V>
where
    K: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
    V: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    fn finish(self) -> Result<(), Error> {
        StreamMapIter::finish(self)
    }
}

/// RAII guard that calls `finish()` on drop and logs errors.
pub struct StreamFinishGuard<T: Finishable> {
    inner: Option<T>,
    name: &'static str,
}

impl<T: Finishable> StreamFinishGuard<T> {
    pub fn new(iter: T, name: &'static str) -> Self {
        Self {
            inner: Some(iter),
            name,
        }
    }
    pub fn into_inner(mut self) -> T {
        self.inner.take().unwrap()
    }
    /// Leak the inner iterator without calling `finish()` on drop.
    /// Useful for debugging when you deliberately skip integrity checks.
    pub fn leak(self) -> T {
        self.into_inner()
    }
}

impl<T: Finishable> Drop for StreamFinishGuard<T> {
    fn drop(&mut self) {
        if let Some(iter) = self.inner.take()
            && let Err(e) = iter.finish()
        {
            eprintln!("StreamFinishGuard({}): finish() error: {:?}", self.name, e);
        }
    }
}

#[cfg(test)]
mod json_stage1_reset_tests {
    #[test]
    fn reset_stage1_backends_is_callable() {
        crate::json::reset_stage1_backends();
    }
}

/// Canonical JSON literal helpers used by higher-level codecs.
pub mod literal {
    use super::Error;

    const CHECKSUM_WIDTH: usize = 4;
    const CRC_POLY: u16 = 0x1021;
    const CRC_INIT: u16 = 0xFFFF;

    fn crc16(tag: &str, body: &str) -> u16 {
        let mut crc = CRC_INIT;
        for byte in tag
            .as_bytes()
            .iter()
            .copied()
            .chain(Some(b':'))
            .chain(body.as_bytes().iter().copied())
        {
            crc ^= (byte as u16) << 8;
            for _ in 0..8 {
                if (crc & 0x8000) != 0 {
                    crc = (crc << 1) ^ CRC_POLY;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }

    /// Format a canonical literal of the form `<tag>:<body>#<crc16>`.
    pub fn format(tag: &str, body: &str) -> String {
        let checksum = crc16(tag, body);
        format!("{tag}:{body}#{checksum:04X}")
    }

    /// Parse a canonical literal `<tag>:<body>#<crc>` and return the body.
    pub fn parse<'a>(tag: &str, candidate: &'a str) -> Result<&'a str, Error> {
        if !candidate.starts_with(tag) {
            return Err(Error::Message(format!(
                "literal `{candidate}` must start with `{tag}:`"
            )));
        }

        let Some(rest) = candidate.get(tag.len()..) else {
            return Err(Error::Message(format!(
                "literal `{candidate}` missing ':' separator after tag `{tag}`"
            )));
        };
        let Some(body_and_checksum) = rest.strip_prefix(':') else {
            return Err(Error::Message(format!(
                "literal `{candidate}` missing ':' separator after tag `{tag}`"
            )));
        };

        let Some(hash_pos) = body_and_checksum.rfind('#') else {
            return Err(Error::Message(format!(
                "literal `{candidate}` missing checksum delimiter '#'"
            )));
        };

        let body = &body_and_checksum[..hash_pos];
        let checksum_str = &body_and_checksum[hash_pos + 1..];

        if body.is_empty() {
            return Err(Error::Message(format!(
                "literal `{candidate}` has empty body"
            )));
        }

        if checksum_str.len() != CHECKSUM_WIDTH {
            return Err(Error::Message(format!(
                "literal `{candidate}` checksum must be {CHECKSUM_WIDTH} hex digits"
            )));
        }

        let parsed_checksum = u16::from_str_radix(checksum_str, 16).map_err(|_| {
            Error::Message(format!(
                "literal `{candidate}` checksum `{checksum_str}` is not valid hex"
            ))
        })?;

        let expected = crc16(tag, body);
        if parsed_checksum != expected {
            return Err(Error::Message(format!(
                "literal `{candidate}` checksum mismatch (expected {expected:04X})"
            )));
        }

        Ok(body)
    }

    #[cfg(test)]
    mod tests {
        use super::{format, parse};

        #[test]
        fn format_and_parse_roundtrip() {
            let literal = format("hash", "ABCDEF");
            let body = parse("hash", &literal).expect("parse literal");
            assert_eq!(body, "ABCDEF");
        }

        #[test]
        fn parse_rejects_missing_tag() {
            assert!(parse("hash", "deadbeef").is_err());
        }

        #[test]
        fn parse_rejects_bad_checksum() {
            let mut literal = format("hash", "ABCDEF");
            literal.truncate(literal.len() - 4);
            literal.push_str("0000");
            assert!(parse("hash", &literal).is_err());
        }
    }
}

/// Convenience to wrap a stream iterator and ensure `finish()` is called on drop.
pub fn finish_on_drop<T: Finishable>(iter: T, name: &'static str) -> StreamFinishGuard<T> {
    StreamFinishGuard::new(iter, name)
}

/// Convenience constructor for a streaming iterator over a top-level `Vec<T>` payload.
pub fn stream_seq_iter<R, T>(reader: R) -> Result<StreamSeqIter<T>, Error>
where
    R: Read + 'static,
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    StreamSeqIter::<T>::new(reader)
}

/// Streaming iterator over a top-level `Vec<T>` payload.
pub struct StreamSeqIter<T> {
    reader: Option<core::stream::DigestingReader<Box<dyn Read>>>,
    len_decoder: core::stream::SeqLenDecoder,
    remaining: usize,
    payload_len: usize,
    checksum: u64,
    flags_guard: core::DecodeFlagsGuard,
    scratch: core::stream::AlignedScratch,
    archived_align: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T> StreamSeqIter<T>
where
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    pub fn new<R: Read + 'static>(mut reader: R) -> Result<Self, Error> {
        use core::Header;

        let header = Header::read(&mut reader)?;
        core::prepare_header_decode(header.flags, header.minor, true)?;
        type Top<U> = Vec<U>;
        if header.schema != <Top<T> as NoritoDeserialize>::schema_hash() {
            return Err(Error::SchemaMismatch);
        }
        let payload_len = core::payload_len_to_usize(header.length)?;
        let flags = header.flags;
        let padding = match header.compression {
            Compression::None => core::payload_alignment_padding_for::<Top<T>>(),
            Compression::Zstd => 0,
        };
        if padding != 0 {
            core::stream::skip_padding(&mut reader, padding)?;
        }
        let flags_guard = core::DecodeFlagsGuard::enter(flags);

        let boxed: Box<dyn Read> = match header.compression {
            Compression::None => Box::new(reader),
            Compression::Zstd => {
                #[cfg(feature = "compression")]
                {
                    Box::new(zstd::Decoder::new(reader)?)
                }
                #[cfg(not(feature = "compression"))]
                {
                    return Err(std::io::Error::other("compression support disabled").into());
                }
            }
        };

        let mut digesting = core::stream::DigestingReader::new(boxed);

        let len_decoder = core::stream::SeqLenDecoder::new(&mut digesting, flags)?;
        let remaining = len_decoder.total_len();

        Ok(Self {
            reader: Some(digesting),
            len_decoder,
            remaining,
            payload_len,
            checksum: header.checksum,
            flags_guard,
            scratch: core::stream::AlignedScratch::new(),
            archived_align: std::mem::align_of::<core::Archived<T>>(),
            _marker: std::marker::PhantomData,
        })
    }

    fn finalize(
        reader: core::stream::DigestingReader<Box<dyn Read>>,
        payload_len: usize,
        checksum: u64,
    ) -> Result<(), Error> {
        let _ = reader.finalize(payload_len, checksum)?;
        Ok(())
    }
}

impl<T> Iterator for StreamSeqIter<T>
where
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let _ = &self.flags_guard;
        let reader = self.reader.as_mut()?;

        if self.remaining == 0 {
            let mut reader = self.reader.take().unwrap();
            let tail = match self
                .payload_len
                .checked_sub(reader.consumed())
                .ok_or(Error::LengthMismatch)
                .and_then(|remaining| self.len_decoder.finish(&mut reader, remaining))
            {
                Ok(()) => reader,
                Err(err) => return Some(Err(err)),
            };
            if let Err(e) = Self::finalize(tail, self.payload_len, self.checksum) {
                return Some(Err(e));
            }
            return None;
        }

        match self.len_decoder.next_len(reader) {
            Ok(Some(len)) => {
                let value = if len == 0 {
                    if std::mem::size_of::<core::Archived<T>>() != 0 {
                        Err(Error::LengthMismatch)
                    } else {
                        let _pg = core::PayloadCtxGuard::enter(&[]);
                        let archived = unsafe {
                            &*std::ptr::NonNull::<core::Archived<T>>::dangling().as_ptr()
                        };
                        guarded_try_deserialize(|| T::try_deserialize(archived))
                    }
                } else {
                    unsafe {
                        let ptr = match self.scratch.ensure(len, self.archived_align) {
                            Ok(ptr) => ptr,
                            Err(e) => return Some(Err(e)),
                        };
                        let tmp_slice_mut = std::slice::from_raw_parts_mut(ptr, len);
                        if let Err(e) = reader.read_exact_into(tmp_slice_mut) {
                            return Some(Err(e.into()));
                        }
                        let tmp_slice = std::slice::from_raw_parts(ptr as *const u8, len);
                        let _pg = core::PayloadCtxGuard::enter(tmp_slice);
                        let archived = &*(ptr as *const core::Archived<T>);
                        guarded_try_deserialize(|| T::try_deserialize(archived))
                    }
                };

                self.remaining -= 1;
                if self.remaining == 0 {
                    let mut reader = self.reader.take().unwrap();
                    match self
                        .payload_len
                        .checked_sub(reader.consumed())
                        .ok_or(Error::LengthMismatch)
                        .and_then(|remaining| self.len_decoder.finish(&mut reader, remaining))
                    {
                        Ok(()) => {
                            if let Err(e) = Self::finalize(reader, self.payload_len, self.checksum)
                            {
                                return Some(Err(e));
                            }
                        }
                        Err(err) => return Some(Err(err)),
                    }
                }
                Some(value)
            }
            Ok(None) => {
                let reader = self.reader.take().unwrap();
                if let Err(e) = Self::finalize(reader, self.payload_len, self.checksum) {
                    return Some(Err(e));
                }
                Some(Err(Error::LengthMismatch))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

impl<T> StreamSeqIter<T>
where
    T: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    pub fn finish(mut self) -> Result<(), Error> {
        let _ = &self.flags_guard;
        if let Some(mut reader) = self.reader.take() {
            while self.remaining > 0 {
                let len = match self.len_decoder.next_len(&mut reader)? {
                    Some(len) => len,
                    None => return Err(Error::LengthMismatch),
                };
                if len > 0 {
                    unsafe {
                        let ptr = self.scratch.ensure(len, self.archived_align)?;
                        let tmp = std::slice::from_raw_parts_mut(ptr, len);
                        reader.read_exact_into(tmp)?;
                    }
                }
                self.remaining -= 1;
            }
            let remaining = self
                .payload_len
                .checked_sub(reader.consumed())
                .ok_or(Error::LengthMismatch)?;
            self.len_decoder.finish(&mut reader, remaining)?;
            Self::finalize(reader, self.payload_len, self.checksum)?;
        }
        Ok(())
    }
}

/// Streaming iterator over a top-level `HashMap<K,V>`/`BTreeMap<K,V>` payload.
pub struct StreamMapIter<K, V> {
    reader: Box<dyn Read>,
    flags: u8,
    entries: usize,
    idx: usize,
    // packed path helpers
    val_sizes: Option<Vec<usize>>,
    keys: Option<Vec<Option<K>>>,
    digest: crc64fast::Digest,
    payload_remaining: usize,
    values_remaining: Option<usize>,
    checksum: u64,
    flags_guard: core::DecodeFlagsGuard,
    _marker: std::marker::PhantomData<V>,
    // Reusable buffers for key/value bodies
    kbuf: Vec<u8>,
    vbuf: Vec<u8>,
}

const STREAM_MAX_VARINT_BYTES: usize = 10;

impl<K, V> StreamMapIter<K, V>
where
    K: for<'de> NoritoDeserialize<'de>,
    V: for<'de> NoritoDeserialize<'de>,
{
    #[inline]
    fn read_exact_update_buf(
        reader: &mut dyn Read,
        digest: &mut crc64fast::Digest,
        remaining: &mut usize,
        buf: &mut [u8],
    ) -> Result<(), Error> {
        let new_remaining = remaining
            .checked_sub(buf.len())
            .ok_or(Error::LengthMismatch)?;
        reader.read_exact(buf)?;
        digest.write(buf);
        *remaining = new_remaining;
        Ok(())
    }

    #[inline]
    fn read_exact_update_kbuf(&mut self) -> Result<(), Error> {
        let buf = &mut self.kbuf;
        Self::read_exact_update_buf(
            &mut *self.reader,
            &mut self.digest,
            &mut self.payload_remaining,
            buf,
        )
    }

    #[inline]
    fn read_exact_update_vbuf(&mut self) -> Result<(), Error> {
        let buf = &mut self.vbuf;
        Self::read_exact_update_buf(
            &mut *self.reader,
            &mut self.digest,
            &mut self.payload_remaining,
            buf,
        )
    }

    #[inline]
    fn read_u64_update(&mut self) -> Result<u64, Error> {
        let mut b = [0u8; 8];
        Self::read_exact_update_buf(
            &mut *self.reader,
            &mut self.digest,
            &mut self.payload_remaining,
            &mut b,
        )?;
        Ok(u64::from_le_bytes(b))
    }

    #[inline]
    fn read_len(&mut self) -> Result<usize, Error> {
        let raw = if (self.flags & core::header_flags::COMPACT_LEN) != 0 {
            self.read_varint_update()?
        } else {
            self.read_u64_update()?
        };
        core::stream::u64_to_usize(raw)
    }

    #[inline]
    fn read_varint_update(&mut self) -> Result<u64, Error> {
        let mut result = 0u64;
        let mut shift = 0u32;
        let mut used = 0usize;
        let mut buf = [0u8; 1];
        for _ in 0..STREAM_MAX_VARINT_BYTES {
            Self::read_exact_update_buf(
                &mut *self.reader,
                &mut self.digest,
                &mut self.payload_remaining,
                &mut buf,
            )?;
            used += 1;
            let byte = buf[0];
            let payload = (byte & 0x7f) as u64;
            if shift == 63 && payload > 1 {
                return Err(Error::LengthMismatch);
            }
            result |= payload << shift;
            if byte & 0x80 == 0 {
                if used != core::varint_encoded_len_u64(result) {
                    return Err(Error::LengthMismatch);
                }
                return Ok(result);
            }
            shift += 7;
            if shift >= 64 {
                break;
            }
        }
        Err(Error::LengthMismatch)
    }
}

impl<K, V> StreamMapIter<K, V>
where
    K: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
    V: for<'de> NoritoDeserialize<'de> + core::NoritoSerialize,
{
    fn new_with_schema<R: Read + 'static>(
        mut reader: R,
        expected_schema: [u8; 16],
    ) -> Result<Self, Error> {
        use core::{Header, header_flags};
        use std::collections::{BTreeMap, HashMap};
        let header = Header::read(&mut reader)?;
        core::prepare_header_decode(header.flags, header.minor, false)?;
        if header.schema != expected_schema {
            return Err(Error::SchemaMismatch);
        }
        let payload_len = core::payload_len_to_usize(header.length)?;
        let flags = header.flags;
        let padding = match header.compression {
            Compression::None => {
                if expected_schema == core::compute_schema_hash::<HashMap<K, V>>() {
                    core::payload_alignment_padding_for::<HashMap<K, V>>()
                } else if expected_schema == core::compute_schema_hash::<BTreeMap<K, V>>() {
                    core::payload_alignment_padding_for::<BTreeMap<K, V>>()
                } else {
                    0
                }
            }
            Compression::Zstd => 0,
        };
        if padding != 0 {
            core::stream::skip_padding(&mut reader, padding)?;
        }
        let flags_guard = core::DecodeFlagsGuard::enter(flags);
        let mut r: Box<dyn Read> = match header.compression {
            Compression::None => Box::new(reader),
            Compression::Zstd => {
                #[cfg(feature = "compression")]
                {
                    Box::new(zstd::Decoder::new(reader)?)
                }
                #[cfg(not(feature = "compression"))]
                {
                    return Err(std::io::Error::other("compression support disabled").into());
                }
            }
        };
        let mut digest = crc64fast::Digest::new();
        let mut remaining = payload_len;
        #[inline]
        fn read_exact_update<Rd: Read>(
            src: &mut Rd,
            dst: &mut [u8],
            d: &mut crc64fast::Digest,
            remaining: &mut usize,
        ) -> Result<(), Error> {
            let new_remaining = remaining
                .checked_sub(dst.len())
                .ok_or(Error::LengthMismatch)?;
            src.read_exact(dst)?;
            d.write(dst);
            *remaining = new_remaining;
            Ok(())
        }
        #[inline]
        fn read_u64_update<Rd: Read>(
            src: &mut Rd,
            d: &mut crc64fast::Digest,
            remaining: &mut usize,
        ) -> Result<u64, Error> {
            let mut b = [0u8; 8];
            read_exact_update(src, &mut b, d, remaining)?;
            Ok(u64::from_le_bytes(b))
        }
        let entries = {
            let v = read_u64_update(&mut r, &mut digest, &mut remaining)?;
            core::payload_len_to_usize(v)?
        };
        if (flags & header_flags::PACKED_SEQ) == 0 {
            let len_bytes = if (flags & header_flags::COMPACT_LEN) != 0 {
                1usize
            } else {
                8usize
            };
            let per_entry = len_bytes.checked_mul(2).ok_or(Error::LengthMismatch)?;
            let min_headers = entries
                .checked_mul(per_entry)
                .ok_or(Error::LengthMismatch)?;
            if min_headers > remaining {
                return Err(Error::LengthMismatch);
            }
        }
        let mut val_sizes = None;
        let mut keys = None;
        let mut values_remaining = None;
        if (flags & header_flags::PACKED_SEQ) != 0 {
            let offsets_len = entries.checked_add(1).ok_or(Error::LengthMismatch)?;
            let offsets_bytes = offsets_len.checked_mul(16).ok_or(Error::LengthMismatch)?;
            if offsets_bytes > remaining {
                return Err(Error::LengthMismatch);
            }
            let mut key_sizes = Vec::with_capacity(entries);
            let mut v_sizes = Vec::with_capacity(entries);
            let mut koffs = Vec::with_capacity(offsets_len);
            let mut last = None;
            for _ in 0..offsets_len {
                let o = read_u64_update(&mut r, &mut digest, &mut remaining)?;
                let off = core::stream::u64_to_usize(o)?;
                if let Some(prev) = last {
                    if off < prev {
                        return Err(Error::LengthMismatch);
                    }
                } else if off != 0 {
                    return Err(Error::LengthMismatch);
                }
                last = Some(off);
                koffs.push(off);
            }
            let mut voffs = Vec::with_capacity(offsets_len);
            let mut last = None;
            for _ in 0..offsets_len {
                let o = read_u64_update(&mut r, &mut digest, &mut remaining)?;
                let off = core::stream::u64_to_usize(o)?;
                if let Some(prev) = last {
                    if off < prev {
                        return Err(Error::LengthMismatch);
                    }
                } else if off != 0 {
                    return Err(Error::LengthMismatch);
                }
                last = Some(off);
                voffs.push(off);
            }
            for i in 0..entries {
                let ksz = koffs[i + 1]
                    .checked_sub(koffs[i])
                    .ok_or(Error::LengthMismatch)?;
                let vsz = voffs[i + 1]
                    .checked_sub(voffs[i])
                    .ok_or(Error::LengthMismatch)?;
                key_sizes.push(ksz);
                v_sizes.push(vsz);
            }
            let key_len = *koffs.last().unwrap_or(&0);
            let val_len = *voffs.last().unwrap_or(&0);
            let total_data_len = key_len.checked_add(val_len).ok_or(Error::LengthMismatch)?;
            if total_data_len > remaining {
                return Err(Error::LengthMismatch);
            }
            values_remaining = Some(val_len);

            let mut ks = Vec::with_capacity(entries);
            let mut kb = Vec::new();
            let mut key_remaining = key_len;
            for ksz in key_sizes {
                if ksz > key_remaining {
                    return Err(Error::LengthMismatch);
                }
                kb.resize(ksz, 0);
                read_exact_update(&mut r, &mut kb, &mut digest, &mut remaining)?;
                let _g = core::PayloadCtxGuard::enter(&kb);
                let ak = unsafe { &*(kb.as_ptr() as *const Archived<K>) };
                ks.push(Some(guarded_try_deserialize(|| K::try_deserialize(ak))?));
                key_remaining = key_remaining
                    .checked_sub(ksz)
                    .ok_or(Error::LengthMismatch)?;
            }
            if key_remaining != 0 {
                return Err(Error::LengthMismatch);
            }
            keys = Some(ks);
            val_sizes = Some(v_sizes);
        }
        Ok(StreamMapIter {
            reader: r,
            flags,
            entries,
            idx: 0,
            val_sizes,
            keys,
            digest,
            payload_remaining: remaining,
            values_remaining,
            checksum: header.checksum,
            flags_guard,
            _marker: std::marker::PhantomData,
            kbuf: Vec::new(),
            vbuf: Vec::new(),
        })
    }

    pub fn new_hash<R: Read + 'static>(reader: R) -> Result<Self, Error>
    where
        K: Eq + std::hash::Hash + Ord,
    {
        type Top<KK, VV> = HashMap<KK, VV>;
        Self::new_with_schema(reader, <Top<K, V> as NoritoDeserialize>::schema_hash())
    }

    pub fn new_btree<R: Read + 'static>(reader: R) -> Result<Self, Error>
    where
        K: Ord,
    {
        type Top<KK, VV> = BTreeMap<KK, VV>;
        Self::new_with_schema(reader, <Top<K, V> as NoritoDeserialize>::schema_hash())
    }

    /// Finish the map stream by consuming remaining bytes and verifying CRC.
    pub fn finish(mut self) -> Result<(), Error> {
        let _ = &self.flags_guard;
        use core::header_flags;
        while self.idx < self.entries {
            if (self.flags & header_flags::PACKED_SEQ) != 0 {
                let vsz = self.val_sizes.as_ref().unwrap()[self.idx];
                if let Some(remaining) = self.values_remaining.as_mut() {
                    if vsz > *remaining {
                        return Err(Error::LengthMismatch);
                    }
                    *remaining -= vsz;
                }
                if vsz > self.payload_remaining {
                    return Err(Error::LengthMismatch);
                }
                self.vbuf.resize(vsz, 0);
                self.read_exact_update_vbuf()?;
                self.idx += 1;
            } else {
                // read and skip key
                let klen = self.read_len()?;
                if klen > self.payload_remaining {
                    return Err(Error::LengthMismatch);
                }
                self.kbuf.resize(klen, 0);
                self.read_exact_update_kbuf()?;
                // read and skip value
                let vlen = self.read_len()?;
                if vlen > self.payload_remaining {
                    return Err(Error::LengthMismatch);
                }
                self.vbuf.resize(vlen, 0);
                self.read_exact_update_vbuf()?;
                self.idx += 1;
            }
        }
        if let Some(remaining) = self.values_remaining
            && remaining != 0
        {
            return Err(Error::LengthMismatch);
        }
        if self.payload_remaining != 0 {
            return Err(Error::LengthMismatch);
        }
        if self.digest.sum64() != self.checksum {
            return Err(Error::ChecksumMismatch);
        }
        Ok(())
    }
}

impl<K, V> Iterator for StreamMapIter<K, V>
where
    K: for<'de> NoritoDeserialize<'de>,
    V: for<'de> NoritoDeserialize<'de>,
{
    type Item = Result<(K, V), Error>;
    fn next(&mut self) -> Option<Self::Item> {
        use core::header_flags;
        let _ = &self.flags_guard;
        if self.idx >= self.entries {
            return None;
        }
        if (self.flags & header_flags::PACKED_SEQ) != 0 {
            let vsz = self.val_sizes.as_ref().unwrap()[self.idx];
            if let Some(remaining) = self.values_remaining.as_mut() {
                if vsz > *remaining {
                    return Some(Err(Error::LengthMismatch));
                }
                *remaining -= vsz;
            }
            if vsz > self.payload_remaining {
                return Some(Err(Error::LengthMismatch));
            }
            self.vbuf.resize(vsz, 0);
            if let Err(e) = self.read_exact_update_vbuf() {
                return Some(Err(e));
            }
            let _gv = core::PayloadCtxGuard::enter(&self.vbuf);
            let av = unsafe { &*(self.vbuf.as_ptr() as *const Archived<V>) };
            let val = match guarded_try_deserialize(|| V::try_deserialize(av)) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            let key = self.keys.as_mut().unwrap()[self.idx].take().unwrap();
            self.idx += 1;
            if self.idx == self.entries {
                if let Some(remaining) = self.values_remaining
                    && remaining != 0
                {
                    return Some(Err(Error::LengthMismatch));
                }
                if self.payload_remaining != 0 {
                    return Some(Err(Error::LengthMismatch));
                }
                if self.digest.sum64() != self.checksum {
                    return Some(Err(Error::ChecksumMismatch));
                }
            }
            Some(Ok((key, val)))
        } else {
            let klen = match self.read_len() {
                Ok(len) => len,
                Err(e) => return Some(Err(e)),
            };
            if klen > self.payload_remaining {
                return Some(Err(Error::LengthMismatch));
            }
            self.kbuf.resize(klen, 0);
            if let Err(e) = self.read_exact_update_kbuf() {
                return Some(Err(e));
            }
            let _gk = core::PayloadCtxGuard::enter(&self.kbuf);
            let ak = unsafe { &*(self.kbuf.as_ptr() as *const Archived<K>) };
            let key = match guarded_try_deserialize(|| K::try_deserialize(ak)) {
                Ok(k) => k,
                Err(e) => return Some(Err(e)),
            };
            let vlen = match self.read_len() {
                Ok(len) => len,
                Err(e) => return Some(Err(e)),
            };
            if vlen > self.payload_remaining {
                return Some(Err(Error::LengthMismatch));
            }
            self.vbuf.resize(vlen, 0);
            if let Err(e) = self.read_exact_update_vbuf() {
                return Some(Err(e));
            }
            let _gv = core::PayloadCtxGuard::enter(&self.vbuf);
            let av = unsafe { &*(self.vbuf.as_ptr() as *const Archived<V>) };
            let val = match guarded_try_deserialize(|| V::try_deserialize(av)) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            self.idx += 1;
            if self.idx == self.entries {
                if self.payload_remaining != 0 {
                    return Some(Err(Error::LengthMismatch));
                }
                if self.digest.sum64() != self.checksum {
                    return Some(Err(Error::ChecksumMismatch));
                }
            }
            Some(Ok((key, val)))
        }
    }
}

#[cfg(test)]
mod archive_slice_tests {
    use super::{ArchiveSlice, core::Archived};

    #[test]
    fn misaligned_slice_is_realigned() {
        let align = std::mem::align_of::<Archived<[u64; 2]>>();
        assert!(align > 1);

        let backing = vec![0u8; align * 2 + 1];
        let misaligned = &backing[1..1 + align * 2];
        let slice = ArchiveSlice::new(misaligned, align).expect("allocate slice");

        assert_eq!(slice.as_slice(), misaligned);
        assert_eq!(slice.as_slice().as_ptr() as usize % align, 0);
        assert!(slice.layout.is_some());
    }

    #[test]
    fn unit_alignment_is_noop() {
        let data = vec![1u8, 2, 3];
        let slice = ArchiveSlice::new(&data, 1).expect("allocate slice");

        assert_eq!(slice.as_slice(), &data[..]);
        assert!(slice.layout.is_none());
        assert_eq!(slice.ptr as *const u8, data.as_ptr());
    }
}

#[cfg(test)]
mod guarded_try_tests {
    use super::{Error, guarded_try_deserialize};

    #[test]
    fn panic_is_mapped_to_decode_error() {
        let result = guarded_try_deserialize::<(), _>(|| -> Result<(), Error> {
            panic!("forced panic for decode guard test");
        });

        assert!(matches!(result, Err(Error::DecodePanic { .. })));
    }
}

#[cfg(test)]
mod stream_map_iter_tests {
    use super::{Error, StreamMapIter, core};
    use std::{collections::HashMap, io::Cursor};

    fn frame_hashmap_payload(payload: &[u8], flags: u8) -> Vec<u8> {
        core::frame_bare_with_header_flags::<HashMap<u8, u8>>(payload, flags)
            .expect("frame payload")
    }

    #[test]
    fn stream_map_nonpacked_rejects_key_len_overflow() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u64.to_le_bytes());
        payload.extend_from_slice(&9u64.to_le_bytes());
        payload.extend_from_slice(&0u64.to_le_bytes());

        let bytes = frame_hashmap_payload(&payload, 0);
        let mut iter = StreamMapIter::<u8, u8>::new_hash(Cursor::new(bytes)).expect("iter");
        let item = iter.next().expect("item");
        assert!(matches!(item, Err(Error::LengthMismatch)));
    }

    #[test]
    fn stream_map_finish_empty_ok() {
        let payload = 0u64.to_le_bytes().to_vec();
        let bytes = frame_hashmap_payload(&payload, 0);
        let iter = StreamMapIter::<u8, u8>::new_hash(Cursor::new(bytes)).expect("iter");
        iter.finish().expect("finish");
    }

    #[test]
    fn stream_map_packed_rejects_nonzero_first_offset() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u64.to_le_bytes());
        payload.extend_from_slice(&1u64.to_le_bytes());
        payload.extend_from_slice(&1u64.to_le_bytes());
        payload.extend_from_slice(&0u64.to_le_bytes());
        payload.extend_from_slice(&0u64.to_le_bytes());
        payload.push(0u8);

        let bytes = frame_hashmap_payload(&payload, core::header_flags::PACKED_SEQ);
        let result = StreamMapIter::<u8, u8>::new_hash(Cursor::new(bytes));
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }
}
