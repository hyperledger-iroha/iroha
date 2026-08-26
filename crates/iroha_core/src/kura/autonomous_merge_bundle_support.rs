/// Complete hash-addressed evidence required to execute one autonomous lane
/// block in a canonical merge batch on a validator that missed original
/// committee fanout.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub(crate) struct AutonomousLaneMergeBundleV1 {
    /// Bundle schema version. Only version one is accepted.
    pub(crate) version: u8,
    /// Immutable payload, origin availability proof, and authenticated cursor chain.
    pub(crate) autonomous: AutonomousLaneBlockArtifact,
    /// Immutable origin proposal with prepare/commit QCs and signer PoPs.
    pub(crate) certified: CertifiedLaneBlockArtifact,
}
impl AutonomousLaneMergeBundleV1 {
    /// Exact coordinated first-release layout accepted by Kura and merge transport.
    pub(crate) const VERSION: u8 = 1;
    /// Stable persistence label for the independently durable canonical bundle pair.
    pub(crate) const FORMAT_LABEL: &'static str = "lane.autonomous_merge_bundle.v1";
    /// Canonical framed bytes used by authenticated bundle transport and merge logs.
    pub(crate) fn encode_framed(&self) -> Result<Vec<u8>> {
        let bytes = norito::encode_canonical(self)?;
        if bytes.len() > MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES {
            return Err(Error::NoritoFrame(norito::Error::Message(
                "autonomous lane merge bundle exceeds hard byte limit".to_owned(),
            )));
        }
        Ok(bytes)
    }
    /// Domain-separated digest committed by canonical merge batches.
    pub(crate) fn bundle_hash(&self) -> Result<Hash> {
        let bytes = self.encode_framed()?;
        Ok(Hash::new_from_chunks(&[
            b"iroha:nexus:autonomous-lane-merge-bundle:v1\0",
            &bytes,
        ]))
    }
    /// Exact producer-authenticated executable payload.
    pub(crate) const fn executable_payload(&self) -> &LaneExecutablePayloadV1 {
        &self.autonomous.executable_payload
    }
}
/// Exact durable autonomous source admitted to canonical merge construction.
///
/// Construction requires both the independently durable canonical bundle
/// data/index slot and its separately durable autonomous attempt, READY
/// certificate, certified slot, and execution-input slot. The bundle bytes
/// must exactly reconstruct from those components under active lane geometry;
/// neither the persisted copy nor the derived view is trusted on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DurableAutonomousLaneMergeSource {
    /// Fully authenticated producer payload and lane certificate.
    pub(crate) bundle: AutonomousLaneMergeBundleV1,
    /// Exact canonical bytes carried unchanged into the merge transcript.
    pub(crate) source_bundle: Vec<u8>,
    /// Domain-separated digest of `source_bundle`.
    pub(crate) bundle_hash: Hash,
    /// Exact independently durable execution input used for deterministic replay.
    pub(crate) input: LaneBlockExecutionInputArtifact,
}
/// Move-only authority to sign one autonomous lane READY vote.
///
/// Kura is the only module that can construct this value. Construction follows
/// a canonical, repair-disabled execution-input readback and binds the exact
/// durable artifact to its proposal, executable payload, FIFO reservation
/// group, validator, and height-context session. The lane signer consumes the
/// value, so retaining an in-memory READY body is not sufficient authority.
#[must_use = "a durable lane READY authorization must be consumed by the exact signer session"]
pub(crate) struct LaneReadyAuthorization {
    durable_execution_input_hash: Hash,
    proposal: LaneBlockProposalV1,
    availability_body: LanePayloadAvailabilityBodyV1,
    reservation_group: LaneQueueReservationGroupBindingV1,
    producer: PeerId,
    signer: PeerId,
    height_context_id: HeightContextId,
}
/// Move-only authority for the first durable autonomous execution-input write.
///
/// The authority is minted from the exact repair-disabled executable payload
/// and reconstructed input. It binds the canonical reservation group and one
/// authenticated committee actor, then is consumed immediately before the
/// indexed Kura sidecar append.
#[must_use = "an autonomous execution-input authorization must be consumed by Kura"]
struct AutonomousLaneExecutionInputPersistenceAuthorization {
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    input: LaneBlockExecutionInputArtifact,
}
impl AutonomousLaneExecutionInputPersistenceAuthorization {
    fn matches_input(&self, input: &LaneBlockExecutionInputArtifact) -> bool {
        self.input == *input
    }
    fn consume_for_persistence(
        self,
        input: &LaneBlockExecutionInputArtifact,
    ) -> Option<ProductionInFlightFirstReleaseTransitionProjection> {
        self.matches_input(input).then_some(self.projection)
    }
}
/// Move-only authority for the exact autonomous READY-QC Kura write.
///
/// Construction validates the certificate against the immutable executable
/// payload and constructs the complete first-release `PersistReadyQc`
/// projection. The Kura writer consumes this value against the same payload
/// and certificate, checks the projection, and only then publishes the durable
/// view state.
#[must_use = "an autonomous READY-QC authorization must be consumed by Kura"]
struct AutonomousLaneReadyQcPersistenceAuthorization {
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
    executable_payload_hash: Hash,
    origin_proposal_hash: Hash,
    reservation_group: LaneQueueReservationGroupBindingV1,
    certificate: DurableLanePayloadAvailabilityCertificateV1,
}
impl AutonomousLaneReadyQcPersistenceAuthorization {
    fn consume_for_persistence(
        self,
        payload: &LaneExecutablePayloadV1,
        certificate: &DurableLanePayloadAvailabilityCertificateV1,
    ) -> Option<ProductionInFlightFirstReleaseTransitionProjection> {
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
                .ok()?;
        if self.network_id != payload.network_id
            || self.epoch != payload.epoch
            || self.executable_payload_hash != payload.payload_hash
            || self.origin_proposal_hash != payload.origin_proposal.proposal_hash
            || self.reservation_group != reservation_group
            || self.certificate != *certificate
        {
            return None;
        }
        Some(self.projection)
    }
}
/// Move-only authority for the first durable autonomous lane-Commit write.
///
/// Kura mints this value only from the exact repair-disabled merge source that
/// joins the immutable executable payload, durable execution input, READY QC,
/// and Prepare/Commit certificate. The certified-session writer matches the
/// complete artifact again and consumes the checked `LaneCommit` projection
/// before publishing either the latest frontier or the indexed certificate.
#[must_use = "an autonomous lane-Commit authorization must be consumed by Kura"]
struct AutonomousLaneCommitPersistenceAuthorization {
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    certified: CertifiedLaneBlockArtifact,
}
impl AutonomousLaneCommitPersistenceAuthorization {
    fn matches_artifact(&self, artifact: &CertifiedLaneBlockArtifact) -> bool {
        self.certified == *artifact
    }
    fn consume_for_persistence(
        self,
        artifact: &CertifiedLaneBlockArtifact,
    ) -> Option<ProductionInFlightFirstReleaseTransitionProjection> {
        self.matches_artifact(artifact).then_some(self.projection)
    }
}
/// Move-only authority for the first durable autonomous slot-retirement write.
///
/// The authority binds the exact authenticated payload, ordered reservation
/// group, retirement identity, and view-state path to one accepted
/// `PersistKuraRetirement` projection. Exact retries are storage stutters and
/// do not manufacture a second transition authority.
#[must_use = "an autonomous slot-retirement authorization must be consumed by Kura"]
struct AutonomousLaneSlotRetirementPersistenceAuthorization {
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    executable_payload_hash: Hash,
    origin_proposal_hash: Hash,
    reservation_group: LaneQueueReservationGroupBindingV1,
    retirement: AutonomousLaneSlotRetirementV1,
    view_state_path: PathBuf,
}
impl AutonomousLaneSlotRetirementPersistenceAuthorization {
    fn consume_for_persistence(
        self,
        payload: &LaneExecutablePayloadV1,
        retirement: &AutonomousLaneSlotRetirementV1,
        view_state_path: &Path,
    ) -> Option<ProductionInFlightFirstReleaseTransitionProjection> {
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
                .ok()?;
        if self.executable_payload_hash != payload.payload_hash
            || self.origin_proposal_hash != payload.origin_proposal.proposal_hash
            || self.reservation_group != reservation_group
            || self.retirement != *retirement
            || self.view_state_path.as_path() != view_state_path
        {
            return None;
        }
        Some(self.projection)
    }
}
/// Exact durable Queue release phase observed by startup reconciliation.
///
/// This is deliberately process-local rather than a persistence layout. Queue
/// owns the durable phase journal; Kura uses the typed observation only to
/// select the one complete formal state compatible with its independently
/// authenticated retirement and entrypoint-claim prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AutonomousLaneRetirementQueueSnapshotPhaseV1 {
    /// Queue durably owns the ordered release barrier while the matching
    /// reservations remain excluded from ordinary FIFO ownership.
    Prepared,
    /// Queue durably owns the release completion while FIFO restoration and
    /// completion forgetting remain pending after restart.
    Completed,
}
/// Immutable payload/committee identity paired with a signed lifecycle cursor.
///
/// The anchor is non-authorizing data extracted from Kura's exact durable
/// payload. Startup Queue recovery must compare every field with the
/// independently signature-validated lifecycle projection for the same
/// reservation group; the retirement evidence alone does not replace that
/// signed identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AutonomousLaneRetirementSnapshotAttemptAnchorV1 {
    origin_proposal_hash: Hash,
    executable_payload_hash: Hash,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_count: u8,
    producer_index: u16,
    local_actor_index: u16,
}
impl AutonomousLaneRetirementSnapshotAttemptAnchorV1 {
    /// Construct exact immutable anchor facts for Queue adversarial tests.
    ///
    /// Production code can obtain this type only from Kura's authenticated
    /// retirement snapshot. Keeping the test seam here preserves that opacity
    /// while rejecting indices which would make actor projection shift outside
    /// the bounded refinement width.
    #[cfg(test)]
    pub(crate) fn from_exact_parts_for_test(
        origin_proposal_hash: Hash,
        executable_payload_hash: Hash,
        validator_set_hash_version: u16,
        validator_set_hash: HashOf<Vec<PeerId>>,
        validator_count: u8,
        producer_index: u16,
        local_actor_index: u16,
    ) -> Result<Self, &'static str> {
        if validator_count == 0
            || validator_count > 128
            || producer_index >= u16::from(validator_count)
            || local_actor_index >= u16::from(validator_count)
        {
            return Err("retirement snapshot test anchor has out-of-bounds actor geometry");
        }
        Ok(Self {
            origin_proposal_hash,
            executable_payload_hash,
            validator_set_hash_version,
            validator_set_hash,
            validator_count,
            producer_index,
            local_actor_index,
        })
    }
    /// Hash of the immutable origin proposal authenticated by the payload.
    #[must_use]
    pub(crate) const fn origin_proposal_hash(&self) -> Hash {
        self.origin_proposal_hash
    }
    /// Hash of the exact producer-authenticated executable payload.
    #[must_use]
    pub(crate) const fn executable_payload_hash(&self) -> Hash {
        self.executable_payload_hash
    }
    /// Versioned ordered validator-set identity and bounded member count.
    #[must_use]
    pub(crate) const fn validator_set_identity(&self) -> (u16, HashOf<Vec<PeerId>>, u8) {
        (
            self.validator_set_hash_version,
            self.validator_set_hash,
            self.validator_count,
        )
    }
    /// Producer and local Kura release actor indices in the ordered validator set.
    #[must_use]
    pub(crate) const fn actor_indices(&self) -> (u16, u16) {
        (self.producer_index, self.local_actor_index)
    }
    /// One-hot producer and local Kura release actors used by the formal state.
    #[must_use]
    pub(crate) fn actor_projections(&self) -> (u128, u128) {
        (
            1_u128 << u32::from(self.producer_index),
            1_u128 << u32::from(self.local_actor_index),
        )
    }
}
/// Opaque, move-only Kura proof for one release-barrier Queue snapshot group.
///
/// Only Kura's bounded, repair-disabled authenticated read can construct this
/// value. It binds the exact payload, retirement, FIFO-ordered reservation
/// group, Queue phase, committee anchor, and complete on-disk claim prefix to
/// one valid composed first-release state. The startup planner may lend these
/// immutable facts to Queue, but Queue must still pair them with the matching
/// signed lifecycle cursor before action-25 recovery.
#[must_use = "authenticated retirement snapshot evidence must be consumed by startup recovery"]
pub(crate) struct AutonomousLaneRetirementSnapshotEvidenceV1 {
    phase: AutonomousLaneRetirementQueueSnapshotPhaseV1,
    reservation_group: LaneQueueReservationGroupBindingV1,
    retirement_hash: Hash,
    attempt_anchor: AutonomousLaneRetirementSnapshotAttemptAnchorV1,
    recovered_state: ProductionInFlightFirstReleaseStateProjection,
}
impl AutonomousLaneRetirementSnapshotEvidenceV1 {
    /// Construct exact opaque evidence parts for Queue adversarial tests.
    ///
    /// This does not emulate Kura authentication and is deliberately absent
    /// from production builds. Tests use it only with an attempt anchor from
    /// the bounded test constructor above.
    #[cfg(test)]
    pub(crate) fn from_exact_parts_for_test(
        phase: AutonomousLaneRetirementQueueSnapshotPhaseV1,
        reservation_group: LaneQueueReservationGroupBindingV1,
        retirement_hash: Hash,
        attempt_anchor: AutonomousLaneRetirementSnapshotAttemptAnchorV1,
        recovered_state: ProductionInFlightFirstReleaseStateProjection,
    ) -> Self {
        Self {
            phase,
            reservation_group,
            retirement_hash,
            attempt_anchor,
            recovered_state,
        }
    }
    /// Durable Queue release phase against which this Kura proof was minted.
    #[must_use]
    pub(crate) const fn phase(&self) -> AutonomousLaneRetirementQueueSnapshotPhaseV1 {
        self.phase
    }
    /// Complete FIFO-ordered reservation-group binding authenticated by Kura.
    #[must_use]
    pub(crate) const fn reservation_group(&self) -> LaneQueueReservationGroupBindingV1 {
        self.reservation_group
    }
    /// Digest of the exact durable slot retirement.
    #[must_use]
    pub(crate) const fn retirement_hash(&self) -> Hash {
        self.retirement_hash
    }
    /// Payload and committee facts which must match the signed lifecycle cursor.
    #[must_use]
    pub(crate) const fn attempt_anchor(&self) -> AutonomousLaneRetirementSnapshotAttemptAnchorV1 {
        self.attempt_anchor
    }
    /// Complete current formal state selected from Queue phase and Kura claims.
    #[must_use]
    pub(crate) const fn recovered_state(&self) -> ProductionInFlightFirstReleaseStateProjection {
        self.recovered_state
    }
}
/// Move-only authority for one exact ordered claim-prefix replacement.
///
/// Claim recovery validates the whole on-disk group before constructing these
/// values. Each value then binds one accepted abstract prefix advance to the
/// exact path and replacement bytes consumed by the atomic Kura sink.
#[must_use = "an autonomous claim-prefix authorization must be consumed by Kura"]
struct AutonomousLaneEntrypointClaimTransitionAuthorization {
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    path: PathBuf,
    replacement: AutonomousLaneEntrypointClaimV1,
}
/// Move-only authority for Queue's exact ordered `PrepareRelease` append.
///
/// Kura constructs this value only after revalidating the durable retirement
/// and the complete canonical `ReleasePending`/`Released` claim prefix. Queue
/// consumes it against the byte-identical barrier immediately before its
/// journal sink. `claims_fully_released` distinguishes a terminal retry after
/// Queue has already forgotten the barrier from an impossible missing-barrier
/// state while Kura still owns pending claims.
#[must_use = "a Kura-authenticated release preparation must be consumed by Queue"]
pub(crate) struct AutonomousLaneQueueReleasePreparationAuthorization {
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    barrier: LaneQueueReservationReleaseBarrierV1,
    claims_fully_released: bool,
}
impl AutonomousLaneQueueReleasePreparationAuthorization {
    /// Consume this proof for the exact Queue barrier it authorizes.
    pub(crate) fn consume_for_queue(
        self,
        barrier: &LaneQueueReservationReleaseBarrierV1,
    ) -> Option<(ProductionInFlightFirstReleaseTransitionProjection, bool)> {
        (self.barrier == *barrier).then_some((self.projection, self.claims_fully_released))
    }
}
/// Move-only authority for Queue's remaining release-side mutations.
///
/// Kura mints this value only after every exact claim is durably `Released` or
/// authenticatedly superseded. It carries separately checked projections for
/// the durable completion append, volatile FIFO publication, and durable
/// completion forget, keeping each authority adjacent to its physical sink.
#[must_use = "a Kura-authenticated release finalization must be consumed by Queue"]
pub(crate) struct AutonomousLaneQueueReleaseFinalizationAuthorization {
    barrier: LaneQueueReservationReleaseBarrierV1,
    complete_projection: ProductionInFlightFirstReleaseTransitionProjection,
    restore_projection: ProductionInFlightFirstReleaseTransitionProjection,
    forget_projection: ProductionInFlightFirstReleaseTransitionProjection,
}
impl AutonomousLaneQueueReleaseFinalizationAuthorization {
    /// Consume this proof for the exact Queue barrier it authorizes.
    pub(crate) fn consume_for_queue(
        self,
        barrier: &LaneQueueReservationReleaseBarrierV1,
    ) -> Option<[ProductionInFlightFirstReleaseTransitionProjection; 3]> {
        (self.barrier == *barrier).then_some([
            self.complete_projection,
            self.restore_projection,
            self.forget_projection,
        ])
    }
}
/// Queue evidence accepted at Kura's `ReleasePending -> Released` boundary.
enum AutonomousLaneQueueReleaseBarrierGate {
    Authorized(DurableLaneQueueReleaseBarrierAuthorization),
    #[cfg(test)]
    DirectTest,
}
impl AutonomousLaneQueueReleaseBarrierGate {
    fn consume_for_claim_transition(
        self,
        barrier: &LaneQueueReservationReleaseBarrierV1,
    ) -> std::result::Result<bool, &'static str> {
        match self {
            Self::Authorized(authorization) => authorization
                .consume_for_kura(barrier)
                .ok_or("Queue release-barrier authority names another exact barrier"),
            #[cfg(test)]
            Self::DirectTest => Ok(false),
        }
    }
}
impl AutonomousLaneEntrypointClaimTransitionAuthorization {
    fn consume_for_persistence(
        self,
        path: &Path,
        replacement: &AutonomousLaneEntrypointClaimV1,
    ) -> Option<ProductionInFlightFirstReleaseTransitionProjection> {
        (self.path.as_path() == path && self.replacement == *replacement).then_some(self.projection)
    }
}
/// Exact concrete facts used to project Kura's autonomous release protocol.
#[derive(Clone, Copy)]
struct AutonomousLaneReleaseProjectionContext {
    validator_count: u8,
    validator_mask: u128,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    producer_index: u16,
    actor_index: u16,
    producer: u128,
    actor: u128,
    payload_owners: u128,
    reservation_group: LaneQueueReservationGroupBindingV1,
    retirement_hash: Hash,
}
impl AutonomousLaneReleaseProjectionContext {
    fn from_payload(
        kura: &Kura,
        payload: &LaneExecutablePayloadV1,
        retirement: &AutonomousLaneSlotRetirementV1,
    ) -> std::result::Result<Self, String> {
        payload
            .validate(payload.network_id, payload.epoch)
            .map_err(|error| error.to_string())?;
        if !retirement.matches_payload(payload) {
            return Err("autonomous release retirement differs from its payload".to_owned());
        }
        let descriptor = &payload.origin_proposal.descriptor;
        let validator_count = u8::try_from(descriptor.validator_set.len())
            .map_err(|_| "autonomous release committee exceeds the refinement width".to_owned())?;
        if validator_count == 0 || validator_count > 128 {
            return Err(
                "autonomous release committee is outside the 1..=128 refinement width".to_owned(),
            );
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let producer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &payload.producer)
            .ok_or_else(|| "autonomous release producer is absent from its committee".to_owned())?;
        let producer_index_u16 = u16::try_from(producer_index).map_err(|_| {
            "autonomous release producer index exceeds the lifecycle width".to_owned()
        })?;
        let producer = 1_u128
            .checked_shl(u32::try_from(producer_index).map_err(|_| {
                "autonomous release producer index exceeds the refinement width".to_owned()
            })?)
            .ok_or_else(|| {
                "autonomous release producer index exceeds the refinement width".to_owned()
            })?;
        // The local committee member is the physical writer when available.
        // A non-committee recovery node projects the producer-authenticated
        // durable payload as the logical custody witness, matching the other
        // autonomous Kura persistence boundaries.
        let actor_peer = kura
            .local_peer_id
            .get()
            .filter(|peer| descriptor.validator_set.contains(*peer))
            .unwrap_or(&payload.producer);
        let actor_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == actor_peer)
            .ok_or_else(|| "autonomous release actor is absent from its committee".to_owned())?;
        let actor_index_u16 = u16::try_from(actor_index)
            .map_err(|_| "autonomous release actor index exceeds the lifecycle width".to_owned())?;
        let actor = 1_u128
            .checked_shl(u32::try_from(actor_index).map_err(|_| {
                "autonomous release actor index exceeds the refinement width".to_owned()
            })?)
            .ok_or_else(|| {
                "autonomous release actor index exceeds the refinement width".to_owned()
            })?;
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
                .map_err(|_| "autonomous release reservation group is not canonical".to_owned())?;
        let selected_count = reservation_group.reservation_count;
        let entrypoint_count = u64::try_from(payload.entrypoint_hashes.len())
            .map_err(|_| "autonomous release entrypoint count exceeds u64".to_owned())?;
        if selected_count != entrypoint_count
            || !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
                .unwrap_or(u64::MAX))
                .contains(&selected_count)
        {
            return Err(
                "autonomous release reservation count is outside the first-release bound"
                    .to_owned(),
            );
        }
        let payload_owners = producer | actor;
        if payload_owners & !validator_mask != 0 {
            return Err("autonomous release ownership exceeds its committee".to_owned());
        }
        let retirement_hash = retirement
            .digest()
            .map_err(|error| format!("cannot hash autonomous release retirement: {error:?}"))?;
        Ok(Self {
            validator_count,
            validator_mask,
            validator_set_hash_version: descriptor.validator_set_hash_version,
            validator_set_hash: descriptor.validator_set_hash,
            producer_index: producer_index_u16,
            actor_index: actor_index_u16,
            producer,
            actor,
            payload_owners,
            reservation_group,
            retirement_hash,
        })
    }
    fn retirement_snapshot_evidence(
        self,
        payload: &LaneExecutablePayloadV1,
        phase: AutonomousLaneRetirementQueueSnapshotPhaseV1,
        pending_prefix: u64,
        released_prefix: u64,
    ) -> std::result::Result<AutonomousLaneRetirementSnapshotEvidenceV1, String> {
        let selected_count = self.reservation_group.reservation_count;
        if pending_prefix != selected_count || released_prefix > pending_prefix {
            return Err(
                "autonomous retirement snapshot has a noncanonical claim prefix".to_owned(),
            );
        }
        let reservation_state = match phase {
            AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared => {
                IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED
            }
            AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed => {
                if released_prefix != selected_count {
                    return Err(
                        "completed Queue release requires every exact claim to be Released"
                            .to_owned(),
                    );
                }
                IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED
            }
        };
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(self.reservation_group);
        let recovered_state = self.state_with_fifo(
            binding_a,
            reservation_state,
            true,
            pending_prefix,
            released_prefix,
            false,
        );
        if !production_in_flight_first_release_state_kernel(recovered_state) {
            return Err(
                "autonomous retirement snapshot failed the composed first-release state gate"
                    .to_owned(),
            );
        }
        Ok(AutonomousLaneRetirementSnapshotEvidenceV1 {
            phase,
            reservation_group: self.reservation_group,
            retirement_hash: self.retirement_hash,
            attempt_anchor: AutonomousLaneRetirementSnapshotAttemptAnchorV1 {
                origin_proposal_hash: payload.origin_proposal.proposal_hash,
                executable_payload_hash: payload.payload_hash,
                validator_set_hash_version: self.validator_set_hash_version,
                validator_set_hash: self.validator_set_hash,
                validator_count: self.validator_count,
                producer_index: self.producer_index,
                local_actor_index: self.actor_index,
            },
            recovered_state,
        })
    }
    fn state(
        self,
        binding_a: CanonicalIdentityProjection,
        reservation_state: u8,
        kura_retired: bool,
        pending_prefix: u64,
        released_prefix: u64,
    ) -> ProductionInFlightFirstReleaseStateProjection {
        self.state_with_fifo(
            binding_a,
            reservation_state,
            kura_retired,
            pending_prefix,
            released_prefix,
            false,
        )
    }
    fn state_with_fifo(
        self,
        binding_a: CanonicalIdentityProjection,
        reservation_state: u8,
        kura_retired: bool,
        pending_prefix: u64,
        released_prefix: u64,
        fifo_restored: bool,
    ) -> ProductionInFlightFirstReleaseStateProjection {
        let decision = if kura_retired {
            ProductionInFlightFirstReleaseDecisionProjection {
                release_scope: binding_a,
                release_owner: self.actor,
                ..ProductionInFlightFirstReleaseDecisionProjection::default()
            }
        } else {
            ProductionInFlightFirstReleaseDecisionProjection::default()
        };
        ProductionInFlightFirstReleaseStateProjection {
            validator_count: self.validator_count,
            producer: self.producer,
            producer_selected_owner: self.producer,
            replicated_carrier_owners: self.validator_mask & !self.producer,
            payload_binding_a: self.payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count: self.reservation_group.reservation_count,
                reservation_state,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: self.payload_owners,
                execution_input_durable: 0,
                ready_qc_durable: false,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: self.payload_owners,
                ready_authorized: 0,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v1: true,
                ever_reservation_v1: true,
                pending_high_water: pending_prefix,
                released_high_water: released_prefix,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision,
            release: ProductionInFlightFirstReleaseReleaseProjection {
                kura_retired,
                pending_prefix,
                released_prefix,
                fifo_restored,
            },
        }
    }
    fn retirement_authorization(
        self,
        payload: &LaneExecutablePayloadV1,
        retirement: &AutonomousLaneSlotRetirementV1,
        view_state_path: &Path,
    ) -> std::result::Result<AutonomousLaneSlotRetirementPersistenceAuthorization, String> {
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(self.reservation_group);
        let before = self.state(
            binding_a,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            false,
            0,
            0,
        );
        let after = self.state(
            binding_a,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            true,
            0,
            0,
        );
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT,
            actor: self.actor,
            target: 0,
            before,
            after,
        };
        if check_production_in_flight_first_release_transition(projection).is_none() {
            return Err(
                "autonomous slot retirement failed the composed first-release transition gate"
                    .to_owned(),
            );
        }
        Ok(AutonomousLaneSlotRetirementPersistenceAuthorization {
            projection,
            executable_payload_hash: payload.payload_hash,
            origin_proposal_hash: payload.origin_proposal.proposal_hash,
            reservation_group: self.reservation_group,
            retirement: retirement.clone(),
            view_state_path: view_state_path.to_path_buf(),
        })
    }
    fn claim_transition_authorization(
        self,
        path: &Path,
        replacement: &AutonomousLaneEntrypointClaimV1,
        finalize_release: bool,
        prefix_before: u64,
    ) -> std::result::Result<AutonomousLaneEntrypointClaimTransitionAuthorization, String> {
        let selected_count = self.reservation_group.reservation_count;
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(self.reservation_group);
        if replacement.retirement_hash() != Some(self.retirement_hash) {
            return Err("autonomous claim transition names another retirement".to_owned());
        }
        let (action, before, after) = if finalize_release {
            if prefix_before >= selected_count
                || !matches!(
                    replacement.state,
                    AutonomousLaneEntrypointClaimStateV1::Released(_)
                )
            {
                return Err("invalid autonomous Released prefix transition".to_owned());
            }
            let before = self.state(
                binding_a,
                IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED,
                true,
                selected_count,
                prefix_before,
            );
            let after = self.state(
                binding_a,
                IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED,
                true,
                selected_count,
                prefix_before + 1,
            );
            (
                IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED,
                before,
                after,
            )
        } else {
            if prefix_before >= selected_count
                || !matches!(
                    replacement.state,
                    AutonomousLaneEntrypointClaimStateV1::ReleasePending(_)
                )
            {
                return Err("invalid autonomous ReleasePending prefix transition".to_owned());
            }
            let before = self.state(
                binding_a,
                IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
                true,
                prefix_before,
                0,
            );
            let after = self.state(
                binding_a,
                IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
                true,
                prefix_before + 1,
                0,
            );
            (
                IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING,
                before,
                after,
            )
        };
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action,
            actor: 0,
            target: 0,
            before,
            after,
        };
        if check_production_in_flight_first_release_transition(projection).is_none() {
            return Err(
                "autonomous claim prefix failed the composed first-release transition gate"
                    .to_owned(),
            );
        }
        Ok(AutonomousLaneEntrypointClaimTransitionAuthorization {
            projection,
            path: path.to_path_buf(),
            replacement: replacement.clone(),
        })
    }
    fn queue_preparation_authorization(
        self,
        retirement: &AutonomousLaneSlotRetirementV1,
        barrier: &LaneQueueReservationReleaseBarrierV1,
        claims_fully_released: bool,
    ) -> std::result::Result<AutonomousLaneQueueReleasePreparationAuthorization, String> {
        let expected_barrier = retirement.queue_release_barrier().map_err(|error| {
            format!("cannot derive autonomous Queue release barrier: {error:?}")
        })?;
        let barrier_group =
            lane_queue_reservation_group_binding_from_ordered_keys(barrier.ordered_keys.iter())
                .map_err(|_| {
                    "autonomous Queue release barrier group is not canonical".to_owned()
                })?;
        if expected_barrier != *barrier || barrier_group != self.reservation_group {
            return Err(
                "autonomous Queue release barrier differs from its Kura retirement".to_owned(),
            );
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(self.reservation_group);
        let selected_count = self.reservation_group.reservation_count;
        let before = self.state(
            binding_a,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            true,
            selected_count,
            0,
        );
        let after = self.state(
            binding_a,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED,
            true,
            selected_count,
            0,
        );
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE,
            actor: 0,
            target: 0,
            before,
            after,
        };
        let checked =
            check_production_in_flight_first_release_transition(projection).ok_or_else(|| {
                "autonomous Queue release preparation failed the composed transition gate"
                    .to_owned()
            })?;
        if checked.into_projection() != projection {
            return Err(
                "checked autonomous Queue release-preparation projection changed".to_owned(),
            );
        }
        Ok(AutonomousLaneQueueReleasePreparationAuthorization {
            projection,
            barrier: barrier.clone(),
            claims_fully_released,
        })
    }
    fn queue_finalization_authorization(
        self,
        retirement: &AutonomousLaneSlotRetirementV1,
        barrier: &LaneQueueReservationReleaseBarrierV1,
    ) -> std::result::Result<AutonomousLaneQueueReleaseFinalizationAuthorization, String> {
        let expected_barrier = retirement.queue_release_barrier().map_err(|error| {
            format!("cannot derive autonomous Queue release barrier: {error:?}")
        })?;
        let barrier_group =
            lane_queue_reservation_group_binding_from_ordered_keys(barrier.ordered_keys.iter())
                .map_err(|_| {
                    "autonomous Queue release barrier group is not canonical".to_owned()
                })?;
        if expected_barrier != *barrier || barrier_group != self.reservation_group {
            return Err(
                "autonomous Queue release barrier differs from its Kura retirement".to_owned(),
            );
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(self.reservation_group);
        let selected_count = self.reservation_group.reservation_count;
        let complete_before = self.state_with_fifo(
            binding_a,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_PREPARED,
            true,
            selected_count,
            selected_count,
            false,
        );
        let complete_after = self.state_with_fifo(
            binding_a,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED,
            true,
            selected_count,
            selected_count,
            false,
        );
        let restore_after = self.state_with_fifo(
            binding_a,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_COMPLETED,
            true,
            selected_count,
            selected_count,
            true,
        );
        let forget_after = self.state_with_fifo(
            binding_a,
            IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN,
            true,
            selected_count,
            selected_count,
            true,
        );
        let complete_projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE,
            actor: 0,
            target: 0,
            before: complete_before,
            after: complete_after,
        };
        let restore_projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO,
            actor: 0,
            target: 0,
            before: complete_after,
            after: restore_after,
        };
        let forget_projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE,
            actor: 0,
            target: 0,
            before: restore_after,
            after: forget_after,
        };
        if [complete_projection, restore_projection, forget_projection]
            .into_iter()
            .any(|projection| {
                check_production_in_flight_first_release_transition(projection).is_none()
            })
        {
            return Err(
                "autonomous Queue release finalization failed the composed transition gate"
                    .to_owned(),
            );
        }
        Ok(AutonomousLaneQueueReleaseFinalizationAuthorization {
            barrier: barrier.clone(),
            complete_projection,
            restore_projection,
            forget_projection,
        })
    }
}
impl LaneReadyAuthorization {
    /// Return whether this one-shot authority names the exact READY signing
    /// request and still has a structurally complete durable-input binding.
    pub(crate) fn matches_signing_request(
        &self,
        proposal: &LaneBlockProposalV1,
        availability_body: &LanePayloadAvailabilityBodyV1,
        signer: &PeerId,
        height_context_id: HeightContextId,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        let group = self.reservation_group;
        self.proposal == *proposal
            && self.availability_body == *availability_body
            && self.signer == *signer
            && self.height_context_id == height_context_id
            && self
                .durable_execution_input_hash
                .as_ref()
                .iter()
                .any(|byte| *byte != 0)
            && group.identity.lane_id == descriptor.lane_id
            && group.identity.dataspace_id == descriptor.dataspace_id
            && group.identity.lane_incarnation == descriptor.lane_incarnation
            && group.identity.proposal_height == descriptor.proposal_height
            && group.identity.lane_block_height == descriptor.lane_block_height
            && group.identity.lane_block_view == descriptor.lane_block_view
            && group.reservation_count
                == u64::try_from(descriptor.accepted_transaction_hashes.len()).unwrap_or(u64::MAX)
    }
    /// Consume this exact durable-input authority at the READY signature
    /// boundary after rechecking the complete first-release projection.
    pub(crate) fn consume_signing_request(
        self,
        proposal: &LaneBlockProposalV1,
        availability_body: &LanePayloadAvailabilityBodyV1,
        signer: &PeerId,
        height_context_id: HeightContextId,
    ) -> bool {
        if !self.matches_signing_request(proposal, availability_body, signer, height_context_id) {
            return false;
        }
        let descriptor = &proposal.descriptor;
        let Ok(validator_count) = u8::try_from(descriptor.validator_set.len()) else {
            return false;
        };
        if validator_count == 0 || validator_count > 128 {
            return false;
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let Some(producer_index) = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &self.producer)
        else {
            return false;
        };
        let Some(signer_index) = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == signer)
        else {
            return false;
        };
        let Some(producer) = u32::try_from(producer_index)
            .ok()
            .and_then(|index| 1_u128.checked_shl(index))
        else {
            return false;
        };
        let Some(actor) = u32::try_from(signer_index)
            .ok()
            .and_then(|index| 1_u128.checked_shl(index))
        else {
            return false;
        };
        let selected_count = self.reservation_group.reservation_count;
        if !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
            .unwrap_or(u64::MAX))
            .contains(&selected_count)
        {
            return false;
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(self.reservation_group);
        let payload_owners = producer | actor;
        let before = ProductionInFlightFirstReleaseStateProjection {
            validator_count,
            producer,
            producer_selected_owner: producer,
            replicated_carrier_owners: validator_mask & !producer,
            payload_binding_a: payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: payload_owners,
                execution_input_durable: actor,
                ready_qc_durable: false,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: payload_owners,
                ready_authorized: actor,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v1: true,
                ever_reservation_v1: true,
                ever_execution_input_durable: actor,
                ever_ready_authorized: actor,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };
        let mut after = before;
        after.history.ready_signed = actor;
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY,
            actor,
            target: 0,
            before,
            after,
        };
        check_production_in_flight_first_release_transition(projection)
            .is_some_and(|checked| checked.into_projection() == projection)
    }
}
impl Kura {
    /// Mint the exact one-shot `PersistKuraRetirement` authority consumed by
    /// the first durable autonomous view-state tombstone write.
    fn authorize_autonomous_lane_slot_retirement_persistence(
        &self,
        payload: &LaneExecutablePayloadV1,
        retirement: &AutonomousLaneSlotRetirementV1,
        view_state_path: &Path,
    ) -> std::result::Result<AutonomousLaneSlotRetirementPersistenceAuthorization, String> {
        AutonomousLaneReleaseProjectionContext::from_payload(self, payload, retirement)?
            .retirement_authorization(payload, retirement, view_state_path)
    }
    /// Mint the one-shot `PersistExecutionInput` authority for an exact
    /// autonomous payload/input pair.
    fn authorize_autonomous_execution_input_persistence(
        &self,
        payload: &LaneExecutablePayloadV1,
        input: &LaneBlockExecutionInputArtifact,
    ) -> std::result::Result<AutonomousLaneExecutionInputPersistenceAuthorization, String> {
        let (network_id, epoch, payload_hash) = input
            .source
            .autonomous_binding()
            .ok_or_else(|| "autonomous execution input has a global-block source".to_owned())?;
        payload
            .validate(network_id, epoch)
            .map_err(|error| error.to_string())?;
        if payload.payload_hash != payload_hash {
            return Err("autonomous execution input names another executable payload".to_owned());
        }
        let expected =
            Self::autonomous_lane_block_execution_input_candidate(payload, network_id, epoch)
                .map_err(|error| format!("{error:?}"))?;
        if expected != *input {
            return Err("autonomous execution input differs from its canonical payload".to_owned());
        }
        let descriptor = &payload.origin_proposal.descriptor;
        let validator_count = u8::try_from(descriptor.validator_set.len()).map_err(|_| {
            "autonomous execution-input committee exceeds the refinement width".to_owned()
        })?;
        if validator_count == 0 || validator_count > 128 {
            return Err(
                "autonomous execution-input committee is outside the 1..=128 refinement width"
                    .to_owned(),
            );
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let producer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &payload.producer)
            .ok_or_else(|| {
                "autonomous execution-input producer is absent from its committee".to_owned()
            })?;
        let producer = 1_u128
            .checked_shl(u32::try_from(producer_index).map_err(|_| {
                "autonomous execution-input producer exceeds the refinement width".to_owned()
            })?)
            .ok_or_else(|| {
                "autonomous execution-input producer exceeds the refinement width".to_owned()
            })?;
        // The bound local identity is the exact physical writer when it is a
        // member of the historical lane committee. Startup repair and
        // non-committee carrier recovery use the producer-authenticated
        // durable payload as their logical custody witness.
        let actor_peer = self
            .local_peer_id
            .get()
            .filter(|peer| descriptor.validator_set.contains(*peer))
            .unwrap_or(&payload.producer);
        let actor_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == actor_peer)
            .ok_or_else(|| {
                "autonomous execution-input actor is absent from its committee".to_owned()
            })?;
        let actor = 1_u128
            .checked_shl(u32::try_from(actor_index).map_err(|_| {
                "autonomous execution-input actor exceeds the refinement width".to_owned()
            })?)
            .ok_or_else(|| {
                "autonomous execution-input actor exceeds the refinement width".to_owned()
            })?;
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
                .map_err(|_| {
                    "autonomous execution-input reservation group is not canonical".to_owned()
                })?;
        let selected_count = reservation_group.reservation_count;
        if !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
            .unwrap_or(u64::MAX))
            .contains(&selected_count)
        {
            return Err(
                "autonomous execution-input reservation count is outside the first-release bound"
                    .to_owned(),
            );
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(reservation_group);
        let payload_owners = producer | actor;
        if payload_owners & !validator_mask != 0 {
            return Err("autonomous execution-input ownership exceeds its committee".to_owned());
        }
        let before = ProductionInFlightFirstReleaseStateProjection {
            validator_count,
            producer,
            producer_selected_owner: producer,
            replicated_carrier_owners: validator_mask & !producer,
            payload_binding_a: payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: payload_owners,
                execution_input_durable: 0,
                ready_qc_durable: false,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: payload_owners,
                ready_authorized: 0,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v1: true,
                ever_reservation_v1: true,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };
        let mut after = before;
        after.carrier.execution_input_durable = actor;
        after.history.ever_execution_input_durable = actor;
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT,
            actor,
            target: 0,
            before,
            after,
        };
        Ok(AutonomousLaneExecutionInputPersistenceAuthorization {
            projection,
            input: input.clone(),
        })
    }
    /// Validate one exact autonomous READY certificate and mint the move-only
    /// composed-transition authority consumed by its Kura persistence sink.
    fn authorize_lane_payload_availability_certificate_persistence(
        payload: &LaneExecutablePayloadV1,
        certificate: &DurableLanePayloadAvailabilityCertificateV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> std::result::Result<AutonomousLaneReadyQcPersistenceAuthorization, String> {
        crate::lane_consensus::validate_lane_payload_availability_certificate(
            certificate,
            payload,
            expected_network_id,
            expected_epoch,
        )
        .map_err(|error| error.to_string())?;
        let descriptor = &payload.origin_proposal.descriptor;
        let validator_count = u8::try_from(descriptor.validator_set.len())
            .map_err(|_| "autonomous READY committee exceeds the refinement width".to_owned())?;
        if validator_count == 0 || validator_count > 128 {
            return Err(
                "autonomous READY committee is outside the 1..=128 refinement width".to_owned(),
            );
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let producer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &payload.producer)
            .ok_or_else(|| {
                "autonomous payload producer is absent from its READY committee".to_owned()
            })?;
        let producer = 1_u128
            .checked_shl(
                u32::try_from(producer_index).map_err(|_| {
                    "autonomous producer index exceeds the refinement width".to_owned()
                })?,
            )
            .ok_or_else(|| "autonomous producer index exceeds the refinement width".to_owned())?;
        let availability_qc = certificate
            .certificate
            .payload_availability_qc
            .as_ref()
            .ok_or_else(|| "autonomous PrepareQC lacks its READY certificate".to_owned())?;
        if availability_qc.validator_set != descriptor.validator_set {
            return Err("autonomous READY committee differs from its proposal".to_owned());
        }
        if availability_qc.signers_bitmap.len() != descriptor.validator_set.len().div_ceil(8) {
            return Err("autonomous READY bitmap has a noncanonical length".to_owned());
        }
        let mut ready_signers = 0_u128;
        for (byte_index, byte) in availability_qc.signers_bitmap.iter().copied().enumerate() {
            for bit_index in 0..8_usize {
                if byte & (1_u8 << bit_index) == 0 {
                    continue;
                }
                let index = byte_index
                    .checked_mul(8)
                    .and_then(|base| base.checked_add(bit_index))
                    .ok_or_else(|| "autonomous READY bitmap index overflows".to_owned())?;
                if index >= descriptor.validator_set.len() {
                    return Err("autonomous READY bitmap selects a padding bit".to_owned());
                }
                ready_signers |= 1_u128
                    .checked_shl(u32::try_from(index).map_err(|_| {
                        "autonomous READY signer exceeds the refinement width".to_owned()
                    })?)
                    .ok_or_else(|| {
                        "autonomous READY signer exceeds the refinement width".to_owned()
                    })?;
            }
        }
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
                .map_err(|_| "autonomous READY reservation group is not canonical".to_owned())?;
        let selected_count = reservation_group.reservation_count;
        if !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
            .unwrap_or(u64::MAX))
            .contains(&selected_count)
        {
            return Err(
                "autonomous READY reservation count is outside the first-release bound".to_owned(),
            );
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(reservation_group);
        let payload_owners = ready_signers | producer;
        if payload_owners & !validator_mask != 0 {
            return Err("autonomous READY ownership exceeds its committee".to_owned());
        }
        let before = ProductionInFlightFirstReleaseStateProjection {
            validator_count,
            producer,
            producer_selected_owner: producer,
            replicated_carrier_owners: validator_mask & !producer,
            payload_binding_a: payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: payload_owners,
                execution_input_durable: ready_signers,
                ready_qc_durable: false,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: payload_owners,
                ready_authorized: ready_signers,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v1: true,
                ever_reservation_v1: true,
                ever_execution_input_durable: ready_signers,
                ever_ready_authorized: ready_signers,
                ready_signed: ready_signers,
                ever_ready_qc_durable: false,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };
        let mut after = before;
        after.carrier.ready_qc_durable = true;
        after.history.ever_ready_qc_durable = true;
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC,
            actor: 0,
            target: 0,
            before,
            after,
        };
        Ok(AutonomousLaneReadyQcPersistenceAuthorization {
            projection,
            network_id: payload.network_id,
            epoch: payload.epoch,
            executable_payload_hash: payload.payload_hash,
            origin_proposal_hash: payload.origin_proposal.proposal_hash,
            reservation_group,
            certificate: certificate.clone(),
        })
    }
    /// Mint the exact one-shot `LaneCommit` authority for a certified
    /// autonomous source immediately before its first durable certificate
    /// publication.
    fn authorize_autonomous_lane_commit_persistence(
        source: &DurableAutonomousLaneMergeSource,
        artifact: &CertifiedLaneBlockArtifact,
    ) -> std::result::Result<AutonomousLaneCommitPersistenceAuthorization, String> {
        if source.bundle.certified != *artifact {
            return Err(
                "autonomous lane-Commit source differs from the certified artifact".to_owned(),
            );
        }
        let availability_qc = artifact
            .prepare_qc
            .payload_availability_qc
            .as_ref()
            .ok_or_else(|| {
                "autonomous lane-Commit artifact lacks its READY certificate".to_owned()
            })?;
        let network_id = availability_qc.body.network_id;
        let epoch = availability_qc.body.epoch;
        Self::validate_autonomous_lane_merge_bundle(&source.bundle, network_id, epoch)
            .map_err(str::to_owned)?;
        let canonical_source_bundle = source
            .bundle
            .encode_framed()
            .map_err(|error| error.to_string())?;
        if source.source_bundle != canonical_source_bundle
            || source.bundle_hash
                != source
                    .bundle
                    .bundle_hash()
                    .map_err(|error| error.to_string())?
        {
            return Err(
                "autonomous lane-Commit source bytes or bundle hash are not canonical".to_owned(),
            );
        }
        let expected_input = Self::autonomous_lane_block_execution_input_candidate(
            source.bundle.executable_payload(),
            network_id,
            epoch,
        )
        .map_err(|error| format!("{error:?}"))?;
        if source.input != expected_input {
            return Err(
                "autonomous lane-Commit input differs from its executable payload".to_owned(),
            );
        }
        let descriptor = &artifact.proposal.descriptor;
        let validator_count = u8::try_from(descriptor.validator_set.len()).map_err(|_| {
            "autonomous lane-Commit committee exceeds the refinement width".to_owned()
        })?;
        if validator_count == 0 || validator_count > 128 {
            return Err(
                "autonomous lane-Commit committee is outside the 1..=128 refinement width"
                    .to_owned(),
            );
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let producer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &source.bundle.executable_payload().producer)
            .ok_or_else(|| {
                "autonomous lane-Commit producer is absent from its committee".to_owned()
            })?;
        let producer = 1_u128
            .checked_shl(u32::try_from(producer_index).map_err(|_| {
                "autonomous lane-Commit producer index exceeds the refinement width".to_owned()
            })?)
            .ok_or_else(|| {
                "autonomous lane-Commit producer index exceeds the refinement width".to_owned()
            })?;
        let bitmap_mask = |bitmap: &[u8]| {
            if bitmap.len() != descriptor.validator_set.len().div_ceil(8) {
                return Err("autonomous lane-Commit bitmap has a noncanonical length".to_owned());
            }
            let mut mask = 0_u128;
            for (byte_index, byte) in bitmap.iter().copied().enumerate() {
                for bit_index in 0..8_usize {
                    if byte & (1_u8 << bit_index) == 0 {
                        continue;
                    }
                    let index = byte_index
                        .checked_mul(8)
                        .and_then(|base| base.checked_add(bit_index))
                        .ok_or_else(|| {
                            "autonomous lane-Commit bitmap index overflows".to_owned()
                        })?;
                    if index >= descriptor.validator_set.len() {
                        return Err(
                            "autonomous lane-Commit bitmap selects a padding bit".to_owned()
                        );
                    }
                    mask |= 1_u128
                        .checked_shl(u32::try_from(index).map_err(|_| {
                            "autonomous lane-Commit signer exceeds the refinement width".to_owned()
                        })?)
                        .ok_or_else(|| {
                            "autonomous lane-Commit signer exceeds the refinement width".to_owned()
                        })?;
                }
            }
            Ok(mask)
        };
        let ready_signers = bitmap_mask(&availability_qc.signers_bitmap)?;
        let commit_signers = bitmap_mask(&artifact.commit_qc.signers_bitmap)?;
        let lane_commit_candidates = ready_signers & commit_signers;
        if lane_commit_candidates == 0 {
            return Err(
                "autonomous READY and Commit QCs have no common authenticated signer".to_owned(),
            );
        }
        let lane_commit_actor = 1_u128
            .checked_shl(lane_commit_candidates.trailing_zeros())
            .ok_or_else(|| {
                "autonomous lane-Commit signer exceeds the refinement width".to_owned()
            })?;
        let reservation_group = lane_queue_reservation_group_binding_from_ordered_keys(
            source.bundle.executable_payload().reservation_keys.iter(),
        )
        .map_err(|_| "autonomous lane-Commit reservation group is not canonical".to_owned())?;
        let selected_count = reservation_group.reservation_count;
        if !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
            .unwrap_or(u64::MAX))
            .contains(&selected_count)
        {
            return Err(
                "autonomous lane-Commit reservation count is outside the first-release bound"
                    .to_owned(),
            );
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(reservation_group);
        let payload_owners = ready_signers | producer;
        if payload_owners & !validator_mask != 0 {
            return Err("autonomous lane-Commit ownership exceeds its committee".to_owned());
        }
        let before = ProductionInFlightFirstReleaseStateProjection {
            validator_count,
            producer,
            producer_selected_owner: producer,
            replicated_carrier_owners: validator_mask & !producer,
            payload_binding_a: payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: payload_owners,
                execution_input_durable: ready_signers,
                ready_qc_durable: true,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: payload_owners,
                ready_authorized: ready_signers,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v1: true,
                ever_reservation_v1: true,
                ever_execution_input_durable: ready_signers,
                ever_ready_authorized: ready_signers,
                ready_signed: ready_signers,
                ever_ready_qc_durable: true,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };
        let mut after = before;
        after.decision.lane_commit_scope = binding_a;
        after.decision.lane_commit_owner = lane_commit_actor;
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT,
            actor: lane_commit_actor,
            target: 0,
            before,
            after,
        };
        Ok(AutonomousLaneCommitPersistenceAuthorization {
            projection,
            certified: artifact.clone(),
        })
    }
    fn autonomous_lane_merge_bundle_paths_for_entry(
        entry: &LaneConfigEntry,
        store_root: &Path,
    ) -> (PathBuf, PathBuf) {
        let dir = Self::lane_artifact_dir(&entry.blocks_dir(store_root));
        (
            dir.join(AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE),
            dir.join(AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE),
        )
    }
    pub(crate) fn validate_certified_lane_block_artifact(
        artifact: &CertifiedLaneBlockArtifact,
    ) -> std::result::Result<(), &'static str> {
        #[cfg(test)]
        if FAIL_NEXT_CERTIFIED_LANE_BLOCK_ARTIFACT_VALIDATION.with(|flag| flag.replace(false)) {
            return Err("injected certified lane block artifact validation failure");
        }
        artifact
            .encode_framed()
            .map_err(|_| "certified lane block exceeds the merge source envelope byte limit")?;
        crate::lane_consensus::validate_lane_block_proposal(&artifact.proposal)
            .map_err(|_| "invalid lane block proposal")?;
        crate::lane_consensus::validate_lane_block_qc(&artifact.prepare_qc)
            .map_err(|_| "invalid prepare lane block QC")?;
        crate::lane_consensus::validate_lane_block_qc(&artifact.commit_qc)
            .map_err(|_| "invalid commit lane block QC")?;
        let descriptor = &artifact.proposal.descriptor;
        let prepare_body = artifact.proposal.vote_body(CertPhase::Prepare);
        let commit_body = artifact.proposal.vote_body(CertPhase::Commit);
        if artifact.prepare_qc.body != prepare_body {
            return Err("prepare QC body does not match proposal");
        }
        if artifact.commit_qc.body != commit_body {
            return Err("commit QC body does not match proposal");
        }
        for qc in [&artifact.prepare_qc, &artifact.commit_qc] {
            if qc.validator_set_hash_version != descriptor.validator_set_hash_version
                || qc.validator_set_hash != descriptor.validator_set_hash
                || qc.validator_set != descriptor.validator_set
            {
                return Err("QC validator set does not match proposal");
            }
        }
        let mut expected_pops = Self::lane_block_qc_signer_keys(&artifact.prepare_qc)?;
        expected_pops.extend(Self::lane_block_qc_signer_keys(&artifact.commit_qc)?);
        let actual_pops = artifact
            .signer_pops
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        if actual_pops != expected_pops {
            return Err("certified lane block signer PoPs do not match QC signers");
        }
        crate::lane_consensus::validate_lane_block_qc_aggregate(
            &artifact.prepare_qc,
            &artifact.signer_pops,
        )
        .map_err(|_| "invalid prepare lane block QC aggregate")?;
        crate::lane_consensus::validate_lane_block_qc_aggregate(
            &artifact.commit_qc,
            &artifact.signer_pops,
        )
        .map_err(|_| "invalid commit lane block QC aggregate")?;
        Ok(())
    }
    fn validate_autonomous_lane_block_artifact(
        artifact: &AutonomousLaneBlockArtifact,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> std::result::Result<LaneBlockProposalV1, &'static str> {
        artifact
            .encode_framed()
            .map_err(|_| "autonomous lane block exceeds the merge source byte limit")?;
        match artifact.format {
            AutonomousLaneBlockArtifactFormat::Current => {}
        }
        artifact
            .executable_payload
            .validate(expected_network_id, expected_epoch)
            .map_err(|_| "invalid autonomous executable payload")?;
        if artifact
            .executable_payload
            .origin_proposal
            .descriptor
            .lane_block_view
            != 0
        {
            return Err("autonomous executable payload must originate at lane view zero");
        }
        if artifact.new_view_certificates.len() > MAX_LANE_NEW_VIEW_CERTIFICATES {
            return Err("autonomous lane NewView certificate limit exceeded");
        }
        if let Some(certificate) = &artifact.availability_certificate {
            crate::lane_consensus::validate_lane_payload_availability_certificate(
                certificate,
                &artifact.executable_payload,
                expected_network_id,
                expected_epoch,
            )
            .map_err(|_| "invalid autonomous lane payload availability certificate")?;
        }
        let mut current = artifact.executable_payload.origin_proposal.clone();
        if let Some(checkpoint) = &artifact.view_checkpoint {
            crate::lane_consensus::validate_lane_block_view_checkpoint(
                checkpoint,
                &artifact.executable_payload,
                expected_network_id,
                expected_epoch,
            )
            .map_err(|_| "invalid autonomous lane view checkpoint")?;
            current = checkpoint.target_proposal.clone();
        }
        for durable in &artifact.new_view_certificates {
            let target = crate::lane_consensus::retarget_lane_block_proposal_view(
                &current,
                durable.certificate.body.target_view,
            )
            .map_err(|_| "autonomous lane NewView target is not contiguous")?;
            crate::lane_consensus::validate_lane_block_new_view_transition(
                &current,
                &target,
                &artifact.executable_payload,
                durable,
                expected_network_id,
                expected_epoch,
            )
            .map_err(|_| "invalid autonomous lane NewView transition")?;
            current = target;
        }
        Ok(current)
    }
    /// Validate a complete autonomous merge source without consulting mutable
    /// committee state or local sidecars.
    pub(crate) fn validate_autonomous_lane_merge_bundle(
        bundle: &AutonomousLaneMergeBundleV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> std::result::Result<(), &'static str> {
        if bundle.version != AutonomousLaneMergeBundleV1::VERSION {
            return Err("unsupported autonomous lane merge bundle version");
        }
        if bundle
            .encode_framed()
            .map_err(|_| "oversized autonomous lane merge bundle")?
            .len()
            > MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES
        {
            return Err("autonomous lane merge bundle exceeds hard byte limit");
        }
        if bundle.autonomous.availability_certificate.is_none() {
            return Err("autonomous lane merge bundle lacks a durable availability certificate");
        }
        let _cursor = Self::validate_autonomous_lane_block_artifact(
            &bundle.autonomous,
            expected_network_id,
            expected_epoch,
        )?;
        Self::validate_certified_lane_block_artifact(&bundle.certified)?;
        let availability = bundle
            .autonomous
            .availability_certificate
            .as_ref()
            .ok_or("autonomous lane merge bundle lacks a durable availability certificate")?;
        let origin = &bundle.autonomous.executable_payload.origin_proposal;
        if &bundle.certified.proposal != origin {
            return Err("autonomous lane merge bundle must certify the immutable origin proposal");
        }
        if availability.certificate != bundle.certified.prepare_qc
            || bundle.certified.prepare_qc.body != origin.vote_body(CertPhase::Prepare)
        {
            return Err("payload availability certificate is not the exact origin prepare QC");
        }
        Ok(())
    }
    /// Decode exact canonical framed bundle bytes and verify all embedded proofs.
    pub(crate) fn decode_autonomous_lane_merge_bundle(
        bytes: &[u8],
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> std::result::Result<AutonomousLaneMergeBundleV1, &'static str> {
        if bytes.len() > MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES {
            return Err("autonomous lane merge bundle exceeds hard byte limit");
        }
        let bundle =
            norito::decode_canonical::<AutonomousLaneMergeBundleV1>(bytes).map_err(|error| {
                match error {
                    norito::Error::NonCanonicalEncoding => {
                        "autonomous lane merge bundle is not canonical framed Norito"
                    }
                    _ => "autonomous lane merge bundle is not valid framed Norito",
                }
            })?;
        Self::validate_autonomous_lane_merge_bundle(&bundle, expected_network_id, expected_epoch)?;
        Ok(bundle)
    }
    fn autonomous_lane_merge_bundle_pair_entry_limit(&self) -> usize {
        self.lane_history_retention
            .get()
            .saturating_add(usize::try_from(MAX_INDEXED_SIDECAR_GAP_ENTRIES).unwrap_or(usize::MAX))
            .min(MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES)
    }
    fn autonomous_lane_merge_bundle_pair_byte_limit(&self) -> usize {
        self.pending_control_sidecar_limits.aggregate_bytes
    }
    /// Validate the whole bundle pair before any exact-slot admission.
    ///
    /// The configured retention window plus the existing bounded sparse-gap
    /// allowance limits index work. The established autonomous sidecar byte
    /// budget limits payload exposure. Entries must describe one contiguous,
    /// append-only data image, so truncated, overlapping, trailing, and
    /// oversized pairs fail before any payload allocation.
    fn validate_autonomous_lane_merge_bundle_pair_layout_locked(
        &self,
        bound: &mut BoundProgressSidecar,
    ) -> std::result::Result<(SidecarIndexLayout, BTreeSet<u64>), &'static str> {
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err("autonomous merge bundle pair changed before bounded validation");
        }
        let index_len = bound
            .index
            .metadata()
            .map_err(|_| "autonomous merge bundle index metadata is unreadable")?
            .len();
        let layout = SidecarIndexLayout::read_from(&mut bound.index, index_len)
            .map_err(|_| "autonomous merge bundle index is malformed")?;
        if layout.aligned_len != index_len {
            return Err("autonomous merge bundle index has trailing or partial bytes");
        }
        if usize::try_from(layout.entry_count).unwrap_or(usize::MAX)
            > self.autonomous_lane_merge_bundle_pair_entry_limit()
        {
            return Err("autonomous merge bundle index exceeds its bounded entry count");
        }
        let data_len = bound
            .data
            .metadata()
            .map_err(|_| "autonomous merge bundle data metadata is unreadable")?
            .len();
        if data_len
            > u64::try_from(self.autonomous_lane_merge_bundle_pair_byte_limit()).unwrap_or(u64::MAX)
        {
            return Err("autonomous merge bundle data exceeds its aggregate byte budget");
        }
        bound
            .index
            .seek(SeekFrom::Start(layout.entries_offset))
            .map_err(|_| "autonomous merge bundle index entries are unreadable")?;
        let mut heights = BTreeSet::new();
        let mut indexed_end = 0_u64;
        let mut entry_bytes = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        for offset in 0..layout.entry_count {
            bound
                .index
                .read_exact(&mut entry_bytes)
                .map_err(|_| "autonomous merge bundle index entry is unreadable")?;
            let entry = SidecarIndexEntry::from_bytes(entry_bytes);
            if entry.len == 0 {
                if entry.offset != 0 {
                    return Err("empty autonomous merge bundle index entry has a non-zero offset");
                }
                continue;
            }
            if entry.len
                > u64::try_from(MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES).unwrap_or(u64::MAX)
            {
                return Err("autonomous merge bundle entry exceeds its hard byte limit");
            }
            if entry.offset != indexed_end {
                return Err(
                    "autonomous merge bundle data ranges are overlapping, gapped, or reordered",
                );
            }
            indexed_end = entry
                .offset
                .checked_add(entry.len)
                .ok_or("autonomous merge bundle data range overflows")?;
            if indexed_end > data_len {
                return Err("autonomous merge bundle entry extends beyond its data file");
            }
            let height = layout
                .base_height
                .checked_add(offset)
                .ok_or("autonomous merge bundle index height overflows")?;
            heights.insert(height);
        }
        if indexed_end != data_len {
            return Err("autonomous merge bundle data has an unindexed suffix");
        }
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err("autonomous merge bundle pair changed during bounded validation");
        }
        Ok((layout, heights))
    }
    /// Read one exact bundle slot from an already bound progress pair.
    ///
    /// Empty sparse-index entries are absence. Every non-empty entry must be
    /// bounded, canonical, self-validating, and identify its exact lane slot;
    /// malformed bytes are never treated as a repairable miss.
    fn read_autonomous_lane_merge_bundle_from_bound_locked(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        bound: &mut BoundProgressSidecar,
    ) -> std::result::Result<Option<(AutonomousLaneMergeBundleV1, Vec<u8>)>, &'static str> {
        let (layout, populated_heights) =
            self.validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)?;
        if !populated_heights.contains(&lane_block_height) {
            return Ok(None);
        }
        let Some(entry_position) = layout.entry_position(lane_block_height) else {
            return Ok(None);
        };
        let mut entry_bytes = [0_u8; PIPELINE_INDEX_ENTRY_SIZE];
        bound
            .index
            .seek(SeekFrom::Start(entry_position))
            .and_then(|_| bound.index.read_exact(&mut entry_bytes))
            .map_err(|_| "autonomous merge bundle index entry is unreadable")?;
        let entry = SidecarIndexEntry::from_bytes(entry_bytes);
        if entry.len == 0 {
            return Ok(None);
        }
        if entry.len > u64::try_from(MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES).unwrap_or(u64::MAX) {
            return Err("autonomous merge bundle entry exceeds its hard byte limit");
        }
        let payload_len = usize::try_from(entry.len)
            .map_err(|_| "autonomous merge bundle entry length is not representable")?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(payload_len)
            .map_err(|_| "autonomous merge bundle allocation exceeds process limits")?;
        bytes.resize(payload_len, 0);
        bound
            .data
            .seek(SeekFrom::Start(entry.offset))
            .and_then(|_| bound.data.read_exact(&mut bytes))
            .map_err(|_| "autonomous merge bundle entry is unreadable")?;
        let bundle = norito::decode_canonical::<AutonomousLaneMergeBundleV1>(&bytes)
            .map_err(|_| "autonomous merge bundle entry is not canonical framed Norito")?;
        let payload = bundle.executable_payload();
        Self::validate_autonomous_lane_merge_bundle(&bundle, payload.network_id, payload.epoch)
            .map_err(|_| "autonomous merge bundle entry is invalid")?;
        let descriptor = &bundle.certified.proposal.descriptor;
        if descriptor.lane_id != lane_id || descriptor.lane_block_height != lane_block_height {
            return Err("autonomous merge bundle entry names another lane slot");
        }
        if bundle
            .encode_framed()
            .map_err(|_| "autonomous merge bundle entry cannot be re-encoded")?
            != bytes
        {
            return Err("autonomous merge bundle entry is not canonically stable");
        }
        if !self.bound_progress_sidecar_unchanged(bound) {
            return Err("autonomous merge bundle pair changed during exact lookup");
        }
        Ok(Some((bundle, bytes)))
    }
    #[allow(clippy::too_many_lines)]
    fn durable_autonomous_lane_merge_source_under_prune_guard(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
        certified_override: Option<&CertifiedLaneBlockArtifact>,
        require_persisted_bundle: bool,
    ) -> std::result::Result<DurableAutonomousLaneMergeSource, &'static str> {
        if self.prune_recovery_is_required() {
            return Err("Kura prune recovery blocks autonomous merge-source admission");
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self
            .lane_storage_entry(lane_id)
            .map_err(|_| "autonomous merge source has no active lane storage")?;
        let _sidecar_guard = self.sidecar_lock.lock();
        if self.prune_recovery_is_required() {
            return Err("Kura prune recovery blocks autonomous merge-source admission");
        }
        let autonomous_record = self
            .read_autonomous_lane_block_record_locked(
                &entry,
                lane_id,
                lane_block_height,
                expected_network_id,
                expected_epoch,
                None,
            )
            .map_err(|_| "autonomous merge payload failed repair-disabled readback")?
            .ok_or("autonomous lane merge payload is unavailable")?;
        if autonomous_record.retirement.is_some() {
            return Err("retired autonomous lane slot is not merge eligible");
        }
        let view_state_path = &autonomous_record.view_state_path;
        let view_state_parent = view_state_path
            .parent()
            .ok_or("autonomous lane view state has no parent directory")?;
        let view_state_temp = Self::autonomous_lane_block_view_state_temp_path(view_state_path);
        if self
            .regular_sidecar_metadata(&view_state_temp, view_state_parent)
            .map_err(|_| "autonomous lane view recovery artifact is invalid")?
            .is_some()
        {
            return Err("autonomous lane view state has unresolved recovery state");
        }
        let autonomous = autonomous_record.artifact;
        let certified = if let Some(certified) = certified_override {
            self.require_active_lane_artifact(&entry, &certified.proposal.descriptor)
                .map_err(|_| "autonomous merge certificate targets stale lane geometry")?;
            certified.clone()
        } else {
            let (data_path, index_path) =
                Self::certified_lane_block_paths_for_entry(&entry, &self.store_root);
            let namespace = self
                .open_bound_progress_namespace(&data_path, &index_path)
                .map_err(|_| "certified lane block pair could not be bound")?;
            self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
                &namespace,
                &data_path,
                &index_path,
                "certified lane block pair",
            )
            .map_err(|_| "certified lane block pair has unresolved recovery state")?;
            let mut pair = self
                .open_bound_progress_pair(&data_path, &index_path)
                .map_err(|_| "certified lane block pair could not be opened")?;
            let certified = match &mut pair {
                BoundProgressPair::Absent(_) => None,
                BoundProgressPair::Present(bound) => {
                    self.bound_indexed_sidecar_height_range(bound, "certified lane block")
                        .map_err(|_| "certified lane block pair has a malformed index")?;
                    self.read_active_certified_lane_block_artifact_from_bound_locked(
                        &entry,
                        lane_block_height,
                        bound,
                    )
                }
            }
            .ok_or("certified lane block pair lacks the exact autonomous slot")?;
            if let BoundProgressPair::Present(bound) = &pair
                && !self.bound_progress_sidecar_unchanged(bound)
            {
                return Err("certified lane block pair changed during bundle admission");
            }
            certified
        };
        let frontier_read = self
            .read_latest_certified_lane_block_frontier_structural_locked(&entry, false)
            .map_err(|_| "latest certified frontier failed repair-disabled readback")?;
        if certified_override.is_none() && frontier_read.is_none() {
            return Err("certified lane block pair lacks its mandatory durable frontier");
        }
        if let Some(frontier_read) = frontier_read {
            let frontier_artifact = &frontier_read.frontier.artifact;
            let frontier_descriptor = &frontier_artifact.proposal.descriptor;
            self.require_active_lane_artifact(&entry, frontier_descriptor)
                .map_err(|_| "latest certified frontier targets stale lane geometry")?;
            if frontier_descriptor.lane_block_height < lane_block_height {
                return Err("certified lane block pair is ahead of its durable frontier");
            }
            if frontier_descriptor.lane_block_height == lane_block_height
                && frontier_artifact != &certified
            {
                return Err("certified lane block pair conflicts with its exact frontier");
            }
            self.confirm_latest_certified_lane_block_frontier_read_locked(
                &entry,
                &frontier_read.snapshot,
            )
            .map_err(|_| "latest certified frontier changed during bundle admission")?;
        }
        let (input_data_path, input_index_path) =
            Self::lane_block_execution_input_paths_for_entry(&entry, &self.store_root);
        let input_namespace = self
            .open_bound_progress_namespace(&input_data_path, &input_index_path)
            .map_err(|_| "lane execution input pair could not be bound")?;
        self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
            &input_namespace,
            &input_data_path,
            &input_index_path,
            "lane block execution input",
        )
        .map_err(|_| "lane execution input pair has unresolved recovery state")?;
        let mut input_pair = self
            .open_bound_progress_pair(&input_data_path, &input_index_path)
            .map_err(|_| "lane execution input pair could not be opened")?;
        let input = match &mut input_pair {
            BoundProgressPair::Absent(_) => None,
            BoundProgressPair::Present(bound) => {
                self.bound_indexed_sidecar_height_range(bound, "lane block execution input")
                    .map_err(|_| "lane execution input pair has a malformed index")?;
                Self::read_indexed_sidecar_from_open_files(
                    lane_block_height,
                    &mut bound.data,
                    &mut bound.index,
                    &bound.namespace.data_path,
                    &bound.namespace.index_path,
                    norito::decode_canonical::<LaneBlockExecutionInputArtifact>,
                    "lane block execution input",
                )
            }
        }
        .ok_or("durable autonomous execution input is unavailable")?;
        if let BoundProgressPair::Present(bound) = &input_pair
            && !self.bound_progress_sidecar_unchanged(bound)
        {
            return Err("lane execution input pair changed during bundle admission");
        }
        Self::validate_lane_block_execution_input_artifact(&input)
            .map_err(|_| "durable autonomous execution input is invalid")?;
        self.require_active_lane_artifact(&entry, &input.proposal.descriptor)
            .map_err(|_| "autonomous execution input targets stale lane geometry")?;
        let bundle = AutonomousLaneMergeBundleV1 {
            version: AutonomousLaneMergeBundleV1::VERSION,
            autonomous,
            certified,
        };
        Self::validate_autonomous_lane_merge_bundle(&bundle, expected_network_id, expected_epoch)?;
        let expected_input = Self::autonomous_lane_block_execution_input_candidate(
            bundle.executable_payload(),
            expected_network_id,
            expected_epoch,
        )
        .map_err(|_| "autonomous payload cannot reconstruct its canonical execution input")?;
        if input != expected_input {
            return Err("durable execution input differs from the certified autonomous payload");
        }
        let source_bundle = bundle
            .encode_framed()
            .map_err(|_| "autonomous merge bundle cannot be canonically encoded")?;
        if require_persisted_bundle {
            let (bundle_data_path, bundle_index_path) =
                Self::autonomous_lane_merge_bundle_paths_for_entry(&entry, &self.store_root);
            let bundle_namespace = self
                .open_bound_progress_namespace(&bundle_data_path, &bundle_index_path)
                .map_err(|_| "autonomous merge bundle pair could not be bound")?;
            self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
                &bundle_namespace,
                &bundle_data_path,
                &bundle_index_path,
                AutonomousLaneMergeBundleV1::FORMAT_LABEL,
            )
            .map_err(|_| "autonomous merge bundle pair has unresolved recovery state")?;
            let mut bundle_pair = self
                .open_bound_progress_pair(&bundle_data_path, &bundle_index_path)
                .map_err(|_| "autonomous merge bundle pair could not be opened")?;
            let persisted = match &mut bundle_pair {
                BoundProgressPair::Absent(_) => None,
                BoundProgressPair::Present(bound) => self
                    .read_autonomous_lane_merge_bundle_from_bound_locked(
                        lane_id,
                        lane_block_height,
                        bound,
                    )
                    .map_err(|_| "persisted autonomous merge bundle is malformed")?,
            }
            .ok_or("durable autonomous merge bundle is unavailable")?;
            if let BoundProgressPair::Present(bound) = &bundle_pair
                && !self.bound_progress_sidecar_unchanged(bound)
            {
                return Err("autonomous merge bundle pair changed during source admission");
            }
            Self::validate_autonomous_lane_merge_bundle(
                &persisted.0,
                expected_network_id,
                expected_epoch,
            )
            .map_err(|_| "persisted autonomous merge bundle is invalid")?;
            if persisted.0 != bundle || persisted.1 != source_bundle {
                return Err(
                    "persisted autonomous merge bundle differs from exact durable components",
                );
            }
        }
        let bundle_hash = bundle
            .bundle_hash()
            .map_err(|_| "autonomous merge bundle cannot be canonically hashed")?;
        // Extract one lossless first-release trace from the exact durable
        // bundle before it becomes merge eligible. The bitmap projection uses
        // the certificate's canonical committee order; no proposer-local or
        // current-topology state participates.
        let descriptor = &bundle.certified.proposal.descriptor;
        let validator_count = u8::try_from(descriptor.validator_set.len())
            .map_err(|_| "autonomous merge committee exceeds the refinement width")?;
        if validator_count == 0 || validator_count > 128 {
            return Err("autonomous merge committee is outside the 1..=128 refinement width");
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let producer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &bundle.executable_payload().producer)
            .ok_or("autonomous payload producer is absent from its certified committee")?;
        let producer = 1_u128
            .checked_shl(
                u32::try_from(producer_index)
                    .map_err(|_| "autonomous producer index exceeds the refinement width")?,
            )
            .ok_or("autonomous producer index exceeds the refinement width")?;
        let bitmap_mask = |bitmap: &[u8]| {
            if bitmap.len() != descriptor.validator_set.len().div_ceil(8) {
                return Err("autonomous certificate bitmap has a noncanonical length");
            }
            let mut mask = 0_u128;
            for (byte_index, byte) in bitmap.iter().copied().enumerate() {
                for bit_index in 0..8_usize {
                    if byte & (1_u8 << bit_index) == 0 {
                        continue;
                    }
                    let index = byte_index
                        .checked_mul(8)
                        .and_then(|base| base.checked_add(bit_index))
                        .ok_or("autonomous certificate bitmap index overflows")?;
                    if index >= descriptor.validator_set.len() {
                        return Err("autonomous certificate bitmap selects a padding bit");
                    }
                    mask |= 1_u128
                        .checked_shl(u32::try_from(index).map_err(
                            |_| "autonomous certificate signer exceeds the refinement width",
                        )?)
                        .ok_or("autonomous certificate signer exceeds the refinement width")?;
                }
            }
            Ok(mask)
        };
        let availability_qc = bundle
            .certified
            .prepare_qc
            .payload_availability_qc
            .as_ref()
            .ok_or("autonomous prepare QC lacks its durable READY certificate")?;
        if availability_qc.validator_set != descriptor.validator_set {
            return Err("autonomous READY committee differs from the lane certificate");
        }
        let ready_signers = bitmap_mask(&availability_qc.signers_bitmap)?;
        let commit_signers = bitmap_mask(&bundle.certified.commit_qc.signers_bitmap)?;
        let lane_commit_candidates = ready_signers & commit_signers;
        if lane_commit_candidates == 0 {
            return Err("autonomous READY and Commit QCs have no common authenticated signer");
        }
        let lane_commit_actor = 1_u128
            .checked_shl(lane_commit_candidates.trailing_zeros())
            .ok_or("autonomous lane commit signer exceeds the refinement width")?;
        let reservation_group = lane_queue_reservation_group_binding_from_ordered_keys(
            bundle.executable_payload().reservation_keys.iter(),
        )
        .map_err(|_| "autonomous reservation group is not canonical")?;
        let selected_count = reservation_group.reservation_count;
        if !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
            .unwrap_or(u64::MAX))
            .contains(&selected_count)
        {
            return Err("autonomous reservation count is outside the first-release bound");
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(reservation_group);
        let payload_owners = ready_signers | producer;
        if payload_owners & !validator_mask != 0 {
            return Err("autonomous payload ownership exceeds its certified committee");
        }
        let trace_base = ProductionInFlightFirstReleaseStateProjection {
            validator_count,
            producer,
            producer_selected_owner: producer,
            replicated_carrier_owners: validator_mask & !producer,
            payload_binding_a: payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: payload_owners,
                execution_input_durable: ready_signers,
                ready_qc_durable: false,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: payload_owners,
                ready_authorized: ready_signers,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v1: true,
                ever_reservation_v1: true,
                ever_execution_input_durable: ready_signers,
                ever_ready_authorized: ready_signers,
                ready_signed: ready_signers,
                ever_ready_qc_durable: false,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };
        // The source reader has already validated exact durable execution input.
        // Extract the READY and lane decisions carried by those same source bytes.
        let mut ready_qc_after = trace_base;
        ready_qc_after.carrier.ready_qc_durable = true;
        ready_qc_after.history.ever_ready_qc_durable = true;
        let ready_qc_projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC,
            actor: 0,
            target: 0,
            before: trace_base,
            after: ready_qc_after,
        };
        let checked_ready_qc =
            check_production_in_flight_first_release_transition(ready_qc_projection)
                .ok_or("durable READY QC failed the composed first-release transition gate")?;
        if checked_ready_qc.into_projection() != ready_qc_projection {
            return Err("checked durable READY-QC projection changed before admission");
        }
        let mut lane_commit_after = ready_qc_after;
        lane_commit_after.decision.lane_commit_scope = binding_a;
        lane_commit_after.decision.lane_commit_owner = lane_commit_actor;
        let lane_commit_projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT,
            actor: lane_commit_actor,
            target: 0,
            before: ready_qc_after,
            after: lane_commit_after,
        };
        let checked_lane_commit =
            check_production_in_flight_first_release_transition(lane_commit_projection)
                .ok_or("lane CommitQC failed the composed first-release transition gate")?;
        if checked_lane_commit.into_projection() != lane_commit_projection {
            return Err("checked lane-commit projection changed before merge admission");
        }
        Ok(DurableAutonomousLaneMergeSource {
            bundle,
            source_bundle,
            bundle_hash,
            input,
        })
    }
    /// Revalidate the exact independently durable source admitted to merge.
    ///
    /// This read never repairs an execution-input or certified data/index
    /// pair. Startup recovery must complete those barriers explicitly before
    /// merge readiness can become visible.
    pub(crate) fn durable_autonomous_lane_merge_source(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> std::result::Result<DurableAutonomousLaneMergeSource, &'static str> {
        let _prune_guard = self.prune_lock.lock();
        self.durable_autonomous_lane_merge_source_under_prune_guard(
            lane_id,
            lane_block_height,
            expected_network_id,
            expected_epoch,
            None,
            true,
        )
    }
    /// Publish one exact canonical autonomous bundle through an independent
    /// strict data/index/directory durability barrier.
    ///
    /// The caller holds `prune_lock`; the source was assembled from the exact
    /// component set protected by that guard. Conflicting active-slot bytes are
    /// immutable corruption and are never overwritten.
    fn persist_autonomous_lane_merge_bundle_under_prune_guard(
        &self,
        source: &DurableAutonomousLaneMergeSource,
    ) -> Result<()> {
        self.durable_mutation_authorized()?;
        let descriptor = &source.bundle.certified.proposal.descriptor;
        Self::validate_autonomous_lane_merge_bundle(
            &source.bundle,
            source.bundle.executable_payload().network_id,
            source.bundle.executable_payload().epoch,
        )
        .map_err(|message| {
            Self::invalid_lane_artifact_error(self.store_root.clone(), message.to_owned())
        })?;
        if source.bundle.encode_framed()? != source.source_bundle
            || source.bundle.bundle_hash()? != source.bundle_hash
        {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous merge source bytes or hash differ from its canonical bundle",
            ));
        }
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(descriptor.lane_id)?;
        self.require_active_lane_artifact(&entry, descriptor)?;
        let (data_path, index_path) =
            Self::autonomous_lane_merge_bundle_paths_for_entry(&entry, &self.store_root);
        let directory = data_path.parent().map(Path::to_path_buf).ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                data_path.clone(),
                "autonomous merge bundle path has no parent directory",
            )
        })?;
        std::fs::create_dir_all(&directory)
            .map_err(|error| Error::MkDir(error, directory.clone()))?;
        let _sidecar_guard = self.sidecar_lock.lock();
        if !self.recover_bound_progress_sidecar_artifacts(
            &data_path,
            &index_path,
            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
        ) {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "autonomous merge bundle pair recovery did not reach a durable fixed point",
            ));
        }
        let namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
        let mut existing_pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let existing_layout = match &mut existing_pair {
            BoundProgressPair::Absent(_) => None,
            BoundProgressPair::Present(bound) => Some(
                self.validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(data_path.clone(), message.to_owned())
                    })?
                    .0,
            ),
        };
        if let BoundProgressPair::Present(bound) = &mut existing_pair
            && let Some((existing, existing_bytes)) = self
                .read_autonomous_lane_merge_bundle_from_bound_locked(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    bound,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(data_path.clone(), message.to_owned())
                })?
        {
            if existing != source.bundle || existing_bytes != source.source_bundle {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    "active autonomous merge bundle slot contains conflicting canonical bytes",
                ));
            }
            if !self.sync_bound_progress_sidecar(bound, AutonomousLaneMergeBundleV1::FORMAT_LABEL) {
                return Err(Error::IO(
                    std::io::Error::other(
                        "failed to make existing autonomous merge bundle durable",
                    ),
                    data_path,
                ));
            }
            self.consume_autonomous_bundle_pair_capacity(source)?;
            return Ok(());
        }
        drop(existing_pair);
        let projected_entry_count = match existing_layout {
            None | Some(SidecarIndexLayout { entry_count: 0, .. }) => 1_u64,
            Some(layout) if descriptor.lane_block_height < layout.base_height => layout
                .entry_count
                .checked_add(layout.base_height - descriptor.lane_block_height)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.clone(),
                        "autonomous merge bundle index growth overflows",
                    )
                })?,
            Some(layout) => layout.entry_count.max(
                descriptor
                    .lane_block_height
                    .checked_sub(layout.base_height)
                    .and_then(|offset| offset.checked_add(1))
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            index_path.clone(),
                            "autonomous merge bundle index height overflows",
                        )
                    })?,
            ),
        };
        if usize::try_from(projected_entry_count).unwrap_or(usize::MAX)
            > self.autonomous_lane_merge_bundle_pair_entry_limit()
        {
            return Err(Self::invalid_lane_artifact_error(
                index_path,
                "autonomous merge bundle index would exceed its bounded entry count",
            ));
        }
        let projected_data_len = Self::file_len_or_zero(&data_path)?
            .checked_add(u64::try_from(source.source_bundle.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    "autonomous merge bundle data growth overflows",
                )
            })?;
        if projected_data_len
            > u64::try_from(self.autonomous_lane_merge_bundle_pair_byte_limit()).unwrap_or(u64::MAX)
        {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "autonomous merge bundle data would exceed its aggregate byte budget",
            ));
        }
        #[cfg(test)]
        if FAIL_NEXT_AUTONOMOUS_MERGE_BUNDLE_PERSISTENCE.with(|flag| flag.replace(false)) {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "injected autonomous merge bundle publication failure",
            ));
        }
        let before_bytes = Self::sidecar_tracked_bytes(&data_path, &index_path, None)?;
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        #[cfg(test)]
        if FAIL_NEXT_AUTONOMOUS_MERGE_BUNDLE_APPEND_DATA_SYNC.with(|flag| flag.replace(false)) {
            FAIL_NEXT_BOUND_PROGRESS_APPEND_DATA_SYNC.with(|flag| flag.set(true));
        }
        if !Self::append_indexed_progress_sidecar(
            &data_path,
            &index_path,
            descriptor.lane_block_height,
            &source.source_bundle,
            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
            None,
            &namespace,
        ) {
            return Err(Error::IO(
                std::io::Error::other("failed to persist autonomous merge bundle"),
                data_path,
            ));
        }
        let mut readback_pair = self.open_bound_progress_pair(&data_path, &index_path)?;
        let readback = match &mut readback_pair {
            BoundProgressPair::Absent(_) => None,
            BoundProgressPair::Present(bound) => self
                .read_autonomous_lane_merge_bundle_from_bound_locked(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    bound,
                )
                .map_err(|message| {
                    Self::invalid_lane_artifact_error(data_path.clone(), message.to_owned())
                })?,
        }
        .ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                data_path.clone(),
                "autonomous merge bundle disappeared after strict publication",
            )
        })?;
        if readback.0 != source.bundle || readback.1 != source.source_bundle {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "autonomous merge bundle changed before durable readback",
            ));
        }
        let after_bytes = Self::sidecar_tracked_bytes(&data_path, &index_path, None)?;
        self.update_disk_usage_delta(before_bytes, after_bytes);
        accounting_mutation.finish();
        #[cfg(test)]
        if FAIL_AFTER_NEXT_AUTONOMOUS_MERGE_BUNDLE_PAIR.with(|flag| flag.replace(false)) {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "injected failure after autonomous merge bundle pair durability",
            ));
        }
        self.consume_autonomous_bundle_pair_capacity(source)?;
        self.note_committed_lane_status_change();
        Ok(())
    }
    /// Reconcile independently durable autonomous merge bundles with the
    /// exact active certified slots that authorize them.
    ///
    /// A crash may publish the certified frontier/pair and stop before the
    /// bundle pair crosses its own data/index/directory barrier. Startup is
    /// the only repair path: it reconstructs such a missing slot from the
    /// authenticated autonomous payload, certificate, and execution input.
    /// Existing conflicting or orphan bundle bytes always fail closed.
    fn repair_autonomous_lane_merge_bundles_on_startup(&self) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let entries = {
            let _geometry_guard = self.lane_geometry_lock.lock();
            self.lane_storage_entries
                .lock()
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        for entry in entries {
            let (certified, persisted_bundles) = {
                let _geometry_guard = self.lane_geometry_lock.lock();
                let active_entry = self.lane_storage_entry(entry.lane_id)?;
                if active_entry != entry {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "lane geometry changed during autonomous merge bundle startup repair",
                    ));
                }
                let _sidecar_guard = self.sidecar_lock.lock();
                self.ensure_prune_recovery_not_required()?;
                let frontier =
                    self.read_latest_certified_lane_block_frontier_locked(&active_entry, true)?;
                if let Some(frontier) = frontier.as_ref() {
                    self.recover_certified_lane_block_pair_from_frontier_locked(
                        &active_entry,
                        &frontier.frontier.artifact,
                        None,
                    )?;
                    self.confirm_latest_certified_lane_block_frontier_read_locked(
                        &active_entry,
                        &frontier.snapshot,
                    )?;
                }
                let (certified_data_path, certified_index_path) =
                    Self::certified_lane_block_paths_for_entry(&active_entry, &self.store_root);
                if !self.recover_bound_progress_sidecar_artifacts(
                    &certified_data_path,
                    &certified_index_path,
                    CertifiedLaneBlockArtifact::FORMAT_LABEL,
                ) {
                    return Err(Self::invalid_lane_artifact_error(
                        certified_data_path,
                        "certified lane block pair failed startup recovery before bundle repair",
                    ));
                }
                let mut certified_pair =
                    self.open_bound_progress_pair(&certified_data_path, &certified_index_path)?;
                let certified = match &mut certified_pair {
                    BoundProgressPair::Absent(namespace) => {
                        if !self.sync_bound_progress_absence(
                            namespace,
                            CertifiedLaneBlockArtifact::FORMAT_LABEL,
                        ) {
                            return Err(Self::invalid_lane_artifact_error(
                                certified_data_path.clone(),
                                "certified lane block absence is not durable during bundle repair",
                            ));
                        }
                        BTreeMap::new()
                    }
                    BoundProgressPair::Present(bound) => {
                        let heights = self.bound_indexed_sidecar_payload_heights(
                            bound,
                            CertifiedLaneBlockArtifact::FORMAT_LABEL,
                            MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES,
                        )?;
                        let mut artifacts = BTreeMap::new();
                        for lane_block_height in heights {
                            let artifact = self
                                .read_active_certified_lane_block_artifact_from_bound_locked(
                                    &active_entry,
                                    lane_block_height,
                                    bound,
                                )
                                .ok_or_else(|| {
                                    Self::invalid_lane_artifact_error(
                                        certified_data_path.clone(),
                                        "certified lane block slot is malformed during autonomous bundle repair",
                                    )
                                })?;
                            artifacts.insert(lane_block_height, artifact);
                        }
                        if !self.sync_bound_progress_sidecar(
                            bound,
                            CertifiedLaneBlockArtifact::FORMAT_LABEL,
                        ) {
                            return Err(Self::invalid_lane_artifact_error(
                                certified_data_path.clone(),
                                "certified lane block pair is not durable during bundle repair",
                            ));
                        }
                        artifacts
                    }
                };
                if frontier.is_none() && !certified.is_empty() {
                    return Err(Self::invalid_lane_artifact_error(
                        certified_data_path,
                        "certified lane block history exists without its mandatory durable frontier",
                    ));
                }
                let (bundle_data_path, bundle_index_path) =
                    Self::autonomous_lane_merge_bundle_paths_for_entry(
                        &active_entry,
                        &self.store_root,
                    );
                if !self.recover_bound_progress_sidecar_artifacts(
                    &bundle_data_path,
                    &bundle_index_path,
                    AutonomousLaneMergeBundleV1::FORMAT_LABEL,
                ) {
                    return Err(Self::invalid_lane_artifact_error(
                        bundle_data_path,
                        "autonomous merge bundle pair failed startup recovery",
                    ));
                }
                let mut bundle_pair =
                    self.open_bound_progress_pair(&bundle_data_path, &bundle_index_path)?;
                let bundles = match &mut bundle_pair {
                    BoundProgressPair::Absent(namespace) => {
                        if !self.sync_bound_progress_absence(
                            namespace,
                            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
                        ) {
                            return Err(Self::invalid_lane_artifact_error(
                                bundle_data_path.clone(),
                                "autonomous merge bundle absence is not durable during startup repair",
                            ));
                        }
                        BTreeMap::new()
                    }
                    BoundProgressPair::Present(bound) => {
                        let (_, heights) = self
                            .validate_autonomous_lane_merge_bundle_pair_layout_locked(bound)
                            .map_err(|message| {
                                Self::invalid_lane_artifact_error(
                                    bundle_data_path.clone(),
                                    message.to_owned(),
                                )
                            })?;
                        let mut bundles = BTreeMap::new();
                        for lane_block_height in heights {
                            let (bundle, _) = self
                                .read_autonomous_lane_merge_bundle_from_bound_locked(
                                    active_entry.lane_id,
                                    lane_block_height,
                                    bound,
                                )
                                .map_err(|message| {
                                    Self::invalid_lane_artifact_error(
                                        bundle_data_path.clone(),
                                        message.to_owned(),
                                    )
                                })?
                                .ok_or_else(|| {
                                    Self::invalid_lane_artifact_error(
                                        bundle_data_path.clone(),
                                        "enumerated autonomous merge bundle slot disappeared",
                                    )
                                })?;
                            self.require_active_lane_artifact(
                                &active_entry,
                                &bundle.certified.proposal.descriptor,
                            )?;
                            bundles.insert(lane_block_height, bundle);
                        }
                        if !self.sync_bound_progress_sidecar(
                            bound,
                            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
                        ) {
                            return Err(Self::invalid_lane_artifact_error(
                                bundle_data_path.clone(),
                                "autonomous merge bundle pair is not durable during startup repair",
                            ));
                        }
                        bundles
                    }
                };
                (certified, bundles)
            };
            for lane_block_height in persisted_bundles.keys() {
                let Some(artifact) = certified.get(lane_block_height) else {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "autonomous merge bundle exists without its exact certified lane slot",
                    ));
                };
                if artifact.prepare_qc.payload_availability_qc.is_none() {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "autonomous merge bundle exists for a non-autonomous certificate",
                    ));
                }
            }
            for (lane_block_height, artifact) in certified {
                let Some(availability) = artifact.prepare_qc.payload_availability_qc.as_ref()
                else {
                    continue;
                };
                if let Some(bundle) = persisted_bundles.get(&lane_block_height) {
                    let input = self
                        .read_active_lane_block_execution_input_structural(
                            entry.lane_id,
                            lane_block_height,
                            false,
                        )
                        .ok_or_else(|| {
                            Self::invalid_lane_artifact_error(
                                self.store_root.clone(),
                                "persisted autonomous merge bundle has no exact durable execution input",
                            )
                        })?;
                    let expected_input =
                        Self::autonomous_lane_block_execution_input_candidate(
                            bundle.executable_payload(),
                            availability.body.network_id,
                            availability.body.epoch,
                        )
                        .map_err(|_| {
                            Self::invalid_lane_artifact_error(
                                self.store_root.clone(),
                                "persisted autonomous merge bundle cannot reconstruct its execution input",
                            )
                        })?;
                    if bundle.certified != artifact || input != expected_input {
                        return Err(Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            "persisted autonomous merge bundle differs from its certified slot or execution input",
                        ));
                    }
                    let published = self
                        .durable_autonomous_lane_merge_source_under_prune_guard(
                            entry.lane_id,
                            lane_block_height,
                            availability.body.network_id,
                            availability.body.epoch,
                            None,
                            true,
                        )
                        .map_err(|message| {
                            Self::invalid_lane_artifact_error(
                                self.store_root.clone(),
                                format!(
                                    "persisted autonomous merge bundle startup readback failed: {message}"
                                ),
                            )
                        })?;
                    self.ensure_certified_bundle_capacity_reservation_under_prune_guard(
                        &artifact, &published, None,
                    )?;
                    continue;
                }
                let source = self
                    .durable_autonomous_lane_merge_source_under_prune_guard(
                        entry.lane_id,
                        lane_block_height,
                        availability.body.network_id,
                        availability.body.epoch,
                        None,
                        false,
                    )
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            format!(
                                "autonomous merge bundle startup reconstruction failed: {message}"
                            ),
                        )
                    })?;
                if source.bundle.certified != artifact {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup autonomous merge source retained another certificate",
                    ));
                }
                self.persist_autonomous_lane_merge_bundle_under_prune_guard(&source)?;
                let published = self
                    .durable_autonomous_lane_merge_source_under_prune_guard(
                        entry.lane_id,
                        lane_block_height,
                        availability.body.network_id,
                        availability.body.epoch,
                        None,
                        true,
                    )
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            format!("autonomous merge bundle startup readback failed: {message}"),
                        )
                    })?;
                if published != source {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "startup autonomous merge bundle changed during durable publication",
                    ));
                }
                self.ensure_certified_bundle_capacity_reservation_under_prune_guard(
                    &artifact, &published, None,
                )?;
            }
        }
        if self.certified_bundle_capacity_reserved_bytes()? != 0 {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "autonomous merge bundle startup repair left an outstanding certified/bundle reservation",
            ));
        }
        Ok(())
    }
    /// Authenticate the exact durable execution-input readback and mint one
    /// move-only autonomous READY signing authority.
    ///
    /// This read is deliberately repair-disabled: a missing input must cross
    /// the normal persistence barriers before READY authorization. The
    /// returned value binds the complete canonical artifact, ordered
    /// reservation group, proposal/payload identity, signer, and historical or
    /// current height-context session.
    pub(crate) fn mint_lane_ready_authorization(
        &self,
        payload: &LaneExecutablePayloadV1,
        proposal: &LaneBlockProposalV1,
        availability_body: &LanePayloadAvailabilityBodyV1,
        signer: &PeerId,
        height_context_id: HeightContextId,
    ) -> std::result::Result<LaneReadyAuthorization, &'static str> {
        let descriptor = &proposal.descriptor;
        if payload.origin_proposal != *proposal {
            return Err("READY payload does not name the exact proposal");
        }
        let expected_availability =
            lane_payload_availability_body(payload, proposal, payload.network_id, payload.epoch)
                .map_err(|_| "READY payload or proposal is invalid")?;
        if expected_availability != *availability_body {
            return Err("READY body differs from the exact payload and proposal");
        }
        if !descriptor.validator_set.contains(signer)
            || signer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
        {
            return Err("READY signer is not a BLS-normal member of the exact committee");
        }
        let context_suffix = format!(
            "::height-context:{}::epoch:{}::lane-relay:v1:{}:{}",
            hex::encode(height_context_id.0.as_ref()),
            payload.epoch,
            descriptor.dataspace_id.as_u64(),
            descriptor.lane_id.as_u32(),
        );
        if !availability_body.qc_mode_tag.ends_with(&context_suffix) {
            return Err("READY height-context session differs from the proposal");
        }
        let durable = self
            .read_lane_block_execution_input_with_repair_policy(
                descriptor.lane_id,
                descriptor.lane_block_height,
                false,
            )
            .ok_or("READY execution input is not durably readable")?;
        if durable.proposal != *proposal {
            return Err("READY execution input names another proposal or incarnation");
        }
        if durable.source.autonomous_binding()
            != Some((payload.network_id, payload.epoch, payload.payload_hash))
            || durable.entrypoint_hashes != payload.entrypoint_hashes
            || durable.entrypoints != payload.entrypoints
            || durable.reservation_keys != payload.reservation_keys
            || durable.routing_plans != payload.routing_plans
            || durable.native_amx_receipts != payload.native_amx_receipts
        {
            return Err("READY execution input differs from the executable payload");
        }
        let reservation_group =
            lane_queue_reservation_group_binding_from_ordered_keys(durable.reservation_keys.iter())
                .map_err(|_| "READY execution input has an invalid reservation group")?;
        let (reservation_owner_hash, proposal_identity_hash) =
            autonomous_lane_reservation_identity_hashes_for_proposal(
                payload.network_id,
                height_context_id,
                payload.epoch,
                proposal,
                &payload.producer,
            )
            .map_err(|_| "READY reservation group has an invalid signer session")?;
        if reservation_group.identity.lane_id != descriptor.lane_id
            || reservation_group.identity.dataspace_id != descriptor.dataspace_id
            || reservation_group.identity.lane_incarnation != descriptor.lane_incarnation
            || reservation_group.identity.proposal_height != descriptor.proposal_height
            || reservation_group.identity.lane_block_height != descriptor.lane_block_height
            || reservation_group.identity.lane_block_view != descriptor.lane_block_view
            || reservation_group.identity.reservation_owner_hash != reservation_owner_hash
            || reservation_group.identity.proposal_identity_hash != proposal_identity_hash
            || reservation_group.reservation_count
                != u64::try_from(payload.entrypoints.len()).unwrap_or(u64::MAX)
        {
            return Err("READY reservation group differs from the proposal session");
        }
        let durable_bytes = norito::encode_canonical(&durable)
            .map_err(|_| "READY execution input cannot be canonically hashed")?;
        let durable_execution_input_hash = Hash::new_from_chunks(&[
            LANE_READY_EXECUTION_INPUT_AUTHORIZATION_DOMAIN_V1,
            durable_bytes.as_slice(),
        ]);
        // Project the exact committee positions named by the authenticated
        // payload and signing request. The checked token is consumed only
        // after the repair-disabled durable input and full reservation group
        // have both been revalidated.
        let validator_count = u8::try_from(descriptor.validator_set.len())
            .map_err(|_| "READY committee exceeds the refinement width")?;
        if validator_count == 0 || validator_count > 128 {
            return Err("READY committee is outside the 1..=128 refinement width");
        }
        let validator_mask = if validator_count == 128 {
            u128::MAX
        } else {
            (1_u128 << validator_count) - 1
        };
        let producer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == &payload.producer)
            .ok_or("READY payload producer is absent from its committee")?;
        let signer_index = descriptor
            .validator_set
            .iter()
            .position(|peer| peer == signer)
            .ok_or("READY signer is absent from its committee")?;
        let producer = 1_u128
            .checked_shl(
                u32::try_from(producer_index)
                    .map_err(|_| "READY producer index exceeds the refinement width")?,
            )
            .ok_or("READY producer index exceeds the refinement width")?;
        let actor = 1_u128
            .checked_shl(
                u32::try_from(signer_index)
                    .map_err(|_| "READY signer index exceeds the refinement width")?,
            )
            .ok_or("READY signer index exceeds the refinement width")?;
        let selected_count = reservation_group.reservation_count;
        if !(1..=u64::try_from(iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
            .unwrap_or(u64::MAX))
            .contains(&selected_count)
        {
            return Err("READY reservation count is outside the first-release bound");
        }
        let binding_a =
            canonical_lane_queue_reservation_group_identity_projection(reservation_group);
        let payload_owners = producer | actor;
        let before = ProductionInFlightFirstReleaseStateProjection {
            validator_count,
            producer,
            producer_selected_owner: producer,
            replicated_carrier_owners: validator_mask & !producer,
            payload_binding_a: payload_owners,
            binding_a,
            queue: ProductionInFlightFirstReleaseQueueProjection {
                plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
                selected_count,
                reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            },
            carrier: ProductionInFlightFirstReleaseCarrierProjection {
                kura_active: payload_owners,
                execution_input_durable: actor,
                ready_qc_durable: false,
            },
            session: ProductionInFlightFirstReleaseSessionProjection {
                bodies: payload_owners,
                ready_authorized: 0,
                crashed: 0,
                producer_alive: true,
            },
            history: ProductionInFlightFirstReleaseHistoryProjection {
                ever_queue_plan_v1: true,
                ever_reservation_v1: true,
                ever_execution_input_durable: actor,
                ..ProductionInFlightFirstReleaseHistoryProjection::default()
            },
            decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
            release: ProductionInFlightFirstReleaseReleaseProjection::default(),
        };
        let mut after = before;
        after.session.ready_authorized = actor;
        after.history.ever_ready_authorized = actor;
        let projection = ProductionInFlightFirstReleaseTransitionProjection {
            action: IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY,
            actor,
            target: 0,
            before,
            after,
        };
        let checked = check_production_in_flight_first_release_transition(projection)
            .ok_or("READY authorization failed the composed first-release transition gate")?;
        if checked.into_projection() != projection {
            return Err("checked READY authorization projection changed before minting");
        }
        Ok(LaneReadyAuthorization {
            durable_execution_input_hash,
            proposal: proposal.clone(),
            availability_body: availability_body.clone(),
            reservation_group,
            producer: payload.producer.clone(),
            signer: signer.clone(),
            height_context_id,
        })
    }
}
