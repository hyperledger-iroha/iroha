//! Bounded, zeroizing canonical staging for private-settlement hashes and signing preimages.

use iroha_crypto::{Hash, zeroize_value_for_confidential_discard};

pub(super) struct ConfidentialBytes {
    bytes: Vec<u8>,
    expected_len: usize,
}

impl ConfidentialBytes {
    fn with_capacity(expected_len: usize) -> Result<Self, norito::Error> {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(expected_len).map_err(|_| {
            norito::Error::Io(std::io::Error::other(
                "confidential byte buffer allocation failed",
            ))
        })?;
        Ok(Self {
            bytes,
            expected_len,
        })
    }

    pub(super) fn len(&self) -> usize {
        self.bytes.len()
    }

    pub(super) fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn extend_from_slice(&mut self, bytes: &[u8]) -> Result<(), norito::Error> {
        std::io::Write::write_all(self, bytes).map_err(norito::Error::Io)
    }

    fn into_vec(mut self) -> Vec<u8> {
        debug_assert_eq!(self.bytes.len(), self.expected_len);
        std::mem::take(&mut self.bytes)
    }

    fn zeroize_for_confidential_discard(&mut self) {
        zeroize_value_for_confidential_discard(&mut self.bytes);
        self.expected_len = 0;
    }
}

impl std::io::Write for ConfidentialBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next_len = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("confidential byte buffer is too large"))?;
        if next_len > self.expected_len {
            return Err(std::io::Error::other(
                "confidential byte buffer length changed during encoding",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for ConfidentialBytes {
    fn drop(&mut self) {
        #[cfg(test)]
        let had_nonzero_bytes = self.bytes.iter().any(|byte| *byte != 0);
        self.zeroize_for_confidential_discard();
        #[cfg(test)]
        if had_nonzero_bytes && self.bytes.is_empty() {
            CONFIDENTIAL_NONZERO_BUFFER_ZEROIZED_DROPS
                .with(|drops| drops.set(drops.get().saturating_add(1)));
        }
    }
}

#[cfg(test)]
std::thread_local! {
    pub(super) static CONFIDENTIAL_NONZERO_BUFFER_ZEROIZED_DROPS: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

pub(super) fn zeroize_confidential_vec_spare_capacity<T>(values: &mut Vec<T>) {
    zeroize_value_for_confidential_discard(values.spare_capacity_mut());
}

pub(super) fn encode_confidential_canonical<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<ConfidentialBytes, norito::Error> {
    // Consensus-facing `Encode` implementations must be deterministic. The
    // sizing pass exists only to reserve one hard-capped allocation; Norito's
    // canonical streaming pass then rejects length, checksum, or layout drift
    // between its own validation and output passes.
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let encoded_len = norito::core::encoded_frame_len(value)?;
    let mut encoded = ConfidentialBytes::with_capacity(encoded_len)?;
    norito::core::write_canonical_to_writer(value, &mut encoded)?;
    if encoded.len() != encoded_len {
        return Err(norito::Error::LengthMismatch);
    }
    Ok(encoded)
}

pub(super) fn private_settlement_signature_preimage<T: norito::NoritoSerialize>(
    domain: &[u8],
    body: &T,
    too_large_error: &'static str,
) -> Result<Vec<u8>, norito::Error> {
    let body = encode_confidential_canonical(body)?;
    let body_len = u64::try_from(body.len())
        .map_err(|_| norito::Error::Io(std::io::Error::other(too_large_error)))?;
    let preimage_len = domain
        .len()
        .checked_add(std::mem::size_of::<u64>())
        .and_then(|prefix_len| prefix_len.checked_add(body.len()))
        .ok_or_else(|| norito::Error::Io(std::io::Error::other(too_large_error)))?;
    let mut preimage = ConfidentialBytes::with_capacity(preimage_len)?;
    preimage.extend_from_slice(domain)?;
    preimage.extend_from_slice(&body_len.to_le_bytes())?;
    preimage.extend_from_slice(body.as_slice())?;
    Ok(preimage.into_vec())
}

pub(super) fn canonical_hash<T: norito::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<Hash, norito::Error> {
    let encoded = encode_confidential_canonical(value)?;
    let encoded_len = u64::try_from(encoded.len())
        .map_err(|_| norito::Error::Io(std::io::Error::other("canonical payload is too large")))?;
    Ok(Hash::new_from_chunks(&[
        domain,
        &encoded_len.to_le_bytes(),
        encoded.as_slice(),
    ]))
}
