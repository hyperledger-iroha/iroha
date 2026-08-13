//! Source-authenticated, limb-streamed collective encryption.
//!
//! A staged CPK successor publishes every release limb of common `a` and
//! aggregate `b` under distinct content-addressed kinds, then mints the sole
//! move-only key authority. Fresh encryption authenticates all 76 key-limb
//! objects before entropy, rereads one exact limb into a two-owner arithmetic
//! workspace, and publishes 38 independently addressed `c0` limbs followed by
//! 38 independently addressed `c1` limbs. No native `P`-sized key component or
//! `2P`-sized ciphertext crosses this public boundary.
//!
//! Publication receipts are retained in the authority/manifest and every
//! second source pass must finish under the exact provider snapshot observed by
//! the complete prepass before its corresponding output stage can be sealed.
//! A late failure can leave only unauthorizing CAS orphans: neither a key
//! authority nor a ciphertext manifest is issued.
#[cfg(test)]
use super::super::direct_object_transport::{
    ZkAmsMkheDirectObjectPublishedBindingV1, ZkAmsMkheDirectObjectSealTokenV1,
    ZkAmsMkheDirectObjectStagingTokenV1,
};
use super::super::{
    PlaintextModulus, bytes_mod_u64, cyclic_ntt,
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, ZkAmsMkheDirectObjectCasPublicationV1,
        ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1,
        ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheDirectObjectPublicationTransactionV1,
        ZkAmsMkheDirectObjectReadAtProviderV1, ZkAmsMkheDirectObjectReadReceiptV1,
        ZkAmsMkheDirectObjectReadTransactionV1, validate_zk_ams_mkhe_direct_object_v1,
    },
    inverse_cyclic_ntt,
    manifest::RELEASE_MODULI_V1,
    mod_add, mod_inverse, mod_mul,
    packing::ValidatedT256PackedPlaintextV1,
    signed_mod, t256_centered_residue_with_modulus_residue,
};
use super::*;
use crate::vega::VEGA_T256_SCALAR_MODULUS_BE_V1;
// BEGIN PRIVATE INCREMENTAL COLLECTIVE ENCRYPTION PREREQUISITE V1
const COLLECTIVE_RNS_COMPONENT_COUNT_V1: usize = 2;
const STREAMING_COLLECTIVE_RNS_LIMBS_V1: usize = RELEASE_MODULI_V1.len();
const STREAMING_COLLECTIVE_LIMB_COUNT_BYTES_V1: usize = 4;
const STREAMING_COLLECTIVE_KEY_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.streaming-collective-key-binding";
const STREAMING_COLLECTIVE_KEY_ADMISSION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.streaming-collective-key-admission";
const STREAMING_COLLECTIVE_EVAL_ADMISSION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.streaming-collective-eval-admission";
const STREAMING_COLLECTIVE_EVAL_KEY_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.streaming-collective-eval-key-binding";
const STREAMING_COLLECTIVE_KEY_AUTHORITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.streaming-collective-key-authority";
const STREAMING_COLLECTIVE_CIPHERTEXT_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.streaming-collective-ciphertext-manifest";
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
    const fn from_streaming_binding_v1(key: &ZkAmsMkheStreamingCollectiveKeyBindingV1) -> Self {
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
        hash: Box<Keccak256>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile_digest = profile.digest()?;
        Self::new_with_prevalidated_profile_digest_v1(
            profile,
            profile_digest,
            key,
            transcript_digest,
            sample_index,
            hash,
        )
    }
    fn new_with_prevalidated_profile_digest_v1(
        profile: &BgvProfile,
        profile_digest: [u8; 32],
        key: IncrementalCollectiveKeyBindingV1,
        transcript_digest: [u8; 32],
        sample_index: u64,
        mut hash: Box<Keccak256>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if key.profile_digest != profile_digest
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
    hash: Box<Keccak256>,
) -> [u8; 32] {
    collective_encryption_transcript_digest_from_axes_with_preallocated_hash_v1(
        key.profile_digest,
        key.security_certificate_digest,
        key.roster_digest,
        key.key_material_digest,
        key.epoch,
        key.transcript_digest,
        key.digest,
        topology,
        sample_index,
        encryption_nonce,
        hash,
    )
}
#[allow(clippy::too_many_arguments)]
fn collective_encryption_transcript_digest_from_axes_with_preallocated_hash_v1(
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    key_transcript_digest: [u8; 32],
    key_digest: [u8; 32],
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
    hash.update(&profile_digest);
    hash.update(&security_certificate_digest);
    hash.update(&roster_digest);
    hash.update(&key_material_digest);
    hash.update(&epoch.to_be_bytes());
    hash.update(&key_transcript_digest);
    hash.update(&key_digest);
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
#[derive(Clone, Copy)]
struct PurposeForkedCollectiveKeyAdmissionAxesV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    key_digest: [u8; 32],
    staged_admission_digest: [u8; 32],
}
struct StreamingCollectiveKeyAdmissionSealV1;
/// One-shot purpose seal accepted only by key-limb publication.
pub(crate) struct ZkAmsMkheStreamingCollectiveKeyAdmissionV1 {
    _seal: StreamingCollectiveKeyAdmissionSealV1,
    axes: PurposeForkedCollectiveKeyAdmissionAxesV1,
    admission_digest: [u8; 32],
}
struct StreamingCollectiveEvalAdmissionSealV1;
/// Distinct one-shot purpose seal reserved for the evaluated-key runtime.
pub(crate) struct ZkAmsMkheStreamingCollectiveEvalAdmissionV1 {
    _seal: StreamingCollectiveEvalAdmissionSealV1,
    axes: PurposeForkedCollectiveKeyAdmissionAxesV1,
    admission_digest: [u8; 32],
}
/// Consume the raw staged admission once and purpose-fork two non-cloneable
/// successors. Neither successor can be reconstructed from public digests.
pub(crate) fn fork_zk_ams_mkhe_staged_collective_key_admission_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    key: &ZkAmsMkheCollectivePublicKeyV1,
    admission: ZkAmsMkheStagedCollectivePublicKeyAdmissionV1,
) -> Result<
    (
        ZkAmsMkheStreamingCollectiveKeyAdmissionV1,
        ZkAmsMkheStreamingCollectiveEvalAdmissionV1,
    ),
    ZkAmsMkheErrorV1,
> {
    let staged_admission_digest = admission.admission_digest;
    admission.consume_for_key_v1(roster, transcript_digest, key)?;
    let axes = PurposeForkedCollectiveKeyAdmissionAxesV1 {
        profile_digest: key.profile_digest,
        roster_digest: key.roster_digest,
        key_material_digest: key.key_material_digest,
        epoch: key.epoch,
        transcript_digest: key.transcript_digest,
        key_digest: key.digest,
        staged_admission_digest,
    };
    let streaming_digest = purpose_forked_collective_key_admission_digest_v1(
        STREAMING_COLLECTIVE_KEY_ADMISSION_DOMAIN_V1,
        axes,
    );
    let eval_digest = purpose_forked_collective_key_admission_digest_v1(
        STREAMING_COLLECTIVE_EVAL_ADMISSION_DOMAIN_V1,
        axes,
    );
    if streaming_digest == [0; 32] || eval_digest == [0; 32] || streaming_digest == eval_digest {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok((
        ZkAmsMkheStreamingCollectiveKeyAdmissionV1 {
            _seal: StreamingCollectiveKeyAdmissionSealV1,
            axes,
            admission_digest: streaming_digest,
        },
        ZkAmsMkheStreamingCollectiveEvalAdmissionV1 {
            _seal: StreamingCollectiveEvalAdmissionSealV1,
            axes,
            admission_digest: eval_digest,
        },
    ))
}
fn purpose_forked_collective_key_admission_digest_v1(
    domain: &[u8],
    axes: PurposeForkedCollectiveKeyAdmissionAxesV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&axes.profile_digest);
    hash.update(&axes.roster_digest);
    hash.update(&axes.key_material_digest);
    hash.update(&axes.epoch.to_be_bytes());
    hash.update(&axes.transcript_digest);
    hash.update(&axes.key_digest);
    hash.update(&axes.staged_admission_digest);
    hash.finalize()
}
fn validate_purpose_forked_collective_key_admission_v1(
    domain: &[u8],
    axes: PurposeForkedCollectiveKeyAdmissionAxesV1,
    admission_digest: [u8; 32],
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    key: &ZkAmsMkheCollectivePublicKeyV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    roster.validate()?;
    key.validate(&release_profile_v1())?;
    if axes.profile_digest != roster.profile_digest()
        || axes.roster_digest != roster.roster_digest()
        || axes.key_material_digest != roster.key_material_digest()
        || axes.epoch != roster.epoch()
        || axes.transcript_digest != transcript_digest
        || axes.key_digest != key.digest
        || axes.profile_digest != key.profile_digest
        || axes.roster_digest != key.roster_digest
        || axes.key_material_digest != key.key_material_digest
        || axes.epoch != key.epoch
        || axes.transcript_digest != key.transcript_digest
        || axes.staged_admission_digest == [0; 32]
        || admission_digest == [0; 32]
        || admission_digest != purpose_forked_collective_key_admission_digest_v1(domain, axes)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
impl ZkAmsMkheStreamingCollectiveKeyAdmissionV1 {
    fn consume_for_key_v1(
        self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        key: &ZkAmsMkheCollectivePublicKeyV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        validate_purpose_forked_collective_key_admission_v1(
            STREAMING_COLLECTIVE_KEY_ADMISSION_DOMAIN_V1,
            self.axes,
            self.admission_digest,
            roster,
            transcript_digest,
            key,
        )
    }
}
impl ZkAmsMkheStreamingCollectiveEvalAdmissionV1 {
    /// Consume the evaluated-runtime purpose seal beside its exact native key.
    pub(crate) fn consume_for_key_v1(
        self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        key: &ZkAmsMkheCollectivePublicKeyV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        validate_purpose_forked_collective_key_admission_v1(
            STREAMING_COLLECTIVE_EVAL_ADMISSION_DOMAIN_V1,
            self.axes,
            self.admission_digest,
            roster,
            transcript_digest,
            key,
        )
    }
}
struct StreamingCollectiveEncryptionKeyAuthoritySealV1;
struct StreamingCollectiveEvalKeyBindingSealV1;
/// Compact binding shared by source-backed encryption and future bounded
/// evaluated-key runtimes. It contains key identity and source locations only;
/// fresh-encryption topology and sample state deliberately live elsewhere.
pub(super) struct ZkAmsMkheStreamingCollectiveKeyBindingV1 {
    version: u8,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    parties: [ZkAmsMkhePartyIdV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    key_digest: [u8; 32],
    public_a_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    public_b_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    binding_digest: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheStreamingCollectiveKeyBindingV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheStreamingCollectiveKeyBindingV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("key_digest", &hex::encode(self.key_digest))
            .field("limbs", &self.public_a_limb_pointers.len())
            .field("binding_digest", &hex::encode(self.binding_digest))
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheStreamingCollectiveKeyBindingV1 {
    fn from_validated_native_key_v1(
        key: &ZkAmsMkheCollectivePublicKeyV1,
        profile: &BgvProfile,
        public_a_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
        public_b_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        key.validate(profile)?;
        let parties = core::array::from_fn(|index| key.parties.parties[index]);
        let mut binding = Self {
            version: MKHE_VERSION_V1,
            profile_digest: key.profile_digest,
            security_certificate_digest: key.security_certificate_digest,
            roster_digest: key.roster_digest,
            key_material_digest: key.key_material_digest,
            epoch: key.epoch,
            transcript_digest: key.transcript_digest,
            parties,
            share_digests: key.share_digests,
            key_digest: key.digest,
            public_a_limb_pointers,
            public_b_limb_pointers,
            binding_digest: [0; 32],
        };
        binding.binding_digest = streaming_collective_key_binding_digest_v1(&binding, profile)?;
        binding.validate_for_profile_v1(profile)?;
        binding.validate_for_native_key_v1(key, profile)?;
        Ok(binding)
    }
    fn validate_for_profile_v1(&self, profile: &BgvProfile) -> Result<(), ZkAmsMkheErrorV1> {
        profile.validate()?;
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != profile.digest()?
            || self.security_certificate_digest == [0; 32]
            || self.roster_digest
                != governed_roster_digest(self.profile_digest, self.epoch, &self.parties)
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.share_digests.contains(&[0; 32])
            || self.key_digest == [0; 32]
            || self.public_a_limb_pointers.len() != profile.moduli.len()
            || self.public_b_limb_pointers.len() != profile.moduli.len()
            || self.binding_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        for pointer in &self.public_a_limb_pointers {
            validate_streaming_collective_limb_pointer_v1(
                ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
                *pointer,
                profile,
            )?;
        }
        for pointer in &self.public_b_limb_pointers {
            validate_streaming_collective_limb_pointer_v1(
                ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
                *pointer,
                profile,
            )?;
        }
        if self.binding_digest != streaming_collective_key_binding_digest_v1(self, profile)? {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    fn validate_release_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        self.validate_for_profile_v1(&profile)?;
        if profile.moduli.len() != STREAMING_COLLECTIVE_RNS_LIMBS_V1
            || self.security_certificate_digest != release_security_certificate_digest()?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    /// Validate a materialized native reference against every compact identity
    /// axis without constructing a second native key owner.
    pub(super) fn validate_for_native_key_v1(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        profile: &BgvProfile,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_for_profile_v1(profile)?;
        key.validate(profile)?;
        if self.profile_digest != key.profile_digest
            || self.security_certificate_digest != key.security_certificate_digest
            || self.roster_digest != key.roster_digest
            || self.key_material_digest != key.key_material_digest
            || self.epoch != key.epoch
            || self.transcript_digest != key.transcript_digest
            || self.parties.as_slice() != key.parties.parties.as_slice()
            || self.share_digests != key.share_digests
            || self.key_digest != key.digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}
/// Sealed compact authority retained by the evaluated-key runtime.
///
/// The two native key polynomials and the 76 direct-object pointers remain
/// outside this value. Their exact order and publication receipts are already
/// committed by the streaming key binding and authority digests below.
pub(crate) struct ZkAmsMkheStreamingCollectiveEvalKeyBindingV1 {
    _seal: StreamingCollectiveEvalKeyBindingSealV1,
    version: u8,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    key_digest: [u8; 32],
    streaming_key_binding_digest: [u8; 32],
    streaming_key_authority_digest: [u8; 32],
    eval_admission_digest: [u8; 32],
    binding_digest: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheStreamingCollectiveEvalKeyBindingV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheStreamingCollectiveEvalKeyBindingV1")
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("key_digest", &hex::encode(self.key_digest))
            .field("binding_digest", &hex::encode(self.binding_digest))
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheStreamingCollectiveEvalKeyBindingV1 {
    #[cfg(test)]
    pub(crate) fn test_from_verified_axes_v1(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        key_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        roster.validate()?;
        if transcript_digest == [0; 32] || key_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut binding = Self {
            _seal: StreamingCollectiveEvalKeyBindingSealV1,
            version: MKHE_VERSION_V1,
            profile_digest: roster.profile_digest(),
            security_certificate_digest: release_security_certificate_digest()?,
            roster_digest: roster.roster_digest(),
            key_material_digest: roster.key_material_digest(),
            epoch: roster.epoch(),
            transcript_digest,
            key_digest,
            streaming_key_binding_digest: [0x91; 32],
            streaming_key_authority_digest: [0x92; 32],
            eval_admission_digest: [0x93; 32],
            binding_digest: [0; 32],
        };
        binding.binding_digest = streaming_collective_eval_key_binding_digest_v1(&binding);
        binding.validate_release_v1()?;
        Ok(binding)
    }
    pub(crate) fn validate_release_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        profile.validate()?;
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != profile.digest()?
            || self.security_certificate_digest != release_security_certificate_digest()?
            || self.roster_digest == [0; 32]
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.key_digest == [0; 32]
            || self.streaming_key_binding_digest == [0; 32]
            || self.streaming_key_authority_digest == [0; 32]
            || self.eval_admission_digest == [0; 32]
            || self.binding_digest == [0; 32]
            || self.binding_digest != streaming_collective_eval_key_binding_digest_v1(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    pub(crate) const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }
    pub(crate) const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }
    pub(crate) const fn key_material_digest(&self) -> [u8; 32] {
        self.key_material_digest
    }
    pub(crate) const fn epoch(&self) -> u64 {
        self.epoch
    }
    pub(crate) const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }
    pub(crate) const fn key_digest(&self) -> [u8; 32] {
        self.key_digest
    }
    pub(crate) const fn binding_digest(&self) -> [u8; 32] {
        self.binding_digest
    }
    #[cfg(test)]
    pub(crate) fn validate_native_key_v1(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        self.validate_release_v1()?;
        key.validate(&profile)?;
        if key.profile_digest != self.profile_digest
            || key.security_certificate_digest != self.security_certificate_digest
            || key.roster_digest != self.roster_digest
            || key.key_material_digest != self.key_material_digest
            || key.epoch != self.epoch
            || key.transcript_digest != self.transcript_digest
            || key.digest != self.key_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    pub(crate) fn validate_ciphertext_binding_v1(
        &self,
        ciphertext: &ZkAmsMkheStreamingCollectiveCiphertextBindingV1<'_>,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_release_v1()?;
        if ciphertext.profile_digest() != self.profile_digest
            || ciphertext.security_certificate_digest() != self.security_certificate_digest
            || ciphertext.roster_digest() != self.roster_digest
            || ciphertext.key_material_digest() != self.key_material_digest
            || ciphertext.epoch() != self.epoch
            || ciphertext.key_transcript_digest() != self.transcript_digest
            || ciphertext.key_digest() != self.key_digest
            || ciphertext.key_binding_digest() != self.streaming_key_binding_digest
            || ciphertext.key_authority_digest() != self.streaming_key_authority_digest
            || ciphertext.level() != 0
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }
}
/// Move-only proof that the staged CPK successor published and reread every
/// key limb before releasing the native `2P` key owner.
pub struct ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1 {
    _seal: StreamingCollectiveEncryptionKeyAuthoritySealV1,
    binding: ZkAmsMkheStreamingCollectiveKeyBindingV1,
    public_a_publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    public_b_publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    authority_digest: [u8; 32],
    next_sample_index: u64,
    failed: bool,
}
impl core::fmt::Debug for ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1")
            .field("key_digest", &hex::encode(self.binding.key_digest))
            .field("authority_digest", &hex::encode(self.authority_digest))
            .field("next_sample_index", &self.next_sample_index)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1 {
    fn validate_for_profile_v1(&self, profile: &BgvProfile) -> Result<(), ZkAmsMkheErrorV1> {
        self.binding.validate_for_profile_v1(profile)?;
        validate_streaming_collective_key_publication_receipts_v1(
            &self.binding,
            &self.public_a_publication_receipts,
            &self.public_b_publication_receipts,
            profile,
        )?;
        if self.authority_digest == [0; 32]
            || self.authority_digest
                != streaming_collective_key_authority_digest_v1(
                    &self.binding,
                    &self.public_a_publication_receipts,
                    &self.public_b_publication_receipts,
                    profile,
                )?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    fn validate_release_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        self.binding.validate_release_v1()?;
        self.validate_for_profile_v1(&release_profile_v1())
    }
    pub(super) fn binding_v1(&self) -> &ZkAmsMkheStreamingCollectiveKeyBindingV1 {
        &self.binding
    }
    /// Frozen release-profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.binding.profile_digest
    }
    /// Frozen release security-certificate digest.
    #[must_use]
    pub const fn security_certificate_digest(&self) -> [u8; 32] {
        self.binding.security_certificate_digest
    }
    /// Exact governed roster digest.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.binding.roster_digest
    }
    /// Exact governed authentication-key material digest.
    #[must_use]
    pub const fn key_material_digest(&self) -> [u8; 32] {
        self.binding.key_material_digest
    }
    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.binding.epoch
    }
    /// Exact staged CPK transcript digest.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.binding.transcript_digest
    }
    /// Consensus digest of the exact native collective public key.
    #[must_use]
    pub const fn key_digest(&self) -> [u8; 32] {
        self.binding.key_digest
    }
    /// Ordered exact pointers to all common-`a` limbs.
    #[must_use]
    pub fn public_a_limb_pointers(&self) -> &[ZkAmsMkheDirectObjectPointerV1] {
        &self.binding.public_a_limb_pointers
    }
    /// Ordered exact pointers to all aggregate-`b` limbs.
    #[must_use]
    pub fn public_b_limb_pointers(&self) -> &[ZkAmsMkheDirectObjectPointerV1] {
        &self.binding.public_b_limb_pointers
    }
    /// Receipts proving every common-`a` limb was sealed, published, and reread.
    #[must_use]
    pub fn public_a_publication_receipts(&self) -> &[ZkAmsMkheDirectObjectPublicationReceiptV1] {
        &self.public_a_publication_receipts
    }
    /// Receipts proving every aggregate-`b` limb was sealed, published, and reread.
    #[must_use]
    pub fn public_b_publication_receipts(&self) -> &[ZkAmsMkheDirectObjectPublicationReceiptV1] {
        &self.public_b_publication_receipts
    }
    /// Digest binding every key axis, limb pointer, and publication receipt.
    #[must_use]
    pub const fn authority_digest(&self) -> [u8; 32] {
        self.authority_digest
    }
    /// Sole sample index accepted by the next successful fresh encryption.
    #[must_use]
    pub const fn next_sample_index(&self) -> u64 {
        self.next_sample_index
    }
}
fn streaming_collective_eval_key_binding_digest_v1(
    binding: &ZkAmsMkheStreamingCollectiveEvalKeyBindingV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(STREAMING_COLLECTIVE_EVAL_KEY_BINDING_DOMAIN_V1);
    hash.update(&[binding.version]);
    hash.update(&binding.profile_digest);
    hash.update(&binding.security_certificate_digest);
    hash.update(&binding.roster_digest);
    hash.update(&binding.key_material_digest);
    hash.update(&binding.epoch.to_be_bytes());
    hash.update(&binding.transcript_digest);
    hash.update(&binding.key_digest);
    hash.update(&binding.streaming_key_binding_digest);
    hash.update(&binding.streaming_key_authority_digest);
    hash.update(&binding.eval_admission_digest);
    hash.finalize()
}
/// Consume the evaluated-key admission beside the exact published key
/// authority and return the sole compact runtime successor.
pub(crate) fn bind_zk_ams_mkhe_streaming_collective_eval_key_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    key: &ZkAmsMkheCollectivePublicKeyV1,
    admission: ZkAmsMkheStreamingCollectiveEvalAdmissionV1,
    authority: &ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
) -> Result<ZkAmsMkheStreamingCollectiveEvalKeyBindingV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    roster.validate()?;
    key.validate(&profile)?;
    authority.validate_release_v1()?;
    authority
        .binding
        .validate_for_native_key_v1(key, &profile)?;
    if transcript_digest != key.transcript_digest
        || authority.binding.profile_digest != roster.profile_digest()
        || authority.binding.roster_digest != roster.roster_digest()
        || authority.binding.key_material_digest != roster.key_material_digest()
        || authority.binding.epoch != roster.epoch()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let eval_admission_digest = admission.admission_digest;
    admission.consume_for_key_v1(roster, transcript_digest, key)?;
    let mut binding = ZkAmsMkheStreamingCollectiveEvalKeyBindingV1 {
        _seal: StreamingCollectiveEvalKeyBindingSealV1,
        version: MKHE_VERSION_V1,
        profile_digest: authority.binding.profile_digest,
        security_certificate_digest: authority.binding.security_certificate_digest,
        roster_digest: authority.binding.roster_digest,
        key_material_digest: authority.binding.key_material_digest,
        epoch: authority.binding.epoch,
        transcript_digest: authority.binding.transcript_digest,
        key_digest: authority.binding.key_digest,
        streaming_key_binding_digest: authority.binding.binding_digest,
        streaming_key_authority_digest: authority.authority_digest,
        eval_admission_digest,
        binding_digest: [0; 32],
    };
    binding.binding_digest = streaming_collective_eval_key_binding_digest_v1(&binding);
    binding.validate_release_v1()?;
    Ok(binding)
}
/// Consume the staged CPK admission while publishing every native key limb.
/// No authority is returned unless all 76 publication transactions complete
/// their seal, CAS publish, authoritative lookup, and independent readback.
pub(crate) fn mint_zk_ams_mkhe_streaming_collective_encryption_key_authority_v1<P>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    key: &ZkAmsMkheCollectivePublicKeyV1,
    admission: ZkAmsMkheStreamingCollectiveKeyAdmissionV1,
    publisher: &mut P,
) -> Result<ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    let profile = release_profile_v1();
    roster.validate()?;
    key.validate(&profile)?;
    admission.consume_for_key_v1(roster, transcript_digest, key)?;
    if key.security_certificate_digest != release_security_certificate_digest()?
        || profile.moduli.len() != STREAMING_COLLECTIVE_RNS_LIMBS_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let limbs = profile.moduli.len();
    let mut public_a_limb_pointers = try_streaming_vec_with_capacity_v1(limbs)?;
    let mut public_b_limb_pointers = try_streaming_vec_with_capacity_v1(limbs)?;
    let mut public_a_publication_receipts = try_streaming_vec_with_capacity_v1(limbs)?;
    let mut public_b_publication_receipts = try_streaming_vec_with_capacity_v1(limbs)?;
    let mut scratch = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    for limb in 0..limbs {
        let receipt = publish_streaming_collective_limb_v1(
            ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            key.public_a.limb(&profile, limb),
            publisher,
            &mut scratch,
        )?;
        public_a_limb_pointers.push(receipt.pointer());
        public_a_publication_receipts.push(receipt);
    }
    for limb in 0..limbs {
        let receipt = publish_streaming_collective_limb_v1(
            ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            key.collective_public_b.limb(&profile, limb),
            publisher,
            &mut scratch,
        )?;
        public_b_limb_pointers.push(receipt.pointer());
        public_b_publication_receipts.push(receipt);
    }
    let binding = ZkAmsMkheStreamingCollectiveKeyBindingV1::from_validated_native_key_v1(
        key,
        &profile,
        public_a_limb_pointers,
        public_b_limb_pointers,
    )?;
    let authority_digest = streaming_collective_key_authority_digest_v1(
        &binding,
        &public_a_publication_receipts,
        &public_b_publication_receipts,
        &profile,
    )?;
    let authority = ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1 {
        _seal: StreamingCollectiveEncryptionKeyAuthoritySealV1,
        binding,
        public_a_publication_receipts,
        public_b_publication_receipts,
        authority_digest,
        next_sample_index: 0,
        failed: false,
    };
    authority.validate_release_v1()?;
    Ok(authority)
}
fn try_streaming_vec_with_capacity_v1<T>(capacity: usize) -> Result<Vec<T>, ZkAmsMkheErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if values.capacity() != capacity {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    Ok(values)
}
fn streaming_collective_limb_object_bytes_v1(
    profile: &BgvProfile,
) -> Result<u64, ZkAmsMkheErrorV1> {
    let coefficient_bytes = profile
        .ring_degree
        .checked_mul(core::mem::size_of::<u64>())
        .and_then(|bytes| bytes.checked_add(STREAMING_COLLECTIVE_LIMB_COUNT_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    u32::try_from(profile.ring_degree).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    u64::try_from(coefficient_bytes).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}
fn validate_streaming_collective_limb_pointer_v1(
    kind: ZkAmsMkheDirectObjectKindV1,
    pointer: ZkAmsMkheDirectObjectPointerV1,
    profile: &BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    if pointer.kind() != kind
        || pointer.payload_bytes() != streaming_collective_limb_object_bytes_v1(profile)?
        || pointer.payload_blake3() == [0; 32]
        || pointer.pointer_digest() == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
fn publish_streaming_collective_limb_v1<P>(
    kind: ZkAmsMkheDirectObjectKindV1,
    coefficients: &[u64],
    publisher: &mut P,
    scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
) -> Result<ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    if coefficients.is_empty() || coefficients.len() > u32::MAX as usize {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let payload_bytes = coefficients
        .len()
        .checked_mul(core::mem::size_of::<u64>())
        .and_then(|bytes| bytes.checked_add(STREAMING_COLLECTIVE_LIMB_COUNT_BYTES_V1))
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut transaction =
        ZkAmsMkheDirectObjectPublicationTransactionV1::begin(kind, payload_bytes, publisher)?;
    transaction.write_exact(
        &u32::try_from(coefficients.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    )?;
    for chunk in coefficients.chunks(scratch.len() / core::mem::size_of::<u64>()) {
        let encoded_bytes = chunk
            .len()
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for (encoded, coefficient) in scratch[..encoded_bytes]
            .chunks_exact_mut(core::mem::size_of::<u64>())
            .zip(chunk)
        {
            encoded.copy_from_slice(&coefficient.to_be_bytes());
        }
        transaction.write_exact(&scratch[..encoded_bytes])?;
    }
    transaction.finish()
}
fn streaming_collective_key_binding_digest_v1(
    binding: &ZkAmsMkheStreamingCollectiveKeyBindingV1,
    profile: &BgvProfile,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if binding.public_a_limb_pointers.len() != profile.moduli.len()
        || binding.public_b_limb_pointers.len() != profile.moduli.len()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(STREAMING_COLLECTIVE_KEY_BINDING_DOMAIN_V1);
    hash.update(&[binding.version]);
    hash.update(&binding.profile_digest);
    hash.update(&binding.security_certificate_digest);
    hash.update(&binding.roster_digest);
    hash.update(&binding.key_material_digest);
    hash.update(&binding.epoch.to_be_bytes());
    hash.update(&binding.transcript_digest);
    for party in binding.parties {
        hash.update(&party.to_bytes());
    }
    for share_digest in binding.share_digests {
        hash.update(&share_digest);
    }
    hash.update(&binding.key_digest);
    hash.update(
        &u32::try_from(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for (kind, pointers) in [
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            binding.public_a_limb_pointers.as_slice(),
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            binding.public_b_limb_pointers.as_slice(),
        ),
    ] {
        hash.update(&[kind as u8]);
        for (limb, (modulus, pointer)) in profile.moduli.iter().zip(pointers).enumerate() {
            hash.update(
                &u32::try_from(limb)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                    .to_be_bytes(),
            );
            hash.update(&modulus.to_be_bytes());
            hash.update(&pointer.pointer_digest());
        }
    }
    Ok(hash.finalize())
}
fn validate_streaming_collective_key_publication_receipts_v1(
    binding: &ZkAmsMkheStreamingCollectiveKeyBindingV1,
    public_a_receipts: &[ZkAmsMkheDirectObjectPublicationReceiptV1],
    public_b_receipts: &[ZkAmsMkheDirectObjectPublicationReceiptV1],
    profile: &BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    if public_a_receipts.len() != profile.moduli.len()
        || public_b_receipts.len() != profile.moduli.len()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut publication_identity = None;
    for (kind, pointers, receipts) in [
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            binding.public_a_limb_pointers.as_slice(),
            public_a_receipts,
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            binding.public_b_limb_pointers.as_slice(),
            public_b_receipts,
        ),
    ] {
        for (pointer, receipt) in pointers.iter().zip(receipts) {
            validate_streaming_collective_limb_pointer_v1(kind, *pointer, profile)?;
            if receipt.pointer() != *pointer
                || receipt.receipt_digest() == [0; 32]
                || receipt.post_publish_read_receipt().snapshot().pointer() != *pointer
                || receipt.post_publish_read_receipt().receipt_digest() == [0; 32]
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            match publication_identity {
                None => publication_identity = Some(receipt.publication_identity()),
                Some(expected) if expected == receipt.publication_identity() => {}
                Some(_) => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            }
        }
    }
    if publication_identity.is_none() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
fn streaming_collective_key_authority_digest_v1(
    binding: &ZkAmsMkheStreamingCollectiveKeyBindingV1,
    public_a_receipts: &[ZkAmsMkheDirectObjectPublicationReceiptV1],
    public_b_receipts: &[ZkAmsMkheDirectObjectPublicationReceiptV1],
    profile: &BgvProfile,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    validate_streaming_collective_key_publication_receipts_v1(
        binding,
        public_a_receipts,
        public_b_receipts,
        profile,
    )?;
    let mut hash = Keccak256::new();
    hash.update(STREAMING_COLLECTIVE_KEY_AUTHORITY_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&binding.binding_digest);
    for (kind, receipts) in [
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            public_a_receipts,
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            public_b_receipts,
        ),
    ] {
        hash.update(&[kind as u8]);
        for (limb, (modulus, receipt)) in profile.moduli.iter().zip(receipts).enumerate() {
            hash.update(
                &u32::try_from(limb)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                    .to_be_bytes(),
            );
            hash.update(&modulus.to_be_bytes());
            hash.update(&receipt.receipt_digest());
            hash.update(&receipt.post_publish_read_receipt().receipt_digest());
        }
    }
    Ok(hash.finalize())
}
/// Allocation-free canonical reader for one exact `u32 N || N*u64` limb.
/// The transaction never owns the provider and fills an existing arithmetic
/// owner instead of returning a `Vec`.
struct StreamingCollectiveLimbReaderV1 {
    transaction: ZkAmsMkheDirectObjectReadTransactionV1,
    ring_degree: usize,
    consumed: bool,
}
impl StreamingCollectiveLimbReaderV1 {
    fn begin<P>(
        kind: ZkAmsMkheDirectObjectKindV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        profile: &BgvProfile,
        provider: &mut P,
    ) -> Result<Self, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        validate_streaming_collective_limb_pointer_v1(kind, pointer, profile)?;
        let mut transaction =
            ZkAmsMkheDirectObjectReadTransactionV1::begin(kind, pointer, provider)?;
        let mut count = [0_u8; STREAMING_COLLECTIVE_LIMB_COUNT_BYTES_V1];
        if transaction.read_next(provider, &mut count)? != count.len()
            || usize::try_from(u32::from_be_bytes(count)).ok() != Some(profile.ring_degree)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        Ok(Self {
            transaction,
            ring_degree: profile.ring_degree,
            consumed: false,
        })
    }
    fn read_limb_into_v1<P>(
        &mut self,
        provider: &mut P,
        modulus: u64,
        destination: &mut [u64],
        scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self.consumed
            || destination.len() != self.ring_degree
            || scratch.len() < core::mem::size_of::<u64>()
            || !scratch.len().is_multiple_of(core::mem::size_of::<u64>())
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        self.consumed = true;
        let mut filled = 0_usize;
        while filled != destination.len() {
            let take_coefficients =
                (destination.len() - filled).min(scratch.len() / core::mem::size_of::<u64>());
            let take_bytes = take_coefficients
                .checked_mul(core::mem::size_of::<u64>())
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let read = self
                .transaction
                .read_next(provider, &mut scratch[..take_bytes])?;
            if read != take_bytes {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            for (destination, encoded) in destination[filled..filled + take_coefficients]
                .iter_mut()
                .zip(scratch[..read].chunks_exact(core::mem::size_of::<u64>()))
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
        Ok(())
    }
    fn finish<P>(
        self,
        provider: &mut P,
    ) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if !self.consumed || self.transaction.remaining_bytes() != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        self.transaction.finish(provider)
    }
}
fn streaming_source_snapshot_axes_v1(
    receipt: &ZkAmsMkheDirectObjectReadReceiptV1,
) -> ([u8; 32], [u8; 32]) {
    let snapshot = receipt.snapshot();
    (snapshot.provider_identity(), snapshot.snapshot_identity())
}
fn validate_streaming_source_receipt_v1(
    receipt: &ZkAmsMkheDirectObjectReadReceiptV1,
    kind: ZkAmsMkheDirectObjectKindV1,
    pointer: ZkAmsMkheDirectObjectPointerV1,
    profile: &BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_streaming_collective_limb_pointer_v1(kind, pointer, profile)?;
    if receipt.snapshot().pointer() != pointer
        || receipt.canonical_bytes() != pointer.payload_bytes()
        || receipt.payload_blake3() != pointer.payload_blake3()
        || receipt.receipt_digest() == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
fn validate_streaming_second_source_receipt_v1(
    prepass: &ZkAmsMkheDirectObjectReadReceiptV1,
    second_pass: &ZkAmsMkheDirectObjectReadReceiptV1,
    kind: ZkAmsMkheDirectObjectKindV1,
    pointer: ZkAmsMkheDirectObjectPointerV1,
    profile: &BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_streaming_source_receipt_v1(prepass, kind, pointer, profile)?;
    validate_streaming_source_receipt_v1(second_pass, kind, pointer, profile)?;
    if streaming_source_snapshot_axes_v1(prepass) != streaming_source_snapshot_axes_v1(second_pass)
        || prepass.receipt_digest() != second_pass.receipt_digest()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
struct StreamingCollectiveEncryptionRecordOwnersV1 {
    public_a_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    public_b_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    public_a_prepass_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    public_b_prepass_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    public_a_second_pass_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    public_b_second_pass_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    constant_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    linear_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    constant_publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    linear_publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    scratch: Box<[u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1]>,
}
impl StreamingCollectiveEncryptionRecordOwnersV1 {
    fn new_v1(
        binding: &ZkAmsMkheStreamingCollectiveKeyBindingV1,
        profile: &BgvProfile,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        binding.validate_for_profile_v1(profile)?;
        let limbs = profile.moduli.len();
        let mut public_a_limb_pointers = try_streaming_vec_with_capacity_v1(limbs)?;
        let mut public_b_limb_pointers = try_streaming_vec_with_capacity_v1(limbs)?;
        public_a_limb_pointers.extend_from_slice(&binding.public_a_limb_pointers);
        public_b_limb_pointers.extend_from_slice(&binding.public_b_limb_pointers);
        Ok(Self {
            public_a_limb_pointers,
            public_b_limb_pointers,
            public_a_prepass_receipts: try_streaming_vec_with_capacity_v1(limbs)?,
            public_b_prepass_receipts: try_streaming_vec_with_capacity_v1(limbs)?,
            public_a_second_pass_receipts: try_streaming_vec_with_capacity_v1(limbs)?,
            public_b_second_pass_receipts: try_streaming_vec_with_capacity_v1(limbs)?,
            constant_limb_pointers: try_streaming_vec_with_capacity_v1(limbs)?,
            linear_limb_pointers: try_streaming_vec_with_capacity_v1(limbs)?,
            constant_publication_receipts: try_streaming_vec_with_capacity_v1(limbs)?,
            linear_publication_receipts: try_streaming_vec_with_capacity_v1(limbs)?,
            scratch: Box::new([0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1]),
        })
    }
}
/// All secret and record owners fallibly allocated before any source-provider
/// call. Entropy cannot be requested from this state.
struct PreparedStreamingCollectiveEncryptionV1 {
    profile: BgvProfile,
    profile_digest: [u8; 32],
    workspace: ZeroizingCollectiveEncryptionWorkspaceV1,
    ephemeral: ZeroizingCollectiveEncryptionWitnessV1,
    error_zero: ZeroizingCollectiveEncryptionWitnessV1,
    error_one: ZeroizingCollectiveEncryptionWitnessV1,
    entropy_owners: PreallocatedCollectiveEncryptionEntropyOwnersV1,
    records: StreamingCollectiveEncryptionRecordOwnersV1,
}
impl PreparedStreamingCollectiveEncryptionV1 {
    fn new_v1(
        binding: &ZkAmsMkheStreamingCollectiveKeyBindingV1,
        profile: &BgvProfile,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        binding.validate_for_profile_v1(profile)?;
        let profile_digest = profile.digest()?;
        checked_ring_multiplication_work(profile, 2)?;
        let workspace =
            ZeroizingCollectiveEncryptionWorkspaceV1::new_zeroed_v1(profile.ring_degree)?;
        let ephemeral = ZeroizingCollectiveEncryptionWitnessV1::new_zeroed_v1(profile.ring_degree)?;
        let error_zero =
            ZeroizingCollectiveEncryptionWitnessV1::new_zeroed_v1(profile.ring_degree)?;
        let error_one = ZeroizingCollectiveEncryptionWitnessV1::new_zeroed_v1(profile.ring_degree)?;
        let entropy_owners = PreallocatedCollectiveEncryptionEntropyOwnersV1::new_zeroed_v1();
        let records = StreamingCollectiveEncryptionRecordOwnersV1::new_v1(binding, profile)?;
        Ok(Self {
            profile: profile.clone(),
            profile_digest,
            workspace,
            ephemeral,
            error_zero,
            error_one,
            entropy_owners,
            records,
        })
    }
    fn authenticate_key_source_v1<P>(
        mut self,
        provider: &mut P,
    ) -> Result<SourceAuthenticatedStreamingCollectiveEncryptionV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        let mut common_snapshot_axes = None;
        for pointer in &self.records.public_a_limb_pointers {
            let receipt = validate_zk_ams_mkhe_direct_object_v1(
                ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
                *pointer,
                provider,
            )?;
            validate_streaming_source_receipt_v1(
                &receipt,
                ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
                *pointer,
                &self.profile,
            )?;
            let axes = streaming_source_snapshot_axes_v1(&receipt);
            match common_snapshot_axes {
                None => common_snapshot_axes = Some(axes),
                Some(expected) if expected == axes => {}
                Some(_) => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            }
            self.records.public_a_prepass_receipts.push(receipt);
        }
        for pointer in &self.records.public_b_limb_pointers {
            let receipt = validate_zk_ams_mkhe_direct_object_v1(
                ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
                *pointer,
                provider,
            )?;
            validate_streaming_source_receipt_v1(
                &receipt,
                ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
                *pointer,
                &self.profile,
            )?;
            let axes = streaming_source_snapshot_axes_v1(&receipt);
            match common_snapshot_axes {
                Some(expected) if expected == axes => {}
                _ => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            }
            self.records.public_b_prepass_receipts.push(receipt);
        }
        if self.records.public_a_prepass_receipts.len() != self.profile.moduli.len()
            || self.records.public_b_prepass_receipts.len() != self.profile.moduli.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(SourceAuthenticatedStreamingCollectiveEncryptionV1(self))
    }
}
/// Both complete A/B source prepasses have finished; only this state may draw
/// the nonce and RLWE witnesses.
struct SourceAuthenticatedStreamingCollectiveEncryptionV1(PreparedStreamingCollectiveEncryptionV1);
struct StreamingCollectiveEncryptionKernelV1<'plaintext> {
    profile: BgvProfile,
    canonical_plaintext: &'plaintext [[u8; 32]],
    input_identity: CollectiveEncryptionInputIdentityV1,
    transcript_digest: [u8; 32],
    sample_index: u64,
    ephemeral: ZeroizingCollectiveEncryptionWitnessV1,
    error_zero: ZeroizingCollectiveEncryptionWitnessV1,
    error_one: ZeroizingCollectiveEncryptionWitnessV1,
    workspace: ZeroizingCollectiveEncryptionWorkspaceV1,
    ciphertext_digest: ComponentMajorCollectiveCiphertextDigestV1,
    poisoned: bool,
}
struct ActiveStreamingCollectiveEncryptionV1<'plaintext> {
    kernel: StreamingCollectiveEncryptionKernelV1<'plaintext>,
    records: StreamingCollectiveEncryptionRecordOwnersV1,
}
struct CompletedStreamingCollectiveEncryptionV1 {
    topology: CollectiveEncryptionInputTopologyV1,
    sample_index: u64,
    transcript_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    records: StreamingCollectiveEncryptionRecordOwnersV1,
}
impl SourceAuthenticatedStreamingCollectiveEncryptionV1 {
    fn activate_v1<'plaintext, R>(
        self,
        binding: &ZkAmsMkheStreamingCollectiveKeyBindingV1,
        canonical_plaintext: &'plaintext [[u8; 32]],
        topology: CollectiveEncryptionInputTopologyV1,
        sample_index: u64,
        random: &mut R,
    ) -> Result<ActiveStreamingCollectiveEncryptionV1<'plaintext>, ZkAmsMkheErrorV1>
    where
        R: MaskedRelaxedRandomSourceV1,
    {
        let PreparedStreamingCollectiveEncryptionV1 {
            profile,
            profile_digest,
            workspace,
            mut ephemeral,
            mut error_zero,
            mut error_one,
            entropy_owners,
            records,
        } = self.0;
        binding.validate_for_profile_v1(&profile)?;
        validate_incremental_canonical_plaintext_v1(&profile, canonical_plaintext)?;
        if topology.layout_digest == [0; 32]
            || topology.plaintext_used_slots == 0
            || usize::try_from(topology.plaintext_used_slots)
                .map_or(true, |used_slots| used_slots > profile.ring_degree)
            || records.public_a_prepass_receipts.len() != profile.moduli.len()
            || records.public_b_prepass_receipts.len() != profile.moduli.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let PreallocatedCollectiveEncryptionEntropyOwnersV1 {
            mut nonce,
            mut nonce_hash,
            mut transcript_hash,
            mut ciphertext_hash,
        } = entropy_owners;
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
        let transcript_digest =
            collective_encryption_transcript_digest_from_axes_with_preallocated_hash_v1(
                binding.profile_digest,
                binding.security_certificate_digest,
                binding.roster_digest,
                binding.key_material_digest,
                binding.epoch,
                binding.transcript_digest,
                binding.key_digest,
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
            ComponentMajorCollectiveCiphertextDigestV1::new_with_prevalidated_profile_digest_v1(
                &profile,
                profile_digest,
                IncrementalCollectiveKeyBindingV1::from_streaming_binding_v1(binding),
                transcript_digest,
                sample_index,
                ciphertext_hash
                    .take()
                    .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?,
            )?;
        sample_nonzero_ternary_into_v1(&profile, random, &mut ephemeral)?;
        sample_bounded_error_into_v1(&profile, random, &mut error_zero)?;
        sample_bounded_error_into_v1(&profile, random, &mut error_one)?;
        Ok(ActiveStreamingCollectiveEncryptionV1 {
            kernel: StreamingCollectiveEncryptionKernelV1 {
                profile,
                canonical_plaintext,
                input_identity,
                transcript_digest,
                sample_index,
                ephemeral,
                error_zero,
                error_one,
                workspace,
                ciphertext_digest,
                poisoned: false,
            },
            records,
        })
    }
}
impl StreamingCollectiveEncryptionKernelV1<'_> {
    #[allow(clippy::too_many_arguments)]
    fn publish_next_limb_v1<K, P>(
        &mut self,
        component: CollectiveRnsComponentV1,
        limb: usize,
        source_kind: ZkAmsMkheDirectObjectKindV1,
        source_pointer: ZkAmsMkheDirectObjectPointerV1,
        prepass_receipt: &ZkAmsMkheDirectObjectReadReceiptV1,
        output_kind: ZkAmsMkheDirectObjectKindV1,
        key_provider: &mut K,
        ciphertext_publisher: &mut P,
        scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
    ) -> Result<
        (
            ZkAmsMkheDirectObjectReadReceiptV1,
            ZkAmsMkheDirectObjectPublicationReceiptV1,
        ),
        ZkAmsMkheErrorV1,
    >
    where
        K: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        if self.poisoned
            || !self.ciphertext_digest.expects(component, limb)
            || limb >= self.profile.moduli.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let expected_source_kind = match component {
            CollectiveRnsComponentV1::First => ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            CollectiveRnsComponentV1::Second => ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
        };
        let expected_output_kind = match component {
            CollectiveRnsComponentV1::First => ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
            CollectiveRnsComponentV1::Second => ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
        };
        if source_kind != expected_source_kind || output_kind != expected_output_kind {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        validate_streaming_source_receipt_v1(
            prepass_receipt,
            source_kind,
            source_pointer,
            &self.profile,
        )?;
        // Poison before either provider is entered. A caught unwind, source
        // error, or output-stage error can never resume this witness state.
        self.poisoned = true;
        clear_secret_u64_slice_v1(self.workspace.left.as_mut_slice());
        clear_secret_u64_slice_v1(self.workspace.right.as_mut_slice());
        let payload_bytes = streaming_collective_limb_object_bytes_v1(&self.profile)?;
        let mut output_transaction = ZkAmsMkheDirectObjectPublicationTransactionV1::begin(
            output_kind,
            payload_bytes,
            ciphertext_publisher,
        )?;
        output_transaction.write_exact(
            &u32::try_from(self.profile.ring_degree)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        )?;
        let modulus = self.profile.moduli[limb];
        let root = self.profile.negacyclic_roots[limb];
        let mut source_reader = StreamingCollectiveLimbReaderV1::begin(
            source_kind,
            source_pointer,
            &self.profile,
            key_provider,
        )?;
        source_reader.read_limb_into_v1(
            key_provider,
            modulus,
            self.workspace.left.as_mut_slice(),
            scratch,
        )?;
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
        write_streaming_collective_limb_coefficients_v1(
            &mut output_transaction,
            self.workspace.left.as_slice(),
            scratch,
        )?;
        // The complete second source hash and exact prepass snapshot equality
        // are checked before output sealing/publishing can begin.
        let second_pass_receipt = source_reader.finish(key_provider)?;
        validate_streaming_second_source_receipt_v1(
            prepass_receipt,
            &second_pass_receipt,
            source_kind,
            source_pointer,
            &self.profile,
        )?;
        let output_receipt = output_transaction.finish()?;
        validate_streaming_collective_limb_pointer_v1(
            output_kind,
            output_receipt.pointer(),
            &self.profile,
        )?;
        self.poisoned = false;
        Ok((second_pass_receipt, output_receipt))
    }
}
fn write_streaming_collective_limb_coefficients_v1<P>(
    transaction: &mut ZkAmsMkheDirectObjectPublicationTransactionV1<'_, P>,
    coefficients: &[u64],
    scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    if coefficients.is_empty() {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for chunk in coefficients.chunks(scratch.len() / core::mem::size_of::<u64>()) {
        let encoded_bytes = chunk
            .len()
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for (encoded, coefficient) in scratch[..encoded_bytes]
            .chunks_exact_mut(core::mem::size_of::<u64>())
            .zip(chunk)
        {
            encoded.copy_from_slice(&coefficient.to_be_bytes());
        }
        transaction.write_exact(&scratch[..encoded_bytes])?;
    }
    Ok(())
}
impl ActiveStreamingCollectiveEncryptionV1<'_> {
    fn publish_all_v1<K, P>(
        &mut self,
        key_provider: &mut K,
        ciphertext_publisher: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        K: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        let limbs = self.kernel.profile.moduli.len();
        let mut output_publication_identity = None;
        for limb in 0..limbs {
            let pointer = self.records.public_b_limb_pointers[limb];
            let (second_pass, publication) = self.kernel.publish_next_limb_v1(
                CollectiveRnsComponentV1::First,
                limb,
                ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
                pointer,
                &self.records.public_b_prepass_receipts[limb],
                ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
                key_provider,
                ciphertext_publisher,
                self.records.scratch.as_mut(),
            )?;
            match output_publication_identity {
                None => output_publication_identity = Some(publication.publication_identity()),
                Some(expected) if expected == publication.publication_identity() => {}
                Some(_) => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
            }
            self.records.public_b_second_pass_receipts.push(second_pass);
            self.records
                .constant_limb_pointers
                .push(publication.pointer());
            self.records.constant_publication_receipts.push(publication);
        }
        for limb in 0..limbs {
            let pointer = self.records.public_a_limb_pointers[limb];
            let (second_pass, publication) = self.kernel.publish_next_limb_v1(
                CollectiveRnsComponentV1::Second,
                limb,
                ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
                pointer,
                &self.records.public_a_prepass_receipts[limb],
                ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
                key_provider,
                ciphertext_publisher,
                self.records.scratch.as_mut(),
            )?;
            match output_publication_identity {
                Some(expected) if expected == publication.publication_identity() => {}
                _ => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
            }
            self.records.public_a_second_pass_receipts.push(second_pass);
            self.records
                .linear_limb_pointers
                .push(publication.pointer());
            self.records.linear_publication_receipts.push(publication);
        }
        Ok(())
    }
    fn finish(self) -> Result<CompletedStreamingCollectiveEncryptionV1, ZkAmsMkheErrorV1> {
        if self.kernel.poisoned
            || self.kernel.input_identity.encryption_nonce.is_zero()
            || self.kernel.input_identity.topology.layout_digest == [0; 32]
            || self.records.public_a_second_pass_receipts.len() != self.kernel.profile.moduli.len()
            || self.records.public_b_second_pass_receipts.len() != self.kernel.profile.moduli.len()
            || self.records.constant_publication_receipts.len() != self.kernel.profile.moduli.len()
            || self.records.linear_publication_receipts.len() != self.kernel.profile.moduli.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let topology = self.kernel.input_identity.topology;
        let sample_index = self.kernel.sample_index;
        let transcript_digest = self.kernel.transcript_digest;
        let ciphertext_digest = self.kernel.ciphertext_digest.finish()?;
        Ok(CompletedStreamingCollectiveEncryptionV1 {
            topology,
            sample_index,
            transcript_digest,
            ciphertext_digest,
            records: self.records,
        })
    }
}
struct StreamingCollectiveAutomorphismDigestV1 {
    hash: Keccak256,
    ring_degree: usize,
    moduli: &'static [u64],
    next_component: usize,
    next_limb: usize,
}
impl StreamingCollectiveAutomorphismDigestV1 {
    fn new_v1(
        eval_key: &ZkAmsMkheStreamingCollectiveEvalKeyBindingV1,
        output_transcript_digest: [u8; 32],
        sample_index: u64,
        profile: &BgvProfile,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        eval_key.validate_release_v1()?;
        if output_transcript_digest == [0; 32] || eval_key.profile_digest != profile.digest()? {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let coefficient_count = profile
            .ring_degree
            .checked_mul(profile.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut hash = Keccak256::new();
        hash.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
        hash.update(&eval_key.profile_digest);
        hash.update(&eval_key.roster_digest);
        hash.update(&eval_key.epoch.to_be_bytes());
        hash.update(&output_transcript_digest);
        hash.update(&sample_index.to_be_bytes());
        hash.update(&[0]);
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
    fn absorb_limb_v1(
        &mut self,
        component: usize,
        limb: usize,
        coefficients: &[u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if component != self.next_component
            || limb != self.next_limb
            || component >= COLLECTIVE_RNS_COMPONENT_COUNT_V1
            || limb >= self.moduli.len()
            || coefficients.len() != self.ring_degree
            || coefficients
                .iter()
                .any(|coefficient| *coefficient >= self.moduli[limb])
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
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
    fn finish(self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.next_component != COLLECTIVE_RNS_COMPONENT_COUNT_V1 || self.next_limb != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let digest = self.hash.finalize();
        if digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(digest)
    }
}
/// Preallocated, poison-on-failure output publication state for one streamed
/// automorphism. No output pointer can be supplied independently of its
/// publication/readback receipt.
pub(crate) struct ZkAmsMkheStreamingCollectiveAutomorphismOutputV1 {
    expected_input_manifest_digest: [u8; 32],
    eval_binding_digest: [u8; 32],
    output_transcript_digest: [u8; 32],
    digest: StreamingCollectiveAutomorphismDigestV1,
    constant_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    linear_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    constant_publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    linear_publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    scratch: [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
    failed: bool,
}
impl ZkAmsMkheStreamingCollectiveAutomorphismOutputV1 {
    pub(crate) fn publish_constant_limb_v1<P>(
        &mut self,
        limb: usize,
        coefficients: &[u64],
        publisher: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        self.publish_limb_v1(
            0,
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
            limb,
            coefficients,
            publisher,
        )
    }
    pub(crate) fn publish_linear_limb_v1<P>(
        &mut self,
        limb: usize,
        coefficients: &[u64],
        publisher: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        self.publish_limb_v1(
            1,
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
            limb,
            coefficients,
            publisher,
        )
    }
    fn publish_limb_v1<P>(
        &mut self,
        component: usize,
        kind: ZkAmsMkheDirectObjectKindV1,
        limb: usize,
        coefficients: &[u64],
        publisher: &mut P,
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    {
        if self.failed {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        self.failed = true;
        let result = (|| {
            let (pointers, receipts) = match component {
                0 => (
                    &mut self.constant_limb_pointers,
                    &mut self.constant_publication_receipts,
                ),
                1 => (
                    &mut self.linear_limb_pointers,
                    &mut self.linear_publication_receipts,
                ),
                _ => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
            };
            if limb != pointers.len()
                || limb != receipts.len()
                || pointers.len() >= pointers.capacity()
                || receipts.len() >= receipts.capacity()
            {
                return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
            }
            self.digest.absorb_limb_v1(component, limb, coefficients)?;
            let receipt = publish_streaming_collective_limb_v1(
                kind,
                coefficients,
                publisher,
                &mut self.scratch,
            )?;
            pointers.push(receipt.pointer());
            receipts.push(receipt);
            Ok(())
        })();
        if result.is_ok() {
            self.failed = false;
        }
        result
    }
    pub(crate) fn finish_v1(
        self,
        mut input: ZkAmsMkheStreamingCollectiveCiphertextV1,
        eval_key: &ZkAmsMkheStreamingCollectiveEvalKeyBindingV1,
    ) -> Result<ZkAmsMkheStreamingCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let limbs = profile.moduli.len();
        if self.failed
            || self.eval_binding_digest != eval_key.binding_digest()
            || input.manifest_digest != self.expected_input_manifest_digest
            || self.constant_limb_pointers.len() != limbs
            || self.linear_limb_pointers.len() != limbs
            || self.constant_publication_receipts.len() != limbs
            || self.linear_publication_receipts.len() != limbs
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        {
            let input_binding = input.sealed_binding_v1()?;
            eval_key.validate_ciphertext_binding_v1(&input_binding)?;
        }
        let output_ciphertext_digest = self.digest.finish()?;
        input.transcript_digest = self.output_transcript_digest;
        input.ciphertext_digest = output_ciphertext_digest;
        input.constant_limb_pointers = self.constant_limb_pointers;
        input.linear_limb_pointers = self.linear_limb_pointers;
        input.constant_publication_receipts = self.constant_publication_receipts;
        input.linear_publication_receipts = self.linear_publication_receipts;
        input.manifest_digest = [0; 32];
        input.manifest_digest =
            streaming_collective_ciphertext_manifest_digest_v1(&input, &profile)?;
        input.validate_for_profile_v1(&profile)?;
        let output_binding = input.sealed_binding_v1()?;
        eval_key.validate_ciphertext_binding_v1(&output_binding)?;
        Ok(input)
    }
}
pub(crate) fn prepare_zk_ams_mkhe_streaming_collective_automorphism_output_v1(
    input: &ZkAmsMkheStreamingCollectiveCiphertextV1,
    eval_key: &ZkAmsMkheStreamingCollectiveEvalKeyBindingV1,
    output_transcript_digest: [u8; 32],
) -> Result<ZkAmsMkheStreamingCollectiveAutomorphismOutputV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let input_binding = input.sealed_binding_v1()?;
    eval_key.validate_ciphertext_binding_v1(&input_binding)?;
    let digest = StreamingCollectiveAutomorphismDigestV1::new_v1(
        eval_key,
        output_transcript_digest,
        input_binding.sample_index(),
        &profile,
    )?;
    let limbs = profile.moduli.len();
    Ok(ZkAmsMkheStreamingCollectiveAutomorphismOutputV1 {
        expected_input_manifest_digest: input_binding.manifest_digest(),
        eval_binding_digest: eval_key.binding_digest(),
        output_transcript_digest,
        digest,
        constant_limb_pointers: try_streaming_vec_with_capacity_v1(limbs)?,
        linear_limb_pointers: try_streaming_vec_with_capacity_v1(limbs)?,
        constant_publication_receipts: try_streaming_vec_with_capacity_v1(limbs)?,
        linear_publication_receipts: try_streaming_vec_with_capacity_v1(limbs)?,
        scratch: [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
        failed: false,
    })
}
/// Move-only compact authority for one exact source-backed collective
/// ciphertext. Every component is represented by 38 independently addressed
/// limb objects; no native `2P` ciphertext owner or secret opening is retained.
pub struct ZkAmsMkheStreamingCollectiveCiphertextV1 {
    version: u8,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    key_transcript_digest: [u8; 32],
    key_digest: [u8; 32],
    key_binding_digest: [u8; 32],
    key_authority_digest: [u8; 32],
    topology: CollectiveEncryptionInputTopologyV1,
    sample_index: u64,
    level: u8,
    transcript_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    public_a_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    public_b_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    constant_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    linear_limb_pointers: Vec<ZkAmsMkheDirectObjectPointerV1>,
    public_a_prepass_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    public_b_prepass_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    public_a_second_pass_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    public_b_second_pass_receipts: Vec<ZkAmsMkheDirectObjectReadReceiptV1>,
    constant_publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    linear_publication_receipts: Vec<ZkAmsMkheDirectObjectPublicationReceiptV1>,
    manifest_digest: [u8; 32],
}
struct StreamingCollectiveCiphertextBindingSealV1;
/// Sealed borrowed view used by bounded decryption and evaluated-key runtimes.
/// It has no public constructor and cannot outlive the validated manifest.
pub(crate) struct ZkAmsMkheStreamingCollectiveCiphertextBindingV1<'manifest> {
    _seal: StreamingCollectiveCiphertextBindingSealV1,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    key_transcript_digest: [u8; 32],
    key_digest: [u8; 32],
    key_binding_digest: [u8; 32],
    key_authority_digest: [u8; 32],
    sample_index: u64,
    level: u8,
    transcript_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    constant_limb_pointers: &'manifest [ZkAmsMkheDirectObjectPointerV1],
    linear_limb_pointers: &'manifest [ZkAmsMkheDirectObjectPointerV1],
    constant_publication_receipts: &'manifest [ZkAmsMkheDirectObjectPublicationReceiptV1],
    linear_publication_receipts: &'manifest [ZkAmsMkheDirectObjectPublicationReceiptV1],
    manifest_digest: [u8; 32],
}
impl ZkAmsMkheStreamingCollectiveCiphertextBindingV1<'_> {
    /// Frozen release-profile digest.
    pub(crate) const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }
    /// Frozen security-certificate digest.
    pub(crate) const fn security_certificate_digest(&self) -> [u8; 32] {
        self.security_certificate_digest
    }
    /// Exact governed roster digest.
    pub(crate) const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }
    /// Exact governed authentication-key material digest.
    pub(crate) const fn key_material_digest(&self) -> [u8; 32] {
        self.key_material_digest
    }
    /// Governed secret/key epoch.
    pub(crate) const fn epoch(&self) -> u64 {
        self.epoch
    }
    /// Exact staged CPK transcript digest.
    pub(crate) const fn key_transcript_digest(&self) -> [u8; 32] {
        self.key_transcript_digest
    }
    /// Consensus digest of the collective public key.
    pub(crate) const fn key_digest(&self) -> [u8; 32] {
        self.key_digest
    }
    /// Digest binding ordered A/B limb pointers and key identity.
    pub(crate) const fn key_binding_digest(&self) -> [u8; 32] {
        self.key_binding_digest
    }
    /// Digest of the move-only key publication authority.
    pub(crate) const fn key_authority_digest(&self) -> [u8; 32] {
        self.key_authority_digest
    }
    /// Monotonic fresh-encryption sample index.
    pub(crate) const fn sample_index(&self) -> u64 {
        self.sample_index
    }
    /// Ciphertext level; fresh ingress is exactly zero.
    pub(crate) const fn level(&self) -> u8 {
        self.level
    }
    /// Exact fresh-encryption transcript digest.
    pub(crate) const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }
    /// Native component-major ciphertext digest.
    pub(crate) const fn ciphertext_digest(&self) -> [u8; 32] {
        self.ciphertext_digest
    }
    /// Ordered constant-component limb pointers.
    pub(crate) const fn constant_limb_pointers(&self) -> &[ZkAmsMkheDirectObjectPointerV1] {
        self.constant_limb_pointers
    }
    /// Ordered linear-component limb pointers.
    pub(crate) const fn linear_limb_pointers(&self) -> &[ZkAmsMkheDirectObjectPointerV1] {
        self.linear_limb_pointers
    }
    /// Publication/readback receipts for every constant limb.
    pub(crate) const fn constant_publication_receipts(
        &self,
    ) -> &[ZkAmsMkheDirectObjectPublicationReceiptV1] {
        self.constant_publication_receipts
    }
    /// Publication/readback receipts for every linear limb.
    pub(crate) const fn linear_publication_receipts(
        &self,
    ) -> &[ZkAmsMkheDirectObjectPublicationReceiptV1] {
        self.linear_publication_receipts
    }
    /// Digest binding the complete source and publication manifest.
    pub(crate) const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }
    fn read_component_limb_into_v1<P>(
        &self,
        kind: ZkAmsMkheDirectObjectKindV1,
        limb: usize,
        profile: &BgvProfile,
        provider: &mut P,
        destination: &mut [u64],
        scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
    ) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        let (pointers, publications) = match kind {
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0 => (
                self.constant_limb_pointers,
                self.constant_publication_receipts(),
            ),
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1 => (
                self.linear_limb_pointers,
                self.linear_publication_receipts(),
            ),
            _ => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
        };
        let pointer = *pointers
            .get(limb)
            .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?;
        let publication = publications
            .get(limb)
            .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?;
        let modulus = *profile
            .moduli
            .get(limb)
            .ok_or(ZkAmsMkheErrorV1::InvalidCiphertext)?;
        let mut reader = StreamingCollectiveLimbReaderV1::begin(kind, pointer, profile, provider)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)?;
        reader
            .read_limb_into_v1(provider, modulus, destination, scratch)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)?;
        let receipt = reader
            .finish(provider)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)?;
        if receipt != *publication.post_publish_read_receipt() {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(receipt)
    }
    /// Reread and authenticate one exact constant-component limb into caller
    /// storage. The fresh receipt must equal the manifest's post-publication
    /// receipt, including provider and immutable-snapshot identity.
    pub(in crate::vega::zk_ams::mkhe) fn read_constant_limb_into_v1<P>(
        &self,
        limb: usize,
        profile: &BgvProfile,
        provider: &mut P,
        destination: &mut [u64],
        scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
    ) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        self.read_component_limb_into_v1(
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
            limb,
            profile,
            provider,
            destination,
            scratch,
        )
    }
    /// Reread and authenticate one exact linear-component limb into caller
    /// storage. The fresh receipt must equal the manifest's post-publication
    /// receipt, including provider and immutable-snapshot identity.
    pub(in crate::vega::zk_ams::mkhe) fn read_linear_limb_into_v1<P>(
        &self,
        limb: usize,
        profile: &BgvProfile,
        provider: &mut P,
        destination: &mut [u64],
        scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
    ) -> Result<ZkAmsMkheDirectObjectReadReceiptV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        self.read_component_limb_into_v1(
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
            limb,
            profile,
            provider,
            destination,
            scratch,
        )
    }
}
impl core::fmt::Debug for ZkAmsMkheStreamingCollectiveCiphertextV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheStreamingCollectiveCiphertextV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("sample_index", &self.sample_index)
            .field("ciphertext_digest", &hex::encode(self.ciphertext_digest))
            .field("manifest_digest", &hex::encode(self.manifest_digest))
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheStreamingCollectiveCiphertextV1 {
    fn from_completed_v1(
        completed: CompletedStreamingCollectiveEncryptionV1,
        binding: &ZkAmsMkheStreamingCollectiveKeyBindingV1,
        key_authority_digest: [u8; 32],
        profile: &BgvProfile,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let StreamingCollectiveEncryptionRecordOwnersV1 {
            public_a_limb_pointers,
            public_b_limb_pointers,
            public_a_prepass_receipts,
            public_b_prepass_receipts,
            public_a_second_pass_receipts,
            public_b_second_pass_receipts,
            constant_limb_pointers,
            linear_limb_pointers,
            constant_publication_receipts,
            linear_publication_receipts,
            scratch: _,
        } = completed.records;
        let mut manifest = Self {
            version: MKHE_VERSION_V1,
            profile_digest: binding.profile_digest,
            security_certificate_digest: binding.security_certificate_digest,
            roster_digest: binding.roster_digest,
            key_material_digest: binding.key_material_digest,
            epoch: binding.epoch,
            key_transcript_digest: binding.transcript_digest,
            key_digest: binding.key_digest,
            key_binding_digest: binding.binding_digest,
            key_authority_digest,
            topology: completed.topology,
            sample_index: completed.sample_index,
            level: 0,
            transcript_digest: completed.transcript_digest,
            ciphertext_digest: completed.ciphertext_digest,
            public_a_limb_pointers,
            public_b_limb_pointers,
            constant_limb_pointers,
            linear_limb_pointers,
            public_a_prepass_receipts,
            public_b_prepass_receipts,
            public_a_second_pass_receipts,
            public_b_second_pass_receipts,
            constant_publication_receipts,
            linear_publication_receipts,
            manifest_digest: [0; 32],
        };
        manifest.manifest_digest =
            streaming_collective_ciphertext_manifest_digest_v1(&manifest, profile)?;
        manifest.validate_with_profile_digest_v1(profile, binding.profile_digest)?;
        Ok(manifest)
    }
    fn validate_for_profile_v1(&self, profile: &BgvProfile) -> Result<(), ZkAmsMkheErrorV1> {
        let profile_digest = profile.digest()?;
        self.validate_with_profile_digest_v1(profile, profile_digest)
    }
    fn validate_with_profile_digest_v1(
        &self,
        profile: &BgvProfile,
        profile_digest: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        profile.validate()?;
        let limbs = profile.moduli.len();
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != profile_digest
            || self.security_certificate_digest == [0; 32]
            || self.roster_digest == [0; 32]
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.key_transcript_digest == [0; 32]
            || self.key_digest == [0; 32]
            || self.key_binding_digest == [0; 32]
            || self.key_authority_digest == [0; 32]
            || self.topology.layout_digest == [0; 32]
            || self.topology.plaintext_used_slots == 0
            || usize::try_from(self.topology.plaintext_used_slots)
                .map_or(true, |used_slots| used_slots > profile.ring_degree)
            || self.transcript_digest == [0; 32]
            || self.level != 0
            || self.ciphertext_digest == [0; 32]
            || self.public_a_limb_pointers.len() != limbs
            || self.public_b_limb_pointers.len() != limbs
            || self.constant_limb_pointers.len() != limbs
            || self.linear_limb_pointers.len() != limbs
            || self.public_a_prepass_receipts.len() != limbs
            || self.public_b_prepass_receipts.len() != limbs
            || self.public_a_second_pass_receipts.len() != limbs
            || self.public_b_second_pass_receipts.len() != limbs
            || self.constant_publication_receipts.len() != limbs
            || self.linear_publication_receipts.len() != limbs
            || self.manifest_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let mut common_source_snapshot = None;
        for (kind, pointers, prepasses, second_passes) in [
            (
                ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
                self.public_a_limb_pointers.as_slice(),
                self.public_a_prepass_receipts.as_slice(),
                self.public_a_second_pass_receipts.as_slice(),
            ),
            (
                ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
                self.public_b_limb_pointers.as_slice(),
                self.public_b_prepass_receipts.as_slice(),
                self.public_b_second_pass_receipts.as_slice(),
            ),
        ] {
            for ((pointer, prepass), second_pass) in
                pointers.iter().zip(prepasses).zip(second_passes)
            {
                validate_streaming_second_source_receipt_v1(
                    prepass,
                    second_pass,
                    kind,
                    *pointer,
                    profile,
                )?;
                let axes = streaming_source_snapshot_axes_v1(prepass);
                match common_source_snapshot {
                    None => common_source_snapshot = Some(axes),
                    Some(expected) if expected == axes => {}
                    Some(_) => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
                }
            }
        }
        let mut output_publication_identity = None;
        let mut common_output_snapshot = None;
        for (kind, pointers, receipts) in [
            (
                ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
                self.constant_limb_pointers.as_slice(),
                self.constant_publication_receipts.as_slice(),
            ),
            (
                ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
                self.linear_limb_pointers.as_slice(),
                self.linear_publication_receipts.as_slice(),
            ),
        ] {
            for (pointer, receipt) in pointers.iter().zip(receipts) {
                validate_streaming_collective_limb_pointer_v1(kind, *pointer, profile)?;
                if receipt.pointer() != *pointer
                    || receipt.receipt_digest() == [0; 32]
                    || receipt.post_publish_read_receipt().snapshot().pointer() != *pointer
                    || receipt.post_publish_read_receipt().receipt_digest() == [0; 32]
                {
                    return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
                }
                match output_publication_identity {
                    None => output_publication_identity = Some(receipt.publication_identity()),
                    Some(expected) if expected == receipt.publication_identity() => {}
                    Some(_) => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
                }
                let axes = streaming_source_snapshot_axes_v1(receipt.post_publish_read_receipt());
                match common_output_snapshot {
                    None => common_output_snapshot = Some(axes),
                    Some(expected) if expected == axes => {}
                    Some(_) => return Err(ZkAmsMkheErrorV1::InvalidCiphertext),
                }
            }
        }
        if common_source_snapshot.is_none()
            || output_publication_identity.is_none()
            || common_output_snapshot.is_none()
            || self.manifest_digest
                != streaming_collective_ciphertext_manifest_digest_v1(self, profile)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }
    /// Validate the self-contained manifest beside the exact retained key authority.
    pub(super) fn validate_for_authority_v1(
        &self,
        authority: &ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        self.validate_with_profile_digest_v1(&profile, authority.profile_digest())?;
        let binding = authority.binding_v1();
        if self.profile_digest != binding.profile_digest
            || self.security_certificate_digest != binding.security_certificate_digest
            || self.roster_digest != binding.roster_digest
            || self.key_material_digest != binding.key_material_digest
            || self.epoch != binding.epoch
            || self.key_transcript_digest != binding.transcript_digest
            || self.key_digest != binding.key_digest
            || self.key_binding_digest != binding.binding_digest
            || self.key_authority_digest != authority.authority_digest
            || self.public_a_limb_pointers != binding.public_a_limb_pointers
            || self.public_b_limb_pointers != binding.public_b_limb_pointers
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }
    fn sealed_binding_with_profile_v1(
        &self,
        profile: &BgvProfile,
    ) -> Result<ZkAmsMkheStreamingCollectiveCiphertextBindingV1<'_>, ZkAmsMkheErrorV1> {
        self.validate_for_profile_v1(profile)?;
        Ok(ZkAmsMkheStreamingCollectiveCiphertextBindingV1 {
            _seal: StreamingCollectiveCiphertextBindingSealV1,
            profile_digest: self.profile_digest,
            security_certificate_digest: self.security_certificate_digest,
            roster_digest: self.roster_digest,
            key_material_digest: self.key_material_digest,
            epoch: self.epoch,
            key_transcript_digest: self.key_transcript_digest,
            key_digest: self.key_digest,
            key_binding_digest: self.key_binding_digest,
            key_authority_digest: self.key_authority_digest,
            sample_index: self.sample_index,
            level: self.level,
            transcript_digest: self.transcript_digest,
            ciphertext_digest: self.ciphertext_digest,
            constant_limb_pointers: &self.constant_limb_pointers,
            linear_limb_pointers: &self.linear_limb_pointers,
            constant_publication_receipts: &self.constant_publication_receipts,
            linear_publication_receipts: &self.linear_publication_receipts,
            manifest_digest: self.manifest_digest,
        })
    }
    /// Borrow the sole validated downstream binding. No raw-digest or
    /// pointer-only constructor exists for this capability.
    pub(crate) fn sealed_binding_v1(
        &self,
    ) -> Result<ZkAmsMkheStreamingCollectiveCiphertextBindingV1<'_>, ZkAmsMkheErrorV1> {
        self.sealed_binding_with_profile_v1(&release_profile_v1())
    }
    /// Frozen release-profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }
    /// Exact governed roster digest.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }
    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    /// Exact collective-public-key digest.
    #[must_use]
    pub const fn key_digest(&self) -> [u8; 32] {
        self.key_digest
    }
    /// Monotonic fresh-encryption sample index.
    #[must_use]
    pub const fn sample_index(&self) -> u64 {
        self.sample_index
    }
    /// Fresh ciphertext level. The bounded ingress path emits exactly level zero.
    #[must_use]
    pub const fn level(&self) -> u8 {
        self.level
    }
    /// Exact fresh-encryption transcript digest, byte-identical to the native path.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }
    /// Native component-major ciphertext digest without a native ciphertext owner.
    #[must_use]
    pub const fn ciphertext_digest(&self) -> [u8; 32] {
        self.ciphertext_digest
    }
    /// Ordered exact pointers to all 38 constant-component limbs.
    #[must_use]
    pub fn constant_limb_pointers(&self) -> &[ZkAmsMkheDirectObjectPointerV1] {
        &self.constant_limb_pointers
    }
    /// Ordered exact pointers to all 38 linear-component limbs.
    #[must_use]
    pub fn linear_limb_pointers(&self) -> &[ZkAmsMkheDirectObjectPointerV1] {
        &self.linear_limb_pointers
    }
    /// Digest binding key authority, source passes, output publications, and order.
    #[must_use]
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }
}
fn streaming_collective_ciphertext_manifest_digest_v1(
    manifest: &ZkAmsMkheStreamingCollectiveCiphertextV1,
    profile: &BgvProfile,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let limbs = profile.moduli.len();
    if manifest.public_a_limb_pointers.len() != limbs
        || manifest.public_b_limb_pointers.len() != limbs
        || manifest.constant_limb_pointers.len() != limbs
        || manifest.linear_limb_pointers.len() != limbs
        || manifest.public_a_prepass_receipts.len() != limbs
        || manifest.public_b_prepass_receipts.len() != limbs
        || manifest.public_a_second_pass_receipts.len() != limbs
        || manifest.public_b_second_pass_receipts.len() != limbs
        || manifest.constant_publication_receipts.len() != limbs
        || manifest.linear_publication_receipts.len() != limbs
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let mut hash = Keccak256::new();
    hash.update(STREAMING_COLLECTIVE_CIPHERTEXT_MANIFEST_DOMAIN_V1);
    hash.update(&[manifest.version]);
    hash.update(&manifest.profile_digest);
    hash.update(&manifest.security_certificate_digest);
    hash.update(&manifest.roster_digest);
    hash.update(&manifest.key_material_digest);
    hash.update(&manifest.epoch.to_be_bytes());
    hash.update(&manifest.key_transcript_digest);
    hash.update(&manifest.key_digest);
    hash.update(&manifest.key_binding_digest);
    hash.update(&manifest.key_authority_digest);
    hash.update(&manifest.topology.layout_digest);
    hash.update(&manifest.topology.plaintext_chunk_index.to_be_bytes());
    hash.update(&manifest.topology.plaintext_used_slots.to_be_bytes());
    hash.update(&manifest.sample_index.to_be_bytes());
    hash.update(&[manifest.level]);
    hash.update(&manifest.transcript_digest);
    hash.update(&manifest.ciphertext_digest);
    hash.update(
        &u32::try_from(limbs)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for (kind, pointers, prepasses, second_passes) in [
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicA,
            manifest.public_a_limb_pointers.as_slice(),
            manifest.public_a_prepass_receipts.as_slice(),
            manifest.public_a_second_pass_receipts.as_slice(),
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectivePublicB,
            manifest.public_b_limb_pointers.as_slice(),
            manifest.public_b_prepass_receipts.as_slice(),
            manifest.public_b_second_pass_receipts.as_slice(),
        ),
    ] {
        hash.update(&[kind as u8]);
        for (limb, (((modulus, pointer), prepass), second_pass)) in profile
            .moduli
            .iter()
            .zip(pointers)
            .zip(prepasses)
            .zip(second_passes)
            .enumerate()
        {
            hash.update(
                &u32::try_from(limb)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                    .to_be_bytes(),
            );
            hash.update(&modulus.to_be_bytes());
            hash.update(&pointer.pointer_digest());
            hash.update(&prepass.receipt_digest());
            hash.update(&second_pass.receipt_digest());
        }
    }
    for (kind, pointers, receipts) in [
        (
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC0,
            manifest.constant_limb_pointers.as_slice(),
            manifest.constant_publication_receipts.as_slice(),
        ),
        (
            ZkAmsMkheDirectObjectKindV1::CollectiveCiphertextC1,
            manifest.linear_limb_pointers.as_slice(),
            manifest.linear_publication_receipts.as_slice(),
        ),
    ] {
        hash.update(&[kind as u8]);
        for (limb, ((modulus, pointer), receipt)) in profile
            .moduli
            .iter()
            .zip(pointers)
            .zip(receipts)
            .enumerate()
        {
            hash.update(
                &u32::try_from(limb)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                    .to_be_bytes(),
            );
            hash.update(&modulus.to_be_bytes());
            hash.update(&pointer.pointer_digest());
            hash.update(&receipt.receipt_digest());
            hash.update(&receipt.post_publish_read_receipt().receipt_digest());
        }
    }
    Ok(hash.finalize())
}
/// Parent-private borrowed core with one synchronous pre-publication hook.
/// The public wrapper below still owns and erases the packed plaintext.
fn encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1<
    R,
    K,
    P,
    Prepare,
    F,
>(
    authority: &mut ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    layout: ZkAmsT256PackingLayoutV1,
    plaintext: &ZkAmsT256PackedPlaintextV1,
    random: &mut R,
    key_provider: &mut K,
    ciphertext_publisher: &mut P,
    prepare_before_entropy: Prepare,
) -> Result<ZkAmsMkheStreamingCollectiveCiphertextV1, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    K: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
    Prepare: FnOnce() -> Result<F, ZkAmsMkheErrorV1>,
    F: FnOnce(&[[u8; 32]], &[i64], &[i64], &[i64], &[u8; 32]) -> Result<(), ZkAmsMkheErrorV1>,
{
    let profile = release_profile_v1();
    authority.validate_release_v1()?;
    if authority.failed
        || authority.binding.security_certificate_digest != release_security_certificate_digest()?
        || layout.profile_digest != authority.binding.profile_digest
        || plaintext.profile_digest != authority.binding.profile_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let sample_index = authority.next_sample_index;
    let next_sample_index = sample_index
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if sample_index >= zk_ams_mkhe_release_manifest_v1()?.max_samples_per_secret_epoch {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    // This decoder scratch is scoped away before the two limbs, three
    // witnesses, record capacities, or provider transactions are established.
    {
        let _validated =
            ValidatedT256PackedPlaintextV1::validate_for_release_limb_stream_v1(layout, plaintext)?;
    }
    // Phase-23 uses this factory to allocate its complete confidential-record
    // pool after full packed validation but before any encryption owner,
    // provider call, nonce draw, or witness sample exists.
    let before_output_publication = prepare_before_entropy()?;
    let topology = CollectiveEncryptionInputTopologyV1::from_packed(layout, plaintext);
    let prepared = PreparedStreamingCollectiveEncryptionV1::new_v1(&authority.binding, &profile)?;
    // From here onward any returned provider/random/output error permanently
    // poisons this authority. All local allocations already exist, and a
    // caught unwind cannot resume a partially used source or witness state.
    authority.failed = true;
    let result = (|| {
        let authenticated = prepared.authenticate_key_source_v1(key_provider)?;
        let mut active = authenticated.activate_v1(
            &authority.binding,
            &plaintext.coefficients,
            topology,
            sample_index,
            random,
        )?;
        // The callback is parent-private, generic, and completes synchronously:
        // no witness or nonce borrow can outlive this call.  Its Phase-23
        // factory preallocated every confidential block before activation, so
        // the established no-local-allocation-after-entropy invariant remains
        // intact. Output publication cannot begin until this call succeeds.
        before_output_publication(
            active.kernel.canonical_plaintext,
            active.kernel.ephemeral.as_slice(),
            active.kernel.error_zero.as_slice(),
            active.kernel.error_one.as_slice(),
            active.kernel.input_identity.encryption_nonce.as_bytes(),
        )?;
        active.publish_all_v1(key_provider, ciphertext_publisher)?;
        let completed = active.finish()?;
        let manifest = ZkAmsMkheStreamingCollectiveCiphertextV1::from_completed_v1(
            completed,
            &authority.binding,
            authority.authority_digest,
            &profile,
        )?;
        manifest.validate_for_authority_v1(authority)?;
        Ok(manifest)
    })();
    match result {
        Ok(manifest) => {
            authority.next_sample_index = next_sample_index;
            authority.failed = false;
            Ok(manifest)
        }
        Err(error) => Err(error),
    }
}
/// Encrypt one validated packed plaintext with bounded key-source and output
/// memory. The packed owner is consumed and zeroized on every return path.
/// Sample order is authority-owned; callers cannot repeat or skip an index.
pub fn encrypt_zk_ams_mkhe_collective_packed_streaming_v1<R, K, P>(
    authority: &mut ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    layout: ZkAmsT256PackingLayoutV1,
    plaintext: ZkAmsT256PackedPlaintextV1,
    random: &mut R,
    key_provider: &mut K,
    ciphertext_publisher: &mut P,
) -> Result<ZkAmsMkheStreamingCollectiveCiphertextV1, ZkAmsMkheErrorV1>
where
    R: MaskedRelaxedRandomSourceV1,
    K: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    P: ZkAmsMkheDirectObjectCasPublicationV1 + ?Sized,
{
    encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1(
        authority,
        layout,
        &plaintext,
        random,
        key_provider,
        ciphertext_publisher,
        || Ok(|_: &[[u8; 32]], _: &[i64], _: &[i64], _: &[i64], _: &[u8; 32]| Ok(())),
    )
}
#[cfg(test)]
#[path = "incremental_source_tests.rs"]
mod tests;
