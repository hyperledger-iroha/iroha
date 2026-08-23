//! Sealed q-native relation-adapter prerequisite for the 43 native openings.
//!
//! This child is deliberately narrower than a Fiat-Shamir relation adapter.
//! It can be constructed only inside Phase 23 and can advance only when the
//! collective opening verifier lends its unconstructible permit after checking
//! both native RLWE equations.  It fixes the 43-opening order, counts the two
//! equations across all 38 release limbs, and binds only public geometry and canonical topology.
//!
//! It does not receive, retain, or hash witness coefficients or derived
//! q-native relation values.  The current Merkle + FRI prototype is not hiding,
//! so producing relation polynomials or opening values here would disclose
//! witness information. Fresh encryption lineage is nonce-hiding, but this
//! adapter still has no deterministic source/witness link, and the
//! packed-plaintext owner remains public `Clone + Debug` rather than move-only
//! and zeroizing. Fresh `(r, gamma, beta)` relation aggregation, zero-knowledge
//! masking, source-link hiding, secret-owner hardening, Hyrax
//! cross-representation equality, external-store proving, wire integration,
//! and every release/readiness/audit gate therefore remain unimplemented and
//! false.
use super::super::ZkAmsMkheErrorV1;
use super::{
    RNS_LINK_FAMILY_COUNT_V1, RNS_LINK_RELEASE_COMMITMENTS_V1, RNS_LINK_VERSION_V1,
    ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1,
    ZkAmsPhase23NativeBgvOpeningVerifierPermitV1, ZkAmsPhase23RnsLinkFamilyV1,
    ZkAmsPhase23RnsLinkReleaseGeometryV1,
    q_pcs::zk_ams_phase23_rns_link_q_pcs_release_parameter_digest_v1,
};
use crate::vega::sponge::Keccak256;
const Q_NATIVE_RELATION_ADAPTER_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.q-native-relation-adapter-prerequisite";
const Q_NATIVE_TOPOLOGY_OPENING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.q-native-topology-opening";
const Q_NATIVE_RELATION_METADATA_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.q-native-relation-unverified-metadata";
const RLWE_EQUATIONS_PER_OPENING_V1: u16 = 2;
const RELEASE_OPENING_COUNT_V1: u16 = RNS_LINK_RELEASE_COMMITMENTS_V1 as u16;
const RELEASE_RLWE_EQUATION_COUNT_V1: u16 =
    RELEASE_OPENING_COUNT_V1 * RLWE_EQUATIONS_PER_OPENING_V1;
const RELEASE_Q_NATIVE_RELATION_COORDINATE_COUNT_V1: u32 = RELEASE_RLWE_EQUATION_COUNT_V1 as u32
    * ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1 as u32;
const WITNESS_POLYNOMIALS_CONSTRUCTED_V1: bool = false;
const FIAT_SHAMIR_RELATION_BOUND_V1: bool = false;
const ZERO_KNOWLEDGE_MASKED_V1: bool = false;
const DETERMINISTIC_PLAINTEXT_LINEAGE_HIDDEN_V1: bool = false;
const SECRET_PACKED_PLAINTEXT_OWNER_HARDENED_V1: bool = false;
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
    assert!(RELEASE_OPENING_COUNT_V1 == 43);
    assert!(RELEASE_RLWE_EQUATION_COUNT_V1 == 86);
    assert!(RELEASE_Q_NATIVE_RELATION_COORDINATE_COUNT_V1 == 3_268);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalOpeningPositionV1 {
    family: ZkAmsPhase23RnsLinkFamilyV1,
    chunk_index: u16,
    family_chunk_count: u16,
}
/// Concrete, move-only sink for the sole private native-opening path.
///
/// The type is nameable by the collective sibling only so that its verifier
/// can accept a mutable borrow.  Its constructor is restricted to the Phase-23
/// parent subtree, and every advance additionally requires a borrow of the
/// opening-verifier permit.  It implements no clone, codec, callback, or
/// sibling-supplied interface.
#[must_use = "dropping this sink produces no q-native relation proof"]
pub(in super::super) struct ZkAmsPhase23QNativeRelationAdapterSinkV1 {
    release_parameter_digest: [u8; 32],
    geometry: ZkAmsPhase23RnsLinkReleaseGeometryV1,
    next_opening: u16,
    validated_rlwe_equation_count: u16,
    validated_q_native_relation_coordinate_count: u32,
    ordered_topology_hash: Keccak256,
}
/// Public-only, explicitly unverified metadata returned after all 43 native
/// opening validations reach the sealed sink in canonical order.
///
/// This is neither a polynomial commitment nor a proof/capability.  It binds
/// no plaintext, RNS-binding, ciphertext-lineage, or other witness-derived
/// digest and is accepted by no release consumer. The same release geometry
/// deliberately produces the same topology root across different statements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsPhase23QNativeRelationUnverifiedMetadataV1 {
    pub(super) release_parameter_digest: [u8; 32],
    pub(super) ordered_topology_root: [u8; 32],
    pub(super) opening_count: u16,
    pub(super) rlwe_equation_count: u16,
    pub(super) q_native_relation_coordinate_count: u32,
    pub(super) witness_polynomials_constructed: bool,
    pub(super) fiat_shamir_relation_bound: bool,
    pub(super) zero_knowledge_masked: bool,
    pub(super) deterministic_plaintext_lineage_hidden: bool,
    pub(super) secret_packed_plaintext_owner_hardened: bool,
    pub(super) digest: [u8; 32],
}
impl ZkAmsPhase23QNativeRelationAdapterSinkV1 {
    /// Construct the sealed release-shape sink inside the Phase-23 subtree.
    /// No collective sibling can call this constructor.
    pub(super) fn new(
        geometry: ZkAmsPhase23RnsLinkReleaseGeometryV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_release_geometry_v1(&geometry)?;
        let release_parameter_digest = zk_ams_phase23_rns_link_q_pcs_release_parameter_digest_v1()?;
        if release_parameter_digest == [0; 32]
            || geometry.digest == [0; 32]
            || geometry.profile_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut ordered_topology_hash = Keccak256::new();
        ordered_topology_hash.update(Q_NATIVE_RELATION_ADAPTER_DOMAIN_V1);
        ordered_topology_hash.update(&[RNS_LINK_VERSION_V1]);
        ordered_topology_hash.update(&release_parameter_digest);
        ordered_topology_hash.update(&geometry.digest);
        ordered_topology_hash.update(&geometry.profile_digest);
        ordered_topology_hash.update(&RELEASE_OPENING_COUNT_V1.to_be_bytes());
        ordered_topology_hash.update(&RELEASE_RLWE_EQUATION_COUNT_V1.to_be_bytes());
        ordered_topology_hash.update(&RELEASE_Q_NATIVE_RELATION_COORDINATE_COUNT_V1.to_be_bytes());
        Ok(Self {
            release_parameter_digest,
            geometry,
            next_opening: 0,
            validated_rlwe_equation_count: 0,
            validated_q_native_relation_coordinate_count: 0,
            ordered_topology_hash,
        })
    }
    /// Record that both native equations for the next canonical opening were validated while the
    /// owned opening was still live in the collective verifier.
    ///
    /// The collective verifier has already checked the exact public artifacts, RNS binding, and
    /// both native equations before this call. Consequently the sink accepts only the
    /// unconstructible permit. It derives the next family, chunk, and layout from geometry captured
    /// by its restricted constructor; no witness-bearing reference crosses this boundary.
    pub(in super::super) fn absorb_validated_opening_topology_v1(
        &mut self,
        _permit: &ZkAmsPhase23NativeBgvOpeningVerifierPermitV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let position = canonical_opening_position_v1(usize::from(self.next_opening))
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let family_geometry = self.geometry.family(position.family)?;
        if family_geometry.family != position.family
            || family_geometry.chunk_count != position.family_chunk_count
            || family_geometry.packing_layout_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let next_opening = self
            .next_opening
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let validated_rlwe_equation_count = self
            .validated_rlwe_equation_count
            .checked_add(RLWE_EQUATIONS_PER_OPENING_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let validated_q_native_relation_coordinate_count = self
            .validated_q_native_relation_coordinate_count
            .checked_add(
                u32::from(RLWE_EQUATIONS_PER_OPENING_V1)
                    .checked_mul(
                        u32::try_from(ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1)
                            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                    )
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut topology_hash = Keccak256::new();
        topology_hash.update(Q_NATIVE_TOPOLOGY_OPENING_DOMAIN_V1);
        topology_hash.update(&[RNS_LINK_VERSION_V1]);
        topology_hash.update(&self.release_parameter_digest);
        topology_hash.update(&self.geometry.digest);
        topology_hash.update(&self.next_opening.to_be_bytes());
        topology_hash.update(&[position.family as u8]);
        topology_hash.update(&position.chunk_index.to_be_bytes());
        topology_hash.update(&family_geometry.packing_layout_digest);
        let topology_digest = topology_hash.finalize();
        if topology_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        self.ordered_topology_hash.update(&topology_digest);
        self.next_opening = next_opening;
        self.validated_rlwe_equation_count = validated_rlwe_equation_count;
        self.validated_q_native_relation_coordinate_count =
            validated_q_native_relation_coordinate_count;
        Ok(())
    }
    /// Finish by value and emit public-only, unverified stage metadata.
    pub(super) fn finish_into_unverified_metadata_v1(
        self,
    ) -> Result<ZkAmsPhase23QNativeRelationUnverifiedMetadataV1, ZkAmsMkheErrorV1> {
        require_complete_counts_v1(
            self.next_opening,
            self.validated_rlwe_equation_count,
            self.validated_q_native_relation_coordinate_count,
        )?;
        let ordered_topology_root = self.ordered_topology_hash.finalize();
        if ordered_topology_root == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut hash = Keccak256::new();
        hash.update(Q_NATIVE_RELATION_METADATA_DOMAIN_V1);
        hash.update(&[RNS_LINK_VERSION_V1]);
        hash.update(&self.release_parameter_digest);
        hash.update(&self.geometry.digest);
        hash.update(&self.geometry.profile_digest);
        hash.update(&ordered_topology_root);
        hash.update(&self.next_opening.to_be_bytes());
        hash.update(&self.validated_rlwe_equation_count.to_be_bytes());
        hash.update(
            &self
                .validated_q_native_relation_coordinate_count
                .to_be_bytes(),
        );
        hash.update(&[
            WITNESS_POLYNOMIALS_CONSTRUCTED_V1 as u8,
            FIAT_SHAMIR_RELATION_BOUND_V1 as u8,
            ZERO_KNOWLEDGE_MASKED_V1 as u8,
            DETERMINISTIC_PLAINTEXT_LINEAGE_HIDDEN_V1 as u8,
            SECRET_PACKED_PLAINTEXT_OWNER_HARDENED_V1 as u8,
        ]);
        let digest = hash.finalize();
        if digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(ZkAmsPhase23QNativeRelationUnverifiedMetadataV1 {
            release_parameter_digest: self.release_parameter_digest,
            ordered_topology_root,
            opening_count: self.next_opening,
            rlwe_equation_count: self.validated_rlwe_equation_count,
            q_native_relation_coordinate_count: self.validated_q_native_relation_coordinate_count,
            witness_polynomials_constructed: WITNESS_POLYNOMIALS_CONSTRUCTED_V1,
            fiat_shamir_relation_bound: FIAT_SHAMIR_RELATION_BOUND_V1,
            zero_knowledge_masked: ZERO_KNOWLEDGE_MASKED_V1,
            deterministic_plaintext_lineage_hidden: DETERMINISTIC_PLAINTEXT_LINEAGE_HIDDEN_V1,
            secret_packed_plaintext_owner_hardened: SECRET_PACKED_PLAINTEXT_OWNER_HARDENED_V1,
            digest,
        })
    }
}
fn validate_release_geometry_v1(
    geometry: &ZkAmsPhase23RnsLinkReleaseGeometryV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if usize::from(geometry.rns_limb_count) != ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1
        || geometry.commitment_count != RELEASE_OPENING_COUNT_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let mut opening_count = 0_u16;
    for (family, chunk_count) in CANONICAL_FAMILY_CHUNK_COUNTS_V1 {
        let actual = geometry.family(family)?;
        if actual.family != family || actual.chunk_count != chunk_count {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        opening_count = opening_count
            .checked_add(chunk_count)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    if opening_count != RELEASE_OPENING_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}
fn canonical_opening_position_v1(ordinal: usize) -> Option<CanonicalOpeningPositionV1> {
    let mut family_start = 0_usize;
    for (family, family_chunk_count) in CANONICAL_FAMILY_CHUNK_COUNTS_V1 {
        let family_end = family_start.checked_add(usize::from(family_chunk_count))?;
        if ordinal < family_end {
            return Some(CanonicalOpeningPositionV1 {
                family,
                chunk_index: u16::try_from(ordinal.checked_sub(family_start)?).ok()?,
                family_chunk_count,
            });
        }
        family_start = family_end;
    }
    None
}
fn require_complete_counts_v1(
    opening_count: u16,
    rlwe_equation_count: u16,
    q_native_relation_coordinate_count: u32,
) -> Result<(), ZkAmsMkheErrorV1> {
    if opening_count != RELEASE_OPENING_COUNT_V1
        || rlwe_equation_count != RELEASE_RLWE_EQUATION_COUNT_V1
        || q_native_relation_coordinate_count != RELEASE_Q_NATIVE_RELATION_COORDINATE_COUNT_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_order_and_relation_coordinate_counts_are_exact() {
        let mut ordinal = 0_usize;
        for (family, family_chunk_count) in CANONICAL_FAMILY_CHUNK_COUNTS_V1 {
            for chunk_index in 0..family_chunk_count {
                assert_eq!(
                    canonical_opening_position_v1(ordinal),
                    Some(CanonicalOpeningPositionV1 {
                        family,
                        chunk_index,
                        family_chunk_count,
                    })
                );
                ordinal += 1;
            }
        }
        assert_eq!(ordinal, 43);
        assert_eq!(canonical_opening_position_v1(43), None);
        require_complete_counts_v1(43, 86, 3_268).unwrap();
        for hostile in [(42, 86, 3_268), (43, 84, 3_268), (43, 86, 3_267)] {
            assert_eq!(
                require_complete_counts_v1(hostile.0, hostile.1, hostile.2),
                Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
            );
        }
    }
    #[test]
    fn source_boundary_is_concrete_public_only_and_fail_closed() {
        let source = include_str!("phase23_rns_link_q_relation_adapter.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source prefix");
        let parent = include_str!("phase23_rns_link.rs");
        let collective = include_str!("collective.rs");
        let q_pcs = include_str!("phase23_rns_link_q_pcs.rs");
        assert!(source.lines().count() <= 450);
        assert!(source.len() <= 20_000);
        for forbidden in [
            "SecretPolynomial",
            "RnsPolynomial",
            "ZkAmsT256PackedPlaintextV1",
            "ZkAmsT256PackingLayoutV1",
            "ZkAmsMkheCollectiveCiphertextV1",
            "ZkAmsMkheCollectivePublicKeyV1",
            ".coefficients",
            "ephemeral",
            "error_zero",
            "error_one",
            "impl Fn",
            "NoritoSerialize",
            "NoritoDeserialize",
            "serde",
            "expected_rns_binding_digest",
            "ciphertext_digest",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden witness or authority surface: {forbidden}"
            );
        }
        assert!(
            production
                .contains("pub(in super::super) struct ZkAmsPhase23QNativeRelationAdapterSinkV1")
        );
        assert!(production.contains(
            "pub(super) fn new(\n        geometry: ZkAmsPhase23RnsLinkReleaseGeometryV1"
        ));
        assert!(production.contains("_permit: &ZkAmsPhase23NativeBgvOpeningVerifierPermitV1"));
        for required in [
            "const WITNESS_POLYNOMIALS_CONSTRUCTED_V1: bool = false;",
            "const FIAT_SHAMIR_RELATION_BOUND_V1: bool = false;",
            "const ZERO_KNOWLEDGE_MASKED_V1: bool = false;",
            "const DETERMINISTIC_PLAINTEXT_LINEAGE_HIDDEN_V1: bool = false;",
            "const SECRET_PACKED_PLAINTEXT_OWNER_HARDENED_V1: bool = false;",
        ] {
            assert!(
                production.contains(required),
                "missing false axis: {required}"
            );
        }
        assert!(parent.contains(
            "#[cfg(test)]\n#[path = \"phase23_rns_link_q_relation_adapter.rs\"]\nmod q_relation_adapter;"
        ));
        assert!(collective.contains("relation_sink.absorb_validated_opening_topology_v1(&permit)"));
        assert!(q_pcs.contains("fiat_shamir_relation_adapter_implemented: false"));
        assert!(q_pcs.contains("zero_knowledge_masking_implemented: false"));
        assert!(q_pcs.contains("deterministic_plaintext_lineage_hiding_implemented: false"));
        assert!(q_pcs.contains("secret_packed_plaintext_owner_hardened: false"));
        assert!(q_pcs.contains("release_qualified: false"));
    }
}
