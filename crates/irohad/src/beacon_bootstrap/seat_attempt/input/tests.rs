//! Real FIFO partial-read, exact-header admission and original-source retry controls.

use super::*;
use iroha_allocation::release::ReleaseRegistration;
use std::{
    io::Write as _,
    os::fd::AsRawFd as _,
    task::{Context, Waker},
    time::Duration,
};

fn input(maximum: usize, budget: &AllocationBudget) -> (File, FrameInput) {
    let (read, write) = super::super::test_pipe::pipe(true);
    let input = FrameInput::new(
        File::from(read),
        maximum,
        Instant::now() + Duration::from_secs(30),
        budget,
    )
    .unwrap_or_else(|(_, error)| panic!("original input preparation: {error}"));
    (File::from(write), input)
}
fn assert_would_block(result: Result<bool, FrameReadError>) {
    let FrameReadError::Io(source) = result.unwrap_err() else {
        panic!("real empty nonblocking FIFO must retain its syscall error");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::WouldBlock);
}

#[test]
fn partial_header_and_body_retry_original_fifo_without_consuming_the_next_frame() {
    let budget = AllocationBudget::new(1024);
    let (mut write, mut input) = input(256, &budget);
    let fd = input.descriptor.as_raw_fd();
    let first = b"original public phase";
    let header = u32::try_from(first.len()).unwrap().to_be_bytes();
    write.write_all(&header[..1]).unwrap();
    assert!(!input.read_ready().unwrap());
    assert_eq!(input.header_read, 1);
    assert_would_block(input.read_ready());
    assert_eq!(input.header_read, 1);
    assert_eq!(budget.reserved_bytes(), 0);
    write.write_all(&header[1..]).unwrap();
    assert!(!input.read_ready().unwrap());
    assert_eq!(input.header_read, 4);
    assert_eq!(budget.reserved_bytes(), first.len());
    let pointer = input.body.as_ref().unwrap().as_slice().as_ptr();
    write.write_all(&first[..5]).unwrap();
    assert!(!input.read_ready().unwrap());
    assert_would_block(input.read_ready());
    assert_eq!(input.body_read, 5);
    assert!(input.frame().is_none());
    assert!(matches!(
        input.consume_verified_frame(),
        Err(FrameReadError::Phase)
    ));
    let second = b"next phase";
    write.write_all(&first[5..]).unwrap();
    write
        .write_all(&u32::try_from(second.len()).unwrap().to_be_bytes())
        .unwrap();
    write.write_all(second).unwrap();
    input.read_until_complete().unwrap();
    assert_eq!(input.frame().unwrap(), first);
    assert_eq!(input.descriptor.as_raw_fd(), fd);
    assert_eq!(input.body.as_ref().unwrap().as_slice().as_ptr(), pointer);
    for _ in 0..3 {
        assert!(input.read_ready().unwrap());
        assert_eq!(input.frame().unwrap(), first);
    }
    assert_eq!(input.generation, 0);
    input.consume_verified_frame().unwrap();
    assert_eq!(input.generation, 1);
    assert_eq!(budget.reserved_bytes(), 0);
    input.read_until_complete().unwrap();
    assert_eq!(input.frame().unwrap(), second);
    drop(input);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn exact_header_refusal_keeps_body_unconsumed_and_retries_only_the_original_pool() {
    let layout = ReleaseRegistration::allocation_layout();
    let budget = AllocationBudget::new(layout.size() + 17);
    let mut prepaid = budget.try_reserve(layout).unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut prepaid).unwrap();
    drop(prepaid);
    let floor = budget.reserved_bytes();
    let (mut write, mut input) = input(256, &budget);
    let body = [0x9A; 17];
    write.write_all(&17u32.to_be_bytes()).unwrap();
    write.write_all(&body).unwrap();
    let occupied = budget.try_reserve_bytes(17).unwrap();
    let expected = budget.try_reserve_bytes(17).unwrap_err();
    let FrameReadError::Admission(actual) = input.read_ready().unwrap_err() else {
        panic!("original body admission refusal");
    };
    assert_eq!(actual, expected);
    assert_eq!(input.header_read, 4);
    assert_eq!(input.body_read, 0);
    assert!(input.body.is_none());
    assert_eq!(input.header, 17u32.to_be_bytes());
    let AllocationRefusal::Capacity { release, .. } = actual else {
        panic!("actual capacity source");
    };
    let mut cx = Context::from_waker(Waker::noop());
    assert!(registration.poll_wait(&release, &mut cx).is_pending());
    let foreign = AllocationBudget::new(17);
    drop(foreign.try_reserve_bytes(17).unwrap());
    assert!(registration.poll_wait(&release, &mut cx).is_pending());
    drop(occupied);
    assert!(registration.poll_wait(&release, &mut cx).is_ready());
    registration.cancel();
    input.read_until_complete().unwrap();
    assert_eq!(input.frame().unwrap(), body);
    assert_eq!(budget.reserved_bytes(), floor + body.len());
    drop(input);
    assert_eq!(budget.reserved_bytes(), floor);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn zero_and_oversized_lengths_never_consume_body_or_allocate_a_frame() {
    for length in [0u32, 257] {
        let budget = AllocationBudget::new(1024);
        let (mut write, mut input) = input(256, &budget);
        write.write_all(&length.to_be_bytes()).unwrap();
        write.write_all(b"retained unread body").unwrap();
        assert!(matches!(input.read_ready(), Err(FrameReadError::Length)));
        assert_eq!(input.header_read, 4);
        assert_eq!(input.body_read, 0);
        assert_eq!(budget.reserved_bytes(), 0);
        let mut actual = [0; 20];
        assert_eq!(
            rustix::io::read(&input.descriptor, &mut actual).unwrap(),
            actual.len()
        );
        assert_eq!(&actual, b"retained unread body");
    }
}

#[test]
fn eof_and_original_deadline_preserve_consumed_offsets_without_resetting_source() {
    let budget = AllocationBudget::new(1024);
    let (mut write, mut input) = input(256, &budget);
    write.write_all(&3u32.to_be_bytes()).unwrap();
    write.write_all(b"x").unwrap();
    assert!(!input.read_ready().unwrap());
    assert!(!input.read_ready().unwrap());
    drop(write);
    assert!(matches!(
        input.read_ready(),
        Err(FrameReadError::EndOfStream)
    ));
    assert_eq!(input.header_read, 4);
    assert_eq!(input.body_read, 1);
    assert_eq!(&input.body.as_ref().unwrap().as_slice()[..1], b"x");
    assert_eq!(input.generation, 0);
    let retained = budget.reserved_bytes();
    let (read, mut write) = super::super::test_pipe::pipe(true);
    let deadline = Instant::now();
    let mut expired = FrameInput::new(File::from(read), 256, deadline, &budget)
        .unwrap_or_else(|(_, error)| panic!("expiry fixture: {error}"));
    rustix::io::write(&mut write, &3u32.to_be_bytes()).unwrap();
    assert!(matches!(
        expired.read_until_complete(),
        Err(FrameReadError::Deadline)
    ));
    assert_eq!(expired.header_read, 0);
    assert_eq!(expired.deadline, deadline);
    assert_eq!(budget.reserved_bytes(), retained);
    assert!(matches!(
        expired.read_until_complete(),
        Err(FrameReadError::Deadline)
    ));
    drop(expired);
    drop(input);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn replaced_source_is_rejected_before_read_and_original_backing_refunds_on_unwind() {
    let budget = AllocationBudget::new(1024);
    let (mut write, mut input) = input(256, &budget);
    write.write_all(&3u32.to_be_bytes()).unwrap();
    write.write_all(b"one").unwrap();
    input.read_until_complete().unwrap();
    let (other_read, mut other_write) = super::super::test_pipe::pipe(true);
    rustix::io::write(&mut other_write, b"untouched").unwrap();
    let original = std::mem::replace(&mut input.descriptor, File::from(other_read));
    assert!(matches!(input.read_ready(), Err(FrameReadError::Custody)));
    let mut unread = [0; 9];
    assert_eq!(rustix::io::read(&input.descriptor, &mut unread).unwrap(), 9);
    assert_eq!(&unread, b"untouched");
    input.descriptor = original;
    assert!(input.read_ready().unwrap());
    assert_eq!(input.frame().unwrap(), b"one");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _owner = input;
        panic!("controlled original attempt cancellation");
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn original_blocking_fifo_becomes_nonblocking_before_any_consumption() {
    let budget = AllocationBudget::new(1024);
    let (read, write) = super::super::test_pipe::pipe(false);
    let original_flags = rustix::fs::fcntl_getfl(&read).unwrap();
    assert!(!original_flags.contains(rustix::fs::OFlags::NONBLOCK));
    let original_descriptor = read.as_raw_fd();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut input = FrameInput::new(File::from(read), 256, deadline, &budget)
        .unwrap_or_else(|(_, error)| panic!("blocking FIFO preparation: {error}"));
    assert_eq!(input.descriptor.as_raw_fd(), original_descriptor);
    assert_eq!(
        rustix::fs::fcntl_getfl(&input.descriptor).unwrap(),
        original_flags | rustix::fs::OFlags::NONBLOCK,
    );
    assert_would_block(input.read_ready());
    assert_eq!(input.deadline, deadline);
    assert_eq!(input.header_read, 0);
    assert_eq!(input.generation, 0);
    assert_eq!(budget.reserved_bytes(), 0);
    drop(write);
    assert!(matches!(
        input.read_ready(),
        Err(FrameReadError::EndOfStream)
    ));
}

#[test]
fn write_only_input_returns_the_same_descriptor_before_mutating_its_flags() {
    let budget = AllocationBudget::new(1024);
    let (read, write) = super::super::test_pipe::pipe(false);
    let original_fd = write.as_raw_fd();
    let original_flags = rustix::fs::fcntl_getfl(&write).unwrap();
    let (original, error) = FrameInput::new(
        File::from(write),
        256,
        Instant::now() + Duration::from_secs(30),
        &budget,
    )
    .err()
    .expect("write end is not an input");
    assert!(matches!(error, FrameReadError::Custody));
    assert_eq!(original.as_raw_fd(), original_fd);
    assert_eq!(rustix::fs::fcntl_getfl(&original).unwrap(), original_flags);
    assert_eq!(budget.reserved_bytes(), 0);
    rustix::io::write(&original, b"same write end").unwrap();
    let mut actual = [0; 14];
    assert_eq!(rustix::io::read(&read, &mut actual).unwrap(), actual.len());
    assert_eq!(&actual, b"same write end");
}

#[test]
fn aliased_phase_descriptors_are_rejected_without_reading_either_stream() {
    let budget = AllocationBudget::new(1024);
    let (mut write, original) = input(256, &budget);
    let duplicate = original.descriptor.try_clone().unwrap();
    assert_ne!(duplicate.as_raw_fd(), original.descriptor.as_raw_fd());
    let alias = FrameInput::new(duplicate, 256, original.deadline, &budget)
        .unwrap_or_else(|(_, error)| panic!("aliased descriptor fixture: {error}"));
    write.write_all(b"still unread").unwrap();
    assert!(matches!(
        original.require_distinct_source(&alias),
        Err(FrameReadError::Custody)
    ));
    assert_eq!(original.header_read, 0);
    assert_eq!(alias.header_read, 0);
    let (_other_write, other) = input(256, &budget);
    original.require_distinct_source(&other).unwrap();
    other.require_distinct_source(&original).unwrap();
    let mut actual = [0; 12];
    assert_eq!(
        rustix::io::read(&original.descriptor, &mut actual).unwrap(),
        actual.len()
    );
    assert_eq!(&actual, b"still unread");
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn authenticated_next_frame_bound_cannot_reprice_a_consumed_header() {
    let budget = AllocationBudget::new(128);
    let (mut write, mut input) = input(4, &budget);
    write.write_all(&4u32.to_be_bytes()[..1]).unwrap();
    assert!(!input.read_ready().unwrap());
    assert!(matches!(
        input.set_next_maximum(8),
        Err(FrameReadError::Phase)
    ));
    assert_eq!(input.maximum, 4);
    write.write_all(&4u32.to_be_bytes()[1..]).unwrap();
    write.write_all(b"four").unwrap();
    input.read_until_complete().unwrap();
    assert!(matches!(
        input.set_next_maximum(8),
        Err(FrameReadError::Phase)
    ));
    input.consume_verified_frame().unwrap();
    input.set_next_maximum(8).unwrap();
    write.write_all(&8u32.to_be_bytes()).unwrap();
    write.write_all(b"original").unwrap();
    input.read_until_complete().unwrap();
    assert_eq!(input.frame(), Some(b"original".as_slice()));
}

#[test]
fn bounded_phase_reader_consumes_exact_frame_without_advancing_next_frame() {
    let budget = AllocationBudget::new(32);
    let (mut writer, mut input) = input(16, &budget);
    writer.write_all(&5u32.to_be_bytes()).unwrap();
    writer.write_all(b"first").unwrap();
    writer.write_all(&6u32.to_be_bytes()).unwrap();
    writer.write_all(b"second").unwrap();
    input.read_until_complete().unwrap();
    assert_eq!(input.frame().unwrap(), b"first");
    input.consume_verified_frame().unwrap();
    input.read_until_complete().unwrap();
    assert_eq!(input.frame().unwrap(), b"second");
    drop(input);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn original_recorded_stream_generation_restores_only_the_same_empty_inherited_source() {
    let pool = AllocationBudget::new(1024);
    let (mut writer, mut receiver) = input(256, &pool);
    let source = receiver.source_identity().unwrap();
    let fd = receiver.descriptor.as_raw_fd();
    let deadline = receiver.deadline;
    receiver.restore_empty_cursor(7, 128, source).unwrap();
    assert_eq!(receiver.generation(), 7);
    assert_eq!(receiver.maximum, 128);
    assert_eq!(receiver.descriptor.as_raw_fd(), fd);
    assert_eq!(receiver.deadline, deadline);
    receiver.restore_empty_cursor(7, 128, source).unwrap();
    assert!(matches!(
        receiver.restore_empty_cursor(8, 128, source),
        Err(FrameReadError::Custody)
    ));
    let mut foreign = source;
    foreign[1] ^= 1;
    assert!(matches!(
        receiver.restore_empty_cursor(7, 128, foreign),
        Err(FrameReadError::Custody)
    ));
    writer.write_all(&3u32.to_be_bytes()[..1]).unwrap();
    assert!(!receiver.read_ready().unwrap());
    assert_eq!(receiver.header_read, 1);
    assert!(matches!(
        receiver.restore_empty_cursor(7, 256, source),
        Err(FrameReadError::Phase)
    ));
    assert_eq!(receiver.maximum, 128);
    assert_eq!(receiver.generation(), 7);
    assert_eq!(receiver.header_read, 1);
    writer.write_all(&3u32.to_be_bytes()[1..]).unwrap();
    writer.write_all(b"old").unwrap();
    receiver.read_until_complete().unwrap();
    let pointer = receiver.frame().unwrap().as_ptr();
    assert!(matches!(
        receiver.restore_empty_cursor(7, 256, source),
        Err(FrameReadError::Phase)
    ));
    assert_eq!(receiver.frame().unwrap().as_ptr(), pointer);
    assert_eq!(receiver.frame().unwrap(), b"old");
    assert_eq!(receiver.deadline, deadline);
    assert_eq!(receiver.descriptor.as_raw_fd(), fd);
    drop(receiver);
    assert_eq!(pool.reserved_bytes(), 0);
}
