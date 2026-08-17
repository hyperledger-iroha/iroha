from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import sys
from typing import Any

import pytest


ROOT = Path(__file__).resolve().parents[2]
HELPER_PATH = ROOT / "scripts/copy_sumeragi_v2_release_cargo_cache.py"
CLI_PATH = ROOT / "scripts/copy_sumeragi_v2_release_cargo_cache_cli.py"
FRAMEWORK_PYTHON_312 = Path("/opt/homebrew/bin/python3.12")


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
        "hashlib": hashlib,
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


def test_macho_dependencies_accept_identical_fat_slices(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    scope = _load_cli()
    path = Path("/fixture/Python")
    payload = (
        f"{path} (architecture x86_64):\n"
        "\t@rpath/Python (compatibility version 3.9.0, current version 3.9.0)\n"
        "\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)\n"
        f"{path} (architecture arm64):\n"
        "\t@rpath/Python (compatibility version 3.9.0, current version 3.9.0)\n"
        "\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)\n"
    ).encode()
    monkeypatch.setitem(
        scope,
        "_macho_run",
        lambda *_args, **_kwargs: __import__("types").SimpleNamespace(
            returncode=0,
            stdout=payload,
            stderr=b"",
        ),
    )

    assert scope["_macho_dependencies"](
        path, Path("/usr/bin/otool"), RuntimeError
    ) == ["@rpath/Python", "/usr/lib/libSystem.B.dylib"]


def test_macho_dependencies_reject_mixed_thin_and_fat_headers(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    scope = _load_cli()
    path = Path("/fixture/Python")
    payload = (
        f"{path}:\n"
        "\t@rpath/Python (compatibility version 3.9.0, current version 3.9.0)\n"
        f"{path} (architecture arm64):\n"
        "\t@rpath/Python (compatibility version 3.9.0, current version 3.9.0)\n"
    ).encode()
    monkeypatch.setitem(
        scope,
        "_macho_run",
        lambda *_args, **_kwargs: __import__("types").SimpleNamespace(
            returncode=0,
            stdout=payload,
            stderr=b"",
        ),
    )

    with pytest.raises(RuntimeError, match="output is malformed"):
        scope["_macho_dependencies"](
            path, Path("/usr/bin/otool"), RuntimeError
        )


def _thin_macho(cpu_type: int) -> bytes:
    name = b"@rpath/Fixture\0"
    command_size = (24 + len(name) + 7) & ~7
    command = (
        struct.pack("<6I", 0x0C, command_size, 24, 0, 0, 0)
        + name
        + bytes(command_size - 24 - len(name))
    )
    return struct.pack(
        "<8I", 0xFEEDFACF, cpu_type, 0, 2, 1, len(command), 0, 0,
    ) + command


def test_strict_macho_parser_accepts_thin_and_nonoverlapping_fat_images() -> None:
    scope = _load_cli()
    arm = _thin_macho(0x0100000C)
    x86 = _thin_macho(0x01000007)
    table_size = 8 + 2 * 20
    arm_offset = table_size
    x86_offset = arm_offset + len(arm)
    assert arm_offset % 8 == 0 and x86_offset % 8 == 0
    fat = (
        struct.pack(">II", 0xCAFEBABE, 2)
        + struct.pack(">5I", 0x0100000C, 0, arm_offset, len(arm), 3)
        + struct.pack(">5I", 0x01000007, 0, x86_offset, len(x86), 3)
        + arm + x86
    )

    thin = scope["_parse_macho"](arm, "thin")
    universal = scope["_parse_macho"](fat, "fat")
    assert thin is not None and len(thin) == 1
    assert universal is not None
    assert [item["cpu_type"] for item in universal] == [
        0x0100000C, 0x01000007,
    ]
    assert universal[0]["commands"][0]["name"] == "@rpath/Fixture"


def test_strict_macho_parser_rejects_nonzero_fat64_reserved_field() -> None:
    scope = _load_cli()
    thin = _thin_macho(0x0100000C)
    offset = 40
    fat = (
        struct.pack(">II", 0xCAFEBABF, 1)
        + struct.pack(">IIQQII", 0x0100000C, 0, offset, len(thin), 3, 1)
        + thin
    )
    with pytest.raises(RuntimeError, match="reserved field"):
        scope["_parse_macho"](fat, "fat64")


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
    runtime_images = []
    transforms = []
    for name, input_path in (
        ("launcher", "python3"),
        ("trampoline", "Resources/Python.app/Contents/MacOS/Python"),
    ):
        artifact = artifacts[name]
        rewritten = artifact["derived"]["framework_dependency"]
        runtime_images.append({
            "path": artifact["path"],
            "size_bytes": artifact["derived"]["size_bytes"],
            "sha256": artifact["derived"]["sha256"],
            "slices": [{
                "cpu_type": 0x0100000C, "cpu_subtype": 0, "file_type": 2,
                "dependencies": [
                    {
                        "command": 0x0C, "binding": "archive",
                        "install_name": rewritten, "target": "Python",
                    },
                    {
                        "command": 0x0C, "binding": "system",
                        "install_name_sha256": _digest("/usr/lib/libSystem.B.dylib"),
                    },
                ],
                "id_dylib_sha256": [], "rpath_sha256": [],
                "code_signature": "embedded",
            }],
        })
        transforms.append({
            "input_path": input_path, "path": artifact["path"],
            "source_mode": artifact["source"]["mode"],
            "source_sha256": artifact["source"]["sha256"],
            "source_size_bytes": artifact["source"]["size_bytes"],
            "derived_mode": artifact["derived"]["mode"],
            "derived_sha256": artifact["derived"]["sha256"],
            "derived_size_bytes": artifact["derived"]["size_bytes"],
            "operations": [{
                "operation": "change",
                "source_install_name_sha256": _digest("/fixture/Python"),
                "replacement": rewritten,
            }],
            "codesign": "adhoc",
        })
    runtime_images.append({
        "path": "Python", "size_bytes": 64,
        "sha256": _digest("framework"),
        "slices": [{
            "cpu_type": 0x0100000C, "cpu_subtype": 0, "file_type": 6,
            "dependencies": [{
                "command": 0x0C, "binding": "system",
                "install_name_sha256": _digest("/usr/lib/libSystem.B.dylib"),
            }],
            "id_dylib_sha256": [_digest("/fixture/Python")],
            "rpath_sha256": [], "code_signature": "embedded",
        }],
    })
    runtime_images.sort(key=lambda image: image["path"])
    transforms.sort(key=lambda transform: transform["path"])
    return {
        "format": "iroha-sumeragi-v2-framework-python-relocation",
        "schema_version": 2,
        "framework": "Python",
        "tools": tools,
        "artifacts": artifacts,
        "closure": {
            "format": "iroha-sumeragi-v2-framework-python-mach-o-transcript",
            "schema_version": 1,
            "source_image_count": 3,
            "external_sources": [],
            "transforms": transforms,
            "runtime": {
                "format": "iroha-sumeragi-v2-framework-python-mach-o",
                "schema_version": 1, "image_count": 3,
                "images": runtime_images,
            },
        },
    }


def _already_local_contract() -> dict[str, object]:
    contract = _contract()
    transforms = {
        transform["path"]: transform
        for transform in contract["closure"]["transforms"]
    }
    for artifact in contract["artifacts"].values():
        source = artifact["source"]
        derived = artifact["derived"]
        source["sha256"] = derived["sha256"]
        source["size_bytes"] = derived["size_bytes"]
        source["framework_dependency_sha256"] = _digest(
            derived["framework_dependency"]
        )
        source["dependency_vector_sha256"] = derived[
            "dependency_vector_sha256"
        ]
        transform = transforms[artifact["path"]]
        transform["source_sha256"] = source["sha256"]
        transform["source_size_bytes"] = source["size_bytes"]
        transform["operations"] = []
    return contract


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
    def observe(**kwargs):
        observed = copy.deepcopy(contract)
        observed["tools"] = {
            name: scope["_macho_tool_record"](name, None, RuntimeError)
            for name in sorted(contract["tools"])
        }
        paths = {
            "launcher": (
                Path("/fixture/bin/source-launcher"),
                Path("/archive/bin/python3"),
                "@executable_path/../Python",
            ),
            "trampoline": (
                Path("/fixture/Resources/Python.app/Contents/MacOS/Python"),
                Path("/archive/Resources/Python.app/Contents/MacOS/Python"),
                "@executable_path/../../../../Python",
            ),
        }
        for name, (source_path, output_path, rewritten) in paths.items():
            source_dependencies = observed_dependencies(source_path, None, RuntimeError)
            derived_dependencies = observed_dependencies(output_path, None, RuntimeError)
            expected = [
                rewritten if dependency == old else dependency
                for dependency in source_dependencies
            ]
            if derived_dependencies != expected:
                raise RuntimeError("framework Python relocated dependency is not exact")
            scope["_require_adhoc_signature"](output_path, None, RuntimeError)
            observed["artifacts"][name] = {
                "path": contract["artifacts"][name]["path"],
                "source": scope["_artifact_record"](
                    source_path, source_dependencies, old, None, RuntimeError,
                    source=True,
                ),
                "derived": scope["_artifact_record"](
                    output_path, derived_dependencies, rewritten, None,
                    RuntimeError, source=False,
                ),
            }
        return observed, {}

    monkeypatch.setitem(scope, "_observe_framework_python_relocation", observe)
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
        with pytest.raises(RuntimeError, match="binding"):
            validate(changed, RuntimeError)
    changed = _contract()
    changed["artifacts"]["launcher"]["source"]["mode"] = "0777"
    with pytest.raises(RuntimeError, match="mode is unsafe"):
        validate(changed, RuntimeError)
    changed = _contract()
    changed["artifacts"]["launcher"]["source"]["size_bytes"] = (
        256 * 1024 * 1024 + 1
    )
    with pytest.raises(RuntimeError, match="size is outside"):
        validate(changed, RuntimeError)


def test_relocation_contract_rejects_open_or_unsigned_runtime_closure() -> None:
    scope = _load_cli()
    validate = scope["_validate_framework_python_relocation_contract"]
    changed = _contract()
    launcher = next(
        image for image in changed["closure"]["runtime"]["images"]
        if image["path"] == "bin/python3"
    )
    launcher["slices"][0]["dependencies"][0]["install_name"] = "/fixture/Python"
    with pytest.raises(RuntimeError, match="archive dependency is unsafe"):
        validate(changed, RuntimeError)
    changed = _contract()
    launcher = next(
        image for image in changed["closure"]["runtime"]["images"]
        if image["path"] == "bin/python3"
    )
    launcher["slices"][0]["code_signature"] = "unsigned"
    with pytest.raises(RuntimeError, match="unsigned"):
        validate(changed, RuntimeError)


def test_already_local_launchers_are_signed_zero_operation_transforms() -> None:
    contract = _already_local_contract()
    scope = _load_cli()
    assert scope["_validate_framework_python_relocation_contract"](
        contract, RuntimeError,
    ) == contract

    inputs, outputs = _inventory_records(contract)
    total = sum(record["size"] for record in inputs if record["kind"] == "file")
    _load_publication()["_validate_framework_python_relocation_evidence"](
        contract, contract, inputs, outputs, len(inputs), total,
    )

    changed = _contract()
    launcher = next(
        transform for transform in changed["closure"]["transforms"]
        if transform["path"] == "bin/python3"
    )
    launcher["operations"] = []
    with pytest.raises(RuntimeError, match="closure binding"):
        scope["_validate_framework_python_relocation_contract"](
            changed, RuntimeError,
        )

    changed = _contract()
    launcher = next(
        transform for transform in changed["closure"]["transforms"]
        if transform["path"] == "bin/python3"
    )
    launcher["operations"][0]["source_install_name_sha256"] = _digest(
        "wrong dependency"
    )
    with pytest.raises(RuntimeError, match="closure binding"):
        scope["_validate_framework_python_relocation_contract"](
            changed, RuntimeError,
        )
    inputs, outputs = _inventory_records(changed)
    total = sum(record["size"] for record in inputs if record["kind"] == "file")
    with pytest.raises(_ReceiptFailure, match="derivation"):
        _load_publication()["_validate_framework_python_relocation_evidence"](
            changed, changed, inputs, outputs, len(inputs), total,
        )


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
    sys.platform != "darwin" or not FRAMEWORK_PYTHON_312.is_file(),
    reason="requires Homebrew framework Python 3.12",
)
def test_real_framework_archive_is_rewritten_signed_and_reverified(tmp_path: Path) -> None:
    runtime = tmp_path / "python-runtime"
    inventory = tmp_path / "python-runtime-input.json"
    try:
        for operation in ("--copy-framework-python", "--verify-framework-python"):
            result = subprocess.run(
                [
                    str(FRAMEWORK_PYTHON_312), "-I", "-S",
                    str(HELPER_PATH), operation,
                    "--runtime-root", str(runtime),
                    "--runtime-inventory", str(inventory),
                ],
                cwd=ROOT, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, check=False, timeout=60,
            )
            assert (result.returncode, result.stdout, result.stderr) == (0, b"", b"")
        document = json.loads(inventory.read_bytes())
        relocation = document["relocation"]
        framework = relocation["framework"]
        assert isinstance(framework, str) and framework
        closure = relocation["closure"]
        assert closure["source_image_count"] == closure["runtime"]["image_count"]
        assert closure["external_sources"]
        assert {
            source["path"] for source in closure["external_sources"]
        } <= {
            transform["path"] for transform in closure["transforms"]
        }
        for name, expected in (
            ("launcher", f"@executable_path/../{framework}"),
            ("trampoline", f"@executable_path/../../../../{framework}"),
        ):
            derived = relocation["artifacts"][name]["derived"]
            assert derived["framework_dependency"] == expected
            assert derived["mode"] == "0500"
            assert derived["codesign"] == "adhoc"
        assert stat.S_IMODE((runtime / "bin/python3").stat().st_mode) == 0o500
        probe = subprocess.run(
            [
                str(runtime / "bin/python3"), "-I", "-S", "-c",
                "import _decimal, _hashlib, _lzma, _sqlite3, _ssl",
            ],
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
