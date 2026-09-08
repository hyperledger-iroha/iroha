// Included only under cfg(test) at Kura module scope. This observer cannot
// manufacture snapshot authority or alter accounting/reconciliation decisions.

#[derive(Debug)]
struct SnapshotFinalizationResourceObservation {
    inventory: std::result::Result<IndexResourceCounts, resource_inventory::Unavailable>,
    measured: std::result::Result<IndexResourceCounts, resource_inventory::Unavailable>,
}

#[derive(Debug, Default)]
enum SnapshotFinalizationResourceProbe {
    #[default]
    Disabled,
    Armed,
    Recorded(SnapshotFinalizationResourceObservation),
}

impl Kura {
    fn arm_snapshot_finalization_resource_observation_for_test(&self) {
        let mut probe = self.snapshot_finalization_resource_probe.lock();
        assert!(matches!(
            *probe,
            SnapshotFinalizationResourceProbe::Disabled
        ));
        *probe = SnapshotFinalizationResourceProbe::Armed;
    }

    fn observe_snapshot_finalization_resources_before_reconcile_for_test(&self) {
        let mut probe = self.snapshot_finalization_resource_probe.lock();
        if !matches!(*probe, SnapshotFinalizationResourceProbe::Armed) {
            return;
        }
        let inventory = (|| {
            let mut values = [ResourceUsage::default(); resource_inventory::FAMILY_COUNT];
            for family in PHYSICAL_RESOURCE_FAMILIES {
                values[family as usize] =
                    self.resource_inventory.component_usage_for_tests(family)?;
            }
            Ok(values)
        })();
        let measured = self
            .physical_resource_scope()
            .and_then(|scope| scope.observe(self.evidence_resource_limits()));
        *probe =
            SnapshotFinalizationResourceProbe::Recorded(SnapshotFinalizationResourceObservation {
                inventory,
                measured,
            });
    }

    fn take_snapshot_finalization_resource_observation_for_test(
        &self,
    ) -> SnapshotFinalizationResourceObservation {
        match std::mem::take(&mut *self.snapshot_finalization_resource_probe.lock()) {
            SnapshotFinalizationResourceProbe::Recorded(observation) => observation,
            SnapshotFinalizationResourceProbe::Disabled
            | SnapshotFinalizationResourceProbe::Armed => {
                panic!("the authenticated pre-reconcile boundary was not observed")
            }
        }
    }
}
