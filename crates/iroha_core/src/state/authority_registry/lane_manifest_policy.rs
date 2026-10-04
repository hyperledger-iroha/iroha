//! Canonical lane-manifest authority captured from one actual State generation.
//!
//! The installed registry is the source owner. The canonical runtime and World
//! catalog must project the same effective policy before this cell can be used
//! in a future complete State root. This function does not publish such a root.

use crate::state::{State, StateViewError, is_stable_state_view_generation};

/// A local read retains its original release source separately from invalid authority.
#[derive(Debug, thiserror::Error)]
pub(super) enum StateAuthorityCaptureError {
    /// The original State view could not yet be captured or was invalid.
    #[error("State authority view: {0}")]
    View(#[from] StateViewError),
    /// Materialized authority or its canonical encoding was invalid.
    #[error("{0}")]
    Invalid(String),
}

impl From<String> for StateAuthorityCaptureError {
    fn from(reason: String) -> Self {
        Self::Invalid(reason)
    }
}
impl From<&str> for StateAuthorityCaptureError {
    fn from(reason: &str) -> Self {
        Self::Invalid(reason.into())
    }
}

/// Capture one exact, materialized State authority preimage.
///
/// Concurrent publication and physical contention retain the original State
/// release source. A provisional emergency registry or a source/catalog
/// substitution is invalid even when its effective status appears empty.
pub(super) fn canonical_preimage_once(
    state: &State,
) -> Result<Option<Vec<u8>>, StateAuthorityCaptureError> {
    let publication_release = state.view_publication_release();
    let generation = state.state_view_generation();
    if generation & 1 != 0 {
        return Err(StateViewError::Busy(publication_release).into());
    }
    let installed = state.lane_manifests.read().clone();
    let materialized = installed.validate_materialized_source_projection();
    let view = state.try_view_once();
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Err(StateViewError::Busy(publication_release).into());
    }
    // Report a stable provisional source first, even if the derived runtime
    // view also refuses to project it. A racing publication instead retries.
    materialized?;
    let view = view?;
    let installed_bytes = installed.canonical_materialized_authority_preimage(
        &view.nexus.lane_catalog,
        &view.nexus.governance,
    );
    let projected_bytes = view
        .lane_manifests
        .canonical_materialized_authority_preimage(
            &view.nexus.lane_catalog,
            &view.nexus.governance,
        );
    if !is_stable_state_view_generation(generation, state.state_view_generation()) {
        return Err(StateViewError::Busy(publication_release).into());
    }
    let installed_bytes = installed_bytes?;
    let projected_bytes = projected_bytes?;
    if installed_bytes != projected_bytes {
        return Err(
            "installed lane manifest authority differs from canonical runtime and World projection"
                .into(),
        );
    }
    Ok(Some(installed_bytes))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use super::*;
    use crate::{
        governance::manifest::LaneManifestRegistry, kura::Kura, query::store::LiveQueryStore,
        state::World,
    };

    fn state() -> State {
        State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    #[test]
    fn materialized_installed_authority_matches_exact_runtime_projection() {
        let state = state();
        let nexus = state.nexus_snapshot();
        let manifests = Arc::new(LaneManifestRegistry::from_config(
            &nexus.lane_catalog,
            &nexus.governance,
            &nexus.registry,
        ));
        state
            .install_materialized_lane_manifests_for_catalog(
                &manifests,
                &nexus.lane_catalog,
                &nexus.governance,
            )
            .expect("materialized source is authorized");
        let encoded = canonical_preimage_once(&state)
            .expect("actual State projection")
            .expect("stable generation");
        assert_eq!(
            encoded,
            manifests
                .canonical_materialized_authority_preimage(&nexus.lane_catalog, &nexus.governance,)
                .expect("exact installed authority")
        );
    }

    #[test]
    fn authority_captures_preserve_exact_state_reader_refusal() {
        let state = state();
        let nexus = state.nexus_snapshot();
        let manifests = Arc::new(LaneManifestRegistry::from_config(
            &nexus.lane_catalog,
            &nexus.governance,
            &nexus.registry,
        ));
        state
            .install_materialized_lane_manifests_for_catalog(
                &manifests,
                &nexus.lane_catalog,
                &nexus.governance,
            )
            .unwrap();
        let original = state.latest_block_header.write();
        let expected = state.latest_block_header.try_read_or_wait().err().unwrap();
        for attempt in [
            canonical_preimage_once(&state),
            super::super::nexus_policy::canonical_preimage_once(&state),
        ] {
            let Err(StateAuthorityCaptureError::View(StateViewError::Busy(actual))) = attempt
            else {
                panic!("original physical reader refusal must remain typed");
            };
            assert_eq!(actual, expected);
        }
        drop(original);
        assert!(canonical_preimage_once(&state).unwrap().is_some());
        assert!(
            super::super::nexus_policy::canonical_preimage_once(&state)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn provisional_and_status_only_installed_authority_cannot_be_encoded() {
        let mut emergency = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing_in_emergency_fast_mode(),
            LiveQueryStore::start_test(),
        );
        emergency
            .install_provisional_empty_lane_manifests_for_emergency_fast_pre_auth()
            .expect("provisional pre-authentication install");
        assert!(
            canonical_preimage_once(&emergency)
                .unwrap_err()
                .to_string()
                .contains("materialized frozen source")
        );
        let state = state();
        state.install_lane_manifests_for_testing(&Arc::new(LaneManifestRegistry::from_statuses(
            BTreeMap::new(),
        )));
        assert!(
            canonical_preimage_once(&state)
                .unwrap_err()
                .to_string()
                .contains("materialized frozen source")
        );
    }
}
