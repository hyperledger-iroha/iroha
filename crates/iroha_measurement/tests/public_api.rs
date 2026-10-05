//! The public recording interface returns no decision and accepts no payload.
//!
//! Two guards hold this. The function-pointer coercions stop compiling when a
//! listed recording method changes its signature. The source scan compares
//! every `pub fn` and every `pub` field of `src/recorder.rs` with an exact
//! allowlist, so a new method that returns a value instrumented code could
//! branch on, or that accepts borrowed text or bytes, fails the test until it
//! is reviewed and listed here.

use std::path::PathBuf;

use iroha_allocation::AllocationBudget;
use iroha_measurement::{
    AllocationCounter, ByteKind, CollectingSink, Declaration, DeclaredClass, DirectorySink,
    Finding, FlowKind, HARNESS_CONTEXT_FILE, HARNESS_OUTPUT_DIR_ENV, LiveBuffer, MeasurementRecord,
    PhaseGuard, PhaseParent, RecordSink, RunContext, RunIdentity, RunOutcome, Session,
    SessionHandle, Unit, WorkerDeclaration, read_harness_context, unclassified_numbers,
};

fn assert_send_sync<T: Send + Sync>() {}

fn assert_unwind_safe<T: std::panic::UnwindSafe + std::panic::RefUnwindSafe>() {}

#[test]
fn recording_methods_return_unit_or_opaque_guards() {
    let _: fn(&SessionHandle, &'static str) -> PhaseGuard = SessionHandle::enter;
    let _: fn(&SessionHandle, ByteKind, &'static str, u64) = SessionHandle::record_bytes;
    let _: fn(&SessionHandle, &'static str, u64) = SessionHandle::record_work;
    let _: fn(&SessionHandle, &'static str, &'static str) = SessionHandle::record_failure;
    let _: fn(&SessionHandle, Declaration) = SessionHandle::declare;
    let _: fn(&SessionHandle, u32) = SessionHandle::declare_phase_workers;
    let _: fn(&SessionHandle, &'static str) -> AllocationCounter =
        SessionHandle::allocation_counter;
    let _: fn(&SessionHandle, &'static str, &AllocationBudget) =
        SessionHandle::observe_allocation_budget;
    let _: fn(PhaseGuard) = PhaseGuard::complete;
    let _: fn(&PhaseGuard) -> PhaseParent = PhaseGuard::parent;
    let _: fn(&PhaseParent, &'static str) -> PhaseGuard = PhaseParent::enter;
    let _: fn(&AllocationCounter, u64) = AllocationCounter::allocated;
    let _: fn(&AllocationCounter, u64) = AllocationCounter::freed;
    let _: fn(&AllocationCounter, u64) -> LiveBuffer = AllocationCounter::buffer;
    let _: fn(RunIdentity, &'static str, WorkerDeclaration, Box<dyn RecordSink>) -> Session =
        Session::begin;
    let _: fn(&Session) -> SessionHandle = Session::handle;
    let _: fn(Session) = Session::finish;
    // Worker threads record through sendable handles and parent tokens.
    assert_send_sync::<SessionHandle>();
    assert_send_sync::<PhaseParent>();
    assert_send_sync::<AllocationCounter>();
    assert_send_sync::<CollectingSink>();
    // Diagnostics observe prover unwinds: every recorder type, and a receipt
    // that owns one, may cross a `catch_unwind` boundary.
    assert_unwind_safe::<Session>();
    assert_unwind_safe::<SessionHandle>();
    assert_unwind_safe::<PhaseGuard>();
    assert_unwind_safe::<PhaseParent>();
    assert_unwind_safe::<AllocationCounter>();
    assert_unwind_safe::<LiveBuffer>();
    assert_unwind_safe::<CollectingSink>();
    assert_unwind_safe::<MeasurementRecord>();
}

/// Compiles only when `T` implements neither `Send` nor `Sync`: with either
/// implementation the marker parameter of `some_item` becomes ambiguous.
macro_rules! assert_neither_send_nor_sync {
    ($type:ty) => {{
        trait AmbiguousIfImplemented<Marker> {
            fn some_item() {}
        }
        impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}
        impl<T: ?Sized + Send> AmbiguousIfImplemented<u8> for T {}
        impl<T: ?Sized + Sync> AmbiguousIfImplemented<u16> for T {}
        let _ = <$type as AmbiguousIfImplemented<_>>::some_item;
    }};
}

#[test]
fn guards_and_the_session_stay_on_the_thread_that_opened_them() {
    // A phase's CPU time is read from the opening thread's clock, so its
    // guard and the session that owns the root must not cross threads.
    assert_neither_send_nor_sync!(PhaseGuard);
    assert_neither_send_nor_sync!(Session);
}

/// The part of the recorder source that is not its unit tests.
fn recorder_source() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/recorder.rs");
    let source = std::fs::read_to_string(path).expect("recorder source");
    let end = source
        .find("#[cfg(test)]\nmod tests {")
        .expect("recorder unit tests follow the implementation");
    source[..end].to_owned()
}

/// Name, parameter list and return type of every `pub fn` in `source`, with
/// whitespace collapsed. Doc comments are skipped.
fn public_functions(source: &str) -> Vec<(String, String, String)> {
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut functions = Vec::new();
    let mut rest = code.as_str();
    while let Some(position) = rest.find("pub ") {
        rest = &rest[position + "pub ".len()..];
        let declaration = rest.strip_prefix("const ").unwrap_or(rest);
        let Some(after) = declaration.strip_prefix("fn ") else {
            continue;
        };
        let open = after.find('(').expect("parameter list");
        let name = after[..open].trim().to_owned();
        let mut depth = 0_usize;
        let close = after[open..]
            .char_indices()
            .find_map(|(offset, character)| {
                match character {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                (depth == 0).then_some(open + offset)
            })
            .expect("closing parenthesis");
        let collapse = |text: &str| {
            let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
            joined
                .replace("( ", "(")
                .replace(", )", ")")
                .trim_end_matches(',')
                .trim()
                .to_owned()
        };
        let parameters = collapse(&after[open + 1..close]);
        let body = after[close + 1..].find('{').expect("function body");
        let returned = collapse(
            after[close + 1..close + 1 + body]
                .trim()
                .trim_start_matches("->"),
        );
        functions.push((name, parameters, returned));
    }
    functions.sort();
    functions
}

/// Name and type of every `pub` field in `source`.
fn public_fields(source: &str) -> Vec<(String, String)> {
    let mut fields: Vec<_> = source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .filter(|rest| !rest.starts_with("fn ") && !rest.starts_with("const "))
        .filter_map(|rest| rest.trim_end_matches(',').split_once(": "))
        .map(|(name, kind)| (name.to_owned(), kind.to_owned()))
        .collect();
    fields.sort();
    fields
}

#[test]
fn source_scanner_reads_signatures_fields_and_ignores_comments() {
    let source = r"
/// pub fn documented_only(&self) -> u64 {
pub struct Declared {
    /// Documented.
    pub label: &'static str,
    pub value: u64,
    private: bool,
}
impl Thing {
    pub const fn pair(self) -> (A, B) {
        (A, B)
    }
    pub fn long(
        &self,
        label: &'static str,
        bytes: u64,
    ) {
    }
    pub fn leak(&self) -> u64 { 0 }
    fn private(&self) -> u64 { 0 }
}
";
    assert_eq!(
        public_functions(source),
        [
            ("leak".to_owned(), "&self".to_owned(), "u64".to_owned()),
            (
                "long".to_owned(),
                "&self, label: &'static str, bytes: u64".to_owned(),
                String::new()
            ),
            ("pair".to_owned(), "self".to_owned(), "(A, B)".to_owned()),
        ]
    );
    assert_eq!(
        public_fields(source),
        [
            ("label".to_owned(), "&'static str".to_owned()),
            ("value".to_owned(), "u64".to_owned()),
        ]
    );
}

#[test]
fn every_public_recorder_function_and_field_is_on_the_reviewed_allowlist() {
    // Each row: name, parameters, return type. A recording method returns
    // nothing or an opaque guard, and takes only `&'static str` labels,
    // enumerated kinds and unsigned integers. `schema_pair` belongs to the
    // declaration enum and maps it to two enumerated words; `begin`, `handle`
    // and `finish` open, share and close the session.
    let allowed: [(&str, &str, &str); 18] = [
        ("allocated", "&self, bytes: u64", ""),
        (
            "allocation_counter",
            "&self, label: &'static str",
            "AllocationCounter",
        ),
        (
            "begin",
            "identity: RunIdentity, root: &'static str, workers: WorkerDeclaration, \
             sink: Box<dyn RecordSink>",
            "Self",
        ),
        ("buffer", "&self, bytes: u64", "LiveBuffer"),
        ("complete", "mut self", ""),
        ("declare", "&self, declaration: Declaration", ""),
        ("declare_phase_workers", "&self, workers: u32", ""),
        ("enter", "&self, label: &'static str", "PhaseGuard"),
        ("enter", "&self, label: &'static str", "PhaseGuard"),
        ("finish", "self", ""),
        ("freed", "&self, bytes: u64", ""),
        ("handle", "&self", "SessionHandle"),
        (
            "observe_allocation_budget",
            "&self, label: &'static str, budget: &AllocationBudget",
            "",
        ),
        ("parent", "&self", "PhaseParent"),
        (
            "record_bytes",
            "&self, kind: ByteKind, label: &'static str, bytes: u64",
            "",
        ),
        (
            "record_failure",
            "&self, stage: &'static str, code: &'static str",
            "",
        ),
        ("record_work", "&self, label: &'static str, units: u64", ""),
        ("schema_pair", "self", "(Classification, ProvenanceKind)"),
    ];
    let source = recorder_source();
    let actual = public_functions(&source);
    let expected: Vec<_> = allowed
        .iter()
        .map(|(name, parameters, returned)| {
            (
                (*name).to_owned(),
                (*parameters).to_owned(),
                (*returned).to_owned(),
            )
        })
        .collect();
    assert_eq!(
        actual, expected,
        "a public recorder function changed: a recording method must return nothing or an \
         opaque guard and accept only static labels, enumerated kinds and integers"
    );
    // Stated independently of the table: nothing readable comes back and no
    // borrowed text or byte slice goes in.
    for (name, parameters, returned) in &actual {
        assert!(
            [
                "",
                "PhaseGuard",
                "PhaseParent",
                "AllocationCounter",
                "LiveBuffer",
                "SessionHandle",
                "Self",
                "(Classification, ProvenanceKind)"
            ]
            .contains(&returned.as_str()),
            "{name} returns {returned}"
        );
        for forbidden in ["&str", "&[", "String", "impl ", "Vec<", "Display", "Debug"] {
            assert!(!parameters.contains(forbidden), "{name}({parameters})");
        }
        assert_eq!(
            parameters.matches("str").count(),
            parameters.matches("&'static str").count(),
            "{name}({parameters})"
        );
    }
    assert_eq!(
        public_fields(&source),
        [
            ("class", "DeclaredClass"),
            ("label", "&'static str"),
            ("provenance", "&'static str"),
            ("provenance", "&'static str"),
            ("unit", "Unit"),
            ("value", "u64"),
            ("workers", "u32"),
        ]
        .map(|(name, kind)| (name.to_owned(), kind.to_owned()))
    );
}

fn identity(context: RunContext, flow: FlowKind) -> RunIdentity {
    RunIdentity::new(context, "public_api_flow", flow, "rust.public_api_test")
}

const WORKERS: WorkerDeclaration = WorkerDeclaration {
    workers: 2,
    provenance: "test.scoped_threads",
};

#[test]
fn a_complete_flow_is_recorded_through_the_public_interface_only() {
    let sink = CollectingSink::new();
    let budget = AllocationBudget::new(1 << 16);
    let session = Session::begin(
        identity(RunContext::unbound(), FlowKind::Verification),
        "verify",
        WORKERS,
        Box::new(sink.clone()),
    );
    session.observe_allocation_budget("verifier_pool", &budget);
    session.declare(Declaration {
        class: DeclaredClass::ConsensusBoundFromProtocolConstant,
        label: "max_proof_bytes",
        unit: Unit::Bytes,
        value: 9_437_184,
        provenance: "zk_x509/profile.rs:ZK_X509_MAX_PROOF_BYTES_V1",
    });
    let decode = session.enter("decode");
    session.record_bytes(ByteKind::Proof, "proof_frame", 4096);
    decode.complete();
    let openings = session.enter("openings");
    let parent = openings.parent();
    let handle = session.handle();
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let parent = parent.clone();
            let handle = handle.clone();
            scope.spawn(move || {
                let worker = parent.enter("merkle_paths");
                let counter = handle.allocation_counter("path_scratch");
                drop(counter.buffer(512));
                worker.complete();
            });
        }
    });
    openings.complete();
    session.finish();

    let mut records = sink.take();
    assert_eq!(records.len(), 1);
    let record = records.remove(0);
    assert_eq!(record.outcome, RunOutcome::Succeeded);
    assert_eq!(record.identity.flow, FlowKind::Verification);
    let labels: Vec<_> = record
        .phase_tree
        .nodes
        .iter()
        .map(|node| (node.label.as_str(), node.parent, node.calls))
        .collect();
    assert_eq!(
        labels,
        [
            ("verify", None, 1),
            ("decode", Some(0), 1),
            ("openings", Some(0), 1),
            ("merkle_paths", Some(2), 2),
        ]
    );
    assert_eq!(record.phase_tree.nodes[3].allocations, 2);
    assert_eq!(record.allocations.sources.len(), 3);
    assert_eq!(record.byte_counters.entries[0].total_bytes, 4096);
    assert_eq!(record.scheduling.workers, 2);
    assert!(unclassified_numbers(&record.to_json_value()).is_empty());
    // An unbound local run is retained, and its identity is reported incomplete.
    let findings = record.findings();
    assert!(findings.contains(&Finding::IdentityIncomplete { field: "hardware" }));
    assert!(findings.iter().all(|finding| matches!(
        finding,
        Finding::IdentityIncomplete { .. }
            | Finding::DirtyDigestMismatch
            | Finding::UnattributedExceedsLimit { .. }
    )));
    // Both wire forms carry the same record.
    let bytes = record.to_norito_bytes().unwrap();
    assert_eq!(
        MeasurementRecord::from_norito_bytes(&bytes).unwrap(),
        record
    );
    assert_eq!(
        MeasurementRecord::from_json_view(&record.to_json_view()).unwrap(),
        record
    );
}

fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "iroha-measurement-public-api-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn harness_handoff_binds_identity_and_retains_success_and_failure_records() {
    // The harness writes the context; the adapter is handed the directory.
    assert_eq!(HARNESS_OUTPUT_DIR_ENV, "IROHA_MEASUREMENT_OUTPUT_DIR");
    let directory = scratch("handoff");
    let context = RunContext {
        source_commit: "7e93d3e049".repeat(4),
        source_dirty: false,
        source_dirty_digest: None,
        artifact: "sha256:test-binary".into(),
        profile: "test".into(),
        config: "defaults".into(),
        hardware: "reference/unit-test-host".into(),
        cache_policy: iroha_measurement::CachePolicy::Warm,
    };
    std::fs::write(directory.join(HARNESS_CONTEXT_FILE), context.to_json_view()).unwrap();
    let bound = read_harness_context(&directory).unwrap();
    assert_eq!(bound, context);

    let run = |fail: bool| {
        let sink = DirectorySink::new(directory.clone(), "public_api_flow");
        let report = sink.report();
        let session = Session::begin(
            identity(bound.clone(), FlowKind::Native),
            "encrypt",
            WORKERS,
            Box::new(sink),
        );
        let phase = session.enter("keygen");
        if fail {
            session.record_failure("keygen", "rejected_parameters");
            drop(phase);
        } else {
            phase.complete();
        }
        session.finish();
        assert!(report.errors().is_empty());
        report.written().remove(0)
    };
    let success = run(false);
    let failure = run(true);
    assert_ne!(success, failure);
    // The failed run is a retained record, and the earlier success is intact.
    let decode = |path: &PathBuf| {
        MeasurementRecord::from_norito_bytes(&std::fs::read(path).unwrap()).unwrap()
    };
    let (succeeded, failed) = (decode(&success), decode(&failure));
    assert_eq!(succeeded.outcome, RunOutcome::Succeeded);
    assert_eq!(failed.outcome, RunOutcome::Failed);
    assert_eq!(failed.failures.entries[0].code, "rejected_parameters");
    assert_eq!(succeeded.identity.context, context);
    assert_eq!(failed.identity.context, context);
    assert!(failed.findings().contains(&Finding::RunNotSucceeded {
        outcome: RunOutcome::Failed
    }));
    assert_eq!(
        std::fs::read_dir(&directory).unwrap().count(),
        5,
        "context plus two records in two forms"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
