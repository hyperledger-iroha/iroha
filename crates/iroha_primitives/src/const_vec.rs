//! A compact, heap-allocated container for immutable elements.
//!
//! [`ConstVec`] behaves similarly to [`Vec`] but omits the capacity field, making it cheaper to
//! store when the number of elements is known never to change. It is primarily used for byte
//! buffers or other data that is loaded once and then treated as read‑only for the remainder of the
//! program's lifetime.
use crate::ffi;
use core::ops::Deref;
use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, VecMeta};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};
use norito::{
    DeserializePayload, NoritoDeserialize, NoritoSerialize, SerializePayload, core as ncore,
};
use std::{boxed::Box, format, string::String, vec::Vec};
ffi::ffi_item! {
    /// Stores bytes that are not supposed to change during the runtime of the
    /// program in a compact way.
    ///
    /// Compared to `Vec<T>` this type omits the capacity field, reducing the
    /// memory footprint when the collection is immutable. The trade-off is that cloning requires
    /// duplicating the entire buffer because there is no reference counting.
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
impl<T: norito::NoritoSchema> norito::NoritoSchema for ConstVec<T> {
    fn nominal_name() -> String {
        norito::schema::identity::generic_name(
            "iroha_primitives::const_vec::ConstVec",
            &[T::nominal_name()],
        )
    }
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
    fn write_json_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        out.begin_container()?;
        out.push('[')?;
        for (index, item) in self.0.iter().enumerate() {
            if index != 0 {
                out.push(',')?;
            }
            JsonSerialize::json_serialize_to(item, out)?;
        }
        out.push(']')?;
        out.end_container();
        Ok(())
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
impl<T: NoritoSerialize> NoritoSerialize for ConstVec<T> {}
impl<T: SerializePayload> SerializePayload for ConstVec<T> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::write_element_sequence::<T, _>(writer, self.0.iter(), ncore::max_archive_len())
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
impl<'a, T> ncore::DecodeFromSlice<'a> for ConstVec<T>
where
    T: for<'de> DeserializePayload<'de> + SerializePayload,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let (items, used) = ncore::decode_element_sequence_from_slice_serial::<T>(bytes)?;
        Ok((Self::from(items), used))
    }
}

fn decode_const_vec_exact<T>(bytes: &[u8]) -> Result<ConstVec<T>, ncore::Error>
where
    T: for<'de> DeserializePayload<'de> + SerializePayload,
{
    let (items, used) = ncore::decode_element_sequence_from_slice_serial::<T>(bytes)?;
    if used != bytes.len() {
        return Err(ncore::Error::LengthMismatch);
    }
    Ok(ConstVec::from(items))
}

impl<T> NoritoDeserialize<'_> for ConstVec<T> where
    T: for<'de> NoritoDeserialize<'de> + SerializePayload
{
}
impl<'a, T> DeserializePayload<'a> for ConstVec<T>
where
    T: for<'de> DeserializePayload<'de> + SerializePayload,
{
    fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).unwrap_or_else(|error| {
            panic!(
                "ConstVec<{}> decode failed: {error:?}",
                core::any::type_name::<T>()
            )
        })
    }

    fn try_deserialize(archived: &'a ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        decode_const_vec_exact::<T>(bytes)
    }
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
    use super::{ConstVec, ToConstVec, decode_const_vec_exact, ncore};
    use norito::{
        DeserializePayload, NoritoSerialize, SerializePayload,
        codec::{self, Decode, Encode},
    };
    use std::cell::Cell;
    #[repr(transparent)]
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct InexactBytes(Vec<u8>);
    impl norito::NoritoSerialize for InexactBytes {}
    impl norito::SerializePayload for InexactBytes {
        fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
            self.0.serialize(writer)
        }
        fn encoded_len_hint(&self) -> Option<usize> {
            let mut bytes = Vec::new();
            ncore::serialize_to_buffer(self, &mut bytes).ok()?;
            Some(bytes.len())
        }
        fn encoded_len_exact(&self) -> Option<usize> {
            None
        }
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct InexactByte(u8);
    impl norito::NoritoSerialize for InexactByte {}
    impl norito::SerializePayload for InexactByte {
        fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
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
    fn packed_serialization_rejects_a_changed_counted_payload() {
        struct Growing(Cell<usize>);
        impl NoritoSerialize for Growing {}
        impl SerializePayload for Growing {
            fn serialize(
                &self,
                writer: &mut norito::core::Encoder<'_>,
            ) -> Result<(), ncore::Error> {
                let pass = self.0.get();
                self.0.set(pass + 1);
                writer.write_all(if pass == 0 { &[0x11] } else { &[0x11, 0x22] })?;
                Ok(())
            }
        }
        let values = ConstVec::new(vec![Growing(Cell::new(0))]);
        let mut encoded = Vec::new();
        let _guard = ncore::DecodeFlagsGuard::enter(ncore::header_flags::PACKED_SEQ);
        let error = ncore::serialize_to_buffer(&values, &mut encoded)
            .expect_err("a changed second pass must invalidate packed offsets");
        assert!(matches!(error, ncore::Error::LengthMismatch));
        assert_eq!(values[0].0.get(), 2);
    }
    #[test]
    fn nested_const_vec_measurement_visits_each_leaf_once_in_every_layout() {
        struct Leaf<'a>(&'a Cell<usize>);
        impl NoritoSerialize for Leaf<'_> {}
        impl SerializePayload for Leaf<'_> {
            fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
                self.0.set(self.0.get() + 1);
                writer.write_all(&[0xAB])?;
                Ok(())
            }
        }
        for flags in (0..=ncore::supported_header_flags())
            .filter(|flags| ncore::validate_header_flags(*flags).is_ok())
        {
            let _flags = ncore::DecodeFlagsGuard::enter(flags);
            let calls = Cell::new(0);
            let value = ConstVec::new(vec![ConstVec::new(vec![ConstVec::new(vec![Leaf(&calls)])])]);
            let measured = ncore::encoded_payload_len(&value).unwrap();
            assert_eq!(calls.get(), 1, "flags {flags:#x}");
            let mut bytes = Vec::new();
            ncore::serialize_to_buffer(&value, &mut bytes).unwrap();
            assert_eq!(measured, bytes.len());
        }
    }
    #[test]
    fn primitive_containers_accept_bare_only_children_in_every_layout() {
        use crate::{small::SmallVec, unique_vec::UniqueVec};

        // This leaf deliberately has neither a frame identity nor NoritoSerialize.
        #[derive(PartialEq)]
        struct BareLeaf(u16);
        impl SerializePayload for BareLeaf {
            fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
                self.0.serialize(writer)
            }
        }

        fn assert_payload(value: &dyn SerializePayload, expected: &dyn SerializePayload) {
            let mut expected_bytes = Vec::new();
            ncore::serialize_to_buffer(expected, &mut expected_bytes).unwrap();
            let mut bytes = Vec::new();
            ncore::serialize_to_buffer(value, &mut bytes).unwrap();
            assert_eq!(bytes, expected_bytes);
            assert_eq!(ncore::encoded_payload_len(value).unwrap(), bytes.len());
            let mut checked_bytes = Vec::new();
            ncore::serialize_to_writer_exact(value, &mut checked_bytes, bytes.len()).unwrap();
            assert_eq!(checked_bytes, bytes);
        }

        for flags in (0..=ncore::supported_header_flags())
            .filter(|flags| ncore::validate_header_flags(*flags).is_ok())
        {
            let _flags = ncore::DecodeFlagsGuard::enter(flags);
            for values in [Vec::new(), vec![0x1020_u16, 0x3040]] {
                let constant =
                    ConstVec::new(values.iter().copied().map(BareLeaf).collect::<Vec<_>>());
                assert_payload(&constant, &ConstVec::new(values.clone()));
                // ConstVec and UniqueVec retain the canonical element sequence layout.
                assert_payload(&constant, &values);
                let unique: UniqueVec<_> = values.iter().copied().map(BareLeaf).collect();
                assert_payload(&unique, &values);
                let small: SmallVec<[BareLeaf; 2]> = values.iter().copied().map(BareLeaf).collect();
                assert_payload(&small, &SmallVec::<[u16; 2]>::from(values.clone()));
                // SmallVec retains its distinct fixed count and field-prefix layout.
                let mut fixed = u64::try_from(values.len()).unwrap().to_le_bytes().to_vec();
                for value in &values {
                    fixed.extend_from_slice(&2_u64.to_le_bytes());
                    fixed.extend_from_slice(&value.to_le_bytes());
                }
                let mut small_bytes = Vec::new();
                ncore::serialize_to_buffer(&small, &mut small_bytes).unwrap();
                assert_eq!(small_bytes, fixed);
            }
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
    fn direct_decoder_rejects_truncated_payload_without_panicking() {
        let error = <ConstVec<u16> as ncore::DecodeFromSlice>::decode_from_slice(&[0])
            .expect_err("truncated vector payload must be rejected");
        assert!(matches!(error, ncore::Error::LengthMismatch));
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
    fn direct_decoder_is_independent_of_source_alignment() {
        let bytes = ConstVec::from(vec![11_u16, 12, 13]).encode();
        let mut storage = Vec::with_capacity(bytes.len() + 1);
        storage.push(0xA5);
        storage.extend_from_slice(&bytes);
        let source = &storage[1..];
        let (decoded, used) = <ConstVec<u16> as ncore::DecodeFromSlice>::decode_from_slice(source)
            .expect("unaligned source should decode through slice reads");
        assert_eq!(decoded.into_vec(), vec![11, 12, 13]);
        assert_eq!(used, bytes.len());
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
    fn try_deserialize_rejects_zero_length_payload_context() {
        let value = ConstVec::from(vec![1_u8, 2, 3]);
        let framed = norito::core::to_bytes(&value).expect("frame const vec");
        let archived = norito::core::from_bytes::<ConstVec<u8>>(&framed).expect("decode header");
        let _payload_ctx = ncore::PayloadCtxGuard::enter_with_len(framed.as_slice(), 0);
        let error = <ConstVec<u8> as DeserializePayload>::try_deserialize(archived)
            .expect_err("an empty logical payload cannot contain a sequence count");
        assert!(matches!(error, ncore::Error::LengthMismatch));
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
    fn byte_decode_from_slice_reports_prefix_used_bytes() {
        let items = vec![1_u8, 2, 3, 4, 5];
        let bytes = ConstVec::from(items.clone()).encode();
        let mut with_tail = bytes.clone();
        with_tail.extend_from_slice(&[0xAA, 0xBB]);
        let (decoded, used) =
            <ConstVec<u8> as ncore::DecodeFromSlice>::decode_from_slice(&with_tail)
                .expect("decode byte const vec prefix from slice");
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
            ncore::serialize_to_buffer(&value, &mut payload).expect("serialize const vec");
        }
        let archived =
            ncore::archived_from_slice::<ConstVec<u8>>(&payload).expect("archived const vec");
        let _payload_ctx = ncore::PayloadCtxGuard::enter(&payload);
        let _flags = ncore::DecodeFlagsGuard::enter(ncore::header_flags::COMPACT_LEN);
        let decoded = <ConstVec<u8> as DeserializePayload>::try_deserialize(archived.as_ref())
            .expect("compact unpacked byte const vec should decode");
        assert_eq!(decoded.as_ref(), bytes.as_slice());
    }
    #[test]
    fn fixed_v1_byte_const_vec_uses_fixed_length_words() {
        let _guard = ncore::DecodeFlagsGuard::enter(0);
        let bytes = vec![0xA1_u8, 0xB2];
        let value = ConstVec::from(bytes.clone());
        let mut encoded = Vec::new();
        ncore::serialize_to_buffer(&value, &mut encoded).expect("serialize fixed V1 const vec");
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
    #[test]
    fn packed_seq_matches_vec_layout() {
        let flags = ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let items = vec![vec![1u8, 2, 3], vec![4u8, 5]];
        let const_vec = ConstVec::from(items.clone());
        let mut const_bytes = Vec::new();
        ncore::serialize_to_buffer(&const_vec, &mut const_bytes)
            .expect("serialize ConstVec<Vec<u8>> with packed-seq flags");
        let mut vec_bytes = Vec::new();
        ncore::serialize_to_buffer(&items, &mut vec_bytes).expect("serialize Vec<Vec<u8>>");
        assert_eq!(
            const_bytes, vec_bytes,
            "ConstVec encoding diverges from Vec under packed-seq layout"
        );
    }
    #[test]
    fn packed_seq_payload_requires_flags() {
        let value = ConstVec::from(vec![1_u8, 2, 3]);
        let flags = ncore::header_flags::PACKED_SEQ;
        let mut packed = Vec::new();
        {
            let _guard = ncore::DecodeFlagsGuard::enter(flags);
            ncore::serialize_to_buffer(&value, &mut packed).expect("serialize packed const vec");
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
        ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize const vec");
        assert_eq!(value.encoded_len_exact(), None);
        assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
    }
    #[test]
    fn packed_encoded_len_exact_is_none_when_element_exact_len_is_unknown() {
        let flags = ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let value = ConstVec::from(vec![InexactByte(1), InexactByte(2), InexactByte(3)]);
        let mut bytes = Vec::new();
        ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize const vec");
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
            ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize const vec");
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
        ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize const vec");
        assert_eq!(value.encoded_len_exact(), Some(bytes.len()));
        assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
        assert_eq!(bytes.len(), 12);
    }
    #[test]
    fn packed_seq_roundtrip_alignment() {
        let flags = ncore::header_flags::PACKED_SEQ;
        let encode_guard = ncore::DecodeFlagsGuard::enter(flags);
        let items = ConstVec::from(vec![1_u128, 2, 3, 4, 5]);
        let encoded = items.encode();
        drop(encode_guard);
        let decode_guard = ncore::DecodeFlagsGuard::enter(flags);
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
            ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize compat const vec");
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
        ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize const vec");
        assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
        assert_eq!(value.encoded_len_exact(), Some(bytes.len()));
    }
    #[test]
    fn direct_decoder_respects_compact_len() {
        let flags = ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let expected = vec![1_u8, 2_u8, 3_u8];
        let value = ConstVec::from(expected.clone());
        let mut bytes = Vec::new();
        ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize const vec");
        let decoded = decode_const_vec_exact::<u8>(&bytes).expect("decode compact const vec");
        assert_eq!(decoded.into_vec(), expected);
    }
    #[test]
    fn packed_seq_lengths_support_inexact_elements() {
        let flags = ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let value = ConstVec::from(vec![InexactByte(4), InexactByte(5)]);
        let mut bytes = Vec::new();
        ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize const vec");
        assert_eq!(value.encoded_len_hint(), Some(bytes.len()));
        assert_eq!(value.encoded_len_exact(), None);
    }
    #[test]
    fn direct_decoder_respects_packed_seq() {
        let flags = ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let expected = vec![vec![1_u8, 2], vec![3_u8, 4, 5]];
        let value = ConstVec::from(expected.clone());
        let mut bytes = Vec::new();
        ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize const vec");
        let decoded = decode_const_vec_exact::<Vec<u8>>(&bytes).expect("decode packed const vec");
        assert_eq!(decoded.into_vec(), expected);
    }
    #[test]
    fn direct_decoder_rejects_clobbered_unpacked_length_words() {
        let _guard = ncore::DecodeFlagsGuard::enter(0);
        let value = ConstVec::from(vec![vec![1_u8, 2, 3], vec![4_u8, 5]]);
        let mut bytes = Vec::new();
        ncore::serialize_to_buffer(&value, &mut bytes).expect("serialize const vec");
        bytes[8..16].copy_from_slice(&99_u64.to_le_bytes());
        let error = decode_const_vec_exact::<Vec<u8>>(&bytes)
            .expect_err("a non-canonical element length must be rejected");
        assert!(matches!(error, ncore::Error::LengthMismatch));
    }
    #[test]
    fn corrupted_packed_header_is_rejected() {
        let flags = ncore::header_flags::PACKED_SEQ;
        let _guard = ncore::DecodeFlagsGuard::enter(flags);
        let value = ConstVec::from(vec![vec![1_u8, 2, 3], vec![4_u8, 5, 6]]);
        let mut payload = Vec::new();
        ncore::serialize_to_buffer(&value, &mut payload).expect("serialize const vec");
        let (_, header_len) = ncore::read_seq_len_slice(&payload).expect("sequence header");
        payload[..header_len].fill(0);
        let error = decode_const_vec_exact::<Vec<u8>>(&payload)
            .expect_err("a corrupt count must not be recovered");
        assert!(matches!(error, ncore::Error::LengthMismatch));
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
            ncore::serialize_to_buffer(element, &mut element_bytes)
                .expect("serialize manual unpacked element");
            bytes.extend_from_slice(&(element_bytes.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&element_bytes);
        }
        bytes
    }
    #[test]
    fn direct_decoder_decodes_empty_vector() {
        let bytes = 0_u64.to_le_bytes();
        let decoded = decode_const_vec_exact::<u8>(&bytes).expect("decode empty const vec");
        assert!(decoded.is_empty());
    }
    #[test]
    fn direct_decoder_decodes_length_prefixed_bytes() {
        let bytes = manual_unpacked_payload(&[&[1], &[2], &[3]]);
        let _guard = ncore::DecodeFlagsGuard::enter(0);
        let decoded = decode_const_vec_exact::<u8>(&bytes).expect("decode byte const vec");
        assert_eq!(decoded.into_vec(), vec![1, 2, 3]);
    }
    #[test]
    fn direct_decoder_decodes_non_byte_scalars() {
        let expected = vec![0x1234_u16, 0xABCD_u16];
        let bytes = manual_unpacked_payload_from_values(&expected);
        let _guard = ncore::DecodeFlagsGuard::enter(0);
        let decoded = decode_const_vec_exact::<u16>(&bytes).expect("decode scalar const vec");
        assert_eq!(decoded.into_vec(), expected);
    }
    #[test]
    fn direct_decoder_decodes_nested_byte_vectors() {
        let expected = vec![vec![1_u8, 2, 3], vec![4_u8, 5]];
        let bytes = manual_unpacked_payload_from_values(&expected);
        let _guard = ncore::DecodeFlagsGuard::enter(0);
        let decoded = decode_const_vec_exact::<Vec<u8>>(&bytes).expect("decode nested const vec");
        assert_eq!(decoded.into_vec(), expected);
    }
    #[test]
    fn exact_decoder_rejects_zero_count_with_trailing_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.push(0xAA);
        let error = decode_const_vec_exact::<u8>(&bytes)
            .expect_err("exact decode must reject trailing payload");
        assert!(matches!(error, ncore::Error::LengthMismatch));
    }
    #[test]
    fn prefix_decoder_reports_zero_count_boundary() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.push(0xAA);
        let (decoded, used) = <ConstVec<u8> as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
            .expect("prefix decode should stop at the sequence boundary");
        assert!(decoded.is_empty());
        assert_eq!(used, 8);
    }
    #[test]
    fn direct_decoder_rejects_short_count_header() {
        let error = decode_const_vec_exact::<u8>(&[0; 7])
            .expect_err("short count header should be rejected");
        assert!(matches!(error, ncore::Error::LengthMismatch));
    }
    #[test]
    fn direct_decoder_rejects_impossible_count_before_allocating() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x4000_0000_0000_0002_u64.to_le_bytes());
        bytes.extend_from_slice(&2_u64.to_le_bytes());
        bytes.extend_from_slice(&[1, 2]);
        let error =
            decode_const_vec_exact::<u8>(&bytes).expect_err("impossible count should be rejected");
        assert!(matches!(error, ncore::Error::LengthMismatch));
    }
    #[test]
    fn direct_decoder_rejects_element_length_overflow() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        let error = decode_const_vec_exact::<u8>(&bytes)
            .expect_err("overflowing element length should be rejected");
        assert!(matches!(error, ncore::Error::LengthMismatch));
    }
    #[test]
    fn direct_decoder_rejects_truncated_later_element_header() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&[0; 7]);
        let error = decode_const_vec_exact::<u8>(&bytes)
            .expect_err("truncated second element header should be rejected");
        assert!(matches!(error, ncore::Error::LengthMismatch));
    }
    #[test]
    fn direct_decoder_rejects_truncated_element_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&2_u64.to_le_bytes());
        bytes.push(1);
        let error = decode_const_vec_exact::<u8>(&bytes)
            .expect_err("truncated element payload should be rejected");
        assert!(matches!(error, ncore::Error::LengthMismatch));
    }
    #[test]
    fn direct_decoder_rejects_invalid_later_element_body() {
        let bytes = manual_unpacked_payload(&[&[5], &[]]);
        let error = decode_const_vec_exact::<u8>(&bytes)
            .expect_err("invalid second u8 element should be rejected");
        assert!(matches!(error, ncore::Error::LengthMismatch));
    }
    #[test]
    fn direct_decoder_rejects_wrong_scalar_element_length() {
        let bytes = manual_unpacked_payload(&[&[0x12]]);
        let error = decode_const_vec_exact::<u16>(&bytes)
            .expect_err("short u16 element should be rejected");
        assert!(matches!(error, ncore::Error::LengthMismatch));
    }
    #[test]
    fn structural_error_is_not_masked_by_recharging_the_element_budget() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.push(1);
        let limits =
            ncore::DecodeLimits::new(2, usize::MAX, 2, usize::MAX, ncore::MAX_VALUE_NESTING_DEPTH);
        let error = ncore::with_decode_limits(limits, || decode_const_vec_exact::<u8>(&bytes))
            .expect_err("the missing second element must remain the reported error");
        assert!(
            matches!(error, ncore::Error::LengthMismatch),
            "one-pass decoding must preserve the initial structural error: {error:?}"
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
