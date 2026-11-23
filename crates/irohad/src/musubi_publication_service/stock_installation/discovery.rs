//! Cold ordinary account discovery from one actual current native cut.

use super::{Cache, Error as FactoryError};
use iroha_allocation::{AllocationBudget, AllocationCharge};
use iroha_core::{
    state::{BlockHashRead, State, StateReadOnly, WorldStateSnapshotError},
    sumeragi::finality::{
        NativeCurrentFinalityV1, NativeCurrentStepV1, NativeFinalityCursorErrorV1,
        NativeFinalityCursorV1,
    },
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
    ops::ControlFlow,
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
        let mut clock_floor = now()?;
        let mut cursor = None;
        let mut previous_cut = None;
        let mut durable_floor = None;
        advance_current_in_slices(norito::core::decode_limits_active(), |steps, may_retry| {
            check(deadline)?;
            let view = self.state.view();
            let height = view.height();
            if height < 2
                || view.network_id() != &self.network
                || view.chain_id().as_str() != self.chain
            {
                return Err(Error::Unavailable);
            }
            require_previous_cut(previous_cut, view.block_hashes())?;
            require_durable_floor(durable_floor.take(), height)?;
            let held = match &mut cursor {
                Some(held) => held,
                None => cursor.insert(self.cursor.try_lock().map_err(|_| Error::Unavailable)?),
            };
            let current = match held
                .advance_current_or_superseded(&view, &self.budget, deadline, steps)
                .map_err(|error| match error {
                    NativeFinalityCursorErrorV1::Deadline => Error::Deadline,
                    _ => Error::Unavailable,
                })? {
                NativeCurrentStepV1::Current(current) => current,
                NativeCurrentStepV1::Incomplete => {
                    previous_cut =
                        Some((height, view.latest_block_hash().ok_or(Error::Unavailable)?));
                    return Ok(None);
                }
                NativeCurrentStepV1::Superseded(cut) => {
                    // This cut has no result. A later slice continues only at a strictly
                    // newer cut that extends it.
                    let previous = superseded_cut(
                        (cut.height(), cut.hash(), cut.durable_height()),
                        view.block_hashes(),
                        may_retry,
                    )?;
                    previous_cut = Some(previous);
                    durable_floor = Some(cut.durable_height());
                    return Ok(None);
                }
            };
            drop(view);
            drop(cursor.take());
            match self.read_at(original, &current, &mut clock_floor, deadline, may_retry)? {
                ControlFlow::Break(result) => Ok(Some(result)),
                ControlFlow::Continue(cut) => {
                    previous_cut = Some(cut);
                    Ok(None)
                }
            }
        })
    }

    fn read_at(
        &self,
        original: &GeneratedLocalProviderTransportV1,
        current: &NativeCurrentFinalityV1,
        clock_floor: &mut u64,
        deadline: Instant,
        may_retry: bool,
    ) -> Result<ControlFlow<VerifiedAccountReadProviderV1, DiscoveryCut>, Error> {
        check(deadline)?;
        let tip = current.native_block();
        let height = tip.height();
        let provider = original.provider_id();
        let observed = now()?;
        if observed < *clock_floor {
            return Err(Error::Unavailable);
        }
        *clock_floor = observed;
        check(deadline)?;
        let guard = self.cache.try_read().map_err(|_| Error::Unavailable)?;
        let advert = guard
            .unverified_record_for_native_proof(provider.as_bytes())
            .ok_or(Error::Unavailable)?
            .advert();
        let length = norito::canonical_frame_len(advert).map_err(|_| Error::Rejected)?;
        if length > MAX_PROVIDER_DISCOVERY_ADVERT_BYTES_V1 {
            return Err(Error::Rejected);
        }
        let advert_charge = self
            .budget
            .try_reserve_bytes(length)
            .map_err(|_| Error::Unavailable)?;
        let advert = {
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::to_bytes_bounded(advert, length).map_err(|_| Error::Unavailable)?
        };
        drop(guard);
        check(deadline)?;
        let proof_charge = self
            .budget
            .try_reserve_bytes(MAX_PROVIDER_DISCOVERY_BYTES_V1 + 2 * 128 * 1024 * 1024)
            .map_err(|_| Error::Unavailable)?;
        let result = self.state.with_native_provider_admission_snapshot_v1(
            tip,
            provider,
            &self.budget,
            |originals| {
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
            },
        );
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                let Some(minimum) = advanced_snapshot_height(&error, height).filter(|_| may_retry)
                else {
                    return Err(Error::Unavailable);
                };
                check(deadline)?;
                let view = self.state.view();
                return checked_extension(
                    height,
                    tip.block_hash(),
                    minimum,
                    view.block_hashes(),
                    view.kura().exact_durable_blocks_count().ok(),
                )
                .map(ControlFlow::Continue);
            }
        };
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
            return Err(Error::Unavailable);
        }
        *clock_floor = after;
        let view = self.state.view();
        if may_retry && u64::try_from(view.height()).is_ok_and(|live| live > height) {
            return checked_extension(
                height,
                tip.block_hash(),
                height,
                view.block_hashes(),
                view.kura().exact_durable_blocks_count().ok(),
            )
            .map(ControlFlow::Continue);
        }
        if u64::try_from(view.height()).ok() != Some(height)
            || view.latest_block_hash() != Some(tip.block_hash())
            || view.kura().exact_durable_blocks_count().ok() != Some(view.block_hashes().len())
        {
            return Err(Error::Unavailable);
        }
        check(deadline)?;
        Ok(ControlFlow::Break(result))
    }
}

// A completed proof is released only at its original exact durable head. A verified
// extension may consume another slice of the same attempt to select and prove a fresh cut.
type DiscoveryCut = (usize, HashOf<BlockHeader>);

fn advanced_snapshot_height(error: &WorldStateSnapshotError, selected: u64) -> Option<u64> {
    match error {
        WorldStateSnapshotError::CutChanged {
            requested_height,
            observed_height,
        } if *requested_height == selected && *observed_height > selected => Some(*observed_height),
        _ => None,
    }
}

fn checked_extension(
    selected: u64,
    selected_hash: HashOf<BlockHeader>,
    minimum: u64,
    hashes: &dyn BlockHashRead,
    durable_height: Option<usize>,
) -> Result<DiscoveryCut, Error> {
    let height = hashes.len();
    let live = u64::try_from(height).map_err(|_| Error::Unavailable)?;
    if live <= selected || live < minimum || durable_height != Some(height) {
        return Err(Error::Unavailable);
    }
    let origin = usize::try_from(selected).map_err(|_| Error::Unavailable)?;
    require_previous_cut(Some((origin, selected_hash)), hashes)?;
    let hash = height
        .checked_sub(1)
        .and_then(|index| hashes.get(index))
        .ok_or(Error::Unavailable)?;
    Ok((height, *hash))
}

// A newer selection may only extend the original physical view. A regressed or
// forked view must never make a prior incomplete native call current.
fn require_previous_cut(
    previous: Option<DiscoveryCut>,
    hashes: &dyn BlockHashRead,
) -> Result<(), Error> {
    if let Some((height, hash)) = previous
        && height.checked_sub(1).and_then(|index| hashes.get(index)) != Some(&hash)
    {
        return Err(Error::Unavailable);
    }
    Ok(())
}

// A cut the cursor reported superseded by a strictly newer durable count is never current.
// It may continue only from the exact view it classified, while another slice remains.
fn superseded_cut(
    (height, hash, durable): (u64, HashOf<BlockHeader>, u64),
    hashes: &dyn BlockHashRead,
    may_retry: bool,
) -> Result<DiscoveryCut, Error> {
    let origin = usize::try_from(height).map_err(|_| Error::Unavailable)?;
    if !may_retry || durable <= height || hashes.len() != origin || hashes.last() != Some(&hash) {
        return Err(Error::Unavailable);
    }
    Ok((origin, hash))
}

// A new applied view must reach the durable height that superseded its predecessor.
// The predecessor hash is checked independently by `require_previous_cut`.
fn require_durable_floor(floor: Option<u64>, applied: usize) -> Result<(), Error> {
    if let Some(floor) = floor
        && u64::try_from(applied).map_err(|_| Error::Unavailable)? < floor
    {
        return Err(Error::Unavailable);
    }
    Ok(())
}

// Each incomplete native call retains its authenticated progress. Reopen the current
// view before the next group so a long catch-up does not pin its original tip throughout.
// All calls share the same cursor, deadline and maximum aggregate successor count.
fn advance_current_in_slices<T>(
    metered: bool,
    mut advance: impl FnMut(NonZeroU16, bool) -> Result<Option<T>, Error>,
) -> Result<T, Error> {
    const TOTAL: u16 = 64;
    let slice = if metered { TOTAL } else { 4 };
    let steps = NonZeroU16::new(slice).expect("fixed nonzero finality step count");
    for group in 1..=TOTAL / slice {
        if let Some(current) = advance(steps, group < TOTAL / slice)? {
            return Ok(current);
        }
    }
    Err(Error::Unavailable)
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
        .map_err(|_| Error::Unavailable)?
        .as_millis();
    u64::try_from(value)
        .ok()
        .filter(|value| *value != 0)
        .ok_or(Error::Unavailable)
}

#[cfg(test)]
mod cut_tests {
    use super::*;

    #[test]
    fn current_views_follow_only_successful_bounded_progress() {
        let mut rounds = 0;
        let answer = advance_current_in_slices(false, |steps, _| {
            assert_eq!(steps.get(), 4);
            rounds += 1;
            Ok((rounds == 5).then_some(76))
        })
        .unwrap();
        assert_eq!((rounds, answer), (5, 76));
        for metered in [false, true] {
            let mut requested = Vec::new();
            assert!(matches!(
                advance_current_in_slices::<()>(metered, |steps, may_retry| {
                    requested.push(steps.get());
                    assert_eq!(may_retry, requested.iter().sum::<u16>() < 64);
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
    fn snapshot_retry_requires_the_original_typed_forward_observation() {
        for (requested, observed, expected) in [
            (75, 76, Some(76)),
            (75, 75, None),
            (75, 74, None),
            (74, 76, None),
        ] {
            let cut = WorldStateSnapshotError::CutChanged {
                requested_height: requested,
                observed_height: observed,
            };
            assert_eq!(advanced_snapshot_height(&cut, 75), expected);
        }
        let invalid = WorldStateSnapshotError::Invalid("World snapshot cut changed".to_owned());
        assert_eq!(advanced_snapshot_height(&invalid, 75), None);
    }

    #[test]
    fn reselected_head_requires_a_durable_linked_extension_and_full_new_proof() {
        let hash = |byte| {
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([byte; 32]))
        };
        let (first, second, third) = (hash(1), hash(2), hash(3));
        let hashes = [first, second, third];
        assert_eq!(
            checked_extension(2, second, 3, &hashes, Some(3)),
            Ok((3, third))
        );
        assert!(checked_extension(2, third, 3, &hashes, Some(3)).is_err());
        assert!(checked_extension(3, third, 3, &hashes, Some(3)).is_err());
        assert!(checked_extension(4, third, 3, &hashes, Some(3)).is_err());
        assert!(checked_extension(2, second, 4, &hashes, Some(3)).is_err());
        assert!(checked_extension(2, second, 3, &hashes, Some(2)).is_err());
        assert!(checked_extension(2, second, 3, &hashes, None).is_err());
    }

    #[test]
    fn superseded_cut_continues_only_at_a_strictly_newer_linked_view() {
        let hash = |byte| {
            HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::prehashed([byte; 32]))
        };
        let (first, second, third) = (hash(1), hash(2), hash(3));
        // The cursor's own classified view, while another slice remains.
        assert_eq!(
            superseded_cut((2, second, 3), &[first, second], true),
            Ok((2, second))
        );
        // No slice remains, no newer durable count, another view or another tip.
        assert!(superseded_cut((2, second, 3), &[first, second], false).is_err());
        assert!(superseded_cut((2, second, 2), &[first, second], true).is_err());
        assert!(superseded_cut((2, second, 3), &[first, second, third], true).is_err());
        assert!(superseded_cut((2, third, 3), &[first, second], true).is_err());
        // H3 extends H2, but is insufficient after the journal reported durable H4.
        let cut = Some((2, second));
        assert_eq!(require_durable_floor(Some(4), 3), Err(Error::Unavailable));
        assert!(require_durable_floor(Some(4), 4).is_ok());
        assert!(require_durable_floor(Some(4), 5).is_ok());
        assert!(require_durable_floor(None, 2).is_ok());
        assert!(require_previous_cut(cut, &[first, second, third]).is_ok());
        assert_eq!(
            require_previous_cut(cut, &[first, third, second]),
            Err(Error::Unavailable)
        );
    }

    #[test]
    fn any_native_refusal_stops_the_original_discovery() {
        for error in [Error::Rejected, Error::Unavailable, Error::Deadline] {
            let mut rounds = 0;
            let observed = advance_current_in_slices::<()>(false, |_, _| {
                rounds += 1;
                if rounds == 1 { Ok(None) } else { Err(error) }
            })
            .unwrap_err();
            assert_eq!(observed, error);
            assert_eq!(rounds, 2);
        }
    }
}
