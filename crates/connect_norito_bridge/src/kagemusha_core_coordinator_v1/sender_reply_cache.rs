//! Bounded process-local sender observations; original native journals remain authority.
//!
//! The caller admits tokens only after current native selection, complete accepted
//! qualification, signature and original command/digest correlation checks.

use std::collections::BTreeMap;

use super::super::sender_observation::AuthenticatedSenderReplyV1;
use super::{RegistryError, Result};

const MAX_SENDER_REPLIES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admission {
    Keep,
    ReplaceRead,
}

// Keep every field of an exact mutation retry, including a retained original op12 frame.
// This pure decision kernel never authenticates bytes or creates a native capability.
fn admission<T: PartialEq>(operation: u8, previous: &T, next: &T) -> Result<Admission> {
    if !matches!(operation, 5 | 6 | 7 | 8 | 9 | 10 | 12) {
        return Err(RegistryError::Rejected);
    }
    if previous == next {
        return Ok(Admission::Keep);
    }
    if matches!(operation, 6 | 8 | 10) {
        Ok(Admission::ReplaceRead)
    } else {
        Err(RegistryError::Rejected)
    }
}

fn needs_transient_slot(operation: u8) -> bool {
    operation != 12
}

// Only read observations can lose their transient slot. The native caller has authenticated
// the incoming token before choosing a victim; an evicted lookup fails closed and can be read again.
fn read_victim<T>(cache: &BTreeMap<(u8, [u8; 32]), T>) -> Option<(u8, [u8; 32])> {
    cache
        .keys()
        .copied()
        .find(|(operation, _)| matches!(operation, 6 | 8 | 10))
}

// Called only after native Core completed-WAL/Released proof succeeds. This helper performs
// storage retirement, never authentication, and cannot retire another operation's originals.
pub(super) fn retire_completed_operation<T>(
    cache: &mut BTreeMap<(u8, [u8; 32]), T>,
    operation: [u8; 32],
) {
    cache.retain(|(_, id), _| *id != operation);
}

pub(super) fn admit_authenticated(
    cache: &mut BTreeMap<(u8, [u8; 32]), AuthenticatedSenderReplyV1>,
    token: AuthenticatedSenderReplyV1,
) -> Result<()> {
    let operation = token.command().operation;
    if !matches!(operation, 5 | 6 | 7 | 8 | 9 | 10 | 12) {
        return Err(RegistryError::Rejected);
    }
    // Op12 completion must be able to resolve an operation even when every transient slot
    // contains uncertain mutation originals. Its exact frame is retained by the serialized
    // native release attempt and actual private completion WAL, never reconstructed here.
    if !needs_transient_slot(operation) {
        return Ok(());
    }
    let key = (operation, token.command().operation_id);
    if let Some(previous) = cache.get(&key) {
        match admission(operation, previous, &token)? {
            Admission::Keep => return Ok(()),
            Admission::ReplaceRead => {
                // The native operation fixes the historical command. Only its authenticated
                // current observation may refresh; another command cannot borrow this slot.
                if previous.original_command() != token.original_command()
                    // Stable-wallet index revisions do not restart at hardware rotation.
                    || token.reply().index_revision < previous.reply().index_revision
                    || (token.reply().index_revision == previous.reply().index_revision
                        && token.reply().body != previous.reply().body)
                {
                    return Err(RegistryError::Rejected);
                }
            }
        }
    } else if cache.len() >= MAX_SENDER_REPLIES {
        let victim = read_victim(cache).ok_or(RegistryError::Rejected)?;
        cache.remove(&victim);
    }
    cache.insert(key, token);
    Ok(())
}

#[cfg(test)]
#[path = "sender_reply_cache/tests.rs"]
mod tests;
