//! Complete borrowed archive, attestation and current-evidence shapes.

use super::*;
use crate::sorafs::pin_registry::{
    PinManifestRecord, ProviderIngestCompletionAuthorityV1, ReplicationOrderCompletionRecord,
    ReplicationOrderRecord,
};

impl SourceGeometry {
    fn chunker(
        &mut self,
        value: &crate::sorafs::pin_registry::ChunkerProfileHandle,
    ) -> Result<(), SourceGeometryError> {
        let crate::sorafs::pin_registry::ChunkerProfileHandle {
            profile_id: _,
            namespace,
            name,
            semver,
            multihash_code: _,
        } = value;
        self.node()?;
        self.text(namespace)?;
        self.text(name)?;
        self.text(semver)
    }
    fn commitment(&mut self, value: &MusubiArchiveCommitmentV1) -> Result<(), SourceGeometryError> {
        let MusubiArchiveCommitmentV1 {
            root_cid: _,
            chunker,
            chunk_plan_digest: _,
            por_root: _,
            content_length: _,
            car_digest: _,
            car_size: _,
            bundle_digest: _,
            source_tree_digest: _,
            descriptor_digest: _,
            file_count: _,
            chunk_count: _,
        } = value;
        self.node()?;
        self.chunker(chunker)
    }
    pub(super) fn archive(
        &mut self,
        value: &MusubiArchiveRecordV1,
    ) -> Result<(), SourceGeometryError> {
        let MusubiArchiveRecordV1 {
            archive_id: _,
            commitment,
            staging_receipt,
            registered_by,
            registered_at_height: _,
            location_revision: _,
            location_ids,
        } = value;
        self.node()?;
        self.items(location_ids.len())?;
        self.commitment(commitment)?;
        self.receipt(staging_receipt)?;
        self.account(registered_by)
    }
    fn receipt(&mut self, value: &MusubiSeedIngressReceiptV1) -> Result<(), SourceGeometryError> {
        let MusubiSeedIngressReceiptV1 { payload, approvals } = value;
        let MusubiSeedIngressReceiptPayloadV1 {
            version: _,
            binding,
            issued_at_ms: _,
            expires_at_ms: _,
        } = payload;
        let MusubiSeedIngressReceiptBindingV1 {
            network_id: _,
            publisher,
            ingress_broker,
            seed_provider: _,
            semantic_release_manifest_digest: _,
            archive_id: _,
            car_body_digest: _,
            car_body_length: _,
            nonce: _,
        } = binding;
        self.node()?;
        self.items(approvals.len())?;
        self.account(publisher)?;
        self.account(ingress_broker)?;
        for approval in approvals {
            let MusubiSeedIngressReceiptApprovalV1 {
                public_key,
                signature,
            } = approval;
            self.key(public_key)?;
            self.bytes(signature.payload())?;
        }
        Ok(())
    }
    pub(super) fn attestation_record(
        &mut self,
        value: &MusubiProviderBundleAttestationRecordV1,
    ) -> Result<(), SourceGeometryError> {
        let MusubiProviderBundleAttestationRecordV1 {
            key: _,
            attestation_digest: _,
            attestation,
            registered_by,
            registered_at_height: _,
        } = value;
        let MusubiProviderBundleVerificationAttestationV1 { payload, approvals } = attestation;
        let MusubiProviderBundleVerificationPayloadV1 {
            version: _,
            binding,
        } = payload;
        let MusubiProviderBundleVerificationBindingV1 {
            network_id: _,
            provider_id: _,
            completed_by,
            completion_authority,
            replication_order: _,
            assignment_revision: _,
            completion_epoch: _,
            finalized_anchor: _,
            archive_id: _,
            bundle_digest: _,
            descriptor_digest: _,
            semantic_release_manifest_digest: _,
            verification_lock_digest: _,
            source_tree_digest: _,
        } = binding;
        self.node()?;
        self.items(approvals.len())?;
        self.account(completed_by)?;
        self.completion_authority(completion_authority)?;
        self.account(registered_by)?;
        for approval in approvals {
            let MusubiProviderBundleVerificationApprovalV1 {
                public_key,
                signature,
            } = approval;
            self.key(public_key)?;
            self.bytes(signature.payload())?;
        }
        Ok(())
    }
    fn completion_authority(
        &mut self,
        value: &ProviderIngestCompletionAuthorityV1,
    ) -> Result<(), SourceGeometryError> {
        let ProviderIngestCompletionAuthorityV1 {
            provider_owner,
            signer_policy: _,
        } = value;
        self.node()?;
        self.account(provider_owner)
    }
    pub(super) fn location(
        &mut self,
        value: &MusubiArchiveLocationV1,
    ) -> Result<(), SourceGeometryError> {
        let MusubiArchiveLocationV1 {
            location_id: _,
            archive_id: _,
            pin_manifest: _,
            replication_order: _,
            providers,
            provider_attestation_set_digest: _,
            renew_after_epoch: _,
            expires_at_epoch: _,
            finalized_height: _,
            revision: _,
            state: _,
        } = value;
        self.node()?;
        self.items(providers.len())
    }
    pub(super) fn order_binding(
        &mut self,
        value: &MusubiReplicationOrderLocationReferenceV1,
    ) -> Result<(), SourceGeometryError> {
        let MusubiReplicationOrderLocationReferenceV1 { binding, lifecycle } = value;
        let MusubiReplicationOrderArchiveBindingV1 {
            replication_order: _,
            archive_id: _,
            commitment,
        } = binding;
        self.node()?;
        self.commitment(commitment)?;
        match lifecycle {
            MusubiReplicationOrderLocationLifecycleV1::PreLocation
            | MusubiReplicationOrderLocationLifecycleV1::Active(_) => Ok(()),
            MusubiReplicationOrderLocationLifecycleV1::Retired(retired) => {
                let MusubiRetiredReplicationOrderLocationV1 {
                    location: _,
                    providers,
                } = retired;
                self.items(providers.len())
            }
        }
    }
    pub(super) fn pin(&mut self, value: &PinManifestRecord) -> Result<(), SourceGeometryError> {
        let PinManifestRecord {
            digest: _,
            root_cid: _,
            chunker,
            chunk_digest_sha3_256: _,
            por_root: _,
            content_length: _,
            policy: _,
            submitted_by: _,
            submitted_epoch: _,
            approved_epoch: _,
            alias: _,
            successor_of: _,
            metadata: _,
            status: _,
            retirement_reason: _,
            council_envelope_digest: _,
            pin_fee_payment: _,
        } = value;
        self.node()?;
        self.chunker(chunker)
    }
    pub(super) fn order(
        &mut self,
        value: &ReplicationOrderRecord,
    ) -> Result<(), SourceGeometryError> {
        let ReplicationOrderRecord {
            order_id: _,
            manifest_digest: _,
            manifest_root_cid: _,
            musubi_archive: _,
            issued_by: _,
            issued_epoch: _,
            deadline_epoch: _,
            canonical_order: _,
            assignment_revision: _,
            provider_completions,
            status: _,
        } = value;
        self.node()?;
        self.items(provider_completions.len())?;
        for completion in provider_completions {
            let ReplicationOrderCompletionRecord {
                provider_id: _,
                completed_by,
                completion_epoch: _,
                assignment_revision: _,
                completion_authority,
                finalized_anchor: _,
            } = completion;
            self.account(completed_by)?;
            self.completion_authority(completion_authority)?;
        }
        Ok(())
    }
}
