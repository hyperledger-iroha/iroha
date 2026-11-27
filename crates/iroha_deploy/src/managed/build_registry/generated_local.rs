//! Original generated-local intent and fresh native discovery; no readiness or activation owner.

use super::*;
use crate::{
    localnet::{LocalnetServiceProfile, service_authorities::StreamTokenAuthorityRole},
    managed::{
        PreparedLocalnet,
        native_operation::{
            ManagedTransactionFinality, invalid, now_ms, read_selected_peers, require_deadline,
        },
        service_authority::{NetworkPurpose, ServiceAuthority},
        service_policies::GeneratedProviderPolicies,
        stream_token_custody::{ManagedStreamTokenCustody, RetainedCustodyEnrollment},
    },
};
use iroha_data_model::sorafs::{
    capacity::ProviderId,
    provider_admission::discovery::account_read::VerifiedAccountReadProviderV1,
};
use iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock;
use iroha_storage_client::musubi_archive_fetch::GeneratedLocalProviderTransportV1;

pub(super) fn prepare(
    prepared: &PreparedLocalnet,
    deadline: Instant,
) -> Result<Option<(Config, PreparedMusubiArchiveFetchConfigV1)>> {
    require_deadline(deadline)?;
    if prepared.context.dataspace_id != 0
        || prepared.service_profile != LocalnetServiceProfile::StreamTokenAuthorities
    {
        return Ok(None);
    }
    let authority = ServiceAuthority::open_network(prepared, NetworkPurpose::BuildRegistry)?;
    let plans = authority.provider_plans()?;
    let config = authority.config.clone();
    let mut originals = Vec::with_capacity(3);
    for (slot, plan) in plans.iter().enumerate() {
        let inventory = authority.provider_inventory(plan.provider_id())?;
        if plan.network_id() != config.network_id
            || usize::from(plan.slot()) != slot
            || plan.peer_index() != slot
            || plan.reserve_terms().provider_account
                != inventory
                    .authority(StreamTokenAuthorityRole::IssuerOperator)?
                    .account
        {
            return Err(invalid("generated build registry original scope differs"));
        }
        originals.push(
            GeneratedLocalProviderTransportV1::select(
                plan.network_id(),
                config.chain.as_str(),
                plan.provider_id(),
                &plan.reserve_terms().provider_account,
                plan.admission_material(),
            )
            .map_err(|_| invalid("invalid original generated provider transport"))?,
        );
    }
    let originals = originals
        .try_into()
        .map_err(|_| invalid("generated transport count differs"))?;
    // This closure is retained by the prepared configuration and every client built from it.
    // The original operation.lock therefore cannot be released between discovery and use.
    let owner = Mutex::new(authority);
    let transport = PreparedMusubiArchiveFetchConfigV1::from_generated_local_account_registry(
        config.clone(),
        Arc::new(move |provider| {
            check_deadline(deadline)?;
            let mut owner = owner.try_lock().map_err(|error| match error {
                std::sync::TryLockError::WouldBlock => MusubiArchiveDiscoveryErrorV1::Unavailable,
                std::sync::TryLockError::Poisoned(_) => MusubiArchiveDiscoveryErrorV1::Rejected,
            })?;
            discover(&mut owner, provider, deadline).map(|(current, _)| current)
        }),
        originals,
        DISCOVERY_FRESHNESS,
    )
    .map_err(|_| invalid("cannot prepare generated build registry"))?;
    require_deadline(deadline)?;
    Ok(Some((config, transport)))
}

/// Timing from actual current native discovery, not an activation or signing capability.
/// Only the existing verifier below constructs this observation.
#[derive(Clone, Copy)]
pub(in crate::managed) struct GeneratedServiceObservation {
    original_slot: usize,
    certified_cut: (u64, iroha_crypto::Hash),
    valid_until_unix_ms: u64,
    advert_interval: (u64, u64),
}
impl GeneratedServiceObservation {
    pub(in crate::managed) fn valid_until_unix_ms(&self) -> u64 {
        self.valid_until_unix_ms
    }
    pub(in crate::managed) fn advert_interval(&self) -> (u64, u64) {
        self.advert_interval
    }
}

/// Read-only startup postcondition, reusing exactly the cold registry's native verifier.
/// The minimum is only a rejection bound; it cannot create authenticated state.
fn observe_generated_service(
    prepared: &PreparedLocalnet,
    provider: ProviderId,
    enrollment: &RetainedCustodyEnrollment,
    required: ManagedTransactionFinality,
    deadline: Instant,
) -> Result<GeneratedServiceObservation> {
    let request = ServiceRead {
        provider,
        enrollment,
        required,
    };
    request.validate(prepared, deadline)?;
    let parent = crate::managed::service_bootstrap::ManagedServiceBootstrap::open(prepared)?;
    let policies = parent.selected_policies()?;
    drop(parent);
    let policies = policies.provider(provider)?;
    request.require_policy(policies)?;
    let mut owner = ServiceAuthority::open_network(prepared, NetworkPurpose::ServiceObservation)?;
    let (current, block) = discover(&mut owner, provider, deadline)
        .map_err(|_| invalid("fresh native generated provider discovery is unavailable"))?;
    request
        .verify_custody(&owner, policies, &block, deadline)?
        .ok_or_else(|| invalid("current generated provider custody is unavailable"))?;
    request.bind(&owner, policies, current, &block, deadline)
}

#[derive(Clone, Copy)]
pub(in crate::managed) struct ServiceRead<'a> {
    pub(in crate::managed) provider: ProviderId,
    pub(in crate::managed) enrollment: &'a RetainedCustodyEnrollment,
    pub(in crate::managed) required: ManagedTransactionFinality,
}

impl ServiceRead<'_> {
    fn validate(&self, prepared: &PreparedLocalnet, deadline: Instant) -> Result<()> {
        require_deadline(deadline)?;
        if self.required.height < self.enrollment.finalized().height
            || (self.required.height == self.enrollment.finalized().height
                && &self.required != self.enrollment.finalized())
            || self.required.height == 0
            || *self.required.block_hash.as_ref() == [0; 32]
            || prepared.context.dataspace_id != 0
            || prepared.service_profile != LocalnetServiceProfile::StreamTokenAuthorities
        {
            return Err(invalid("generated activation discovery scope differs"));
        }
        Ok(())
    }

    fn require_policy(&self, policies: &GeneratedProviderPolicies) -> Result<()> {
        if self.enrollment.statement().binding != policies.custody.binding {
            return Err(invalid(
                "selected enrollment differs from original custody policy",
            ));
        }
        Ok(())
    }

    fn verify_custody(
        self,
        owner: &ServiceAuthority,
        policies: &GeneratedProviderPolicies,
        block: &VerifiedSumeragiBlock,
        deadline: Instant,
    ) -> Result<Option<()>> {
        // Only the custody owner qualifies the retained enrollment at the certified Global cut.
        ManagedStreamTokenCustody::open_existing_from_original(owner, self.provider, None)?
            .ok_or_else(|| invalid("original provider custody is absent"))?
            .verify_current_enrollment_at(
                self.enrollment,
                &policies.custody,
                self.required.height,
                *self.required.block_hash.as_ref(),
                block,
                deadline,
            )
    }

    fn bind(
        self,
        owner: &ServiceAuthority,
        policies: &GeneratedProviderPolicies,
        current: VerifiedAccountReadProviderV1,
        block: &VerifiedSumeragiBlock,
        deadline: Instant,
    ) -> Result<GeneratedServiceObservation> {
        let Self {
            provider,
            enrollment,
            required,
        } = self;
        if current.discovery().height() < required.height
            || current.token_public_key() != &policies.custody.binding.public_key
            || u64::from(current.token_key_revision()) != policies.custody.binding.key_revision
            || current.enrollment_expires_at_unix_ms() != enrollment.statement().expires_at_unix_ms
        {
            return Err(invalid(
                "current native provider signer differs from original activation",
            ));
        }
        let (original_slot, plan) = owner
            .provider_plans()?
            .iter()
            .enumerate()
            .find(|(_, plan)| plan.provider_id() == provider)
            .ok_or_else(|| invalid("original generated provider plan is absent"))?;
        GeneratedLocalProviderTransportV1::select(
            plan.network_id(),
            owner.config.chain.as_str(),
            plan.provider_id(),
            &plan.reserve_terms().provider_account,
            plan.admission_material(),
        )
        .and_then(|original| original.authenticate_current(&current))
        .map_err(|_| {
            invalid("current native provider differs from original generated transport")
        })?;
        let valid_until = current
            .enrollment_expires_at_unix_ms()
            .min(policies.custody.active_until_unix_ms)
            .min(policies.gateway.valid_until_unix_ms)
            .min(
                current
                    .discovery()
                    .admission()
                    .envelope()
                    .retention_epoch
                    .checked_mul(1_000)
                    .ok_or_else(|| invalid("provider admission interval overflow"))?,
            )
            .min(
                current
                    .discovery()
                    .advert()
                    .expires_at
                    .checked_mul(1_000)
                    .ok_or_else(|| invalid("provider advertisement interval overflow"))?,
            );
        owner.validate_profile()?;
        require_deadline(deadline)?;
        if now_ms()? >= valid_until {
            return Err(invalid("native generated provider observation expired"));
        }
        Ok(GeneratedServiceObservation {
            original_slot,
            certified_cut: (block.height(), block.context_id()),
            valid_until_unix_ms: valid_until,
            advert_interval: (
                current
                    .discovery()
                    .advert()
                    .issued_at
                    .checked_mul(1_000)
                    .ok_or_else(|| invalid("provider advertisement interval overflow"))?,
                current
                    .discovery()
                    .advert()
                    .expires_at
                    .checked_mul(1_000)
                    .ok_or_else(|| invalid("provider advertisement interval overflow"))?,
            ),
        })
    }
}

/// The original three storage seats read the same freshly authenticated Global block. Each
/// worker still verifies the complete provider row and signer custody independently. The
/// network journal has one owner, and all workers join before its final source check.
pub(in crate::managed) fn observe_generated_service_set(
    prepared: &PreparedLocalnet,
    renderer: Option<&crate::managed::generated_service_runtime::GeneratedServiceRuntime>,
    requests: [ServiceRead<'_>; 3],
    deadline: Instant,
) -> Result<[GeneratedServiceObservation; 3]> {
    for request in &requests {
        request.validate(prepared, deadline)?;
    }
    let [first, second, third] = requests.map(|request| request.provider);
    if first == second || first == third || second == third {
        return Err(invalid(
            "generated observation repeated an original provider",
        ));
    }
    if norito::core::decode_limits_active() {
        // Keep each original finality acquisition and its codec charges in the caller.
        // Even separately checked witnesses must name one identical certified decision.
        let read = |request: ServiceRead<'_>| {
            observe_generated_service(
                prepared,
                request.provider,
                request.enrollment,
                request.required,
                deadline,
            )
        };
        let [first, second, third] = requests;
        let rows = [read(first)?, read(second)?, read(third)?];
        if rows.map(|row| row.original_slot) != [0, 1, 2] {
            return Err(invalid("generated observation provider order differs"));
        }
        consistent_cuts(rows.map(|observation| observation.certified_cut))?;
        return Ok(rows);
    }
    let policies = match renderer {
        Some(renderer) => renderer.selected_observation_policies(prepared)?,
        None => crate::managed::service_bootstrap::ManagedServiceBootstrap::open(prepared)?
            .selected_policies()?,
    };
    for (request, original) in requests.iter().zip(&policies.providers) {
        if request.provider != original.provider_id {
            return Err(invalid("generated observation provider order differs"));
        }
        request.require_policy(original)?;
    }
    let mut owner = match renderer {
        Some(renderer) => renderer.open_observation(prepared)?,
        None => ServiceAuthority::open_network(prepared, NetworkPurpose::ServiceObservation)?,
    };
    owner.validate_profile()?;
    let result = read_current_service_rows(
        &mut owner,
        |owner| {
            discovery_cut(owner, deadline)
                .map_err(|_| invalid("fresh native generated provider discovery is unavailable"))
        },
        |owner, block| {
            let read = |slot: usize| {
                let request = requests[slot];
                owner.provider_inventory(request.provider)?;
                owner.validate_profile()?;
                let policies = &policies.providers[slot];
                let current = read_service_proofs(
                    || {
                        read_discovery_at(owner, request.provider, block, deadline).map_err(|_| {
                            invalid("fresh native generated provider discovery is unavailable")
                        })
                    },
                    || request.verify_custody(owner, policies, block, deadline),
                )?;
                current
                    .map(|value| request.bind(owner, policies, value, block, deadline))
                    .transpose()
            };
            crate::managed::provider_round::run([0, 1, 2], true, read, || {
                invalid("generated provider proof worker failed")
            })
        },
    );
    owner.validate_profile()?;
    require_deadline(deadline)?;
    result
}

// Both endpoints serve only their live height. Start the independent custody proof while
// obtaining discovery at the same immutable cut. Every worker is joined, including on a
// refusal. Exact current-proof unavailability from either endpoint permits one whole-set
// re-read at a newer certified cut. Any verification, transport or local refusal ends the turn.
fn read_service_proofs<T>(
    discovery: impl FnOnce() -> Result<Option<T>>,
    custody: impl FnOnce() -> Result<Option<()>> + Send,
) -> Result<Option<T>> {
    if norito::core::decode_limits_active() {
        return match discovery()? {
            Some(current) => Ok(custody()?.map(|()| current)),
            None => Ok(None),
        };
    }
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .spawn_scoped(scope, custody)
            .map_err(|_| invalid("generated provider custody worker unavailable"))?;
        let current = discovery();
        let verified = worker
            .join()
            .map_err(|_| invalid("generated provider custody worker failed"))?;
        let current = current?;
        Ok(verified?.and(current))
    })
}

// A 503 for the exact current-height proof can occur if consensus advances during
// the round. Only a strictly newer certified cut warrants one complete replacement read.
fn read_current_service_rows<T>(
    owner: &mut ServiceAuthority,
    mut cut: impl FnMut(&mut ServiceAuthority) -> Result<VerifiedSumeragiBlock>,
    mut read: impl FnMut(&ServiceAuthority, &VerifiedSumeragiBlock) -> Result<[Option<T>; 3]>,
) -> Result<[T; 3]> {
    let started = Instant::now();
    let first = cut(owner)?;
    let first_cut_elapsed = started.elapsed();
    if let [Some(a), Some(b), Some(c)] = read(owner, &first)? {
        return Ok([a, b, c]);
    }
    // A typed current-height refusal can race ordinary publication. Log only locally
    // verified height and physical durations, never an untrusted response or proof.
    use std::io::Write as _;
    let _ = writeln!(
        std::io::stderr().lock(),
        "generated discovery current rows unavailable height={} cut_ms={} rows_ms={}",
        first.height(),
        first_cut_elapsed.as_millis(),
        started
            .elapsed()
            .saturating_sub(first_cut_elapsed)
            .as_millis(),
    );
    let next = cut(owner)?;
    if next.height() > first.height()
        && let [Some(a), Some(b), Some(c)] = read(owner, &next)?
    {
        return Ok([a, b, c]);
    }
    Err(invalid(
        "fresh native generated provider discovery is unavailable",
    ))
}

fn consistent_cuts(cuts: [(u64, iroha_crypto::Hash); 3]) -> Result<()> {
    if cuts[1..].iter().any(|cut| *cut != cuts[0]) {
        return Err(invalid(
            "generated observations name different certified blocks",
        ));
    }
    Ok(())
}

fn discover(
    owner: &mut ServiceAuthority,
    provider: ProviderId,
    deadline: Instant,
) -> std::result::Result<
    (VerifiedAccountReadProviderV1, VerifiedSumeragiBlock),
    MusubiArchiveDiscoveryErrorV1,
> {
    check_deadline(deadline)?;
    owner
        .provider_inventory(provider)
        .map_err(|_| MusubiArchiveDiscoveryErrorV1::Rejected)?;
    owner
        .validate_profile()
        .map_err(|_| MusubiArchiveDiscoveryErrorV1::Rejected)?;
    let block = discovery_cut(owner, deadline)?;
    let verified = read_discovery_at(owner, provider, &block, deadline)?
        .ok_or(MusubiArchiveDiscoveryErrorV1::Unavailable)?;
    Ok((verified, block))
}

fn discovery_cut(
    owner: &mut ServiceAuthority,
    deadline: Instant,
) -> std::result::Result<VerifiedSumeragiBlock, MusubiArchiveDiscoveryErrorV1> {
    let finality = owner.observe_finality(deadline);
    check_deadline(deadline)?;
    let finality = finality.map_err(|_| MusubiArchiveDiscoveryErrorV1::Unavailable)?;
    let block = finality
        .verified_tip()
        .map_err(|_| MusubiArchiveDiscoveryErrorV1::Rejected)?;
    block
        .verify_global_scope(owner.config.network_id, owner.config.chain.as_str())
        .map_err(|_| MusubiArchiveDiscoveryErrorV1::Rejected)?;
    Ok(block)
}

fn read_discovery_at(
    owner: &ServiceAuthority,
    provider: ProviderId,
    block: &VerifiedSumeragiBlock,
    deadline: Instant,
) -> std::result::Result<Option<VerifiedAccountReadProviderV1>, MusubiArchiveDiscoveryErrorV1> {
    check_deadline(deadline)?;
    let schema = iroha_core::state::State::native_world_schema_hashes_v1()
        .map_err(|_| MusubiArchiveDiscoveryErrorV1::Unavailable)?;
    let inventory = owner
        .provider_inventory(provider)
        .map_err(|_| MusubiArchiveDiscoveryErrorV1::Rejected)?;
    // Spread simultaneous proofs across their original validators. Each proof can still
    // fall back to every other selected validator at the same independently certified cut.
    let peers = owner
        .peers
        .iter()
        .cycle()
        .skip(usize::from(inventory.slot))
        .take(owner.peers.len());
    let mut only_current_unavailable = !owner.peers.is_empty();
    let result = read_selected_peers(peers, deadline, |client, selected_deadline| {
        let now = now_ms().inspect_err(|_| only_current_unavailable = false)?;
        client
            .with_request_deadline(selected_deadline)
            .get_account_read_provider_discovery(provider, schema.as_slice(), block, now)
            .map_err(|error| {
                only_current_unavailable &= error
                    .downcast_ref::<iroha::client::ProviderDiscoveryUnavailable>()
                    .is_some();
                invalid("generated native provider discovery unavailable or invalid")
            })
    });
    check_deadline(deadline)?;
    let verified = match result {
        Ok(verified) => verified,
        Err(_) if only_current_unavailable => return Ok(None),
        Err(_) => return Err(MusubiArchiveDiscoveryErrorV1::Unavailable),
    };
    let discovery = verified.discovery();
    if verified.chain_id() != owner.config.chain.as_str()
        || discovery.network_id() != owner.config.network_id
        || discovery.height() != block.height()
        || discovery.context_id() != block.context_id()
        || discovery.owner()
            != &inventory
                .authority(StreamTokenAuthorityRole::IssuerOperator)
                .map_err(|_| MusubiArchiveDiscoveryErrorV1::Rejected)?
                .account
    {
        return Err(MusubiArchiveDiscoveryErrorV1::Rejected);
    }
    owner
        .validate_profile()
        .map_err(|_| MusubiArchiveDiscoveryErrorV1::Rejected)?;
    check_deadline(deadline)?;
    Ok(Some(verified))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod activation_tests;

#[cfg(test)]
mod readback_origin_tests;
