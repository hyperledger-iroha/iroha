fn reservation_group_identity(
    key: &crate::queue::LaneQueueReservationKeyV2,
) -> LaneQueueReservationGroupIdentityV1 {
    LaneQueueReservationGroupIdentityV1 {
        lane_id: key.lane_id,
        dataspace_id: key.dataspace_id,
        lane_incarnation: key.lane_incarnation,
        proposal_height: key.proposal_height,
        lane_block_height: key.lane_block_height,
        lane_block_view: key.lane_block_view,
        reservation_owner_hash: key.reservation_owner_hash,
        proposal_identity_hash: key.proposal_identity_hash,
    }
}
fn reservation_key_matches_group(
    key: &crate::queue::LaneQueueReservationKeyV2,
    group: &LaneQueueReservationGroupIdentityV1,
) -> bool {
    reservation_group_identity(key) == *group
}
fn invalid_historical_autonomous_recovery(
    input: &HistoricalAutonomousReservationInstallV1,
    detail: impl Into<String>,
) -> V2ReservationLifecycleError {
    V2ReservationLifecycleError::InvalidHistoricalAutonomousRecovery {
        recovery_id: input.recovery_id,
        detail: detail.into(),
    }
}
fn canonical_payload_contains_group_in_order(
    payload: &LaneExecutablePayloadV1,
    group: &LaneQueueReservationReconciliationGroupV1,
) -> bool {
    payload.reservation_keys == group.ordered_keys
}
fn autonomous_payload_overlaps_group_transaction_identity(
    payload: &LaneExecutablePayloadV1,
    group: &LaneQueueReservationReconciliationGroupV1,
) -> bool {
    payload.reservation_keys.iter().any(|candidate| {
        group.ordered_keys.iter().any(|expected| {
            candidate.signed_transaction_hash == expected.signed_transaction_hash
                || candidate.entrypoint_hash == expected.entrypoint_hash
        })
    })
}
fn proposal_from_canonical_lane_ownership(
    ownership: &SumeragiLanePayloadOwnership,
    block_hash: HashOf<BlockHeader>,
) -> Option<LaneBlockProposalV1> {
    let descriptor_hash = ownership.lane_block_descriptor_hash?;
    let descriptor = LaneBlockDescriptorV1 {
        lane_id: ownership.lane_id,
        dataspace_id: ownership.dataspace_id,
        lane_incarnation: ownership.lane_incarnation,
        proposal_height: ownership.proposal_height,
        previous_lane_block_height: ownership.previous_lane_block_height,
        previous_lane_block_descriptor_hash: ownership.previous_lane_block_descriptor_hash,
        lane_block_height: ownership.lane_block_height,
        lane_block_view: ownership.lane_block_view,
        subject_hash: ownership.subject_hash,
        payload_ownership_hash: ownership.payload_ownership_hash,
        rbc_instance_hash: ownership.rbc_instance_hash,
        accepted_candidate_indices: ownership.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
        validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&ownership.lane_block_descriptor_validator_set),
        validator_set: ownership.lane_block_descriptor_validator_set.clone(),
        validator_count: ownership.lane_block_descriptor_validator_count,
        min_quorum: ownership.lane_block_descriptor_min_quorum,
        qc_mode_tag: ownership.qc_mode_tag.clone(),
        descriptor_hash,
    };
    if descriptor.computed_descriptor_hash() != descriptor_hash {
        return None;
    }
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: Some(LaneBlockProposalPayloadHintV1 {
            proposal_height: ownership.proposal_height,
            proposal_view: ownership.proposal_view,
            proposal_block_hash: block_hash,
        }),
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    Some(proposal)
}
/// Immutable, finality-authenticated installation input for one unfinished
/// historical autonomous lane proposal.
///
/// This Encode-only value is an in-memory planner DTO, not a durable schema.
/// Kura publishes a separately versioned, decodeable recovery record only
/// after the referenced executable payload, historical PoPs, and execution
/// input are validated and durable. The Queue startup gate may then treat an
/// exact durable-record read-back as a persistent owner which the lane adapter
/// can hydrate after publication resumes.
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
pub(crate) struct HistoricalAutonomousReservationInstallV1 {
    /// Schema version of the installation identity.
    pub(crate) version: u16,
    /// Domain-separated digest of every remaining field.
    pub(crate) recovery_id: Hash,
    /// Exact finality/execution identity of the canonical global carrier.
    pub(crate) canonical_body: CanonicalExecutedBlockNeedV1,
    /// Complete frozen consensus context which authenticated the carrier.
    pub(crate) historical_context: wire::HeightContext,
    /// Redundant context identifier used by reservation-identity validation.
    pub(crate) historical_context_id: wire::HeightContextId,
    /// Hash of the complete historical context, including its roster. Validator
    /// PoPs are not carried by `HeightContext` and must be pinned separately by
    /// the durable installer.
    pub(crate) historical_context_hash: HashOf<wire::HeightContext>,
    /// Canonical global view of the carrier block.
    pub(crate) carrier_view: u64,
    /// Exact producer-authenticated payload, with its canonical global hint.
    pub(crate) payload: LaneExecutablePayloadV1,
    /// Exact FIFO-ordered Queue ownership group carried by the payload.
    pub(crate) reservation_group: LaneQueueReservationReconciliationGroupV1,
}
impl HistoricalAutonomousReservationInstallV1 {
    pub(crate) const VERSION: u16 = 1;
    const DIGEST_DOMAIN: &'static [u8] =
        b"iroha:sumeragi:historical-autonomous-reservation-recovery:v1\0";
    fn new(
        canonical_body: CanonicalExecutedBlockNeedV1,
        historical_context: wire::HeightContext,
        carrier_view: u64,
        payload: LaneExecutablePayloadV1,
        reservation_group: LaneQueueReservationReconciliationGroupV1,
    ) -> Self {
        let historical_context_id = historical_context.id();
        let historical_context_hash = HashOf::new(&historical_context);
        let mut install = Self {
            version: Self::VERSION,
            recovery_id: Hash::prehashed([0; Hash::LENGTH]),
            canonical_body,
            historical_context,
            historical_context_id,
            historical_context_hash,
            carrier_view,
            payload,
            reservation_group,
        };
        install.recovery_id = install.computed_recovery_id();
        install
    }
    /// Recompute the exact immutable record identity. Kura must reject any
    /// installation whose stored identity differs from this value.
    #[must_use]
    pub(crate) fn computed_recovery_id(&self) -> Hash {
        let mut canonical = self.clone();
        canonical.recovery_id = Hash::prehashed([0; Hash::LENGTH]);
        let identity: Hash = HashOf::new(&canonical).into();
        Hash::new_from_chunks(&[Self::DIGEST_DOMAIN, identity.as_ref()])
    }
    #[must_use]
    pub(crate) fn has_valid_identity(&self) -> bool {
        self.version == Self::VERSION
            && self.historical_context.id() == self.historical_context_id
            && HashOf::new(&self.historical_context) == self.historical_context_hash
            && self.computed_recovery_id() == self.recovery_id
    }
}
/// Durable publication result for one immutable historical autonomous record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoricalAutonomousLaneRecoveryInstallOutcome {
    /// Payload, execution input, and recovery record crossed their durability barriers.
    Installed,
    /// The exact complete record and both dependencies were already durable.
    AlreadyInstalled,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum CanonicalAutonomousCarrierDisposition {
    NotFinalized,
    Absent,
    /// A unique canonical autonomous envelope contains the complete executable
    /// payload needed to install historical certification work.
    ExactAutonomous(HistoricalAutonomousReservationInstallV1),
    /// The canonical body contains only ordinary ownership. This authenticates
    /// the proposal but cannot reconstruct autonomous executable bytes.
    ExactOrdinary,
}
impl CanonicalAutonomousCarrierDisposition {
    fn is_exact(&self) -> bool {
        matches!(self, Self::ExactAutonomous(_) | Self::ExactOrdinary)
    }
    fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}
enum CanonicalAutonomousCarrierInspection {
    Available(CanonicalAutonomousCarrierDisposition),
    MissingBody(CanonicalExecutedBlockNeedV1),
}
fn collect_canonical_executed_block_need(
    needs: &mut BTreeMap<u64, CanonicalExecutedBlockNeedV1>,
    need: CanonicalExecutedBlockNeedV1,
) -> Result<(), V2ReservationLifecycleError> {
    match needs.entry(need.height) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(need);
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &need => {}
        std::collections::btree_map::Entry::Occupied(_) => {
            return Err(V2ReservationLifecycleError::CanonicalContextMismatch {
                height: need.height,
            });
        }
    }
    Ok(())
}
