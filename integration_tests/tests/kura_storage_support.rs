//! Bounded read-only inspection of exact first-release lane-instance locators.

use std::{
    fs,
    io::Read as _,
    path::{Path, PathBuf},
};

use eyre::{Result, ensure};
use iroha_core::kura::LaneStorageIdentity;
use iroha_crypto::Hash;
use iroha_data_model::NetworkId;
use iroha_model_base::topology::{DataSpaceId, LaneId};
use norito::codec::{DecodeAll as _, Encode as _};

const MARKER_FILE: &str = ".lane-incarnation.norito";
const MAX_INSTANCES: usize = 65_536;
const MAX_MARKER_BYTES: u64 = 16 * 1024;

#[derive(Clone, norito::Encode, norito::Decode)]
struct LaneIncarnationMarkerV4 {
    version: u8,
    network_id: NetworkId,
    dataspace_id: DataSpaceId,
    lane_id: LaneId,
    incarnation: Hash,
    activation_height: u64,
    move_target_blocks: Option<String>,
    move_target_merge: Option<String>,
    block_store_digest: Hash,
    merge_log_digest: Hash,
}

/// Locate a stopped peer's retained lane instance using independently expected identity.
/// An absent expected incarnation requires that the lane has no retained instance.
/// When activation is known by the fixture, it must also match the marker.
/// The result is only a path; callers must authenticate the evidence they read.
///
/// # Errors
/// Rejects unsupported or noncanonical markers, unsafe paths, excess entries or bytes,
/// identity/path mismatches, unexpected lanes, and conflicting matching instances.
pub(super) fn lane_instance_blocks_dir(
    store_root: &Path,
    network_id: NetworkId,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    incarnation: Option<Hash>,
    activation_height: Option<u64>,
) -> Result<Option<PathBuf>> {
    let instances = store_root.join("blocks/instances");
    let entries = match fs::read_dir(&instances) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        fs::symlink_metadata(&instances)?.is_dir(),
        "instance root must be a real directory"
    );
    let mut found = None;
    for (index, entry) in entries.enumerate() {
        ensure!(
            index < MAX_INSTANCES,
            "lane-instance inspection exceeds its entry bound"
        );
        let path = entry?.path();
        ensure!(
            fs::symlink_metadata(&path)?.is_dir(),
            "lane instance must be a real directory"
        );
        let marker_path = path.join(MARKER_FILE);
        let metadata = fs::symlink_metadata(&marker_path)?;
        ensure!(
            metadata.is_file() && metadata.len() <= MAX_MARKER_BYTES,
            "lane marker must be a bounded regular file"
        );
        let mut bytes = Vec::new();
        fs::File::open(&marker_path)?
            .take(MAX_MARKER_BYTES + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            u64::try_from(bytes.len())? <= MAX_MARKER_BYTES,
            "lane marker grew beyond its byte bound"
        );
        let marker = LaneIncarnationMarkerV4::decode_all(&mut bytes.as_slice())?;
        ensure!(
            marker.version == 4 && marker.encode() == bytes,
            "lane marker must use canonical version 4"
        );
        ensure!(
            marker.network_id == network_id,
            "lane marker belongs to another network"
        );
        let identity = LaneStorageIdentity::new(
            marker.network_id,
            marker.lane_id,
            marker.dataspace_id,
            marker.incarnation,
            marker.activation_height,
        );
        ensure!(
            identity.blocks_dir(store_root) == path,
            "lane marker differs from its exact instance path"
        );
        if marker.lane_id != lane_id {
            continue;
        }
        ensure!(
            incarnation.is_some(),
            "unexpected retained instance for an absent lane"
        );
        if Some(marker.incarnation) != incarnation {
            continue;
        }
        ensure!(
            marker.dataspace_id == dataspace_id,
            "lane dataspace differs from the expected instance"
        );
        ensure!(
            activation_height.is_none_or(|height| height == marker.activation_height),
            "lane activation height differs from the expected instance"
        );
        ensure!(
            found.replace(path).is_none(),
            "multiple paths claim the exact lane instance"
        );
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::HashOf;

    #[test]
    fn inspection_binds_the_complete_expected_identity_and_canonical_path() {
        let temp = tempfile::tempdir().unwrap();
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"network")));
        let marker = LaneIncarnationMarkerV4 {
            version: 4,
            network_id: network,
            dataspace_id: DataSpaceId::new(2),
            lane_id: LaneId::new(3),
            incarnation: Hash::new(b"incarnation"),
            activation_height: 7,
            move_target_blocks: None,
            move_target_merge: None,
            block_store_digest: Hash::new(b"blocks"),
            merge_log_digest: Hash::new(b"merge"),
        };
        let path = LaneStorageIdentity::new(
            network,
            marker.lane_id,
            marker.dataspace_id,
            marker.incarnation,
            7,
        )
        .blocks_dir(temp.path());
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join(MARKER_FILE), marker.encode()).unwrap();
        let inspect = |incarnation, activation| {
            lane_instance_blocks_dir(
                temp.path(),
                network,
                marker.lane_id,
                marker.dataspace_id,
                incarnation,
                activation,
            )
        };
        assert_eq!(
            inspect(Some(marker.incarnation), Some(7)).unwrap(),
            Some(path.clone())
        );
        assert!(inspect(None, None).is_err());
        assert!(inspect(Some(marker.incarnation), Some(8)).is_err());
        assert!(
            inspect(Some(Hash::new(b"foreign incarnation")), Some(7))
                .unwrap()
                .is_none()
        );
        assert!(
            lane_instance_blocks_dir(
                temp.path(),
                network,
                marker.lane_id,
                DataSpaceId::new(9),
                Some(marker.incarnation),
                Some(7)
            )
            .is_err()
        );
        let foreign_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
            Hash::new(b"foreign network"),
        ));
        assert!(
            lane_instance_blocks_dir(
                temp.path(),
                foreign_network,
                marker.lane_id,
                marker.dataspace_id,
                Some(marker.incarnation),
                Some(7)
            )
            .is_err()
        );
        let mut obsolete = marker.clone();
        obsolete.version = 3;
        fs::write(path.join(MARKER_FILE), obsolete.encode()).unwrap();
        assert!(inspect(Some(marker.incarnation), Some(7)).is_err());
        fs::write(
            path.join(MARKER_FILE),
            vec![0; MAX_MARKER_BYTES as usize + 1],
        )
        .unwrap();
        assert!(inspect(Some(marker.incarnation), Some(7)).is_err());
        fs::write(path.join(MARKER_FILE), marker.encode()).unwrap();
        fs::rename(&path, path.with_file_name("foreign-instance")).unwrap();
        assert!(inspect(Some(marker.incarnation), Some(7)).is_err());
    }
}
