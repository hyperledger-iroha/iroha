"""Regression test for audited panic recovery boundaries."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path


def load_guard_module():
    root = Path(__file__).resolve().parents[2]
    path = root / "scripts/check_panic_recovery_boundaries.py"
    spec = importlib.util.spec_from_file_location("check_panic_recovery_boundaries", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_panic_recovery_boundary_guard() -> None:
    root = Path(__file__).resolve().parents[2]
    completed = subprocess.run(
        [sys.executable, str(root / "scripts/check_panic_recovery_boundaries.py")],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_closed_inventory_rejects_a_new_torii_module(tmp_path: Path) -> None:
    module = load_guard_module()
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)
    source = tmp_path / "crates/iroha_torii/src/new_worker.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "fn run() { tokio::task::spawn_blocking(|| 1); }\n", encoding="utf-8"
    )

    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert "Torii spawn_blocking site count drifted (expected 0, found 1)" in failures
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_rejects_a_new_daemon_module(tmp_path: Path) -> None:
    module = load_guard_module()
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)
    source = tmp_path / "crates/irohad/src/new_provider_worker.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "fn run() { tokio::task::spawn_blocking(|| provider_call()); }\n",
        encoding="utf-8",
    )

    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert "Torii spawn_blocking site count drifted (expected 0, found 1)" in failures
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_rejects_a_new_untracked_git_module(tmp_path: Path) -> None:
    module = load_guard_module()
    tracked = tmp_path / "crates/iroha_torii/src/lib.rs"
    tracked.parent.mkdir(parents=True)
    tracked.write_text("fn stable() {}\n", encoding="utf-8")
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "add", str(tracked)], cwd=tmp_path, check=True)
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    untracked = tmp_path / "crates/iroha_torii/src/new_worker.rs"
    untracked.write_text(
        "fn run() { tokio::spawn(async { work().await }); }\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert "Torii task_spawn site count drifted (expected 0, found 1)" in failures
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_rejects_an_aliased_recovery_boundary(tmp_path: Path) -> None:
    module = load_guard_module()
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)
    source = tmp_path / "crates/iroha_torii/src/alias.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "use std::panic::catch_unwind as recover;\n"
        "fn run() { let _ = recover(|| 1); }\n",
        encoding="utf-8",
    )

    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert "Torii catch_unwind site count drifted (expected 0, found 1)" in failures
    assert any("source inventory drifted" in failure for failure in failures)
    assert module.torii_boundary_alias_failures(tmp_path) == [
        "crates/iroha_torii/src/alias.rs: catch_unwind boundary alias 'recover' "
        "is forbidden; use the audited spelling so cross-module calls remain visible"
    ]


def test_closed_inventory_binds_each_reviewed_call_site(tmp_path: Path) -> None:
    module = load_guard_module()
    relative = "crates/iroha_torii/src/critical_worker.rs"
    source = tmp_path / relative
    source.parent.mkdir(parents=True)
    source.write_text(
        "fn run() { tokio::task::spawn_blocking(|| 1); }\n", encoding="utf-8"
    )
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    assert (
        module.closed_torii_boundary_inventory_failures(
            tmp_path, expected_records
        )
        == []
    )

    source.write_text(
        "fn moved() { tokio::task::spawn_blocking(|| 1); }\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )
    assert not any("site count drifted" in failure for failure in failures)
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_rejects_bare_joined_task_recovery(tmp_path: Path) -> None:
    module = load_guard_module()
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)
    source = tmp_path / "crates/iroha_torii/src/joined.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "async fn run() {\n"
        "    let task = tokio::spawn(async { panic!(\"request panic\") });\n"
        "    let _controlled = task.await.map_err(|_| \"controlled\");\n"
        "}\n",
        encoding="utf-8",
    )

    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )
    assert "Torii task_spawn site count drifted (expected 0, found 1)" in failures
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_binds_join_handling_in_existing_spawn_unit(
    tmp_path: Path,
) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/joined.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "async fn run() {\n"
        "    let task = tokio::spawn(async { do_work().await });\n"
        "    task.await.expect(\"supervisor task must not panic\");\n"
        "}\n",
        encoding="utf-8",
    )
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    source.write_text(
        "async fn run() {\n"
        "    let task = tokio::spawn(async { do_work().await });\n"
        "    task.await.map_err(|_| \"controlled request error\")?;\n"
        "}\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert not any("site count drifted" in failure for failure in failures)
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_binds_complete_macro_recovery_unit(tmp_path: Path) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/macro_worker.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "worker_test! { catches_worker_panic\n"
        "    let result = std::panic::catch_unwind(|| work());\n"
        "    assert!(result.is_err());\n"
        "}\n",
        encoding="utf-8",
    )
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    source.write_text(
        "worker_test! { catches_worker_panic\n"
        "    let result = std::panic::catch_unwind(|| work());\n"
        "    return_controlled_error(result);\n"
        "}\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert not any("site count drifted" in failure for failure in failures)
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_binds_complete_macro_rules_definition(
    tmp_path: Path,
) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/macro_definition.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "macro_rules! launch {\n"
        "    ($task:expr) => {{\n"
        "        let task = tokio::spawn($task);\n"
        "        task\n"
        "    }};\n"
        "}\n",
        encoding="utf-8",
    )
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    source.write_text(
        "macro_rules! launch {\n"
        "    ($task:expr) => {{\n"
        "        let task = tokio::spawn($task);\n"
        "        task.await.map_err(|_| \"controlled\")\n"
        "    }};\n"
        "}\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert not any("site count drifted" in failure for failure in failures)
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_binds_parenthesized_macro_invocation(tmp_path: Path) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/parenthesized_macro.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "worker_test!( catches_worker_panic\n"
        "    let task = tokio::spawn(work());\n"
        "    task.await.expect(\"supervisor task must not panic\");\n"
        ");\n",
        encoding="utf-8",
    )
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    source.write_text(
        "worker_test!( catches_worker_panic\n"
        "    let task = tokio::spawn(work());\n"
        "    task.await.map_err(|_| \"controlled\")?;\n"
        ");\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert not any("site count drifted" in failure for failure in failures)
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_prefers_function_over_nested_macro_unit(tmp_path: Path) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/nested_macro.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "async fn run() {\n"
        "    let task = make_task! { tokio::spawn(work()) };\n"
        "    task.await.expect(\"supervisor task must not panic\");\n"
        "}\n",
        encoding="utf-8",
    )
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    source.write_text(
        "async fn run() {\n"
        "    let task = make_task! { tokio::spawn(work()) };\n"
        "    task.await.map_err(|_| \"controlled\")?;\n"
        "}\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert not any("site count drifted" in failure for failure in failures)
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_parses_array_return_signature(tmp_path: Path) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/array_return.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "async fn run() -> [u8; 32] {\n"
        "    let task = tokio::spawn(work());\n"
        "    task.await.expect(\"supervisor task must not panic\")\n"
        "}\n",
        encoding="utf-8",
    )
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    source.write_text(
        "async fn run() -> [u8; 32] {\n"
        "    let task = tokio::spawn(work());\n"
        "    task.await.map_err(|_| \"controlled\")?\n"
        "}\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert not any("site count drifted" in failure for failure in failures)
    assert any("source inventory drifted" in failure for failure in failures)


def test_closed_inventory_rejects_spawn_local(tmp_path: Path) -> None:
    module = load_guard_module()
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)
    source = tmp_path / "crates/iroha_torii/src/local.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "async fn run() {\n"
        "    tokio::task::spawn_local(async { panic!(\"request panic\") })\n"
        "        .await\n"
        "        .map_err(|_| \"controlled\")?;\n"
        "}\n",
        encoding="utf-8",
    )

    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert "Torii task_spawn site count drifted (expected 0, found 1)" in failures


def test_closed_inventory_rejects_unreviewed_websocket_upgrade_task(
    tmp_path: Path,
) -> None:
    module = load_guard_module()
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)
    source = tmp_path / "crates/iroha_torii/src/websocket.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "fn upgrade(ws: WebSocketUpgrade) {\n"
        "    ws.on_upgrade(|socket| async move { serve(socket).await });\n"
        "}\n",
        encoding="utf-8",
    )

    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert "Torii upgrade_task site count drifted (expected 0, found 1)" in failures


def test_boundary_alias_is_forbidden_across_modules(tmp_path: Path) -> None:
    module = load_guard_module()
    aliases = tmp_path / "crates/iroha_torii/src/aliases.rs"
    caller = tmp_path / "crates/iroha_torii/src/caller.rs"
    aliases.parent.mkdir(parents=True)
    aliases.write_text("pub use tokio::spawn as launch;\n", encoding="utf-8")
    caller.write_text(
        "fn run(task: Task) { crate::aliases::launch(task); }\n", encoding="utf-8"
    )

    failures = module.torii_boundary_alias_failures(tmp_path)

    assert failures == [
        "crates/iroha_torii/src/aliases.rs: task_spawn boundary alias 'launch' "
        "is forbidden; use the audited spelling so cross-module calls remain visible"
    ]


def test_boundary_alias_check_does_not_treat_cast_as_import_alias(
    tmp_path: Path,
) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/cast.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "fn run(handle: Handle, task: Task) {\n"
        "    let _opaque = handle.spawn(task) as usize;\n"
        "}\n",
        encoding="utf-8",
    )

    assert module.torii_boundary_alias_failures(tmp_path) == []


def test_boundary_alias_check_rejects_local_function_item_alias(tmp_path: Path) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/local_alias.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "fn run() {\n"
        "    let recover = std::panic::catch_unwind;\n"
        "    let _ = recover(|| provider_call());\n"
        "}\n",
        encoding="utf-8",
    )

    assert module.torii_boundary_alias_failures(tmp_path) == [
        "crates/iroha_torii/src/local_alias.rs: catch_unwind boundary alias "
        "'recover' is forbidden; use the audited spelling so cross-module calls "
        "remain visible"
    ]


def test_bare_blocking_check_rejects_join_set_method(tmp_path: Path) -> None:
    module = load_guard_module()
    source = (
        "fn run(tasks: &mut tokio::task::JoinSet<()>) {\n"
        "    tasks.spawn_blocking(|| provider_call());\n"
        "}\n"
    )

    assert module._bare_blocking_lines(source) == [2]


def test_bare_thread_check_rejects_direct_and_builder_spawns(tmp_path: Path) -> None:
    module = load_guard_module()
    source = (
        "fn run() {\n"
        "    std::thread::spawn(|| provider_call());\n"
        "    std::thread::Builder::new().spawn(|| provider_call());\n"
        "}\n"
    )

    assert module._bare_std_thread_lines(source) == [2, 3]


def test_inventory_counts_join_set_and_std_thread_boundaries(tmp_path: Path) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/irohad/src/provider_worker.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "fn run(tasks: &mut tokio::task::JoinSet<()>) {\n"
        "    tasks.spawn_blocking(|| provider_call());\n"
        "    std::thread::Builder::new().spawn(|| provider_call());\n"
        "}\n",
        encoding="utf-8",
    )

    _, _, counts = module.torii_boundary_inventory(tmp_path)

    assert counts["spawn_blocking"] == 1
    assert counts["task_spawn"] == 1


def test_closed_inventory_rejects_spawn_on_families(tmp_path: Path) -> None:
    module = load_guard_module()
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)
    source = tmp_path / "crates/iroha_torii/src/on.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "fn run(runtime: Runtime, task: Task) {\n"
        "    runtime.spawn_on(task);\n"
        "    runtime.spawn_blocking_on(|| work());\n"
        "}\n",
        encoding="utf-8",
    )

    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert "Torii task_spawn site count drifted (expected 0, found 1)" in failures
    assert "Torii spawn_blocking site count drifted (expected 0, found 1)" in failures


def test_closed_inventory_rejects_spawn_turbofish_and_comment(tmp_path: Path) -> None:
    module = load_guard_module()
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)
    source = tmp_path / "crates/iroha_torii/src/turbofish.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        "fn run(handle: Handle, task: Task) {\n"
        "    handle.spawn::<_>(task);\n"
        "    handle.spawn /* spelling gap */ (task);\n"
        "}\n",
        encoding="utf-8",
    )

    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert "Torii task_spawn site count drifted (expected 0, found 2)" in failures


def test_closed_inventory_binds_external_build_support_source(tmp_path: Path) -> None:
    module = load_guard_module()
    manifest = tmp_path / "crates/iroha_torii/Cargo.toml"
    build_script = tmp_path / "crates/build-support/script.rs"
    build_library = tmp_path / "crates/build-support/src/lib.rs"
    manifest.parent.mkdir(parents=True)
    build_library.parent.mkdir(parents=True)
    manifest.write_text(
        '[package]\nname = "iroha_torii"\nbuild = "../build-support/script.rs"\n',
        encoding="utf-8",
    )
    build_script.write_text("fn main() { build_support::emit(); }\n", encoding="utf-8")
    build_library.write_text("pub fn emit() {}\n", encoding="utf-8")
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    build_library.write_text(
        "pub fn emit() { std::thread::spawn(|| panic!(\"build panic\")); }\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )

    assert any("source inventory drifted" in failure for failure in failures)


def test_source_closure_rejects_symlink_indirection(tmp_path: Path) -> None:
    module = load_guard_module()
    torii = tmp_path / "crates/iroha_torii"
    build_support = tmp_path / "crates/build-support"
    outside = tmp_path / "outside.rs"
    torii.mkdir(parents=True)
    build_support.mkdir(parents=True)
    (torii / "Cargo.toml").write_text(
        '[package]\nname = "iroha_torii"\nbuild = "../build-support/script.rs"\n',
        encoding="utf-8",
    )
    (build_support / "script.rs").write_text("fn main() {}\n", encoding="utf-8")
    outside.write_text("pub fn outside() {}\n", encoding="utf-8")
    (torii / "outside.rs").symlink_to(outside)

    failures = module.torii_source_path_failures(tmp_path)

    assert failures == [
        "crates/iroha_torii/outside.rs: symlink is forbidden in the audited source closure"
    ]


def test_source_closure_rejects_unreviewed_build_script_path(tmp_path: Path) -> None:
    module = load_guard_module()
    torii = tmp_path / "crates/iroha_torii"
    build_support = tmp_path / "crates/build-support"
    torii.mkdir(parents=True)
    build_support.mkdir(parents=True)
    (build_support / "script.rs").write_text("fn main() {}\n", encoding="utf-8")
    (torii / "Cargo.toml").write_text(
        '[package]\nname = "iroha_torii"\nbuild = "../../outside.rs"\n',
        encoding="utf-8",
    )

    failures = module.torii_source_path_failures(tmp_path)

    assert (
        "crates/iroha_torii/Cargo.toml: package build script escapes the audited "
        "source roots: ../../outside.rs"
    ) in failures
    assert (
        "crates/iroha_torii/Cargo.toml: build script escaped the audited source closure "
        "(expected crates/build-support/script.rs)"
    ) in failures


def test_source_closure_rejects_escaping_explicit_cargo_targets(
    tmp_path: Path,
) -> None:
    module = load_guard_module()
    torii = tmp_path / "crates/iroha_torii"
    build_support = tmp_path / "crates/build-support"
    torii.mkdir(parents=True)
    build_support.mkdir(parents=True)
    (build_support / "script.rs").write_text("fn main() {}\n", encoding="utf-8")
    (tmp_path / "outside.rs").write_text("fn main() {}\n", encoding="utf-8")
    (torii / "Cargo.toml").write_text(
        '[package]\nname = "iroha_torii"\nbuild = "../build-support/script.rs"\n'
        '[lib]\npath = "../../outside.rs"\n'
        '[[bin]]\nname = "escape"\npath = "../../outside.rs"\n'
        '[[example]]\nname = "escape"\npath = "../../outside.rs"\n'
        '[[test]]\nname = "escape"\npath = "../../outside.rs"\n'
        '[[bench]]\nname = "escape"\npath = "../../outside.rs"\n',
        encoding="utf-8",
    )

    failures = module.torii_source_path_failures(tmp_path)

    for label in (
        "lib target",
        "bin target #1",
        "example target #1",
        "test target #1",
        "bench target #1",
    ):
        assert any(
            f"Cargo.toml: {label} escapes the audited source roots: ../../outside.rs"
            in failure
            for failure in failures
        )


def test_source_closure_rejects_unsealed_explicit_cargo_target(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_guard_module()
    torii = tmp_path / "crates/iroha_torii"
    build_support = tmp_path / "crates/build-support"
    source = torii / "src/generated.rs"
    source.parent.mkdir(parents=True)
    build_support.mkdir(parents=True)
    (build_support / "script.rs").write_text("fn main() {}\n", encoding="utf-8")
    source.write_text("fn main() {}\n", encoding="utf-8")
    manifest = torii / "Cargo.toml"
    manifest.write_text(
        '[package]\nname = "iroha_torii"\nbuild = "../build-support/script.rs"\n'
        '[[bin]]\nname = "generated"\npath = "src/generated.rs"\n',
        encoding="utf-8",
    )
    sealed = {manifest.resolve(), (build_support / "script.rs").resolve()}
    monkeypatch.setattr(
        module,
        "torii_audited_files",
        lambda _root: sorted(sealed),
    )

    failures = module.torii_source_path_failures(tmp_path)

    assert (
        "crates/iroha_torii/Cargo.toml: bin target #1 is outside the sealed "
        "repository-file inventory: src/generated.rs"
    ) in failures


def test_source_closure_rejects_gitlinks_under_audited_roots(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_guard_module()
    torii = tmp_path / "crates/iroha_torii"
    torii.mkdir(parents=True)
    gitlink = Path("crates/iroha_torii/vendor/provider")
    monkeypatch.setattr(
        module,
        "_git_audited_entries",
        lambda _root: [("160000", gitlink)],
    )

    failures = module.torii_source_path_failures(tmp_path)

    assert (
        "crates/iroha_torii/vendor/provider: gitlink/submodule is forbidden in "
        "the audited source closure"
    ) in failures


def test_source_closure_rejects_unresolved_git_index_stages(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_guard_module()
    torii = tmp_path / "crates/iroha_torii"
    torii.mkdir(parents=True)
    conflicted = Path("crates/iroha_torii/src/lib.rs")
    monkeypatch.setattr(
        module,
        "_git_audited_entries",
        lambda _root: [("conflict:2:100644", conflicted)],
    )

    failures = module.torii_source_path_failures(tmp_path)

    assert (
        "crates/iroha_torii/src/lib.rs: unresolved Git index stage 2 is forbidden "
        "in the audited source closure"
    ) in failures


def test_source_closure_inventories_transitive_non_rs_include(tmp_path: Path) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/lib.rs"
    included = tmp_path / "crates/iroha_torii/src/worker.inc"
    source.parent.mkdir(parents=True)
    source.write_text('include!("worker.inc");\n', encoding="utf-8")
    included.write_text(
        "fn run() { tokio::spawn(async { work().await }); }\n",
        encoding="utf-8",
    )
    expected_records, _, counts = module.torii_boundary_inventory(tmp_path)

    assert any(
        record.startswith("crates/iroha_torii/src/worker.inc\t")
        for record in expected_records
    )
    assert counts["task_spawn"] == 1

    included.write_text(
        "fn run() { tokio::spawn(async { changed().await }); }\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )
    assert not any("site count drifted" in failure for failure in failures)
    assert any("source inventory drifted" in failure for failure in failures)


def test_source_closure_inventories_brace_and_bracket_includes(tmp_path: Path) -> None:
    module = load_guard_module()
    for label, invocation in (
        ("brace", 'include! { "worker.inc" }\n'),
        ("bracket", 'include!["worker.inc"]\n'),
    ):
        root = tmp_path / label
        source = root / "crates/iroha_torii/src/lib.rs"
        included = root / "crates/iroha_torii/src/worker.inc"
        source.parent.mkdir(parents=True)
        source.write_text(invocation, encoding="utf-8")
        included.write_text(
            "fn run() { tokio::spawn(async { work().await }); }\n",
            encoding="utf-8",
        )
        expected_records, _, counts = module.torii_boundary_inventory(root)
        assert any(
            record.startswith("crates/iroha_torii/src/worker.inc\t")
            for record in expected_records
        )
        assert counts["task_spawn"] == 1

        included.write_text(
            "fn run() { tokio::spawn(async { changed().await }); }\n",
            encoding="utf-8",
        )
        failures = module.closed_torii_boundary_inventory_failures(
            root, expected_records
        )
        assert not any("site count drifted" in failure for failure in failures)
        assert any("source inventory drifted" in failure for failure in failures)


def test_source_closure_rejects_escaped_include_and_path_attribute(
    tmp_path: Path,
) -> None:
    module = load_guard_module()
    source_root = tmp_path / "crates/iroha_torii/src"
    source_root.mkdir(parents=True)
    (tmp_path / "outside.rs").write_text("fn outside() {}\n", encoding="utf-8")
    (source_root / "included.rs").write_text(
        'include!("../../../outside.rs");\n', encoding="utf-8"
    )
    (source_root / "module.rs").write_text(
        '#[path = "../../../outside.rs"]\nmod outside;\n', encoding="utf-8"
    )

    failures = module.torii_source_path_failures(tmp_path)

    assert any(
        "included.rs:1: include! source path escapes the audited source roots"
        in failure
        for failure in failures
    )
    assert any(
        "module.rs:1: #[path] source path escapes the audited source roots"
        in failure
        for failure in failures
    )


def test_source_closure_rejects_dynamic_include_path(tmp_path: Path) -> None:
    module = load_guard_module()
    source = tmp_path / "crates/iroha_torii/src/lib.rs"
    source.parent.mkdir(parents=True)
    source.write_text(
        'include!(concat!("worker", ".inc"));\n', encoding="utf-8"
    )

    failures = module.torii_source_path_failures(tmp_path)

    assert any(
        "lib.rs:1: include! must name one static local source file" in failure
        for failure in failures
    )


def test_complete_file_closure_binds_nested_inline_module_path_target(
    tmp_path: Path,
) -> None:
    module = load_guard_module()
    source_root = tmp_path / "crates/iroha_torii/src"
    nested = source_root / "outer/worker.inc"
    decoy = source_root / "worker.inc"
    nested.parent.mkdir(parents=True)
    source_root.mkdir(parents=True, exist_ok=True)
    (source_root / "lib.rs").write_text(
        'mod outer { #[path = "worker.inc"] mod worker; }\n', encoding="utf-8"
    )
    decoy.write_text("fn decoy() {}\n", encoding="utf-8")
    nested.write_text(
        "fn run() { tokio::spawn(async { work().await }); }\n",
        encoding="utf-8",
    )
    expected_records, _, _ = module.torii_boundary_inventory(tmp_path)

    assert any(
        record.startswith("crates/iroha_torii/src/outer/worker.inc\t")
        for record in expected_records
    )
    nested.write_text(
        "fn run() { tokio::spawn(async { changed().await }); }\n",
        encoding="utf-8",
    )
    failures = module.closed_torii_boundary_inventory_failures(
        tmp_path, expected_records
    )
    assert any("source inventory drifted" in failure for failure in failures)


def test_source_closure_rejects_ignored_conventional_module(tmp_path: Path) -> None:
    module = load_guard_module()
    source_root = tmp_path / "crates/iroha_torii/src"
    source_root.mkdir(parents=True)
    root_source = source_root / "lib.rs"
    hidden = source_root / "hidden.rs"
    root_source.write_text("mod hidden;\n", encoding="utf-8")
    hidden.write_text(
        "fn run() { tokio::task::spawn_blocking(|| provider_call()); }\n",
        encoding="utf-8",
    )
    (tmp_path / ".gitignore").write_text(
        "crates/iroha_torii/src/hidden.rs\n", encoding="utf-8"
    )
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(
        ["git", "add", ".gitignore", "crates/iroha_torii/src/lib.rs"],
        cwd=tmp_path,
        check=True,
    )

    records, _, counts = module.torii_boundary_inventory(tmp_path)
    failures = module.torii_source_path_failures(tmp_path)

    assert any(
        record.startswith("crates/iroha_torii/src/hidden.rs\t")
        for record in records
    )
    assert counts["spawn_blocking"] == 1
    assert (
        "crates/iroha_torii/src/hidden.rs: textual module source is outside the "
        "sealed repository-file inventory"
    ) in failures


def test_source_closure_resolves_nested_inline_path_before_sealing(tmp_path: Path) -> None:
    module = load_guard_module()
    source_root = tmp_path / "crates/iroha_torii/src"
    nested = source_root / "outer/worker.inc"
    decoy = source_root / "worker.inc"
    nested.parent.mkdir(parents=True)
    root_source = source_root / "lib.rs"
    root_source.write_text(
        'mod outer { #[path = "worker.inc"] mod worker; }\n', encoding="utf-8"
    )
    decoy.write_text("fn decoy() {}\n", encoding="utf-8")
    nested.write_text(
        "fn run() { tokio::task::spawn_blocking(|| provider_call()); }\n",
        encoding="utf-8",
    )
    (tmp_path / ".gitignore").write_text(
        "crates/iroha_torii/src/outer/worker.inc\n", encoding="utf-8"
    )
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(
        [
            "git",
            "add",
            ".gitignore",
            "crates/iroha_torii/src/lib.rs",
            "crates/iroha_torii/src/worker.inc",
        ],
        cwd=tmp_path,
        check=True,
    )

    records, _, counts = module.torii_boundary_inventory(tmp_path)
    failures = module.torii_source_path_failures(tmp_path)

    assert any(
        record.startswith("crates/iroha_torii/src/outer/worker.inc\t")
        for record in records
    )
    assert counts["spawn_blocking"] == 1
    assert (
        "crates/iroha_torii/src/outer/worker.inc: textual module source is outside "
        "the sealed repository-file inventory"
    ) in failures
