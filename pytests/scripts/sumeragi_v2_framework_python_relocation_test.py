from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import sysconfig
from typing import Any

import pytest


ROOT = Path(__file__).resolve().parents[2]
HELPER_PATH = ROOT / "scripts/copy_sumeragi_v2_release_cargo_cache.py"
CLI_PATH = ROOT / "scripts/copy_sumeragi_v2_release_cargo_cache_cli.py"


def _load_helper():
    spec = importlib.util.spec_from_file_location("framework_relocation_helper", HELPER_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _load_cli() -> dict[str, object]:
    scope = {"__file__": str(CLI_PATH), "__name__": "framework_relocation_cli"}
    exec(compile(CLI_PATH.read_bytes(), str(CLI_PATH), "exec"), scope)
    return scope


class _ReceiptFailure(RuntimeError):
    pass


def _load_publication() -> dict[str, object]:
    path = ROOT / "scripts/write_sumeragi_v2_release_receipt_publication.py"
    scope = {
        "__file__": str(path), "Any": Any, "Path": Path,
        "PurePosixPath": __import__("pathlib").PurePosixPath,
        "ReceiptError": _ReceiptFailure, "re": __import__("re"),
        "_DIGEST_RE": __import__("re").compile(r"[0-9a-f]{64}"),
        "_MAX_FRAMEWORK_RUNTIME_MEMBERS": 250_000,
        "_MAX_FRAMEWORK_RUNTIME_BYTES": 4 * 1024 * 1024 * 1024,
    }
    exec(compile(path.read_bytes(), str(path), "exec"), scope)
    return scope


def _digest(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def _vector(dependencies: list[str]) -> str:
    return hashlib.sha256(
        json.dumps(dependencies, ensure_ascii=False, separators=(",", ":")).encode()
    ).hexdigest()


def _contract() -> dict[str, object]:
    tools = {
        name: {
            "path": f"/usr/bin/{name}",
            "mode": "0755",
            "sha256": _digest(f"tool:{name}"),
            "size_bytes": 128,
        }
        for name in ("codesign", "install_name_tool", "otool")
    }
    artifacts = {}
    for name, path, rewritten in (
        ("launcher", "bin/python3", "@executable_path/../Python"),
        (
            "trampoline",
            "Resources/Python.app/Contents/MacOS/Python",
            "@executable_path/../../../../Python",
        ),
    ):
        source_dependencies = (
            ["/fixture/Python", "/usr/lib/libSystem.B.dylib"]
            if name == "launcher"
            else ["/System/CoreFoundation", "/fixture/Python",
                  "/usr/lib/libSystem.B.dylib"]
        )
        derived_dependencies = [
            rewritten if dependency == "/fixture/Python" else dependency
            for dependency in source_dependencies
        ]
        artifacts[name] = {
            "path": path,
            "source": {
                "mode": "0755",
                "sha256": _digest(f"source:{name}"),
                "size_bytes": 100,
                "framework_dependency_sha256": _digest("/fixture/Python"),
                "dependency_vector_sha256": _vector(source_dependencies),
            },
            "derived": {
                "mode": "0500",
                "sha256": _digest(f"derived:{name}"),
                "size_bytes": 120,
                "framework_dependency": rewritten,
                "dependency_vector_sha256": _vector(derived_dependencies),
                "codesign": "adhoc",
            },
        }
    return {
        "format": "iroha-sumeragi-v2-framework-python-relocation",
        "schema_version": 1,
        "framework": "Python",
        "tools": tools,
        "artifacts": artifacts,
    }


def _verification_fixture(monkeypatch: pytest.MonkeyPatch):
    scope = _load_cli()
    contract = _contract()
    old = "/fixture/Python"
    dependencies = {
        "source-launcher": [old, "/usr/lib/libSystem.B.dylib"],
        "source-trampoline": ["/System/CoreFoundation", old, "/usr/lib/libSystem.B.dylib"],
        "python3": ["@executable_path/../Python", "/usr/lib/libSystem.B.dylib"],
        "Python": [
            "/System/CoreFoundation", "@executable_path/../../../../Python",
            "/usr/lib/libSystem.B.dylib",
        ],
    }
    monkeypatch.setitem(scope, "_preflight_macho", lambda *args, **kwargs: None)
    monkeypatch.setitem(scope, "_require_adhoc_signature", lambda *args, **kwargs: None)
    monkeypatch.setitem(
        scope,
        "_macho_tool_record",
        lambda name, digest_regular, error_type: copy.deepcopy(contract["tools"][name]),
    )
    def observed_dependencies(path, otool, error_type):
        if str(path).startswith("/fixture/Resources/"):
            key = "source-trampoline"
        else:
            key = path.name
        return list(dependencies[key])

    monkeypatch.setitem(scope, "_macho_dependencies", observed_dependencies)

    def artifact(path, observed, framework_dependency, digest_regular, error_type, *, source):
        name = "launcher" if path.name in {"source-launcher", "python3"} else "trampoline"
        side = "source" if source else "derived"
        record = copy.deepcopy(contract["artifacts"][name][side])
        record["dependency_vector_sha256"] = _vector(observed)
        return record

    monkeypatch.setitem(scope, "_artifact_record", artifact)
    arguments = {
        "version_root": Path("/fixture"),
        "framework": "Python",
        "source_python": Path("/fixture/bin/source-launcher"),
        "runtime_root": Path("/archive"),
        "contract": contract,
        "digest_regular": lambda *args: None,
        "error_type": RuntimeError,
    }
    return scope, contract, dependencies, arguments


def _inventory_records(contract: dict[str, object]):
    inputs = []
    outputs = [{
        "path": "Python", "kind": "file", "device": 1, "inode": 1,
        "mode": "0500", "sha256": _digest("framework"), "size": 64,
    }]
    for name, input_path in (
        ("launcher", "python3"),
        ("trampoline", "Resources/Python.app/Contents/MacOS/Python"),
    ):
        artifact = contract["artifacts"][name]
        inputs.append({
            "path": input_path, "kind": "file", "source_device": 1,
            "source_inode": len(inputs) + 2, "source_mode": artifact["source"]["mode"],
            "destination_device": 2, "destination_inode": len(inputs) + 3,
            "destination_mode": artifact["derived"]["mode"],
            "sha256": artifact["source"]["sha256"],
            "size": artifact["source"]["size_bytes"],
        })
        outputs.append({
            "path": artifact["path"], "kind": "file", "device": 1,
            "inode": len(outputs) + 2, "mode": artifact["derived"]["mode"],
            "sha256": artifact["derived"]["sha256"],
            "size": artifact["derived"]["size_bytes"],
        })
    inputs.sort(key=lambda record: record["path"])
    outputs.sort(key=lambda record: record["path"])
    return inputs, outputs


def test_relocation_contract_rejects_wrong_rewrite_and_unsigned_output() -> None:
    scope = _load_cli()
    validate = scope["_validate_framework_python_relocation_contract"]
    for field, value in (
        ("framework_dependency", "@rpath/Python"),
        ("codesign", "unsigned"),
    ):
        changed = _contract()
        changed["artifacts"]["launcher"]["derived"][field] = value
        with pytest.raises(RuntimeError, match="artifact binding"):
            validate(changed, RuntimeError)
    changed = _contract()
    changed["artifacts"]["launcher"]["source"]["mode"] = "0777"
    with pytest.raises(RuntimeError, match="digest binding"):
        validate(changed, RuntimeError)
    changed = _contract()
    changed["artifacts"]["launcher"]["source"]["size_bytes"] = (
        256 * 1024 * 1024 + 1
    )
    with pytest.raises(RuntimeError, match="digest binding"):
        validate(changed, RuntimeError)


def test_verifier_rejects_residual_absolute_dependency(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    scope, _, dependencies, arguments = _verification_fixture(monkeypatch)
    dependencies["python3"] = [
        "/fixture/Python", "@executable_path/../Python", "/usr/lib/libSystem.B.dylib"
    ]
    with pytest.raises(RuntimeError, match="relocated dependency"):
        scope["verify_framework_python_relocation"](**arguments)


def test_verifier_rejects_wrong_rewrite_and_unsigned_output(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    scope, _, dependencies, arguments = _verification_fixture(monkeypatch)
    dependencies["Python"][1] = "@rpath/Python"
    with pytest.raises(RuntimeError, match="relocated dependency"):
        scope["verify_framework_python_relocation"](**arguments)
    dependencies["Python"][1] = "@executable_path/../../../../Python"
    monkeypatch.setitem(
        scope,
        "_require_adhoc_signature",
        lambda *args, **kwargs: (_ for _ in ()).throw(RuntimeError("unsigned")),
    )
    with pytest.raises(RuntimeError, match="unsigned"):
        scope["verify_framework_python_relocation"](**arguments)


def test_verifier_rejects_authenticated_tool_digest_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    scope, contract, _, arguments = _verification_fixture(monkeypatch)

    def drift(name, digest_regular, error_type):
        record = copy.deepcopy(contract["tools"][name])
        if name == "otool":
            record["sha256"] = _digest("changed otool")
        return record

    monkeypatch.setitem(scope, "_macho_tool_record", drift)
    with pytest.raises(RuntimeError, match="provenance changed"):
        scope["verify_framework_python_relocation"](**arguments)


def test_verifier_rejects_source_dependency_vector_drift(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    scope, _, dependencies, arguments = _verification_fixture(monkeypatch)
    dependencies["source-launcher"].append("/usr/lib/libChanged.dylib")
    dependencies["python3"].append("/usr/lib/libChanged.dylib")
    with pytest.raises(RuntimeError, match="provenance changed"):
        scope["verify_framework_python_relocation"](**arguments)


@pytest.mark.parametrize("side,field", [
    ("source", "sha256"),
    ("source", "dependency_vector_sha256"),
    ("derived", "sha256"),
    ("derived", "dependency_vector_sha256"),
])
def test_verifier_rejects_source_and_derived_drift(
    monkeypatch: pytest.MonkeyPatch, side: str, field: str,
) -> None:
    scope, contract, _, arguments = _verification_fixture(monkeypatch)
    original = scope["_artifact_record"]

    def drift(*args, source, **kwargs):
        record = original(*args, source=source, **kwargs)
        if ("source" if source else "derived") == side and args[0].name in {
            "source-launcher", "python3",
        }:
            record[field] = _digest(f"drift:{side}:{field}")
        return record

    monkeypatch.setitem(scope, "_artifact_record", drift)
    with pytest.raises(RuntimeError, match="provenance changed"):
        scope["verify_framework_python_relocation"](**arguments)


def test_binding_rejects_inventory_digest_drift() -> None:
    scope = _load_cli()
    contract = _contract()
    inputs, outputs = _inventory_records(contract)
    next(record for record in outputs if record["path"] == "bin/python3")[
        "sha256"
    ] = _digest("inventory drift")
    with pytest.raises(RuntimeError, match="source/derived binding"):
        scope["bind_framework_python_relocation"](
            inputs, outputs, contract, RuntimeError, update=True
        )


def test_receipt_rejects_public_private_mismatch_and_artifact_swap() -> None:
    validate = _load_publication()[
        "_validate_framework_python_relocation_evidence"
    ]
    contract = _contract()
    inputs, outputs = _inventory_records(contract)
    total = sum(record["size"] for record in inputs if record["kind"] == "file")
    private = copy.deepcopy(contract)
    private["tools"]["otool"]["sha256"] = _digest("private drift")
    with pytest.raises(_ReceiptFailure, match="malformed"):
        validate(contract, private, inputs, outputs, len(inputs), total)
    swapped = copy.deepcopy(contract)
    swapped["artifacts"]["launcher"], swapped["artifacts"]["trampoline"] = (
        swapped["artifacts"]["trampoline"], swapped["artifacts"]["launcher"]
    )
    with pytest.raises(_ReceiptFailure, match="artifact binding"):
        validate(swapped, swapped, inputs, outputs, len(inputs), total)
    duplicate = copy.deepcopy(inputs)
    duplicate[1]["path"] = duplicate[0]["path"]
    for changed, count, size in (
        (duplicate, len(duplicate), total),
        (inputs, len(inputs) + 1, total),
        (inputs, len(inputs), total + 1),
    ):
        with pytest.raises(_ReceiptFailure, match="source inventory"):
            validate(contract, contract, changed, outputs, count, size)
    for field in ("source_mode", "destination_mode"):
        unsafe = copy.deepcopy(inputs)
        unsafe[0][field] = "0777"
        with pytest.raises(_ReceiptFailure, match="source mode"):
            validate(contract, contract, unsafe, outputs, len(unsafe), total)


@pytest.mark.parametrize("descriptor", [1, 2])
def test_bounded_capture_reaps_stdout_and_stderr_flood(
    tmp_path: Path, descriptor: int,
) -> None:
    scope = _load_cli()
    pid_path = tmp_path / "child.pid"
    program = """
import os, sys, time
with open(sys.argv[1], "w", encoding="ascii") as stream:
    stream.write(str(os.getpid()))
try:
    while True:
        os.write(int(sys.argv[2]), b"x" * 65536)
except BrokenPipeError:
    time.sleep(60)
"""
    with pytest.raises(RuntimeError, match="output exceeds"):
        scope["_macho_run"](
            [sys.executable, "-I", "-S", "-c", program, str(pid_path),
             str(descriptor)],
            tmp_path, RuntimeError,
        )
    pid = pid_path.read_text(encoding="ascii")
    observed = subprocess.run(
        ["/bin/ps", "-p", pid, "-o", "pid="],
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        check=False, timeout=5,
    )
    assert observed.stdout.strip() == b""


def test_macho_digest_read_has_a_hard_artifact_cap(tmp_path: Path) -> None:
    helper = _load_helper()
    scope = _load_cli()
    artifact = tmp_path / "grown-macho"
    with artifact.open("wb") as stream:
        stream.truncate(256 * 1024 * 1024 + 1)
    artifact.chmod(0o500)
    scope["_preflight_macho"] = lambda *args, **kwargs: None
    with pytest.raises(helper.CacheCopyError, match="metadata is unsafe"):
        scope["_artifact_record"](
            artifact, ["/fixture/Python"], "/fixture/Python",
            helper._digest_regular, helper.CacheCopyError, source=True,
        )


def test_top_level_runtime_source_growth_is_bounded_before_read(
    tmp_path: Path,
) -> None:
    helper = _load_helper()
    source = tmp_path / "python3"
    with source.open("wb") as stream:
        stream.truncate(helper.MAXIMUM_FILE_BYTES + 1)
    source.chmod(0o500)
    metadata = source.stat()
    record = {
        "path": "python3", "kind": "file",
        "source_device": metadata.st_dev, "source_inode": metadata.st_ino,
        "source_mode": "0500", "destination_device": 1,
        "destination_inode": 1, "destination_mode": "0500",
        "size": metadata.st_size, "sha256": "0" * 64,
    }
    with pytest.raises(helper.CacheCopyError, match="exceeds its bound"):
        helper._verify_runtime_sources({"python3": source}, [record])


def test_release_runner_selects_relocated_framework_launcher() -> None:
    source = (ROOT / "scripts/run_sumeragi_v2_release_gates.sh").read_text(
        encoding="utf-8"
    )
    assert (
        '${SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR}/python-runtime/bin/python3'
        in source
    )
    assert '"$release_python_bin" != "$bootstrap_python"' in source
    assert '"$release_python_bin" != "$release_bootstrap_evidence_dir/python3"' not in source


@pytest.mark.skipif(
    sys.platform != "darwin" or not sysconfig.get_config_var("PYTHONFRAMEWORK"),
    reason="requires the selected macOS framework Python",
)
def test_real_framework_archive_is_rewritten_signed_and_reverified(tmp_path: Path) -> None:
    runtime = tmp_path / "python-runtime"
    inventory = tmp_path / "python-runtime-input.json"
    try:
        for operation in ("--copy-framework-python", "--verify-framework-python"):
            result = subprocess.run(
                [
                    sys.executable, "-I", "-S", str(HELPER_PATH), operation,
                    "--runtime-root", str(runtime),
                    "--runtime-inventory", str(inventory),
                ],
                cwd=ROOT, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, check=False, timeout=60,
            )
            assert (result.returncode, result.stdout, result.stderr) == (0, b"", b"")
        document = json.loads(inventory.read_bytes())
        relocation = document["relocation"]
        for name, expected in (
            ("launcher", "@executable_path/../Python"),
            ("trampoline", "@executable_path/../../../../Python"),
        ):
            derived = relocation["artifacts"][name]["derived"]
            assert derived["framework_dependency"] == expected
            assert derived["mode"] == "0500"
            assert derived["codesign"] == "adhoc"
        assert stat.S_IMODE((runtime / "bin/python3").stat().st_mode) == 0o500
        probe = subprocess.run(
            [str(runtime / "bin/python3"), "-I", "-S", "-c", "pass"],
            cwd=runtime, env={"LANG": "C", "LC_ALL": "C", "PATH": str(runtime / "bin")},
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            check=False, timeout=30,
        )
        assert (probe.returncode, probe.stdout, probe.stderr) == (0, b"", b"")
    finally:
        if runtime.exists():
            helper = _load_helper()
            helper._quiescent_remove_tree(runtime, "framework relocation test runtime")
        if inventory.exists():
            inventory.unlink()
