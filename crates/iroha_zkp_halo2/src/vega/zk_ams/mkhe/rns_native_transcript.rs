//! Canonical Fiat--Shamir transcript for the replacement RNS-native proof.
//!
//! This module owns transcript integrity only.  It fixes the order and domain
//! of every public binding, commitment, root, and challenge used by the
//! 40-limb composite proof.  Its stages are move-only and every later input is
//! tagged with the exact prior-stage binding, so a caller cannot rewind a
//! transcript or splice a proof fragment across contexts.  Successfully
//! finalizing this transcript does not verify a proof and grants no release or
//! readiness authority.

#![allow(
    clippy::large_types_passed_by_value,
    reason = "move-only transcript stages deliberately transfer fixed-capacity owners by value"
)]

use super::{
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1, ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1,
        ZkAmsMkheRnsNativeFamilyV1, zk_ams_mkhe_rns_native_profile_manifest_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1, zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_source::{ZkAmsMkheRnsNativeSourceLayoutV1, ZkAmsMkheRnsNativeSourceReceiptV1},
};
use crate::vega::sponge::Keccak256;

const TRANSCRIPT_INITIAL_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-transcript.initial";
const TRANSCRIPT_ABSORB_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-transcript.absorb";
const TRANSCRIPT_OPENING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-transcript.opening";
const TRANSCRIPT_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-transcript.challenge";
const TRANSCRIPT_RATCHET_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-transcript.ratchet";

const OPENING_COUNT_V1: usize = 43;
const FRI_ROOT_COUNT_V1: usize = 18;
const CHALLENGE_COUNT_U16_V1: u16 = 28;
const INPUT_DIGEST_COUNT_V1: usize = 12 + 2 * OPENING_COUNT_V1 + 3 + 2 + FRI_ROOT_COUNT_V1 + 3;
const MAX_REGISTERED_DIGESTS_V1: usize =
    INPUT_DIGEST_COUNT_V1 + ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_CHALLENGE_COUNT_V1;

/// Transcript schema version.
pub const ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_VERSION_V1: u8 = 1;
/// Exact qPCS root count: initial, quotient, then eighteen ordered FRI roots.
pub const ZK_AMS_MKHE_RNS_NATIVE_QPCS_ROOT_COUNT_V1: usize = 2 + FRI_ROOT_COUNT_V1;
/// Exact number of domain-separated challenge seeds in the composite schedule.
pub const ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_CHALLENGE_COUNT_V1: usize = 28;

const _: () = {
    assert!(OPENING_COUNT_V1 == 43);
    assert!(FRI_ROOT_COUNT_V1 == 18);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_QPCS_ROOT_COUNT_V1 == 20);
    assert!(INPUT_DIGEST_COUNT_V1 == 124);
    assert!(MAX_REGISTERED_DIGESTS_V1 == 152);
};

/// Structural transcript failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkAmsMkheRnsNativeTranscriptErrorV1 {
    /// The canonical replacement profile, topology, or candidate identity failed validation.
    InvalidProfileBinding,
    /// The structural source receipt did not validate against the exact supplied layout.
    InvalidSourceContext,
    /// A digest that must name a concrete transcript object was zero.
    ZeroDigest,
    /// Two semantically distinct transcript objects reused one digest.
    DuplicateDigest,
    /// An opening role or family-local index was invalid.
    InvalidOpening,
    /// The 43 opening records were not in `X1/U16/E16/rE1/W8/rW1` order.
    InvalidOpeningOrder,
    /// A typed proof root used an out-of-range role or layer.
    InvalidRoot,
    /// Ordered proof roots were not in their sole canonical schedule.
    InvalidRootOrder,
    /// A later proof fragment was tagged for a different prior transcript stage.
    ContextMismatch,
    /// A challenge or ratcheted transcript state was invalid.
    InvalidChallenge,
}

impl core::fmt::Display for ZkAmsMkheRnsNativeTranscriptErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidProfileBinding => "invalid RNS-native transcript profile binding",
            Self::InvalidSourceContext => "invalid RNS-native transcript source context",
            Self::ZeroDigest => "zero RNS-native transcript digest",
            Self::DuplicateDigest => "duplicated RNS-native transcript digest",
            Self::InvalidOpening => "invalid RNS-native transcript opening",
            Self::InvalidOpeningOrder => "invalid RNS-native transcript opening order",
            Self::InvalidRoot => "invalid RNS-native transcript root",
            Self::InvalidRootOrder => "invalid RNS-native transcript root order",
            Self::ContextMismatch => "mismatched RNS-native prior transcript binding",
            Self::InvalidChallenge => "invalid RNS-native transcript challenge",
        })
    }
}

impl std::error::Error for ZkAmsMkheRnsNativeTranscriptErrorV1 {}

/// Governed public inputs not already carried by the typed source layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativePublicContextV1 {
    governed_roster_digest: [u8; 32],
    public_ciphertext_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativePublicContextV1 {
    /// Construct the exact governed-roster and public-ciphertext binding.
    ///
    /// # Errors
    ///
    /// Rejects a zero digest or reuse of one digest for both public objects.
    pub fn new(
        governed_roster_digest: [u8; 32],
        public_ciphertext_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        validate_distinct_digests_v1(&[governed_roster_digest, public_ciphertext_digest])?;
        Ok(Self {
            governed_roster_digest,
            public_ciphertext_digest,
        })
    }

    /// Return the exact governed-roster digest.
    #[must_use]
    pub const fn governed_roster_digest(self) -> [u8; 32] {
        self.governed_roster_digest
    }

    /// Return the exact public-ciphertext digest.
    #[must_use]
    pub const fn public_ciphertext_digest(self) -> [u8; 32] {
        self.public_ciphertext_digest
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TranscriptContextIdentitiesV1 {
    profile_manifest_digest: [u8; 32],
    profile_digest: [u8; 32],
    topology_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    source_binding_digest: [u8; 32],
    main_snapshot_digest: [u8; 32],
    nonce_snapshot_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    governed_roster_digest: [u8; 32],
    public_ciphertext_digest: [u8; 32],
}

impl TranscriptContextIdentitiesV1 {
    const fn ordered(self) -> [[u8; 32]; 12] {
        [
            self.profile_manifest_digest,
            self.profile_digest,
            self.topology_digest,
            self.release_candidate_digest,
            self.statement_digest,
            self.operational_context_digest,
            self.source_binding_digest,
            self.main_snapshot_digest,
            self.nonce_snapshot_digest,
            self.source_receipt_digest,
            self.governed_roster_digest,
            self.public_ciphertext_digest,
        ]
    }
}

/// One source/Hyrax commitment pair at a canonical family-local position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeOpeningCommitmentV1 {
    family: ZkAmsMkheRnsNativeFamilyV1,
    family_index: u8,
    source_commitment_digest: [u8; 32],
    hyrax_commitment_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeOpeningCommitmentV1 {
    /// Construct one typed commitment pair.
    ///
    /// # Errors
    ///
    /// Rejects an out-of-range family index, zero digest, or equal source and
    /// Hyrax digests.
    pub fn new(
        family: ZkAmsMkheRnsNativeFamilyV1,
        family_index: u8,
        source_commitment_digest: [u8; 32],
        hyrax_commitment_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        if family_index >= family.record_count() {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidOpening);
        }
        validate_distinct_digests_v1(&[source_commitment_digest, hyrax_commitment_digest])?;
        Ok(Self {
            family,
            family_index,
            source_commitment_digest,
            hyrax_commitment_digest,
        })
    }

    /// Return the canonical family.
    #[must_use]
    pub const fn family(self) -> ZkAmsMkheRnsNativeFamilyV1 {
        self.family
    }

    /// Return the family-local record index.
    #[must_use]
    pub const fn family_index(self) -> u8 {
        self.family_index
    }

    /// Return the source-commitment digest.
    #[must_use]
    pub const fn source_commitment_digest(self) -> [u8; 32] {
        self.source_commitment_digest
    }

    /// Return the matching Hyrax-commitment digest.
    #[must_use]
    pub const fn hyrax_commitment_digest(self) -> [u8; 32] {
        self.hyrax_commitment_digest
    }
}

/// Move-only owner of all 43 canonically ordered commitment pairs.
#[allow(
    missing_copy_implementations,
    reason = "the ordered commitment owner is consumed by its sole transcript stage"
)]
pub struct ZkAmsMkheRnsNativeOpeningCommitmentsV1 {
    prior_transcript_binding: [u8; 32],
    records: [ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1],
}

impl ZkAmsMkheRnsNativeOpeningCommitmentsV1 {
    /// Validate and bind the exact `X1/U16/E16/rE1/W8/rW1` record sequence.
    ///
    /// # Errors
    ///
    /// Rejects a zero prior binding, a wrong role/index position, or any
    /// duplicated commitment digest.
    pub fn new(
        prior_transcript_binding: [u8; 32],
        records: [ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1],
    ) -> Result<Self, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        if prior_transcript_binding == [0; 32] {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ZeroDigest);
        }
        let mut digests = DigestRegistryV1::new();
        digests.insert(prior_transcript_binding)?;
        for (ordinal, record) in records.iter().enumerate() {
            let expected = opening_role_v1(ordinal)
                .ok_or(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidOpeningOrder)?;
            if (record.family, record.family_index) != expected {
                return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidOpeningOrder);
            }
            digests.insert(record.source_commitment_digest)?;
            digests.insert(record.hyrax_commitment_digest)?;
        }
        Ok(Self {
            prior_transcript_binding,
            records,
        })
    }

    /// Borrow the exact ordered commitment records.
    #[must_use]
    pub const fn records(&self) -> &[ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1] {
        &self.records
    }
}

/// Mapping and terminal Hyrax/cross-basis commitment roots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeTerminalBridgeV1 {
    prior_transcript_binding: [u8; 32],
    mapping_root: [u8; 32],
    terminal_hyrax_root: [u8; 32],
    cross_basis_bridge_root: [u8; 32],
}

impl ZkAmsMkheRnsNativeTerminalBridgeV1 {
    /// Construct a terminal bridge tagged for one exact commitment transcript.
    ///
    /// # Errors
    ///
    /// Rejects zero or duplicated binding/root digests.
    pub fn new(
        prior_transcript_binding: [u8; 32],
        mapping_root: [u8; 32],
        terminal_hyrax_root: [u8; 32],
        cross_basis_bridge_root: [u8; 32],
    ) -> Result<Self, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        validate_distinct_digests_v1(&[
            prior_transcript_binding,
            mapping_root,
            terminal_hyrax_root,
            cross_basis_bridge_root,
        ])?;
        Ok(Self {
            prior_transcript_binding,
            mapping_root,
            terminal_hyrax_root,
            cross_basis_bridge_root,
        })
    }

    /// Return the source-to-Hyrax mapping root.
    #[must_use]
    pub const fn mapping_root(self) -> [u8; 32] {
        self.mapping_root
    }

    /// Return the terminal Hyrax root.
    #[must_use]
    pub const fn terminal_hyrax_root(self) -> [u8; 32] {
        self.terminal_hyrax_root
    }

    /// Return the cross-basis bridge root.
    #[must_use]
    pub const fn cross_basis_bridge_root(self) -> [u8; 32] {
        self.cross_basis_bridge_root
    }
}

/// One typed qPCS FRI root at its canonical layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeQpcsFriRootV1 {
    layer: u8,
    root: [u8; 32],
}

impl ZkAmsMkheRnsNativeQpcsFriRootV1 {
    /// Construct one nonzero root for a canonical FRI layer.
    ///
    /// # Errors
    ///
    /// Rejects a layer outside `0..18` or a zero root.
    pub fn new(layer: u8, root: [u8; 32]) -> Result<Self, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        if usize::from(layer) >= FRI_ROOT_COUNT_V1 {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidRoot);
        }
        if root == [0; 32] {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ZeroDigest);
        }
        Ok(Self { layer, root })
    }

    /// Return the zero-based FRI layer.
    #[must_use]
    pub const fn layer(self) -> u8 {
        self.layer
    }

    /// Return the root at this layer.
    #[must_use]
    pub const fn root(self) -> [u8; 32] {
        self.root
    }
}

/// Exact qPCS root schedule: initial, quotient, then FRI layers zero through seventeen.
#[allow(
    missing_copy_implementations,
    reason = "the qPCS root schedule is consumed by its sole transcript stage"
)]
pub struct ZkAmsMkheRnsNativeQpcsRootsV1 {
    prior_transcript_binding: [u8; 32],
    initial_root: [u8; 32],
    quotient_root: [u8; 32],
    fri_roots: [ZkAmsMkheRnsNativeQpcsFriRootV1; FRI_ROOT_COUNT_V1],
}

impl ZkAmsMkheRnsNativeQpcsRootsV1 {
    /// Construct the fixed-width qPCS root schedule for one terminal transcript.
    ///
    /// # Errors
    ///
    /// Rejects zero/duplicated digests or FRI roots outside exact layer order.
    pub fn new(
        prior_transcript_binding: [u8; 32],
        initial_root: [u8; 32],
        quotient_root: [u8; 32],
        fri_roots: [ZkAmsMkheRnsNativeQpcsFriRootV1; FRI_ROOT_COUNT_V1],
    ) -> Result<Self, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        let mut digests = DigestRegistryV1::new();
        for digest in [prior_transcript_binding, initial_root, quotient_root] {
            digests.insert(digest)?;
        }
        for (layer, root) in fri_roots.iter().enumerate() {
            if usize::from(root.layer) != layer {
                return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidRootOrder);
            }
            digests.insert(root.root)?;
        }
        Ok(Self {
            prior_transcript_binding,
            initial_root,
            quotient_root,
            fri_roots,
        })
    }

    /// Return the initial committed-codeword root.
    #[must_use]
    pub const fn initial_root(&self) -> [u8; 32] {
        self.initial_root
    }

    /// Return the opening-quotient root.
    #[must_use]
    pub const fn quotient_root(&self) -> [u8; 32] {
        self.quotient_root
    }

    /// Borrow the eighteen FRI roots in layer order.
    #[must_use]
    pub const fn fri_roots(&self) -> &[ZkAmsMkheRnsNativeQpcsFriRootV1; FRI_ROOT_COUNT_V1] {
        &self.fri_roots
    }
}

/// Cross-field, global-lookup, and zero-padding roots in terminal order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeTerminalRootsV1 {
    prior_transcript_binding: [u8; 32],
    cross_field_root: [u8; 32],
    global_lookup_root: [u8; 32],
    zero_padding_root: [u8; 32],
}

impl ZkAmsMkheRnsNativeTerminalRootsV1 {
    /// Construct terminal roots tagged for one exact post-qPCS transcript.
    ///
    /// # Errors
    ///
    /// Rejects zero or duplicated binding/root digests.
    pub fn new(
        prior_transcript_binding: [u8; 32],
        cross_field_root: [u8; 32],
        global_lookup_root: [u8; 32],
        zero_padding_root: [u8; 32],
    ) -> Result<Self, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        validate_distinct_digests_v1(&[
            prior_transcript_binding,
            cross_field_root,
            global_lookup_root,
            zero_padding_root,
        ])?;
        Ok(Self {
            prior_transcript_binding,
            cross_field_root,
            global_lookup_root,
            zero_padding_root,
        })
    }

    /// Return the cross-field proof root.
    #[must_use]
    pub const fn cross_field_root(self) -> [u8; 32] {
        self.cross_field_root
    }

    /// Return the committed global-lookup root.
    #[must_use]
    pub const fn global_lookup_root(self) -> [u8; 32] {
        self.global_lookup_root
    }

    /// Return the governed zero-padding root.
    #[must_use]
    pub const fn zero_padding_root(self) -> [u8; 32] {
        self.zero_padding_root
    }
}

/// Move-only transcript after canonical context/source validation.
#[allow(
    missing_copy_implementations,
    reason = "a transcript stage must not be copied or rewound"
)]
pub struct ZkAmsMkheRnsNativeTranscriptV1 {
    state: [u8; 32],
    digests: DigestRegistryV1,
    context_identities: TranscriptContextIdentitiesV1,
}

impl ZkAmsMkheRnsNativeTranscriptV1 {
    /// Start the sole transcript from canonical profile and typed source bindings.
    ///
    /// # Errors
    ///
    /// Rejects an invalid profile/topology/candidate identity, a receipt that
    /// does not validate against the supplied layout, or zero/duplicated
    /// semantic digests.
    pub fn new(
        source_layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
        public_context: ZkAmsMkheRnsNativePublicContextV1,
    ) -> Result<Self, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        source_layout
            .validate()
            .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidSourceContext)?;
        source_receipt
            .validate(source_layout)
            .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidSourceContext)?;

        let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()
            .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidProfileBinding)?;
        manifest
            .validate()
            .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidProfileBinding)?;
        let topology = zk_ams_mkhe_rns_native_topology_v1()
            .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidProfileBinding)?;
        topology
            .validate()
            .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidProfileBinding)?;
        let release_candidate = zk_ams_mkhe_rns_native_release_candidate_digest_v1()
            .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidProfileBinding)?;
        if manifest.profile_digest != source_layout.profile_digest()
            || manifest.proof_topology_digest != topology.topology_digest
            || topology.topology_digest != source_layout.topology_digest()
            || release_candidate != source_layout.release_candidate_digest()
        {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidProfileBinding);
        }

        let context_identities = TranscriptContextIdentitiesV1 {
            profile_manifest_digest: manifest.manifest_digest,
            profile_digest: manifest.profile_digest,
            topology_digest: topology.topology_digest,
            release_candidate_digest: release_candidate,
            statement_digest: source_layout.statement_digest(),
            operational_context_digest: source_layout.operational_context_digest(),
            source_binding_digest: source_layout.source_binding_digest(),
            main_snapshot_digest: source_receipt.main_snapshot_digest,
            nonce_snapshot_digest: source_receipt.nonce_snapshot_digest,
            source_receipt_digest: source_receipt.receipt_digest,
            governed_roster_digest: public_context.governed_roster_digest,
            public_ciphertext_digest: public_context.public_ciphertext_digest,
        };
        let mut digests = DigestRegistryV1::new();
        let mut state = initial_state_v1();
        for (ordinal, digest) in context_identities.ordered().into_iter().enumerate() {
            digests.insert(digest)?;
            let ordinal = u16::try_from(ordinal)
                .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidChallenge)?;
            state = absorb_digest_v1(state, AbsorbKindV1::Identity, ordinal, digest);
        }
        if state == [0; 32] {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidChallenge);
        }
        Ok(Self {
            state,
            digests,
            context_identities,
        })
    }

    /// Return the context binding required by the exact 43-opening bundle.
    #[must_use]
    pub const fn binding_digest(&self) -> [u8; 32] {
        self.state
    }

    /// Consume the context stage and bind all 43 ordered commitment pairs.
    ///
    /// # Errors
    ///
    /// Rejects a bundle tagged for another context, cross-stage digest reuse,
    /// or an invalid derived challenge.
    pub fn bind_opening_commitments(
        mut self,
        commitments: ZkAmsMkheRnsNativeOpeningCommitmentsV1,
    ) -> Result<ZkAmsMkheRnsNativeCommitmentsBoundTranscriptV1, ZkAmsMkheRnsNativeTranscriptErrorV1>
    {
        if commitments.prior_transcript_binding != self.state {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch);
        }
        let opening_commitments = commitments.records;
        for (ordinal, record) in opening_commitments.into_iter().enumerate() {
            self.digests.insert(record.source_commitment_digest)?;
            self.digests.insert(record.hyrax_commitment_digest)?;
            let ordinal = u8::try_from(ordinal)
                .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidOpeningOrder)?;
            self.state = absorb_opening_v1(self.state, ordinal, record);
        }
        let (state, mapping_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            0,
            ChallengePurposeV1::Mapping,
            0,
        )?;
        Ok(ZkAmsMkheRnsNativeCommitmentsBoundTranscriptV1 {
            state,
            digests: self.digests,
            context_identities: self.context_identities,
            opening_commitments,
            mapping_challenge_seed,
        })
    }
}

/// Move-only transcript after all source and Hyrax commitments are bound.
#[allow(
    missing_copy_implementations,
    reason = "a transcript stage must not be copied or rewound"
)]
pub struct ZkAmsMkheRnsNativeCommitmentsBoundTranscriptV1 {
    state: [u8; 32],
    digests: DigestRegistryV1,
    context_identities: TranscriptContextIdentitiesV1,
    opening_commitments: [ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1],
    mapping_challenge_seed: [u8; 32],
}

impl ZkAmsMkheRnsNativeCommitmentsBoundTranscriptV1 {
    /// Return the binding required by the mapping/cross-basis terminal bridge.
    #[must_use]
    pub const fn binding_digest(&self) -> [u8; 32] {
        self.state
    }

    /// Consume the commitment stage and bind the complete terminal bridge.
    ///
    /// # Errors
    ///
    /// Rejects a bridge tagged for another commitment transcript, cross-stage
    /// digest reuse, or an invalid derived challenge.
    pub fn bind_terminal_bridge(
        mut self,
        bridge: ZkAmsMkheRnsNativeTerminalBridgeV1,
    ) -> Result<ZkAmsMkheRnsNativeTerminalBoundTranscriptV1, ZkAmsMkheRnsNativeTranscriptErrorV1>
    {
        if bridge.prior_transcript_binding != self.state {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch);
        }
        for (ordinal, digest) in [bridge.mapping_root, bridge.terminal_hyrax_root]
            .into_iter()
            .enumerate()
        {
            self.digests.insert(digest)?;
            let ordinal = u16::try_from(ordinal)
                .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidChallenge)?;
            self.state =
                absorb_digest_v1(self.state, AbsorbKindV1::TerminalMapping, ordinal, digest);
        }
        let (state, cross_basis_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            1,
            ChallengePurposeV1::CrossBasis,
            0,
        )?;
        self.state = state;
        self.digests.insert(bridge.cross_basis_bridge_root)?;
        self.state = absorb_digest_v1(
            self.state,
            AbsorbKindV1::CrossBasisBridge,
            0,
            bridge.cross_basis_bridge_root,
        );
        let (state, rns_aggregation_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            2,
            ChallengePurposeV1::RnsAggregation,
            0,
        )?;
        Ok(ZkAmsMkheRnsNativeTerminalBoundTranscriptV1 {
            state,
            digests: self.digests,
            context_identities: self.context_identities,
            opening_commitments: self.opening_commitments,
            mapping_root: bridge.mapping_root,
            terminal_hyrax_root: bridge.terminal_hyrax_root,
            cross_basis_bridge_root: bridge.cross_basis_bridge_root,
            mapping_challenge_seed: self.mapping_challenge_seed,
            cross_basis_challenge_seed,
            rns_aggregation_challenge_seed,
        })
    }
}

/// Move-only transcript after the mapping and cross-basis bridge are bound.
#[allow(
    missing_copy_implementations,
    reason = "a transcript stage must not be copied or rewound"
)]
pub struct ZkAmsMkheRnsNativeTerminalBoundTranscriptV1 {
    state: [u8; 32],
    digests: DigestRegistryV1,
    context_identities: TranscriptContextIdentitiesV1,
    opening_commitments: [ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1],
    mapping_root: [u8; 32],
    terminal_hyrax_root: [u8; 32],
    cross_basis_bridge_root: [u8; 32],
    mapping_challenge_seed: [u8; 32],
    cross_basis_challenge_seed: [u8; 32],
    rns_aggregation_challenge_seed: [u8; 32],
}

impl ZkAmsMkheRnsNativeTerminalBoundTranscriptV1 {
    /// Return the binding required by the exact qPCS root schedule.
    #[must_use]
    pub const fn binding_digest(&self) -> [u8; 32] {
        self.state
    }

    /// Consume the terminal stage and bind all twenty qPCS roots.
    ///
    /// # Errors
    ///
    /// Rejects roots tagged for another terminal transcript, cross-stage
    /// digest reuse, or an invalid derived challenge.
    pub fn bind_qpcs_roots(
        mut self,
        roots: ZkAmsMkheRnsNativeQpcsRootsV1,
    ) -> Result<ZkAmsMkheRnsNativeQpcsBoundTranscriptV1, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        if roots.prior_transcript_binding != self.state {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch);
        }

        self.digests.insert(roots.initial_root)?;
        self.state = absorb_digest_v1(self.state, AbsorbKindV1::QpcsInitial, 0, roots.initial_root);
        let (state, qpcs_relation_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            3,
            ChallengePurposeV1::QpcsRelation,
            0,
        )?;
        self.state = state;

        self.digests.insert(roots.quotient_root)?;
        self.state = absorb_digest_v1(
            self.state,
            AbsorbKindV1::QpcsQuotient,
            0,
            roots.quotient_root,
        );
        let (state, qpcs_batching_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            4,
            ChallengePurposeV1::QpcsBatching,
            0,
        )?;
        self.state = state;

        let qpcs_fri_roots = roots.fri_roots;
        let mut qpcs_fri_fold_challenge_seeds = [[0_u8; 32]; FRI_ROOT_COUNT_V1];
        for (layer, root) in qpcs_fri_roots.into_iter().enumerate() {
            self.digests.insert(root.root)?;
            let layer_u8 = u8::try_from(layer)
                .map_err(|_| ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidRootOrder)?;
            self.state = absorb_digest_v1(
                self.state,
                AbsorbKindV1::QpcsFri,
                u16::from(layer_u8),
                root.root,
            );
            let (state, seed) = derive_registered_challenge_v1(
                self.state,
                &mut self.digests,
                5 + layer_u8,
                ChallengePurposeV1::QpcsFriFold,
                layer_u8,
            )?;
            self.state = state;
            qpcs_fri_fold_challenge_seeds[layer] = seed;
        }
        let (state, qpcs_query_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            23,
            ChallengePurposeV1::QpcsQuery,
            0,
        )?;
        self.state = state;
        let (state, cross_field_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            24,
            ChallengePurposeV1::CrossField,
            0,
        )?;
        Ok(ZkAmsMkheRnsNativeQpcsBoundTranscriptV1 {
            state,
            digests: self.digests,
            context_identities: self.context_identities,
            opening_commitments: self.opening_commitments,
            mapping_root: self.mapping_root,
            terminal_hyrax_root: self.terminal_hyrax_root,
            cross_basis_bridge_root: self.cross_basis_bridge_root,
            qpcs_initial_root: roots.initial_root,
            qpcs_quotient_root: roots.quotient_root,
            qpcs_fri_roots,
            mapping_challenge_seed: self.mapping_challenge_seed,
            cross_basis_challenge_seed: self.cross_basis_challenge_seed,
            rns_aggregation_challenge_seed: self.rns_aggregation_challenge_seed,
            qpcs_relation_challenge_seed,
            qpcs_batching_challenge_seed,
            qpcs_fri_fold_challenge_seeds,
            qpcs_query_challenge_seed,
            cross_field_challenge_seed,
        })
    }
}

/// Move-only transcript after all qPCS roots and challenges are bound.
#[allow(
    missing_copy_implementations,
    reason = "a transcript stage must not be copied or rewound"
)]
pub struct ZkAmsMkheRnsNativeQpcsBoundTranscriptV1 {
    state: [u8; 32],
    digests: DigestRegistryV1,
    context_identities: TranscriptContextIdentitiesV1,
    opening_commitments: [ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1],
    mapping_root: [u8; 32],
    terminal_hyrax_root: [u8; 32],
    cross_basis_bridge_root: [u8; 32],
    qpcs_initial_root: [u8; 32],
    qpcs_quotient_root: [u8; 32],
    qpcs_fri_roots: [ZkAmsMkheRnsNativeQpcsFriRootV1; FRI_ROOT_COUNT_V1],
    mapping_challenge_seed: [u8; 32],
    cross_basis_challenge_seed: [u8; 32],
    rns_aggregation_challenge_seed: [u8; 32],
    qpcs_relation_challenge_seed: [u8; 32],
    qpcs_batching_challenge_seed: [u8; 32],
    qpcs_fri_fold_challenge_seeds: [[u8; 32]; FRI_ROOT_COUNT_V1],
    qpcs_query_challenge_seed: [u8; 32],
    cross_field_challenge_seed: [u8; 32],
}

impl ZkAmsMkheRnsNativeQpcsBoundTranscriptV1 {
    /// Return the binding required by the three terminal roots.
    #[must_use]
    pub const fn binding_digest(&self) -> [u8; 32] {
        self.state
    }

    /// Consume the qPCS stage and finalize all terminal challenge bindings.
    ///
    /// # Errors
    ///
    /// Rejects roots tagged for another qPCS transcript, cross-stage digest
    /// reuse, or an invalid derived challenge.
    pub fn bind_terminal_roots(
        mut self,
        roots: ZkAmsMkheRnsNativeTerminalRootsV1,
    ) -> Result<ZkAmsMkheRnsNativeChallengeSeedsV1, ZkAmsMkheRnsNativeTranscriptErrorV1> {
        if roots.prior_transcript_binding != self.state {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ContextMismatch);
        }

        self.digests.insert(roots.cross_field_root)?;
        self.state = absorb_digest_v1(
            self.state,
            AbsorbKindV1::CrossField,
            0,
            roots.cross_field_root,
        );
        let (state, global_lookup_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            25,
            ChallengePurposeV1::GlobalLookup,
            0,
        )?;
        self.state = state;

        self.digests.insert(roots.global_lookup_root)?;
        self.state = absorb_digest_v1(
            self.state,
            AbsorbKindV1::GlobalLookup,
            0,
            roots.global_lookup_root,
        );
        let (state, zero_padding_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            26,
            ChallengePurposeV1::ZeroPadding,
            0,
        )?;
        self.state = state;

        self.digests.insert(roots.zero_padding_root)?;
        self.state = absorb_digest_v1(
            self.state,
            AbsorbKindV1::ZeroPadding,
            0,
            roots.zero_padding_root,
        );
        let (state, composite_binding_challenge_seed) = derive_registered_challenge_v1(
            self.state,
            &mut self.digests,
            27,
            ChallengePurposeV1::CompositeBinding,
            0,
        )?;
        Ok(ZkAmsMkheRnsNativeChallengeSeedsV1 {
            context_identities: self.context_identities,
            opening_commitments: self.opening_commitments,
            mapping_root: self.mapping_root,
            terminal_hyrax_root: self.terminal_hyrax_root,
            cross_basis_bridge_root: self.cross_basis_bridge_root,
            qpcs_initial_root: self.qpcs_initial_root,
            qpcs_quotient_root: self.qpcs_quotient_root,
            qpcs_fri_roots: self.qpcs_fri_roots,
            cross_field_root: roots.cross_field_root,
            global_lookup_root: roots.global_lookup_root,
            zero_padding_root: roots.zero_padding_root,
            mapping_challenge_seed: self.mapping_challenge_seed,
            cross_basis_challenge_seed: self.cross_basis_challenge_seed,
            rns_aggregation_challenge_seed: self.rns_aggregation_challenge_seed,
            qpcs_relation_challenge_seed: self.qpcs_relation_challenge_seed,
            qpcs_batching_challenge_seed: self.qpcs_batching_challenge_seed,
            qpcs_fri_fold_challenge_seeds: self.qpcs_fri_fold_challenge_seeds,
            qpcs_query_challenge_seed: self.qpcs_query_challenge_seed,
            cross_field_challenge_seed: self.cross_field_challenge_seed,
            global_lookup_challenge_seed,
            zero_padding_challenge_seed,
            composite_binding_challenge_seed,
            transcript_digest: state,
        })
    }
}

/// Final domain-separated challenge seeds and terminal transcript digest.
///
/// This record is a deterministic transcript result, not proof verification or
/// readiness authority.  It intentionally does not implement `Clone`.
#[derive(Debug, PartialEq, Eq)]
#[allow(
    missing_copy_implementations,
    reason = "the final challenge owner remains move-only like its producing transcript"
)]
pub struct ZkAmsMkheRnsNativeChallengeSeedsV1 {
    context_identities: TranscriptContextIdentitiesV1,
    opening_commitments: [ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1],
    mapping_root: [u8; 32],
    terminal_hyrax_root: [u8; 32],
    cross_basis_bridge_root: [u8; 32],
    qpcs_initial_root: [u8; 32],
    qpcs_quotient_root: [u8; 32],
    qpcs_fri_roots: [ZkAmsMkheRnsNativeQpcsFriRootV1; FRI_ROOT_COUNT_V1],
    cross_field_root: [u8; 32],
    global_lookup_root: [u8; 32],
    zero_padding_root: [u8; 32],
    mapping_challenge_seed: [u8; 32],
    cross_basis_challenge_seed: [u8; 32],
    rns_aggregation_challenge_seed: [u8; 32],
    qpcs_relation_challenge_seed: [u8; 32],
    qpcs_batching_challenge_seed: [u8; 32],
    qpcs_fri_fold_challenge_seeds: [[u8; 32]; FRI_ROOT_COUNT_V1],
    qpcs_query_challenge_seed: [u8; 32],
    cross_field_challenge_seed: [u8; 32],
    global_lookup_challenge_seed: [u8; 32],
    zero_padding_challenge_seed: [u8; 32],
    composite_binding_challenge_seed: [u8; 32],
    transcript_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeChallengeSeedsV1 {
    /// Canonical replacement profile-manifest identity absorbed first.
    #[must_use]
    pub const fn profile_manifest_digest(&self) -> [u8; 32] {
        self.context_identities.profile_manifest_digest
    }

    /// Canonical replacement profile identity.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.context_identities.profile_digest
    }

    /// Canonical replacement proof-topology identity.
    #[must_use]
    pub const fn topology_digest(&self) -> [u8; 32] {
        self.context_identities.topology_digest
    }

    /// Non-authorizing release-candidate identity.
    #[must_use]
    pub const fn release_candidate_digest(&self) -> [u8; 32] {
        self.context_identities.release_candidate_digest
    }

    /// Exact proved-statement identity.
    #[must_use]
    pub const fn statement_digest(&self) -> [u8; 32] {
        self.context_identities.statement_digest
    }

    /// Exact operational and replay-context identity.
    #[must_use]
    pub const fn operational_context_digest(&self) -> [u8; 32] {
        self.context_identities.operational_context_digest
    }

    /// Exact confidential-source layout identity.
    #[must_use]
    pub const fn source_binding_digest(&self) -> [u8; 32] {
        self.context_identities.source_binding_digest
    }

    /// Authenticated main source-snapshot identity.
    #[must_use]
    pub const fn main_snapshot_digest(&self) -> [u8; 32] {
        self.context_identities.main_snapshot_digest
    }

    /// Authenticated nonce source-snapshot identity.
    #[must_use]
    pub const fn nonce_snapshot_digest(&self) -> [u8; 32] {
        self.context_identities.nonce_snapshot_digest
    }

    /// Structural source-receipt identity.
    #[must_use]
    pub const fn source_receipt_digest(&self) -> [u8; 32] {
        self.context_identities.source_receipt_digest
    }

    /// Governed verifier-roster identity.
    #[must_use]
    pub const fn governed_roster_digest(&self) -> [u8; 32] {
        self.context_identities.governed_roster_digest
    }

    /// Public ciphertext/statement-material identity.
    #[must_use]
    pub const fn public_ciphertext_digest(&self) -> [u8; 32] {
        self.context_identities.public_ciphertext_digest
    }

    /// Borrow the exact 43 opening commitment pairs in transcript order.
    #[must_use]
    pub const fn opening_commitments(
        &self,
    ) -> &[ZkAmsMkheRnsNativeOpeningCommitmentV1; OPENING_COUNT_V1] {
        &self.opening_commitments
    }

    /// Root of the source-to-Hyrax mapping proof.
    #[must_use]
    pub const fn mapping_root(&self) -> [u8; 32] {
        self.mapping_root
    }

    /// Root of the terminal Hyrax proof.
    #[must_use]
    pub const fn terminal_hyrax_root(&self) -> [u8; 32] {
        self.terminal_hyrax_root
    }

    /// Root of the terminal cross-basis bridge.
    #[must_use]
    pub const fn cross_basis_bridge_root(&self) -> [u8; 32] {
        self.cross_basis_bridge_root
    }

    /// Initial qPCS committed-codeword root.
    #[must_use]
    pub const fn qpcs_initial_root(&self) -> [u8; 32] {
        self.qpcs_initial_root
    }

    /// qPCS quotient-opening root.
    #[must_use]
    pub const fn qpcs_quotient_root(&self) -> [u8; 32] {
        self.qpcs_quotient_root
    }

    /// Borrow the eighteen qPCS FRI roots in transcript order.
    #[must_use]
    pub const fn qpcs_fri_roots(&self) -> &[ZkAmsMkheRnsNativeQpcsFriRootV1; FRI_ROOT_COUNT_V1] {
        &self.qpcs_fri_roots
    }

    /// Root of the cross-field proof.
    #[must_use]
    pub const fn cross_field_root(&self) -> [u8; 32] {
        self.cross_field_root
    }

    /// Root of the committed global lookup.
    #[must_use]
    pub const fn global_lookup_root(&self) -> [u8; 32] {
        self.global_lookup_root
    }

    /// Root of the governed zero-padding proof.
    #[must_use]
    pub const fn zero_padding_root(&self) -> [u8; 32] {
        self.zero_padding_root
    }

    /// Challenge for the source-to-Hyrax mapping.
    #[must_use]
    pub const fn mapping_challenge_seed(&self) -> [u8; 32] {
        self.mapping_challenge_seed
    }

    /// Challenge for the terminal cross-basis bridge.
    #[must_use]
    pub const fn cross_basis_challenge_seed(&self) -> [u8; 32] {
        self.cross_basis_challenge_seed
    }

    /// Challenge aggregating the two RNS equations.
    #[must_use]
    pub const fn rns_aggregation_challenge_seed(&self) -> [u8; 32] {
        self.rns_aggregation_challenge_seed
    }

    /// qPCS relation/evaluation challenge.
    #[must_use]
    pub const fn qpcs_relation_challenge_seed(&self) -> [u8; 32] {
        self.qpcs_relation_challenge_seed
    }

    /// qPCS row-batching challenge.
    #[must_use]
    pub const fn qpcs_batching_challenge_seed(&self) -> [u8; 32] {
        self.qpcs_batching_challenge_seed
    }

    /// Borrow the eighteen FRI-fold challenges in layer order.
    #[must_use]
    pub const fn qpcs_fri_fold_challenge_seeds(&self) -> &[[u8; 32]; FRI_ROOT_COUNT_V1] {
        &self.qpcs_fri_fold_challenge_seeds
    }

    /// qPCS common-query challenge.
    #[must_use]
    pub const fn qpcs_query_challenge_seed(&self) -> [u8; 32] {
        self.qpcs_query_challenge_seed
    }

    /// Cross-field challenge after the complete qPCS schedule.
    #[must_use]
    pub const fn cross_field_challenge_seed(&self) -> [u8; 32] {
        self.cross_field_challenge_seed
    }

    /// Committed global-lookup challenge.
    #[must_use]
    pub const fn global_lookup_challenge_seed(&self) -> [u8; 32] {
        self.global_lookup_challenge_seed
    }

    /// Governed zero-padding challenge.
    #[must_use]
    pub const fn zero_padding_challenge_seed(&self) -> [u8; 32] {
        self.zero_padding_challenge_seed
    }

    /// Final composite-binding challenge after every root.
    #[must_use]
    pub const fn composite_binding_challenge_seed(&self) -> [u8; 32] {
        self.composite_binding_challenge_seed
    }

    /// Digest of the fully ratcheted transcript.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }

    /// Return all challenge seeds in their sole canonical derivation order.
    #[must_use]
    pub fn ordered_challenge_seeds(
        &self,
    ) -> [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_CHALLENGE_COUNT_V1] {
        let mut ordered = [[0_u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_CHALLENGE_COUNT_V1];
        ordered[0] = self.mapping_challenge_seed;
        ordered[1] = self.cross_basis_challenge_seed;
        ordered[2] = self.rns_aggregation_challenge_seed;
        ordered[3] = self.qpcs_relation_challenge_seed;
        ordered[4] = self.qpcs_batching_challenge_seed;
        ordered[5..23].copy_from_slice(&self.qpcs_fri_fold_challenge_seeds);
        ordered[23] = self.qpcs_query_challenge_seed;
        ordered[24] = self.cross_field_challenge_seed;
        ordered[25] = self.global_lookup_challenge_seed;
        ordered[26] = self.zero_padding_challenge_seed;
        ordered[27] = self.composite_binding_challenge_seed;
        ordered
    }
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum AbsorbKindV1 {
    Identity = 1,
    TerminalMapping = 2,
    CrossBasisBridge = 3,
    QpcsInitial = 4,
    QpcsQuotient = 5,
    QpcsFri = 6,
    CrossField = 7,
    GlobalLookup = 8,
    ZeroPadding = 9,
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum ChallengePurposeV1 {
    Mapping = 1,
    CrossBasis = 2,
    RnsAggregation = 3,
    QpcsRelation = 4,
    QpcsBatching = 5,
    QpcsFriFold = 6,
    QpcsQuery = 7,
    CrossField = 8,
    GlobalLookup = 9,
    ZeroPadding = 10,
    CompositeBinding = 11,
}

struct DigestRegistryV1 {
    digests: [[u8; 32]; MAX_REGISTERED_DIGESTS_V1],
    len: usize,
}

impl DigestRegistryV1 {
    const fn new() -> Self {
        Self {
            digests: [[0; 32]; MAX_REGISTERED_DIGESTS_V1],
            len: 0,
        }
    }

    fn insert(&mut self, digest: [u8; 32]) -> Result<(), ZkAmsMkheRnsNativeTranscriptErrorV1> {
        if digest == [0; 32] {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::ZeroDigest);
        }
        if self.digests[..self.len].contains(&digest) {
            return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::DuplicateDigest);
        }
        let destination = self
            .digests
            .get_mut(self.len)
            .ok_or(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidChallenge)?;
        *destination = digest;
        self.len += 1;
        Ok(())
    }
}

fn validate_distinct_digests_v1(
    digests: &[[u8; 32]],
) -> Result<(), ZkAmsMkheRnsNativeTranscriptErrorV1> {
    let mut registry = DigestRegistryV1::new();
    for digest in digests {
        registry.insert(*digest)?;
    }
    Ok(())
}

fn opening_role_v1(ordinal: usize) -> Option<(ZkAmsMkheRnsNativeFamilyV1, u8)> {
    match ordinal {
        0 => Some((ZkAmsMkheRnsNativeFamilyV1::X, 0)),
        1..=16 => Some((
            ZkAmsMkheRnsNativeFamilyV1::U,
            u8::try_from(ordinal - 1).ok()?,
        )),
        17..=32 => Some((
            ZkAmsMkheRnsNativeFamilyV1::E,
            u8::try_from(ordinal - 17).ok()?,
        )),
        33 => Some((ZkAmsMkheRnsNativeFamilyV1::RE, 0)),
        34..=41 => Some((
            ZkAmsMkheRnsNativeFamilyV1::W,
            u8::try_from(ordinal - 34).ok()?,
        )),
        42 => Some((ZkAmsMkheRnsNativeFamilyV1::RW, 0)),
        _ => None,
    }
}

fn initial_state_v1() -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(TRANSCRIPT_INITIAL_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_VERSION_V1]);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1]);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1]);
    hash.update(&CHALLENGE_COUNT_U16_V1.to_be_bytes());
    hash.finalize()
}

fn absorb_digest_v1(
    state: [u8; 32],
    kind: AbsorbKindV1,
    ordinal: u16,
    digest: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(TRANSCRIPT_ABSORB_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_VERSION_V1, kind as u8]);
    hash.update(&ordinal.to_be_bytes());
    hash.update(&state);
    hash.update(&digest);
    hash.finalize()
}

fn absorb_opening_v1(
    state: [u8; 32],
    ordinal: u8,
    opening: ZkAmsMkheRnsNativeOpeningCommitmentV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(TRANSCRIPT_OPENING_DOMAIN_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_VERSION_V1]);
    hash.update(&state);
    hash.update(&[ordinal, opening.family as u8, opening.family_index]);
    hash.update(&opening.source_commitment_digest);
    hash.update(&opening.hyrax_commitment_digest);
    hash.finalize()
}

fn derive_registered_challenge_v1(
    state: [u8; 32],
    digests: &mut DigestRegistryV1,
    ordinal: u8,
    purpose: ChallengePurposeV1,
    subindex: u8,
) -> Result<([u8; 32], [u8; 32]), ZkAmsMkheRnsNativeTranscriptErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(TRANSCRIPT_CHALLENGE_DOMAIN_V1);
    hash.update(&[
        ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_VERSION_V1,
        ordinal,
        purpose as u8,
        subindex,
    ]);
    hash.update(&state);
    let challenge = hash.finalize();
    if challenge == [0; 32] || challenge == state {
        return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidChallenge);
    }
    digests.insert(challenge)?;

    let mut ratchet = Keccak256::new();
    ratchet.update(TRANSCRIPT_RATCHET_DOMAIN_V1);
    ratchet.update(&[
        ZK_AMS_MKHE_RNS_NATIVE_TRANSCRIPT_VERSION_V1,
        ordinal,
        purpose as u8,
        subindex,
    ]);
    ratchet.update(&state);
    ratchet.update(&challenge);
    let next_state = ratchet.finalize();
    if next_state == [0; 32] || next_state == state || next_state == challenge {
        return Err(ZkAmsMkheRnsNativeTranscriptErrorV1::InvalidChallenge);
    }
    Ok((next_state, challenge))
}

#[cfg(test)]
#[path = "rns_native_transcript_tests.rs"]
mod tests;
