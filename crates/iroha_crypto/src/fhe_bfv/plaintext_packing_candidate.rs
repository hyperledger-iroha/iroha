//! Test-only F257 packing and public admission grammar; no encryption profile.
//!
//! See `specs/ram_lfe_plaintext_packing.md`. The private owners clear their own
//! allocated words. Borrowed input copies remain the caller's responsibility.
//! TODO: independently qualify parameters, input admission, noise, circuit privacy
//! and the complete execution relation before considering production adoption.

use core::fmt;
use zeroize::{Zeroize, Zeroizing};

const MODULUS: u16 = 257;
const SLOTS: usize = 128;
const RING_DEGREE: usize = 4096;
const STRIDE: usize = RING_DEGREE / SLOTS;
const ROOT: u16 = 3;
const INVERSE_SLOTS: u16 = 255;
const MAX_INPUT_BYTES: usize = 63;
const MAX_OUTPUTS: usize = 64;
/// Fixed key-switch roles shared with the test-only structural planner.
pub const GALOIS_EXPONENTS: [u16; 7] = [5, 25, 625, 5601, 4033, 3969, 8191];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PackingError {
    Length,
    NonCanonical { index: usize },
    Byte { index: usize },
    Padding { index: usize },
    Slot,
    Automorphism,
}

struct ClearingWords(Box<[u16; SLOTS]>);

impl ClearingWords {
    fn zero() -> Self {
        Self(Box::new([0; SLOTS]))
    }

    fn copy_canonical(input: &[u16]) -> Result<Self, PackingError> {
        if input.len() != SLOTS {
            return Err(PackingError::Length);
        }
        let mut owned = Self::zero();
        for (index, &word) in input.iter().enumerate() {
            if word >= MODULUS {
                return Err(PackingError::NonCanonical { index });
            }
            owned.0[index] = word;
        }
        Ok(owned)
    }
}

impl Drop for ClearingWords {
    fn drop(&mut self) {
        self.0.as_mut().zeroize();
        observe_cleared_cells(self.0.as_ref());
    }
}

impl fmt::Debug for ClearingWords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClearingWords([REDACTED; 128])")
    }
}

#[derive(Debug)]
struct ScalarSlots(ClearingWords);

impl ScalarSlots {
    fn copy_canonical(input: &[u16]) -> Result<Self, PackingError> {
        ClearingWords::copy_canonical(input).map(Self)
    }
}

/// Canonical client byte input in the existing scalar allocation. No encryption
/// relation is checked; caller-owned bytes and returned scalar copies stay private.
struct ClientInput(ScalarSlots);

impl ClientInput {
    fn from_bytes(input: &[u8]) -> Result<Self, PackingError> {
        if input.len() > MAX_INPUT_BYTES {
            return Err(PackingError::Length);
        }
        let mut words = ClearingWords::zero();
        words.0[0] = u16::try_from(input.len()).expect("bounded input");
        for (dst, &byte) in words.0[1..].iter_mut().zip(input) {
            *dst = u16::from(byte);
        }
        Ok(Self(ScalarSlots(words)))
    }

    fn from_slots(slots: ScalarSlots) -> Result<Self, PackingError> {
        let count = usize::from(slots.0.0[0]);
        if count > MAX_INPUT_BYTES {
            return Err(PackingError::Length);
        }
        for (index, &word) in slots.0.0.iter().enumerate().skip(1) {
            if index <= count && word > u16::from(u8::MAX) {
                return Err(PackingError::Byte { index });
            }
            if index > count && word != 0 {
                return Err(PackingError::Padding { index });
            }
        }
        Ok(Self(slots))
    }

    fn encode(&self) -> SparsePlaintext {
        SparsePlaintext::encode(&self.0)
    }
}

impl fmt::Debug for ClientInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClientInput([REDACTED])")
    }
}

/// Ordered output snapshot with a private count; 256 remains a scalar, not a byte.
/// Both constructors preserve the caller's source and own one existing slot buffer.
struct ScalarOutput {
    slots: ScalarSlots,
    count: usize,
}

impl ScalarOutput {
    fn from_values(values: &[u16]) -> Result<Self, PackingError> {
        if values.is_empty() || values.len() > MAX_OUTPUTS {
            return Err(PackingError::Length);
        }
        let mut words = ClearingWords::zero();
        for (index, &word) in values.iter().enumerate() {
            if word >= MODULUS {
                return Err(PackingError::NonCanonical { index });
            }
            words.0[index] = word;
        }
        Ok(Self {
            slots: ScalarSlots(words),
            count: values.len(),
        })
    }

    fn from_slots(count: usize, slots: ScalarSlots) -> Result<Self, PackingError> {
        if count == 0 || count > MAX_OUTPUTS {
            return Err(PackingError::Length);
        }
        for (index, &word) in slots.0.0.iter().enumerate().skip(count) {
            if word != 0 {
                return Err(PackingError::Padding { index });
            }
        }
        Ok(Self { slots, count })
    }

    fn encode(&self) -> SparsePlaintext {
        SparsePlaintext::encode(&self.slots)
    }
}

impl fmt::Debug for ScalarOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ScalarOutput([REDACTED])")
    }
}

impl Drop for ScalarOutput {
    fn drop(&mut self) {
        self.count.zeroize();
        // The existing ScalarSlots/ClearingWords field clears its full allocation.
        observe_cleared_count(self.count);
    }
}

/// Public identities only. A future purpose verifier must derive these from
/// authenticated policy state. Copying digests here authenticates no key/profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AdmissionContext {
    policy_hash: crate::Hash,
    parameter_digest: crate::Hash,
    public_key_digest: crate::Hash,
    evaluation_key_digest: crate::Hash,
    encryption_profile_digest: crate::Hash,
    semantic_context_hash: crate::Hash,
}

/// Candidate public grammar shared by the future input and execution relations.
/// This binds opaque submitted ciphertext bytes, not their canonical encoding or
/// plaintext. It is neither an admitted ciphertext nor a proof-verification token.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_crypto::ram_lfe::InputAdmissionStatementCandidateV1",
    frame = "iroha_crypto::ram_lfe::InputAdmissionStatementCandidateV1"
)]
struct InputAdmissionStatement {
    version: u8,
    policy_hash: crate::Hash,
    parameter_digest: crate::Hash,
    public_key_digest: crate::Hash,
    evaluation_key_digest: crate::Hash,
    encryption_profile_digest: crate::Hash,
    semantic_context_hash: crate::Hash,
    packing_contract_hash: crate::Hash,
    associated_data_hash: crate::Hash,
    input_ciphertext_hash: crate::Hash,
    // Public length of the submitted ciphertext bytes, never encrypted slot 0.
    // A selected fixed-profile codec must additionally enforce its fixed shape.
    input_ciphertext_bytes: u64,
}

// Header 40; version field 2; nine raw Hash fields (1+32); u64 field (1+8).
// All fields have alignment <=8, so the 40-byte header needs no extra padding.
const ADMISSION_STATEMENT_BYTES: usize = 40 + 2 + 9 * 33 + 9;
const ENCRYPTED_INPUT_BYTES: usize = 1_048_576;
const PROOF_ENVELOPE_BYTES: usize = 1_048_576;
const ASSOCIATED_DATA_BYTES: usize = 512;
const COMPOUND_PROOF_BYTES: usize = 192 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AdmissionError {
    InputLength,
    AssociatedDataLength,
    Metadata,
    Codec,
    StatementMismatch,
    ArithmeticOverflow,
    ProofBudget,
    EnvelopeBudget,
}

#[derive(norito::Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_crypto::ram_lfe::ScalarPackingContractCandidateV1",
    frame = "iroha_crypto::ram_lfe::ScalarPackingContractCandidateV1"
)]
// Encoding and logical key roles only. Refresh/sanitization, their extra keys
// and physical scheduling belong to the unresolved encryption profile.
struct PackingContract {
    version: u8,
    ring_degree: u16,
    modulus: u16,
    scalar_slots: u16,
    maximum_input_bytes: u16,
    maximum_outputs: u16,
    galois_exponents: [u16; 7],
    relinearization_required: bool,
}

fn packing_contract_hash() -> Result<crate::Hash, AdmissionError> {
    let contract = PackingContract {
        version: 1,
        ring_degree: u16::try_from(RING_DEGREE).map_err(|_| AdmissionError::ArithmeticOverflow)?,
        modulus: MODULUS,
        scalar_slots: u16::try_from(SLOTS).map_err(|_| AdmissionError::ArithmeticOverflow)?,
        maximum_input_bytes: u16::try_from(MAX_INPUT_BYTES)
            .map_err(|_| AdmissionError::ArithmeticOverflow)?,
        maximum_outputs: u16::try_from(MAX_OUTPUTS)
            .map_err(|_| AdmissionError::ArithmeticOverflow)?,
        galois_exponents: GALOIS_EXPONENTS,
        relinearization_required: true,
    };
    norito::encode_canonical(&contract)
        .map(crate::Hash::new)
        .map_err(|_| AdmissionError::Codec)
}

impl InputAdmissionStatement {
    fn for_input(
        context: AdmissionContext,
        associated_data: &[u8],
        input_ciphertext: &[u8],
    ) -> Result<Self, AdmissionError> {
        if input_ciphertext.is_empty() || input_ciphertext.len() > ENCRYPTED_INPUT_BYTES {
            return Err(AdmissionError::InputLength);
        }
        if associated_data.len() > ASSOCIATED_DATA_BYTES {
            return Err(AdmissionError::AssociatedDataLength);
        }
        Ok(Self {
            version: 1,
            policy_hash: context.policy_hash,
            parameter_digest: context.parameter_digest,
            public_key_digest: context.public_key_digest,
            evaluation_key_digest: context.evaluation_key_digest,
            encryption_profile_digest: context.encryption_profile_digest,
            semantic_context_hash: context.semantic_context_hash,
            packing_contract_hash: packing_contract_hash()?,
            associated_data_hash: crate::Hash::new(associated_data),
            input_ciphertext_hash: crate::Hash::new(input_ciphertext),
            input_ciphertext_bytes: u64::try_from(input_ciphertext.len())
                .map_err(|_| AdmissionError::ArithmeticOverflow)?,
        })
    }

    fn validate_metadata(&self) -> Result<(), AdmissionError> {
        if self.version != 1 || self.packing_contract_hash != packing_contract_hash()? {
            return Err(AdmissionError::Metadata);
        }
        if self.input_ciphertext_bytes == 0
            || self.input_ciphertext_bytes > ENCRYPTED_INPUT_BYTES as u64
        {
            return Err(AdmissionError::InputLength);
        }
        Ok(())
    }

    fn to_bytes(&self) -> Result<Vec<u8>, AdmissionError> {
        self.validate_metadata()?;
        if norito::canonical_frame_len(self).map_err(|_| AdmissionError::Codec)?
            != ADMISSION_STATEMENT_BYTES
        {
            return Err(AdmissionError::Codec);
        }
        norito::encode_canonical(self).map_err(|_| AdmissionError::Codec)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, AdmissionError> {
        // No variable-length fields or ciphertext/key copy enters this decoder.
        if bytes.len() != ADMISSION_STATEMENT_BYTES {
            return Err(AdmissionError::Codec);
        }
        let limits = norito::DecodeLimits::new(
            32,
            ADMISSION_STATEMENT_BYTES,
            ADMISSION_STATEMENT_BYTES,
            ADMISSION_STATEMENT_BYTES * 4,
            8,
        );
        let statement: Self = norito::decode_canonical_with_limits(bytes, limits)
            .map_err(|_| AdmissionError::Codec)?;
        statement.validate_metadata()?;
        Ok(statement)
    }

    /// Exact public-context comparison only. The expected value is not trusted
    /// merely because it has this Rust type; both proof relations remain absent.
    fn require_same_statement(&self, expected: &Self) -> Result<(), AdmissionError> {
        self.validate_metadata()?;
        expected.validate_metadata()?;
        if self != expected {
            return Err(AdmissionError::StatementMismatch);
        }
        Ok(())
    }

    /// Checked declared inventory, not a complete envelope codec or qualification.
    /// `other_bytes` includes every remaining frame/instance and input/output/key
    /// bytes ONLY when actually carried by this proof envelope. The encrypted-input
    /// cap is independent; hash binding does not duplicate that owner. Policy-key loading,
    /// algorithms and prover scratch remain unresolved; none are declared free.
    fn checked_proof_envelope_bytes(
        &self,
        input_proof_bytes: usize,
        execution_proof_bytes: usize,
        other_bytes: usize,
    ) -> Result<usize, AdmissionError> {
        self.validate_metadata()?;
        let proofs = input_proof_bytes
            .checked_add(execution_proof_bytes)
            .ok_or(AdmissionError::ArithmeticOverflow)?;
        if input_proof_bytes == 0 || execution_proof_bytes == 0 || proofs > COMPOUND_PROOF_BYTES {
            return Err(AdmissionError::ProofBudget);
        }
        let total = ADMISSION_STATEMENT_BYTES
            .checked_add(proofs)
            .and_then(|n| n.checked_add(other_bytes))
            .ok_or(AdmissionError::ArithmeticOverflow)?;
        if total > PROOF_ENVELOPE_BYTES {
            return Err(AdmissionError::EnvelopeBudget);
        }
        Ok(total)
    }
}

/// Stores only the plaintext coefficients of `g(x^32)`, never ciphertext data.
#[derive(Debug)]
struct SparsePlaintext(ClearingWords);

impl SparsePlaintext {
    fn copy_canonical(input: &[u16]) -> Result<Self, PackingError> {
        ClearingWords::copy_canonical(input).map(Self)
    }

    fn encode(slots: &ScalarSlots) -> Self {
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            let mut sum = Zeroizing::new(0_u16);
            for j in 0..SLOTS {
                // The inverse transform factors depend only on public indices.
                let inverse_beta_power = power(ROOT, (256 - ((2 * j + 1) * k) % 256) % 256);
                *sum = add(*sum, mul(slots.0.0[j], inverse_beta_power));
            }
            output.0[k] = mul(*sum, INVERSE_SLOTS);
        }
        Self(output)
    }

    fn decode(&self) -> ScalarSlots {
        let mut output = ClearingWords::zero();
        for j in 0..SLOTS {
            let beta = power(ROOT, 2 * j + 1);
            let mut value = Zeroizing::new(0_u16);
            for &coefficient in self.0.0.iter().rev() {
                *value = add(mul(*value, beta), coefficient);
            }
            output.0[j] = *value;
        }
        ScalarSlots(output)
    }

    fn mask(slot: usize) -> Result<Self, PackingError> {
        if slot >= SLOTS {
            return Err(PackingError::Slot);
        }
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            let inverse_beta_power = power(ROOT, (256 - ((2 * slot + 1) * k) % 256) % 256);
            output.0[k] = mul(INVERSE_SLOTS, inverse_beta_power);
        }
        Ok(Self(output))
    }

    fn add(&self, other: &Self) -> Self {
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            output.0[k] = add(self.0.0[k], other.0.0[k]);
        }
        Self(output)
    }

    fn subtract(&self, other: &Self) -> Self {
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            output.0[k] = sub(self.0.0[k], other.0.0[k]);
        }
        Self(output)
    }

    fn multiply(&self, other: &Self) -> Self {
        let mut output = ClearingWords::zero();
        for i in 0..SLOTS {
            for j in 0..SLOTS {
                let product = Zeroizing::new(mul(self.0.0[i], other.0.0[j]));
                let k = i + j;
                output.0[k % SLOTS] = if k < SLOTS {
                    add(output.0[k], *product)
                } else {
                    sub(output.0[k % SLOTS], *product)
                };
            }
        }
        Self(output)
    }

    fn automorphism(&self, exponent: usize) -> Result<Self, PackingError> {
        if exponent == 0 || exponent >= 2 * RING_DEGREE || exponent.is_multiple_of(2) {
            return Err(PackingError::Automorphism);
        }
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            let mapped = k * exponent % (2 * SLOTS);
            output.0[mapped % SLOTS] = if mapped < SLOTS {
                self.0.0[k]
            } else {
                sub(0, self.0.0[k])
            };
        }
        Ok(Self(output))
    }

    fn coefficient(&self, exponent: usize) -> Option<u16> {
        if exponent >= RING_DEGREE {
            None
        } else if exponent.is_multiple_of(STRIDE) {
            Some(self.0.0[exponent / STRIDE])
        } else {
            Some(0)
        }
    }
}

fn add(a: u16, b: u16) -> u16 {
    u16::try_from((u32::from(a) + u32::from(b)) % u32::from(MODULUS)).expect("reduced F257 element")
}

fn sub(a: u16, b: u16) -> u16 {
    u16::try_from((u32::from(a) + u32::from(MODULUS) - u32::from(b)) % u32::from(MODULUS))
        .expect("reduced F257 element")
}

fn mul(a: u16, b: u16) -> u16 {
    u16::try_from((u32::from(a) * u32::from(b)) % u32::from(MODULUS)).expect("reduced F257 element")
}

/// Only used with public transform bases and indices, never secret exponents.
fn power(mut base: u16, mut exponent: usize) -> u16 {
    let mut result = 1;
    while exponent > 0 {
        if exponent & 1 != 0 {
            result = mul(result, base);
        }
        base = mul(base, base);
        exponent >>= 1;
    }
    result
}

std::thread_local! {
    static WIPE_OBSERVATIONS: core::cell::RefCell<Option<Vec<bool>>> = const {
        core::cell::RefCell::new(None)
    };
}

fn observe_cleared_cells(words: &[u16; SLOTS]) {
    WIPE_OBSERVATIONS.with_borrow_mut(|observations| {
        if let Some(observations) = observations {
            observations.push(words.iter().all(|&word| word == 0));
        }
    });
}

fn observe_cleared_count(count: usize) {
    WIPE_OBSERVATIONS.with_borrow_mut(|observations| {
        if let Some(observations) = observations {
            observations.push(count == 0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    struct WipeObserver;

    impl WipeObserver {
        fn start() -> Self {
            WIPE_OBSERVATIONS.with_borrow_mut(|log| {
                assert!(log.is_none());
                *log = Some(Vec::new());
            });
            Self
        }

        fn assert_cleared(expected: usize) {
            WIPE_OBSERVATIONS.with_borrow(|log| {
                let log = log.as_ref().unwrap();
                assert_eq!(log.len(), expected);
                assert!(log.iter().all(|&cleared| cleared));
            });
        }
    }

    impl Drop for WipeObserver {
        fn drop(&mut self) {
            WIPE_OBSERVATIONS.with_borrow_mut(|log| *log = None);
        }
    }

    #[test]
    fn canonical_owners_reject_length_and_late_invalid_values() {
        let _observer = WipeObserver::start();
        assert!(matches!(
            ScalarSlots::copy_canonical(&[0; SLOTS - 1]),
            Err(PackingError::Length)
        ));
        assert!(matches!(
            SparsePlaintext::copy_canonical(&[0; SLOTS + 1]),
            Err(PackingError::Length)
        ));
        WipeObserver::assert_cleared(0);
        let mut input = [256; SLOTS];
        input[SLOTS - 1] = 257;
        assert!(matches!(
            ScalarSlots::copy_canonical(&input),
            Err(PackingError::NonCanonical { index: 127 })
        ));
        input[0] = u16::MAX;
        assert!(matches!(
            SparsePlaintext::copy_canonical(&input),
            Err(PackingError::NonCanonical { index: 0 })
        ));
        WipeObserver::assert_cleared(2);
        assert_eq!(input[SLOTS - 1], 257, "borrowed caller input is not erased");
    }

    #[test]
    fn every_basis_roundtrips_with_fixed_sparse_positions_and_mask_norm() {
        assert_eq!(power(ROOT, 128), 256);
        assert_eq!(mul(INVERSE_SLOTS, 128), 1);
        for slot in 0..SLOTS {
            let input = core::array::from_fn::<_, SLOTS, _>(|j| u16::from(j == slot));
            let values = ScalarSlots::copy_canonical(&input).unwrap();
            let encoded = SparsePlaintext::encode(&values);
            assert_eq!(*encoded.decode().0.0, input);
            assert_eq!(*encoded.0.0, *SparsePlaintext::mask(slot).unwrap().0.0);
            assert_eq!(
                encoded
                    .0
                    .0
                    .iter()
                    .map(|&x| usize::from(x.min(MODULUS - x)))
                    .sum::<usize>(),
                8256
            );
            for exponent in 0..RING_DEGREE {
                assert_eq!(
                    encoded.coefficient(exponent),
                    Some(if exponent % STRIDE == 0 {
                        encoded.0.0[exponent / STRIDE]
                    } else {
                        0
                    })
                );
            }
            assert_eq!(encoded.coefficient(RING_DEGREE), None);
        }
        assert!(matches!(
            SparsePlaintext::mask(SLOTS),
            Err(PackingError::Slot)
        ));
        assert!(matches!(
            SparsePlaintext::mask(usize::MAX),
            Err(PackingError::Slot)
        ));
    }

    #[test]
    fn encode_matches_independent_public_algebra_vector() {
        // Generated by the reviewed integer-only interpolation reference, not by this module.
        let expected = [
            192, 96, 183, 162, 205, 149, 213, 24, 215, 76, 129, 32, 248, 196, 26, 178, 45, 131,
            175, 158, 121, 177, 81, 11, 60, 192, 155, 117, 237, 151, 202, 203, 34, 35, 48, 7, 234,
            211, 94, 111, 63, 145, 101, 123, 83, 10, 77, 104, 85, 132, 210, 87, 21, 88, 240, 139,
            164, 154, 207, 219, 165, 255, 86, 98, 249, 98, 86, 255, 165, 219, 207, 154, 164, 139,
            240, 88, 21, 87, 210, 132, 85, 104, 77, 10, 83, 123, 101, 145, 63, 111, 94, 211, 234,
            7, 48, 35, 34, 203, 202, 151, 237, 117, 155, 192, 60, 11, 81, 177, 121, 158, 175, 131,
            45, 178, 26, 196, 248, 32, 129, 76, 215, 24, 213, 149, 205, 162, 183, 96,
        ];
        let input = core::array::from_fn::<_, SLOTS, _>(|j| u16::try_from(j).unwrap());
        let encoded = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&input).unwrap());
        assert_eq!(*encoded.0.0, expected);
        assert_eq!(
            *SparsePlaintext::copy_canonical(&expected)
                .unwrap()
                .decode()
                .0
                .0,
            input
        );
    }

    #[test]
    fn products_and_complete_selection_preserve_all_scalar_values() {
        let one = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&[1; SLOTS]).unwrap());
        for offset in [0, 128, 256] {
            let condition =
                core::array::from_fn::<_, SLOTS, _>(|j| u16::try_from((j + offset) % 257).unwrap());
            let zero =
                core::array::from_fn::<_, SLOTS, _>(|j| u16::try_from((7 * j + 2) % 257).unwrap());
            let nonzero =
                core::array::from_fn::<_, SLOTS, _>(|j| u16::try_from((j * j + 9) % 257).unwrap());
            let a = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&zero).unwrap());
            let b = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&nonzero).unwrap());
            assert_eq!(
                *a.multiply(&b).decode().0.0,
                core::array::from_fn(|j| mul(zero[j], nonzero[j]))
            );
            let mut powered =
                SparsePlaintext::encode(&ScalarSlots::copy_canonical(&condition).unwrap());
            for _ in 0..8 {
                powered = powered.multiply(&powered);
            }
            powered = one.multiply(&powered);
            let indicator = one.subtract(&powered);
            assert_eq!(
                *indicator.decode().0.0,
                core::array::from_fn(|j| u16::from(condition[j] == 0))
            );
            let selected = b.add(&indicator.multiply(&a.subtract(&b)));
            assert_eq!(
                *selected.decode().0.0,
                core::array::from_fn(|j| if condition[j] == 0 {
                    zero[j]
                } else {
                    nonzero[j]
                })
            );
        }
        // An extension-field coordinate is not an admitted scalar slot.
        assert_eq!(sub(1, power(ROOT, 8)), 122);
    }

    #[test]
    fn fixed_automorphisms_broadcast_every_masked_slot() {
        for slot in 0..SLOTS {
            let mut encoded = SparsePlaintext::mask(slot).unwrap();
            for exponent in GALOIS_EXPONENTS.map(usize::from) {
                let permuted = encoded.automorphism(exponent).unwrap();
                let slots = encoded.decode();
                assert_eq!(
                    *permuted.decode().0.0,
                    core::array::from_fn(|j| slots.0.0[((exponent * (2 * j + 1) - 1) / 2) % SLOTS])
                );
                encoded = encoded.add(&permuted);
            }
            assert_eq!(*encoded.decode().0.0, [1; SLOTS]);
        }
        let input = SparsePlaintext::mask(0).unwrap();
        for invalid in [0, 2, 8192, usize::MAX] {
            assert!(matches!(
                input.automorphism(invalid),
                Err(PackingError::Automorphism)
            ));
        }
        assert_eq!(*input.automorphism(1).unwrap().0.0, *input.0.0);
    }

    #[test]
    fn output_mask_clears_every_undeclared_slot() {
        let input = core::array::from_fn::<_, SLOTS, _>(|j| u16::try_from((13 * j) % 257).unwrap());
        let mask = core::array::from_fn::<_, SLOTS, _>(|j| u16::from(j < 64));
        let output = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&input).unwrap())
            .multiply(&SparsePlaintext::encode(
                &ScalarSlots::copy_canonical(&mask).unwrap(),
            ));
        assert_eq!(
            *output.decode().0.0,
            core::array::from_fn(|j| if j < 64 { input[j] } else { 0 })
        );
    }

    #[test]
    fn owners_clear_live_cells_before_deallocation_on_success_and_unwind() {
        let _observer = WipeObserver::start();
        {
            let values = ScalarSlots::copy_canonical(&[256; SLOTS]).unwrap();
            let encoded = SparsePlaintext::encode(&values);
            assert!(!format!("{values:?} {encoded:?}").contains("256"));
            assert!(format!("{values:?}").contains("REDACTED"));
        }
        WipeObserver::assert_cleared(2);
        let result = std::panic::catch_unwind(|| {
            let _owned = SparsePlaintext::copy_canonical(&[123; SLOTS]).unwrap();
            panic!("public fixture unwind");
        });
        assert!(result.is_err());
        WipeObserver::assert_cleared(3);
    }
    #[test]
    fn typed_client_input_preserves_empty_maximum_bytes_and_scalar_encoding() {
        for bytes in [Vec::new(), vec![0; 63], vec![255; 63]] {
            let input = ClientInput::from_bytes(&bytes).unwrap();
            assert_eq!(usize::from(input.0.0.0[0]), bytes.len());
            assert!(input.0.0.0[bytes.len() + 1..].iter().all(|&x| x == 0));
            let recovered = ClientInput::from_slots(input.encode().decode()).unwrap();
            assert_eq!(*recovered.0.0.0, *input.0.0.0);
            assert_eq!(
                &input.0.0.0[1..=bytes.len()],
                bytes.iter().copied().map(u16::from).collect::<Vec<_>>()
            );
        }
        assert!(matches!(
            ClientInput::from_bytes(&[1; 64]),
            Err(PackingError::Length)
        ));
    }

    #[test]
    fn typed_input_rejects_length_byte_and_all_padding_positions() {
        for length in [64, 128, 256] {
            let mut words = [0; SLOTS];
            words[0] = length;
            assert!(matches!(
                ClientInput::from_slots(ScalarSlots::copy_canonical(&words).unwrap()),
                Err(PackingError::Length)
            ));
        }
        for index in 1..=63 {
            let mut words = [0; SLOTS];
            words[0] = 63;
            words[index] = 256;
            assert!(
                matches!(ClientInput::from_slots(ScalarSlots::copy_canonical(&words).unwrap()), Err(PackingError::Byte { index: found }) if found == index)
            );
        }
        for index in 1..SLOTS {
            let mut words = [0; SLOTS];
            words[index] = 1;
            assert!(
                matches!(ClientInput::from_slots(ScalarSlots::copy_canonical(&words).unwrap()), Err(PackingError::Padding { index: found }) if found == index)
            );
        }
    }

    #[test]
    fn typed_owners_transfer_existing_allocation_without_plaintext_copy() {
        let slots = ScalarSlots::copy_canonical(&[0; SLOTS]).unwrap();
        let ptr = slots.0.0.as_ptr();
        let input = ClientInput::from_slots(slots).unwrap();
        assert_eq!(input.0.0.0.as_ptr(), ptr);
        let slots = ScalarSlots::copy_canonical(&[0; SLOTS]).unwrap();
        let ptr = slots.0.0.as_ptr();
        let output = ScalarOutput::from_slots(64, slots).unwrap();
        assert_eq!(output.slots.0.0.as_ptr(), ptr);
        assert_eq!(
            format!("{input:?} {output:?}"),
            "ClientInput([REDACTED]) ScalarOutput([REDACTED])"
        );
    }

    #[test]
    fn typed_output_preserves_scalar_256_order_count_and_snapshots() {
        for count in [1, 64] {
            let mut values: Vec<_> = (0..count)
                .map(|i| {
                    if i % 2 == 0 {
                        256
                    } else {
                        u16::try_from(i).expect("fixture index is below 64")
                    }
                })
                .collect();
            let output = ScalarOutput::from_values(&values).unwrap();
            let recovered = ScalarOutput::from_slots(count, output.encode().decode()).unwrap();
            assert_eq!(recovered.count, count);
            assert_eq!(&recovered.slots.0.0[..count], values);
            assert!(recovered.slots.0.0[count..].iter().all(|&x| x == 0));
            values.fill(17);
            assert_eq!(output.slots.0.0[0], 256, "owned output remains a snapshot");
        }
    }

    #[test]
    fn typed_output_rejects_bad_count_scalar_and_each_undeclared_slot() {
        for count in [0, 65, usize::MAX] {
            assert!(matches!(
                ScalarOutput::from_slots(count, ScalarSlots::copy_canonical(&[0; SLOTS]).unwrap()),
                Err(PackingError::Length)
            ));
        }
        for values in [vec![], vec![0; 65], vec![0; 129]] {
            assert!(matches!(
                ScalarOutput::from_values(&values),
                Err(PackingError::Length)
            ));
        }
        for invalid in [257, u16::MAX] {
            let mut values = [256; 64];
            values[63] = invalid;
            assert!(matches!(
                ScalarOutput::from_values(&values),
                Err(PackingError::NonCanonical { index: 63 })
            ));
        }
        for count in 1..=MAX_OUTPUTS {
            for index in count..SLOTS {
                let mut words = [0; SLOTS];
                words[index] = 256;
                assert!(
                    matches!(ScalarOutput::from_slots(count, ScalarSlots::copy_canonical(&words).unwrap()), Err(PackingError::Padding { index: found }) if found == index)
                );
            }
        }
    }

    #[test]
    fn typed_owners_clear_partial_complete_and_unwound_storage_and_output_count() {
        let _observer = WipeObserver::start();
        assert!(ClientInput::from_bytes(&[1; 64]).is_err());
        assert!(ScalarOutput::from_values(&[1; 65]).is_err());
        WipeObserver::assert_cleared(0);
        drop(ClientInput::from_bytes(&[13; 63]).unwrap());
        drop(ScalarOutput::from_values(&[256; 64]).unwrap());
        WipeObserver::assert_cleared(3); // input words, output count, output words
        let mut partial = [256; 64];
        partial[63] = 257;
        assert!(ScalarOutput::from_values(&partial).is_err());
        let mut padded = [0; SLOTS];
        padded[127] = 17;
        assert!(ClientInput::from_slots(ScalarSlots::copy_canonical(&padded).unwrap()).is_err());
        assert!(
            ScalarOutput::from_slots(64, ScalarSlots::copy_canonical(&padded).unwrap()).is_err()
        );
        WipeObserver::assert_cleared(6);
        assert!(
            std::panic::catch_unwind(|| {
                let _input = ClientInput::from_bytes(&[41; 63]).unwrap();
                let _output = ScalarOutput::from_values(&[256; 64]).unwrap();
                panic!("public fixture unwind");
            })
            .is_err()
        );
        WipeObserver::assert_cleared(9);
        assert_eq!(partial[0], 256, "caller copy is outside the owner");
    }

    fn admission_context() -> AdmissionContext {
        AdmissionContext {
            policy_hash: crate::Hash::new(b"policy"),
            parameter_digest: crate::Hash::new(b"parameters"),
            public_key_digest: crate::Hash::new(b"public key"),
            evaluation_key_digest: crate::Hash::new(b"evaluation keys"),
            encryption_profile_digest: crate::Hash::new(b"unqualified profile fixture"),
            semantic_context_hash: crate::Hash::new(b"chain/program/owner/backend context"),
        }
    }

    fn admission_statement() -> InputAdmissionStatement {
        // These opaque public bytes are deliberately not an encryption fixture.
        InputAdmissionStatement::for_input(admission_context(), b"context", b"opaque input")
            .unwrap()
    }

    #[test]
    fn admission_statement_has_one_exact_canonical_frame_and_ambient_independence() {
        let statement = admission_statement();
        let expected = statement.to_bytes().unwrap();
        assert_eq!(expected.len(), 348);
        assert_eq!(
            InputAdmissionStatement::from_bytes(&expected).unwrap(),
            statement
        );
        for flags in [0, norito::core::default_encode_flags()] {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(statement.to_bytes().unwrap(), expected);
            assert_eq!(
                InputAdmissionStatement::from_bytes(&expected).unwrap(),
                statement
            );
        }
        statement.require_same_statement(&statement).unwrap();
    }

    #[test]
    fn admission_statement_binds_each_policy_key_profile_context_and_input_field() {
        let statement = admission_statement();
        let changed_hash = crate::Hash::new(b"different public identity");
        for field in 0..9 {
            let mut changed = statement.clone();
            match field {
                0 => changed.policy_hash = changed_hash,
                1 => changed.parameter_digest = changed_hash,
                2 => changed.public_key_digest = changed_hash,
                3 => changed.evaluation_key_digest = changed_hash,
                4 => changed.encryption_profile_digest = changed_hash,
                5 => changed.semantic_context_hash = changed_hash,
                6 => changed.associated_data_hash = changed_hash,
                7 => changed.input_ciphertext_hash = changed_hash,
                _ => {
                    changed.input_ciphertext_bytes += 1;
                }
            }
            let frame = changed.to_bytes().unwrap();
            assert_ne!(frame, statement.to_bytes().unwrap());
            let parsed = InputAdmissionStatement::from_bytes(&frame).unwrap();
            assert_eq!(
                parsed.require_same_statement(&statement),
                Err(AdmissionError::StatementMismatch)
            );
        }
        let different_ad =
            InputAdmissionStatement::for_input(admission_context(), b"other", b"opaque input")
                .unwrap();
        let different_input =
            InputAdmissionStatement::for_input(admission_context(), b"context", b"changed input")
                .unwrap();
        assert_ne!(
            statement.associated_data_hash,
            different_ad.associated_data_hash
        );
        assert_ne!(
            statement.input_ciphertext_hash,
            different_input.input_ciphertext_hash
        );
        // Hash values are commitments to public encodings, never Fp conversions.
        let mut context = admission_context();
        context.policy_hash = crate::Hash::prehashed([0; 32]);
        assert!(InputAdmissionStatement::for_input(context, b"", b"x").is_ok());
    }

    #[test]
    fn admission_statement_rejects_malformed_noncanonical_and_foreign_metadata() {
        let statement = admission_statement();
        let frame = statement.to_bytes().unwrap();
        for length in [
            0,
            frame.len() - 1,
            frame.len() + 1,
            ENCRYPTED_INPUT_BYTES + 1,
        ] {
            assert_eq!(
                InputAdmissionStatement::from_bytes(&vec![0; length]),
                Err(AdmissionError::Codec)
            );
        }
        for index in [0, 6, 22, 23, 31, 39, 40, frame.len() - 1] {
            let mut bad = frame.clone();
            bad[index] ^= 1;
            assert!(InputAdmissionStatement::from_bytes(&bad).is_err());
        }
        let mut foreign = statement.clone();
        foreign.version = 2;
        let frame = norito::encode_canonical(&foreign).unwrap();
        assert_eq!(
            InputAdmissionStatement::from_bytes(&frame),
            Err(AdmissionError::Metadata)
        );
        foreign = statement.clone();
        foreign.packing_contract_hash = crate::Hash::new(b"other encoding/key-role contract");
        assert_eq!(foreign.to_bytes(), Err(AdmissionError::Metadata));
        assert_eq!(
            foreign.require_same_statement(&statement),
            Err(AdmissionError::Metadata)
        );
        assert_eq!(
            statement.require_same_statement(&foreign),
            Err(AdmissionError::Metadata)
        );
        assert_eq!(
            InputAdmissionStatement::from_bytes(&norito::encode_canonical(&foreign).unwrap()),
            Err(AdmissionError::Metadata)
        );
        for invalid in [0, ENCRYPTED_INPUT_BYTES as u64 + 1, u64::MAX] {
            let mut bad = statement.clone();
            bad.input_ciphertext_bytes = invalid;
            assert_eq!(bad.to_bytes(), Err(AdmissionError::InputLength));
            assert_eq!(
                InputAdmissionStatement::from_bytes(&norito::encode_canonical(&bad).unwrap()),
                Err(AdmissionError::InputLength)
            );
        }
        let _guard = norito::core::DecodeFlagsGuard::enter(0);
        let noncanonical = norito::core::to_bytes(&statement).unwrap();
        assert_ne!(noncanonical, statement.to_bytes().unwrap());
        assert!(InputAdmissionStatement::from_bytes(&noncanonical).is_err());
    }

    #[test]
    fn admission_input_and_associated_data_caps_precede_hashing() {
        for input in [vec![], vec![0; ENCRYPTED_INPUT_BYTES + 1]] {
            assert_eq!(
                InputAdmissionStatement::for_input(admission_context(), b"", &input),
                Err(AdmissionError::InputLength)
            );
        }
        assert_eq!(
            InputAdmissionStatement::for_input(
                admission_context(),
                &[0; ASSOCIATED_DATA_BYTES + 1],
                b"x"
            ),
            Err(AdmissionError::AssociatedDataLength)
        );
        let statement = InputAdmissionStatement::for_input(
            admission_context(),
            &[0; ASSOCIATED_DATA_BYTES],
            &vec![0; ENCRYPTED_INPUT_BYTES],
        )
        .unwrap();
        assert_eq!(
            statement.input_ciphertext_bytes,
            ENCRYPTED_INPUT_BYTES as u64
        );
        assert_eq!(
            statement.checked_proof_envelope_bytes(1, 1, 0),
            Ok(ADMISSION_STATEMENT_BYTES + 2)
        );
        assert_eq!(
            statement.checked_proof_envelope_bytes(1, 1, ENCRYPTED_INPUT_BYTES),
            Err(AdmissionError::EnvelopeBudget),
            "actual optional co-location still counts"
        );
    }

    #[test]
    fn separate_input_and_proof_envelope_caps_preserve_checked_sums() {
        let statement = admission_statement();
        let base = ADMISSION_STATEMENT_BYTES + COMPOUND_PROOF_BYTES;
        assert_eq!(
            statement.checked_proof_envelope_bytes(
                1,
                COMPOUND_PROOF_BYTES - 1,
                PROOF_ENVELOPE_BYTES - base
            ),
            Ok(PROOF_ENVELOPE_BYTES)
        );
        assert_eq!(
            statement.checked_proof_envelope_bytes(
                1,
                COMPOUND_PROOF_BYTES - 1,
                PROOF_ENVELOPE_BYTES - base + 1
            ),
            Err(AdmissionError::EnvelopeBudget)
        );
        for proofs in [(0, 1), (1, 0), (1, COMPOUND_PROOF_BYTES)] {
            assert_eq!(
                statement.checked_proof_envelope_bytes(proofs.0, proofs.1, 0),
                Err(AdmissionError::ProofBudget)
            );
        }
        assert_eq!(
            statement.checked_proof_envelope_bytes(usize::MAX, 1, 0),
            Err(AdmissionError::ArithmeticOverflow)
        );
        assert_eq!(
            statement.checked_proof_envelope_bytes(1, 1, usize::MAX),
            Err(AdmissionError::ArithmeticOverflow)
        );
        let mut bad = statement;
        bad.input_ciphertext_bytes = u64::MAX;
        assert_eq!(
            bad.checked_proof_envelope_bytes(1, 1, 0),
            Err(AdmissionError::InputLength)
        );
    }

    #[test]
    fn candidate_packing_descriptor_binds_the_shared_galois_roles() {
        assert_eq!(GALOIS_EXPONENTS, [5, 25, 625, 5601, 4033, 3969, 8191]);
        assert!(
            GALOIS_EXPONENTS
                .iter()
                .all(|&x| x % 2 == 1 && usize::from(x) < 2 * RING_DEGREE)
        );
        assert_eq!(
            GALOIS_EXPONENTS
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            7
        );
        assert_eq!(
            admission_statement().packing_contract_hash,
            packing_contract_hash().unwrap()
        );
    }
}
