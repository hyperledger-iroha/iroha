//! Authenticated typed privacy-state query execution.

use crate::{
    smartcontracts::ValidSingularQuery,
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::{
    privacy::{
        PrivacyActionExecutionReceiptViewV1, PrivacyAnonymousPgcPoolStateViewV1,
        PrivacyOrchardNullifierProvenanceV1, PrivacyOrchardPoolStateViewV1,
        PrivacyProofManagedPoolStateViewV1, PrivacyProtocolIdV1,
        PrivacyZkAceReplayNullifierProvenanceV1, PrivacyZkAmsAdmissionViewV1,
        PrivacyZkAmsProvisionViewV1, PrivacyZkX509CertificateNullifierProvenanceV1,
    },
    query::{
        error::QueryExecutionFail as QueryError,
        privacy::prelude::{
            FindPrivacyActionExecutionReceiptV1, FindPrivacyAnonymousPgcPoolStateV1,
            FindPrivacyOrchardNullifierV1, FindPrivacyOrchardPoolStateV1,
            FindPrivacyProofManagedPoolStateV1, FindPrivacyZkAceReplayNullifierV1,
            FindPrivacyZkAmsAdmissionV1, FindPrivacyZkAmsProvisionV1,
            FindPrivacyZkX509CertificateNullifierV1,
        },
    },
};

impl ValidSingularQuery for FindPrivacyZkAceReplayNullifierV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<PrivacyZkAceReplayNullifierProvenanceV1, QueryError> {
        let Some((policy_record_digest, statement_digest, admitted_at_height, action_index)) =
            state_ro
                .world()
                .privacy_zk_ace_replay_nullifier_fields_v1(
                    self.policy_id(),
                    self.replay_nullifier(),
                )
                .map_err(QueryError::Conversion)?
        else {
            return Err(QueryError::NotFound);
        };
        let finalized_height = u64::try_from(state_ro.height()).map_err(|_| {
            QueryError::Conversion("finalized ZK-ACE query height does not fit u64".to_owned())
        })?;
        let finalized_block_hash = state_ro.latest_block_hash().ok_or_else(|| {
            QueryError::Conversion(
                "durable ZK-ACE replay marker has no finalized block anchor".to_owned(),
            )
        })?;
        let provenance = PrivacyZkAceReplayNullifierProvenanceV1 {
            network_id: *state_ro.network_id(),
            policy_id: self.policy_id(),
            replay_nullifier: self.replay_nullifier(),
            policy_record_digest,
            statement_digest,
            admitted_at_height,
            action_index,
            finalized_height,
            finalized_block_hash,
        };
        provenance.validate().map_err(|error| {
            QueryError::Conversion(format!(
                "persisted ZK-ACE replay provenance cannot form a finalized query view: {error}"
            ))
        })?;
        Ok(provenance)
    }
}

impl ValidSingularQuery for FindPrivacyProofManagedPoolStateV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<PrivacyProofManagedPoolStateViewV1, QueryError> {
        if !matches!(
            self.protocol_id(),
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
                | PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
                | PrivacyProtocolIdV1::PqMaspStarkV0
        ) || self.pool_id().is_zero()
        {
            return Err(QueryError::Conversion(
                "proof-managed pool query requires FCMP++, private-IVM, or PQ-MASP and a non-zero pool id"
                    .to_owned(),
            ));
        }
        let Some(fields) = state_ro
            .world()
            .privacy_proof_managed_pool_query_state_v1(self.protocol_id(), self.pool_id())
            .map_err(QueryError::Conversion)?
        else {
            return Err(QueryError::NotFound);
        };
        let finalized_height = u64::try_from(state_ro.height()).map_err(|_| {
            QueryError::Conversion(
                "finalized proof-managed pool query height does not fit u64".to_owned(),
            )
        })?;
        let finalized_block_hash = state_ro.latest_block_hash().ok_or_else(|| {
            QueryError::Conversion(
                "durable proof-managed pool state has no finalized block anchor".to_owned(),
            )
        })?;
        let view = PrivacyProofManagedPoolStateViewV1 {
            network_id: *state_ro.network_id(),
            protocol_id: self.protocol_id(),
            pool_id: self.pool_id(),
            asset_definition_id: fields.asset_definition_id,
            root_role: fields.root_role,
            bootstrap_digest: fields.bootstrap_digest,
            initial_root: fields.initial_root,
            current_epoch: fields.current_epoch,
            current_root: fields.current_root,
            output_count: fields.output_count,
            bootstrap_admitted_at_height: fields.bootstrap_admitted_at_height,
            latest_transition: fields.latest_transition,
            finalized_height,
            finalized_block_hash,
        };
        view.validate().map_err(|error| {
            QueryError::Conversion(format!(
                "persisted proof-managed pool cannot form a finalized query view: {error}"
            ))
        })?;
        Ok(view)
    }
}

impl ValidSingularQuery for FindPrivacyOrchardPoolStateV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<PrivacyOrchardPoolStateViewV1, QueryError> {
        if self.pool_id().is_zero() {
            return Err(QueryError::Conversion(
                "Orchard pool query requires a non-zero pool id".to_owned(),
            ));
        }
        let Some(fields) = state_ro
            .world()
            .privacy_orchard_pool_query_state_v1(self.pool_id())
            .map_err(QueryError::Conversion)?
        else {
            return Err(QueryError::NotFound);
        };
        let finalized_height = u64::try_from(state_ro.height()).map_err(|_| {
            QueryError::Conversion(
                "finalized Orchard pool query height does not fit u64".to_owned(),
            )
        })?;
        let finalized_block_hash = state_ro.latest_block_hash().ok_or_else(|| {
            QueryError::Conversion(
                "durable Orchard pool state has no finalized block anchor".to_owned(),
            )
        })?;
        let view = PrivacyOrchardPoolStateViewV1 {
            network_id: *state_ro.network_id(),
            pool_id: self.pool_id(),
            asset_definition_id: fields.asset_definition_id,
            public_balance_scope: fields.public_balance_scope,
            reserve_account: fields.reserve_account,
            bootstrap_digest: fields.bootstrap_digest,
            current_epoch: fields.current_epoch,
            current_root: fields.current_root,
            tree_size: fields.tree_size,
            latest_transition: fields.latest_transition,
            finalized_height,
            finalized_block_hash,
        };
        view.validate().map_err(|error| {
            QueryError::Conversion(format!(
                "persisted Orchard pool cannot form a finalized query view: {error}"
            ))
        })?;
        Ok(view)
    }
}

impl ValidSingularQuery for FindPrivacyOrchardNullifierV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<PrivacyOrchardNullifierProvenanceV1, QueryError> {
        let Some((bootstrap_digest, statement_digest, admitted_at_height, action_index)) = state_ro
            .world()
            .privacy_orchard_nullifier_fields_v1(self.pool_id(), self.nullifier())
            .map_err(QueryError::Conversion)?
        else {
            return Err(QueryError::NotFound);
        };
        let finalized_height = u64::try_from(state_ro.height()).map_err(|_| {
            QueryError::Conversion(
                "finalized Orchard nullifier query height does not fit u64".to_owned(),
            )
        })?;
        let finalized_block_hash = state_ro.latest_block_hash().ok_or_else(|| {
            QueryError::Conversion(
                "durable Orchard nullifier has no finalized block anchor".to_owned(),
            )
        })?;
        let provenance = PrivacyOrchardNullifierProvenanceV1 {
            network_id: *state_ro.network_id(),
            pool_id: self.pool_id(),
            nullifier: self.nullifier(),
            bootstrap_digest,
            statement_digest,
            admitted_at_height,
            action_index,
            finalized_height,
            finalized_block_hash,
        };
        provenance.validate().map_err(|error| {
            QueryError::Conversion(format!(
                "persisted Orchard nullifier cannot form a finalized query view: {error}"
            ))
        })?;
        Ok(provenance)
    }
}

impl ValidSingularQuery for FindPrivacyAnonymousPgcPoolStateV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<PrivacyAnonymousPgcPoolStateViewV1, QueryError> {
        if self.pool_id().is_zero() {
            return Err(QueryError::Conversion(
                "Anonymous PGC pool query requires a non-zero pool id".to_owned(),
            ));
        }
        let Some(fields) = state_ro
            .world()
            .privacy_anonymous_pgc_pool_query_state_v1(self.pool_id())
            .map_err(QueryError::Conversion)?
        else {
            return Err(QueryError::NotFound);
        };
        let finalized_height = u64::try_from(state_ro.height()).map_err(|_| {
            QueryError::Conversion(
                "finalized Anonymous PGC query height does not fit u64".to_owned(),
            )
        })?;
        let finalized_block_hash = state_ro.latest_block_hash().ok_or_else(|| {
            QueryError::Conversion(
                "durable Anonymous PGC pool has no finalized block anchor".to_owned(),
            )
        })?;
        let view = PrivacyAnonymousPgcPoolStateViewV1 {
            network_id: *state_ro.network_id(),
            pool_id: self.pool_id(),
            total_supply: fields.total_supply,
            bootstrap_root: fields.bootstrap_root,
            bootstrap_digest: fields.bootstrap_digest,
            bootstrap_proof_digest: fields.bootstrap_proof_digest,
            current_epoch: fields.current_epoch,
            current_root: fields.current_root,
            account_count: fields.account_count,
            current_state_admitted_at_height: fields.current_state_admitted_at_height,
            latest_transition: fields.latest_transition,
            finalized_height,
            finalized_block_hash,
        };
        view.validate().map_err(|error| {
            QueryError::Conversion(format!(
                "persisted Anonymous PGC pool cannot form a finalized query view: {error}"
            ))
        })?;
        Ok(view)
    }
}

impl ValidSingularQuery for FindPrivacyZkAmsAdmissionV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<PrivacyZkAmsAdmissionViewV1, QueryError> {
        let (issuer_id, registry_id, policy_id) = self.namespace_components();
        let Some(fields) = state_ro
            .world()
            .privacy_zk_ams_admission_query_state_v1(
                issuer_id,
                registry_id,
                policy_id,
                self.phc_hash(),
            )
            .map_err(QueryError::Conversion)?
        else {
            return Err(QueryError::NotFound);
        };
        let finalized_height = u64::try_from(state_ro.height()).map_err(|_| {
            QueryError::Conversion("finalized ZK-AMS query height does not fit u64".to_owned())
        })?;
        let finalized_block_hash = state_ro.latest_block_hash().ok_or_else(|| {
            QueryError::Conversion(
                "durable ZK-AMS admission has no finalized block anchor".to_owned(),
            )
        })?;
        let view = PrivacyZkAmsAdmissionViewV1 {
            network_id: *state_ro.network_id(),
            issuer_id,
            registry_id,
            policy_id,
            phc_hash: self.phc_hash(),
            seed_public_key: fields.seed_public_key,
            bootstrap_digest: fields.bootstrap_digest,
            issuer_policy_record_digest: fields.issuer_policy_record_digest,
            policy_digest: fields.policy_digest,
            registry_record_digest: fields.registry_record_digest,
            parent_epoch: fields.parent_epoch,
            parent_root: fields.parent_root,
            anchor_index: fields.anchor_index,
            batch_size: fields.batch_size,
            successor_epoch: fields.successor_epoch,
            successor_root: fields.successor_root,
            statement_digest: fields.statement_digest,
            admitted_at_height: fields.admitted_at_height,
            action_index: fields.action_index,
            finalized_height,
            finalized_block_hash,
        };
        view.validate().map_err(|error| {
            QueryError::Conversion(format!(
                "persisted ZK-AMS admission cannot form a finalized query view: {error}"
            ))
        })?;
        Ok(view)
    }
}

impl ValidSingularQuery for FindPrivacyZkAmsProvisionV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<PrivacyZkAmsProvisionViewV1, QueryError> {
        let (issuer_id, registry_id, policy_id) = self.namespace_components();
        let Some(fields) = state_ro
            .world()
            .privacy_zk_ams_provision_query_state_v1(
                issuer_id,
                registry_id,
                policy_id,
                self.key_image(),
            )
            .map_err(QueryError::Conversion)?
        else {
            return Err(QueryError::NotFound);
        };
        let finalized_height = u64::try_from(state_ro.height()).map_err(|_| {
            QueryError::Conversion("finalized ZK-AMS query height does not fit u64".to_owned())
        })?;
        let finalized_block_hash = state_ro.latest_block_hash().ok_or_else(|| {
            QueryError::Conversion(
                "durable ZK-AMS provision has no finalized block anchor".to_owned(),
            )
        })?;
        let view = PrivacyZkAmsProvisionViewV1 {
            network_id: *state_ro.network_id(),
            issuer_id,
            registry_id,
            policy_id,
            key_image: self.key_image(),
            account_id: fields.account_id,
            bootstrap_digest: fields.bootstrap_digest,
            issuer_policy_record_digest: fields.issuer_policy_record_digest,
            policy_digest: fields.policy_digest,
            registry_record_digest: fields.registry_record_digest,
            registry_epoch: fields.registry_epoch,
            registry_root: fields.registry_root,
            statement_digest: fields.statement_digest,
            admitted_at_height: fields.admitted_at_height,
            action_index: fields.action_index,
            finalized_height,
            finalized_block_hash,
        };
        view.validate().map_err(|error| {
            QueryError::Conversion(format!(
                "persisted ZK-AMS provision cannot form a finalized query view: {error}"
            ))
        })?;
        Ok(view)
    }
}

impl ValidSingularQuery for FindPrivacyZkX509CertificateNullifierV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<PrivacyZkX509CertificateNullifierProvenanceV1, QueryError> {
        let (trust_anchor_id, policy_id) = self.namespace_components();
        let Some(fields) = state_ro
            .world()
            .privacy_zk_x509_certificate_nullifier_query_state_v1(
                trust_anchor_id,
                policy_id,
                self.nullifier(),
            )
            .map_err(QueryError::Conversion)?
        else {
            return Err(QueryError::NotFound);
        };
        let finalized_height = u64::try_from(state_ro.height()).map_err(|_| {
            QueryError::Conversion("finalized ZK-X509 query height does not fit u64".to_owned())
        })?;
        let finalized_block_hash = state_ro.latest_block_hash().ok_or_else(|| {
            QueryError::Conversion(
                "durable ZK-X509 certificate nullifier has no finalized block anchor".to_owned(),
            )
        })?;
        let provenance = PrivacyZkX509CertificateNullifierProvenanceV1 {
            network_id: *state_ro.network_id(),
            trust_anchor_id,
            policy_id,
            nullifier: self.nullifier(),
            trust_anchor_record_digest: fields.trust_anchor_record_digest,
            trust_anchor_record_epoch: fields.trust_anchor_record_epoch,
            certificate_policy_record_digest: fields.certificate_policy_record_digest,
            certificate_policy_record_epoch: fields.certificate_policy_record_epoch,
            crl_record_digest: fields.crl_record_digest,
            crl_record_epoch: fields.crl_record_epoch,
            statement_digest: fields.statement_digest,
            admitted_at_height: fields.admitted_at_height,
            action_index: fields.action_index,
            finalized_height,
            finalized_block_hash,
        };
        provenance.validate().map_err(|error| {
            QueryError::Conversion(format!(
                "persisted ZK-X509 nullifier cannot form a finalized query view: {error}"
            ))
        })?;
        Ok(provenance)
    }
}

impl ValidSingularQuery for FindPrivacyActionExecutionReceiptV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<PrivacyActionExecutionReceiptViewV1, QueryError> {
        let Some(receipt) = state_ro
            .world()
            .privacy_action_execution_receipt_record_v1(
                self.protocol_id(),
                self.transaction_hash(),
                self.action_index(),
            )
            .map_err(QueryError::Conversion)?
        else {
            return Err(QueryError::NotFound);
        };
        if receipt.network_id != *state_ro.network_id() {
            return Err(QueryError::Conversion(
                "persisted privacy execution receipt belongs to a different NetworkId".to_owned(),
            ));
        }
        let finalized_height = u64::try_from(state_ro.height()).map_err(|_| {
            QueryError::Conversion(
                "finalized privacy execution-receipt height does not fit u64".to_owned(),
            )
        })?;
        let finalized_block_hash = state_ro.latest_block_hash().ok_or_else(|| {
            QueryError::Conversion(
                "durable privacy execution receipt has no finalized block anchor".to_owned(),
            )
        })?;
        let view = PrivacyActionExecutionReceiptViewV1 {
            version: receipt.version,
            network_id: receipt.network_id,
            protocol_id: receipt.protocol_id,
            operation_schema: receipt.operation_schema,
            ledger_effect_kind: receipt.ledger_effect_kind,
            transaction_hash: receipt.transaction_hash,
            action_index: receipt.action_index,
            transaction_intent_digest: receipt.transaction_intent_digest,
            statement_digest: receipt.statement_digest,
            proof_envelope_hash: receipt.proof_envelope_hash,
            capability_manifest_digest: receipt.capability_manifest_digest,
            capability_committed_height: receipt.capability_committed_height,
            admitted_at_height: receipt.admitted_at_height,
            finalized_height,
            finalized_block_hash,
        };
        view.validate().map_err(|error| {
            QueryError::Conversion(format!(
                "persisted privacy execution receipt cannot form a finalized query view: {error}"
            ))
        })?;
        Ok(view)
    }
}
