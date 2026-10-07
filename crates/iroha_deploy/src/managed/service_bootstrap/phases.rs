//! Sequential original-bootstrap phases, each retaining its sole native child owner.

use super::{super::service_policies::GeneratedProviderPolicies, *};

/// Only completed original evidence advances to the next child; all other progress returns intact.
enum Phase<T> {
    Complete(T),
    Incomplete(Incomplete),
}

/// An unfinished child cannot carry the complete aggregate history.
enum Incomplete {
    Pending {
        step: ServiceBootstrapStep,
        status: OperationStatus,
    },
    Funding {
        provider_id: ProviderId,
        progress: ProviderFundingProgress,
    },
}

impl Incomplete {
    /// Form the outer report only after the native child owner has returned.
    #[inline(never)]
    fn into_progress(self) -> Result<ServiceBootstrapProgress> {
        Ok(match self {
            Self::Pending { step, status } => ServiceBootstrapProgress::Pending { step, status },
            Self::Funding {
                provider_id,
                progress,
            } => ServiceBootstrapProgress::Funding {
                provider_id,
                progress,
            },
        })
    }
}

fn absent<T>(step: ServiceBootstrapStep) -> Result<Phase<T>> {
    Ok(Phase::Incomplete(Incomplete::Pending {
        step,
        status: OperationStatus::Absent,
    }))
}

// Open-existing paths authenticate without creating directories or lock files. Only the
// live startup branch can create a genuinely missing purpose after its prerequisites.
macro_rules! open_child {
    ($run:expr, $owner:ty, $step:expr $(, $provider:expr)?) => {{
        match <$owner>::open_existing(&$run.authority.prepared $(, $provider)?)? {
            Some(owner) => owner,
            None if $run.may_submit()? => <$owner>::open(&$run.authority.prepared $(, $provider)?)?,
            None => return absent($step),
        }
    }};
}

macro_rules! completed {
    ($phase:expr) => {
        match $phase? {
            Phase::Complete(value) => value,
            Phase::Incomplete(progress) => return progress.into_progress(),
        }
    };
}

struct Run<'a> {
    authority: &'a ServiceAuthority,
    original: &'a Original,
    deadline: Instant,
    mode: Mode,
    authorization: Option<&'a GeneratedBootstrapAuthorization>,
}

/// Retain the original dependency order while each child owner occupies its own frame.
pub(super) fn run(
    authority: &ServiceAuthority,
    original: &Original,
    deadline: Instant,
    mode: Mode,
    authorization: Option<&GeneratedBootstrapAuthorization>,
) -> Result<ServiceBootstrapProgress> {
    let run = Run {
        authority,
        original,
        deadline,
        mode,
        authorization,
    };
    let reserve_policy = completed!(run.reserve());
    let mut histories = Vec::with_capacity(3);
    let mut prior_height = reserve_policy.height;
    for (slot, selected) in original.policies.providers.iter().enumerate() {
        let provider_id = selected.provider_id;
        let (custody_policy, custody_enrollment) = completed!(run.custody(selected, prior_height));
        let reserve_account = completed!(run.account(slot, provider_id, custody_enrollment.height));
        let (funding_result, funding_height) =
            completed!(run.funding(provider_id, reserve_account.height));
        let ingest_finality = completed!(run.ingest(selected, funding_height));
        let gateway_finality = completed!(run.gateway(selected, ingest_finality.height));
        prior_height = gateway_finality.height;
        histories.push(HistoricalProviderBootstrap {
            provider_id,
            custody_policy,
            custody_enrollment,
            reserve_account,
            funding: funding_result,
            provider_ingest: ingest_finality,
            gateway: gateway_finality,
        });
    }
    let history = completed!(run.reputation(reserve_policy, histories, prior_height));
    Ok(ServiceBootstrapProgress::Complete(history))
}

impl Run<'_> {
    fn may_submit(&self) -> Result<bool> {
        if self.mode != Mode::Advance {
            return Ok(false);
        }
        self.authorization
            .ok_or_else(|| invalid("bootstrap advance requires its live worker authorization"))?
            .validate(self.authority, self.deadline)?;
        Ok(true)
    }

    #[inline(never)]
    fn reserve(&self) -> Result<Phase<ManagedTransactionFinality>> {
        let policies = &self.original.policies;
        let fees = &self.original.fees;
        let deadline = self.deadline;
        let mode = self.mode;
        let authorization = self.authorization;
        // This single phase can reuse the parent's immutable original profile for a fresh
        // name-only absence observation. Enclosing codec owners retain the exact original
        // independent child capture/admission path; present custody also keeps that owner.
        let purpose_absent = if norito::core::decode_limits_active() {
            false
        } else {
            super::super::service_authority::ServiceChildInventory::begin(self.authority)?
                .initial_reserve_policy_absent()?
        };
        let mut reserve = if purpose_absent {
            if !self.may_submit()? {
                return absent(ServiceBootstrapStep::ReservePolicy);
            }
            // The fresh observation grants no signing/current-state authority. The ordinary
            // new child still authenticates its own profile and lock after live authorization.
            ManagedInitialReservePolicy::open(&self.authority.prepared)?
        } else {
            open_child!(
                self,
                ManagedInitialReservePolicy,
                ServiceBootstrapStep::ReservePolicy
            )
        };
        let mut result = incomplete(if mode == Mode::Local {
            reserve.recover_local_selected_if_present(&policies.network.reserve, fees, deadline)
        } else {
            reserve.recover_selected_if_present(&policies.network.reserve, fees, deadline)
        })?;
        if result
            .as_ref()
            .is_none_or(|value| value.finalized.is_none())
            && self.may_submit()?
        {
            result = Some(
                reserve.advance_selected(
                    &policies.network.reserve,
                    &authorization
                        .ok_or_else(|| {
                            invalid("bootstrap advance requires its live worker authorization")
                        })?
                        .child(Purpose::ReservePolicy)?,
                    deadline,
                )?,
            );
        }
        let Some(result) = result else {
            return absent(ServiceBootstrapStep::ReservePolicy);
        };
        let Some(reserve_policy) = result.finalized else {
            return Ok(Phase::Incomplete(Incomplete::Pending {
                step: ServiceBootstrapStep::ReservePolicy,
                status: result.transaction_status,
            }));
        };
        drop(reserve);
        Ok(Phase::Complete(reserve_policy))
    }

    #[inline(never)]
    fn custody(
        &self,
        selected: &GeneratedProviderPolicies,
        prior_height: u64,
    ) -> Result<Phase<(ManagedTransactionFinality, ManagedTransactionFinality)>> {
        let fees = &self.original.fees;
        let deadline = self.deadline;
        let mode = self.mode;
        let authorization = self.authorization;
        let provider_id = selected.provider_id;
        let mut custody = open_child!(
            self,
            ManagedStreamTokenCustody,
            ServiceBootstrapStep::CustodyPolicy { provider_id },
            provider_id
        );
        let mut result = incomplete(if mode == Mode::Local {
            custody.recover_configure_local_selected_if_present(&selected.custody, fees, deadline)
        } else {
            custody.recover_configure_selected_if_present(&selected.custody, fees, deadline)
        })?;
        if result
            .as_ref()
            .is_none_or(|value| value.finalized.is_none())
            && self.may_submit()?
        {
            result = Some(
                custody.advance_configure_selected(
                    &selected.custody,
                    &authorization
                        .ok_or_else(|| {
                            invalid("bootstrap advance requires its live worker authorization")
                        })?
                        .child(Purpose::CustodyConfigure(provider_id))?,
                    deadline,
                )?,
            );
        }
        let Some(result) = result else {
            return absent(ServiceBootstrapStep::CustodyPolicy { provider_id });
        };
        let Some(custody_policy) = result.finalized else {
            return Ok(Phase::Incomplete(Incomplete::Pending {
                step: ServiceBootstrapStep::CustodyPolicy { provider_id },
                status: result.transaction_status,
            }));
        };

        require_after(&custody_policy, prior_height)?;
        let mut result = incomplete(if mode == Mode::Local {
            custody.recover_enroll_local_selected_if_present(&selected.custody, fees, deadline)
        } else {
            custody.recover_enroll_selected_if_present(&selected.custody, fees, deadline)
        })?;
        if result
            .as_ref()
            .is_none_or(|value| value.finalized.is_none())
            && self.may_submit()?
        {
            result = Some(
                custody.advance_enroll_selected(
                    &selected.custody,
                    &authorization
                        .ok_or_else(|| {
                            invalid("bootstrap advance requires its live worker authorization")
                        })?
                        .child(Purpose::CustodyEnroll(provider_id))?,
                    deadline,
                )?,
            );
        }
        let Some(result) = result else {
            return absent(ServiceBootstrapStep::CustodyEnrollment { provider_id });
        };
        let Some(custody_enrollment) = result.finalized else {
            return Ok(Phase::Incomplete(Incomplete::Pending {
                step: ServiceBootstrapStep::CustodyEnrollment { provider_id },
                status: result.transaction_status,
            }));
        };
        require_after(&custody_enrollment, custody_policy.height)?;
        drop(custody);
        Ok(Phase::Complete((custody_policy, custody_enrollment)))
    }

    #[inline(never)]
    fn account(
        &self,
        slot: usize,
        provider_id: ProviderId,
        custody_height: u64,
    ) -> Result<Phase<ManagedTransactionFinality>> {
        let policies = &self.original.policies;
        let fees = &self.original.fees;
        let deadline = self.deadline;
        let mode = self.mode;
        let authorization = self.authorization;
        let original = self.original;
        let mut account = open_child!(
            self,
            ManagedReserveAccountRegistration,
            ServiceBootstrapStep::ReserveAccount { provider_id },
            provider_id
        );
        let mut result = incomplete(if mode == Mode::Local {
            account.recover_local_selected_if_present(
                &policies.network.reserve,
                &original.underwriting[slot],
                fees,
                deadline,
            )
        } else {
            account.recover_selected_if_present(
                &policies.network.reserve,
                &original.underwriting[slot],
                fees,
                deadline,
            )
        })?;
        if result
            .as_ref()
            .is_none_or(|value| value.finalized.is_none())
            && self.may_submit()?
        {
            result = Some(
                account.advance_selected(
                    &policies.network.reserve,
                    &original.underwriting[slot],
                    &authorization
                        .ok_or_else(|| {
                            invalid("bootstrap advance requires its live worker authorization")
                        })?
                        .child(Purpose::ReserveAccount(provider_id))?,
                    deadline,
                )?,
            );
        }
        let Some(result) = result else {
            return absent(ServiceBootstrapStep::ReserveAccount { provider_id });
        };
        let Some(reserve_account) = result.finalized else {
            return Ok(Phase::Incomplete(Incomplete::Pending {
                step: ServiceBootstrapStep::ReserveAccount { provider_id },
                status: result.transaction_status,
            }));
        };
        require_after(&reserve_account, custody_height)?;
        drop(account);
        Ok(Phase::Complete(reserve_account))
    }

    #[inline(never)]
    fn funding(
        &self,
        provider_id: ProviderId,
        account_height: u64,
    ) -> Result<Phase<(CompletedFunding, u64)>> {
        let policies = &self.original.policies;
        let fees = &self.original.fees;
        let deadline = self.deadline;
        let mode = self.mode;
        let authorization = self.authorization;
        let mut funding = open_child!(
            self,
            ProviderFundingBootstrap,
            ServiceBootstrapStep::ProviderFunding { provider_id },
            provider_id
        );
        let mut result = incomplete(if mode == Mode::Local {
            funding.recover_local_selected_if_present(&policies.network.reserve, fees, deadline)
        } else {
            funding.recover_selected_if_present(&policies.network.reserve, fees, deadline)
        })?;
        if result
            .as_ref()
            .is_none_or(|value| !matches!(value, ProviderFundingProgress::Complete { .. }))
            && self.may_submit()?
        {
            result = Some(
                funding.advance_selected(
                    &policies.network.reserve,
                    &authorization
                        .ok_or_else(|| {
                            invalid("bootstrap advance requires its live worker authorization")
                        })?
                        .funding(provider_id)?,
                    deadline,
                )?,
            );
        }
        let Some(funding_result) = result else {
            return absent(ServiceBootstrapStep::ProviderFunding { provider_id });
        };
        let ProviderFundingProgress::Complete {
            credit, capacity, ..
        } = &funding_result
        else {
            return Ok(Phase::Incomplete(Incomplete::Funding {
                provider_id,
                progress: funding_result,
            }));
        };
        require_after(credit, account_height)?;
        require_after(capacity, credit.height)?;
        let funding_height = capacity.height;
        drop(funding);
        let completed = CompletedFunding::from_progress(funding_result)?;
        Ok(Phase::Complete((completed, funding_height)))
    }

    #[inline(never)]
    fn ingest(
        &self,
        selected: &GeneratedProviderPolicies,
        funding_height: u64,
    ) -> Result<Phase<ManagedTransactionFinality>> {
        let fees = &self.original.fees;
        let deadline = self.deadline;
        let mode = self.mode;
        let authorization = self.authorization;
        let provider_id = selected.provider_id;
        let mut ingest = open_child!(
            self,
            ManagedInitialProviderIngestAuthority,
            ServiceBootstrapStep::ProviderIngest { provider_id },
            provider_id
        );
        let mut result = incomplete(if mode == Mode::Local {
            ingest.recover_local_selected_if_present(&selected.provider_ingest, fees, deadline)
        } else {
            ingest.recover_selected_if_present(&selected.provider_ingest, fees, deadline)
        })?;
        if result
            .as_ref()
            .is_none_or(|value| value.finalized.is_none())
            && self.may_submit()?
        {
            result = Some(
                ingest.advance_selected(
                    &selected.provider_ingest,
                    &authorization
                        .ok_or_else(|| {
                            invalid("bootstrap advance requires its live worker authorization")
                        })?
                        .child(Purpose::ProviderIngest(provider_id))?,
                    deadline,
                )?,
            );
        }
        let Some(result) = result else {
            return absent(ServiceBootstrapStep::ProviderIngest { provider_id });
        };
        let Some(ingest_finality) = result.finalized else {
            return Ok(Phase::Incomplete(Incomplete::Pending {
                step: ServiceBootstrapStep::ProviderIngest { provider_id },
                status: result.transaction_status,
            }));
        };
        require_after(&ingest_finality, funding_height)?;
        drop(ingest);
        Ok(Phase::Complete(ingest_finality))
    }

    #[inline(never)]
    fn gateway(
        &self,
        selected: &GeneratedProviderPolicies,
        ingest_height: u64,
    ) -> Result<Phase<ManagedTransactionFinality>> {
        let fees = &self.original.fees;
        let deadline = self.deadline;
        let mode = self.mode;
        let authorization = self.authorization;
        let provider_id = selected.provider_id;
        let mut gateway = open_child!(
            self,
            ManagedInitialGatewaySetup,
            ServiceBootstrapStep::Gateway { provider_id },
            provider_id
        );
        let mut result = incomplete(if mode == Mode::Local {
            gateway.recover_local_selected_if_present(&selected.gateway, fees, deadline)
        } else {
            gateway.recover_selected_if_present(&selected.gateway, fees, deadline)
        })?;
        if result
            .as_ref()
            .is_none_or(|value| value.finalized.is_none())
            && self.may_submit()?
        {
            result = Some(
                gateway.advance_selected(
                    &selected.gateway,
                    &authorization
                        .ok_or_else(|| {
                            invalid("bootstrap advance requires its live worker authorization")
                        })?
                        .child(Purpose::Gateway(provider_id))?,
                    deadline,
                )?,
            );
        }
        let Some(result) = result else {
            return absent(ServiceBootstrapStep::Gateway { provider_id });
        };
        let Some(gateway_finality) = result.finalized else {
            return Ok(Phase::Incomplete(Incomplete::Pending {
                step: ServiceBootstrapStep::Gateway { provider_id },
                status: result.transaction_status,
            }));
        };
        require_after(&gateway_finality, ingest_height)?;
        drop(gateway);
        Ok(Phase::Complete(gateway_finality))
    }

    #[inline(never)]
    fn reputation(
        &self,
        reserve_policy: ManagedTransactionFinality,
        histories: Vec<HistoricalProviderBootstrap>,
        prior_height: u64,
    ) -> Result<Phase<HistoricalServiceBootstrap>> {
        let policies = &self.original.policies;
        let fees = &self.original.fees;
        let deadline = self.deadline;
        let mode = self.mode;
        let authorization = self.authorization;
        let mut reputation = open_child!(
            self,
            ManagedInitialReputationPolicy,
            ServiceBootstrapStep::Reputation
        );
        let labels = policies.gateway_labels();
        let mut result = incomplete(if mode == Mode::Local {
            reputation.recover_local_selected_if_present(
                &labels,
                &policies.network.reputation,
                fees,
                deadline,
            )
        } else {
            reputation.recover_selected_if_present(
                &labels,
                &policies.network.reputation,
                fees,
                deadline,
            )
        })?;
        if result
            .as_ref()
            .is_none_or(|value| value.finalized.is_none())
            && self.may_submit()?
        {
            result = Some(
                reputation.advance_selected(
                    &labels,
                    &policies.network.reputation,
                    &authorization
                        .ok_or_else(|| {
                            invalid("bootstrap advance requires its live worker authorization")
                        })?
                        .child(Purpose::Reputation)?,
                    deadline,
                )?,
            );
        }
        let Some(result) = result else {
            return absent(ServiceBootstrapStep::Reputation);
        };
        let Some(reputation_finality) = result.finalized else {
            return Ok(Phase::Incomplete(Incomplete::Pending {
                step: ServiceBootstrapStep::Reputation,
                status: result.transaction_status,
            }));
        };
        require_after(&reputation_finality, prior_height)?;
        let providers = histories
            .try_into()
            .map_err(|_| invalid("bootstrap provider history count differs"))?;
        let history = HistoricalServiceBootstrap {
            reserve_policy,
            providers,
            reputation: reputation_finality,
        };
        history.ordered_carriers()?;
        Ok(Phase::Complete(history))
    }
}

#[cfg(test)]
mod tests {
    //! Unfinished child storage must exclude the impossible complete aggregate payload.

    use super::*;
    use std::mem::size_of;

    #[test]
    fn unfinished_phase_does_not_reserve_complete_aggregate_storage() {
        assert!(size_of::<Incomplete>() < size_of::<ServiceBootstrapProgress>());
        assert!(size_of::<Phase<()>>() < size_of::<ServiceBootstrapProgress>());
    }
}

#[cfg(test)]
#[path = "absence_tests.rs"]
mod absence_tests;
