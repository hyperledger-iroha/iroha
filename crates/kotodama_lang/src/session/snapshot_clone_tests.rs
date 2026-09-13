//! Compiler snapshots and type names use their iterative owners on the caller stack.
use super::*;

#[test]
fn parsed_and_resolved_snapshots_clone_without_compiler_workers() {
    let expression = format!(
        "{}0{}",
        "[".repeat(crate::source::MAX_NESTING_DEPTH - 2),
        "]".repeat(crate::source::MAX_NESTING_DEPTH - 2)
    );
    let source = format!("seiyaku Clone {{ hajimari() {{ let value = {expression}; }} }}");
    let session = CompilerSession::default();
    let parsed = session
        .parse_compilation_unit(CompileRequest {
            source: &source,
            source_name: Some("clone.ko"),
        })
        .unwrap();
    reset_compiler_worker_spawn_count();
    let copy = parsed.clone();
    assert_eq!(compiler_worker_spawn_count(), 0);
    assert_eq!(parsed.program.program, copy.program.program);
    assert_eq!(parsed.source, copy.source);
    assert_eq!(parsed.source_name, copy.source_name);
    drop(copy);
    assert_eq!(compiler_worker_spawn_count(), 0);
    let resolved = session.resolve_compilation_unit(parsed).unwrap();
    reset_compiler_worker_spawn_count();
    let copy = resolved.clone();
    assert_eq!(compiler_worker_spawn_count(), 0);
    assert_eq!(resolved.program.get(), copy.program.get());
    assert_eq!(resolved.source, copy.source);
    assert_eq!(resolved.source_name, copy.source_name);
    drop(copy);
    drop(resolved);
    assert_eq!(compiler_worker_spawn_count(), 0);
}

#[test]
fn source_type_rendering_never_spawns_a_compiler_worker() {
    let mut ty = crate::semantic::Type::Int;
    for _ in 0..crate::source::MAX_NESTING_DEPTH {
        ty = crate::semantic::Type::Option(Box::new(ty));
    }
    reset_compiler_worker_spawn_count();
    assert_eq!(
        crate::semantic::render_type_name(&ty),
        format!(
            "{}int{}",
            "Option<".repeat(crate::source::MAX_NESTING_DEPTH),
            ">".repeat(crate::source::MAX_NESTING_DEPTH)
        )
    );
    assert_eq!(compiler_worker_spawn_count(), 0);
}
