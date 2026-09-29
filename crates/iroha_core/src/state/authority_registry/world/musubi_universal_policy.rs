//! Independent revision authority of Musubi's universal resolver and directory.
//!
//! The full physical rows remain in the publication journal and snapshots. Their
//! package, release, archive, availability, and selection fields are checked
//! against the authoritative sources on the current and rollback-visible World
//! cuts before this narrower State value may be used.

use iroha_data_model::musubi::{MusubiOrderedPackageEntryV1, MusubiResolverReleaseRowV1};
use norito::{Decode, Encode, NoritoSchema};

/// Last independent update revision of one resolver row; its release is the table key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:musubi-resolver-authority:v1")]
pub(in crate::state) struct MusubiResolverAuthorityV1 {
    index_revision: u64,
}

impl MusubiResolverAuthorityV1 {
    /// Borrow the revision after the source-equality validator accepts the row.
    pub(in crate::state) fn from_record(row: &MusubiResolverReleaseRowV1) -> Self {
        Self {
            index_revision: row.index_revision,
        }
    }
}

/// Last independent update revision of one directory row; its selector is the table key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:musubi-directory-authority:v1")]
pub(in crate::state) struct MusubiDirectoryAuthorityV1 {
    index_revision: u64,
}

impl MusubiDirectoryAuthorityV1 {
    /// Borrow the revision after the source-equality validator accepts the row.
    pub(in crate::state) fn from_record(row: &MusubiOrderedPackageEntryV1) -> Self {
        Self {
            index_revision: row.index_revision,
        }
    }
}
