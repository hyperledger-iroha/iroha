//! Borrowed policy and requester originals for private native counter computation.
//!
//! This helper grants no read permission and emits no wire proof. Its internal consumer
//! must authorize and compute under one unchanged certified archive boundary. Account
//! values changed by the deterministic tail cannot be represented as certified originals.

use super::*;
use iroha_data_model::{account::AccountValue, block::consensus::SumeragiRootScope};
use std::io::{self, Write};

// Policy and manifest together fit within their separately enforced native frame bounds.
// Refuse unrelated account metadata that would exceed the original-value work allowance.
const MAX_COUNTER_ACCOUNT_ORIGINAL_BYTES: usize = 2 * 1024 * 1024;

impl State {
    /// Borrow the installed policy authority and requester at the original certified cut.
    ///
    /// `policy_authority` is retained by the native startup owner from genesis; it must
    /// never be selected by an HTTP request. This verifies exact complete AccountValue
    /// hashes under the reconstructed pre-tail World. The caller retains authorization,
    /// the finite operation budget and unchanged archive boundary through computation.
    /// References cannot outlive the locked native World overlay or the original budget.
    ///
    /// # Errors
    /// Refuses genesis/Global roots, missing or tail-modified originals, changed native
    /// publication generation, and exhausted original allocation or account byte bounds.
    pub(crate) fn with_native_private_counter_accounts_v1<T>(
        &self,
        tip: &CommittedBlock,
        policy_authority: &AccountId,
        requester: &AccountId,
        budget: &AllocationBudget,
        consume: impl FnOnce(&WorldStateSnapshotV1, &AccountValue, &AccountValue) -> Result<T, String>,
    ) -> Result<T, WorldStateSnapshotError> {
        if tip.height() < 2 {
            return Err("Private counters require a certified successor".into());
        }
        self.with_native_world_state_snapshot_cut_v1(tip, None, budget, |snapshot, world| {
            if !matches!(
                crate::sumeragi::lanes::routing::committed_root_scope(world),
                Some(SumeragiRootScope::Dataspace { .. })
            ) {
                return Err("Private counters require an original private root".into());
            }
            let authority = counter_account_original(snapshot, world, policy_authority)?;
            let reader = counter_account_original(snapshot, world, requester)?;
            consume(snapshot, authority, reader)
        })
    }
}

fn counter_account_original<'a>(
    snapshot: &WorldStateSnapshotV1,
    world: &'a WorldBlock<'_>,
    account: &AccountId,
) -> Result<&'a AccountValue, String> {
    let original = world
        .accounts()
        .get(account)
        .ok_or("Private counters original account is absent")?;
    let mut bound = AccountOriginalBound {
        remaining: MAX_COUNTER_ACCOUNT_ORIGINAL_BYTES,
    };
    // Native encoding writes into a refusal boundary, rather than allocating a copy or
    // completing an arbitrarily large metadata traversal before checking its size.
    norito::codec::encode_adaptive_into(original, &mut bound)
        .map_err(|_| "Private counters original account exceeds its byte bound")?;
    require_target(
        snapshot,
        "world.accounts",
        WorldStateElementKindV1::Table,
        Some(hash_value(account)?),
        hash_value(original)?,
    )?;
    Ok(original)
}

struct AccountOriginalBound {
    remaining: usize,
}

impl Write for AccountOriginalBound {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "private counter account original exceeds its byte bound",
            ));
        }
        self.remaining -= bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "private_counter_accounts/tests.rs"]
mod tests;
