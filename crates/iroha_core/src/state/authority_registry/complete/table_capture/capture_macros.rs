// Scoped adapters retain the exact State pool; resource policy grants no authority.

// Every invocation names an actual typed World storage field. The fixed catalog
// below checks identity and order against the complete nested registry before
// any reader can run. Each reader still issues only a scoped, non-finalized pair.
macro_rules! capture_world_table_once {
    ($vis:vis $name:ident, $field:ident, $identity:literal) => {
        #[doc = concat!("Capture the complete native rows of `", $identity, "` at a stable generation.")]
        $vis fn $name(
            state: &State,
            limits: LeafLimits,
        ) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
            let generation = state.state_view_generation();
            if generation & 1 != 0 {
                return Ok(None);
            }
            let budget = state.pipeline_ivm_prepared_cache.read().execution_budget().clone();
            let rows = state.world.$field.view();
            let snapshot =
                CanonicalTableLeafSet::paired_table_from_rows($identity, limits, &budget, rows.iter())?;
            drop(rows);
            if !is_stable_state_view_generation(generation, state.state_view_generation()) {
                return Ok(None);
            }
            Ok(Some(snapshot))
        }
    };
}

// Trigger action and contract values have owner-specific semantic preimages.
// The Set validates and encodes them; a raw Storage value cannot substitute.
macro_rules! capture_trigger_semantic_once {
    ($name:ident, $method:ident) => {
        fn $name(
            state: &State,
            limits: LeafLimits,
        ) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
            let generation = state.state_view_generation();
            if generation & 1 != 0 {
                return Ok(None);
            }
            let budget = state
                .pipeline_ivm_prepared_cache
                .read()
                .execution_budget()
                .clone();
            let snapshot = state.world.triggers.$method(limits, &budget)?;
            if !is_stable_state_view_generation(generation, state.state_view_generation()) {
                return Ok(None);
            }
            Ok(Some(snapshot))
        }
    };
}
