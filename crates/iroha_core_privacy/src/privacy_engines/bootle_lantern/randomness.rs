//! Purpose-separated entropy expansion for the closed issuance lifecycle.
//!
//! Holder and issuer APIs accept one cryptographic master source per
//! operation.  One health-checked 64-byte root is then split internally into
//! fixed, non-interchangeable SHAKE256 streams.  Callers cannot accidentally
//! supply identically seeded RNG objects for two different secret roles.
use crate::privacy_engines::prover_randomness::{
    HealthCheckedCryptoRngV1, ProverRandomnessErrorV1,
};
use rand_core::{TryCryptoRng, TryRng};
use sha3::digest::{ExtendableOutput, Update, XofReader};
use shake::Shake256;
use zeroize::{Zeroize, Zeroizing};
const MASTER_ROOT_BYTES_V1: usize = 64;
const STREAM_KEY_BYTES_V1: usize = 64;
const STREAM_BLOCK_BYTES_V1: usize = 64;
const ROOT_DERIVATION_DOMAIN_V1: &[u8] =
    b"iroha.privacy.bootle-lantern.issuance-randomness-root.v1";
const STREAM_EXPANSION_DOMAIN_V1: &[u8] =
    b"iroha.privacy.bootle-lantern.issuance-randomness-stream.v1";
const HOLDER_OPERATION_V1: &[u8] = b"holder-blind-request-v1";
const ISSUER_OPERATION_V1: &[u8] = b"issuer-blind-response-v1";
const HOLDER_MASK_PURPOSE_V1: &[u8] = b"credential-mask-v1";
const HOLDER_PROOF_PURPOSE_V1: &[u8] = b"blind-request-proof-v1";
const ISSUER_TAG_PURPOSE_V1: &[u8] = b"credential-tag-v1";
const ISSUER_PREIMAGE_PURPOSE_V1: &[u8] = b"falcon-preimage-coins-v1";
/// Canonical master-root and fixed-purpose substream policy.
pub const BOOTLE_LANTERN_ISSUANCE_RANDOMNESS_DESCRIPTOR_V1: &[u8] = b"master:one-health-checked-fixed64-source-block-per-operation|split:SHAKE256(frame(root-domain)+frame(operation)+frame(purpose)+frame(context-digest32)+frame(master64))->key64|expand:SHAKE256(frame(stream-domain)+frame(key64)+frame(counter-u64be))->block64|counter:checked-u64|holder:authorization-digest->{credential-mask,blind-request-proof}|issuer:request-digest->{credential-tag,falcon-preimage-coins}|closed-purpose-enum:no-caller-selected-labels|chunk-invariant-reservoir64|zeroize-root+keys+reservoirs:v1";
/// A single health-checked root consumed to construct one closed stream pair.
pub struct BootleLanternIssuanceRandomnessRootV1 {
    root: Zeroizing<[u8; MASTER_ROOT_BYTES_V1]>,
}
impl core::fmt::Debug for BootleLanternIssuanceRandomnessRootV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("BootleLanternIssuanceRandomnessRootV1(<redacted>)")
    }
}
impl BootleLanternIssuanceRandomnessRootV1 {
    /// Draw exactly one canonical health-checked master block.
    pub(crate) fn from_rng_v1<R: TryCryptoRng + TryRng>(
        source: &mut R,
    ) -> Result<Self, ProverRandomnessErrorV1> {
        let mut checked = HealthCheckedCryptoRngV1::new(source)?;
        let mut root = Zeroizing::new([0_u8; MASTER_ROOT_BYTES_V1]);
        checked
            .try_fill_bytes(root.as_mut())
            .map_err(|_| ProverRandomnessErrorV1::Unavailable)?;
        Ok(Self { root })
    }
    /// Consume the root into the two fixed holder-side streams.
    pub(crate) fn split_holder_v1(
        self,
        authorization_digest: [u8; 32],
    ) -> (BootleLanternPurposeRngV1, BootleLanternPurposeRngV1) {
        let mask = self.derive_v1(
            HOLDER_OPERATION_V1,
            HOLDER_MASK_PURPOSE_V1,
            &authorization_digest,
        );
        let proof = self.derive_v1(
            HOLDER_OPERATION_V1,
            HOLDER_PROOF_PURPOSE_V1,
            &authorization_digest,
        );
        (mask, proof)
    }
    /// Consume the root into the two fixed issuer-side streams.
    pub(crate) fn split_issuer_v1(
        self,
        request_digest: [u8; 32],
    ) -> (BootleLanternPurposeRngV1, BootleLanternPurposeRngV1) {
        let tag = self.derive_v1(ISSUER_OPERATION_V1, ISSUER_TAG_PURPOSE_V1, &request_digest);
        let preimage = self.derive_v1(
            ISSUER_OPERATION_V1,
            ISSUER_PREIMAGE_PURPOSE_V1,
            &request_digest,
        );
        (tag, preimage)
    }
    fn derive_v1(
        &self,
        operation: &[u8],
        purpose: &[u8],
        context_digest: &[u8; 32],
    ) -> BootleLanternPurposeRngV1 {
        let mut state = Shake256::default();
        absorb_frame_v1(&mut state, ROOT_DERIVATION_DOMAIN_V1);
        absorb_frame_v1(&mut state, operation);
        absorb_frame_v1(&mut state, purpose);
        absorb_frame_v1(&mut state, context_digest);
        absorb_frame_v1(&mut state, self.root.as_slice());
        let mut key = Zeroizing::new([0_u8; STREAM_KEY_BYTES_V1]);
        state.finalize_xof().read(key.as_mut());
        BootleLanternPurposeRngV1::from_key_v1(key)
    }
}
/// Deterministic cryptographic RNG for one fixed issuance purpose.
pub struct BootleLanternPurposeRngV1 {
    key: Zeroizing<[u8; STREAM_KEY_BYTES_V1]>,
    reservoir: Zeroizing<[u8; STREAM_BLOCK_BYTES_V1]>,
    cursor: usize,
    counter: u64,
}
impl core::fmt::Debug for BootleLanternPurposeRngV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("BootleLanternPurposeRngV1(<redacted>)")
    }
}
impl BootleLanternPurposeRngV1 {
    fn from_key_v1(key: Zeroizing<[u8; STREAM_KEY_BYTES_V1]>) -> Self {
        Self {
            key,
            reservoir: Zeroizing::new([0_u8; STREAM_BLOCK_BYTES_V1]),
            cursor: STREAM_BLOCK_BYTES_V1,
            counter: 0,
        }
    }
    fn refill_v1(&mut self) {
        let counter = self.counter.to_be_bytes();
        self.counter = self
            .counter
            .checked_add(1)
            .expect("fixed issuance work cannot exhaust u64 randomness blocks");
        self.reservoir.zeroize();
        let mut state = Shake256::default();
        absorb_frame_v1(&mut state, STREAM_EXPANSION_DOMAIN_V1);
        absorb_frame_v1(&mut state, self.key.as_slice());
        absorb_frame_v1(&mut state, &counter);
        state.finalize_xof().read(self.reservoir.as_mut());
        self.cursor = 0;
    }
    fn fill_canonical_v1(&mut self, destination: &mut [u8]) {
        let mut written = 0;
        while written < destination.len() {
            if self.cursor == STREAM_BLOCK_BYTES_V1 {
                self.refill_v1();
            }
            let available = STREAM_BLOCK_BYTES_V1 - self.cursor;
            let copied = available.min(destination.len() - written);
            let end = self.cursor + copied;
            destination[written..written + copied]
                .copy_from_slice(&self.reservoir[self.cursor..end]);
            self.reservoir[self.cursor..end].zeroize();
            self.cursor = end;
            written += copied;
        }
    }
}
impl rand::TryRng for BootleLanternPurposeRngV1 {
    type Error = core::convert::Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok({
            let mut bytes = [0_u8; 4];
            self.fill_canonical_v1(&mut bytes);
            u32::from_le_bytes(bytes)
        })
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok({
            let mut bytes = [0_u8; 8];
            self.fill_canonical_v1(&mut bytes);
            u64::from_le_bytes(bytes)
        })
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        self.fill_canonical_v1(destination);
        Ok(())
    }
}
impl rand::TryCryptoRng for BootleLanternPurposeRngV1 {}
impl Drop for BootleLanternPurposeRngV1 {
    fn drop(&mut self) {
        self.key.zeroize();
        self.reservoir.zeroize();
        self.cursor.zeroize();
        self.counter.zeroize();
    }
}
fn absorb_frame_v1(state: &mut Shake256, bytes: &[u8]) {
    state.update(
        &u64::try_from(bytes.len())
            .expect("fixed issuance randomness frame length fits u64")
            .to_be_bytes(),
    );
    state.update(bytes);
}
#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::TryRng;
    use std::io::Error as RngError;
    struct TestRng(u64);
    impl rand::TryRng for TestRng {
        type Error = std::io::Error;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Ok({
                let mut bytes = [0_u8; 4];
                self.try_fill_bytes(&mut bytes)?;
                u32::from_le_bytes(bytes)
            })
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok({
                let mut bytes = [0_u8; 8];
                self.try_fill_bytes(&mut bytes)?;
                u64::from_le_bytes(bytes)
            })
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
            for byte in destination {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                *byte = self.0.to_le_bytes()[0];
            }
            Ok(())
        }
    }
    impl rand::TryCryptoRng for TestRng {}
    struct FailingRng;
    impl rand::TryRng for FailingRng {
        type Error = std::io::Error;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            panic!("fallible boundary must use try_fill_bytes")
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            panic!("fallible boundary must use try_fill_bytes")
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
            destination.fill(0xa5);
            Err(RngError::other("injected master entropy failure"))
        }
    }
    impl rand::TryCryptoRng for FailingRng {}
    struct ConstantRng;
    impl rand::TryRng for ConstantRng {
        type Error = std::io::Error;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Ok(0)
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok(0)
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
            destination.fill(0x42);
            Ok(())
        }
    }
    impl rand::TryCryptoRng for ConstantRng {}
    fn holder_streams(
        seed: u64,
        context: [u8; 32],
    ) -> (BootleLanternPurposeRngV1, BootleLanternPurposeRngV1) {
        BootleLanternIssuanceRandomnessRootV1::from_rng_v1(&mut TestRng(seed))
            .expect("healthy master source")
            .split_holder_v1(context)
    }
    #[test]
    fn fixed_holder_and_issuer_purposes_are_distinct_for_one_master_stream() {
        let mut holder_source = TestRng(0x6a09_e667_f3bc_c908);
        let mut issuer_source = TestRng(0x6a09_e667_f3bc_c908);
        let (mut mask, mut proof) =
            BootleLanternIssuanceRandomnessRootV1::from_rng_v1(&mut holder_source)
                .expect("healthy holder root")
                .split_holder_v1([0x31; 32]);
        let (mut tag, mut preimage) =
            BootleLanternIssuanceRandomnessRootV1::from_rng_v1(&mut issuer_source)
                .expect("healthy issuer root")
                .split_issuer_v1([0x31; 32]);
        let mut outputs = [[0_u8; 96]; 4];
        mask.try_fill_bytes(&mut outputs[0])
            .expect("fixed purpose stream draw");
        proof
            .try_fill_bytes(&mut outputs[1])
            .expect("fixed purpose stream draw");
        tag.try_fill_bytes(&mut outputs[2])
            .expect("fixed purpose stream draw");
        preimage
            .try_fill_bytes(&mut outputs[3])
            .expect("fixed purpose stream draw");
        for left in 0..outputs.len() {
            for right in left + 1..outputs.len() {
                assert_ne!(outputs[left], outputs[right]);
            }
        }
    }
    #[test]
    fn derivation_is_deterministic_context_separated_and_chunk_invariant() {
        let (mut whole, _) = holder_streams(0xbb67_ae85_84ca_a73b, [0x41; 32]);
        let (mut chunked, _) = holder_streams(0xbb67_ae85_84ca_a73b, [0x41; 32]);
        let (mut other_context, _) = holder_streams(0xbb67_ae85_84ca_a73b, [0x42; 32]);
        let mut expected = [0_u8; 193];
        let mut actual = [0_u8; 193];
        let mut separated = [0_u8; 193];
        whole
            .try_fill_bytes(&mut expected)
            .expect("fixed purpose stream draw");
        chunked
            .try_fill_bytes(&mut actual[..1])
            .expect("fixed purpose stream draw");
        chunked
            .try_fill_bytes(&mut actual[1..64])
            .expect("fixed purpose stream draw");
        chunked
            .try_fill_bytes(&mut actual[64..129])
            .expect("fixed purpose stream draw");
        chunked
            .try_fill_bytes(&mut actual[129..])
            .expect("fixed purpose stream draw");
        other_context
            .try_fill_bytes(&mut separated)
            .expect("fixed purpose stream draw");
        assert_eq!(actual, expected);
        assert_ne!(separated, expected);
    }
    #[test]
    fn master_entropy_failure_and_health_sentinels_fail_closed() {
        assert_eq!(
            BootleLanternIssuanceRandomnessRootV1::from_rng_v1(&mut FailingRng).unwrap_err(),
            ProverRandomnessErrorV1::Unavailable
        );
        assert_eq!(
            BootleLanternIssuanceRandomnessRootV1::from_rng_v1(&mut ConstantRng).unwrap_err(),
            ProverRandomnessErrorV1::Unhealthy
        );
    }
    #[test]
    fn debug_output_never_exposes_root_or_stream_material() {
        let root =
            BootleLanternIssuanceRandomnessRootV1::from_rng_v1(&mut TestRng(0x3c6e_f372_fe94_f82b))
                .expect("healthy root");
        assert_eq!(
            format!("{root:?}"),
            "BootleLanternIssuanceRandomnessRootV1(<redacted>)"
        );
        let (stream, _) = root.split_holder_v1([0x51; 32]);
        assert_eq!(
            format!("{stream:?}"),
            "BootleLanternPurposeRngV1(<redacted>)"
        );
    }
}
