//! Cold ordinary account discovery from one actual current native cut.

use super::{Cache, Error as FactoryError};
use iroha_allocation::{AllocationBudget, AllocationCharge};
use iroha_core::{
    state::{BlockHashRead, State, StateReadOnly},
    sumeragi::finality::{NativeFinalityCursorErrorV1, NativeFinalityCursorV1},
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    sorafs::{
        capacity::ProviderId,
        provider_admission::discovery::{
            MAX_PROVIDER_DISCOVERY_ADVERT_BYTES_V1, MAX_PROVIDER_DISCOVERY_BYTES_V1,
            ProviderDiscoveryProofRefV1, ProviderDiscoveryProofV1,
            account_read::VerifiedAccountReadProviderV1,
        },
    },
};
use iroha_storage_client::musubi_archive_fetch::{
    MusubiArchiveDiscoveryErrorV1 as Error, MusubiArchiveProviderDiscoveryV1,
};
use sorafs_car::gateway::GeneratedLocalProviderTransportV1;
use std::{
    alloc::Layout,
    num::NonZeroU16,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) fn prepare(
    state: Arc<State>,
    cache: Cache,
    originals: [GeneratedLocalProviderTransportV1; 3],
    timeout: Duration,
    selection_charge: AllocationCharge,
) -> Result<Arc<MusubiArchiveProviderDiscoveryV1>, FactoryError> {
    if timeout.is_zero() || timeout > Duration::from_millis(iroha_config::parameters::defaults::musubi_publication::MAX_READBACK_REQUEST_TIMEOUT_MS) { return Err(FactoryError::Unqualified); }
    let budget = state.ivm_execution_budget();
    let network = *state.network_id_ref();
    let chain = state.view().chain_id().to_string();
    // The closed service serializes readback calls. This bounds retained verified admission
    // projections while the callback/client exists; full temporary World proof is funded below.
    let layout =
        Layout::array::<u8>(3 * 16 * 1024 * 1024).map_err(|_| FactoryError::Unavailable)?;
    let result_charge = budget
        .try_reserve(layout)
        .map_err(|_| FactoryError::Unavailable)?
        .try_split(layout)
        .map_err(|_| FactoryError::Unavailable)?;
    let selected = Discovery {
        state,
        cache,
        originals,
        timeout,
        budget,
        network,
        chain,
        cursor: Mutex::new(NativeFinalityCursorV1::new(
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
        )),
        _selection_charge: selection_charge,
        _result_charge: result_charge,
    };
    Ok(Arc::new(move |provider| selected.read(provider)))
}

struct Discovery {
    state: Arc<State>,
    cache: Cache,
    originals: [GeneratedLocalProviderTransportV1; 3],
    timeout: Duration,
    budget: AllocationBudget,
    network: NetworkId,
    chain: String,
    cursor: Mutex<NativeFinalityCursorV1>,
    _selection_charge: AllocationCharge,
    _result_charge: AllocationCharge,
}
impl Discovery {
    fn read(&self, provider: ProviderId) -> Result<VerifiedAccountReadProviderV1, Error> {
        // No clock, source acquisition, callback or network request precedes exact selection.
        let original = self
            .originals
            .iter()
            .find(|original| original.provider_id() == provider)
            .ok_or(Error::Rejected)?;
        let deadline = Instant::now()
            .checked_add(self.timeout)
            .ok_or(Error::Deadline)?;
        let started = now()?;
        let mut cursor = None;
        let mut previous_cut = None;
        let (height, current) =
            advance_current_in_slices(norito::core::decode_limits_active(), |steps| {
                check(deadline)?;
                let view = self.state.view();
                let height = view.height();
                if height < 2
                    || view.network_id() != &self.network
                    || view.chain_id().as_str() != self.chain
                {
                    return Err(discovery_unavailable(line!()));
                }
                require_previous_cut(previous_cut, view.block_hashes())?;
                let cursor = match &mut cursor {
                    Some(held) => held,
                    None => cursor.insert(self.cursor.try_lock().map_err(|_| discovery_unavailable(line!()))?),
                };
                let durable_before = view.kura().exact_durable_blocks_count().ok();
                let current = cursor
                    .advance_current(&view, &self.budget, deadline, steps)
                    .map_err(|error| match error {
                        NativeFinalityCursorErrorV1::Deadline => Error::Deadline,
                        other => {
                            eprintln!("[paid native readback] finality cursor refusal variant={:?}", std::mem::discriminant(&other));
                            eprintln!("[paid native readback] cursor refusal pinned={} before={:?} after={:?}", view.block_hashes().len(), durable_before, view.kura().exact_durable_blocks_count().ok());
                            discovery_unavailable(line!())
                        },
                    })?;
                if let Some(current) = current {
                    let height = u64::try_from(height).map_err(|_| discovery_unavailable(line!()))?;
                    return Ok(Some((height, current)));
                }
                previous_cut = Some((height, view.latest_block_hash().ok_or_else(|| discovery_unavailable(line!()))?));
                Ok(None)
            })?;
        drop(cursor);
        check(deadline)?;
        let tip = current.native_block();
        let observed = now()?;
        if observed < started {
            return Err(discovery_unavailable(line!()));
        }
        check(deadline)?;
        let guard = self.cache.try_read().map_err(|_| discovery_unavailable(line!()))?;
        let advert = guard
            .unverified_record_for_native_proof(provider.as_bytes())
            .ok_or_else(|| discovery_unavailable(line!()))?
            .advert();
        let length = norito::canonical_frame_len(advert).map_err(|_| Error::Rejected)?;
        if length > MAX_PROVIDER_DISCOVERY_ADVERT_BYTES_V1 {
            return Err(Error::Rejected);
        }
        let advert_charge = self
            .budget
            .try_reserve_bytes(length)
            .map_err(|_| discovery_unavailable(line!()))?;
        let advert = {
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::to_bytes_bounded(advert, length).map_err(|_| discovery_unavailable(line!()))?
        };
        drop(guard);
        check(deadline)?;
        let proof_charge = self
            .budget
            .try_reserve_bytes(MAX_PROVIDER_DISCOVERY_BYTES_V1 + 2 * 128 * 1024 * 1024)
            .map_err(|_| discovery_unavailable(line!()))?;
        let result = self
            .state
            .with_native_provider_admission_snapshot_v1(tip, provider, &self.budget, |originals| {
                check(deadline).map_err(|_| "native discovery deadline".to_owned())?;
                // One cumulative decoder allowance covers proof and nested native material
                // verification. An equal prepaid envelope covers their bounded model clones.
                norito::with_decode_limits_scope(
                    norito::DecodeLimits::new(
                        131_072,
                        MAX_PROVIDER_DISCOVERY_BYTES_V1,
                        131_072,
                        128 * 1024 * 1024,
                        64,
                    ),
                    || {
                        let borrowed = ProviderDiscoveryProofRefV1::new(
                            originals.world,
                            (originals.council_head, originals.council_predecessor),
                            (originals.provider_head, originals.provider_predecessor),
                            originals.owner,
                            &advert,
                            originals.stream_token,
                        );
                        let length = norito::canonical_frame_len(&borrowed)
                            .map_err(|_| "native discovery extent".to_owned())?;
                        if length > MAX_PROVIDER_DISCOVERY_BYTES_V1 {
                            return Err("native discovery extent".to_owned());
                        }
                        check(deadline).map_err(|_| "native discovery deadline".to_owned())?;
                        let bytes = {
                            let _flags = norito::core::DecodeFlagsGuard::enter(
                                norito::core::default_encode_flags(),
                            );
                            norito::core::to_bytes_bounded(&borrowed, length)
                                .map_err(|_| "native discovery encoding".to_owned())?
                        };
                        let proof = ProviderDiscoveryProofV1::decode_frame(&bytes)
                            .map_err(|_| "native discovery decoding".to_owned())?;
                        check(deadline).map_err(|_| "native discovery deadline".to_owned())?;
                        let result = proof
                            .verify_account_read(
                                &self.chain,
                                self.network,
                                provider,
                                State::select_native_world_schema_v1(proof.world.schema_hash)
                                    .map_err(|_| "native schema unavailable".to_owned())?,
                                current.block(),
                                observed,
                            )
                            .map_err(|_| "native account discovery refused".to_owned())?;
                        // Keep the full temporary envelope until the decoded result is proved to
                        // match the small exact retained original transport graph.
                        original
                            .authenticate_current(&result)
                            .map_err(|_| "native original transport differs".to_owned())?;
                        Ok(result)
                    },
                )
            })
            .map_err(|err| {
                use iroha_core::state::WorldStateSnapshotError as Snapshot;
                match &err {
                    Snapshot::CutChanged { requested_height, observed_height } => eprintln!("[paid native readback] snapshot cut changed requested={} observed={}", requested_height, observed_height),
                    Snapshot::Invalid(reason) => {
                        let label = match reason.as_str() {
                            "native account discovery refused" => "account-validation",
                            "native original transport differs" => "original-transport",
                            "World snapshot publication generation changed" => "publication-generation",
                            "native discovery deadline" => "deadline",
                            "native schema unavailable" => "schema",
                            _ => "other-invalid",
                        };
                        eprintln!("[paid native readback] snapshot invalid reason={label}");
                    }
                    other => eprintln!("[paid native readback] snapshot refusal variant={:?}", std::mem::discriminant(other)),
                }
                discovery_unavailable(line!())
            })?;
        drop(proof_charge);
        drop(advert);
        drop(advert_charge);
        check(deadline)?;
        let after = now()?;
        if after < observed
            || after >= result.enrollment_expires_at_unix_ms()
            || after / 1000 >= result.discovery().advert().expires_at
            || after / 1000 >= result.discovery().admission().envelope().retention_epoch
        {
            return Err(discovery_unavailable(line!()));
        }
        let view = self.state.view();
        if u64::try_from(view.height()).ok() != Some(height)
            || view.latest_block_hash() != Some(tip.block_hash())
            || view.kura().exact_durable_blocks_count().ok() != Some(view.block_hashes().len())
        {
            eprintln!("[paid native readback] final cut unavailable requested={} observed={} same_hash={} canonical_count={:?}", height, view.height(), view.latest_block_hash() == Some(tip.block_hash()), view.kura().exact_durable_blocks_count().ok());
            return Err(discovery_unavailable(line!()));
        }
        check(deadline)?;
        Ok(result)
    }
}
// A newer selection may only extend the original physical view. A regressed or
// forked view must never make a prior incomplete native call current.
fn require_previous_cut(
    previous: Option<(usize, HashOf<BlockHeader>)>,
    hashes: &dyn BlockHashRead,
) -> Result<(), Error> {
    if let Some((height, hash)) = previous
        && height.checked_sub(1).and_then(|index| hashes.get(index)) != Some(&hash)
    {
        return Err(discovery_unavailable(line!()));
    }
    Ok(())
}

// Each incomplete native call retains its authenticated progress. Reopen the current
// view before the next group so a long catch-up does not pin its original tip throughout.
// All calls share the same cursor, deadline and maximum aggregate successor count.
fn advance_current_in_slices<T>(
    metered: bool,
    mut advance: impl FnMut(NonZeroU16) -> Result<Option<T>, Error>,
) -> Result<T, Error> {
    const TOTAL: u16 = 64;
    let slice = if metered { TOTAL } else { 4 };
    let steps = NonZeroU16::new(slice).expect("fixed nonzero finality step count");
    for _ in 0..TOTAL / slice {
        if let Some(current) = advance(steps)? {
            return Ok(current);
        }
    }
    Err(discovery_unavailable(line!()))
}
fn check(deadline: Instant) -> Result<(), Error> {
    if Instant::now() < deadline {
        Ok(())
    } else {
        Err(Error::Deadline)
    }
}
fn now() -> Result<u64, Error> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| discovery_unavailable(line!()))?
        .as_millis();
    u64::try_from(value)
        .ok()
        .filter(|value| *value != 0)
        .ok_or_else(|| discovery_unavailable(line!()))
}

fn discovery_unavailable(line: u32) -> Error {
    eprintln!("[paid native readback] discovery unavailable line={line}");
    Error::Unavailable
}

#[cfg(test)]
mod cut_tests {
    use super::*;

    #[test]
    fn current_views_follow_only_successful_bounded_progress() {
        let mut rounds = 0;
        let answer = advance_current_in_slices(false, |steps| {
            assert_eq!(steps.get(), 4);
            rounds += 1;
            Ok((rounds == 5).then_some(76))
        })
        .unwrap();
        assert_eq!((rounds, answer), (5, 76));
        for metered in [false, true] {
            let mut requested = Vec::new();
            assert!(matches!(
                advance_current_in_slices::<()>(metered, |steps| {
                    requested.push(steps.get());
                    Ok(None)
                }),
                Err(Error::Unavailable)
            ));
            assert_eq!(requested.iter().sum::<u16>(), 64);
            assert_eq!(requested.len(), if metered { 1 } else { 16 });
        }
    }

    #[test]
    fn refreshed_view_requires_the_original_ancestor() {
        let hash = |byte| {
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([byte; 32]))
        };
        let first = hash(1);
        let second = hash(2);
        let third = hash(3);
        let cut = Some((2, second));
        assert!(require_previous_cut(cut, &[first, second]).is_ok());
        assert!(require_previous_cut(cut, &[first, second, third]).is_ok());
        assert_eq!(require_previous_cut(cut, &[first]), Err(Error::Unavailable));
        assert_eq!(
            require_previous_cut(cut, &[first, third, second]),
            Err(Error::Unavailable)
        );
    }

    #[test]
    fn any_native_refusal_stops_the_original_discovery() {
        for error in [Error::Rejected, Error::Unavailable, Error::Deadline] {
            let mut rounds = 0;
            let observed = advance_current_in_slices::<()>(false, |_| {
                rounds += 1;
                if rounds == 1 { Ok(None) } else { Err(error) }
            })
            .unwrap_err();
            assert_eq!(observed, error);
            assert_eq!(rounds, 2);
        }
    }
}
