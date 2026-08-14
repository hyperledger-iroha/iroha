// In-flight reservation and first-release refinement proofs.
//
// Included lexically by `verus_proofs` so public item paths remain unchanged.
verus! {
/// Verus-side lossless four-word projection of one 256-bit digest.
#[derive(Copy, Clone)]
pub struct ProductionDigest256Projection {
    pub word0: u64,
    pub word1: u64,
    pub word2: u64,
    pub word3: u64,
}
/// Verus-side mirror of the production V1 transition witness.
///
/// SHA-256 recomputation is intentionally kept in the production wrapper and
/// its source-bound contract; the reviewed cryptography ledger row remains the
/// trusted boundary. This mirror proves that the version, parameters, and TLA+
/// source identity refine the same composed transition kernel.
#[derive(Copy, Clone)]
pub struct ProductionInFlightFirstReleaseTransitionWitnessV1 {
    pub schema_version: u16,
    pub action: u8,
    pub actor: u128,
    pub target: u128,
    pub before_state_digest: ProductionDigest256Projection,
    pub after_state_digest: ProductionDigest256Projection,
    pub source_identity: ProductionDigest256Projection,
}
/// Exact Verus mirror of the production V1 witness's structural binding.
pub closed spec fn production_in_flight_first_release_witness_binding_kernel(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    witness: ProductionInFlightFirstReleaseTransitionWitnessV1,
) -> bool {
    production_in_flight_first_release_witness_binding_body!(projection, witness)
}
/// Verus-side reverse classification of one terminal economic owner.
#[derive(Copy, Clone)]
pub struct ProductionInFlightFirstReleaseTerminalOwnerProjection {
    pub ordinary_fifo_owner: bool,
    pub canonical_wsv_owner: bool,
    pub commit_terminal: bool,
    pub release_terminal: bool,
}
/// A successful primitive checker result is exactly the shared executable
/// identity/state relation. Collection extraction and cross-store ordering are
/// deliberately outside this theorem.
pub proof fn production_in_flight_reservation_transition_preserves_primitive_identity(
    projection: ProductionInFlightReservationTransitionProjection,
)
    ensures
        check_production_in_flight_reservation_transition(projection) == Some(projection)
            ==> production_in_flight_reservation_transition_kernel(projection),
{
    reveal(check_production_in_flight_reservation_transition);
}
/// Every checked primitive snapshot reconstruction is noninterference for any
/// independently valid composed first-release state.
///
/// The primitive transition may rebuild any exact Live, Committed, Prepared,
/// or Completed owner. Because those process-local indexes do not alter a
/// QueuePlan, carrier, session, history, decision, or release fact, its only
/// composed refinement is the named stutter over the caller's unchanged valid
/// state. No total composed state is reconstructed from journal bytes here.
pub proof fn production_in_flight_reservation_snapshot_replay_refines_composed_stutter(
    primitive: ProductionInFlightReservationTransitionProjection,
    composed: ProductionInFlightFirstReleaseStateProjection,
)
    requires
        primitive.action
            == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT),
        production_in_flight_reservation_transition_kernel(primitive),
        production_in_flight_first_release_state_kernel(composed),
    ensures
        production_in_flight_first_release_transition_kernel(
            ProductionInFlightFirstReleaseTransitionProjection {
                action: refinement_tag_value!(
                    IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT
                ),
                actor: 0u128,
                target: 0u128,
                before: composed,
                after: composed,
            },
        ),
{
    reveal(production_in_flight_reservation_transition_kernel);
    reveal(production_in_flight_first_release_state_kernel);
    reveal(production_in_flight_first_release_transition_kernel);
}
/// Runtime commit cannot fabricate a commit barrier from absent ownership.
pub proof fn production_in_flight_reservation_commit_rejects_absent_owner(
    projection: ProductionInFlightReservationTransitionProjection,
)
    requires
        projection.action == refinement_tag_value!(IN_FLIGHT_RESERVATION_ACTION_COMMIT),
        projection.before.state == refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_ABSENT),
    ensures
        !production_in_flight_reservation_transition_kernel(projection),
{
    reveal(production_in_flight_reservation_transition_kernel);
}
/// The composed checker accepts only the shared complete state/action kernel.
pub proof fn production_in_flight_first_release_transition_refines_named_next(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
)
    ensures
        check_production_in_flight_first_release_transition(projection) == Some(projection)
            ==> production_in_flight_first_release_transition_kernel(projection),
{
    reveal(check_production_in_flight_first_release_transition);
}
/// A structurally authenticated V1 witness names the exact checked action,
/// parameters, and reviewed TLA+ source while refining the same composed
/// relation as every production mutation boundary.
///
/// Exact pre/post digest recomputation is deliberately a separate production
/// obligation: the outer authenticator rebuilds the entire witness from the
/// canonical state encoding. SHA-256 itself remains the reviewed cryptography
/// trusted contract and is not restated as arithmetic in Verus.
pub proof fn production_in_flight_first_release_witness_refines_named_next(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
    witness: ProductionInFlightFirstReleaseTransitionWitnessV1,
)
    requires
        production_in_flight_first_release_transition_kernel(projection),
        production_in_flight_first_release_witness_binding_kernel(projection, witness),
    ensures
        witness.schema_version == 1u16,
        witness.action == projection.action,
        witness.actor == projection.actor,
        witness.target == projection.target,
        witness.source_identity.word0 == 0x9b9babea9e018b44u64,
        witness.source_identity.word1 == 0xfb739f96b2690f17u64,
        witness.source_identity.word2 == 0xe1f8d08aa23a38f4u64,
        witness.source_identity.word3 == 0x2a16ecef1e858f7du64,
        production_in_flight_first_release_transition_kernel(projection),
{
    reveal(production_in_flight_first_release_witness_binding_kernel);
}
/// Snapshot reconstruction cannot manufacture a new abstract durable owner.
pub proof fn production_in_flight_first_release_snapshot_recovery_is_stutter(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
)
    requires
        projection.action
            == refinement_tag_value!(
                IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT
            ),
        production_in_flight_first_release_transition_kernel(projection),
    ensures
        in_flight_first_release_state_equal_body!(projection.before, projection.after),
{
    reveal(production_in_flight_first_release_transition_kernel);
}
/// Local Kura custody recovery restores exactly one actor's volatile body and
/// cannot manufacture READY authority or another durable/economic fact.
pub proof fn production_in_flight_first_release_local_kura_rehydration_is_exact(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
)
    requires
        projection.action
            == refinement_tag_value!(
                IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY
            ),
        production_in_flight_first_release_transition_kernel(projection),
    ensures
        projection.target == 0u128,
        (projection.before.session.crashed & projection.actor) == 0u128,
        (projection.before.carrier.kura_active & projection.actor) != 0u128,
        (projection.before.session.bodies & projection.actor) == 0u128,
        projection.after.session.bodies
            == (projection.before.session.bodies | projection.actor),
        projection.after.session.ready_authorized
            == projection.before.session.ready_authorized,
        projection.after.session.crashed == projection.before.session.crashed,
        projection.after.session.producer_alive
            == if projection.actor == projection.before.producer {
                true
            } else {
                projection.before.session.producer_alive
            },
        in_flight_first_release_queue_equal_body!(
            projection.before.queue,
            projection.after.queue
        ),
        in_flight_first_release_carrier_equal_body!(
            projection.before.carrier,
            projection.after.carrier
        ),
        in_flight_first_release_history_equal_body!(
            projection.before.history,
            projection.after.history
        ),
        in_flight_first_release_decision_equal_body!(
            projection.before.decision,
            projection.after.decision
        ),
        in_flight_first_release_release_equal_body!(
            projection.before.release,
            projection.after.release
        ),
{
    reveal(production_in_flight_first_release_transition_kernel);
}
/// Kura rehydration fails closed when the actor lacks the exact durable payload.
pub proof fn production_in_flight_first_release_local_kura_rehydration_rejects_missing_payload(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
)
    requires
        projection.action
            == refinement_tag_value!(
                IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY
            ),
        (projection.before.carrier.kura_active & projection.actor) == 0u128,
    ensures
        !production_in_flight_first_release_transition_kernel(projection),
{
    reveal(production_in_flight_first_release_transition_kernel);
}
/// Kura rehydration cannot omit body publication or invent READY authority.
pub proof fn production_in_flight_first_release_local_kura_rehydration_rejects_volatile_drift(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
)
    requires
        projection.action
            == refinement_tag_value!(
                IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY
            ),
        projection.after.session.bodies
                != (projection.before.session.bodies | projection.actor)
            || projection.after.session.ready_authorized
                != projection.before.session.ready_authorized,
    ensures
        !production_in_flight_first_release_transition_kernel(projection),
{
    reveal(production_in_flight_first_release_transition_kernel);
}
/// Terminal economic ownership and Kura retirement are resurrection barriers.
pub proof fn production_in_flight_first_release_local_kura_rehydration_rejects_terminal_state(
    projection: ProductionInFlightFirstReleaseTransitionProjection,
)
    requires
        projection.action
            == refinement_tag_value!(
                IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY
            ),
        projection.before.release.kura_retired
            || projection.before.decision.wsv_committed
            || projection.before.decision.release_owner != 0u128
            || projection.before.queue.reservation_state
                == refinement_tag_value!(
                    IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN
                )
            || projection.before.queue.reservation_state
                == refinement_tag_value!(
                    IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN
                )
            || projection.before.queue.reservation_state
                == refinement_tag_value!(
                    IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED
                ),
    ensures
        !production_in_flight_first_release_transition_kernel(projection),
{
    reveal(production_in_flight_first_release_transition_kernel);
}
/// Every extracted terminal owner is exclusive: WSV for commit, FIFO for release.
pub proof fn production_in_flight_first_release_terminal_owner_is_exclusive(
    state: ProductionInFlightFirstReleaseStateProjection,
    terminal: ProductionInFlightFirstReleaseTerminalOwnerProjection,
)
    requires
        production_in_flight_first_release_terminal_owner(state) == Some(terminal),
    ensures
        terminal.ordinary_fifo_owner != terminal.canonical_wsv_owner,
        terminal.commit_terminal != terminal.release_terminal,
        terminal.canonical_wsv_owner ==> (
            state.history.reservation_commit_forgotten_prefix
                == state.queue.selected_count
        ),
{
    reveal(production_in_flight_first_release_terminal_owner);
}
} // verus!
