//! Norito core serialization library.
//!
//! This crate provides deterministic serialization and zero-copy deserialization
//! for common Rust data structures including primitives, strings, vectors, maps,
//! and simple structs.
//!
//! The format begins with a small metadata header followed by a byte buffer
//! containing archived values. Every value has a corresponding [`Archived`] type
//! which represents the layout in the buffer.

use core::convert::TryFrom;
use std::{
    alloc::{Layout, handle_alloc_error},
    any::TypeId,
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque},
    hash::Hash,
    io::{Read, Write},
    marker::PhantomData,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
#[cfg(feature = "derive")]
pub use norito_derive::{NoritoDeserialize, NoritoSerialize};
use sha2::{Digest, Sha256};

#[cfg(feature = "schema-structural")]
use crate::json;
use crate::{ArchiveSlice, guarded_try_deserialize};

pub mod heuristics;
pub mod hw;
pub mod simd_crc64;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub use simd_crc64::crc64_neon;
#[cfg(all(target_arch = "x86_64", target_feature = "sse4.2"))]
pub use simd_crc64::crc64_sse42;
pub use simd_crc64::{crc64_fallback, hardware_crc64};

#[cfg(feature = "gpu-compression")]
pub mod gpu_zstd;

/// Default upper bound on Norito archive length (bytes) when hosts do not
/// provide an explicit configuration.
const DEFAULT_MAX_ARCHIVE_LEN: u64 = 64 * 1024 * 1024; // 64 MiB

static MAX_ARCHIVE_LEN: AtomicU64 = AtomicU64::new(DEFAULT_MAX_ARCHIVE_LEN);

fn serialize_owned<W: Write, T: NoritoSerialize>(mut writer: W, value: &T) -> Result<(), Error> {
    let _flags_guard = if decode_flags_active() {
        None
    } else {
        Some(DecodeFlagsGuard::enter(default_encode_flags()))
    };
    let flags = effective_layout_flags();
    let exact_len = value.encoded_len_exact();
    let hint_len = exact_len.or_else(|| value.encoded_len_hint());
    let mut payload = Vec::new();
    if let Some(hint) = hint_len {
        payload
            .try_reserve(hint)
            .map_err(|_| Error::LengthMismatch)?;
    }
    value.serialize(&mut payload)?;
    let len = u64::try_from(payload.len()).map_err(|_| Error::LengthMismatch)?;
    write_len_with_flags(&mut writer, len, flags)?;
    writer.write_all(&payload)?;
    Ok(())
}

#[cfg(test)]
mod serialize_owned_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static EXACT_CALLS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Clone, Copy)]
    struct ExactLen(u8);

    impl NoritoSerialize for ExactLen {
        fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
            writer.write_all(&[self.0])?;
            Ok(())
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            EXACT_CALLS.fetch_add(1, Ordering::Relaxed);
            Some(1)
        }
    }

    #[test]
    fn serialize_owned_uses_encoded_len_exact_when_available() {
        EXACT_CALLS.store(0, Ordering::Relaxed);
        reset_decode_state();
        let value = Box::new(ExactLen(0xAB));
        let mut buf = Vec::new();
        value.serialize(&mut buf).expect("serialize owned payload");
        let (payload, used) = parse_owned_payload(&buf).expect("parse owned payload");
        assert_eq!(used, buf.len());
        assert_eq!(payload, &[0xAB]);
        assert_eq!(EXACT_CALLS.load(Ordering::Relaxed), 1);
        reset_decode_state();
    }

    #[derive(Clone, Copy)]
    struct BadExactLen;

    impl NoritoSerialize for BadExactLen {
        fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
            writer.write_all(&[0xAA, 0xBB])?;
            Ok(())
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            Some(1)
        }
    }

    #[test]
    fn serialize_owned_recomputes_length_when_exact_is_wrong() {
        reset_decode_state();
        let value = Box::new(BadExactLen);
        let mut buf = Vec::new();
        value.serialize(&mut buf).expect("serialize owned payload");
        let (payload, used) = parse_owned_payload(&buf).expect("parse owned payload");
        assert_eq!(used, buf.len());
        assert_eq!(payload, &[0xAA, 0xBB]);
        reset_decode_state();
    }
}

fn owned_bytes_from_ctx<A>(archived: &Archived<A>) -> Result<&[u8], Error> {
    let ptr = archived as *const _ as *const u8;
    let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
    let offset = (ptr as usize)
        .checked_sub(base)
        .ok_or(Error::LengthMismatch)?;
    if offset > total {
        return Err(Error::LengthMismatch);
    }
    let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
    Ok(&payload[offset..])
}

fn parse_owned_payload(bytes: &[u8]) -> Result<(&[u8], usize), Error> {
    let (len, hdr) = read_len_from_slice(bytes)?;
    let end = hdr.checked_add(len).ok_or(Error::LengthMismatch)?;
    if end > bytes.len() {
        return Err(Error::LengthMismatch);
    }
    Ok((&bytes[hdr..end], end))
}

/// Override the maximum allowed Norito archive length (bytes).
///
/// Hosts should call this during initialization using the configured limit. A
/// value of `0` disables the explicit cap and falls back to the platform pointer
/// width (`usize::MAX`).
pub fn set_max_archive_len(limit: u64) {
    let resolved = if limit == 0 { u64::MAX } else { limit };
    let capped = resolved.min(usize::MAX as u64);
    MAX_ARCHIVE_LEN.store(capped, Ordering::Relaxed);
}

/// Effective maximum Norito archive length in bytes.
#[inline]
pub fn max_archive_len() -> u64 {
    MAX_ARCHIVE_LEN
        .load(Ordering::Relaxed)
        .min(usize::MAX as u64)
}

/// Convert a header-declared length into a `usize`, enforcing the configured cap.
pub(crate) fn payload_len_to_usize(length: u64) -> Result<usize, Error> {
    let limit = max_archive_len();
    if length > limit {
        return Err(Error::ArchiveLengthExceeded { length, limit });
    }
    usize::try_from(length).map_err(|_| Error::ArchiveLengthExceeded { length, limit })
}

#[inline]
pub(crate) fn len_u64_to_usize(length: u64) -> Result<usize, Error> {
    usize::try_from(length).map_err(|_| Error::LengthMismatch)
}

/// Maximum padding (bytes) allowed between a Norito header and the payload when
/// the target type's alignment is unknown.
const MAX_HEADER_PADDING: usize = 64;

/// Strip alignment padding that was inserted between the header and the payload.
///
/// Rejects inputs whose leading bytes exceed `max_padding` to prevent accepting
/// arbitrarily padded archives.
pub(crate) fn payload_without_leading_padding(
    slice: &[u8],
    payload_len: usize,
    max_padding: usize,
) -> Result<&[u8], Error> {
    if slice.len() < payload_len {
        return Err(Error::LengthMismatch);
    }
    let padding = slice.len() - payload_len;
    if padding > max_padding {
        return Err(Error::LengthMismatch);
    }
    if padding != 0 && slice[..padding].iter().any(|&b| b != 0) {
        return Err(Error::LengthMismatch);
    }
    Ok(match padding {
        0 => slice,
        _ => &slice[padding..],
    })
}

/// Strip alignment padding between the header and payload, requiring an exact
/// padding length and zero-filled padding bytes.
pub(crate) fn payload_without_leading_padding_exact(
    slice: &[u8],
    payload_len: usize,
    padding: usize,
) -> Result<&[u8], Error> {
    let expected = padding
        .checked_add(payload_len)
        .ok_or(Error::LengthMismatch)?;
    if slice.len() != expected {
        return Err(Error::LengthMismatch);
    }
    if padding != 0 && slice[..padding].iter().any(|&b| b != 0) {
        return Err(Error::LengthMismatch);
    }
    Ok(&slice[padding..])
}

const TYPE_NAME_SCHEMA_HASH_DOMAIN: &[u8] = b"norito:v1:type-name\0";
#[cfg(feature = "schema-structural")]
const STRUCTURAL_SCHEMA_HASH_DOMAIN: &[u8] = b"norito:v1:structural-schema\0";

/// CRC64 polynomial used to compute integrity checks.
/// Header flags stored in the final padding byte.
pub mod header_flags {
    /// Packed sequence layouts are used for variable-sized collections.
    pub const PACKED_SEQ: u8 = 0x01;
    /// Compact varint lengths are used for per-field/element length prefixes
    /// (including string/blob lengths). Does not affect packed-seq offsets or
    /// the outer sequence length header.
    pub const COMPACT_LEN: u8 = 0x02;
    /// Packed struct layout (offsets + data) for derive-generated types.
    pub const PACKED_STRUCT: u8 = 0x04;
    /// Reserved in v1; packed sequences always use fixed u64 offsets.
    pub const VARINT_OFFSETS: u8 = 0x08;
    /// Reserved in v1; sequence length headers are fixed u64.
    pub const COMPACT_SEQ_LEN: u8 = 0x10;
    /// Packed-struct omits per-field sizes for self-delimiting/fixed-size fields
    /// and prefixes a compact bitset indicating which fields carry an explicit size.
    /// Applies only when packed-struct and compact-len are enabled.
    pub const FIELD_BITSET: u8 = 0x20;
}

fn schema_hash_with_domain(domain: &[u8], bytes: &[u8]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn type_name_schema_hash_from_bytes(bytes: &[u8]) -> [u8; 16] {
    schema_hash_with_domain(TYPE_NAME_SCHEMA_HASH_DOMAIN, bytes)
}

#[cfg(feature = "schema-structural")]
fn structural_schema_hash_from_bytes(bytes: &[u8]) -> [u8; 16] {
    schema_hash_with_domain(STRUCTURAL_SCHEMA_HASH_DOMAIN, bytes)
}

/// Generate a 16-byte schema hash for type `T`.
///
/// The hash is derived from the domain-prefixed SHA-256 digest of the fully
/// qualified type name, truncated to 16 bytes.
pub(crate) fn compute_schema_hash<T>() -> [u8; 16] {
    let name = core::any::type_name::<T>();
    type_name_schema_hash_from_bytes(name.as_bytes())
}

/// Public helper to compute type-name based schema hash (fallback when structural schema is not used).
pub fn type_name_schema_hash<T>() -> [u8; 16] {
    compute_schema_hash::<T>()
}

/// Compute a type-name-based schema hash for an arbitrary type name string.
pub fn schema_hash_for_name(name: &str) -> [u8; 16] {
    type_name_schema_hash_from_bytes(name.as_bytes())
}

#[cfg(feature = "schema-structural")]
/// Compute a structural schema hash using `iroha_schema`'s canonical serialization.
pub fn schema_hash_structural<T: iroha_schema::IntoSchema>() -> [u8; 16] {
    let map = <T as iroha_schema::IntoSchema>::schema();
    schema_hash_from_json_serializable(&map)
}

#[cfg(feature = "schema-structural")]
fn schema_hash_from_json_serializable<T>(value: &T) -> [u8; 16]
where
    T: json::JsonSerialize + ?Sized,
{
    let mut out = String::new();
    json::JsonSerialize::json_serialize(value, &mut out);
    structural_schema_hash_from_bytes(out.as_bytes())
}

#[cfg(feature = "schema-structural")]
/// Compute a structural schema hash from a pre-built Norito JSON [`Value`].
///
/// The `Value` is serialized using Norito's canonical writer to guarantee the
/// same byte representation as other language bindings (Python, Java).
pub fn schema_hash_structural_value(value: &json::Value) -> [u8; 16] {
    schema_hash_from_json_serializable(value)
}

#[cfg(feature = "schema-structural")]
/// Compute a structural schema hash from a JSON string descriptor.
///
/// The descriptor is parsed using Norito's native JSON parser and then
/// canonicalized using the same writer as [`schema_hash_structural_value`].
pub fn schema_hash_structural_from_json_str(input: &str) -> Result<[u8; 16], json::Error> {
    let value = json::parse_value(input)?;
    Ok(schema_hash_structural_value(&value))
}

#[cfg(feature = "schema-structural")]
/// Compute a structural schema hash from UTF-8 JSON bytes.
pub fn schema_hash_structural_from_json_bytes(bytes: &[u8]) -> Result<[u8; 16], json::Error> {
    let input = std::str::from_utf8(bytes).map_err(|_| json::Error::InvalidUtf8)?;
    schema_hash_structural_from_json_str(input)
}

/// Compute the CRC64-XZ checksum over `data` using the ECMA polynomial.
fn crc64(data: &[u8]) -> u64 {
    simd_crc64::hardware_crc64(data)
}

const MAX_VARINT_BYTES: usize = 10;

/// Number of bytes used to encode a length prefix for `value`.
///
/// Honors the active `COMPACT_LEN` layout flag so callers can pre-compute
/// per-value length prefix sizes accurately.
pub fn len_prefix_len(value: usize) -> usize {
    len_prefix_len_with_flags(value, effective_layout_flags())
}

/// Number of bytes used to encode a length prefix for `value` under `flags`.
///
/// This is the flag-explicit variant of [`len_prefix_len`] for tight loops
/// that have already captured the active Norito layout.
#[doc(hidden)]
pub fn len_prefix_len_with_flags(value: usize, flags: u8) -> usize {
    if compact_len_enabled_for_flags(flags) {
        varint_encoded_len(value as u64)
    } else {
        8
    }
}

#[inline]
fn len_prefixed_payload_len_with_flags(payload_len: usize, flags: u8) -> Option<usize> {
    len_prefix_len_with_flags(payload_len, flags).checked_add(payload_len)
}

#[inline]
fn len_prefixed_payload_len(payload_len: usize) -> Option<usize> {
    len_prefixed_payload_len_with_flags(payload_len, effective_layout_flags())
}

#[inline]
fn tagged_len_prefixed_payload_len_with_flags(payload_len: usize, flags: u8) -> Option<usize> {
    1usize
        .checked_add(len_prefix_len_with_flags(payload_len, flags))?
        .checked_add(payload_len)
}

#[inline]
fn tagged_len_prefixed_payload_len(payload_len: usize) -> Option<usize> {
    tagged_len_prefixed_payload_len_with_flags(payload_len, effective_layout_flags())
}

/// Number of bytes used to encode a sequence length prefix for `value`.
///
/// Sequence length headers are fixed-width in v1.
pub fn seq_len_prefix_len(value: usize) -> usize {
    let _ = value;
    8
}

/// Number of bytes used to encode a compact varint length prefix.
pub fn varint_len_prefix_len(value: usize) -> usize {
    varint_encoded_len(value as u64)
}

#[inline]
pub const fn should_emit_varint_tail(_len: usize) -> bool {
    let _ = _len;
    false
}

#[inline]
/// Return the set of Norito header layout flags supported by this build.
pub const fn supported_header_flags() -> u8 {
    // Mask of known header flag bits allowed in v1 headers.
    header_flags::PACKED_SEQ
        | header_flags::COMPACT_LEN
        | header_flags::PACKED_STRUCT
        | header_flags::FIELD_BITSET
}

#[inline]
/// Validate that a Norito header flag byte uses only supported v1 layout combinations.
pub fn validate_header_flags(flags: u8) -> Result<(), Error> {
    let unsupported_layout = flags & !supported_header_flags();
    if unsupported_layout != 0 {
        return Err(Error::UnsupportedFeature("layout flag"));
    }
    if (flags & header_flags::FIELD_BITSET) != 0 {
        let required = header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN;
        if (flags & required) != required {
            return Err(Error::UnsupportedFeature("layout flag combination"));
        }
    }
    Ok(())
}

#[inline]
fn sanitize_layout_flags(flags: u8) -> u8 {
    flags & supported_header_flags()
}

/// Return the default v1 encode layout flags.
///
/// The v1 minor byte remains fixed at [`VERSION_MINOR`]; payloads advertise
/// layout selections through header flags. Compact per-value lengths are the
/// default because they reduce wire size without changing collection offsets or
/// sequence length headers.
#[inline]
pub const fn default_encode_flags() -> u8 {
    V1_DECODE_FLAGS | header_flags::COMPACT_LEN
}

#[derive(Clone)]
struct PayloadCtxState {
    base: usize,
    len: usize,
    schema: Option<[u8; 16]>,
    max_access: usize,
    flags: u8,
    flags_hint: u8,
    flags_active: bool,
}

// Decode context managed per-thread; set based on header flags.
thread_local! {
    static DECODE_FLAGS: Cell<u8> = const { Cell::new(0) };
}

thread_local! {
    static DECODE_FLAGS_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

thread_local! {
    static DECODE_FLAGS_HINT: Cell<u8> = const { Cell::new(0) };
}

thread_local! {
    static ENCODE_CONTEXT_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

thread_local! {
    static ENCODE_PACKED_FIXED_USED: Cell<bool> = const { Cell::new(false) };
}

thread_local! {
    static ENCODE_FIELD_BITSET_USED: Cell<bool> = const { Cell::new(false) };
}

thread_local! {
    static ENCODE_COMPACT_LEN_USED: Cell<bool> = const { Cell::new(false) };
}

thread_local! {
    static LAST_HEADER_FLAGS: Cell<Option<u8>> = const { Cell::new(None) };
}

thread_local! {
    static FORCE_SEQUENTIAL: Cell<bool> = const { Cell::new(false) };
}

pub(crate) struct EncodeContextGuard {
    prev_active: bool,
    prev_fixed_used: bool,
    prev_field_bitset_used: bool,
    prev_compact_len_used: bool,
}

impl EncodeContextGuard {
    pub(crate) fn enter() -> Self {
        let prev_active = ENCODE_CONTEXT_ACTIVE.with(|cell| {
            let prev = cell.get();
            cell.set(true);
            prev
        });
        let prev_fixed_used = ENCODE_PACKED_FIXED_USED.with(|cell| {
            let prev = cell.get();
            cell.set(false);
            prev
        });
        let prev_field_bitset_used = ENCODE_FIELD_BITSET_USED.with(|cell| {
            let prev = cell.get();
            cell.set(false);
            prev
        });
        let prev_compact_len_used = ENCODE_COMPACT_LEN_USED.with(|cell| {
            let prev = cell.get();
            cell.set(false);
            prev
        });
        Self {
            prev_active,
            prev_fixed_used,
            prev_field_bitset_used,
            prev_compact_len_used,
        }
    }
}

pub struct SequentialOverrideGuard {
    prev: bool,
}

impl SequentialOverrideGuard {
    pub fn enter() -> Self {
        let prev = FORCE_SEQUENTIAL.with(|cell| {
            let prev = cell.get();
            cell.set(true);
            prev
        });
        #[cfg(debug_assertions)]
        if crate::debug_trace_enabled() {
            eprintln!("SequentialOverrideGuard::enter prev={prev}");
        }
        Self { prev }
    }
}

impl Drop for SequentialOverrideGuard {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if crate::debug_trace_enabled() {
            eprintln!(
                "SequentialOverrideGuard::drop restoring prev={prev}",
                prev = self.prev
            );
        }
        FORCE_SEQUENTIAL.with(|cell| cell.set(self.prev));
    }
}

impl Drop for EncodeContextGuard {
    fn drop(&mut self) {
        ENCODE_CONTEXT_ACTIVE.with(|cell| cell.set(self.prev_active));
        ENCODE_PACKED_FIXED_USED.with(|cell| cell.set(self.prev_fixed_used));
        ENCODE_FIELD_BITSET_USED.with(|cell| cell.set(self.prev_field_bitset_used));
        ENCODE_COMPACT_LEN_USED.with(|cell| cell.set(self.prev_compact_len_used));
    }
}

fn mark_fixed_offsets_used_if_encoding() {
    ENCODE_CONTEXT_ACTIVE.with(|active| {
        if active.get() {
            ENCODE_PACKED_FIXED_USED.with(|flag| flag.set(true));
        }
    });
}

/// Record that a packed sequence emitted fixed-width offsets in the current encode pass.
pub fn note_fixed_offsets_emitted() {
    mark_fixed_offsets_used_if_encoding();
}

fn mark_compact_len_used_if_encoding() {
    ENCODE_CONTEXT_ACTIVE.with(|active| {
        if active.get() {
            ENCODE_COMPACT_LEN_USED.with(|flag| flag.set(true));
        }
    });
}

pub fn note_compact_len_emitted() {
    mark_compact_len_used_if_encoding();
    #[cfg(debug_assertions)]
    if crate::debug_trace_enabled() {
        eprintln!(
            "note_compact_len_emitted compact_len_used={}",
            ENCODE_COMPACT_LEN_USED.with(|flag| flag.get())
        );
    }
}

pub fn mark_field_bitset_used_if_encoding() {
    ENCODE_CONTEXT_ACTIVE.with(|active| {
        if active.get() {
            ENCODE_FIELD_BITSET_USED.with(|flag| flag.set(true));
        }
    });
}

pub(crate) fn fixed_offsets_used() -> bool {
    ENCODE_PACKED_FIXED_USED.with(|flag| flag.get())
}

/// Return whether field bitsets were emitted during the current encode pass.
pub(crate) fn field_bitset_used() -> bool {
    ENCODE_FIELD_BITSET_USED.with(|flag| flag.get())
}

pub(crate) fn compact_len_used() -> bool {
    ENCODE_COMPACT_LEN_USED.with(|flag| flag.get())
}

pub(crate) fn record_last_header_flags(flags: u8) {
    let sanitized = flags & supported_header_flags();
    LAST_HEADER_FLAGS.with(|cell| cell.set(Some(sanitized)));
}

pub(crate) fn take_last_header_flags() -> Option<u8> {
    LAST_HEADER_FLAGS.with(|cell| cell.replace(None))
}

// Thread-local payload context: base pointer, length, and optional schema hash.
thread_local! {
    static DECODE_PAYLOAD_CTX: RefCell<Option<PayloadCtxState>> = const { RefCell::new(None) };
    static DECODE_ROOT_SPAN: RefCell<Option<(usize, usize)>> = const { RefCell::new(None) };
}

/// Record the root payload span for the current thread.
pub fn set_decode_root(bytes: &[u8]) {
    DECODE_ROOT_SPAN.with(|slot| *slot.borrow_mut() = Some((bytes.as_ptr() as usize, bytes.len())));
}

/// Clear the root payload span for the current thread.
pub fn clear_decode_root() {
    DECODE_ROOT_SPAN.with(|slot| *slot.borrow_mut() = None);
}

/// Fetch the currently recorded root payload span, if any.
pub fn payload_root_span() -> Option<(usize, usize)> {
    DECODE_ROOT_SPAN.with(|slot| *slot.borrow())
}

// Per-thread payload context (base pointer, length, and optional schema hash).
fn set_payload_ctx_state(
    bytes: &[u8],
    schema: Option<[u8; 16]>,
    flags: Option<u8>,
    hint: Option<u8>,
) {
    set_payload_ctx_state_with_len(bytes, bytes.len(), schema, flags, hint);
}

fn set_payload_ctx_state_with_len(
    bytes: &[u8],
    len: usize,
    schema: Option<[u8; 16]>,
    flags: Option<u8>,
    hint: Option<u8>,
) {
    let len = len.min(bytes.len());
    DECODE_PAYLOAD_CTX.with(|c| {
        let flags_value = flags.unwrap_or(0);
        let hint_value = hint.unwrap_or(flags_value);
        *c.borrow_mut() = Some(PayloadCtxState {
            base: bytes.as_ptr() as usize,
            len,
            schema,
            max_access: 0,
            flags: flags_value,
            flags_hint: hint_value,
            flags_active: flags.is_some() || hint.is_some(),
        });
    });
}

/// Set the current payload context for slice-based decoders.
pub fn set_payload_ctx(bytes: &[u8]) {
    set_payload_ctx_state(bytes, None, None, None);
}

/// Set the payload context and record negotiated decode flags.
pub fn set_payload_ctx_with_flags(bytes: &[u8], flags: u8) {
    set_payload_ctx_state(bytes, None, Some(flags), Some(flags));
}

/// Clear the payload context.
pub fn clear_payload_ctx() {
    DECODE_PAYLOAD_CTX.with(|c| *c.borrow_mut() = None);
}

pub fn payload_ctx() -> Option<(usize, usize)> {
    payload_ctx_state().map(|state| (state.base, state.len))
}

fn payload_ctx_flags() -> Option<(u8, u8)> {
    payload_ctx_state().and_then(|state| {
        if state.flags_active {
            Some((state.flags, state.flags_hint))
        } else {
            None
        }
    })
}

fn payload_ctx_state() -> Option<PayloadCtxState> {
    DECODE_PAYLOAD_CTX.with(|c| c.borrow().clone())
}

fn payload_ctx_state_mut<R>(f: impl FnOnce(&mut PayloadCtxState) -> R) -> Option<R> {
    DECODE_PAYLOAD_CTX.with(|c| {
        let mut guard = c.borrow_mut();
        guard.as_mut().map(f)
    })
}

fn record_payload_access(ptr: *const u8, len: usize) {
    if len == 0 {
        return;
    }
    payload_ctx_state_mut(|state| {
        let base = state.base;
        let ptr_us = ptr as usize;
        if ptr_us < base {
            return;
        }
        let rel = ptr_us - base;
        let end = rel.saturating_add(len);
        if end > state.len {
            state.max_access = state.len;
        } else if end > state.max_access {
            state.max_access = end;
        }
    });
}

pub(crate) fn payload_ctx_max_access() -> Option<usize> {
    payload_ctx_state().map(|state| state.max_access)
}

fn record_slice_access(bytes: &[u8], len: usize) {
    let cap = bytes.len();
    let used = len.min(cap);
    if used == 0 {
        return;
    }
    record_payload_access(bytes.as_ptr(), used);
}

/// Record that `len` bytes of `bytes` were accessed while decoding within an active payload
/// context.
///
/// This is intended for custom `NoritoDeserialize` implementations that can validate full
/// consumption without re-encoding, allowing `decode_field_canonical` to skip canonical-length
/// recomputation.
pub fn note_payload_access(bytes: &[u8], len: usize) {
    record_slice_access(bytes, len);
}

#[inline]
fn payload_bytes_with_offset(ptr: *const u8) -> Result<(&'static [u8], usize), Error> {
    let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
    let ptr_us = ptr as usize;
    let base_end = base.checked_add(total).ok_or(Error::LengthMismatch)?;
    if ptr_us >= base && ptr_us <= base_end {
        let offset = ptr_us - base;
        let payload = unsafe { core::slice::from_raw_parts(base as *const u8, total) };
        return Ok((payload, offset));
    }

    if let Some((root_base, root_len)) = payload_root_span() {
        let root_end = root_base
            .checked_add(root_len)
            .ok_or(Error::LengthMismatch)?;
        if ptr_us >= root_base && ptr_us <= root_end {
            payload_ctx_state_mut(|state| {
                if state.max_access < state.len {
                    state.max_access = state.len;
                }
            });
            let payload = unsafe { core::slice::from_raw_parts(root_base as *const u8, root_len) };
            let offset = ptr_us - root_base;
            return Ok((payload, offset));
        }
    }

    Err(Error::LengthMismatch)
}

#[inline]
pub fn payload_slice_from_ptr(ptr: *const u8) -> Result<&'static [u8], Error> {
    let (payload, offset) = payload_bytes_with_offset(ptr)?;
    Ok(&payload[offset..])
}

#[inline]
fn payload_range_from_ptr(ptr: *const u8, len: usize) -> Result<&'static [u8], Error> {
    let (payload, offset) = payload_bytes_with_offset(ptr)?;
    let end = offset.checked_add(len).ok_or(Error::LengthMismatch)?;
    if end > payload.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(&payload[offset..end])
}

#[inline]
fn ensure_len_within_remaining(len: usize, remaining: usize) -> Result<(), Error> {
    if len > remaining {
        return Err(Error::LengthMismatch);
    }
    Ok(())
}

#[cfg(test)]
#[inline]
fn btreemap_entry_slices<'a>(
    key_bytes: &'a [u8],
    value_bytes: &'a [u8],
    ks: usize,
    ke: usize,
    vs: usize,
    ve: usize,
    index: usize,
) -> Result<(&'a [u8], &'a [u8]), Error> {
    let Some(key_slice) = key_bytes.get(ks..ke) else {
        #[cfg(debug_assertions)]
        eprintln!(
            "norito: BTreeMap slice_oob key i={index} ks={ks} ke={ke} len={}",
            key_bytes.len()
        );
        return Err(Error::LengthMismatch);
    };
    let Some(value_slice) = value_bytes.get(vs..ve) else {
        #[cfg(debug_assertions)]
        eprintln!(
            "norito: BTreeMap slice_oob val i={index} vs={vs} ve={ve} len={}",
            value_bytes.len()
        );
        return Err(Error::LengthMismatch);
    };
    if crate::debug_trace_enabled() {
        eprintln!(
            "norito: BTreeMap entry[{index}] ks={ks} ke={ke} vs={vs} ve={ve} key_len={} val_len={}",
            key_slice.len(),
            value_slice.len()
        );
    }
    record_slice_access(key_slice, key_slice.len());
    record_slice_access(value_slice, value_slice.len());
    Ok((key_slice, value_slice))
}

pub fn set_payload_ctx_with_schema(bytes: &[u8], schema: [u8; 16]) {
    set_payload_ctx_state(bytes, Some(schema), None, None);
}

/// Set payload context with both schema and negotiated decode flags.
pub fn set_payload_ctx_with_schema_and_flags(bytes: &[u8], schema: [u8; 16], flags: u8) {
    set_payload_ctx_state(bytes, Some(schema), Some(flags), Some(flags));
}

#[inline]
#[allow(dead_code)]
fn safe_read_u64_from_ctx(ptr: *const u8, offset: usize) -> Option<u64> {
    let (base, total) = payload_ctx()?;
    let ptr_us = ptr as usize;
    if ptr_us < base {
        return None;
    }
    let base_offset = ptr_us - base;
    let off = base_offset.checked_add(offset)?;
    let slice_end = off.checked_add(8)?;
    if slice_end > total {
        return None;
    }
    let payload = unsafe { core::slice::from_raw_parts(base as *const u8, total) };
    let mut lb = [0u8; 8];
    lb.copy_from_slice(&payload[off..slice_end]);
    Some(u64::from_le_bytes(lb))
}

/// Read a dynamic length header at a given pointer using the current payload context.
/// Returns (value, bytes_consumed).
fn read_len_dyn_at_ptr(ptr: *const u8) -> Result<(usize, usize), Error> {
    let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
    let ptr_us = ptr as usize;
    let base_end = base.checked_add(total).ok_or(Error::LengthMismatch)?;
    if ptr_us < base || ptr_us >= base_end {
        #[cfg(debug_assertions)]
        if crate::debug_trace_enabled() {
            eprintln!(
                "read_len_dyn_at_ptr: ptr={ptr:?} base=0x{base:016x} total={total} base_end=0x{base_end:016x}"
            );
        }
        return Err(Error::LengthMismatch);
    }
    let off = ptr_us - base;
    debug_assert!(off <= total);
    let head_slice = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
    let head = &head_slice[off..];
    let (len, used) = read_len_dyn_slice(head)?;
    record_payload_access(ptr, used);
    Ok((len, used))
}

/// Allocate a raw buffer for `layout`, aborting with `handle_alloc_error` on OOM.
///
/// Returns a pair of `(ptr, needs_dealloc)` where `needs_dealloc` indicates whether
/// the pointer must be passed to `dealloc` when it is no longer needed.
#[inline]
unsafe fn alloc_checked(layout: Layout) -> (*mut u8, bool) {
    if layout.size() == 0 {
        let align = layout.align();
        // For ZSTs, return an arbitrary non-null pointer that satisfies the
        // required alignment so downstream casts remain well-formed.
        let ptr = align as *mut u8;
        debug_assert_eq!((ptr as usize) & (align - 1), 0);
        return (ptr, false);
    }
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        handle_alloc_error(layout);
    }
    (ptr, true)
}

#[inline]
unsafe fn dealloc_checked(ptr: *mut u8, layout: Layout, needs_dealloc: bool) {
    if needs_dealloc {
        unsafe { std::alloc::dealloc(ptr, layout) };
    }
}

#[inline]
unsafe fn copy_from_payload(src: *const u8, dst: *mut u8, len: usize) -> Result<(), Error> {
    if len == 0 {
        return Ok(());
    }
    let slice = payload_range_from_ptr(src, len)?;
    record_payload_access(src, len);
    let dst_slice = unsafe { core::slice::from_raw_parts_mut(dst, len) };
    dst_slice.copy_from_slice(slice);
    Ok(())
}

/// RAII guard to set and restore the current payload context.
pub struct PayloadCtxGuard {
    prev: Option<PayloadCtxState>,
    flags_guard: Option<DecodeFlagsGuard>,
}
impl PayloadCtxGuard {
    pub fn enter(bytes: &[u8]) -> Self {
        let prev = payload_ctx_state();
        let schema = prev.as_ref().and_then(|state| state.schema);
        let (flags_opt, hint_opt) = if let Some(ref state) = prev {
            if state.flags_active {
                (Some(state.flags), Some(state.flags_hint))
            } else if let Some(effective) = current_decode_flags_effective() {
                (Some(effective), Some(effective))
            } else {
                (None, None)
            }
        } else if let Some(effective) = current_decode_flags_effective() {
            (Some(effective), Some(effective))
        } else {
            (None, None)
        };
        set_payload_ctx_state(bytes, schema, flags_opt, hint_opt);
        let flags_guard = if !decode_flags_active() {
            match (flags_opt, hint_opt) {
                (Some(flags), Some(hint)) => Some(DecodeFlagsGuard::enter_with_hint(flags, hint)),
                (Some(flags), None) => Some(DecodeFlagsGuard::enter_with_hint(flags, flags)),
                (None, Some(hint)) => Some(DecodeFlagsGuard::enter_with_hint(hint, hint)),
                _ => None,
            }
        } else {
            None
        };
        PayloadCtxGuard { prev, flags_guard }
    }

    /// Install a payload context with an explicit logical length while preserving
    /// any active schema/flag state from the current decode context.
    pub fn enter_with_len(bytes: &[u8], logical_len: usize) -> Self {
        let prev = payload_ctx_state();
        let schema = prev.as_ref().and_then(|state| state.schema);
        let (flags_opt, hint_opt) = if let Some(ref state) = prev {
            if state.flags_active {
                (Some(state.flags), Some(state.flags_hint))
            } else if let Some(effective) = current_decode_flags_effective() {
                (Some(effective), Some(effective))
            } else {
                (None, None)
            }
        } else if let Some(effective) = current_decode_flags_effective() {
            (Some(effective), Some(effective))
        } else {
            (None, None)
        };
        set_payload_ctx_state_with_len(bytes, logical_len, schema, flags_opt, hint_opt);
        let flags_guard = if !decode_flags_active() {
            match (flags_opt, hint_opt) {
                (Some(flags), Some(hint)) => Some(DecodeFlagsGuard::enter_with_hint(flags, hint)),
                (Some(flags), None) => Some(DecodeFlagsGuard::enter_with_hint(flags, flags)),
                (None, Some(hint)) => Some(DecodeFlagsGuard::enter_with_hint(hint, hint)),
                _ => None,
            }
        } else {
            None
        };
        PayloadCtxGuard { prev, flags_guard }
    }

    pub fn enter_with_schema(bytes: &[u8], schema: [u8; 16]) -> Self {
        let prev = payload_ctx_state();
        set_payload_ctx_with_schema(bytes, schema);
        PayloadCtxGuard {
            prev,
            flags_guard: None,
        }
    }

    pub fn enter_with_flags(bytes: &[u8], flags: u8) -> Self {
        let prev = payload_ctx_state();
        let prev_active = decode_flags_active();
        let prev_flags = get_decode_flags();
        let prev_hint = DECODE_FLAGS_HINT.with(|c| c.get());
        set_payload_ctx_with_flags(bytes, flags);
        let needs_guard = !prev_active || prev_flags != flags || prev_hint != flags;
        let flags_guard = if needs_guard {
            Some(DecodeFlagsGuard::enter_with_hint(flags, flags))
        } else {
            None
        };
        PayloadCtxGuard { prev, flags_guard }
    }

    pub fn enter_with_flags_hint(bytes: &[u8], flags: u8, hint: u8) -> Self {
        let prev = payload_ctx_state();
        let prev_active = decode_flags_active();
        let prev_flags = get_decode_flags();
        let prev_hint = DECODE_FLAGS_HINT.with(|c| c.get());
        set_payload_ctx_state(bytes, None, Some(flags), Some(hint));
        let needs_guard = !prev_active || prev_flags != flags || prev_hint != hint;
        let flags_guard = if needs_guard {
            Some(DecodeFlagsGuard::enter_with_hint(flags, hint))
        } else {
            None
        };
        PayloadCtxGuard { prev, flags_guard }
    }

    pub fn enter_with_schema_and_flags(bytes: &[u8], schema: [u8; 16], flags: u8) -> Self {
        let prev = payload_ctx_state();
        let prev_active = decode_flags_active();
        let prev_flags = get_decode_flags();
        let prev_hint = DECODE_FLAGS_HINT.with(|c| c.get());
        set_payload_ctx_with_schema_and_flags(bytes, schema, flags);
        let needs_guard = !prev_active || prev_flags != flags || prev_hint != flags;
        let flags_guard = if needs_guard {
            Some(DecodeFlagsGuard::enter_with_hint(flags, flags))
        } else {
            None
        };
        PayloadCtxGuard { prev, flags_guard }
    }

    pub fn enter_with_schema_flags_hint(
        bytes: &[u8],
        schema: [u8; 16],
        flags: u8,
        hint: u8,
    ) -> Self {
        let prev = payload_ctx_state();
        let prev_active = decode_flags_active();
        let prev_flags = get_decode_flags();
        let prev_hint = DECODE_FLAGS_HINT.with(|c| c.get());
        set_payload_ctx_state(bytes, Some(schema), Some(flags), Some(hint));
        let needs_guard = !prev_active || prev_flags != flags || prev_hint != hint;
        let flags_guard = if needs_guard {
            Some(DecodeFlagsGuard::enter_with_hint(flags, hint))
        } else {
            None
        };
        PayloadCtxGuard { prev, flags_guard }
    }

    /// Install a payload context using `logical_len` bytes from `bytes` while tracking
    /// decode flags.
    pub fn enter_with_flags_len(bytes: &[u8], logical_len: usize, flags: u8) -> Self {
        let prev = payload_ctx_state();
        let prev_active = decode_flags_active();
        let prev_flags = get_decode_flags();
        let prev_hint = DECODE_FLAGS_HINT.with(|c| c.get());
        set_payload_ctx_state_with_len(bytes, logical_len, None, Some(flags), Some(flags));
        let needs_guard = !prev_active || prev_flags != flags || prev_hint != flags;
        let flags_guard = if needs_guard {
            Some(DecodeFlagsGuard::enter_with_hint(flags, flags))
        } else {
            None
        };
        PayloadCtxGuard { prev, flags_guard }
    }

    /// Install a payload context using `logical_len` bytes and explicit flags + hint.
    pub fn enter_with_flags_hint_len(
        bytes: &[u8],
        logical_len: usize,
        flags: u8,
        hint: u8,
    ) -> Self {
        let prev = payload_ctx_state();
        let prev_active = decode_flags_active();
        let prev_flags = get_decode_flags();
        let prev_hint = DECODE_FLAGS_HINT.with(|c| c.get());
        set_payload_ctx_state_with_len(bytes, logical_len, None, Some(flags), Some(hint));
        let needs_guard = !prev_active || prev_flags != flags || prev_hint != hint;
        let flags_guard = if needs_guard {
            Some(DecodeFlagsGuard::enter_with_hint(flags, hint))
        } else {
            None
        };
        PayloadCtxGuard { prev, flags_guard }
    }
}
impl Drop for PayloadCtxGuard {
    fn drop(&mut self) {
        // Drop the decode flags guard first so the previous decode state is restored before
        // we reconcile the payload context stack.
        self.flags_guard.take();
        DECODE_PAYLOAD_CTX.with(|cell| {
            let mut slot = cell.borrow_mut();
            let current = slot.take();
            let restored = match (&self.prev, current) {
                (Some(prev_state), Some(curr_state)) => {
                    let mut merged = prev_state.clone();
                    if curr_state.max_access > 0 {
                        if curr_state.base == merged.base && curr_state.len == merged.len {
                            if curr_state.max_access > merged.max_access {
                                merged.max_access = curr_state.max_access;
                            }
                        } else if curr_state.base >= merged.base
                            && curr_state.base < merged.base + merged.len
                        {
                            let rel = curr_state.base - merged.base;
                            let end = rel.saturating_add(curr_state.max_access);
                            if end > merged.max_access {
                                merged.max_access = merged.len.min(end);
                            }
                        }
                    }
                    Some(merged)
                }
                (prev, _) => prev.clone(),
            };
            *slot = restored;
        });
    }
}

/// Set decode flags for the current thread.
fn set_decode_flags_raw(flags: u8) {
    DECODE_FLAGS.with(|c| c.set(sanitize_layout_flags(flags)));
}

fn set_decode_flags_active(active: bool) {
    DECODE_FLAGS_ACTIVE.with(|c| c.set(active));
}

pub fn set_decode_flags(flags: u8) {
    set_decode_flags_raw(flags);
    set_decode_flags_active(true);
    set_decode_flags_hint(flags);
}

/// Get decode flags for the current thread.
pub fn get_decode_flags() -> u8 {
    DECODE_FLAGS.with(|c| c.get())
}

pub(crate) fn decode_flags_active() -> bool {
    DECODE_FLAGS_ACTIVE.with(|c| c.get())
}

fn set_decode_flags_hint(flags: u8) {
    DECODE_FLAGS_HINT.with(|c| c.set(sanitize_layout_flags(flags)));
}

// Effective flags for encode/decode: if none were set explicitly for the
// current thread, fall back to compile-time defaults so that bare encoders
// produce compact layouts when the crate is compiled with `compact-len`.
// Note: removed — encoding uses explicit defaults via Encode guard; decoders
// rely on flags set by header or caller.

/// RAII guard to restore previous decode flags.
pub struct DecodeFlagsGuard {
    prev_flags: u8,
    prev_hint: u8,
    prev_active: bool,
}
impl DecodeFlagsGuard {
    pub fn enter(flags: u8) -> Self {
        Self::enter_with_hint(flags, flags)
    }

    pub fn enter_with_hint(flags: u8, hint: u8) -> Self {
        let prev_flags = get_decode_flags();
        let prev_hint = DECODE_FLAGS_HINT.with(|c| c.get());
        let prev_active = decode_flags_active();
        #[cfg(debug_assertions)]
        if crate::debug_trace_enabled() {
            eprintln!("DecodeFlagsGuard::enter_with_hint flags=0x{flags:02x} hint=0x{hint:02x}");
        }
        DECODE_FLAGS_HINT.with(|c| {
            c.set(hint);
        });
        set_decode_flags_raw(flags);
        set_decode_flags_active(true);
        Self {
            prev_flags,
            prev_hint,
            prev_active,
        }
    }
}
impl Drop for DecodeFlagsGuard {
    fn drop(&mut self) {
        set_decode_flags_raw(self.prev_flags);
        set_decode_flags_active(self.prev_active);
        set_decode_flags_hint(self.prev_hint);
    }
}

/// Convenience: reset both decode flags and payload context.
///
/// Useful in tests or when switching between payloads with different negotiated
/// layouts in the same thread.
pub fn reset_decode_state() {
    set_decode_flags_raw(0);
    set_decode_flags_active(false);
    clear_payload_ctx();
    set_decode_flags_hint(0);
}

#[inline]
fn decode_flags_hint() -> u8 {
    DECODE_FLAGS_HINT.with(|c| c.get())
}

pub(crate) fn prepare_header_decode(flags: u8, hint: u8, require_match: bool) -> Result<(), Error> {
    if decode_flags_active() {
        let active_flags = get_decode_flags();
        let active_hint = decode_flags_hint();
        if require_match && active_flags != flags {
            return Err(Error::DecodeFlagsMismatch {
                header_flags: flags,
                header_hint: hint,
                active_flags,
                active_hint,
            });
        }
        clear_payload_ctx();
        return Ok(());
    }
    reset_decode_state();
    Ok(())
}

#[inline]
fn combine_flags(flags: u8, _hint: u8) -> u8 {
    flags
}

fn current_decode_flags_effective() -> Option<u8> {
    if decode_flags_active() {
        let flags = get_decode_flags();
        let hint = DECODE_FLAGS_HINT.with(|c| c.get());
        Some(combine_flags(flags, hint))
    } else if let Some((ctx_flags, ctx_hint)) = payload_ctx_flags() {
        Some(combine_flags(ctx_flags, ctx_hint))
    } else {
        None
    }
}

/// Return the currently effective decode flags, if any.
///
/// When a Norito header is being decoded this reflects the stored flag set.
/// Otherwise it falls back to any explicitly configured `DecodeFlagsGuard`.
pub fn effective_decode_flags() -> Option<u8> {
    current_decode_flags_effective()
}

#[inline]
fn sequential_override_active() -> bool {
    FORCE_SEQUENTIAL.with(|flag| flag.get())
}

#[inline]
fn effective_layout_flags() -> u8 {
    current_decode_flags_effective().unwrap_or_else(default_encode_flags)
}

#[inline]
fn layout_flag_enabled(flag: u8) -> bool {
    if sequential_override_active() {
        return (default_encode_flags() & flag) != 0;
    }
    (effective_layout_flags() & flag) != 0
}

#[inline]
fn layout_flag_enabled_for_flags(flags: u8, flag: u8) -> bool {
    if sequential_override_active() {
        return (default_encode_flags() & flag) != 0;
    }
    (sanitize_layout_flags(flags) & flag) != 0
}

/// True if packed sequence layouts are enabled for the current decode.
pub fn use_packed_seq() -> bool {
    layout_flag_enabled(header_flags::PACKED_SEQ)
}

/// True if packed sequence layouts are enabled in an explicit flag snapshot.
#[doc(hidden)]
pub fn packed_seq_enabled_for_flags(flags: u8) -> bool {
    layout_flag_enabled_for_flags(flags, header_flags::PACKED_SEQ)
}

/// True if compact varint length encoding is enabled for the current decode.
pub fn use_compact_len() -> bool {
    layout_flag_enabled(header_flags::COMPACT_LEN)
}

/// True if compact length encoding is enabled in an explicit flag snapshot.
#[doc(hidden)]
pub fn compact_len_enabled_for_flags(flags: u8) -> bool {
    layout_flag_enabled_for_flags(flags, header_flags::COMPACT_LEN)
}

/// True if packed struct layout is enabled for the current decode.
pub fn use_packed_struct() -> bool {
    layout_flag_enabled(header_flags::PACKED_STRUCT)
}

/// True if packed-struct encodes a bitset selecting fields with explicit sizes.
pub fn use_field_bitset() -> bool {
    layout_flag_enabled(header_flags::FIELD_BITSET)
}

/// Decode packed-struct offsets when the layout is enabled.
///
/// Returns the computed offsets, number of header bytes consumed, and packed
/// data length.
pub fn decode_packed_offsets_slice(
    slice: &[u8],
    count: usize,
) -> Result<(Vec<usize>, usize, usize, usize), Error> {
    if count == 0 {
        if slice.len() < 8 {
            return Err(Error::LengthMismatch);
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&slice[..8]);
        if u64::from_le_bytes(buf) != 0 {
            return Err(Error::LengthMismatch);
        }
        return Ok((vec![0], 8, 0, 0));
    }

    let entries = count.checked_add(1).ok_or(Error::LengthMismatch)?;
    let bytes_needed = entries.checked_mul(8).ok_or(Error::LengthMismatch)?;
    if slice.len() < bytes_needed {
        return Err(Error::LengthMismatch);
    }

    let mut offsets: Vec<usize> = Vec::with_capacity(entries);
    for idx in 0..entries {
        let start = idx * 8;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&slice[start..start + 8]);
        let raw = u64::from_le_bytes(buf);
        let off = usize::try_from(raw).map_err(|_| Error::LengthMismatch)?;
        if idx == 0 {
            if off != 0 {
                return Err(Error::LengthMismatch);
            }
        } else if off < *offsets.last().unwrap() {
            return Err(Error::LengthMismatch);
        }
        offsets.push(off);
    }
    let data_len = *offsets.last().unwrap_or(&0);
    Ok((offsets, bytes_needed, data_len, 0))
}

/// Byte range for a planned binary sequence element.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SequenceSpan {
    /// Inclusive start offset in the source byte slice.
    pub start: usize,
    /// Exclusive end offset in the source byte slice.
    pub end: usize,
}

impl SequenceSpan {
    /// Return this span as a slice of `bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LengthMismatch`] when the planned range is outside
    /// `bytes`.
    #[doc(hidden)]
    #[inline]
    pub fn get<'a>(&self, bytes: &'a [u8]) -> Result<&'a [u8], Error> {
        bytes.get(self.start..self.end).ok_or(Error::LengthMismatch)
    }

    /// Length of the planned element payload.
    #[doc(hidden)]
    #[inline]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the planned element payload is empty.
    #[doc(hidden)]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Planned element spans and total bytes consumed by a binary sequence.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequencePlan {
    /// Element payload ranges in original sequence order.
    pub spans: Vec<SequenceSpan>,
    /// Total bytes consumed from the source sequence payload.
    pub used: usize,
}

/// Binary sequence layout to plan.
#[doc(hidden)]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinarySequenceLayout {
    /// Sequence is encoded as `[count_u64][len][payload]...`.
    LengthPrefixed = 0,
    /// Sequence is encoded as `[count_u64][(count + 1) u64 offsets][payloads...]`.
    FixedOffsets = 1,
}

impl BinarySequenceLayout {
    /// Resolve the sequence layout advertised by Norito header flags.
    #[doc(hidden)]
    #[inline]
    pub fn from_flags(flags: u8) -> Self {
        if (flags & header_flags::PACKED_SEQ) != 0 {
            Self::FixedOffsets
        } else {
            Self::LengthPrefixed
        }
    }

    #[cfg(any(feature = "codec-gpu-metal", feature = "codec-gpu-cuda"))]
    #[inline]
    fn abi_kind(self) -> u32 {
        self as u32
    }
}

#[cfg(any(feature = "codec-gpu-metal", feature = "codec-gpu-cuda"))]
const SEQUENCE_GPU_MIN_BYTES: usize = 1 << 20;
#[cfg(any(feature = "codec-gpu-metal", feature = "codec-gpu-cuda"))]
const SEQUENCE_GPU_MIN_ELEMENTS: usize = 4096;

/// Plan element byte spans for a Norito binary sequence without materializing
/// values.
///
/// The input starts at the sequence count header. Returned spans are byte
/// offsets into the same input slice, and `used` is the total sequence payload
/// length consumed from the front of `bytes`.
#[doc(hidden)]
pub fn plan_binary_sequence(
    bytes: &[u8],
    flags: u8,
    layout: BinarySequenceLayout,
) -> Result<SequencePlan, Error> {
    validate_header_flags(flags)?;
    let (count, _) = read_seq_len_slice(bytes)?;

    #[cfg(any(feature = "codec-gpu-metal", feature = "codec-gpu-cuda"))]
    if bytes.len() >= SEQUENCE_GPU_MIN_BYTES
        && count >= SEQUENCE_GPU_MIN_ELEMENTS
        && let Some(plan) = sequence_gpu::try_plan_binary_sequence(bytes, flags, layout)
    {
        note_payload_access(bytes, plan.used);
        return Ok(plan);
    }

    let plan = plan_binary_sequence_scalar_with_count(bytes, flags, layout, count)?;
    note_payload_access(bytes, plan.used);
    Ok(plan)
}

/// Decode a pre-planned sequence in parallel at typed call sites that can prove
/// `T: Send`.
///
/// Values preserve the original span order. If any element fails, this returns
/// the first error in original sequence order after in-flight work finishes.
#[cfg(feature = "parallel-decode")]
#[doc(hidden)]
pub fn decode_planned_sequence_parallel<T>(
    bytes: &[u8],
    flags: u8,
    plan: &SequencePlan,
) -> Result<Vec<T>, Error>
where
    T: for<'de> crate::NoritoDeserialize<'de> + crate::NoritoSerialize + Send,
{
    use rayon::prelude::*;

    validate_header_flags(flags)?;
    let decoded: Vec<Result<T, Error>> = plan
        .spans
        .par_iter()
        .map(|span| {
            let element_slice = span.get(bytes)?;
            let _guard = DecodeFlagsGuard::enter_with_hint(flags, flags);
            let (value, used) = decode_field_canonical::<T>(element_slice)?;
            if used != span.len() {
                return Err(Error::LengthMismatch);
            }
            Ok(value)
        })
        .collect();

    let mut out = Vec::new();
    out.try_reserve(decoded.len())
        .map_err(|_| Error::LengthMismatch)?;
    for value in decoded {
        out.push(value?);
    }
    Ok(out)
}

#[cfg(any(feature = "codec-gpu-metal", feature = "codec-gpu-cuda"))]
fn plan_binary_sequence_scalar(
    bytes: &[u8],
    flags: u8,
    layout: BinarySequenceLayout,
) -> Result<SequencePlan, Error> {
    validate_header_flags(flags)?;
    let (count, _) = read_seq_len_slice(bytes)?;
    plan_binary_sequence_scalar_with_count(bytes, flags, layout, count)
}

fn plan_binary_sequence_scalar_with_count(
    bytes: &[u8],
    flags: u8,
    layout: BinarySequenceLayout,
    count: usize,
) -> Result<SequencePlan, Error> {
    let (_, mut offset) = read_seq_len_slice(bytes)?;
    match layout {
        BinarySequenceLayout::LengthPrefixed => {
            let mut spans = Vec::new();
            spans
                .try_reserve(count)
                .map_err(|_| Error::LengthMismatch)?;
            for _ in 0..count {
                let tail = bytes.get(offset..).ok_or(Error::LengthMismatch)?;
                let (elem_len, header_len) = read_len_from_slice_with_flags(tail, flags)?;
                let start = offset
                    .checked_add(header_len)
                    .ok_or(Error::LengthMismatch)?;
                let end = start.checked_add(elem_len).ok_or(Error::LengthMismatch)?;
                if end > bytes.len() {
                    return Err(Error::LengthMismatch);
                }
                spans.push(SequenceSpan { start, end });
                offset = end;
            }
            Ok(SequencePlan {
                spans,
                used: offset,
            })
        }
        BinarySequenceLayout::FixedOffsets => {
            let entries = count.checked_add(1).ok_or(Error::LengthMismatch)?;
            let offset_table_len = entries.checked_mul(8).ok_or(Error::LengthMismatch)?;
            let offsets_start = offset;
            let offsets_end = offsets_start
                .checked_add(offset_table_len)
                .ok_or(Error::LengthMismatch)?;
            let offsets = bytes
                .get(offsets_start..offsets_end)
                .ok_or(Error::LengthMismatch)?;

            let mut spans = Vec::new();
            spans
                .try_reserve(count)
                .map_err(|_| Error::LengthMismatch)?;

            let first = read_u64_le_at(offsets, 0)?;
            if first != 0 {
                return Err(Error::LengthMismatch);
            }
            let data_len = read_u64_le_at(offsets, count)?
                .try_into()
                .map_err(|_| Error::LengthMismatch)?;
            let data_start = offsets_end;
            let data_end = data_start
                .checked_add(data_len)
                .ok_or(Error::LengthMismatch)?;
            if data_end > bytes.len() {
                return Err(Error::LengthMismatch);
            }

            let mut prev = 0usize;
            for idx in 0..count {
                let next = read_u64_le_at(offsets, idx + 1)?
                    .try_into()
                    .map_err(|_| Error::LengthMismatch)?;
                if next < prev || next > data_len {
                    return Err(Error::LengthMismatch);
                }
                let start = data_start.checked_add(prev).ok_or(Error::LengthMismatch)?;
                let end = data_start.checked_add(next).ok_or(Error::LengthMismatch)?;
                spans.push(SequenceSpan { start, end });
                prev = next;
            }
            if prev != data_len {
                return Err(Error::LengthMismatch);
            }

            Ok(SequencePlan {
                spans,
                used: data_end,
            })
        }
    }
}

#[inline]
fn read_u64_le_at(bytes: &[u8], idx: usize) -> Result<u64, Error> {
    let start = idx.checked_mul(8).ok_or(Error::LengthMismatch)?;
    let end = start.checked_add(8).ok_or(Error::LengthMismatch)?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(bytes.get(start..end).ok_or(Error::LengthMismatch)?);
    Ok(u64::from_le_bytes(buf))
}

#[inline]
fn read_fixed_offset_usize_at(bytes: &[u8], idx: usize) -> Result<usize, Error> {
    len_u64_to_usize(read_u64_le_at(bytes, idx)?)
}

fn validate_fixed_offset_table(bytes: &[u8], entries: usize) -> Result<usize, Error> {
    let mut prev = 0usize;
    for idx in 0..entries {
        let off = read_fixed_offset_usize_at(bytes, idx)?;
        if idx == 0 {
            if off != 0 {
                return Err(Error::LengthMismatch);
            }
        } else if off < prev {
            return Err(Error::LengthMismatch);
        }
        prev = off;
    }
    Ok(prev)
}

#[cfg(any(feature = "codec-gpu-metal", feature = "codec-gpu-cuda"))]
mod sequence_gpu {
    use std::{
        ffi::{c_char, c_int, c_void},
        path::PathBuf,
        sync::{Mutex, OnceLock},
    };

    use super::{BinarySequenceLayout, SequencePlan, SequenceSpan, plan_binary_sequence_scalar};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct AbiSpan {
        start: usize,
        end: usize,
    }

    type SequencePlanHelperFn = unsafe extern "C" fn(
        input_ptr: *const u8,
        input_len: usize,
        flags: u8,
        layout_kind: u32,
        out_spans: *mut AbiSpan,
        out_capacity: usize,
        out_count: *mut usize,
        out_used: *mut usize,
    ) -> i32;

    const RC_OK: i32 = 0;
    const RC_INVALID: i32 = 1;
    const RC_NO_SPACE: i32 = 2;
    const RC_UNAVAILABLE: i32 = 3;

    struct SequencePlanLib {
        handle: *mut c_void,
        func: SequencePlanHelperFn,
    }

    unsafe impl Send for SequencePlanLib {}
    unsafe impl Sync for SequencePlanLib {}

    impl Drop for SequencePlanLib {
        fn drop(&mut self) {
            unsafe { close_library(self.handle) };
        }
    }

    enum SequencePlanCache {
        Unknown,
        Loaded(SequencePlanLib),
        Disabled,
    }

    static SEQUENCE_PLAN_LIB: OnceLock<Mutex<SequencePlanCache>> = OnceLock::new();

    pub(super) fn try_plan_binary_sequence(
        bytes: &[u8],
        flags: u8,
        layout: BinarySequenceLayout,
    ) -> Option<SequencePlan> {
        let cache = SEQUENCE_PLAN_LIB.get_or_init(|| Mutex::new(SequencePlanCache::Unknown));
        let mut guard = cache.lock().expect("sequence GPU cache poisoned");
        if matches!(*guard, SequencePlanCache::Unknown) {
            *guard = unsafe { load_sequence_plan_library() }
                .map(SequencePlanCache::Loaded)
                .unwrap_or(SequencePlanCache::Disabled);
        }
        let SequencePlanCache::Loaded(lib) = &*guard else {
            return None;
        };
        let plan = match unsafe { call_helper(lib.func, bytes, flags, layout) } {
            HelperOutcome::Planned(plan) => plan,
            HelperOutcome::InvalidInput => {
                if plan_binary_sequence_scalar(bytes, flags, layout).is_ok() {
                    *guard = SequencePlanCache::Disabled;
                }
                return None;
            }
            HelperOutcome::BackendFailure => {
                *guard = SequencePlanCache::Disabled;
                return None;
            }
            HelperOutcome::BackendUnavailable => return None,
        };
        match plan_binary_sequence_scalar(bytes, flags, layout) {
            Ok(scalar) if scalar == plan => Some(plan),
            Ok(_) | Err(_) => {
                *guard = SequencePlanCache::Disabled;
                None
            }
        }
    }

    enum HelperOutcome {
        Planned(SequencePlan),
        InvalidInput,
        BackendUnavailable,
        BackendFailure,
    }

    unsafe fn call_helper(
        func: SequencePlanHelperFn,
        bytes: &[u8],
        flags: u8,
        layout: BinarySequenceLayout,
    ) -> HelperOutcome {
        let mut spans: Vec<AbiSpan> = Vec::new();
        let mut out_count = 0usize;
        let mut out_used = 0usize;
        let rc = unsafe {
            func(
                bytes.as_ptr(),
                bytes.len(),
                flags,
                layout.abi_kind(),
                spans.as_mut_ptr(),
                spans.capacity(),
                &mut out_count,
                &mut out_used,
            )
        };
        if rc != RC_NO_SPACE {
            return if rc == RC_OK && out_count == 0 && out_used <= bytes.len() {
                HelperOutcome::Planned(SequencePlan {
                    spans: Vec::new(),
                    used: out_used,
                })
            } else if rc == RC_OK || rc == RC_INVALID {
                HelperOutcome::InvalidInput
            } else if rc == RC_UNAVAILABLE {
                HelperOutcome::BackendUnavailable
            } else {
                HelperOutcome::BackendFailure
            };
        }
        if spans.try_reserve(out_count).is_err() {
            return HelperOutcome::BackendFailure;
        }
        let rc = unsafe {
            func(
                bytes.as_ptr(),
                bytes.len(),
                flags,
                layout.abi_kind(),
                spans.as_mut_ptr(),
                spans.capacity(),
                &mut out_count,
                &mut out_used,
            )
        };
        if rc != RC_OK || out_count > spans.capacity() || out_used > bytes.len() {
            return match rc {
                RC_INVALID => HelperOutcome::InvalidInput,
                RC_UNAVAILABLE => HelperOutcome::BackendUnavailable,
                _ => HelperOutcome::BackendFailure,
            };
        }
        unsafe {
            spans.set_len(out_count);
        }
        let spans = spans
            .into_iter()
            .map(|span| {
                if span.start <= span.end && span.end <= bytes.len() {
                    Some(SequenceSpan {
                        start: span.start,
                        end: span.end,
                    })
                } else {
                    None
                }
            })
            .collect::<Option<Vec<_>>>();
        match spans {
            Some(spans) => HelperOutcome::Planned(SequencePlan {
                spans,
                used: out_used,
            }),
            None => HelperOutcome::BackendFailure,
        }
    }

    unsafe fn load_sequence_plan_library() -> Option<SequencePlanLib> {
        #[cfg(unix)]
        {
            for path in candidate_paths() {
                let Some(lib) = (unsafe { load_library_unix(&path) }) else {
                    continue;
                };
                let Some(func) = (unsafe { resolve_symbol(lib) }) else {
                    unsafe { close_library(lib) };
                    continue;
                };
                if sequence_plan_helper_self_test(func) {
                    return Some(SequencePlanLib { handle: lib, func });
                }
                unsafe { close_library(lib) };
            }
        }
        None
    }

    fn sequence_plan_helper_self_test(func: SequencePlanHelperFn) -> bool {
        let cases = [
            (
                make_unpacked_case(super::header_flags::COMPACT_LEN),
                super::header_flags::COMPACT_LEN,
                BinarySequenceLayout::LengthPrefixed,
            ),
            (
                make_packed_case(),
                super::header_flags::PACKED_SEQ,
                BinarySequenceLayout::FixedOffsets,
            ),
        ];

        for (bytes, flags, layout) in cases {
            let accel = match unsafe { call_helper(func, &bytes, flags, layout) } {
                HelperOutcome::Planned(plan) => plan,
                HelperOutcome::InvalidInput
                | HelperOutcome::BackendUnavailable
                | HelperOutcome::BackendFailure => return false,
            };
            let Ok(scalar) = plan_binary_sequence_scalar(&bytes, flags, layout) else {
                return false;
            };
            if accel != scalar {
                return false;
            }
        }
        true
    }

    fn make_unpacked_case(flags: u8) -> Vec<u8> {
        let _guard = super::DecodeFlagsGuard::enter_with_hint(flags, flags);
        let mut out = Vec::new();
        super::write_seq_len(&mut out, 3).expect("write sequence length");
        for payload in [&b"a"[..], &[0x55; 130][..], b"tail"] {
            super::write_len(&mut out, payload.len() as u64).expect("write element length");
            out.extend_from_slice(payload);
        }
        out
    }

    fn make_packed_case() -> Vec<u8> {
        let mut out = Vec::new();
        super::write_seq_len(&mut out, 3).expect("write sequence length");
        for offset in [0u64, 1, 3, 6] {
            out.extend_from_slice(&offset.to_le_bytes());
        }
        out.extend_from_slice(b"abcdef");
        out
    }

    fn candidate_paths() -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            #[cfg(all(feature = "codec-gpu-metal", target_os = "macos"))]
            {
                candidates.push(dir.join("libjsonstage1_metal.dylib"));
                candidates.push(dir.join("../lib/libjsonstage1_metal.dylib"));
            }
            #[cfg(feature = "codec-gpu-cuda")]
            {
                candidates.push(dir.join("libjsonstage1_cuda.dylib"));
                candidates.push(dir.join("../lib/libjsonstage1_cuda.dylib"));
                candidates.push(dir.join("libjsonstage1_cuda.so"));
                candidates.push(dir.join("../lib/libjsonstage1_cuda.so"));
            }
        }
        candidates
    }

    #[cfg(unix)]
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
    }

    #[cfg(unix)]
    unsafe fn load_library_unix(path: &std::path::Path) -> Option<*mut c_void> {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};

        const RTLD_LAZY: c_int = 1;
        let bytes = path.as_os_str().as_bytes();
        if bytes.contains(&0) {
            return None;
        }
        let cpath = CString::new(bytes).ok()?;
        let handle = unsafe { dlopen(cpath.as_ptr(), RTLD_LAZY) };
        if handle.is_null() { None } else { Some(handle) }
    }

    #[cfg(not(unix))]
    unsafe fn load_library_unix(_path: &std::path::Path) -> Option<*mut c_void> {
        None
    }

    unsafe fn resolve_symbol(handle: *mut c_void) -> Option<SequencePlanHelperFn> {
        #[cfg(unix)]
        {
            let sym = unsafe { dlsym(handle, c"norito_binary_sequence_plan".as_ptr()) };
            if sym.is_null() {
                return None;
            }
            Some(unsafe { std::mem::transmute(sym) })
        }
        #[cfg(not(unix))]
        {
            let _ = handle;
            None
        }
    }

    unsafe fn close_library(handle: *mut c_void) {
        #[cfg(unix)]
        if !handle.is_null() {
            let _ = unsafe { dlclose(handle) };
        }
        #[cfg(not(unix))]
        let _ = handle;
    }

    #[cfg(test)]
    mod tests {
        use super::{
            AbiSpan, BinarySequenceLayout, HelperOutcome, RC_NO_SPACE, RC_UNAVAILABLE, call_helper,
            sequence_plan_helper_self_test,
        };

        unsafe extern "C" fn mismatched_helper(
            _input_ptr: *const u8,
            input_len: usize,
            _flags: u8,
            _layout_kind: u32,
            out_spans: *mut AbiSpan,
            out_capacity: usize,
            out_count: *mut usize,
            out_used: *mut usize,
        ) -> i32 {
            unsafe {
                *out_count = 1;
                *out_used = input_len;
            }
            if out_capacity == 0 {
                return RC_NO_SPACE;
            }
            unsafe {
                *out_spans = AbiSpan { start: 0, end: 0 };
            }
            0
        }

        unsafe extern "C" fn backend_error_helper(
            _input_ptr: *const u8,
            _input_len: usize,
            _flags: u8,
            _layout_kind: u32,
            _out_spans: *mut AbiSpan,
            _out_capacity: usize,
            _out_count: *mut usize,
            _out_used: *mut usize,
        ) -> i32 {
            4
        }

        unsafe extern "C" fn unavailable_helper(
            _input_ptr: *const u8,
            _input_len: usize,
            _flags: u8,
            _layout_kind: u32,
            _out_spans: *mut AbiSpan,
            _out_capacity: usize,
            _out_count: *mut usize,
            _out_used: *mut usize,
        ) -> i32 {
            RC_UNAVAILABLE
        }

        #[test]
        fn sequence_plan_helper_self_test_rejects_mismatched_helper() {
            assert!(!sequence_plan_helper_self_test(mismatched_helper));
        }

        #[test]
        fn helper_backend_errors_are_distinguished_from_bad_input() {
            let bytes = super::make_unpacked_case(super::super::header_flags::COMPACT_LEN);
            let outcome = unsafe {
                call_helper(
                    backend_error_helper,
                    &bytes,
                    super::super::header_flags::COMPACT_LEN,
                    BinarySequenceLayout::LengthPrefixed,
                )
            };

            assert!(matches!(outcome, HelperOutcome::BackendFailure));
        }

        #[test]
        fn helper_unavailable_is_a_fallback_not_backend_failure() {
            let bytes = super::make_unpacked_case(super::super::header_flags::COMPACT_LEN);
            let outcome = unsafe {
                call_helper(
                    unavailable_helper,
                    &bytes,
                    super::super::header_flags::COMPACT_LEN,
                    BinarySequenceLayout::LengthPrefixed,
                )
            };

            assert!(matches!(outcome, HelperOutcome::BackendUnavailable));
        }
    }
}

/// Write a length prefix honoring `COMPACT_LEN`.
pub fn write_len<W: Write>(writer: &mut W, value: u64) -> std::io::Result<()> {
    write_len_with_flags(writer, value, effective_layout_flags())
}

/// Write a length prefix honoring `COMPACT_LEN` from an explicit flag snapshot.
#[doc(hidden)]
pub fn write_len_with_flags<W: Write>(
    writer: &mut W,
    value: u64,
    flags: u8,
) -> std::io::Result<()> {
    if compact_len_enabled_for_flags(flags) {
        let mut buf = [0u8; MAX_VARINT_BYTES];
        let used = encode_varint(value, &mut buf);
        writer.write_all(&buf[..used])?;
        note_compact_len_emitted();
        Ok(())
    } else {
        writer.write_all(&value.to_le_bytes())
    }
}

/// Write a sequence length prefix (fixed u64).
pub fn write_seq_len<W: Write>(writer: &mut W, value: u64) -> std::io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

/// Append a length prefix honoring `COMPACT_LEN` to `out`.
#[inline]
pub fn write_len_to_vec(out: &mut Vec<u8>, value: u64) {
    write_len_to_vec_with_flags(out, value, effective_layout_flags());
}

/// Append a length prefix honoring `COMPACT_LEN` from an explicit flag snapshot.
#[inline]
#[doc(hidden)]
pub fn write_len_to_vec_with_flags(out: &mut Vec<u8>, value: u64, flags: u8) {
    if compact_len_enabled_for_flags(flags) {
        let mut buf = [0u8; MAX_VARINT_BYTES];
        let used = encode_varint(value, &mut buf);
        out.extend_from_slice(&buf[..used]);
        note_compact_len_emitted();
    } else {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

/// Serialize a value into `buf`, then write its length prefix and bytes.
///
/// This keeps length prefixes consistent with the actual serialized payload,
/// even if `encoded_len_exact` is incorrect.
pub fn write_len_prefixed<W: Write, T: NoritoSerialize, const N: usize>(
    writer: &mut W,
    value: &T,
    buf: &mut SmallBuf<N>,
) -> Result<(), Error> {
    let flags = effective_layout_flags();
    buf.clear();
    value.serialize(&mut *buf)?;
    let len = u64::try_from(buf.len()).map_err(|_| Error::LengthMismatch)?;
    write_len_with_flags(writer, len, flags)?;
    writer.write_all(buf.as_slice())?;
    Ok(())
}

/// Write a compact varint length prefix regardless of layout flags.
pub fn write_varint_len<W: Write>(writer: &mut W, value: u64) -> std::io::Result<()> {
    let mut buf = [0u8; MAX_VARINT_BYTES];
    let used = encode_varint(value, &mut buf);
    writer.write_all(&buf[..used])
}

/// Append a compact varint length prefix regardless of layout flags.
pub fn write_varint_len_to_vec(out: &mut Vec<u8>, value: u64) {
    let mut buf = [0u8; MAX_VARINT_BYTES];
    let used = encode_varint(value, &mut buf);
    out.extend_from_slice(&buf[..used]);
}

fn write_fixed_offsets<W: Write>(writer: &mut W, lengths: &[usize]) -> Result<(), Error> {
    let mut offset = 0u64;
    writer.write_all(&0u64.to_le_bytes())?;
    for len in lengths {
        let len_u64 = u64::try_from(*len).map_err(|_| Error::LengthMismatch)?;
        offset = offset.checked_add(len_u64).ok_or(Error::LengthMismatch)?;
        writer.write_all(&offset.to_le_bytes())?;
    }
    Ok(())
}

fn encode_seq_payloads<W, I, F, H>(
    out: &mut W,
    len: usize,
    iter: I,
    mut encode_elem: F,
    mut hint_elem: H,
) -> Result<(), Error>
where
    W: Write,
    I: IntoIterator,
    F: FnMut(I::Item, &mut Vec<u8>) -> Result<(), Error>,
    H: FnMut(&I::Item) -> Option<usize>,
{
    let writer = out;
    write_seq_len(
        writer,
        u64::try_from(len).map_err(|_| Error::LengthMismatch)?,
    )?;
    let packed = use_packed_seq();
    if !packed {
        let mut buf = Vec::new();
        let flags = effective_layout_flags();
        for item in iter {
            buf.clear();
            if let Some(hint) = hint_elem(&item)
                && buf.capacity() < hint
            {
                buf.try_reserve(hint - buf.capacity())
                    .map_err(|_| Error::LengthMismatch)?;
            }
            encode_elem(item, &mut buf)?;
            write_len_with_flags(
                writer,
                u64::try_from(buf.len()).map_err(|_| Error::LengthMismatch)?,
                flags,
            )?;
            writer.write_all(&buf)?;
        }
        return Ok(());
    }

    note_fixed_offsets_emitted();
    if len == 0 {
        write_fixed_offsets(writer, &[])?;
        return Ok(());
    }

    let table_len = len
        .checked_add(1)
        .and_then(|entries| entries.checked_mul(core::mem::size_of::<u64>()))
        .ok_or(Error::LengthMismatch)?;
    let mut packed = vec![0; table_len];
    let mut total = 0u64;
    let mut count = 0usize;
    for (idx, item) in iter.into_iter().enumerate() {
        if idx >= len {
            return Err(Error::LengthMismatch);
        }
        if let Some(hint) = hint_elem(&item) {
            let needed = packed
                .len()
                .checked_add(hint)
                .ok_or(Error::LengthMismatch)?;
            if packed.capacity() < needed {
                packed
                    .try_reserve(needed - packed.capacity())
                    .map_err(|_| Error::LengthMismatch)?;
            }
        }
        let elem_start = packed.len();
        encode_elem(item, &mut packed)?;
        let elem_len = packed
            .len()
            .checked_sub(elem_start)
            .ok_or(Error::LengthMismatch)?;
        total = total
            .checked_add(u64::try_from(elem_len).map_err(|_| Error::LengthMismatch)?)
            .ok_or(Error::LengthMismatch)?;
        let offset_pos = idx
            .checked_add(1)
            .and_then(|offset| offset.checked_mul(core::mem::size_of::<u64>()))
            .ok_or(Error::LengthMismatch)?;
        packed[offset_pos..offset_pos + core::mem::size_of::<u64>()]
            .copy_from_slice(&total.to_le_bytes());
        count += 1;
    }
    if count != len {
        return Err(Error::LengthMismatch);
    }
    writer.write_all(&packed)?;
    Ok(())
}

fn encode_slice_payloads<W, T>(writer: &mut W, slice: &[T]) -> Result<(), Error>
where
    W: Write,
    T: NoritoSerialize,
{
    write_seq_len(
        writer,
        u64::try_from(slice.len()).map_err(|_| Error::LengthMismatch)?,
    )?;

    if !use_packed_seq() {
        let mut buf = Vec::new();
        if let Some(max_hint) = slice
            .iter()
            .filter_map(|item| item.encoded_len_exact().or_else(|| item.encoded_len_hint()))
            .max()
        {
            buf.try_reserve(max_hint)
                .map_err(|_| Error::LengthMismatch)?;
        }
        let flags = effective_layout_flags();
        for item in slice {
            buf.clear();
            item.serialize(&mut buf)?;
            write_len_with_flags(
                writer,
                u64::try_from(buf.len()).map_err(|_| Error::LengthMismatch)?,
                flags,
            )?;
            writer.write_all(&buf)?;
        }
        return Ok(());
    }

    note_fixed_offsets_emitted();
    let table_len = slice
        .len()
        .checked_add(1)
        .and_then(|entries| entries.checked_mul(core::mem::size_of::<u64>()))
        .ok_or(Error::LengthMismatch)?;
    let mut packed = vec![0; table_len];

    let mut data_reserve = 0usize;
    for item in slice {
        if let Some(hint) = item.encoded_len_exact().or_else(|| item.encoded_len_hint()) {
            data_reserve = data_reserve
                .checked_add(hint)
                .ok_or(Error::LengthMismatch)?;
        }
    }
    if data_reserve > 0 {
        packed
            .try_reserve(data_reserve)
            .map_err(|_| Error::LengthMismatch)?;
    }

    let mut total = 0u64;
    for (idx, item) in slice.iter().enumerate() {
        let elem_start = packed.len();
        item.serialize(&mut packed)?;
        let elem_len = packed
            .len()
            .checked_sub(elem_start)
            .ok_or(Error::LengthMismatch)?;
        total = total
            .checked_add(u64::try_from(elem_len).map_err(|_| Error::LengthMismatch)?)
            .ok_or(Error::LengthMismatch)?;
        let offset_pos = idx
            .checked_add(1)
            .and_then(|offset| offset.checked_mul(core::mem::size_of::<u64>()))
            .ok_or(Error::LengthMismatch)?;
        packed[offset_pos..offset_pos + core::mem::size_of::<u64>()]
            .copy_from_slice(&total.to_le_bytes());
    }
    writer.write_all(&packed)?;
    Ok(())
}

fn sequence_encoded_len_hint<'a, T, I>(len: usize, items: I) -> Option<usize>
where
    T: NoritoSerialize + 'a,
    I: IntoIterator<Item = &'a T>,
{
    let mut total = 8usize;
    if use_packed_seq() {
        let entries = len.checked_add(1)?;
        total = total.checked_add(entries.checked_mul(8)?)?;
        for item in items {
            let elem_len = item
                .encoded_len_exact()
                .or_else(|| item.encoded_len_hint())?;
            total = total.checked_add(elem_len)?;
        }
        return Some(total);
    }

    let flags = effective_layout_flags();
    for item in items {
        let elem_len = item
            .encoded_len_exact()
            .or_else(|| item.encoded_len_hint())?;
        total = total.checked_add(len_prefix_len_with_flags(elem_len, flags))?;
        total = total.checked_add(elem_len)?;
    }
    Some(total)
}

fn sequence_encoded_len_exact<'a, T, I>(len: usize, items: I) -> Option<usize>
where
    T: NoritoSerialize + 'a,
    I: IntoIterator<Item = &'a T>,
{
    let mut total = 8usize;
    if use_packed_seq() {
        let entries = len.checked_add(1)?;
        total = total.checked_add(entries.checked_mul(8)?)?;
        for item in items {
            total = total.checked_add(item.encoded_len_exact()?)?;
        }
        return Some(total);
    }

    let flags = effective_layout_flags();
    for item in items {
        let elem_len = item.encoded_len_exact()?;
        total = total.checked_add(len_prefix_len_with_flags(elem_len, flags))?;
        total = total.checked_add(elem_len)?;
    }
    Some(total)
}

fn map_encoded_len_hint<'a, K, V, I>(len: usize, entries: I) -> Option<usize>
where
    K: NoritoSerialize + 'a,
    V: NoritoSerialize + 'a,
    I: IntoIterator<Item = (&'a K, &'a V)>,
{
    let mut total = 8usize;
    if use_packed_seq() {
        let table_len = len
            .checked_add(1)?
            .checked_mul(core::mem::size_of::<u64>())?;
        total = total.checked_add(table_len.checked_mul(2)?)?;
        for (key, value) in entries {
            let key_len = key.encoded_len_exact().or_else(|| key.encoded_len_hint())?;
            let value_len = value
                .encoded_len_exact()
                .or_else(|| value.encoded_len_hint())?;
            total = total.checked_add(key_len)?;
            total = total.checked_add(value_len)?;
        }
        return Some(total);
    }

    let flags = effective_layout_flags();
    for (key, value) in entries {
        let key_len = key.encoded_len_exact().or_else(|| key.encoded_len_hint())?;
        let value_len = value
            .encoded_len_exact()
            .or_else(|| value.encoded_len_hint())?;
        total = total.checked_add(len_prefix_len_with_flags(key_len, flags))?;
        total = total.checked_add(key_len)?;
        total = total.checked_add(len_prefix_len_with_flags(value_len, flags))?;
        total = total.checked_add(value_len)?;
    }
    Some(total)
}

fn map_encoded_len_exact<'a, K, V, I>(len: usize, entries: I) -> Option<usize>
where
    K: NoritoSerialize + 'a,
    V: NoritoSerialize + 'a,
    I: IntoIterator<Item = (&'a K, &'a V)>,
{
    let mut total = 8usize;
    if use_packed_seq() {
        let table_len = len
            .checked_add(1)?
            .checked_mul(core::mem::size_of::<u64>())?;
        total = total.checked_add(table_len.checked_mul(2)?)?;
        for (key, value) in entries {
            total = total.checked_add(key.encoded_len_exact()?)?;
            total = total.checked_add(value.encoded_len_exact()?)?;
        }
        return Some(total);
    }

    let flags = effective_layout_flags();
    for (key, value) in entries {
        let key_len = key.encoded_len_exact()?;
        let value_len = value.encoded_len_exact()?;
        total = total.checked_add(len_prefix_len_with_flags(key_len, flags))?;
        total = total.checked_add(key_len)?;
        total = total.checked_add(len_prefix_len_with_flags(value_len, flags))?;
        total = total.checked_add(value_len)?;
    }
    Some(total)
}

fn map_max_field_len_hint<'a, K, V, I>(entries: I) -> Option<usize>
where
    K: NoritoSerialize + 'a,
    V: NoritoSerialize + 'a,
    I: IntoIterator<Item = (&'a K, &'a V)>,
{
    let mut max_len: Option<usize> = None;
    for (key, value) in entries {
        for field_len in [
            key.encoded_len_exact().or_else(|| key.encoded_len_hint()),
            value
                .encoded_len_exact()
                .or_else(|| value.encoded_len_hint()),
        ]
        .into_iter()
        .flatten()
        {
            max_len = Some(match max_len {
                Some(max_len) => max_len.max(field_len),
                None => field_len,
            });
        }
    }
    max_len
}

#[cfg(test)]
mod encode_seq_payloads_tests {
    use super::{DecodeFlagsGuard, encode_seq_payloads, encode_slice_payloads, header_flags};

    #[test]
    fn encode_seq_payloads_packed_uses_hints_and_keeps_layout() {
        let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
        let items: Vec<Vec<u8>> = vec![vec![1, 2], vec![3], vec![4, 5, 6]];
        let mut out = Vec::new();
        encode_seq_payloads(
            &mut out,
            items.len(),
            items.iter(),
            |item, buf| {
                buf.extend_from_slice(item);
                Ok(())
            },
            |item| Some(item.len().saturating_add(4)),
        )
        .expect("encode packed seq");

        let mut cursor = 0usize;
        let len_bytes: [u8; 8] = out[cursor..cursor + 8].try_into().expect("len bytes");
        let len = u64::from_le_bytes(len_bytes) as usize;
        cursor += 8;
        assert_eq!(len, items.len());

        let mut offsets = Vec::with_capacity(len + 1);
        for _ in 0..=len {
            let off_bytes: [u8; 8] = out[cursor..cursor + 8].try_into().expect("off bytes");
            offsets.push(u64::from_le_bytes(off_bytes) as usize);
            cursor += 8;
        }
        assert_eq!(offsets.first().copied(), Some(0));
        let expected_total: usize = items.iter().map(|item| item.len()).sum();
        assert_eq!(offsets.last().copied(), Some(expected_total));

        let data = &out[cursor..];
        let expected: Vec<u8> = items.iter().flatten().copied().collect();
        assert_eq!(data, expected.as_slice());
    }

    #[test]
    fn encode_slice_payloads_matches_packed_layout() {
        let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
        let items = [0x0102_u16, 0x0304_u16, 0x0506_u16];
        let mut out = Vec::new();

        encode_slice_payloads(&mut out, &items).expect("encode packed slice");

        assert_eq!(&out[..8], &(3u64).to_le_bytes());
        assert_eq!(&out[8..16], &(0u64).to_le_bytes());
        assert_eq!(&out[16..24], &(2u64).to_le_bytes());
        assert_eq!(&out[24..32], &(4u64).to_le_bytes());
        assert_eq!(&out[32..40], &(6u64).to_le_bytes());
        assert_eq!(&out[40..], &[0x02, 0x01, 0x04, 0x03, 0x06, 0x05]);
    }
}

/// Emit a length prefix honoring the `COMPACT_LEN` layout flag.
///
/// When `COMPACT_LEN` is set this writes a compact varint. Otherwise it falls
/// back to a fixed 8-byte little-endian `u64` header.
pub fn write_len_header<W: Write>(writer: &mut W, value: u64) -> std::io::Result<()> {
    write_len(writer, value)
}

/// Append a length prefix honoring the `COMPACT_LEN` layout flag.
///
/// Writes a compact varint when `COMPACT_LEN` is set, or an 8-byte
/// little-endian `u64` otherwise.
pub fn write_len_header_to_vec(out: &mut Vec<u8>, value: u64) {
    write_len_to_vec(out, value);
}

/// Read a length prefix from a slice honoring `COMPACT_LEN`.
/// Returns (value, bytes_consumed).
pub fn read_len_from_slice(bytes: &[u8]) -> Result<(usize, usize), Error> {
    read_len_from_slice_with_flags(bytes, effective_layout_flags())
}

/// Read a length prefix from a slice using an explicit Norito flag snapshot.
/// Returns (value, bytes consumed).
#[doc(hidden)]
pub fn read_len_from_slice_with_flags(bytes: &[u8], flags: u8) -> Result<(usize, usize), Error> {
    if compact_len_enabled_for_flags(flags) {
        let (value, used) = decode_varint_from_slice(bytes)?;
        record_slice_access(bytes, used);
        let len = usize::try_from(value).map_err(|_| Error::LengthMismatch)?;
        Ok((len, used))
    } else {
        if bytes.len() < 8 {
            return Err(Error::LengthMismatch);
        }
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&bytes[..8]);
        record_slice_access(bytes, 8);
        let value = u64::from_le_bytes(len_bytes);
        let len = usize::try_from(value).map_err(|_| Error::LengthMismatch)?;
        Ok((len, 8))
    }
}

/// Dynamically read a length prefix from a slice, honoring `COMPACT_LEN`.
/// Returns (value, bytes consumed).
pub fn read_len_dyn_slice(bytes: &[u8]) -> Result<(usize, usize), Error> {
    read_len_from_slice(bytes)
}

/// Read a top-level sequence length header from a slice (fixed u64).
/// Returns (value, bytes consumed).
pub fn read_seq_len_slice(bytes: &[u8]) -> Result<(usize, usize), Error> {
    if bytes.len() < 8 {
        return Err(Error::LengthMismatch);
    }
    let mut len_bytes = [0u8; 8];
    len_bytes.copy_from_slice(&bytes[..8]);
    record_slice_access(bytes, 8);
    let len = len_u64_to_usize(u64::from_le_bytes(len_bytes))?;
    Ok((len, 8))
}

/// Pointer-based length read without relying on a global payload context.
///
/// # Safety
/// `ptr` must be valid for reading up to the maximum header size
/// (8 bytes for fixed `u64` length or up to 10 bytes for compact varint).
/// Callers must ensure the pointer is within the validated payload.
#[inline]
pub unsafe fn read_len_ptr_unchecked(ptr: *const u8) -> (usize, usize) {
    match unsafe { try_read_len_ptr_unchecked(ptr) } {
        Ok(res) => res,
        Err(err) => {
            panic!("read_len_ptr_unchecked: invalid length header ({err:?})");
        }
    }
}

/// Fallible variant of [`read_len_ptr_unchecked`] that returns an error if a
/// compact-varint length exceeds 10 bytes (malformed input).
///
/// # Safety
/// Same as [`read_len_ptr_unchecked`].
#[inline]
pub unsafe fn try_read_len_ptr_unchecked(ptr: *const u8) -> Result<(usize, usize), Error> {
    if use_compact_len() {
        let (value, used) = unsafe { decode_varint_from_ptr(ptr)? };
        record_payload_access(ptr, used);
        let len = usize::try_from(value).map_err(|_| Error::LengthMismatch)?;
        Ok((len, used))
    } else {
        let mut lb = [0u8; 8];
        let header = unsafe { core::slice::from_raw_parts(ptr, 8) };
        lb.copy_from_slice(header);
        record_payload_access(ptr, 8);
        let len = len_u64_to_usize(u64::from_le_bytes(lb))?;
        Ok((len, 8))
    }
}

#[inline]
fn encode_varint(mut value: u64, out: &mut [u8; MAX_VARINT_BYTES]) -> usize {
    let mut i = 0;
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out[i] = byte;
            i += 1;
            break;
        } else {
            out[i] = byte | 0x80;
            i += 1;
        }
    }
    i
}

#[inline]
fn varint_encoded_len(value: u64) -> usize {
    let mut buf = [0u8; MAX_VARINT_BYTES];
    encode_varint(value, &mut buf)
}

pub(crate) fn varint_encoded_len_u64(value: u64) -> usize {
    varint_encoded_len(value)
}

fn decode_varint_from_slice(bytes: &[u8]) -> Result<(u64, usize), Error> {
    if bytes.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let mut result = 0u64;
    let mut shift = 0u32;
    for (idx, byte) in bytes.iter().copied().enumerate().take(MAX_VARINT_BYTES) {
        let payload = (byte & 0x7f) as u64;
        if shift == 63 && payload > 1 {
            return Err(Error::LengthMismatch);
        }
        result |= payload << shift;
        if byte & 0x80 == 0 {
            let used = idx + 1;
            if used != varint_encoded_len(result) {
                return Err(Error::LengthMismatch);
            }
            return Ok((result, used));
        }
        shift += 7;
        if shift >= 64 {
            break;
        }
    }
    Err(Error::LengthMismatch)
}

#[inline]
unsafe fn decode_varint_from_ptr(ptr: *const u8) -> Result<(u64, usize), Error> {
    let (payload, offset) = payload_bytes_with_offset(ptr)?;
    let slice = &payload[offset..];
    decode_varint_from_slice(slice)
}

/// Read a top-level sequence length header at a raw pointer (fixed u64).
/// Returns (value, bytes consumed).
///
/// # Safety
/// `ptr` must be valid for reading up to 10 bytes (varint) or 8 bytes (u64).
#[inline]
pub unsafe fn read_seq_len_ptr(ptr: *const u8) -> Result<(usize, usize), Error> {
    let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
    let ptr_us = ptr as usize;
    if ptr_us < base || ptr_us >= base + total {
        return Err(Error::LengthMismatch);
    }
    let offset = ptr_us - base;
    let payload = unsafe { core::slice::from_raw_parts(base as *const u8, total) };
    let head = &payload[offset..];
    let (len, used) = read_seq_len_slice(head)?;
    record_payload_access(ptr, used);
    Ok((len, used))
}

/// Pointer-based helper for reading a length header within the current payload context.
///
/// # Safety
/// `ptr` must point into the currently active payload (see [`set_payload_ctx`]).
#[inline]
pub unsafe fn read_len_dyn(ptr: *const u8) -> Result<(usize, usize), Error> {
    read_len_dyn_at_ptr(ptr)
}

/// Safe decode-from-slice trait used by the strict-safe path.
pub trait DecodeFromSlice<'a>: Sized {
    /// Decode from the front of `bytes`, returning the value and bytes consumed.
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error>;
}

impl<'a> DecodeFromSlice<'a> for () {
    fn decode_from_slice(_bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        Ok(((), 0))
    }
}

// DecodeFromSlice for primitives and common types
impl<'a> DecodeFromSlice<'a> for u8 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let b = *bytes.first().ok_or(Error::LengthMismatch)?;
        Ok((b, 1))
    }
}

impl<'a> DecodeFromSlice<'a> for i8 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let b = *bytes.first().ok_or(Error::LengthMismatch)? as i8;
        Ok((b, 1))
    }
}

impl<'a> DecodeFromSlice<'a> for bool {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let b = *bytes.first().ok_or(Error::LengthMismatch)?;
        Ok((b != 0, 1))
    }
}

macro_rules! impl_decode_from_slice_le_int {
    ($t:ty, $n:expr) => {
        impl<'a> DecodeFromSlice<'a> for $t {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
                if bytes.len() < $n {
                    return Err(Error::LengthMismatch);
                }
                let mut b = [0u8; $n];
                b.copy_from_slice(&bytes[..$n]);
                Ok((<$t>::from_le_bytes(b), $n))
            }
        }
    };
}

impl_decode_from_slice_le_int!(u16, 2);
impl_decode_from_slice_le_int!(i16, 2);
impl_decode_from_slice_le_int!(u32, 4);
impl_decode_from_slice_le_int!(i32, 4);
impl_decode_from_slice_le_int!(u64, 8);
impl_decode_from_slice_le_int!(i64, 8);
impl_decode_from_slice_le_int!(u128, 16);
impl_decode_from_slice_le_int!(i128, 16);

impl<'a> DecodeFromSlice<'a> for usize {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        if bytes.len() < 8 {
            return Err(Error::LengthMismatch);
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[..8]);
        let value = len_u64_to_usize(u64::from_le_bytes(b))?;
        Ok((value, 8))
    }
}

impl<'a> DecodeFromSlice<'a> for isize {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        if bytes.len() < 8 {
            return Err(Error::LengthMismatch);
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[..8]);
        let value = i64::from_le_bytes(b);
        let value = isize::try_from(value).map_err(|_| Error::LengthMismatch)?;
        Ok((value, 8))
    }
}

impl<'a> DecodeFromSlice<'a> for f32 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        if bytes.len() < 4 {
            return Err(Error::LengthMismatch);
        }
        let mut b = [0u8; 4];
        b.copy_from_slice(&bytes[..4]);
        Ok((f32::from_bits(u32::from_le_bytes(b)), 4))
    }
}

impl<'a> DecodeFromSlice<'a> for f64 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        if bytes.len() < 8 {
            return Err(Error::LengthMismatch);
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[..8]);
        Ok((f64::from_bits(u64::from_le_bytes(b)), 8))
    }
}

// DecodeFromSlice for NonZero integer wrappers via their primitive
impl<'a> DecodeFromSlice<'a> for NonZeroU16 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (v, used) = <u16 as DecodeFromSlice>::decode_from_slice(bytes)?;
        let nz = NonZeroU16::new(v).ok_or(Error::InvalidNonZero)?;
        Ok((nz, used))
    }
}

impl<'a> DecodeFromSlice<'a> for NonZeroU32 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (v, used) = <u32 as DecodeFromSlice>::decode_from_slice(bytes)?;
        let nz = NonZeroU32::new(v).ok_or(Error::InvalidNonZero)?;
        Ok((nz, used))
    }
}

impl<'a> DecodeFromSlice<'a> for NonZeroU64 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (v, used) = <u64 as DecodeFromSlice>::decode_from_slice(bytes)?;
        let nz = NonZeroU64::new(v).ok_or(Error::InvalidNonZero)?;
        Ok((nz, used))
    }
}

impl<'a> DecodeFromSlice<'a> for char {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        if bytes.len() < 4 {
            return Err(Error::LengthMismatch);
        }
        let mut b = [0u8; 4];
        b.copy_from_slice(&bytes[..4]);
        let v = u32::from_le_bytes(b);
        let c = char::from_u32(v).ok_or_else(|| Error::Message("invalid char".into()))?;
        Ok((c, 4))
    }
}

impl<'a, T> DecodeFromSlice<'a> for Vec<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (len, offset) = read_seq_len_slice(bytes)?;
        // `Vec<u8>` is encoded as `len(u64)` + raw bytes for efficiency.
        if core::any::type_name::<T>() == "u8"
            && let Some(end) = offset.checked_add(len)
            && end == bytes.len()
        {
            // SAFETY: we verified `T == u8` via `type_name`.
            let out = unsafe {
                let slice = bytes.get(offset..end).ok_or(Error::LengthMismatch)?;
                let vec = slice.to_vec();
                std::mem::transmute::<Vec<u8>, Vec<T>>(vec)
            };
            return Ok((out, end));
        }
        if core::any::type_name::<T>() == "u8" {
            return Err(Error::LengthMismatch);
        }
        if crate::debug_trace_enabled() {
            eprintln!(
                "Vec::<{}>::decode len={} packed_seq={}",
                core::any::type_name::<T>(),
                len,
                use_packed_seq()
            );
        }
        let flags = effective_decode_flags().unwrap_or_else(default_encode_flags);
        let layout = if use_packed_seq() {
            BinarySequenceLayout::FixedOffsets
        } else {
            BinarySequenceLayout::LengthPrefixed
        };
        let plan = plan_binary_sequence(bytes, flags, layout)?;
        if layout == BinarySequenceLayout::LengthPrefixed
            && core::mem::size_of::<T>() != 0
            && plan.spans.iter().any(SequenceSpan::is_empty)
        {
            return decode_length_prefixed_sequence_legacy::<T>(bytes, len, offset);
        }

        let mut out = Vec::new();
        out.try_reserve(plan.spans.len())
            .map_err(|_| Error::LengthMismatch)?;
        for span in &plan.spans {
            let element_slice = span.get(bytes)?;
            record_slice_access(element_slice, span.len());
            let (value, used) = decode_field_canonical::<T>(element_slice)?;
            if used != span.len() {
                return Err(Error::LengthMismatch);
            }
            out.push(value);
        }
        Ok((out, plan.used))
    }
}

fn decode_length_prefixed_sequence_legacy<'a, T>(
    bytes: &'a [u8],
    len: usize,
    mut offset: usize,
) -> Result<(Vec<T>, usize), Error>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    let mut out = Vec::new();
    out.try_reserve(len).map_err(|_| Error::LengthMismatch)?;
    for _ in 0..len {
        let (elem_len, header_len) = read_len_dyn_slice(&bytes[offset..])?;
        offset = offset
            .checked_add(header_len)
            .ok_or(Error::LengthMismatch)?;
        let data_start = offset;
        let expected_end = data_start
            .checked_add(elem_len)
            .ok_or(Error::LengthMismatch)?;
        if crate::debug_trace_enabled() {
            eprintln!(
                "Vec::<{}> element header_len={} elem_len={} offset_start={}",
                core::any::type_name::<T>(),
                header_len,
                elem_len,
                data_start
            );
        }
        if expected_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let decode_input = if elem_len == 0 {
            &bytes[data_start..]
        } else {
            &bytes[data_start..expected_end]
        };
        let (value, used) = decode_field_canonical::<T>(decode_input)?;
        if used > bytes.len().saturating_sub(data_start) {
            return Err(Error::LengthMismatch);
        }
        if elem_len != 0 && used != elem_len {
            return Err(Error::LengthMismatch);
        }
        if elem_len == 0 && used == 0 && core::mem::size_of::<T>() != 0 {
            return Err(Error::LengthMismatch);
        }
        let consumed = if elem_len == 0 { used } else { elem_len };
        let data_end = data_start
            .checked_add(consumed)
            .ok_or(Error::LengthMismatch)?;
        record_slice_access(&bytes[data_start..data_end], consumed);
        out.push(value);
        offset = data_end;
    }
    Ok((out, offset))
}

fn decode_length_prefixed_sequence_legacy_with<'a, T, F>(
    bytes: &'a [u8],
    len: usize,
    mut offset: usize,
    mut on_value: F,
) -> Result<usize, Error>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
    F: FnMut(T) -> Result<(), Error>,
{
    for _ in 0..len {
        let (elem_len, header_len) = read_len_dyn_slice(&bytes[offset..])?;
        offset = offset
            .checked_add(header_len)
            .ok_or(Error::LengthMismatch)?;
        let data_start = offset;
        let expected_end = data_start
            .checked_add(elem_len)
            .ok_or(Error::LengthMismatch)?;
        if expected_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let decode_input = if elem_len == 0 {
            &bytes[data_start..]
        } else {
            &bytes[data_start..expected_end]
        };
        let (value, used) = decode_field_canonical::<T>(decode_input)?;
        if used > bytes.len().saturating_sub(data_start) {
            return Err(Error::LengthMismatch);
        }
        if elem_len != 0 && used != elem_len {
            return Err(Error::LengthMismatch);
        }
        if elem_len == 0 && used == 0 && core::mem::size_of::<T>() != 0 {
            return Err(Error::LengthMismatch);
        }
        let consumed = if elem_len == 0 { used } else { elem_len };
        let data_end = data_start
            .checked_add(consumed)
            .ok_or(Error::LengthMismatch)?;
        record_slice_access(&bytes[data_start..data_end], consumed);
        on_value(value)?;
        offset = data_end;
    }
    Ok(offset)
}

fn decode_sequence_elements<'a, T, F>(bytes: &'a [u8], mut on_value: F) -> Result<usize, Error>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
    F: FnMut(T) -> Result<(), Error>,
{
    let (len, offset) = read_seq_len_slice(bytes)?;
    let flags = effective_decode_flags().unwrap_or_else(default_encode_flags);
    let layout = if use_packed_seq() {
        BinarySequenceLayout::FixedOffsets
    } else {
        BinarySequenceLayout::LengthPrefixed
    };
    let plan = plan_binary_sequence(bytes, flags, layout)?;
    if layout == BinarySequenceLayout::LengthPrefixed
        && core::mem::size_of::<T>() != 0
        && plan.spans.iter().any(SequenceSpan::is_empty)
    {
        return decode_length_prefixed_sequence_legacy_with::<T, _>(bytes, len, offset, on_value);
    }

    for span in &plan.spans {
        let element_slice = span.get(bytes)?;
        record_slice_access(element_slice, span.len());
        let (value, used) = decode_field_canonical::<T>(element_slice)?;
        if used != span.len() {
            return Err(Error::LengthMismatch);
        }
        on_value(value)?;
    }
    Ok(plan.used)
}

impl<'a> DecodeFromSlice<'a> for &'a [u8] {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (len, hdr) = read_len_dyn_slice(bytes)?;
        let end = hdr.checked_add(len).ok_or(Error::LengthMismatch)?;
        let data = bytes.get(hdr..end).ok_or(Error::LengthMismatch)?;
        Ok((data, end))
    }
}

impl<'a, T: DecodeFromSlice<'a> + 'static, const N: usize> DecodeFromSlice<'a> for [T; N] {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        if TypeId::of::<T>() == TypeId::of::<u8>() {
            record_slice_access(bytes, bytes.len());
            if bytes.len() == N {
                let mut buf = [0u8; N];
                buf.copy_from_slice(bytes);
                let mut arr = core::mem::MaybeUninit::<[T; N]>::uninit();
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        buf.as_ptr() as *const T,
                        arr.as_mut_ptr() as *mut T,
                        N,
                    );
                    return Ok((arr.assume_init(), N));
                }
            }

            let mut tmp = [0u8; N];
            let mut offset = 0usize;
            for slot in &mut tmp {
                let remaining = bytes.get(offset..).ok_or(Error::LengthMismatch)?;
                let (elem_len, elem_hdr) = read_len_dyn_slice(remaining)?;
                offset = offset.checked_add(elem_hdr).ok_or(Error::LengthMismatch)?;
                if elem_len != 1 {
                    return Err(Error::LengthMismatch);
                }
                let byte_pos = offset;
                offset = offset.checked_add(elem_len).ok_or(Error::LengthMismatch)?;
                let byte = *bytes.get(byte_pos).ok_or(Error::LengthMismatch)?;
                *slot = byte;
            }
            if offset != bytes.len() {
                return Err(Error::LengthMismatch);
            }
            let mut arr = core::mem::MaybeUninit::<[T; N]>::uninit();
            unsafe {
                core::ptr::copy_nonoverlapping(
                    tmp.as_ptr() as *const T,
                    arr.as_mut_ptr() as *mut T,
                    N,
                );
                return Ok((arr.assume_init(), bytes.len()));
            }
        }

        let mut offset = 0usize;
        let mut out: Vec<T> = Vec::with_capacity(N);
        for _ in 0..N {
            let (len, hdr) = read_len_dyn_slice(&bytes[offset..])?;
            offset += hdr;
            if offset + len > bytes.len() {
                return Err(Error::LengthMismatch);
            }
            let start = offset.checked_sub(hdr).ok_or(Error::LengthMismatch)?;
            record_slice_access(&bytes[start..], hdr + len);
            let slice = &bytes[offset..offset + len];
            let (val, used) = T::decode_from_slice(slice)?;
            debug_assert_eq!(used, len);
            out.push(val);
            offset += len;
        }
        Ok((out.try_into().unwrap_or_else(|_| unreachable!()), offset))
    }
}

impl<'a, T> DecodeFromSlice<'a> for VecDeque<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (len, _) = read_seq_len_slice(bytes)?;
        let mut deque = VecDeque::with_capacity(len);
        let used = decode_sequence_elements::<T, _>(bytes, |value| {
            deque.push_back(value);
            Ok(())
        })?;
        Ok((deque, used))
    }
}

impl<'a, T> DecodeFromSlice<'a> for LinkedList<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let mut list = LinkedList::new();
        let used = decode_sequence_elements::<T, _>(bytes, |value| {
            list.push_back(value);
            Ok(())
        })?;
        Ok((list, used))
    }
}

impl<'a, T> DecodeFromSlice<'a> for BinaryHeap<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Ord,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (len, _) = read_seq_len_slice(bytes)?;
        let mut heap = BinaryHeap::with_capacity(len);
        let used = decode_sequence_elements::<T, _>(bytes, |value| {
            heap.push(value);
            Ok(())
        })?;
        Ok((heap, used))
    }
}

impl<'a, T> DecodeFromSlice<'a> for BTreeSet<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Ord,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let mut out = BTreeSet::new();
        let used = decode_sequence_elements::<T, _>(bytes, |value| {
            if !out.insert(value) {
                return Err(Error::LengthMismatch);
            }
            Ok(())
        })?;
        Ok((out, used))
    }
}

impl<'a, T> DecodeFromSlice<'a> for HashSet<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Eq + Hash + Ord,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (len, _) = read_seq_len_slice(bytes)?;
        let mut out = HashSet::with_capacity(len);
        let used = decode_sequence_elements::<T, _>(bytes, |value| {
            if !out.insert(value) {
                return Err(Error::LengthMismatch);
            }
            Ok(())
        })?;
        Ok((out, used))
    }
}

fn decode_map_entries<'a, K, V, F>(
    bytes: &'a [u8],
    mut on_entry: F,
) -> Result<(usize, usize), Error>
where
    K: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
    F: FnMut(K, V) -> Result<(), Error>,
{
    let (len, mut offset) = read_seq_len_slice(bytes)?;
    if use_packed_seq() {
        let entries = len.checked_add(1).ok_or(Error::LengthMismatch)?;
        let table_len = entries.checked_mul(8).ok_or(Error::LengthMismatch)?;
        let offsets_bytes = table_len.checked_mul(2).ok_or(Error::LengthMismatch)?;
        let header_end = offset
            .checked_add(offsets_bytes)
            .ok_or(Error::LengthMismatch)?;
        if header_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }

        let key_offsets = bytes
            .get(offset..offset + table_len)
            .ok_or(Error::LengthMismatch)?;
        let value_offsets_start = offset.checked_add(table_len).ok_or(Error::LengthMismatch)?;
        let value_offsets = bytes
            .get(value_offsets_start..header_end)
            .ok_or(Error::LengthMismatch)?;
        offset = header_end;

        let key_total = validate_fixed_offset_table(key_offsets, entries)?;
        let val_total = validate_fixed_offset_table(value_offsets, entries)?;
        let key_data_start = offset;
        let key_data_end = key_data_start
            .checked_add(key_total)
            .ok_or(Error::LengthMismatch)?;
        let val_data_start = key_data_end;
        let val_data_end = val_data_start
            .checked_add(val_total)
            .ok_or(Error::LengthMismatch)?;
        if val_data_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }

        for idx in 0..len {
            let key_start_offset = read_fixed_offset_usize_at(key_offsets, idx)?;
            let key_end_offset = read_fixed_offset_usize_at(key_offsets, idx + 1)?;
            let key_start = key_data_start
                .checked_add(key_start_offset)
                .ok_or(Error::LengthMismatch)?;
            let key_end = key_data_start
                .checked_add(key_end_offset)
                .ok_or(Error::LengthMismatch)?;
            if key_end > key_data_end || key_start > key_end {
                return Err(Error::LengthMismatch);
            }
            let key_slice = bytes.get(key_start..key_end).ok_or(Error::LengthMismatch)?;
            record_slice_access(key_slice, key_end - key_start);
            let (key, key_used) = decode_field_canonical::<K>(key_slice)?;
            if key_used != key_end - key_start {
                return Err(Error::LengthMismatch);
            }

            let value_start_offset = read_fixed_offset_usize_at(value_offsets, idx)?;
            let value_end_offset = read_fixed_offset_usize_at(value_offsets, idx + 1)?;
            let value_start = val_data_start
                .checked_add(value_start_offset)
                .ok_or(Error::LengthMismatch)?;
            let value_end = val_data_start
                .checked_add(value_end_offset)
                .ok_or(Error::LengthMismatch)?;
            if value_end > val_data_end || value_start > value_end {
                return Err(Error::LengthMismatch);
            }
            let value_slice = bytes
                .get(value_start..value_end)
                .ok_or(Error::LengthMismatch)?;
            record_slice_access(value_slice, value_end - value_start);
            let (value, value_used) = decode_field_canonical::<V>(value_slice)?;
            if value_used != value_end - value_start {
                return Err(Error::LengthMismatch);
            }
            on_entry(key, value)?;
        }
        return Ok((len, val_data_end));
    }

    for _ in 0..len {
        let (key_len, key_hdr) = read_len_dyn_slice(&bytes[offset..])?;
        offset = offset.checked_add(key_hdr).ok_or(Error::LengthMismatch)?;
        let key_end = offset.checked_add(key_len).ok_or(Error::LengthMismatch)?;
        if key_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let key_slice = &bytes[offset..key_end];
        record_slice_access(key_slice, key_len);
        let (key, key_used) = decode_field_canonical::<K>(key_slice)?;
        if key_used != key_len {
            return Err(Error::LengthMismatch);
        }
        offset = key_end;

        let (value_len, value_hdr) = read_len_dyn_slice(&bytes[offset..])?;
        offset = offset.checked_add(value_hdr).ok_or(Error::LengthMismatch)?;
        let value_end = offset.checked_add(value_len).ok_or(Error::LengthMismatch)?;
        if value_end > bytes.len() {
            return Err(Error::LengthMismatch);
        }
        let value_slice = &bytes[offset..value_end];
        record_slice_access(value_slice, value_len);
        let (value, value_used) = decode_field_canonical::<V>(value_slice)?;
        if value_used != value_len {
            return Err(Error::LengthMismatch);
        }
        offset = value_end;
        on_entry(key, value)?;
    }
    Ok((len, offset))
}

impl<'a, K, V> DecodeFromSlice<'a> for BTreeMap<K, V>
where
    K: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Ord,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let mut out = BTreeMap::new();
        let (_, used) = decode_map_entries::<K, V, _>(bytes, |key, value| {
            if out.insert(key, value).is_some() {
                return Err(Error::LengthMismatch);
            }
            Ok(())
        })?;
        Ok((out, used))
    }
}

impl<'a, K, V> DecodeFromSlice<'a> for HashMap<K, V>
where
    K: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Eq + Hash + Ord,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (len, _) = read_seq_len_slice(bytes)?;
        let mut out = HashMap::with_capacity(len);
        let (_, used) = decode_map_entries::<K, V, _>(bytes, |key, value| {
            if out.insert(key, value).is_some() {
                return Err(Error::LengthMismatch);
            }
            Ok(())
        })?;
        Ok((out, used))
    }
}

impl<K, V> NoritoSerialize for BTreeMap<K, V>
where
    K: NoritoSerialize + Ord,
    V: NoritoSerialize,
{
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let len = self.len();
        write_seq_len(
            &mut writer,
            u64::try_from(len).map_err(|_| Error::LengthMismatch)?,
        )?;

        let packed = use_packed_seq();
        if !packed {
            let mut buffer = Vec::new();
            if let Some(max_field_len) = map_max_field_len_hint(self.iter()) {
                buffer
                    .try_reserve(max_field_len)
                    .map_err(|_| Error::LengthMismatch)?;
            }
            let flags = effective_layout_flags();
            for (key, value) in self.iter() {
                buffer.clear();
                key.serialize(&mut buffer)?;
                write_len_with_flags(
                    &mut writer,
                    u64::try_from(buffer.len()).map_err(|_| Error::LengthMismatch)?,
                    flags,
                )?;
                writer.write_all(&buffer)?;

                buffer.clear();
                value.serialize(&mut buffer)?;
                write_len_with_flags(
                    &mut writer,
                    u64::try_from(buffer.len()).map_err(|_| Error::LengthMismatch)?,
                    flags,
                )?;
                writer.write_all(&buffer)?;
            }
            return Ok(());
        }

        note_fixed_offsets_emitted();
        let table_len = len
            .checked_add(1)
            .and_then(|entries| entries.checked_mul(core::mem::size_of::<u64>()))
            .ok_or(Error::LengthMismatch)?;
        let tables_len = table_len.checked_mul(2).ok_or(Error::LengthMismatch)?;
        let mut packed = vec![0; tables_len];
        let mut data_reserve = 0usize;
        for (key, value) in self.iter() {
            if let Some(hint) = key.encoded_len_exact().or_else(|| key.encoded_len_hint()) {
                data_reserve = data_reserve
                    .checked_add(hint)
                    .ok_or(Error::LengthMismatch)?;
            }
            if let Some(hint) = value
                .encoded_len_exact()
                .or_else(|| value.encoded_len_hint())
            {
                data_reserve = data_reserve
                    .checked_add(hint)
                    .ok_or(Error::LengthMismatch)?;
            }
        }
        if data_reserve > 0 {
            packed
                .try_reserve(data_reserve)
                .map_err(|_| Error::LengthMismatch)?;
        }

        let mut key_total = 0u64;
        for (idx, (key, _)) in self.iter().enumerate() {
            let elem_start = packed.len();
            key.serialize(&mut packed)?;
            let elem_len = packed
                .len()
                .checked_sub(elem_start)
                .ok_or(Error::LengthMismatch)?;
            key_total = key_total
                .checked_add(u64::try_from(elem_len).map_err(|_| Error::LengthMismatch)?)
                .ok_or(Error::LengthMismatch)?;
            let offset_pos = idx
                .checked_add(1)
                .and_then(|offset| offset.checked_mul(core::mem::size_of::<u64>()))
                .ok_or(Error::LengthMismatch)?;
            packed[offset_pos..offset_pos + core::mem::size_of::<u64>()]
                .copy_from_slice(&key_total.to_le_bytes());
        }

        let mut value_total = 0u64;
        for (idx, (_, value)) in self.iter().enumerate() {
            let elem_start = packed.len();
            value.serialize(&mut packed)?;
            let elem_len = packed
                .len()
                .checked_sub(elem_start)
                .ok_or(Error::LengthMismatch)?;
            value_total = value_total
                .checked_add(u64::try_from(elem_len).map_err(|_| Error::LengthMismatch)?)
                .ok_or(Error::LengthMismatch)?;
            let offset_pos = idx
                .checked_add(1)
                .and_then(|offset| offset.checked_mul(core::mem::size_of::<u64>()))
                .and_then(|offset| table_len.checked_add(offset))
                .ok_or(Error::LengthMismatch)?;
            packed[offset_pos..offset_pos + core::mem::size_of::<u64>()]
                .copy_from_slice(&value_total.to_le_bytes());
        }

        writer.write_all(&packed)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        map_encoded_len_hint(self.len(), self.iter())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        map_encoded_len_exact(self.len(), self.iter())
    }
}

impl<'a, K, V> NoritoDeserialize<'a> for BTreeMap<K, V>
where
    K: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Ord,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn deserialize(archived: &'a Archived<BTreeMap<K, V>>) -> Self {
        Self::try_deserialize(archived).expect("BTreeMap decode")
    }

    fn try_deserialize(archived: &'a Archived<BTreeMap<K, V>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let offset = (ptr as usize)
            .checked_sub(base)
            .ok_or(Error::LengthMismatch)?;
        if offset > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let bytes = &payload[offset..];
        let (map, _used) = <BTreeMap<K, V> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(map)
    }
}

impl<K, V> NoritoSerialize for HashMap<K, V>
where
    K: NoritoSerialize + Eq + Hash + Ord,
    V: NoritoSerialize,
{
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let mut entries: Vec<_> = self.iter().collect();
        entries.sort_by(|(ka, _), (kb, _)| ka.cmp(kb));
        let len = entries.len();
        write_seq_len(
            &mut writer,
            u64::try_from(len).map_err(|_| Error::LengthMismatch)?,
        )?;

        let packed = use_packed_seq();
        if !packed {
            let mut buffer = Vec::new();
            if let Some(max_field_len) = map_max_field_len_hint(entries.iter().copied()) {
                buffer
                    .try_reserve(max_field_len)
                    .map_err(|_| Error::LengthMismatch)?;
            }
            let flags = effective_layout_flags();
            for (key, value) in entries.into_iter() {
                buffer.clear();
                key.serialize(&mut buffer)?;
                write_len_with_flags(
                    &mut writer,
                    u64::try_from(buffer.len()).map_err(|_| Error::LengthMismatch)?,
                    flags,
                )?;
                writer.write_all(&buffer)?;

                buffer.clear();
                value.serialize(&mut buffer)?;
                write_len_with_flags(
                    &mut writer,
                    u64::try_from(buffer.len()).map_err(|_| Error::LengthMismatch)?,
                    flags,
                )?;
                writer.write_all(&buffer)?;
            }
            return Ok(());
        }

        note_fixed_offsets_emitted();
        let table_len = len
            .checked_add(1)
            .and_then(|entries| entries.checked_mul(core::mem::size_of::<u64>()))
            .ok_or(Error::LengthMismatch)?;
        let tables_len = table_len.checked_mul(2).ok_or(Error::LengthMismatch)?;
        let mut packed = vec![0; tables_len];
        let mut data_reserve = 0usize;
        for &(key, value) in &entries {
            if let Some(hint) = key.encoded_len_exact().or_else(|| key.encoded_len_hint()) {
                data_reserve = data_reserve
                    .checked_add(hint)
                    .ok_or(Error::LengthMismatch)?;
            }
            if let Some(hint) = value
                .encoded_len_exact()
                .or_else(|| value.encoded_len_hint())
            {
                data_reserve = data_reserve
                    .checked_add(hint)
                    .ok_or(Error::LengthMismatch)?;
            }
        }
        if data_reserve > 0 {
            packed
                .try_reserve(data_reserve)
                .map_err(|_| Error::LengthMismatch)?;
        }

        let mut key_total = 0u64;
        for (idx, &(key, _)) in entries.iter().enumerate() {
            let elem_start = packed.len();
            key.serialize(&mut packed)?;
            let elem_len = packed
                .len()
                .checked_sub(elem_start)
                .ok_or(Error::LengthMismatch)?;
            key_total = key_total
                .checked_add(u64::try_from(elem_len).map_err(|_| Error::LengthMismatch)?)
                .ok_or(Error::LengthMismatch)?;
            let offset_pos = idx
                .checked_add(1)
                .and_then(|offset| offset.checked_mul(core::mem::size_of::<u64>()))
                .ok_or(Error::LengthMismatch)?;
            packed[offset_pos..offset_pos + core::mem::size_of::<u64>()]
                .copy_from_slice(&key_total.to_le_bytes());
        }

        let mut value_total = 0u64;
        for (idx, &(_, value)) in entries.iter().enumerate() {
            let elem_start = packed.len();
            value.serialize(&mut packed)?;
            let elem_len = packed
                .len()
                .checked_sub(elem_start)
                .ok_or(Error::LengthMismatch)?;
            value_total = value_total
                .checked_add(u64::try_from(elem_len).map_err(|_| Error::LengthMismatch)?)
                .ok_or(Error::LengthMismatch)?;
            let offset_pos = idx
                .checked_add(1)
                .and_then(|offset| offset.checked_mul(core::mem::size_of::<u64>()))
                .and_then(|offset| table_len.checked_add(offset))
                .ok_or(Error::LengthMismatch)?;
            packed[offset_pos..offset_pos + core::mem::size_of::<u64>()]
                .copy_from_slice(&value_total.to_le_bytes());
        }

        writer.write_all(&packed)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        map_encoded_len_hint(self.len(), self.iter())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        map_encoded_len_exact(self.len(), self.iter())
    }
}

impl<'a, K, V> NoritoDeserialize<'a> for HashMap<K, V>
where
    K: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Eq + Hash + Ord,
    V: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn deserialize(archived: &'a Archived<HashMap<K, V>>) -> Self {
        Self::try_deserialize(archived).expect("HashMap decode")
    }

    fn try_deserialize(archived: &'a Archived<HashMap<K, V>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let offset = (ptr as usize)
            .checked_sub(base)
            .ok_or(Error::LengthMismatch)?;
        if offset > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let bytes = &payload[offset..];
        let (map, _used) = <HashMap<K, V> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(map)
    }
}

impl<T> NoritoSerialize for BTreeSet<T>
where
    T: NoritoSerialize + Ord,
{
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        encode_seq_payloads(
            &mut writer,
            self.len(),
            self.iter(),
            |item, buf| item.serialize(buf),
            |item| item.encoded_len_exact().or_else(|| item.encoded_len_hint()),
        )
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        sequence_encoded_len_hint(self.len(), self.iter())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        sequence_encoded_len_exact(self.len(), self.iter())
    }
}

impl<'a, T> NoritoDeserialize<'a> for BTreeSet<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Ord,
{
    fn deserialize(archived: &'a Archived<BTreeSet<T>>) -> Self {
        Self::try_deserialize(archived).expect("BTreeSet decode")
    }

    fn try_deserialize(archived: &'a Archived<BTreeSet<T>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let offset = (ptr as usize)
            .checked_sub(base)
            .ok_or(Error::LengthMismatch)?;
        if offset > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let bytes = &payload[offset..];
        let (set, _used) = <BTreeSet<T> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(set)
    }
}

impl<T> NoritoSerialize for HashSet<T>
where
    T: NoritoSerialize + Eq + Hash + Ord,
{
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let mut items: Vec<_> = self.iter().collect();
        items.sort();
        encode_seq_payloads(
            &mut writer,
            items.len(),
            items,
            |item, buf| item.serialize(buf),
            |item| item.encoded_len_exact().or_else(|| item.encoded_len_hint()),
        )
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        sequence_encoded_len_hint(self.len(), self.iter())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        sequence_encoded_len_exact(self.len(), self.iter())
    }
}

impl<'a, T> NoritoDeserialize<'a> for HashSet<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize + Eq + Hash + Ord,
{
    fn deserialize(archived: &'a Archived<HashSet<T>>) -> Self {
        Self::try_deserialize(archived).expect("HashSet decode")
    }

    fn try_deserialize(archived: &'a Archived<HashSet<T>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let offset = (ptr as usize)
            .checked_sub(base)
            .ok_or(Error::LengthMismatch)?;
        if offset > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let bytes = &payload[offset..];
        let (set, _used) = <HashSet<T> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(set)
    }
}

impl<'a, T> DecodeFromSlice<'a> for Box<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (payload, used) = parse_owned_payload(bytes)?;
        let (value, consumed) = decode_field_canonical::<T>(payload)?;
        if consumed != payload.len() {
            return Err(Error::LengthMismatch);
        }
        Ok((Box::new(value), used))
    }
}

impl<'a, T> DecodeFromSlice<'a> for Rc<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (payload, used) = parse_owned_payload(bytes)?;
        let (value, consumed) = decode_field_canonical::<T>(payload)?;
        if consumed != payload.len() {
            return Err(Error::LengthMismatch);
        }
        Ok((Rc::new(value), used))
    }
}

impl<'a, T> DecodeFromSlice<'a> for Arc<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (payload, used) = parse_owned_payload(bytes)?;
        let (value, consumed) = decode_field_canonical::<T>(payload)?;
        if consumed != payload.len() {
            return Err(Error::LengthMismatch);
        }
        Ok((Arc::new(value), used))
    }
}

impl<'a, T: DecodeFromSlice<'a> + Copy> DecodeFromSlice<'a> for Cell<T> {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (v, used) = T::decode_from_slice(bytes)?;
        Ok((Cell::new(v), used))
    }
}

impl<'a, T: DecodeFromSlice<'a>> DecodeFromSlice<'a> for RefCell<T> {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (v, used) = T::decode_from_slice(bytes)?;
        Ok((RefCell::new(v), used))
    }
}

macro_rules! impl_decode_tuple_from_slice {
    ($( $name:ident $var:ident ),+ ) => {
        impl<'a, $( $name: DecodeFromSlice<'a> ),+> DecodeFromSlice<'a> for ( $( $name, )+ ) {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
                let mut offset = 0usize;
                $(
                    let (len, hdr) = read_len_dyn_slice(&bytes[offset..])?;
                    offset += hdr;
                    let ($var, used) = $name::decode_from_slice(&bytes[offset..offset + len])?;
                    debug_assert_eq!(used, len);
                    offset += len;
                )+
                Ok((( $( $var, )+ ), offset))
            }
        }
    };
}

impl_decode_tuple_from_slice!(A a, B b);
impl_decode_tuple_from_slice!(A a, B b, C c);
impl_decode_tuple_from_slice!(A a, B b, C c, D d);
impl_decode_tuple_from_slice!(A a, B b, C c, D d, E e);
impl_decode_tuple_from_slice!(A a, B b, C c, D d, E e, F f);
impl_decode_tuple_from_slice!(A a, B b, C c, D d, E e, F f, G g);
impl_decode_tuple_from_slice!(A a, B b, C c, D d, E e, F f, G g, H h);
impl_decode_tuple_from_slice!(A a, B b, C c, D d, E e, F f, G g, H h, I i);
impl_decode_tuple_from_slice!(A a, B b, C c, D d, E e, F f, G g, H h, I i, J j);
impl_decode_tuple_from_slice!(A a, B b, C c, D d, E e, F f, G g, H h, I i, J j, K k);
impl_decode_tuple_from_slice!(A a, B b, C c, D d, E e, F f, G g, H h, I i, J j, K k, L l);

impl<'a, T> DecodeFromSlice<'a> for Option<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let tag = *bytes.first().ok_or(Error::LengthMismatch)?;
        record_slice_access(bytes, 1);
        match tag {
            0 => Ok((None, 1)),
            1 => {
                let (len, hdr) = read_len_dyn_slice(&bytes[1..])?;
                let start = 1 + hdr;
                let end = start.checked_add(len).ok_or(Error::LengthMismatch)?;
                if end > bytes.len() {
                    return Err(Error::LengthMismatch);
                }
                let flags = effective_layout_flags();
                let _flags_guard = DecodeFlagsGuard::enter(flags);
                let schema = <T as NoritoSerialize>::schema_hash();
                let align = core::mem::align_of::<Archived<T>>();
                let archive = ArchiveSlice::new(&bytes[start..end], align)?;
                let slice = archive.as_slice();
                let payload_guard =
                    PayloadCtxGuard::enter_with_schema_flags_hint(slice, schema, flags, flags);
                let archived = unsafe { &*(slice.as_ptr() as *const Archived<T>) };
                let value = crate::guarded_try_deserialize(|| T::try_deserialize(archived))?;
                drop(payload_guard);
                record_slice_access(bytes, end);
                Ok((Some(value), end))
            }
            _ => Err(Error::Message("invalid option tag".into())),
        }
    }
}

impl<'a, T: DecodeFromSlice<'a>, E: DecodeFromSlice<'a>> DecodeFromSlice<'a> for Result<T, E> {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let tag = *bytes.first().ok_or(Error::LengthMismatch)?;
        record_slice_access(bytes, 1);
        match tag {
            0 => {
                let (len, hdr) = read_len_dyn_slice(&bytes[1..])?;
                let start = 1 + hdr;
                let end = start.checked_add(len).ok_or(Error::LengthMismatch)?;
                if end > bytes.len() {
                    return Err(Error::LengthMismatch);
                }
                let (val, used) = T::decode_from_slice(&bytes[start..end])?;
                record_slice_access(bytes, end);
                debug_assert_eq!(used, len);
                Ok((Ok(val), end))
            }
            1 => {
                let (len, hdr) = read_len_dyn_slice(&bytes[1..])?;
                let start = 1 + hdr;
                let end = start.checked_add(len).ok_or(Error::LengthMismatch)?;
                if end > bytes.len() {
                    return Err(Error::LengthMismatch);
                }
                let (err, used) = E::decode_from_slice(&bytes[start..end])?;
                record_slice_access(bytes, end);
                debug_assert_eq!(used, len);
                Ok((Err(err), end))
            }
            _ => Err(Error::Message("invalid result tag".into())),
        }
    }
}

impl<'a> DecodeFromSlice<'a> for Box<str> {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        String::decode_from_slice(bytes).map(|(s, used)| (s.into_boxed_str(), used))
    }
}

impl<'a> DecodeFromSlice<'a> for Cow<'a, str> {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        <&'a str as DecodeFromSlice>::decode_from_slice(bytes)
            .map(|(s, used)| (Cow::Borrowed(s), used))
    }
}

/// Small, stack-backed write buffer to avoid heap allocations for small fields.
///
/// This buffer writes into a fixed-size stack array first and spills over to a
/// heap `Vec<u8>` only if needed. It is intended for short-lived, per-field
/// serialization to compute length-prefixed payloads in a single pass without
/// extra allocations for common, small cases.
pub struct SmallBuf<const N: usize = 256> {
    stack: [u8; N],
    len: usize,
    spill: Vec<u8>,
    spilled: bool,
}

#[allow(clippy::new_without_default)]
impl<const N: usize> SmallBuf<N> {
    /// Create a new empty buffer.
    pub fn new() -> Self {
        Self {
            stack: [0; N],
            len: 0,
            spill: Vec::new(),
            spilled: false,
        }
    }

    /// Reset to empty without freeing spill capacity.
    pub fn clear(&mut self) {
        self.len = 0;
        if self.spilled {
            self.spill.clear();
            self.spilled = false;
        }
    }

    /// Current length in bytes.
    pub fn len(&self) -> usize {
        if self.spilled {
            self.spill.len()
        } else {
            self.len
        }
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a read-only view of the written bytes.
    pub fn as_slice(&self) -> &[u8] {
        if self.spilled {
            &self.spill
        } else {
            &self.stack[..self.len]
        }
    }
}

impl<const N: usize> std::io::Write for SmallBuf<N> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if !self.spilled {
            let avail = N.saturating_sub(self.len);
            if buf.len() <= avail {
                // write fits in stack
                self.stack[self.len..self.len + buf.len()].copy_from_slice(buf);
                self.len += buf.len();
                return Ok(buf.len());
            }
            // spill to heap: move existing stack content
            self.spilled = true;
            self.spill.reserve(self.len + buf.len());
            self.spill.extend_from_slice(&self.stack[..self.len]);
        }
        // write to spill vec
        self.spill.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Preferred small temporary buffer size for derive-generated serializers.
///
/// This size balances stack usage and the goal of avoiding heap spills for
/// common small and medium fields. Adjust via benchmarks as needed.
pub const DERIVE_SMALLBUF_SIZE: usize = 384;

/// Type alias used by proc-macro derives for their per-field temporary buffer.
pub type DeriveSmallBuf = SmallBuf<{ DERIVE_SMALLBUF_SIZE }>;

/// Magic bytes that identify a Norito archive.
pub const MAGIC: [u8; 4] = *b"NRT0";

/// Current major version of the format.
pub const VERSION_MAJOR: u8 = 0;
/// Fixed v1 minor-version layout hint.
pub const V1_LAYOUT_FLAGS: u8 = 0;
/// Fixed v1 decode hint recorded in the header minor version.
pub const V1_DECODE_FLAGS: u8 = V1_LAYOUT_FLAGS;
/// Current minor version of the format.
///
/// This stays `0` for v1. Header flags carry all active layout selections for a
/// payload, including the compact-length default.
pub const VERSION_MINOR: u8 = V1_DECODE_FLAGS;

/// Compression algorithm used for the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// No compression.
    None = 0,
    /// Zstd compression.
    Zstd = 1,
}

impl Compression {
    fn from_u8(value: u8) -> Result<Self, Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Zstd),
            v => Err(Error::unsupported_compression(v)),
        }
    }
}

/// Errors returned during serialization and deserialization.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// I/O failure when reading or writing.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Invalid magic bytes.
    #[error("invalid magic header")]
    InvalidMagic,
    /// Unsupported major version in the header.
    #[error("unsupported version {found} (expected {expected})")]
    UnsupportedVersion { found: u8, expected: u8 },
    /// Unsupported minor version.
    #[error("unsupported minor version {found:#04x} (supported mask {supported:#04x})")]
    UnsupportedMinorVersion { found: u8, supported: u8 },
    /// Unknown compression algorithm or unsupported in this build.
    #[error("unsupported compression {found} (supported {supported:?})")]
    UnsupportedCompression {
        found: u8,
        supported: &'static [Compression],
    },
    /// Stored length does not match data size.
    #[error("length mismatch")]
    LengthMismatch,
    /// Archive length exceeds the supported limit for this build.
    #[error("archive length {length} exceeds limit {limit}")]
    ArchiveLengthExceeded { length: u64, limit: u64 },
    /// Checksum verification failed.
    #[error("checksum mismatch")]
    ChecksumMismatch,
    /// Schema hash does not match expected type.
    #[error("schema mismatch")]
    SchemaMismatch,
    /// Encountered a payload that uses a feature not supported by this build.
    #[error("unsupported feature: {0}")]
    UnsupportedFeature(&'static str),
    /// Missing payload context for pointer-based decode path.
    #[error("missing payload context")]
    MissingPayloadContext,
    /// Layout flags were not provided, so the payload cannot be framed deterministically.
    #[error("missing layout flags")]
    MissingLayoutFlags,
    /// Header layout flags conflict with an active decode-flag guard.
    #[error(
        "decode flags mismatch (header={header_flags:#04x}/{header_hint:#04x}, active={active_flags:#04x}/{active_hint:#04x})"
    )]
    DecodeFlagsMismatch {
        header_flags: u8,
        header_hint: u8,
        active_flags: u8,
        active_hint: u8,
    },
    /// Invalid UTF-8 in string-like value.
    #[error("invalid utf8")]
    InvalidUtf8,
    /// JSON helper error (only available when the `json` feature is enabled).
    #[cfg(feature = "json")]
    #[error("json error: {0}")]
    Json(#[from] crate::json::Error),
    /// Invalid tag/discriminant during decode (Option/Result/enum).
    #[error("invalid tag {tag} while decoding {context}")]
    InvalidTag { context: &'static str, tag: u8 },
    /// NonZero value decoded as zero.
    #[error("non-zero value expected")]
    InvalidNonZero,
    /// A panic occurred during decode; returned when a decode guard intercepted a panic.
    #[error("panic during decode of {context}")]
    DecodePanic { context: &'static str },
    /// Payload base pointer does not satisfy the required alignment.
    #[error("payload misaligned (requires {align}-byte alignment, addr={addr:#x})")]
    Misaligned { align: usize, addr: usize },
    /// Custom error message.
    #[error("{0}")]
    Message(String),
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Self::Message(msg)
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        msg.to_owned().into()
    }
}

impl Error {
    /// Construct a detailed length mismatch error while preserving the canonical variant.
    pub fn length_mismatch_detail(
        context: &'static str,
        offset: usize,
        required: usize,
        available: usize,
    ) -> Self {
        Self::Message(format!(
            "{context}: need {required} bytes at offset {offset}, but only {available} remain"
        ))
    }

    /// Construct an invalid tag error with contextual information.
    pub fn invalid_tag(context: &'static str, tag: u8) -> Self {
        Self::InvalidTag { context, tag }
    }

    /// Construct a decode panic error annotated with the type being decoded.
    pub fn decode_panic(context: &'static str) -> Self {
        Self::DecodePanic { context }
    }

    /// Construct a misalignment error including the offending address.
    pub fn misaligned(align: usize, addr: *const u8) -> Self {
        Self::Misaligned {
            align,
            addr: addr as usize,
        }
    }

    fn supported_compressions() -> &'static [Compression] {
        #[cfg(feature = "compression")]
        {
            const SUPPORTED: &[Compression] = &[Compression::None, Compression::Zstd];
            SUPPORTED
        }
        #[cfg(not(feature = "compression"))]
        {
            const SUPPORTED: &[Compression] = &[Compression::None];
            SUPPORTED
        }
    }

    /// Construct an unsupported compression error using the default supported set.
    pub fn unsupported_compression(found: u8) -> Self {
        Self::UnsupportedCompression {
            found,
            supported: Self::supported_compressions(),
        }
    }

    /// Construct an unsupported compression error with an explicit supported list.
    pub fn unsupported_compression_with(found: u8, supported: &'static [Compression]) -> Self {
        Self::UnsupportedCompression { found, supported }
    }
}

/// Metadata header that prefixes every archive.
///
/// All serialized objects start with this structure.  It allows the
/// deserializer to validate the data before interpreting the payload.
#[derive(Debug, Clone, Copy)]
pub struct Header {
    /// Magic bytes for detection.
    pub magic: [u8; 4],
    /// Major format version.
    pub major: u8,
    /// Minor format version.
    pub minor: u8,
    /// Schema hash of serialized type.
    pub schema: [u8; 16],
    /// Compression algorithm used for the payload.
    pub compression: Compression,
    /// Uncompressed payload length.
    pub length: u64,
    /// CRC64 checksum of the payload.
    pub checksum: u64,
    /// Feature flags (stored in the final padding byte).
    pub flags: u8,
}

impl Header {
    /// Size of the serialized header in bytes.
    ///
    /// All fields add up to 40 bytes. We include a single padding byte at the
    /// end so that future extensions can reuse it without shifting the payload
    /// layout.
    pub const SIZE: usize = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1;
}

#[inline]
const fn payload_alignment_padding_for_align(align: usize) -> usize {
    if align <= 1 {
        return 0;
    }
    let remainder = Header::SIZE % align;
    if remainder == 0 { 0 } else { align - remainder }
}

#[inline]
pub(crate) fn payload_alignment_padding_for<T>() -> usize {
    payload_alignment_padding_for_align(core::mem::align_of::<Archived<T>>())
}

#[inline]
fn append_payload_with_padding<T>(out: &mut Vec<u8>, payload: &[u8]) {
    let padding = payload_alignment_padding_for::<T>();
    if padding != 0 {
        let len = out.len();
        out.resize(len + padding, 0);
    }
    out.extend_from_slice(payload);
}

impl Default for Header {
    fn default() -> Self {
        Self::new([0; 16], 0, 0)
    }
}

impl Header {
    /// Create a new header with provided fields.
    ///
    /// This helper sets the magic bytes and current version while allowing the
    /// caller to specify the schema hash, payload length and checksum.
    pub fn new(schema: [u8; 16], length: u64, checksum: u64) -> Self {
        Self {
            magic: MAGIC,
            major: VERSION_MAJOR,
            minor: VERSION_MINOR,
            schema,
            compression: Compression::None,
            length,
            checksum,
            flags: 0,
        }
    }

    /// Write the header to the given writer.
    fn write(&self, mut w: impl Write) -> Result<(), Error> {
        validate_header_flags(self.flags)?;
        w.write_all(&self.magic)?;
        w.write_all(&[self.major])?;
        w.write_all(&[self.minor])?;
        w.write_all(&self.schema)?;
        w.write_all(&[self.compression as u8])?;
        w.write_all(&self.length.to_le_bytes())?;
        w.write_all(&self.checksum.to_le_bytes())?;
        w.write_all(&[self.flags])?; // flags in padding byte
        Ok(())
    }

    /// Read a header from the given reader and validate it.
    pub(crate) fn read(mut r: impl Read) -> Result<Self, Error> {
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(Error::InvalidMagic);
        }
        let major = r.read_u8()?;
        if major != VERSION_MAJOR {
            return Err(Error::UnsupportedVersion {
                found: major,
                expected: VERSION_MAJOR,
            });
        }
        let minor = r.read_u8()?;
        if minor != VERSION_MINOR {
            return Err(Error::UnsupportedMinorVersion {
                found: minor,
                supported: VERSION_MINOR,
            });
        }
        let mut schema = [0u8; 16];
        r.read_exact(&mut schema)?;
        let compression = Compression::from_u8(r.read_u8()?)?;
        let length = r.read_u64::<LittleEndian>()?;
        let checksum = r.read_u64::<LittleEndian>()?;
        let mut pad = [0u8; 1];
        r.read_exact(&mut pad)?;
        let flags = pad[0];
        validate_header_flags(flags)?;
        Ok(Self {
            magic,
            major,
            minor,
            schema,
            compression,
            length,
            checksum,
            flags,
        })
    }
}

/// Trait implemented for types that can be archived.
///
/// The archived representation of `Self` is [`Archived<Self>`].
pub trait NoritoSerialize {
    /// Hash representing the schema of the type.
    ///
    /// Implementations may override this if they want a custom scheme, but by
    /// default it is derived from the fully qualified Rust type name.
    fn schema_hash() -> [u8; 16]
    where
        Self: Sized,
    {
        compute_schema_hash::<Self>()
    }
    /// Serialize `self` into the writer.
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error>;

    /// Optional hint: estimated encoded byte length for `self`.
    ///
    /// Implementations should return `Some(len)` when the exact or a tight
    /// upper-bound length is cheap to compute, otherwise return `None`.
    /// The encoder uses this to pre-reserve buffer capacity to reduce
    /// reallocations. Returning an underestimate may cause reallocations;
    /// an overestimate only over-allocates the buffer.
    fn encoded_len_hint(&self) -> Option<usize> {
        None
    }

    /// Exact encoded length for this value, if cheaply computable.
    ///
    /// Returns the exact number of bytes that `serialize()` will write for the
    /// archived payload (excluding the outer Norito header). Implementations
    /// should avoid expensive work; return `None` when exact sizing is not
    /// readily available.
    fn encoded_len_exact(&self) -> Option<usize> {
        None
    }
}

/// Trait for reconstructing types from their archived form.
pub trait NoritoDeserialize<'a>: Sized {
    /// Schema hash used for validation.
    ///
    /// By default this mirrors [`NoritoSerialize::schema_hash`].
    fn schema_hash() -> [u8; 16] {
        compute_schema_hash::<Self>()
    }
    /// Deserialize from bytes without allocations.
    fn deserialize(archived: &'a Archived<Self>) -> Self;

    /// Fallible deserialization helper.
    ///
    /// Types that can fail during deserialization can override this method to
    /// surface an error instead of panicking. By default deserialization is
    /// assumed to be infallible and wraps [`NoritoDeserialize::deserialize`].
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
        Ok(Self::deserialize(archived))
    }
}

/// Archived representation of a value. For most primitive types this is just the
/// value itself in little-endian form. For structs it contains offsets for
/// dynamic data.
#[repr(C)]
pub struct Archived<T: ?Sized>(T);

impl<T: ?Sized> Archived<T> {
    /// Reinterpret the archived value as a different archived type.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the archived representation of `T` and `U`
    /// is identical. This is typically true for newtype wrappers with
    /// `#[repr(transparent)]`.
    #[allow(unsafe_code)]
    pub fn cast<U>(&self) -> &Archived<U> {
        // SAFETY: the caller guarantees that `T` and `U` have the same archived
        // layout so reinterpreting the pointer is sound.
        unsafe { &*core::ptr::from_ref(self).cast::<Archived<U>>() }
    }
}

enum ArchivedBacking<'a> {
    Borrowed(&'a [u8]),
    Owned(ArchiveSlice),
}

/// Owned or borrowed handle to an archived value reconstructed from a byte
/// slice. When the input slice is misaligned for `T` the bytes are copied into
/// an internal buffer with suitable alignment, ensuring the returned reference
/// is always well-formed.
pub struct ArchivedRef<'a, T: ?Sized> {
    ptr: *const Archived<T>,
    backing: ArchivedBacking<'a>,
    _marker: PhantomData<&'a Archived<T>>,
}

impl<'a, T: ?Sized> ArchivedRef<'a, T> {
    #[inline]
    #[allow(unsafe_code)]
    fn as_ref_impl(&self) -> &Archived<T> {
        // SAFETY: `ptr` always points to a properly aligned `Archived<T>`
        // backed by either the borrowed slice or an owned aligned buffer.
        unsafe { &*self.ptr }
    }

    /// Reference to the archived value.
    #[inline]
    pub fn archived(&self) -> &Archived<T> {
        self.as_ref_impl()
    }

    /// Underlying byte slice backing the archived value.
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        match &self.backing {
            ArchivedBacking::Borrowed(bytes) => bytes,
            ArchivedBacking::Owned(slice) => slice.as_slice(),
        }
    }

    /// Reinterpret the archived value as a different archived type.
    #[inline]
    pub fn cast<U>(&self) -> &Archived<U> {
        self.archived().cast::<U>()
    }
}

impl<'a, T: ?Sized> std::convert::AsRef<Archived<T>> for ArchivedRef<'a, T> {
    #[inline]
    fn as_ref(&self) -> &Archived<T> {
        self.as_ref_impl()
    }
}

impl<'a, T: ?Sized> std::ops::Deref for ArchivedRef<'a, T> {
    type Target = Archived<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_ref_impl()
    }
}

/// Owned archived payload backed by an aligned byte buffer.
///
/// This owns the payload bytes (plus any alignment padding) and guarantees the
/// archived pointer stays valid for the lifetime of the wrapper.
pub struct ArchivedBox<T> {
    buffer: Vec<u8>,
    offset: usize,
    len: usize,
    _marker: PhantomData<Archived<T>>,
}

impl<T> ArchivedBox<T> {
    #[inline]
    pub(crate) fn from_payload(payload: Vec<u8>) -> Self {
        let len = payload.len();
        let align = core::mem::align_of::<Archived<T>>();
        if len == 0 || align <= 1 {
            return Self {
                buffer: payload,
                offset: 0,
                len,
                _marker: PhantomData,
            };
        }
        let base = payload.as_ptr() as usize;
        if base.is_multiple_of(align) {
            return Self {
                buffer: payload,
                offset: 0,
                len,
                _marker: PhantomData,
            };
        }

        let mut buffer = vec![0u8; len + align];
        let buf_base = buffer.as_ptr() as usize;
        let offset = (align - (buf_base % align)) % align;
        buffer[offset..offset + len].copy_from_slice(&payload);
        Self {
            buffer,
            offset,
            len,
            _marker: PhantomData,
        }
    }

    /// Reference to the archived value.
    #[inline]
    pub fn archived(&self) -> &Archived<T> {
        if self.len == 0 {
            // SAFETY: zero-sized archived values do not require backing storage.
            return unsafe { &*core::ptr::NonNull::<Archived<T>>::dangling().as_ptr() };
        }
        let ptr = unsafe { self.buffer.as_ptr().add(self.offset) as *const Archived<T> };
        // SAFETY: `ptr` always points into `buffer` with `Archived<T>` alignment.
        unsafe { &*ptr }
    }

    /// Underlying archived bytes.
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        if self.len == 0 {
            return &[];
        }
        &self.buffer[self.offset..self.offset + self.len]
    }
}

impl<T> std::convert::AsRef<Archived<T>> for ArchivedBox<T> {
    #[inline]
    fn as_ref(&self) -> &Archived<T> {
        self.archived()
    }
}

impl<T> std::ops::Deref for ArchivedBox<T> {
    type Target = Archived<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.archived()
    }
}

/// Interpret `bytes` as an archived value and ensure the slice is large enough.
///
/// Returns an error if `bytes` is shorter than the minimum archived header for `T`.
#[inline]
pub fn archived_from_slice<'a, T>(bytes: &'a [u8]) -> Result<ArchivedRef<'a, T>, Error> {
    let min = core::mem::size_of::<Archived<T>>();
    if min > 0 && bytes.len() < min {
        return Err(Error::LengthMismatch);
    }
    Ok(archived_from_slice_unchecked(bytes))
}

/// Interpret `bytes` as an archived value, realigning the payload if required.
///
/// This helper mirrors the historical behaviour of Iroha's block decoder which
/// accepted misaligned archived payloads emitted by earlier Norito encoders.
/// Callers must ensure the slice contains a valid archived representation of
/// `T`. When the input is misaligned the bytes are copied into an internal
/// buffer so the returned reference is always properly aligned.
#[inline]
#[allow(unsafe_code)]
pub fn archived_from_slice_unchecked<'a, T>(bytes: &'a [u8]) -> ArchivedRef<'a, T> {
    debug_assert!(
        bytes.len() >= core::mem::size_of::<Archived<T>>()
            || core::mem::size_of::<Archived<T>>() == 0,
        "slice shorter than Archived<{}>",
        core::any::type_name::<T>()
    );
    let type_align = core::mem::align_of::<Archived<T>>();
    let min_align = core::mem::align_of::<u128>();
    let align_target = type_align.max(min_align);
    let ptr_usize = bytes.as_ptr() as usize;
    let needs_copy =
        align_target > 1 && !bytes.is_empty() && !ptr_usize.is_multiple_of(align_target);
    if needs_copy {
        let slice = ArchiveSlice::new(bytes, align_target).unwrap_or_else(|err| {
            panic!(
                "failed to realign archived payload for {}: {err:?}",
                core::any::type_name::<T>()
            )
        });
        let ptr = slice.as_slice().as_ptr() as *const Archived<T>;
        ArchivedRef {
            ptr,
            backing: ArchivedBacking::Owned(slice),
            _marker: PhantomData,
        }
    } else {
        let ptr = if bytes.is_empty() {
            core::ptr::NonNull::<Archived<T>>::dangling().as_ptr()
        } else {
            bytes.as_ptr() as *const Archived<T>
        };
        ArchivedRef {
            ptr,
            backing: ArchivedBacking::Borrowed(bytes),
            _marker: PhantomData,
        }
    }
}

impl NoritoSerialize for u8 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&[*self])?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(1)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(1)
    }
}

impl<'a> NoritoDeserialize<'a> for u8 {
    fn deserialize(archived: &'a Archived<u8>) -> Self {
        archived.0
    }
}

impl NoritoSerialize for i8 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&[*self as u8])?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(1)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(1)
    }
}

impl<'a> NoritoDeserialize<'a> for i8 {
    fn deserialize(archived: &'a Archived<i8>) -> Self {
        archived.0
    }
}

impl NoritoSerialize for u16 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(2)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(2)
    }
}

impl<'a> NoritoDeserialize<'a> for u16 {
    fn deserialize(archived: &'a Archived<u16>) -> Self {
        u16::from_le(archived.0)
    }
}

impl NoritoSerialize for NonZeroU16 {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        self.get().serialize(writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.get().encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.get().encoded_len_exact()
    }
}

impl<'a> NoritoDeserialize<'a> for NonZeroU16 {
    fn deserialize(archived: &'a Archived<NonZeroU16>) -> Self {
        let val_arch: &Archived<u16> = archived.cast();
        let value = <u16 as NoritoDeserialize>::deserialize(val_arch);
        NonZeroU16::new(value).expect("non-zero")
    }

    fn try_deserialize(archived: &'a Archived<NonZeroU16>) -> Result<Self, Error> {
        let val_arch: &Archived<u16> = archived.cast();
        let value = <u16 as NoritoDeserialize>::deserialize(val_arch);
        NonZeroU16::new(value).ok_or(Error::InvalidNonZero)
    }
}

impl NoritoSerialize for i16 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(2)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(2)
    }
}

impl<'a> NoritoDeserialize<'a> for i16 {
    fn deserialize(archived: &'a Archived<i16>) -> Self {
        i16::from_le(archived.0)
    }
}

impl NoritoSerialize for u32 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(4)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(4)
    }
}

impl<'a> NoritoDeserialize<'a> for u32 {
    fn deserialize(archived: &'a Archived<u32>) -> Self {
        u32::from_le(archived.0)
    }
}

impl NoritoSerialize for NonZeroU32 {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        self.get().serialize(writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.get().encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.get().encoded_len_exact()
    }
}

impl<'a> NoritoDeserialize<'a> for NonZeroU32 {
    fn deserialize(archived: &'a Archived<NonZeroU32>) -> Self {
        let val_arch: &Archived<u32> = archived.cast();
        let value = <u32 as NoritoDeserialize>::deserialize(val_arch);
        NonZeroU32::new(value).expect("non-zero")
    }

    fn try_deserialize(archived: &'a Archived<NonZeroU32>) -> Result<Self, Error> {
        let val_arch: &Archived<u32> = archived.cast();
        let value = <u32 as NoritoDeserialize>::deserialize(val_arch);
        NonZeroU32::new(value).ok_or(Error::InvalidNonZero)
    }
}

impl NoritoSerialize for bool {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&[*self as u8])?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(1)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(1)
    }
}

impl<'a> NoritoDeserialize<'a> for bool {
    fn deserialize(archived: &'a Archived<bool>) -> Self {
        archived.0
    }
}

impl NoritoSerialize for char {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&(*self as u32).to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(4)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(4)
    }
}

impl<'a> NoritoDeserialize<'a> for char {
    fn deserialize(archived: &'a Archived<char>) -> Self {
        // SAFETY-FREE: decode via integer path stored little-endian
        let v = u32::from_le(unsafe { *(archived as *const _ as *const u32) });
        char::from_u32(v).expect("invalid char")
    }
}

impl NoritoSerialize for () {
    fn serialize<W: Write>(&self, _writer: W) -> Result<(), Error> {
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(0)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(0)
    }
}

impl<'a> NoritoDeserialize<'a> for () {
    fn deserialize(_archived: &'a Archived<()>) -> Self {
        // unit type has no data
    }
}

impl<T> NoritoSerialize for PhantomData<T> {
    fn serialize<W: Write>(&self, _writer: W) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, T> NoritoDeserialize<'a> for PhantomData<T> {
    fn deserialize(_archived: &'a Archived<PhantomData<T>>) -> Self {
        PhantomData
    }
}

impl<'a, T> DecodeFromSlice<'a> for PhantomData<T> {
    fn decode_from_slice(_bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        Ok((PhantomData, 0))
    }
}

impl NoritoSerialize for i32 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(4)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(4)
    }
}

impl<'a> NoritoDeserialize<'a> for i32 {
    fn deserialize(archived: &'a Archived<i32>) -> Self {
        i32::from_le(archived.0)
    }
}

impl NoritoSerialize for u64 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(8)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(8)
    }
}

impl<'a> NoritoDeserialize<'a> for u64 {
    fn deserialize(archived: &'a Archived<u64>) -> Self {
        u64::from_le(archived.0)
    }
}

impl NoritoSerialize for NonZeroU64 {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        self.get().serialize(writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.get().encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.get().encoded_len_exact()
    }
}

impl<'a> NoritoDeserialize<'a> for NonZeroU64 {
    fn deserialize(archived: &'a Archived<NonZeroU64>) -> Self {
        let val_arch: &Archived<u64> = archived.cast();
        let value = <u64 as NoritoDeserialize>::deserialize(val_arch);
        NonZeroU64::new(value).expect("non-zero")
    }

    fn try_deserialize(archived: &'a Archived<NonZeroU64>) -> Result<Self, Error> {
        let val_arch: &Archived<u64> = archived.cast();
        let value = <u64 as NoritoDeserialize>::deserialize(val_arch);
        NonZeroU64::new(value).ok_or(Error::InvalidNonZero)
    }
}

impl NoritoSerialize for i64 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(8)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(8)
    }
}

impl<'a> NoritoDeserialize<'a> for i64 {
    fn deserialize(archived: &'a Archived<i64>) -> Self {
        i64::from_le(archived.0)
    }
}

impl NoritoSerialize for usize {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&(*self as u64).to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(8)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(8)
    }
}

impl<'a> NoritoDeserialize<'a> for usize {
    fn deserialize(archived: &'a Archived<usize>) -> Self {
        usize::from_le(archived.0)
    }
}

impl NoritoSerialize for isize {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&(*self as i64).to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(8)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(8)
    }
}

impl<'a> NoritoDeserialize<'a> for isize {
    fn deserialize(archived: &'a Archived<isize>) -> Self {
        isize::from_le(archived.0)
    }
}

impl NoritoSerialize for u128 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(16)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(16)
    }
}

impl<'a> NoritoDeserialize<'a> for u128 {
    fn deserialize(archived: &'a Archived<u128>) -> Self {
        u128::from_le(archived.0)
    }
}

impl NoritoSerialize for i128 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(16)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(16)
    }
}

impl<'a> NoritoDeserialize<'a> for i128 {
    fn deserialize(archived: &'a Archived<i128>) -> Self {
        i128::from_le(archived.0)
    }
}

impl NoritoSerialize for f32 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_bits().to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(4)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(4)
    }
}

impl<'a> NoritoDeserialize<'a> for f32 {
    fn deserialize(archived: &'a Archived<f32>) -> Self {
        // Prefer reading from the validated payload context when available.
        // Falls back to a direct pointer read otherwise.
        let ptr = archived as *const _ as *const u8;
        if let Some((base, total)) = payload_ctx() {
            let ptr_us = ptr as usize;
            let base_end = match base.checked_add(total) {
                Some(v) => v,
                None => {
                    return Self::from_bits(u32::from_le_bytes({
                        let mut tmp = [0u8; 4];
                        unsafe { tmp.copy_from_slice(std::slice::from_raw_parts(ptr, 4)) };
                        tmp
                    }));
                }
            };
            let ptr_end = match ptr_us.checked_add(4) {
                Some(v) => v,
                None => {
                    return Self::from_bits(u32::from_le_bytes({
                        let mut tmp = [0u8; 4];
                        unsafe { tmp.copy_from_slice(std::slice::from_raw_parts(ptr, 4)) };
                        tmp
                    }));
                }
            };
            if ptr_us >= base && ptr_end <= base_end {
                let off = ptr_us - base;
                let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
                let mut lb = [0u8; 4];
                let slice_end = match off.checked_add(4) {
                    Some(v) => v,
                    None => {
                        return f32::from_bits(u32::from_le_bytes({
                            let mut tmp = [0u8; 4];
                            unsafe { tmp.copy_from_slice(std::slice::from_raw_parts(ptr, 4)) };
                            tmp
                        }));
                    }
                };
                if slice_end <= total {
                    lb.copy_from_slice(&payload[off..slice_end]);
                    return f32::from_bits(u32::from_le_bytes(lb));
                }
            }
        }
        let mut bytes = [0u8; 4];
        unsafe {
            bytes.copy_from_slice(std::slice::from_raw_parts(ptr, 4));
        }
        f32::from_bits(u32::from_le_bytes(bytes))
    }
}

impl NoritoSerialize for f64 {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        writer.write_all(&self.to_bits().to_le_bytes())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(8)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(8)
    }
}

impl<'a> NoritoDeserialize<'a> for f64 {
    fn deserialize(archived: &'a Archived<f64>) -> Self {
        let ptr = archived as *const _ as *const u8;
        if let Some((base, total)) = payload_ctx() {
            let ptr_us = ptr as usize;
            let base_end = match base.checked_add(total) {
                Some(v) => v,
                None => {
                    let mut tmp = [0u8; 8];
                    unsafe { tmp.copy_from_slice(std::slice::from_raw_parts(ptr, 8)) };
                    return f64::from_bits(u64::from_le_bytes(tmp));
                }
            };
            let ptr_end = match ptr_us.checked_add(8) {
                Some(v) => v,
                None => {
                    let mut tmp = [0u8; 8];
                    unsafe { tmp.copy_from_slice(std::slice::from_raw_parts(ptr, 8)) };
                    return f64::from_bits(u64::from_le_bytes(tmp));
                }
            };
            if ptr_us >= base && ptr_end <= base_end {
                let off = ptr_us - base;
                let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
                let mut lb = [0u8; 8];
                let slice_end = match off.checked_add(8) {
                    Some(v) => v,
                    None => {
                        let mut tmp = [0u8; 8];
                        unsafe { tmp.copy_from_slice(std::slice::from_raw_parts(ptr, 8)) };
                        return f64::from_bits(u64::from_le_bytes(tmp));
                    }
                };
                if slice_end <= total {
                    lb.copy_from_slice(&payload[off..slice_end]);
                    return f64::from_bits(u64::from_le_bytes(lb));
                }
            }
        }
        let mut bytes = [0u8; 8];
        unsafe {
            bytes.copy_from_slice(std::slice::from_raw_parts(ptr, 8));
        }
        f64::from_bits(u64::from_le_bytes(bytes))
    }
}

#[inline]
unsafe fn decode_archived_string_without_ctx(ptr: *const u8) -> String {
    unsafe { decode_archived_string_without_ctx_fixed(ptr) }
}

#[inline]
unsafe fn decode_archived_string_without_ctx_fixed(ptr: *const u8) -> String {
    let mut len_bytes = [0u8; 8];
    let len_src = unsafe { core::slice::from_raw_parts(ptr, 8) };
    len_bytes.copy_from_slice(len_src);
    let len = len_u64_to_usize(u64::from_le_bytes(len_bytes))
        .unwrap_or_else(|_| panic!("norito: archived String length overflow"));
    if 8usize.checked_add(len).is_none() {
        panic!("norito: archived String length overflow");
    }
    let body_ptr = unsafe { ptr.add(8) };
    let bytes = unsafe { std::slice::from_raw_parts(body_ptr, len) };
    String::from_utf8(bytes.to_vec()).expect("norito: invalid UTF-8 in archived String")
}

impl NoritoSerialize for String {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let bytes = self.as_bytes();
        let len = u64::try_from(bytes.len()).map_err(|_| Error::LengthMismatch)?;
        write_len_with_flags(&mut writer, len, effective_layout_flags())?;
        writer.write_all(bytes)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        len_prefixed_payload_len(self.len())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        len_prefixed_payload_len(self.len())
    }
}

impl<'a> NoritoDeserialize<'a> for String {
    fn deserialize(archived: &'a Archived<String>) -> Self {
        match Self::try_deserialize(archived) {
            Ok(value) => value,
            Err(Error::MissingPayloadContext) => {
                let ptr = archived as *const _ as *const u8;
                unsafe { decode_archived_string_without_ctx(ptr) }
            }
            Err(err) => panic!("norito: invalid archived String: {err:?}"),
        }
    }

    fn try_deserialize(archived: &'a Archived<String>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let ptr_us = ptr as usize;
        if ptr_us < base || ptr_us >= base + total {
            return Err(Error::LengthMismatch);
        }
        let (len, hdr) = read_len_dyn_at_ptr(ptr)?;
        let off = (ptr_us - base) + hdr;
        let end = off.checked_add(len).ok_or(Error::LengthMismatch)?;
        if end > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        unsafe {
            record_payload_access(payload.as_ptr().add(off), len);
        }
        let bytes = &payload[off..end];
        String::from_utf8(bytes.to_vec()).map_err(|_| Error::InvalidUtf8)
    }
}

impl NoritoSerialize for &str {
    fn schema_hash() -> [u8; 16] {
        compute_schema_hash::<String>()
    }

    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let bytes = self.as_bytes();
        let len = u64::try_from(bytes.len()).map_err(|_| Error::LengthMismatch)?;
        write_len_with_flags(&mut writer, len, effective_layout_flags())?;
        writer.write_all(bytes)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        len_prefixed_payload_len(self.len())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        len_prefixed_payload_len(self.len())
    }
}

impl<'a> NoritoDeserialize<'a> for &'a str {
    fn schema_hash() -> [u8; 16] {
        compute_schema_hash::<String>()
    }

    fn deserialize(archived: &'a Archived<&'a str>) -> Self {
        match Self::try_deserialize(archived) {
            Ok(value) => value,
            Err(Error::MissingPayloadContext) => {
                let ptr = archived as *const _ as *const u8;
                unsafe {
                    let mut len_bytes = [0u8; 8];
                    len_bytes.copy_from_slice(std::slice::from_raw_parts(ptr, 8));
                    let len = len_u64_to_usize(u64::from_le_bytes(len_bytes))
                        .unwrap_or_else(|_| panic!("norito: archived &str length overflow"));
                    let bytes = std::slice::from_raw_parts(ptr.add(8), len);
                    std::str::from_utf8(bytes).expect("invalid utf8")
                }
            }
            Err(err) => panic!("norito: invalid archived &str: {err:?}"),
        }
    }

    fn try_deserialize(archived: &'a Archived<&'a str>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let ptr_us = ptr as usize;
        if ptr_us < base || ptr_us >= base + total {
            return Err(Error::LengthMismatch);
        }
        let (len, hdr) = read_len_dyn_at_ptr(ptr)?;
        let off = (ptr_us - base) + hdr;
        let end = off.checked_add(len).ok_or(Error::LengthMismatch)?;
        if end > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        unsafe {
            record_payload_access(payload.as_ptr().add(off), len);
        }
        let bytes = &payload[off..end];
        std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
    }
}

// Strict-safe implementations for decoding from slices (no raw pointers)
impl<'a> DecodeFromSlice<'a> for String {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (len, hdr) = read_len_dyn_slice(bytes)?;
        let end = hdr.checked_add(len).ok_or(Error::LengthMismatch)?;
        let data = bytes.get(hdr..end).ok_or(Error::LengthMismatch)?;
        let s =
            String::from_utf8(data.to_vec()).map_err(|_| Error::Message("invalid utf8".into()))?;
        record_slice_access(bytes, end);
        Ok((s, end))
    }
}

impl<'a> DecodeFromSlice<'a> for &'a str {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
        let (len, hdr) = read_len_dyn_slice(bytes)?;
        let end = hdr.checked_add(len).ok_or(Error::LengthMismatch)?;
        let data = bytes.get(hdr..end).ok_or(Error::LengthMismatch)?;
        let s = std::str::from_utf8(data).map_err(|_| Error::Message("invalid utf8".into()))?;
        record_slice_access(bytes, end);
        Ok((s, end))
    }
}

impl NoritoSerialize for Cow<'_, str> {
    fn schema_hash() -> [u8; 16] {
        compute_schema_hash::<String>()
    }

    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let bytes = self.as_ref().as_bytes();
        let len = u64::try_from(bytes.len()).map_err(|_| Error::LengthMismatch)?;
        write_len_with_flags(&mut writer, len, effective_layout_flags())?;
        writer.write_all(bytes)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        len_prefixed_payload_len(self.len())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        len_prefixed_payload_len(self.len())
    }
}

impl<'a> NoritoDeserialize<'a> for Cow<'a, str> {
    fn schema_hash() -> [u8; 16] {
        compute_schema_hash::<String>()
    }

    fn deserialize(archived: &'a Archived<Cow<'a, str>>) -> Self {
        let ptr = archived as *const Archived<Cow<'a, str>> as *const Archived<&'a str>;
        let s = <&'a str as NoritoDeserialize>::deserialize(unsafe { &*ptr });
        Cow::Borrowed(s)
    }
}

impl NoritoSerialize for Box<str> {
    fn schema_hash() -> [u8; 16] {
        compute_schema_hash::<String>()
    }

    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        self.as_ref().serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        len_prefixed_payload_len(self.len())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        len_prefixed_payload_len(self.len())
    }
}

impl<'a> NoritoDeserialize<'a> for Box<str> {
    fn schema_hash() -> [u8; 16] {
        compute_schema_hash::<String>()
    }

    fn deserialize(archived: &'a Archived<Box<str>>) -> Self {
        let s = String::deserialize(archived.cast());
        s.into_boxed_str()
    }

    fn try_deserialize(archived: &'a Archived<Box<str>>) -> Result<Self, Error> {
        guarded_try_deserialize(|| String::try_deserialize(archived.cast()))
            .map(|s| s.into_boxed_str())
    }
}

impl<T: NoritoSerialize> NoritoSerialize for Box<T> {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        serialize_owned(writer, &**self)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        (**self)
            .encoded_len_exact()
            .or_else(|| (**self).encoded_len_hint())
            .and_then(len_prefixed_payload_len)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        (**self)
            .encoded_len_exact()
            .and_then(len_prefixed_payload_len)
    }
}

impl<'a, T> NoritoDeserialize<'a> for Box<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    fn deserialize(archived: &'a Archived<Box<T>>) -> Self {
        Self::try_deserialize(archived).expect("Box decode")
    }

    fn try_deserialize(archived: &'a Archived<Box<T>>) -> Result<Self, Error> {
        let bytes = owned_bytes_from_ctx(archived)?;
        let (value, _) = <Box<T> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(value)
    }
}

impl<T: NoritoSerialize> NoritoSerialize for Rc<T> {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        serialize_owned(writer, &**self)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        (**self)
            .encoded_len_exact()
            .or_else(|| (**self).encoded_len_hint())
            .and_then(len_prefixed_payload_len)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        (**self)
            .encoded_len_exact()
            .and_then(len_prefixed_payload_len)
    }
}

impl<'a, T> NoritoDeserialize<'a> for Rc<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    fn deserialize(archived: &'a Archived<Rc<T>>) -> Self {
        Self::try_deserialize(archived).expect("Rc decode")
    }

    fn try_deserialize(archived: &'a Archived<Rc<T>>) -> Result<Self, Error> {
        let bytes = owned_bytes_from_ctx(archived)?;
        let (value, _) = <Rc<T> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(value)
    }
}

impl<T: NoritoSerialize> NoritoSerialize for Arc<T> {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        serialize_owned(writer, &**self)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        (**self)
            .encoded_len_exact()
            .or_else(|| (**self).encoded_len_hint())
            .and_then(len_prefixed_payload_len)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        (**self)
            .encoded_len_exact()
            .and_then(len_prefixed_payload_len)
    }
}

impl<'a, T> NoritoDeserialize<'a> for Arc<T>
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    fn deserialize(archived: &'a Archived<Arc<T>>) -> Self {
        Self::try_deserialize(archived).expect("Arc decode")
    }

    fn try_deserialize(archived: &'a Archived<Arc<T>>) -> Result<Self, Error> {
        let bytes = owned_bytes_from_ctx(archived)?;
        let (value, _) = <Arc<T> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(value)
    }
}

impl<T: NoritoSerialize + Copy> NoritoSerialize for Cell<T> {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        self.get().serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(core::mem::size_of::<T>())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(core::mem::size_of::<T>())
    }
}

impl<'a, T: NoritoDeserialize<'a> + Copy> NoritoDeserialize<'a> for Cell<T> {
    fn deserialize(archived: &'a Archived<Cell<T>>) -> Self {
        let inner = unsafe { &*(archived as *const _ as *const Archived<T>) };
        Cell::new(T::deserialize(inner))
    }

    fn try_deserialize(archived: &'a Archived<Cell<T>>) -> Result<Self, Error> {
        let inner = unsafe { &*(archived as *const _ as *const Archived<T>) };
        guarded_try_deserialize(|| T::try_deserialize(inner)).map(Cell::new)
    }
}

impl<T: NoritoSerialize> NoritoSerialize for RefCell<T> {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        self.borrow().serialize(writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.borrow().encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.borrow().encoded_len_exact()
    }
}

impl<'a, T: NoritoDeserialize<'a>> NoritoDeserialize<'a> for RefCell<T> {
    fn deserialize(archived: &'a Archived<RefCell<T>>) -> Self {
        let inner = unsafe { &*(archived as *const _ as *const Archived<T>) };
        RefCell::new(T::deserialize(inner))
    }

    fn try_deserialize(archived: &'a Archived<RefCell<T>>) -> Result<Self, Error> {
        let inner = unsafe { &*(archived as *const _ as *const Archived<T>) };
        guarded_try_deserialize(|| T::try_deserialize(inner)).map(RefCell::new)
    }
}

impl<T: NoritoSerialize> NoritoSerialize for Option<T> {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let mut tmp: DeriveSmallBuf = DeriveSmallBuf::new();
        match self {
            Some(value) => {
                writer.write_all(&[1])?;
                write_len_prefixed(&mut writer, value, &mut tmp)?;
            }
            None => {
                writer.write_all(&[0])?;
            }
        }
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        match self {
            Some(v) => v
                .encoded_len_exact()
                .or_else(|| v.encoded_len_hint())
                .and_then(tagged_len_prefixed_payload_len),
            None => Some(1),
        }
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        match self {
            Some(v) => v
                .encoded_len_exact()
                .and_then(tagged_len_prefixed_payload_len),
            None => Some(1),
        }
    }
}

impl<'a, T> NoritoDeserialize<'a> for Option<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn deserialize(archived: &'a Archived<Option<T>>) -> Self {
        match Self::try_deserialize(archived) {
            Ok(value) => value,
            Err(err) => panic!("norito: Option decode failed: {err:?}"),
        }
    }

    fn try_deserialize(archived: &'a Archived<Option<T>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let tag = unsafe { *ptr };
        match tag {
            0 => Ok(None),
            1 => {
                let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
                let ptr_us = ptr as usize;
                if ptr_us < base || ptr_us >= base + total {
                    return Err(Error::LengthMismatch);
                }
                let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
                let start = ptr_us - base + 1; // skip tag
                if start > payload.len() {
                    return Err(Error::LengthMismatch);
                }
                let bytes = &payload[start..];
                let (data_len, hdr) = read_len_dyn_slice(bytes)?;
                let data_start = hdr;
                let data_end = data_start
                    .checked_add(data_len)
                    .ok_or(Error::LengthMismatch)?;
                if data_end > bytes.len() {
                    return Err(Error::LengthMismatch);
                }
                let archived_slice = &bytes[data_start..data_end];
                let (value, used) = decode_field_canonical::<T>(archived_slice)?;
                if used != data_len {
                    return Err(Error::LengthMismatch);
                }
                Ok(Some(value))
            }
            _ => Err(Error::invalid_tag("Option::try_deserialize", tag)),
        }
    }
}

impl<T: NoritoSerialize, E: NoritoSerialize> NoritoSerialize for Result<T, E> {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let mut tmp: DeriveSmallBuf = DeriveSmallBuf::new();
        match self {
            Ok(value) => {
                writer.write_all(&[0])?;
                write_len_prefixed(&mut writer, value, &mut tmp)?;
            }
            Err(err) => {
                writer.write_all(&[1])?;
                write_len_prefixed(&mut writer, err, &mut tmp)?;
            }
        }
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        match self {
            Ok(v) => v
                .encoded_len_exact()
                .or_else(|| v.encoded_len_hint())
                .and_then(tagged_len_prefixed_payload_len),
            Err(e) => e
                .encoded_len_exact()
                .or_else(|| e.encoded_len_hint())
                .and_then(tagged_len_prefixed_payload_len),
        }
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        match self {
            Ok(v) => v
                .encoded_len_exact()
                .and_then(tagged_len_prefixed_payload_len),
            Err(e) => e
                .encoded_len_exact()
                .and_then(tagged_len_prefixed_payload_len),
        }
    }
}

impl<'a, T, E> NoritoDeserialize<'a> for Result<T, E>
where
    T: NoritoDeserialize<'a> + DecodeFromSlice<'a>,
    E: NoritoDeserialize<'a> + DecodeFromSlice<'a>,
{
    fn deserialize(archived: &'a Archived<Result<T, E>>) -> Self {
        match Self::try_deserialize(archived) {
            Ok(value) => value,
            Err(err) => panic!("norito: Result decode failed: {err:?}"),
        }
    }

    fn try_deserialize(archived: &'a Archived<Result<T, E>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let tag = unsafe { *ptr };
        let (len, hdr) = unsafe { try_read_len_ptr_unchecked(ptr.add(1)) }?;
        let data_ptr = unsafe { ptr.add(1 + hdr) };
        match tag {
            0 => {
                let layout = Layout::from_size_align(len, core::mem::align_of::<Archived<T>>())
                    .map_err(|_| Error::LengthMismatch)?;
                let (tmp_ptr, needs_dealloc) = unsafe { alloc_checked(layout) };
                if let Err(err) = unsafe { copy_from_payload(data_ptr, tmp_ptr, len) } {
                    unsafe { dealloc_checked(tmp_ptr, layout, needs_dealloc) };
                    return Err(err);
                }
                let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, len) };
                let _g = PayloadCtxGuard::enter(tmp_slice);
                let value = guarded_try_deserialize(|| unsafe {
                    T::try_deserialize(&*(tmp_ptr as *const Archived<T>))
                });
                unsafe { dealloc_checked(tmp_ptr, layout, needs_dealloc) };
                value.map(Ok)
            }
            1 => {
                let layout = Layout::from_size_align(len, core::mem::align_of::<Archived<E>>())
                    .map_err(|_| Error::LengthMismatch)?;
                let (tmp_ptr, needs_dealloc) = unsafe { alloc_checked(layout) };
                if let Err(err) = unsafe { copy_from_payload(data_ptr, tmp_ptr, len) } {
                    unsafe { dealloc_checked(tmp_ptr, layout, needs_dealloc) };
                    return Err(err);
                }
                let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, len) };
                let _g = PayloadCtxGuard::enter(tmp_slice);
                let err = guarded_try_deserialize(|| unsafe {
                    E::try_deserialize(&*(tmp_ptr as *const Archived<E>))
                });
                unsafe { dealloc_checked(tmp_ptr, layout, needs_dealloc) };
                err.map(Err)
            }
            _ => Err(Error::invalid_tag("Result::try_deserialize", tag)),
        }
    }
}

impl<T: NoritoSerialize, const N: usize> NoritoSerialize for [T; N] {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let mut buf = Vec::new();
        if let Some(max_hint) = self
            .iter()
            .filter_map(|item| item.encoded_len_exact().or_else(|| item.encoded_len_hint()))
            .max()
        {
            buf.try_reserve(max_hint)
                .map_err(|_| Error::LengthMismatch)?;
        }
        let flags = effective_layout_flags();
        for item in self {
            buf.clear();
            item.serialize(&mut buf)?;
            write_len_with_flags(
                &mut writer,
                u64::try_from(buf.len()).map_err(|_| Error::LengthMismatch)?,
                flags,
            )?;
            writer.write_all(&buf)?;
        }
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        let flags = effective_layout_flags();
        let mut total = 0usize;
        for item in self {
            let elem_len = item
                .encoded_len_exact()
                .or_else(|| item.encoded_len_hint())?;
            total = total
                .checked_add(len_prefix_len_with_flags(elem_len, flags))?
                .checked_add(elem_len)?;
        }
        Some(total)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let flags = effective_layout_flags();
        let mut total = 0usize;
        for item in self {
            let elem_len = item.encoded_len_exact()?;
            total = total
                .checked_add(len_prefix_len_with_flags(elem_len, flags))?
                .checked_add(elem_len)?;
        }
        Some(total)
    }
}

impl<'a, T: NoritoDeserialize<'a> + 'static, const N: usize> NoritoDeserialize<'a> for [T; N] {
    fn deserialize(archived: &'a Archived<[T; N]>) -> Self {
        match Self::try_deserialize(archived) {
            Ok(value) => value,
            Err(err) => panic!("norito: array decode failed: {err:?}"),
        }
    }

    fn try_deserialize(archived: &'a Archived<[T; N]>) -> Result<Self, Error> {
        if TypeId::of::<T>() == TypeId::of::<u8>() {
            let ptr = archived as *const _ as *const u8;
            let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
            if ptr as usize > base + total {
                return Err(Error::LengthMismatch);
            }
            let start = (ptr as usize)
                .checked_sub(base)
                .ok_or(Error::LengthMismatch)?;
            let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
            let slice = &payload[start..];
            let (value_u8, used) = <[u8; N] as DecodeFromSlice>::decode_from_slice(slice)?;
            record_payload_access(ptr, used);
            let mut arr = core::mem::MaybeUninit::<[T; N]>::uninit();
            unsafe {
                core::ptr::copy_nonoverlapping(
                    value_u8.as_ptr() as *const T,
                    arr.as_mut_ptr() as *mut T,
                    N,
                );
                return Ok(arr.assume_init());
            }
        }

        let ptr = archived as *const _ as *const u8;
        let mut offset = 0usize;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let start = (ptr as usize).saturating_sub(base);
        if start > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let bytes = &payload[start..];
        let mut out: Vec<T> = Vec::with_capacity(N);
        for _ in 0..N {
            let (elem_len, hdr) = read_len_dyn_slice(&bytes[offset..])?;
            offset += hdr;
            unsafe {
                let layout =
                    Layout::from_size_align(elem_len, core::mem::align_of::<Archived<T>>())
                        .map_err(|_| Error::LengthMismatch)?;
                let (tmp_ptr, needs_dealloc) = alloc_checked(layout);
                if let Err(err) = copy_from_payload(bytes[offset..].as_ptr(), tmp_ptr, elem_len) {
                    dealloc_checked(tmp_ptr, layout, needs_dealloc);
                    return Err(err);
                }
                let tmp_slice = std::slice::from_raw_parts(tmp_ptr as *const u8, elem_len);
                let _g = PayloadCtxGuard::enter(tmp_slice);
                let archived = &*(tmp_ptr as *const Archived<T>);
                let v = guarded_try_deserialize(|| T::try_deserialize(archived))?;
                dealloc_checked(tmp_ptr, layout, needs_dealloc);
                out.push(v);
            }
            offset += elem_len;
        }
        out.try_into().map_err(|_| Error::LengthMismatch)
    }
}

pub mod stream {
    #[cfg(feature = "compression")]
    use std::io::BufReader;
    use std::{
        alloc::{Layout, alloc, dealloc, handle_alloc_error},
        io::{self, Read},
    };

    use crc64fast::Digest;

    use super::{
        Archived, Compression, DecodeFlagsGuard, Error, Header, NoritoDeserialize, PayloadCtxGuard,
        header_flags,
    };
    use crate::guarded_try_deserialize;

    pub(crate) fn fold_sequence_from_reader<R, T, Acc, Init, F>(
        mut reader: R,
        init: Init,
        mut f: F,
        expected_schema: [u8; 16],
        padding: usize,
    ) -> Result<Acc, Error>
    where
        R: Read,
        T: for<'de> NoritoDeserialize<'de>,
        Init: FnOnce(usize) -> Acc,
        F: FnMut(Acc, T) -> Acc,
    {
        let header = Header::read(&mut reader)?;
        super::prepare_header_decode(header.flags, header.minor, false)?;
        if header.schema != expected_schema {
            return Err(Error::SchemaMismatch);
        }
        let payload_len = super::payload_len_to_usize(header.length)?;

        let padding = match header.compression {
            Compression::None => padding,
            Compression::Zstd => 0,
        };
        if padding != 0 {
            skip_padding(&mut reader, padding)?;
        }
        let _fg = DecodeFlagsGuard::enter(header.flags);
        let mut payload = DigestingReader::new(PayloadStream::new(reader, header.compression)?);

        let mut len_decoder = SeqLenDecoder::new(&mut payload, header.flags)?;
        let mut acc = init(len_decoder.total_len());

        let mut scratch = AlignedScratch::new();
        let archived_align = std::mem::align_of::<Archived<T>>();

        while let Some(elem_len) = len_decoder.next_len(&mut payload)? {
            if elem_len == 0 {
                if std::mem::size_of::<Archived<T>>() != 0 {
                    return Err(Error::LengthMismatch);
                }
                let _pg = PayloadCtxGuard::enter(&[]);
                let archived = unsafe { &*std::ptr::NonNull::<Archived<T>>::dangling().as_ptr() };
                let val = guarded_try_deserialize(|| {
                    <T as NoritoDeserialize>::try_deserialize(archived)
                })?;
                acc = f(acc, val);
            } else {
                unsafe {
                    let ptr = scratch.ensure(elem_len, archived_align)?;
                    let tmp_slice_mut = std::slice::from_raw_parts_mut(ptr, elem_len);
                    payload.read_exact_into(tmp_slice_mut)?;
                    let tmp_slice = std::slice::from_raw_parts(ptr as *const u8, elem_len);
                    let _pg = PayloadCtxGuard::enter(tmp_slice);
                    let archived = &*(ptr as *const Archived<T>);
                    let val = guarded_try_deserialize(|| {
                        <T as NoritoDeserialize>::try_deserialize(archived)
                    })?;
                    acc = f(acc, val);
                }
            }
        }

        if len_decoder.remaining() != 0 {
            return Err(Error::LengthMismatch);
        }

        let remaining_tail = payload_len
            .checked_sub(payload.consumed)
            .ok_or(Error::LengthMismatch)?;
        len_decoder.finish(&mut payload, remaining_tail)?;

        let _ = payload.finalize(payload_len, header.checksum)?;
        Ok(acc)
    }

    #[inline]
    pub(crate) fn u64_to_usize(value: u64) -> Result<usize, Error> {
        super::payload_len_to_usize(value)
    }

    #[inline]
    pub(crate) fn skip_padding<R: Read>(reader: &mut R, mut padding: usize) -> Result<(), Error> {
        if padding == 0 {
            return Ok(());
        }
        let mut buf = [0u8; 1024];
        while padding > 0 {
            let chunk = padding.min(buf.len());
            let read = reader.read(&mut buf[..chunk])?;
            if read == 0 {
                return Err(Error::LengthMismatch);
            }
            if buf[..read].iter().any(|&b| b != 0) {
                return Err(Error::LengthMismatch);
            }
            padding -= read;
        }
        Ok(())
    }

    pub(crate) struct AlignedScratch {
        ptr: *mut u8,
        layout: Option<Layout>,
        capacity: usize,
        align: usize,
    }

    impl AlignedScratch {
        pub(crate) fn new() -> Self {
            Self {
                ptr: std::ptr::null_mut(),
                layout: None,
                capacity: 0,
                align: 1,
            }
        }

        pub(crate) unsafe fn ensure(&mut self, len: usize, align: usize) -> Result<*mut u8, Error> {
            if len == 0 {
                return Ok(std::ptr::NonNull::<u8>::dangling().as_ptr());
            }
            if len <= self.capacity && align <= self.align {
                return Ok(self.ptr);
            }
            if !self.ptr.is_null() {
                if let Some(layout) = self.layout.take() {
                    unsafe { dealloc(self.ptr, layout) };
                }
                self.ptr = std::ptr::null_mut();
            }
            let layout = Layout::from_size_align(len, align).map_err(|_| Error::LengthMismatch)?;
            let ptr = unsafe { alloc(layout) };
            if ptr.is_null() {
                handle_alloc_error(layout);
            }
            self.ptr = ptr;
            self.capacity = len;
            self.align = align;
            self.layout = Some(layout);
            Ok(ptr)
        }
    }

    impl Drop for AlignedScratch {
        fn drop(&mut self) {
            if let Some(layout) = self.layout.take()
                && !self.ptr.is_null()
            {
                unsafe { dealloc(self.ptr, layout) };
            }
        }
    }

    pub(super) enum PayloadStream<R: Read> {
        Plain(R),
        #[cfg(feature = "compression")]
        Zstd(zstd::Decoder<'static, BufReader<R>>),
    }

    impl<R: Read> PayloadStream<R> {
        pub(super) fn new(reader: R, compression: Compression) -> Result<Self, Error> {
            match compression {
                Compression::None => Ok(Self::Plain(reader)),
                Compression::Zstd => {
                    #[cfg(feature = "compression")]
                    {
                        Ok(Self::Zstd(zstd::Decoder::new(reader)?))
                    }
                    #[cfg(not(feature = "compression"))]
                    {
                        let _ = reader;
                        Err(io::Error::other("compression support disabled").into())
                    }
                }
            }
        }
    }

    impl<R: Read> Read for PayloadStream<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self {
                PayloadStream::Plain(inner) => inner.read(buf),
                #[cfg(feature = "compression")]
                PayloadStream::Zstd(decoder) => decoder.read(buf),
            }
        }
    }

    pub(crate) struct DigestingReader<R> {
        inner: R,
        digest: Digest,
        consumed: usize,
    }

    impl<R: Read> DigestingReader<R> {
        pub(crate) fn new(inner: R) -> Self {
            Self {
                inner,
                digest: Digest::new(),
                consumed: 0,
            }
        }

        pub(crate) fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
            self.inner.read_exact(buf)?;
            self.digest.write(buf);
            self.consumed += buf.len();
            Ok(())
        }

        pub(crate) fn read_exact_into(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.read_exact(buf)?;
            Ok(buf.len())
        }

        pub(crate) fn read_u64(&mut self) -> io::Result<u64> {
            let mut b = [0u8; 8];
            self.read_exact(&mut b)?;
            Ok(u64::from_le_bytes(b))
        }

        pub(crate) fn read_varint_u64(&mut self) -> Result<u64, Error> {
            let mut result = 0u64;
            let mut shift = 0u32;
            let mut used = 0usize;
            for _ in 0..super::MAX_VARINT_BYTES {
                let mut byte = [0u8; 1];
                self.read_exact(&mut byte)?;
                used += 1;
                let b = byte[0];
                let payload = (b & 0x7f) as u64;
                if shift == 63 && payload > 1 {
                    return Err(Error::LengthMismatch);
                }
                result |= payload << shift;
                if b & 0x80 == 0 {
                    if used != super::varint_encoded_len(result) {
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

        pub(crate) fn read_varint_len(&mut self) -> Result<usize, Error> {
            let raw = self.read_varint_u64()?;
            u64_to_usize(raw)
        }

        pub(crate) fn finalize(self, expected_len: usize, checksum: u64) -> Result<R, Error> {
            if self.consumed != expected_len {
                dbg!(self.consumed, expected_len);
                return Err(Error::LengthMismatch);
            }
            if self.digest.sum64() != checksum {
                return Err(Error::ChecksumMismatch);
            }
            Ok(self.inner)
        }

        pub(crate) fn consumed(&self) -> usize {
            self.consumed
        }
    }

    pub(crate) struct SeqLenDecoder {
        flags: u8,
        total: usize,
        mode: SeqLenMode,
    }

    enum SeqLenMode {
        Plain { remaining: usize },
        Packed { lengths: Vec<usize>, index: usize },
    }

    impl SeqLenDecoder {
        #[inline]
        fn read_u64_len<R: Read>(reader: &mut DigestingReader<R>) -> Result<u64, Error> {
            reader.read_u64().map_err(|err| {
                if err.kind() == io::ErrorKind::UnexpectedEof {
                    Error::LengthMismatch
                } else {
                    Error::Io(err)
                }
            })
        }

        #[inline]
        fn map_unexpected_eof(err: Error) -> Error {
            match err {
                Error::Io(e) if e.kind() == io::ErrorKind::UnexpectedEof => Error::LengthMismatch,
                other => other,
            }
        }

        pub(crate) fn new<R: Read>(
            reader: &mut DigestingReader<R>,
            flags: u8,
        ) -> Result<Self, Error> {
            let supported = header_flags::PACKED_SEQ
                | header_flags::COMPACT_LEN
                | header_flags::PACKED_STRUCT
                | header_flags::FIELD_BITSET;
            let unsupported = flags & !supported;
            if unsupported != 0 {
                return Err(Error::UnsupportedFeature("sequence layout flag"));
            }

            let packed = (flags & header_flags::PACKED_SEQ) != 0;
            let len = Self::read_u64_len(reader)?;
            let total = u64_to_usize(len)?;

            let mode = if packed {
                let entries = total.checked_add(1).ok_or(Error::LengthMismatch)?;
                let mut offsets = Vec::with_capacity(entries);
                for _ in 0..entries {
                    let raw = Self::read_u64_len(reader)?;
                    offsets.push(u64_to_usize(raw)?);
                }
                if offsets.first().copied().unwrap_or(0) != 0 {
                    return Err(Error::LengthMismatch);
                }
                if offsets.windows(2).any(|w| w[1] < w[0]) {
                    return Err(Error::LengthMismatch);
                }
                let mut lengths = Vec::with_capacity(total);
                for pair in offsets.windows(2) {
                    lengths.push(pair[1] - pair[0]);
                }
                SeqLenMode::Packed { lengths, index: 0 }
            } else {
                SeqLenMode::Plain { remaining: total }
            };

            Ok(Self { flags, total, mode })
        }

        pub(crate) fn next_len<R: Read>(
            &mut self,
            reader: &mut DigestingReader<R>,
        ) -> Result<Option<usize>, Error> {
            match &mut self.mode {
                SeqLenMode::Plain { remaining } => {
                    if *remaining == 0 {
                        return Ok(None);
                    }
                    let len = if (self.flags & header_flags::COMPACT_LEN) != 0 {
                        reader.read_varint_len().map_err(Self::map_unexpected_eof)?
                    } else {
                        let raw = Self::read_u64_len(reader)?;
                        u64_to_usize(raw)?
                    };
                    *remaining -= 1;
                    Ok(Some(len))
                }
                SeqLenMode::Packed { lengths, index } => {
                    if *index >= lengths.len() {
                        return Ok(None);
                    }
                    let len = lengths[*index];
                    *index += 1;
                    Ok(Some(len))
                }
            }
        }

        pub(crate) fn remaining(&self) -> usize {
            match &self.mode {
                SeqLenMode::Plain { remaining } => *remaining,
                SeqLenMode::Packed { lengths, index } => lengths.len().saturating_sub(*index),
            }
        }

        pub(crate) fn total_len(&self) -> usize {
            self.total
        }

        pub(crate) fn finish<R: Read>(
            &self,
            _reader: &mut DigestingReader<R>,
            remaining_bytes: usize,
        ) -> Result<(), Error> {
            if self.remaining() != 0 || remaining_bytes != 0 {
                return Err(Error::LengthMismatch);
            }
            Ok(())
        }
    }
}

impl<T: NoritoSerialize> NoritoSerialize for VecDeque<T> {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        encode_seq_payloads(
            &mut writer,
            self.len(),
            self.iter(),
            |item, buf| item.serialize(buf),
            |item| item.encoded_len_exact().or_else(|| item.encoded_len_hint()),
        )
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        sequence_encoded_len_hint(self.len(), self.iter())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        sequence_encoded_len_exact(self.len(), self.iter())
    }
}

impl<'a, T> NoritoDeserialize<'a> for VecDeque<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn deserialize(archived: &'a Archived<VecDeque<T>>) -> Self {
        Self::try_deserialize(archived).expect("norito: VecDeque decode failed")
    }

    fn try_deserialize(archived: &'a Archived<VecDeque<T>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let offset = (ptr as usize)
            .checked_sub(base)
            .ok_or(Error::LengthMismatch)?;
        if offset > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let bytes = &payload[offset..];
        let (deque, _used) = <VecDeque<T> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(deque)
    }
}

impl<T: NoritoSerialize> NoritoSerialize for LinkedList<T> {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        encode_seq_payloads(
            &mut writer,
            self.len(),
            self.iter(),
            |item, buf| item.serialize(buf),
            |item| item.encoded_len_exact().or_else(|| item.encoded_len_hint()),
        )
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        sequence_encoded_len_hint(self.len(), self.iter())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        sequence_encoded_len_exact(self.len(), self.iter())
    }
}

impl<'a, T> NoritoDeserialize<'a> for LinkedList<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn deserialize(archived: &'a Archived<LinkedList<T>>) -> Self {
        Self::try_deserialize(archived).expect("norito: LinkedList decode failed")
    }

    fn try_deserialize(archived: &'a Archived<LinkedList<T>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let offset = (ptr as usize)
            .checked_sub(base)
            .ok_or(Error::LengthMismatch)?;
        if offset > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let bytes = &payload[offset..];
        let (list, _used) = <LinkedList<T> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(list)
    }
}

impl<T> NoritoSerialize for BinaryHeap<T>
where
    T: NoritoSerialize + Ord,
{
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        let mut items: Vec<_> = self.iter().collect();
        items.sort();
        encode_seq_payloads(
            &mut writer,
            items.len(),
            items,
            |item, buf| item.serialize(buf),
            |item| item.encoded_len_exact().or_else(|| item.encoded_len_hint()),
        )
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        sequence_encoded_len_hint(self.len(), self.iter())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        sequence_encoded_len_exact(self.len(), self.iter())
    }
}

impl<'a, T> NoritoDeserialize<'a> for BinaryHeap<T>
where
    T: NoritoDeserialize<'a> + Ord,
{
    fn deserialize(archived: &'a Archived<BinaryHeap<T>>) -> Self {
        Self::try_deserialize(archived).expect("BinaryHeap decode")
    }

    fn try_deserialize(archived: &'a Archived<BinaryHeap<T>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let payload = payload_slice_from_ptr(ptr)?;
        let (len, hdr) = unsafe { read_seq_len_ptr(ptr) }?;
        ensure_len_within_remaining(len, payload.len().saturating_sub(hdr))?;
        let mut offset = hdr;
        let mut out = BinaryHeap::with_capacity(len);
        let align = core::mem::align_of::<Archived<T>>();
        let mut cap = 1usize;
        let mut layout = Layout::from_size_align(cap, align).map_err(|_| Error::LengthMismatch)?;
        let (mut buf, mut buf_needs_dealloc) = unsafe { alloc_checked(layout) };
        for _ in 0..len {
            let (elem_len, hdr) = match read_len_dyn_at_ptr(unsafe { ptr.add(offset) }) {
                Ok(res) => res,
                Err(err) => {
                    unsafe { dealloc_checked(buf, layout, buf_needs_dealloc) };
                    return Err(err);
                }
            };
            offset += hdr;
            if offset + elem_len > payload.len() {
                unsafe { dealloc_checked(buf, layout, buf_needs_dealloc) };
                return Err(Error::LengthMismatch);
            }
            if elem_len > cap {
                let new_cap = core::cmp::max(elem_len, cap.saturating_mul(2));
                let new_layout =
                    Layout::from_size_align(new_cap, align).map_err(|_| Error::LengthMismatch)?;
                unsafe { dealloc_checked(buf, layout, buf_needs_dealloc) };
                let (new_buf, new_buf_needs_dealloc) = unsafe { alloc_checked(new_layout) };
                buf = new_buf;
                buf_needs_dealloc = new_buf_needs_dealloc;
                layout = new_layout;
                cap = new_cap;
            }
            unsafe {
                if elem_len != 0 {
                    copy_from_payload(ptr.add(offset), buf, elem_len)?;
                }
                let tmp_slice = std::slice::from_raw_parts(buf as *const u8, elem_len);
                let _g = PayloadCtxGuard::enter(tmp_slice);
                let archived = &*(buf as *const Archived<T>);
                let v = guarded_try_deserialize(|| T::try_deserialize(archived))?;
                out.push(v);
            }
            offset += elem_len;
        }
        unsafe {
            dealloc_checked(buf, layout, buf_needs_dealloc);
        }
        Ok(out)
    }
}

impl<T: NoritoSerialize> NoritoSerialize for Vec<T> {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
        if core::any::type_name::<T>() == "u8" {
            let len_u64 = u64::try_from(self.len()).map_err(|_| Error::LengthMismatch)?;
            write_seq_len(&mut writer, len_u64)?;
            // SAFETY: we verified `T == u8` via `type_name`.
            let bytes =
                unsafe { core::slice::from_raw_parts(self.as_ptr().cast::<u8>(), self.len()) };
            writer.write_all(bytes)?;
            return Ok(());
        }
        encode_slice_payloads(&mut writer, self)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        if core::any::type_name::<T>() == "u8" {
            return self.len().checked_add(8);
        }
        sequence_encoded_len_hint(self.len(), self.iter())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        if core::any::type_name::<T>() == "u8" {
            return self.len().checked_add(8);
        }
        sequence_encoded_len_exact(self.len(), self.iter())
    }
}

impl<'a, T> NoritoDeserialize<'a> for Vec<T>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    fn deserialize(archived: &'a Archived<Vec<T>>) -> Self {
        Self::try_deserialize(archived).expect("Vec decode")
    }

    fn try_deserialize(archived: &'a Archived<Vec<T>>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let (base, total) = payload_ctx().ok_or(Error::MissingPayloadContext)?;
        let offset = (ptr as usize).saturating_sub(base);
        if offset > total {
            return Err(Error::LengthMismatch);
        }
        let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
        let bytes = &payload[offset..];
        let (vec, _used) = <Vec<T> as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok(vec)
    }
}

#[inline]
fn tuple_serialization_flags() -> u8 {
    let defaults = default_encode_flags();
    let dynamic_mask = header_flags::PACKED_SEQ;
    let static_defaults = defaults & !dynamic_mask;
    match current_decode_flags_effective() {
        None => defaults,
        Some(0) => 0,
        Some(current) => {
            let current_dynamic = current & dynamic_mask;
            let current_static = current & !dynamic_mask;
            let effective_static = if current_static == 0 {
                static_defaults
            } else {
                current_static | static_defaults
            };
            current_dynamic | effective_static
        }
    }
}

macro_rules! impl_tuple {
    ($( $name:ident $var:ident $idx:tt ),+ $(,)?) => {
        impl<$( $name: NoritoSerialize ),+> NoritoSerialize for ( $( $name, )+ ) {
            fn serialize<W: Write>(&self, mut writer: W) -> Result<(), Error> {
                // Ensure inner element serializers observe the same layout
                // defaults as the bare codec path (packed seq/struct and
                // compact lengths when enabled). This keeps nested encodings
                // like `Vec<u8>` consistent with the decoder's expectations.
                let __merged = tuple_serialization_flags();
                let __guard = DecodeFlagsGuard::enter_with_hint(__merged, __merged);
                // Use a small stack-backed buffer to avoid heap traffic for
                // fixed/small fields.
                let mut __buf = DeriveSmallBuf::new();
                #[cfg(debug_assertions)]
                if crate::debug_trace_enabled() {
                    eprintln!(
                        "norito.tuple.serialize: flags=0x{:02x}",
                        get_decode_flags()
                    );
                }
                $(
                    __buf.clear();
                    self.$idx.serialize(&mut __buf)?;
                    write_len_with_flags(
                        &mut writer,
                        u64::try_from(__buf.len()).map_err(|_| Error::LengthMismatch)?,
                        __merged,
                    )?;
                    writer.write_all(__buf.as_slice())?;
                )+
                Ok(())
            }

            fn encoded_len_hint(&self) -> Option<usize> {
                let mut total = 0usize;
                let flags = tuple_serialization_flags();
                $(
                    let elem_len = self.$idx
                        .encoded_len_exact()
                        .or_else(|| self.$idx.encoded_len_hint())?;
                    total = total
                        .checked_add(len_prefix_len_with_flags(elem_len, flags))?
                        .checked_add(elem_len)?;
                )+
                Some(total)
            }

            fn encoded_len_exact(&self) -> Option<usize> {
                let mut total = 0usize;
                let flags = tuple_serialization_flags();
                $(
                    let elem_len = self.$idx.encoded_len_exact()?;
                    total = total
                        .checked_add(len_prefix_len_with_flags(elem_len, flags))?
                        .checked_add(elem_len)?;
                )+
                Some(total)
            }
        }

        impl<'a, $( $name: NoritoDeserialize<'a> ),+> NoritoDeserialize<'a> for ( $( $name, )+ ) {
            fn deserialize(archived: &'a Archived<( $( $name, )+ )>) -> Self {
                let ptr = archived as *const _ as *const u8;
                let mut offset = 0usize;
                $(
                    let (elem_len, hdr) = read_len_dyn_at_ptr(unsafe { ptr.add(offset) })
                        .expect("tuple field length header");
                    offset += hdr;
                    let $var = unsafe {
                        let layout = Layout::from_size_align(
                            elem_len,
                            core::mem::align_of::<Archived<$name>>(),
                        )
                        .unwrap();
                        let (tmp_ptr, needs_dealloc) = alloc_checked(layout);
                        copy_from_payload(ptr.add(offset), tmp_ptr, elem_len)
                            .expect("tuple payload copy");
                        let tmp_slice = std::slice::from_raw_parts(tmp_ptr as *const u8, elem_len);
                        let _g = PayloadCtxGuard::enter(tmp_slice);
                        let value = $name::deserialize(&*(tmp_ptr as *const Archived<$name>));
                        dealloc_checked(tmp_ptr, layout, needs_dealloc);
                        value
                    };
                    offset += elem_len;
                )+
                let _ = offset;
                ( $( $var ),+ )
            }

            fn try_deserialize(archived: &'a Archived<( $( $name, )+ )>) -> Result<Self, Error> {
                let ptr = archived as *const _ as *const u8;
                let mut offset = 0usize;
                $(
                    let (elem_len, hdr) = read_len_dyn_at_ptr(unsafe { ptr.add(offset) })?;
                    offset += hdr;
                    let $var = unsafe {
                        let layout = Layout::from_size_align(
                            elem_len,
                            core::mem::align_of::<Archived<$name>>(),
                        ).map_err(|_| Error::LengthMismatch)?;
                        let (tmp_ptr, needs_dealloc) = alloc_checked(layout);
                        if let Err(err) = copy_from_payload(ptr.add(offset), tmp_ptr, elem_len) {
                            dealloc_checked(tmp_ptr, layout, needs_dealloc);
                            return Err(err);
                        }
                        let tmp_slice = std::slice::from_raw_parts(tmp_ptr as *const u8, elem_len);
                        let _g = PayloadCtxGuard::enter(tmp_slice);
                        let archived = &*(tmp_ptr as *const Archived<$name>);
                        let value = guarded_try_deserialize(|| $name::try_deserialize(archived))?;
                        dealloc_checked(tmp_ptr, layout, needs_dealloc);
                        value
                    };
                    offset += elem_len;
                )+
                let _ = offset;
                Ok(( $( $var ),+ ))
            }
        }
    };
}

impl_tuple!(A a 0, B b 1);
impl_tuple!(A a 0, B b 1, C c 2);
impl_tuple!(A a 0, B b 1, C c 2, D d 3);
impl_tuple!(A a 0, B b 1, C c 2, D d 3, E e 4);
impl_tuple!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5);
impl_tuple!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6);
impl_tuple!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7);
impl_tuple!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, I i 8);
impl_tuple!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, I i 8, J j 9);
impl_tuple!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, I i 8, J j 9, K k 10);
impl_tuple!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, I i 8, J j 9, K k 10, L l 11);

/// Internal byte sink with aligned growth and incremental CRC64.
///
/// ByteSink implements `Write` and is optimized for small, frequent writes by
/// using a growth policy and typed little-endian writers. It reserves an
/// initial headroom to make space for the Norito header and computes CRC64
/// incrementally over the appended payload bytes.
pub(crate) struct ByteSink {
    buf: Vec<u8>,
    headroom: usize,
    digest: crc64fast::Digest,
}

impl ByteSink {
    /// Create a new sink with capacity hint for payload and reserved headroom.
    pub(crate) fn with_headroom(payload_capacity: usize, headroom: usize) -> Self {
        // Ensure a minimal capacity to avoid early reallocations on tiny values.
        let mut buf = Vec::with_capacity(headroom + payload_capacity.max(1024));
        buf.resize(headroom, 0); // reserve header space
        Self {
            buf,
            headroom,
            digest: crc64fast::Digest::new(),
        }
    }

    /// Reuse an existing buffer while reserving headroom and payload capacity.
    pub(crate) fn with_headroom_from(
        mut buf: Vec<u8>,
        payload_capacity: usize,
        headroom: usize,
    ) -> Self {
        buf.clear();
        let min_capacity = headroom + payload_capacity.max(1024);
        if buf.capacity() < min_capacity {
            buf.reserve(min_capacity - buf.capacity());
        }
        buf.resize(headroom, 0);
        Self {
            buf,
            headroom,
            digest: crc64fast::Digest::new(),
        }
    }

    #[inline]
    pub(crate) fn ensure_capacity(&mut self, extra: usize) {
        let needed = self.buf.len() + extra;
        if self.buf.capacity() < needed {
            // Growth: round up to the next power-of-two-like step to amortize.
            let mut cap = self.buf.capacity().max(self.headroom + 1024);
            while cap < needed {
                cap = cap.saturating_mul(2);
            }
            self.buf.reserve(cap - self.buf.capacity());
        }
    }

    #[inline]
    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) {
        self.ensure_capacity(bytes.len());
        self.buf.extend_from_slice(bytes);
        self.digest.write(bytes);
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn write_u8(&mut self, v: u8) {
        self.ensure_capacity(1);
        self.buf.push(v);
        self.digest.write(&[v]);
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn write_u16_le(&mut self, v: u16) {
        let b = v.to_le_bytes();
        self.write_bytes(&b);
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn write_u32_le(&mut self, v: u32) {
        let b = v.to_le_bytes();
        self.write_bytes(&b);
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn write_u64_le(&mut self, v: u64) {
        let b = v.to_le_bytes();
        self.write_bytes(&b);
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn write_var_u64(&mut self, mut v: u64) {
        let mut buf = [0u8; 10];
        let mut idx = 0usize;
        while v >= 0x80 {
            buf[idx] = (v as u8) | 0x80;
            v >>= 7;
            idx += 1;
        }
        buf[idx] = v as u8;
        self.write_bytes(&buf[..=idx]);
    }

    /// Write a compact varint (7-bit) length when `compact-len` is enabled.
    /// Align the payload to `align` bytes by writing zero padding.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn align_to(&mut self, align: usize) {
        debug_assert!(align.is_power_of_two());
        let len = self.buf.len();
        let mis = len & (align - 1);
        if mis != 0 {
            let pad = align - mis;
            self.ensure_capacity(pad);
            let start = self.buf.len();
            self.buf.resize(start + pad, 0);
            self.digest.write(&self.buf[start..start + pad]);
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn checksum(&self) -> u64 {
        self.digest.sum64()
    }

    #[inline]
    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.buf
    }
}

impl Write for ByteSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.write_bytes(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn encode_bare_with_flags<T: NoritoSerialize>(
    value: &T,
) -> Result<(Vec<u8>, u8), Error> {
    let encode_guard = EncodeContextGuard::enter();
    let base_flags = current_decode_flags_effective().unwrap_or_else(default_encode_flags);
    let estimated = value
        .encoded_len_exact()
        .or_else(|| value.encoded_len_hint())
        .unwrap_or(0);
    let flags = base_flags;
    let mut sink = ByteSink::with_headroom(estimated, 0);
    {
        let _guard = DecodeFlagsGuard::enter(flags);
        value.serialize(&mut sink)?;
    }
    let payload = sink.into_inner();
    let fixed_offsets_used = fixed_offsets_used();
    let field_bitset_used = field_bitset_used();
    let compact_len_used = compact_len_used();
    drop(encode_guard);
    let mut final_flags = flags;
    if field_bitset_used {
        final_flags |= header_flags::FIELD_BITSET;
    } else {
        final_flags &= !header_flags::FIELD_BITSET;
    }
    if compact_len_used {
        final_flags |= header_flags::COMPACT_LEN;
    } else {
        final_flags &= !header_flags::COMPACT_LEN;
    }
    let packed_seq_used = fixed_offsets_used;
    if packed_seq_used {
        final_flags |= header_flags::PACKED_SEQ;
    } else {
        final_flags &= !header_flags::PACKED_SEQ;
    }
    record_last_header_flags(final_flags);
    Ok((payload, final_flags))
}

/// Serialize an object to a new byte vector.
///
/// The returned buffer begins with [`Header::SIZE`] bytes reserved for the
/// metadata header followed by the archived payload.
/// The header is populated after serialization so that the checksum and length
/// fields reflect the final payload.
pub fn to_bytes<T: NoritoSerialize>(value: &T) -> Result<Vec<u8>, Error> {
    let (payload, flags) = encode_bare_with_flags(value)?;
    let len = payload.len() as u64;
    let checksum = crc64(&payload);
    let mut header = Header::new(T::schema_hash(), len, checksum);
    header.flags |= flags;
    let padding = payload_alignment_padding_for::<T>();
    let mut out = Vec::with_capacity(Header::SIZE + padding + payload.len());
    header.write(&mut out)?;
    append_payload_with_padding::<T>(&mut out, &payload);
    Ok(out)
}

/// Serialize an object into the provided byte buffer.
///
/// The output buffer will be overwritten with a Norito-framed payload containing
/// the header, alignment padding, and archived bytes.
pub fn to_bytes_in<T: NoritoSerialize>(value: &T, out: &mut Vec<u8>) -> Result<(), Error> {
    let encode_guard = EncodeContextGuard::enter();
    let base_flags = current_decode_flags_effective().unwrap_or_else(default_encode_flags);
    let estimated = value
        .encoded_len_exact()
        .or_else(|| value.encoded_len_hint())
        .unwrap_or(0);
    let flags = base_flags;
    let padding = payload_alignment_padding_for::<T>();
    let headroom = Header::SIZE + padding;
    let mut sink = ByteSink::with_headroom_from(std::mem::take(out), estimated, headroom);
    {
        let _guard = DecodeFlagsGuard::enter(flags);
        value.serialize(&mut sink)?;
    }
    let payload_len = sink.buf.len().saturating_sub(headroom) as u64;
    let checksum = sink.checksum();
    let fixed_offsets_used = fixed_offsets_used();
    let field_bitset_used = field_bitset_used();
    let compact_len_used = compact_len_used();
    drop(encode_guard);

    let mut final_flags = flags;
    if field_bitset_used {
        final_flags |= header_flags::FIELD_BITSET;
    } else {
        final_flags &= !header_flags::FIELD_BITSET;
    }
    if compact_len_used {
        final_flags |= header_flags::COMPACT_LEN;
    } else {
        final_flags &= !header_flags::COMPACT_LEN;
    }
    let packed_seq_used = fixed_offsets_used;
    if packed_seq_used {
        final_flags |= header_flags::PACKED_SEQ;
    } else {
        final_flags &= !header_flags::PACKED_SEQ;
    }
    record_last_header_flags(final_flags);

    let mut header = Header::new(T::schema_hash(), payload_len, checksum);
    header.flags |= final_flags;
    {
        let mut header_slice = &mut sink.buf[..Header::SIZE];
        header.write(&mut header_slice)?;
    }
    *out = sink.into_inner();
    Ok(())
}

/// Frame a bare payload (produced by `codec::Encode::encode_to`) with a Norito header
/// using the layout flags recorded by the most recent encode/decode context.
///
/// This helper consumes the thread-local layout metadata populated by the adaptive
/// encoder and therefore only succeeds when the payload was produced (or is currently
/// being decoded) on the same thread. Callers that need to frame arbitrary bytes must
/// obtain explicit flags and use [`frame_bare_with_header_flags`].
pub fn frame_bare_with_header_flags<T: NoritoSerialize>(
    payload: &[u8],
    flags: u8,
) -> Result<Vec<u8>, Error> {
    let mut header = Header::new(T::schema_hash(), payload.len() as u64, crc64(payload));
    header.flags |= flags;
    let padding = payload_alignment_padding_for::<T>();
    let mut out = Vec::with_capacity(Header::SIZE + padding + payload.len());
    header.write(&mut out)?;
    append_payload_with_padding::<T>(&mut out, payload);
    Ok(out)
}

/// Write a bare payload with a Norito header directly to `writer`.
///
/// This is the streaming equivalent of [`frame_bare_with_header_flags`]. It is
/// useful when the framed payload is embedded immediately into a larger archive
/// and materializing a temporary `Vec<u8>` would add avoidable copy traffic.
pub fn write_bare_frame_with_header_flags<T, W>(
    writer: &mut W,
    payload: &[u8],
    flags: u8,
) -> Result<(), Error>
where
    T: NoritoSerialize,
    W: Write + ?Sized,
{
    let mut header = Header::new(T::schema_hash(), payload.len() as u64, crc64(payload));
    header.flags |= flags;
    header.write(&mut *writer)?;

    let mut padding = payload_alignment_padding_for::<T>();
    if padding != 0 {
        const ZEROS: [u8; 64] = [0; 64];
        while padding != 0 {
            let chunk = padding.min(ZEROS.len());
            writer.write_all(&ZEROS[..chunk])?;
            padding -= chunk;
        }
    }
    writer.write_all(payload)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn frame_bare_with_default_header<T: NoritoSerialize>(
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    if let Some(flags) = take_last_header_flags() {
        return frame_bare_with_header_flags::<T>(payload, flags);
    }
    if let Some(flags) = current_decode_flags_effective() {
        return frame_bare_with_header_flags::<T>(payload, flags);
    }
    Err(Error::MissingLayoutFlags)
}

/// Convenience: frame the currently-decoding bare payload (from payload context)
/// with a Norito header using the active decode flags so it can be decoded via
/// `from_bytes`.
/// Returns `Error::MissingPayloadContext` when no payload context is active and
/// `Error::MissingLayoutFlags` if the decoder did not negotiate layout flags.
pub fn frame_current_payload_with_default_header<T: NoritoSerialize>() -> Result<Vec<u8>, Error> {
    if let Some(state) = payload_ctx_state() {
        if let Some(schema) = state.schema
            && schema != T::schema_hash()
        {
            return Err(Error::SchemaMismatch);
        }
        // SAFETY: payload_ctx provides a valid slice for the active decode buffer
        let bytes = unsafe { core::slice::from_raw_parts(state.base as *const u8, state.len) };
        let header_flags = if state.flags_active {
            combine_flags(state.flags, state.flags_hint)
        } else if let Some(flags) = current_decode_flags_effective() {
            flags
        } else {
            return Err(Error::MissingLayoutFlags);
        };
        frame_bare_with_header_flags::<T>(bytes, header_flags)
    } else {
        Err(Error::MissingPayloadContext)
    }
}

#[cfg(test)]
mod bytesink_tests {
    use super::*;

    #[test]
    fn bytesink_crc_matches_direct() {
        let mut s = ByteSink::with_headroom(4, Header::SIZE);
        s.write_all(&[1, 2, 3, 4, 5]).unwrap();
        assert_eq!(s.checksum(), crc64(&[1, 2, 3, 4, 5]));
    }

    #[test]
    fn smallbuf_clear_returns_short_writes_to_stack_storage() {
        use std::io::Write as _;

        let mut buf = SmallBuf::<4>::new();
        buf.write_all(&[1, 2, 3, 4, 5]).unwrap();
        assert!(buf.spilled);
        assert_eq!(buf.as_slice(), &[1, 2, 3, 4, 5]);

        buf.clear();
        assert!(!buf.spilled);
        assert!(buf.is_empty());

        buf.write_all(&[9, 8]).unwrap();
        assert!(!buf.spilled);
        assert_eq!(buf.as_slice(), &[9, 8]);

        buf.write_all(&[7, 6, 5]).unwrap();
        assert!(buf.spilled);
        assert_eq!(buf.as_slice(), &[9, 8, 7, 6, 5]);
    }

    #[test]
    fn bytesink_typed_writes_le() {
        let mut s = ByteSink::with_headroom(0, 0);
        s.write_u16_le(0x1234);
        s.write_u32_le(0x89ABCDEF);
        s.write_u64_le(0x0123456789ABCDEF);
        let b = s.into_inner();
        assert_eq!(b.len(), 2 + 4 + 8);
        assert_eq!(&b[0..2], &0x1234u16.to_le_bytes());
        assert_eq!(&b[2..6], &0x89ABCDEFu32.to_le_bytes());
        assert_eq!(&b[6..14], &0x0123456789ABCDEFu64.to_le_bytes());
    }

    #[test]
    fn bytesink_align_to() {
        let mut s = ByteSink::with_headroom(0, 0);
        s.write_u8(0xAA);
        s.align_to(8);
        let before = s.buf.len();
        // Next write starts at 8-byte boundary
        s.write_u8(0xBB);
        let b = s.into_inner();
        assert_eq!(before, 8);
        assert_eq!(b[0], 0xAA);
        assert!(b[1..8].iter().all(|&x| x == 0));
        assert_eq!(b[8], 0xBB);
    }
}

/// Configuration for compression.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompressionConfig {
    /// Compression level for zstd.
    pub level: i32,
}

// Lightweight telemetry for compression decisions and cost.
// Always counts calls and bytes; timing is gated behind `adaptive-telemetry`.
mod telemetry_compress {
    use std::sync::atomic::{AtomicU64, Ordering};

    static CALLS: AtomicU64 = AtomicU64::new(0);
    static NONE_SELECTED: AtomicU64 = AtomicU64::new(0);
    static ZSTD_SELECTED: AtomicU64 = AtomicU64::new(0);
    static BYTES_IN_TOTAL: AtomicU64 = AtomicU64::new(0);
    static BYTES_OUT_TOTAL: AtomicU64 = AtomicU64::new(0);
    #[cfg(feature = "adaptive-telemetry")]
    static COMPRESS_TIME_NS_TOTAL: AtomicU64 = AtomicU64::new(0);

    #[derive(Clone, Copy, Debug)]
    pub struct Snapshot {
        pub calls: u64,
        pub none_selected: u64,
        pub zstd_selected: u64,
        pub bytes_in_total: u64,
        pub bytes_out_total: u64,
        #[cfg(feature = "adaptive-telemetry")]
        pub compress_time_ns_total: u64,
    }

    #[inline]
    pub fn record(is_zstd: bool, bytes_in: usize, bytes_out: usize, _time_ns: u64) {
        CALLS.fetch_add(1, Ordering::Relaxed);
        if is_zstd {
            ZSTD_SELECTED.fetch_add(1, Ordering::Relaxed);
        } else {
            NONE_SELECTED.fetch_add(1, Ordering::Relaxed);
        }
        BYTES_IN_TOTAL.fetch_add(bytes_in as u64, Ordering::Relaxed);
        BYTES_OUT_TOTAL.fetch_add(bytes_out as u64, Ordering::Relaxed);
        #[cfg(feature = "adaptive-telemetry")]
        {
            COMPRESS_TIME_NS_TOTAL.fetch_add(_time_ns, Ordering::Relaxed);
        }
    }

    pub fn snapshot() -> Snapshot {
        Snapshot {
            calls: CALLS.load(Ordering::Relaxed),
            none_selected: NONE_SELECTED.load(Ordering::Relaxed),
            zstd_selected: ZSTD_SELECTED.load(Ordering::Relaxed),
            bytes_in_total: BYTES_IN_TOTAL.load(Ordering::Relaxed),
            bytes_out_total: BYTES_OUT_TOTAL.load(Ordering::Relaxed),
            #[cfg(feature = "adaptive-telemetry")]
            compress_time_ns_total: COMPRESS_TIME_NS_TOTAL.load(Ordering::Relaxed),
        }
    }

    pub fn reset() {
        CALLS.store(0, Ordering::Relaxed);
        NONE_SELECTED.store(0, Ordering::Relaxed);
        ZSTD_SELECTED.store(0, Ordering::Relaxed);
        BYTES_IN_TOTAL.store(0, Ordering::Relaxed);
        BYTES_OUT_TOTAL.store(0, Ordering::Relaxed);
        #[cfg(feature = "adaptive-telemetry")]
        {
            COMPRESS_TIME_NS_TOTAL.store(0, Ordering::Relaxed);
        }
    }

    pub(crate) use Snapshot as CompressSnapshot;
}

/// Return a snapshot of compression telemetry metrics.
pub fn compression_metrics_snapshot() -> telemetry_compress::CompressSnapshot {
    telemetry_compress::snapshot()
}

/// Reset compression telemetry metrics.
#[allow(dead_code)]
pub fn compression_metrics_reset() {
    telemetry_compress::reset()
}

/// JSON: export compression telemetry metrics as a compact JSON value.
#[cfg(feature = "json")]
pub fn compression_metrics_json_value() -> crate::json::Value {
    let s = compression_metrics_snapshot();
    let mut map = crate::json::Map::new();
    map.insert("calls".into(), crate::json::Value::from(s.calls));
    map.insert(
        "none_selected".into(),
        crate::json::Value::from(s.none_selected),
    );
    map.insert(
        "zstd_selected".into(),
        crate::json::Value::from(s.zstd_selected),
    );
    map.insert(
        "bytes_in_total".into(),
        crate::json::Value::from(s.bytes_in_total),
    );
    map.insert(
        "bytes_out_total".into(),
        crate::json::Value::from(s.bytes_out_total),
    );
    #[cfg(feature = "adaptive-telemetry")]
    {
        map.insert(
            "compress_time_ns_total".into(),
            crate::json::Value::from(s.compress_time_ns_total),
        );
    }
    crate::json::Value::Object(map)
}

/// JSON: export compression telemetry metrics as a compact JSON string.
#[cfg(feature = "json")]
pub fn compression_metrics_json_string() -> String {
    let v = compression_metrics_json_value();
    crate::json::to_string(&v).unwrap_or_else(|_| String::from("{}"))
}

/// JSON: compute fieldwise delta between two compression telemetry JSON maps.
#[cfg(feature = "json")]
pub fn compression_metrics_delta_json(
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
        "none_selected",
        "zstd_selected",
        "bytes_in_total",
        "bytes_out_total",
        #[cfg(feature = "adaptive-telemetry")]
        "compress_time_ns_total",
    ] {
        if let (Some(Value::Number(a)), Some(Value::Number(b))) = (p.get(k), c.get(k)) {
            let av = a.as_u64().unwrap_or(0);
            let bv = b.as_u64().unwrap_or(0);
            out.insert(k.to_string(), Value::from(bv.saturating_sub(av)));
        }
    }
    Value::Object(out)
}

/// Serialize an object and adaptively choose compression based on payload size
/// and hardware availability.
///
/// This API preserves determinism and on‑wire format stability: it only picks
/// whether to apply zstd compression (and GPU offload when compiled and
/// available) based on heuristics. The header encodes the chosen compression and
/// layout flags exactly as in [`to_compressed_bytes`].
pub fn to_bytes_auto<T: NoritoSerialize>(value: &T) -> Result<Vec<u8>, Error> {
    let (payload, flags) = encode_bare_with_flags(value)?;
    let checksum = crc64(&payload);
    let len = payload.len() as u64;

    // Choose compression engine using heuristics module
    #[cfg(feature = "adaptive-telemetry")]
    let __t0 = std::time::Instant::now();
    let (algorithm, body) = match heuristics::compress_auto(payload) {
        Ok((Compression::None, p)) => (Compression::None, p),
        Ok((Compression::Zstd, p)) => (Compression::Zstd, p),
        Err(e) => return Err(e.into()),
    };
    #[cfg(feature = "adaptive-telemetry")]
    let __ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    #[cfg(not(feature = "adaptive-telemetry"))]
    let __ns: u64 = 0;
    telemetry_compress::record(
        matches!(algorithm, Compression::Zstd),
        len as usize,
        body.len(),
        __ns,
    );

    let padding = if matches!(algorithm, Compression::None) {
        payload_alignment_padding_for::<T>()
    } else {
        0
    };
    let mut out = Vec::with_capacity(Header::SIZE + padding + body.len());
    let mut header = Header::new(T::schema_hash(), len, checksum);
    header.compression = algorithm;
    header.flags |= flags;
    header.write(&mut out)?;
    if padding != 0 {
        let len = out.len();
        out.resize(len + padding, 0);
    }
    out.extend_from_slice(&body);
    Ok(out)
}

/// Serialize an object using optional compression.
///
/// When the `gpu-compression` feature is enabled, the zstd encoder is executed
/// on the GPU (CUDA on most platforms, Metal on Apple silicon). Otherwise, the
/// CPU implementation is used.
pub fn to_compressed_bytes<T: NoritoSerialize>(
    value: &T,
    compression: Option<CompressionConfig>,
) -> Result<Vec<u8>, Error> {
    let (payload, flags) = encode_bare_with_flags(value)?;
    let checksum = crc64(&payload);
    let len = payload.len() as u64;
    #[cfg(feature = "adaptive-telemetry")]
    let __t0 = std::time::Instant::now();
    let (algorithm, body) = match compression {
        None => (Compression::None, payload),
        Some(cfg) => {
            #[cfg(not(feature = "compression"))]
            {
                let _ = cfg;
                return Err(std::io::Error::other("compression support disabled").into());
            }

            #[cfg(all(feature = "compression", feature = "gpu-compression"))]
            {
                (Compression::Zstd, gpu_zstd::encode_all(payload, cfg.level)?)
            }
            #[cfg(all(feature = "compression", not(feature = "gpu-compression")))]
            {
                (
                    Compression::Zstd,
                    zstd::encode_all(std::io::Cursor::new(payload), cfg.level)?,
                )
            }
        }
    };
    #[cfg(feature = "adaptive-telemetry")]
    let __ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    #[cfg(not(feature = "adaptive-telemetry"))]
    let __ns: u64 = 0;
    telemetry_compress::record(
        matches!(algorithm, Compression::Zstd),
        len as usize,
        body.len(),
        __ns,
    );
    let padding = if matches!(algorithm, Compression::None) {
        payload_alignment_padding_for::<T>()
    } else {
        0
    };
    let mut out = Vec::with_capacity(Header::SIZE + padding + body.len());
    let mut header = Header::new(T::schema_hash(), len, checksum);
    header.compression = algorithm;
    header.flags |= flags;
    header.write(&mut out)?;
    if padding != 0 {
        let len = out.len();
        out.resize(len + padding, 0);
    }
    out.extend_from_slice(&body);
    Ok(out)
}

fn validate_zstd_stream(compressed: &[u8]) -> Result<(), Error> {
    if compressed.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let frame_len = zstd::zstd_safe::find_frame_compressed_size(compressed)
        .map_err(|_| Error::LengthMismatch)?;
    if frame_len != compressed.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(())
}

/// Decompress bytes produced by [`to_compressed_bytes`].
///
/// If the `gpu-compression` feature is enabled, decompression runs on the GPU
/// (CUDA or Metal depending on the target). Otherwise the CPU path is used.
///
/// Returns an owning archive wrapper so the payload bytes remain valid for the
/// lifetime of the returned value.
pub fn from_compressed_bytes<T: for<'de> NoritoDeserialize<'de>>(
    bytes: &[u8],
) -> Result<ArchivedBox<T>, Error> {
    let mut cursor = std::io::Cursor::new(bytes);
    let header = Header::read(&mut cursor)?;
    prepare_header_decode(header.flags, header.minor, true)?;
    if header.schema != T::schema_hash() {
        return Err(Error::SchemaMismatch);
    }
    let payload_len = payload_len_to_usize(header.length)?;
    let compressed = &bytes[Header::SIZE..];
    let payload = match header.compression {
        Compression::None => {
            let padding = payload_alignment_padding_for::<T>();
            let trimmed = payload_without_leading_padding_exact(compressed, payload_len, padding)?;
            trimmed.to_vec()
        }
        Compression::Zstd => {
            validate_zstd_stream(compressed)?;
            #[cfg(all(feature = "compression", feature = "gpu-compression"))]
            {
                gpu_zstd::decode_all(compressed, header.length)?
            }
            #[cfg(all(feature = "compression", not(feature = "gpu-compression")))]
            {
                let decoder = zstd::Decoder::new(compressed)?;
                // Bound output to the declared payload length (+1 to detect overflow).
                let mut out = Vec::with_capacity(payload_len);
                let max_len = payload_len.saturating_add(1);
                decoder.take(max_len as u64).read_to_end(&mut out)?;
                out
            }
            #[cfg(not(feature = "compression"))]
            {
                return Err(std::io::Error::other("compression support disabled").into());
            }
        }
    };
    if payload.len() != payload_len {
        return Err(Error::LengthMismatch);
    }
    if crc64(&payload) != header.checksum {
        return Err(Error::ChecksumMismatch);
    }
    let archived = ArchivedBox::from_payload(payload);
    // Provide payload ctx so pointer-based decoders can read lengths/offsets.
    set_payload_ctx_state(
        archived.bytes(),
        Some(header.schema),
        Some(header.flags),
        Some(header.minor),
    );
    Ok(archived)
}

/// Obtain a reference to an archived value from bytes.
///
/// The function validates the header, verifies the checksum and schema hash and
/// then returns a reference to the archived payload without any allocation.
pub fn from_bytes<'a, T: NoritoDeserialize<'a>>(bytes: &'a [u8]) -> Result<&'a Archived<T>, Error> {
    let mut cursor = std::io::Cursor::new(bytes);
    let header = Header::read(&mut cursor)?;
    prepare_header_decode(header.flags, header.minor, true)?;
    if header.compression != Compression::None {
        return Err(Error::unsupported_compression_with(
            header.compression as u8,
            &[Compression::None],
        ));
    }
    if header.schema != T::schema_hash() {
        return Err(Error::SchemaMismatch);
    }
    let pos = cursor.position() as usize;
    let slice = &bytes[pos..];
    let payload_len = payload_len_to_usize(header.length)?;
    let padding = payload_alignment_padding_for::<T>();
    let payload = payload_without_leading_padding_exact(slice, payload_len, padding)?;
    if crc64(payload) != header.checksum {
        return Err(Error::ChecksumMismatch);
    }
    let align = std::mem::align_of::<Archived<T>>();
    if align > 1 && (payload.as_ptr() as usize & (align - 1)) != 0 {
        return Err(Error::misaligned(align, payload.as_ptr()));
    }
    // Provide payload ctx for any subsequent decoders that rely on it.
    set_payload_ctx_state(
        payload,
        Some(header.schema),
        Some(header.flags),
        Some(header.minor),
    );
    // SAFETY: layout of Archived<T> is validated by construction in `to_bytes`.
    let ptr = payload.as_ptr() as *const Archived<T>;
    Ok(unsafe { &*ptr })
}

/// A validated view over an archived payload.
///
/// This contains a reference to the payload bytes (after the header) and carries
/// the decode flags set from the header.
#[derive(Clone, Copy)]
pub struct ArchiveView<'a> {
    bytes: &'a [u8],
    padding_len: usize,
    flags: u8,
    flags_hint: u8,
    schema: [u8; 16],
}

impl<'a> ArchiveView<'a> {
    /// Payload bytes (already validated by checksum and header length).
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Expose header layout flags.
    pub fn flags(&self) -> u8 {
        self.flags
    }

    /// Expose header minor flags (hint).
    pub fn flags_hint(&self) -> u8 {
        self.flags_hint
    }

    /// Expose the schema hash stored in the header.
    pub fn schema(&self) -> [u8; 16] {
        self.schema
    }

    fn decode_inner<T: DecodeFromSlice<'a>>(&self) -> Result<T, Error> {
        let _ctx = PayloadCtxGuard::enter_with_flags_hint(self.bytes, self.flags, self.flags_hint);
        let effective = combine_flags(self.flags, self.flags_hint);
        let _flags = DecodeFlagsGuard::enter_with_hint(self.flags, effective);
        let (value, used) = <T as DecodeFromSlice>::decode_from_slice(self.bytes)?;
        if used != self.bytes.len() {
            // Some payloads carry trailing zero alignment bytes after the logical
            // decode boundary. Accept those, while still rejecting non-zero tails.
            let tail = self.bytes.get(used..).ok_or(Error::LengthMismatch)?;
            if tail.iter().any(|byte| *byte != 0) {
                return Err(Error::LengthMismatch);
            }
        }
        Ok(value)
    }

    /// Decode a value from the payload using the strict-safe slice-based path,
    /// enforcing the header schema hash.
    pub fn decode<T>(&self) -> Result<T, Error>
    where
        T: DecodeFromSlice<'a> + NoritoDeserialize<'a>,
    {
        if self.schema != T::schema_hash() {
            return Err(Error::SchemaMismatch);
        }
        if self.padding_len != payload_alignment_padding_for::<T>() {
            return Err(Error::LengthMismatch);
        }
        self.decode_inner()
    }

    /// Decode a value from the payload without enforcing the schema hash.
    pub fn decode_unchecked<T: DecodeFromSlice<'a>>(&self) -> Result<T, Error> {
        self.decode_inner()
    }
}

/// Validate bytes and return an archive view over the payload slice.
pub fn from_bytes_view<'a>(bytes: &'a [u8]) -> Result<ArchiveView<'a>, Error> {
    let mut cursor = std::io::Cursor::new(bytes);
    let header = Header::read(&mut cursor)?;
    prepare_header_decode(header.flags, header.minor, true)?;
    if header.compression != Compression::None {
        return Err(Error::unsupported_compression_with(
            header.compression as u8,
            &[Compression::None],
        ));
    }
    let pos = cursor.position() as usize;
    let slice = &bytes[pos..];
    let payload_len = payload_len_to_usize(header.length)?;
    let payload = payload_without_leading_padding(slice, payload_len, MAX_HEADER_PADDING)?;
    let padding_len = slice.len() - payload.len();
    if crc64(payload) != header.checksum {
        return Err(Error::ChecksumMismatch);
    }
    set_payload_ctx_state(
        payload,
        Some(header.schema),
        Some(header.flags),
        Some(header.minor),
    );
    Ok(ArchiveView {
        bytes: payload,
        padding_len,
        flags: header.flags,
        flags_hint: header.minor,
        schema: header.schema,
    })
}

/// Convenience: decode a value of `T` directly from bytes via archive view.
pub fn decode_from_bytes<'a, T>(bytes: &'a [u8]) -> Result<T, Error>
where
    T: crate::NoritoDeserialize<'a> + DecodeFromSlice<'a> + 'a,
{
    let view = from_bytes_view(bytes)?;
    if view.schema() != T::schema_hash() {
        return Err(Error::SchemaMismatch);
    }
    if view.padding_len != payload_alignment_padding_for::<T>() {
        return Err(Error::LengthMismatch);
    }
    let payload_src = view.as_bytes();
    let flags = view.flags;
    let flags_hint = view.flags_hint;
    let align = core::mem::align_of::<Archived<T>>();
    let header_len = core::mem::size_of::<Archived<T>>();
    let ptr_us = payload_src.as_ptr() as usize;
    let needs_realign = align > 1 && !ptr_us.is_multiple_of(align);
    let needs_slice = needs_realign || (header_len > 0 && payload_src.len() < header_len);

    if needs_slice {
        return crate::guarded_try_deserialize(|| {
            let _guard = PayloadCtxGuard::enter_with_flags_hint(payload_src, flags, flags_hint);
            let (value, _used) = <T as DecodeFromSlice>::decode_from_slice(payload_src)?;
            Ok(value)
        });
    }

    let archived_ptr: *const Archived<T> = if header_len == 0 {
        core::ptr::NonNull::<Archived<T>>::dangling().as_ptr()
    } else {
        payload_src.as_ptr() as *const Archived<T>
    };

    crate::guarded_try_deserialize(|| {
        let _guard = PayloadCtxGuard::enter_with_flags_hint(payload_src, flags, flags_hint);
        let archived = unsafe { &*archived_ptr };
        T::try_deserialize(archived)
    })
}

/// Decode a field payload using the canonical codec implementation without
/// requiring a specialized `DecodeFromSlice` implementation.
///
/// Returns both the decoded value and the number of bytes that were consumed
/// from `bytes`.
pub fn decode_field_canonical<T>(bytes: &[u8]) -> Result<(T, usize), Error>
where
    T: for<'de> crate::NoritoDeserialize<'de> + crate::NoritoSerialize,
{
    if bytes.is_empty() {
        if core::mem::size_of::<Archived<T>>() == 0 {
            let _guard = PayloadCtxGuard::enter(&[]);
            let value = unsafe {
                crate::guarded_try_deserialize(|| {
                    T::try_deserialize(&*std::ptr::NonNull::<Archived<T>>::dangling().as_ptr())
                })?
            };
            return Ok((value, 0));
        }
        return Err(Error::LengthMismatch);
    }

    struct RootGuard(bool);
    impl Drop for RootGuard {
        fn drop(&mut self) {
            if self.0 {
                clear_decode_root();
            }
        }
    }
    let _root_guard = if payload_root_span().is_none() {
        set_decode_root(bytes);
        Some(RootGuard(true))
    } else {
        None
    };

    let bytes_len = bytes.len();
    let _flags_guard = if decode_flags_active() {
        None
    } else {
        Some(DecodeFlagsGuard::enter(default_encode_flags()))
    };

    let align = core::mem::align_of::<Archived<T>>();
    let ptr = bytes.as_ptr();
    let ptr_us = ptr as usize;

    if align <= 1 || ptr_us.is_multiple_of(align) {
        if crate::debug_trace_enabled() {
            eprintln!(
                "decode_field_canonical::<{}> aligned path len={}",
                core::any::type_name::<T>(),
                bytes_len
            );
        }
        let payload_guard = PayloadCtxGuard::enter(bytes);
        let archived = unsafe { &*(ptr as *const Archived<T>) };
        let value = crate::guarded_try_deserialize(|| T::try_deserialize(archived))?;
        let used_ctx = payload_ctx_max_access();
        drop(payload_guard);
        let used = match used_ctx {
            Some(used) if used != 0 => {
                if used == bytes_len {
                    Ok(used)
                } else if used > bytes_len {
                    if crate::debug_trace_enabled() {
                        eprintln!(
                            "decode_field_canonical::<{}>: max_access={} exceeds payload_len={}",
                            core::any::type_name::<T>(),
                            used,
                            bytes_len
                        );
                    }
                    Err(Error::LengthMismatch)
                } else {
                    let recomputed = recompute_canonical_len(&value)?;
                    if recomputed != bytes_len {
                        if crate::debug_trace_enabled() {
                            eprintln!(
                                "decode_field_canonical::<{}>: recomputed_len={} mismatches payload_len={} (max_access={used})",
                                core::any::type_name::<T>(),
                                recomputed,
                                bytes_len
                            );
                        }
                        Err(Error::LengthMismatch)
                    } else {
                        Ok(recomputed)
                    }
                }
            }
            _ => {
                let recomputed = recompute_canonical_len(&value)?;
                if recomputed != bytes_len {
                    if crate::debug_trace_enabled() {
                        eprintln!(
                            "decode_field_canonical::<{}>: recomputed_len={} mismatches payload_len={}",
                            core::any::type_name::<T>(),
                            recomputed,
                            bytes_len
                        );
                    }
                    Err(Error::LengthMismatch)
                } else {
                    Ok(recomputed)
                }
            }
        }?;
        // Propagate canonical full-consumption to any parent payload context. Some deserializers
        // (especially scalar-only) may not record payload accesses, which would otherwise cause
        // parent decoders to fall back to expensive canonical-length recomputation.
        note_payload_access(bytes, used);
        return Ok((value, used));
    }

    struct TmpAlloc {
        ptr: *mut u8,
        layout: Layout,
        len: usize,
        needs_dealloc: bool,
    }

    impl Drop for TmpAlloc {
        fn drop(&mut self) {
            unsafe { dealloc_checked(self.ptr, self.layout, self.needs_dealloc) };
        }
    }

    let layout =
        Layout::from_size_align(bytes_len.max(1), align).map_err(|_| Error::LengthMismatch)?;
    if crate::debug_trace_enabled() {
        eprintln!(
            "decode_field_canonical::<{}> misaligned path len={} align={} layout_size={}",
            core::any::type_name::<T>(),
            bytes_len,
            align,
            layout.size()
        );
    }
    let (tmp_ptr, needs_dealloc) = unsafe { alloc_checked(layout) };
    let alloc = TmpAlloc {
        ptr: tmp_ptr,
        layout,
        len: bytes_len,
        needs_dealloc,
    };
    let (value, used) = unsafe {
        core::ptr::copy(bytes.as_ptr(), alloc.ptr, bytes_len);
        let tmp_slice = std::slice::from_raw_parts(alloc.ptr as *const u8, alloc.len);
        let payload_guard = PayloadCtxGuard::enter(tmp_slice);
        let archived = &*(alloc.ptr as *const Archived<T>);
        let value = crate::guarded_try_deserialize(|| T::try_deserialize(archived))?;
        let used_ctx = payload_ctx_max_access();
        drop(payload_guard);
        let used = match used_ctx {
            Some(used) if used != 0 => {
                if used == bytes_len {
                    Ok(used)
                } else if used > bytes_len {
                    if crate::debug_trace_enabled() {
                        eprintln!(
                            "decode_field_canonical::<{}>: misaligned copy max_access={} exceeds payload_len={}",
                            core::any::type_name::<T>(),
                            used,
                            bytes_len
                        );
                    }
                    Err(Error::LengthMismatch)
                } else {
                    let recomputed = recompute_canonical_len(&value)?;
                    if recomputed != bytes_len {
                        if crate::debug_trace_enabled() {
                            eprintln!(
                                "decode_field_canonical::<{}>: misaligned recomputed_len={} mismatches payload_len={}",
                                core::any::type_name::<T>(),
                                recomputed,
                                bytes_len
                            );
                        }
                        Err(Error::LengthMismatch)
                    } else {
                        Ok(recomputed)
                    }
                }
            }
            _ => {
                let recomputed = recompute_canonical_len(&value)?;
                if recomputed != bytes_len {
                    if crate::debug_trace_enabled() {
                        eprintln!(
                            "decode_field_canonical::<{}>: misaligned recomputed_len={} mismatches payload_len={}",
                            core::any::type_name::<T>(),
                            recomputed,
                            bytes_len
                        );
                    }
                    Err(Error::LengthMismatch)
                } else {
                    Ok(recomputed)
                }
            }
        }?;
        Result::<(T, usize), Error>::Ok((value, used))
    }?;
    // In the misaligned path, we decode from a temporary aligned copy, so the payload context
    // cannot merge accesses into any parent decoder. Record canonical consumption in the restored
    // parent payload context explicitly.
    note_payload_access(bytes, used);
    Ok((value, used))
}

/// Decode a single field from the front of `bytes`, permitting trailing bytes after the field.
///
/// This is intended for packed layouts where a self-delimiting field is followed by additional
/// packed fields in the same payload. The returned `usize` is the canonical byte length consumed
/// by the decoded field.
pub fn decode_field_prefix<T>(bytes: &[u8]) -> Result<(T, usize), Error>
where
    T: for<'de> crate::NoritoDeserialize<'de> + crate::NoritoSerialize,
{
    #[inline]
    fn resolve_prefix_used<T>(
        value: &T,
        payload_len: usize,
        used_ctx: Option<usize>,
    ) -> Result<usize, Error>
    where
        T: crate::NoritoSerialize,
    {
        match used_ctx {
            Some(used) if used != 0 => {
                if used > payload_len {
                    return Err(Error::LengthMismatch);
                }
                let recomputed = recompute_canonical_len(value)?;
                if recomputed > payload_len || used > recomputed {
                    return Err(Error::LengthMismatch);
                }
                Ok(recomputed)
            }
            _ => {
                let recomputed = recompute_canonical_len(value)?;
                if recomputed > payload_len {
                    return Err(Error::LengthMismatch);
                }
                Ok(recomputed)
            }
        }
    }

    if bytes.is_empty() {
        if core::mem::size_of::<Archived<T>>() == 0 {
            let _guard = PayloadCtxGuard::enter(&[]);
            let value = unsafe {
                crate::guarded_try_deserialize(|| {
                    T::try_deserialize(&*std::ptr::NonNull::<Archived<T>>::dangling().as_ptr())
                })?
            };
            return Ok((value, 0));
        }
        return Err(Error::LengthMismatch);
    }

    struct RootGuard(bool);
    impl Drop for RootGuard {
        fn drop(&mut self) {
            if self.0 {
                clear_decode_root();
            }
        }
    }
    let _root_guard = if payload_root_span().is_none() {
        set_decode_root(bytes);
        Some(RootGuard(true))
    } else {
        None
    };

    let bytes_len = bytes.len();
    let _flags_guard = if decode_flags_active() {
        None
    } else {
        Some(DecodeFlagsGuard::enter(default_encode_flags()))
    };

    let align = core::mem::align_of::<Archived<T>>();
    let ptr = bytes.as_ptr();
    let ptr_us = ptr as usize;

    if align <= 1 || ptr_us.is_multiple_of(align) {
        let payload_guard = PayloadCtxGuard::enter(bytes);
        let archived = unsafe { &*(ptr as *const Archived<T>) };
        let value = crate::guarded_try_deserialize(|| T::try_deserialize(archived))?;
        let used_ctx = payload_ctx_max_access();
        drop(payload_guard);
        let used = resolve_prefix_used(&value, bytes_len, used_ctx)?;
        note_payload_access(bytes, used);
        return Ok((value, used));
    }

    struct TmpAlloc {
        ptr: *mut u8,
        layout: Layout,
        len: usize,
        needs_dealloc: bool,
    }

    impl Drop for TmpAlloc {
        fn drop(&mut self) {
            unsafe { dealloc_checked(self.ptr, self.layout, self.needs_dealloc) };
        }
    }

    let layout =
        Layout::from_size_align(bytes_len.max(1), align).map_err(|_| Error::LengthMismatch)?;
    let (tmp_ptr, needs_dealloc) = unsafe { alloc_checked(layout) };
    let alloc = TmpAlloc {
        ptr: tmp_ptr,
        layout,
        len: bytes_len,
        needs_dealloc,
    };
    let (value, used) = unsafe {
        core::ptr::copy(bytes.as_ptr(), alloc.ptr, bytes_len);
        let tmp_slice = std::slice::from_raw_parts(alloc.ptr as *const u8, alloc.len);
        let payload_guard = PayloadCtxGuard::enter(tmp_slice);
        let archived = &*(alloc.ptr as *const Archived<T>);
        let value = crate::guarded_try_deserialize(|| T::try_deserialize(archived))?;
        let used_ctx = payload_ctx_max_access();
        drop(payload_guard);
        let used = resolve_prefix_used(&value, bytes_len, used_ctx)?;
        Result::<(T, usize), Error>::Ok((value, used))
    }?;
    note_payload_access(bytes, used);
    Ok((value, used))
}

/// Decode a field using its `DecodeFromSlice` implementation, ensuring full
/// consumption without re-encoding for canonical length checks.
///
/// This is intended for hot paths where re-serializing is too costly and the
/// slice-based decoder already guarantees canonical consumption.
fn decode_field_with_slice_decoder<T>(
    bytes: &[u8],
    require_full_consumption: bool,
) -> Result<(T, usize), Error>
where
    T: for<'de> crate::NoritoDeserialize<'de> + for<'de> DecodeFromSlice<'de>,
{
    if bytes.is_empty() {
        if core::mem::size_of::<Archived<T>>() == 0 {
            let _guard = PayloadCtxGuard::enter(&[]);
            let value = unsafe {
                crate::guarded_try_deserialize(|| {
                    T::try_deserialize(&*std::ptr::NonNull::<Archived<T>>::dangling().as_ptr())
                })?
            };
            return Ok((value, 0));
        }
        return Err(Error::LengthMismatch);
    }

    struct RootGuard(bool);
    impl Drop for RootGuard {
        fn drop(&mut self) {
            if self.0 {
                clear_decode_root();
            }
        }
    }
    let _root_guard = if payload_root_span().is_none() {
        set_decode_root(bytes);
        Some(RootGuard(true))
    } else {
        None
    };

    let _flags_guard = if decode_flags_active() {
        None
    } else {
        Some(DecodeFlagsGuard::enter(default_encode_flags()))
    };

    let payload_guard = PayloadCtxGuard::enter(bytes);
    let (value, used) = <T as DecodeFromSlice>::decode_from_slice(bytes)?;
    if require_full_consumption && used != bytes.len() {
        return Err(Error::LengthMismatch);
    }
    note_payload_access(bytes, used);
    drop(payload_guard);
    Ok((value, used))
}

pub fn decode_field_canonical_slice<T>(bytes: &[u8]) -> Result<(T, usize), Error>
where
    T: for<'de> crate::NoritoDeserialize<'de> + for<'de> DecodeFromSlice<'de>,
{
    decode_field_with_slice_decoder(bytes, true)
}

/// Decode a field using its `DecodeFromSlice` implementation, ensuring full
/// consumption without re-encoding for canonical length checks.
///
/// This is intended for hot paths where re-serializing is too costly and the
/// slice-based decoder already guarantees canonical consumption.
pub fn decode_field_canonical_from_slice<T>(bytes: &[u8]) -> Result<(T, usize), Error>
where
    T: for<'de> crate::NoritoDeserialize<'de> + for<'de> DecodeFromSlice<'de>,
{
    decode_field_canonical_slice(bytes)
}

fn recompute_canonical_len<T>(value: &T) -> Result<usize, Error>
where
    T: crate::NoritoSerialize,
{
    struct LenWriter {
        len: usize,
    }

    impl std::io::Write for LenWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.len = self
                .len
                .checked_add(buf.len())
                .ok_or_else(|| std::io::Error::other("norito length overflow"))?;
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
            let mut total = 0usize;
            for b in bufs {
                total = total
                    .checked_add(b.len())
                    .ok_or_else(|| std::io::Error::other("norito length overflow"))?;
            }
            self.len = self
                .len
                .checked_add(total)
                .ok_or_else(|| std::io::Error::other("norito length overflow"))?;
            Ok(total)
        }
    }

    let mut sink = LenWriter { len: 0 };
    value.serialize(&mut sink)?;
    Ok(sink.len)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use crc64fast::Digest;

    use super::*;
    use crate::{
        NoritoDeserialize, NoritoSerialize, codec,
        codec::{encode_adaptive, encode_with_header_flags},
    };

    #[test]
    fn crc64_matches_digest() {
        let data = b"123456789";
        let mut digest = Digest::new();
        digest.write(data);
        assert_eq!(crc64(data), digest.sum64());
    }

    #[test]
    fn copy_from_payload_allows_zero_len() {
        let mut out = 0u8;
        let ptr = core::ptr::NonNull::<u8>::dangling().as_ptr();
        let res = unsafe { copy_from_payload(ptr, &mut out as *mut u8, 0) };
        assert!(res.is_ok());
    }

    #[cfg(feature = "compression")]
    #[test]
    fn payload_stream_reads_zstd_payload() {
        use std::io::{Cursor, Read};

        let payload = b"payload stream zstd check".to_vec();
        let compressed =
            zstd::encode_all(Cursor::new(payload.clone()), 0).expect("compress payload");
        let cursor = Cursor::new(compressed);
        let mut stream =
            stream::PayloadStream::new(cursor, Compression::Zstd).expect("create zstd stream");
        let mut decoded = Vec::new();
        Read::read_to_end(&mut stream, &mut decoded).expect("read zstd payload");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_field_canonical_reports_scalar_consumed() {
        reset_decode_state();
        let mut buf = Vec::new();
        0xDEADBEEFu32.serialize(&mut buf).unwrap();
        let (value, used) = decode_field_canonical::<u32>(&buf).expect("scalar decode");
        assert_eq!(value, 0xDEADBEEF);
        assert_eq!(used, buf.len());
    }

    #[test]
    fn decode_field_canonical_propagates_access_to_parent_ctx() {
        reset_decode_state();
        let mut buf = Vec::new();
        0xAABBCCDDu32.serialize(&mut buf).unwrap();

        let _outer = PayloadCtxGuard::enter(&buf);
        let (value, used) = decode_field_canonical::<u32>(&buf).expect("scalar decode");
        assert_eq!(value, 0xAABBCCDD);
        assert_eq!(used, buf.len());
        assert_eq!(
            payload_ctx_max_access().unwrap(),
            buf.len(),
            "outer payload ctx must observe canonical consumption"
        );
    }

    #[test]
    fn decode_field_canonical_propagates_access_from_misaligned_copy() {
        reset_decode_state();
        let mut buf = Vec::new();
        0xDEADBEEFu32.serialize(&mut buf).unwrap();

        let mut storage = Vec::with_capacity(buf.len() + 1);
        storage.push(0xAA);
        storage.extend_from_slice(&buf);
        let misaligned = &storage[1..];
        assert_ne!(
            misaligned.as_ptr() as usize % core::mem::align_of::<Archived<u32>>(),
            0,
            "expected misaligned scalar payload"
        );

        let _outer = PayloadCtxGuard::enter(misaligned);
        let (value, used) =
            decode_field_canonical::<u32>(misaligned).expect("decode misaligned scalar");
        assert_eq!(value, 0xDEADBEEF);
        assert_eq!(used, misaligned.len());
        assert_eq!(
            payload_ctx_max_access().unwrap(),
            misaligned.len(),
            "outer payload ctx must observe canonical consumption from misaligned decode"
        );
    }

    static PANIC_ON_SERIALIZE: AtomicBool = AtomicBool::new(false);

    #[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
    struct CanonicalStruct {
        a: u32,
        b: Vec<u64>,
        c: Option<Vec<u8>>,
    }

    #[repr(transparent)]
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct CanonicalStructNoRecompute(CanonicalStruct);

    impl NoritoSerialize for CanonicalStructNoRecompute {
        fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
            if PANIC_ON_SERIALIZE.load(Ordering::Relaxed) {
                panic!("serialize called during canonical decode recompute");
            }
            self.0.serialize(writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            self.0.encoded_len_hint()
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            self.0.encoded_len_exact()
        }
    }

    impl<'a> NoritoDeserialize<'a> for CanonicalStructNoRecompute {
        fn deserialize(archived: &'a Archived<Self>) -> Self {
            Self::try_deserialize(archived).expect("CanonicalStructNoRecompute decode")
        }

        fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
            let value = CanonicalStruct::try_deserialize(archived.cast::<CanonicalStruct>())?;
            Ok(Self(value))
        }
    }

    #[test]
    fn decode_field_canonical_does_not_recompute_for_derived_struct() {
        PANIC_ON_SERIALIZE.store(false, Ordering::Relaxed);
        reset_decode_state();

        let value = CanonicalStructNoRecompute(CanonicalStruct {
            a: 0xAABBCCDD,
            b: vec![1, 2, 3, 4, 5],
            c: Some(vec![9, 8, 7]),
        });
        let encoded = encode_adaptive(&value);

        PANIC_ON_SERIALIZE.store(true, Ordering::Relaxed);
        let (decoded, used) =
            decode_field_canonical::<CanonicalStructNoRecompute>(&encoded).expect("decode struct");
        assert_eq!(used, encoded.len());
        assert_eq!(decoded, value);

        PANIC_ON_SERIALIZE.store(false, Ordering::Relaxed);
    }

    #[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
    enum CanonicalEnum {
        Unit,
        One(u32),
        Many { a: u32, b: Vec<u8> },
    }

    #[repr(transparent)]
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct CanonicalEnumNoRecompute(CanonicalEnum);

    impl NoritoSerialize for CanonicalEnumNoRecompute {
        fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
            if PANIC_ON_SERIALIZE.load(Ordering::Relaxed) {
                panic!("serialize called during canonical decode recompute");
            }
            self.0.serialize(writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            self.0.encoded_len_hint()
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            self.0.encoded_len_exact()
        }
    }

    impl<'a> NoritoDeserialize<'a> for CanonicalEnumNoRecompute {
        fn deserialize(archived: &'a Archived<Self>) -> Self {
            Self::try_deserialize(archived).expect("CanonicalEnumNoRecompute decode")
        }

        fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
            let value = CanonicalEnum::try_deserialize(archived.cast::<CanonicalEnum>())?;
            Ok(Self(value))
        }
    }

    #[test]
    fn decode_field_canonical_does_not_recompute_for_derived_enum() {
        PANIC_ON_SERIALIZE.store(false, Ordering::Relaxed);
        reset_decode_state();

        let value = CanonicalEnumNoRecompute(CanonicalEnum::Many {
            a: 0x01020304,
            b: vec![0xAA, 0xBB, 0xCC],
        });
        let encoded = encode_adaptive(&value);

        PANIC_ON_SERIALIZE.store(true, Ordering::Relaxed);
        let (decoded, used) =
            decode_field_canonical::<CanonicalEnumNoRecompute>(&encoded).expect("decode enum");
        assert_eq!(used, encoded.len());
        assert_eq!(decoded, value);

        PANIC_ON_SERIALIZE.store(false, Ordering::Relaxed);
    }

    #[test]
    fn decode_field_canonical_handles_misaligned_payload() {
        let value: Vec<Vec<u64>> = vec![vec![1, 2, 3], vec![], vec![4, 5]];
        let encoded = encode_adaptive(&value);

        let mut storage = Vec::with_capacity(encoded.len() + 1);
        storage.push(0xAA);
        storage.extend_from_slice(&encoded);
        let misaligned = &storage[1..];
        assert_ne!(
            misaligned.as_ptr() as usize % core::mem::align_of::<Archived<Vec<Vec<u64>>>>(),
            0,
            "expected misaligned test payload"
        );

        let (decoded, used) =
            decode_field_canonical::<Vec<Vec<u64>>>(misaligned).expect("decode misaligned field");
        assert_eq!(decoded, value);
        assert_eq!(used, encoded.len());
    }

    #[test]
    fn note_payload_access_updates_max_access() {
        reset_decode_state();
        let payload: Vec<u8> = (0..32).collect();
        let _guard = PayloadCtxGuard::enter(&payload);
        note_payload_access(&payload, payload.len());
        assert_eq!(payload_ctx_max_access().unwrap(), payload.len());
    }

    #[test]
    fn decode_field_canonical_from_slice_reads_value() {
        let mut buf = Vec::new();
        0xAABBCCDDu32.serialize(&mut buf).expect("encode");
        let (value, used) = decode_field_canonical_from_slice::<u32>(&buf).expect("decode slice");
        assert_eq!(value, 0xAABBCCDD);
        assert_eq!(used, buf.len());
    }

    #[test]
    fn decode_field_canonical_slice_reads_value() {
        let mut buf = Vec::new();
        0xDEADBEEFu32.serialize(&mut buf).expect("encode");
        let (value, used) = decode_field_canonical_slice::<u32>(&buf).expect("decode slice");
        assert_eq!(value, 0xDEADBEEF);
        assert_eq!(used, buf.len());
    }

    #[test]
    fn decode_field_canonical_from_slice_rejects_trailing_bytes() {
        let mut buf = Vec::new();
        7u32.serialize(&mut buf).expect("encode");
        buf.push(0xFF);
        let err = decode_field_canonical_from_slice::<u32>(&buf).expect_err("trailing bytes");
        assert!(matches!(err, Error::LengthMismatch));
    }

    #[test]
    fn to_bytes_in_matches_to_bytes() {
        let value: Vec<u64> = vec![1, 2, 3, 4];
        let mut out = Vec::with_capacity(128);
        to_bytes_in(&value, &mut out).expect("encode to buffer");
        let expected = to_bytes(&value).expect("encode expected");
        assert_eq!(out, expected);

        let cap = out.capacity();
        to_bytes_in(&value, &mut out).expect("encode to buffer again");
        assert!(out.capacity() >= cap);
    }

    #[test]
    fn byte_sink_with_headroom_from_preserves_capacity() {
        let mut buf = Vec::with_capacity(2048);
        buf.extend_from_slice(&[1u8, 2, 3, 4]);
        let cap = buf.capacity();
        let sink = ByteSink::with_headroom_from(buf, 16, Header::SIZE);
        assert_eq!(sink.buf.len(), Header::SIZE);
        assert!(sink.buf.capacity() >= cap);
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct BadExactLen(u32);

    impl crate::NoritoSerialize for BadExactLen {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), Error> {
            crate::NoritoSerialize::serialize(&self.0, writer)
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            Some(1)
        }
    }

    impl<'de> crate::NoritoDeserialize<'de> for BadExactLen {
        fn deserialize(archived: &'de Archived<Self>) -> Self {
            Self::try_deserialize(archived).expect("BadExactLen decode must succeed")
        }

        fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, Error> {
            let ptr = core::ptr::from_ref(archived).cast::<u8>();
            let payload = payload_slice_from_ptr(ptr)?;
            let (value, _) = decode_field_canonical::<u32>(payload)?;
            Ok(BadExactLen(value))
        }
    }

    impl<'a> DecodeFromSlice<'a> for BadExactLen {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
            let (value, used) = decode_field_canonical::<u32>(bytes)?;
            Ok((BadExactLen(value), used))
        }
    }

    #[test]
    fn decode_field_canonical_ignores_bad_encoded_len_exact() {
        let value = BadExactLen(0xAABBCCDD);
        let encoded = encode_adaptive(&value);
        let (decoded, used) =
            decode_field_canonical::<BadExactLen>(&encoded).expect("decode bad exact len");
        assert_eq!(decoded, value);
        assert_eq!(used, encoded.len());
    }

    #[test]
    fn write_len_prefixed_uses_actual_length() {
        let value = BadExactLen(0xDEADBEEF);
        let mut out = Vec::new();
        let mut tmp: DeriveSmallBuf = DeriveSmallBuf::new();
        write_len_prefixed(&mut out, &value, &mut tmp).expect("write len prefixed");
        let (len, hdr) = read_len_from_slice(&out).expect("read len");
        assert_eq!(len, out.len() - hdr);
    }

    #[derive(Clone, Debug, PartialEq, crate::Encode, crate::Decode)]
    struct BadExactWrapper {
        inner: BadExactLen,
    }

    #[derive(Clone, Debug, PartialEq, crate::Encode, crate::Decode)]
    enum BadExactEnum {
        One(BadExactLen),
    }

    #[test]
    fn derived_struct_uses_actual_length_prefix() {
        let value = BadExactWrapper {
            inner: BadExactLen(0xAABBCCDD),
        };
        let bytes = encode_adaptive(&value);
        let decoded: BadExactWrapper = codec::decode_adaptive(&bytes).expect("decode wrapper");
        assert_eq!(decoded, value);
    }

    #[test]
    fn derived_enum_uses_actual_length_prefix() {
        let value = BadExactEnum::One(BadExactLen(0x11223344));
        let bytes = encode_adaptive(&value);
        let decoded: BadExactEnum = codec::decode_adaptive(&bytes).expect("decode enum");
        assert_eq!(decoded, value);
    }

    #[test]
    fn result_uses_actual_length_prefix() {
        let value: Result<BadExactLen, BadExactLen> = Ok(BadExactLen(0x01020304));
        let bytes = encode_adaptive(&value);
        let decoded: Result<BadExactLen, BadExactLen> =
            codec::decode_adaptive(&bytes).expect("decode result");
        assert_eq!(decoded, value);
    }

    #[derive(Clone, Copy)]
    struct RootAware(u32);

    impl crate::NoritoSerialize for RootAware {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), Error> {
            crate::NoritoSerialize::serialize(&self.0, writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            crate::NoritoSerialize::encoded_len_hint(&self.0)
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            crate::NoritoSerialize::encoded_len_exact(&self.0)
        }
    }

    impl<'de> crate::NoritoDeserialize<'de> for RootAware {
        fn deserialize(archived: &'de Archived<Self>) -> Self {
            Self::try_deserialize(archived).expect("RootAware decode must succeed")
        }

        fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, Error> {
            let ptr = core::ptr::from_ref(archived).cast::<u8>();
            let payload = payload_slice_from_ptr(ptr)?;
            let (value, _) = decode_field_canonical::<u32>(payload)?;
            Ok(RootAware(value))
        }
    }

    #[test]
    fn decode_field_canonical_installs_root_span() {
        let (payload, flags) = encode_with_header_flags(&RootAware(99));
        let _flags_guard = DecodeFlagsGuard::enter_with_hint(flags, flags);
        let (decoded, used) =
            decode_field_canonical::<RootAware>(&payload).expect("root-aware decode");
        assert_eq!(used, payload.len());
        assert_eq!(decoded.0, 99);
    }

    #[test]
    fn archived_from_slice_unchecked_realigns_payload() {
        #[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
        #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
        struct AlignSensitive {
            data: Vec<u128>,
        }

        reset_decode_state();
        let value = AlignSensitive {
            data: vec![11_u128, 22, 33, 44],
        };
        let (payload, flags) = encode_with_header_flags(&value);

        let mut storage = Vec::with_capacity(payload.len() + 1);
        storage.push(0u8);
        storage.extend_from_slice(&payload);
        let misaligned = &storage[1..];
        assert_eq!(
            misaligned,
            payload.as_slice(),
            "offset slice must retain original payload bytes"
        );
        assert_ne!(
            misaligned.as_ptr() as usize % core::mem::align_of::<Archived<AlignSensitive>>(),
            0,
            "expected misaligned payload for test coverage"
        );

        let archived = archived_from_slice_unchecked::<AlignSensitive>(misaligned);
        let _flags = DecodeFlagsGuard::enter_with_hint(flags, flags);
        let _payload = PayloadCtxGuard::enter(archived.bytes());
        let decoded = <AlignSensitive as NoritoDeserialize>::try_deserialize(archived.as_ref())
            .expect("decode misaligned AlignSensitive");
        assert_eq!(decoded, value);
        reset_decode_state();
    }

    #[test]
    fn decode_vec_recovers_from_zero_length_element_header() {
        let original: Vec<(String, Vec<u8>)> = vec![("kind".to_owned(), vec![1, 2, 3, 4])];
        let framed = to_bytes(&original).expect("serialize vec");
        let flags = framed[Header::SIZE - 1];
        let mut payload = framed[Header::SIZE..].to_vec();

        let (len, seq_hdr) = {
            let _guard = DecodeFlagsGuard::enter(flags);
            read_seq_len_slice(&payload).expect("sequence header")
        };
        assert_eq!(len, original.len());

        let elem_hdr_len = {
            let _guard = DecodeFlagsGuard::enter(flags);
            let (_, hdr) = read_len_dyn_slice(&payload[seq_hdr..]).expect("element length header");
            hdr
        };
        assert!(
            seq_hdr + elem_hdr_len <= payload.len(),
            "element header extends beyond payload"
        );
        for byte in &mut payload[seq_hdr..seq_hdr + elem_hdr_len] {
            *byte = 0;
        }

        let decoded = {
            let _guard = DecodeFlagsGuard::enter(flags);
            let (value, used) =
                decode_field_canonical::<Vec<(String, Vec<u8>)>>(&payload).expect("decode vec");
            assert_eq!(used, payload.len());
            value
        };
        reset_decode_state();
        assert_eq!(decoded, original);
    }

    #[test]
    fn payload_slice_from_ptr_falls_back_to_root_span() {
        reset_decode_state();
        let payload: Vec<u8> = (0..64).collect();
        set_decode_root(&payload);
        let ctx = &payload[8..24];
        let guard = PayloadCtxGuard::enter(ctx);
        let ptr = payload[48..].as_ptr();

        let slice = payload_slice_from_ptr(ptr).expect("fallback to root span");
        assert_eq!(slice, &payload[48..]);
        assert_eq!(payload_ctx_max_access().unwrap(), ctx.len());

        drop(guard);
        clear_decode_root();
        reset_decode_state();
    }

    #[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    #[norito(decode_from_slice)]
    struct PackedProof {
        domain: String,
        uri: String,
        statement: String,
        issued_at: String,
        nonce: String,
    }

    #[test]
    fn option_roundtrip_respects_compact_flags() {
        reset_decode_state();
        let payload: Option<PackedProof> = Some(PackedProof {
            domain: "example.org".into(),
            uri: "https://example.org/login".into(),
            statement: "Please sign in".into(),
            issued_at: "2025-01-01T00:00:00Z".into(),
            nonce: "abc123".into(),
        });
        let flags = header_flags::COMPACT_LEN
            | header_flags::PACKED_STRUCT
            | header_flags::FIELD_BITSET
            | header_flags::PACKED_SEQ;
        let encoded = {
            let _guard = DecodeFlagsGuard::enter(flags);
            let mut buf = Vec::new();
            payload.serialize(&mut buf).expect("serialize option");
            buf
        };
        reset_decode_state();
        let decoded = {
            let _guard = DecodeFlagsGuard::enter(flags);
            let (decoded, used) =
                <Option<PackedProof> as DecodeFromSlice>::decode_from_slice(&encoded)
                    .expect("decode option");
            assert_eq!(used, encoded.len());
            decoded
        };
        assert_eq!(decoded, payload);
    }

    #[test]
    fn payload_without_padding_preserves_aligned_slice() {
        let payload = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let view =
            payload_without_leading_padding(payload.as_slice(), payload.len(), 0).expect("aligned");
        assert_eq!(view, payload.as_slice());
    }

    #[test]
    fn payload_without_padding_trims_alignment_prefix() {
        let padded = vec![0, 0, 0, 1, 2, 3, 4];
        let view =
            payload_without_leading_padding(&padded, 4, 3).expect("padding trimmed successfully");
        assert_eq!(view, &padded[3..]);
    }

    #[test]
    fn payload_without_padding_rejects_nonzero_prefix() {
        let padded = vec![1, 0, 0, 1, 2, 3, 4];
        let err = payload_without_leading_padding(&padded, 4, 3)
            .expect_err("nonzero padding should be rejected");
        assert!(matches!(err, Error::LengthMismatch));
    }

    #[test]
    fn payload_without_padding_exact_accepts_expected_padding() {
        let padded = vec![0, 0, 0xAA, 0xBB];
        let view = payload_without_leading_padding_exact(&padded, 2, 2)
            .expect("exact padding should trim");
        assert_eq!(view, &padded[2..]);
    }

    #[test]
    fn payload_without_padding_exact_rejects_nonzero_padding() {
        let padded = vec![1, 0, 0xAA, 0xBB];
        let err = payload_without_leading_padding_exact(&padded, 2, 2)
            .expect_err("nonzero padding should be rejected");
        assert!(matches!(err, Error::LengthMismatch));
    }

    #[test]
    fn payload_without_padding_rejects_short_slice() {
        let data = vec![1, 2];
        let err = payload_without_leading_padding(&data, 3, 0).expect_err("length mismatch");
        matches!(err, Error::LengthMismatch);
    }

    #[test]
    fn payload_without_padding_rejects_excess_prefix() {
        let payload = vec![9u8, 8, 7, 6];
        let padded = vec![0u8; 8];
        let mut with_prefix = Vec::new();
        with_prefix.extend_from_slice(&padded);
        with_prefix.extend_from_slice(&payload);

        let err = payload_without_leading_padding(&with_prefix, payload.len(), 4)
            .expect_err("excess padding should be rejected");
        assert!(matches!(err, Error::LengthMismatch));
    }

    #[test]
    fn from_bytes_rejects_excess_padding() {
        let value: u64 = 0x1122_3344_5566_7788;
        let bytes = to_bytes(&value).expect("encode header-framed payload");
        let insert_at = Header::SIZE + payload_alignment_padding_for::<u64>();
        let mut mutated = Vec::with_capacity(bytes.len() + 2);
        mutated.extend_from_slice(&bytes[..insert_at]);
        mutated.extend_from_slice(&[0u8; 2]); // extra padding beyond alignment
        mutated.extend_from_slice(&bytes[insert_at..]);

        let result = from_bytes::<u64>(&mutated);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[test]
    fn decode_from_bytes_rejects_excess_padding() {
        let value: u64 = 0x1122_3344_5566_7788;
        let bytes = to_bytes(&value).expect("encode header-framed payload");
        let insert_at = Header::SIZE + payload_alignment_padding_for::<u64>();
        let mut mutated = Vec::with_capacity(bytes.len() + 2);
        mutated.extend_from_slice(&bytes[..insert_at]);
        mutated.extend_from_slice(&[0u8; 2]); // extra padding beyond alignment
        mutated.extend_from_slice(&bytes[insert_at..]);

        let result = crate::decode_from_bytes::<u64>(&mutated);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[test]
    fn from_bytes_rejects_trailing_bytes() {
        let value: u64 = 0xCAFEBABE_DEADBEEF;
        let mut bytes = to_bytes(&value).expect("encode header-framed payload");
        bytes.push(0);

        let result = from_bytes::<u64>(&bytes);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[test]
    fn from_bytes_rejects_invalid_flag_combo() {
        reset_decode_state();
        let value: u64 = 0xABCD_EF01_2345_6789;
        let mut bytes = to_bytes(&value).expect("encode header-framed payload");
        bytes[Header::SIZE - 1] = header_flags::FIELD_BITSET;

        let result = from_bytes::<u64>(&bytes);
        assert!(matches!(
            result,
            Err(Error::UnsupportedFeature("layout flag combination"))
        ));
        reset_decode_state();
    }

    #[test]
    fn from_compressed_bytes_rejects_trailing_bytes() {
        let value: u64 = 0x1111_2222_3333_4444;
        let mut bytes = to_bytes(&value).expect("encode header-framed payload");
        bytes.push(0);

        let result = from_compressed_bytes::<u64>(&bytes);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[cfg(feature = "compression")]
    #[test]
    fn from_compressed_bytes_rejects_trailing_compressed_bytes() {
        let value = vec![0u8; 64];
        let mut bytes =
            to_compressed_bytes(&value, Some(CompressionConfig::default())).expect("encode");
        bytes.extend_from_slice(&[0xAA, 0xBB]);

        let result = from_compressed_bytes::<Vec<u8>>(&bytes);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[test]
    fn from_compressed_bytes_accepts_aligned_padding() {
        let value: u64 = 0xDEAD_BEEF_DEAD_BEEFu64;
        let bytes = to_compressed_bytes(&value, None).expect("encode compressed payload");
        let archived = from_compressed_bytes::<u64>(&bytes).expect("decode compressed payload");
        let decoded = u64::deserialize(&archived);
        assert_eq!(decoded, value);
    }

    #[cfg(feature = "compression")]
    #[test]
    fn from_compressed_bytes_rejects_length_mismatch() {
        let value = vec![0u8; 64];
        let mut bytes =
            to_compressed_bytes(&value, Some(CompressionConfig::default())).expect("encode");
        let len_offset = 4 + 1 + 1 + 16 + 1;
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&bytes[len_offset..len_offset + 8]);
        let len = u64::from_le_bytes(len_bytes);
        let new_len = len.saturating_sub(1);
        bytes[len_offset..len_offset + 8].copy_from_slice(&new_len.to_le_bytes());

        let result = from_compressed_bytes::<Vec<u8>>(&bytes);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[allow(dead_code)]
    #[repr(align(64))]
    struct Align64(u8);

    #[test]
    fn archived_box_aligns_payload() {
        let archived = ArchivedBox::<Align64>::from_payload(vec![0xAA]);
        let ptr = archived.archived() as *const Archived<Align64> as usize;
        assert_eq!(ptr % core::mem::align_of::<Archived<Align64>>(), 0);
        assert_eq!(archived.bytes(), &[0xAA]);
    }

    #[test]
    fn frame_bare_with_default_header_prefers_recorded_flags() {
        reset_decode_state();
        let value = vec![1u32, 2, 3];
        let bare = encode_adaptive(&value);
        // Override the recorded header flags with a combination that differs from
        // compile-time defaults to ensure the framing helper uses it.
        let expected = header_flags::PACKED_SEQ | header_flags::COMPACT_LEN;
        record_last_header_flags(expected);
        let stored = take_last_header_flags().expect("recorded flags should be present");
        assert_eq!(stored, expected);
        record_last_header_flags(stored);

        let framed =
            frame_bare_with_default_header::<Vec<u32>>(&bare).expect("frame payload with header");
        assert_eq!(framed[Header::SIZE - 1], expected,);
        let mut cursor = std::io::Cursor::new(&framed);
        let header = Header::read(&mut cursor).expect("read framed header");
        assert_eq!(header.flags & !supported_header_flags(), 0);
        assert_eq!(
            header.flags & (header_flags::PACKED_SEQ | header_flags::COMPACT_LEN),
            expected,
        );
    }

    #[test]
    fn write_bare_frame_with_header_flags_matches_vec_framer() {
        reset_decode_state();
        let value = vec![9u32, 8, 7, 6];
        let (bare, flags) = crate::codec::encode_with_header_flags(&value);
        let expected =
            frame_bare_with_header_flags::<Vec<u32>>(&bare, flags).expect("frame payload into vec");
        let mut actual = Vec::new();
        write_bare_frame_with_header_flags::<Vec<u32>, _>(&mut actual, &bare, flags)
            .expect("frame payload into writer");

        assert_eq!(actual, expected);
    }

    #[test]
    fn frame_current_payload_preserves_active_flags() {
        reset_decode_state();
        let value = vec![5u64, 6, 7, 8];
        let bytes = to_bytes(&value).expect("encode value");
        let original_flags = bytes[Header::SIZE - 1];
        let archived = from_bytes::<Vec<u64>>(&bytes).expect("decode header-framed payload");
        let _ = archived;

        let reframed = frame_current_payload_with_default_header::<Vec<u64>>()
            .expect("reframe current payload");
        let mut cursor = std::io::Cursor::new(&reframed);
        let header = Header::read(&mut cursor).expect("read reframed header");
        assert_eq!(header.flags, original_flags);
        reset_decode_state();
    }

    #[test]
    fn frame_bare_with_default_header_requires_layout_metadata() {
        reset_decode_state();
        let _ = take_last_header_flags();
        let payload = vec![0u8; 4];
        let err = frame_bare_with_default_header::<Vec<u8>>(&payload)
            .expect_err("missing flags should surface an error");
        matches!(err, Error::MissingLayoutFlags);
    }

    #[test]
    fn frame_current_payload_requires_negotiated_flags() {
        reset_decode_state();
        let bytes = vec![0u8; 8];
        let _ctx = PayloadCtxGuard::enter(&bytes);
        let err = frame_current_payload_with_default_header::<Vec<u8>>()
            .expect_err("payload context without flags should fail");
        matches!(err, Error::MissingLayoutFlags);
    }

    #[test]
    fn encode_with_header_flags_exposes_recorded_layout() {
        reset_decode_state();
        let value = vec![5u64, 6, 7];
        let (bare, flags) = crate::codec::encode_with_header_flags(&value);
        let framed = frame_bare_with_header_flags::<Vec<u64>>(&bare, flags)
            .expect("frame payload with explicit flags");
        let mut cursor = std::io::Cursor::new(&framed);
        let header = Header::read(&mut cursor).expect("read header");
        assert_eq!(header.flags, flags);
    }

    #[test]
    fn encode_with_header_flags_respects_decode_guard() {
        reset_decode_state();
        let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
        let value = vec![1u32, 2, 3, 4];
        let (_payload, flags) = crate::codec::encode_with_header_flags(&value);
        assert_ne!(flags & header_flags::PACKED_SEQ, 0);
        reset_decode_state();
    }

    #[test]
    fn read_len_readers_honor_compact_flags() {
        let flags = header_flags::COMPACT_LEN;
        let mut bytes = Vec::new();
        {
            let _guard = DecodeFlagsGuard::enter(flags);
            write_len_to_vec(&mut bytes, 3);
        }
        bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        {
            let _guard = DecodeFlagsGuard::enter(flags);
            let (len_slice, hdr_slice) = read_len_dyn_slice(&bytes).expect("slice length header");
            assert_eq!(len_slice, 3);
            assert_eq!(hdr_slice, 1, "varint header should consume one byte");

            let _payload_guard = PayloadCtxGuard::enter(&bytes);
            let (len_ptr, hdr_ptr) =
                read_len_dyn_at_ptr(bytes.as_ptr()).expect("pointer length header");
            assert_eq!(len_ptr, 3);
            assert_eq!(hdr_ptr, 1);
        }

        // Sequence headers are fixed-width in v1.
        let mut seq_fixed = Vec::new();
        seq_fixed.extend_from_slice(&3u64.to_le_bytes());
        seq_fixed.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        let seq_flags_without_seq = header_flags::PACKED_SEQ;
        {
            let _guard = DecodeFlagsGuard::enter(seq_flags_without_seq);
            let (seq_len_slice, seq_hdr_slice) =
                read_seq_len_slice(&seq_fixed).expect("sequence len slice");
            assert_eq!(seq_len_slice, 3);
            assert_eq!(seq_hdr_slice, 8);
            let _seq_payload = PayloadCtxGuard::enter(&seq_fixed);
            let (seq_len_ptr, seq_hdr_ptr) =
                unsafe { read_seq_len_ptr(seq_fixed.as_ptr()) }.expect("sequence len ptr");
            assert_eq!(seq_len_ptr, 3);
            assert_eq!(seq_hdr_ptr, 8);
        }
    }

    #[test]
    fn decode_flags_guard_clears_hint_between_payloads() {
        if (default_encode_flags() & header_flags::PACKED_SEQ) == 0 {
            return;
        }
        reset_decode_state();
        {
            let _packed =
                DecodeFlagsGuard::enter(header_flags::PACKED_SEQ | header_flags::COMPACT_LEN);
            assert!(decode_flags_active());
            assert_eq!(
                get_decode_flags(),
                header_flags::PACKED_SEQ | header_flags::COMPACT_LEN
            );
            assert!(use_packed_seq());
            assert!(use_compact_len());
        }

        assert!(!decode_flags_active());
        assert_eq!(get_decode_flags(), 0);
        assert!(!use_packed_seq());
        assert!(!use_compact_len());

        {
            let _neutral = DecodeFlagsGuard::enter(0);
            assert!(decode_flags_active());
            assert_eq!(get_decode_flags(), 0);
            assert!(!use_packed_seq());
            assert!(!use_compact_len());
        }

        reset_decode_state();
    }

    #[test]
    fn decode_flags_guard_overrides_active_payload_context() {
        reset_decode_state();
        let payload = [0_u8; 8];
        let _ctx = PayloadCtxGuard::enter_with_flags(&payload, header_flags::COMPACT_LEN);
        assert_eq!(
            current_decode_flags_effective(),
            Some(header_flags::COMPACT_LEN)
        );
        {
            let _neutral = DecodeFlagsGuard::enter(0);
            assert_eq!(current_decode_flags_effective(), Some(0));
            assert!(!use_compact_len());
        }
        reset_decode_state();
    }

    #[derive(Debug, PartialEq, iroha_schema::IntoSchema, NoritoSerialize, NoritoDeserialize)]
    struct StringAndNumber {
        first: String,
        second: u64,
    }

    #[test]
    fn canonical_string_field_preserves_following_values() {
        let input = StringAndNumber {
            first: String::from("hello world"),
            second: 0xDEADBEEFCAFEBABE,
        };
        let bytes = crate::to_bytes(&input).expect("encode struct");
        let archived = crate::from_bytes::<StringAndNumber>(&bytes).expect("decode struct");
        let decoded = StringAndNumber::deserialize(archived);
        assert_eq!(decoded, input);
    }

    #[test]
    fn primitive_roundtrip() {
        let value: u32 = 42;
        let bytes = to_bytes(&value).unwrap();
        let decoded: u32 = decode_from_bytes(&bytes).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn signed_primitive_roundtrip() {
        let value: i64 = -42;
        let bytes = to_bytes(&value).unwrap();
        let decoded: i64 = decode_from_bytes(&bytes).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn btreemap_entry_slices_returns_expected_windows() {
        let keys = [1u8, 2, 3, 4];
        let values = [10u8, 11, 12];
        let (key_slice, value_slice) =
            super::btreemap_entry_slices(&keys, &values, 1, 3, 0, 2, 0).expect("slices");
        assert_eq!(key_slice, &[2, 3]);
        assert_eq!(value_slice, &[10, 11]);
    }

    #[test]
    fn btreemap_entry_slices_detects_out_of_bounds() {
        let keys = [1u8, 2];
        let values = [3u8, 4];
        let err = super::btreemap_entry_slices(&keys, &values, 0, 3, 0, 1, 0)
            .expect_err("slice bounds check");
        assert!(matches!(err, Error::LengthMismatch));
    }

    #[test]
    fn packed_maps_keep_key_then_value_payload_layout() {
        fn read_u64_at(bytes: &[u8], offset: usize) -> u64 {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[offset..offset + 8]);
            u64::from_le_bytes(buf)
        }

        reset_decode_state();
        let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
        let mut tree = std::collections::BTreeMap::new();
        tree.insert(0x0102_u16, 0x0304_0506_u32);
        tree.insert(0x0708_u16, 0x090A_0B0C_u32);

        let mut bytes = Vec::new();
        NoritoSerialize::serialize(&tree, &mut bytes).expect("serialize packed map");
        assert_eq!(read_u64_at(&bytes, 0), 2);
        assert_eq!(read_u64_at(&bytes, 8), 0);
        assert_eq!(read_u64_at(&bytes, 16), 2);
        assert_eq!(read_u64_at(&bytes, 24), 4);
        assert_eq!(read_u64_at(&bytes, 32), 0);
        assert_eq!(read_u64_at(&bytes, 40), 4);
        assert_eq!(read_u64_at(&bytes, 48), 8);
        assert_eq!(
            &bytes[56..],
            &[
                0x02, 0x01, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x0C, 0x0B, 0x0A, 0x09
            ]
        );

        let (decoded_tree, used) =
            <std::collections::BTreeMap<u16, u32> as DecodeFromSlice>::decode_from_slice(&bytes)
                .expect("decode packed btree map");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded_tree, tree);

        let mut hash = std::collections::HashMap::new();
        hash.insert(0x0708_u16, 0x090A_0B0C_u32);
        hash.insert(0x0102_u16, 0x0304_0506_u32);
        let mut hash_bytes = Vec::new();
        NoritoSerialize::serialize(&hash, &mut hash_bytes).expect("serialize packed hash map");
        assert_eq!(hash_bytes, bytes);
        let (decoded_hash, used) =
            <std::collections::HashMap<u16, u32> as DecodeFromSlice>::decode_from_slice(
                &hash_bytes,
            )
            .expect("decode packed hash map");
        assert_eq!(used, hash_bytes.len());
        assert_eq!(decoded_hash, hash);
        reset_decode_state();
    }

    #[test]
    fn collection_decoders_handle_u8_element_sequences_directly() {
        use std::collections::{BTreeSet, BinaryHeap, HashSet, LinkedList, VecDeque};

        reset_decode_state();
        let deque = VecDeque::from([1_u8, 2, 3]);
        let mut deque_bytes = Vec::new();
        NoritoSerialize::serialize(&deque, &mut deque_bytes).expect("serialize vecdeque");
        assert!(
            deque_bytes.len() > 8 + deque.len(),
            "VecDeque<u8> uses per-element sequence framing"
        );
        let (decoded_deque, used) =
            <VecDeque<u8> as DecodeFromSlice>::decode_from_slice(&deque_bytes)
                .expect("decode vecdeque");
        assert_eq!(used, deque_bytes.len());
        assert_eq!(decoded_deque, deque);

        let list = LinkedList::from([4_u8, 5, 6]);
        let mut list_bytes = Vec::new();
        NoritoSerialize::serialize(&list, &mut list_bytes).expect("serialize linked list");
        let (decoded_list, used) =
            <LinkedList<u8> as DecodeFromSlice>::decode_from_slice(&list_bytes)
                .expect("decode linked list");
        assert_eq!(used, list_bytes.len());
        assert_eq!(decoded_list, list);

        let heap = BinaryHeap::from([7_u8, 8, 9]);
        let mut heap_bytes = Vec::new();
        NoritoSerialize::serialize(&heap, &mut heap_bytes).expect("serialize heap");
        let (decoded_heap, used) =
            <BinaryHeap<u8> as DecodeFromSlice>::decode_from_slice(&heap_bytes)
                .expect("decode heap");
        assert_eq!(used, heap_bytes.len());
        assert_eq!(decoded_heap.into_sorted_vec(), heap.into_sorted_vec());

        let btree = BTreeSet::from([10_u8, 11, 12]);
        let mut btree_bytes = Vec::new();
        NoritoSerialize::serialize(&btree, &mut btree_bytes).expect("serialize btree set");
        let (decoded_btree, used) =
            <BTreeSet<u8> as DecodeFromSlice>::decode_from_slice(&btree_bytes)
                .expect("decode btree set");
        assert_eq!(used, btree_bytes.len());
        assert_eq!(decoded_btree, btree);

        let hash = HashSet::from([13_u8, 14, 15]);
        let mut hash_bytes = Vec::new();
        NoritoSerialize::serialize(&hash, &mut hash_bytes).expect("serialize hash set");
        let (decoded_hash, used) = <HashSet<u8> as DecodeFromSlice>::decode_from_slice(&hash_bytes)
            .expect("decode hash set");
        assert_eq!(used, hash_bytes.len());
        assert_eq!(decoded_hash, hash);
        reset_decode_state();
    }

    #[test]
    fn collection_and_map_encoded_lengths_match_payloads() {
        use std::collections::{
            BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque,
        };

        fn assert_lengths<T: NoritoSerialize>(value: &T) {
            let mut bytes = Vec::new();
            NoritoSerialize::serialize(value, &mut bytes).expect("serialize value");
            assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
            assert_eq!(value.encoded_len_exact(), Some(bytes.len()));
        }

        reset_decode_state();
        assert_lengths(&VecDeque::from([1_u16, 2, 3]));
        assert_lengths(&LinkedList::from([4_u16, 5, 6]));
        assert_lengths(&BinaryHeap::from([7_u16, 8, 9]));
        assert_lengths(&BTreeSet::from([10_u16, 11, 12]));
        assert_lengths(&HashSet::from([13_u16, 14, 15]));
        assert_lengths(&BTreeMap::from([(1_u16, 2_u32), (3, 4)]));
        assert_lengths(&HashMap::from([(5_u16, 6_u32), (7, 8)]));

        let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
        assert_lengths(&VecDeque::from([1_u16, 2, 3]));
        assert_lengths(&LinkedList::from([4_u16, 5, 6]));
        assert_lengths(&BinaryHeap::from([7_u16, 8, 9]));
        assert_lengths(&BTreeSet::from([10_u16, 11, 12]));
        assert_lengths(&HashSet::from([13_u16, 14, 15]));
        assert_lengths(&BTreeMap::from([(1_u16, 2_u32), (3, 4)]));
        assert_lengths(&HashMap::from([(5_u16, 6_u32), (7, 8)]));
        reset_decode_state();
    }

    #[test]
    fn array_and_tuple_serialization_use_compact_element_lengths() {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);

        let array = [5_u8, 7];
        let mut array_bytes = Vec::new();
        NoritoSerialize::serialize(&array, &mut array_bytes).expect("serialize array");
        assert_eq!(array_bytes, [1, 5, 1, 7]);
        assert_eq!(array.encoded_len_hint(), Some(array_bytes.len()));
        assert_eq!(array.encoded_len_exact(), Some(array_bytes.len()));

        let mut tuple_bytes = Vec::new();
        let tuple = (5_u8, 7_u8);
        NoritoSerialize::serialize(&tuple, &mut tuple_bytes).expect("serialize tuple");
        assert_eq!(tuple_bytes, [1, 5, 1, 7]);
        assert_eq!(tuple.encoded_len_hint(), Some(tuple_bytes.len()));
        assert_eq!(tuple.encoded_len_exact(), Some(tuple_bytes.len()));

        reset_decode_state();
    }

    #[test]
    fn string_and_result_lengths_match_compact_payloads() {
        use std::{borrow::Cow, rc::Rc, sync::Arc};

        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);

        let value = String::from("ok");
        let mut string_bytes = Vec::new();
        NoritoSerialize::serialize(&value, &mut string_bytes).expect("serialize string");
        assert_eq!(string_bytes, [2, b'o', b'k']);
        assert_eq!(value.encoded_len_hint(), Some(string_bytes.len()));
        assert_eq!(value.encoded_len_exact(), Some(string_bytes.len()));

        let borrowed = "ok";
        let mut borrowed_bytes = Vec::new();
        NoritoSerialize::serialize(&borrowed, &mut borrowed_bytes).expect("serialize &str");
        assert_eq!(borrowed_bytes, string_bytes);
        assert_eq!(borrowed.encoded_len_hint(), Some(borrowed_bytes.len()));
        assert_eq!(borrowed.encoded_len_exact(), Some(borrowed_bytes.len()));

        let cow: Cow<'_, str> = Cow::Borrowed("ok");
        let mut cow_bytes = Vec::new();
        NoritoSerialize::serialize(&cow, &mut cow_bytes).expect("serialize cow str");
        assert_eq!(cow_bytes, string_bytes);
        assert_eq!(cow.encoded_len_hint(), Some(cow_bytes.len()));
        assert_eq!(cow.encoded_len_exact(), Some(cow_bytes.len()));

        let boxed_str = String::from("ok").into_boxed_str();
        let mut boxed_str_bytes = Vec::new();
        NoritoSerialize::serialize(&boxed_str, &mut boxed_str_bytes).expect("serialize box str");
        assert_eq!(boxed_str_bytes, string_bytes);
        assert_eq!(boxed_str.encoded_len_hint(), Some(boxed_str_bytes.len()));
        assert_eq!(boxed_str.encoded_len_exact(), Some(boxed_str_bytes.len()));

        let boxed = Box::new(String::from("ok"));
        let mut boxed_bytes = Vec::new();
        NoritoSerialize::serialize(&boxed, &mut boxed_bytes).expect("serialize boxed string");
        assert_eq!(boxed_bytes, [3, 2, b'o', b'k']);
        assert_eq!(boxed.encoded_len_hint(), Some(boxed_bytes.len()));
        assert_eq!(boxed.encoded_len_exact(), Some(boxed_bytes.len()));

        let rc = Rc::new(String::from("ok"));
        let mut rc_bytes = Vec::new();
        NoritoSerialize::serialize(&rc, &mut rc_bytes).expect("serialize rc string");
        assert_eq!(rc_bytes, boxed_bytes);
        assert_eq!(rc.encoded_len_hint(), Some(rc_bytes.len()));
        assert_eq!(rc.encoded_len_exact(), Some(rc_bytes.len()));

        let arc = Arc::new(String::from("ok"));
        let mut arc_bytes = Vec::new();
        NoritoSerialize::serialize(&arc, &mut arc_bytes).expect("serialize arc string");
        assert_eq!(arc_bytes, boxed_bytes);
        assert_eq!(arc.encoded_len_hint(), Some(arc_bytes.len()));
        assert_eq!(arc.encoded_len_exact(), Some(arc_bytes.len()));

        let some = Some(String::from("ok"));
        let mut some_bytes = Vec::new();
        NoritoSerialize::serialize(&some, &mut some_bytes).expect("serialize option some");
        assert_eq!(some_bytes, [1, 3, 2, b'o', b'k']);
        assert_eq!(some.encoded_len_hint(), Some(some_bytes.len()));
        assert_eq!(some.encoded_len_exact(), Some(some_bytes.len()));

        let none: Option<String> = None;
        let mut none_bytes = Vec::new();
        NoritoSerialize::serialize(&none, &mut none_bytes).expect("serialize option none");
        assert_eq!(none_bytes, [0]);
        assert_eq!(none.encoded_len_hint(), Some(none_bytes.len()));
        assert_eq!(none.encoded_len_exact(), Some(none_bytes.len()));

        let ok: Result<String, String> = Ok(value);
        let mut ok_bytes = Vec::new();
        NoritoSerialize::serialize(&ok, &mut ok_bytes).expect("serialize result ok");
        assert_eq!(ok_bytes, [0, 3, 2, b'o', b'k']);
        assert_eq!(ok.encoded_len_hint(), Some(ok_bytes.len()));
        assert_eq!(ok.encoded_len_exact(), Some(ok_bytes.len()));

        let err: Result<String, String> = Err(String::from("no"));
        let mut err_bytes = Vec::new();
        NoritoSerialize::serialize(&err, &mut err_bytes).expect("serialize result err");
        assert_eq!(err_bytes, [1, 3, 2, b'n', b'o']);
        assert_eq!(err.encoded_len_hint(), Some(err_bytes.len()));
        assert_eq!(err.encoded_len_exact(), Some(err_bytes.len()));

        reset_decode_state();
    }

    #[test]
    fn float_roundtrip() {
        let f32_val: f32 = 3.5;
        let bytes = to_bytes(&f32_val).unwrap();
        let decoded: f32 = decode_from_bytes(&bytes).unwrap();
        assert_eq!(f32_val, decoded);

        let f64_val: f64 = -2.25;
        let bytes = to_bytes(&f64_val).unwrap();
        let decoded: f64 = decode_from_bytes(&bytes).unwrap();
        assert_eq!(f64_val, decoded);
    }

    #[test]
    fn string_roundtrip() {
        let value = String::from("norito");
        let bytes = to_bytes(&value).unwrap();
        let decoded: String = decode_from_bytes(&bytes).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn box_roundtrip() {
        let value: Box<u32> = Box::new(41);
        let bytes = to_bytes(&value).unwrap();
        let archived = from_bytes::<Box<u32>>(&bytes).unwrap();
        let decoded = <Box<u32> as NoritoDeserialize>::deserialize(archived);
        assert_eq!(value, decoded);

        let str_box: Box<String> = Box::new("boxed".into());
        let bytes = to_bytes(&str_box).unwrap();
        let archived = from_bytes::<Box<String>>(&bytes).unwrap();
        let decoded = <Box<String> as NoritoDeserialize>::deserialize(archived);
        assert_eq!(str_box, decoded);
    }

    #[test]
    fn rc_roundtrip() {
        let value: Rc<u32> = Rc::new(7);
        let bytes = to_bytes(&value).unwrap();
        let archived = from_bytes::<Rc<u32>>(&bytes).unwrap();
        let decoded = <Rc<u32> as NoritoDeserialize>::deserialize(archived);
        assert_eq!(value, decoded);

        let str_rc: Rc<String> = Rc::new(String::from("shared"));
        let bytes = to_bytes(&str_rc).unwrap();
        let archived = from_bytes::<Rc<String>>(&bytes).unwrap();
        let decoded = <Rc<String> as NoritoDeserialize>::deserialize(archived);
        assert_eq!(str_rc, decoded);
    }

    #[test]
    fn arc_roundtrip() {
        let value: Arc<u32> = Arc::new(99);
        let bytes = to_bytes(&value).unwrap();
        let archived = from_bytes::<Arc<u32>>(&bytes).unwrap();
        let decoded = <Arc<u32> as NoritoDeserialize>::deserialize(archived);
        assert_eq!(value, decoded);

        let str_arc: Arc<String> = Arc::new(String::from("threads"));
        let bytes = to_bytes(&str_arc).unwrap();
        let archived = from_bytes::<Arc<String>>(&bytes).unwrap();
        let decoded = <Arc<String> as NoritoDeserialize>::deserialize(archived);
        assert_eq!(str_arc, decoded);
    }

    #[test]
    fn option_roundtrip() {
        let value = Some(5u32);
        let bytes = to_bytes(&value).unwrap();
        let decoded: Option<u32> = decode_from_bytes(&bytes).unwrap();
        assert_eq!(value, decoded);

        let none: Option<u32> = None;
        let bytes = to_bytes(&none).unwrap();
        let decoded: Option<u32> = decode_from_bytes(&bytes).unwrap();
        assert_eq!(none, decoded);

        let str_some: Option<String> = Some("abc".into());
        let bytes = to_bytes(&str_some).unwrap();
        let decoded: Option<String> = decode_from_bytes(&bytes).unwrap();
        assert_eq!(str_some, decoded);

        let str_none: Option<String> = None;
        let bytes = to_bytes(&str_none).unwrap();
        let decoded: Option<String> = decode_from_bytes(&bytes).unwrap();
        assert_eq!(str_none, decoded);
    }

    #[test]
    fn vec_roundtrip() {
        let value = vec![1u32, 2, 3];
        let bytes = to_bytes(&value).unwrap();
        let decoded: Vec<u32> = decode_from_bytes(&bytes).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn decode_from_slice_vec_u32_packed_offsets() {
        let elems = [1u32, 2, 3];
        let mut buf = Vec::new();
        buf.extend_from_slice(&(elems.len() as u64).to_le_bytes());
        let mut offset = 0u64;
        for _ in 0..elems.len() {
            buf.extend_from_slice(&offset.to_le_bytes());
            offset += 4;
        }
        buf.extend_from_slice(&offset.to_le_bytes());
        for value in elems {
            buf.extend_from_slice(&value.to_le_bytes());
        }
        let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
        let (decoded, used) = <Vec<u32> as DecodeFromSlice>::decode_from_slice(&buf).unwrap();
        assert_eq!(decoded, elems);
        assert_eq!(used, buf.len());
    }

    #[test]
    fn decode_from_slice_vec_u32() {
        // Build payload for packed-seq Vec<T>: [len:u64][(len+1) offsets][data]
        let elems = [1u32, 2, 3];
        let mut buf = Vec::new();
        buf.extend_from_slice(&(elems.len() as u64).to_le_bytes());
        // Compute element encodings and cumulative offsets
        let mut encs: Vec<Vec<u8>> = Vec::new();
        let mut offsets: Vec<u64> = Vec::new();
        let mut total: u64 = 0;
        for &e in &elems {
            offsets.push(total);
            let mut eb = Vec::new();
            e.serialize(&mut eb).unwrap();
            total += eb.len() as u64;
            encs.push(eb);
        }
        offsets.push(total);
        for off in offsets {
            buf.extend_from_slice(&off.to_le_bytes());
        }
        for eb in encs {
            buf.extend_from_slice(&eb);
        }
        let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
        let (out, used) = <Vec<u32> as DecodeFromSlice>::decode_from_slice(&buf).unwrap();
        assert_eq!(out, elems);
        assert_eq!(used, buf.len());
    }

    #[test]
    fn vec_header_is_u64() {
        use crate::core::header_flags;

        let value = vec![42u8; 3];
        reset_decode_state();
        let flags = header_flags::PACKED_SEQ
            | header_flags::PACKED_STRUCT
            | header_flags::FIELD_BITSET
            | header_flags::COMPACT_LEN;
        let guard = DecodeFlagsGuard::enter_with_hint(flags, flags);
        let bytes = encode_adaptive(&value);
        drop(guard);
        assert!(bytes.len() >= 8);
        let mut hdr = [0u8; 8];
        hdr.copy_from_slice(&bytes[..8]);
        let reported = u64::from_le_bytes(hdr);
        assert_eq!(reported as usize, value.len());
    }

    #[test]
    fn decode_from_slice_option_and_result() {
        // Use compact-len for these slice-based decodes since we encode lengths via `write_len`.
        set_decode_flags(header_flags::COMPACT_LEN);
        clear_payload_ctx();
        // Option::Some(String)
        let s = String::from("ok");
        let mut sbuf = Vec::new();
        s.serialize(&mut sbuf).unwrap();
        let mut obuf = Vec::new();
        obuf.push(1u8); // tag
        crate::core::write_len(&mut obuf, sbuf.len() as u64).unwrap();
        obuf.extend_from_slice(&sbuf);
        let (oval, used) = <Option<String> as DecodeFromSlice>::decode_from_slice(&obuf).unwrap();
        assert_eq!(oval, Some(s));
        assert_eq!(used, obuf.len());

        // Result::Err(String)
        let e = String::from("err");
        let mut ebuf = Vec::new();
        e.serialize(&mut ebuf).unwrap();
        let mut rbuf = Vec::new();
        rbuf.push(1u8); // Err tag
        crate::core::write_len(&mut rbuf, ebuf.len() as u64).unwrap();
        rbuf.extend_from_slice(&ebuf);
        let (rval, used) =
            <Result<String, String> as DecodeFromSlice>::decode_from_slice(&rbuf).unwrap();
        assert_eq!(rval, Err(String::from("err")));
        assert_eq!(used, rbuf.len());
        reset_decode_state();
    }

    #[test]
    fn decode_from_slice_borrowed_bytes() {
        set_decode_flags(header_flags::COMPACT_LEN);
        let payload = b"bytes";
        let mut buf = Vec::new();
        crate::core::write_len(&mut buf, payload.len() as u64).unwrap();
        buf.extend_from_slice(payload);
        let (out, used) = <&[u8] as DecodeFromSlice>::decode_from_slice(&buf).unwrap();
        assert_eq!(out, &payload[..]);
        assert_eq!(used, buf.len());
        reset_decode_state();
    }

    #[test]
    fn vec_string_roundtrip() {
        let value = vec![String::from("foo"), String::from("bar")];
        let bytes = to_bytes(&value).unwrap();
        let decoded: Vec<String> = decode_from_bytes(&bytes).unwrap();
        assert_eq!(value, decoded);
    }

    #[test]
    fn archived_cast_roundtrip() {
        #[repr(transparent)]
        struct Wrapper(Vec<u32>);

        let value = vec![1u32, 2, 3];
        let bytes = to_bytes(&value).unwrap();
        let archived_vec = from_bytes::<Vec<u32>>(&bytes).unwrap();
        let archived_wrapper: &Archived<Wrapper> = archived_vec.cast::<Wrapper>();
        let archived_vec_again: &Archived<Vec<u32>> = archived_wrapper.cast::<Vec<u32>>();
        let decoded = <Vec<u32> as NoritoDeserialize>::deserialize(archived_vec_again);
        assert_eq!(value, decoded);
    }

    #[test]
    fn result_string_roundtrip() {
        let ok: Result<String, String> = Ok("ok".into());
        let bytes = to_bytes(&ok).unwrap();
        let decoded: Result<String, String> = decode_from_bytes(&bytes).unwrap();
        assert_eq!(ok, decoded);

        let err: Result<String, String> = Err("err".into());
        let bytes = to_bytes(&err).unwrap();
        let decoded: Result<String, String> = decode_from_bytes(&bytes).unwrap();
        assert_eq!(err, decoded);
    }

    #[test]
    fn view_decode_string_and_vec() {
        let s = String::from("hello");
        let bytes = to_bytes(&s).unwrap();
        let view = from_bytes_view(&bytes).unwrap();
        let decoded: String = view.decode().unwrap();
        assert_eq!(decoded, s);

        let v = vec![1u32, 2, 3, 4];
        let bytes = to_bytes(&v).unwrap();
        let view = from_bytes_view(&bytes).unwrap();
        let decoded: Vec<u32> = view.decode().unwrap();
        assert_eq!(decoded, v);
    }

    #[test]
    fn header_driven_compact_len_string_decode() {
        #[cfg(not(feature = "compact-len"))]
        {
            // Compact-len feature disabled; no additional coverage required.
        }

        #[cfg(feature = "compact-len")]
        {
            // Build a varint-length encoded string payload: len=3, data="abc"
            let mut payload = Vec::new();
            {
                let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
                write_len_to_vec(&mut payload, 3);
            }
            payload.extend_from_slice(b"abc");

            // Compose header with COMPACT_LEN flag set
            let mut bytes = Vec::new();
            let mut header = Header::new(
                <String as NoritoSerialize>::schema_hash(),
                payload.len() as u64,
                crc64(&payload),
            );
            header.flags |= header_flags::COMPACT_LEN;
            header.write(&mut bytes).unwrap();
            bytes.extend_from_slice(&payload);

            let decoded: String = decode_from_bytes(&bytes).unwrap();
            assert_eq!(decoded, "abc");
        }
    }

    #[cfg(feature = "compact-len")]
    #[test]
    fn write_len_marks_compact_len_usage() {
        reset_decode_state();
        let encode_guard = EncodeContextGuard::enter();
        {
            let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
            let mut buf = Vec::new();
            write_len_to_vec(&mut buf, 5);
            assert_eq!(buf, vec![5u8]);
        }
        assert!(
            compact_len_used(),
            "compact-len usage flag must be recorded"
        );
        drop(encode_guard);
        reset_decode_state();
    }

    #[test]
    fn length_prefix_requires_compact_len_flag() {
        reset_decode_state();
        {
            let _guard = DecodeFlagsGuard::enter(0);
            let mut buf = Vec::new();
            write_len_to_vec(&mut buf, 5);
            assert_eq!(buf.len(), 8);
            let (len, used) = read_len_from_slice(&buf).expect("read len");
            assert_eq!(len, 5);
            assert_eq!(used, 8);
            assert_eq!(len_prefix_len(5), 8);
        }
        reset_decode_state();
    }

    #[test]
    fn header_driven_fixed_len_string_decode() {
        // Build a fixed-u64 length encoded string payload with len=3
        let mut payload = Vec::new();
        payload.extend_from_slice(&(3u64.to_le_bytes()));
        payload.extend_from_slice(b"abc");

        // Compose header without COMPACT_LEN flag
        let mut bytes = Vec::new();
        let header = Header::new(
            <String as NoritoSerialize>::schema_hash(),
            payload.len() as u64,
            crc64(&payload),
        );
        header.write(&mut bytes).unwrap();
        bytes.extend_from_slice(&payload);

        let decoded: String = decode_from_bytes(&bytes).unwrap();
        assert_eq!(decoded, "abc");
    }

    #[test]
    fn seq_len_respects_explicit_flags() {
        reset_decode_state();
        let guard = DecodeFlagsGuard::enter_with_hint(0, default_encode_flags());
        let mut fixed_len_buf = Vec::new();
        fixed_len_buf.extend_from_slice(&(4u64.to_le_bytes()));
        let (len, used) = read_seq_len_slice(&fixed_len_buf).expect("fallback fixed len");
        assert_eq!(len, 4);
        assert_eq!(used, 8);
        drop(guard);
        reset_decode_state();
    }
    #[test]
    fn strict_safe_read_len_and_decode_from_slice() {
        set_decode_flags(header_flags::COMPACT_LEN);
        let s = "hello-世界";
        let mut buf = Vec::new();
        crate::core::write_len(&mut buf, s.len() as u64).unwrap();
        buf.extend_from_slice(s.as_bytes());

        let (out_s, used) = String::decode_from_slice(&buf).unwrap();
        assert_eq!(out_s, s);
        assert_eq!(used, buf.len());

        let (out_str, used2) = <&str as DecodeFromSlice>::decode_from_slice(&buf).unwrap();
        assert_eq!(out_str, s);
        assert_eq!(used2, buf.len());
        reset_decode_state();
    }

    #[test]
    fn fixed_u64_length_respects_usize_limits() {
        reset_decode_state();
        let _guard = DecodeFlagsGuard::enter_with_hint(0, 0);
        let overflow = (usize::MAX as u128)
            .checked_add(1)
            .and_then(|value| u64::try_from(value).ok());

        if let Some(len) = overflow {
            let buf = len.to_le_bytes();
            assert!(matches!(
                read_seq_len_slice(&buf),
                Err(Error::LengthMismatch)
            ));
            let result = unsafe { try_read_len_ptr_unchecked(buf.as_ptr()) };
            assert!(matches!(result, Err(Error::LengthMismatch)));
        } else {
            let len = 42u64;
            let buf = len.to_le_bytes();
            let (value, used) = read_seq_len_slice(&buf).expect("fixed len");
            assert_eq!(value, 42usize);
            assert_eq!(used, 8);
            let result = unsafe { try_read_len_ptr_unchecked(buf.as_ptr()) };
            let (value, used) = result.expect("fixed len ptr");
            assert_eq!(value, 42usize);
            assert_eq!(used, 8);
        }
    }

    #[test]
    fn owned_payload_len_respects_usize_limits() {
        reset_decode_state();
        let guard = DecodeFlagsGuard::enter_with_hint(0, 0);
        let overflow = (usize::MAX as u128)
            .checked_add(1)
            .and_then(|value| u64::try_from(value).ok());

        if let Some(len) = overflow {
            let mut buf = Vec::new();
            buf.extend_from_slice(&len.to_le_bytes());
            let result = <Box<u8> as DecodeFromSlice>::decode_from_slice(&buf);
            assert!(matches!(result, Err(Error::LengthMismatch)));
        } else {
            let mut buf = Vec::new();
            buf.extend_from_slice(&1u64.to_le_bytes());
            buf.push(7);
            let (value, used) =
                <Box<u8> as DecodeFromSlice>::decode_from_slice(&buf).expect("decode box");
            assert_eq!(*value, 7);
            assert_eq!(used, buf.len());
        }
        drop(guard);
        reset_decode_state();
    }

    #[test]
    fn decode_slice_usize_isize_respects_width() {
        let value = 17u64;
        let buf = value.to_le_bytes();
        let (out, used) = <usize as DecodeFromSlice>::decode_from_slice(&buf).expect("usize");
        assert_eq!(out, 17usize);
        assert_eq!(used, 8);

        let value = -9i64;
        let buf = value.to_le_bytes();
        let (out, used) = <isize as DecodeFromSlice>::decode_from_slice(&buf).expect("isize");
        assert_eq!(out, -9isize);
        assert_eq!(used, 8);

        let overflow = (usize::MAX as u128)
            .checked_add(1)
            .and_then(|value| u64::try_from(value).ok());
        if let Some(value) = overflow {
            let buf = value.to_le_bytes();
            let result = <usize as DecodeFromSlice>::decode_from_slice(&buf);
            assert!(matches!(result, Err(Error::LengthMismatch)));

            let value = i64::try_from(value).expect("overflow fits i64");
            let buf = value.to_le_bytes();
            let result = <isize as DecodeFromSlice>::decode_from_slice(&buf);
            assert!(matches!(result, Err(Error::LengthMismatch)));
        }
    }

    #[test]
    fn vec_decode_rejects_impossible_header_sizes() {
        reset_decode_state();
        let _guard = DecodeFlagsGuard::enter_with_hint(0, 0);
        let len = usize::MAX / 8 + 1;
        let len_u64 = u64::try_from(len).expect("len fits u64");
        let mut buf = Vec::new();
        buf.extend_from_slice(&len_u64.to_le_bytes());

        let result = <Vec<u8> as DecodeFromSlice>::decode_from_slice(&buf);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[test]
    fn vec_u8_encodes_as_len_plus_raw_bytes() {
        reset_decode_state();
        let value: Vec<u8> = (0..32).collect();
        let payload = encode_adaptive(&value);
        assert_eq!(payload.len(), 8 + value.len());
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&payload[..8]);
        let len = u64::from_le_bytes(len_bytes) as usize;
        assert_eq!(len, value.len());
        assert_eq!(&payload[8..], value.as_slice());
        reset_decode_state();
    }

    #[test]
    fn vec_u8_decode_rejects_len_prefixed_elements() {
        reset_decode_state();
        let guard = DecodeFlagsGuard::enter_with_hint(0, 0);
        let value: Vec<u8> = vec![1, 2, 3, 4];
        let mut payload = Vec::new();
        payload.extend_from_slice(&(value.len() as u64).to_le_bytes());
        for byte in &value {
            payload.extend_from_slice(&1u64.to_le_bytes());
            payload.push(*byte);
        }

        let result = <Vec<u8> as DecodeFromSlice>::decode_from_slice(&payload);
        assert!(matches!(result, Err(Error::LengthMismatch)));
        drop(guard);
        reset_decode_state();
    }

    #[test]
    fn vec_u8_raw_decode_works_even_with_packed_seq_flag() {
        reset_decode_state();
        let value: Vec<u8> = vec![7, 8, 9];
        let mut raw = Vec::new();
        raw.extend_from_slice(&(value.len() as u64).to_le_bytes());
        raw.extend_from_slice(&value);

        let _guard = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
        let (decoded, used) =
            <Vec<u8> as DecodeFromSlice>::decode_from_slice(&raw).expect("decode raw vec");
        assert_eq!(used, raw.len());
        assert_eq!(decoded, value);
        reset_decode_state();
    }

    #[test]
    fn missing_ctx_string_and_str_decode() {
        clear_payload_ctx();
        let mut payload = Vec::new();
        payload.extend_from_slice(&2u64.to_le_bytes());
        payload.extend_from_slice(b"ok");

        let align =
            core::mem::align_of::<Archived<String>>().max(core::mem::align_of::<Archived<&str>>());
        let archive = crate::ArchiveSlice::new(&payload, align).expect("align payload");

        let archived_string = unsafe { &*(archive.as_slice().as_ptr() as *const Archived<String>) };
        let decoded = String::deserialize(archived_string);
        assert_eq!(decoded, "ok");

        clear_payload_ctx();
        let archived_str = unsafe { &*(archive.as_slice().as_ptr() as *const Archived<&str>) };
        let decoded = <&str as NoritoDeserialize>::deserialize(archived_str);
        assert_eq!(decoded, "ok");
    }

    #[test]
    fn aos_views_reject_oversize_field_len() {
        reset_decode_state();
        let _guard = DecodeFlagsGuard::enter_with_hint(0, 0);
        let overflow = (usize::MAX as u128)
            .checked_add(1)
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(16);

        let mut body = Vec::new();
        body.extend_from_slice(&1u64.to_le_bytes());
        body.push(crate::aos::AOS_FORMAT_VERSION);
        body.extend_from_slice(&1u64.to_le_bytes());
        body.extend_from_slice(&overflow.to_le_bytes());

        let result = crate::columnar::view_aos_u64_str_bool(&body);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }
}
