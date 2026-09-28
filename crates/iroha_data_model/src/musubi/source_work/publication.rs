//! Complete borrowed publication and duplicated resolver/directory shapes.

use super::*;

impl SourceGeometry {
    pub(super) fn package(
        &mut self,
        value: &MusubiPackageRecordV1,
    ) -> Result<(), SourceGeometryError> {
        let MusubiPackageRecordV1 {
            package,
            claimed_namespace,
            claimed_namespace_binding: _,
            owners,
            member_accounts,
            claimed_at_height: _,
            revisions: _,
        } = value;
        self.node()?;
        self.items(owners.len())?;
        self.items(member_accounts.len())?;
        self.package_id(package)?;
        self.text(claimed_namespace.as_str())?;
        for account in owners.iter().chain(member_accounts) {
            self.account(account)?;
        }
        Ok(())
    }
    fn metadata(&mut self, value: &MusubiReleaseMetadataV1) -> Result<(), SourceGeometryError> {
        let MusubiReleaseMetadataV1 {
            description,
            readme,
            license,
            repository,
            keywords,
        } = value;
        self.node()?;
        self.items(keywords.len())?;
        if let Some(value) = description {
            self.text(value.as_str())?;
        }
        for value in [readme, license, repository].into_iter().flatten() {
            self.text(value.as_str())?;
        }
        for value in keywords {
            self.text(&value.0)?;
        }
        Ok(())
    }
    pub(super) fn manifest(
        &mut self,
        value: &MusubiReleaseManifestV1,
    ) -> Result<(), SourceGeometryError> {
        let MusubiReleaseManifestV1 {
            release,
            edition: _,
            abi: _,
            dependencies,
            exports,
            interface_digest: _,
            metadata,
            archive_id: _,
            verification_lock_digest: _,
        } = value;
        self.node()?;
        self.items(exports.len())?;
        self.release_id(release)?;
        self.dependencies(dependencies)?;
        for export in exports {
            self.text(export.as_ref())?;
        }
        self.metadata(metadata)
    }
    fn yank(&mut self, value: &MusubiReleaseYankV1) -> Result<(), SourceGeometryError> {
        let MusubiReleaseYankV1 {
            release,
            yanked: _,
            reason,
            changed_by,
            changed_at_height: _,
            revision: _,
        } = value;
        self.node()?;
        self.release_id(release)?;
        self.text(reason.as_str())?;
        self.account(changed_by)
    }
    fn governance(
        &mut self,
        value: &MusubiArtifactGovernanceStateV1,
    ) -> Result<(), SourceGeometryError> {
        self.node()?;
        match value {
            MusubiArtifactGovernanceStateV1::Available => Ok(()),
            MusubiArtifactGovernanceStateV1::TakenDown(value) => {
                let MusubiArtifactTakedownV1 {
                    action_digest: _,
                    reason,
                    applied_at_height: _,
                } = value;
                self.text(reason.as_str())
            }
        }
    }
    pub(super) fn release(
        &mut self,
        value: &MusubiReleaseRecordV1,
    ) -> Result<(), SourceGeometryError> {
        let MusubiReleaseRecordV1 {
            manifest,
            release_digest: _,
            published_by,
            published_at_height: _,
            yank,
            artifact_governance,
            revisions: _,
        } = value;
        self.node()?;
        self.manifest(manifest)?;
        self.account(published_by)?;
        self.yank(yank)?;
        self.governance(artifact_governance)
    }
    pub(super) fn resolver(
        &mut self,
        value: &MusubiResolverReleaseRowV1,
    ) -> Result<(), SourceGeometryError> {
        let MusubiResolverReleaseRowV1 {
            release,
            release_digest: _,
            archive_id: _,
            source_digest: _,
            interface_digest: _,
            abi: _,
            dependencies,
            selection,
            index_revision: _,
        } = value;
        let MusubiReleaseSelectionStateV1 {
            yank,
            storage: _,
            governance,
        } = selection;
        self.node()?;
        self.release_id(release)?;
        self.dependencies(dependencies)?;
        self.yank(yank)?;
        self.governance(governance)
    }
    pub(super) fn directory(
        &mut self,
        value: &MusubiOrderedPackageEntryV1,
    ) -> Result<(), SourceGeometryError> {
        let MusubiOrderedPackageEntryV1 {
            selector,
            package,
            latest_selectable,
            metadata_revision: _,
            index_revision: _,
        } = value;
        self.node()?;
        self.selector(selector)?;
        self.package_id(package)?;
        if let Some(version) = latest_selectable {
            self.version(version)?;
        }
        Ok(())
    }
}
