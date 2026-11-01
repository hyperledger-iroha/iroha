"""File/source admission controls; inert bytes never qualify a native build.

These controls do not compile, install or load native code. The real maintained
gate supplies original clean source, fresh build and ABI evidence separately.
"""

from __future__ import annotations

import base64
import csv
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import stat
import tempfile
import uuid
import shutil
import sys
import zipfile

import pytest
from native_bash_fixture import bash_executable


NATIVE_NAME = "_crypto.pyd" if os.name == "nt" else "_crypto.abi3.so"
WHEEL_TAG = "cp39-abi3-win_amd64" if os.name == "nt" else "cp39-abi3-linux_x86_64"

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "ci"))
SPEC = importlib.util.spec_from_file_location("native_source_delivery_controls", ROOT / "ci/python_native_source_delivery.py")
delivery = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(delivery)


def make_wheel(path: Path, source: Path, native: bytes) -> Path:
    """Produce archive/RECORD control data, with no native execution claim."""
    entries = {"iroha_native/__init__.py": (source / "__init__.py").read_bytes(),
               "iroha_native/_loader.py": (source / "_loader.py").read_bytes(),
               f"iroha_native/{NATIVE_NAME}": native,
               "iroha_native-0.0.1.dist-info/METADATA": b"Metadata-Version: 2.1\nName: iroha-native\nVersion: 0.0.1\n",
               "iroha_native-0.0.1.dist-info/WHEEL": f"Wheel-Version: 1.0\nGenerator: inert-file-control\nRoot-Is-Purelib: false\nTag: {WHEEL_TAG}\n".encode()}
    output = io.StringIO()
    writer = csv.writer(output, lineterminator="\n")
    for name, raw in entries.items():
        digest = base64.urlsafe_b64encode(hashlib.sha256(raw).digest()).rstrip(b"=").decode()
        writer.writerow([name, "sha256=" + digest, str(len(raw))])
    record = "iroha_native-0.0.1.dist-info/RECORD"
    writer.writerow([record, "", ""])
    entries[record] = output.getvalue().encode()
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, raw in entries.items():
            archive.writestr(name, raw)
    return path


@pytest.fixture
def originals(tmp_path, monkeypatch, request):
    if os.name == "nt":
        # CPython 3.12 mode0700 uses OWNER RIGHTS, which is deliberately not the
        # canonical exact-SID custody policy. Use normal inherited Temp ancestry;
        # the native helper creates the private backup with its exact initial ACL.
        tmp_path = (Path(tempfile.gettempdir()) / ("iroha-native-controls-" + uuid.uuid4().hex)).resolve()
        tmp_path.mkdir(mode=0o777)
        request.addfinalizer(lambda: shutil.rmtree(tmp_path))
    root = tmp_path / "source"
    source = root / "python/iroha_native/src/iroha_native"
    source.mkdir(parents=True)
    (source / "__init__.py").write_bytes(b'"""Inert source owner control."""\n')
    (source / "_loader.py").write_bytes(b'"""No native code is loaded in file controls."""\n')
    old = b"original private source artifact control"
    (source / NATIVE_NAME).write_bytes(old)
    native = b"fresh native member file-relation control"
    wheel_path = make_wheel(tmp_path / "fresh.whl", source, native)
    wheel_seal = delivery.wheel.seal_wheel(wheel_path).render()
    # This explicit stub isolates file delivery. It is neither Git identity nor
    # genuine native ABI evidence and is never presented as qualification.
    identity = {"head_commit": "1" * 40, "workspace_source_manifest_sha256": "2" * 64}
    pin = "unqualified-source-admission-unit-control"
    monkeypatch.setattr(delivery, "assert_source_pin", lambda root, expected: identity)
    manifest = {"schema": delivery.artifact.SCHEMA, "sdk": "python", "target": "inert-unit-control",
                "source_commit": identity["head_commit"], "source_tree_clean": True,
                "workspace_source_manifest_sha256": identity["workspace_source_manifest_sha256"],
                "artifact_sha256": hashlib.sha256(native).hexdigest(), "artifact_size": len(native),
                "bridge_abi_version": 33, "required_symbols": list(delivery.artifact.REQUIRED_SYMBOLS["python"]),
                "privacy_c_exports": [], "privacy_c_exports_inspected": False}
    manifest_path = tmp_path / "manifest.json"
    manifest_path.write_bytes(delivery.artifact.canonical_manifest_bytes(manifest))
    manifest_seal = delivery.wheel.seal_wheel(manifest_path).render()
    state = delivery._json(delivery.inspect_source(root))
    backup = root / ".codex-native-backup"
    if os.name == "nt":
        # Exact private creation belongs to the real native owner, including in
        # tests. EOF closes its original handles before test-specific mutations.
        session = delivery._NativeDeliverySession(_configured_native_helper())
        try:
            session.request("begin", source=str(source), backup=str(backup), name=NATIVE_NAME,
                            old_sha256=hashlib.sha256(old).hexdigest())
        finally:
            session.close()
    else:
        backup.mkdir(mode=0o700)
    return root, source, old, native, pin, state, wheel_path, wheel_seal, manifest_path, manifest_seal, backup


@pytest.fixture
def retained_native_helper():
    """Native component CI supplies the freshly built filesystem tool explicitly."""
    configured = os.environ.get("IROHA_NATIVE_SOURCE_DELIVERY_TEST_HELPER")
    if configured is None:
        pytest.skip("retained native helper controls require the explicit compiled test tool")
    helper = Path(configured).resolve(strict=True)
    assert helper.is_file(), "configured native helper must exist; never silently skip it"
    return helper


def _configured_native_helper():
    configured = os.environ.get("IROHA_NATIVE_SOURCE_DELIVERY_TEST_HELPER")
    assert configured, "Windows controls require the freshly built native delivery helper"
    return Path(configured).resolve(strict=True)


def _replace_bytes(path, raw):
    """Mutate an unretained owner even when its original private seal is read-only."""
    if os.name == "nt":
        path.rename(path.with_name(path.name + ".retained-before-mutation"))
    path.write_bytes(raw)


def _observe_moves(originals, monkeypatch, before=None, after=None):
    """Inject the same semantic namespace boundary into either native platform owner."""
    if os.name == "nt":
        actual = delivery._NativeDeliverySession.request

        def request(session, operation, **fields):
            name = None
            if operation == "backup":
                name = NATIVE_NAME
            elif operation == "activate":
                name = next(originals[-1].glob(".fresh-native-*")).name
            if name is not None and before is not None:
                before(name, NATIVE_NAME)
            result = actual(session, operation, **fields)
            if name is not None and after is not None:
                after(name, NATIVE_NAME)
            return result

        monkeypatch.setattr(delivery._NativeDeliverySession, "request", request)
    else:
        actual = delivery._move_no_replace

        def move(source_fd, source_name, destination_fd, destination_name):
            if before is not None:
                before(source_name, destination_name)
            result = actual(source_fd, source_name, destination_fd, destination_name)
            if after is not None:
                after(source_name, destination_name)
            return result

        monkeypatch.setattr(delivery, "_move_no_replace", move)


def promote_retained(originals, helper):
    root, _, _, _, pin, state, wheel_path, wheel_seal, manifest_path, manifest_seal, backup = originals
    # The native creator installs exact private access at initial creation, including
    # Windows protected SID/DACL custody; never repair a pre-existing public directory.
    backup.rmdir()
    return delivery.promote_source(root, pin, state, wheel_path, wheel_seal,
                                   manifest_path, manifest_seal, backup, native_helper=helper)


def test_regular_reader_preserves_binary_bytes_and_noninheritance(tmp_path):
    path = (tmp_path / "binary-input").resolve()
    raw = bytes(range(256)) + b"\r\n\x1a\r\n\x00"
    path.write_bytes(raw)
    descriptor = delivery.wheel._open_regular_reader(path, os.O_RDONLY | getattr(os, "O_BINARY", 0))
    try:
        assert not os.get_inheritable(descriptor)
        assert os.read(descriptor, len(raw) + 1) == raw
    finally:
        os.close(descriptor)
    observed, seal = delivery.wheel._read_stable_regular_file(path, label="binary control", max_bytes=1024)
    assert observed == raw and seal.sha256 == hashlib.sha256(raw).hexdigest()


@pytest.mark.parametrize("has_original", [True, False])
def test_retained_native_exact_delivery_and_receipt(originals, retained_native_helper, has_original):
    values = list(originals)
    if not has_original:
        (values[1] / NATIVE_NAME).unlink()
        values[5] = delivery._json(delivery.inspect_source(values[0]))
    values = tuple(values)
    receipt = promote_retained(values, retained_native_helper)
    assert (values[1] / NATIVE_NAME).read_bytes() == values[3]
    if has_original:
        assert (values[-1] / NATIVE_NAME).read_bytes() == values[2]
    verify(values, receipt)


@pytest.mark.parametrize("boundary", ["backup", "activate", "receipt"])
def test_retained_native_interruption_preserves_authentic_objects(
    originals, retained_native_helper, monkeypatch, boundary
):
    original_request = delivery._NativeDeliverySession.request

    def interrupted(session, operation, **fields):
        if operation == boundary:
            raise delivery.DeliveryError("injected parent interruption")
        return original_request(session, operation, **fields)

    monkeypatch.setattr(delivery._NativeDeliverySession, "request", interrupted)
    with pytest.raises(delivery.DeliveryError, match="injected parent interruption"):
        promote_retained(originals, retained_native_helper)
    _, source, old, native, *_, backup = originals
    assert (source / NATIVE_NAME if boundary == "backup" else backup / NATIVE_NAME).read_bytes() == old
    if boundary == "receipt":
        assert (source / NATIVE_NAME).read_bytes() == native
    else:
        staged = tuple(backup.glob(".fresh-native-*"))
        assert len(staged) == 1 and staged[0].read_bytes() == native
    assert (backup / "intent.json").is_file()
    assert not (backup / "delivery.json").exists()


def test_retained_native_racing_destination_is_preserved(originals, retained_native_helper, monkeypatch):
    original_request = delivery._NativeDeliverySession.request
    source, backup = originals[1], originals[-1]

    def competing(session, operation, **fields):
        if operation == "activate":
            (source / NATIVE_NAME).write_bytes(b"competing owner")
        return original_request(session, operation, **fields)

    monkeypatch.setattr(delivery._NativeDeliverySession, "request", competing)
    with pytest.raises(delivery.DeliveryError, match="native delivery owner refused"):
        promote_retained(originals, retained_native_helper)
    assert (source / NATIVE_NAME).read_bytes() == b"competing owner"
    assert (backup / NATIVE_NAME).read_bytes() == originals[2]
    assert not (backup / "delivery.json").exists()


def test_retained_native_same_bytes_substitution_before_admission_is_refused(
    originals, retained_native_helper, monkeypatch
):
    original_init = delivery._NativeDeliverySession.__init__
    source, backup = originals[1], originals[-1]

    def substituted(session, executable):
        original_init(session, executable)
        native = source / NATIVE_NAME
        replacement = originals[0] / "substitute"
        replacement.write_bytes(native.read_bytes())
        replacement.replace(native)

    monkeypatch.setattr(delivery._NativeDeliverySession, "__init__", substituted)
    with pytest.raises(delivery.DeliveryError, match="changed before retained admission"):
        promote_retained(originals, retained_native_helper)
    assert (source / NATIVE_NAME).read_bytes() == originals[2]
    assert not (backup / "intent.json").exists()


def test_retained_native_changed_receipt_is_refused_before_completion(originals, retained_native_helper, monkeypatch):
    request = delivery._NativeDeliverySession.request
    operations = []

    def change_receipt(session, operation, **fields):
        operations.append(operation)
        if operation == "receipt":
            receipt = json.loads(fields["json"])
            receipt["source_pin"] = "changed after parent authored receipt"
            fields["json"] = delivery._json(receipt)
        return request(session, operation, **fields)

    monkeypatch.setattr(delivery._NativeDeliverySession, "request", change_receipt)
    with pytest.raises(delivery.DeliveryError, match="receipt readback differs"):
        promote_retained(originals, retained_native_helper)
    assert "complete" not in operations
    assert (originals[-1] / "delivery.json").is_file()
    assert (originals[-1] / NATIVE_NAME).read_bytes() == originals[2]
    assert (originals[1] / NATIVE_NAME).read_bytes() == originals[3]


def test_retained_native_corrupt_stream_never_moves_original(originals, retained_native_helper, monkeypatch):
    original_request = delivery._NativeDeliverySession.request

    def corrupt(session, operation, **fields):
        if operation == "chunk":
            fields["hex"] = "00" * (len(fields["hex"]) // 2)
        return original_request(session, operation, **fields)

    monkeypatch.setattr(delivery._NativeDeliverySession, "request", corrupt)
    with pytest.raises(delivery.DeliveryError, match="native delivery owner refused"):
        promote_retained(originals, retained_native_helper)
    assert (originals[1] / NATIVE_NAME).read_bytes() == originals[2]
    assert not (originals[-1] / "delivery.json").exists()


def promote(originals):
    root, _, _, _, pin, state, wheel_path, wheel_seal, manifest_path, manifest_seal, backup = originals
    helper = _configured_native_helper() if os.name == "nt" else None
    return delivery.promote_source(root, pin, state, wheel_path, wheel_seal, manifest_path,
                                   manifest_seal, backup, native_helper=helper)


def verify(originals, receipt_seal):
    root, _, _, _, pin, _, wheel_path, wheel_seal, manifest_path, manifest_seal, backup = originals
    delivery.verify_source(root, pin, wheel_path, wheel_seal, manifest_path, manifest_seal,
                           backup / "delivery.json", receipt_seal.render())


def test_exact_original_artifact_moves_and_same_wheel_member_is_retained(originals):
    _, source, old, native, _, _, _, _, _, _, backup = originals
    old_inode = (source / NATIVE_NAME).stat().st_ino
    receipt = promote(originals)
    assert (source / NATIVE_NAME).read_bytes() == native
    assert (backup / NATIVE_NAME).read_bytes() == old
    assert (backup / NATIVE_NAME).stat().st_ino == old_inode
    assert (source / NATIVE_NAME).stat().st_nlink == 1
    assert (backup / NATIVE_NAME).stat().st_nlink == 1
    verify(originals, receipt)
    with pytest.raises(delivery.DeliveryError, match="changed during"):
        promote(originals)
    assert (backup / NATIVE_NAME).read_bytes() == old


@pytest.mark.parametrize("mutation", ["old", "owner", "wheel", "manifest", "backup", "sibling", "symlink"])
def test_pre_dispatch_mutation_preserves_original_and_creates_no_new_source(originals, mutation):
    _, source, old, _, _, _, wheel_path, _, manifest_path, _, backup = originals
    original = source / NATIVE_NAME
    if mutation == "old":
        original.write_bytes(old + b"changed")
    elif mutation == "owner":
        (source / "_loader.py").write_bytes(b"changed source")
    elif mutation == "wheel":
        wheel_path.write_bytes(wheel_path.read_bytes() + b"changed")
    elif mutation == "manifest":
        manifest_path.write_bytes(manifest_path.read_bytes() + b"changed")
    elif mutation == "backup":
        (backup / "foreign").write_bytes(b"preserve me")
    elif mutation == "sibling":
        (source / "_crypto.other.so").write_bytes(b"foreign artifact")
    else:
        original.rename(source / "original.bin")
        original.symlink_to(source / "original.bin")
    before = original.read_bytes()
    with pytest.raises((delivery.DeliveryError, delivery.wheel.VerificationError)):
        promote(originals)
    assert original.read_bytes() == before
    assert not (backup / "delivery.json").exists()
    assert not (backup / NATIVE_NAME).exists()


def test_competing_source_is_not_overwritten_and_old_original_is_retained(originals, monkeypatch):
    _, source, old, native, _, _, _, _, _, _, backup = originals
    old_inode = (source / NATIVE_NAME).stat().st_ino

    def competing_move(source_name, destination_name):
        if source_name.startswith(".fresh-native-"):
            (source / destination_name).write_bytes(b"competing source artifact")
    _observe_moves(originals, monkeypatch, before=competing_move)
    with pytest.raises((FileExistsError, delivery.DeliveryError)):
        promote(originals)
    assert (source / NATIVE_NAME).read_bytes() == b"competing source artifact"
    assert (backup / NATIVE_NAME).read_bytes() == old
    assert (backup / NATIVE_NAME).stat().st_ino == old_inode
    assert (backup / "intent.json").exists()
    assert not (backup / "delivery.json").exists()
    assert not list(source.glob(".iroha-native-*.so"))
    candidates = list(backup.glob(".fresh-native-*"))
    assert len(candidates) == 1 and candidates[0].read_bytes() == native


def test_backup_name_race_is_refused_without_overwriting_any_owner(originals, monkeypatch):
    _, source, old, native, _, _, _, _, _, _, backup = originals
    old_inode = (source / NATIVE_NAME).stat().st_ino
    competitor = b"competing backup must survive"

    def competing_backup(source_name, destination_name):
        if source_name == NATIVE_NAME:
            (backup / destination_name).write_bytes(competitor)
    _observe_moves(originals, monkeypatch, before=competing_backup)
    with pytest.raises((FileExistsError, delivery.DeliveryError)):
        promote(originals)
    assert (source / NATIVE_NAME).read_bytes() == old
    assert (source / NATIVE_NAME).stat().st_ino == old_inode
    assert (backup / NATIVE_NAME).read_bytes() == competitor
    assert (backup / "intent.json").exists()
    assert not (backup / "delivery.json").exists()
    candidates = list(backup.glob(".fresh-native-*"))
    assert len(candidates) == 1 and candidates[0].read_bytes() == native


@pytest.mark.parametrize("mutation", ["source", "backup", "receipt"])
def test_revalidation_refuses_every_retained_owner_change(originals, mutation):
    _, source, _, _, _, _, _, _, _, _, backup = originals
    receipt = promote(originals)
    path = {"source": source / NATIVE_NAME, "backup": backup / NATIVE_NAME,
            "receipt": backup / "delivery.json"}[mutation]
    _replace_bytes(path, path.read_bytes() + b"changed")
    with pytest.raises((delivery.DeliveryError, delivery.wheel.VerificationError)):
        verify(originals, receipt)


def test_before_build_source_pin_is_exact_and_refuses_change_or_dirty_source(tmp_path, monkeypatch):
    identity = {"head_commit": "1" * 40, "cargo_lock_sha256": "2" * 64}
    monkeypatch.setattr(delivery, "release_source_identity", lambda root: dict(identity))
    pin = delivery.source_pin(tmp_path)
    assert delivery.assert_source_pin(tmp_path, pin) == identity
    identity["cargo_lock_sha256"] = "3" * 64
    with pytest.raises(delivery.DeliveryError, match="since the before-build"):
        delivery.assert_source_pin(tmp_path, pin)

    def dirty(root):
        raise RuntimeError("existing clean-source owner refused dirty source")

    monkeypatch.setattr(delivery, "release_source_identity", dirty)
    with pytest.raises(RuntimeError, match="refused dirty source"):
        delivery.source_pin(tmp_path)


def test_original_before_build_shell_refusal_stops_before_builder_dispatch(tmp_path):
    runner = (ROOT / "ci/check_sorafs_python_native_sdk.sh").read_text()
    start = runner.index('SOURCE_DELIVERY=')
    end = runner.index('"${PYTHON_BIN}" -m venv', start)
    capture_start = runner.index('capture_python() {')
    capture_end = runner.index('\nPYTHON_VERSION=', capture_start)
    command = runner[capture_start:capture_end] + runner[start:end] + '\nprintf dispatched > "$ROOT_DIR/builder-marker"\n'
    # Explicit dispatch control only; this child produces no source identity.
    child = tmp_path / "refuse-source"
    child.write_text('#!/bin/sh\nexit 78\n', newline="\n")
    child.chmod(0o700)
    result = subprocess.run([bash_executable(), "-euo", "pipefail", "-c", command],
                            env=dict(os.environ, ROOT_DIR=tmp_path.as_posix(), PYTHON_BIN=child.as_posix()),
                            capture_output=True, text=True, timeout=10)
    assert result.returncode == 78
    assert not (tmp_path / "builder-marker").exists()


def test_invalid_closed_control_and_unsupported_directory_operations_refuse(originals, monkeypatch):
    with pytest.raises(delivery.artifact.ArtifactContractError, match="duplicate key"):
        delivery._control(b'{"schema":"x","schema":"x"}', delivery.DELIVERY_SCHEMA)
    monkeypatch.setattr(delivery.os, "supports_dir_fd", set())
    with pytest.raises(delivery.DeliveryError, match="descriptor-relative"):
        delivery._open_directory(originals[-1])
    assert not (originals[-1] / "intent.json").exists()


def test_isolated_source_pin_cli_reaches_existing_clean_owner_without_sdk_imports(tmp_path):
    # The real owner refuses this unqualified root. No synthetic Git identity
    # substitutes for the original owner, and the child never loads native code.
    result = subprocess.run([sys.executable, "-I", "-B", str(ROOT / "ci/python_native_source_delivery.py"),
                             "pin", "--root", str(tmp_path)],
                            capture_output=True, text=True, timeout=10)
    assert result.returncode == 1
    assert "ModuleNotFoundError" not in result.stderr
    assert any(reason in result.stderr for reason in ("git command failed", "not a git repository",
                                                    "release worktree has tracked changes"))
    assert not result.stdout


def test_partial_native_file_write_refusal_removes_only_its_own_temporary(originals, monkeypatch):
    _, source, old, _, _, _, _, _, _, _, backup = originals
    fsync = delivery.os.fsync

    def refuse_native_fsync(descriptor):
        if stat.S_ISREG(delivery.os.fstat(descriptor).st_mode):
            # intent.json is already durable; refuse only the actual new native
            # file. Do not weaken source or wheel admission to reach this point.
            size = delivery.os.fstat(descriptor).st_size
            if size == len(originals[3]):
                raise OSError("actual new native write refused")
        return fsync(descriptor)

    if os.name == "nt":
        actual = delivery._NativeDeliverySession.request

        def refuse_native_seal(session, operation, **fields):
            if operation == "seal":
                raise OSError("actual new native write refused")
            return actual(session, operation, **fields)

        monkeypatch.setattr(delivery._NativeDeliverySession, "request", refuse_native_seal)
    else:
        monkeypatch.setattr(delivery.os, "fsync", refuse_native_fsync)
    with pytest.raises(OSError, match="new native write refused"):
        promote(originals)
    assert (source / NATIVE_NAME).read_bytes() == old
    assert not list(source.glob(".iroha-native-*.so"))
    assert (backup / "intent.json").exists()
    assert not (backup / "delivery.json").exists()


@pytest.mark.parametrize("mutation", ["omit-backup", "before-identity", "before-fields", "owner-fields"])
def test_rebound_receipt_still_requires_complete_original_custody_graph(originals, mutation):
    receipt = promote(originals)
    path = originals[-1] / "delivery.json"
    value = json.loads(path.read_bytes())
    if mutation == "omit-backup":
        value["original_backup"] = None
    elif mutation == "before-identity":
        original = delivery.wheel.FileSeal.parse(value["source_before"]["native"]["seal"])
        parts = original.render().split(":")
        parts[2] = str(original.inode + 1)
        value["source_before"]["native"]["seal"] = ":".join(parts)
    elif mutation == "before-fields":
        value["source_before"]["unexpected"] = True
    else:
        value["source_before"]["files"][0]["unexpected"] = True
    _replace_bytes(path, delivery._json(value).encode())
    # Even a supplied new receipt seal cannot substitute an omitted or partial
    # original custody binding for the complete source graph.
    changed = delivery.wheel.seal_wheel(path)
    with pytest.raises(delivery.DeliveryError):
        verify(originals, changed)


@pytest.mark.parametrize("arguments", [
    ["pin", "--source-pin", "unexpected"],
    ["assert-pin"],
    ["inspect", "--receipt", "unexpected"],
    ["verify", "--source-pin", "missing-original-bindings"],
    ["pin", "--roo", "unexpected-alias"],
])
def test_cli_refuses_incomplete_or_extra_field_inventory_without_source_dispatch(tmp_path, arguments):
    result = subprocess.run([sys.executable, "-I", "-B", str(ROOT / "ci/python_native_source_delivery.py"),
                             *arguments, "--root", str(tmp_path)],
                            capture_output=True, text=True, timeout=10)
    assert result.returncode == 2
    assert "exact original field inventory" in result.stderr or "unrecognized arguments" in result.stderr
    assert "release worktree" not in result.stderr
    assert not result.stdout


def test_private_temporary_replacement_is_retained_on_refusal_and_descriptors_close(originals, monkeypatch):
    _, source, old, native, _, _, _, _, _, _, backup = originals
    if os.name == "nt":
        sessions = []
        actual_init = delivery._NativeDeliverySession.__init__

        def retained_init(session, executable):
            actual_init(session, executable)
            sessions.append(session)

        def refuse_after_substitution_attempt(source_name, destination_name):
            if source_name.startswith(".fresh-native-"):
                with pytest.raises(OSError):
                    (backup / source_name).rename(backup / "retained-original-native-temporary")
                raise OSError("source move refused")

        monkeypatch.setattr(delivery._NativeDeliverySession, "__init__", retained_init)
        _observe_moves(originals, monkeypatch, before=refuse_after_substitution_attempt)
        with pytest.raises(OSError, match="source move refused"):
            promote(originals)
        candidates = tuple(backup.glob(".fresh-native-*"))
        assert len(candidates) == 1 and candidates[0].read_bytes() == native
        assert (backup / NATIVE_NAME).read_bytes() == old
        assert not (backup / "delivery.json").exists()
        assert not list(source.glob(".iroha-native-*"))
        assert all(session.process.poll() is not None and session.process.stdin.closed for session in sessions)
        candidates[0].rename(backup / "retained-original-native-temporary")
        assert (backup / "retained-original-native-temporary").read_bytes() == native
        return
    descriptors = []
    open_directory = delivery._open_directory
    actual_move = delivery._move_no_replace

    def retain_directory(path):
        descriptor = open_directory(path)
        descriptors.append(descriptor)
        return descriptor

    def replace_private_temporary_before_refused_move(source_fd, source_name, destination_fd, destination_name):
        if source_name.startswith(".fresh-native-"):
            path = backup / source_name
            path.rename(backup / "retained-original-native-temporary")
            path.write_bytes(b"competing temporary owner")
            raise OSError("source move refused")
        return actual_move(source_fd, source_name, destination_fd, destination_name)

    monkeypatch.setattr(delivery, "_open_directory", retain_directory)
    monkeypatch.setattr(delivery, "_move_no_replace", replace_private_temporary_before_refused_move)
    with pytest.raises(OSError, match="source move refused"):
        promote(originals)
    competitors = list(backup.glob(".fresh-native-*.so"))
    assert len(competitors) == 1 and competitors[0].read_bytes() == b"competing temporary owner"
    assert (backup / "retained-original-native-temporary").read_bytes() == native
    assert not list(source.glob(".iroha-native-*.so"))
    assert (backup / NATIVE_NAME).read_bytes() == old
    assert not (backup / "delivery.json").exists()
    for descriptor in descriptors:
        with pytest.raises(OSError):
            delivery.os.fstat(descriptor)


def test_no_shared_path_unlink_is_used_for_success_or_refusal(originals, monkeypatch):
    def forbidden_unlink(*args, **kwargs):
        raise AssertionError("shared-name unlink would delete an unowned competitor")

    with monkeypatch.context() as active:
        active.setattr(delivery.os, "unlink", forbidden_unlink)
        receipt = promote(originals)
        verify(originals, receipt)


def test_equal_byte_native_substitution_cannot_qualify_a_different_inode(originals, monkeypatch):
    _, source, _, native, _, _, _, _, _, _, backup = originals

    def replace_after_move(source_name, destination_name):
        if source_name.startswith(".fresh-native-"):
            path = source / destination_name
            if os.name == "nt":
                with pytest.raises(OSError):
                    path.rename(backup / "retained-original-fresh-native")
                raise delivery.DeliveryError("original fresh native physical object cannot be replaced")
            path.rename(backup / "retained-original-fresh-native")
            path.write_bytes(native)

    _observe_moves(originals, monkeypatch, after=replace_after_move)
    with pytest.raises(delivery.DeliveryError, match="original fresh native physical object"):
        promote(originals)
    assert (source / NATIVE_NAME).read_bytes() == native
    if os.name == "nt":
        assert not (backup / "retained-original-fresh-native").exists()
        (source / NATIVE_NAME).rename(backup / "retained-original-fresh-native")
    assert (backup / "retained-original-fresh-native").read_bytes() == native
    assert not (backup / "delivery.json").exists()


def test_unsupported_atomic_move_host_refuses_before_intent(originals, monkeypatch):
    monkeypatch.setattr(delivery.sys, "platform", "unsupported-control-host")
    with pytest.raises(delivery.DeliveryError, match="atomic no-overwrite rename host"):
        delivery._atomic_move_function()
    assert not (originals[-1] / "intent.json").exists()


def test_fresh_backup_and_existing_parent_are_synced_before_old_source_removal(originals, monkeypatch):
    directories = {}
    events = []
    open_directory = delivery._open_directory
    fsync = delivery.os.fsync
    actual_move = delivery._move_no_replace
    backup = originals[-1]
    if os.name == "nt":
        # Native directory syncs are executed inside the retained owner. The
        # intent acknowledgement is emitted only after both durable syncs;
        # the subprocess keeps those handles through the later backup request.
        actual = delivery._NativeDeliverySession.request
        sessions = []

        def capture_boundary(session, operation, **fields):
            if operation == "backup":
                assert events == ["durable-intent"]
                assert session.process.poll() is None
                assert (backup / "intent.json").is_file()
                events.append("move-original")
            result = actual(session, operation, **fields)
            if operation == "intent":
                events.append("durable-intent")
                sessions.append(session)
            return result

        monkeypatch.setattr(delivery._NativeDeliverySession, "request", capture_boundary)
        receipt = promote(originals)
        verify(originals, receipt)
        assert events == ["durable-intent", "move-original"]
        assert all(session.process.poll() == 0 and session.process.stdin.closed for session in sessions)
        return

    def capture_directory(path):
        descriptor = open_directory(path)
        directories[descriptor] = path
        return descriptor

    def capture_fsync(descriptor):
        if descriptor in directories:
            events.append(("fsync", directories[descriptor]))
        return fsync(descriptor)

    def capture_move(source_fd, source_name, destination_fd, destination_name):
        if source_name == NATIVE_NAME:
            assert ("fsync", backup) in events
            assert ("fsync", backup.parent) in events
        events.append(("move", source_name))
        return actual_move(source_fd, source_name, destination_fd, destination_name)

    monkeypatch.setattr(delivery, "_open_directory", capture_directory)
    monkeypatch.setattr(delivery.os, "fsync", capture_fsync)
    monkeypatch.setattr(delivery, "_move_no_replace", capture_move)
    receipt = promote(originals)
    verify(originals, receipt)
    first_move = next(index for index, event in enumerate(events) if event[0] == "move")
    assert events.index(("fsync", backup)) < events.index(("fsync", backup.parent)) < first_move
    for descriptor in directories:
        with pytest.raises(OSError):
            delivery.os.fstat(descriptor)
