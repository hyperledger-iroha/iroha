//! Exact static byte/work accounting for the collective-ingress candidate.
//!
//! This certificate intentionally distinguishes algebraic accounting from
//! release evidence.  Static sizes can be derived before the contribution
//! proofs exist, but they do not close the resource gate: canonical proof
//! sizes, a full Phase-II/III work trace, and measured release-size peak memory
//! must all be pinned by the release KAT.

use super::{
    BgvProfile, ZkAmsMkheErrorV1, checked_hybrid_streaming_workspace_bytes,
    compact_collective_key_switch_ring_multiplication_count,
    packing::ZK_AMS_T256_GALOIS_KEY_COUNT_V1, phase23_max_composed_rotation_key_switch_count,
    ring_multiplication_work, wire::derive_wire_length_certificate_v1,
};

/// Exact, overflow-checked static resource certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheResourceCertificateV1 {
    /// Exact canonical bytes of the fixed eight-party governed roster.
    pub governed_roster_wire_bytes: u64,
    /// Cyclotomic degree used by every RNS polynomial.
    pub ring_degree: u32,
    /// Number of frozen RNS limbs.
    pub rns_limb_count: u8,
    /// Exact canonical bytes of one limb-major RNS polynomial.
    pub rns_polynomial_wire_bytes: u64,
    /// Exact canonical bytes of one compact two-polynomial collective ciphertext.
    pub compact_collective_ciphertext_wire_bytes: u64,
    /// Exact transient bytes of the three-polynomial multiplication result.
    pub multiplication_triple_wire_bytes: u64,
    /// Exact canonical bytes of one collective seeded-`a`, stored-`b` `s^2` key.
    pub seeded_collective_relinearization_key_wire_bytes: u64,
    /// One relinearization key plus the complete minimal Galois-key schedule.
    pub collective_evaluated_key_count: u8,
    /// Exact bytes of the complete content-addressed collective evaluated-key payload.
    pub total_collective_evaluated_key_artifact_bytes: u64,
    /// Contribution bytes before the as-yet-unfrozen active-security proof.
    pub streamed_contribution_base_wire_bytes: u64,
    /// Fixed bytes before points/scalars/auxiliary in one proof envelope.
    pub proof_envelope_header_wire_bytes: u64,
    /// Remaining proof budget under the per-round ceiling.
    pub max_round_contribution_proof_bytes: u64,
    /// Remaining proof budget under the per-share ceiling.
    pub max_decryption_share_proof_bytes: u64,
    /// Portable accounted peak of the streamed hybrid key-switch workspace.
    pub streamed_hybrid_workspace_bytes: u64,
    /// Abstract work units for one full-RNS negacyclic multiplication.
    pub ring_multiplication_work_units: u64,
    /// Abstract work units for a complete streamed 38-limb hybrid key switch.
    pub hybrid_key_switch_work_units: u64,
    /// Maximum constituent key switches in one canonical signed-binary packed
    /// rotation under the release slot topology.
    pub max_composed_rotation_key_switch_count: u8,
    /// Conservative work units for the longest canonical composed rotation
    /// using proof-bound compact collective Galois keys.
    pub max_composed_rotation_work_units: u64,
    /// The compact ciphertext is within its frozen ceiling.
    pub ciphertext_ceiling_met: bool,
    /// Every individual compact evaluated key is within its frozen per-key ceiling.
    pub per_evaluated_key_ceiling_met: bool,
    /// The portable streamed workspace is within its frozen ceiling.
    pub workspace_ceiling_met: bool,
    /// The longest canonical composed rotation is within the per-operation
    /// work ceiling.
    pub composed_rotation_work_ceiling_met: bool,
    /// Canonical CKS/RKG/share proof sizes have been implemented and certified.
    pub contribution_proof_sizes_certified: bool,
    /// The complete evaluated-key payload and manifest have passed the SoraFS
    /// publication, retrieval, digest, and streaming-verification KAT.
    pub evaluated_key_artifact_transport_certified: bool,
    /// A complete Phase-II/III work trace has been measured below its ceiling.
    pub phase23_work_measured: bool,
    /// Release-parameter peak resident memory has been measured and pinned.
    pub release_peak_memory_measured: bool,
}

impl ZkAmsMkheResourceCertificateV1 {
    /// Return true only when both static accounting and measured release
    /// evidence are complete.
    #[must_use]
    pub const fn is_release_ready(self) -> bool {
        self.ciphertext_ceiling_met
            && self.per_evaluated_key_ceiling_met
            && self.workspace_ceiling_met
            && self.composed_rotation_work_ceiling_met
            && self.contribution_proof_sizes_certified
            && self.evaluated_key_artifact_transport_certified
            && self.phase23_work_measured
            && self.release_peak_memory_measured
    }
}

pub(super) fn derive_resource_certificate_v1(
    profile: &BgvProfile,
    party_count: usize,
) -> Result<ZkAmsMkheResourceCertificateV1, ZkAmsMkheErrorV1> {
    profile.validate()?;
    if !profile.hybrid_rns_decomposition || !(2..=8).contains(&party_count) {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }

    let wire = derive_wire_length_certificate_v1(profile)?;
    let polynomial = wire.rns_polynomial_wire_bytes;
    let compact_ciphertext = wire.compact_collective_ciphertext_wire_bytes;
    let multiplication_triple = wire.multiplication_triple_wire_bytes;
    let seeded_key = wire.seeded_collective_relinearization_key_wire_bytes;
    let collective_evaluated_key_count = ZK_AMS_T256_GALOIS_KEY_COUNT_V1
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let total_collective_evaluated_key_artifact_bytes = seeded_key
        .checked_mul(collective_evaluated_key_count)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let contribution_base = wire.streamed_contribution_base_wire_bytes;
    let round_proof_budget = profile
        .max_round_bytes
        .checked_sub(contribution_base)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let share_proof_budget = profile
        .max_share_bytes
        .checked_sub(contribution_base)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let workspace = checked_hybrid_streaming_workspace_bytes(profile)?;
    let multiplication_work = ring_multiplication_work(profile)?;
    let key_switch_multiplications =
        compact_collective_key_switch_ring_multiplication_count(profile, 1)?;
    let key_switch_work = multiplication_work
        .checked_mul(
            u64::try_from(key_switch_multiplications)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let slot_count = profile.ring_degree / 2;
    let max_composed_rotation_key_switch_count =
        phase23_max_composed_rotation_key_switch_count(slot_count)?;
    let max_composed_rotation_multiplications =
        compact_collective_key_switch_ring_multiplication_count(
            profile,
            max_composed_rotation_key_switch_count,
        )?;
    let max_composed_rotation_work = multiplication_work
        .checked_mul(
            u64::try_from(max_composed_rotation_multiplications)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;

    Ok(ZkAmsMkheResourceCertificateV1 {
        governed_roster_wire_bytes: as_u64(wire.governed_roster_wire_bytes)?,
        ring_degree: u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        rns_limb_count: u8::try_from(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        rns_polynomial_wire_bytes: as_u64(polynomial)?,
        compact_collective_ciphertext_wire_bytes: as_u64(compact_ciphertext)?,
        multiplication_triple_wire_bytes: as_u64(multiplication_triple)?,
        seeded_collective_relinearization_key_wire_bytes: as_u64(seeded_key)?,
        collective_evaluated_key_count: u8::try_from(collective_evaluated_key_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        total_collective_evaluated_key_artifact_bytes: as_u64(
            total_collective_evaluated_key_artifact_bytes,
        )?,
        streamed_contribution_base_wire_bytes: as_u64(contribution_base)?,
        proof_envelope_header_wire_bytes: as_u64(wire.proof_envelope_header_wire_bytes)?,
        max_round_contribution_proof_bytes: as_u64(round_proof_budget)?,
        max_decryption_share_proof_bytes: as_u64(share_proof_budget)?,
        streamed_hybrid_workspace_bytes: as_u64(workspace)?,
        ring_multiplication_work_units: multiplication_work,
        hybrid_key_switch_work_units: key_switch_work,
        max_composed_rotation_key_switch_count: u8::try_from(
            max_composed_rotation_key_switch_count,
        )
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        max_composed_rotation_work_units: max_composed_rotation_work,
        ciphertext_ceiling_met: compact_ciphertext <= profile.max_ciphertext_bytes,
        per_evaluated_key_ceiling_met: seeded_key <= profile.max_evaluated_key_bytes,
        workspace_ceiling_met: workspace <= profile.max_workspace_bytes,
        composed_rotation_work_ceiling_met: max_composed_rotation_work <= profile.max_work_units,
        // These remain false until the canonical proof system and release KAT
        // exist.  Static formulas are not substituted for runtime evidence.
        contribution_proof_sizes_certified: false,
        evaluated_key_artifact_transport_certified: false,
        phase23_work_measured: false,
        release_peak_memory_measured: false,
    })
}

fn as_u64(value: usize) -> Result<u64, ZkAmsMkheErrorV1> {
    u64::try_from(value).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_static_accounting_is_exact_but_not_runtime_evidence() {
        let profile = super::super::manifest::release_profile_v1();
        let certificate =
            derive_resource_certificate_v1(&profile, 8).expect("resource certificate");
        assert_eq!(certificate.governed_roster_wire_bytes, 302);
        assert_eq!(certificate.rns_polynomial_wire_bytes, 39_845_892);
        assert_eq!(
            certificate.compact_collective_ciphertext_wire_bytes,
            79_691_906
        );
        assert_eq!(certificate.multiplication_triple_wire_bytes, 119_537_798);
        assert_eq!(
            certificate.seeded_collective_relinearization_key_wire_bytes,
            1_514_144_113
        );
        assert_eq!(certificate.collective_evaluated_key_count, 32);
        assert_eq!(
            certificate.total_collective_evaluated_key_artifact_bytes,
            48_452_611_616
        );
        assert_eq!(
            certificate.streamed_contribution_base_wire_bytes,
            39_846_173
        );
        assert_eq!(certificate.proof_envelope_header_wire_bytes, 151);
        assert_eq!(certificate.max_round_contribution_proof_bytes, 27_262_691);
        assert_eq!(certificate.max_decryption_share_proof_bytes, 27_262_691);
        assert_eq!(certificate.streamed_hybrid_workspace_bytes, 122_683_404);
        assert_eq!(certificate.ring_multiplication_work_units, 89_653_248);
        assert_eq!(certificate.hybrid_key_switch_work_units, 6_813_646_848);
        assert_eq!(certificate.max_composed_rotation_key_switch_count, 8);
        assert_eq!(certificate.max_composed_rotation_work_units, 54_509_174_784);
        assert!(certificate.ciphertext_ceiling_met);
        assert!(certificate.per_evaluated_key_ceiling_met);
        assert!(certificate.workspace_ceiling_met);
        assert!(certificate.composed_rotation_work_ceiling_met);
        assert!(!certificate.contribution_proof_sizes_certified);
        assert!(!certificate.evaluated_key_artifact_transport_certified);
        assert!(!certificate.phase23_work_measured);
        assert!(!certificate.release_peak_memory_measured);
        assert!(!certificate.is_release_ready());
    }

    #[test]
    fn every_static_ceiling_fails_independently_at_one_byte_below_accounting() {
        let baseline = super::super::manifest::release_profile_v1();
        let certificate =
            derive_resource_certificate_v1(&baseline, 8).expect("resource certificate");

        let mut ciphertext = baseline.clone();
        ciphertext.max_ciphertext_bytes =
            usize::try_from(certificate.compact_collective_ciphertext_wire_bytes - 1).unwrap();
        let ciphertext_certificate = derive_resource_certificate_v1(&ciphertext, 8)
            .expect("structural profile permits canonical wire ceiling check");
        assert!(!ciphertext_certificate.ciphertext_ceiling_met);

        let mut evaluated_key = baseline.clone();
        evaluated_key.max_evaluated_key_bytes =
            usize::try_from(certificate.seeded_collective_relinearization_key_wire_bytes - 1)
                .unwrap();
        let key_certificate = derive_resource_certificate_v1(&evaluated_key, 8)
            .expect("structural profile permits key check");
        assert!(!key_certificate.per_evaluated_key_ceiling_met);

        let mut composed_exact = baseline.clone();
        composed_exact.max_work_units = certificate.max_composed_rotation_work_units;
        let composed_exact_certificate = derive_resource_certificate_v1(&composed_exact, 8)
            .expect("exact composed-rotation work boundary");
        assert!(composed_exact_certificate.composed_rotation_work_ceiling_met);

        let mut composed_below = baseline.clone();
        composed_below.max_work_units = certificate.max_composed_rotation_work_units - 1;
        let composed_below_certificate = derive_resource_certificate_v1(&composed_below, 8)
            .expect("one-unit-below composed-rotation work boundary");
        assert!(!composed_below_certificate.composed_rotation_work_ceiling_met);

        let mut round = baseline.clone();
        round.max_round_bytes =
            usize::try_from(certificate.streamed_contribution_base_wire_bytes - 1).unwrap();
        assert_eq!(
            derive_resource_certificate_v1(&round, 8),
            Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
        );

        let mut share = baseline.clone();
        share.max_share_bytes =
            usize::try_from(certificate.streamed_contribution_base_wire_bytes - 1).unwrap();
        assert_eq!(
            derive_resource_certificate_v1(&share, 8),
            Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
        );

        for party_count in [0, 1, 9, usize::MAX] {
            assert_eq!(
                derive_resource_certificate_v1(&baseline, party_count),
                Err(ZkAmsMkheErrorV1::InvalidProfile)
            );
        }

        let mut workspace = baseline;
        workspace.max_workspace_bytes =
            usize::try_from(certificate.streamed_hybrid_workspace_bytes - 1).unwrap();
        assert_eq!(
            derive_resource_certificate_v1(&workspace, 8),
            Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
        );
    }

    #[test]
    fn compact_collective_runtime_work_is_roster_size_independent() {
        let profile = super::super::manifest::release_profile_v1();
        let baseline = derive_resource_certificate_v1(&profile, 8).expect("eight-party release");
        assert_eq!(baseline.max_composed_rotation_key_switch_count, 8);
        assert_eq!(baseline.max_composed_rotation_work_units, 54_509_174_784);
        assert!(baseline.composed_rotation_work_ceiling_met);

        for party_count in 2..=8 {
            let candidate =
                derive_resource_certificate_v1(&profile, party_count).expect("valid roster size");
            assert_eq!(
                candidate.max_composed_rotation_work_units,
                baseline.max_composed_rotation_work_units,
                "offline CKS compaction must remove the runtime roster factor"
            );
        }
    }
}
