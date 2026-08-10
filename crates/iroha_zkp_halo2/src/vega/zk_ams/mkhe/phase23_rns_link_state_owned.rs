//! Streaming state-owner binding for native RNS-Link accumulator openings.
//!
//! This module stops at public, explicitly unverified aggregate metadata. It
//! does not prove a carry, quotient, packing, or Hyrax-equality equation and
//! cannot mint an operational receipt.

use core::ptr;

use super::super::{
    ZkAmsMkheErrorV1,
    collective::{
        ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheCollectiveEncryptionOpeningV1,
        ZkAmsMkheCollectivePublicKeyV1,
    },
    packing::{
        ZkAmsT256PackedPlaintextV1, ZkAmsT256PackingLayoutV1, zk_ams_t256_packing_layout_v1,
    },
    phase23_encrypted::ZkAmsPhase23PackedAccumulatorSetV1,
};
use super::{
    RNS_LINK_FAMILY_COUNT_V1, RNS_LINK_RELEASE_COMMITMENTS_V1, RNS_LINK_VERSION_V1,
    ZkAmsPhase23RnsLinkFamilyGeometryV1, ZkAmsPhase23RnsLinkFamilyV1,
    ZkAmsPhase23RnsLinkReleaseGeometryV1, ZkAmsPhase23RnsLinkUnverifiedNativePackedPreflightV1,
    derive_zk_ams_phase23_rns_link_release_geometry_v1,
    preflight_zk_ams_phase23_rns_link_native_packed_geometry_v1,
    verify_zk_ams_phase23_native_bgv_opening_v1,
};
use crate::vega::sponge::Keccak256;

const STATE_OWNED_OPENINGS_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.state-owned-native-bgv-openings";
const STATE_OWNED_OPENING_CHUNK_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.state-owned-native-bgv-chunk";

/// Exact number of native openings in `X/U/E/rE/W/rW` chunk order.
pub(super) const ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1: usize =
    RNS_LINK_RELEASE_COMMITMENTS_V1;

const CANONICAL_FAMILY_CHUNK_COUNTS_V1: [(ZkAmsPhase23RnsLinkFamilyV1, u16);
    RNS_LINK_FAMILY_COUNT_V1] = [
    (ZkAmsPhase23RnsLinkFamilyV1::X, 1),
    (ZkAmsPhase23RnsLinkFamilyV1::U, 16),
    (ZkAmsPhase23RnsLinkFamilyV1::E, 16),
    (ZkAmsPhase23RnsLinkFamilyV1::RE, 1),
    (ZkAmsPhase23RnsLinkFamilyV1::W, 8),
    (ZkAmsPhase23RnsLinkFamilyV1::RW, 1),
];

const _: () = {
    assert!(ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1 == 43);
    assert!(1 + 16 + 16 + 1 + 8 + 1 == ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalOpeningPositionV1 {
    family: ZkAmsPhase23RnsLinkFamilyV1,
    chunk_index: u16,
}

/// Move-only streaming verifier for one canonical packed accumulator owner.
///
/// `new` accepts no family, plaintext, or chunk-index labels. The only
/// accepted sequence is derived from `packed_owner` as
/// `X/U/E/rE/W/rW`, in ascending within-family chunk order. The exact
/// ciphertext-reference array fixes the corresponding public ciphertext
/// owner at every ordinal, while `common_key` fixes one public-key owner for
/// the complete stream.
///
/// Residual provenance: the caller still assembles the ciphertext-reference
/// array. This boundary proves pointer uniqueness and exact ordered opening to
/// canonical-owner binding, but not that one upstream state-owned encryption
/// producer handed over that array as a set.
///
/// Each call to `absorb_next_opening_v1` takes one owned secret opening. It
/// verifies and drops that opening before returning, immediately reduces the
/// short-lived checked capability into a hash record, and retains neither an
/// opening nor a checked capability. A failed or unwinding call poisons and
/// destroys the live stream; it cannot be retried. Deliberately neither
/// `Clone`, `Copy`, `Debug`, nor serializable.
#[must_use = "dropping this stream produces no RNS-Link equation"]
pub(super) struct StateOwnedRnsLinkAccumulatorOpeningsV1<'a> {
    live: Option<StateOwnedRnsLinkAccumulatorOpeningStreamV1<'a>>,
}

struct StateOwnedRnsLinkAccumulatorOpeningStreamV1<'a> {
    packed_owner: &'a ZkAmsPhase23PackedAccumulatorSetV1,
    common_key: &'a ZkAmsMkheCollectivePublicKeyV1,
    ciphertexts:
        [&'a ZkAmsMkheCollectiveCiphertextV1; ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1],
    geometry: ZkAmsPhase23RnsLinkReleaseGeometryV1,
    packed_preflight: ZkAmsPhase23RnsLinkUnverifiedNativePackedPreflightV1,
    next_opening: usize,
    ordered_opening_hash: Keccak256,
}

/// Aggregate public metadata emitted only after all 43 native openings were
/// consumed successfully in canonical state-owner order.
///
/// This is intentionally copyable, explicitly `Unverified` metadata, not a
/// proof or capability. No verifier, receipt minter, materializer, decrypter,
/// readiness gate, or audit gate accepts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23RnsLinkUnverifiedStateOwnedNativeBgvPreflightV1 {
    geometry_digest: [u8; 32],
    packed_preflight_digest: [u8; 32],
    packed_owner_chunk_root: [u8; 32],
    key_digest: [u8; 32],
    ordered_opening_root: [u8; 32],
    opening_count: u16,
    digest: [u8; 32],
}

impl<'a> StateOwnedRnsLinkAccumulatorOpeningsV1<'a> {
    /// Begin the exact 43-opening stream for one packed owner, one public-key
    /// owner, and one canonically ordered ciphertext-reference set.
    pub(super) fn new(
        packed_owner: &'a ZkAmsPhase23PackedAccumulatorSetV1,
        common_key: &'a ZkAmsMkheCollectivePublicKeyV1,
        ciphertexts: [&'a ZkAmsMkheCollectiveCiphertextV1;
            ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        // Pointer duplication is rejected before geometry/RNS preflight and
        // before any expensive secret-opening verification.
        validate_unique_pointer_set_v1(&ciphertexts)?;
        validate_common_ciphertext_context_v1(common_key, &ciphertexts)?;

        let geometry = derive_zk_ams_phase23_rns_link_release_geometry_v1()?;
        validate_canonical_geometry_v1(&geometry)?;
        let packed_preflight =
            preflight_zk_ams_phase23_rns_link_native_packed_geometry_v1(packed_owner)?;
        if packed_preflight.geometry_digest != geometry.digest
            || usize::from(packed_preflight.chunk_count)
                != ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }

        let key_digest = common_key.digest();
        if key_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut ordered_opening_hash = Keccak256::new();
        ordered_opening_hash.update(STATE_OWNED_OPENINGS_DOMAIN_V1);
        ordered_opening_hash.update(&[RNS_LINK_VERSION_V1]);
        ordered_opening_hash.update(&geometry.digest);
        ordered_opening_hash.update(&packed_preflight.digest);
        ordered_opening_hash.update(&packed_preflight.ordered_native_chunk_root);
        ordered_opening_hash.update(&key_digest);
        ordered_opening_hash.update(&packed_preflight.chunk_count.to_be_bytes());

        Ok(Self {
            live: Some(StateOwnedRnsLinkAccumulatorOpeningStreamV1 {
                packed_owner,
                common_key,
                ciphertexts,
                geometry,
                packed_preflight,
                next_opening: 0,
                ordered_opening_hash,
            }),
        })
    }

    /// Consume, verify, and forget exactly the next canonical owned opening.
    ///
    /// The return type exposes no family/index label, callback, checked token,
    /// partial digest, or retry handle. On error or unwind the live state is
    /// removed before verification begins and is never restored.
    pub(super) fn absorb_next_opening_v1(
        &mut self,
        opening: ZkAmsMkheCollectiveEncryptionOpeningV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        advance_poisoning_v1(&mut self.live, move |live| {
            live.absorb_next_opening_v1(opening)
        })
    }

    /// Finish by value only after exactly 43 successful ordered absorbs.
    ///
    /// Early finish consumes the stream and returns no partial metadata.
    pub(super) fn finish_into_unverified_metadata_v1(
        self,
    ) -> Result<ZkAmsPhase23RnsLinkUnverifiedStateOwnedNativeBgvPreflightV1, ZkAmsMkheErrorV1> {
        self.live
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .finish_into_unverified_metadata_v1()
    }
}

impl StateOwnedRnsLinkAccumulatorOpeningStreamV1<'_> {
    fn absorb_next_opening_v1(
        &mut self,
        opening: ZkAmsMkheCollectiveEncryptionOpeningV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let position = canonical_opening_position_v1(self.next_opening)
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let family_geometry = self.geometry.family(position.family)?;
        let expected_layout = zk_ams_t256_packing_layout_v1(family_geometry.packed_value_count)?;
        let expected_plaintext = canonical_packed_plaintext_v1(self.packed_owner, position)?;
        let expected_ciphertext =
            expected_borrow_for_ordinal_v1(&self.ciphertexts, self.next_opening)?;
        validate_expected_chunk_v1(
            &self.geometry,
            family_geometry,
            position,
            expected_layout,
            expected_plaintext,
            self.common_key,
            expected_ciphertext,
        )?;

        // The owned opening is dropped by this private verifier on success,
        // error, or unwind. The resulting small checked value is consumed in
        // this stack frame and is never retained by the stream.
        let verified = verify_zk_ams_phase23_native_bgv_opening_v1(
            self.common_key,
            expected_layout,
            expected_plaintext,
            expected_ciphertext,
            opening,
        )?;
        let sealed_rns_binding_digest = verified.rns_binding_digest;
        let geometry_digest = self.geometry.digest;
        let key_digest = self.common_key.digest();
        let mut chunk_digest = None;
        verified.consume(|key, layout, plaintext, ciphertext| {
            validate_consumed_chunk_v1(
                &self.geometry,
                family_geometry,
                position,
                self.common_key,
                expected_layout,
                expected_plaintext,
                expected_ciphertext,
                key,
                layout,
                plaintext,
                ciphertext,
            )?;
            let plaintext_digest = plaintext.digest;
            let ciphertext_digest = ciphertext.digest();
            if [
                plaintext_digest,
                sealed_rns_binding_digest,
                ciphertext_digest,
            ]
            .contains(&[0; 32])
            {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }

            let mut hash = Keccak256::new();
            hash.update(STATE_OWNED_OPENING_CHUNK_DOMAIN_V1);
            hash.update(&[RNS_LINK_VERSION_V1]);
            hash.update(&geometry_digest);
            hash.update(&[position.family as u8]);
            hash.update(&position.chunk_index.to_be_bytes());
            hash.update(&key_digest);
            hash.update(&layout.digest);
            hash.update(&plaintext_digest);
            hash.update(&sealed_rns_binding_digest);
            hash.update(&ciphertext_digest);
            let digest = hash.finalize();
            if digest == [0; 32] {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            chunk_digest = Some(digest);
            Ok(())
        })?;
        self.ordered_opening_hash
            .update(&chunk_digest.ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?);
        self.next_opening = self
            .next_opening
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(())
    }

    fn finish_into_unverified_metadata_v1(
        self,
    ) -> Result<ZkAmsPhase23RnsLinkUnverifiedStateOwnedNativeBgvPreflightV1, ZkAmsMkheErrorV1> {
        require_complete_opening_count_v1(self.next_opening)?;
        let Self {
            common_key,
            geometry,
            packed_preflight,
            ordered_opening_hash,
            ..
        } = self;
        let key_digest = common_key.digest();
        let ordered_opening_root = ordered_opening_hash.finalize();
        if key_digest == [0; 32] || ordered_opening_root == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }

        let mut hash = Keccak256::new();
        hash.update(STATE_OWNED_OPENINGS_DOMAIN_V1);
        hash.update(&[RNS_LINK_VERSION_V1]);
        hash.update(&geometry.digest);
        hash.update(&packed_preflight.digest);
        hash.update(&packed_preflight.ordered_native_chunk_root);
        hash.update(&key_digest);
        hash.update(&ordered_opening_root);
        hash.update(&packed_preflight.chunk_count.to_be_bytes());
        let digest = hash.finalize();
        if digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }

        Ok(
            ZkAmsPhase23RnsLinkUnverifiedStateOwnedNativeBgvPreflightV1 {
                geometry_digest: geometry.digest,
                packed_preflight_digest: packed_preflight.digest,
                packed_owner_chunk_root: packed_preflight.ordered_native_chunk_root,
                key_digest,
                ordered_opening_root,
                opening_count: packed_preflight.chunk_count,
                digest,
            },
        )
    }
}

fn canonical_opening_position_v1(ordinal: usize) -> Option<CanonicalOpeningPositionV1> {
    let mut family_start = 0_usize;
    for (family, chunk_count) in CANONICAL_FAMILY_CHUNK_COUNTS_V1 {
        let family_end = family_start.checked_add(usize::from(chunk_count))?;
        if ordinal < family_end {
            return Some(CanonicalOpeningPositionV1 {
                family,
                chunk_index: u16::try_from(ordinal.checked_sub(family_start)?).ok()?,
            });
        }
        family_start = family_end;
    }
    None
}

fn canonical_packed_plaintext_v1(
    packed: &ZkAmsPhase23PackedAccumulatorSetV1,
    position: CanonicalOpeningPositionV1,
) -> Result<&ZkAmsT256PackedPlaintextV1, ZkAmsMkheErrorV1> {
    let chunks = match position.family {
        ZkAmsPhase23RnsLinkFamilyV1::X => packed.x.as_slice(),
        ZkAmsPhase23RnsLinkFamilyV1::U => packed.u.as_slice(),
        ZkAmsPhase23RnsLinkFamilyV1::E => packed.e.as_slice(),
        ZkAmsPhase23RnsLinkFamilyV1::RE => packed.r_e.as_slice(),
        ZkAmsPhase23RnsLinkFamilyV1::W => packed.w.as_slice(),
        ZkAmsPhase23RnsLinkFamilyV1::RW => packed.r_w.as_slice(),
    };
    chunks
        .get(usize::from(position.chunk_index))
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn expected_borrow_for_ordinal_v1<'a, T>(
    items: &[&'a T; ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1],
    ordinal: usize,
) -> Result<&'a T, ZkAmsMkheErrorV1> {
    items
        .get(ordinal)
        .copied()
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn validate_canonical_geometry_v1(
    geometry: &ZkAmsPhase23RnsLinkReleaseGeometryV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let mut total = 0_usize;
    for (family, chunk_count) in CANONICAL_FAMILY_CHUNK_COUNTS_V1 {
        let actual = geometry.family(family)?;
        if actual.family != family || actual.chunk_count != chunk_count {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        total = total
            .checked_add(usize::from(chunk_count))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    if total != ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1
        || usize::from(geometry.commitment_count) != total
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn validate_unique_pointer_set_v1<T>(
    items: &[&T; ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    for (index, item) in items.iter().enumerate() {
        if items[..index].iter().any(|prior| ptr::eq(*prior, *item)) {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
    }
    Ok(())
}

fn validate_common_ciphertext_context_v1(
    key: &ZkAmsMkheCollectivePublicKeyV1,
    ciphertexts: &[&ZkAmsMkheCollectiveCiphertextV1;
         ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    let key_digest = key.digest();
    if key_digest == [0; 32]
        || ciphertexts.iter().any(|ciphertext| {
            ciphertext.digest() == [0; 32]
                || ciphertext.profile_digest() != key.profile_digest()
                || ciphertext.roster_digest() != key.roster_digest()
                || ciphertext.epoch() != key.epoch()
                || ciphertext.level() != 0
                || ciphertext.evaluation_key_digest() != Some(key_digest)
        })
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    Ok(())
}

fn validate_expected_chunk_v1(
    geometry: &ZkAmsPhase23RnsLinkReleaseGeometryV1,
    expected_family: ZkAmsPhase23RnsLinkFamilyGeometryV1,
    position: CanonicalOpeningPositionV1,
    expected_layout: ZkAmsT256PackingLayoutV1,
    expected_plaintext: &ZkAmsT256PackedPlaintextV1,
    common_key: &ZkAmsMkheCollectivePublicKeyV1,
    expected_ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let expected_used_slots = if position.chunk_index + 1 == expected_family.chunk_count {
        expected_family.final_chunk_used_slots
    } else {
        expected_layout.slots_per_chunk
    };
    if expected_family.family != position.family
        || expected_layout.profile_digest != geometry.profile_digest
        || expected_layout.digest != expected_family.packing_layout_digest
        || expected_layout.logical_value_count != expected_family.packed_value_count
        || expected_layout.chunk_count != u32::from(expected_family.chunk_count)
        || expected_plaintext.profile_digest != geometry.profile_digest
        || expected_plaintext.layout_digest != expected_layout.digest
        || expected_plaintext.chunk_index != u32::from(position.chunk_index)
        || expected_plaintext.used_slots != expected_used_slots
        || expected_ciphertext.profile_digest() != geometry.profile_digest
        || expected_ciphertext.evaluation_key_digest() != Some(common_key.digest())
        || geometry.family(position.family)? != expected_family
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_consumed_chunk_v1(
    geometry: &ZkAmsPhase23RnsLinkReleaseGeometryV1,
    expected_family: ZkAmsPhase23RnsLinkFamilyGeometryV1,
    position: CanonicalOpeningPositionV1,
    expected_key: &ZkAmsMkheCollectivePublicKeyV1,
    expected_layout: ZkAmsT256PackingLayoutV1,
    expected_plaintext: &ZkAmsT256PackedPlaintextV1,
    expected_ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    actual_key: &ZkAmsMkheCollectivePublicKeyV1,
    actual_layout: ZkAmsT256PackingLayoutV1,
    actual_plaintext: &ZkAmsT256PackedPlaintextV1,
    actual_ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_exact_pointer_binding_v1(
        expected_key,
        actual_key,
        expected_plaintext,
        actual_plaintext,
        expected_ciphertext,
        actual_ciphertext,
    )?;
    if actual_layout != expected_layout {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    validate_expected_chunk_v1(
        geometry,
        expected_family,
        position,
        actual_layout,
        actual_plaintext,
        actual_key,
        actual_ciphertext,
    )
}

fn validate_exact_pointer_binding_v1<K, P, C>(
    expected_key: &K,
    actual_key: &K,
    expected_plaintext: &P,
    actual_plaintext: &P,
    expected_ciphertext: &C,
    actual_ciphertext: &C,
) -> Result<(), ZkAmsMkheErrorV1> {
    if !ptr::eq(expected_key, actual_key)
        || !ptr::eq(expected_plaintext, actual_plaintext)
        || !ptr::eq(expected_ciphertext, actual_ciphertext)
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

fn advance_poisoning_v1<T>(
    live: &mut Option<T>,
    advance: impl FnOnce(&mut T) -> Result<(), ZkAmsMkheErrorV1>,
) -> Result<(), ZkAmsMkheErrorV1> {
    let mut state = live.take().ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    advance(&mut state)?;
    *live = Some(state);
    Ok(())
}

fn require_complete_opening_count_v1(count: usize) -> Result<(), ZkAmsMkheErrorV1> {
    if count != ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    use super::*;

    type BeginStateOwnedOpeningsV1 =
        for<'a> fn(
            &'a ZkAmsPhase23PackedAccumulatorSetV1,
            &'a ZkAmsMkheCollectivePublicKeyV1,
            [&'a ZkAmsMkheCollectiveCiphertextV1;
                ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1],
        ) -> Result<StateOwnedRnsLinkAccumulatorOpeningsV1<'a>, ZkAmsMkheErrorV1>;
    type AbsorbStateOwnedOpeningV1 = for<'a> fn(
        &mut StateOwnedRnsLinkAccumulatorOpeningsV1<'a>,
        ZkAmsMkheCollectiveEncryptionOpeningV1,
    ) -> Result<(), ZkAmsMkheErrorV1>;
    type FinishStateOwnedOpeningsV1 = for<'a> fn(
        StateOwnedRnsLinkAccumulatorOpeningsV1<'a>,
    ) -> Result<
        ZkAmsPhase23RnsLinkUnverifiedStateOwnedNativeBgvPreflightV1,
        ZkAmsMkheErrorV1,
    >;

    fn begin_state_owned_openings_v1<'a>(
        packed_owner: &'a ZkAmsPhase23PackedAccumulatorSetV1,
        common_key: &'a ZkAmsMkheCollectivePublicKeyV1,
        ciphertexts: [&'a ZkAmsMkheCollectiveCiphertextV1;
            ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1],
    ) -> Result<StateOwnedRnsLinkAccumulatorOpeningsV1<'a>, ZkAmsMkheErrorV1> {
        StateOwnedRnsLinkAccumulatorOpeningsV1::new(packed_owner, common_key, ciphertexts)
    }

    fn absorb_state_owned_opening_v1<'a>(
        stream: &mut StateOwnedRnsLinkAccumulatorOpeningsV1<'a>,
        opening: ZkAmsMkheCollectiveEncryptionOpeningV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        stream.absorb_next_opening_v1(opening)
    }

    fn finish_state_owned_openings_v1<'a>(
        stream: StateOwnedRnsLinkAccumulatorOpeningsV1<'a>,
    ) -> Result<ZkAmsPhase23RnsLinkUnverifiedStateOwnedNativeBgvPreflightV1, ZkAmsMkheErrorV1> {
        stream.finish_into_unverified_metadata_v1()
    }

    #[test]
    fn streaming_boundary_is_fixed_width_borrow_tied_and_small() {
        let _: BeginStateOwnedOpeningsV1 = begin_state_owned_openings_v1;
        let _: AbsorbStateOwnedOpeningV1 = absorb_state_owned_opening_v1;
        let _: FinishStateOwnedOpeningsV1 = finish_state_owned_openings_v1;
        assert_eq!(ZK_AMS_PHASE23_RNS_LINK_STATE_OWNED_OPENING_COUNT_V1, 43);
        // The checked single-opening value contains only borrowed public
        // artifacts, one copied layout, and one digest. Streaming is an API
        // and residency hardening, not a claim that this value owns the
        // opening's ciphertext-sized secret buffers.
        assert!(
            core::mem::size_of::<super::super::VerifiedZkAmsPhase23NativeBgvOpeningV1<'static>>()
                < 512
        );
        assert!(core::mem::size_of::<StateOwnedRnsLinkAccumulatorOpeningsV1<'static>>() < 4096);
    }

    #[test]
    fn canonical_positions_cover_exact_x_u_e_re_w_rw_order_once() {
        let expected = CANONICAL_FAMILY_CHUNK_COUNTS_V1;
        let mut ordinal = 0_usize;
        for (family, chunk_count) in expected {
            for chunk_index in 0..chunk_count {
                assert_eq!(
                    canonical_opening_position_v1(ordinal),
                    Some(CanonicalOpeningPositionV1 {
                        family,
                        chunk_index,
                    })
                );
                ordinal += 1;
            }
        }
        assert_eq!(ordinal, 43);
        assert_eq!(canonical_opening_position_v1(43), None);
        assert_eq!(canonical_opening_position_v1(usize::MAX), None);
    }

    #[test]
    fn early_finish_and_excess_opening_are_rejected() {
        for incomplete in [0, 1, 42, 44, usize::MAX] {
            assert_eq!(
                require_complete_opening_count_v1(incomplete),
                Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
            );
        }
        require_complete_opening_count_v1(43).unwrap();
        assert!(canonical_opening_position_v1(42).is_some());
        assert!(canonical_opening_position_v1(43).is_none());
    }

    #[test]
    fn duplicate_ciphertext_pointers_are_rejected_before_verification() {
        let values: [u16; 43] = core::array::from_fn(|index| u16::try_from(index).unwrap());
        let unique = core::array::from_fn(|index| &values[index]);
        validate_unique_pointer_set_v1(&unique).unwrap();
        let mut duplicate = unique;
        duplicate[42] = duplicate[0];
        assert_eq!(
            validate_unique_pointer_set_v1(&duplicate),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );

        let production = include_str!("phase23_rns_link_state_owned.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source prefix");
        assert!(
            production
                .find("validate_unique_pointer_set_v1(&ciphertexts)")
                .unwrap()
                < production
                    .find("derive_zk_ams_phase23_rns_link_release_geometry_v1()")
                    .unwrap()
        );
        assert!(
            production
                .find("validate_unique_pointer_set_v1(&ciphertexts)")
                .unwrap()
                < production
                    .find("verify_zk_ams_phase23_native_bgv_opening_v1(")
                    .unwrap()
        );
    }

    #[test]
    fn reorder_and_foreign_owner_key_or_ciphertext_pointers_are_rejected() {
        let owner = [7_u16, 7];
        let key = [11_u16, 11];
        let ciphertext = [13_u16, 17];
        validate_exact_pointer_binding_v1(
            &key[0],
            &key[0],
            &owner[0],
            &owner[0],
            &ciphertext[0],
            &ciphertext[0],
        )
        .unwrap();
        for result in [
            validate_exact_pointer_binding_v1(
                &key[0],
                &key[0],
                &owner[0],
                &owner[1],
                &ciphertext[0],
                &ciphertext[0],
            ),
            validate_exact_pointer_binding_v1(
                &key[0],
                &key[1],
                &owner[0],
                &owner[0],
                &ciphertext[0],
                &ciphertext[0],
            ),
            validate_exact_pointer_binding_v1(
                &key[0],
                &key[0],
                &owner[0],
                &owner[0],
                &ciphertext[0],
                &ciphertext[1],
            ),
        ] {
            assert_eq!(result, Err(ZkAmsMkheErrorV1::InvalidPhase23Fold));
        }

        let ciphertext_values: [u16; 43] =
            core::array::from_fn(|index| u16::try_from(index).unwrap());
        let ciphertexts = core::array::from_fn(|index| &ciphertext_values[index]);
        for ordinal in 0..43 {
            assert!(ptr::eq(
                expected_borrow_for_ordinal_v1(&ciphertexts, ordinal).unwrap(),
                &ciphertext_values[ordinal]
            ));
        }
        assert_eq!(
            expected_borrow_for_ordinal_v1(&ciphertexts, 43),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }

    struct DropProbe(Rc<Cell<bool>>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    #[test]
    fn error_and_unwind_destroy_live_state_and_forbid_retry() {
        let error_drop = Rc::new(Cell::new(false));
        let mut live = Some(DropProbe(Rc::clone(&error_drop)));
        assert_eq!(
            advance_poisoning_v1(&mut live, |_| { Err(ZkAmsMkheErrorV1::InvalidCiphertext) }),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );
        assert!(live.is_none());
        assert!(error_drop.get());

        let retry_called = Cell::new(false);
        assert_eq!(
            advance_poisoning_v1(&mut live, |_| {
                retry_called.set(true);
                Ok(())
            }),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
        assert!(!retry_called.get());

        let unwind_drop = Rc::new(Cell::new(false));
        let mut unwinding_live = Some(DropProbe(Rc::clone(&unwind_drop)));
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let _ =
                advance_poisoning_v1(&mut unwinding_live, |_| -> Result<(), ZkAmsMkheErrorV1> {
                    panic!("hostile verifier unwind")
                });
        }));
        assert!(caught.is_err());
        assert!(unwinding_live.is_none());
        assert!(unwind_drop.get());
    }

    #[test]
    fn production_surface_has_no_labels_partial_token_or_heavy_collection() {
        let source = include_str!("phase23_rns_link_state_owned.rs");
        let parent_source = include_str!("phase23_rns_link.rs");
        let collective_source = include_str!("collective.rs");
        let audit_source = include_str!("receipt_capability_audit.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source prefix");
        for forbidden in [
            "NoritoSerialize",
            "NoritoDeserialize",
            "serde",
            "release_kat_digest",
            "zk_ams_mkhe_readiness",
            "Vec<VerifiedZkAmsPhase23NativeBgvOpeningV1",
            "[VerifiedZkAmsPhase23NativeBgvOpeningV1",
            "Vec<ZkAmsMkheCollectiveEncryptionOpeningV1",
            "[ZkAmsMkheCollectiveEncryptionOpeningV1",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden production surface: {forbidden}"
            );
        }
        assert!(!production.contains("Vec<"));
        assert!(production.contains("ordered_opening_hash: Keccak256"));
        assert!(production.contains("live: Option<StateOwnedRnsLinkAccumulatorOpeningStreamV1"));
        assert!(
            production.contains(
                "Residual provenance: the caller still assembles the ciphertext-reference"
            )
        );
        assert!(!production.contains("pub(in super::super)"));

        let constructor = production
            .split("pub(super) fn new(")
            .nth(1)
            .expect("stream constructor")
            .split(") -> Result<Self")
            .next()
            .expect("constructor signature");
        for required in ["packed_owner:", "common_key:", "ciphertexts:"] {
            assert!(
                constructor.contains(required),
                "missing constructor input: {required}"
            );
        }
        for forbidden in ["family", "chunk_index", "opening:"] {
            assert!(
                !constructor.contains(forbidden),
                "caller label escaped: {forbidden}"
            );
        }

        let absorb = production
            .split("pub(super) fn absorb_next_opening_v1(")
            .nth(1)
            .expect("stream absorb")
            .split('{')
            .next()
            .expect("absorb signature");
        assert!(absorb.contains("&mut self"));
        assert!(absorb.contains("opening: ZkAmsMkheCollectiveEncryptionOpeningV1"));
        assert!(absorb.contains("Result<(), ZkAmsMkheErrorV1>"));
        for forbidden in ["family", "chunk_index", "impl Fn", "-> Result<Verified"] {
            assert!(
                !absorb.contains(forbidden),
                "absorb authority escaped: {forbidden}"
            );
        }

        let finish = production
            .split("pub(super) fn finish_into_unverified_metadata_v1(")
            .nth(1)
            .expect("stream finish")
            .split('{')
            .next()
            .expect("finish signature");
        assert!(finish.contains("self"));
        assert!(!finish.contains("&self"));
        assert!(finish.contains("Unverified"));
        assert_eq!(production.matches("pub(super) fn ").count(), 3);

        assert!(
            parent_source
                .contains("pub(super) struct ZkAmsPhase23NativeBgvOpeningVerifierPermitV1(());")
        );
        let permit_offset = parent_source
            .find("pub(super) struct ZkAmsPhase23NativeBgvOpeningVerifierPermitV1")
            .expect("opaque opening-verifier permit");
        let permit_attributes = parent_source[..permit_offset]
            .rsplit("\n\n")
            .next()
            .expect("permit attribute block");
        assert!(!permit_attributes.contains("#[derive"));
        for forbidden in [
            "impl Clone for ZkAmsPhase23NativeBgvOpeningVerifierPermitV1",
            "impl Copy for ZkAmsPhase23NativeBgvOpeningVerifierPermitV1",
            "impl Default for ZkAmsPhase23NativeBgvOpeningVerifierPermitV1",
            "NoritoSerialize for ZkAmsPhase23NativeBgvOpeningVerifierPermitV1",
            "NoritoDeserialize for ZkAmsPhase23NativeBgvOpeningVerifierPermitV1",
        ] {
            assert!(
                !parent_source.contains(forbidden),
                "permit gains forbidden construction or codec: {forbidden}"
            );
        }

        assert!(!collective_source.contains("with_validated_proof_witness_v1"));
        let unit_adapter = collective_source
            .split("pub(super) fn verify_and_consume_phase23_native_bgv_opening_v1(")
            .nth(1)
            .expect("unit-only collective opening adapter")
            .split('{')
            .next()
            .expect("unit-only adapter signature");
        for required in [
            "self,",
            "ZkAmsPhase23NativeBgvOpeningVerifierPermitV1",
            "Result<(), ZkAmsMkheErrorV1>",
        ] {
            assert!(
                unit_adapter.contains(required),
                "missing unit adapter pin: {required}"
            );
        }
        for forbidden in ["&self", "<T>", "impl Fn", "Result<T"] {
            assert!(
                !unit_adapter.contains(forbidden),
                "witness return corridor escaped: {forbidden}"
            );
        }
        assert_eq!(
            collective_source
                .matches("verify_and_consume_phase23_native_bgv_opening_v1")
                .count(),
            1
        );
        assert_eq!(
            parent_source
                .matches("verify_and_consume_phase23_native_bgv_opening_v1")
                .count(),
            1
        );
        assert!(!production.contains("verify_and_consume_phase23_native_bgv_opening_v1"));

        assert!(parent_source.contains("\nstruct VerifiedZkAmsPhase23NativeBgvOpeningV1"));
        assert!(
            !parent_source.contains("pub(super) struct VerifiedZkAmsPhase23NativeBgvOpeningV1")
        );
        assert!(parent_source.contains("\nfn verify_zk_ams_phase23_native_bgv_opening_v1"));
        assert!(
            !parent_source.contains("pub(super) fn verify_zk_ams_phase23_native_bgv_opening_v1")
        );
        let verified_impl = parent_source
            .split("impl<'a> VerifiedZkAmsPhase23NativeBgvOpeningV1<'a>")
            .nth(1)
            .expect("private checked-value implementation")
            .split("#[path = \"phase23_rns_link_state_owned.rs\"]")
            .next()
            .expect("private checked-value implementation boundary");
        assert!(verified_impl.contains("\n    fn consume("));
        assert!(!verified_impl.contains("pub(super) fn consume("));
        assert!(parent_source.contains(
            "#[cfg(test)]\npub(super) fn test_verify_and_consume_zk_ams_phase23_native_bgv_opening_v1"
        ));
        assert!(parent_source.contains(
            "#[path = \"phase23_rns_link_state_owned.rs\"]\nmod state_owned;\n#[cfg(test)]\npub(super) use state_owned::"
        ));
        assert!(!parent_source.contains("pub(super) mod state_owned"));

        for forbidden in [
            "VerifiedZkAmsPhase23NativeBgvOpeningV1",
            "VerifyNativeBgvOpeningV1",
            "verify_zk_ams_phase23_native_bgv_opening_v1",
        ] {
            assert!(
                !audit_source.contains(forbidden),
                "audit pins individual opening boundary: {forbidden}"
            );
        }
        for required in [
            "BeginStateOwnedNativeBgvOpeningsV1",
            "AbsorbStateOwnedNativeBgvOpeningV1",
            "FinishStateOwnedNativeBgvOpeningsV1",
        ] {
            assert!(
                audit_source.contains(required),
                "audit omits aggregate streaming boundary: {required}"
            );
        }

        let canonical_order = [
            "ZkAmsPhase23RnsLinkFamilyV1::X => packed.x.as_slice()",
            "ZkAmsPhase23RnsLinkFamilyV1::U => packed.u.as_slice()",
            "ZkAmsPhase23RnsLinkFamilyV1::E => packed.e.as_slice()",
            "ZkAmsPhase23RnsLinkFamilyV1::RE => packed.r_e.as_slice()",
            "ZkAmsPhase23RnsLinkFamilyV1::W => packed.w.as_slice()",
            "ZkAmsPhase23RnsLinkFamilyV1::RW => packed.r_w.as_slice()",
        ];
        let mut remainder = production;
        for entry in canonical_order {
            remainder = remainder
                .split(entry)
                .nth(1)
                .unwrap_or_else(|| panic!("missing or reordered canonical family: {entry}"));
        }
    }
}
