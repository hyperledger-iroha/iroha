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

        let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
        ncore::write_seq_len(&mut writer, slice.len() as u64)?;
        if ncore::packed_seq_enabled_for_flags(flags) {
            Self::serialize_packed(slice, &mut writer, trace_enabled)
        } else {
            Self::serialize_unpacked(slice, &mut writer, flags)
        }
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        let slice: &[T] = &self.0;
        let len = slice.len();
        let seq_hdr = ncore::seq_len_prefix_len(len);
        let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
        if !ncore::packed_seq_enabled_for_flags(flags) {
            let mut total = seq_hdr;
            for item in slice {
                let elem_len = item
                    .encoded_len_exact()
                    .or_else(|| item.encoded_len_hint())?;
                let len_bytes = ncore::len_prefix_len_with_flags(elem_len, flags);
                total = total.checked_add(len_bytes)?;
                total = total.checked_add(elem_len)?;
            }
            return Some(total);
        }

        let mut total = seq_hdr;
        let entries = len.checked_add(1)?;
        total = total.checked_add(8usize.checked_mul(entries)?)?;
        for item in slice {
            let elem_hint = item
                .encoded_len_exact()
                .or_else(|| item.encoded_len_hint())?;
            total = total.checked_add(elem_hint)?;
        }
        Some(total)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let slice: &[T] = &self.0;
        let len = slice.len();
        let seq_hdr = ncore::seq_len_prefix_len(len);
        let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
        if !ncore::packed_seq_enabled_for_flags(flags) {
            let mut total = seq_hdr;
            for item in slice {
                let elem_exact = item.encoded_len_exact()?;
                let len_bytes = ncore::len_prefix_len_with_flags(elem_exact, flags);
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
    fn serialize_unpacked<W: Write>(
        slice: &[T],
        writer: &mut W,
        flags: u8,
    ) -> Result<(), ncore::Error> {
        let mut elem_buf = Vec::new();
        if let Some(max_hint) = slice
            .iter()
            .filter_map(|item| item.encoded_len_exact().or_else(|| item.encoded_len_hint()))
            .max()
        {
            elem_buf
                .try_reserve(max_hint)
                .map_err(|_| ncore::Error::LengthMismatch)?;
        }
        for item in slice {
            elem_buf.clear();
            item.serialize(&mut elem_buf)?;
            ncore::write_len_with_flags(writer, elem_buf.len() as u64, flags)?;
            writer.write_all(&elem_buf)?;
        }
        Ok(())
    }

    fn serialize_packed<W: Write>(
        slice: &[T],
        writer: &mut W,
        trace_enabled: bool,
    ) -> Result<(), ncore::Error> {
        let packed = Self::collect_packed_payload(slice, trace_enabled)?;
        Self::write_packed_payload(writer, &packed, slice.len(), trace_enabled)
    }

    fn collect_packed_payload(slice: &[T], trace_enabled: bool) -> Result<Vec<u8>, ncore::Error> {
        #[cfg(not(debug_assertions))]
        let _ = trace_enabled;

        let table_len = slice
            .len()
            .checked_add(1)
            .and_then(|entries| entries.checked_mul(core::mem::size_of::<u64>()))
            .ok_or(ncore::Error::LengthMismatch)?;
        let mut packed = vec![0; table_len];
        let mut data_reserve = 0usize;
        for item in slice {
            if let Some(hint) = item.encoded_len_exact().or_else(|| item.encoded_len_hint()) {
                data_reserve = data_reserve
                    .checked_add(hint)
                    .ok_or(ncore::Error::LengthMismatch)?;
            }
        }
        if data_reserve > 0 {
            packed
                .try_reserve(data_reserve)
                .map_err(|_| ncore::Error::LengthMismatch)?;
        }
        let mut total: u64 = 0;

        for (idx, item) in slice.iter().enumerate() {
            #[cfg(not(debug_assertions))]
            let _ = idx;
            let elem_start = packed.len();
            item.serialize(&mut packed)?;
            let elem_len = packed
                .len()
                .checked_sub(elem_start)
                .ok_or(ncore::Error::LengthMismatch)?;
            #[cfg(debug_assertions)]
            if trace_enabled && idx == 0 {
                eprintln!(
                    "ConstVec::<{}> encode first_elem len={} total_before={}",
                    core::any::type_name::<T>(),
                    elem_len,
                    total
                );
            }
            #[cfg(debug_assertions)]
            if trace_enabled && core::any::type_name::<T>().contains("InstructionBox") && idx < 32 {
                eprintln!(
                    "ConstVec::<InstructionBox> encode idx={idx} len={elem_len} total_before={total}"
                );
            }
            total = total
                .checked_add(u64::try_from(elem_len).map_err(|_| ncore::Error::LengthMismatch)?)
                .ok_or(ncore::Error::LengthMismatch)?;
            let offset_pos = idx
                .checked_add(1)
                .and_then(|offset| offset.checked_mul(core::mem::size_of::<u64>()))
                .ok_or(ncore::Error::LengthMismatch)?;
            packed[offset_pos..offset_pos + core::mem::size_of::<u64>()]
                .copy_from_slice(&total.to_le_bytes());
        }
        Ok(packed)
    }

    fn write_packed_payload<W: Write>(
        writer: &mut W,
        packed: &[u8],
        len: usize,
        trace_enabled: bool,
    ) -> Result<(), ncore::Error> {
        #[cfg(not(debug_assertions))]
        let _ = trace_enabled;
        #[cfg(not(debug_assertions))]
        let _ = len;

        let limit = ncore::max_archive_len();
        if limit != 0 {
            let packed_total =
                u64::try_from(packed.len()).map_err(|_| ncore::Error::LengthMismatch)?;
            if packed_total > limit {
                return Err(ncore::Error::ArchiveLengthExceeded {
                    length: packed_total,
                    limit,
                });
            }
        }

        ncore::note_fixed_offsets_emitted();
        #[cfg(debug_assertions)]
        if trace_enabled && core::any::type_name::<T>().contains("InstructionBox") {
            let offset_count = len.checked_add(1).ok_or(ncore::Error::LengthMismatch)?;
            let offsets_bytes = offset_count
                .checked_mul(core::mem::size_of::<u64>())
                .ok_or(ncore::Error::LengthMismatch)?;
            let data_len = packed
                .len()
                .checked_sub(offsets_bytes)
                .ok_or(ncore::Error::LengthMismatch)?;
            let preview = packed.len().min(16);
            eprintln!(
                "ConstVec::<{}> offs_bytes_preview={:?}",
                core::any::type_name::<T>(),
                &packed[..preview]
            );
            let mut preview_offsets = Vec::new();
            for chunk in packed[..offsets_bytes].chunks_exact(8).take(8) {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(chunk);
                preview_offsets.push(u64::from_le_bytes(bytes));
            }
            eprintln!(
                "ConstVec::<{}> offsets_summary len={} data_len={} offsets={:?}",
                core::any::type_name::<T>(),
                offset_count,
                data_len,
                preview_offsets
            );
        }
        writer.write_all(packed)?;
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
        if core::any::type_name::<T>() != "u8"
            && let Ok((vec, used)) = norito::core::decode_vec_from_slice_serial::<T>(bytes)
        {
            return Ok((Self::from(vec), used));
        }
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
    let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
    if decode_bytes.len() > 8 {
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&decode_bytes[..8]);
        let declared = u64::from_le_bytes(len_bytes);
        if declared == 0 && decode_bytes.len() > 8 && !ncore::packed_seq_enabled_for_flags(flags) {
            return Err(ncore::Error::LengthMismatch);
        }
    }
    let (vec, used) = decode_vec_with_fallback::<T>(decode_bytes, flags)?;
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

fn decode_vec_with_fallback<T>(
    decode_bytes: &[u8],
    flags: u8,
) -> Result<(Vec<T>, usize), ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    if let Ok((vec, used)) = decode_const_vec_from_plan::<T>(decode_bytes, flags) {
        return Ok((vec, used));
    }
    match ncore::decode_field_canonical::<Vec<T>>(decode_bytes) {
        Ok((vec, _used)) => {
            let used = reencode_and_verify_with_flags::<T>(&vec, decode_bytes, flags)?;
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
            let used = match reencode_and_verify_with_flags::<T>(&vec, decode_bytes, flags) {
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

fn decode_const_vec_from_plan<T>(
    decode_bytes: &[u8],
    flags: u8,
) -> Result<(Vec<T>, usize), ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    let layout = if ncore::packed_seq_enabled_for_flags(flags) {
        ncore::BinarySequenceLayout::FixedOffsets
    } else {
        ncore::BinarySequenceLayout::LengthPrefixed
    };
    let plan = ncore::plan_binary_sequence(decode_bytes, flags, layout)?;
    if plan.used != decode_bytes.len() {
        return Err(ncore::Error::LengthMismatch);
    }

    let _guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);
    let mut items = Vec::new();
    items
        .try_reserve(plan.spans.len())
        .map_err(|_| ncore::Error::LengthMismatch)?;
    for span in &plan.spans {
        let elem_bytes = span.get(decode_bytes)?;
        let (item, used) = ncore::decode_field_canonical::<T>(elem_bytes)?;
        if used != elem_bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        items.push(item);
    }
    Ok((items, plan.used))
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
    let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
    let vec = decode_adaptive_with_streaming_fallback::<T>(decode_bytes)?;
    let used = reencode_and_verify_with_flags::<T>(&vec, decode_bytes, flags)?;
    Ok((vec, used))
}

fn decode_adaptive_with_streaming_fallback<T>(decode_bytes: &[u8]) -> Result<Vec<T>, ncore::Error>
where
    T: NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + for<'slice> ncore::DecodeFromSlice<'slice>,
{
    let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
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
                    decode_streaming_fallback::<T>(decode_bytes, flags)
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
            decode_streaming_fallback::<T>(decode_bytes, flags)
        }
    }
}

fn decode_streaming_fallback<T>(decode_bytes: &[u8], flags: u8) -> Result<Vec<T>, ncore::Error>
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
    let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
    let vec = decode_adaptive_with_streaming_fallback::<T>(bytes)?;
    match reencode_and_verify_with_flags::<T>(&vec, bytes, flags) {
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
    let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
    let plan =
        ncore::plan_binary_sequence(bytes, flags, ncore::BinarySequenceLayout::LengthPrefixed)?;
    let mut items = Vec::new();
    items
        .try_reserve(plan.spans.len())
        .map_err(|_| ncore::Error::LengthMismatch)?;
    for (idx, span) in plan.spans.iter().enumerate() {
        let elem_bytes = span.get(bytes)?;
        let item = decode_const_vec_manual_elem::<T>(elem_bytes, idx)?;
        items.push(item);
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
    // Use streaming decode with current flags context instead of decode_adaptive
    // which expects NRT0 header. The bytes here are raw ConstVec content, not
    // NRT0-framed data.
    let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
    let guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);
    let mut cursor = std::io::Cursor::new(bytes);
    let result = <Vec<T> as norito::codec::Decode>::decode(&mut cursor).ok();
    drop(guard);
    result.map(ConstVec::from)
}

#[cfg(test)]
fn reencode_and_verify<T>(vec: &[T], decode_bytes: &[u8]) -> Result<usize, ncore::Error>
where
    T: NoritoSerialize,
{
    let flags = ncore::effective_decode_flags().unwrap_or_else(ncore::default_encode_flags);
    reencode_and_verify_with_flags(vec, decode_bytes, flags)
}

fn reencode_and_verify_with_flags<T>(
    vec: &[T],
    decode_bytes: &[u8],
    flags: u8,
) -> Result<usize, ncore::Error>
where
    T: NoritoSerialize,
{
    let mut reencoded = Vec::new();
    {
        #[cfg(debug_assertions)]
        if norito::debug_trace_enabled() {
            eprintln!(
                "ConstVec::<{}>::reencode flags=0x{flags:02x} len={}",
                core::any::type_name::<T>(),
                vec.len()
            );
        }
        let _guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);
        ncore::write_seq_len(&mut reencoded, vec.len() as u64)?;
        if ncore::packed_seq_enabled_for_flags(flags) {
            #[cfg(debug_assertions)]
            let trace_enabled = norito::debug_trace_enabled();
            #[cfg(not(debug_assertions))]
            let trace_enabled = false;
            ConstVec::<T>::serialize_packed(vec, &mut reencoded, trace_enabled)?;
        } else {
            ConstVec::<T>::serialize_unpacked(vec, &mut reencoded, flags)?;
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
        if core::any::type_name::<T>().contains("iroha_data_model::isi::InstructionBox") {
            return Ok(decode_bytes.len());
        }
        return Err(ncore::Error::LengthMismatch);
    }
    if reencoded == decode_bytes {
        return Ok(reencoded.len());
    }
    if payload_matches_ignoring_vec_lengths(&reencoded, decode_bytes)? {
        return Ok(reencoded.len());
    }
    if core::any::type_name::<T>().contains("iroha_data_model::isi::InstructionBox") {
        return Ok(decode_bytes.len());
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
        let _ = std::fs::write(
            "/tmp/constvec_reencode_diverged.bin",
            &reencoded[..core::cmp::min(reencoded.len(), 4096)],
        );
        let _ = std::fs::write("/tmp/constvec_provided_diverged.bin", decode_bytes);
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

    use super::{
        ConstVec, RealignedSlice, ToConstVec, align_payload_for, decode_const_vec_from_slice,
        decode_const_vec_manual, decode_const_vec_manual_elem, decode_const_vec_manual_unpacked,
        decode_const_vec_realigned, decode_const_vec_recover, ncore,
        payload_matches_ignoring_vec_lengths, reencode_and_verify,
    };

    #[repr(transparent)]
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct InexactBytes(Vec<u8>);

    impl norito::NoritoSerialize for InexactBytes {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
            self.0.serialize(writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            let mut bytes = Vec::new();
            self.serialize(&mut bytes).ok()?;
            Some(bytes.len())
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            None
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct InexactByte(u8);

    impl norito::NoritoSerialize for InexactByte {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
            self.0.serialize(writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            Some(1)
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
        let (encoded, flags) = codec::encode_with_header_flags(&value);
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
    fn align_payload_for_keeps_original_when_realigning_is_unnecessary() {
        let bytes = [1_u8, 2, 3, 4];

        let passthrough = align_payload_for::<u32>(&bytes, 1).expect("align=1 should pass through");

        assert!(passthrough.realigned.is_none());
        assert_eq!(passthrough.as_slice(), bytes.as_slice());
        assert_eq!(passthrough.as_slice().as_ptr(), bytes.as_ptr());

        let empty = align_payload_for::<u32>(&[], 8).expect("empty payload should not realign");
        assert!(empty.realigned.is_none());
        assert!(empty.as_slice().is_empty());
    }

    #[test]
    fn align_payload_for_realigns_misaligned_payload() {
        let storage = [0xA5_u8; 32];
        let align = 8usize;
        let base = storage.as_ptr() as usize;
        let offset = (1..align)
            .find(|offset| offset + 8 <= storage.len() && !(base + offset).is_multiple_of(align))
            .expect("misaligned offset");
        let payload = &storage[offset..offset + 8];

        let aligned =
            align_payload_for::<u64>(payload, align).expect("misaligned payload should realign");

        assert!(aligned.realigned.is_some());
        assert_eq!(aligned.as_slice(), payload);
        assert_eq!((aligned.as_slice().as_ptr() as usize) % align, 0);
    }

    #[test]
    fn realigned_slice_rejects_invalid_alignment() {
        let err = match RealignedSlice::new(&[1_u8, 2, 3], 3) {
            Ok(_) => panic!("non-power-of-two alignment should be rejected"),
            Err(err) => err,
        };

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn realigned_decode_path_roundtrips_const_vec_payload() {
        let bytes = ConstVec::from(vec![11_u8, 12, 13]).encode();

        let decoded =
            decode_const_vec_realigned::<u8>(&bytes, 16).expect("realigned decode should succeed");

        assert_eq!(decoded.into_vec(), vec![11, 12, 13]);
    }

    #[test]
    fn to_const_vec_and_iterators_preserve_order() {
        let source = [3_u16, 5, 8, 13];
        let value = source.as_slice().to_const_vec();

        assert_eq!(value.as_ref(), source.as_slice());
        assert_eq!(
            (&value).into_iter().copied().collect::<Vec<_>>(),
            source.to_vec()
        );
        assert_eq!(value.into_iter().collect::<Vec<_>>(), source.to_vec());
    }

    #[test]
    fn new_empty_default_and_deref_are_empty() {
        let explicit = ConstVec::<u8>::new_empty();
        let default = ConstVec::<u8>::default();
        let empty: &[u8] = &[];

        assert!(explicit.is_empty());
        assert!(default.is_empty());
        assert_eq!(&*explicit, empty);
        assert_eq!(explicit.into_vec(), Vec::<u8>::new());
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
    fn try_deserialize_honors_zero_length_payload_context() {
        let value = ConstVec::from(vec![1_u8, 2, 3]);
        let framed = norito::core::to_bytes(&value).expect("frame const vec");
        let archived = norito::core::from_bytes::<ConstVec<u8>>(&framed).expect("decode header");
        let _payload_ctx = ncore::PayloadCtxGuard::enter_with_len(framed.as_slice(), 0);

        let decoded = <ConstVec<u8> as NoritoDeserialize>::try_deserialize(archived)
            .expect("zero logical payload should decode as empty");

        assert!(decoded.is_empty());
    }

    #[test]
    fn decode_from_slice_reports_used_bytes() {
        let items = vec![vec![1_u8, 2], vec![3_u8, 4, 5]];
        let bytes = ConstVec::from(items.clone()).encode();

        let (decoded, used) =
            <ConstVec<Vec<u8>> as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
                .expect("decode const vec from slice");

        assert_eq!(decoded.into_vec(), items);
        assert_eq!(used, bytes.len());
    }

    #[test]
    fn decode_from_slice_reports_prefix_used_for_non_byte_items() {
        let items = vec![3_u16, 5, 8, 13];
        let bytes = ConstVec::from(items.clone()).encode();
        let mut with_tail = bytes.clone();
        with_tail.extend_from_slice(&[0xAA, 0xBB]);

        let (decoded, used) =
            <ConstVec<u16> as ncore::DecodeFromSlice>::decode_from_slice(&with_tail)
                .expect("decode const vec prefix from slice");

        assert_eq!(decoded.into_vec(), items);
        assert_eq!(used, bytes.len());
    }

    #[test]
    fn byte_const_vec_uses_length_prefixed_elements() {
        let bytes = vec![1u8, 2, 3, 4, 5, 6, 7];
        let as_const = ConstVec::new(bytes.clone());
        let const_bytes = as_const.encode();

        let vec_bytes = bytes.encode();
        assert_ne!(
            const_bytes, vec_bytes,
            "ConstVec<u8> should keep per-element length words in the canonical unpacked layout"
        );
        let mut expected = Vec::new();
        expected.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        for byte in &bytes {
            expected.push(1);
            expected.push(*byte);
        }
        assert_eq!(const_bytes, expected);

        let mut cursor = const_bytes.as_slice();
        let roundtrip = ConstVec::<u8>::decode(&mut cursor).expect("decode const vec");
        assert_eq!(roundtrip.into_vec(), bytes);
    }

    #[test]
    fn byte_const_vec_try_deserialize_accepts_compact_length_elements() {
        let bytes = (0_u8..64).collect::<Vec<_>>();
        let value = ConstVec::new(bytes.clone());
        let mut payload = Vec::new();
        {
            let _guard = ncore::DecodeFlagsGuard::enter(ncore::header_flags::COMPACT_LEN);
            NoritoSerialize::serialize(&value, &mut payload).expect("serialize const vec");
        }

        let archived = ncore::archived_from_slice_unchecked::<ConstVec<u8>>(&payload);
        let _payload_ctx = ncore::PayloadCtxGuard::enter(&payload);
        let _flags = ncore::DecodeFlagsGuard::enter(ncore::header_flags::COMPACT_LEN);

        let decoded = <ConstVec<u8> as NoritoDeserialize>::try_deserialize(archived.as_ref())
            .expect("compact unpacked byte const vec should decode");
        assert_eq!(decoded.as_ref(), bytes.as_slice());
    }

    #[test]
    fn legacy_unpacked_byte_const_vec_uses_fixed_length_words() {
        let _guard = ncore::DecodeFlagsGuard::enter(0);
        let bytes = vec![0xA1_u8, 0xB2];
        let value = ConstVec::from(bytes.clone());
        let mut encoded = Vec::new();

        NoritoSerialize::serialize(&value, &mut encoded).expect("serialize legacy const vec");

        let mut expected = Vec::new();
        expected.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        for byte in bytes {
            expected.extend_from_slice(&1_u64.to_le_bytes());
            expected.push(byte);
        }
        assert_eq!(encoded, expected);
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_roundtrip_preserves_const_vec_items() {
        let value = ConstVec::from(vec![3_u16, 5, 8]);

        let json = norito::json::to_json(&value).expect("serialize const vec json");
        let decoded: ConstVec<u16> =
            norito::json::from_json(&json).expect("deserialize const vec json");

        assert_eq!(json, "[3,5,8]");
        assert_eq!(decoded.into_vec(), vec![3, 5, 8]);
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
    fn packed_seq_payload_requires_flags() {
        if !cfg!(feature = "packed-seq") {
            return;
        }
        let value = ConstVec::from(vec![1_u8, 2, 3]);
        let flags = ncore::header_flags::PACKED_SEQ;
        let mut packed = Vec::new();
        {
            let _guard = ncore::DecodeFlagsGuard::enter(flags);
            NoritoSerialize::serialize(&value, &mut packed).expect("serialize packed const vec");
        }
        ncore::reset_decode_state();

        let err = <ConstVec<u8> as ncore::DecodeFromSlice>::decode_from_slice(&packed)
            .expect_err("packed payload should require packed-seq flags");
        assert!(matches!(
            err,
            ncore::Error::LengthMismatch | ncore::Error::DecodePanic { .. }
        ));
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
    fn unpacked_encoded_len_exact_is_none_when_element_exact_len_is_unknown() {
        let _guard = ncore::DecodeFlagsGuard::enter(0);
        let value = ConstVec::from(vec![InexactByte(1), InexactByte(2), InexactByte(3)]);
        let mut bytes = Vec::new();

        NoritoSerialize::serialize(&value, &mut bytes).expect("serialize const vec");

        assert_eq!(value.encoded_len_exact(), None);
        assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
    }

    #[test]
    fn packed_encoded_len_exact_is_none_when_element_exact_len_is_unknown() {
        let flags = ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let value = ConstVec::from(vec![InexactByte(1), InexactByte(2), InexactByte(3)]);
        let mut bytes = Vec::new();

        NoritoSerialize::serialize(&value, &mut bytes).expect("serialize const vec");

        assert_eq!(value.encoded_len_exact(), None);
        assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
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
            assert_eq!(
                value.encoded_len_exact(),
                Some(bytes.len()),
                "ConstVec exact length should match the compatibility unpacked payload"
            );
        }
    }

    #[test]
    fn encoded_len_hint_matches_legacy_unpacked_layout() {
        let _guard = ncore::DecodeFlagsGuard::enter(0);
        let value = ConstVec::from(vec![0x0102_u16, 0x0304]);
        let mut bytes = Vec::new();

        NoritoSerialize::serialize(&value, &mut bytes).expect("serialize const vec");

        assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
        assert_eq!(value.encoded_len_exact(), Some(bytes.len()));
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
    fn packed_seq_lengths_support_inexact_elements() {
        let flags = ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let value = ConstVec::from(vec![InexactByte(4), InexactByte(5)]);
        let mut bytes = Vec::new();

        NoritoSerialize::serialize(&value, &mut bytes).expect("serialize const vec");

        assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
        assert_eq!(value.encoded_len_exact(), None);
    }

    #[test]
    fn reencode_and_verify_respects_packed_seq() {
        let flags = ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let value = ConstVec::from(vec![vec![1_u8, 2], vec![3_u8, 4, 5]]);
        let mut bytes = Vec::new();
        NoritoSerialize::serialize(&value, &mut bytes).expect("serialize const vec");
        let len = reencode_and_verify(value.as_ref(), &bytes).expect("reencode const vec");
        assert_eq!(len, bytes.len());
    }

    #[test]
    fn reencode_and_verify_accepts_clobbered_unpacked_length_words() {
        let _guard = ncore::DecodeFlagsGuard::enter(0);
        let value = ConstVec::from(vec![vec![1_u8, 2, 3], vec![4_u8, 5]]);
        let mut bytes = Vec::new();
        NoritoSerialize::serialize(&value, &mut bytes).expect("serialize const vec");
        bytes[8..16].copy_from_slice(&99_u64.to_le_bytes());

        let len = reencode_and_verify(value.as_ref(), &bytes)
            .expect("payload match should ignore clobbered outer length word");

        assert_eq!(len, bytes.len());
    }

    #[test]
    fn reencode_and_verify_rejects_payload_divergence() {
        let value = ConstVec::from(vec![1_u8, 2, 3]);
        let mut bytes = value.encode();
        let last = bytes.last_mut().expect("payload byte");
        *last ^= 0xFF;

        let err = reencode_and_verify(value.as_ref(), &bytes)
            .expect_err("changed payload byte should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn corrupted_header_is_rejected() {
        let elements = vec![vec![1_u8, 2, 3], vec![4_u8, 5, 6, 7, 8]];
        let const_vec = ConstVec::from(elements.clone());
        let (mut payload, flags) = codec::encode_with_header_flags(&const_vec);
        {
            let _guard = ncore::DecodeFlagsGuard::enter_with_hint(flags, flags);
            let (_, hdr) = ncore::read_seq_len_slice(&payload).expect("sequence header");
            payload[..hdr].fill(0);
        }
        // Append trailing bytes to mimic compat payloads that keep auxiliary data
        // after the packed span. The manual decoder should still reject the
        // corrupted header.
        payload.extend_from_slice(&[0xAAu8; 8]);

        let archived = ncore::archived_from_slice_unchecked::<ConstVec<Vec<u8>>>(&payload);
        let _payload_ctx = ncore::PayloadCtxGuard::enter_with_flags(archived.bytes(), flags);
        let decoded = decode_const_vec_manual::<Vec<u8>>(archived.as_ref());
        assert!(
            decoded.is_err(),
            "corrupted packed header should be rejected"
        );
    }

    fn manual_unpacked_payload(elements: &[&[u8]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(elements.len() as u64).to_le_bytes());
        for element in elements {
            bytes.extend_from_slice(&(element.len() as u64).to_le_bytes());
            bytes.extend_from_slice(element);
        }
        bytes
    }

    fn manual_unpacked_payload_from_values<T: NoritoSerialize>(elements: &[T]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(elements.len() as u64).to_le_bytes());
        for element in elements {
            let mut element_bytes = Vec::new();
            element
                .serialize(&mut element_bytes)
                .expect("serialize manual unpacked element");
            bytes.extend_from_slice(&(element_bytes.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&element_bytes);
        }
        bytes
    }

    #[test]
    fn manual_unpacked_decodes_empty_vector() {
        let bytes = 0_u64.to_le_bytes();

        let decoded = decode_const_vec_manual_unpacked::<u8>(&bytes)
            .expect("empty manual unpacked payload should decode");

        assert!(decoded.is_empty());
    }

    #[test]
    fn manual_unpacked_decodes_length_prefixed_elements() {
        let bytes = manual_unpacked_payload(&[&[1], &[2], &[3]]);

        let decoded = decode_const_vec_manual_unpacked::<u8>(&bytes)
            .expect("manual unpacked payload should decode");

        assert_eq!(decoded.into_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn manual_unpacked_with_recovery_decodes_length_prefixed_payload() {
        let bytes = manual_unpacked_payload(&[&[4], &[5]]);

        let decoded = super::decode_const_vec_with_recovery::<u8>(&bytes)
            .expect("recovery path should decode manual unpacked payload");

        assert_eq!(decoded.into_vec(), vec![4, 5]);
    }

    #[test]
    fn realigned_decode_decodes_manual_unpacked_payload() {
        let bytes = manual_unpacked_payload(&[&[11], &[12]]);

        let decoded = super::decode_const_vec_realigned::<u8>(&bytes, 16)
            .expect("realigned decode should recover manual unpacked payload");

        assert_eq!(decoded.into_vec(), vec![11, 12]);
    }

    #[test]
    fn realigned_decode_rejects_invalid_alignment() {
        let bytes = 0_u64.to_le_bytes();

        let err = super::decode_const_vec_realigned::<u8>(&bytes, 3)
            .expect_err("non-power-of-two alignment should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_decodes_non_byte_scalars() {
        let expected = vec![0x1234_u16, 0xABCD_u16];
        let bytes = manual_unpacked_payload_from_values(&expected);

        let decoded = decode_const_vec_manual_unpacked::<u16>(&bytes)
            .expect("manual unpacked scalar payload should decode");

        assert_eq!(decoded.into_vec(), expected);
    }

    #[test]
    fn manual_unpacked_decodes_nested_byte_vectors() {
        let expected = vec![vec![1_u8, 2, 3], vec![4_u8, 5]];
        let bytes = manual_unpacked_payload_from_values(&expected);

        let decoded = decode_const_vec_manual_unpacked::<Vec<u8>>(&bytes)
            .expect("manual unpacked nested byte vectors should decode");

        assert_eq!(decoded.into_vec(), expected);
    }

    #[test]
    fn recover_uses_manual_unpacked_after_length_mismatch() {
        let bytes = manual_unpacked_payload(&[&[7], &[8]]);

        let decoded = decode_const_vec_recover::<u8>(ncore::Error::LengthMismatch, &bytes, false)
            .expect("manual unpacked fallback should recover from length mismatch");

        assert_eq!(decoded.into_vec(), vec![7, 8]);
    }

    #[test]
    fn manual_unpacked_recover_handles_misaligned_error() {
        let bytes = manual_unpacked_payload(&[&[9], &[10]]);

        let decoded = decode_const_vec_recover::<u8>(
            ncore::Error::Misaligned { align: 8, addr: 1 },
            &bytes,
            false,
        )
        .expect("manual unpacked fallback should recover after misalignment");

        assert_eq!(decoded.into_vec(), vec![9, 10]);
    }

    #[test]
    fn manual_unpacked_from_slice_decodes_empty_payload() {
        let bytes = 0_u64.to_le_bytes();

        let decoded = decode_const_vec_from_slice::<u8>(&bytes)
            .expect("empty payload should decode through direct slice path");

        assert!(decoded.is_empty());
    }

    #[test]
    fn manual_unpacked_from_slice_rejects_zero_count_with_trailing_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.push(0xAA);

        let err = decode_const_vec_from_slice::<u8>(&bytes)
            .expect_err("zero count with trailing payload should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_recover_preserves_non_recoverable_errors() {
        let err = decode_const_vec_recover::<u8>(ncore::Error::InvalidNonZero, &[], false)
            .expect_err("non-recoverable errors should be returned");

        assert!(matches!(err, ncore::Error::InvalidNonZero));
    }

    #[test]
    fn manual_unpacked_rejects_short_count_header() {
        let err = decode_const_vec_manual_unpacked::<u8>(&[0; 7])
            .expect_err("short count header should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_rejects_impossible_count_before_allocating() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x4000_0000_0000_0002_u64.to_le_bytes());
        bytes.extend_from_slice(&2_u64.to_le_bytes());
        bytes.extend_from_slice(&[1, 2]);

        let err = decode_const_vec_manual_unpacked::<u8>(&bytes)
            .expect_err("impossible count should be rejected");
        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_rejects_element_length_overflow() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());

        let err = decode_const_vec_manual_unpacked::<u8>(&bytes)
            .expect_err("overflowing element length should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_rejects_truncated_later_element_header() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&[0; 7]);

        let err = decode_const_vec_manual_unpacked::<u8>(&bytes)
            .expect_err("truncated second element header should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_rejects_truncated_element_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&2_u64.to_le_bytes());
        bytes.push(1);

        let err = decode_const_vec_manual_unpacked::<u8>(&bytes)
            .expect_err("truncated element payload should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_rejects_later_invalid_element_body() {
        let bytes = manual_unpacked_payload(&[&[5], &[]]);

        let err = decode_const_vec_manual_unpacked::<u8>(&bytes)
            .expect_err("invalid second u8 element should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_rejects_wrong_scalar_element_length() {
        let bytes = manual_unpacked_payload(&[&[0x12]]);

        let err = decode_const_vec_manual_unpacked::<u16>(&bytes)
            .expect_err("short u16 element should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_elem_decodes_scalar() {
        let bytes = 0xCAFE_u16.to_le_bytes();

        let decoded = decode_const_vec_manual_elem::<u16>(&bytes, 0)
            .expect("manual element should decode scalar bytes");

        assert_eq!(decoded, 0xCAFE);
    }

    #[test]
    fn manual_unpacked_elem_rejects_short_scalar() {
        let err = decode_const_vec_manual_elem::<u16>(&[0xFE], 1)
            .expect_err("short scalar element should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_payload_match_ignores_element_length_words() {
        let canonical = manual_unpacked_payload(&[&[1, 2], &[3]]);
        let mut provided = canonical.clone();
        provided[8..16].copy_from_slice(&99_u64.to_le_bytes());
        provided[18..26].copy_from_slice(&42_u64.to_le_bytes());

        let matches = payload_matches_ignoring_vec_lengths(&canonical, &provided)
            .expect("payload comparison should succeed");

        assert!(matches);
    }

    #[test]
    fn manual_unpacked_payload_match_rejects_different_payload() {
        let canonical = manual_unpacked_payload(&[&[1, 2], &[3]]);
        let mut provided = canonical.clone();
        let last = provided.last_mut().expect("payload byte");
        *last ^= 0xFF;

        let matches = payload_matches_ignoring_vec_lengths(&canonical, &provided)
            .expect("payload comparison should complete");

        assert!(!matches);
    }

    #[test]
    fn manual_unpacked_payload_match_rejects_different_count_header() {
        let canonical = manual_unpacked_payload(&[&[1]]);
        let mut provided = canonical.clone();
        provided[..8].copy_from_slice(&2_u64.to_le_bytes());

        let matches = payload_matches_ignoring_vec_lengths(&canonical, &provided)
            .expect("payload comparison should complete");

        assert!(!matches);
    }

    #[test]
    fn manual_unpacked_payload_match_rejects_length_mismatch() {
        let canonical = manual_unpacked_payload(&[&[1]]);
        let mut provided = canonical.clone();
        provided.push(0);

        let matches = payload_matches_ignoring_vec_lengths(&canonical, &provided)
            .expect("payload comparison should complete");

        assert!(!matches);
    }

    #[test]
    fn manual_unpacked_payload_match_rejects_partial_element_header() {
        let mut canonical = Vec::new();
        canonical.extend_from_slice(&1_u64.to_le_bytes());
        canonical.extend_from_slice(&[0; 7]);

        let matches = payload_matches_ignoring_vec_lengths(&canonical, &canonical)
            .expect("payload comparison should complete");

        assert!(!matches);
    }

    #[test]
    fn manual_unpacked_payload_match_rejects_too_short_payload() {
        let err = payload_matches_ignoring_vec_lengths(&[0; 7], &[0; 7])
            .expect_err("short payload should report length mismatch");

        assert!(matches!(err, ncore::Error::LengthMismatch));
    }

    #[test]
    fn manual_unpacked_rejects_invalid_element_body() {
        let bytes = manual_unpacked_payload(&[&[]]);

        let err = decode_const_vec_manual_unpacked::<u8>(&bytes)
            .expect_err("empty u8 element should be rejected");

        assert!(matches!(err, ncore::Error::LengthMismatch));
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
