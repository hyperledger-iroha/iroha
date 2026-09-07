//! Bounded borrowed-field decoding for Kura's fixed-V1 Kagemusha sidecars.
//!
//! The durable encoding is unchanged. Repeated casting bindings are decoded
//! from borrowed field slices, avoiding whole-vector and whole-binding copies
//! and their cumulative allocation charges. Owned vectors and all allocations
//! inside ordinary Norito field decoders remain charged before allocation.

use super::{
    KagemushaFinalitySidecarV1, MAX_KAGEMUSHA_FINALITY_SIDECAR_BYTES,
    ParliamentTimedOvnCastingContextBindingV1, StagedKagemushaFinalitySidecarV1,
    kagemusha_finality_decode_limits,
};
use norito::{Error, NoritoDeserialize, NoritoSerialize};

// Encode::encode fixes V1 bare payloads to these flags, independent of an
// ambient header/layout context. A codec-default change requires a reviewed
// sidecar version/layout change rather than heuristic decoding here.
const V1_FLAGS: u8 = norito::core::header_flags::COMPACT_LEN;
const _: () = assert!(norito::core::default_encode_flags() == V1_FLAGS);
// Envelope -> binding vector -> binding are parsed without recursion. Reserve
// those three levels when limiting the remaining ordinary Norito field graph.
const BORROWED_CONTAINER_DEPTH: usize = 3;

pub(super) fn decode_staged(bytes: &[u8]) -> Result<StagedKagemushaFinalitySidecarV1, Error> {
    decode_sidecar(bytes, |fields| {
        Ok(StagedKagemushaFinalitySidecarV1 {
            version: fields.version()?,
            height: fields.decode()?,
            block_hash: fields.decode()?,
            ordinary_writes_root: fields.decode()?,
            post_state_root: fields.decode()?,
            validation_fee_policy_witness: fields.decode()?,
            parliament_timed_ovn_casting_witness: fields.decode()?,
            parliament_timed_ovn_casting_bindings: fields.bindings()?,
            kagemusha_reserve_receipts: fields.decode()?,
        })
    })
}

pub(super) fn decode_finalized(bytes: &[u8]) -> Result<KagemushaFinalitySidecarV1, Error> {
    decode_sidecar(bytes, |fields| {
        Ok(KagemushaFinalitySidecarV1 {
            version: fields.version()?,
            height: fields.decode()?,
            block_hash: fields.decode()?,
            ordinary_writes_root: fields.decode()?,
            post_state_root: fields.decode()?,
            finality_artifact_hash: fields.decode()?,
            validation_fee_policy_witness: fields.decode()?,
            parliament_timed_ovn_casting_witness: fields.decode()?,
            parliament_timed_ovn_casting_bindings: fields.bindings()?,
            kagemusha_reserve_receipts: fields.decode()?,
        })
    })
}

fn decode_sidecar<T>(
    bytes: &[u8],
    decode: impl FnOnce(&mut Fields<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    if bytes.len() > MAX_KAGEMUSHA_FINALITY_SIDECAR_BYTES {
        return Err(Error::FieldLengthExceeded {
            length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            limit: MAX_KAGEMUSHA_FINALITY_SIDECAR_BYTES as u64,
        });
    }
    let limits = kagemusha_finality_decode_limits(bytes.len());
    let nested_limits = norito::DecodeLimits::new(
        limits.max_sequence_elements(),
        limits.max_field_bytes(),
        limits.max_total_elements(),
        limits.max_total_allocated_bytes(),
        limits.max_nesting_depth() - BORROWED_CONTAINER_DEPTH,
    );
    // Neither guard resets an enclosing budget. The fixed layout and borrowed
    // payload contexts are restored even if a nested decoder returns an error.
    let _flags = norito::core::DecodeFlagsGuard::enter(V1_FLAGS);
    let _payload = norito::core::PayloadCtxGuard::enter_with_flags(bytes, V1_FLAGS);
    norito::with_decode_limits(nested_limits, || {
        let mut fields = Fields { remaining: bytes };
        let value = decode(&mut fields)?;
        fields.finish()?;
        Ok(value)
    })
}

/// A fixed-V1 sequence of length-delimited borrowed fields; no heap storage.
struct Fields<'a> {
    remaining: &'a [u8],
}

impl<'a> Fields<'a> {
    fn take(&mut self) -> Result<&'a [u8], Error> {
        // This checks the canonical length prefix and field limit without
        // charging a copy of the bytes: the returned slice remains borrowed.
        let (len, prefix_len) = norito::core::inspect_len_from_slice(self.remaining)?;
        let end = prefix_len.checked_add(len).ok_or(Error::LengthMismatch)?;
        let field = self
            .remaining
            .get(prefix_len..end)
            .ok_or(Error::LengthMismatch)?;
        self.remaining = &self.remaining[end..];
        Ok(field)
    }

    fn decode<T>(&mut self) -> Result<T, Error>
    where
        T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
    {
        let field = self.take()?;
        let (value, used) = norito::core::decode_field_canonical(field)?;
        if used != field.len() {
            return Err(Error::LengthMismatch);
        }
        Ok(value)
    }

    fn version(&mut self) -> Result<u16, Error> {
        let version = self.decode()?;
        if version != KagemushaFinalitySidecarV1::VERSION {
            return Err(Error::UnsupportedFeature(
                "Kagemusha finality sidecar version",
            ));
        }
        Ok(version)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        // Derive encodes a byte-array struct field as one raw N-byte field,
        // unlike a standalone [u8; N] value's element-framed representation.
        self.take()?.try_into().map_err(|_| Error::LengthMismatch)
    }

    fn finish(&self) -> Result<(), Error> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(Error::LengthMismatch)
        }
    }

    fn bindings(&mut self) -> Result<Vec<ParliamentTimedOvnCastingContextBindingV1>, Error> {
        let bytes = self.take()?;
        // Charges sequence count and cumulative element metadata before any
        // reservation; lengths above the protocol maximum fail here.
        let (count, prefix_len) = norito::core::read_seq_len_slice(bytes)?;
        let body = bytes.get(prefix_len..).ok_or(Error::LengthMismatch)?;
        // Validate every borrowed span before reserving the output, so a tiny
        // payload cannot manufacture a large vector through its count alone.
        let mut spans = Fields { remaining: body };
        for _ in 0..count {
            spans.take()?;
        }
        spans.finish()?;
        let allocation = count
            .checked_mul(std::mem::size_of::<ParliamentTimedOvnCastingContextBindingV1>())
            .ok_or(Error::LengthMismatch)?;
        norito::core::reserve_decode_allocation(allocation)?;
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed {
                bytes: u64::try_from(allocation).unwrap_or(u64::MAX),
            })?;
        let mut elements = Fields { remaining: body };
        for _ in 0..count {
            bindings.push(decode_binding(elements.take()?)?);
        }
        elements.finish()?;
        Ok(bindings)
    }
}

fn decode_binding(bytes: &[u8]) -> Result<ParliamentTimedOvnCastingContextBindingV1, Error> {
    let mut fields = Fields { remaining: bytes };
    let value = ParliamentTimedOvnCastingContextBindingV1 {
        version: fields.decode()?,
        evaluated_height: fields.decode()?,
        phase: fields.decode()?,
        network_id: fields.array()?,
        proposal_content_id: fields.decode()?,
        governance_attempt_id: fields.decode()?,
        body_instance_id: fields.decode()?,
        ballot_attempt_id: fields.decode()?,
        parameter_hash: fields.array()?,
        tle_key_session_id: fields.decode()?,
        tle_key_transcript_hash: fields.array()?,
        tle_master_public_key: fields.array()?,
        registration_opened_at_finalized_height: fields.decode()?,
        registration_close_height: fields.decode()?,
        survivor_freeze_height: fields.decode()?,
        commitment_close_height: fields.decode()?,
        target_finalized_height: fields.decode()?,
        registration_corpus: fields.decode()?,
        survivor_count: fields.decode()?,
        dropout_root: fields.decode()?,
        release_identity: fields.decode()?,
    };
    fields.finish()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_fields_require_canonical_lengths_values_and_complete_consumption() {
        let _flags = norito::core::DecodeFlagsGuard::enter(V1_FLAGS);
        let mut fields = Fields {
            remaining: &[2, 1, 0, 3, 4, 5, 6],
        };
        assert_eq!(fields.version().expect("exact V1 version"), 1);
        assert!(fields.finish().is_err());
        assert_eq!(fields.array::<3>().expect("raw fixed array"), [4, 5, 6]);
        fields.finish().expect("complete field sequence");
        for bytes in [&[][..], &[2, 1], &[0x82, 0, 1, 0], &[0xFF; 10]] {
            assert!(Fields { remaining: bytes }.take().is_err());
        }
        assert!(
            Fields {
                remaining: &[2, 2, 0]
            }
            .version()
            .is_err()
        );
        assert!(Fields { remaining: &[1, 0] }.decode::<u16>().is_err());
        assert!(
            Fields {
                remaining: &[3, 1, 0, 0]
            }
            .decode::<u16>()
            .is_err()
        );
        assert!(
            Fields {
                remaining: &[2, 1, 0]
            }
            .array::<3>()
            .is_err()
        );
    }

    #[test]
    fn borrowed_binding_sequence_preserves_outer_element_allocation_and_field_limits() {
        let empty = [8, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(
            decode_sidecar(&empty, |fields| fields.bindings())
                .expect("empty vector")
                .is_empty()
        );
        let mut oversized = empty;
        oversized[1..].copy_from_slice(&1_001_u64.to_le_bytes());
        assert!(matches!(
            decode_sidecar(&oversized, |fields| fields.bindings()),
            Err(Error::SequenceLengthExceeded {
                length: 1_001,
                limit: 1_000
            })
        ));
        let mut missing = empty;
        missing[1..].copy_from_slice(&1_u64.to_le_bytes());
        assert!(matches!(
            decode_sidecar(&missing, |fields| fields.bindings()),
            Err(Error::LengthMismatch)
        ));
        let empty_element = [9, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        let allocation_limit = norito::DecodeLimits::new(1_000, 100, 1_000, 1, 32);
        assert!(matches!(
            norito::with_decode_limits(allocation_limit, || decode_sidecar(
                &empty_element,
                |fields| fields.bindings()
            )),
            Err(Error::TotalAllocationExceeded { limit: 1, .. })
        ));
        let element_limit = norito::DecodeLimits::new(1_000, 100, 0, 1_000_000, 32);
        assert!(matches!(
            norito::with_decode_limits(element_limit, || decode_sidecar(
                &empty_element,
                |fields| fields.bindings()
            )),
            Err(Error::TotalElementsExceeded { limit: 0, .. })
        ));
        let field_limit = norito::DecodeLimits::new(1_000, 0, 1_000, 1_000_000, 32);
        assert!(matches!(
            norito::with_decode_limits(field_limit, || decode_sidecar(&empty, |fields| fields
                .bindings())),
            Err(Error::FieldLengthExceeded { limit: 0, .. })
        ));
        let depth_limit = norito::DecodeLimits::new(1_000, 100, 1_000, 1_000_000, 0);
        assert!(matches!(
            norito::with_decode_limits(depth_limit, || decode_sidecar(&[2, 1, 0], |fields| fields
                .version())),
            Err(Error::NestingDepthExceeded { limit: 0, .. })
        ));
        let too_large = vec![0; MAX_KAGEMUSHA_FINALITY_SIDECAR_BYTES + 1];
        assert!(matches!(
            decode_sidecar(&too_large, |fields| fields.bindings()),
            Err(Error::FieldLengthExceeded { .. })
        ));
    }
}
