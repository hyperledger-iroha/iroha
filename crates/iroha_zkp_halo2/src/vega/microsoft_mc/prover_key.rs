//! Exact bounded Microsoft Vega-MC proving-key codec and verifier-key binding.
//!
//! The field order matches `VegaMcProverKey` at pinned Microsoft commit
//! `c0ee259053cd12eaf43ed71b5cde375452b3ee4d`. This is only an artifact
//! boundary: it deliberately does not generate setup material and it does not
//! make the Microsoft prover available.
use super::{
    verifier_key::{
        HyraxKeyWire, McVerifierKeyWire, MultiRoundShapeWire, RegularShapeWire, SplitShapeWire,
        checked_sum, hyrax_key_encoded_len, multi_round_shape_encoded_len, read_hyrax_key,
        read_multi_round_shape, read_regular_shape, read_split_shape, regular_shape_encoded_len,
        split_shape_encoded_len, validate_prover_components, write_hyrax_key,
        write_multi_round_shape, write_regular_shape, write_split_shape,
    },
    wire::{McCodecError, Reader, try_vec_with_capacity},
};

const MAX_PROVER_KEY_BYTES: usize = 512 * 1024 * 1024;
const VERIFIER_DIGEST_BYTES: usize = 32;

/// Exact fixed-little-endian representation of Microsoft's `VegaMcProverKey`.
///
/// The owner is intentionally not `Clone`: a governed Figure 9 proving key can
/// contain hundreds of MiB of matrices and must not acquire implicit copies.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct McProverKeyWire {
    pub(super) application_key: HyraxKeyWire,
    pub(super) step_shape: SplitShapeWire,
    pub(super) core_shape: SplitShapeWire,
    pub(super) verifier_digest: [u8; VERIFIER_DIGEST_BYTES],
    pub(super) verifier_shape: MultiRoundShapeWire,
    pub(super) verifier_regular_shape: RegularShapeWire,
    pub(super) verifier_commitment_key: HyraxKeyWire,
}

impl McProverKeyWire {
    /// Decode one proving-key candidate under an absolute pre-allocation cap.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, McCodecError> {
        Self::decode_with_max_bytes(bytes, MAX_PROVER_KEY_BYTES)
    }

    fn decode_with_max_bytes(bytes: &[u8], max_bytes: usize) -> Result<Self, McCodecError> {
        if bytes.is_empty() || bytes.len() > max_bytes {
            return Err(McCodecError::InvalidEncoding);
        }
        let mut reader = Reader::new(bytes);
        let key = Self {
            application_key: read_hyrax_key(&mut reader)?,
            step_shape: read_split_shape(&mut reader)?,
            core_shape: read_split_shape(&mut reader)?,
            verifier_digest: reader
                .take(VERIFIER_DIGEST_BYTES)?
                .try_into()
                .map_err(|_| McCodecError::InvalidEncoding)?,
            verifier_shape: read_multi_round_shape(&mut reader)?,
            verifier_regular_shape: read_regular_shape(&mut reader)?,
            verifier_commitment_key: read_hyrax_key(&mut reader)?,
        };
        reader.finish()?;
        key.validate()?;
        Ok(key)
    }

    /// Encode the ordinary Microsoft bincode-compatible proving-key value.
    pub(super) fn encode(&self) -> Result<Vec<u8>, McCodecError> {
        self.validate()?;
        let encoded_len = self.encoded_len()?;
        if encoded_len > MAX_PROVER_KEY_BYTES {
            return Err(McCodecError::InvalidEncoding);
        }
        let mut output = try_vec_with_capacity(encoded_len)?;
        write_hyrax_key(&mut output, &self.application_key)?;
        write_split_shape(&mut output, &self.step_shape)?;
        write_split_shape(&mut output, &self.core_shape)?;
        output.extend_from_slice(&self.verifier_digest);
        write_multi_round_shape(&mut output, &self.verifier_shape)?;
        write_regular_shape(&mut output, &self.verifier_regular_shape)?;
        write_hyrax_key(&mut output, &self.verifier_commitment_key)?;
        if output.len() != encoded_len {
            return Err(McCodecError::InvalidEncoding);
        }
        Ok(output)
    }

    fn encoded_len(&self) -> Result<usize, McCodecError> {
        checked_sum(&[
            hyrax_key_encoded_len(&self.application_key)?,
            split_shape_encoded_len(&self.step_shape)?,
            split_shape_encoded_len(&self.core_shape)?,
            VERIFIER_DIGEST_BYTES,
            multi_round_shape_encoded_len(&self.verifier_shape)?,
            regular_shape_encoded_len(&self.verifier_regular_shape)?,
            hyrax_key_encoded_len(&self.verifier_commitment_key)?,
        ])
    }

    /// Require every setup component and the verifier digest to match one VK.
    pub(super) fn validate_against(
        &self,
        verifier_key: &McVerifierKeyWire,
    ) -> Result<(), McCodecError> {
        self.validate()?;
        if self.application_key != verifier_key.application_key
            || self.step_shape != verifier_key.step_shape
            || self.core_shape != verifier_key.core_shape
            || self.verifier_shape != verifier_key.verifier_shape
            || self.verifier_regular_shape != verifier_key.verifier_regular_shape
            || self.verifier_commitment_key != verifier_key.verifier_commitment_key
            || self.verifier_digest != verifier_key.digest()?
        {
            return Err(McCodecError::InvalidEncoding);
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), McCodecError> {
        validate_prover_components(
            &self.application_key,
            &self.step_shape,
            &self.core_shape,
            &self.verifier_shape,
            &self.verifier_regular_shape,
            &self.verifier_commitment_key,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PYTHON_VK: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/vega-prover/reference/fixtures/cubic/python_vk.bin"
    ));

    fn cubic_pair() -> (McVerifierKeyWire, McProverKeyWire) {
        let verifier_key =
            McVerifierKeyWire::decode(PYTHON_VK).expect("independent canonical verifier key");
        let prover_key = McProverKeyWire {
            application_key: verifier_key.application_key.clone(),
            step_shape: verifier_key.step_shape.clone(),
            core_shape: verifier_key.core_shape.clone(),
            verifier_digest: verifier_key.digest().expect("verifier digest"),
            verifier_shape: verifier_key.verifier_shape.clone(),
            verifier_regular_shape: verifier_key.verifier_regular_shape.clone(),
            verifier_commitment_key: verifier_key.verifier_commitment_key.clone(),
        };
        (verifier_key, prover_key)
    }

    #[test]
    fn cubic_derived_proving_key_roundtrips_and_matches_every_vk_component() {
        let (verifier_key, prover_key) = cubic_pair();
        prover_key
            .validate_against(&verifier_key)
            .expect("exact Microsoft proving/verifier pair");
        let encoded = prover_key.encode().expect("canonical proving key");
        let decoded = McProverKeyWire::decode(&encoded).expect("canonical proving-key decode");
        assert_eq!(decoded, prover_key);
        decoded
            .validate_against(&verifier_key)
            .expect("roundtripped pair remains exact");
    }

    #[test]
    fn proving_key_decoder_rejects_empty_trailing_truncated_and_cap_plus_one() {
        let (_, prover_key) = cubic_pair();
        let encoded = prover_key.encode().expect("canonical proving key");
        assert_eq!(
            McProverKeyWire::decode(&[]),
            Err(McCodecError::InvalidEncoding)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            McProverKeyWire::decode(&trailing),
            Err(McCodecError::InvalidEncoding)
        );
        for cut in [1, 7, encoded.len() / 2, encoded.len() - 1] {
            assert_eq!(
                McProverKeyWire::decode(&encoded[..cut]),
                Err(McCodecError::InvalidEncoding)
            );
        }
        assert_eq!(
            McProverKeyWire::decode_with_max_bytes(&encoded, encoded.len() - 1),
            Err(McCodecError::InvalidEncoding)
        );
    }

    #[test]
    fn proving_key_pairing_rejects_digest_and_structural_substitution() {
        let (verifier_key, mut wrong_digest) = cubic_pair();
        wrong_digest.verifier_digest[0] ^= 1;
        assert_eq!(
            wrong_digest.validate_against(&verifier_key),
            Err(McCodecError::InvalidEncoding)
        );

        let (_, mut wrong_commitment_key) = cubic_pair();
        wrong_commitment_key.application_key.generators.swap(0, 1);
        wrong_commitment_key
            .validate()
            .expect("generator order mutation remains structurally valid");
        assert_eq!(
            wrong_commitment_key.validate_against(&verifier_key),
            Err(McCodecError::InvalidEncoding)
        );
    }

    #[test]
    fn proving_key_decoder_rejects_structural_length_mutation_before_allocation() {
        let (_, prover_key) = cubic_pair();
        let mut encoded = prover_key.encode().expect("canonical proving key");
        encoded[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            McProverKeyWire::decode(&encoded),
            Err(McCodecError::InvalidEncoding)
        );
    }
}
