//! Consuming, direct-object collective automorphism and compact key switch.
//!
//! The input and output keep every component as 38 independently addressed
//! canonical limbs.  The only full-polynomial owners are the two output
//! accumulators; decomposition rereads authenticated C1 stripes into a bounded
//! five-digit batch and never constructs a native input ciphertext or digit.
use super::super::{
    collective::{
        ZkAmsMkheStreamingCollectiveAutomorphismOutputV1, ZkAmsMkheStreamingCollectiveCiphertextV1,
        prepare_zk_ams_mkhe_streaming_collective_automorphism_output_v1,
    },
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, ZkAmsMkheDirectObjectCasPublicationV1,
        ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1,
        ZkAmsMkheDirectObjectReadAtProviderV1, ZkAmsMkheDirectObjectReadReceiptV1,
        ZkAmsMkheDirectObjectReadTransactionV1,
    },
};
use super::*;
const STREAMING_COLLECTIVE_LIMB_COUNT_BYTES_V1: usize = core::mem::size_of::<u32>();
const STREAMING_AUTOMORPHISM_STRIPE_COEFFICIENTS_V1: usize =
    ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / core::mem::size_of::<u64>();
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StreamingAutomorphismInputSnapshotV1 {
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
}
impl StreamingAutomorphismInputSnapshotV1 {
    fn observe_v1(
        expected: &mut Option<Self>,
        receipt: &ZkAmsMkheDirectObjectReadReceiptV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let snapshot = receipt.snapshot();
        if snapshot.pointer() != pointer
            || receipt.canonical_bytes() != pointer.payload_bytes()
            || receipt.payload_blake3() != pointer.payload_blake3()
            || receipt.receipt_digest() == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let observed = Self {
            provider_identity: snapshot.provider_identity(),
            snapshot_identity: snapshot.snapshot_identity(),
        };
        match expected {
            None => *expected = Some(observed),
            Some(bound) if *bound == observed => {}
            Some(_) => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
        }
        Ok(())
    }
}
struct StreamingAutomorphismInputScratchV1 {
    stripe: Vec<u8>,
    residues: Vec<u64>,
    transactions: Vec<ZkAmsMkheDirectObjectReadTransactionV1>,
}
impl StreamingAutomorphismInputScratchV1 {
    fn new_v1(profile: &BgvProfile) -> Result<Self, ZkAmsMkheErrorV1> {
        let stripe_bytes = profile
            .moduli
            .len()
            .checked_mul(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut stripe = Vec::new();
        stripe
            .try_reserve_exact(stripe_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if stripe.capacity() != stripe_bytes {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        stripe.resize(stripe_bytes, 0);
        let mut residues = Vec::new();
        residues
            .try_reserve_exact(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if residues.capacity() != profile.moduli.len() {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        residues.resize(profile.moduli.len(), 0);
        let mut transactions = Vec::new();
        transactions
            .try_reserve_exact(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if transactions.capacity() != profile.moduli.len() {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(Self {
            stripe,
            residues,
            transactions,
        })
    }
    fn begin_transactions_v1<P>(
        &mut self,
        kind: ZkAmsMkheDirectObjectKindV1,
        pointers: &[ZkAmsMkheDirectObjectPointerV1],
        reverse: bool,
        provider: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if !self.transactions.is_empty()
            || pointers.len() != self.transactions.capacity()
            || self.transactions.capacity() != self.residues.capacity()
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        if reverse {
            for pointer in pointers.iter().rev() {
                self.transactions
                    .push(ZkAmsMkheDirectObjectReadTransactionV1::begin(
                        kind, *pointer, provider,
                    )?);
            }
        } else {
            for pointer in pointers {
                self.transactions
                    .push(ZkAmsMkheDirectObjectReadTransactionV1::begin(
                        kind, *pointer, provider,
                    )?);
            }
        }
        Ok(())
    }
}
impl Drop for StreamingAutomorphismInputScratchV1 {
    fn drop(&mut self) {
        let stripe = core::hint::black_box(&mut self.stripe);
        stripe.fill(0);
        let residues = core::hint::black_box(&mut self.residues);
        residues.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *stripe);
        let _ = core::hint::black_box(&mut *residues);
    }
}
struct StreamingAutomorphismHybridBatchV1 {
    first_digit: usize,
    digit_count: usize,
    ciphertext_modulus: WideUint,
    half_modulus: WideUint,
    base: u64,
    signed_digits: Vec<i64>,
}
impl StreamingAutomorphismHybridBatchV1 {
    fn new_v1(profile: &BgvProfile) -> Result<Self, ZkAmsMkheErrorV1> {
        if !profile.hybrid_rns_decomposition
            || profile.gadget_base_log != 60
            || profile.gadget_digits != profile.moduli.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        let signed_count = profile
            .ring_degree
            .checked_mul(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut signed_digits = Vec::new();
        signed_digits
            .try_reserve_exact(signed_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if signed_digits.capacity() != signed_count {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        signed_digits.resize(signed_count, 0);
        let ciphertext_modulus = modulus_product(profile.moduli)?;
        let half_modulus = ciphertext_modulus.shr_one();
        let base = 1_u64
            .checked_shl(u32::from(profile.gadget_base_log))
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
        Ok(Self {
            first_digit: 0,
            digit_count: 0,
            ciphertext_modulus,
            half_modulus,
            base,
            signed_digits,
        })
    }
    fn begin_batch_v1(
        &mut self,
        profile: &BgvProfile,
        first_digit: usize,
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        if first_digit >= profile.gadget_digits {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        let digit_count =
            (profile.gadget_digits - first_digit).min(HOISTED_HYBRID_DIGIT_BATCH_SIZE_V1);
        self.first_digit = first_digit;
        self.digit_count = digit_count;
        self.signed_digits.fill(0);
        first_digit
            .checked_add(digit_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    }
    fn absorb_coefficient_v1(
        &mut self,
        profile: &BgvProfile,
        exponent: usize,
        source_coefficient: usize,
        residues: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.digit_count == 0
            || residues.len() != profile.moduli.len()
            || source_coefficient >= profile.ring_degree
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let twice_degree = profile
            .ring_degree
            .checked_mul(2)
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
        if exponent == 0 || exponent >= twice_degree || exponent.is_multiple_of(2) {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let mapped = source_coefficient
            .checked_mul(exponent)
            .map(|mapped| mapped % twice_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let (destination, automorphism_negates) = if mapped >= profile.ring_degree {
            (mapped - profile.ring_degree, true)
        } else {
            (mapped, false)
        };
        let canonical = WideUint::crt(residues, profile.moduli)?;
        let (coefficient_negates, magnitude) = if canonical > self.half_modulus {
            (
                true,
                self.ciphertext_modulus
                    .checked_sub(canonical)
                    .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?,
            )
        } else {
            (false, canonical)
        };
        let output_negates = coefficient_negates ^ automorphism_negates;
        let mut carry = 0_u64;
        let end_digit = self
            .first_digit
            .checked_add(self.digit_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for digit in 0..profile.gadget_digits {
            let chunk = magnitude.bits_at(
                digit
                    .checked_mul(usize::from(profile.gadget_base_log))
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                usize::from(profile.gadget_base_log),
            )?;
            let with_carry = chunk
                .checked_add(carry)
                .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
            let (balanced, next_carry) = if with_carry >= self.base / 2 {
                (
                    i64::try_from(with_carry).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
                        - i64::try_from(self.base).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
                    1,
                )
            } else {
                (
                    i64::try_from(with_carry).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
                    0,
                )
            };
            if (self.first_digit..end_digit).contains(&digit) {
                let local_digit = digit - self.first_digit;
                let offset = local_digit
                    .checked_mul(profile.ring_degree)
                    .and_then(|offset| offset.checked_add(destination))
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                self.signed_digits[offset] = if output_negates {
                    balanced
                        .checked_neg()
                        .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?
                } else {
                    balanced
                };
            }
            carry = next_carry;
        }
        if carry != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }
    fn fill_digit_limb_v1(
        &self,
        profile: &BgvProfile,
        digit_index: usize,
        limb_index: usize,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let end_digit = self
            .first_digit
            .checked_add(self.digit_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if !(self.first_digit..end_digit).contains(&digit_index)
            || limb_index >= profile.moduli.len()
            || output.len() != profile.ring_degree
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let local_digit = digit_index - self.first_digit;
        let start = local_digit
            .checked_mul(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let values = self
            .signed_digits
            .get(start..start + profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        let modulus = profile.moduli[limb_index];
        for (output, value) in output.iter_mut().zip(values) {
            *output = signed_mod(*value, modulus);
        }
        Ok(())
    }
}
impl Drop for StreamingAutomorphismHybridBatchV1 {
    fn drop(&mut self) {
        let signed_digits = core::hint::black_box(&mut self.signed_digits);
        signed_digits.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *signed_digits);
    }
}
fn exact_zero_rns_v1(profile: &BgvProfile) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(coefficient_count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if coefficients.capacity() != coefficient_count {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    coefficients.resize(coefficient_count, 0);
    RnsPolynomial::from_flat(profile, coefficients)
}
fn exact_limb_mut_v1<'a>(
    profile: &BgvProfile,
    polynomial: &'a mut RnsPolynomial,
    limb_index: usize,
) -> Result<&'a mut [u64], ZkAmsMkheErrorV1> {
    let start = limb_index
        .checked_mul(profile.ring_degree)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = start
        .checked_add(profile.ring_degree)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    polynomial
        .coefficients
        .get_mut(start..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)
}
fn read_exact_limb_into_v1<P>(
    profile: &BgvProfile,
    modulus: u64,
    transaction: &mut ZkAmsMkheDirectObjectReadTransactionV1,
    provider: &mut P,
    output: &mut [u64],
    scratch: &mut [u8],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    if output.len() != profile.ring_degree
        || scratch.len() != ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut count = [0_u8; STREAMING_COLLECTIVE_LIMB_COUNT_BYTES_V1];
    if transaction.read_next(provider, &mut count)? != count.len()
        || usize::try_from(u32::from_be_bytes(count)).ok() != Some(profile.ring_degree)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut filled = 0_usize;
    while filled != output.len() {
        let take_coefficients =
            (output.len() - filled).min(scratch.len() / core::mem::size_of::<u64>());
        let take_bytes = take_coefficients
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if transaction.read_next(provider, &mut scratch[..take_bytes])? != take_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        for (destination, encoded) in output[filled..filled + take_coefficients]
            .iter_mut()
            .zip(scratch[..take_bytes].chunks_exact(core::mem::size_of::<u64>()))
        {
            let residue = u64::from_be_bytes(
                encoded
                    .try_into()
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?,
            );
            if residue >= modulus {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            *destination = residue;
        }
        filled = filled
            .checked_add(take_coefficients)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    if transaction.remaining_bytes() != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    Ok(())
}
#[allow(
    clippy::too_many_arguments,
    reason = "fixed streaming axes remain explicit to preserve authenticated read order"
)]
fn read_automorphed_constant_v1<P>(
    profile: &BgvProfile,
    exponent: usize,
    pointers: &[ZkAmsMkheDirectObjectPointerV1],
    provider: &mut P,
    output: &mut RnsPolynomial,
    workspace: &mut KeySwitchLimbWorkspaceV1,
    input: &mut StreamingAutomorphismInputScratchV1,
    snapshot: &mut Option<StreamingAutomorphismInputSnapshotV1>,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    input.begin_transactions_v1(
        ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
        pointers,
        true,
        provider,
    )?;
    let twice_degree = profile
        .ring_degree
        .checked_mul(2)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    if exponent == 0 || exponent >= twice_degree || exponent.is_multiple_of(2) {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    for (limb_index, pointer) in pointers.iter().enumerate() {
        let mut transaction = input
            .transactions
            .pop()
            .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?;
        read_exact_limb_into_v1(
            profile,
            profile.moduli[limb_index],
            &mut transaction,
            provider,
            &mut workspace.evaluated_key,
            &mut input.stripe[..ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
        )?;
        let receipt = transaction.finish(provider)?;
        StreamingAutomorphismInputSnapshotV1::observe_v1(snapshot, &receipt, *pointer)?;
        let output_limb = exact_limb_mut_v1(profile, output, limb_index)?;
        for (source, value) in workspace.evaluated_key.iter().copied().enumerate() {
            let mapped = source
                .checked_mul(exponent)
                .map(|mapped| mapped % twice_degree)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let (destination, value) = if mapped >= profile.ring_degree {
                (
                    mapped - profile.ring_degree,
                    if value == 0 {
                        0
                    } else {
                        profile.moduli[limb_index] - value
                    },
                )
            } else {
                (mapped, value)
            };
            output_limb[destination] = value;
        }
    }
    if !input.transactions.is_empty() {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    Ok(())
}
fn read_automorphed_linear_batch_v1<P>(
    profile: &BgvProfile,
    exponent: usize,
    pointers: &[ZkAmsMkheDirectObjectPointerV1],
    provider: &mut P,
    hoisted: &mut StreamingAutomorphismHybridBatchV1,
    input: &mut StreamingAutomorphismInputScratchV1,
    snapshot: &mut Option<StreamingAutomorphismInputSnapshotV1>,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    input.begin_transactions_v1(
        ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
        pointers,
        false,
        provider,
    )?;
    let mut count = [0_u8; STREAMING_COLLECTIVE_LIMB_COUNT_BYTES_V1];
    for transaction in &mut input.transactions {
        if transaction.read_next(provider, &mut count)? != count.len()
            || usize::try_from(u32::from_be_bytes(count)).ok() != Some(profile.ring_degree)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
    }
    if !profile
        .ring_degree
        .is_multiple_of(STREAMING_AUTOMORPHISM_STRIPE_COEFFICIENTS_V1)
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    for stripe_start in
        (0..profile.ring_degree).step_by(STREAMING_AUTOMORPHISM_STRIPE_COEFFICIENTS_V1)
    {
        for (limb_index, transaction) in input.transactions.iter_mut().enumerate() {
            let start = limb_index
                .checked_mul(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let end = start
                .checked_add(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if transaction.read_next(provider, &mut input.stripe[start..end])?
                != ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1
            {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
        }
        for local_coefficient in 0..STREAMING_AUTOMORPHISM_STRIPE_COEFFICIENTS_V1 {
            for (limb_index, residue) in input.residues.iter_mut().enumerate() {
                let start = limb_index
                    .checked_mul(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1)
                    .and_then(|start| {
                        local_coefficient
                            .checked_mul(core::mem::size_of::<u64>())
                            .and_then(|offset| start.checked_add(offset))
                    })
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                let end = start
                    .checked_add(core::mem::size_of::<u64>())
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                let value = u64::from_be_bytes(
                    input.stripe[start..end]
                        .try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?,
                );
                if value >= profile.moduli[limb_index] {
                    return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
                }
                *residue = value;
            }
            hoisted.absorb_coefficient_v1(
                profile,
                exponent,
                stripe_start
                    .checked_add(local_coefficient)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                &input.residues,
            )?;
        }
    }
    for transaction in &input.transactions {
        if transaction.remaining_bytes() != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
    }
    for (limb_index, transaction) in input.transactions.drain(..).enumerate() {
        let receipt = transaction.finish(provider)?;
        StreamingAutomorphismInputSnapshotV1::observe_v1(snapshot, &receipt, pointers[limb_index])?;
    }
    Ok(())
}
/// Apply one frozen Galois automorphism directly to a consuming 38-limb
/// ciphertext manifest and compactly switch the linear component back to the
/// governed collective key.
///
/// Both providers are externally owned. Their opaque caches and backend
/// residency are deliberately outside managed-memory accounting; every byte
/// owned by this function, the retained runtime/key handle, and the consuming
/// input manifest is enumerated by the companion accounting function.
pub fn automorphism_switch_zk_ams_mkhe_collective_streaming_v1<K, D>(
    runtime: &ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1,
    schedule_index: usize,
    key: &ZkAmsMkheValidatedCollectiveEvaluatedKeyV1,
    evaluated_key_provider: &mut K,
    ciphertext: ZkAmsMkheStreamingCollectiveCiphertextV1,
    direct_objects: &mut D,
) -> Result<ZkAmsMkheStreamingCollectiveCiphertextV1, ZkAmsMkheErrorV1>
where
    K: ZkAmsMkheCollectiveEvaluatedKeyProviderV1 + ?Sized,
    D: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let profile = &runtime.profile;
    profile.validate()?;
    let accounting = zk_ams_mkhe_streaming_collective_automorphism_accounting_v1()?;
    if accounting.whole_operation_managed_peak_bytes
        > u64::try_from(profile.max_workspace_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let ordinal = schedule_index
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    runtime.validate_key_context(key)?;
    if key.entry.purpose() != ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois
        || usize::from(key.entry.ordinal()) != ordinal
    {
        return Err(ZkAmsMkheErrorV1::MissingEvaluatedKey);
    }
    let exponent = usize::try_from(key.entry.galois_exponent())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let output_transcript_digest = runtime.output_lineage(key, ciphertext.ciphertext_digest())?;
    // The complete output authority, all output heap capacity, and every
    // arithmetic/read owner are allocated before the first ciphertext byte is
    // requested from the direct-object provider.
    let mut output_publication = prepare_zk_ams_mkhe_streaming_collective_automorphism_output_v1(
        &ciphertext,
        &runtime.eval_key_binding,
        output_transcript_digest,
    )?;
    let mut constant = exact_zero_rns_v1(profile)?;
    let mut linear = exact_zero_rns_v1(profile)?;
    {
        let mut workspace = KeySwitchLimbWorkspaceV1::new(profile)?;
        let mut input = StreamingAutomorphismInputScratchV1::new_v1(profile)?;
        let mut hoisted = StreamingAutomorphismHybridBatchV1::new_v1(profile)?;
        let mut input_snapshot = None;
        runtime.validate_provider_state(key, evaluated_key_provider)?;
        {
            let binding = ciphertext.sealed_binding_v1()?;
            read_automorphed_constant_v1(
                profile,
                exponent,
                binding.constant_limb_pointers(),
                direct_objects,
                &mut constant,
                &mut workspace,
                &mut input,
                &mut input_snapshot,
            )?;
        }
        let mut first_digit = 0_usize;
        while first_digit < profile.gadget_digits {
            let end_digit = hoisted.begin_batch_v1(profile, first_digit)?;
            {
                let binding = ciphertext.sealed_binding_v1()?;
                read_automorphed_linear_batch_v1(
                    profile,
                    exponent,
                    binding.linear_limb_pointers(),
                    direct_objects,
                    &mut hoisted,
                    &mut input,
                    &mut input_snapshot,
                )?;
            }
            for digit_index in first_digit..end_digit {
                for limb_index in 0..profile.moduli.len() {
                    runtime.stored_b_limb(
                        key,
                        evaluated_key_provider,
                        digit_index,
                        limb_index,
                        &mut workspace.evaluated_key,
                    )?;
                    hoisted.fill_digit_limb_v1(
                        profile,
                        digit_index,
                        limb_index,
                        &mut workspace.signed_digit,
                    )?;
                    multiply_accumulate_limb_in_place(
                        profile,
                        &mut constant,
                        limb_index,
                        &mut workspace.evaluated_key,
                        &mut workspace.signed_digit,
                    )?;
                    runtime.seeded_a_limb(
                        key,
                        digit_index,
                        limb_index,
                        &mut workspace.evaluated_key,
                    )?;
                    hoisted.fill_digit_limb_v1(
                        profile,
                        digit_index,
                        limb_index,
                        &mut workspace.signed_digit,
                    )?;
                    multiply_accumulate_limb_in_place(
                        profile,
                        &mut linear,
                        limb_index,
                        &mut workspace.evaluated_key,
                        &mut workspace.signed_digit,
                    )?;
                }
            }
            first_digit = end_digit;
        }
        if input_snapshot.is_none() {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        runtime.validate_provider_state(key, evaluated_key_provider)?;
    }
    // Every C0 pass and all eight complete 38-limb C1 passes have finished and
    // authenticated before the first externally visible output publication.
    for limb_index in 0..profile.moduli.len() {
        output_publication.publish_constant_limb_v1(
            limb_index,
            constant.limb(profile, limb_index),
            direct_objects,
        )?;
    }
    for limb_index in 0..profile.moduli.len() {
        output_publication.publish_linear_limb_v1(
            limb_index,
            linear.limb(profile, limb_index),
            direct_objects,
        )?;
    }
    drop(constant);
    drop(linear);
    output_publication.finish_v1(ciphertext, &runtime.eval_key_binding)
}
/// Portable managed-memory accounting for the direct streaming automorphism.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheStreamingCollectiveAutomorphismAccountingV1 {
    /// Existing exact `2P + 5W + 2L` in-place key-switch heap.
    pub arithmetic_heap_bytes: u64,
    /// Exact 38-by-8KiB ciphertext stripe.
    pub input_stripe_bytes: u64,
    /// Canonical bytes authenticated in one complete 38-limb component pass.
    pub input_component_pass_bytes: u64,
    /// One C0 pass plus eight complete C1 decomposition passes.
    pub authenticated_input_read_bytes: u64,
    /// Existing exact stored-B reads from the seekable evaluated-key provider.
    pub evaluated_key_read_bytes: u64,
    /// Sum of direct ciphertext and seekable evaluated-key authenticated reads.
    pub total_authenticated_read_bytes: u64,
    /// Forward C0 automorphism scatter work not present in the key-switch account.
    pub automorphism_scatter_work_units: u64,
    /// Existing decomposition, multiplication, accumulation, plus C0 scatter.
    pub total_arithmetic_work_units: u64,
    /// Exact heap capacity of 38 authenticated input transactions.
    pub input_transaction_bytes: u64,
    /// Bounded evaluated-key read buffer plus incremental BLAKE3 state.
    pub evaluated_key_provider_read_state_bytes: u64,
    /// Exact preallocated output pointer and publication-receipt heap.
    pub output_publication_heap_bytes: u64,
    /// Complete fixed owner/control state live at the arithmetic peak.
    pub fixed_control_bytes: u64,
    /// Complete managed peak owned by the direct kernel.
    pub kernel_managed_peak_bytes: u64,
    /// Exact retained consuming input-manifest object and heap.
    pub caller_input_manifest_bytes: u64,
    /// Exact retained validated evaluated-key handle and index heap.
    pub caller_validated_key_bytes: u64,
    /// Exact retained reusable runtime and ordered-entry heap.
    pub caller_runtime_bytes: u64,
    /// Whole known operation peak, including caller-retained managed owners.
    pub whole_operation_managed_peak_bytes: u64,
    /// Opaque provider/backend residency is not introspectable through either trait.
    pub opaque_provider_residency_included: bool,
}
/// Return exact target-layout accounting for the frozen release profile.
pub fn zk_ams_mkhe_streaming_collective_automorphism_accounting_v1()
-> Result<ZkAmsMkheStreamingCollectiveAutomorphismAccountingV1, ZkAmsMkheErrorV1> {
    use core::mem::size_of;
    let profile = release_profile_v1();
    profile.validate()?;
    let limbs = profile.moduli.len();
    let seekable = seekable_evaluated_key_accounting(&profile)?;
    let arithmetic_heap_bytes = seekable
        .output_accumulator_bytes
        .checked_add(seekable.signed_decomposition_scratch_bytes)
        .and_then(|bytes| bytes.checked_add(seekable.ntt_limb_scratch_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let input_stripe_bytes = limbs
        .checked_mul(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let input_component_pass_bytes = seekable
        .native_polynomial_allocation_bytes
        .checked_add(
            u64::try_from(
                limbs
                    .checked_mul(STREAMING_COLLECTIVE_LIMB_COUNT_BYTES_V1)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let authenticated_input_read_bytes = input_component_pass_bytes
        .checked_mul(9)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let evaluated_key_read_bytes = seekable.per_key_switch_read_bytes;
    let total_authenticated_read_bytes = authenticated_input_read_bytes
        .checked_add(evaluated_key_read_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let automorphism_scatter_work_units = profile
        .ring_degree
        .checked_mul(limbs)
        .and_then(|work| u64::try_from(work).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let total_arithmetic_work_units = seekable
        .total_key_switch_work_units
        .checked_add(automorphism_scatter_work_units)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if total_arithmetic_work_units > profile.max_work_units {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let input_transaction_bytes = limbs
        .checked_mul(size_of::<ZkAmsMkheDirectObjectReadTransactionV1>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let evaluated_key_provider_read_state_bytes = seekable
        .provider_read_buffer_bytes
        .checked_add(seekable.provider_hash_state_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let output_publication_heap_bytes = limbs
        .checked_mul(2)
        .and_then(|objects| {
            objects.checked_mul(
                size_of::<ZkAmsMkheDirectObjectPointerV1>()
                    + size_of::<
                        super::super::direct_object_transport::ZkAmsMkheDirectObjectPublicationReceiptV1,
                    >(),
            )
        })
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let crt_residue_bytes = limbs
        .checked_mul(size_of::<u64>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let fixed_control_bytes = u64::try_from(
        2_usize
            .checked_mul(size_of::<RnsPolynomial>())
            .and_then(|bytes| bytes.checked_add(size_of::<KeySwitchLimbWorkspaceV1>()))
            .and_then(|bytes| bytes.checked_add(size_of::<StreamingAutomorphismHybridBatchV1>()))
            .and_then(|bytes| bytes.checked_add(size_of::<StreamingAutomorphismInputScratchV1>()))
            .and_then(|bytes| {
                bytes.checked_add(size_of::<ZkAmsMkheStreamingCollectiveAutomorphismOutputV1>())
            })
            .and_then(|bytes| {
                bytes.checked_add(size_of::<Option<StreamingAutomorphismInputSnapshotV1>>())
            })
            .and_then(|bytes| {
                2_usize
                    .checked_mul(size_of::<&mut [u64]>())
                    .and_then(|views| bytes.checked_add(views))
            })
            .and_then(|bytes| bytes.checked_add(size_of::<usize>()))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let kernel_managed_peak_bytes = arithmetic_heap_bytes
        .checked_add(input_stripe_bytes)
        .and_then(|bytes| bytes.checked_add(crt_residue_bytes))
        .and_then(|bytes| bytes.checked_add(input_transaction_bytes))
        .and_then(|bytes| bytes.checked_add(evaluated_key_provider_read_state_bytes))
        .and_then(|bytes| bytes.checked_add(output_publication_heap_bytes))
        .and_then(|bytes| bytes.checked_add(fixed_control_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let caller_input_manifest_bytes = u64::try_from(
        size_of::<ZkAmsMkheStreamingCollectiveCiphertextV1>()
            .checked_add(
                limbs
                    .checked_mul(4)
                    .and_then(|objects| {
                        objects.checked_mul(size_of::<ZkAmsMkheDirectObjectPointerV1>())
                    })
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .and_then(|bytes| {
                limbs
                    .checked_mul(4)
                    .and_then(|objects| {
                        objects.checked_mul(size_of::<ZkAmsMkheDirectObjectReadReceiptV1>())
                    })
                    .and_then(|heap| bytes.checked_add(heap))
            })
            .and_then(|bytes| {
                limbs
                    .checked_mul(2)
                    .and_then(|objects| {
                        objects.checked_mul(size_of::<
                            super::super::direct_object_transport::ZkAmsMkheDirectObjectPublicationReceiptV1,
                        >())
                    })
                    .and_then(|heap| bytes.checked_add(heap))
            })
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let caller_validated_key_bytes =
        u64::try_from(size_of::<ZkAmsMkheValidatedCollectiveEvaluatedKeyV1>())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .checked_add(seekable.validation_metadata_bytes)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let entry_heap_bytes = runtime_entry_count_v1()?
        .checked_mul(size_of::<ZkAmsMkheCollectiveEvaluatedKeyEntryV1>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    // The release-only `T256` plaintext-modulus variant is a ZST. Tests add a
    // `Tiny(u64)` variant to `BgvProfile`; subtract that cfg-only layout so the
    // public certificate remains byte-identical to the production target.
    let production_runtime_control_bytes = size_of::<ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1>()
        .checked_sub(size_of::<super::super::PlaintextModulus>())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let caller_runtime_bytes = u64::try_from(production_runtime_control_bytes)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .checked_add(entry_heap_bytes)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let whole_operation_managed_peak_bytes = kernel_managed_peak_bytes
        .checked_add(caller_input_manifest_bytes)
        .and_then(|bytes| bytes.checked_add(caller_validated_key_bytes))
        .and_then(|bytes| bytes.checked_add(caller_runtime_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    Ok(ZkAmsMkheStreamingCollectiveAutomorphismAccountingV1 {
        arithmetic_heap_bytes,
        input_stripe_bytes,
        input_component_pass_bytes,
        authenticated_input_read_bytes,
        evaluated_key_read_bytes,
        total_authenticated_read_bytes,
        automorphism_scatter_work_units,
        total_arithmetic_work_units,
        input_transaction_bytes,
        evaluated_key_provider_read_state_bytes,
        output_publication_heap_bytes,
        fixed_control_bytes,
        kernel_managed_peak_bytes,
        caller_input_manifest_bytes,
        caller_validated_key_bytes,
        caller_runtime_bytes,
        whole_operation_managed_peak_bytes,
        opaque_provider_residency_included: false,
    })
}
fn runtime_entry_count_v1() -> Result<usize, ZkAmsMkheErrorV1> {
    ZK_AMS_T256_GALOIS_KEY_COUNT_V1
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
#[cfg(test)]
mod tests {
    use super::*;
    const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
    const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];
    fn hybrid_profile_v1() -> BgvProfile {
        BgvProfile {
            profile_id: [0x79; 32],
            ring_degree: 8,
            moduli: &TEST_MODULI,
            negacyclic_roots: &TEST_ROOTS,
            plaintext_modulus: super::super::super::PlaintextModulus::Tiny(17),
            error_eta: 2,
            hybrid_rns_decomposition: true,
            gadget_base_log: 60,
            gadget_digits: TEST_MODULI.len(),
            max_ciphertext_bytes: 1 << 20,
            max_evaluated_key_bytes: 16 << 20,
            max_round_bytes: 16 << 20,
            max_share_bytes: 4 << 20,
            max_workspace_bytes: 16 << 20,
            max_work_units: 16 << 20,
        }
    }
    #[test]
    fn direct_forward_scatter_digits_match_native_inverse_gather_v1() {
        let profile = hybrid_profile_v1();
        let polynomial =
            RnsPolynomial::from_signed(&profile, &[17, -19, 23, -29, 31, -37, 41, -43]).unwrap();
        let exponent = 3;
        let expected = HoistedHybridDigitBatchV1::new_automorphed_batch(
            &profile,
            &polynomial,
            exponent,
            0,
            profile.gadget_digits,
        )
        .unwrap();
        let mut direct = StreamingAutomorphismHybridBatchV1::new_v1(&profile).unwrap();
        assert_eq!(direct.begin_batch_v1(&profile, 0).unwrap(), 2);
        let mut residues = [0_u64; TEST_MODULI.len()];
        for coefficient in 0..profile.ring_degree {
            for (limb, residue) in residues.iter_mut().enumerate() {
                *residue = polynomial.limb(&profile, limb)[coefficient];
            }
            direct
                .absorb_coefficient_v1(&profile, exponent, coefficient, &residues)
                .unwrap();
        }
        for digit in 0..profile.gadget_digits {
            for limb in 0..profile.moduli.len() {
                let mut expected_limb = [0_u64; 8];
                let mut direct_limb = [0_u64; 8];
                expected
                    .fill_digit_limb(digit, limb, &mut expected_limb)
                    .unwrap();
                direct
                    .fill_digit_limb_v1(&profile, digit, limb, &mut direct_limb)
                    .unwrap();
                assert_eq!(direct_limb, expected_limb);
            }
        }
    }
    #[test]
    fn release_streaming_automorphism_accounting_is_exact_v1() {
        use core::mem::size_of;
        assert_eq!(size_of::<ZkAmsMkheDirectObjectPointerV1>(), 80);
        assert_eq!(size_of::<ZkAmsMkheDirectObjectReadReceiptV1>(), 248);
        assert_eq!(size_of::<ZkAmsMkheDirectObjectReadTransactionV1>(), 2_112);
        assert_eq!(
            size_of::<
                super::super::super::direct_object_transport::ZkAmsMkheDirectObjectPublicationReceiptV1,
            >(),
            704
        );
        assert_eq!(
            size_of::<ZkAmsMkheStreamingCollectiveAutomorphismOutputV1>(),
            8_776
        );
        assert_eq!(size_of::<StreamingAutomorphismHybridBatchV1>(), 656);
        assert_eq!(size_of::<StreamingAutomorphismInputScratchV1>(), 72);
        assert_eq!(size_of::<KeySwitchLimbWorkspaceV1>(), 48);
        assert_eq!(
            size_of::<Option<StreamingAutomorphismInputSnapshotV1>>(),
            65
        );
        assert_eq!(size_of::<ZkAmsMkheStreamingCollectiveCiphertextV1>(), 656);
        assert_eq!(size_of::<ZkAmsMkheValidatedCollectiveEvaluatedKeyV1>(), 528);
        assert_eq!(
            size_of::<ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1>()
                - size_of::<super::super::super::PlaintextModulus>(),
            1_056
        );
        let accounting = zk_ams_mkhe_streaming_collective_automorphism_accounting_v1().unwrap();
        assert_eq!(accounting.arithmetic_heap_bytes, 87_031_808);
        assert_eq!(accounting.input_stripe_bytes, 311_296);
        assert_eq!(accounting.input_component_pass_bytes, 39_846_040);
        assert_eq!(accounting.authenticated_input_read_bytes, 358_614_360);
        assert_eq!(accounting.evaluated_key_read_bytes, 1_514_143_744);
        assert_eq!(accounting.total_authenticated_read_bytes, 1_872_758_104);
        assert_eq!(accounting.automorphism_scatter_work_units, 4_980_736);
        assert_eq!(accounting.total_arithmetic_work_units, 10_494_410_752);
        assert_eq!(accounting.input_transaction_bytes, 80_256);
        assert_eq!(accounting.evaluated_key_provider_read_state_bytes, 10_112);
        assert_eq!(accounting.output_publication_heap_bytes, 59_584);
        assert_eq!(accounting.fixed_control_bytes, 9_705);
        assert_eq!(accounting.kernel_managed_peak_bytes, 87_503_065);
        assert_eq!(accounting.caller_input_manifest_bytes, 104_016);
        assert_eq!(accounting.caller_validated_key_bytes, 71_664);
        assert_eq!(accounting.caller_runtime_bytes, 4_896);
        assert_eq!(accounting.whole_operation_managed_peak_bytes, 87_683_641);
        assert!(!accounting.opaque_provider_residency_included);
        assert!(accounting.whole_operation_managed_peak_bytes < 84 * 1024 * 1024);
    }
}
