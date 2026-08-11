//! Fail-closed boundary for the missing Phase-23 verifier source link.
//!
//! This sibling records public release topology and static residency planning
//! only. The byte counts are proposal facts, not measurements or evidence.
//! Every implementation, qualification, authorization, and release axis stays
//! false, and this module exposes no callable surface.

/// Uninhabited, move-only marker for the unavailable verifier source link.
///
/// The private uninhabited field prevents safe construction. No trait or API
/// makes the marker duplicable, inspectable, serializable, or consumable.
struct BlockedAwaitingVerifierSourceLinkV1 {
    _uninhabited: core::convert::Infallible,
}

/// Public-shape planning facts for the unavailable source link.
///
/// This private record is not runtime evidence and carries no authority.
struct ExternalSourceLinkPlanV1 {
    release_family_count: usize,
    release_record_count: usize,
    release_rns_limb_count: usize,
    native_equation_count: usize,
    native_relation_coordinate_count: usize,
    current_state_owner_bytes: u64,
    proposed_specialized_encryption_bytes: u64,
    limb_arithmetic_plus_8_kib_bytes: u64,
    encrypted_arena_bytes: u64,
    total_with_current_q_pcs_scratch_bytes: u64,
    confidential_authenticated_arena_complete: bool,
    source_owned_record_streaming_complete: bool,
    incremental_native_preflight_complete: bool,
    source_commitment_complete: bool,
    source_equality_complete: bool,
    packed_link_complete: bool,
    hyrax_link_complete: bool,
    nonce_link_complete: bool,
    external_store_rss_complete: bool,
    external_store_kat_complete: bool,
    q_pcs_handoff_complete: bool,
    receipt_complete: bool,
    release_complete: bool,
}

const EXTERNAL_SOURCE_LINK_PLAN_V1: ExternalSourceLinkPlanV1 = ExternalSourceLinkPlanV1 {
    release_family_count: 6,
    release_record_count: 43,
    release_rns_limb_count: 38,
    native_equation_count: 86,
    native_relation_coordinate_count: 3_268,
    current_state_owner_bytes: 3_686_793_216,
    proposed_specialized_encryption_bytes: 9_445_392,
    limb_arithmetic_plus_8_kib_bytes: 2_105_360,
    encrypted_arena_bytes: 3_829_526_544,
    total_with_current_q_pcs_scratch_bytes: 4_785_827_856,
    confidential_authenticated_arena_complete: false,
    source_owned_record_streaming_complete: false,
    incremental_native_preflight_complete: false,
    source_commitment_complete: false,
    source_equality_complete: false,
    packed_link_complete: false,
    hyrax_link_complete: false,
    nonce_link_complete: false,
    external_store_rss_complete: false,
    external_store_kat_complete: false,
    q_pcs_handoff_complete: false,
    receipt_complete: false,
    release_complete: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_topology_and_resource_plan_is_exact_but_not_evidence() {
        let plan = EXTERNAL_SOURCE_LINK_PLAN_V1;
        assert_eq!(plan.release_family_count, 6);
        assert_eq!(plan.release_record_count, 43);
        assert_eq!(plan.release_rns_limb_count, 38);
        assert_eq!(plan.native_equation_count, 86);
        assert_eq!(plan.native_relation_coordinate_count, 3_268);
        assert_eq!(plan.current_state_owner_bytes, 3_686_793_216);
        assert_eq!(plan.proposed_specialized_encryption_bytes, 9_445_392);
        assert_eq!(plan.limb_arithmetic_plus_8_kib_bytes, 2_105_360);
        assert_eq!(plan.encrypted_arena_bytes, 3_829_526_544);
        assert_eq!(plan.total_with_current_q_pcs_scratch_bytes, 4_785_827_856);
        assert_eq!(
            plan.total_with_current_q_pcs_scratch_bytes - plan.encrypted_arena_bytes,
            956_301_312
        );
        assert!(!plan.confidential_authenticated_arena_complete);
        assert!(!plan.source_owned_record_streaming_complete);
        assert!(!plan.incremental_native_preflight_complete);
        assert!(!plan.source_commitment_complete);
        assert!(!plan.source_equality_complete);
        assert!(!plan.packed_link_complete);
        assert!(!plan.hyrax_link_complete);
        assert!(!plan.nonce_link_complete);
        assert!(!plan.external_store_rss_complete);
        assert!(!plan.external_store_kat_complete);
        assert!(!plan.q_pcs_handoff_complete);
        assert!(!plan.receipt_complete);
        assert!(!plan.release_complete);
    }

    #[test]
    fn source_boundary_is_private_unconstructible_move_only_and_unwired() {
        let source = include_str!("phase23_rns_link_external_source.rs");
        let parent = include_str!("phase23_rns_link.rs");
        let audit = include_str!("receipt_capability_audit.rs");
        let manifest = include_str!("manifest.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source prefix");

        assert!(source.lines().count() <= 180);
        assert!(source.len() <= 8_000);
        assert_eq!(
            production
                .matches("BlockedAwaitingVerifierSourceLinkV1")
                .count(),
            1
        );
        assert!(production.contains("_uninhabited: core::convert::Infallible"));
        assert!(!production.contains("pub(super)"));
        assert!(!production.contains("pub(crate)"));
        assert!(!production.contains("pub struct"));
        assert!(!production.contains("pub const"));
        assert!(!production.contains("fn "));
        assert!(!production.contains("impl "));
        for forbidden in [
            "Clone",
            "Copy",
            "Debug",
            "Default",
            "Norito",
            "Encode",
            "Decode",
            "serde",
            "plaintext",
            "opening",
            "key",
            "cipher",
            "path",
            "backend",
            "digest",
            "relation_sink",
            "permit",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden source-link surface: {forbidden}"
            );
        }

        assert!(
            parent.contains(
                "#[path = \"phase23_rns_link_external_source.rs\"]\nmod external_source;"
            )
        );
        assert!(!parent.contains(concat!("pub use ", "external_source")));
        assert!(!parent.contains("BlockedAwaitingVerifierSourceLinkV1"));
        assert!(!audit.contains("phase23_rns_link_external_source"));
        assert!(!audit.contains("BlockedAwaitingVerifierSourceLinkV1"));
        assert!(!manifest.contains("phase23_rns_link_external_source"));
        assert!(!manifest.contains("BlockedAwaitingVerifierSourceLinkV1"));
    }
}
