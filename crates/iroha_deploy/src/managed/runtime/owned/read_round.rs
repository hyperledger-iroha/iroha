//! Callback-free original child and renderer checks for read-only maintenance operations.

use super::*;

impl OwnedGateway {
    pub(in crate::managed::runtime) fn read_renderer(&self) -> Option<&GeneratedServiceRuntime> {
        self.launch
            .owner
            .shares_original_for_read()
            .then_some(self.launch.owner.as_ref())
    }

    fn shared_read_launch<'a>(
        live: &'a [Self; 3],
        imports: Option<&mut crate::managed::generated_service_runtime::CatalogLaunchReads<'_>>,
    ) -> Result<&'a GeneratedLaunch> {
        let launch = &live[0].launch;
        for (index, gateway) in live.iter().enumerate() {
            if !Arc::ptr_eq(launch, &gateway.launch)
                || live[..index].iter().any(|other| {
                    other.provider == gateway.provider || Arc::ptr_eq(&other.child, &gateway.child)
                })
            {
                return Err(invalid(
                    "read round requires distinct children of one original launch",
                ));
            }
            gateway.require_running()?;
        }
        launch.validate_with(imports)?;
        Ok(launch)
    }

    /// Bind each actual status reader's independently selected plan to its owned child.
    /// Called at entry, immediately before and after status GETs, and at the original exit.
    pub(in crate::managed::runtime) fn validate_catalog_read(
        prepared: &PreparedLocalnet,
        live: &[Self; 3],
        plans: [&RetainedGatewayCompliancePlan; 3],
        imports: Option<&mut crate::managed::generated_service_runtime::CatalogLaunchReads<'_>>,
    ) -> Result<()> {
        Self::shared_read_launch(live, imports)?;
        for (gateway, plan) in live.iter().zip(plans) {
            gateway.require_original(prepared)?;
            gateway.validate_binding(prepared, plan)?;
        }
        for gateway in live {
            gateway.require_running()?;
        }
        Ok(())
    }

    /// Project the three original enrollments for exactly the floor proved on all peers.
    /// No network call or caller code runs while the opaque revision is being projected.
    pub(in crate::managed::runtime) fn read_enrollments(
        live: &[Self; 3],
        floors: [ManagedTransactionFinality; 3],
    ) -> Result<[&RetainedCustodyEnrollment; 3]> {
        if live[0].read_renderer().is_none() {
            return Ok([
                live[0].selected_enrollment(floors[0])?,
                live[1].selected_enrollment(floors[1])?,
                live[2].selected_enrollment(floors[2])?,
            ]);
        }
        let launch = Self::shared_read_launch(live, None)?;
        let current = launch.revision.observation_floor()?;
        if floors.iter().any(|floor| *floor != current) {
            return Err(invalid(
                "owned runtime transaction differs from selected observation",
            ));
        }
        let project = |index: usize| {
            launch
                .revision
                .selected_enrollment(live[index].provider)?
                .ok_or_else(|| invalid("owned runtime has no retained enrollment"))
        };
        let selected = [project(0)?, project(1)?, project(2)?];
        for gateway in live {
            gateway.require_running()?;
        }
        Ok(selected)
    }
}
