#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FairV2IngressWireKey {
    origin: PeerId,
    hash: CryptoHash,
}
/// Closed productive v2 ingress class carried only in node-local metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) enum FairV2IngressLeaderWireSourceClass {
    /// Proposal, vote, certificate, or timeout control.
    Control,
    /// One manifest-bound data-availability chunk.
    Chunk,
    /// One authenticated certified-body response.
    CertifiedResponse,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub(crate) enum FairV2IngressLeaderWirePhase {
    Proposal,
    PrepareVote,
    CommitVote,
    PrepareQc,
    CommitQc,
    TimeoutVote,
    TimeoutCertificate,
    Chunk,
    CertifiedResponse,
}
impl FairV2IngressLeaderWirePhase {
    const fn source_class(self) -> FairV2IngressLeaderWireSourceClass {
        match self {
            Self::Proposal
            | Self::PrepareVote
            | Self::CommitVote
            | Self::PrepareQc
            | Self::CommitQc
            | Self::TimeoutVote
            | Self::TimeoutCertificate => FairV2IngressLeaderWireSourceClass::Control,
            Self::Chunk => FairV2IngressLeaderWireSourceClass::Chunk,
            Self::CertifiedResponse => FairV2IngressLeaderWireSourceClass::CertifiedResponse,
        }
    }
    const fn code(self) -> u8 {
        match self {
            Self::Proposal => 0,
            Self::PrepareVote => 1,
            Self::CommitVote => 2,
            Self::PrepareQc => 3,
            Self::CommitQc => 4,
            Self::TimeoutVote => 5,
            Self::TimeoutCertificate => 6,
            Self::Chunk => 7,
            Self::CertifiedResponse => 8,
        }
    }
}
/// Finite semantic owner address for one productive leader wire.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct FairV2IngressLeaderWireSlot {
    semantic_origin: PeerId,
    phase: FairV2IngressLeaderWirePhase,
    chunk_index: Option<u32>,
}
/// Full immutable identity retained across queue, runtime, and durable cuts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct FairV2IngressLeaderWireIdentity {
    context_id: iroha_data_model::block::consensus_v2::HeightContextId,
    height: iroha_data_model::block::consensus_v2::Height,
    view: iroha_data_model::block::consensus_v2::View,
    subject_hash: CryptoHash,
    manifest_hash: Option<CryptoHash>,
    phase: FairV2IngressLeaderWirePhase,
    semantic_origin: PeerId,
    canonical_wire_hash: CryptoHash,
}
impl FairV2IngressLeaderWireIdentity {
    /// Stable route-neutral projection persisted by the downstream owner.
    pub(crate) fn projection_hash(&self) -> CryptoHash {
        let mut projection = Vec::new();
        projection.extend_from_slice(b"iroha:sumeragi:v2:leader-wire-lifecycle:v1");
        let context = self.context_id.encode();
        projection.extend_from_slice(
            &u64::try_from(context.len())
                .expect("height-context identity length fits u64")
                .to_le_bytes(),
        );
        projection.extend_from_slice(&context);
        projection.extend_from_slice(&self.height.to_le_bytes());
        projection.extend_from_slice(&self.view.to_le_bytes());
        projection.extend_from_slice(self.subject_hash.as_ref());
        projection.push(self.phase.code());
        let origin = self.semantic_origin.encode();
        projection.extend_from_slice(
            &u64::try_from(origin.len())
                .expect("semantic-origin identity length fits u64")
                .to_le_bytes(),
        );
        projection.extend_from_slice(&origin);
        match self.manifest_hash {
            None => projection.push(0),
            Some(hash) => {
                projection.push(1);
                projection.extend_from_slice(hash.as_ref());
            }
        }
        projection.extend_from_slice(self.canonical_wire_hash.as_ref());
        CryptoHash::new(projection)
    }
}
/// Exact internal reservation token attached to fair-ingress ownership.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[norito(deny_unknown_fields)]
pub(crate) struct FairV2IngressLeaderWireToken {
    identity: FairV2IngressLeaderWireIdentity,
    slot: FairV2IngressLeaderWireSlot,
    /// Immutable first-reservation position for this logical lifecycle.
    ///
    /// A restart retry retains this identity ordinal while its new physical
    /// fair-ingress carrier receives a fresh `FairV2IngressEntry` ordinal.
    admission_ordinal: u64,
    /// Actor-global producer/runtime scheduler position.
    scheduler_ordinal: u128,
    source_class: FairV2IngressLeaderWireSourceClass,
}
impl FairV2IngressLeaderWireToken {
    /// Stable route-neutral identity used by durable consumer receipts.
    pub(crate) fn identity_hash(&self) -> CryptoHash {
        self.identity.projection_hash()
    }
    /// Immutable first reservation ordinal.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const fn admission_ordinal(&self) -> u64 {
        self.admission_ordinal
    }
    /// Immutable shared scheduler position retained through restart.
    pub(crate) const fn scheduler_ordinal(&self) -> u128 {
        self.scheduler_ordinal
    }
    /// Proposal view retained by this exact productive wire.
    pub(crate) const fn view(&self) -> iroha_data_model::block::consensus_v2::View {
        self.identity.view
    }
    /// Whether this token is the exact chunk lifecycle for one manifest hash.
    pub(crate) fn matches_chunk_manifest(
        &self,
        manifest_hash: HashOf<iroha_data_model::block::consensus_v2::PayloadManifest>,
    ) -> bool {
        self.identity.phase == FairV2IngressLeaderWirePhase::Chunk
            && self.source_class == FairV2IngressLeaderWireSourceClass::Chunk
            && self.identity.manifest_hash == Some(manifest_hash.into())
    }
    /// Whether this chunk token names the exact proposal coordinates.
    pub(crate) fn matches_body_coordinates(
        &self,
        round: iroha_data_model::block::consensus_v2::ConsensusRound,
        subject: iroha_data_model::block::consensus_v2::BlockSubject,
    ) -> bool {
        self.identity.phase == FairV2IngressLeaderWirePhase::Chunk
            && self.source_class == FairV2IngressLeaderWireSourceClass::Chunk
            && self.identity.context_id == round.context_id
            && self.identity.height == round.height
            && self.identity.view == round.view
            && self.identity.subject_hash == fair_v2_ingress_subject_hash(Some(&subject))
    }
    /// Whether this chunk token names one exact proposal body.
    pub(crate) fn matches_exact_body(
        &self,
        round: iroha_data_model::block::consensus_v2::ConsensusRound,
        subject: iroha_data_model::block::consensus_v2::BlockSubject,
        manifest_hash: HashOf<iroha_data_model::block::consensus_v2::PayloadManifest>,
    ) -> bool {
        self.matches_body_coordinates(round, subject) && self.matches_chunk_manifest(manifest_hash)
    }
    /// Validate the complete context-bound token against configured geometry.
    pub(crate) fn validate_exact(
        &self,
        context_id: iroha_data_model::block::consensus_v2::HeightContextId,
        height: iroha_data_model::block::consensus_v2::Height,
        roster: &BTreeSet<PeerId>,
        max_chunk_count: u32,
    ) -> bool {
        let manifest_shape_exact = match self.identity.phase {
            FairV2IngressLeaderWirePhase::Proposal
            | FairV2IngressLeaderWirePhase::CertifiedResponse => {
                self.identity.manifest_hash.is_some() && self.slot.chunk_index.is_none()
            }
            FairV2IngressLeaderWirePhase::Chunk => {
                self.identity.manifest_hash.is_some()
                    && self
                        .slot
                        .chunk_index
                        .is_some_and(|index| index < max_chunk_count)
            }
            FairV2IngressLeaderWirePhase::PrepareVote
            | FairV2IngressLeaderWirePhase::CommitVote
            | FairV2IngressLeaderWirePhase::PrepareQc
            | FairV2IngressLeaderWirePhase::CommitQc
            | FairV2IngressLeaderWirePhase::TimeoutVote
            | FairV2IngressLeaderWirePhase::TimeoutCertificate => {
                self.identity.manifest_hash.is_none() && self.slot.chunk_index.is_none()
            }
        };
        self.admission_ordinal != 0
            && self.scheduler_ordinal != 0
            && self.identity.context_id == context_id
            && self.identity.height == height
            && roster.contains(&self.identity.semantic_origin)
            && self.slot.semantic_origin == self.identity.semantic_origin
            && self.slot.phase == self.identity.phase
            && self.source_class == self.identity.phase.source_class()
            && manifest_shape_exact
    }
}
