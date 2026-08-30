//! Fail-closed audit of the ZK-AMS verified-receipt capability graph.
//!
//! This module deliberately distinguishes implemented proof components from
//! operational authorization. A sealed native CPK relation receipt is useful,
//! but the retired structural RNS-Link prototypes never minted an authorizing
//! receipt. The audit therefore keeps a separate bit for each handoff and
//! refuses each operational requirement whose corresponding handoff remains
//! open. Full release remains unavailable until every handoff closes.
//!
//! The release manifest and readiness gate consume the complete audit digest, blocker mask, and
//! aggregate availability. This makes later implementation flips reviewable without allowing an
//! unrelated readiness bit to bypass an open receipt handoff.

use super::{MKHE_VERSION_V1, ZkAmsMkheErrorV1};
use crate::vega::sponge::Keccak256;
const RECEIPT_CAPABILITY_AUDIT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.verified-receipt-capability-audit";
const BLOCKER_CPK_AGGREGATE_V1: u16 = 1 << 0;
const BLOCKER_RKG_ROUND_ONE_V1: u16 = 1 << 1;
const BLOCKER_RKG_ROUND_TWO_V1: u16 = 1 << 2;
const BLOCKER_GALOIS_KEY_V1: u16 = 1 << 3;
const BLOCKER_RNS_LINK_VERIFICATION_V1: u16 = 1 << 4;
const BLOCKER_TERMINAL_MATERIALIZATION_V1: u16 = 1 << 5;
const BLOCKER_SPLIT_DECRYPTION_V1: u16 = 1 << 6;
const BLOCKER_PERSISTENT_DECRYPTION_EQUALITY_V1: u16 = 1 << 7;
const BLOCKER_SPLIT_DECRYPTION_SOURCE_EQUALITY_V1: u16 = 1 << 8;
// These bits describe implementation prerequisites, not operational
// consumers.  Keeping the inventories separate prevents a completed local
// receipt handoff from being mistaken for a completed proof relation.
const PREREQUISITE_VERIFIER_DERIVED_RNS_TRANSPORT_V1: u8 = 1 << 0;
const PREREQUISITE_PRODUCTION_NATIVE_BGV_OPENING_RECEIPT_V1: u8 = 1 << 1;
const PREREQUISITE_CORRECTED_RNS_GEOMETRY_END_TO_END_V1: u8 = 1 << 2;
const PREREQUISITE_RNS_CARRY_QUOTIENT_SOURCE_RELATION_V1: u8 = 1 << 3;
const PREREQUISITE_HYRAX_SOURCE_MATERIALIZATION_EQUALITY_V1: u8 = 1 << 4;
const PREREQUISITE_PERSISTENT_DECRYPTION_SIS_CERTIFICATE_V1: u8 = 1 << 5;
const PREREQUISITE_SPLIT_SOURCE_CIPHERTEXT_EQUATIONS_V1: u8 = 1 << 6;
const ALL_IMPLEMENTATION_PREREQUISITES_V1: u8 = PREREQUISITE_VERIFIER_DERIVED_RNS_TRANSPORT_V1
    | PREREQUISITE_PRODUCTION_NATIVE_BGV_OPENING_RECEIPT_V1
    | PREREQUISITE_CORRECTED_RNS_GEOMETRY_END_TO_END_V1
    | PREREQUISITE_RNS_CARRY_QUOTIENT_SOURCE_RELATION_V1
    | PREREQUISITE_HYRAX_SOURCE_MATERIALIZATION_EQUALITY_V1
    | PREREQUISITE_PERSISTENT_DECRYPTION_SIS_CERTIFICATE_V1
    | PREREQUISITE_SPLIT_SOURCE_CIPHERTEXT_EQUATIONS_V1;
const RNS_TRANSPORT_AND_OPENING_PREREQUISITES_V1: u8 =
    PREREQUISITE_VERIFIER_DERIVED_RNS_TRANSPORT_V1
        | PREREQUISITE_PRODUCTION_NATIVE_BGV_OPENING_RECEIPT_V1;
// The production streaming encryption path now computes, publishes, reads
// back, and transcript-binds all 76 native BGV equations before minting a
// move-only receipt. The replacement composite verifier also accepts only a
// bounded, canonical, move-only transport whose context and exact 43-pair
// commitment root it derives and authenticates. The other five prerequisites
// remain independently open.
const CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1: u8 = ALL_IMPLEMENTATION_PREREQUISITES_V1
    & !(PREREQUISITE_VERIFIER_DERIVED_RNS_TRANSPORT_V1
        | PREREQUISITE_PRODUCTION_NATIVE_BGV_OPENING_RECEIPT_V1);
const ALL_RECEIPT_CAPABILITY_BLOCKERS_V1: u16 = BLOCKER_CPK_AGGREGATE_V1
    | BLOCKER_RKG_ROUND_ONE_V1
    | BLOCKER_RKG_ROUND_TWO_V1
    | BLOCKER_GALOIS_KEY_V1
    | BLOCKER_RNS_LINK_VERIFICATION_V1
    | BLOCKER_TERMINAL_MATERIALIZATION_V1
    | BLOCKER_SPLIT_DECRYPTION_V1
    | BLOCKER_PERSISTENT_DECRYPTION_EQUALITY_V1
    | BLOCKER_SPLIT_DECRYPTION_SOURCE_EQUALITY_V1;
const CURRENT_OPEN_RECEIPT_CAPABILITY_BLOCKERS_V1: u16 = ALL_RECEIPT_CAPABILITY_BLOCKERS_V1
    & !(BLOCKER_CPK_AGGREGATE_V1
        | BLOCKER_RKG_ROUND_ONE_V1
        | BLOCKER_RKG_ROUND_TWO_V1
        | BLOCKER_GALOIS_KEY_V1);
const _: () = assert!(ALL_RECEIPT_CAPABILITY_BLOCKERS_V1 == 0x1ff);
const _: () = assert!(CURRENT_OPEN_RECEIPT_CAPABILITY_BLOCKERS_V1 == 0x1f0);
const _: () = assert!(ALL_IMPLEMENTATION_PREREQUISITES_V1 == 0x7f);
const _: () = assert!(CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1 == 0x7c);
const _: () = assert!(RNS_TRANSPORT_AND_OPENING_PREREQUISITES_V1 == 0x03);
const _: () = assert!(
    CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1 & RNS_TRANSPORT_AND_OPENING_PREREQUISITES_V1
        == 0
);
/// One release operation which must be authorized by opaque verified receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ZkAmsMkheReceiptCapabilityConsumerV1 {
    /// Aggregate and admit the exact proof-carrying collective public key.
    CollectivePublicKeyAggregate,
    /// Generate and admit RKG round-one contributions.
    RkgRoundOne,
    /// Generate and admit RKG round-two contributions and normalization.
    RkgRoundTwo,
    /// Generate and admit one key in the frozen Galois schedule.
    GaloisKey,
    /// Begin the one-use state-owned native-BGV opening stream.
    BeginStateOwnedNativeBgvOpeningsV1,
    /// Absorb the next canonically ordered opening into that stream.
    AbsorbStateOwnedNativeBgvOpeningV1,
    /// Finish the complete stream and mint its aggregate verification result.
    FinishStateOwnedNativeBgvOpeningsV1,
    /// Turn a bound RNS-Link proof into an algebraically verified receipt.
    RnsLinkVerification,
    /// Materialize the six terminal accumulator families.
    TerminalMaterialization,
    /// Prove and publish one authenticated split-decryption share.
    SplitDecryption,
    /// Materialize the exact corrected-profile confidential source.
    SplitDecryptionSourceMaterialization,
}
/// Digest-bound status of every mandatory verified-receipt handoff.
///
/// Fields ending in `_specified` or `_bound` describe implemented structure,
/// not semantic proof success.  Fields ending in `_enforced` are true only
/// when every production path for that consumer requires the opaque receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkheReceiptCapabilityAuditV1 {
    /// Audit schema version.
    pub(super) version: u8,
    /// The native CPK verifier mints a sealed, non-serializable relation receipt.
    pub(super) cpk_relation_receipt_sealed: bool,
    /// Party-state admission consumes the sealed CPK relation receipt.
    pub(super) cpk_party_state_admission_consumes_receipt: bool,
    /// Every collective-key aggregate path requires the verified CPK receipt set.
    pub(super) cpk_aggregate_receipt_enforced: bool,
    /// Every RKG round-one generator requires a single-use CPK-derived capability.
    pub(super) rkg_round_one_receipt_enforced: bool,
    /// Every RKG round-two/normalization generator requires the chained capability.
    pub(super) rkg_round_two_receipt_enforced: bool,
    /// Every Galois-key generator requires the CPK-derived lineage capability.
    pub(super) galois_key_receipt_enforced: bool,
    /// Canonical RNS-Link bytes bind verifier-derived context and commitments.
    pub(super) rns_link_transport_bound: bool,
    /// Whether production streaming encryption mints a sealed receipt after
    /// checking canonical packing, all 38 release limbs, and both RLWE
    /// equations, and every manifest projection consumes that receipt.
    /// Explicitly unverified topology metadata does not satisfy this.
    pub(super) native_bgv_opening_receipt_sealed: bool,
    /// A terminal-local receipt checks the complete relaxed assignment and
    /// recomputes both Hyrax commitments from the materialized openings.
    pub(super) native_materialized_hyrax_receipt_sealed: bool,
    /// The terminal prover consumes that native materialized-opening receipt.
    pub(super) terminal_prover_consumes_native_materialized_receipt: bool,
    /// Whether native release geometry (`X=89`, replicated `U=1_048_576`) is
    /// enforced end to end from state-owned openings through proof checking.
    pub(super) rns_link_family_geometry_matches_native: bool,
    /// Whether the wire carries enough authenticated data to verify all 38
    /// radix/CRT-carry and negacyclic-quotient response equations.
    pub(super) rns_link_carry_quotient_responses_verifiable: bool,
    /// Whether the wire proves that its packed BGV openings equal the existing
    /// 512-row/1024-row Hyrax accumulator commitments.
    pub(super) hyrax_bgv_equality_responses_verifiable: bool,
    /// The full packing/carry/quotient/equality verifier mints an opaque receipt.
    pub(super) rns_link_algebraic_receipt_complete: bool,
    /// Terminal materialization consumes that opaque RNS-Link receipt.
    pub(super) terminal_materialization_receipt_enforced: bool,
    /// Split-decryption proving consumes both CPK and RNS-Link capabilities.
    pub(super) split_decryption_receipts_enforced: bool,
    /// The approximately 1,855-bit smudge relation proves equality to the CPK secret.
    pub(super) persistent_decryption_equality_complete: bool,
    /// The opaque context-issued party use binds roster, epoch, ciphertext,
    /// statement, record, sample, and commitment transcript replay axes.
    pub(super) persistent_decryption_replay_axes_specified: bool,
    /// The corrected 40-limb source rejects legacy-profile decryption results.
    pub(super) split_decryption_source_profile_separated: bool,
    /// Every recovered plaintext, `r/e0/e1` witness, and nonce passes its
    /// local source-domain checks before the record receipt is minted.
    pub(super) split_decryption_source_domain_checks_enforced: bool,
    /// The move-only writer seals only after the exact ordered 43-record set.
    pub(super) split_decryption_source_exact_43_receipt_sealed: bool,
    /// Whether a proof binds each materialized source record back to its
    /// authenticated ciphertext equations rather than only its digest.
    pub(super) split_decryption_source_ciphertext_equality_complete: bool,
    /// Exact open-handoff bit set.
    pub(super) blocker_mask: u16,
    /// True only when every structural prerequisite and handoff closes together.
    pub(super) release_available: bool,
    /// Digest of every preceding field.
    pub(super) digest: [u8; 32],
}
impl ZkAmsMkheReceiptCapabilityAuditV1 {
    /// Recheck the blocker set, aggregate state, audit digest, and the exact
    /// source-level implementation-prerequisite inventory.
    pub(super) fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_logical_graph_v1()?;
        self.validate_current_implementation_v1()
    }

    fn validate_logical_graph_v1(self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.version != MKHE_VERSION_V1
            || self.blocker_mask != receipt_capability_blocker_mask_v1(self)
            || self.release_available != receipt_capability_release_available_v1(self)
            || self.digest == [0; 32]
            || self.digest != receipt_capability_audit_digest_v1(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn validate_current_implementation_v1(self) -> Result<(), ZkAmsMkheErrorV1> {
        if implementation_prerequisite_blocker_mask_v1(self)
            != CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    /// Return whether one exact consumer is currently authorized.
    pub(super) const fn authorizes(self, consumer: ZkAmsMkheReceiptCapabilityConsumerV1) -> bool {
        match consumer {
            ZkAmsMkheReceiptCapabilityConsumerV1::CollectivePublicKeyAggregate => {
                self.cpk_relation_receipt_sealed
                    && self.cpk_party_state_admission_consumes_receipt
                    && self.cpk_aggregate_receipt_enforced
            }
            ZkAmsMkheReceiptCapabilityConsumerV1::RkgRoundOne => {
                self.cpk_aggregate_receipt_enforced && self.rkg_round_one_receipt_enforced
            }
            ZkAmsMkheReceiptCapabilityConsumerV1::RkgRoundTwo => {
                self.rkg_round_one_receipt_enforced && self.rkg_round_two_receipt_enforced
            }
            ZkAmsMkheReceiptCapabilityConsumerV1::GaloisKey => {
                self.cpk_aggregate_receipt_enforced && self.galois_key_receipt_enforced
            }
            ZkAmsMkheReceiptCapabilityConsumerV1::BeginStateOwnedNativeBgvOpeningsV1 => {
                self.rns_link_transport_bound && self.rns_link_family_geometry_matches_native
            }
            ZkAmsMkheReceiptCapabilityConsumerV1::AbsorbStateOwnedNativeBgvOpeningV1 => {
                self.authorizes(
                    ZkAmsMkheReceiptCapabilityConsumerV1::BeginStateOwnedNativeBgvOpeningsV1,
                ) && self.native_bgv_opening_receipt_sealed
            }
            ZkAmsMkheReceiptCapabilityConsumerV1::FinishStateOwnedNativeBgvOpeningsV1 => {
                self.authorizes(
                    ZkAmsMkheReceiptCapabilityConsumerV1::AbsorbStateOwnedNativeBgvOpeningV1,
                ) && self.rns_link_carry_quotient_responses_verifiable
                    && self.hyrax_bgv_equality_responses_verifiable
                    && self.rns_link_algebraic_receipt_complete
            }
            ZkAmsMkheReceiptCapabilityConsumerV1::RnsLinkVerification => self.authorizes(
                ZkAmsMkheReceiptCapabilityConsumerV1::FinishStateOwnedNativeBgvOpeningsV1,
            ),
            ZkAmsMkheReceiptCapabilityConsumerV1::TerminalMaterialization => {
                self.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::RnsLinkVerification)
                    && self.native_materialized_hyrax_receipt_sealed
                    && self.terminal_prover_consumes_native_materialized_receipt
                    && self.terminal_materialization_receipt_enforced
            }
            ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryption => {
                self.cpk_aggregate_receipt_enforced
                    && self.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::RnsLinkVerification)
                    && self.split_decryption_receipts_enforced
                    && self.persistent_decryption_equality_complete
                    && self.persistent_decryption_replay_axes_specified
            }
            ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryptionSourceMaterialization => {
                self.split_decryption_source_profile_separated
                    && self.split_decryption_source_domain_checks_enforced
                    && self.split_decryption_source_exact_43_receipt_sealed
                    && self.split_decryption_source_ciphertext_equality_complete
            }
        }
    }
}
/// Evaluate the current source-level capability graph without inferring proof
/// success from a type name, a nonzero digest, or canonical transport bytes.
pub(super) fn zk_ams_mkhe_receipt_capability_audit_v1() -> ZkAmsMkheReceiptCapabilityAuditV1 {
    let mut audit = ZkAmsMkheReceiptCapabilityAuditV1 {
        version: MKHE_VERSION_V1,
        cpk_relation_receipt_sealed: true,
        cpk_party_state_admission_consumes_receipt: true,
        // The public ceremony's sole per-party transition invokes the complete
        // relation verifier, converts its sealed receipt into a move-only
        // contribution, and consumes that contribution into the party-state
        // binding plus staged admission. Finalization requires fixed arrays of
        // all eight admissions and validates each one again; the legacy raw
        // aggregate is test-only.
        cpk_aggregate_receipt_enforced: true,
        // The sole evaluated-key provider admission now consumes a sealed,
        // move-only aggregate of every exact ordered source and CKS receipt
        // before provider I/O, then joins every expected CKS compact output to
        // the authenticated ZARK scan. This closes the three receipt-consumer
        // handoffs only. It does not establish the stronger cross-set algebraic
        // equality between accumulated source outputs and the CKS relation.
        rkg_round_one_receipt_enforced: true,
        rkg_round_two_receipt_enforced: true,
        galois_key_receipt_enforced: true,
        // The replacement verifier's sole public proof entry points consume a
        // sealed, move-only transport minted only after bounded exact decoding.
        // That mint reconstructs the canonical source/transcript context,
        // authenticates every typed section and all 43 ordered source/Hyrax
        // commitment pairs, and binds the derived context/root/wire identities
        // transitively into the algebraic receipt and both live consumers. The
        // retired 38-limb structural path remains test-only.
        rns_link_transport_bound: true,
        // The sole production streaming-encryption core computes both exact
        // native BGV equations for all 38 release limbs, authenticates the
        // immutable source and published-output readbacks, and seals their
        // canonical 76-equation transcript into a move-only manifest-bound
        // receipt. The public and RNS-native-tail projections revalidate and
        // consume it; Phase-23 revalidates and retains every receipt.
        native_bgv_opening_receipt_sealed: true,
        native_materialized_hyrax_receipt_sealed: true,
        terminal_prover_consumes_native_materialized_receipt: true,
        // No retained RNS-Link schema proves that release geometry and all
        // native openings equal the materialized accumulator commitments.
        rns_link_family_geometry_matches_native: false,
        // A point, one evaluation scalar, and one nonzero response per section
        // do not encode opening/IPA equations for either relation.
        rns_link_carry_quotient_responses_verifiable: false,
        hyrax_bgv_equality_responses_verifiable: false,
        // The atomic composite verifier's terminal typestate now retains a
        // private verification seal and can mint one move-only, non-codec
        // algebraic receipt only by re-running all four stages over consumed
        // proof/context owners. This closes receipt minting, not proof
        // availability: native geometry, carry/quotient, and Hyrax/BGV
        // equality remain independent false axes below.
        rns_link_algebraic_receipt_complete: true,
        // Both live consumers now require move-only capabilities minted only
        // by consuming the opaque algebraic receipt. These local handoff axes
        // are therefore closed, but neither consumer authorizes release by
        // itself: terminal materialization still depends on the complete RNS
        // verification chain, and split decryption additionally depends on
        // the persistent secret/share equality proof below.
        terminal_materialization_receipt_enforced: true,
        split_decryption_receipts_enforced: true,
        // One atomic ceremony call consumes all eight complete CPK verifier
        // capabilities and emits both the secret-free ordered context and the
        // move-only bindings admitted into party states. The retained context
        // binds fresh use sets to later statements. Prove/verify/split/
        // reconstruct bind its actual points to the shared-secret transcript.
        // Keep this false until the transitive short-solution/SIS equality
        // argument and replacement release-size KAT are certified.
        persistent_decryption_equality_complete: false,
        persistent_decryption_replay_axes_specified: true,
        // The corrected-profile move-only source writer now checks canonical
        // T256 coefficients, ternary/nonzero r, bounded e0/e1, a nonzero
        // nonce, and the exact ordered 43-record chronology before minting its
        // digest-bound receipt. The exact receipt set and final source seal can
        // now mint one move-only input bound to the complete RNS-native public
        // transcript; the input hard-codes all equality and release flags to
        // false because no ciphertext-equation proof exists. This closes only
        // the verifier-input handoff, not the bit-8 equality requirement.
        split_decryption_source_profile_separated: true,
        split_decryption_source_domain_checks_enforced: true,
        split_decryption_source_exact_43_receipt_sealed: true,
        split_decryption_source_ciphertext_equality_complete: false,
        blocker_mask: 0,
        release_available: false,
        digest: [0; 32],
    };
    audit.blocker_mask = receipt_capability_blocker_mask_v1(audit);
    audit.release_available = receipt_capability_release_available_v1(audit);
    audit.digest = receipt_capability_audit_digest_v1(audit);
    audit
}
/// Require one operational capability without accepting a digest shell or
/// retired structural RNS-Link metadata as a substitute.
///
/// The collective-public-key aggregate and the three evaluated-key receipt consumers are authorized
/// through their sealed handoffs. The independent algebraic/materialization/decryption blockers
/// remain open, so these local successes do not make the complete release available.
pub(super) fn require_zk_ams_mkhe_receipt_capability_v1(
    consumer: ZkAmsMkheReceiptCapabilityConsumerV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let audit = zk_ams_mkhe_receipt_capability_audit_v1();
    audit.validate()?;
    if !audit.authorizes(consumer) {
        return Err(ZkAmsMkheErrorV1::ReleaseUnavailable);
    }
    Ok(())
}
fn receipt_capability_blocker_mask_v1(audit: ZkAmsMkheReceiptCapabilityAuditV1) -> u16 {
    let mut blockers = 0_u16;
    for (closed, blocker) in [
        (
            audit.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::CollectivePublicKeyAggregate),
            BLOCKER_CPK_AGGREGATE_V1,
        ),
        (
            audit.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::RkgRoundOne),
            BLOCKER_RKG_ROUND_ONE_V1,
        ),
        (
            audit.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::RkgRoundTwo),
            BLOCKER_RKG_ROUND_TWO_V1,
        ),
        (
            audit.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::GaloisKey),
            BLOCKER_GALOIS_KEY_V1,
        ),
        (
            audit.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::RnsLinkVerification),
            BLOCKER_RNS_LINK_VERIFICATION_V1,
        ),
        (
            audit.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::TerminalMaterialization),
            BLOCKER_TERMINAL_MATERIALIZATION_V1,
        ),
        (
            audit.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryption),
            BLOCKER_SPLIT_DECRYPTION_V1,
        ),
        (
            audit.persistent_decryption_equality_complete,
            BLOCKER_PERSISTENT_DECRYPTION_EQUALITY_V1,
        ),
        (
            audit.split_decryption_source_profile_separated
                && audit.split_decryption_source_domain_checks_enforced
                && audit.split_decryption_source_exact_43_receipt_sealed
                && audit.split_decryption_source_ciphertext_equality_complete,
            BLOCKER_SPLIT_DECRYPTION_SOURCE_EQUALITY_V1,
        ),
    ] {
        if !closed {
            blockers |= blocker;
        }
    }
    blockers
}

/// Exact source-level prerequisites that are still absent from production.
///
/// The production native-BGV opening receipt and verifier-derived replacement
/// transport are closed independently. The legacy state-owned opening stream
/// remains test-only. The replacement verifier derives and seals the exact
/// canonical wire, source/transcript context, and ordered 43-pair commitment
/// root before either public proof entry point can run; both receipt consumers
/// retain that transport identity. Its four production adapters retain
/// explicit unavailable exits for source/Hyrax mapping, the RLWE/source
/// relation, corrected cross-field/global lookup, and governed padding
/// ownership. The remaining two bits name the independently uncertified
/// persistent-decryption SIS argument and absent split-source ciphertext
/// equations.
fn implementation_prerequisite_blocker_mask_v1(audit: ZkAmsMkheReceiptCapabilityAuditV1) -> u8 {
    let mut blockers = 0_u8;
    for (closed, blocker) in [
        (
            audit.rns_link_transport_bound,
            PREREQUISITE_VERIFIER_DERIVED_RNS_TRANSPORT_V1,
        ),
        (
            audit.native_bgv_opening_receipt_sealed,
            PREREQUISITE_PRODUCTION_NATIVE_BGV_OPENING_RECEIPT_V1,
        ),
        (
            audit.rns_link_family_geometry_matches_native,
            PREREQUISITE_CORRECTED_RNS_GEOMETRY_END_TO_END_V1,
        ),
        (
            audit.rns_link_carry_quotient_responses_verifiable,
            PREREQUISITE_RNS_CARRY_QUOTIENT_SOURCE_RELATION_V1,
        ),
        (
            audit.hyrax_bgv_equality_responses_verifiable,
            PREREQUISITE_HYRAX_SOURCE_MATERIALIZATION_EQUALITY_V1,
        ),
        (
            audit.persistent_decryption_equality_complete,
            PREREQUISITE_PERSISTENT_DECRYPTION_SIS_CERTIFICATE_V1,
        ),
        (
            audit.split_decryption_source_ciphertext_equality_complete,
            PREREQUISITE_SPLIT_SOURCE_CIPHERTEXT_EQUATIONS_V1,
        ),
    ] {
        if !closed {
            blockers |= blocker;
        }
    }
    blockers
}
const fn receipt_capability_release_available_v1(audit: ZkAmsMkheReceiptCapabilityAuditV1) -> bool {
    audit.cpk_relation_receipt_sealed
        && audit.cpk_party_state_admission_consumes_receipt
        && audit.cpk_aggregate_receipt_enforced
        && audit.rkg_round_one_receipt_enforced
        && audit.rkg_round_two_receipt_enforced
        && audit.galois_key_receipt_enforced
        && audit.rns_link_transport_bound
        && audit.native_bgv_opening_receipt_sealed
        && audit.native_materialized_hyrax_receipt_sealed
        && audit.terminal_prover_consumes_native_materialized_receipt
        && audit.rns_link_family_geometry_matches_native
        && audit.rns_link_carry_quotient_responses_verifiable
        && audit.hyrax_bgv_equality_responses_verifiable
        && audit.rns_link_algebraic_receipt_complete
        && audit.terminal_materialization_receipt_enforced
        && audit.split_decryption_receipts_enforced
        && audit.persistent_decryption_equality_complete
        && audit.persistent_decryption_replay_axes_specified
        && audit.split_decryption_source_profile_separated
        && audit.split_decryption_source_domain_checks_enforced
        && audit.split_decryption_source_exact_43_receipt_sealed
        && audit.split_decryption_source_ciphertext_equality_complete
        && audit.blocker_mask == 0
}
fn receipt_capability_audit_digest_v1(audit: ZkAmsMkheReceiptCapabilityAuditV1) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(RECEIPT_CAPABILITY_AUDIT_DOMAIN_V1);
    hash.update(&[audit.version]);
    hash.update(&[
        audit.cpk_relation_receipt_sealed.into(),
        audit.cpk_party_state_admission_consumes_receipt.into(),
        audit.cpk_aggregate_receipt_enforced.into(),
        audit.rkg_round_one_receipt_enforced.into(),
        audit.rkg_round_two_receipt_enforced.into(),
        audit.galois_key_receipt_enforced.into(),
        audit.rns_link_transport_bound.into(),
        audit.native_bgv_opening_receipt_sealed.into(),
        audit.native_materialized_hyrax_receipt_sealed.into(),
        audit
            .terminal_prover_consumes_native_materialized_receipt
            .into(),
        audit.rns_link_family_geometry_matches_native.into(),
        audit.rns_link_carry_quotient_responses_verifiable.into(),
        audit.hyrax_bgv_equality_responses_verifiable.into(),
        audit.rns_link_algebraic_receipt_complete.into(),
        audit.terminal_materialization_receipt_enforced.into(),
        audit.split_decryption_receipts_enforced.into(),
        audit.persistent_decryption_equality_complete.into(),
        audit.persistent_decryption_replay_axes_specified.into(),
        audit.split_decryption_source_profile_separated.into(),
        audit.split_decryption_source_domain_checks_enforced.into(),
        audit.split_decryption_source_exact_43_receipt_sealed.into(),
        audit
            .split_decryption_source_ciphertext_equality_complete
            .into(),
    ]);
    hash.update(&audit.blocker_mask.to_be_bytes());
    hash.update(&[audit.release_available.into()]);
    hash.finalize()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::zk_ams::mkhe::{
        active::ZkAmsMkheGovernedActiveRosterV1,
        active_exact_binding::{
            VerifiedDirectRelationProofReceiptV1, VerifiedPersistentWitnessBindingV1,
            VerifiedPersistentWitnessDirectRelationUseV1,
            mint_collective_secret_binding_from_verified_cpk_v1,
            verify_and_consume_direct_relation_use_v1,
        },
        cpk_relation::{VerifiedZkAmsMkheCpkBindingSourceV1, VerifiedZkAmsMkheCpkContributionV1},
        decryption::{
            ZkAmsMkheAuthenticatedDecryptionShareV1, ZkAmsMkheDecryptionSplitTransportV1,
            ZkAmsMkheDecryptionStatementV1, split_zk_ams_mkhe_decryption_share_v1,
        },
        manifest::ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
        packing::ZkAmsT256PackedPlaintextV1,
        persistent_decryption_equality::{
            ZkAmsMkhePersistentDecryptionPartyUseV1,
            ZkAmsMkhePersistentDecryptionVerificationContextV1,
            prepare_zk_ams_mkhe_persistent_decryption_from_verified_cpk_v1,
        },
        phase23_encrypted::{
            ZkAmsPhase23AccumulatorShapeV1, ZkAmsPhase23MaterializedAccumulatorsV1,
            zk_ams_phase23_materialize_release_accumulator_chunks_v1,
        },
    };
    type MintCpkBindingV1 = fn(
        &ZkAmsMkheGovernedActiveRosterV1,
        [u8; 32],
        usize,
        [u8; 32],
        VerifiedZkAmsMkheCpkBindingSourceV1,
    ) -> Result<VerifiedPersistentWitnessBindingV1, ZkAmsMkheErrorV1>;
    type PrepareSecretFreePersistentDecryptionV1 = for<'a, 'b> fn(
        &'a ZkAmsMkheGovernedActiveRosterV1,
        ZkAmsMkheDecryptionStatementV1<'b>,
        [VerifiedZkAmsMkheCpkContributionV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<
        (
            ZkAmsMkhePersistentDecryptionVerificationContextV1,
            [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
            [VerifiedPersistentWitnessBindingV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        ),
        ZkAmsMkheErrorV1,
    >;
    type VerifyDirectRelationV1 =
        fn(
            VerifiedPersistentWitnessDirectRelationUseV1,
            &[u8],
        ) -> Result<VerifiedDirectRelationProofReceiptV1, ZkAmsMkheErrorV1>;
    type BindPersistentDecryptionStatementV1 = for<'a, 'b> fn(
        &'a ZkAmsMkhePersistentDecryptionVerificationContextV1,
        ZkAmsMkheDecryptionStatementV1<'b>,
    ) -> Result<
        [ZkAmsMkhePersistentDecryptionPartyUseV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
        ZkAmsMkheErrorV1,
    >;
    type MaterializeWithoutReceiptV1 =
        fn(
            [u8; 32],
            [u8; 32],
            [u8; 32],
            [u8; 32],
            [u8; 32],
            u8,
            ZkAmsPhase23AccumulatorShapeV1,
            std::vec::IntoIter<Result<ZkAmsT256PackedPlaintextV1, ZkAmsMkheErrorV1>>,
        ) -> Result<ZkAmsPhase23MaterializedAccumulatorsV1, ZkAmsMkheErrorV1>;
    type SplitWithPersistentReceiptV1 =
        for<'a, 'b, 'c> fn(
            ZkAmsMkheDecryptionStatementV1<'a>,
            &'b ZkAmsMkhePersistentDecryptionVerificationContextV1,
            &'c ZkAmsMkheAuthenticatedDecryptionShareV1,
        )
            -> Result<ZkAmsMkheDecryptionSplitTransportV1, ZkAmsMkheErrorV1>;
    fn assert_source_markers_in_order_v1(mut source: &str, markers: &[&str]) {
        for marker in markers {
            source = source
                .split_once(marker)
                .unwrap_or_else(|| panic!("missing ordered source marker: {marker}"))
                .1;
        }
    }
    fn recompute_audit_shell_v1(
        mut audit: ZkAmsMkheReceiptCapabilityAuditV1,
    ) -> ZkAmsMkheReceiptCapabilityAuditV1 {
        audit.blocker_mask = receipt_capability_blocker_mask_v1(audit);
        audit.release_available = receipt_capability_release_available_v1(audit);
        audit.digest = receipt_capability_audit_digest_v1(audit);
        audit
    }
    #[test]
    fn source_surface_guards_distinguish_receipts_from_bypasses() {
        let production = include_str!("receipt_capability_audit.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source prefix");
        assert!(production.contains("native_bgv_opening_receipt_sealed: true"));
        assert!(!production.contains("native_bgv_opening_receipt_sealed: false"));
        assert!(production.contains("rns_link_transport_bound: true"));
        assert!(!production.contains("rns_link_transport_bound: false"));
        let native_source = include_str!("collective/incremental_source.rs");
        assert!(native_source.contains(
            "const _: () = assert!(STREAMING_NATIVE_BGV_OPENING_EQUATION_COUNT_V1 == 76);"
        ));
        assert!(native_source.contains("struct StreamingNativeBgvOpeningReceiptSealV1;"));
        assert!(native_source.contains("VerifiedStreamingNativeBgvOpeningReceiptV1"));
        assert!(!native_source.contains("from_raw_opening_receipt"));
        let native_product = native_source
            .split("impl VerifiedStreamingCollectiveEncryptionProductV1")
            .nth(1)
            .expect("native-BGV verified product")
            .split("struct PurposeForkedCollectiveKeyAdmissionAxesV1")
            .next()
            .expect("native-BGV verified product boundary");
        assert_source_markers_in_order_v1(
            native_product,
            &[
                "fn into_verified_manifest_v1(",
                "native_bgv_opening_receipt.validate_for_manifest_v1(&manifest)?;",
                "#[cfg(test)]",
                "fn into_verified_manifest_with_profile_v1(",
            ],
        );
        assert!(!native_product.contains("fn into_manifest_v1("));
        assert!(
            !native_source.contains(
                "impl core::ops::Deref for VerifiedStreamingCollectiveEncryptionProductV1"
            )
        );
        let public_native_path = native_source
            .split("pub fn encrypt_zk_ams_mkhe_collective_packed_streaming_v1")
            .nth(1)
            .expect("public native-BGV streaming path")
            .split("#[cfg(test)]")
            .next()
            .expect("public native-BGV streaming boundary");
        assert!(public_native_path.contains(
            ".and_then(VerifiedStreamingCollectiveEncryptionProductV1::into_verified_manifest_v1)"
        ));
        let native_tail =
            include_str!("collective/incremental_source_rns_native_tail_publication_v2.rs");
        assert!(native_tail.contains(".and_then(|product| product.into_verified_manifest_v1())"));
        let phase23_native = include_str!("collective/incremental_source_phase23.rs");
        assert_source_markers_in_order_v1(
            phase23_native,
            &[
                "match product.into_verified_parts_v1()",
                ".push(native_bgv_opening_receipt)",
            ],
        );
        assert_source_markers_in_order_v1(
            native_product,
            &[
                "fn into_verified_parts_v1(",
                "native_bgv_opening_receipt.validate_for_manifest_v1(&manifest)?;",
                "Ok((manifest, native_bgv_opening_receipt))",
            ],
        );
        let _: MintCpkBindingV1 = mint_collective_secret_binding_from_verified_cpk_v1;
        let _: PrepareSecretFreePersistentDecryptionV1 =
            prepare_zk_ams_mkhe_persistent_decryption_from_verified_cpk_v1;
        let _: BindPersistentDecryptionStatementV1 =
            ZkAmsMkhePersistentDecryptionVerificationContextV1::bind_statement_v1;
        let _: VerifyDirectRelationV1 = verify_and_consume_direct_relation_use_v1;
        let _: MaterializeWithoutReceiptV1 =
            zk_ams_phase23_materialize_release_accumulator_chunks_v1::<
                std::vec::IntoIter<Result<ZkAmsT256PackedPlaintextV1, ZkAmsMkheErrorV1>>,
            >;
        let _: SplitWithPersistentReceiptV1 = split_zk_ams_mkhe_decryption_share_v1;
        let ceremony_source = include_str!("cpk_ceremony.rs");
        assert!(ceremony_source.contains("pub struct ZkAmsMkheCpkCeremonyV1"));
        let transition = ceremony_source
            .split("pub fn verify_and_absorb_next_party_v1")
            .nth(1)
            .expect("public CPK party transition")
            .split("pub fn finish_v1")
            .next()
            .expect("public CPK party transition boundary");
        assert_source_markers_in_order_v1(
            transition,
            &[
                "let receipt = verify_zk_ams_mkhe_cpk_relation_v1(",
                "VerifiedZkAmsMkheCpkContributionV1::from_verified_relation(receipt)",
                "builder.absorb_verified_party_v1(contribution, share, &mut state, backend)?",
            ],
        );
        let finish = ceremony_source
            .split("pub fn finish_v1")
            .nth(1)
            .expect("public CPK finish transition")
            .split("/// Sealed successful CPK products")
            .next()
            .expect("public CPK finish boundary");
        assert_source_markers_in_order_v1(
            finish,
            &[
                "let staged = builder.finish_staging_v1()?;",
                "staged.finalize_v1(backend)?",
            ],
        );
        let relation_source = include_str!("cpk_relation.rs");
        assert!(relation_source.contains("struct CpkRelationVerificationSealV1;"));
        assert!(relation_source.contains(
            "pub(super) struct VerifiedZkAmsMkheCpkRelationReceiptV1 {\n    _seal: CpkRelationVerificationSealV1,"
        ));
        assert!(relation_source.contains(
            "pub(super) struct VerifiedZkAmsMkheCpkContributionV1 {\n    receipt: VerifiedZkAmsMkheCpkRelationReceiptV1,"
        ));
        assert!(relation_source.contains(
            "pub(super) fn from_verified_relation(receipt: VerifiedZkAmsMkheCpkRelationReceiptV1)"
        ));
        let staged_source = include_str!("persistent_decryption_equality.rs");
        let staged_constructor = staged_source
            .split("pub(super) fn new(")
            .nth(1)
            .expect("bounded CPK staging constructor")
            .split("/// Borrow the builder-owned common `a`")
            .next()
            .expect("bounded CPK staging constructor boundary");
        assert_eq!(
            staged_constructor
                .matches("try_exact_capacity_vec_v1(")
                .count(),
            5
        );
        assert_eq!(
            staged_constructor
                .matches("try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)",)
                .count(),
            5
        );
        assert_eq!(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, 8);
        assert_eq!(
            staged_constructor
                .matches("ZkAmsMkheErrorV1::ResourceCeilingExceeded")
                .count(),
            5
        );
        assert!(!staged_constructor.contains("Vec::new"));
        assert!(!staged_constructor.contains("try_reserve_exact"));
        let verified_absorb = staged_source
            .split("fn absorb_verified_party_inner_v1")
            .nth(1)
            .expect("verified CPK absorption")
            .split("/// Seal only after all eight ordered shares/proofs were consumed.")
            .next()
            .expect("verified CPK absorption boundary");
        assert_source_markers_in_order_v1(
            verified_absorb,
            &[
                ".into_compact_decryption_source(",
                "mint_collective_secret_binding_from_verified_cpk_v1(",
                "consume_collective_public_key_share_for_staging_v1(",
                ".fork_for_state_and_verifier_v1()",
                "self.admissions.push(admission);",
                "party_state.admit_staged_verified_cpk_binding_v1(",
                "self.next_party_index += 1;",
            ],
        );
        let staging_finish = staged_source
            .split("pub(super) fn finish_staging_v1")
            .nth(1)
            .expect("bounded CPK staging finish")
            .split("fn staged_cpk_batch_digest_v1")
            .next()
            .expect("bounded CPK staging boundary");
        for exact_count_guard in [
            "self.next_party_index != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1",
            "self.admissions.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1",
            "self.bindings.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1",
            "self.party_b_pointers.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1",
            "self.verification_read_receipts.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1",
            "self.publication_receipts.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1",
        ] {
            assert!(staging_finish.contains(exact_count_guard));
        }
        assert!(staging_finish.contains(
            "let admissions: [VerifiedCollectivePublicKeyShareStagedAdmissionV1;\n            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1]"
        ));
        let staged_batch = staged_source
            .split("impl ZkAmsMkheStagedCpkBatchV1")
            .nth(1)
            .expect("sealed staged CPK batch implementation");
        assert!(staged_batch.contains(
            "for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {\n            let admission = &self.admissions[party_index];\n            admission.validate_for_v1"
        ));
        assert!(
            staged_batch
                .matches("for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1")
                .count()
                >= 2
        );
        let collective_source = include_str!("collective.rs");
        assert_eq!(
            collective_source
                .matches("fn aggregate_zk_ams_mkhe_collective_public_key_v1")
                .count(),
            1
        );
        assert!(collective_source.contains(
            "#[cfg(test)]\npub(super) fn aggregate_zk_ams_mkhe_collective_public_key_v1"
        ));
        let staged_aggregate = collective_source
            .split("pub(super) fn finalize_collective_public_key_from_staged_v1")
            .nth(1)
            .expect("sealed staged CPK aggregate")
            .split("fn validate_collective_public_key_share_for_verified_cpk_v1")
            .next()
            .expect("sealed staged CPK aggregate boundary");
        assert!(
            staged_aggregate.contains("for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1")
        );
        assert!(
            staged_aggregate
                .contains("admission.validate_for_v1(roster, transcript_digest, party_index)?")
        );
        for facade_source in [
            include_str!("../mkhe.rs"),
            include_str!("../../zk_ams.rs"),
            include_str!("../../../vega.rs"),
        ] {
            assert!(facade_source.contains("ZkAmsMkheCpkCeremonyV1"));
        }
        let evaluated_key_runtime = include_str!("collective_eval_keys/runtime.rs");
        let runtime_constructor = evaluated_key_runtime
            .split("pub(super) fn new_from_compact_cpk_v1")
            .nth(1)
            .expect("evaluated-key runtime constructor")
            .split("/// Verified aggregate collective-public-key digest.")
            .next()
            .expect("evaluated-key runtime constructor boundary");
        assert!(runtime_constructor.contains(
            "eval_key_binding: super::collective::ZkAmsMkheStreamingCollectiveEvalKeyBindingV1"
        ));
        assert!(
            runtime_constructor.contains("manifest: &ZkAmsMkheCollectiveEvaluatedKeyManifestV1")
        );
        let provider_admission = evaluated_key_runtime
            .split("pub fn validate_seekable_key_provider")
            .nth(1)
            .expect("provider admission")
            .split("fn entry(")
            .next()
            .expect("provider admission boundary");
        assert!(provider_admission.contains("ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1"));
        let cap = provider_admission
            .find("consume_evidence_set_before_provider_v1")
            .expect("capability is consumed");
        let provider = provider_admission
            .find("validate_seekable_evaluated_key")
            .expect("provider is scanned");
        assert!(cap < provider);
        let preflight = evaluated_key_runtime
            .split("fn consume_evidence_set_before_provider_v1")
            .nth(1)
            .expect("private evidence preflight")
            .split("impl ZkAmsMkheCollectiveEvaluatedKeyRuntimeV1")
            .next()
            .expect("private evidence preflight boundary");
        assert!(preflight.contains("consume_for_runtime_v1(expected)"));

        let corrected_source = include_str!("rns_native_split_decryption_source_v2.rs");
        for native_semantic_boundary in [
            "plaintext_coefficients_are_canonical_v2",
            "validate_witness_chunk_v2",
            "validate_nonce_chunk_v2",
            "canonical_plaintext_verified: true",
            "witness_ranges_verified: true",
            "ephemeral_nonzero: true",
            "nonce_nonzero: true",
            "record_source_domain_checks_verified: true",
        ] {
            assert!(corrected_source.contains(native_semantic_boundary));
        }
        assert!(corrected_source.contains("persistent_equality_verified: false"));
        assert!(corrected_source.contains("release_available: false"));

        let rns_link_verifier = include_str!("rns_native_composite_verifier.rs");
        for sealed_receipt_boundary in [
            "struct CompositeAlgebraicVerificationSealV1",
            "pub struct ZkAmsMkheRnsNativeAlgebraicReceiptV1",
            "verify_algebraic_with_first_party_authority_v1(",
            "verify_with_first_party_authority_v1(",
            ".into_algebraic_receipt_v1()",
            "validate_composite_candidate_integrity_v1(&self.composite)?;",
            "verification_context_digest_from_candidate_v1(candidate)",
            "candidate.verification_seal.candidate_digest != candidate.candidate_digest",
        ] {
            assert!(rns_link_verifier.contains(sealed_receipt_boundary));
        }
        assert!(!rns_link_verifier.contains("pub fn into_algebraic_receipt_v1"));
        assert!(!rns_link_verifier.contains("pub fn from_verified_composite_v1"));
    }
    #[test]
    fn authenticated_transport_is_required_while_legacy_openings_remain_quarantined() {
        let phase23_source = include_str!("phase23_rns_link.rs");
        for test_only_boundary in [
            "#[cfg(test)]\n#[path = \"phase23_rns_link_q_relation_adapter.rs\"]\nmod q_relation_adapter;",
            "#[cfg(test)]\n#[path = \"phase23_rns_link_state_owned.rs\"]\nmod state_owned;",
            "#[cfg(test)]\npub(super) struct ZkAmsPhase23NativeBgvOpeningVerifierPermitV1(());",
            "#[cfg(test)]\nfn verify_zk_ams_phase23_native_bgv_opening_v1(",
        ] {
            assert!(
                phase23_source.contains(test_only_boundary),
                "native opening boundary escaped its test-only quarantine: {test_only_boundary}",
            );
        }
        assert_eq!(
            phase23_source
                .matches("#[path = \"phase23_rns_link_state_owned.rs\"]")
                .count(),
            1
        );
        assert_eq!(
            phase23_source
                .matches("fn verify_zk_ams_phase23_native_bgv_opening_v1(")
                .count(),
            1
        );

        let collective_source = include_str!("collective.rs");
        assert!(collective_source.contains(
            "#[cfg(test)]\n    pub(super) fn verify_and_consume_phase23_native_bgv_opening_v1("
        ));
        let state_owned_source = include_str!("phase23_rns_link_state_owned.rs");
        for non_authorizing_boundary in [
            "finish_into_unverified_metadata_v1",
            "ZkAmsPhase23RnsLinkUnverifiedStateOwnedNativeBgvPreflightV1",
            "q_native_witness_polynomials_constructed: false",
            "q_native_fiat_shamir_relation_bound: false",
            "q_native_zero_knowledge_masked: false",
        ] {
            assert!(
                state_owned_source.contains(non_authorizing_boundary),
                "state-owned opening quarantine lost boundary: {non_authorizing_boundary}",
            );
        }

        let composite_source = include_str!("rns_native_composite_verifier.rs");
        for transport_boundary in [
            "struct VerifierAuthenticatedTransportSealV1;",
            "pub struct ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1",
            "pub fn authenticate_canonical_exact_v1(",
            "ZkAmsMkheRnsNativeProofEnvelopeV1::from_canonical_bytes_exact_v1(",
            "let axes = validate_context_v1(&envelope, source_layout, source_receipt, &transcript)?;",
            "let opening_commitment_root = opening_commitment_root_v1(transcript)?;",
            "axes.verifier_context_digest = verification_context_digest_v1(&axes);",
            "axes.verifier_transport_digest = verifier_authenticated_transport_digest_v1(&axes);",
        ] {
            assert!(
                composite_source.contains(transport_boundary),
                "verifier-authenticated transport lost boundary: {transport_boundary}",
            );
        }
        let context_constructor = composite_source
            .split("impl ContextCheckedV1 {")
            .nth(1)
            .expect("composite context typestate")
            .split("fn verify_stage_v1")
            .next()
            .expect("composite context constructor boundary");
        assert!(
            context_constructor
                .contains("transport: ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1")
        );
        for forbidden_raw_input in [
            "envelope: ZkAmsMkheRnsNativeProofEnvelopeV1",
            "source_layout: ZkAmsMkheRnsNativeSourceLayoutV1",
            "source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1",
            "transcript: ZkAmsMkheRnsNativeChallengeSeedsV1",
        ] {
            assert!(!context_constructor.contains(forbidden_raw_input));
        }
        for public_verifier in [
            "pub fn verify_zk_ams_mkhe_rns_native_composite_v1(",
            "pub fn verify_zk_ams_mkhe_rns_native_algebraic_v1(",
        ] {
            let signature = composite_source
                .split(public_verifier)
                .nth(1)
                .expect("public composite verifier")
                .split(") -> Result")
                .next()
                .expect("public composite verifier signature");
            assert!(
                signature.contains("transport: ZkAmsMkheRnsNativeVerifierAuthenticatedTransportV1")
            );
            assert!(!signature.contains("ZkAmsMkheRnsNativeProofEnvelopeV1"));
            assert!(!signature.contains("ZkAmsMkheRnsNativeChallengeSeedsV1"));
        }
        let consumers = include_str!("rns_native_receipt_consumers.rs");
        for transitive_binding in [
            "verifier_context_digest: receipt.verifier_context_digest()",
            "opening_commitment_root: receipt.opening_commitment_root()",
            "verifier_transport_digest: receipt.verifier_transport_digest()",
            "hash.update(&use_value.verifier_context_digest);",
            "hash.update(&use_value.opening_commitment_root);",
            "hash.update(&use_value.verifier_transport_digest);",
        ] {
            assert!(
                consumers.contains(transitive_binding),
                "production consumer lost transport binding: {transitive_binding}",
            );
        }
        for unavailable_stage in [
            "ZkAmsMkheRnsNativeVerificationStageV1::TerminalHyraxBpBridge",
            "ZkAmsMkheRnsNativeVerificationStageV1::RnsRelationQpcs",
            "ZkAmsMkheRnsNativeVerificationStageV1::CrossFieldGlobalLookup",
            "ZkAmsMkheRnsNativeVerificationStageV1::ZeroPadding",
        ] {
            let unavailable_marker = format!(
                "ZkAmsMkheRnsNativeCompositeVerificationErrorV1::StageUnavailable(\n            {unavailable_stage},"
            );
            assert!(
                composite_source.contains(&unavailable_marker),
                "production stage no longer carries its explicit unavailable exit: {unavailable_stage}",
            );
        }
    }
    #[test]
    fn current_graph_records_every_open_handoff_without_false_certification() {
        let audit = zk_ams_mkhe_receipt_capability_audit_v1();
        audit.validate().unwrap();
        assert_eq!(
            implementation_prerequisite_blocker_mask_v1(audit),
            CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1
        );
        assert_eq!(implementation_prerequisite_blocker_mask_v1(audit), 0x7c);
        assert!(audit.cpk_relation_receipt_sealed);
        assert!(audit.cpk_party_state_admission_consumes_receipt);
        assert!(audit.cpk_aggregate_receipt_enforced);
        assert!(audit.rkg_round_one_receipt_enforced);
        assert!(audit.rkg_round_two_receipt_enforced);
        assert!(audit.galois_key_receipt_enforced);
        assert!(audit.rns_link_transport_bound);
        assert!(audit.native_bgv_opening_receipt_sealed);
        assert!(audit.native_materialized_hyrax_receipt_sealed);
        assert!(audit.terminal_prover_consumes_native_materialized_receipt);
        assert!(!audit.rns_link_family_geometry_matches_native);
        assert!(!audit.rns_link_carry_quotient_responses_verifiable);
        assert!(!audit.hyrax_bgv_equality_responses_verifiable);
        assert!(audit.rns_link_algebraic_receipt_complete);
        assert!(audit.persistent_decryption_replay_axes_specified);
        assert!(audit.split_decryption_source_profile_separated);
        assert!(audit.split_decryption_source_domain_checks_enforced);
        assert!(audit.split_decryption_source_exact_43_receipt_sealed);
        assert!(!audit.split_decryption_source_ciphertext_equality_complete);
        assert_eq!(
            audit.blocker_mask,
            CURRENT_OPEN_RECEIPT_CAPABILITY_BLOCKERS_V1
        );
        assert_ne!(audit.blocker_mask & BLOCKER_RNS_LINK_VERIFICATION_V1, 0);
        assert_eq!(audit.blocker_mask, 0x1f0);
        assert_ne!(audit.digest, [0; 32]);
        assert!(!audit.release_available);
    }
    #[test]
    fn receipt_consumers_closed_by_sealed_evidence_sets_are_authorized() {
        for consumer in [
            ZkAmsMkheReceiptCapabilityConsumerV1::CollectivePublicKeyAggregate,
            ZkAmsMkheReceiptCapabilityConsumerV1::RkgRoundOne,
            ZkAmsMkheReceiptCapabilityConsumerV1::RkgRoundTwo,
            ZkAmsMkheReceiptCapabilityConsumerV1::GaloisKey,
        ] {
            assert!(zk_ams_mkhe_receipt_capability_audit_v1().authorizes(consumer));
            assert_eq!(require_zk_ams_mkhe_receipt_capability_v1(consumer), Ok(()));
        }
        for consumer in [
            ZkAmsMkheReceiptCapabilityConsumerV1::BeginStateOwnedNativeBgvOpeningsV1,
            ZkAmsMkheReceiptCapabilityConsumerV1::AbsorbStateOwnedNativeBgvOpeningV1,
            ZkAmsMkheReceiptCapabilityConsumerV1::FinishStateOwnedNativeBgvOpeningsV1,
            ZkAmsMkheReceiptCapabilityConsumerV1::RnsLinkVerification,
            ZkAmsMkheReceiptCapabilityConsumerV1::TerminalMaterialization,
            ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryption,
            ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryptionSourceMaterialization,
        ] {
            assert!(!zk_ams_mkhe_receipt_capability_audit_v1().authorizes(consumer));
            assert_eq!(
                require_zk_ams_mkhe_receipt_capability_v1(consumer),
                Err(ZkAmsMkheErrorV1::ReleaseUnavailable)
            );
        }
    }
    #[test]
    fn recomputed_begin_absorb_finish_shells_cannot_escape_current_source_inventory() {
        let baseline = zk_ams_mkhe_receipt_capability_audit_v1();

        let mut begin_shell = baseline;
        begin_shell.rns_link_transport_bound = true;
        begin_shell.rns_link_family_geometry_matches_native = true;
        begin_shell.native_bgv_opening_receipt_sealed = false;
        let begin_shell = recompute_audit_shell_v1(begin_shell);
        begin_shell.validate_logical_graph_v1().unwrap();
        assert!(
            begin_shell.authorizes(
                ZkAmsMkheReceiptCapabilityConsumerV1::BeginStateOwnedNativeBgvOpeningsV1
            )
        );
        assert!(
            !begin_shell.authorizes(
                ZkAmsMkheReceiptCapabilityConsumerV1::AbsorbStateOwnedNativeBgvOpeningV1
            )
        );
        assert_eq!(
            begin_shell.validate(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );

        let mut absorb_shell = begin_shell;
        absorb_shell.native_bgv_opening_receipt_sealed = true;
        let absorb_shell = recompute_audit_shell_v1(absorb_shell);
        absorb_shell.validate_logical_graph_v1().unwrap();
        assert!(
            absorb_shell.authorizes(
                ZkAmsMkheReceiptCapabilityConsumerV1::AbsorbStateOwnedNativeBgvOpeningV1
            )
        );
        assert!(
            !absorb_shell.authorizes(
                ZkAmsMkheReceiptCapabilityConsumerV1::FinishStateOwnedNativeBgvOpeningsV1
            )
        );
        assert_eq!(
            absorb_shell.validate(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );

        let mut finish_shell = absorb_shell;
        finish_shell.rns_link_carry_quotient_responses_verifiable = true;
        finish_shell.hyrax_bgv_equality_responses_verifiable = true;
        let finish_shell = recompute_audit_shell_v1(finish_shell);
        finish_shell.validate_logical_graph_v1().unwrap();
        assert!(
            finish_shell.authorizes(
                ZkAmsMkheReceiptCapabilityConsumerV1::FinishStateOwnedNativeBgvOpeningsV1
            )
        );
        assert!(finish_shell.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::RnsLinkVerification));
        assert_eq!(
            finish_shell.validate(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );

        let mut forged_release = finish_shell;
        forged_release.persistent_decryption_equality_complete = true;
        forged_release.split_decryption_source_ciphertext_equality_complete = true;
        let forged_release = recompute_audit_shell_v1(forged_release);
        forged_release.validate_logical_graph_v1().unwrap();
        assert_eq!(forged_release.blocker_mask, 0);
        assert_eq!(
            implementation_prerequisite_blocker_mask_v1(forged_release),
            0
        );
        assert!(forged_release.release_available);
        assert_eq!(
            forged_release.validate(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
    }
    #[test]
    fn rns_link_blocker_requires_the_complete_operational_verification_chain() {
        let baseline = zk_ams_mkhe_receipt_capability_audit_v1();
        assert_ne!(baseline.blocker_mask & BLOCKER_RNS_LINK_VERIFICATION_V1, 0);

        let mut closed = baseline;
        closed.rns_link_transport_bound = true;
        closed.native_bgv_opening_receipt_sealed = true;
        closed.rns_link_family_geometry_matches_native = true;
        closed.rns_link_carry_quotient_responses_verifiable = true;
        closed.hyrax_bgv_equality_responses_verifiable = true;
        closed.rns_link_algebraic_receipt_complete = true;
        closed.blocker_mask = receipt_capability_blocker_mask_v1(closed);
        closed.release_available = receipt_capability_release_available_v1(closed);
        closed.digest = receipt_capability_audit_digest_v1(closed);
        closed.validate_logical_graph_v1().unwrap();
        assert_eq!(
            closed.validate_current_implementation_v1(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
        assert!(closed.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::RnsLinkVerification));
        assert_eq!(closed.blocker_mask & BLOCKER_RNS_LINK_VERIFICATION_V1, 0);
        assert!(!closed.release_available);

        macro_rules! assert_rns_axis_required {
            ($field:ident) => {{
                let mut incomplete = closed;
                incomplete.$field = false;
                incomplete.blocker_mask = receipt_capability_blocker_mask_v1(incomplete);
                incomplete.release_available = receipt_capability_release_available_v1(incomplete);
                incomplete.digest = receipt_capability_audit_digest_v1(incomplete);
                incomplete.validate_logical_graph_v1().unwrap();
                assert!(
                    !incomplete
                        .authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::RnsLinkVerification)
                );
                assert_ne!(
                    incomplete.blocker_mask & BLOCKER_RNS_LINK_VERIFICATION_V1,
                    0,
                    stringify!($field),
                );
            }};
        }
        assert_rns_axis_required!(rns_link_transport_bound);
        assert_rns_axis_required!(native_bgv_opening_receipt_sealed);
        assert_rns_axis_required!(rns_link_family_geometry_matches_native);
        assert_rns_axis_required!(rns_link_carry_quotient_responses_verifiable);
        assert_rns_axis_required!(hyrax_bgv_equality_responses_verifiable);
        assert_rns_axis_required!(rns_link_algebraic_receipt_complete);
    }
    #[test]
    fn terminal_and_split_consumers_cannot_bypass_the_rns_link_chain() {
        let baseline = zk_ams_mkhe_receipt_capability_audit_v1();

        let mut local_handoffs_only = baseline;
        local_handoffs_only.terminal_materialization_receipt_enforced = true;
        local_handoffs_only.split_decryption_receipts_enforced = true;
        local_handoffs_only.persistent_decryption_equality_complete = true;
        local_handoffs_only.blocker_mask = receipt_capability_blocker_mask_v1(local_handoffs_only);
        local_handoffs_only.release_available =
            receipt_capability_release_available_v1(local_handoffs_only);
        local_handoffs_only.digest = receipt_capability_audit_digest_v1(local_handoffs_only);
        local_handoffs_only.validate_logical_graph_v1().unwrap();
        assert!(
            !local_handoffs_only
                .authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::TerminalMaterialization)
        );
        assert!(
            !local_handoffs_only.authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryption)
        );
        assert_ne!(
            local_handoffs_only.blocker_mask & BLOCKER_TERMINAL_MATERIALIZATION_V1,
            0
        );
        assert_ne!(
            local_handoffs_only.blocker_mask & BLOCKER_SPLIT_DECRYPTION_V1,
            0
        );

        let mut complete_prerequisites = local_handoffs_only;
        complete_prerequisites.rns_link_transport_bound = true;
        complete_prerequisites.native_bgv_opening_receipt_sealed = true;
        complete_prerequisites.rns_link_family_geometry_matches_native = true;
        complete_prerequisites.rns_link_carry_quotient_responses_verifiable = true;
        complete_prerequisites.hyrax_bgv_equality_responses_verifiable = true;
        complete_prerequisites.rns_link_algebraic_receipt_complete = true;
        complete_prerequisites.blocker_mask =
            receipt_capability_blocker_mask_v1(complete_prerequisites);
        complete_prerequisites.release_available =
            receipt_capability_release_available_v1(complete_prerequisites);
        complete_prerequisites.digest = receipt_capability_audit_digest_v1(complete_prerequisites);
        complete_prerequisites.validate_logical_graph_v1().unwrap();
        assert!(
            complete_prerequisites
                .authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::TerminalMaterialization)
        );
        assert!(
            complete_prerequisites
                .authorizes(ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryption)
        );
        assert_eq!(
            complete_prerequisites.blocker_mask
                & (BLOCKER_RNS_LINK_VERIFICATION_V1
                    | BLOCKER_TERMINAL_MATERIALIZATION_V1
                    | BLOCKER_SPLIT_DECRYPTION_V1
                    | BLOCKER_PERSISTENT_DECRYPTION_EQUALITY_V1),
            0
        );
        assert!(!complete_prerequisites.release_available);
    }
    #[test]
    fn source_materialization_cannot_bypass_any_typed_axis() {
        let baseline = zk_ams_mkhe_receipt_capability_audit_v1();
        assert!(!baseline.authorizes(
            ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryptionSourceMaterialization
        ));
        assert_ne!(
            baseline.blocker_mask & BLOCKER_SPLIT_DECRYPTION_SOURCE_EQUALITY_V1,
            0
        );

        let mut closed_source = baseline;
        closed_source.split_decryption_source_ciphertext_equality_complete = true;
        closed_source.blocker_mask = receipt_capability_blocker_mask_v1(closed_source);
        closed_source.release_available = receipt_capability_release_available_v1(closed_source);
        closed_source.digest = receipt_capability_audit_digest_v1(closed_source);
        closed_source.validate_logical_graph_v1().unwrap();
        assert_eq!(
            closed_source.validate_current_implementation_v1(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
        assert!(closed_source.authorizes(
            ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryptionSourceMaterialization
        ));
        assert_eq!(
            closed_source.blocker_mask & BLOCKER_SPLIT_DECRYPTION_SOURCE_EQUALITY_V1,
            0
        );
        assert!(!closed_source.release_available);

        macro_rules! assert_axis_required {
            ($field:ident) => {{
                let mut incomplete = closed_source;
                incomplete.$field = false;
                incomplete.blocker_mask = receipt_capability_blocker_mask_v1(incomplete);
                incomplete.release_available = receipt_capability_release_available_v1(incomplete);
                incomplete.digest = receipt_capability_audit_digest_v1(incomplete);
                incomplete.validate_logical_graph_v1().unwrap();
                assert!(!incomplete.authorizes(
                    ZkAmsMkheReceiptCapabilityConsumerV1::SplitDecryptionSourceMaterialization
                ));
                assert_ne!(
                    incomplete.blocker_mask & BLOCKER_SPLIT_DECRYPTION_SOURCE_EQUALITY_V1,
                    0,
                    stringify!($field),
                );
            }};
        }
        assert_axis_required!(split_decryption_source_profile_separated);
        assert_axis_required!(split_decryption_source_domain_checks_enforced);
        assert_axis_required!(split_decryption_source_exact_43_receipt_sealed);
        assert_axis_required!(split_decryption_source_ciphertext_equality_complete);
    }
    #[test]
    fn each_missing_production_prerequisite_rejects_a_silent_audit_axis_flip() {
        let baseline = zk_ams_mkhe_receipt_capability_audit_v1();
        baseline.validate().unwrap();
        assert_eq!(
            implementation_prerequisite_blocker_mask_v1(baseline),
            CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1
        );

        let mut native_receipt_regression = baseline;
        native_receipt_regression.native_bgv_opening_receipt_sealed = false;
        let native_receipt_regression = recompute_audit_shell_v1(native_receipt_regression);
        native_receipt_regression
            .validate_logical_graph_v1()
            .unwrap();
        assert_eq!(
            implementation_prerequisite_blocker_mask_v1(native_receipt_regression),
            CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1
                | PREREQUISITE_PRODUCTION_NATIVE_BGV_OPENING_RECEIPT_V1
        );
        assert_eq!(
            native_receipt_regression.validate(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );

        let mut transport_regression = baseline;
        transport_regression.rns_link_transport_bound = false;
        let transport_regression = recompute_audit_shell_v1(transport_regression);
        transport_regression.validate_logical_graph_v1().unwrap();
        assert_eq!(
            implementation_prerequisite_blocker_mask_v1(transport_regression),
            CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1
                | PREREQUISITE_VERIFIER_DERIVED_RNS_TRANSPORT_V1
        );
        assert_eq!(
            transport_regression.validate(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );

        macro_rules! assert_silent_flip_rejected {
            ($field:ident, $prerequisite:ident) => {{
                let mut changed = baseline;
                changed.$field = true;
                changed.blocker_mask = receipt_capability_blocker_mask_v1(changed);
                changed.release_available = receipt_capability_release_available_v1(changed);
                changed.digest = receipt_capability_audit_digest_v1(changed);
                changed.validate_logical_graph_v1().unwrap();
                assert_eq!(
                    implementation_prerequisite_blocker_mask_v1(changed),
                    CURRENT_MISSING_IMPLEMENTATION_PREREQUISITES_V1 & !$prerequisite
                );
                assert_eq!(
                    changed.validate(),
                    Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
                    stringify!($field),
                );
            }};
        }

        assert_silent_flip_rejected!(
            rns_link_family_geometry_matches_native,
            PREREQUISITE_CORRECTED_RNS_GEOMETRY_END_TO_END_V1
        );
        assert_silent_flip_rejected!(
            rns_link_carry_quotient_responses_verifiable,
            PREREQUISITE_RNS_CARRY_QUOTIENT_SOURCE_RELATION_V1
        );
        assert_silent_flip_rejected!(
            hyrax_bgv_equality_responses_verifiable,
            PREREQUISITE_HYRAX_SOURCE_MATERIALIZATION_EQUALITY_V1
        );
        assert_silent_flip_rejected!(
            persistent_decryption_equality_complete,
            PREREQUISITE_PERSISTENT_DECRYPTION_SIS_CERTIFICATE_V1
        );
        assert_silent_flip_rejected!(
            split_decryption_source_ciphertext_equality_complete,
            PREREQUISITE_SPLIT_SOURCE_CIPHERTEXT_EQUATIONS_V1
        );
    }
    #[test]
    fn audit_digest_binds_every_capability_axis() {
        let baseline = zk_ams_mkhe_receipt_capability_audit_v1();
        macro_rules! assert_flag_bound {
            ($field:ident) => {{
                let mut changed = baseline;
                changed.$field = !changed.$field;
                assert_ne!(receipt_capability_audit_digest_v1(changed), baseline.digest);
            }};
        }
        assert_flag_bound!(cpk_relation_receipt_sealed);
        assert_flag_bound!(cpk_party_state_admission_consumes_receipt);
        assert_flag_bound!(cpk_aggregate_receipt_enforced);
        assert_flag_bound!(rkg_round_one_receipt_enforced);
        assert_flag_bound!(rkg_round_two_receipt_enforced);
        assert_flag_bound!(galois_key_receipt_enforced);
        assert_flag_bound!(rns_link_transport_bound);
        assert_flag_bound!(native_bgv_opening_receipt_sealed);
        assert_flag_bound!(native_materialized_hyrax_receipt_sealed);
        assert_flag_bound!(terminal_prover_consumes_native_materialized_receipt);
        assert_flag_bound!(rns_link_family_geometry_matches_native);
        assert_flag_bound!(rns_link_carry_quotient_responses_verifiable);
        assert_flag_bound!(hyrax_bgv_equality_responses_verifiable);
        assert_flag_bound!(rns_link_algebraic_receipt_complete);
        assert_flag_bound!(terminal_materialization_receipt_enforced);
        assert_flag_bound!(split_decryption_receipts_enforced);
        assert_flag_bound!(persistent_decryption_equality_complete);
        assert_flag_bound!(persistent_decryption_replay_axes_specified);
        assert_flag_bound!(split_decryption_source_profile_separated);
        assert_flag_bound!(split_decryption_source_domain_checks_enforced);
        assert_flag_bound!(split_decryption_source_exact_43_receipt_sealed);
        assert_flag_bound!(split_decryption_source_ciphertext_equality_complete);
        let mut changed = baseline;
        changed.blocker_mask ^= BLOCKER_RNS_LINK_VERIFICATION_V1;
        assert_ne!(receipt_capability_audit_digest_v1(changed), baseline.digest);
        assert_eq!(
            changed.validate(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
        let mut changed = baseline;
        changed.release_available = true;
        assert_ne!(receipt_capability_audit_digest_v1(changed), baseline.digest);
        assert_eq!(
            changed.validate(),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
    }
}
