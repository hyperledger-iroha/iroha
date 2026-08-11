//! Private component-major digest and two-limb fresh-encryption prerequisite.
//!
//! This module owns no source record or release authority. Its bounded kernel
//! remains private until an authenticated confidential-store integration can
//! consume the typed public-limb borrows without widening witness access.
//!
//! TODO: replace the caller-owned materialized collective key and packed
//! decoder with one authenticated component-major source that validates and
//! streams a single limb at a time. The 9,445,392-byte figure below is a
//! current working-set accounting: it charges the borrowed canonical plaintext
//! view, three witness owners, two limb owners, and one spool record. It is
//! not a claim that this kernel owns all of those bytes, nor a release claim:
//! the current path still retains the full key, immutable packed artifact, and
//! sequential prevalidation scratch. A future source-owned path must account
//! for and remove those payloads independently before this can be released.

use super::super::{
    PlaintextModulus, bytes_mod_u64, cyclic_ntt, inverse_cyclic_ntt, mod_add, mod_inverse, mod_mul,
    packing::ValidatedT256PackedPlaintextV1, signed_mod,
    t256_centered_residue_with_modulus_residue,
};
use super::*;
use crate::vega::VEGA_T256_SCALAR_MODULUS_BE_V1;

// BEGIN PRIVATE INCREMENTAL COLLECTIVE ENCRYPTION PREREQUISITE V1

const COLLECTIVE_RNS_COMPONENT_COUNT_V1: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CollectiveRnsComponentV1 {
    First,
    Second,
}

impl CollectiveRnsComponentV1 {
    const fn ordinal(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }
}

/// Incremental hash state for exactly two flat, component-major RNS
/// polynomials. Limb and modulus ordinals are validation inputs only: the
/// legacy framing commits one big-endian flat coefficient count followed by
/// every big-endian residue of component zero, then repeats that framing for
/// component one.
///
/// The state deliberately implements neither `Clone` nor `Debug`. Its sponge
/// is allocated before the first component residue is absorbed and finalized
/// through a mutable borrow.
struct ComponentMajorRnsDigestStateV1 {
    hash: Box<Keccak256>,
    ring_degree: usize,
    moduli: &'static [u64],
    next_component: usize,
    next_limb: usize,
}

impl ComponentMajorRnsDigestStateV1 {
    fn new(mut hash: Box<Keccak256>, profile: &BgvProfile) -> Result<Self, ZkAmsMkheErrorV1> {
        profile.validate()?;
        let coefficient_count = profile
            .ring_degree
            .checked_mul(profile.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        hash.update(
            &u32::try_from(coefficient_count)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        Ok(Self {
            hash,
            ring_degree: profile.ring_degree,
            moduli: profile.moduli,
            next_component: 0,
            next_limb: 0,
        })
    }

    fn expects(&self, component: CollectiveRnsComponentV1, limb: usize) -> bool {
        self.next_component == component.ordinal() && self.next_limb == limb
    }

    fn absorb_next_limb_v1(
        &mut self,
        component: CollectiveRnsComponentV1,
        limb: usize,
        coefficients: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if !self.expects(component, limb)
            || limb >= self.moduli.len()
            || coefficients.len() != self.ring_degree
            || coefficients
                .iter()
                .any(|coefficient| *coefficient >= self.moduli[limb])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        for coefficient in coefficients {
            self.hash.update(&coefficient.to_be_bytes());
        }
        self.next_limb = self
            .next_limb
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if self.next_limb == self.moduli.len() {
            self.next_component = self
                .next_component
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            self.next_limb = 0;
            if self.next_component < COLLECTIVE_RNS_COMPONENT_COUNT_V1 {
                let coefficient_count = self
                    .ring_degree
                    .checked_mul(self.moduli.len())
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                self.hash.update(
                    &u32::try_from(coefficient_count)
                        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                        .to_be_bytes(),
                );
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.next_component != COLLECTIVE_RNS_COMPONENT_COUNT_V1 || self.next_limb != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let mut digest = [0_u8; 32];
        self.hash.finalize_into(&mut digest);
        if digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        Ok(digest)
    }
}

/// Exact component-major collective-public-key digest cursor. This is a hash
/// parity helper only, not a validated-key or source-record capability.
#[allow(
    dead_code,
    reason = "private incremental source prerequisite is not wired to an external store yet"
)]
pub(super) struct ComponentMajorCollectivePublicKeyDigestV1 {
    state: ComponentMajorRnsDigestStateV1,
}

#[allow(
    dead_code,
    reason = "private incremental source prerequisite is not wired to an external store yet"
)]
impl ComponentMajorCollectivePublicKeyDigestV1 {
    pub(super) fn new(
        key: &ZkAmsMkheCollectivePublicKeyV1,
        profile: &BgvProfile,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        key.public_a.validate(profile)?;
        key.collective_public_b.validate(profile)?;
        let mut hash = Box::new(Keccak256::new());
        hash.update(COLLECTIVE_PUBLIC_KEY_DOMAIN_V1);
        hash.update(&[key.version]);
        hash.update(&key.profile_digest);
        hash.update(&key.security_certificate_digest);
        hash.update(&key.roster_digest);
        hash.update(&key.key_material_digest);
        hash.update(&key.epoch.to_be_bytes());
        hash.update(&key.transcript_digest);
        for party in &key.parties.parties {
            hash.update(&party.to_bytes());
        }
        Ok(Self {
            state: ComponentMajorRnsDigestStateV1::new(hash, profile)?,
        })
    }

    pub(super) fn absorb_next_public_a_limb_v1(
        &mut self,
        limb: usize,
        coefficients: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.state
            .absorb_next_limb_v1(CollectiveRnsComponentV1::First, limb, coefficients)
    }

    pub(super) fn absorb_next_collective_public_b_limb_v1(
        &mut self,
        limb: usize,
        coefficients: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.state
            .absorb_next_limb_v1(CollectiveRnsComponentV1::Second, limb, coefficients)
    }

    pub(super) fn finish(
        self,
        share_digests: &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if share_digests.contains(&[0; 32])
            || self.state.next_component != COLLECTIVE_RNS_COMPONENT_COUNT_V1
            || self.state.next_limb != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut state = self.state;
        for share_digest in share_digests {
            state.hash.update(share_digest);
        }
        state
            .finish()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)
    }
}

#[derive(Clone, Copy)]
struct IncrementalCollectiveKeyBindingV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
}

impl IncrementalCollectiveKeyBindingV1 {
    const fn from_validated_key_v1(key: &ZkAmsMkheCollectivePublicKeyV1) -> Self {
        Self {
            profile_digest: key.profile_digest,
            roster_digest: key.roster_digest,
            epoch: key.epoch,
        }
    }
}

/// Private proof that the full legacy key was validated exactly once before
/// any plaintext scratch allocation or randomness. It is a transitional
/// prerequisite; the future external source must mint an equivalent binding
/// while streaming the two component-major polynomials.
#[derive(Clone, Copy)]
struct ValidatedIncrementalCollectiveKeyV1<'key> {
    key: &'key ZkAmsMkheCollectivePublicKeyV1,
    binding: IncrementalCollectiveKeyBindingV1,
}

impl<'key> ValidatedIncrementalCollectiveKeyV1<'key> {
    fn new(
        key: &'key ZkAmsMkheCollectivePublicKeyV1,
        profile: &BgvProfile,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        key.validate(profile)?;
        Ok(Self {
            key,
            binding: IncrementalCollectiveKeyBindingV1::from_validated_key_v1(key),
        })
    }
}

/// Exact component-major collective-ciphertext digest cursor. Its framing is
/// byte-for-byte identical to `ZkAmsMkheCollectiveCiphertextV1::compute_digest`.
struct ComponentMajorCollectiveCiphertextDigestV1 {
    state: ComponentMajorRnsDigestStateV1,
}

impl ComponentMajorCollectiveCiphertextDigestV1 {
    fn new_with_preallocated_hash_v1(
        profile: &BgvProfile,
        key: IncrementalCollectiveKeyBindingV1,
        transcript_digest: [u8; 32],
        sample_index: u64,
        mut hash: Box<Keccak256>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if key.profile_digest != profile.digest()?
            || key.roster_digest == [0; 32]
            || key.epoch == 0
            || transcript_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        hash.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
        hash.update(&key.profile_digest);
        hash.update(&key.roster_digest);
        hash.update(&key.epoch.to_be_bytes());
        hash.update(&transcript_digest);
        hash.update(&sample_index.to_be_bytes());
        hash.update(&[0]);
        Ok(Self {
            state: ComponentMajorRnsDigestStateV1::new(hash, profile)?,
        })
    }

    fn expects(&self, component: CollectiveRnsComponentV1, limb: usize) -> bool {
        self.state.expects(component, limb)
    }

    fn absorb_next_limb_v1(
        &mut self,
        component: CollectiveRnsComponentV1,
        limb: usize,
        coefficients: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.state
            .absorb_next_limb_v1(component, limb, coefficients)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)
    }

    fn finish(self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.state
            .finish()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)
    }
}

/// Heap-stable, optimizer-resistant owner for one reusable release-RNS limb.
/// It is allocated while zero and deliberately implements neither `Clone` nor
/// `Debug`.
struct ZeroizingCollectiveEncryptionLimbV1(Box<[u64]>);

#[cfg(test)]
std::thread_local! {
    static COLLECTIVE_ENCRYPTION_LIMB_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

impl ZeroizingCollectiveEncryptionLimbV1 {
    fn new_zeroed_v1(coefficient_count: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        coefficients.resize(coefficient_count, 0);
        Ok(Self(coefficients.into_boxed_slice()))
    }

    fn as_slice(&self) -> &[u64] {
        &self.0
    }

    fn as_mut_slice(&mut self) -> &mut [u64] {
        &mut self.0
    }
}

impl Drop for ZeroizingCollectiveEncryptionLimbV1 {
    fn drop(&mut self) {
        clear_secret_u64_slice_v1(self.0.as_mut());

        #[cfg(test)]
        if self.0.iter().all(|coefficient| *coefficient == 0) {
            let _ = COLLECTIVE_ENCRYPTION_LIMB_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
    }
}

/// Heap-stable, zeroizing owner for one of the three signed RLWE witnesses.
/// It is allocated before entropy is requested and filled in place, so moves
/// after sampling move only its box pointer.
struct ZeroizingCollectiveEncryptionWitnessV1(Box<[i64]>);

#[cfg(test)]
std::thread_local! {
    static COLLECTIVE_ENCRYPTION_WITNESS_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

impl ZeroizingCollectiveEncryptionWitnessV1 {
    fn new_zeroed_v1(coefficient_count: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        coefficients.resize(coefficient_count, 0);
        Ok(Self(coefficients.into_boxed_slice()))
    }

    fn as_slice(&self) -> &[i64] {
        &self.0
    }

    fn as_mut_slice(&mut self) -> &mut [i64] {
        &mut self.0
    }

    fn is_zero(&self) -> bool {
        self.0.iter().all(|coefficient| *coefficient == 0)
    }
}

impl Drop for ZeroizingCollectiveEncryptionWitnessV1 {
    fn drop(&mut self) {
        clear_secret_i64_slice_v1(self.0.as_mut());

        #[cfg(test)]
        if self.is_zero() {
            let _ = COLLECTIVE_ENCRYPTION_WITNESS_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
    }
}

/// Exactly two reusable one-limb owners. No third arithmetic/result limb is
/// allocated: the left owner becomes the result, while the right owner is
/// erased and reused after the NTT product.
struct ZeroizingCollectiveEncryptionWorkspaceV1 {
    left: ZeroizingCollectiveEncryptionLimbV1,
    right: ZeroizingCollectiveEncryptionLimbV1,
}

/// All heap owners established before the first entropy draw. The bounded limb
/// and witness owners are fallibly allocated; fixed-size sponge boxes retain
/// Rust's usual OOM-abort semantics and are never claimed to be fallible.
struct PreallocatedCollectiveEncryptionEntropyOwnersV1 {
    nonce: ZeroizingEncryptionNonce,
    nonce_hash: Option<Box<Keccak256>>,
    transcript_hash: Option<Box<Keccak256>>,
    ciphertext_hash: Option<Box<Keccak256>>,
}

impl PreallocatedCollectiveEncryptionEntropyOwnersV1 {
    fn new_zeroed_v1() -> Self {
        Self {
            nonce: ZeroizingEncryptionNonce::zeroed(),
            nonce_hash: Some(Box::new(Keccak256::new())),
            transcript_hash: Some(Box::new(Keccak256::new())),
            ciphertext_hash: Some(Box::new(Keccak256::new())),
        }
    }
}

impl ZeroizingCollectiveEncryptionWorkspaceV1 {
    fn new_zeroed_v1(coefficient_count: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        Ok(Self {
            left: ZeroizingCollectiveEncryptionLimbV1::new_zeroed_v1(coefficient_count)?,
            right: ZeroizingCollectiveEncryptionLimbV1::new_zeroed_v1(coefficient_count)?,
        })
    }
}

/// Typed immutable borrow of one completed public ciphertext limb. The
/// component/limb/modulus association cannot be changed by the writer that
/// receives this borrow.
struct FilledIncrementalCollectiveCiphertextLimbV1<'limb> {
    component: CollectiveRnsComponentV1,
    limb: usize,
    modulus: u64,
    coefficients: &'limb [u64],
}

#[allow(
    dead_code,
    reason = "private source prerequisite is parity-tested before confidential-store wiring"
)]
impl FilledIncrementalCollectiveCiphertextLimbV1<'_> {
    fn component(&self) -> CollectiveRnsComponentV1 {
        self.component
    }

    fn limb(&self) -> usize {
        self.limb
    }

    fn modulus(&self) -> u64 {
        self.modulus
    }

    fn coefficients(&self) -> &[u64] {
        self.coefficients
    }
}

/// Private, non-authorizing one-limb encryption kernel.
///
/// The only retained secret state is the opening nonce and the three native
/// signed witnesses `(r,e0,e1)`. Canonical coefficient bytes are borrowed from
/// one artifact that was validated before randomness, and the two reusable
/// limb owners are the complete ring-arithmetic workspace. Output order is
/// fixed to all 38 `c0` limbs followed by all 38 `c1` limbs so the incremental
/// digest is byte-identical to the native ciphertext digest.
///
/// A failed or unwinding fill poisons the kernel. Successful calls return only
/// an immutable public-output borrow; no witness borrow, callback, receipt, or
/// source authority leaves this boundary.
#[allow(
    dead_code,
    reason = "private source prerequisite is parity-tested before confidential-store wiring"
)]
struct IncrementalCollectiveEncryptionKernelV1<'plaintext, 'key> {
    profile: BgvProfile,
    key: &'key ZkAmsMkheCollectivePublicKeyV1,
    canonical_plaintext: &'plaintext [[u8; 32]],
    input_identity: CollectiveEncryptionInputIdentityV1,
    transcript_digest: [u8; 32],
    ephemeral: ZeroizingCollectiveEncryptionWitnessV1,
    error_zero: ZeroizingCollectiveEncryptionWitnessV1,
    error_one: ZeroizingCollectiveEncryptionWitnessV1,
    workspace: ZeroizingCollectiveEncryptionWorkspaceV1,
    ciphertext_digest: ComponentMajorCollectiveCiphertextDigestV1,
    poisoned: bool,
}

#[allow(
    dead_code,
    reason = "private source prerequisite is parity-tested before confidential-store wiring"
)]
struct CompletedIncrementalCollectiveEncryptionV1 {
    transcript_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
}

#[allow(
    dead_code,
    reason = "private source prerequisite is parity-tested before confidential-store wiring"
)]
impl<'plaintext, 'key> IncrementalCollectiveEncryptionKernelV1<'plaintext, 'key> {
    fn new_release_v1<R: MaskedRelaxedRandomSourceV1>(
        key: &'key ZkAmsMkheCollectivePublicKeyV1,
        layout: ZkAmsT256PackingLayoutV1,
        plaintext: &'plaintext ZkAmsT256PackedPlaintextV1,
        sample_index: u64,
        random: &mut R,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let validated_key = ValidatedIncrementalCollectiveKeyV1::new(key, &profile)?;
        if key.security_certificate_digest != release_security_certificate_digest()?
            || sample_index >= zk_ams_mkhe_release_manifest_v1()?.max_samples_per_secret_epoch
            || layout.profile_digest != key.profile_digest
            || plaintext.profile_digest != key.profile_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let _validated =
            ValidatedT256PackedPlaintextV1::validate_for_release_limb_stream_v1(layout, plaintext)?;
        Self::new_with_validated_key_v1(
            &profile,
            validated_key,
            &plaintext.coefficients,
            CollectiveEncryptionInputTopologyV1::from_packed(layout, plaintext),
            sample_index,
            random,
        )
    }

    fn new_validated_inner_v1<R: MaskedRelaxedRandomSourceV1>(
        profile: &BgvProfile,
        key: &'key ZkAmsMkheCollectivePublicKeyV1,
        canonical_plaintext: &'plaintext [[u8; 32]],
        topology: CollectiveEncryptionInputTopologyV1,
        sample_index: u64,
        random: &mut R,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let validated_key = ValidatedIncrementalCollectiveKeyV1::new(key, profile)?;
        Self::new_with_validated_key_v1(
            profile,
            validated_key,
            canonical_plaintext,
            topology,
            sample_index,
            random,
        )
    }

    fn new_with_validated_key_v1<R: MaskedRelaxedRandomSourceV1>(
        profile: &BgvProfile,
        validated_key: ValidatedIncrementalCollectiveKeyV1<'key>,
        canonical_plaintext: &'plaintext [[u8; 32]],
        topology: CollectiveEncryptionInputTopologyV1,
        sample_index: u64,
        random: &mut R,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let key = validated_key.key;
        profile.validate()?;
        let key_binding = validated_key.binding;
        validate_incremental_canonical_plaintext_v1(profile, canonical_plaintext)?;
        if topology.layout_digest == [0; 32]
            || topology.plaintext_used_slots == 0
            || usize::try_from(topology.plaintext_used_slots)
                .map_or(true, |used_slots| used_slots > profile.ring_degree)
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        checked_ring_multiplication_work(profile, 2)?;
        // Both all-zero limb allocations precede the first secret byte.
        let workspace =
            ZeroizingCollectiveEncryptionWorkspaceV1::new_zeroed_v1(profile.ring_degree)?;
        let mut ephemeral =
            ZeroizingCollectiveEncryptionWitnessV1::new_zeroed_v1(profile.ring_degree)?;
        let mut error_zero =
            ZeroizingCollectiveEncryptionWitnessV1::new_zeroed_v1(profile.ring_degree)?;
        let mut error_one =
            ZeroizingCollectiveEncryptionWitnessV1::new_zeroed_v1(profile.ring_degree)?;
        let PreallocatedCollectiveEncryptionEntropyOwnersV1 {
            mut nonce,
            mut nonce_hash,
            mut transcript_hash,
            mut ciphertext_hash,
        } = PreallocatedCollectiveEncryptionEntropyOwnersV1::new_zeroed_v1();
        derive_collective_encryption_nonce_into_v1(
            random,
            &mut nonce,
            nonce_hash
                .take()
                .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?,
        )?;
        let input_identity = CollectiveEncryptionInputIdentityV1 {
            topology,
            encryption_nonce: nonce,
        };
        let transcript_digest = collective_encryption_transcript_digest_with_preallocated_hash_v1(
            key,
            topology,
            sample_index,
            input_identity.encryption_nonce.as_bytes(),
            transcript_hash
                .take()
                .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?,
        );
        if transcript_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::RandomUnavailable);
        }
        let ciphertext_digest =
            ComponentMajorCollectiveCiphertextDigestV1::new_with_preallocated_hash_v1(
                profile,
                key_binding,
                transcript_digest,
                sample_index,
                ciphertext_hash
                    .take()
                    .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?,
            )?;
        // Nonce derivation deliberately precedes every RLWE witness sample.
        sample_nonzero_ternary_into_v1(profile, random, &mut ephemeral)?;
        sample_bounded_error_into_v1(profile, random, &mut error_zero)?;
        sample_bounded_error_into_v1(profile, random, &mut error_one)?;
        Ok(Self {
            profile: profile.clone(),
            key,
            canonical_plaintext,
            input_identity,
            transcript_digest,
            ephemeral,
            error_zero,
            error_one,
            workspace,
            ciphertext_digest,
            poisoned: false,
        })
    }

    fn absorb_next_constant_limb_v1(
        &mut self,
        limb: usize,
    ) -> Result<FilledIncrementalCollectiveCiphertextLimbV1<'_>, ZkAmsMkheErrorV1> {
        self.absorb_next_limb_inner_v1(CollectiveRnsComponentV1::First, limb)
    }

    fn absorb_next_linear_limb_v1(
        &mut self,
        limb: usize,
    ) -> Result<FilledIncrementalCollectiveCiphertextLimbV1<'_>, ZkAmsMkheErrorV1> {
        self.absorb_next_limb_inner_v1(CollectiveRnsComponentV1::Second, limb)
    }

    fn absorb_next_limb_inner_v1(
        &mut self,
        component: CollectiveRnsComponentV1,
        limb: usize,
    ) -> Result<FilledIncrementalCollectiveCiphertextLimbV1<'_>, ZkAmsMkheErrorV1> {
        if self.poisoned
            || !self.ciphertext_digest.expects(component, limb)
            || limb >= self.profile.moduli.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let modulus = self.profile.moduli[limb];
        let root = self.profile.negacyclic_roots[limb];
        let source = match component {
            CollectiveRnsComponentV1::First => {
                self.key.collective_public_b.limb(&self.profile, limb)
            }
            CollectiveRnsComponentV1::Second => self.key.public_a.limb(&self.profile, limb),
        };
        if source.len() != self.profile.ring_degree
            || source.iter().any(|coefficient| *coefficient >= modulus)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        self.poisoned = true;
        clear_secret_u64_slice_v1(self.workspace.left.as_mut_slice());
        clear_secret_u64_slice_v1(self.workspace.right.as_mut_slice());
        self.workspace.left.as_mut_slice().copy_from_slice(source);
        negacyclic_multiply_signed_rhs_two_limb_v1(
            self.workspace.left.as_mut_slice(),
            self.workspace.right.as_mut_slice(),
            self.ephemeral.as_slice(),
            modulus,
            root,
        )?;
        clear_secret_u64_slice_v1(self.workspace.right.as_mut_slice());
        match component {
            CollectiveRnsComponentV1::First => {
                fill_incremental_plaintext_limb_v1(
                    &self.profile,
                    self.canonical_plaintext,
                    limb,
                    self.workspace.right.as_mut_slice(),
                )?;
                add_scaled_error_and_message_in_place_v1(
                    &self.profile,
                    modulus,
                    self.workspace.left.as_mut_slice(),
                    self.error_zero.as_slice(),
                    Some(self.workspace.right.as_slice()),
                )?;
            }
            CollectiveRnsComponentV1::Second => {
                add_scaled_error_and_message_in_place_v1(
                    &self.profile,
                    modulus,
                    self.workspace.left.as_mut_slice(),
                    self.error_one.as_slice(),
                    None,
                )?;
            }
        }
        clear_secret_u64_slice_v1(self.workspace.right.as_mut_slice());
        self.ciphertext_digest.absorb_next_limb_v1(
            component,
            limb,
            self.workspace.left.as_slice(),
        )?;
        self.poisoned = false;
        Ok(FilledIncrementalCollectiveCiphertextLimbV1 {
            component,
            limb,
            modulus,
            coefficients: self.workspace.left.as_slice(),
        })
    }

    fn finish(self) -> Result<CompletedIncrementalCollectiveEncryptionV1, ZkAmsMkheErrorV1> {
        if self.poisoned
            || self.input_identity.encryption_nonce.is_zero()
            || self.input_identity.topology.layout_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let transcript_digest = self.transcript_digest;
        let ciphertext_digest = self.ciphertext_digest.finish()?;
        Ok(CompletedIncrementalCollectiveEncryptionV1 {
            transcript_digest,
            ciphertext_digest,
        })
    }
}

fn validate_incremental_canonical_plaintext_v1(
    profile: &BgvProfile,
    canonical_plaintext: &[[u8; 32]],
) -> Result<(), ZkAmsMkheErrorV1> {
    if canonical_plaintext.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    match profile.plaintext_modulus {
        PlaintextModulus::T256 => {
            if canonical_plaintext
                .iter()
                .any(|coefficient| *coefficient >= VEGA_T256_SCALAR_MODULUS_BE_V1)
            {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
        }
        #[cfg(test)]
        PlaintextModulus::Tiny(plaintext_modulus) => {
            for coefficient in canonical_plaintext {
                if coefficient[..24].iter().any(|byte| *byte != 0) {
                    return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
                }
                let mut low = [0_u8; 8];
                low.copy_from_slice(&coefficient[24..]);
                if u64::from_be_bytes(low) >= plaintext_modulus {
                    return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
                }
            }
        }
    }
    Ok(())
}

fn fill_incremental_plaintext_limb_v1(
    profile: &BgvProfile,
    canonical_plaintext: &[[u8; 32]],
    limb: usize,
    output: &mut [u64],
) -> Result<(), ZkAmsMkheErrorV1> {
    let Some(&modulus) = profile.moduli.get(limb) else {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    };
    if canonical_plaintext.len() != profile.ring_degree || output.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    match profile.plaintext_modulus {
        PlaintextModulus::T256 => {
            let plaintext_modulus_residue = bytes_mod_u64(&VEGA_T256_SCALAR_MODULUS_BE_V1, modulus);
            for (output, coefficient) in output.iter_mut().zip(canonical_plaintext) {
                *output = t256_centered_residue_with_modulus_residue(
                    coefficient,
                    modulus,
                    plaintext_modulus_residue,
                );
            }
        }
        #[cfg(test)]
        PlaintextModulus::Tiny(_) => {
            for (output, coefficient) in output.iter_mut().zip(canonical_plaintext) {
                let mut low = [0_u8; 8];
                low.copy_from_slice(&coefficient[24..]);
                *output = u64::from_be_bytes(low) % modulus;
            }
        }
    }
    Ok(())
}

fn derive_collective_encryption_nonce_into_v1<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
    nonce: &mut ZeroizingEncryptionNonce,
    mut hash: Box<Keccak256>,
) -> Result<(), ZkAmsMkheErrorV1> {
    let mut first = ZeroizingEntropyProbe([0; 32]);
    let mut second = ZeroizingEntropyProbe([0; 32]);
    random
        .fill_bytes(&mut first.0)
        .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
    random
        .fill_bytes(&mut second.0)
        .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
    if first.0 == second.0
        || entropy_probe_has_short_period(&first.0)
        || entropy_probe_has_short_period(&second.0)
    {
        return Err(ZkAmsMkheErrorV1::RandomUnavailable);
    }
    hash.update(COLLECTIVE_ENCRYPTION_NONCE_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&first.0);
    hash.update(&second.0);
    hash.finalize_into(nonce.as_mut_bytes());
    if nonce.is_zero() {
        return Err(ZkAmsMkheErrorV1::RandomUnavailable);
    }
    Ok(())
}

fn collective_encryption_transcript_digest_with_preallocated_hash_v1(
    key: &ZkAmsMkheCollectivePublicKeyV1,
    topology: CollectiveEncryptionInputTopologyV1,
    sample_index: u64,
    encryption_nonce: &[u8; 32],
    mut hash: Box<Keccak256>,
) -> [u8; 32] {
    let chunk_index = topology.plaintext_chunk_index.to_be_bytes();
    let used_slots = topology.plaintext_used_slots.to_be_bytes();
    let sample_index = sample_index.to_be_bytes();
    hash.update(COLLECTIVE_ENCRYPTION_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&key.profile_digest);
    hash.update(&key.security_certificate_digest);
    hash.update(&key.roster_digest);
    hash.update(&key.key_material_digest);
    hash.update(&key.epoch.to_be_bytes());
    hash.update(&key.transcript_digest);
    hash.update(&key.digest);
    hash.update(&0_u32.to_be_bytes());
    hash.update(&5_u32.to_be_bytes());
    hash.update(&32_u32.to_be_bytes());
    hash.update(&topology.layout_digest);
    hash.update(&4_u32.to_be_bytes());
    hash.update(&chunk_index);
    hash.update(&4_u32.to_be_bytes());
    hash.update(&used_slots);
    hash.update(&8_u32.to_be_bytes());
    hash.update(&sample_index);
    hash.update(&32_u32.to_be_bytes());
    hash.update(encryption_nonce);
    let mut digest = [0_u8; 32];
    hash.finalize_into(&mut digest);
    digest
}

fn sample_nonzero_ternary_into_v1<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    random: &mut R,
    output: &mut ZeroizingCollectiveEncryptionWitnessV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let max_bytes = profile
        .ring_degree
        .checked_mul(super::super::MAX_TERNARY_SAMPLE_BYTES_PER_COEFFICIENT_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    super::super::checked_rng_bytes(profile, max_bytes)?;
    if output.as_slice().len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        output.as_mut_slice().fill(0);
        let mut filled = 0;
        for _ in 0..max_bytes {
            let mut byte = ZeroizingRandomByte([0]);
            random
                .fill_bytes(&mut byte.0)
                .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
            for shift in [0, 2, 4, 6] {
                let coefficient = match (byte.0[0] >> shift) & 0x03 {
                    0 => -1,
                    1 => 0,
                    2 => 1,
                    _ => continue,
                };
                output.as_mut_slice()[filled] = coefficient;
                filled += 1;
                if filled == profile.ring_degree {
                    if !output.is_zero() {
                        return Ok(());
                    }
                    break;
                }
            }
            if filled == profile.ring_degree {
                break;
            }
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn sample_bounded_error_into_v1<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    random: &mut R,
    output: &mut ZeroizingCollectiveEncryptionWitnessV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let max_bytes = profile
        .ring_degree
        .checked_mul(usize::from(profile.error_eta))
        .and_then(|value| value.checked_mul(2))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    super::super::checked_rng_bytes(profile, max_bytes)?;
    if output.as_slice().len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for coefficient in output.as_mut_slice() {
        let mut positive = 0_i64;
        let mut negative = 0_i64;
        for _ in 0..profile.error_eta {
            let mut byte = ZeroizingRandomByte([0]);
            random
                .fill_bytes(&mut byte.0)
                .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
            positive += i64::from(byte.0[0] & 1);
            let mut byte = ZeroizingRandomByte([0]);
            random
                .fill_bytes(&mut byte.0)
                .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
            negative += i64::from(byte.0[0] & 1);
        }
        *coefficient = positive - negative;
    }
    Ok(())
}

fn negacyclic_multiply_signed_rhs_two_limb_v1(
    left: &mut [u64],
    right: &mut [u64],
    signed_right: &[i64],
    modulus: u64,
    psi: u64,
) -> Result<(), ZkAmsMkheErrorV1> {
    if left.len() != right.len()
        || left.len() != signed_right.len()
        || left.is_empty()
        || !left.len().is_power_of_two()
        || left.iter().any(|coefficient| *coefficient >= modulus)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut twist = 1_u64;
    for ((left, right), signed_right) in left.iter_mut().zip(right.iter_mut()).zip(signed_right) {
        *left = mod_mul(*left, twist, modulus);
        *right = mod_mul(signed_mod(*signed_right, modulus), twist, modulus);
        twist = mod_mul(twist, psi, modulus);
    }
    let root = mod_mul(psi, psi, modulus);
    cyclic_ntt(left, root, modulus);
    cyclic_ntt(right, root, modulus);
    for (left, right) in left.iter_mut().zip(right.iter().copied()) {
        *left = mod_mul(*left, right, modulus);
    }
    inverse_cyclic_ntt(left, root, modulus)?;
    let inverse_psi = mod_inverse(psi, modulus).ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut untwist = 1_u64;
    for coefficient in left {
        *coefficient = mod_mul(*coefficient, untwist, modulus);
        untwist = mod_mul(untwist, inverse_psi, modulus);
    }
    Ok(())
}

fn add_scaled_error_and_message_in_place_v1(
    profile: &BgvProfile,
    modulus: u64,
    output: &mut [u64],
    error: &[i64],
    message: Option<&[u64]>,
) -> Result<(), ZkAmsMkheErrorV1> {
    if output.len() != profile.ring_degree
        || error.len() != profile.ring_degree
        || message.is_some_and(|message| message.len() != profile.ring_degree)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
    for index in 0..output.len() {
        let scaled_error = mod_mul(
            signed_mod(error[index], modulus),
            plaintext_modulus,
            modulus,
        );
        output[index] = mod_add(output[index], scaled_error, modulus);
        if let Some(message) = message {
            if message[index] >= modulus {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            output[index] = mod_add(output[index], message[index], modulus);
        }
    }
    Ok(())
}

// END PRIVATE INCREMENTAL COLLECTIVE ENCRYPTION PREREQUISITE V1

#[cfg(test)]
mod tests {
    use super::super::super::MaskedRelaxedRandomErrorV1;
    use super::super::tests::{
        KatRandom, encrypt_test_with_opening, test_canonical_plaintext, test_input_topology,
        test_key, test_profile,
    };
    use super::*;

    struct FailingRandom;

    impl MaskedRelaxedRandomSourceV1 for FailingRandom {
        fn fill_bytes(
            &mut self,
            _destination: &mut [u8],
        ) -> Result<(), MaskedRelaxedRandomErrorV1> {
            Err(MaskedRelaxedRandomErrorV1::Unavailable)
        }
    }

    #[test]
    fn component_major_incremental_key_digest_matches_native_and_rejects_order_drift() {
        let profile = test_profile();
        let (key, _) = test_key(0xa4);

        let mut early = ComponentMajorCollectivePublicKeyDigestV1::new(&key, &profile).unwrap();
        assert_eq!(
            early.absorb_next_collective_public_b_limb_v1(
                0,
                key.collective_public_b.limb(&profile, 0),
            ),
            Err(ZkAmsMkheErrorV1::InvalidPolynomial)
        );
        assert_eq!(
            early.finish(&key.share_digests),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );

        let mut incremental =
            ComponentMajorCollectivePublicKeyDigestV1::new(&key, &profile).unwrap();
        assert_eq!(
            incremental.absorb_next_public_a_limb_v1(1, key.public_a.limb(&profile, 1)),
            Err(ZkAmsMkheErrorV1::InvalidPolynomial)
        );
        assert_eq!(
            incremental.absorb_next_public_a_limb_v1(
                0,
                &key.public_a.limb(&profile, 0)[..profile.ring_degree - 1],
            ),
            Err(ZkAmsMkheErrorV1::InvalidPolynomial)
        );
        let mut noncanonical = key.public_a.limb(&profile, 0).to_vec();
        noncanonical[0] = profile.moduli[0];
        assert_eq!(
            incremental.absorb_next_public_a_limb_v1(0, &noncanonical),
            Err(ZkAmsMkheErrorV1::InvalidPolynomial)
        );
        for limb in 0..profile.moduli.len() {
            incremental
                .absorb_next_public_a_limb_v1(limb, key.public_a.limb(&profile, limb))
                .unwrap();
        }
        assert_eq!(
            incremental.absorb_next_public_a_limb_v1(0, key.public_a.limb(&profile, 0)),
            Err(ZkAmsMkheErrorV1::InvalidPolynomial)
        );
        for limb in 0..profile.moduli.len() {
            incremental
                .absorb_next_collective_public_b_limb_v1(
                    limb,
                    key.collective_public_b.limb(&profile, limb),
                )
                .unwrap();
        }
        let incremental_digest = incremental.finish(&key.share_digests).unwrap();
        assert_eq!(incremental_digest, key.digest);
        assert_eq!(
            incremental_digest,
            collective_public_key_digest(&key, &profile).unwrap()
        );

        let release = release_profile_v1();
        let release_flat_coefficient_count = release.ring_degree * release.moduli.len();
        assert_eq!(release_flat_coefficient_count, 4_980_736);
        assert_eq!(
            u32::try_from(release_flat_coefficient_count)
                .unwrap()
                .to_be_bytes(),
            [0x00, 0x4c, 0x00, 0x00]
        );
    }

    #[test]
    fn two_limb_incremental_encryption_matches_native_bytes_digest_and_nonce_lineage() {
        let profile = test_profile();
        let (key, _) = test_key(0xa5);
        let values = [0, 1, 2, 3, 5, 8, 13, 16];
        let sample_index = 37;
        let label = b"two-limb-incremental-parity";
        let (ciphertext, opening, _message, canonical, topology, transcript_digest) =
            encrypt_test_with_opening(&profile, &key, &values, sample_index, label);
        let mut kernel = IncrementalCollectiveEncryptionKernelV1::new_validated_inner_v1(
            &profile,
            &key,
            &canonical,
            topology,
            sample_index,
            &mut KatRandom::new(label),
        )
        .unwrap();

        assert_eq!(
            kernel.input_identity.encryption_nonce.as_bytes(),
            opening.input_identity.encryption_nonce.as_bytes()
        );
        assert_eq!(kernel.ephemeral.as_slice(), opening.ephemeral.coefficients);
        assert_eq!(
            kernel.error_zero.as_slice(),
            opening.error_zero.coefficients
        );
        assert_eq!(kernel.error_one.as_slice(), opening.error_one.coefficients);

        assert!(matches!(
            kernel.absorb_next_linear_limb_v1(0),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        ));

        for limb in 0..profile.moduli.len() {
            let filled = kernel.absorb_next_constant_limb_v1(limb).unwrap();
            assert_eq!(filled.component(), CollectiveRnsComponentV1::First);
            assert_eq!(filled.limb(), limb);
            assert_eq!(filled.modulus(), profile.moduli[limb]);
            assert_eq!(
                filled.coefficients(),
                ciphertext.constant().limb(&profile, limb)
            );
        }

        assert!(matches!(
            kernel.absorb_next_constant_limb_v1(0),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        ));

        for limb in 0..profile.moduli.len() {
            let filled = kernel.absorb_next_linear_limb_v1(limb).unwrap();
            assert_eq!(filled.component(), CollectiveRnsComponentV1::Second);
            assert_eq!(filled.limb(), limb);
            assert_eq!(filled.modulus(), profile.moduli[limb]);
            assert_eq!(
                filled.coefficients(),
                ciphertext.linear().limb(&profile, limb)
            );
        }

        let completed = kernel.finish().unwrap();
        assert_eq!(completed.transcript_digest, transcript_digest);
        assert_eq!(completed.ciphertext_digest, ciphertext.digest());
        drop(opening);
    }

    #[test]
    fn two_limb_incremental_kernel_rejects_foreign_key_and_zeroizes_preallocated_drop_paths() {
        let reset_drop_audits = || {
            COLLECTIVE_ENCRYPTION_LIMB_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
            ENCRYPTION_NONCE_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
            COLLECTIVE_ENCRYPTION_WITNESS_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
        };
        let drop_audits = || {
            (
                COLLECTIVE_ENCRYPTION_LIMB_ZEROIZED_DROPS_V1.with(std::cell::Cell::get),
                ENCRYPTION_NONCE_ZEROIZED_DROPS_V1.with(std::cell::Cell::get),
                COLLECTIVE_ENCRYPTION_WITNESS_ZEROIZED_DROPS_V1.with(std::cell::Cell::get),
            )
        };
        let profile = test_profile();
        let (mut key, _) = test_key(0xa6);
        let canonical = test_canonical_plaintext(&[0, 1, 2, 3, 5, 8, 13, 16]);
        let topology = test_input_topology(&profile, b"incremental-poison");

        reset_drop_audits();
        let mut failing_random = FailingRandom;
        assert!(matches!(
            IncrementalCollectiveEncryptionKernelV1::new_validated_inner_v1(
                &profile,
                &key,
                &canonical,
                topology,
                38,
                &mut failing_random,
            ),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));
        assert_eq!(drop_audits(), (2, 1, 3));

        key.digest[0] ^= 1;
        let mut healthy_random = KatRandom::new(b"incremental-foreign-key");
        assert!(
            IncrementalCollectiveEncryptionKernelV1::new_validated_inner_v1(
                &profile,
                &key,
                &canonical,
                topology,
                39,
                &mut healthy_random,
            )
            .is_err()
        );
        assert_eq!(drop_audits(), (2, 1, 3));

        let mut witness = ZeroizingCollectiveEncryptionWitnessV1::new_zeroed_v1(8).unwrap();
        assert!(witness.is_zero());
        witness.as_mut_slice()[0] = 1;
        assert!(!witness.is_zero());
        drop(witness);
        assert_eq!(
            COLLECTIVE_ENCRYPTION_WITNESS_ZEROIZED_DROPS_V1.with(std::cell::Cell::get),
            4
        );
    }

    #[test]
    fn incremental_source_prerequisite_is_private_two_limb_and_non_authorizing() {
        let source = include_str!("incremental_source.rs");
        assert!(source.contains("current working-set accounting"));
        assert!(source.contains("borrowed canonical plaintext"));
        assert!(source.contains("current path still retains the full key"));
        assert!(source.contains("future source-owned path"));
        let prerequisite = source
            .split("// BEGIN PRIVATE INCREMENTAL COLLECTIVE ENCRYPTION PREREQUISITE V1")
            .nth(1)
            .expect("incremental prerequisite start")
            .split("// END PRIVATE INCREMENTAL COLLECTIVE ENCRYPTION PREREQUISITE V1")
            .next()
            .expect("incremental prerequisite end");

        for forbidden in [
            "plaintext_lift",
            "RnsPolynomial",
            "Vec<RnsPolynomial",
            "pub struct",
            "pub fn",
            "source_owned",
            "SourceRecord",
            "mint_",
            "impl FnOnce",
            "fill_public_component",
        ] {
            assert!(
                !prerequisite.contains(forbidden),
                "incremental prerequisite contains forbidden surface: {forbidden}"
            );
        }
        assert!(prerequisite.contains("input_identity: CollectiveEncryptionInputIdentityV1"));
        assert!(prerequisite.contains("key: &'key ZkAmsMkheCollectivePublicKeyV1"));
        assert!(prerequisite.contains("ephemeral: ZeroizingCollectiveEncryptionWitnessV1"));
        assert!(prerequisite.contains("error_zero: ZeroizingCollectiveEncryptionWitnessV1"));
        assert!(prerequisite.contains("error_one: ZeroizingCollectiveEncryptionWitnessV1"));
        assert!(prerequisite.contains("left: ZeroizingCollectiveEncryptionLimbV1"));
        assert!(prerequisite.contains("right: ZeroizingCollectiveEncryptionLimbV1"));
        assert!(prerequisite.contains("CollectiveRnsComponentV1::First"));
        assert!(prerequisite.contains("CollectiveRnsComponentV1::Second"));
        assert!(prerequisite.contains("ValidatedIncrementalCollectiveKeyV1"));
        assert!(prerequisite.contains("self.key.collective_public_b.limb"));
        assert!(prerequisite.contains("self.key.public_a.limb"));
        assert_eq!(prerequisite.matches("key.validate(profile)?").count(), 1);
        assert!(!prerequisite.contains("key.validate(&profile)?"));

        let work_gate = prerequisite
            .find("checked_ring_multiplication_work(profile, 2)?")
            .expect("two-multiplication work gate");
        let workspace = prerequisite
            .find("ZeroizingCollectiveEncryptionWorkspaceV1::new_zeroed_v1")
            .expect("two-limb zeroed workspace allocation");
        let ephemeral_owner = prerequisite
            .find("let mut ephemeral =")
            .expect("ephemeral zeroed witness allocation");
        let entropy_owners = prerequisite
            .find("} = PreallocatedCollectiveEncryptionEntropyOwnersV1::new_zeroed_v1();")
            .expect("nonce and hash owner allocation");
        let nonce = prerequisite
            .find("derive_collective_encryption_nonce_into_v1(")
            .expect("nonce derivation");
        let ephemeral = prerequisite
            .find("sample_nonzero_ternary_into_v1(profile, random, &mut ephemeral)?")
            .expect("in-place ephemeral sampling");
        let error_zero = prerequisite
            .find("sample_bounded_error_into_v1(profile, random, &mut error_zero)?")
            .expect("in-place first error sampling");
        let error_one = prerequisite
            .find("sample_bounded_error_into_v1(profile, random, &mut error_one)?")
            .expect("in-place second error sampling");
        assert!(work_gate < workspace);
        assert!(workspace < ephemeral_owner);
        assert!(ephemeral_owner < entropy_owners);
        assert!(entropy_owners < nonce);
        assert!(workspace < nonce);
        assert!(nonce < ephemeral);
        assert!(ephemeral < error_zero);
        assert!(error_zero < error_one);

        let witness_is_zero = source
            .split("fn is_zero(&self) -> bool {")
            .nth(1)
            .expect("witness is-zero helper")
            .split("    }\n}")
            .next()
            .expect("witness is-zero helper end");
        assert!(witness_is_zero.contains("iter().all"));
        assert!(!witness_is_zero.contains("Vec"));
        assert!(!witness_is_zero.contains("Box"));

        let release = release_profile_v1();
        let limb_bytes = release.ring_degree * core::mem::size_of::<u64>();
        let canonical_plaintext_bytes = release.ring_degree * 32;
        let signed_witness_bytes = release.ring_degree * core::mem::size_of::<i64>();
        assert_eq!(limb_bytes, 1_048_576);
        assert_eq!(
            limb_bytes * super::super::super::manifest::RELEASE_MODULI_V1.len() * 2,
            79_691_776
        );
        assert_eq!(limb_bytes * 2 + 8_208, 2_105_360);
        assert_eq!(
            canonical_plaintext_bytes + signed_witness_bytes * 3 + limb_bytes * 2 + 8_208,
            9_445_392
        );
    }
}
