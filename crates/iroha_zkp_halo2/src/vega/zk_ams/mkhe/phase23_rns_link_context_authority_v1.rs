//! Sole field and construction authority for the private RNS-Link context.

#![forbid(unsafe_code)]

use super::super::{
    ZkAmsMkheErrorV1, manifest::release_profile_v1,
    phase23_encrypted::zk_ams_phase23_release_map_set_digest_v1,
};
use super::{CONTEXT_DOMAIN_V1, RNS_LINK_VERSION_V1, immutable_algorithm_manifest_digest_v1};
use crate::vega::sponge::keccak256;

/// Leaf-private non-duplicable storage for one context axis. Its missing
/// `Clone` and `Copy` implementations make an external `Copy` implementation
/// for the containing context ill-formed in safe Rust.
struct ContextAxisDigestV1([u8; 32]);

/// Opaque binding of every immutable context axis that precedes an RNS-Link commitment.
///
/// The type is visible at the existing MKHE-private scope, while its fields remain visible only in
/// this leaf. Production deliberately has no way to construct or duplicate a value.
pub(in super::super) struct ZkAmsPhase23RnsLinkContextV1 {
    profile_digest: ContextAxisDigestV1,
    algorithm_manifest_digest: ContextAxisDigestV1,
    network_context_digest: ContextAxisDigestV1,
    statement_context_digest: ContextAxisDigestV1,
    transcript_digest: ContextAxisDigestV1,
    batch_digest: ContextAxisDigestV1,
    roster_digest: ContextAxisDigestV1,
    direct_key_admission_digest: ContextAxisDigestV1,
    canonical_map_set_digest: ContextAxisDigestV1,
}

impl ZkAmsPhase23RnsLinkContextV1 {
    pub(super) fn digest(&self) -> [u8; 32] {
        let mut frame = Vec::with_capacity(CONTEXT_DOMAIN_V1.len() + 2 + 9 * 32);
        frame.extend_from_slice(CONTEXT_DOMAIN_V1);
        frame.push(RNS_LINK_VERSION_V1);
        frame.extend_from_slice(&self.profile_digest.0);
        frame.extend_from_slice(&self.algorithm_manifest_digest.0);
        frame.extend_from_slice(&self.network_context_digest.0);
        frame.extend_from_slice(&self.statement_context_digest.0);
        frame.extend_from_slice(&self.transcript_digest.0);
        frame.extend_from_slice(&self.batch_digest.0);
        frame.extend_from_slice(&self.roster_digest.0);
        frame.extend_from_slice(&self.direct_key_admission_digest.0);
        frame.extend_from_slice(&self.canonical_map_set_digest.0);
        keccak256(&frame)
    }

    pub(super) fn validated_release_binding_digests_v1(
        &self,
    ) -> Result<([u8; 32], [u8; 32]), ZkAmsMkheErrorV1> {
        let profile_digest = release_profile_v1().digest()?;
        let algorithm_manifest_digest = immutable_algorithm_manifest_digest_v1()?;
        let canonical_map_set_digest = zk_ams_phase23_release_map_set_digest_v1()?;
        if self.profile_digest.0 != profile_digest
            || self.algorithm_manifest_digest.0 != algorithm_manifest_digest
            || self.canonical_map_set_digest.0 != canonical_map_set_digest
            || [
                self.profile_digest.0,
                self.algorithm_manifest_digest.0,
                self.network_context_digest.0,
                self.statement_context_digest.0,
                self.transcript_digest.0,
                self.batch_digest.0,
                self.roster_digest.0,
                self.direct_key_admission_digest.0,
                self.canonical_map_set_digest.0,
            ]
            .contains(&[0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok((profile_digest, algorithm_manifest_digest))
    }
}

#[cfg(test)]
impl ZkAmsPhase23RnsLinkContextV1 {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn new(
        network_context_digest: [u8; 32],
        statement_context_digest: [u8; 32],
        transcript_digest: [u8; 32],
        batch_digest: [u8; 32],
        roster_digest: [u8; 32],
        direct_key_admission_digest: [u8; 32],
        canonical_map_set_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let context = Self {
            profile_digest: ContextAxisDigestV1(release_profile_v1().digest()?),
            algorithm_manifest_digest: ContextAxisDigestV1(
                immutable_algorithm_manifest_digest_v1()?
            ),
            network_context_digest: ContextAxisDigestV1(network_context_digest),
            statement_context_digest: ContextAxisDigestV1(statement_context_digest),
            transcript_digest: ContextAxisDigestV1(transcript_digest),
            batch_digest: ContextAxisDigestV1(batch_digest),
            roster_digest: ContextAxisDigestV1(roster_digest),
            direct_key_admission_digest: ContextAxisDigestV1(direct_key_admission_digest),
            canonical_map_set_digest: ContextAxisDigestV1(canonical_map_set_digest),
        };
        context.validated_release_binding_digests_v1()?;
        Ok(context)
    }

    /// Produce one bounded hostile fixture without exposing a field or a raw
    /// production constructor. There are exactly nine 32-byte context axes.
    pub(super) fn with_test_axis_byte_flipped_v1(&self, axis: usize, byte: usize) -> Option<Self> {
        if axis >= 9 || byte >= 32 {
            return None;
        }
        let mut changed = Self {
            profile_digest: ContextAxisDigestV1(self.profile_digest.0),
            algorithm_manifest_digest: ContextAxisDigestV1(self.algorithm_manifest_digest.0),
            network_context_digest: ContextAxisDigestV1(self.network_context_digest.0),
            statement_context_digest: ContextAxisDigestV1(self.statement_context_digest.0),
            transcript_digest: ContextAxisDigestV1(self.transcript_digest.0),
            batch_digest: ContextAxisDigestV1(self.batch_digest.0),
            roster_digest: ContextAxisDigestV1(self.roster_digest.0),
            direct_key_admission_digest: ContextAxisDigestV1(self.direct_key_admission_digest.0),
            canonical_map_set_digest: ContextAxisDigestV1(self.canonical_map_set_digest.0),
        };
        let selected = match axis {
            0 => &mut changed.profile_digest,
            1 => &mut changed.algorithm_manifest_digest,
            2 => &mut changed.network_context_digest,
            3 => &mut changed.statement_context_digest,
            4 => &mut changed.transcript_digest,
            5 => &mut changed.batch_digest,
            6 => &mut changed.roster_digest,
            7 => &mut changed.direct_key_admission_digest,
            8 => &mut changed.canonical_map_set_digest,
            _ => return None,
        };
        selected.0[byte] ^= 1;
        Some(changed)
    }
}
