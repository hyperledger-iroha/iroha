//! Private durable replay custody under the original configured safety-record owner.
//!
//! This ledger grants no reader or signing authority. It preserves consumed originals and
//! actual-clock high-water across restarts; a failed or ambiguous write closes the owner.

use std::{alloc::Layout, fs::File};

use iroha_allocation::{AllocationBudget, AllocationCharge, AllocationReservation};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    private_transaction_counters::{
        CounterCutV1, MAX_PRIVATE_COUNTER_CLOCK_SKEW_MS_V1, MAX_PRIVATE_COUNTER_LIFETIME_MS_V1,
        PrivateCountersErrorV1, SignedPrivateCountersRequestV1,
    },
};
use iroha_fs::{FileIdentity, OwnerDirectory, PrivateDirectory, PublishMode};
use iroha_sumeragi::types::Hash32;

use super::super::records::{FileRecordStore, PrivateCounterFirstInstallation};

pub(super) const MAX_REPLAY_ENTRIES: usize = 1024;
const LEDGER_FILE: &str = "ledger.norito";
const LOCK_FILE: &str = "owner.lock";
const MAX_LEDGER_BYTES: usize = 512 * 1024;
const MAX_LEDGER_DECODE_BYTES: usize = 2 * 1024 * 1024;
// Covers bounded original/read/encode buffers, decoded flat ledger and native codec controls.
const REPLAY_CODEC_SCRATCH_BYTES: usize = 4 * 1024 * 1024;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::sumeragi::private_counters::ReplayBindingV1")]
pub(super) struct ReplayBinding {
    pub(super) instance: [u8; 32],
    pub(super) network_id: NetworkId,
    pub(super) scope: SumeragiRootScope,
    pub(super) installed_policy_authority: Hash,
    pub(super) installed_key: Hash,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::sumeragi::private_counters::ConsumedNonceV1")]
struct ConsumedNonce {
    authority: Hash,
    nonce: [u8; 32],
    request_hash: Hash,
    cut: CounterCutV1,
    creation_time_ms: u64,
    time_to_live_ms: u64,
    expires_at_ms: u64,
}

#[derive(Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::sumeragi::private_counters::ReplayLedgerV1")]
struct ReplayLedger {
    version: u16,
    binding: ReplayBinding,
    first_install_time_ms: u64,
    high_water_ms: u64,
    consumed: Vec<ConsumedNonce>,
}

impl ReplayLedger {
    fn validate(&self, expected: &ReplayBinding) -> Result<(), PrivateCountersErrorV1> {
        if self.version != 1
            || self.binding != *expected
            || !matches!(self.binding.scope, SumeragiRootScope::Dataspace { .. })
            || self.first_install_time_ms == 0
            || self.high_water_ms < self.first_install_time_ms
            || self.consumed.len() > MAX_REPLAY_ENTRIES
        {
            return Err(PrivateCountersErrorV1::Unavailable);
        }
        for (index, entry) in self.consumed.iter().enumerate() {
            entry
                .cut
                .validate()
                .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
            if entry.nonce == [0; 32]
                || entry.creation_time_ms < self.first_install_time_ms
                || !(1..=MAX_PRIVATE_COUNTER_LIFETIME_MS_V1).contains(&entry.time_to_live_ms)
                || entry.creation_time_ms
                    > self
                        .high_water_ms
                        .saturating_add(MAX_PRIVATE_COUNTER_CLOCK_SKEW_MS_V1)
                || expiry(entry.creation_time_ms, entry.time_to_live_ms)? != entry.expires_at_ms
                || self.consumed[..index]
                    .iter()
                    .any(|prior| prior.authority == entry.authority && prior.nonce == entry.nonce)
            {
                return Err(PrivateCountersErrorV1::Unavailable);
            }
        }
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>, PrivateCountersErrorV1> {
        self.validate(&self.binding)?;
        norito::core::to_bytes_bounded(self, MAX_LEDGER_BYTES)
            .map_err(|_| PrivateCountersErrorV1::Unavailable)
    }

    fn decode(bytes: &[u8], binding: &ReplayBinding) -> Result<Self, PrivateCountersErrorV1> {
        if bytes.is_empty() || bytes.len() > MAX_LEDGER_BYTES {
            return Err(PrivateCountersErrorV1::Unavailable);
        }
        let decoded: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(
                MAX_REPLAY_ENTRIES,
                MAX_LEDGER_BYTES,
                16 * MAX_REPLAY_ENTRIES,
                MAX_LEDGER_DECODE_BYTES,
                16,
            ),
        )
        .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        decoded.validate(binding)?;
        Ok(decoded)
    }
}

pub(super) struct ReplayOwner {
    ledger: ReplayLedger,
    directory: PrivateDirectory,
    lock: File,
    lock_identity: FileIdentity,
    original_hash: Hash,
    poisoned: bool,
    // The fixed payload drops before its original allocation credit and codec allowance.
    _ledger_charge: AllocationCharge,
    _codec_scratch: AllocationReservation,
    #[cfg(test)]
    failure: Option<PersistenceFailure>,
}

impl ReplayOwner {
    pub(super) fn open(
        records: &FileRecordStore,
        binding: ReplayBinding,
        first_installation: Option<PrivateCounterFirstInstallation>,
        now_ms: u64,
        budget: &AllocationBudget,
    ) -> Result<Self, PrivateCountersErrorV1> {
        if now_ms == 0 || !matches!(binding.scope, SumeragiRootScope::Dataspace { .. }) {
            return Err(PrivateCountersErrorV1::Freshness);
        }
        let layout = Layout::array::<ConsumedNonce>(MAX_REPLAY_ENTRIES)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let mut reservation = budget
            .try_reserve(layout)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let ledger_charge = reservation
            .try_split(layout)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let codec_scratch = budget
            .try_reserve_bytes(REPLAY_CODEC_SCRATCH_BYTES)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let parent = OwnerDirectory::open(records.records_dir())
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        let name = format!(
            "private-counters-v1-{}-{}",
            hex::encode(binding.instance),
            hex::encode(binding.installed_key.as_ref()),
        );
        let directory = match PrivateDirectory::open(parent.path().join(&name)) {
            Ok(directory) => directory,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Missing custody is never an empty replay window on an existing instance.
                let permit = first_installation.ok_or(PrivateCountersErrorV1::Unavailable)?;
                if !permit.authorizes(
                    parent.path(),
                    &Hash32(binding.instance),
                    binding.installed_key,
                ) {
                    return Err(PrivateCountersErrorV1::Unavailable);
                }
                let initial = ReplayLedger {
                    version: 1,
                    binding,
                    first_install_time_ms: now_ms,
                    high_water_ms: now_ms,
                    consumed: Vec::new(),
                }
                .encode()?;
                parent
                    .publish_private_child(&name, &[(LOCK_FILE, &[]), (LEDGER_FILE, &initial)])
                    .map_err(|_| PrivateCountersErrorV1::Unavailable)?
            }
            Err(_) => return Err(PrivateCountersErrorV1::Unavailable),
        };
        let lock = directory
            .open_existing_lock(LOCK_FILE)
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        lock.try_lock()
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        let lock_identity =
            FileIdentity::of(&lock).map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        if lock
            .metadata()
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?
            .len()
            != 0
        {
            return Err(PrivateCountersErrorV1::Unavailable);
        }
        // Only known staging names may be discarded, under this original held operation lock.
        directory
            .reconcile_atomic_staging(&[LOCK_FILE, LEDGER_FILE], 16, 16 * MAX_LEDGER_BYTES)
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        let bytes = directory
            .read(LEDGER_FILE, MAX_LEDGER_BYTES)
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        let mut ledger = ReplayLedger::decode(&bytes, &binding)?;
        // The decoded flat payload is moved into one prepaid fixed-capacity original owner.
        let mut consumed = Vec::new();
        consumed
            .try_reserve_exact(MAX_REPLAY_ENTRIES)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        consumed.extend(ledger.consumed.drain(..));
        ledger.consumed = consumed;
        let original_hash = Hash::new(&bytes);
        drop(bytes);
        let mut owner = Self {
            ledger,
            directory,
            lock,
            lock_identity,
            original_hash,
            poisoned: false,
            _ledger_charge: ledger_charge,
            _codec_scratch: codec_scratch,
            #[cfg(test)]
            failure: None,
        };
        owner.observe_clock(now_ms)?;
        Ok(owner)
    }

    fn verify_original(&self) -> Result<(), PrivateCountersErrorV1> {
        if self.poisoned {
            return Err(PrivateCountersErrorV1::Unavailable);
        }
        self.directory
            .revalidate()
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        let named = self
            .directory
            .open_read(LOCK_FILE)
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        if FileIdentity::of(&named).map_err(|_| PrivateCountersErrorV1::Unavailable)?
            != self.lock_identity
            || FileIdentity::of(&self.lock).map_err(|_| PrivateCountersErrorV1::Unavailable)?
                != self.lock_identity
            || named
                .metadata()
                .map_err(|_| PrivateCountersErrorV1::Unavailable)?
                .len()
                != 0
        {
            return Err(PrivateCountersErrorV1::Unavailable);
        }
        let original = self
            .directory
            .read(LEDGER_FILE, MAX_LEDGER_BYTES)
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        if Hash::new(&original) != self.original_hash {
            return Err(PrivateCountersErrorV1::Unavailable);
        }
        Ok(())
    }

    fn persist(&mut self) -> Result<(), PrivateCountersErrorV1> {
        let result = (|| {
            self.verify_original()?;
            let bytes = self.ledger.encode()?;
            #[cfg(test)]
            if self.failure == Some(PersistenceFailure::BeforePublication) {
                self.failure = None;
                return Err(PrivateCountersErrorV1::Unavailable);
            }
            self.directory
                .write_atomic(LEDGER_FILE, &bytes, PublishMode::Replace)
                .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
            #[cfg(test)]
            if self.failure == Some(PersistenceFailure::AfterPublication) {
                self.failure = None;
                return Err(PrivateCountersErrorV1::Unavailable);
            }
            // A successful sync must still name this exact newly published original.
            let published = self
                .directory
                .read(LEDGER_FILE, MAX_LEDGER_BYTES)
                .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
            if published.as_slice() != bytes.as_slice() {
                return Err(PrivateCountersErrorV1::Unavailable);
            }
            self.original_hash = Hash::new(&bytes);
            self.verify_original()
        })();
        if result.is_err() {
            // An ambiguous publication can contain the consumed nonce. This process never retries.
            self.poisoned = true;
        }
        result
    }

    pub(super) fn observe_clock(&mut self, now_ms: u64) -> Result<(), PrivateCountersErrorV1> {
        self.verify_original()?;
        if now_ms == 0 || now_ms < self.ledger.high_water_ms {
            return Err(PrivateCountersErrorV1::Freshness);
        }
        if now_ms != self.ledger.high_water_ms {
            self.ledger.high_water_ms = now_ms;
            self.ledger
                .consumed
                .retain(|entry| entry.expires_at_ms >= now_ms);
            // Persist before pruned capacity or a newer clock can authorize another original.
            self.persist()?;
        }
        Ok(())
    }

    pub(super) fn check_request(
        &self,
        request: &SignedPrivateCountersRequestV1,
        now_ms: u64,
    ) -> Result<(), PrivateCountersErrorV1> {
        self.verify_original()?;
        let payload = &request.payload;
        if payload.network_id != self.ledger.binding.network_id
            || payload.scope != self.ledger.binding.scope
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        if now_ms < self.ledger.high_water_ms
            || payload.creation_time_ms < self.ledger.first_install_time_ms
            || payload.creation_time_ms
                > now_ms.saturating_add(MAX_PRIVATE_COUNTER_CLOCK_SKEW_MS_V1)
            || now_ms > expiry(payload.creation_time_ms, payload.time_to_live_ms.get())?
        {
            return Err(PrivateCountersErrorV1::Freshness);
        }
        let authority = Hash::from(HashOf::new(&payload.authority));
        if self
            .ledger
            .consumed
            .iter()
            .any(|entry| entry.authority == authority && entry.nonce == payload.nonce)
        {
            return Err(PrivateCountersErrorV1::Replay);
        }
        if self.ledger.consumed.len() == MAX_REPLAY_ENTRIES {
            return Err(PrivateCountersErrorV1::Bounds);
        }
        Ok(())
    }

    pub(super) fn consume(
        &mut self,
        request: &SignedPrivateCountersRequestV1,
    ) -> Result<(), PrivateCountersErrorV1> {
        self.check_request(request, self.ledger.high_water_ms)?;
        self.ledger.consumed.push(ConsumedNonce {
            authority: Hash::from(HashOf::new(&request.payload.authority)),
            nonce: request.payload.nonce,
            request_hash: request.original_hash()?,
            cut: request.payload.cut.clone(),
            creation_time_ms: request.payload.creation_time_ms,
            time_to_live_ms: request.payload.time_to_live_ms.get(),
            expires_at_ms: expiry(
                request.payload.creation_time_ms,
                request.payload.time_to_live_ms.get(),
            )?,
        });
        self.persist()
    }

    #[cfg(test)]
    pub(super) fn consumed_len(&self) -> usize {
        self.ledger.consumed.len()
    }
}

fn expiry(creation_time_ms: u64, ttl_ms: u64) -> Result<u64, PrivateCountersErrorV1> {
    if creation_time_ms == 0 || !(1..=MAX_PRIVATE_COUNTER_LIFETIME_MS_V1).contains(&ttl_ms) {
        return Err(PrivateCountersErrorV1::Freshness);
    }
    creation_time_ms
        .checked_add(ttl_ms)
        .and_then(|time| time.checked_add(MAX_PRIVATE_COUNTER_CLOCK_SKEW_MS_V1))
        .ok_or(PrivateCountersErrorV1::Freshness)
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum PersistenceFailure {
    BeforePublication,
    AfterPublication,
}

#[cfg(test)]
#[path = "replay_owner/tests.rs"]
mod tests;
