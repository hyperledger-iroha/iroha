//! Signed local custody for bounded pending AMX proof walks and the live approval floor.
//! The configured software route key authenticates local progress, never ledger execution.

use std::{
    fs::File,
    path::Path,
    sync::{Arc, Mutex, Weak},
};

use iroha::workflow::{Digest32, TransactionIdentityV1, domain_digest};
use iroha_crypto::{Hash, KeyPair, PublicKey, Signature};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::consensus::SumeragiRootScope,
    sumeragi_finality::{
        FinalityError, FinalityReadError, MAX_FINALITY_CHECKPOINT_BYTES,
        SumeragiFinalityCheckpoint, SumeragiFinalityProof, SumeragiFinalityVerifier,
        VerifiedSumeragiBlock,
    },
};
use iroha_fs::{PrivateDirectory, PublishMode};
use norito::{Decode, Encode};

const SIGN_DOMAIN: &[u8] = b"iroha:amx-relay:local-pending-finality:v1\0";
const NAME_DOMAIN: &[u8] = b"iroha:amx-relay:pending-finality-name:v1\0";
const DIGEST_DOMAIN: &[u8] = b"iroha:amx-relay:pending-finality-record:v1\0";
const LOCK: &str = "custody.lock";
const OBSERVATION: &str = "observation.lock";
const LIVE: &str = "live.finality";
const SUFFIX: &str = ".pending-finality";

/// One separately bounded native frame, plus its finite binding/signature envelope.
pub(super) const MAX_PENDING_RECORD_BYTES: usize = MAX_FINALITY_CHECKPOINT_BYTES + 64 * 1024;
/// Hard admission ceiling; configured admission may be smaller.
pub(super) const MAX_PENDING_RECORDS: usize =
    iroha_config::parameters::defaults::amx_relay::MAX_PENDING_FINALITY_WALKS;

#[derive(Debug, thiserror::Error)]
pub(super) enum CustodyError {
    #[error("AMX pending finality custody differs from its original binding")]
    Binding,
    #[error("AMX pending finality custody capacity exhausted")]
    Capacity,
    #[error("AMX pending finality custody is busy")]
    Busy,
    #[error("AMX pending finality original changed")]
    Conflict,
    #[error("AMX pending finality filesystem custody unavailable")]
    Io(#[source] std::io::Error),
    #[error("AMX pending finality local signing unavailable")]
    Signing(#[source] iroha_crypto::Error),
    #[error(transparent)]
    Codec(#[from] norito::Error),
    #[error(transparent)]
    Finality(#[from] FinalityReadError),
    #[error(transparent)]
    Execution(#[from] FinalityError),
}
type Result<T> = std::result::Result<T, CustodyError>;

enum Transition {
    Reservation,
    Identity,
    NativeProgress,
    RejectedRetry,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "irohad::amx_relay::PendingWalkKindV1")]
pub(super) enum PendingKind {
    Carrier,
    HandoffCarrier(Hash),
    Handoff(Hash),
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "irohad::amx_relay::PendingWalkKeyV1")]
pub(super) struct PendingKey {
    /// Digest of the independently reconstructed original source plan.
    pub source_plan: Digest32,
    /// Exact instruction selected by that original source.
    pub instruction: Hash,
    pub kind: PendingKind,
}

impl PendingKey {
    fn handoff_group(&self) -> Option<(Digest32, Hash, Hash)> {
        match self.kind {
            PendingKind::Carrier => None,
            PendingKind::HandoffCarrier(effect) | PendingKind::Handoff(effect) => {
                Some((self.source_plan, self.instruction, effect))
            }
        }
    }
}

#[derive(Default)]
struct Inventory {
    pending: Vec<PendingKey>,
}

impl Inventory {
    fn admit(&mut self, key: &PendingKey, maximum_ordinary: usize) -> Result<()> {
        if self.pending.contains(key) {
            return Ok(());
        }
        if cfg!(all(test, sumeragi_daemon_mutation = "HC490"))
            && key.handoff_group().is_some()
            && self.pending.len() >= maximum_ordinary
        {
            return Err(CustodyError::Capacity);
        }
        if let Some(group) = key.handoff_group() {
            if self.pending.iter().any(|existing| {
                existing
                    .handoff_group()
                    .is_some_and(|existing| existing != group)
            }) || self
                .pending
                .iter()
                .filter(|key| key.handoff_group().is_some())
                .count()
                >= 2
            {
                return Err(CustodyError::Capacity);
            }
        } else if self
            .pending
            .iter()
            .filter(|key| key.handoff_group().is_none())
            .count()
            >= maximum_ordinary
        {
            return Err(CustodyError::Capacity);
        }
        self.pending.push(key.clone());
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "irohad::amx_relay::PendingWalkProgressV1")]
pub(super) enum Progress {
    /// Durable quota admission before a carrier identity or native checkpoint exists.
    Reserved,
    /// Actual signed identity, retained before dispatch; it grants no outcome or locator.
    SignedCarrier {
        identity: TransactionIdentityV1,
    },
    Carrier {
        identity: TransactionIdentityV1,
        target_height: u64,
    },
    /// Every non-genesis block through `inspected_height` was checked for this exact effect.
    /// A newer generic finality prefix cannot supply this contiguous inspection claim.
    Handoff {
        inspected_height: u64,
        captured_tip: u64,
    },
    LiveApproval,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "irohad::amx_relay::PendingCustodySlotV1")]
pub(super) enum Slot {
    Pending(PendingKey),
    LiveApproval,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "irohad::amx_relay::PendingCustodyRouteV1")]
struct RouteBinding {
    network: NetworkId,
    chain: String,
    scope: SumeragiRootScope,
    instance: [u8; 32],
    authority: AccountId,
    signer: PublicKey,
}

#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "irohad::amx_relay::PendingCustodyBodyV1")]
struct Body {
    route: RouteBinding,
    slot: Slot,
    progress: Progress,
    /// Sole native canonical checkpoint frame, with its own independent decoder bounds.
    checkpoint: Option<Vec<u8>>,
}
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "irohad::amx_relay::PendingCustodyEnvelopeV1")]
struct Envelope {
    body: Vec<u8>,
    signature: Signature,
}

pub(super) struct LoadedProgress {
    pub digest: Digest32,
    pub progress: Progress,
    pub verifier: Option<SumeragiFinalityVerifier>,
    pub tip: Option<VerifiedSumeragiBlock>,
    pub proof: Option<SumeragiFinalityProof>,
}

/// Authenticated local CAS metadata; this is never an execution or finality verdict.
struct RecordedProgress {
    digest: Digest32,
    progress: Progress,
    height: Option<u64>,
}

/// Original route visit custody, retained through signing, dispatch, journal and final cleanup.
#[derive(Clone, Debug)]
pub(super) struct PendingObservationLease {
    file: Arc<File>,
    pub source_plan: Digest32,
    pub instruction: Hash,
    pub handoff_effect: Option<Hash>,
}

struct ActiveObservation {
    file: Weak<File>,
    source_plan: Digest32,
    instruction: Hash,
    handoff_effect: Option<Hash>,
}

/// One route owns N ordinary carriers, one two-slot handoff group, one live slot and one staging slot.
///
/// Each admitted pending slot reserves its entire worst-case record extent. New work cannot
/// consume space reserved for an existing unresolved walk. The route's workflow runner lease
/// remains held outside this owner; its original native quota lock serializes publication and
/// retirement across cooperating processes. No journal checksum enters checkpoint trust.
/// Signed software custody provides restart/crash safety, not hardware antirollback;
/// malicious filesystem rollback and loss/change of the original key are not repaired.
pub(super) struct PendingCustody {
    directory: PrivateDirectory,
    key: KeyPair,
    route: RouteBinding,
    original: SumeragiFinalityVerifier,
    maximum_pending: usize,
    active_observation: Mutex<Option<ActiveObservation>>,
    native_jobs: Arc<tokio::sync::Semaphore>,
    #[cfg(test)]
    test_root: Option<std::sync::Arc<tempfile::TempDir>>,
}

impl PendingCustody {
    pub(super) fn open(
        path: impl AsRef<Path>,
        key: KeyPair,
        authority: AccountId,
        original: SumeragiFinalityVerifier,
        maximum_pending: usize,
    ) -> Result<Self> {
        if !(1..=MAX_PENDING_RECORDS).contains(&maximum_pending) {
            return Err(CustodyError::Capacity);
        }
        maximum_pending
            .checked_add(4)
            .and_then(|slots| slots.checked_mul(MAX_PENDING_RECORD_BYTES))
            .ok_or(CustodyError::Capacity)?;
        let route = RouteBinding {
            network: original.initial_epoch().network_id,
            chain: original.chain_id().to_owned(),
            scope: original.root_scope(),
            instance: original.instance().0,
            authority,
            signer: key.public_key().clone(),
        };
        Ok(Self {
            directory: PrivateDirectory::open_or_create(path).map_err(CustodyError::Io)?,
            key,
            route,
            original: original.into_genesis_verifier(),
            maximum_pending,
            active_observation: Mutex::new(None),
            native_jobs: Arc::new(tokio::sync::Semaphore::new(1)),
            #[cfg(test)]
            test_root: None,
        })
    }

    /// Includes ordinary reservations, two handoff slots, the live floor and one staging file.
    #[cfg(test)]
    pub(super) fn maximum_payload_bytes(&self) -> usize {
        (self.maximum_pending + 4) * MAX_PENDING_RECORD_BYTES
    }

    pub(super) fn acquire_observation(
        &self,
        source_plan: Digest32,
        instruction: Hash,
        handoff_effect: Option<Hash>,
    ) -> Result<PendingObservationLease> {
        let file = self
            .directory
            .open_lock(OBSERVATION)
            .map_err(CustodyError::Io)?;
        match file.try_lock() {
            Ok(()) => (),
            Err(std::fs::TryLockError::WouldBlock) => return Err(CustodyError::Busy),
            Err(std::fs::TryLockError::Error(error)) => return Err(CustodyError::Io(error)),
        }
        if file.metadata().map_err(CustodyError::Io)?.len() != 0 {
            return Err(CustodyError::Binding);
        }
        let lease = PendingObservationLease {
            file: Arc::new(file),
            source_plan,
            instruction,
            handoff_effect,
        };
        self.check_observation(&lease)?;
        *self
            .active_observation
            .lock()
            .map_err(|_| CustodyError::Conflict)? = Some(ActiveObservation {
            file: Arc::downgrade(&lease.file),
            source_plan,
            instruction,
            handoff_effect,
        });
        Ok(lease)
    }

    /// Clone only the active original native lock for one bounded worker job. This owner
    /// keeps a weak reference, so it cannot extend a completed route visit by itself.
    pub(super) fn native_custody(
        &self,
    ) -> Result<(tokio::sync::OwnedSemaphorePermit, PendingObservationLease)> {
        let permit = self
            .native_jobs
            .clone()
            .try_acquire_owned()
            .map_err(|_| CustodyError::Capacity)?;
        let active = self
            .active_observation
            .lock()
            .map_err(|_| CustodyError::Conflict)?;
        let active = active.as_ref().ok_or(CustodyError::Binding)?;
        let lease = PendingObservationLease {
            file: active.file.upgrade().ok_or(CustodyError::Binding)?,
            source_plan: active.source_plan,
            instruction: active.instruction,
            handoff_effect: active.handoff_effect,
        };
        self.check_observation(&lease)?;
        Ok((permit, lease))
    }

    pub(super) fn check_observation(&self, lease: &PendingObservationLease) -> Result<()> {
        self.directory
            .require_same_lock(OBSERVATION, &lease.file)
            .map_err(CustodyError::Io)
    }

    #[cfg(test)]
    pub(super) fn for_test(
        original: &SumeragiFinalityVerifier,
        authority: AccountId,
    ) -> std::sync::Arc<Self> {
        Self::for_test_with_capacity(original, authority, 16)
    }

    #[cfg(test)]
    pub(super) fn for_test_with_capacity(
        original: &SumeragiFinalityVerifier,
        authority: AccountId,
        maximum_pending: usize,
    ) -> std::sync::Arc<Self> {
        let root = std::sync::Arc::new(tempfile::tempdir().unwrap());
        let mut owner = Self::open(
            root.path().join("pending"),
            KeyPair::from_seed(vec![71; 32], iroha_crypto::Algorithm::Ed25519),
            authority,
            original.clone(),
            maximum_pending,
        )
        .unwrap();
        owner.test_root = Some(root);
        std::sync::Arc::new(owner)
    }

    #[cfg(test)]
    pub(super) fn reopen_for_test(&self) -> std::sync::Arc<Self> {
        let mut owner = Self::open(
            self.directory.path(),
            self.key.clone(),
            self.route.authority.clone(),
            self.original.clone(),
            self.maximum_pending,
        )
        .unwrap();
        owner.test_root = self.test_root.clone();
        std::sync::Arc::new(owner)
    }

    fn name(&self, slot: &Slot) -> Result<String> {
        Ok(match slot {
            Slot::LiveApproval => LIVE.to_owned(),
            Slot::Pending(key) => {
                let bytes = norito::encode_canonical(&(self.route.clone(), key.clone()))?;
                format!(
                    "{}{SUFFIX}",
                    hex::encode(domain_digest(NAME_DOMAIN, &[&bytes]))
                )
            }
        })
    }

    fn import(
        &self,
        bytes: &[u8],
    ) -> Result<(
        SumeragiFinalityVerifier,
        VerifiedSumeragiBlock,
        SumeragiFinalityProof,
    )> {
        let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(bytes)?;
        let (mut verifier, tip) = SumeragiFinalityVerifier::from_trusted_checkpoint_with_tip(
            &checkpoint,
            &self.route.network,
            &self.route.chain,
        )?;
        if verifier.initial_epoch() != self.original.initial_epoch()
            || verifier.chain_id() != self.route.chain
            || verifier.instance() != self.original.instance()
            || verifier.root_scope() != self.route.scope
        {
            return Err(CustodyError::Binding);
        }
        verifier.retain_recent_decisions(3);
        Ok((verifier, tip, checkpoint.tip().clone()))
    }

    fn shape(slot: &Slot, progress: &Progress, height: Option<u64>) -> Result<()> {
        let valid = match (slot, progress, height) {
            (Slot::Pending(_), Progress::Reserved, None) => true,
            (
                Slot::Pending(PendingKey {
                    kind: PendingKind::Carrier | PendingKind::HandoffCarrier(_),
                    ..
                }),
                Progress::SignedCarrier { .. },
                None,
            ) => true,
            (Slot::LiveApproval, Progress::LiveApproval, Some(height)) => height >= 1,
            (
                Slot::Pending(PendingKey {
                    kind: PendingKind::Carrier | PendingKind::HandoffCarrier(_),
                    ..
                }),
                Progress::Carrier { target_height, .. },
                Some(height),
            ) => height >= 1 && *target_height >= 2 && height <= *target_height,
            (
                Slot::Pending(PendingKey {
                    kind: PendingKind::Handoff(_),
                    ..
                }),
                Progress::Handoff {
                    inspected_height,
                    captured_tip,
                },
                Some(height),
            ) => *captured_tip >= 2 && *inspected_height == height && height <= *captured_tip,
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(CustodyError::Binding)
        }
    }

    fn read_body(&self, name: &str) -> Result<Option<(Digest32, Body)>> {
        let Some(bytes) = self
            .directory
            .read_optional(name, MAX_PENDING_RECORD_BYTES)
            .map_err(CustodyError::Io)?
        else {
            return Ok(None);
        };
        let envelope: Envelope = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )?;
        let signed = domain_digest(SIGN_DOMAIN, &[&envelope.body]);
        envelope
            .signature
            .verify(self.key.public_key(), &signed)
            .map_err(|_| CustodyError::Binding)?;
        let body: Body = norito::decode_canonical_with_limits(
            &envelope.body,
            norito::canonical_decode_limits(envelope.body.len()),
        )?;
        if body.route != self.route || self.name(&body.slot)? != name {
            return Err(CustodyError::Binding);
        }
        Ok(Some((domain_digest(DIGEST_DOMAIN, &[&bytes]), body)))
    }

    /// Authenticated existence is scheduling information only; it grants no native outcome.
    pub(super) fn contains(&self, slot: &Slot) -> Result<bool> {
        match self.read_body(&self.name(slot)?)? {
            Some((_, body)) if body.slot != *slot => Err(CustodyError::Binding),
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    fn recorded(&self, slot: &Slot) -> Result<Option<RecordedProgress>> {
        let Some((digest, body)) = self.read_body(&self.name(slot)?)? else {
            return Ok(None);
        };
        if body.slot != *slot {
            return Err(CustodyError::Binding);
        }
        // CAS needs only the exact signed local record's bounded progress metadata.
        // Re-importing its native proof here nests a second full verification beneath
        // publication; actual restoration below still performs that verification.
        let height = body
            .checkpoint
            .as_deref()
            .map(SumeragiFinalityCheckpoint::decode_canonical)
            .transpose()?
            .map(|checkpoint| {
                if checkpoint.network_id() != self.route.network
                    || checkpoint.chain_id() != self.route.chain
                {
                    return Err(CustodyError::Binding);
                }
                Ok(checkpoint.height())
            })
            .transpose()?;
        Self::shape(slot, &body.progress, height)?;
        Ok(Some(RecordedProgress {
            digest,
            progress: body.progress,
            height,
        }))
    }

    pub(super) fn load(&self, slot: &Slot) -> Result<Option<LoadedProgress>> {
        let Some((digest, body)) = self.read_body(&self.name(slot)?)? else {
            return Ok(None);
        };
        if body.slot != *slot {
            return Err(CustodyError::Binding);
        }
        let imported = body
            .checkpoint
            .as_deref()
            .map(|bytes| self.import(bytes))
            .transpose()?;
        Self::shape(
            slot,
            &body.progress,
            imported.as_ref().map(|(_, tip, _)| tip.height()),
        )?;
        let (verifier, tip, proof) = match imported {
            Some((verifier, tip, proof)) => (Some(verifier), Some(tip), Some(proof)),
            None => (None, None, None),
        };
        Ok(Some(LoadedProgress {
            digest,
            progress: body.progress,
            verifier,
            tip,
            proof,
        }))
    }

    fn acquire(&self) -> Result<File> {
        let file = self.directory.open_lock(LOCK).map_err(CustodyError::Io)?;
        match file.try_lock() {
            Ok(()) => (),
            Err(std::fs::TryLockError::WouldBlock) => return Err(CustodyError::Busy),
            Err(std::fs::TryLockError::Error(error)) => return Err(CustodyError::Io(error)),
        }
        self.directory
            .require_same_lock(LOCK, &file)
            .map_err(CustodyError::Io)?;
        if file.metadata().map_err(CustodyError::Io)?.len() != 0 {
            return Err(CustodyError::Binding);
        }
        Ok(file)
    }

    /// Charge fixed worst-case reservations, not a transient sum of small current prefixes.
    /// Reconcile only the native writer's one bounded staging leaf under the original lock.
    fn inventory(&self) -> Result<Inventory> {
        let names = self
            .directory
            .entries(self.maximum_pending + 6)
            .map_err(CustodyError::Io)?;
        let mut required = Vec::new();
        let mut inventory = Inventory::default();
        for name in names {
            let name = name.to_str().ok_or(CustodyError::Binding)?;
            if name.starts_with(".iroha-fs-") {
                continue;
            }
            let is_pending = name.strip_suffix(SUFFIX).is_some_and(|id| {
                id.len() == 64
                    && id
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            });
            if !is_pending && name != LIVE && name != LOCK && name != OBSERVATION {
                return Err(CustodyError::Binding);
            }
            let file = self.directory.open_read(name).map_err(CustodyError::Io)?;
            let length = file.metadata().map_err(CustodyError::Io)?.len();
            if length > MAX_PENDING_RECORD_BYTES as u64 {
                return Err(CustodyError::Capacity);
            }
            if name == LOCK || name == OBSERVATION {
                if length != 0 {
                    return Err(CustodyError::Binding);
                }
            } else {
                // A filename cannot relabel an ordinary reservation as handoff quota.
                // Verify one signed binding at a time without importing native history.
                let (_, body) = self.read_body(name)?.ok_or(CustodyError::Conflict)?;
                if let Slot::Pending(key) = body.slot {
                    inventory.admit(&key, self.maximum_pending)?;
                }
            }
            required.push(name.to_owned());
        }
        let borrowed: Vec<_> = required.iter().map(String::as_str).collect();
        self.directory
            .reconcile_atomic_staging(
                &borrowed,
                self.maximum_pending + 5,
                1,
                MAX_PENDING_RECORD_BYTES,
            )
            .map_err(CustodyError::Io)?;
        Ok(inventory)
    }

    /// Sole publisher: export native-authenticated progress before the caller installs it.
    /// For a handoff, the caller must have checked every original block through this exact tip.
    pub(super) fn compare_and_publish(
        &self,
        slot: &Slot,
        expected: Option<&Digest32>,
        progress: Progress,
        verifier: &SumeragiFinalityVerifier,
        proof: &SumeragiFinalityProof,
    ) -> Result<Digest32> {
        if verifier.initial_epoch() != self.original.initial_epoch()
            || verifier.chain_id() != self.route.chain
            || verifier.instance() != self.original.instance()
            || verifier.root_scope() != self.route.scope
        {
            return Err(CustodyError::Binding);
        }
        Self::shape(slot, &progress, Some(proof.height()))?;
        let checkpoint = verifier.export_checkpoint(proof)?.encode_canonical()?;
        self.publish_body(
            slot,
            expected,
            progress,
            Some(checkpoint),
            Some(proof.height()),
            Transition::NativeProgress,
        )
    }

    /// Reserve the complete finite slot before signing or dispatching a new carrier.
    /// No identity, locator, checkpoint or execution verdict is invented by admission.
    pub(super) fn reserve(&self, key: &PendingKey) -> Result<Digest32> {
        if key.kind != PendingKind::Carrier {
            return Err(CustodyError::Binding);
        }
        self.reserve_member(key)
    }

    fn reserve_member(&self, key: &PendingKey) -> Result<Digest32> {
        let slot = Slot::Pending(key.clone());
        if let Some(current) = self.recorded(&slot)? {
            return Ok(current.digest);
        }
        self.publish_body(
            &slot,
            None,
            Progress::Reserved,
            None,
            None,
            Transition::Reservation,
        )
    }

    /// Admit this exact route visit before any signing. The outer observation lease
    /// serializes the group; only identity-free members created by an interrupted
    /// admission may be removed. Existing signed/proved obligations are never evicted.
    pub(super) fn reserve_group(&self, keys: &[PendingKey]) -> Result<()> {
        let exact_group = match keys {
            [
                PendingKey {
                    kind: PendingKind::Carrier,
                    ..
                },
            ] => true,
            [carrier, handoff] => matches!(
                (&carrier.kind, &handoff.kind),
                (PendingKind::HandoffCarrier(first), PendingKind::Handoff(second))
                    if first == second && carrier.source_plan == handoff.source_plan
                        && carrier.instruction == handoff.instruction
            ),
            _ => false,
        };
        if !exact_group {
            return Err(CustodyError::Binding);
        }
        let lock = self.acquire()?;
        let mut inventory = self.inventory()?;
        let loaded: Vec<_> = keys
            .iter()
            .map(|key| self.recorded(&Slot::Pending(key.clone())))
            .collect::<Result<_>>()?;
        let missing = loaded.iter().filter(|entry| entry.is_none()).count();
        if keys
            .iter()
            .any(|key| inventory.admit(key, self.maximum_pending).is_err())
        {
            // A crash may have left half of this exact admission. It has no identity,
            // no dispatch authority and no native progress; don't let it consume the
            // very capacity its own complete admission needs on every future visit.
            if missing > 0 {
                for (key, entry) in keys.iter().zip(&loaded) {
                    if entry
                        .as_ref()
                        .is_some_and(|entry| entry.progress == Progress::Reserved)
                    {
                        self.directory
                            .remove_private(self.name(&Slot::Pending(key.clone()))?)
                            .map_err(CustodyError::Io)?;
                    }
                }
            }
            self.directory
                .require_same_lock(LOCK, &lock)
                .map_err(CustodyError::Io)?;
            return Err(CustodyError::Capacity);
        }
        drop(lock);
        let mut created = Vec::new();
        for (key, previous) in keys.iter().zip(loaded) {
            if previous.is_some() {
                continue;
            }
            let admission = if key.kind == PendingKind::Carrier {
                self.reserve(key)
            } else {
                self.reserve_member(key)
            };
            match admission {
                Ok(digest) => created.push((key, digest)),
                Err(original) => {
                    // An uncertain publication can leave only a Reserved record here.
                    // Reconcile the exact current key too, preserving all other originals.
                    for key in created
                        .iter()
                        .map(|(key, _)| *key)
                        .chain(std::iter::once(key))
                    {
                        let lock = self.acquire()?;
                        if self
                            .recorded(&Slot::Pending(key.clone()))?
                            .is_some_and(|entry| entry.progress == Progress::Reserved)
                        {
                            self.directory
                                .remove_private(self.name(&Slot::Pending(key.clone()))?)
                                .map_err(CustodyError::Io)?;
                        }
                        self.directory
                            .require_same_lock(LOCK, &lock)
                            .map_err(CustodyError::Io)?;
                    }
                    return Err(original);
                }
            }
        }
        Ok(())
    }

    /// Bind actual journaled signed bytes before dispatch. A different identity requires
    /// the explicit native-rejection gate; this method cannot reset an unresolved attempt.
    pub(super) fn bind_signed_identity(
        &self,
        key: &PendingKey,
        expected: &Digest32,
        identity: TransactionIdentityV1,
    ) -> Result<Digest32> {
        let slot = Slot::Pending(key.clone());
        let current = self.recorded(&slot)?.ok_or(CustodyError::Binding)?;
        if current.digest != *expected {
            return Err(CustodyError::Conflict);
        }
        if matches!(&current.progress,
            Progress::SignedCarrier { identity: old } | Progress::Carrier { identity: old, .. } if old == &identity)
        {
            return Ok(current.digest);
        }
        self.publish_body(
            &slot,
            Some(expected),
            Progress::SignedCarrier { identity },
            None,
            None,
            Transition::Identity,
        )
    }

    /// Reuse the original reservation after the engine durably selected its explicit retry.
    /// The retained native tip must independently prove rejection of the exact old input.
    /// This gate is necessary, not sufficient: original source, fees and current approval
    /// remain the route/engine's signing authority. No checksummed label authorizes retry.
    pub(super) fn rebind_after_durable_rejected_attempt(
        &self,
        key: &PendingKey,
        expected: &Digest32,
        identity: TransactionIdentityV1,
        evidence: &iroha_data_model::query::CommittedTransaction,
    ) -> Result<Digest32> {
        use iroha_data_model::transaction::{Executable, TransactionEntrypoint};
        let slot = Slot::Pending(key.clone());
        let current = self.load(&slot)?.ok_or(CustodyError::Binding)?;
        if current.digest != *expected {
            return Err(CustodyError::Conflict);
        }
        let Progress::Carrier {
            identity: old,
            target_height,
        } = current.progress
        else {
            return Err(CustodyError::Binding);
        };
        let tip = current.tip.ok_or(CustodyError::Binding)?;
        if tip.height() != target_height || evidence.result().is_ok() || old == identity {
            return Err(CustodyError::Binding);
        }
        tip.verify_transaction_execution(&self.route.network, evidence)?;
        let TransactionEntrypoint::External(transaction) = evidence.entrypoint() else {
            return Err(CustodyError::Binding);
        };
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(CustodyError::Binding);
        };
        let [instruction] = instructions.as_ref() else {
            return Err(CustodyError::Binding);
        };
        if *Hash::from(transaction.hash()).as_ref() != old.signed_transaction_hash
            || *Hash::from(transaction.hash_as_entrypoint()).as_ref() != old.entrypoint_hash
            || transaction.authority() != &self.route.authority
            || Hash::new(norito::encode_canonical(instruction)?) != key.instruction
        {
            return Err(CustodyError::Binding);
        }
        self.publish_body(
            &slot,
            Some(expected),
            Progress::SignedCarrier { identity },
            None,
            None,
            Transition::RejectedRetry,
        )
    }

    fn publish_body(
        &self,
        slot: &Slot,
        expected: Option<&Digest32>,
        progress: Progress,
        checkpoint: Option<Vec<u8>>,
        height: Option<u64>,
        transition: Transition,
    ) -> Result<Digest32> {
        Self::shape(slot, &progress, height)?;
        let body = Body {
            route: self.route.clone(),
            slot: slot.clone(),
            progress: progress.clone(),
            checkpoint,
        };
        if norito::canonical_frame_len(&body)? > MAX_PENDING_RECORD_BYTES {
            return Err(CustodyError::Capacity);
        }
        let body = norito::encode_canonical(&body)?;
        let signature = Signature::try_new(
            self.key.private_key(),
            &domain_digest(SIGN_DOMAIN, &[&body]),
        )
        .map_err(CustodyError::Signing)?;
        let envelope = Envelope { body, signature };
        if norito::canonical_frame_len(&envelope)? > MAX_PENDING_RECORD_BYTES {
            return Err(CustodyError::Capacity);
        }
        let bytes = norito::encode_canonical(&envelope)?;
        let lock = self.acquire()?;
        let mut inventory = self.inventory()?;
        let current = self.recorded(slot)?;
        if current.as_ref().map(|value| &value.digest) != expected {
            return Err(CustodyError::Conflict);
        }
        if let Slot::Pending(key) = slot {
            inventory.admit(key, self.maximum_pending)?;
        }
        let permitted = match (
            current.as_ref().map(|value| &value.progress),
            &progress,
            transition,
        ) {
            (None, Progress::Reserved, Transition::Reservation)
            | (Some(Progress::Reserved), Progress::SignedCarrier { .. }, Transition::Identity)
            | (Some(Progress::Reserved), Progress::Handoff { .. }, Transition::NativeProgress)
            | (
                None | Some(Progress::LiveApproval),
                Progress::LiveApproval,
                Transition::NativeProgress,
            )
            | (
                Some(Progress::Handoff { .. }),
                Progress::Handoff { .. },
                Transition::NativeProgress,
            )
            | (
                Some(Progress::Carrier { .. }),
                Progress::SignedCarrier { .. },
                Transition::RejectedRetry,
            ) => true,
            (
                Some(
                    Progress::SignedCarrier { identity: old }
                    | Progress::Carrier { identity: old, .. },
                ),
                Progress::Carrier { identity: new, .. },
                Transition::NativeProgress,
            ) => old == new,
            _ => false,
        };
        if !permitted {
            return Err(CustodyError::Binding);
        }
        if let Some(current) = &current {
            let may_reset = matches!((&current.progress, &progress),
                (Progress::Carrier { target_height: old, .. }, Progress::Carrier { target_height: new, .. }) if old != new);
            if !may_reset
                && height.is_some_and(|height| current.height.is_some_and(|tip| height < tip))
            {
                return Err(CustodyError::Binding);
            }
            if let (
                Progress::Handoff {
                    inspected_height: inspected,
                    captured_tip: old,
                },
                Progress::Handoff {
                    captured_tip: new, ..
                },
            ) = (&current.progress, &progress)
                && (new < old || (new != old && inspected < old))
            {
                // A growing server tip cannot move the finish line of an unfinished
                // contiguous scan and postpone fair recovery of its original obligation.
                return Err(CustodyError::Binding);
            }
        }
        self.directory
            .require_same_lock(LOCK, &lock)
            .map_err(CustodyError::Io)?;
        self.directory
            .write_atomic(
                self.name(slot)?,
                &bytes,
                if current.is_some() {
                    PublishMode::Replace
                } else {
                    PublishMode::CreateNew
                },
            )
            .map_err(CustodyError::Io)?;
        self.directory
            .require_same_lock(LOCK, &lock)
            .map_err(CustodyError::Io)?;
        Ok(domain_digest(DIGEST_DOMAIN, &[&bytes]))
    }

    /// Route completion boundary only, after its original engine result is durable and the
    /// final exact outcome/effect has been independently checked. Never call from outcome().
    /// A rejected carrier alone does not complete its business obligation or authorize retry.
    pub(super) fn retire_after_durable_route_result(
        &self,
        key: &PendingKey,
        expected: &Digest32,
    ) -> Result<()> {
        let slot = Slot::Pending(key.clone());
        let lock = self.acquire()?;
        self.inventory()?;
        let current = self.recorded(&slot)?.ok_or(CustodyError::Conflict)?;
        if current.digest != *expected {
            return Err(CustodyError::Conflict);
        }
        self.directory
            .require_same_lock(LOCK, &lock)
            .map_err(CustodyError::Io)?;
        self.directory
            .remove_private(self.name(&slot)?)
            .map_err(CustodyError::Io)?;
        self.directory
            .require_same_lock(LOCK, &lock)
            .map_err(CustodyError::Io)
    }
}

#[cfg(test)]
mod tests;
