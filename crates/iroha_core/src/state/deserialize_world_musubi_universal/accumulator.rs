//! Borrowed per-package resolver facts in the original caller-funded array.

use iroha_data_model::musubi::{
    MusubiPackageIdV1, MusubiReleaseIdV1, MusubiResolverReleaseRowV1, MusubiVersionV1,
};

pub(super) struct PackageRevision<'a> {
    pub(super) package: &'a MusubiPackageIdV1,
    pub(super) latest: Option<&'a MusubiVersionV1>,
    pub(super) maximum: Option<u64>,
    pub(super) directory_present: bool,
}

impl<'a> PackageRevision<'a> {
    pub(super) fn new(package: &'a MusubiPackageIdV1) -> Self {
        Self {
            package,
            latest: None,
            maximum: None,
            directory_present: false,
        }
    }

    pub(super) fn observe(
        &mut self,
        release: &'a MusubiReleaseIdV1,
        row: &MusubiResolverReleaseRowV1,
    ) {
        self.maximum = Some(self.maximum.map_or(row.index_revision, |previous| {
            previous.max(row.index_revision)
        }));
        if row.selection.fresh_selectable()
            && self.latest.is_none_or(|version| version < &release.version)
        {
            self.latest = Some(&release.version);
        }
    }
}
