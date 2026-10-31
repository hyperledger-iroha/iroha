//! Offline replay check of a copied store with this build (`iroha3d --check-replay`,
//! `specs/live_runtime_upgrades.md` §9.2).
//!
//! [`copy_store`] copies an original store, which it only reads, file by file into a temporary
//! directory and opens the copy as a temporary Strict Kura: blocks, native execution context
//! records, lane stores and lane geometry journals all come along, so the copy restores and
//! replays exactly as the original would at startup. The caller builds the State on that copy as
//! a Strict startup builds it: restored from the newest authenticated snapshot, or empty to
//! re-execute signed genesis. [`check_replay`] then re-executes every copied block above that
//! State through the serialized startup executor and its certificate checks, and compares each
//! execution result this build produces with the stored certificate. It stops at the first block
//! whose result differs and names the first differing commitment component
//! ([`ResultComponent`]). A body the store no longer holds is missing history
//! ([`ReplayCheckOutcome::MissingHistory`]), never a divergence. Blocks at or below a configured
//! upgrade checkpoint are never re-executed. Nothing is written outside the temporary copy.

use std::{
    io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

use iroha_allocation::AllocationBudget;
use iroha_config::parameters::actual::{Kura as KuraConfig, LaneConfig};
use iroha_crypto::{Hash, MerkleTreeCommitment};
use iroha_data_model::{
    account::AccountId,
    parameter::system::ConsensusMode,
    sumeragi_finality::{
        CertifiedResultV1, ExecutionResultCommitment, SccpFinalityHeaderV1,
        certified_result::X_LEN, result_of_preimage,
    },
};
use iroha_sumeragi::{message::Qc, types::Hash32};

use super::super::{
    IsolatedSource, NodeError, PrepareInputs, Prepared, prepare_genesis, prepare_restored,
};
use crate::{
    execution_attempt::ExecutionAttemptError,
    kura::Kura,
    state::{State, WorldReadOnly as _, WorldStateVerificationError},
    sumeragi::{
        driver::traits::PublicationError,
        executor::StateExecutor,
        startup::{self, GENESIS_HEIGHT, StartupError},
    },
};

/// One part of a block's certified execution result `X ‖ body` (`specs/sumeragi.md` §4.1).
///
/// Variants are declared in the order a replay check reports the first difference: whether the
/// block still executes, the World it executed on, its transactions and their results, its
/// effects, the World after it, its schedule, then the finality header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultComponent {
    /// This build rejects a block the network certified.
    Validity,
    /// The executed height.
    Height,
    /// Complete World state root the block executed on.
    ParentWorldStateRoot,
    /// Network-input Merkle commitment: the block's transactions.
    TransactionInputs,
    /// Length and hash of the result-bearing block wire: every transaction result and output.
    TransactionResults,
    /// Typed-output Merkle commitment, including internal invocations.
    TransactionOutputs,
    /// Merkle root and count of the events the execution emitted.
    Events,
    /// Root of the witnessed pre-state values of the keys the block changed.
    WitnessedParentStateRoot,
    /// Post-state root of the witnessed writes.
    WitnessedPostStateRoot,
    /// Root of the canonical last-write-wins witnessed writes.
    OrdinaryWritesRoot,
    /// Complete World state root after the block.
    WorldStateRoot,
    /// Native lane context-set proof.
    NativeLanes,
    /// Current epoch context and successor schedule.
    Schedule,
    /// Finalized beacon pulse the execution consumed.
    Beacon,
    /// Fixed SCCP finality header `X`.
    FinalityHeader,
    /// The whole result, when its parts cannot be compared.
    Result,
    /// A cold capture of the replayed World differs from the root its accumulator committed.
    WorldStateAccumulator,
}

impl ResultComponent {
    /// Stable `snake_case` name used in reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Validity => "validity",
            Self::Height => "height",
            Self::ParentWorldStateRoot => "parent_world_state_root",
            Self::TransactionInputs => "transaction_inputs",
            Self::TransactionResults => "transaction_results",
            Self::TransactionOutputs => "transaction_outputs",
            Self::Events => "events",
            Self::WitnessedParentStateRoot => "witnessed_parent_state_root",
            Self::WitnessedPostStateRoot => "witnessed_post_state_root",
            Self::OrdinaryWritesRoot => "ordinary_writes_root",
            Self::WorldStateRoot => "world_state_root",
            Self::NativeLanes => "native_lanes",
            Self::Schedule => "schedule",
            Self::Beacon => "beacon",
            Self::FinalityHeader => "finality_header",
            Self::Result => "result",
            Self::WorldStateAccumulator => "world_state_accumulator",
        }
    }
}

/// One differing component with the certified value and the value this build produced.
///
/// Hashes render as lowercase hex, Merkle commitments as `root (count leaves)`, and composite
/// components (schedule, beacon, native lanes, finality header) as the hash of their encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentDifference {
    /// The component.
    pub component: ResultComponent,
    /// The value the stored certificate commits to.
    pub expected: String,
    /// The value this build produced.
    pub actual: String,
}

/// The first block whose replayed execution result differs from its certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayDivergence {
    /// Height of the block.
    pub height: u64,
    /// The first differing component, with both values.
    pub first: ComponentDifference,
    /// Every differing component in report order, starting with [`Self::first`]'s.
    pub components: Vec<ResultComponent>,
}

impl ReplayDivergence {
    /// Compare the result preimage `X ‖ body` stored for block `height`, certified as `R =
    /// certified`, with the result this build `produced`. A stored preimage that cannot be read
    /// compares as a whole ([`ResultComponent::Result`]).
    #[must_use]
    pub fn between(
        height: u64,
        stored_preimage: &[u8],
        certified: Hash32,
        produced: &CertifiedResultV1,
    ) -> Self {
        let differences = decode_result_preimage(stored_preimage)
            .map(|stored| result_differences(&stored, produced))
            .unwrap_or_default();
        let components: Vec<_> = differences
            .iter()
            .map(|difference| difference.component)
            .collect();
        match differences.into_iter().next() {
            Some(first) => Self {
                height,
                first,
                components,
            },
            None => Self::whole(
                height,
                certified.to_string(),
                produced.result().map_or_else(
                    |error| format!("unencodable result: {error}"),
                    |result| result.to_string(),
                ),
            ),
        }
    }

    /// A certified block this build rejects.
    #[must_use]
    pub fn rejected(height: u64) -> Self {
        Self {
            height,
            first: ComponentDifference {
                component: ResultComponent::Validity,
                expected: "valid".to_owned(),
                actual: "rejected".to_owned(),
            },
            components: vec![ResultComponent::Validity],
        }
    }

    /// A block whose results differ as a whole, with no component breakdown.
    fn whole(height: u64, expected: String, actual: String) -> Self {
        Self {
            height,
            first: ComponentDifference {
                component: ResultComponent::Result,
                expected,
                actual,
            },
            components: vec![ResultComponent::Result],
        }
    }
}

/// Where a replay check started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayCheckStart {
    /// An empty State: signed genesis is re-executed first.
    Genesis,
    /// A State restored from an authenticated snapshot at this height.
    Snapshot(u64),
}

impl ReplayCheckStart {
    /// Height of the State the replay starts from (0 before signed genesis).
    #[must_use]
    pub const fn height(self) -> u64 {
        match self {
            Self::Genesis => 0,
            Self::Snapshot(height) => height,
        }
    }
}

/// The verdict of a replay check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayCheckOutcome {
    /// Every replayed block reproduced its certified result.
    Matched,
    /// The first block whose replayed result differs from its certificate.
    Diverged(Box<ReplayDivergence>),
    /// The store holds no body at this height (absent or pruned); every lower block matched.
    MissingHistory {
        /// The first height without a stored body.
        height: u64,
    },
}

/// The result of [`check_replay`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayCheckReport {
    /// Where the replay started.
    pub start: ReplayCheckStart,
    /// Height of the State after the last block this build reproduced; the start height when it
    /// reproduced none (0 when signed genesis itself diverged).
    pub replayed_to: u64,
    /// The verdict.
    pub outcome: ReplayCheckOutcome,
}

/// Why a replay check could not reach a verdict. A different result is not an error: it is a
/// [`ReplayCheckOutcome::Diverged`] report.
#[derive(Debug, thiserror::Error)]
pub enum ReplayCheckError {
    /// The replay State does not own a temporary copy.
    #[error("replay copy: {0}")]
    Destination(&'static str),
    /// An entry of the original store could not be copied.
    #[error("cannot copy {}: {error}", path.display())]
    Copy {
        /// The entry.
        path: PathBuf,
        /// The failure.
        #[source]
        error: io::Error,
    },
    /// The copy does not open as a Strict store under the configured lane catalog.
    #[error("cannot open the replay copy: {0}")]
    Open(#[source] Box<crate::kura::Error>),
    /// The State starts below the upgrade checkpoint, whose history this release never
    /// re-executes.
    #[error(
        "replay would start at height {start}, below the upgrade checkpoint at height {checkpoint}: blocks up to the checkpoint are the previous release's history and are never re-executed"
    )]
    BelowCheckpoint {
        /// Height of the State.
        start: u64,
        /// Height of the upgrade checkpoint.
        checkpoint: u64,
    },
    /// Signed genesis or the restored State could not be prepared for replay.
    #[error("replay start: {0}")]
    Start(#[source] Box<NodeError>),
    /// A copied block could not be read back.
    #[error("replay read, height {height}: {error}")]
    Read {
        /// Height of the block.
        height: u64,
        /// The read failure.
        #[source]
        error: Box<ExecutionAttemptError<std::io::Error>>,
    },
    /// A block could not be replayed for a reason other than a different result.
    #[error("replay, height {height}: {error}")]
    Replay {
        /// Height of the block.
        height: u64,
        /// The execution or publication failure.
        #[source]
        error: Box<PublicationError>,
    },
    /// The diverged execution or its stored certificate could not be inspected.
    #[error("divergence diagnosis, height {height}: {reason}")]
    Diagnosis {
        /// Height of the block.
        height: u64,
        /// Why.
        reason: String,
    },
    /// The final cold World check could not run.
    #[error("final World state check: {0}")]
    StateRoot(#[source] Box<WorldStateVerificationError>),
}

/// Copy the store `source` holds into a fresh directory in the system temporary directory and
/// open the copy as temporary staging: a Strict Kura under `configured_lane_catalog` that removes
/// the copy when dropped. Every regular file and directory is copied except the store-root lock,
/// so blocks, native execution context records, lane stores and lane geometry journals come
/// along. Keep `source` open in emergency-Fast mode while copying: its store-root lock keeps a
/// node from starting on the original, and nothing in the original is written. The copy keeps
/// `config` except that it opens in Strict mode, without a disk budget or debug block output.
/// `progress` receives the bytes copied so far.
///
/// # Errors
/// An entry of the store is neither a regular file nor a directory, a file cannot be copied, or
/// the copy does not open under the configured catalog.
pub fn copy_store(
    source: &Kura,
    config: &KuraConfig,
    lane_config: &LaneConfig,
    configured_lane_catalog: &iroha_data_model::nexus::LaneCatalog,
    progress: &mut dyn FnMut(u64),
) -> Result<Arc<Kura>, ReplayCheckError> {
    let directory = tempfile::Builder::new()
        .prefix("iroha-replay-check-")
        .tempdir()
        .map_err(copy_failure(&std::env::temp_dir()))?;
    let mut copied = 0;
    copy_tree(
        &source.store_root(),
        directory.path(),
        true,
        &mut copied,
        progress,
    )?;
    let mut strict = config.clone();
    strict.init_mode = iroha_config::kura::InitMode::Strict;
    strict.max_disk_usage_bytes = iroha_config::base::util::Bytes(0);
    strict.debug_output_new_blocks = false;
    Kura::adopt_temporary_with_configured_lane_catalog(
        &strict,
        lane_config,
        configured_lane_catalog,
        directory,
    )
    .map_err(|error| ReplayCheckError::Open(Box::new(error)))
}

fn copy_tree(
    from: &Path,
    to: &Path,
    root: bool,
    copied: &mut u64,
    progress: &mut dyn FnMut(u64),
) -> Result<(), ReplayCheckError> {
    for entry in std::fs::read_dir(from).map_err(copy_failure(from))? {
        let entry = entry.map_err(copy_failure(from))?;
        let source = entry.path();
        // The copy takes its own lock when it opens.
        if root && entry.file_name() == crate::kura::STORE_ROOT_LOCK_FILE_NAME {
            continue;
        }
        let target = to.join(entry.file_name());
        let kind = entry.file_type().map_err(copy_failure(&source))?;
        if kind.is_dir() {
            let permissions = entry
                .metadata()
                .map_err(copy_failure(&source))?
                .permissions();
            std::fs::create_dir(&target).map_err(copy_failure(&target))?;
            copy_tree(&source, &target, false, copied, progress)?;
            std::fs::set_permissions(&target, permissions).map_err(copy_failure(&target))?;
        } else if kind.is_file() {
            *copied += std::fs::copy(&source, &target).map_err(copy_failure(&source))?;
            progress(*copied);
        } else {
            return Err(ReplayCheckError::Copy {
                path: source,
                error: io::Error::new(
                    io::ErrorKind::InvalidData,
                    "a store entry is neither a regular file nor a directory",
                ),
            });
        }
    }
    Ok(())
}

fn copy_failure(path: &Path) -> impl FnOnce(io::Error) -> ReplayCheckError + '_ {
    move |error| ReplayCheckError::Copy {
        path: path.to_path_buf(),
        error,
    }
}

/// Inputs of [`check_replay`].
pub struct ReplayCheckInputs {
    /// State on the temporary copy, built as a Strict startup builds it: restored from an
    /// authenticated snapshot, or empty to re-execute signed genesis.
    pub state: Arc<State>,
    /// Lane block frames to replay from (`<store>/lanes` of the copy), only ever read.
    pub source_lanes: PathBuf,
    /// The genesis account.
    pub genesis_account: AccountId,
    /// Consensus mode of the signed genesis.
    pub consensus_mode: ConsensusMode,
}

/// Re-execute every block the copy holds above `inputs.state` with this build and compare each
/// produced execution result with the stored certificate. Signed genesis is re-executed first
/// when the State is empty. Each block's certificate is verified as at startup, then the block is
/// executed and, when its result matches, applied; the first different result ends the check
/// with a [`ReplayCheckOutcome::Diverged`] report naming the first differing component, and the
/// first absent body with [`ReplayCheckOutcome::MissingHistory`]. After the last block the
/// replayed World is checked against a cold capture, as at startup. `progress` receives each
/// reproduced height and the copy's height.
///
/// # Errors
/// See [`ReplayCheckError`]; in particular a State below the configured upgrade checkpoint is
/// refused before anything executes.
pub fn check_replay(
    inputs: ReplayCheckInputs,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<ReplayCheckReport, ReplayCheckError> {
    let ReplayCheckInputs {
        state,
        source_lanes,
        genesis_account,
        consensus_mode,
    } = inputs;
    let kura = state.kura_handle();
    if !kura.is_temporary_staging() {
        return Err(ReplayCheckError::Destination(
            "the replay State must own a temporary store",
        ));
    }
    let restored = startup::applied_height(&state);
    if let Some(checkpoint) = kura.upgrade_checkpoint()
        && restored < checkpoint.height
    {
        return Err(ReplayCheckError::BelowCheckpoint {
            start: restored,
            checkpoint: checkpoint.height,
        });
    }
    let start = if restored == 0 {
        ReplayCheckStart::Genesis
    } else {
        ReplayCheckStart::Snapshot(restored)
    };
    let stored = as_height(kura.blocks_count());
    if stored == 0 {
        return Ok(ReplayCheckReport {
            start,
            replayed_to: 0,
            outcome: ReplayCheckOutcome::Matched,
        });
    }
    // Process-wide status observations stay local, as for every isolated replay; the executor
    // isolates its own thread.
    let _observations = crate::status::isolate_replay_observations();
    let (events, _) = tokio::sync::broadcast::channel(1);
    let prepare_inputs = PrepareInputs {
        state: Arc::clone(&state),
        events,
        genesis: None,
        genesis_account,
        consensus_mode,
    };
    let isolated = Some(IsolatedSource {
        lanes: source_lanes,
        fastpq_target: None,
    });
    let prepared = if restored == 0 {
        match prepare_genesis(prepare_inputs, isolated) {
            Ok(prepared) => prepared,
            Err(NodeError::Startup(StartupError::GenesisDiverged)) => {
                // TODO: name the differing component of a diverged genesis; startup compares
                // only the complete certificates and does not hand back the produced result.
                let certified = stored_result_preimage(&kura, &state.ivm_execution_budget(), 1)
                    .ok()
                    .and_then(|preimage| result_of_preimage(&preimage))
                    .map_or_else(|| "unreadable".to_owned(), |result| result.to_string());
                return Ok(ReplayCheckReport {
                    start,
                    replayed_to: 0,
                    outcome: ReplayCheckOutcome::Diverged(Box::new(ReplayDivergence::whole(
                        GENESIS_HEIGHT,
                        certified,
                        "a different genesis result".to_owned(),
                    ))),
                });
            }
            Err(error) => return Err(ReplayCheckError::Start(Box::new(error))),
        }
    } else {
        prepare_restored(prepare_inputs, restored, isolated)
            .map_err(|error| ReplayCheckError::Start(Box::new(error)))?
    };
    let Prepared {
        blocks,
        mut executor,
        lane_stores,
        ..
    } = prepared;
    let mut replayed_to = restored.max(GENESIS_HEIGHT);
    if restored == 0 {
        progress(GENESIS_HEIGHT, stored);
    }
    for height in replayed_to + 1..=stored {
        let read = match blocks.committed_body(height) {
            Ok(read) => read,
            Err(_) if body_is_absent(&kura, &state.ivm_execution_budget(), height) => None,
            Err(error) => {
                return Err(ReplayCheckError::Read {
                    height,
                    error: Box::new(error),
                });
            }
        };
        let Some((body, qc)) = read else {
            // An absent or pruned body is missing history, never a divergence.
            return Ok(ReplayCheckReport {
                start,
                replayed_to,
                outcome: ReplayCheckOutcome::MissingHistory { height },
            });
        };
        let diverged = match executor.prepare_replay(&body, &qc) {
            Ok(Some(result)) if result == qc.result => None,
            Ok(Some(_)) => Some(produced_divergence(&state, &executor, height, &qc)?),
            Ok(None) => Some(ReplayDivergence::rejected(height)),
            Err(error) => {
                return Err(ReplayCheckError::Replay {
                    height,
                    error: Box::new(error),
                });
            }
        };
        if let Some(divergence) = diverged {
            return Ok(ReplayCheckReport {
                start,
                replayed_to,
                outcome: ReplayCheckOutcome::Diverged(Box::new(divergence)),
            });
        }
        executor
            .replay(&body, &qc)
            .map_err(|error| ReplayCheckError::Replay {
                height,
                error: Box::new(error),
            })?;
        // No lane runner reconciles retirement during replay, as at startup.
        let lanes = state.view().world().sumeragi_lanes().clone();
        lane_stores.release_retired(&lanes);
        replayed_to = height;
        progress(height, stored);
    }
    drop(executor);
    let outcome = match state.verify_world_state_accumulator() {
        Ok(_) => ReplayCheckOutcome::Matched,
        Err(WorldStateVerificationError::Mismatch {
            stored_root,
            captured_root,
            ..
        }) => ReplayCheckOutcome::Diverged(Box::new(ReplayDivergence {
            height: replayed_to,
            first: ComponentDifference {
                component: ResultComponent::WorldStateAccumulator,
                expected: stored_root.to_string(),
                actual: captured_root.to_string(),
            },
            components: vec![ResultComponent::WorldStateAccumulator],
        })),
        Err(error) => return Err(ReplayCheckError::StateRoot(Box::new(error))),
    };
    Ok(ReplayCheckReport {
        start,
        replayed_to,
        outcome,
    })
}

/// Every component in which `actual` differs from `expected`, in [`ResultComponent`] order.
#[must_use]
pub fn result_differences(
    expected: &CertifiedResultV1,
    actual: &CertifiedResultV1,
) -> Vec<ComponentDifference> {
    let mut differences = Vec::new();
    let (left, right) = (expected.commitment(), actual.commitment());
    let (before, after) = (&left.execution, &right.execution);
    let root = |hash: &Hash| hash.to_string();
    compare(
        &mut differences,
        ResultComponent::Height,
        &left.height,
        &right.height,
        u64::to_string,
    );
    compare(
        &mut differences,
        ResultComponent::ParentWorldStateRoot,
        &before.parent_world_state_root,
        &after.parent_world_state_root,
        root,
    );
    compare(
        &mut differences,
        ResultComponent::TransactionInputs,
        &before.transaction_input_commitment,
        &after.transaction_input_commitment,
        merkle,
    );
    compare(
        &mut differences,
        ResultComponent::TransactionResults,
        &(
            before.executed_block_wire_len,
            before.executed_block_wire_hash,
        ),
        &(
            after.executed_block_wire_len,
            after.executed_block_wire_hash,
        ),
        |&(length, hash)| format!("{hash} ({length} bytes)"),
    );
    compare(
        &mut differences,
        ResultComponent::TransactionOutputs,
        &before.transaction_output_commitment,
        &after.transaction_output_commitment,
        merkle,
    );
    compare(
        &mut differences,
        ResultComponent::Events,
        &before.event_commitment,
        &after.event_commitment,
        merkle,
    );
    compare(
        &mut differences,
        ResultComponent::WitnessedParentStateRoot,
        &before.parent_state_root,
        &after.parent_state_root,
        root,
    );
    compare(
        &mut differences,
        ResultComponent::WitnessedPostStateRoot,
        &before.post_state_root,
        &after.post_state_root,
        root,
    );
    compare(
        &mut differences,
        ResultComponent::OrdinaryWritesRoot,
        &before.ordinary_writes_root,
        &after.ordinary_writes_root,
        root,
    );
    compare(
        &mut differences,
        ResultComponent::WorldStateRoot,
        &before.world_state_root,
        &after.world_state_root,
        root,
    );
    compare(
        &mut differences,
        ResultComponent::NativeLanes,
        &left.native_lanes,
        &right.native_lanes,
        digest,
    );
    compare(
        &mut differences,
        ResultComponent::Schedule,
        &left.schedule,
        &right.schedule,
        digest,
    );
    compare(
        &mut differences,
        ResultComponent::Beacon,
        &left.beacon,
        &right.beacon,
        |beacon| beacon.as_ref().map_or_else(|| "none".to_owned(), digest),
    );
    compare(
        &mut differences,
        ResultComponent::FinalityHeader,
        expected.header().as_bytes(),
        actual.header().as_bytes(),
        |header| Hash::new(header).to_string(),
    );
    differences
}

fn compare<T: PartialEq + ?Sized>(
    differences: &mut Vec<ComponentDifference>,
    component: ResultComponent,
    expected: &T,
    actual: &T,
    render: impl Fn(&T) -> String,
) {
    if expected != actual {
        differences.push(ComponentDifference {
            component,
            expected: render(expected),
            actual: render(actual),
        });
    }
}

fn merkle<T>(commitment: &Option<MerkleTreeCommitment<T>>) -> String {
    commitment.as_ref().map_or_else(
        || "none".to_owned(),
        |commitment| format!("{} ({} leaves)", commitment.root(), commitment.leaf_count()),
    )
}

// The values were canonically encoded inside the result preimage, so encoding cannot fail.
fn digest<T: norito::core::SerializePayload>(value: &T) -> String {
    Hash::new(norito::codec::Encode::encode(value)).to_string()
}

/// Read a stored result preimage `X ‖ body`. The certificate's verification already applied the
/// scope rule and the header bindings, so only the two parts are decoded.
fn decode_result_preimage(preimage: &[u8]) -> Option<CertifiedResultV1> {
    if preimage.len() < X_LEN {
        return None;
    }
    let (header, body) = preimage.split_at(X_LEN);
    let header = SccpFinalityHeaderV1::parse(header).ok()?;
    let body = ExecutionResultCommitment::decode(body).ok()?;
    Some(CertifiedResultV1::from_parts_unvalidated(header, body))
}

fn produced_divergence(
    state: &State,
    executor: &StateExecutor,
    height: u64,
    qc: &Qc,
) -> Result<ReplayDivergence, ReplayCheckError> {
    let produced = executor
        .executed_result(qc.block_hash)
        .map_err(|reason| ReplayCheckError::Diagnosis { height, reason })?;
    let preimage = stored_result_preimage(state.kura(), &state.ivm_execution_budget(), height)?;
    Ok(ReplayDivergence::between(
        height, &preimage, qc.result, &produced,
    ))
}

/// Whether the copy holds no body at `height` (an absent or pruned slot), as opposed to a body it
/// cannot read or authenticate.
fn body_is_absent(kura: &Kura, budget: &AllocationBudget, height: u64) -> bool {
    usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .is_some_and(|index| matches!(kura.get_block(index, budget), Ok(None)))
}

fn stored_result_preimage(
    kura: &Kura,
    budget: &AllocationBudget,
    height: u64,
) -> Result<Vec<u8>, ReplayCheckError> {
    let diagnosis = |reason: String| ReplayCheckError::Diagnosis { height, reason };
    let index = usize::try_from(height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| diagnosis("the height exceeds this host's index range".to_owned()))?;
    let block = kura
        .get_block(index, budget)
        .map_err(|error| diagnosis(error.to_string()))?
        .ok_or_else(|| diagnosis("the stored block is unavailable".to_owned()))?;
    block
        .commit_certificate()
        .map(|certificate| certificate.result_preimage().to_vec())
        .ok_or_else(|| diagnosis("the stored block has no certificate".to_owned()))
}

// Kura heights are `usize` on the host; every supported host is at most 64-bit.
fn as_height(height: usize) -> u64 {
    u64::try_from(height).expect("block heights fit u64")
}

#[cfg(test)]
mod tests;
