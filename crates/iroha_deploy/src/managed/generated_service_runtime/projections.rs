//! Fresh original gateway projections and existing Bootstrap admission from the retained renderer.

use super::*;

impl GeneratedServiceRuntime {
    /// Reopen the already-created Bootstrap purpose from this exact renderer generation.
    /// This grants no signing authority or current-state verdict; each child keeps its own
    /// fresh native lock and profile admission. Active decode limits and owned profiles
    /// retain the standalone full-capture path in the existing-only constructor.
    pub(in crate::managed) fn open_original_bootstrap(
        &self,
        prepared: &PreparedLocalnet,
    ) -> Result<ManagedServiceBootstrap> {
        if prepared != &self.authority.prepared {
            return Err(invalid(
                "generated runtime bootstrap belongs to another prepared generation",
            ));
        }
        self.authority.validate_profile()?;
        let result = ManagedServiceBootstrap::open_existing_from_original(&self.authority);
        // Keep a successful child's native owner live while closing the renderer on every
        // ordinary result, including active/owned fallback and initial purpose absence.
        self.authority.validate_profile()?;
        result?.ok_or_else(|| invalid("original service bootstrap purpose is absent"))
    }

    pub(in crate::managed) fn original_provider_plan(
        &self,
        prepared: &PreparedLocalnet,
        provider: ProviderId,
    ) -> Result<Option<RetainedProviderServicePlan>> {
        // A retained renderer projects only its own exact prepared generation.
        if prepared != &self.authority.prepared {
            return Err(invalid(
                "generated runtime projection belongs to another prepared generation",
            ));
        }
        let Some(intent) = self.authority.original_intent_if_shared()? else {
            return prepared.provider_service_plan(provider);
        };
        let result = intent
            .provider_plan(provider)
            .map(Some)
            .map_err(|_| invalid("retained generated provider plan differs"));
        // A closing original-image/operation-lock failure supersedes every projection result.
        intent.finish()?;
        result
    }

    pub(in crate::managed) fn original_provider_plans(
        &self,
        prepared: &PreparedLocalnet,
    ) -> Result<Option<[RetainedProviderServicePlan; 3]>> {
        if prepared != &self.authority.prepared {
            return Err(invalid(
                "generated runtime projection belongs to another prepared generation",
            ));
        }
        let Some(intent) = self.authority.original_intent_if_shared()? else {
            return prepared.provider_service_plans();
        };
        let result = intent.provider_plans().map(|plans| Some(plans.clone()));
        intent.finish()?;
        result
    }

    pub(in crate::managed) fn original_gateway_compliance_plan(
        &self,
        prepared: &PreparedLocalnet,
        provider: ProviderId,
    ) -> Result<Option<RetainedGatewayCompliancePlan>> {
        if prepared != &self.authority.prepared {
            return Err(invalid(
                "generated runtime projection belongs to another prepared generation",
            ));
        }
        let Some(intent) = self.authority.original_intent_if_shared()? else {
            return prepared.gateway_compliance_plan(provider);
        };
        // Compliance still runs its ordinary bounded canonical template decode.
        let result = intent
            .gateway_compliance_plan(provider)
            .map(Some)
            .map_err(|_| invalid("retained generated compliance plan differs"));
        intent.finish()?;
        result
    }
}

#[cfg(test)]
mod tests;
