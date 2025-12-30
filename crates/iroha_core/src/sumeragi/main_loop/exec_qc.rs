//! Execution-QC gating and persistence helpers.

use eyre::{Result, bail};
use iroha_crypto::HashOf;
use iroha_data_model::{block::BlockHeader, consensus::ExecutionQcRecord};
use iroha_logger::prelude::*;
use mv::storage::StorageReadOnly;

use super::{Actor, PendingBlock, ProofPolicy};
use crate::state::{State, WorldReadOnly};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExecQcGateStatus {
    Allow,
    PersistCached,
    AwaitWsv,
    AwaitAny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExecQcMissingParentDecision {
    Allow,
    Block,
}

pub(super) fn exec_qc_missing_parent_decision(
    status: ExecQcGateStatus,
    parent_is_genesis: bool,
    parent_known: bool,
) -> ExecQcMissingParentDecision {
    if parent_is_genesis {
        return ExecQcMissingParentDecision::Allow;
    }
    if !parent_known {
        return ExecQcMissingParentDecision::Block;
    }
    match status {
        ExecQcGateStatus::AwaitWsv => ExecQcMissingParentDecision::Block,
        ExecQcGateStatus::Allow | ExecQcGateStatus::PersistCached | ExecQcGateStatus::AwaitAny => {
            ExecQcMissingParentDecision::Allow
        }
    }
}

#[allow(clippy::fn_params_excessive_bools)]
pub(super) fn exec_qc_gate_status(
    require_exec: bool,
    require_wsv: bool,
    wsv_present: bool,
    cached_present: bool,
) -> ExecQcGateStatus {
    if !require_exec {
        return ExecQcGateStatus::Allow;
    }
    if wsv_present {
        return ExecQcGateStatus::Allow;
    }
    if require_wsv {
        if cached_present {
            ExecQcGateStatus::PersistCached
        } else {
            ExecQcGateStatus::AwaitWsv
        }
    } else if cached_present {
        ExecQcGateStatus::Allow
    } else {
        ExecQcGateStatus::AwaitAny
    }
}

pub(super) fn persist_execution_qc_record_in_wsv(
    state: &State,
    record: ExecutionQcRecord,
) -> Result<()> {
    if record.signers_bitmap.is_empty() || record.bls_aggregate_signature.is_empty() {
        bail!("ExecutionQC record must carry a signer bitmap and aggregate BLS signature");
    }
    let subject = record.subject_block_hash;

    // Release the write guard before taking any state views to avoid re-entrant deadlocks.
    {
        let mut qcs = state.world.exec_qcs.block();
        let replace_placeholder = qcs
            .get(&subject)
            .is_some_and(|existing| existing.signers_bitmap.is_empty())
            && (!record.signers_bitmap.is_empty() || !record.bls_aggregate_signature.is_empty());
        let should_insert = match qcs.get(&subject) {
            None => true,
            Some(_) if replace_placeholder => true,
            Some(_) => false,
        };
        if should_insert {
            qcs.insert(subject, record);
            qcs.commit();
        }
    }

    let (stored_root, existing_root) = {
        let view = state.view();
        let stored_root = view
            .world()
            .exec_qcs()
            .get(&subject)
            .map(|qc| qc.post_state_root);
        let existing_root = view.world().exec_roots().get(&subject).copied();
        drop(view);
        (stored_root, existing_root)
    };
    if let Some(stored_root) = stored_root {
        if existing_root.is_some_and(|root| root != stored_root) {
            warn!(
                block = ?subject,
                qc_root = %stored_root,
                exec_root = ?existing_root,
                "exec_root mismatch with stored ExecutionQC record; overwriting"
            );
        }
        if existing_root != Some(stored_root) {
            let mut roots = state.world.exec_roots.block();
            roots.insert(subject, stored_root);
            roots.commit();
        }
    }
    Ok(())
}

impl Actor {
    pub(super) fn exec_qc_gate_allows(&mut self, pending: &PendingBlock) -> Result<bool> {
        let require_exec = self.config.require_execution_qc
            || matches!(
                self.config.proof_policy,
                ProofPolicy::ExecQcOnly | ProofPolicy::Hybrid
            );
        if !require_exec {
            return Ok(true);
        }
        let Some(parent_hash) = pending.block.header().prev_block_hash() else {
            return Ok(true);
        };
        let parent_height = pending.height.saturating_sub(1);
        let wsv_present = {
            let view = self.state.view();
            let present = view.world().exec_qcs().get(&parent_hash).is_some();
            drop(view);
            present
        };
        let cached_qc = self.execution_qc_cache.get(&parent_hash).cloned();
        let status = exec_qc_gate_status(
            require_exec,
            self.config.require_wsv_exec_qc,
            wsv_present,
            cached_qc.is_some(),
        );
        match status {
            ExecQcGateStatus::Allow => {
                if !wsv_present {
                    if let Some(qc) = cached_qc.as_ref() {
                        self.persist_execution_qc(qc)?;
                    }
                }
                Ok(true)
            }
            ExecQcGateStatus::PersistCached => {
                if let Some(qc) = cached_qc {
                    self.persist_execution_qc(&qc)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            ExecQcGateStatus::AwaitWsv | ExecQcGateStatus::AwaitAny => {
                self.exec_qc_gate_missing_parent(pending, parent_hash, parent_height, status)
            }
        }
    }

    fn exec_qc_gate_missing_parent(
        &self,
        pending: &PendingBlock,
        parent_hash: HashOf<BlockHeader>,
        parent_height: u64,
        status: ExecQcGateStatus,
    ) -> Result<bool> {
        let parent_block = self
            .kura
            .get_block_height_by_hash(parent_hash)
            .and_then(|height| self.kura.get_block(height));
        let parent_is_genesis = parent_block
            .as_ref()
            .is_some_and(|block| block.header().prev_block_hash().is_none());
        let decision =
            exec_qc_missing_parent_decision(status, parent_is_genesis, parent_block.is_some());
        match decision {
            ExecQcMissingParentDecision::Allow => {
                if parent_is_genesis {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        parent_height,
                        parent = ?parent_hash,
                        "allowing commit: parent is genesis and has no ExecutionQC"
                    );
                } else {
                    warn!(
                        height = pending.height,
                        view = pending.view,
                        parent_height,
                        parent = ?parent_hash,
                        "gating commit: ExecutionQC not available; allowing commit in non-strict mode"
                    );
                }
                Ok(true)
            }
            ExecQcMissingParentDecision::Block => {
                if matches!(status, ExecQcGateStatus::AwaitWsv) {
                    warn!(
                        height = pending.height,
                        view = pending.view,
                        parent_height,
                        parent = ?parent_hash,
                        "gating commit: parent block payload missing while awaiting strict ExecutionQC"
                    );
                } else {
                    debug!(
                        height = pending.height,
                        view = pending.view,
                        parent_height,
                        parent = ?parent_hash,
                        "gating commit: parent block payload missing while awaiting ExecutionQC"
                    );
                }
                Ok(false)
            }
        }
    }

    pub(super) fn persist_execution_qc(
        &self,
        qc: &crate::sumeragi::consensus::ExecutionQC,
    ) -> Result<()> {
        persist_execution_qc_record_in_wsv(
            self.state.as_ref(),
            ExecutionQcRecord {
                subject_block_hash: qc.subject_block_hash,
                post_state_root: qc.post_state_root,
                height: qc.height,
                view: qc.view,
                epoch: qc.epoch,
                signers_bitmap: qc.aggregate.signers_bitmap.clone(),
                bls_aggregate_signature: qc.aggregate.bls_aggregate_signature.clone(),
            },
        )
    }

    pub(super) fn persist_execution_qc_record(&self, record: ExecutionQcRecord) -> Result<()> {
        persist_execution_qc_record_in_wsv(self.state.as_ref(), record)
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{block::BlockHeader, consensus::ExecutionQcRecord};
    use mv::storage::StorageReadOnly;

    use super::persist_execution_qc_record_in_wsv;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World, WorldReadOnly},
    };

    #[test]
    fn persist_execution_qc_record_writes_root() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let subject = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32]));
        let post_state_root = Hash::prehashed([0x22; 32]);
        let record = ExecutionQcRecord {
            subject_block_hash: subject,
            post_state_root,
            height: 1,
            view: 0,
            epoch: 0,
            signers_bitmap: vec![0x01],
            bls_aggregate_signature: vec![0xAA],
        };

        persist_execution_qc_record_in_wsv(&state, record).expect("persist qc");

        let view = state.view();
        let stored = view
            .world()
            .exec_qcs()
            .get(&subject)
            .expect("execution QC stored");
        assert_eq!(stored.post_state_root, post_state_root);
        let root = view.world().exec_roots().get(&subject).copied();
        assert_eq!(root, Some(post_state_root));
    }
}
