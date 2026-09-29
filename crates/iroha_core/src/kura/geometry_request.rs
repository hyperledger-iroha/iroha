// Exact native geometry input and original namespace creation custody.

pub(crate) struct GeometryBindingRequest<'a> {
    pub previous: &'a LaneConfig,
    pub updated: &'a LaneConfig,
    pub previous_incarnations: &'a BTreeMap<LaneId, Hash>,
    pub updated_incarnations: &'a BTreeMap<LaneId, Hash>,
    pub previous_activation_heights: &'a BTreeMap<LaneId, u64>,
    pub updated_activation_heights: &'a BTreeMap<LaneId, u64>,
    pub previous_lineage_root: Hash,
    pub updated_lineage_root: Hash,
    pub transition_height: u64,
}

impl GeometryBindingRequest<'_> {
    /// Validate the currently supported storage transition before taking custody.
    ///
    /// TODO(S6): removal and reincarnation require the native authenticated close
    /// frontier and original execution owner. No retired drain receipt grants it.
    pub(crate) fn validate_additions_only(&self, replaced: &BTreeSet<LaneId>) -> Result<()> {
        let reject = |message: &'static str| {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidInput, message),
                PathBuf::new(),
            )
        };
        if !replaced.is_empty() {
            return Err(reject(
                "native geometry replacement requires authenticated close authority",
            ));
        }
        for (config, incarnations, activations) in [
            (
                self.previous,
                self.previous_incarnations,
                self.previous_activation_heights,
            ),
            (
                self.updated,
                self.updated_incarnations,
                self.updated_activation_heights,
            ),
        ] {
            let entries = config.entries();
            if entries.len() != incarnations.len()
                || entries.len() != activations.len()
                || entries.iter().any(|entry| {
                    !incarnations.contains_key(&entry.lane_id)
                        || !activations.contains_key(&entry.lane_id)
                })
            {
                return Err(reject(
                    "native geometry metadata must cover the exact lane catalog",
                ));
            }
        }
        for previous in self.previous.entries() {
            let Some(updated) = self.updated.entry(previous.lane_id) else {
                return Err(reject(
                    "native geometry removal requires authenticated close authority",
                ));
            };
            if previous.dataspace_id != updated.dataspace_id
                || self.previous_incarnations.get(&previous.lane_id)
                    != self.updated_incarnations.get(&previous.lane_id)
                || self.previous_activation_heights.get(&previous.lane_id)
                    != self.updated_activation_heights.get(&previous.lane_id)
            {
                return Err(reject(
                    "native geometry cannot replace a retained lane binding",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod geometry_request_tests {
    use super::*;
    use iroha_data_model::nexus::{LaneCatalog, LaneConfig as ModelLaneConfig};
    use std::num::NonZeroU32;

    fn with_request(test: impl FnOnce(&GeometryBindingRequest<'_>)) {
        let first = ModelLaneConfig::default();
        let second = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "next-lane".into(),
            ..ModelLaneConfig::default()
        };
        let count = NonZeroU32::new(2).unwrap();
        let previous =
            LaneConfig::from_catalog(&LaneCatalog::new(count, vec![first.clone()]).unwrap());
        let updated =
            LaneConfig::from_catalog(&LaneCatalog::new(count, vec![first, second]).unwrap());
        let previous_incarnations = BTreeMap::from([(LaneId::SINGLE, Hash::new(b"first"))]);
        let updated_incarnations = BTreeMap::from([
            (LaneId::SINGLE, Hash::new(b"first")),
            (LaneId::new(1), Hash::new(b"next")),
        ]);
        let previous_activation_heights = BTreeMap::from([(LaneId::SINGLE, 0)]);
        let updated_activation_heights = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 9)]);
        test(&GeometryBindingRequest {
            previous: &previous,
            updated: &updated,
            previous_incarnations: &previous_incarnations,
            updated_incarnations: &updated_incarnations,
            previous_activation_heights: &previous_activation_heights,
            updated_activation_heights: &updated_activation_heights,
            previous_lineage_root: Hash::new(b"previous"),
            updated_lineage_root: Hash::new(b"updated"),
            transition_height: 9,
        });
    }

    #[test]
    fn exact_addition_is_admitted_but_retirement_and_replacement_are_rejected() {
        with_request(|request| {
            request.validate_additions_only(&BTreeSet::new()).unwrap();
            assert!(
                request
                    .validate_additions_only(&BTreeSet::from([LaneId::SINGLE]))
                    .is_err()
            );
            let removal = GeometryBindingRequest {
                previous: request.updated,
                updated: request.previous,
                previous_incarnations: request.updated_incarnations,
                updated_incarnations: request.previous_incarnations,
                previous_activation_heights: request.updated_activation_heights,
                updated_activation_heights: request.previous_activation_heights,
                previous_lineage_root: request.updated_lineage_root,
                updated_lineage_root: request.previous_lineage_root,
                transition_height: 10,
            };
            assert!(removal.validate_additions_only(&BTreeSet::new()).is_err());
        });
    }

    #[test]
    fn retained_bindings_and_metadata_coverage_are_exact() {
        with_request(|request| {
            let mut incarnations = request.updated_incarnations.clone();
            incarnations.insert(LaneId::SINGLE, Hash::new(b"substitution"));
            let changed = GeometryBindingRequest {
                updated_incarnations: &incarnations,
                ..*request
            };
            assert!(changed.validate_additions_only(&BTreeSet::new()).is_err());
            for lane in [LaneId::SINGLE, LaneId::new(99)] {
                let mut missing = request.updated_incarnations.clone();
                if lane == LaneId::SINGLE {
                    missing.remove(&lane);
                } else {
                    missing.insert(lane, Hash::new(b"extra"));
                }
                let changed = GeometryBindingRequest {
                    updated_incarnations: &missing,
                    ..*request
                };
                assert!(changed.validate_additions_only(&BTreeSet::new()).is_err());
            }
            let mut heights = request.updated_activation_heights.clone();
            heights.insert(LaneId::SINGLE, 9);
            let changed = GeometryBindingRequest {
                updated_activation_heights: &heights,
                ..*request
            };
            assert!(changed.validate_additions_only(&BTreeSet::new()).is_err());
        });
    }
}
