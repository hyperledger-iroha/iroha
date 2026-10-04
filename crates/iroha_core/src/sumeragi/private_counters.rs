//! Installed private native computation signer, separate from consensus votes.
//!
//! Only original reader-signed requests enter this owner. It computes the claim itself;
//! no caller-provided hash, projection or counts can be signed. The private key stays in
//! the same installed signer owner shared with the actual running consensus driver.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use iroha_allocation::{AllocationBudget, AllocationReservation};
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    account::AccountId,
    private_transaction_counters::{
        CounterMemberAttestationV1, CounterMemberBodyV1, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1,
        PRIVATE_COUNTER_MEMBER_DOMAIN_V1, PrivateCountersErrorV1, PrivateCountersResponseV1,
        SignedPrivateCountersRequestV1,
    },
};
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{crypto::Signer, types::Hash32};
use parking_lot::Mutex;

use super::{
    crypto::KeyPairSigner,
    records::{FileRecordStore, FreshKeyAssertion},
};

#[path = "private_counters/replay_owner.rs"]
mod replay_owner;
use crate::{
    query::archive_finality::CertifiedArchiveView,
    state::{State, StateReadOnly, is_stable_state_view_generation},
};
use replay_owner::{ReplayBinding, ReplayOwner};

const MAX_COUNTER_OPERATION_BYTES: usize = 64 * 1024 * 1024;
const MAX_COUNTER_TIP_HEIGHT: u64 = 10_000;
const REQUEST_AND_MEMBER_ALLOCATION_BYTES: usize = 4 * 1024 * 1024 + 512 * 1024;

/// Original canonical private-counter bytes with their actual native State-pool funding.
///
/// The complete prepaid operation allowance remains owned through transport destruction.
/// Reading these bytes grants no authority to construct or sign a counters claim. This owner
/// deliberately has no cloning or allocation-detaching conversion.
#[must_use = "retain the original response owner until its bytes are destroyed"]
pub struct PrivateCounterOriginalV1 {
    // Rust drops fields in declaration order: destroy the original before its funding.
    original: Vec<u8>,
    _operation_allocation: AllocationReservation,
}

impl AsRef<[u8]> for PrivateCounterOriginalV1 {
    fn as_ref(&self) -> &[u8] {
        &self.original
    }
}

impl std::ops::Deref for PrivateCounterOriginalV1 {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl std::fmt::Debug for PrivateCounterOriginalV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivateCounterOriginalV1")
            .field("length", &self.original.len())
            .finish_non_exhaustive()
    }
}

/// Runtime-only signer custody. Its sole callable operation performs native computation.
pub(super) struct PrivateCounterService {
    state: Arc<State>,
    policy_authority: AccountId,
    identity: PeerId,
    signer: Arc<KeyPairSigner>,
    replay: Mutex<ReplayOwner>,
}

impl PrivateCounterService {
    pub(super) fn new(
        state: Arc<State>,
        policy_authority: AccountId,
        identity: PeerId,
        signer: Arc<KeyPairSigner>,
        instance: Hash32,
        records: &FileRecordStore,
        fresh_key: Option<&FreshKeyAssertion>,
    ) -> Result<Self, PrivateCountersErrorV1> {
        if super::crypto::core_key(identity.public_key())
            .map_err(|_| PrivateCountersErrorV1::Signature)?
            != *signer.public_key()
        {
            return Err(PrivateCountersErrorV1::Signature);
        }
        let now_ms = native_time_ms()?;
        let view = state
            .try_view_once()
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        let scope = super::lanes::routing::committed_root_scope(view.world())
            .ok_or(PrivateCountersErrorV1::Context)?;
        if !matches!(
            scope,
            iroha_data_model::block::consensus::SumeragiRootScope::Dataspace { .. }
        ) || scope
            .instance_id(
                &super::crypto::BlsCrypto::new(),
                *view.network_id(),
                &view.chain_id().to_string(),
            )
            .map_err(|_| PrivateCountersErrorV1::Context)?
            != instance
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        let binding = ReplayBinding {
            instance: instance.0,
            network_id: *view.network_id(),
            scope,
            installed_policy_authority: Hash::from(HashOf::new(&policy_authority)),
            installed_key: Hash::new(signer.public_key().as_bytes()),
        };
        drop(view);
        let first_installation = records
            .private_counter_first_installation(&instance, signer.public_key(), fresh_key)
            .map_err(|_| PrivateCountersErrorV1::Unavailable)?;
        let replay = ReplayOwner::open(
            records,
            binding,
            first_installation,
            now_ms,
            &state.ivm_execution_budget(),
        )?;
        Ok(Self {
            state,
            policy_authority,
            identity,
            signer,
            replay: Mutex::new(replay),
        })
    }

    pub(super) fn compute_original(
        &self,
        original: &[u8],
        still_ready: impl Fn() -> bool,
    ) -> Result<PrivateCounterOriginalV1, PrivateCountersErrorV1> {
        if original.is_empty() || original.len() > MAX_PRIVATE_COUNTER_FRAME_BYTES_V1 {
            return Err(PrivateCountersErrorV1::Bounds);
        }
        // Physical try-lock bounds concurrency; waiting requests cannot accumulate behind
        // expensive original history work. Live nonces are never evicted to admit a request.
        let mut replay = self
            .replay
            .try_lock()
            .ok_or(PrivateCountersErrorV1::Unavailable)?;
        if !still_ready() {
            return Err(PrivateCountersErrorV1::Unavailable);
        }
        let now_ms = native_time_ms()?;
        let global_budget = self.state.ivm_execution_budget();
        let _operation_charge = global_budget
            .try_reserve_bytes(MAX_COUNTER_OPERATION_BYTES)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        // One operation pool is retained across decode, original World capture, indexed
        // computation, member signing and original response encoding. Its full maximum is
        // funded by the native State pool before any new operation-owned allocation.
        let budget = AllocationBudget::new(MAX_COUNTER_OPERATION_BYTES);
        // The request graph and member envelope stay funded until their final destruction.
        let _request_and_member_allocation = budget
            .try_reserve_bytes(REQUEST_AND_MEMBER_ALLOCATION_BYTES)
            .map_err(|_| PrivateCountersErrorV1::Bounds)?;
        let request = SignedPrivateCountersRequestV1::decode_bounded_canonical(original)?;
        request.verify_signature()?;
        replay.observe_clock(now_ms)?;
        replay.check_request(&request, now_ms)?;

        let generation = self.state.state_view_generation();
        if generation % 2 != 0 || !still_ready() {
            return Err(PrivateCountersErrorV1::Context);
        }
        let computed =
            crate::query::private_transaction_counters::compute_private_transaction_counters_v1(
                &self.state,
                &self.policy_authority,
                &request,
                now_ms,
                &budget,
            )?;
        let (claim, _computed_allocation) = computed.into_parts();
        if claim.request_hash != request.original_hash()?
            || claim.cut != request.payload.cut
            || !is_stable_state_view_generation(generation, self.state.state_view_generation())
            || !still_ready()
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        // Independently read the original certified current committee after computation.
        // Another cut cannot supply member-index custody even if it contains the same key.
        let view = self
            .state
            .try_view_once()
            .map_err(|_| PrivateCountersErrorV1::Context)?;
        let height = u64::try_from(view.height()).map_err(|_| PrivateCountersErrorV1::Bounds)?;
        if height > MAX_COUNTER_TIP_HEIGHT || height != claim.cut.height {
            return Err(PrivateCountersErrorV1::Context);
        }
        let archive = CertifiedArchiveView::new_with_budget(&view, view.kura(), &budget)
            .map_err(|_| PrivateCountersErrorV1::Context)?;
        if archive.tip_height() != height {
            return Err(PrivateCountersErrorV1::Context);
        }
        let certified = archive
            .block(height)
            .map_err(|_| PrivateCountersErrorV1::Context)?;
        let tip = certified.committed();
        let epoch = &tip.commitment().schedule.current;
        if tip.block_hash() != claim.cut.block_hash
            || tip.id().0.as_ref() != claim.cut.context_id.as_ref()
            || tip.commitment().execution.world_state_root != claim.cut.world_root
            || epoch
                .context_id()
                .map_err(|_| PrivateCountersErrorV1::Context)?
                != claim.cut.epoch_context_id
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        let member_index = epoch
            .committee
            .iter()
            .position(|member| member.validator == self.identity)
            .and_then(|index| u16::try_from(index).ok())
            .ok_or(PrivateCountersErrorV1::Unauthorized)?;
        let observed_at_ms = native_time_ms()?;
        replay.observe_clock(observed_at_ms)?;
        replay.check_request(&request, observed_at_ms)?;
        archive
            .verify_unchanged()
            .map_err(|_| PrivateCountersErrorV1::Context)?;
        if !is_stable_state_view_generation(generation, self.state.state_view_generation())
            || !still_ready()
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        let body = CounterMemberBodyV1 {
            domain: PRIVATE_COUNTER_MEMBER_DOMAIN_V1,
            version: 1,
            claim_hash: claim.commitment()?,
            member_index,
            observed_at_ms,
        };
        let preimage = body.signing_preimage()?;
        // Atomically persist consumed original + clock BEFORE signing. An uncertain write
        // closes this owner; no installed signature or retry may follow that refusal.
        replay.consume(&request)?;
        let raw = self.signer.sign(&preimage);
        let signature =
            Signature::try_from_bytes(&raw.0).map_err(|_| PrivateCountersErrorV1::Signature)?;
        signature
            .verify(self.identity.public_key(), &preimage)
            .map_err(|_| PrivateCountersErrorV1::Signature)?;
        let response = PrivateCountersResponseV1 {
            claim,
            attestation: CounterMemberAttestationV1 { body, signature },
        };
        let wire = response.encode_canonical()?;
        archive
            .verify_unchanged()
            .map_err(|_| PrivateCountersErrorV1::Context)?;
        if !is_stable_state_view_generation(generation, self.state.state_view_generation())
            || !still_ready()
        {
            return Err(PrivateCountersErrorV1::Context);
        }
        Ok(PrivateCounterOriginalV1 {
            original: wire,
            _operation_allocation: _operation_charge,
        })
    }
}

fn native_time_ms() -> Result<u64, PrivateCountersErrorV1> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .filter(|now| *now != 0)
        .ok_or(PrivateCountersErrorV1::Freshness)
}

#[cfg(test)]
#[path = "private_counters/tests.rs"]
mod tests;
