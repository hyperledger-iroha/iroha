//! Authenticated fixed historical identity without a full-body or transaction receipt.

use super::*;

/// Header and certified context identity; deliberately exposes no body or native proof source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CommittedIdentity {
    header: IrohaHeader,
    core: Hash32,
    result: Hash32,
}

impl CommittedIdentity {
    /// Exact committed Iroha header hash.
    pub(crate) fn block_hash(self) -> HashOf<IrohaHeader> {
        self.header.hash()
    }
    /// Certified context identifier binding original core header and execution result.
    pub(crate) fn id(self) -> HeightContextId {
        certified_block_id(&self.core, &self.result)
    }
    /// Original committed ledger time.
    pub(crate) fn block_time_ms(self) -> u64 {
        u64::try_from(self.header.creation_time().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Authenticate fixed metadata from either an original native carrier or installed portable
/// ancestry. Portable observations never become `CommittedBlock` or prove a transaction.
pub(crate) fn committed_identity(
    state: &(impl StateReadOnly + ?Sized),
    height: u64,
) -> Result<CommittedIdentity, ExecutionAttemptError<ChainReadError>> {
    if height == 0 || height > state.height() as u64 {
        return Err(ChainReadError::NotInView { height }.into());
    }
    if state
        .kura()
        .checkpoint_prefix_height()
        .is_some_and(|cut| height <= cut)
    {
        let mut found = None;
        let hashes = state.block_hashes();
        state
            .execution_budget()
            .with_deferred_refund_notifications(|_| {
                state.kura().walk_checkpoint_prefix_lookup(
                    state.chain_id(),
                    *state.network_id(),
                    hashes.len(),
                    &|index| hashes.get(index).copied(),
                    &mut |parent| {
                        if parent.header().height().get() == height {
                            found = Some(CommittedIdentity {
                                header: *parent.header(),
                                core: parent.core_hash(),
                                result: parent.result(),
                            });
                        }
                        Ok(())
                    },
                )
            })
            .map_err(|error| {
                error.map_rejection(|original| ChainReadError::Malformed {
                    height,
                    reason: original.to_string(),
                })
            })?;
        return found.ok_or_else(|| ChainReadError::NotInView { height }.into());
    }
    let original = committed_block(state, height)?;
    Ok(CommittedIdentity {
        header: original.block().header(),
        core: original.core_hash(),
        result: original.result(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_identity_matches_original_context_without_granting_body_custody() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let fixture = crate::snapshot::tests::transferred_checkpoint_fixture();
                let view = fixture.restored.view();
                for height in 1..=2 {
                    let original = fixture.source.committed(height);
                    let identity = committed_identity(&view, height).unwrap();
                    assert_eq!(identity.block_hash(), original.block_hash());
                    assert_eq!(identity.id(), original.id());
                    assert_eq!(identity.block_time_ms(), original.block_time_ms());
                }
                assert!(committed_block(&view, 2).is_err());
                assert!(committed_identity(&view, 3).is_err());
                let refused = norito::core::with_decode_limits_scope(
                    norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 128),
                    || committed_identity(&view, 2),
                );
                assert!(matches!(refused, Err(ExecutionAttemptError::Deferred(_))));
                assert_eq!(
                    committed_identity(&view, 2).unwrap().id(),
                    fixture.source.committed(2).id()
                );
            })
            .unwrap()
            .join()
            .unwrap_or_else(|error| std::panic::resume_unwind(error));
    }
}
