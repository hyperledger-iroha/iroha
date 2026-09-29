/// Hard limit for one startup replay WSV checkpoint.
const MAX_WSV_CHECKPOINT_BYTES: usize = 64 * 1024;
/// Hard limit for one startup replay commit manifest.
const MAX_COMMIT_MANIFEST_BYTES: usize = 64 * 1024;
#[derive(Debug, Clone)]
struct StableSidecarDirectoryMetadata {
    expected_path: PathBuf,
    canonical_path: Option<PathBuf>,
    metadata: Option<SecureMetadata>,
}
#[derive(Debug, Clone)]
struct StableSidecarMetadata {
    canonical_path: PathBuf,
    file: SecureMetadata,
    directory: SecureMetadata,
}
#[derive(Debug)]
struct StableSidecarRead {
    bytes: Vec<u8>,
    bytes_hash: Hash,
    metadata: StableSidecarMetadata,
}
#[derive(Debug)]
struct BoundProgressDirectory {
    expected_path: PathBuf,
    canonical_path: PathBuf,
    /// Entry name relative to the next bound ancestor; `None` only for Kura root.
    entry_name: Option<std::ffi::OsString>,
    file: std::fs::File,
    metadata: SecureMetadata,
}
#[derive(Debug)]
struct BoundProgressNamespace {
    data_path: PathBuf,
    index_path: PathBuf,
    /// Bound directories in durability order: immediate parent through Kura root.
    directories: Vec<BoundProgressDirectory>,
}
