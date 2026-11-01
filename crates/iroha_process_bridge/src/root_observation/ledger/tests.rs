//! Pure lifecycle counterexamples; actual owned child controls use the emitted `cdylib`.
use super::*;

fn task(
    kind: u32,
    group: u32,
    thread: u32,
    parent_group: u32,
    parent_thread: u32,
    time: u64,
) -> Record {
    Record {
        kind,
        pid: group,
        tid: thread,
        ppid: parent_group,
        ptid: parent_thread,
        time,
        cpu: 0,
        exec: false,
    }
}
fn exec(time: u64) -> Record {
    Record {
        exec: true,
        ..task(3, 200, 200, 0, 0, time)
    }
}
fn normal() -> Vec<Record> {
    vec![
        task(7, 200, 200, 100, 100, 10),
        exec(20),
        task(4, 200, 200, 100, 100, 40),
    ]
}
fn refusal(records: &[Record], reason: &'static str) {
    assert_eq!(
        terminal(records, 100, Some(200), 1),
        Err(Failure::new(reason))
    );
}

#[test]
fn root_and_threads_require_complete_original_birth_exec_and_exit() {
    let mut rows = normal();
    rows.extend([
        task(7, 200, 201, 200, 200, 25),
        task(4, 200, 201, 200, 200, 30),
    ]);
    rows.reverse(); // Cross-CPU capture order is not the final global order.
    assert_eq!(
        terminal(&rows, 100, Some(200), 1),
        Ok(Facts {
            target_tasks: 2,
            initial_exec_ns: Some(20)
        })
    );
    rows.retain(|row| !(row.kind == 4 && row.tid == 201));
    refusal(&rows, "live_target_task");
}

#[test]
fn joined_external_process_is_rejected_even_with_complete_exit() {
    let mut rows = normal();
    rows.extend([
        task(7, 300, 300, 200, 200, 25),
        task(4, 300, 300, 200, 200, 30),
    ]);
    refusal(&rows, "unexpected_process_fork");
}

#[test]
fn same_image_and_nonleader_exec_cannot_obtain_a_certificate() {
    let mut rows = normal();
    rows.push(exec(35));
    refusal(&rows, "unexpected_exec");
    let mut rows = normal();
    rows.extend([
        task(7, 200, 201, 200, 200, 25),
        Record {
            exec: true,
            ..task(3, 200, 201, 0, 0, 30)
        },
    ]);
    refusal(&rows, "unexpected_exec");
}

#[test]
fn failed_exec_has_no_image_transition_and_failed_popen_is_only_closed_preparation() {
    // A syscall failure adds no COMM_EXEC; a normal original still qualifies.
    assert!(terminal(&normal(), 100, Some(200), 1).is_ok());
    let no_exec = [
        task(7, 200, 200, 100, 100, 10),
        task(4, 200, 200, 100, 100, 20),
    ];
    assert_eq!(
        terminal(&no_exec, 100, None, 1),
        Ok(Facts {
            target_tasks: 1,
            initial_exec_ns: None
        })
    );
    assert_eq!(
        terminal(&no_exec, 100, Some(200), 1),
        Err(Failure::new("incomplete_initial_image_transition"))
    );
    assert!(terminal(&normal(), 100, None, 1).is_err());
    assert!(terminal(&no_exec[1..], 100, None, 1).is_err());
    assert!(terminal(&no_exec[..1], 100, None, 1).is_err());
    assert_eq!(terminal(&[], 100, None, 1), Ok(Facts::default()));
}

#[test]
fn ties_unknown_lineage_reuse_and_missing_initial_records_refuse() {
    let mut rows = normal();
    rows.push(task(3, 200, 200, 0, 0, 20));
    refusal(&rows, "ambiguous_event_time");
    let mut rows = normal();
    rows.push(task(7, 200, 201, 200, 999, 25));
    refusal(&rows, "unknown_parent");
    let mut rows = normal();
    rows.push(task(7, 200, 200, 200, 200, 25));
    refusal(&rows, "reused_task_identifier");
    refusal(&normal()[1..], "unknown_task");
    refusal(
        &[normal()[0], normal()[2]],
        "incomplete_initial_image_transition",
    );
}

#[test]
fn observer_reader_threads_are_distinct_from_target_scope() {
    let mut rows = normal();
    rows.push(task(7, 100, 101, 100, 100, 25));
    assert!(terminal(&rows, 100, Some(200), 1).is_ok());
    rows.push(task(7, 300, 300, 100, 101, 30));
    refusal(&rows, "unbound_process_fork");
}

#[test]
fn pid_input_selects_only_an_existing_direct_birth() {
    assert_eq!(birth(&[], 100, 200, 1), Ok(false));
    assert_eq!(birth(&normal(), 100, 200, 1), Ok(true));
    assert_eq!(
        birth(&normal(), 100, 999, 1),
        Err(Failure::new("unobserved_child_selection"))
    );
    assert_eq!(
        birth(&normal(), 100, 200, 11),
        Err(Failure::new("unobserved_child_selection"))
    );
    let mut duplicate = normal();
    duplicate.push(normal()[0]);
    assert_eq!(
        birth(&duplicate, 100, 200, 1),
        Err(Failure::new("ambiguous_root_birth"))
    );
}

#[test]
fn record_parser_refuses_loss_unknown_cpu_and_invalid_extents() {
    let mut bytes = [0_u8; 56];
    bytes[..4].copy_from_slice(&7_u32.to_ne_bytes());
    bytes[6..8].copy_from_slice(&56_u16.to_ne_bytes());
    for (offset, value) in [
        (8, 200_u32),
        (12, 100),
        (16, 200),
        (20, 100),
        (32, 100),
        (36, 100),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes[40..48].copy_from_slice(&10_u64.to_ne_bytes());
    assert_eq!(Record::parse(&bytes, 0).unwrap(), normal()[0]);
    assert_eq!(Record::parse(&bytes, 1), Err(Failure::new("record_cpu")));
    bytes[..4].copy_from_slice(&2_u32.to_ne_bytes());
    assert_eq!(Record::parse(&bytes, 0), Err(Failure::new("event_loss")));
    bytes[..4].copy_from_slice(&99_u32.to_ne_bytes());
    assert_eq!(Record::parse(&bytes, 0), Err(Failure::new("unknown_event")));
    assert!(Record::parse(&bytes[..55], 0).is_err());
}
