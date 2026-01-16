//! A compact, heap-allocated container for immutable elements.
//!
//! [`ConstVec`] behaves similarly to [`Vec`] but omits the capacity field,
//! making it cheaper to store when the number of elements is known never to
//! change. It is primarily used for byte buffers or other data that is loaded
//! once and then treated as read‑only for the remainder of the program's
//! lifetime.
use core::{ops::Deref, ptr};
use std::{boxed::Box, format, io::Write, string::String, vec::Vec};

use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, VecMeta};
use ncore::WriteBytesExt;
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};
use norito::{NoritoDeserialize, NoritoSerialize, core as ncore};

use crate::ffi;

struct RealignedSlice {
    ptr: *mut u8,
    layout: std::alloc::Layout,
    len: usize,
}

impl RealignedSlice {
    #[allow(unsafe_code)]
    fn new(bytes: &[u8], align: usize) -> Result<Self, ncore::Error> {
        debug_assert!(!bytes.is_empty());
        let layout = std::alloc::Layout::from_size_align(bytes.len(), align)
            .map_err(|_| ncore::Error::LengthMismatch)?;
        let ptr = unsafe {
            let ptr = std::alloc::alloc(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            ptr
        };
        Ok(Self {
            ptr,
            layout,
            len: bytes.len(),
        })
    }

    #[allow(unsafe_code)]
    fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr.cast_const(), self.len) }
    }
}

impl Drop for RealignedSlice {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        unsafe {
            std::alloc::dealloc(self.ptr, self.layout);
        }
    }
}

#[allow(dead_code)]
struct AlignedPayload<'a> {
    original: &'a [u8],
    realigned: Option<RealignedSlice>,
}

#[allow(single_use_lifetimes)]
impl AlignedPayload<'_> {
    #[allow(dead_code)]
    fn as_slice(&self) -> &[u8] {
        self.realigned
            .as_ref()
            .map_or(self.original, RealignedSlice::as_slice)
    }
}

#[allow(dead_code)]
fn align_payload_for<T>(bytes: &[u8], align: usize) -> Result<AlignedPayload<'_>, ncore::Error>
where
    T: NoritoSerialize,
{
    let needs_realignment =
        align > 1 && !bytes.is_empty() && !(bytes.as_ptr() as usize).is_multiple_of(align);
    #[cfg(debug_assertions)]
    if norito::debug_trace_enabled() {
        eprintln!(
            "ConstVec::<{}>::decode align={} ptr={:#x} needs_realignment={}",
            core::any::type_name::<T>(),
            align,
            bytes.as_ptr() as usize,
            needs_realignment
        );
    }
    #[cfg(debug_assertions)]
    if needs_realignment && norito::debug_trace_enabled() {
        eprintln!(
            "ConstVec::<{}>::decode realigning payload align={} addr={:#x}",
            core::any::type_name::<T>(),
            align,
            bytes.as_ptr() as usize
        );
    }
    let realigned = if needs_realignment {
        Some(RealignedSlice::new(bytes, align)?)
    } else {
        None
    };
    Ok(AlignedPayload {
        original: bytes,
        realigned,
    })
}

ffi::ffi_item! {
    /// Stores bytes that are not supposed to change during the runtime of the
    /// program in a compact way.
    ///
    /// Compared to `Vec<T>` this type omits the capacity field, reducing the
    /// memory footprint when the collection is immutable. The trade-off is that
    /// cloning requires duplicating the entire buffer because there is no
    /// reference counting.
    #[derive(
        Clone,
        Eq,
        PartialEq,
        Ord,
        PartialOrd,
        Hash,
        Debug,
        Default,
    )]
    #[repr(transparent)]
    pub struct ConstVec<T>(Box<[T]>);

    // SAFETY: `ConstVec` has no trap representation in ConstVec
    ffi_type(unsafe {robust})
}

impl<T> ConstVec<T> {
    /// Create a new `ConstVec` from something convertible into a `Box<[T]>`.
    ///
    /// Using `Vec<T>` here would take ownership of the data without needing to copy it (if length is the same as capacity).
    #[inline]
    pub fn new(content: impl Into<Box<[T]>>) -> Self {
        Self(content.into())
    }

    /// Creates an empty `ConstVec`. This operation does not allocate any memory.
    #[inline]
    pub fn new_empty() -> Self {
        Self(Vec::new().into())
    }

    /// Converts the `ConstVec` into a `Vec<T>`, reusing the heap allocation.
    #[inline]
    pub fn into_vec(self) -> Vec<T> {
        self.0.into_vec()
    }
}

impl<T> AsRef<[T]> for ConstVec<T> {
    fn as_ref(&self) -> &[T] {
        self.0.as_ref()
    }
}

impl<T> Deref for ConstVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> From<Vec<T>> for ConstVec<T> {
    fn from(value: Vec<T>) -> Self {
        Self::new(value)
    }
}

#[cfg(feature = "json")]
impl<T> json::FastJsonWrite for ConstVec<T>
where
    T: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('[');
        let mut iter = self.0.iter();
        if let Some(first) = iter.next() {
            JsonSerialize::json_serialize(first, out);
            for item in iter {
                out.push(',');
                JsonSerialize::json_serialize(item, out);
            }
        }
        out.push(']');
    }
}

#[cfg(feature = "json")]
impl<T> JsonDeserialize for ConstVec<T>
where
    T: JsonDeserialize,
{
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let values: Vec<T> = Vec::<T>::json_deserialize(parser)?;
        Ok(ConstVec::from(values))
    }
}

impl<T: NoritoSerialize> NoritoSerialize for ConstVec<T> {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), ncore::Error> {
        let slice: &[T] = &self.0;
        #[cfg(debug_assertions)]
        let trace_enabled = norito::debug_trace_enabled();
        #[cfg(not(debug_assertions))]
        let trace_enabled = false;

        #[cfg(debug_assertions)]
        if trace_enabled {
            eprintln!(
                "ConstVec::<{}>::serialize len={} use_packed_seq={}",
                core::any::type_name::<T>(),
                slice.len(),
                ncore::use_packed_seq(),
            );
        }

        ncore::write_seq_len(&mut writer, slice.len() as u64)?;
        if ncore::use_packed_seq() {
            Self::serialize_packed(slice, &mut writer, trace_enabled)
        } else {
            Self::serialize_unpacked(slice, &mut writer)
        }
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        let slice: &[T] = &self.0;
        let len = slice.len();
        let seq_hdr = ncore::seq_len_prefix_len(len);
        if !ncore::use_packed_seq() {
            let mut total = seq_hdr;
            let use_compact = ncore::use_compact_len();
            for item in slice {
                let elem_len = item.encoded_len_hint()?;
                let len_bytes = if use_compact {
                    ncore::len_prefix_len(elem_len)
                } else {
                    8
                };
                total = total.saturating_add(len_bytes);
                total = total.saturating_add(elem_len);
            }
            return Some(total);
        }

        let mut total = seq_hdr;
        let entries = len.saturating_add(1);
        total = total.saturating_add(8usize.saturating_mul(entries));
        for item in slice {
            let elem_hint = item.encoded_len_hint()?;
            total = total.saturating_add(elem_hint);
        }
        Some(total)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let slice: &[T] = &self.0;
        let len = slice.len();
        let seq_hdr = ncore::seq_len_prefix_len(len);
        if !ncore::use_packed_seq() {
            let mut total = seq_hdr;
            let use_compact = ncore::use_compact_len();
            for item in slice {
                let elem_exact = item.encoded_len_exact()?;
                let len_bytes = if use_compact {
                    ncore::len_prefix_len(elem_exact)
                } else {
                    8
                };
                total = total.checked_add(len_bytes)?;
                total = total.checked_add(elem_exact)?;
            }
            return Some(total);
        }

        let mut total = seq_hdr;
        let entries = len.checked_add(1)?;
        let offsets_bytes = entries.checked_mul(8)?;
        total = total.checked_add(offsets_bytes)?;
        let mut data_total = 0usize;
        for item in slice {
            let elem_exact = item.encoded_len_exact()?;
            data_total = data_total.checked_add(elem_exact)?;
        }
        total = total.checked_add(data_total)?;
        Some(total)
    }
}

impl<T: NoritoSerialize> ConstVec<T> {
    fn serialize_unpacked<W: Write>(slice: &[T], writer: &mut W) -> Result<(), ncore::Error> {
        let mut elem_buf = Vec::new();
        #[cfg(feature = "compact-len")]
        let use_compact = ncore::use_compact_len();
        for item in slice {
            elem_buf.clear();
            item.serialize(&mut elem_buf)?;
            #[cfg(feature = "compact-len")]
            {
                if use_compact {
                    ncore::write_len(writer, elem_buf.len() as u64)?;
                } else {
                    writer.write_u64::<ncore::LittleEndian>(elem_buf.len() as u64)?;
                }
            }
            #[cfg(not(feature = "compact-len"))]
            {
                writer.write_u64::<ncore::LittleEndian>(elem_buf.len() as u64)?;
            }
            writer.write_all(&elem_buf)?;
        }
        Ok(())
    }

    fn serialize_packed<W: Write>(
        slice: &[T],
        writer: &mut W,
        trace_enabled: bool,
    ) -> Result<(), ncore::Error> {
        let (offsets, data) = Self::collect_offsets(slice, trace_enabled)?;
        Self::write_fixed_offsets(writer, &offsets, &data, trace_enabled)
    }

    fn collect_offsets(
        slice: &[T],
        trace_enabled: bool,
    ) -> Result<(Vec<u64>, Vec<u8>), ncore::Error> {
        #[cfg(not(debug_assertions))]
        let _ = trace_enabled;

        let mut offsets = Vec::with_capacity(slice.len() + 1);
        let mut data = Vec::new();
        let mut elem_buf = Vec::new();
        let mut total: u64 = 0;

        for (idx, item) in slice.iter().enumerate() {
            #[cfg(not(debug_assertions))]
            let _ = idx;
            offsets.push(total);
            elem_buf.clear();
            item.serialize(&mut elem_buf)?;
            #[cfg(debug_assertions)]
            if trace_enabled && idx == 0 {
                eprintln!(
                    "ConstVec::<{}> encode first_elem len={} total_before={}",
                    core::any::type_name::<T>(),
                    elem_buf.len(),
                    total
                );
            }
            #[cfg(debug_assertions)]
            if trace_enabled && core::any::type_name::<T>().contains("InstructionBox") && idx < 32 {
                eprintln!(
                    "ConstVec::<InstructionBox> encode idx={} len={} total_before={}",
                    idx,
                    elem_buf.len(),
                    total
                );
            }
            total = total.wrapping_add(elem_buf.len() as u64);
            data.extend_from_slice(&elem_buf);
        }
        offsets.push(total);
        Ok((offsets, data))
    }

    #[cfg(feature = "compact-len")]
    fn write_fixed_offsets<W: Write>(
        writer: &mut W,
        offsets: &[u64],
        data: &[u8],
        trace_enabled: bool,
    ) -> Result<(), ncore::Error> {
        #[cfg(not(debug_assertions))]
        let _ = trace_enabled;

        let limit = ncore::max_archive_len();
        if limit != 0 {
            let offsets_bytes = offsets
                .len()
                .checked_mul(8)
                .ok_or(ncore::Error::LengthMismatch)?;
            let packed_total = offsets_bytes
                .checked_add(data.len())
                .ok_or(ncore::Error::LengthMismatch)?;
            let packed_total =
                u64::try_from(packed_total).map_err(|_| ncore::Error::LengthMismatch)?;
            if packed_total > limit {
                return Err(ncore::Error::ArchiveLengthExceeded {
                    length: packed_total,
                    limit,
                });
            }
        }

        ncore::note_fixed_offsets_emitted();
        let mut offs_bytes = Vec::with_capacity(offsets.len() * 8);
        for &off in offsets {
            offs_bytes.extend_from_slice(&off.to_le_bytes());
        }
        #[cfg(debug_assertions)]
        if trace_enabled && core::any::type_name::<T>().contains("InstructionBox") {
            let preview = offs_bytes.len().min(16);
            eprintln!(
                "ConstVec::<{}> offs_bytes_preview={:?}",
                core::any::type_name::<T>(),
                &offs_bytes[..preview]
            );
            eprintln!(
                "ConstVec::<{}> offsets_summary len={} data_len={} offsets={:?}",
                core::any::type_name::<T>(),
                offsets.len(),
                data.len(),
                &offsets[..core::cmp::min(offsets.len(), 8)]
            );
        }
        writer.write_all(&offs_bytes)?;
        writer.write_all(data)?;
        Ok(())
    }

    #[cfg(not(feature = "compact-len"))]
    fn write_fixed_offsets<W: Write>(
        writer: &mut W,
        offsets: &[u64],
        data: &[u8],
        _trace_enabled: bool,
    ) -> Result<(), ncore::Error> {
        ncore::note_fixed_offsets_emitted();
        let mut offs_bytes = Vec::with_capacity(offsets.len() * 8);
        for &off in offsets {
            offs_bytes.extend_from_slice(&off.to_le_bytes());
        }
        writer.write_all(&offs_bytes)?;
        writer.write_all(data)?;
        Ok(())
    }
}

impl<'a, T> norito::core::DecodeFromSlice<'a> for ConstVec<T>
where
    T: for<'de> norito::NoritoDeserialize<'de>,
    T: NoritoSerialize,
    T: norito::core::DecodeFromSlice<'a>,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (vec, used) = norito::core::decode_field_canonical::<Vec<T>>(bytes)?;
        Ok((Self::from(vec), used))
    }
}

impl<'a, T> NoritoDeserialize<'a> for ConstVec<T>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).unwrap_or_else(|err| {
            panic!(
                "ConstVec<{}> decode failed: {err:?}",
                core::any::type_name::<T>()
            )
        })
    }

    fn try_deserialize(archived: &'a ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        if let Some((_, len)) = ncore::payload_ctx()
            && len == 0
        {
            return Ok(ConstVec::new_empty());
        }

        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let ctx_len = ncore::payload_ctx().map(|(_, len)| len);
        let bytes_full = ncore::payload_slice_from_ptr(ptr)?;
        #[cfg(debug_assertions)]
        if norito::debug_trace_enabled() {
            eprintln!(
                "ConstVec::<{}>::try_deserialize ctx_len={ctx_len:?} bytes_full_len={}",
                core::any::type_name::<T>(),
                bytes_full.len()
            );
        }
        let bytes = ctx_len
            .and_then(|len| bytes_full.get(..len))
            .unwrap_or(bytes_full);
        let align = core::mem::align_of::<ncore::Archived<ConstVec<T>>>()
            .max(core::mem::align_of::<ncore::Archived<Vec<T>>>())
            .max(core::mem::align_of::<u128>());
        #[cfg(debug_assertions)]
        if norito::debug_trace_enabled() {
            eprintln!(
                "ConstVec::<{}>::try_deserialize align={} ptr={:#x}",
                core::any::type_name::<T>(),
                align,
                ptr as usize
            );
        }
        if align > 1 && !bytes.is_empty() && !(ptr as usize).is_multiple_of(align) {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::try_deserialize realigning archived payload align={} addr={:#x}",
                    core::any::type_name::<T>(),
                    align,
                    ptr as usize
                );
            }
            return decode_const_vec_realigned::<T>(bytes, align);
        }
        decode_const_vec_with_recovery::<T>(bytes)
    }
}

fn decode_const_vec_realigned<T>(bytes: &[u8], align: usize) -> Result<ConstVec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    let realigned = RealignedSlice::new(bytes, align)?;
    let aligned = realigned.as_slice();
    decode_const_vec_with_label::<T>(aligned, bytes, "realigned ", false)
}

fn decode_const_vec_with_recovery<T>(bytes: &[u8]) -> Result<ConstVec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    decode_const_vec_with_label::<T>(bytes, bytes, "", true)
}

#[cfg_attr(not(debug_assertions), allow(unused_variables))]
fn decode_const_vec_with_label<T>(
    decode_bytes: &[u8],
    fallback_bytes: &[u8],
    label: &str,
    log_fallback_diagnostics: bool,
) -> Result<ConstVec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    match decode_const_vec_from_slice::<T>(decode_bytes) {
        Ok(vec) => Ok(vec),
        Err(err) => {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::try_deserialize {label}decode failed: {err:?} (len={})",
                    core::any::type_name::<T>(),
                    decode_bytes.len()
                );
            }
            decode_const_vec_recover::<T>(err, fallback_bytes, log_fallback_diagnostics)
        }
    }
}

fn decode_const_vec_recover<T>(
    err: ncore::Error,
    bytes: &[u8],
    log_fallback_diagnostics: bool,
) -> Result<ConstVec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    match err {
        ncore::Error::Misaligned { .. } | ncore::Error::LengthMismatch => {
            match decode_const_vec_via_codec(bytes) {
                Ok(vec) => Ok(vec),
                Err(fallback) => {
                    if let Ok(manual) = decode_const_vec_manual_unpacked(bytes) {
                        return Ok(manual);
                    }
                    if let Some(instr_vec) = decode_instruction_vec_ignore_lengths::<T>(bytes) {
                        return Ok(instr_vec);
                    }
                    if log_fallback_diagnostics {
                        #[cfg(debug_assertions)]
                        if norito::debug_trace_enabled() {
                            eprintln!(
                                "ConstVec::<{}>::try_deserialize fallback decode failed: {fallback:?} (len={})",
                                core::any::type_name::<T>(),
                                bytes.len()
                            );
                            let _ = std::fs::write(
                                "/tmp/constvec_failure.bin",
                                &bytes[..core::cmp::min(bytes.len(), 4096)],
                            );
                            let _ = std::fs::write("/tmp/constvec_failure_full.bin", bytes);
                        }
                    }
                    Err(fallback)
                }
            }
        }
        other => {
            if let Some(instr_vec) = decode_instruction_vec_ignore_lengths::<T>(bytes) {
                return Ok(instr_vec);
            }
            Err(other)
        }
    }
}

fn decode_const_vec_from_slice<T>(bytes: &[u8]) -> Result<ConstVec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    let align = core::mem::align_of::<ncore::Archived<Vec<T>>>()
        .max(core::mem::align_of::<ncore::Archived<ConstVec<T>>>())
        .max(core::mem::align_of::<u128>());
    let aligned = align_payload_for::<T>(bytes, align)?;
    let decode_bytes = aligned.as_slice();
    if decode_bytes.len() > 8 {
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&decode_bytes[..8]);
        let declared = u64::from_le_bytes(len_bytes);
        if declared == 0 && decode_bytes.len() > 8 {
            return Err(ncore::Error::LengthMismatch);
        }
    }
    let (vec, used) = decode_vec_with_fallback::<T>(decode_bytes)?;
    #[cfg(debug_assertions)]
    if norito::debug_trace_enabled() {
        eprintln!(
            "ConstVec::<{}>::decode canonical used={} available={}",
            core::any::type_name::<T>(),
            used,
            bytes.len()
        );
    }
    if used > bytes.len() {
        #[cfg(debug_assertions)]
        if norito::debug_trace_enabled() {
            let _ = std::fs::write(
                "/tmp/constvec_failure.bin",
                &bytes[..core::cmp::min(bytes.len(), 4096)],
            );
        }
        return Err(ncore::Error::LengthMismatch);
    }
    Ok(ConstVec::from(vec))
}

fn decode_vec_with_fallback<T>(decode_bytes: &[u8]) -> Result<(Vec<T>, usize), ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    match ncore::decode_field_canonical::<Vec<T>>(decode_bytes) {
        Ok((vec, _used)) => {
            let used = reencode_and_verify::<T>(&vec, decode_bytes)?;
            Ok((vec, used))
        }
        Err(ncore::Error::LengthMismatch) => {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::decode canonical length mismatch available={}",
                    core::any::type_name::<T>(),
                    decode_bytes.len()
                );
            }
            let vec = decode_adaptive_with_streaming_fallback::<T>(decode_bytes)?;
            let used = match reencode_and_verify::<T>(&vec, decode_bytes) {
                Ok(used) => used,
                Err(ncore::Error::LengthMismatch) => {
                    // Accept encodings whose payload matches after fallback even if the cached
                    // length headers were clobbered. The caller already validated the body by
                    // deserialising each element.
                    decode_bytes.len()
                }
                Err(err) => return Err(err),
            };
            Ok((vec, used))
        }
        Err(ncore::Error::Misaligned {
            align: required,
            addr,
        }) => decode_misaligned_payload::<T>(decode_bytes, required, addr),
        Err(err) => {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                let _ = std::fs::write(
                    "/tmp/constvec_failure.bin",
                    &decode_bytes[..core::cmp::min(decode_bytes.len(), 4096)],
                );
                eprintln!(
                    "ConstVec::<{}>::decode canonical failed err={err:?} available={}",
                    core::any::type_name::<T>(),
                    decode_bytes.len()
                );
            }
            if decode_bytes.len() > 8 {
                let mut len_bytes = [0u8; 8];
                len_bytes.copy_from_slice(&decode_bytes[..8]);
                if u64::from_le_bytes(len_bytes) == 1 {
                    let elem_slice = &decode_bytes[8..];
                    if let Ok((elem, _used)) = ncore::decode_field_canonical::<T>(elem_slice) {
                        #[cfg(debug_assertions)]
                        if norito::debug_trace_enabled() {
                            eprintln!(
                                "ConstVec::<{}>::decode single-element manual fallback accepted len={}",
                                core::any::type_name::<T>(),
                                decode_bytes.len()
                            );
                        }
                        return Ok((vec![elem], decode_bytes.len()));
                    }
                }
            }
            if let Some(instr_vec) = decode_instruction_vec_ignore_lengths::<T>(decode_bytes) {
                return Ok((instr_vec.into_vec(), decode_bytes.len()));
            }
            Err(err)
        }
    }
}

#[allow(unused_variables)]
fn decode_misaligned_payload<T>(
    decode_bytes: &[u8],
    required: usize,
    addr: usize,
) -> Result<(Vec<T>, usize), ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    #[cfg(debug_assertions)]
    if norito::debug_trace_enabled() {
        eprintln!(
            "ConstVec::<{}>::decode canonical misaligned align={} addr={addr:#x} -- falling back to adaptive path",
            core::any::type_name::<T>(),
            required,
        );
    }
    let vec = decode_adaptive_with_streaming_fallback::<T>(decode_bytes)?;
    let used = reencode_and_verify::<T>(&vec, decode_bytes)?;
    Ok((vec, used))
}

fn decode_adaptive_with_streaming_fallback<T>(decode_bytes: &[u8]) -> Result<Vec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    match norito::codec::decode_adaptive::<Vec<T>>(decode_bytes) {
        Ok(vec) => Ok(vec),
        Err(ncore::Error::Misaligned { .. }) => {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::decode adaptive path misaligned; forcing sequential decode",
                    core::any::type_name::<T>()
                );
            }
            let seq_guard = ncore::SequentialOverrideGuard::enter();
            let result = norito::codec::decode_adaptive::<Vec<T>>(decode_bytes);
            drop(seq_guard);
            match result {
                Ok(vec) => Ok(vec),
                Err(ncore::Error::Misaligned { .. }) => {
                    decode_streaming_fallback::<T>(decode_bytes)
                }
                Err(err) => Err(err),
            }
        }
        Err(err) => {
            #[cfg(not(debug_assertions))]
            let _ = &err;
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::decode adaptive path failed err={err:?} available={}",
                    core::any::type_name::<T>(),
                    decode_bytes.len()
                );
            }
            decode_streaming_fallback::<T>(decode_bytes)
        }
    }
}

fn decode_streaming_fallback<T>(decode_bytes: &[u8]) -> Result<Vec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    #[cfg(debug_assertions)]
    if norito::debug_trace_enabled() {
        eprintln!(
            "ConstVec::<{}>::decode unpacked manual fallback engaged",
            core::any::type_name::<T>()
        );
    }
    let flags = ncore::default_encode_flags();
    let guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);
    let mut cursor = std::io::Cursor::new(decode_bytes);
    let decode_result = <Vec<T> as norito::codec::Decode>::decode(&mut cursor);
    drop(guard);
    match decode_result {
        Ok(vec) => {
            let consumed =
                usize::try_from(cursor.position()).map_err(|_| ncore::Error::LengthMismatch)?;
            if consumed > decode_bytes.len() {
                return Err(ncore::Error::LengthMismatch);
            }
            #[cfg(debug_assertions)]
            if consumed != decode_bytes.len() && norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::decode streaming fallback consumed {consumed} bytes of {}; accepting trailing payload",
                    core::any::type_name::<T>(),
                    decode_bytes.len()
                );
            }
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::decode streaming fallback succeeded",
                    core::any::type_name::<T>()
                );
            }
            Ok(vec)
        }
        Err(err) => {
            #[cfg(not(debug_assertions))]
            let _ = &err;
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::decode streaming fallback failed err={err:?}; trying manual unpacked decode",
                    core::any::type_name::<T>()
                );
            }
            let archived_vec = ncore::archived_from_slice_unchecked::<Vec<T>>(decode_bytes);
            let bytes = archived_vec.bytes();
            let _payload_guard = ncore::PayloadCtxGuard::enter(bytes);
            let vec = Vec::<T>::try_deserialize(archived_vec.as_ref())?;
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::decode archived fallback succeeded len={}",
                    core::any::type_name::<T>(),
                    vec.len()
                );
            }
            Ok(vec)
        }
    }
}

fn decode_const_vec_via_codec<T>(bytes: &[u8]) -> Result<ConstVec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    let vec = match decode_adaptive_with_streaming_fallback::<T>(bytes) {
        Ok(vec) => vec,
        Err(err) => {
            let packed_flags = ncore::header_flags::PACKED_SEQ;
            let vec_packed = {
                let _guard = ncore::DecodeFlagsGuard::enter_with_hint(packed_flags, packed_flags);
                decode_adaptive_with_streaming_fallback::<T>(bytes)
            };
            match vec_packed {
                Ok(vec) => vec,
                Err(packed_err) => {
                    #[cfg(not(debug_assertions))]
                    let _ = &err;
                    #[cfg(debug_assertions)]
                    if norito::debug_trace_enabled() {
                        eprintln!(
                            "ConstVec::<{}>::decode_const_vec_via_codec adaptive path failed err={err:?} len={}; packed_flags_err={packed_err:?}",
                            core::any::type_name::<T>(),
                            bytes.len()
                        );
                    }
                    return Err(packed_err);
                }
            }
        }
    };
    match reencode_and_verify::<T>(&vec, bytes) {
        Ok(_) => Ok(ConstVec::from(vec)),
        Err(ncore::Error::LengthMismatch) => {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::decode adaptive reencode mismatch accepted provided_len={}",
                    core::any::type_name::<T>(),
                    bytes.len()
                );
            }
            Ok(ConstVec::from(vec))
        }
        Err(err) => {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::decode_const_vec_via_codec reencode failed err={err:?} len={}",
                    core::any::type_name::<T>(),
                    bytes.len()
                );
            }
            Err(err)
        }
    }
}

fn decode_const_vec_manual_unpacked<T>(bytes: &[u8]) -> Result<ConstVec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    if bytes.len() < 8 {
        return Err(ncore::Error::LengthMismatch);
    }
    let mut len_bytes = [0u8; 8];
    len_bytes.copy_from_slice(&bytes[..8]);
    let count =
        usize::try_from(u64::from_le_bytes(len_bytes)).map_err(|_| ncore::Error::LengthMismatch)?;
    let mut offset = 8usize;
    let mut items = Vec::with_capacity(count);
    for idx in 0..count {
        if offset.checked_add(8).is_none_or(|end| end > bytes.len()) {
            return Err(ncore::Error::LengthMismatch);
        }
        len_bytes.copy_from_slice(&bytes[offset..offset + 8]);
        let elem_len = usize::try_from(u64::from_le_bytes(len_bytes))
            .map_err(|_| ncore::Error::LengthMismatch)?;
        let start = offset.checked_add(8).ok_or(ncore::Error::LengthMismatch)?;
        let end = start
            .checked_add(elem_len)
            .ok_or(ncore::Error::LengthMismatch)?;
        if end > bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let elem_bytes = &bytes[start..end];
        let item = decode_const_vec_manual_elem::<T>(elem_bytes, idx)?;
        items.push(item);
        offset = end;
    }
    #[cfg(debug_assertions)]
    if norito::debug_trace_enabled() {
        eprintln!(
            "ConstVec::<{}>::manual decode succeeded len={} items={}",
            core::any::type_name::<T>(),
            bytes.len(),
            items.len()
        );
    }
    Ok(ConstVec::from(items))
}

fn decode_const_vec_manual_elem<T>(elem_bytes: &[u8], idx: usize) -> Result<T, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    #[cfg(not(debug_assertions))]
    let _ = idx;

    match ncore::decode_field_canonical::<T>(elem_bytes) {
        Ok((value, _used)) => Ok(value),
        Err(err) => {
            #[cfg(debug_assertions)]
            if norito::debug_trace_enabled() {
                eprintln!(
                    "ConstVec::<{}>::manual elem decode failed idx={idx} len={} err={err:?}",
                    core::any::type_name::<T>(),
                    elem_bytes.len()
                );
            }
            Err(err)
        }
    }
}

fn decode_instruction_vec_ignore_lengths<T>(bytes: &[u8]) -> Option<ConstVec<T>>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    let type_name = core::any::type_name::<T>();
    if !type_name.contains("InstructionBox") {
        return None;
    }
    norito::codec::decode_adaptive::<Vec<T>>(bytes)
        .ok()
        .map(ConstVec::from)
}

fn reencode_and_verify<T>(vec: &[T], decode_bytes: &[u8]) -> Result<usize, ncore::Error>
where
    T: NoritoSerialize,
{
    let mut reencoded = Vec::new();
    {
        let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
        let _guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);
        ncore::write_seq_len(&mut reencoded, vec.len() as u64)?;
        if ncore::use_packed_seq() {
            #[cfg(debug_assertions)]
            let trace_enabled = norito::debug_trace_enabled();
            #[cfg(not(debug_assertions))]
            let trace_enabled = false;
            ConstVec::<T>::serialize_packed(vec, &mut reencoded, trace_enabled)?;
        } else {
            ConstVec::<T>::serialize_unpacked(vec, &mut reencoded)?;
        }
    }
    if reencoded.len() != decode_bytes.len() {
        #[cfg(debug_assertions)]
        if norito::debug_trace_enabled() {
            eprintln!(
                "ConstVec::<{}>::decode length mismatch reencoded={} provided={}",
                core::any::type_name::<T>(),
                reencoded.len(),
                decode_bytes.len()
            );
            let _ = std::fs::write(
                "/tmp/constvec_reencode.bin",
                &reencoded[..core::cmp::min(reencoded.len(), 4096)],
            );
        }
        return Err(ncore::Error::LengthMismatch);
    }
    if reencoded == decode_bytes {
        return Ok(reencoded.len());
    }
    if payload_matches_ignoring_vec_lengths(&reencoded, decode_bytes)? {
        return Ok(reencoded.len());
    }
    #[cfg(debug_assertions)]
    if norito::debug_trace_enabled() {
        let preview = reencoded
            .iter()
            .copied()
            .zip(decode_bytes.iter().copied())
            .take(16)
            .collect::<Vec<_>>();
        eprintln!(
            "ConstVec::<{}>::decode adaptive fallback diverged from canonical payload preview={preview:?}",
            core::any::type_name::<T>()
        );
    }
    Err(ncore::Error::LengthMismatch)
}

fn payload_matches_ignoring_vec_lengths(
    canonical: &[u8],
    provided: &[u8],
) -> Result<bool, ncore::Error> {
    if canonical.len() != provided.len() {
        return Ok(false);
    }
    if canonical.len() < 8 {
        return Err(ncore::Error::LengthMismatch);
    }
    if canonical[..8] != provided[..8] {
        return Ok(false);
    }
    let mut cursor = 8;
    while cursor < canonical.len() {
        if cursor + 8 > canonical.len() {
            return Ok(false);
        }
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&canonical[cursor..cursor + 8]);
        let elem_len = usize::try_from(u64::from_le_bytes(len_bytes))
            .map_err(|_| ncore::Error::LengthMismatch)?;
        let start = cursor + 8;
        let end = start
            .checked_add(elem_len)
            .ok_or(ncore::Error::LengthMismatch)?;
        if end > canonical.len() || end > provided.len() {
            return Ok(false);
        }
        if canonical[start..end] != provided[start..end] {
            return Ok(false);
        }
        cursor = end;
    }
    Ok(cursor == canonical.len())
}

#[cfg(test)]
fn decode_const_vec_manual<T>(
    archived: &ncore::Archived<ConstVec<T>>,
) -> Result<ConstVec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    let ptr = core::ptr::from_ref(archived).cast::<u8>();
    let bytes = ncore::payload_slice_from_ptr(ptr)?;
    decode_const_vec_from_slice::<T>(bytes)
}

impl<T: TypeId> TypeId for ConstVec<T> {
    fn id() -> String {
        format!("ConstVec<{}>", T::id())
    }
}
impl<T: IntoSchema> IntoSchema for ConstVec<T> {
    fn type_name() -> String {
        format!("Vec<{}>", T::type_name())
    }
    fn update_schema_map(map: &mut MetaMap) {
        if !map.contains_key::<Self>() {
            map.insert::<Self>(Metadata::Vec(VecMeta {
                ty: core::any::TypeId::of::<T>(),
            }));

            T::update_schema_map(map);
        }
    }
}

impl<'a, T> IntoIterator for &'a ConstVec<T> {
    type Item = &'a T;

    type IntoIter = <&'a [T] as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T> IntoIterator for ConstVec<T> {
    type Item = T;

    type IntoIter = <Vec<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.into_vec().into_iter()
    }
}

impl<T> FromIterator<T> for ConstVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let vec: Vec<T> = iter.into_iter().collect();
        Self::new(vec)
    }
}

/// Trait to extend `[T]` with a method to convert it to `ConstVec<T>` by analogy with `[T]::to_vec()`.
pub trait ToConstVec {
    /// The type of the items in the slice.
    type Item;

    /// Copies `self` into a new [`ConstVec`].
    fn to_const_vec(&self) -> ConstVec<Self::Item>;
}

impl<T: Clone> ToConstVec for [T] {
    type Item = T;

    fn to_const_vec(&self) -> ConstVec<Self::Item> {
        ConstVec::new(self)
    }
}

#[cfg(test)]
mod tests {
    use norito::{
        NoritoDeserialize, NoritoSerialize,
        codec::{self, Decode, Encode},
    };

    use super::{ConstVec, decode_const_vec_manual, ncore, reencode_and_verify};

    #[repr(transparent)]
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct InexactBytes(Vec<u8>);

    impl norito::NoritoSerialize for InexactBytes {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
            self.0.serialize(writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            self.0.encoded_len_hint()
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            None
        }
    }

    #[test]
    fn encode_decode_round_trip() {
        let bytes = vec![1u8, 2, 3, 4, 5];
        let encoded = ConstVec::<u8>::new(bytes.clone());
        let raw = encoded.encode();
        let mut cursor = raw.as_slice();
        let decoded = ConstVec::<u8>::decode(&mut cursor).unwrap();
        assert_eq!(bytes, decoded.into_vec());
    }

    #[test]
    fn const_vec_roundtrip_records_default_flags() {
        let value = ConstVec::from(vec![1_u8, 2, 3, 4, 5, 6]);
        let encoded = value.encode();
        let flags = codec::take_last_encode_flags().expect("encode flags");
        assert_eq!(
            flags,
            ncore::default_encode_flags(),
            "ConstVec should use canonical header flags"
        );
        let mut cursor = encoded.as_slice();
        let decoded = ConstVec::<u8>::decode(&mut cursor).expect("decode const vec");
        assert_eq!(decoded.as_ref(), value.as_ref());
    }

    #[test]
    fn norito_header_round_trip() {
        let bytes = vec![0xAAu8, 0xBB, 0xCC];
        let value = ConstVec::new(bytes.clone());

        let framed = norito::core::to_bytes(&value).expect("frame ConstVec");
        let archived = norito::core::from_bytes::<ConstVec<u8>>(&framed).expect("decode header");
        let decoded = ConstVec::<u8>::deserialize(archived);

        assert_eq!(decoded.into_vec(), bytes);
    }

    #[test]
    fn matches_vec_encoding() {
        let bytes = vec![1u8, 2, 3, 4, 5, 6, 7];
        let as_const = ConstVec::new(bytes.clone());
        let const_bytes = as_const.encode();

        let vec_bytes = bytes.encode();
        assert_eq!(
            const_bytes, vec_bytes,
            "ConstVec encoding diverges from Vec"
        );

        let mut cursor = const_bytes.as_slice();
        let roundtrip = Vec::<u8>::decode(&mut cursor).expect("decode vec");
        assert_eq!(roundtrip, bytes);
    }

    #[cfg(feature = "compact-len")]
    #[test]
    fn packed_seq_matches_vec_layout() {
        let flags = ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);

        let items = vec![vec![1u8, 2, 3], vec![4u8, 5]];
        let const_vec = ConstVec::from(items.clone());
        let const_bytes = const_vec.encode();

        let mut vec_bytes = Vec::new();
        NoritoSerialize::serialize(&items, &mut vec_bytes).expect("serialize Vec<Vec<u8>>");

        assert_eq!(
            const_bytes, vec_bytes,
            "ConstVec encoding diverges from Vec under packed-seq layout"
        );
    }

    #[test]
    fn matches_vec_encoding_canonical_flags() {
        let items = vec![vec![0xAAu8; 17], vec![0xBBu8; 9], vec![0xCCu8; 23]];
        let const_bytes = ConstVec::from(items.clone()).encode();
        let vec_bytes = items.encode();
        assert_eq!(
            const_bytes, vec_bytes,
            "ConstVec encoding diverges from Vec under canonical flags"
        );
    }

    #[test]
    fn nested_collections_roundtrip() {
        use std::collections::BTreeSet;

        let first = BTreeSet::from([1u32, 3, 5]);
        let second = BTreeSet::from([2u32, 4, 6, 8]);
        let third = BTreeSet::from([10u32]);
        let items = vec![first, second, third];

        let const_vec = ConstVec::from(items.clone());
        let encoded = const_vec.encode();
        let decoded = codec::decode_adaptive::<ConstVec<BTreeSet<u32>>>(&encoded)
            .expect("decode nested const vec");

        assert_eq!(decoded.into_vec(), items);
    }

    #[test]
    fn staged_path_handles_inexact_element_lengths() {
        let items = vec![
            InexactBytes(vec![1, 2, 3, 4]),
            InexactBytes((0u8..64).collect()),
            InexactBytes(vec![9; 17]),
        ];
        let const_vec = ConstVec::from(items.clone());
        let expected_plain =
            ConstVec::from(items.into_iter().map(|b| b.0).collect::<Vec<Vec<u8>>>());

        let encoded = const_vec.encode();
        let mut cursor = encoded.as_slice();
        let decoded = ConstVec::<Vec<u8>>::decode(&mut cursor).expect("decode const vec");

        assert_eq!(decoded, expected_plain);
    }

    #[test]
    fn encoded_len_exact_matches_packed_seq() {
        let value = ConstVec::from(vec![vec![1_u8, 2, 3], vec![4_u8, 5, 6, 7]]);
        let mut bytes = Vec::new();
        {
            let flags = ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN;
            let _guard = ncore::DecodeFlagsGuard::enter(flags);
            NoritoSerialize::serialize(&value, &mut bytes).expect("serialize const vec");
            assert_eq!(
                value.encoded_len_exact(),
                Some(bytes.len()),
                "ConstVec exact length should match packed layout payload"
            );
        }
    }

    #[test]
    fn compact_len_updates_encoded_lengths() {
        let flags = ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let value = ConstVec::from(vec![1_u8, 2_u8]);
        let mut bytes = Vec::new();
        NoritoSerialize::serialize(&value, &mut bytes).expect("serialize const vec");
        assert_eq!(value.encoded_len_exact(), Some(bytes.len()));
        assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
        assert_eq!(bytes.len(), 12);
    }

    #[test]
    fn packed_seq_roundtrip_alignment() {
        let flags = ncore::header_flags::PACKED_SEQ;
        let encode_guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);
        let items = ConstVec::from(vec![1_u128, 2, 3, 4, 5]);
        let encoded = items.encode();
        drop(encode_guard);

        let decode_guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);
        let decoded = norito::codec::decode_adaptive::<ConstVec<u128>>(&encoded)
            .expect("packed seq roundtrip");
        drop(decode_guard);

        assert_eq!(decoded.into_vec(), items.into_vec());
    }

    #[test]
    fn encoded_len_exact_matches_compat_offsets() {
        let value = ConstVec::from(vec![vec![0_u8; 2], vec![1_u8; 5]]);
        let mut bytes = Vec::new();
        {
            let _guard = ncore::DecodeFlagsGuard::enter(0);
            NoritoSerialize::serialize(&value, &mut bytes).expect("serialize compat const vec");
            assert!(
                value.encoded_len_exact().is_none(),
                "ConstVec exact length is no longer reported under canonical layout"
            );
        }
    }

    #[test]
    fn reencode_and_verify_respects_compact_len() {
        let flags = ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let value = ConstVec::from(vec![1_u8, 2_u8, 3_u8]);
        let mut bytes = Vec::new();
        NoritoSerialize::serialize(&value, &mut bytes).expect("serialize const vec");
        let len = reencode_and_verify(value.as_ref(), &bytes).expect("reencode const vec");
        assert_eq!(len, bytes.len());
    }

    #[test]
    fn corrupted_header_is_rejected() {
        let elements = vec![vec![1_u8, 2, 3], vec![4_u8, 5, 6, 7, 8]];
        let const_vec = ConstVec::from(elements.clone());
        let mut payload = const_vec.encode();
        let flags = codec::take_last_encode_flags().expect("encode flags captured");
        {
            let _guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);
            let (_, hdr) = ncore::read_seq_len_slice(&payload).expect("sequence header");
            payload[..hdr].fill(0);
        }
        // Append trailing bytes to mimic compat payloads that keep auxiliary data
        // after the packed span. Prior to the fix, this confused the manual
        // decoder into discarding the inferred offsets.
        payload.extend_from_slice(&[0xAAu8; 8]);

        let archived = ncore::archived_from_slice_unchecked::<ConstVec<Vec<u8>>>(&payload);
        let _payload_ctx = ncore::PayloadCtxGuard::enter_with_flags(archived.bytes(), flags);
        let decoded = decode_const_vec_manual::<Vec<u8>>(archived.as_ref());
        assert!(
            decoded.is_err(),
            "corrupted packed header should be rejected"
        );
    }

    #[test]
    fn invalid_element_fails_without_recursing() {
        use std::num::NonZeroU16;

        let value = ConstVec::from(vec![NonZeroU16::new(1).expect("nonzero")]);
        let mut bytes = value.encode();
        let len = bytes.len();
        bytes[len.saturating_sub(2)..].fill(0);

        let err = norito::codec::decode_adaptive::<ConstVec<NonZeroU16>>(&bytes)
            .expect_err("invalid element should be rejected");
        assert!(matches!(err, norito::Error::InvalidNonZero));
    }
}
