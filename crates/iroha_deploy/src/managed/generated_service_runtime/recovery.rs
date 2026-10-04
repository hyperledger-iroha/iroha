//! Reconstruct each original provider history without guessing a global renewal order.
use super::*;

impl GeneratedServiceRuntime {
    /// Only independently current native heads select the three sequences. Every prior component
    /// must already exist; only a currently selected component may finish interrupted publication.
    /// This path never enrolls, signs, submits or replaces original wallet authorization.
    pub(in crate::managed) fn prepare_current_stream_tokens(
        &self,
        deadline: Instant,
    ) -> Result<GeneratedServiceRuntimeRevision> {
        require_deadline(deadline)?;
        let selection = RuntimeSelection::read(&self.authority)?;
        selection.validate_interval(None)?;
        let mut parent = ManagedServiceBootstrap::open(&self.authority.prepared)?;
        let ServiceBootstrapProgress::Complete(history) = parent.recover(deadline)? else {
            return Err(invalid(
                "generated bootstrap has incomplete original execution",
            ));
        };
        let terminal = history.reputation();
        let carriers = history.ordered_carriers()?;
        // Custody and component validation may reopen parent selection. Release its sole lock.
        drop(parent);
        let mut components = Vec::with_capacity(3);
        for (index, plan) in selection.plans.iter().enumerate() {
            require_deadline(deadline)?;
            let provider = plan.provider_id();
            let mut custody = ManagedStreamTokenCustody::open(&self.authority.prepared, provider)?;
            let policy = &selection.policies.provider(provider)?.custody;
            let selected = custody.retained_current_enrollment(
                policy,
                selection.initial(index)?,
                terminal.height,
                *terminal.block_hash.as_ref(),
                deadline,
            )?;
            let component = self.reconstruct_provider(
                &selection,
                index,
                history.provider(provider)?.custody_enrollment(),
                terminal,
                selected,
                &mut custody,
                deadline,
            )?;
            components.push(component);
        }
        let prepared: [PreparedProviderComponent; 3] = components
            .try_into()
            .map_err(|_| invalid("current generated provider count differs"))?;
        let required = RequiredTransactions::from_originals(
            carriers
                .into_iter()
                .chain(prepared.iter().map(|value| value.finalized())),
        )?;
        let digests = std::array::from_fn(|index| prepared[index].digest());
        let retentions = self.component_retentions(&selection, digests, &required)?;
        let components: [Arc<ProviderComponent>; 3] = prepared
            .into_iter()
            .zip(retentions)
            .map(|(value, retention)| value.retain(&self.authority.directory, retention))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| invalid("current generated provider count differs"))?;
        // All component reconstruction is bounded, but may be slow. Recheck all exact heads
        // against the highest retained carrier before deriving any launch configuration.
        self.verify_current_components(&selection, &components, &required, deadline)?;
        self.publish(&selection, Some(components), Some(required), deadline)
    }

    fn reconstruct_provider(
        &self,
        selection: &RuntimeSelection,
        index: usize,
        initial_finality: ManagedTransactionFinality,
        terminal: ManagedTransactionFinality,
        selected: RetainedCustodyEnrollment,
        custody: &mut ManagedStreamTokenCustody,
        deadline: Instant,
    ) -> Result<PreparedProviderComponent> {
        let sequence = selected.statement().sequence;
        if !(1..=64).contains(&sequence) {
            return Err(invalid(
                "current generated enrollment sequence exceeds bound",
            ));
        }
        let identity = selection.identity(&self.authority, index)?;
        let policy = &selection.policies.provider(identity.provider)?.custody;
        let initial = selection.initial(index)?;
        let mut previous: Option<Arc<ProviderComponent>> = None;
        for ancestor_sequence in 1..sequence {
            require_deadline(deadline)?;
            let ancestor = if ancestor_sequence == 1 {
                custody.retained_initial_enrollment(policy, initial, deadline)?
            } else {
                custody.retained_renewed_enrollment(ancestor_sequence, policy, deadline)?
            };
            check_initial(&ancestor, initial, initial_finality, terminal)?;
            previous = Some(ProviderComponent::retain(
                &self.authority.directory,
                identity,
                ancestor,
                previous.as_deref(),
                Retention::ExistingMaterial,
            )?);
        }
        check_initial(&selected, initial, initial_finality, terminal)?;
        ProviderComponent::prepare(identity, selected, previous.as_deref())
    }
}

fn check_initial(
    enrollment: &RetainedCustodyEnrollment,
    interval: ManagedCustodyEnrollmentInterval,
    initial: ManagedTransactionFinality,
    terminal: ManagedTransactionFinality,
) -> Result<()> {
    if enrollment.statement().sequence == 1
        && (*enrollment.finalized() != initial
            || enrollment.statement().issued_at_unix_ms != interval.issued_at_unix_ms
            || enrollment.statement().expires_at_unix_ms != interval.expires_at_unix_ms
            || terminal.height < initial.height)
    {
        return Err(invalid(
            "initial component differs from original parent execution",
        ));
    }
    Ok(())
}
