"""CLI dispatch for the authenticated Sumeragi v2 release cache helper."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import stat
import subprocess
import sys
import time


_MACHO_TOOL_PATHS = {
    "codesign": Path("/usr/bin/codesign"),
    "install_name_tool": Path("/usr/bin/install_name_tool"),
    "otool": Path("/usr/bin/otool"),
}
_MACHO_OUTPUT_BYTES = 1024 * 1024
_MACHO_ARTIFACT_BYTES = 256 * 1024 * 1024
_MACHO_TOOL_BYTES = 64 * 1024 * 1024
_MACHO_REWRITES = {
    "launcher": ("bin/python3", "@executable_path/../Python"),
    "trampoline": (
        "Resources/Python.app/Contents/MacOS/Python",
        "@executable_path/../../../../Python",
    ),
}
_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_MODE_RE = re.compile(r"[0-7]{4}")
_FRAMEWORK_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]*")


def _stop_macho_process(process: subprocess.Popen[bytes]) -> None:
    """Stop and reap only the authenticated Mach-O child owned by this call."""

    if process.poll() is not None:
        process.wait()
        return
    try:
        process.terminate()
        process.wait(timeout=1)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
    except ProcessLookupError:
        process.wait()


def _macho_run(
    argv: list[str],
    cwd: Path,
    error_type: type[Exception],
    *,
    environment: dict[str, str] | None = None,
    maximum_output_bytes: int = _MACHO_OUTPUT_BYTES,
    label: str = "Mach-O tool",
):
    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    streams: dict[str, bytearray] = {
        "stdout": bytearray(),
        "stderr": bytearray(),
    }
    overflow = False
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=environment
            or {"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
        )
        assert process.stdout is not None and process.stderr is not None
        selector.register(process.stdout, selectors.EVENT_READ, "stdout")
        selector.register(process.stderr, selectors.EVENT_READ, "stderr")
        deadline = time.monotonic() + 30
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(argv, 30)
            events = selector.select(remaining)
            if not events:
                raise subprocess.TimeoutExpired(argv, 30)
            for key, _ in events:
                stream_name = key.data
                retained = streams[stream_name]
                allowance = maximum_output_bytes - len(retained)
                chunk = os.read(key.fd, min(64 * 1024, allowance + 1))
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                if len(chunk) > allowance:
                    retained.extend(chunk[:allowance])
                    overflow = True
                    break
                retained.extend(chunk)
            if overflow:
                break
        if overflow:
            for key in list(selector.get_map().values()):
                selector.unregister(key.fileobj)
                key.fileobj.close()
            _stop_macho_process(process)
            returncode = process.returncode
        else:
            returncode = process.wait(
                timeout=max(deadline - time.monotonic(), 0.001)
            )
    except (OSError, subprocess.TimeoutExpired) as error:
        if process is not None:
            for stream in (process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()
            _stop_macho_process(process)
        raise error_type(f"framework Python {label} could not run") from error
    finally:
        selector.close()
    if overflow:
        raise error_type(f"framework Python {label} output exceeds its bound")
    return subprocess.CompletedProcess(
        argv, returncode, bytes(streams["stdout"]), bytes(streams["stderr"])
    )


def _macho_tool_record(
    name: str, digest_regular, error_type: type[Exception]
) -> dict[str, object]:
    path = _MACHO_TOOL_PATHS[name]
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise error_type(f"framework Python {name} is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_nlink < 1
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or not metadata.st_mode & 0o111
        or not 0 < metadata.st_size <= _MACHO_TOOL_BYTES
    ):
        raise error_type(f"framework Python {name} metadata is unsafe")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
        opened = os.fstat(descriptor)
        payload = bytearray()
        while block := os.read(descriptor, 1024 * 1024):
            payload.extend(block)
            if len(payload) > _MACHO_TOOL_BYTES:
                raise error_type(f"framework Python {name} exceeds its bound")
        after = os.fstat(descriptor)
    except OSError as error:
        raise error_type(f"framework Python {name} could not be read") from error
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    current = path.lstat()
    stable = ("st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
              "st_size", "st_mtime_ns", "st_ctime_ns")
    if any(
        getattr(metadata, field) != getattr(observed, field)
        for observed in (opened, after, current)
        for field in stable
    ) or len(payload) != opened.st_size:
        raise error_type(f"framework Python {name} metadata is unsafe")
    return {
        "path": str(path),
        "mode": f"{stat.S_IMODE(opened.st_mode):04o}",
        "sha256": hashlib.sha256(payload).hexdigest(),
        "size_bytes": len(payload),
    }


def _preflight_macho(
    path: Path, error_type: type[Exception], *, source: bool
) -> None:
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise error_type("framework Python Mach-O input is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid not in ({0, os.geteuid()} if source else {os.geteuid()})
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or not metadata.st_mode & 0o111
        or not 0 < metadata.st_size <= _MACHO_ARTIFACT_BYTES
    ):
        raise error_type("framework Python Mach-O input metadata is unsafe")


def _macho_dependencies(
    path: Path, otool: Path, error_type: type[Exception]
) -> list[str]:
    result = _macho_run([str(otool), "-L", str(path)], path.parent, error_type)
    if result.returncode != 0 or result.stderr:
        raise error_type("framework Python dependency inspection failed")
    try:
        lines = result.stdout.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise error_type("framework Python dependency output is not UTF-8") from error
    if not lines or lines[0] != f"{path}:" or len(lines) < 2:
        raise error_type("framework Python dependency output is malformed")
    dependencies: list[str] = []
    for line in lines[1:]:
        marker = line.find(" (")
        dependency = line[1:marker] if line.startswith("\t") and marker > 1 else ""
        if (
            not dependency
            or not line.endswith(")")
            or "\0" in dependency
            or any(ord(character) < 0x20 for character in dependency)
        ):
            raise error_type("framework Python dependency output is malformed")
        dependencies.append(dependency)
    if len(set(dependencies)) != len(dependencies):
        raise error_type("framework Python dependencies are not unique")
    return dependencies


def _dependency_vector_sha256(dependencies: list[str]) -> str:
    payload = json.dumps(
        dependencies, ensure_ascii=False, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def probe_framework_python_runtime(
    *,
    runtime_root: Path,
    stdlib_name: str,
    probe_code: str,
    error_type: type[Exception],
) -> bytes:
    """Run the relocated interpreter with hard-bounded output and child reaping."""

    executable = runtime_root / "bin/python3"
    _preflight_macho(executable, error_type, source=False)
    metadata = executable.lstat()
    if stat.S_IMODE(metadata.st_mode) != 0o500:
        raise error_type("archived framework Python executable metadata is unsafe")
    expected_zip = runtime_root / "lib" / (
        f"python{sys.version_info.major}{sys.version_info.minor}.zip"
    )
    expected_stdlib = runtime_root / "lib" / stdlib_name
    expected_dynload = expected_stdlib / "lib-dynload"
    result = _macho_run(
        [
            str(executable), "-I", "-S", "-c", probe_code,
            str(executable), str(runtime_root), str(expected_zip),
            str(expected_stdlib), str(expected_dynload),
        ],
        runtime_root,
        error_type,
        environment={
            "LANG": "C", "LC_ALL": "C", "PATH": str(runtime_root / "bin"),
        },
        maximum_output_bytes=4096,
        label="archived interpreter probe",
    )
    expected_stdout = os.fsencode(str(executable)) + b"\n"
    if (
        result.returncode != 0
        or result.stdout != expected_stdout
        or result.stderr
    ):
        raise error_type(
            "archived framework Python isolated probe did not report its executable"
        )
    return expected_stdout


def _require_adhoc_signature(
    path: Path, codesign: Path, error_type: type[Exception]
) -> None:
    verification = _macho_run(
        [str(codesign), "--verify", "--strict", "--verbose=0", str(path)],
        path.parent,
        error_type,
    )
    details = _macho_run(
        [str(codesign), "-d", "--verbose=4", str(path)],
        path.parent,
        error_type,
    )
    try:
        detail_lines = details.stderr.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise error_type("framework Python signature output is not UTF-8") from error
    if (
        verification.returncode != 0
        or verification.stdout
        or verification.stderr
        or details.returncode != 0
        or details.stdout
        or detail_lines.count("Signature=adhoc") != 1
    ):
        raise error_type("framework Python relocated output is not ad-hoc signed")


def _artifact_record(
    path: Path,
    dependencies: list[str],
    framework_dependency: str,
    digest_regular,
    error_type: type[Exception],
    *,
    source: bool,
) -> dict[str, object]:
    _preflight_macho(path, error_type, source=source)
    digest, size, metadata = digest_regular(
        path,
        "framework Python Mach-O input",
        maximum_bytes=_MACHO_ARTIFACT_BYTES,
    )
    try:
        current = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise error_type("framework Python Mach-O input changed") from error
    mode = f"{stat.S_IMODE(metadata.st_mode):04o}"
    if (
        resolved != path
        or
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or not metadata.st_mode & 0o111
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or metadata.st_uid not in ({0, os.geteuid()} if source else {os.geteuid()})
        or (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_mode,
            metadata.st_uid,
            metadata.st_nlink,
            metadata.st_size,
        )
        != (
            current.st_dev,
            current.st_ino,
            current.st_mode,
            current.st_uid,
            current.st_nlink,
            current.st_size,
        )
    ):
        raise error_type("framework Python Mach-O input metadata is unsafe")
    record: dict[str, object] = {
        "mode": mode,
        "sha256": digest,
        "size_bytes": size,
        "dependency_vector_sha256": _dependency_vector_sha256(dependencies),
    }
    if source:
        record["framework_dependency_sha256"] = hashlib.sha256(
            framework_dependency.encode("utf-8")
        ).hexdigest()
    else:
        record["framework_dependency"] = framework_dependency
        record["codesign"] = "adhoc"
    return record


def _seal_relocated_macho(path: Path, error_type: type[Exception]) -> None:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        before = path.lstat()
        descriptor = os.open(path, flags)
    except OSError as error:
        raise error_type("framework Python relocated output could not be sealed") from error
    try:
        opened = os.fstat(descriptor)
        if (
            stat.S_ISLNK(before.st_mode)
            or not stat.S_ISREG(before.st_mode)
            or before.st_uid != os.geteuid()
            or before.st_nlink != 1
            or (before.st_dev, before.st_ino, before.st_mode, before.st_size)
            != (opened.st_dev, opened.st_ino, opened.st_mode, opened.st_size)
        ):
            raise error_type("framework Python relocated output metadata is unsafe")
        os.fchmod(descriptor, 0o500)
        os.fsync(descriptor)
        sealed = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = path.lstat()
    if (
        (sealed.st_dev, sealed.st_ino, sealed.st_uid, sealed.st_nlink, sealed.st_size)
        != (after.st_dev, after.st_ino, after.st_uid, after.st_nlink, after.st_size)
        or stat.S_IMODE(sealed.st_mode) != 0o500
        or stat.S_IMODE(after.st_mode) != 0o500
    ):
        raise error_type("framework Python relocated output sealing changed identity")


def _validate_framework_python_relocation_contract(
    value: object, error_type: type[Exception]
) -> dict[str, object]:
    if not isinstance(value, dict) or set(value) != {
        "format", "schema_version", "framework", "tools", "artifacts"
    }:
        raise error_type("framework Python relocation contract is malformed")
    if (
        value["format"] != "iroha-sumeragi-v2-framework-python-relocation"
        or type(value["schema_version"]) is not int
        or value["schema_version"] != 1
        or not isinstance(value["framework"], str)
        or _FRAMEWORK_RE.fullmatch(value["framework"]) is None
        or not isinstance(value["tools"], dict)
        or set(value["tools"]) != set(_MACHO_TOOL_PATHS)
        or not isinstance(value["artifacts"], dict)
        or set(value["artifacts"]) != set(_MACHO_REWRITES)
    ):
        raise error_type("framework Python relocation contract is not exact")
    for name, expected_path in _MACHO_TOOL_PATHS.items():
        record = value["tools"][name]
        if (
            not isinstance(record, dict)
            or set(record) != {"path", "mode", "sha256", "size_bytes"}
            or record["path"] != str(expected_path)
            or not isinstance(record["mode"], str)
            or _MODE_RE.fullmatch(record["mode"]) is None
            or int(record["mode"], 8) & 0o022
            or not int(record["mode"], 8) & 0o111
            or not isinstance(record["sha256"], str)
            or _DIGEST_RE.fullmatch(record["sha256"]) is None
            or type(record["size_bytes"]) is not int
            or not 0 < record["size_bytes"] <= 64 * 1024 * 1024
        ):
            raise error_type("framework Python relocation tool binding is not exact")
    for name, (relative, rewritten) in _MACHO_REWRITES.items():
        artifact = value["artifacts"][name]
        if not isinstance(artifact, dict) or set(artifact) != {
            "path", "source", "derived"
        } or artifact["path"] != relative:
            raise error_type("framework Python relocation artifact is not exact")
        source = artifact["source"]
        derived = artifact["derived"]
        if (
            not isinstance(source, dict)
            or set(source) != {
                "mode", "sha256", "size_bytes",
                "framework_dependency_sha256", "dependency_vector_sha256",
            }
            or not isinstance(derived, dict)
            or set(derived) != {
                "mode", "sha256", "size_bytes", "framework_dependency",
                "dependency_vector_sha256", "codesign",
            }
            or derived["mode"] != "0500"
            or derived["framework_dependency"] != rewritten
            or derived["codesign"] != "adhoc"
        ):
            raise error_type("framework Python relocation artifact binding is wrong")
        for record in (source, derived):
            if (
                not isinstance(record["mode"], str)
                or _MODE_RE.fullmatch(record["mode"]) is None
                or not int(record["mode"], 8) & 0o111
                or (record is source and int(record["mode"], 8) & 0o022)
                or not isinstance(record["sha256"], str)
                or _DIGEST_RE.fullmatch(record["sha256"]) is None
                or type(record["size_bytes"]) is not int
                or not 0 < record["size_bytes"] <= _MACHO_ARTIFACT_BYTES
                or not isinstance(record["dependency_vector_sha256"], str)
                or _DIGEST_RE.fullmatch(record["dependency_vector_sha256"]) is None
            ):
                raise error_type("framework Python relocation digest binding is wrong")
        if (
            not isinstance(source["framework_dependency_sha256"], str)
            or _DIGEST_RE.fullmatch(source["framework_dependency_sha256"]) is None
        ):
            raise error_type("framework Python source dependency binding is wrong")
    return value


def relocate_framework_python_runtime(
    *,
    version_root: Path,
    framework: str,
    source_python: Path,
    runtime_root: Path,
    digest_regular,
    error_type: type[Exception],
) -> dict[str, object]:
    """Rewrite and ad-hoc sign exactly two copied framework-Python Mach-O files."""

    tools = {
        name: _macho_tool_record(name, digest_regular, error_type)
        for name in sorted(_MACHO_TOOL_PATHS)
    }
    otool = _MACHO_TOOL_PATHS["otool"]
    install_name_tool = _MACHO_TOOL_PATHS["install_name_tool"]
    codesign = _MACHO_TOOL_PATHS["codesign"]
    old_dependency = str(version_root / framework)
    sources = {
        "launcher": source_python,
        "trampoline": (
            version_root / "Resources/Python.app/Contents/MacOS/Python"
        ),
    }
    artifacts: dict[str, object] = {}
    for name, (relative, rewritten) in _MACHO_REWRITES.items():
        source_path = sources[name]
        destination = runtime_root / relative
        _preflight_macho(source_path, error_type, source=True)
        _preflight_macho(destination, error_type, source=False)
        source_dependencies = _macho_dependencies(
            source_path, otool, error_type
        )
        copied_dependencies = _macho_dependencies(
            destination, otool, error_type
        )
        source_digest, source_size, _ = digest_regular(
            source_path, f"framework Python source {name}",
            maximum_bytes=_MACHO_ARTIFACT_BYTES,
        )
        copied_digest, copied_size, copied_metadata = digest_regular(
            destination, f"framework Python copied {name}",
            maximum_bytes=_MACHO_ARTIFACT_BYTES,
        )
        if (
            source_dependencies != copied_dependencies
            or source_dependencies.count(old_dependency) != 1
            or rewritten in source_dependencies
            or (source_digest, source_size) != (copied_digest, copied_size)
            or stat.S_IMODE(copied_metadata.st_mode) != 0o700
        ):
            raise error_type("framework Python source dependency is not exact")
        rewrite = _macho_run(
            [
                str(install_name_tool), "-change", old_dependency,
                rewritten, str(destination),
            ],
            destination.parent,
            error_type,
        )
        if rewrite.returncode != 0 or rewrite.stdout:
            raise error_type("framework Python dependency rewrite failed")
        expected_dependencies = [
            rewritten if dependency == old_dependency else dependency
            for dependency in source_dependencies
        ]
        if _macho_dependencies(destination, otool, error_type) != expected_dependencies:
            raise error_type("framework Python dependency rewrite is not exact")
        signing = _macho_run(
            [
                str(codesign), "--force", "--sign", "-",
                "--timestamp=none", str(destination),
            ],
            destination.parent,
            error_type,
        )
        if signing.returncode != 0 or signing.stdout:
            raise error_type("framework Python ad-hoc signing failed")
        derived_dependencies = _macho_dependencies(destination, otool, error_type)
        if derived_dependencies != expected_dependencies:
            raise error_type("framework Python signing changed its dependencies")
        _require_adhoc_signature(destination, codesign, error_type)
        _seal_relocated_macho(destination, error_type)
        source = _artifact_record(
            source_path, source_dependencies, old_dependency,
            digest_regular, error_type, source=True,
        )
        derived = _artifact_record(
            destination, derived_dependencies, rewritten,
            digest_regular, error_type, source=False,
        )
        artifacts[name] = {
            "path": relative,
            "source": source,
            "derived": derived,
        }
    if tools != {
        name: _macho_tool_record(name, digest_regular, error_type)
        for name in sorted(_MACHO_TOOL_PATHS)
    }:
        raise error_type("framework Python Mach-O tools changed during relocation")
    document = {
        "format": "iroha-sumeragi-v2-framework-python-relocation",
        "schema_version": 1,
        "framework": framework,
        "tools": tools,
        "artifacts": artifacts,
    }
    return _validate_framework_python_relocation_contract(document, error_type)


def verify_framework_python_relocation(
    *,
    version_root: Path,
    framework: str,
    source_python: Path,
    runtime_root: Path,
    contract: object,
    digest_regular,
    error_type: type[Exception],
) -> None:
    """Reauthenticate tools, original inputs, derived bytes, links, and signatures."""

    expected = _validate_framework_python_relocation_contract(contract, error_type)
    old_dependency = str(version_root / framework)
    tools = {
        name: _macho_tool_record(name, digest_regular, error_type)
        for name in sorted(_MACHO_TOOL_PATHS)
    }
    artifacts: dict[str, object] = {}
    sources = {
        "launcher": source_python,
        "trampoline": (
            version_root / "Resources/Python.app/Contents/MacOS/Python"
        ),
    }
    for name, (relative, rewritten) in _MACHO_REWRITES.items():
        source_path = sources[name]
        destination = runtime_root / relative
        _preflight_macho(source_path, error_type, source=True)
        _preflight_macho(destination, error_type, source=False)
        source_dependencies = _macho_dependencies(
            source_path, _MACHO_TOOL_PATHS["otool"], error_type
        )
        derived_dependencies = _macho_dependencies(
            destination, _MACHO_TOOL_PATHS["otool"], error_type
        )
        replacement = [
            rewritten if dependency == old_dependency else dependency
            for dependency in source_dependencies
        ]
        if (
            source_dependencies.count(old_dependency) != 1
            or rewritten in source_dependencies
            or derived_dependencies != replacement
            or old_dependency in derived_dependencies
            or derived_dependencies.count(rewritten) != 1
        ):
            raise error_type("framework Python relocated dependency is not exact")
        _require_adhoc_signature(
            destination, _MACHO_TOOL_PATHS["codesign"], error_type
        )
        source = _artifact_record(
            source_path, source_dependencies, old_dependency,
            digest_regular, error_type, source=True,
        )
        derived = _artifact_record(
            destination, derived_dependencies, rewritten,
            digest_regular, error_type, source=False,
        )
        artifacts[name] = {
            "path": relative,
            "source": source,
            "derived": derived,
        }
    observed = {
        "format": "iroha-sumeragi-v2-framework-python-relocation",
        "schema_version": 1,
        "framework": framework,
        "tools": tools,
        "artifacts": artifacts,
    }
    if observed != expected:
        raise error_type("framework Python relocation provenance changed")


def bind_framework_python_relocation(
    inputs: list[dict[str, object]],
    outputs: list[dict[str, object]],
    contract: object,
    error_type: type[Exception],
    *,
    update: bool,
) -> set[str]:
    """Bind original records to the two intentionally derived archive members."""

    document = _validate_framework_python_relocation_contract(
        contract, error_type
    )
    input_by_path = {
        record.get("path"): record for record in inputs if isinstance(record, dict)
    }
    output_by_path = {
        record.get("path"): record for record in outputs if isinstance(record, dict)
    }
    relocated: set[str] = set()
    for name, (output_path, _) in _MACHO_REWRITES.items():
        input_path = "python3" if name == "launcher" else output_path
        source = input_by_path.get(input_path)
        output = output_by_path.get(output_path)
        artifact = document["artifacts"][name]
        if (
            not isinstance(source, dict)
            or not isinstance(output, dict)
            or source.get("kind") != "file"
            or output.get("kind") != "file"
            or (
                source.get("source_mode"), source.get("sha256"),
                source.get("size"),
            )
            != (
                artifact["source"]["mode"], artifact["source"]["sha256"],
                artifact["source"]["size_bytes"],
            )
            or (
                output.get("mode"), output.get("sha256"), output.get("size"),
            )
            != (
                artifact["derived"]["mode"], artifact["derived"]["sha256"],
                artifact["derived"]["size_bytes"],
            )
        ):
            raise error_type("framework Python source/derived binding changed")
        destination = {
            "destination_device": output.get("device"),
            "destination_inode": output.get("inode"),
            "destination_mode": output.get("mode"),
        }
        if update:
            source.update(destination)
        elif any(source.get(key) != value for key, value in destination.items()):
            raise error_type("framework Python destination provenance changed")
        relocated.add(input_path)
    return relocated


def run(
    *,
    error_type: type[Exception],
    cleanup_invocation,
    cleanup_sdk_command_work,
    copy_cache,
    copy_framework_python_runtime,
    copy_private_bundle,
    copy_runtime,
    copy_sdk_dependencies,
    create_sdk_command_work,
    publish_validation_failure,
    seal_release_result,
    snapshot_cache,
    verify_cache_sources,
    verify_framework_python_runtime,
    verify_private_bundle,
    verify_runtime_sources,
    verify_sdk_dependencies,
    argv: list[str] | None = None,
) -> int:
    """Parse ``argv`` and dispatch through the explicitly supplied operations."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--source-cargo-home", type=Path)
    parser.add_argument("--cargo-home", type=Path)
    parser.add_argument("--inventory", type=Path)
    parser.add_argument("--final", action="store_true")
    parser.add_argument("--publish-validation-failure", action="store_true")
    parser.add_argument("--seal-release-result", action="store_true")
    parser.add_argument("--copy-runtime", action="store_true")
    parser.add_argument("--copy-framework-python", action="store_true")
    parser.add_argument("--verify-framework-python", action="store_true")
    parser.add_argument("--verify-runtime-sources", action="store_true")
    parser.add_argument("--verify-cache-sources", action="store_true")
    parser.add_argument("--copy-private-bundle", action="store_true")
    parser.add_argument("--verify-private-bundle", action="store_true")
    parser.add_argument("--copy-sdk-dependencies", action="store_true")
    parser.add_argument("--verify-sdk-dependencies", action="store_true")
    parser.add_argument("--create-sdk-command-work", action="store_true")
    parser.add_argument("--cleanup-sdk-command-work", action="store_true")
    parser.add_argument("--cleanup-invocation", action="store_true")
    parser.add_argument("--runtime-root", type=Path)
    parser.add_argument("--runtime-inventory", type=Path)
    parser.add_argument("--runtime-source", type=Path, action="append", default=[])
    parser.add_argument("--bundle-source", type=Path)
    parser.add_argument("--bundle-root", type=Path)
    parser.add_argument("--sdk-dependency-bundle-manifest", type=Path)
    parser.add_argument("--expected-sdk-dependency-bundle-manifest-sha256")
    parser.add_argument("--repository-root", type=Path)
    parser.add_argument("--sdk-input-root", type=Path)
    parser.add_argument("--sdk-work-root", type=Path)
    parser.add_argument("--sdk-archive", type=Path)
    parser.add_argument("--sdk-dependency-inventory", type=Path)
    parser.add_argument("--sdk-work-final-inventory", type=Path)
    parser.add_argument("--invocation-root", type=Path)
    parser.add_argument("--bootstrap-evidence", type=Path)
    parser.add_argument("--source-manifest-sha256")
    parser.add_argument("--candidate-root", type=Path)
    parser.add_argument("--scaling-evidence-manifest", type=Path)
    parser.add_argument("--expected-signer-fingerprint")
    parser.add_argument("--expected-scaling-trial-harness-sha256")
    parser.add_argument("--expected-scaling-configuration-sha256")
    parser.add_argument("--expected-scaling-irohad-sha256")
    parser.add_argument("--expected-scaling-iroha-cli-sha256")
    parser.add_argument("--validator-exit-status", type=int)
    parser.add_argument("--cleanup-base", type=Path)
    parser.add_argument("--cleanup-prefix")
    args = parser.parse_args(argv)
    try:
        if args.create_sdk_command_work or args.cleanup_sdk_command_work:
            if (
                args.create_sdk_command_work == args.cleanup_sdk_command_work
                or args.sdk_input_root is None
                or args.sdk_work_root is None
                or any((
                    args.final, args.publish_validation_failure,
                    args.seal_release_result, args.copy_runtime,
                    args.copy_framework_python, args.verify_framework_python,
                    args.verify_runtime_sources, args.verify_cache_sources,
                    args.copy_private_bundle, args.verify_private_bundle,
                    args.copy_sdk_dependencies, args.verify_sdk_dependencies,
                    args.cleanup_invocation,
                ))
                or args.runtime_source
                or any(value is not None for value in (
                    args.source_cargo_home, args.cargo_home, args.inventory,
                    args.runtime_root, args.runtime_inventory,
                    args.bundle_source, args.bundle_root,
                    args.sdk_dependency_bundle_manifest,
                    args.expected_sdk_dependency_bundle_manifest_sha256,
                    args.repository_root, args.sdk_archive,
                    args.sdk_dependency_inventory,
                    args.sdk_work_final_inventory, args.invocation_root,
                    args.bootstrap_evidence, args.source_manifest_sha256,
                    args.candidate_root, args.scaling_evidence_manifest,
                    args.expected_signer_fingerprint,
                    args.expected_scaling_trial_harness_sha256,
                    args.expected_scaling_configuration_sha256,
                    args.expected_scaling_irohad_sha256,
                    args.expected_scaling_iroha_cli_sha256,
                    args.validator_exit_status, args.cleanup_base,
                    args.cleanup_prefix,
                ))
            ):
                raise error_type("SDK command work inputs are not exact")
            if args.create_sdk_command_work:
                create_sdk_command_work(args.sdk_input_root, args.sdk_work_root)
            else:
                cleanup_sdk_command_work(args.sdk_input_root, args.sdk_work_root)
            return 0
        if args.copy_sdk_dependencies or args.verify_sdk_dependencies:
            if (
                args.copy_sdk_dependencies == args.verify_sdk_dependencies
                or any(value is None for value in (
                    args.sdk_dependency_bundle_manifest,
                    args.expected_sdk_dependency_bundle_manifest_sha256,
                    args.repository_root, args.sdk_input_root, args.sdk_work_root,
                    args.sdk_archive, args.sdk_dependency_inventory,
                ))
                or (args.copy_sdk_dependencies and args.sdk_work_final_inventory is not None)
                or (args.verify_sdk_dependencies and args.sdk_work_final_inventory is None)
                or any((
                    args.final, args.publish_validation_failure,
                    args.seal_release_result, args.copy_runtime,
                    args.copy_framework_python, args.verify_framework_python,
                    args.verify_runtime_sources, args.verify_cache_sources,
                    args.copy_private_bundle, args.verify_private_bundle,
                    args.create_sdk_command_work,
                    args.cleanup_sdk_command_work,
                    args.cleanup_invocation,
                ))
                or any(value is not None for value in (
                    args.source_cargo_home, args.cargo_home, args.inventory,
                    args.runtime_root, args.runtime_inventory,
                    args.bundle_source, args.bundle_root,
                    args.invocation_root, args.bootstrap_evidence,
                    args.source_manifest_sha256, args.candidate_root,
                    args.scaling_evidence_manifest,
                    args.expected_signer_fingerprint,
                    args.expected_scaling_trial_harness_sha256,
                    args.expected_scaling_configuration_sha256,
                    args.expected_scaling_irohad_sha256,
                    args.expected_scaling_iroha_cli_sha256,
                    args.validator_exit_status, args.cleanup_base,
                    args.cleanup_prefix,
                ))
                or args.runtime_source
            ):
                raise error_type("SDK dependency bundle inputs are not exact")
            sdk_arguments = (
                args.sdk_dependency_bundle_manifest,
                args.expected_sdk_dependency_bundle_manifest_sha256,
                args.repository_root, args.sdk_input_root, args.sdk_work_root,
                args.sdk_archive, args.sdk_dependency_inventory,
            )
            if args.copy_sdk_dependencies:
                copy_sdk_dependencies(*sdk_arguments)
            else:
                verify_sdk_dependencies(
                    *sdk_arguments,
                    final_work_inventory=args.sdk_work_final_inventory,
                )
            return 0
        if args.copy_framework_python or args.verify_framework_python:
            if (
                args.copy_framework_python == args.verify_framework_python
                or args.runtime_root is None
                or args.runtime_inventory is None
                or args.runtime_source
                or args.source_cargo_home is not None
                or args.cargo_home is not None
                or args.inventory is not None
                or args.final
                or args.publish_validation_failure
                or args.seal_release_result
                or args.copy_runtime
                or args.verify_runtime_sources
                or args.verify_cache_sources
                or args.copy_private_bundle
                or args.verify_private_bundle
                or args.copy_sdk_dependencies
                or args.verify_sdk_dependencies
                or args.create_sdk_command_work
                or args.cleanup_sdk_command_work
                or args.cleanup_invocation
            ):
                raise error_type("framework Python runtime inputs are not exact")
            if args.copy_framework_python:
                copy_framework_python_runtime(
                    args.runtime_root, args.runtime_inventory,
                )
            else:
                verify_framework_python_runtime(
                    args.runtime_root, args.runtime_inventory,
                )
            return 0
        if args.verify_cache_sources:
            if args.source_cargo_home is None or args.cargo_home is None or args.inventory is None:
                raise error_type("caller cache verification lacks required inputs")
            verify_cache_sources(args.source_cargo_home, args.cargo_home, args.inventory)
        elif args.verify_private_bundle:
            if args.bundle_source is None or args.bundle_root is None or args.inventory is None:
                raise error_type("private bundle verification lacks required inputs")
            verify_private_bundle(args.bundle_source, args.bundle_root, args.inventory)
        elif args.copy_private_bundle:
            if args.bundle_source is None or args.bundle_root is None or args.inventory is None:
                raise error_type("private bundle copy lacks required inputs")
            copy_private_bundle(args.bundle_source, args.bundle_root, args.inventory)
        elif args.verify_runtime_sources:
            if args.runtime_root is None or args.runtime_inventory is None:
                raise error_type("runtime source verification lacks its inventory")
            verify_runtime_sources(args.runtime_source, args.runtime_root, args.runtime_inventory)
        elif args.cleanup_invocation:
            if args.cleanup_base is None or args.invocation_root is None or args.cleanup_prefix is None:
                raise error_type("private invocation cleanup lacks required inputs")
            cleanup_invocation(args.cleanup_base, args.invocation_root, args.cleanup_prefix)
        elif args.publish_validation_failure:
            if any(value is None for value in (
                args.invocation_root, args.bootstrap_evidence, args.cleanup_base,
                args.cleanup_prefix, args.source_manifest_sha256,
                args.validator_exit_status,
            )):
                raise error_type("receipt validation failure lacks required inputs")
            publish_validation_failure(
                args.invocation_root, args.bootstrap_evidence, args.cleanup_base,
                args.cleanup_prefix, args.source_manifest_sha256,
                args.validator_exit_status,
            )
        elif args.copy_runtime:
            if args.runtime_root is None or args.runtime_inventory is None:
                raise error_type("private child runtime lacks its root")
            copy_runtime(args.runtime_root, args.runtime_source, args.runtime_inventory)
        elif args.seal_release_result:
            if any(value is None for value in (
                args.invocation_root,
                args.bootstrap_evidence,
                args.source_manifest_sha256,
                args.candidate_root,
                args.scaling_evidence_manifest,
                args.expected_signer_fingerprint,
                args.expected_scaling_trial_harness_sha256,
                args.expected_scaling_configuration_sha256,
                args.expected_scaling_irohad_sha256,
                args.expected_scaling_iroha_cli_sha256,
            )):
                raise error_type("retained release publication lacks required inputs")
            seal_release_result(
                args.invocation_root,
                args.bootstrap_evidence,
                args.source_manifest_sha256,
                args.candidate_root,
                args.scaling_evidence_manifest,
                args.expected_signer_fingerprint,
                args.expected_scaling_trial_harness_sha256,
                args.expected_scaling_configuration_sha256,
                args.expected_scaling_irohad_sha256,
                args.expected_scaling_iroha_cli_sha256,
            )
        elif args.final:
            if args.cargo_home is None or args.inventory is None:
                raise error_type("final cache snapshot lacks required paths")
            if args.source_cargo_home is not None:
                raise error_type("final cache snapshot does not accept a source home")
            snapshot_cache(args.cargo_home, args.inventory)
        else:
            if args.source_cargo_home is None or args.cargo_home is None or args.inventory is None:
                raise error_type("cache copy requires a source home")
            copy_cache(args.source_cargo_home, args.cargo_home, args.inventory)
    except (error_type, OSError) as error:
        print(f"release Cargo cache isolation failed: {error}", file=sys.stderr)
        return 1
    return 0
