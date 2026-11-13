//! Original exported bytes cross the bounded transport, then pass the unchanged native startup.

use super::*;
use std::{io::ErrorKind, sync::Arc};

fn limits() -> TransferLimits {
    TransferLimits {
        max_artifacts: NonZeroUsize::new(128).unwrap(),
        max_total_bytes: NonZeroU64::new(512 * 1024 * 1024).unwrap(),
        max_artifact_bytes: NonZeroU64::new(256 * 1024 * 1024).unwrap(),
        chunk_bytes: NonZeroUsize::new(16 * 1024).unwrap(),
    }
}
fn run(operation: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(operation)
        .unwrap()
        .join()
        .unwrap_or_else(|failure| std::panic::resume_unwind(failure));
}
fn provider(
    fixture: &crate::snapshot::tests::TransferredCheckpointFixture,
    budget: &AllocationBudget,
) -> ExportedCheckpoint {
    ExportedCheckpoint::open(
        &fixture.directory.path().join("transfer"),
        fixture.source.state().chain_id_ref(),
        *fixture.source.state().network_id_ref(),
        limits(),
        budget,
    )
    .unwrap()
}

#[test]
fn portable_transfer_transport_restores_complete_state_and_replays_real_successor() {
    run(|| {
        let fixture = crate::snapshot::tests::transferred_checkpoint_fixture();
        let budget = AllocationBudget::new(32 * 1024 * 1024);
        let mut source = provider(&fixture, &budget);
        let descriptor = source.descriptor().unwrap();
        let encoded = norito::encode_canonical(&descriptor).unwrap();
        assert_eq!(
            norito::decode_canonical::<PackageDescriptorV1>(&encoded).unwrap(),
            descriptor
        );
        let first = source.artifact(0).unwrap();
        assert_eq!(
            norito::decode_canonical::<ArtifactV1>(&norito::encode_canonical(&first).unwrap())
                .unwrap(),
            first
        );
        let output = fixture.directory.path().join("received");
        let received = receive_checkpoint(
            &mut source,
            &descriptor,
            fixture.source.state().chain_id_ref(),
            *fixture.source.state().network_id_ref(),
            limits(),
            &budget,
            &output,
        )
        .unwrap();
        assert_eq!(
            received.block_hash,
            fixture.source.committed(2).block_hash()
        );
        assert_eq!(
            std::fs::read(output.join(TRANSFER_MARKER)).unwrap(),
            std::fs::read(
                fixture
                    .directory
                    .path()
                    .join("transfer")
                    .join(TRANSFER_MARKER)
            )
            .unwrap()
        );
        assert!(
            receive_checkpoint(
                &mut source,
                &descriptor,
                fixture.source.state().chain_id_ref(),
                *fixture.source.state().network_id_ref(),
                limits(),
                &budget,
                &output,
            )
            .is_err()
        );
        let mut kura_config = fixture.kura_config.clone();
        kura_config.store_dir = iroha_config_base::WithOrigin::inline(output.join("kura"));
        let (kura, _) = crate::kura::Kura::open_test_kura_with_configured_lane_config(
            &kura_config,
            &iroha_config::parameters::actual::LaneConfig::default(),
        )
        .unwrap();
        assert_eq!(kura.checkpoint_prefix_height(), Some(2));
        assert!(
            kura.native_frame_read(2, fixture.source.committed(2).block_hash())
                .unwrap()
                .is_none()
        );
        let mut restored = crate::snapshot::tests::strict_snapshot_read_for_custody_test(
            &output.join("snapshots"),
            fixture.source.state(),
            &kura,
            &fixture.key,
            &crate::snapshot::tests::snapshot_read_budget_for_testing(),
            &|_| Ok(()),
        )
        .unwrap();
        crate::snapshot::tests::authenticate_native_snapshot(fixture.source.state(), &mut restored);
        assert_eq!(restored.committed_height(), 2);
        let restored: Arc<crate::state::State> = Arc::from(restored);
        let _prepared = crate::sumeragi::node::prepare(crate::sumeragi::node::PrepareInputs {
            state: restored.clone(),
            events: tokio::sync::broadcast::channel(64).0,
            genesis: None,
            genesis_account: fixture.source.genesis_account().clone(),
            consensus_mode: iroha_data_model::parameter::system::ConsensusMode::Permissioned,
        })
        .unwrap();
        assert_eq!(restored.committed_height(), 3);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(),
            crate::snapshot::canonical_state_snapshot_hash(fixture.source.state()).unwrap()
        );
    });
}

#[derive(Clone, Copy)]
enum Fault {
    CorruptChunk,
    WrongIndex,
    Truncated,
    Finish,
}
struct Changed<'a> {
    source: &'a mut ExportedCheckpoint,
    fault: Fault,
}

struct ChangedDestination<'a> {
    source: &'a mut ExportedCheckpoint,
    destination: PathBuf,
    first_length: u64,
    append: bool,
    mutated: bool,
}

impl CheckpointTransport for ChangedDestination<'_> {
    fn descriptor(&mut self) -> Attempt<PackageDescriptorV1> {
        self.source.descriptor()
    }
    fn artifact(&mut self, index: u64) -> Attempt<ArtifactV1> {
        self.source.artifact(index)
    }
    fn read_range(&mut self, index: u64, offset: u64, output: &mut [u8]) -> Attempt<()> {
        use std::io::{Read as _, Seek as _, SeekFrom};
        self.source.read_range(index, offset, output)?;
        if index == 0 && offset > 0 && offset + output.len() as u64 == self.first_length {
            // The receiver has written earlier chunks but has not sealed this file yet.
            // Do not alter transport bytes: their hash must still match the original artifact.
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.destination)
                .map_err(io)?;
            if self.append {
                file.seek(SeekFrom::Start(self.first_length)).map_err(io)?;
                file.write_all(b"unexpected appended bytes").map_err(io)?;
            } else {
                let mut byte = [0];
                file.read_exact(&mut byte).map_err(io)?;
                byte[0] ^= 1;
                file.seek(SeekFrom::Start(0)).map_err(io)?;
                file.write_all(&byte).map_err(io)?;
            }
            file.sync_all().map_err(io)?;
            self.mutated = true;
        }
        Ok(())
    }
    fn finish(&mut self, expected: &PackageDescriptorV1) -> Attempt<()> {
        self.source.finish(expected)
    }
}

#[test]
fn portable_transfer_transport_rejects_destination_changes_before_first_sealed_snapshot() {
    run(|| {
        let fixture = crate::snapshot::tests::transferred_checkpoint_fixture();
        let budget = AllocationBudget::new(32 * 1024 * 1024);
        let mut source = provider(&fixture, &budget);
        let descriptor = source.descriptor().unwrap();
        let first_length = source.artifact(0).unwrap().length;
        let mut policy = limits();
        policy.chunk_bytes = NonZeroUsize::new(1024).unwrap();
        assert!(first_length > policy.chunk_bytes.get() as u64);
        for append in [false, true] {
            let output = fixture
                .directory
                .path()
                .join(format!("destination-changed-{append}"));
            let mut changed = ChangedDestination {
                source: &mut source,
                destination: output.join(relative(&descriptor, 0).unwrap()),
                first_length,
                append,
                mutated: false,
            };
            let result = receive_checkpoint(
                &mut changed,
                &descriptor,
                fixture.source.state().chain_id_ref(),
                *fixture.source.state().network_id_ref(),
                policy,
                &budget,
                &output,
            );
            assert!(
                changed.mutated,
                "fixture must alter a partially received file"
            );
            assert!(matches!(
                result,
                Err(ExecutionAttemptError::Rejected(Error::Invalid(_)))
            ));
            assert!(!output.join(TRANSFER_MARKER).exists());
        }
    });
}
impl CheckpointTransport for Changed<'_> {
    fn descriptor(&mut self) -> Attempt<PackageDescriptorV1> {
        self.source.descriptor()
    }
    fn artifact(&mut self, index: u64) -> Attempt<ArtifactV1> {
        let mut value = self.source.artifact(index)?;
        if matches!(self.fault, Fault::WrongIndex) {
            value.index += 1;
        }
        Ok(value)
    }
    fn read_range(&mut self, index: u64, offset: u64, output: &mut [u8]) -> Attempt<()> {
        self.source.read_range(index, offset, output)?;
        match self.fault {
            Fault::CorruptChunk => output[0] ^= 1,
            Fault::Truncated => {
                return Err(io(std::io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "original range ended",
                )));
            }
            _ => {}
        }
        Ok(())
    }
    fn finish(&mut self, expected: &PackageDescriptorV1) -> Attempt<()> {
        self.source.finish(expected)?;
        if matches!(self.fault, Fault::Finish) {
            return Err(io(std::io::Error::new(
                ErrorKind::WouldBlock,
                "original source busy",
            )));
        }
        Ok(())
    }
}

#[test]
fn portable_transfer_transport_rejects_partial_corrupt_foreign_and_changed_sources() {
    run(|| {
        let fixture = crate::snapshot::tests::transferred_checkpoint_fixture();
        let budget = AllocationBudget::new(32 * 1024 * 1024);
        let mut source = provider(&fixture, &budget);
        let descriptor = source.descriptor().unwrap();
        let chain = fixture.source.state().chain_id_ref();
        let network = *fixture.source.state().network_id_ref();
        for (index, fault) in [
            Fault::CorruptChunk,
            Fault::WrongIndex,
            Fault::Truncated,
            Fault::Finish,
        ]
        .into_iter()
        .enumerate()
        {
            let output = fixture.directory.path().join(format!("refused-{index}"));
            let result = receive_checkpoint(
                &mut Changed {
                    source: &mut source,
                    fault,
                },
                &descriptor,
                chain,
                network,
                limits(),
                &budget,
                &output,
            );
            assert!(result.is_err());
            if matches!(fault, Fault::Truncated | Fault::Finish) {
                assert!(
                    matches!(result, Err(ExecutionAttemptError::Rejected(Error::Io(ref original)))
                    if original.kind() == if matches!(fault, Fault::Truncated) { ErrorKind::UnexpectedEof } else { ErrorKind::WouldBlock })
                );
            }
            assert!(
                !output.join(TRANSFER_MARKER).exists(),
                "failed transfer cannot publish completion"
            );
        }
        let output = fixture.directory.path().join("foreign");
        assert!(
            receive_checkpoint(
                &mut source,
                &descriptor,
                &ChainId::from("foreign"),
                network,
                limits(),
                &budget,
                &output
            )
            .is_err()
        );
        assert!(!output.exists());
        let mut too_many = limits();
        too_many.max_artifacts = NonZeroUsize::new(1).unwrap();
        assert!(
            receive_checkpoint(
                &mut source,
                &descriptor,
                chain,
                network,
                too_many,
                &budget,
                &output
            )
            .is_err()
        );
        let mut too_large = limits();
        too_large.max_total_bytes = NonZeroU64::new(1).unwrap();
        assert!(
            receive_checkpoint(
                &mut source,
                &descriptor,
                chain,
                network,
                too_large,
                &budget,
                &output
            )
            .is_err()
        );
        assert!(!output.exists());
        let original_budget = AllocationBudget::new(1);
        let refused = receive_checkpoint(
            &mut source,
            &descriptor,
            chain,
            network,
            limits(),
            &original_budget,
            &output,
        );
        assert!(matches!(refused, Err(ExecutionAttemptError::Deferred(_))));
        assert!(
            !output.exists(),
            "original resource refusal precedes fresh directory creation"
        );
        let held = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let refused = receive_checkpoint(
            &mut source,
            &descriptor,
            chain,
            network,
            limits(),
            &budget,
            &output,
        );
        assert!(
            matches!(refused, Err(ExecutionAttemptError::Deferred(ref original))
            if matches!(original.allocation_refusal(), Some(iroha_allocation::AllocationRefusal::Capacity { release, .. }) if budget.is_source_of(release)))
        );
        assert!(!output.exists());
        drop(held);
        receive_checkpoint(
            &mut source,
            &descriptor,
            chain,
            network,
            limits(),
            &budget,
            &output,
        )
        .unwrap();
        assert!(output.join(TRANSFER_MARKER).exists());
        assert!(source.artifact(u64::MAX).is_err());
        assert!(source.read_range(0, u64::MAX, &mut [0; 1]).is_err());
        assert!(source.read_range(0, 0, &mut []).is_err());
        let marker_index = descriptor.height + FIXED_ARTIFACTS - 1;
        let path = fixture
            .directory
            .path()
            .join("transfer")
            .join(TRANSFER_MARKER);
        let mut changed = std::fs::read(&path).unwrap();
        changed.push(b' ');
        std::fs::write(&path, changed).unwrap();
        assert!(source.read_range(marker_index, 0, &mut [0; 1]).is_err());
        assert!(source.finish(&descriptor).is_err());
    });
}
