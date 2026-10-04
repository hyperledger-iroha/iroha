//! Closed read-only dependency census. Presence can refuse signing, never establish native state.

use super::*;
use crate::managed::service_authority::ProviderPurpose;

impl ManagedServiceBootstrap {
    fn validate_dependency_inventory(&self, progress: &ServiceBootstrapProgress) -> Result<()> {
        let original = read_original(
            &self.authority.directory.open_child("initial")?,
            &self.authority,
        )?
        .ok_or_else(|| invalid("original bootstrap intent is absent"))?;
        self.validate_child_inventory(&original.policies, Some(progress))
    }

    // None means no parent exists: all child purposes must be absent or exactly empty.
    // Presence only refuses publication; this never establishes native completion.
    fn validate_child_inventory(
        &self,
        policies: &GeneratedServicePolicies,
        progress: Option<&ServiceBootstrapProgress>,
    ) -> Result<()> {
        let mut stages = Vec::with_capacity(20);
        stages.push(ServiceBootstrapStep::ReservePolicy);
        for selected in &policies.providers {
            let provider_id = selected.provider_id;
            stages.extend([
                ServiceBootstrapStep::CustodyPolicy { provider_id },
                ServiceBootstrapStep::CustodyEnrollment { provider_id },
                ServiceBootstrapStep::ReserveAccount { provider_id },
                ServiceBootstrapStep::ProviderFunding { provider_id },
                ServiceBootstrapStep::ProviderIngest { provider_id },
                ServiceBootstrapStep::Gateway { provider_id },
            ]);
        }
        stages.push(ServiceBootstrapStep::Reputation);
        let pending = match progress {
            Some(ServiceBootstrapProgress::Pending { step, .. }) => Some(*step),
            Some(ServiceBootstrapProgress::Funding { provider_id, .. }) => {
                Some(ServiceBootstrapStep::ProviderFunding {
                    provider_id: *provider_id,
                })
            }
            Some(ServiceBootstrapProgress::Complete(_)) | None => None,
        };
        let first_incomplete = pending
            .map(|step| {
                stages
                    .iter()
                    .position(|selected| selected == &step)
                    .ok_or_else(|| invalid("bootstrap progress selected another dependency"))
            })
            .transpose()?;
        for (index, step) in stages.iter().enumerate() {
            let later = progress.is_none() || first_incomplete.is_some_and(|first| index > first);
            match *step {
                ServiceBootstrapStep::ReservePolicy => {
                    self.census_network(NetworkPurpose::InitialReservePolicy, "set", later)?
                }
                ServiceBootstrapStep::CustodyPolicy { provider_id } => self.census_provider(
                    provider_id,
                    ProviderPurpose::Custody,
                    "configure",
                    later,
                    progress.is_none() || first_incomplete.is_some(),
                )?,
                ServiceBootstrapStep::CustodyEnrollment { provider_id } => self.census_provider(
                    provider_id,
                    ProviderPurpose::Custody,
                    "enroll",
                    later,
                    progress.is_none() || first_incomplete.is_some(),
                )?,
                ServiceBootstrapStep::ReserveAccount { provider_id } => self.census_provider(
                    provider_id,
                    ProviderPurpose::ReserveAccountRegistration,
                    "register",
                    later,
                    false,
                )?,
                ServiceBootstrapStep::ProviderFunding { provider_id } => {
                    // Nested incomplete stage ordering is checked by the composite's exact native
                    // recovery. Only wholly later funding must have all five purpose roots empty.
                    for (purpose, operation) in [
                        (ProviderPurpose::ProviderFundingBootstrap, "funding"),
                        (ProviderPurpose::ReserveTopUpRequest, "request"),
                        (ProviderPurpose::ReserveTopUpApproval, "approval"),
                        (ProviderPurpose::InitialProviderCredit, "install"),
                        (ProviderPurpose::ProviderCapacityDeclaration, "declare"),
                    ] {
                        self.census_provider(provider_id, purpose, operation, later, false)?;
                    }
                }
                ServiceBootstrapStep::ProviderIngest { provider_id } => self.census_provider(
                    provider_id,
                    ProviderPurpose::InitialProviderIngestAuthority,
                    "setup",
                    later,
                    false,
                )?,
                ServiceBootstrapStep::Gateway { provider_id } => self.census_provider(
                    provider_id,
                    ProviderPurpose::InitialGatewaySetup,
                    "setup",
                    later,
                    false,
                )?,
                ServiceBootstrapStep::Reputation => {
                    self.census_network(NetworkPurpose::InitialReputationPolicy, "setup", later)?
                }
            }
        }
        self.authority.validate_profile()?;
        Ok(())
    }
    fn census_network(&self, purpose: NetworkPurpose, operation: &str, later: bool) -> Result<()> {
        if let Some(owner) =
            ServiceAuthority::open_network_existing(&self.authority.prepared, purpose)?
        {
            census(&owner.directory, operation, later, false, false)?;
        }
        Ok(())
    }
    fn census_provider(
        &self,
        provider: ProviderId,
        purpose: ProviderPurpose,
        operation: &str,
        later: bool,
        incomplete: bool,
    ) -> Result<()> {
        let custody = matches!(purpose, ProviderPurpose::Custody);
        if let Some(owner) =
            ServiceAuthority::open_provider_existing(&self.authority.prepared, provider, purpose)?
        {
            census(&owner.directory, operation, later, custody, incomplete)?;
        }
        Ok(())
    }
}

fn census(
    directory: &PrivateDirectory,
    operation: &str,
    later: bool,
    custody: bool,
    incomplete: bool,
) -> Result<()> {
    let names = directory.entries(66)?;
    for name in &names {
        if name == "operation.lock" || name == operation {
            continue;
        }
        if custody && (name == "configure" || name == "enroll") {
            continue;
        }
        if custody && !incomplete {
            // Complete initial history may have exact bounded renewal journals. Their native
            // authority is still selected and checked only by the custody renewal owner.
            let valid = (2..=64).any(|sequence| {
                crate::managed::stream_token_custody::renewal::directory_name(sequence)
                    .is_ok_and(|expected| name == std::ffi::OsStr::new(&expected))
            });
            if valid {
                continue;
            }
        }
        return Err(invalid(
            "bootstrap purpose contains unknown or out-of-order material",
        ));
    }
    if later {
        match directory.open_child(operation) {
            Ok(child) => require_empty(&child).map_err(|_| {
                invalid("later bootstrap material exists behind an incomplete prerequisite")
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    if directory.entries(66)? != names {
        return Err(invalid(
            "bootstrap dependency inventory changed during inspection",
        ));
    }
    Ok(())
}
