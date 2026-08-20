/// Derive and check the exact `RepairPostCarrierEvidence` stutter.
///
/// Post-carrier repair is authorized only after canonical WSV application and
/// cannot change any composed safety fact.
#[must_use]
pub(crate) fn check_production_in_flight_first_release_repair_post_carrier_evidence_transition(
    before: ProductionInFlightFirstReleaseStateProjection,
) -> Option<CheckedProductionTransition<ProductionInFlightFirstReleaseTransitionProjection>> {
    check_derived_production_in_flight_first_release_transition(
        IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER,
        0,
        0,
        before,
        before,
    )
}
