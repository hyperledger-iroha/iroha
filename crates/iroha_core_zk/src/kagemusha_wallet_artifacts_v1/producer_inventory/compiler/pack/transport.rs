//! Bounded transport DATA exported only from the genuine completed wallet pack.

use std::{fmt::Write as _, io, io::Read as _, io::Write as _, path::Path};

use iroha_fs::PrivateDirectory;
use rand::rand_core::TryRngCore as _;
use sha2::{Digest as _, Sha256};

use super::{BlobV1, WalletArtifactOriginalsV1};
use crate::kagemusha_wallet_artifacts_v1::producer_inventory::DirectoryOriginalsV1;

const TRANSPORT_MAX_BYTES: usize = 32 * 1024 * 1024;
const PACK_MAX_BYTES: usize = 16 * 1024 * 1024 + 64 * 1024;
const CATALOG_MAX_BYTES: usize = 16 * 1024 * 1024;
const ORIGINAL_MAX_BYTES: u64 = 1024 * 1024 * 1024;
const WALLET_MAX_ROWS: usize = 3 * 4096;
const FINALITY_MAX_ROWS: usize = 3 * 65536;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn rows(output: &mut String, originals: &[BlobV1], maximum: usize) -> io::Result<()> {
    if originals.len() > maximum {
        return Err(invalid("original transport row ceiling exceeded"));
    }
    let mut previous = None;
    output.push('[');
    for (index, blob) in originals.iter().enumerate() {
        if blob.bytes == 0
            || blob.bytes > ORIGINAL_MAX_BYTES
            || blob.sha256 == [0; 32]
            || previous.is_some_and(|digest| digest >= blob.sha256)
        {
            return Err(invalid(
                "original transport identity is not closed and ascending",
            ));
        }
        previous = Some(blob.sha256);
        if index != 0 {
            output.push(',');
        }
        write!(
            output,
            "{{\"bytes\":{},\"sha256\":\"{}\"}}",
            blob.bytes,
            hex::encode(blob.sha256)
        )
        .map_err(|_| invalid("original transport encoding failed"))?;
        if output.len() > TRANSPORT_MAX_BYTES {
            return Err(invalid("original transport byte ceiling exceeded"));
        }
    }
    output.push(']');
    Ok(())
}

fn verify_metadata(directory: &PrivateDirectory, name: &str, bytes: &[u8]) -> io::Result<()> {
    directory.revalidate()?;
    let mut original = directory.open_retained_read_only(name, bytes.len())?;
    if original.len()? != bytes.len() as u64 {
        return Err(invalid("existing metadata extent differs"));
    }
    let snapshot = original.snapshot()?;
    let expected: [u8; 32] = Sha256::digest(bytes).into();
    let mut hash = Sha256::new();
    let mut total = 0_usize;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        original.revalidate()?;
        if original.snapshot()? != snapshot {
            return Err(invalid("metadata changed before streaming"));
        }
        let count = original.read(&mut buffer)?;
        original.revalidate()?;
        if original.snapshot()? != snapshot {
            return Err(invalid("metadata changed during streaming"));
        }
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count)
            .ok_or_else(|| invalid("metadata extent overflow"))?;
        if total > bytes.len() {
            return Err(invalid("metadata extended during streaming"));
        }
        hash.update(&buffer[..count]);
    }
    let actual: [u8; 32] = hash.finalize().into();
    if total != bytes.len() || actual != expected {
        return Err(invalid("existing metadata original differs"));
    }
    directory.sync()?;
    directory.revalidate()
}

fn publish_metadata(directory: &PrivateDirectory, name: &str, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() {
        return Err(invalid("empty metadata original"));
    }
    match verify_metadata(directory, name, bytes) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut nonce = [0_u8; 16];
    rand::rngs::OsRng
        .try_fill_bytes(&mut nonce)
        .map_err(io::Error::other)?;
    let staging = format!("wallet-metadata-{}.partial", hex::encode(nonce));
    let mut writer = directory.create_retained_private(staging, bytes.len())?;
    writer.write_all(bytes)?;
    let sealed = writer.seal_read_only()?;
    match sealed.publish_new_name(name) {
        Ok(original) => original.revalidate()?,
        // A competing identical writer may win. Its whole original must still pass
        // all checks; neither the winning file nor our partial evidence is replaced.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    verify_metadata(directory, name, bytes)
}

impl WalletArtifactOriginalsV1 {
    /// Deterministic bounded transport identities from the completed original graph.
    /// No path, key, installed identity, authority flag or wallet-open grant is encoded.
    ///
    /// # Errors
    /// Invalid/nonascending identity, excessive row count or JSON byte ceiling.
    pub fn transport_json(&self) -> io::Result<Vec<u8>> {
        let mut output = String::from(
            "{\"schema\":\"iroha.kagemusha.wallet-artifact-original-transport.v1\",\"walletOriginals\":",
        );
        rows(&mut output, &self.wallet_originals, WALLET_MAX_ROWS)?;
        output.push_str(",\"finalityOriginals\":");
        rows(&mut output, &self.finality_originals, FINALITY_MAX_ROWS)?;
        output.push_str("}\n");
        if output.len() > TRANSPORT_MAX_BYTES {
            return Err(invalid("original transport byte ceiling exceeded"));
        }
        Ok(output.into_bytes())
    }

    /// Export the three whole metadata originals into an existing private bundle root.
    /// The genuine compiler sink must be its `wallet-originals` child; the separately
    /// supplied finality graph must be its `finality-originals` child. Every referenced
    /// original is reauthenticated by a 64 KiB stream before metadata publication.
    /// Files are sealed 0400 before atomic no-replace publication. Exact retries retain
    /// original inodes; conflicts and partial publication evidence are preserved.
    ///
    /// # Errors
    /// Unsafe/missing/wrong ancestry, changed or absent original, bounds, conflict,
    /// unavailable storage/entropy or uncertain publication/durability. This export
    /// never authenticates signed genesis or grants Native installation authority.
    pub fn write_bundle_metadata(
        &self,
        root: impl AsRef<Path>,
        wallet: &DirectoryOriginalsV1,
        finality: &DirectoryOriginalsV1,
    ) -> io::Result<()> {
        if self.verifier_pack.is_empty()
            || self.verifier_pack.len() > PACK_MAX_BYTES
            || self.producer_inventory.is_empty()
            || self.producer_inventory.len() > CATALOG_MAX_BYTES
        {
            return Err(invalid("whole metadata original byte ceiling exceeded"));
        }
        let transport = self.transport_json()?;
        let directory = PrivateDirectory::open_exact(root)?;
        if wallet.root()? != directory.path().join("wallet-originals")
            || finality.root()? != directory.path().join("finality-originals")
        {
            return Err(invalid(
                "original graph does not belong to exact bundle roots",
            ));
        }
        for blob in &self.wallet_originals {
            wallet.verify_original(*blob)?;
        }
        for blob in &self.finality_originals {
            finality.verify_original(*blob)?;
        }
        directory.revalidate()?;
        publish_metadata(&directory, "verifier-pack.norito", &self.verifier_pack)?;
        publish_metadata(
            &directory,
            "producer-inventory.norito",
            &self.producer_inventory,
        )?;
        publish_metadata(&directory, "transport.json", &transport)?;
        wallet.root()?;
        finality.root()?;
        directory.sync()?;
        directory.revalidate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Explicit DATA fixtures exercise custody/export only, never compile or install
    // a production wallet. Production fields remain private and finish-only.
    fn fixture() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        DirectoryOriginalsV1,
        DirectoryOriginalsV1,
        WalletArtifactOriginalsV1,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("bundle");
        PrivateDirectory::open_or_create(&root).unwrap();
        PrivateDirectory::open_or_create(root.join("wallet-originals")).unwrap();
        PrivateDirectory::open_or_create(root.join("finality-originals")).unwrap();
        let root = root.canonicalize().unwrap();
        let mut wallet =
            DirectoryOriginalsV1::open_existing(root.join("wallet-originals"), 1024).unwrap();
        let mut finality =
            DirectoryOriginalsV1::open_existing(root.join("finality-originals"), 1024).unwrap();
        let one = BlobV1::of(b"actual wallet original");
        let two = BlobV1::of(b"independent finality original");
        wallet
            .store_original(one, b"actual wallet original")
            .unwrap();
        finality
            .store_original(two, b"independent finality original")
            .unwrap();
        let originals = WalletArtifactOriginalsV1 {
            verifier_pack: b"transport fixture whole pack".to_vec(),
            producer_inventory: b"transport fixture whole inventory".to_vec(),
            wallet_originals: vec![one],
            finality_originals: vec![two],
        };
        (temp, root, wallet, finality, originals)
    }

    #[test]
    fn identical_export_keeps_sealed_metadata_inodes_and_exact_originals() {
        let (_temp, root, wallet, finality, originals) = fixture();
        originals
            .write_bundle_metadata(&root, &wallet, &finality)
            .unwrap();
        let directory = PrivateDirectory::open_exact(&root).unwrap();
        let before = directory
            .open_retained_read_only("verifier-pack.norito", PACK_MAX_BYTES)
            .unwrap()
            .identity()
            .unwrap();
        originals
            .write_bundle_metadata(&root, &wallet, &finality)
            .unwrap();
        let after = directory
            .open_retained_read_only("verifier-pack.norito", PACK_MAX_BYTES)
            .unwrap()
            .identity()
            .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            std::fs::read(root.join("verifier-pack.norito")).unwrap(),
            originals.verifier_pack
        );
        assert_eq!(
            std::fs::read(root.join("transport.json")).unwrap(),
            originals.transport_json().unwrap()
        );
    }

    #[test]
    fn absent_actual_original_prevents_metadata_publication() {
        let (_temp, root, wallet, finality, mut originals) = fixture();
        originals.finality_originals[0] = BlobV1::of(b"missing authentic original");
        assert!(
            originals
                .write_bundle_metadata(&root, &wallet, &finality)
                .is_err()
        );
        assert!(!root.join("verifier-pack.norito").exists());
        assert!(!root.join("transport.json").exists());
    }

    #[test]
    fn wrong_graph_root_and_nonascending_transport_are_refused() {
        let (_temp, root, wallet, finality, mut originals) = fixture();
        assert!(
            originals
                .write_bundle_metadata(&root, &finality, &wallet)
                .is_err()
        );
        let one = originals.wallet_originals[0];
        originals.wallet_originals.push(one);
        assert!(originals.transport_json().is_err());
        originals.wallet_originals = vec![BlobV1 {
            bytes: 1,
            sha256: [0; 32],
        }];
        assert!(originals.transport_json().is_err());
        originals.wallet_originals = vec![BlobV1 {
            bytes: ORIGINAL_MAX_BYTES + 1,
            sha256: [1; 32],
        }];
        assert!(originals.transport_json().is_err());
    }

    #[test]
    fn conflicting_partial_export_is_retained_and_never_overwritten() {
        let (_temp, root, wallet, finality, originals) = fixture();
        let directory = PrivateDirectory::open_exact(&root).unwrap();
        let mut writer = directory
            .create_retained_private("producer-inventory.norito", 8)
            .unwrap();
        writer.write_all(b"conflict").unwrap();
        let sealed = writer.seal_read_only().unwrap();
        let before = sealed.identity().unwrap();
        assert!(
            originals
                .write_bundle_metadata(&root, &wallet, &finality)
                .is_err()
        );
        assert!(root.join("verifier-pack.norito").exists());
        assert!(!root.join("transport.json").exists());
        let existing = directory
            .open_retained_read_only("producer-inventory.norito", 8)
            .unwrap();
        assert_eq!(before, existing.identity().unwrap());
        assert_eq!(
            std::fs::read(root.join("producer-inventory.norito")).unwrap(),
            b"conflict"
        );
    }
}
