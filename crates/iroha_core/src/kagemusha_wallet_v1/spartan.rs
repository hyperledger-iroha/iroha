//! Governed Spartan verifier installation, wallet activation and Unload settlement.
use super::{
    Error, Registration, Result, SuffixTransaction, Transaction, artifacts, ledger, wsv::WsvLedger,
};
use crate::state::{StateReadOnly, StateTransaction, StorageReadOnly, WorldReadOnly};
use iroha_core_zk::kagemusha_spartan_native::native::{
    self, AuthenticatedParameters, NativeEngine,
};
use iroha_data_model::{
    account::AccountId,
    isi::kagemusha_wallet::{
        KagemushaWalletLedgerV1,
        spartan::{
            ActivateKagemushaSpartanV1 as Activate, InstallKagemushaSpartanProgramsV1 as Install,
            UnloadKagemushaSpartanV1 as Unload,
        },
    },
    kagemusha::{KagemushaWalletPayoutRecordV1, kagemusha_wallet_poseidon_bytes_v1},
    runtime::definition::{DefinitionId, RestrictionV1, RootSelectionV1},
    sumeragi_finality::{FinalityValidator, SumeragiCommitVerifierV1, SumeragiFinalityVerifier},
};
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{Arc, LazyLock, Mutex},
};

const _: () = assert!(Activate::MAX_ACTIVATION_BYTES == native::MAX_ACTIVATION_CLAIM_BYTES);

fn native_error(error: native::Error) -> Error {
    match error {
        native::Error::Unavailable(_) | native::Error::NotEnoughMemory { .. } => {
            Error::ArtifactsUnavailable
        }
        _ => Error::Proof,
    }
}

fn owner(selection: &RootSelectionV1, wire: &str) -> Option<DefinitionId> {
    selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire)
        .map(|binding| binding.implementation)
}

fn selects(selection: &RootSelectionV1, wire: &str) -> Result<bool> {
    let catalog = crate::runtime::release_catalog().map_err(|_| Error::Unavailable)?;
    let module = crate::runtime::catalog::additions::KAGEMUSHA_SPARTAN.module;
    let Some(ledger) = owner(selection, KagemushaWalletLedgerV1::WIRE_ID) else {
        return Ok(false);
    };
    if catalog.native_module(&ledger) != Some(module) {
        return Ok(false);
    }
    let addition = catalog
        .additions
        .iter()
        .find(|addition| addition.module == module)
        .ok_or(Error::Unavailable)?;
    let storage = catalog
        .world_storage
        .get("world.kagemusha_spartan_programs")
        .ok_or(Error::Unavailable)?;
    Ok(owner(selection, wire) == Some(ledger)
        && addition
            .interfaces
            .iter()
            .chain(&addition.rebinds)
            .all(|(wire, interface)| {
                selection.bindings.iter().any(|binding| {
                    binding.wire_id == *wire
                        && binding.interface == *interface
                        && binding.implementation == ledger
                })
            })
        && selection.installed_storage.contains(storage))
}

/// An installed scheme's signed Unload settles the already-admitted reserve. The
/// normal handler verifies the proofs, signed destinations, amounts and nullifier, including
/// exact retries after the final payout drained the balance.
pub(crate) fn completes_reserve(world: &impl WorldReadOnly, request: &Unload) -> bool {
    !request.claim.is_empty()
        && request.claim.len() <= native::MAX_UNLOAD_CLAIM_BYTES
        && world
            .kagemusha_spartan_programs()
            .get(&request.scheme)
            .is_some_and(|installed| {
                installed.scheme == request.scheme
                    && installed.asset == request.asset
                    && installed.manifest_digest == request.manifest_digest
            })
        && selects(world.runtime_selection(), Unload::WIRE_ID).unwrap_or(false)
}

/// No new exposure may be created against an installed reserve once its future
/// redemption has been disabled, even before a scheduled restriction takes effect.
pub(crate) fn redemption_available(world: &impl WorldReadOnly) -> bool {
    crate::runtime::obligations::can_admit_with_completion(
        world.runtime_selection(),
        &crate::runtime::change_set::PublishedDefinitions(world.runtime_definitions()),
        Unload::WIRE_ID,
    )
}

fn completion_changes(
    world: &impl WorldReadOnly,
    before: &RootSelectionV1,
    after: &RootSelectionV1,
) -> bool {
    crate::runtime::obligations::closure_gap(
        before,
        after,
        &crate::runtime::change_set::PublishedDefinitions(world.runtime_definitions()),
        &[Unload::WIRE_ID],
        false,
    )
    .is_some()
}

/// A hand-authored change set must preserve the same ledger implementation and containment
/// as a catalog-generated activation. Published variants have distinct service dependencies.
pub(crate) fn validate_cutover(
    current: &RootSelectionV1,
    next: &RootSelectionV1,
    height: u64,
) -> Result<()> {
    let catalog = crate::runtime::release_catalog().map_err(|_| Error::Unavailable)?;
    let module = crate::runtime::catalog::additions::KAGEMUSHA_SPARTAN.module;
    let wire = KagemushaWalletLedgerV1::WIRE_ID;
    let successor = owner(next, wire);
    let predecessor = owner(current, wire);
    if successor == predecessor
        || successor.as_ref().and_then(|id| catalog.native_module(id)) != Some(module)
    {
        return Ok(());
    }
    let predecessor = predecessor.ok_or(Error::Binding)?;
    let successor = successor.ok_or(Error::Binding)?;
    if catalog.spartan_predecessors.get(&predecessor) != Some(&successor)
        || !selects(next, Install::WIRE_ID)?
    {
        return Err(Error::Binding);
    }
    if let Some(old) = current
        .restrictions
        .iter()
        .find(|r| r.definition == predecessor)
    {
        let inherited = match old.restriction {
            RestrictionV1::Deprecated(at) if at <= height => RestrictionV1::Deprecated(height),
            RestrictionV1::AdmissionClosed(at) if at <= height => {
                RestrictionV1::AdmissionClosed(height)
            }
            RestrictionV1::CompletionOnly(at) if at <= height => {
                RestrictionV1::CompletionOnly(height)
            }
            RestrictionV1::Disabled(at) if at <= height => RestrictionV1::Disabled(height),
            _ => return Err(Error::Binding),
        };
        if !next
            .restrictions
            .iter()
            .any(|r| r.definition == successor && r.restriction == inherited)
        {
            return Err(Error::Binding);
        }
    }
    Ok(())
}

/// Preserve the original verifier owner for every installation. A funded reserve
/// also retains its executable settlement dependencies until redemption drains it.
pub(crate) fn check_transition(world: &impl WorldReadOnly, next: &RootSelectionV1) -> Result<()> {
    let programs = world.kagemusha_spartan_programs();
    if programs.iter().next().is_none() {
        return Ok(());
    }
    let current = world.runtime_selection();
    if !selects(current, Unload::WIRE_ID)?
        || !selects(next, Unload::WIRE_ID)?
        || owner(current, Unload::WIRE_ID) != owner(next, Unload::WIRE_ID)
    {
        return Err(Error::Conflict);
    }
    // A plain deposit can credit even a retired reserve. Check at the transition
    // that stops redemption. Once committed, new Loads are refused and unrelated
    // asset credits create no issued offline value.
    if current != next && redemption_available(world) && completion_changes(world, current, next) {
        for (scheme, installed) in programs.iter() {
            let key = super::storage::key(super::storage::REGISTRATION, *scheme, installed.asset);
            let bytes = world
                .kagemusha_wallet_ledger()
                .get(&key)
                .ok_or(Error::Unavailable)?;
            let registration: Registration = super::storage::decode(bytes, bytes.len())?;
            registration.require(scheme, &installed.asset)?;
            if world
                .assets()
                .get(&super::custody::reserve_id(&registration))
                .is_some_and(|quantity| !quantity.as_ref().is_zero())
            {
                return Err(Error::Conflict);
            }
        }
    }
    Ok(())
}

fn original_bounds(installation: &Install) -> Result<()> {
    if installation.manifest.is_empty()
        || installation.manifest.len() > Install::MAX_MANIFEST_BYTES
        || installation.programs.is_empty()
        || installation.programs.len() > native::MAX_PROGRAM_BUNDLE_BYTES
        || installation.scheme == [0; 32]
        || installation.asset == [0; 32]
        || installation.manifest_digest == [0; 32]
    {
        return Err(Error::Binding);
    }
    let _fixed = norito::core::WireLayoutGuard::enter(norito::core::WireLayout::Fixed);
    if norito::core::encoded_frame_len(installation).map_err(|_| Error::Binding)?
        > Install::MAX_BYTES
        || kagemusha_wallet_poseidon_bytes_v1(
            u64::from_le_bytes(*b"kgwparm1"),
            &installation.manifest,
        ) != installation.manifest_digest
    {
        return Err(Error::Binding);
    }
    Ok(())
}

/// Validate retained originals against the same-cut reserve, signed scheme and manifest.
pub(crate) fn validate_snapshot(world: &impl WorldReadOnly) -> Result<()> {
    check_transition(world, world.runtime_selection())?;
    for (scheme, installation) in world.kagemusha_spartan_programs().iter() {
        if *scheme != installation.scheme
            || world
                .kagemusha_wallet_ledger()
                .get(&artifacts::key(*scheme))
                .is_some()
        {
            return Err(Error::Binding);
        }
        original_bounds(installation)?;
        let key = super::storage::key(super::storage::REGISTRATION, *scheme, installation.asset);
        let bytes = world
            .kagemusha_wallet_ledger()
            .get(&key)
            .ok_or(Error::Unavailable)?;
        let registration: Registration = super::storage::decode(bytes, bytes.len())?;
        registration.require(scheme, &installation.asset)?;
        let manifest = AuthenticatedParameters::governed_manifest(
            &installation.manifest, &installation.signature, &installation.manifest_digest, &registration.scheme,
        ).map_err(native_error)?;
        manifest.verify_program_originals(&installation.programs).map_err(native_error)?;
    }
    Ok(())
}

fn finality(state: &StateTransaction<'_, '_>) -> Result<SumeragiCommitVerifierV1> {
    let genesis = state
        .block_by_height(NonZeroUsize::MIN)
        .map_err(|error| {
            state
                .world
                .attempt_error_to_instruction_error(error.map_rejection(|why| {
                    iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(
                        why.to_string().into(),
                    )
                }))
        })?
        .ok_or(Error::ArtifactsUnavailable)?;
    if iroha_data_model::NetworkId::from_genesis_hash(genesis.hash()) != *state.network_id() {
        return Err(Error::Binding);
    }
    // Ordinary transactions run after genesis. Recover the original committee from
    // the signed, State-hash-selected H1 body; the current epoch can have rotated.
    let reader_error = |error: iroha_data_model::sumeragi_finality::FinalityReadError| {
        match crate::sumeragi::finality::ProofError::from(error) {
            crate::sumeragi::finality::ProofError::Deferred(original) => {
                Error::Execution(state.world.attempt_error_to_instruction_error(
                    crate::execution_attempt::ExecutionAttemptError::Deferred(original),
                ))
            }
            _ => Error::Binding,
        }
    };
    let (epoch, _) = iroha_data_model::sumeragi_finality::authenticated_genesis(&genesis)
        .map_err(|error| reader_error(error.into()))?
        .into_parts();
    let roster = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect();
    let verifier = SumeragiFinalityVerifier::new(&genesis, state.chain_id().as_str(), roster)
        .map_err(reader_error)?;
    SumeragiCommitVerifierV1::new(&verifier).map_err(|_| Error::Binding)
}

#[derive(PartialEq, Eq)]
struct CacheKey {
    manifest: [u8; 32],
    programs: iroha_crypto::Hash,
    signature: [u8; 64],
}

type Slot = Arc<Mutex<Option<Arc<AuthenticatedParameters>>>>;
// Only two mounted program sets can occupy this process cache. One slot per exact
// original shares cold-import work. The index is released before any curve/SSA work.
type ProgramCache = Mutex<VecDeque<(CacheKey, Slot)>>;
static PROGRAM_CACHE: LazyLock<ProgramCache> = LazyLock::new(Mutex::default);

fn cache_slot(cache: &ProgramCache, key: CacheKey) -> Result<Slot> {
    let mut entries = cache.lock().map_err(|_| Error::ArtifactsUnavailable)?;
    if let Some(index) = entries.iter().position(|(k, _)| *k == key) {
        let entry = entries.remove(index).ok_or(Error::ArtifactsUnavailable)?;
        let slot = entry.1.clone();
        entries.push_back(entry);
        return Ok(slot);
    }
    if entries.len() >= 2 {
        let index = entries
            .iter()
            .position(|(_, slot)| {
                Arc::strong_count(slot) == 1
                    && slot.try_lock().is_ok_and(|value| {
                        value
                            .as_ref()
                            .is_none_or(|parameters| Arc::strong_count(parameters) == 1)
                    })
            })
            .ok_or(Error::ArtifactsUnavailable)?;
        entries.remove(index);
    }
    let slot = Arc::new(Mutex::new(None));
    entries.push_back((key, Arc::clone(&slot)));
    Ok(slot)
}

fn mounted(
    state: &StateTransaction<'_, '_>,
    registration: &Registration,
    installation: &Install,
) -> Result<Arc<AuthenticatedParameters>> {
    registration.require(&installation.scheme, &installation.asset)?;
    if state
        .world
        .kagemusha_wallet_ledger
        .get(&artifacts::key(installation.scheme))
        .is_some()
    {
        return Err(Error::Conflict);
    }
    original_bounds(installation)?;
    let pin = installation.manifest_digest;
    let manifest = AuthenticatedParameters::governed_manifest(
        &installation.manifest, &installation.signature, &pin, &registration.scheme,
    ).map_err(native_error)?;
    let (network, chain) = manifest.finality_identity();
    if chain != state.chain_id().as_str() || network != state.network_id().to_string() {
        return Err(Error::Binding);
    }
    let genesis_hash = crate::state::WorldStateSnapshot::block_hashes(state)
        .hash_at(0)
        .ok_or(Error::ArtifactsUnavailable)?;
    if iroha_data_model::NetworkId::from_genesis_hash(*genesis_hash) != *state.network_id() {
        return Err(Error::Binding);
    }
    let slot = cache_slot(
        &PROGRAM_CACHE,
        CacheKey {
            manifest: pin,
            programs: iroha_crypto::Hash::new(&installation.programs),
            signature: installation.signature,
        },
    )?;
    let mut retained = slot.lock().map_err(|_| Error::ArtifactsUnavailable)?;
    if let Some(parameters) = &*retained {
        return Ok(Arc::clone(parameters));
    }
    let params = AuthenticatedParameters::install_governed(
        &installation.programs,
        &installation.manifest,
        installation.signature,
        &pin,
        &registration.scheme,
        finality(state)?,
    )
    .map_err(native_error)?;
    if params.manifest_digest() != &installation.manifest_digest
        || params.id() != &registration.scheme.relation_id
    {
        return Err(Error::Binding);
    }
    let params = Arc::new(params);
    *retained = Some(Arc::clone(&params));
    Ok(params)
}

pub(crate) fn install(
    state: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    installation: &Install,
) -> Result<()> {
    if !selects(state.world.runtime_selection(), Install::WIRE_ID)? {
        return Err(Error::Binding);
    }
    let tx = WsvLedger::new(state, authority)?;
    let registration = tx.registration(&installation.scheme, &installation.asset)?;
    tx.require_verifier_manager(&registration)?;
    if let Some(previous) = tx
        .state
        .world
        .kagemusha_spartan_programs
        .get(&installation.scheme)
    {
        return if previous == installation {
            Ok(())
        } else {
            Err(Error::Conflict)
        };
    }
    if !redemption_available(&*tx.state.world) {
        return Err(Error::Lifecycle);
    }
    mounted(tx.state, &registration, installation)?;
    tx.state
        .world
        .kagemusha_spartan_programs
        .insert(installation.scheme, installation.clone());
    Ok(())
}

pub(crate) fn activate(
    state: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    request: &Activate,
) -> Result<()> {
    if request.wallet == [0; 32]
        || request.activation.is_empty()
        || request.activation.len() > Activate::MAX_ACTIVATION_BYTES
    {
        return Err(Error::Proof);
    }
    if !selects(state.world.runtime_selection(), Activate::WIRE_ID)?
        || !redemption_available(&*state.world)
    {
        return Err(Error::Binding);
    }
    let mut tx = WsvLedger::new(state, authority)?;
    let registration = tx.registration(&request.scheme, &request.asset)?;
    tx.reserve_suffix_proofs(request.activation.len(), 2)?;
    let installation = tx
        .state
        .world
        .kagemusha_spartan_programs
        .get(&request.scheme)
        .ok_or(Error::Unavailable)?;
    if installation.asset != request.asset
        || installation.manifest_digest != request.manifest_digest
    {
        return Err(Error::Binding);
    }
    let params = mounted(tx.state, &registration, installation)?;
    let verified = NativeEngine::default()
        .verify_activation_claim(
            &request.activation,
            &params,
            &request.scheme,
            &request.asset,
        )
        .map_err(native_error)?;
    let account = iroha_data_model::kagemusha::kagemusha_wallet_account_digest_v1(authority)?;
    if verified.wallet_id() != &request.wallet || verified.account_digest() != &account {
        return Err(Error::Binding);
    }
    ledger::activate_verified(
        &mut tx,
        request.scheme,
        request.asset,
        request.wallet,
        *verified.package_digest(),
    )
}

pub(crate) fn unload(
    state: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    request: &Unload,
) -> Result<KagemushaWalletPayoutRecordV1> {
    if request.claim.is_empty() || request.claim.len() > native::MAX_UNLOAD_CLAIM_BYTES {
        return Err(Error::Proof);
    }
    if !selects(state.world.runtime_selection(), Unload::WIRE_ID)? {
        return Err(Error::Binding);
    }
    let mut tx = WsvLedger::new(state, authority)?;
    let registration = tx.registration(&request.scheme, &request.asset)?;
    tx.reserve_suffix_proofs(request.claim.len(), 2)?;
    let installation = tx
        .state
        .world
        .kagemusha_spartan_programs
        .get(&request.scheme)
        .ok_or(Error::Unavailable)?;
    if installation.asset != request.asset
        || installation.manifest_digest != request.manifest_digest
    {
        return Err(Error::Binding);
    }
    let parameters = mounted(tx.state, &registration, installation)?;
    let verified = NativeEngine::default()
        .verify_unload_claim(&request.claim, &parameters, &request.scheme, &request.asset)
        .map_err(native_error)?;
    let destinations = verified.destinations();
    ledger::settle_unload(
        &mut tx,
        &registration,
        *verified.nullifier(),
        *verified.digest(),
        verified.amount(),
        &destinations.account,
        &destinations.charge,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simultaneous_callers_share_exact_originals_and_only_idle_cache_slots_can_be_evicted() {
        let cache = ProgramCache::default();
        let key = |seed: u8| CacheKey {
            manifest: [seed; 32],
            programs: iroha_crypto::Hash::new([seed]),
            signature: [seed; 64],
        };
        let first = cache_slot(&cache, key(1)).unwrap();
        let same = cache_slot(&cache, key(1)).unwrap();
        assert!(Arc::ptr_eq(&first, &same));
        let second = cache_slot(&cache, key(2)).unwrap();
        assert!(matches!(
            cache_slot(&cache, key(3)),
            Err(Error::ArtifactsUnavailable)
        ));
        drop(first);
        assert!(matches!(
            cache_slot(&cache, key(3)),
            Err(Error::ArtifactsUnavailable)
        ));
        drop(same);
        let third = cache_slot(&cache, key(3)).unwrap();
        assert!(!Arc::ptr_eq(&second, &third));
        assert_eq!(cache.lock().unwrap().len(), 2);
    }
}
