//! Heap ownership for exact fixed-size PGC proof arrays.

use std::ops::{Deref, DerefMut};

use norito::{
    Archived, DeserializePayload, SerializePayload,
    core::{DecodeFromSlice, Encoder},
};

/// One fixed wire array whose elements stay on the heap when its parent proof moves.
///
/// Range proofs nest inside selection proofs and complete payments. Keeping their
/// arrays inline multiplies the temporary storage required by each decoder frame.
/// This owner forwards the canonical array payload without a sequence count or
/// an owned-pointer envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FixedProofArray<T, const N: usize>(Box<[T; N]>);

impl<T, const N: usize> TryFrom<Vec<T>> for FixedProofArray<T, N> {
    type Error = Box<[T]>;

    fn try_from(values: Vec<T>) -> Result<Self, Self::Error> {
        values.into_boxed_slice().try_into().map(Self)
    }
}

impl<T, const N: usize> Deref for FixedProofArray<T, N> {
    type Target = [T; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, const N: usize> DerefMut for FixedProofArray<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: SerializePayload, const N: usize> SerializePayload for FixedProofArray<T, N> {
    fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), norito::Error> {
        self.0.as_ref().serialize(encoder)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.as_ref().encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.as_ref().encoded_len_exact()
    }
}

impl<'a, T, const N: usize> DecodeFromSlice<'a> for FixedProofArray<T, N>
where
    T: SerializePayload + for<'de> DeserializePayload<'de> + 'static,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::Error> {
        // The fixed shape determines the allocation; no untrusted count can
        // select its size. Charge the retained owner before allocating it.
        norito::core::reserve_decode_allocation(std::mem::size_of::<[T; N]>())?;
        let (values, used) = norito::core::decode_field_prefix::<[T; N]>(bytes)?;
        Ok((Self(Box::new(values)), used))
    }
}

impl<'a, T, const N: usize> DeserializePayload<'a> for FixedProofArray<T, N>
where
    T: SerializePayload + for<'de> DeserializePayload<'de> + 'static,
{
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("fixed PGC proof array decode")
    }

    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, norito::Error> {
        norito::core::reserve_decode_allocation(std::mem::size_of::<[T; N]>())?;
        // Archived::cast only changes the type of the byte-address marker. The
        // canonical array decoder validates its payload in the current context;
        // its caller owns the exact-field or packed-prefix consumption boundary.
        let values = <[T; N] as DeserializePayload>::try_deserialize(archived.cast())?;
        Ok(Self(Box::new(values)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::anonymous_pgc::{bootstrap, payment};

    #[test]
    fn fixed_owner_preserves_array_payload_and_length_hints() {
        let values = [0x1234_u16, 0x5678, 0x9ABC];
        let owned = FixedProofArray::<_, 3>::try_from(values.to_vec()).expect("exact shape");
        let encoded = norito::codec::encode_adaptive(&owned);
        assert_eq!(encoded, norito::codec::encode_adaptive(&values));
        assert_eq!(owned.encoded_len_hint(), values.encoded_len_hint());
        assert_eq!(owned.encoded_len_exact(), values.encoded_len_exact());
        assert_eq!(
            norito::codec::decode_exact_from_slice::<FixedProofArray<u16, 3>>(&encoded)
                .expect("fixed owner decode"),
            owned
        );
    }

    #[test]
    fn fixed_owner_rejects_wrong_shape_and_noncanonical_payloads() {
        assert_eq!(
            FixedProofArray::<u16, 3>::try_from(vec![1, 2])
                .expect_err("short shape")
                .len(),
            2
        );
        assert_eq!(
            FixedProofArray::<u16, 3>::try_from(vec![1, 2, 3, 4])
                .expect_err("long shape")
                .len(),
            4
        );
        let mut trailing = norito::codec::encode_adaptive(&[1_u16, 2, 3]);
        trailing.push(0);
        for bytes in [
            norito::codec::encode_adaptive(&[1_u16, 2]),
            norito::codec::encode_adaptive(&[1_u16, 2, 3, 4]),
            norito::codec::encode_adaptive(&vec![1_u16, 2, 3]),
            trailing,
        ] {
            assert!(
                norito::codec::decode_exact_from_slice::<FixedProofArray<u16, 3>>(&bytes).is_err(),
                "only the exact fixed-array payload is admitted"
            );
        }
    }

    #[test]
    fn fixed_owner_charges_its_allocation_before_decode() {
        let encoded = norito::codec::encode_adaptive(&[3_u8, 5, 8, 13]);
        let allocation_bytes = std::mem::size_of::<[u8; 4]>();
        // Canonical array decoding also charges its length-prefixed elements.
        // Isolate the retained owner's additional charge from that shared work.
        let limits = norito::DecodeLimits::new(0, encoded.len(), 0, usize::MAX, 16);
        let (baseline, baseline_usage) = norito::core::with_decode_limits_measured(limits, || {
            norito::core::decode_field_prefix::<[u8; 4]>(&encoded)
        });
        assert_eq!(baseline.expect("canonical array baseline").0, [3, 5, 8, 13]);
        let total_allocation_bytes = baseline_usage.total_allocated_bytes() + allocation_bytes;
        let limits = norito::DecodeLimits::new(0, encoded.len(), 0, total_allocation_bytes, 16);
        let (decoded, usage) = norito::core::with_decode_limits_measured(limits, || {
            FixedProofArray::<u8, 4>::decode_from_slice(&encoded)
        });
        assert_eq!(*decoded.expect("exact allocation budget").0, [3, 5, 8, 13]);
        assert_eq!(
            usage.total_allocated_bytes() - baseline_usage.total_allocated_bytes(),
            allocation_bytes
        );
        assert_eq!(
            usage.total_elements(),
            0,
            "a fixed array has no sequence count"
        );
        let limits = norito::DecodeLimits::new(0, encoded.len(), 0, total_allocation_bytes - 1, 16);
        let error = norito::core::with_decode_limits(limits, || {
            FixedProofArray::<u8, 4>::decode_from_slice(&encoded)
        })
        .expect_err("the retained array must fit its allocation budget");
        assert!(matches!(
            error,
            norito::Error::TotalAllocationExceeded { attempted, limit }
                if attempted == total_allocation_bytes as u64
                    && limit == (total_allocation_bytes - 1) as u64
        ));
    }

    #[test]
    fn fixed_owner_clone_and_mutation_keep_independent_arrays() {
        let original = FixedProofArray::<_, 3>::try_from(vec![1_u16, 2, 3]).expect("exact shape");
        let mut changed = original.clone();
        changed[1] = 7;
        assert_eq!(*original, [1, 2, 3]);
        assert_eq!(*changed, [1, 7, 3]);
        assert_ne!(original.as_ptr(), changed.as_ptr());
    }

    #[test]
    fn nested_range_proofs_keep_fixed_arrays_out_of_parent_stack_frames() {
        let pointer_bytes = std::mem::size_of::<usize>();
        assert_eq!(
            std::mem::size_of::<FixedProofArray<u8, 64>>(),
            pointer_bytes
        );
        assert_eq!(
            std::mem::size_of::<payment::PgcUnsignedRangeProofV1>(),
            3 * pointer_bytes
        );
        assert_eq!(
            std::mem::size_of::<bootstrap::PgcBootstrapUnsignedRangeProofV1>(),
            3 * pointer_bytes
        );
        assert!(std::mem::size_of::<payment::PgcSenderSelectionProofV1>() < 1024);
        assert!(std::mem::size_of::<payment::AnonymousPgcPaymentProofV1>() < 2048);
        assert!(std::mem::size_of::<bootstrap::PgcBootstrapAccountProofV1>() < 512);
    }
}
