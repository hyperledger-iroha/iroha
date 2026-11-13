//! Export a retained checkpoint and its certified sparse store into a fresh private directory.
//!
//! The snapshot signature protects transported bytes, not consensus authority. Ordinary startup
//! still reconstructs the complete selected State, matches H+1's parent root, and replays H+1.

/// Bounded source-bound transport of an existing export; startup remains the authority gate.
pub mod transport;

use super::*;
use crate::sumeragi::checkpoint_prefix::{self, CheckpointPrefixExport};

/// Canonical transfer inventory; this DATA does not grant checkpoint admission authority.
#[derive(Debug, Clone, norito::json::JsonSerialize, norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct CheckpointTransferV1 {
    /// Exact transfer layout, with `kura` and `snapshots` child stores.
    pub version: u16,
    /// Independently selected chain label.
    pub chain_id: ChainId,
    /// Genesis-derived network identity.
    pub network_id: NetworkId,
    /// Published checkpoint height H.
    pub height: u64,
    /// Exact executed header hash at H.
    pub block_hash: HashOf<BlockHeader>,
    /// Complete published State root certified by H+1.
    pub published_state_root: iroha_data_model::sumeragi_finality::KeyedStateRoot,
    /// Public key for the unchanged signed snapshot artifacts; not a finality trust anchor.
    pub snapshot_verification_key: PublicKey,
}

/// Transfer failure retaining original source, codec and local resource errors.
#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    /// Original checkpoint validation or bounded snapshot read failed.
    #[error(transparent)]
    Snapshot(#[from] TryReadError),
    /// Original certified history or local finite resource owner failed.
    #[error(transparent)]
    Prefix(#[from] crate::execution_attempt::ExecutionAttemptError<checkpoint_prefix::Error>),
    /// Original destination or private-copy I/O failure.
    #[error("checkpoint transfer {path}: {source}")]
    Io {
        /// Exact original path.
        path: PathBuf,
        /// Original I/O cause.
        #[source]
        source: std::io::Error,
    },
    /// The requested retained checkpoint or export target does not satisfy the contract.
    #[error("checkpoint transfer: {0}")]
    Invalid(String),
    /// The canonical small transfer manifest could not be encoded.
    #[error(transparent)]
    Manifest(#[from] norito::json::Error),
}

fn io(path: &Path, source: std::io::Error) -> TransferError {
    TransferError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Copy an existing retained checkpoint and export its minimal certified prefix.
///
/// The output is a new private directory containing `kura/`, `snapshots/` and a last-written
/// `checkpoint-transfer.json`. Existing destinations are refused. No original store, checkpoint,
/// signing key, policy or chain State is changed. The joining node uses its independently pinned
/// genesis and normal snapshot configuration, including the returned snapshot verification key;
/// it must pass ordinary complete-State authentication before it executes or serves anything.
///
/// # Errors
/// Missing or substituted checkpoint, changed original files, invalid proof, occupied output,
/// local quota/refusal or incomplete durable copying. A directory without the manifest is incomplete.
#[allow(clippy::too_many_arguments)]
pub fn export_checkpoint(
    state: Arc<State>,
    snapshot: &Config,
    kura_config: &iroha_config::parameters::actual::Kura,
    verification_key: &PublicKey,
    height: u64,
    output: &Path,
) -> Result<CheckpointTransferV1, TransferError> {
    let buffer_budget = AllocationBudget::new(snapshot.max_read_buffer_bytes.get());
    let held = checkpoints::SnapshotCheckpoints::from_config(
        snapshot,
        verification_key.clone(),
        buffer_budget.clone(),
    )
    .held(state.chain_id_ref(), *state.network_id_ref())?;
    let checkpoint = held
        .iter()
        .find(|held| held.height == height)
        .ok_or_else(|| {
            TransferError::Invalid("requested height has no retained signed checkpoint".into())
        })?;
    let source_root = snapshot.store_dir.resolve_relative_path();
    let candidates =
        checkpoint_store::entries(&source_root).map_err(|error| io(&source_root, error))?;
    let mut matches = candidates
        .iter()
        .filter(|(candidate, _, _)| *candidate == height);
    let (_, committed_ms, source) = matches
        .next()
        .ok_or_else(|| TransferError::Invalid("retained checkpoint disappeared".into()))?;
    if matches.next().is_some() {
        return Err(TransferError::Invalid(
            "checkpoint height has ambiguous retained generations".into(),
        ));
    }
    let mut exporter = CheckpointPrefixExport::new(Arc::clone(&state), height)?;
    // The root is exclusive. Partial output may be inspected or removed by the operator, but
    // cannot overwrite a previous transfer or be confused with a completed package.
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(output).map_err(|error| io(output, error))?;
    let directory = iroha_fs::OwnerDirectory::open(output).map_err(|error| io(output, error))?;
    let snapshots = directory
        .create_child("snapshots")
        .map_err(|error| io(output, error))?;
    checkpoint_store::copy_to_container(
        snapshots.path(),
        source,
        checkpoint_store::CHECKPOINTS_DIR_NAME,
        height,
        *committed_ms,
        snapshot.max_payload_bytes,
        snapshot.merkle_chunk_size_bytes,
    )
    .map_err(|error| io(snapshots.path(), error))?;
    let mut destination_config = kura_config.clone();
    destination_config.store_dir = iroha_config::base::WithOrigin::inline(output.join("kura"));
    let verified = exporter.write_kura(&destination_config)?;
    if verified.checkpoint().header().hash() != checkpoint.block_hash {
        return Err(TransferError::Invalid(
            "retained checkpoint differs from certified history".into(),
        ));
    }
    let mut destination_snapshot = snapshot.clone();
    destination_snapshot.store_dir =
        iroha_config::base::WithOrigin::inline(output.join("snapshots"));
    let copied = checkpoints::SnapshotCheckpoints::from_config(
        &destination_snapshot,
        verification_key.clone(),
        buffer_budget,
    )
    .held(state.chain_id_ref(), *state.network_id_ref())?;
    if copied.as_slice() != [*checkpoint] {
        return Err(TransferError::Invalid(
            "copied checkpoint artifacts failed exact verification".into(),
        ));
    }
    let manifest = CheckpointTransferV1 {
        version: 1,
        chain_id: state.chain_id_ref().clone(),
        network_id: *state.network_id_ref(),
        height,
        block_hash: checkpoint.block_hash,
        published_state_root: verified.published_root(),
        snapshot_verification_key: verification_key.clone(),
    };
    let bytes = norito::json::to_vec(&manifest)?;
    directory
        .write_private_atomic(
            "checkpoint-transfer.json",
            &bytes,
            iroha_fs::PublishMode::CreateNew,
        )
        .map_err(|error| io(output, error))?;
    directory.revalidate().map_err(|error| io(output, error))?;
    Ok(manifest)
}
