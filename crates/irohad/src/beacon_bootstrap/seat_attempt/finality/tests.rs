//! Real native work, proof replay, same-frame refusal and unchanged finality cursor.

use super::*;
use iroha_allocation::release::ReleaseRegistration;
use std::task::{Context, Waker};

fn limits() -> NativeFinalityLimits {
    NativeFinalityLimits {
        block_bytes: NATIVE_FINALITY_MAX_BLOCK_BYTES,
        journal_bytes: NATIVE_FINALITY_MAX_JOURNAL_BYTES,
        block_count: NATIVE_FINALITY_MAX_BLOCK_COUNT,
        allocated_bytes: 128 * 1024 * 1024,
    }
}
fn proof() -> (Vec<u8>, ChainId, NetworkId) {
    use iroha_core::{
        state::World,
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    let config = TestChainConfig::new(World::default(), 10_000);
    let chain_id = config.chain_id.clone();
    let mut chain = CertifiedTestChain::start(config).unwrap();
    chain.commit_at(20_000, Vec::new()); // Adds actual signed clock work.
    let journal = NativeFinalityJournal {
        blocks: (1..=2)
            .map(|height| {
                iroha_data_model::sumeragi::finality::NativeFinalityArtifact::from_block(
                    chain.committed(height).block(),
                    limits(),
                )
                .unwrap()
            })
            .collect(),
    };
    (
        norito::encode_canonical(&journal).unwrap(),
        chain_id,
        chain.network_id(),
    )
}
fn source(chain: ChainId, network: NetworkId, pool: &AllocationBudget) -> (File, FinalityInput) {
    let (read, write) = super::super::test_pipe::pipe(true);
    let input = FrameInput::new(
        File::from(read),
        limits().journal_bytes,
        Instant::now() + Duration::from_secs(1),
        pool,
    )
    .unwrap_or_else(|(_, error)| panic!("exact FIFO: {error}"));
    let cursor = NativeJournalCursor::new(
        chain,
        network,
        iroha_data_model::block::consensus::SumeragiRootScope::Global,
        limits(),
        pool,
    )
    .unwrap();
    assert!(
        cursor.tip().is_none(),
        "signed genesis alone is not a finalized execution"
    );
    (
        File::from(write),
        FinalityInput::new(input, cursor, 1).unwrap(),
    )
}
fn write_frame(writer: &File, bytes: &[u8]) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    for source in [
        &u32::try_from(bytes.len()).unwrap().to_be_bytes()[..],
        bytes,
    ] {
        let mut offset = 0;
        while offset < source.len() {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(std::io::ErrorKind::TimedOut)?;
            let timeout = rustix::event::Timespec::try_from(remaining)
                .map_err(|_| std::io::ErrorKind::TimedOut)?;
            let mut ready = [rustix::event::PollFd::new(
                writer,
                rustix::event::PollFlags::OUT,
            )];
            if rustix::event::poll(&mut ready, Some(&timeout))? == 0 {
                return Err(std::io::ErrorKind::TimedOut.into());
            }
            match rustix::io::write(writer, &source[offset..]) {
                Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
                Ok(written) => offset += written,
                Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

#[test]
fn current_phase_pipe_accepts_real_work_and_rejects_replay() {
    let (bytes, chain, network) = proof();
    let pool = AllocationBudget::new(64 * 1024 * 1024);
    let (writer, mut input) = source(chain, network, &pool);
    let producer = std::thread::spawn(move || {
        write_frame(&writer, &bytes)?;
        write_frame(&writer, &bytes)
    });
    let first = input.advance_to(2, 4, |_| Ok(()));
    let replay = input.advance_to(3, 4, |_| Ok(()));
    let last = input.height();
    drop(input);
    producer
        .join()
        .expect("original proof producer")
        .expect("two original frames");
    first.expect("genuinely certified native work");
    assert!(matches!(replay, Err(AttemptError::Height)));
    assert_eq!(last, 2);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn original_pool_refusal_keeps_once_decoded_journal_and_same_complete_frame_until_release() {
    let (bytes, chain, network) = proof();
    let pool = AllocationBudget::new(64 * 1024 * 1024);
    let mut reservation = pool
        .try_reserve(ReleaseRegistration::allocation_layout())
        .unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut reservation).unwrap();
    drop(reservation);
    let (writer, mut input) = source(chain, network, &pool);
    let producer = std::thread::spawn(move || write_frame(&writer, &bytes));
    input.input.read_until_complete().unwrap();
    producer.join().unwrap().unwrap();
    let frame_pointer = input.input.frame().unwrap().as_ptr();
    let blocker = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
        .unwrap();
    let error = input.advance_to(2, 4, |_| Ok(())).unwrap_err();
    let AttemptError::Journal(iroha_core::sumeragi::native_journal::NativeJournalError::Block(
        iroha_data_model::block::SharedBlockAdmissionError::Admission(
            AllocationRefusal::Capacity { release, .. },
        ),
    )) = error
    else {
        panic!("exact original native block-control source: {error}")
    };
    assert_eq!(input.height(), 1);
    assert!(input.clock.tip().is_none());
    let journal_pointer = input
        .journal
        .view(input.input.frame().unwrap())
        .unwrap()
        .blocks()
        .next()
        .unwrap()
        .as_ptr();
    assert_eq!(input.input.frame().unwrap().as_ptr(), frame_pointer);
    let mut context = Context::from_waker(Waker::noop());
    assert!(registration.poll_wait(&release, &mut context).is_pending());
    let foreign = AllocationBudget::new(1);
    drop(foreign.try_reserve_bytes(1).unwrap());
    assert!(registration.poll_wait(&release, &mut context).is_pending());
    assert!(input.advance_to(2, 4, |_| Ok(())).is_err());
    assert_eq!(
        input
            .journal
            .view(input.input.frame().unwrap())
            .unwrap()
            .blocks()
            .next()
            .unwrap()
            .as_ptr(),
        journal_pointer
    );
    assert_eq!(input.input.frame().unwrap().as_ptr(), frame_pointer);
    drop(blocker);
    assert!(registration.poll_wait(&release, &mut context).is_ready());
    registration.cancel();
    input.advance_to(2, 4, |_| Ok(())).unwrap();
    assert_eq!(input.height(), 2);
    assert!(!input.journal.is_decoded());
    assert!(input.input.frame().is_none());
    drop(input);
    drop(registration);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn rotation_phase_pipe_rejects_truncated_oversized_and_noncanonical_proofs() {
    for frame in [
        0u32.to_be_bytes().to_vec(),
        u32::try_from(NATIVE_FINALITY_MAX_JOURNAL_BYTES + 1)
            .unwrap()
            .to_be_bytes()
            .to_vec(),
        vec![0, 0, 0, 1, 0],
        vec![0, 0, 0, 2, 0],
    ] {
        let pool = AllocationBudget::new(64 * 1024 * 1024);
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::new(b"retained-proof-negative"),
        ));
        let (mut writer, mut input) =
            source(ChainId::from("retained-proof-negative"), network, &pool);
        writer.write_all(&frame).unwrap();
        drop(writer);
        input.height = 10;
        assert!(input.advance_to(11, 20, |_| Ok(())).is_err());
        assert_eq!(input.height(), 10);
        assert!(input.clock.tip().is_none());
        drop(input);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn malformed_original_journal_and_prepared_source_failures_are_terminal() {
    for phase in [
        Phase::CommitmentsDecoded,
        Phase::EdgesDecoded,
        Phase::SessionDecoded,
    ] {
        let error = NativeFinalityJournal::decode(&[0], limits()).unwrap_err();
        assert!(
            AttemptError::Journal(
                iroha_core::sumeragi::native_journal::NativeJournalError::Decode(error)
            )
            .terminal(phase)
        );
        let pool = AllocationBudget::new(64 * 1024 * 1024);
        let mut prepared = PreparedNativeFinalityJournal::new(limits(), &pool).unwrap();
        let error = prepared.decode(&[0]).unwrap_err();
        assert!(AttemptError::JournalSource(error).terminal(phase));
        let (bytes, _, _) = proof();
        prepared.clear_consumed();
        let error = norito::core::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
            || prepared.decode(&bytes),
        )
        .unwrap_err();
        assert!(!AttemptError::JournalSource(error).terminal(phase));
        prepared.decode(&bytes).unwrap();
    }
}

#[test]
fn original_verified_target_proof_publication_refusal_retains_frame_and_never_advances_twice() {
    use super::super::publication::{PhaseFile, PhasePublication};
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    let (bytes, chain, network) = proof();
    let pool = AllocationBudget::new(64 * 1024 * 1024);
    let (writer, mut input) = source(chain, network, &pool);
    let producer = std::thread::spawn({
        let bytes = bytes.clone();
        move || write_frame(&writer, &bytes)
    });
    input.input.read_until_complete().unwrap();
    producer.join().unwrap().unwrap();
    let pointer = input.input.frame().unwrap().as_ptr();
    let (_temporary, path) = super::super::tests::root();
    let directory = Directory::open(&path).unwrap();
    let file_path = path.join("proof-commitments.norito");
    fs::write(&file_path, b"unrelated exact destination").unwrap();
    fs::set_permissions(&file_path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut publication = PhasePublication::new(PhaseFile::CommitmentsProof);
    let result = input.advance_to(2, 4, |frame| {
        assert_eq!(frame.as_ptr(), pointer);
        publication.publish(&directory, frame)?;
        Ok(())
    });
    let AttemptError::Export(seat_export::ExportError::Io(cause)) = result.unwrap_err() else {
        panic!("actual original exclusive-create cause");
    };
    assert_eq!(cause.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(input.height(), 2);
    assert_eq!(input.clock.tip().unwrap().height(), 2);
    let result = input.clock.tip().unwrap().result();
    assert!(input.committed);
    assert!(input.journal.is_decoded());
    assert_eq!(input.input.frame().unwrap().as_ptr(), pointer);
    assert_eq!(input.generation(), 0);
    assert!(!publication.complete());
    let retained = pool.reserved_bytes();
    fs::remove_file(&file_path).unwrap();
    input
        .advance_to(2, 4, |frame| {
            assert_eq!(frame.as_ptr(), pointer);
            publication.publish(&directory, frame)?;
            Ok(())
        })
        .unwrap();
    assert!(publication.complete());
    assert_eq!(
        publication.complete_hash().unwrap(),
        <[u8; 32]>::from(Hash::new(&bytes))
    );
    assert_eq!(fs::read(&file_path).unwrap(), bytes);
    let inode = fs::metadata(&file_path).unwrap().ino();
    assert_eq!(input.generation(), 1);
    assert_eq!(input.clock.tip().unwrap().result(), result);
    assert!(!input.committed);
    assert!(!input.journal.is_decoded());
    assert!(input.input.frame().is_none());
    assert!(pool.reserved_bytes() < retained);
    assert_eq!(fs::metadata(&file_path).unwrap().ino(), inode);
    drop(publication);
    drop(input);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn original_durable_native_proof_replay_checks_true_ancestry_and_keeps_original_source_on_refusal()
{
    let (bytes, chain, network) = proof();
    let pool = AllocationBudget::new(64 * 1024 * 1024);
    let (_writer, mut input) = source(chain.clone(), network, &pool);
    let source_pointer = bytes.as_ptr();
    let blocker = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
        .unwrap();
    assert!(matches!(
        input.restore_target_from_original_frame(&bytes, 2, 4),
        Err(AttemptError::Journal(_))
    ));
    assert_eq!(input.height(), 1);
    assert!(input.clock.tip().is_none());
    assert!(input.journal.is_decoded());
    let span = input
        .journal
        .view(&bytes)
        .unwrap()
        .blocks()
        .next()
        .unwrap()
        .as_ptr();
    let same_bytes_different_owner = bytes.clone();
    assert!(matches!(
        input.restore_target_from_original_frame(&same_bytes_different_owner, 2, 4),
        Err(AttemptError::Binding)
    ));
    assert_eq!(
        input
            .journal
            .view(&bytes)
            .unwrap()
            .blocks()
            .next()
            .unwrap()
            .as_ptr(),
        span
    );
    drop(blocker);
    input
        .restore_target_from_original_frame(&bytes, 2, 4)
        .unwrap();
    assert_eq!(input.height(), 2);
    assert_eq!(input.clock.tip().unwrap().height(), 2);
    assert_eq!(
        input.generation(),
        0,
        "replay reads only the durable original frame"
    );
    assert_eq!(
        input.restored_source.as_ref().unwrap().address,
        source_pointer.addr()
    );
    assert!(!input.journal.is_decoded());
    let actual_result = input.clock.tip().unwrap().result();
    input
        .restore_target_from_original_frame(&bytes, 2, 4)
        .unwrap();
    assert_eq!(input.clock.tip().unwrap().result(), actual_result);
    assert!(matches!(
        input.restore_target_from_original_frame(&same_bytes_different_owner, 2, 4),
        Err(AttemptError::Binding)
    ));
    let foreign = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::new(b"foreign proof replay network"),
    ));
    let (_foreign_writer, mut wrong) = source(chain, foreign, &pool);
    assert!(
        wrong
            .restore_target_from_original_frame(&bytes, 2, 4)
            .is_err()
    );
    assert!(wrong.clock.tip().is_none());
    assert_eq!(wrong.height(), 1);
    drop(wrong);
    drop(input);
    assert_eq!(pool.reserved_bytes(), 0);
}
