"""Exercise the actual current-tree IVM-only guard against isolated Git repos."""

from pathlib import Path
import os
import subprocess

import pytest


GUARD = Path(__file__).resolve().parents[2] / "scripts/check_ivm_only.py"


@pytest.fixture
def repository(tmp_path):
    subprocess.run(["git", "init", "-q", str(tmp_path)], check=True)
    return tmp_path


def run_guard(root):
    return subprocess.run(
        ["python3", str(GUARD), "--root", str(root)], capture_output=True, text=True,
        env={**os.environ, "STD_ONLY_GUARD_ALLOW": "1", "AGENTS_BASE_REF": "missing-ref"},
    )


@pytest.mark.parametrize("relative,contents", [
    ("crates/codec/Cargo.toml", '[dependencies]\nwasm-bindgen = "0.2"\n'),
    ("crates/codec/src/lib.rs", '#![cfg_attr(feature = "compact", no_std)]\n'),
    ("crates/codec/src/lib.rs", '#[cfg(target_arch = "wasm32")]\nfn alternate() {}\n'),
    ("crates/codec/src/lib.rs", 'pub struct Budget { pub allow_wasi: bool }\n'),
    (".cargo/config.toml", '[build]\ntarget = "wasm64-unknown-unknown"\n'),
    ("javascript/codec.mjs", 'await WebAssembly.instantiate(bytes);\n'),
    ("javascript/package.json", '{"scripts":{"build":"wasm-pack build"}}\n'),
    ("ci/build.sh", 'cargo build --target wasm32-unknown-unknown\n'),
    ("crates/app/src/vendor/runtime.rs", "use wasmtime::Engine;\n"),
    ("vendor/iroha_owned/src/lib.rs", "use wasmtime::Engine;\n"),
    ("crates/app/src/node_modules/runtime.rs", "use wasmtime::Engine;\n"),
    ("crates/app/Cargo.toml", '[dependencies]\nwasmi = "0.46"\n'),
    ("javascript/run.mjs", 'import { WASI } from "node:wasi";\n'),
    ("javascript/run.cjs", 'const { WASI } = require("node:wasi");\n'),
    ("javascript/run.mjs", 'const runtime = await import("node:wasi");\n'),
    ("javascript/build.mjs", 'execFileSync("cargo", ["build", "--target", "wasm32-unknown-unknown"]);\n'),
    ("javascript/view.tsx", "const module = await WebAssembly.compile(bytes);\n"),
    ("javascript/view.jsx", "const module = await WebAssembly.compile(bytes);\n"),
    ("kotlin/codec/build.gradle.kts", "kotlin { wasmJs { browser() } }\n"),
    ("kotlin/codec/build.gradle.kts", "kotlin { wasmWasi { nodejs() } }\n"),
    ("nix/build.nix", '{ buildTarget = "wasm32-unknown-unknown"; }\n'),
    ("codec/CMakeLists.txt", 'set(CMAKE_C_FLAGS "--target=wasm32-unknown-unknown")\n'),
    ("crates/app/src/lib.rs", "#![\n    no_std\n]\n"),
    ("crates/app/src/lib.rs", '#![cfg_attr(\n    feature = "compact",\n    no_std\n)]\n'),
    ("defaults/compute.toml", "[compute.profile]\nallow_wasi = true\n"),
    ("defaults/compute.json", '{"profile": {"allow_wasi": false}}\n'),
    ("javascript/run.mjs", "const vm = globalThis.WebAssembly;\nawait vm.instantiate(bytes);\n"),
])
def test_untracked_and_tracked_forbidden_support_fail(repository, relative, contents):
    path = repository / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents)
    assert run_guard(repository).returncode == 1
    subprocess.run(["git", "-C", str(repository), "add", "."], check=True)
    result = run_guard(repository)
    assert result.returncode == 1
    assert relative in result.stderr
    path.unlink()
    assert run_guard(repository).returncode == 0


@pytest.mark.parametrize("filename,payload", [
    ("program.wasm", b"anything"),
    ("program.wat", b"(module)"),
    ("program.to", b"\0asm\x01\0\0\0"),
    ("docs/history/2026-09-13/runtime.wasm", b"\0asm\x01\0\0\0"),
    ("docs/history/2026-09-13/runtime.to", b"\0asm\x01\0\0\0"),
    ("vendor/streebog/fixture.wasm", b"anything"),
    ("vendor/streebog/fixture.bin", b"\0asm\x01\0\0\0"),
])
def test_artifacts_cannot_hide_behind_extension_or_build_state(repository, filename, payload):
    path = repository / filename
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    assert run_guard(repository).returncode == 1
    subprocess.run(["git", "-C", str(repository), "add", "."], check=True)
    assert run_guard(repository).returncode == 1


def test_native_ivm_and_upstream_target_metadata_are_allowed(repository):
    files = {
        "crates/ivm/src/lib.rs": "//! IVM executes Kotodama bytecode.\nfn execute() {}\n",
        "Cargo.lock": '[[package]]\nname = "wasm-bindgen"\nversion = "0.2"\n',
        "vendor/streebog/src/lib.rs": '#[cfg(target_arch = "wasm32")]\nfn upstream() {}\n',
        "docs/policy.md": "Wasm is prohibited; use native IVM.\n",
        "crates/gateway.rs": 'const ACTIVE_MIME: &str = "application/wasm";\n',
        "crates/model/tests.rs": 'assert!(from_json(r#"{\"allow_wasi\":true}"#).is_err());\n',
        "crates/model/removed_shape_tests.rs": 'for mode in ["IvmOnly", "WasiLite"] { assert!(reject(mode)); }\n',
        "docs/history/2026-09-13/prior_build.sh": "cargo build --target wasm32-unknown-unknown\n",
        "javascript/package-lock.json": '{"packages":{"node_modules/wasm-bindgen":{"optional":true}}}\n',
    }
    for name, contents in files.items():
        path = repository / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents)
    assert run_guard(repository).returncode == 0


def test_unstaged_mutation_is_checked(repository):
    path = repository / "Cargo.toml"
    path.write_text('[package]\nname = "native"\n')
    subprocess.run(["git", "-C", str(repository), "add", "."], check=True)
    path.write_text('[dependencies]\nwasmtime = "1"\n')
    assert run_guard(repository).returncode == 1


def test_ignored_untracked_outputs_are_outside_source_but_tracked_files_are_checked(repository):
    (repository / ".gitignore").write_text("generated/\n")
    path = repository / "generated/runtime.mjs"
    path.parent.mkdir()
    path.write_text("await WebAssembly.instantiate(bytes);\n")
    assert run_guard(repository).returncode == 0
    subprocess.run(["git", "-C", str(repository), "add", "-f", str(path)], check=True)
    assert run_guard(repository).returncode == 1


@pytest.mark.parametrize("relative,contents", [
    ("circuits/sccp/vendor/cpu/cpu_wasm.go", "package cpu\n"),
    ("circuits/sccp/vendor/cpu/cpu_wasip1.go", "package cpu\n"),
    ("circuits/sccp/vendor/cpu/cpu_wasm_test.go", "package cpu\n"),
    ("circuits/sccp/vendor/cpu/endian.go", "//go:build amd64 || wasm\npackage cpu\n"),
    ("circuits/sccp/vendor/tty/platform.go", "//go:build !wasip1\npackage tty\n"),
    ("circuits/sccp/vendor/tty/platform.go", "// +build amd64,linux wasm\npackage tty\n"),
])
def test_go_target_files_and_build_comments_are_checked(repository, relative, contents):
    path = repository / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents)
    assert run_guard(repository).returncode == 1
    subprocess.run(["git", "-C", str(repository), "add", "."], check=True)
    assert run_guard(repository).returncode == 1


NEGATIVE_TARGET_FIXTURE = '''const unsupportedTarget = "wasm32-" + "unknown-unknown";
assert.equal(
  readRepositoryFile(".github/workflows/kotodama_perf.yml").includes(
    unsupportedTarget,
  ),
  false,
);
'''


def test_literal_target_absence_assertion_is_allowed_without_hiding_runtime_uses(repository):
    path = repository / "javascript/targetRemoval.test.js"
    path.parent.mkdir()
    path.write_text(NEGATIVE_TARGET_FIXTURE)
    assert run_guard(repository).returncode == 0
    subprocess.run(["git", "-C", str(repository), "add", "."], check=True)
    for extra in [
        'execFileSync("cargo", ["build", "--target", unsupportedTarget]);\n',
        'const target = unsupportedTarget;\n',
        'export { unsupportedTarget };\n',
        'eval(unsupportedTarget);\n',
        'await WebAssembly.instantiate(bytes);\n',
        'import { WASI } from "node:wasi";\n',
    ]:
        path.write_text(NEGATIVE_TARGET_FIXTURE + extra)
        assert run_guard(repository).returncode == 1, extra


@pytest.mark.parametrize("filename,contents", [
    ("runtime.mjs", NEGATIVE_TARGET_FIXTURE),
    ("targetRemoval.test.js", NEGATIVE_TARGET_FIXTURE.replace("false,", "true,")),
    ("targetRemoval.test.js", NEGATIVE_TARGET_FIXTURE.replace(
        'const unsupportedTarget = "wasm32-" + "unknown-unknown";',
        'const unsupportedTarget = select("wasm32-unknown-unknown");',
    )),
])
def test_target_fixture_exception_requires_literal_data_and_negative_test_use(repository, filename, contents):
    path = repository / filename
    path.write_text(contents)
    assert run_guard(repository).returncode == 1
