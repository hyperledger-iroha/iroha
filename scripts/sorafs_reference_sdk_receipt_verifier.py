"""Run the pinned native ReleaseManifest verifier over private public-input copies.

This is a verification-only subprocess boundary. It accepts no signing key,
environment credential or alternate verifier protocol. The caller owns the
independent input pins and trusted clock; native output remains untrusted until
the signed-manifest source owner validates its exact closed schema and bindings.
"""

from __future__ import annotations

import os
import selectors
import signal
import subprocess
import tempfile
import time
from contextlib import ExitStack
from pathlib import Path

import release_manifest_signing as native
from sorafs_evidence_json import read_evidence_bytes

MAX_RESULT_BYTES = 16 * 1024
VERIFIER_TIMEOUT_SECONDS = 120
INPUT_LIMITS = {
    "manifest": native.MAX_MANIFEST_SIZE, "signature": 64, "public_key": 32,
    "signer_policy": 64 * 1024, "custody_trust": 64 * 1024,
    "completed_operation_state": 64 * 1024, "operation_receipt": 64 * 1024,
}


class ReceiptVerifierError(ValueError):
    """Native verification failed without exposing source paths or diagnostics."""


def _group_exists(process: subprocess.Popen[bytes]) -> bool:
    try:
        os.killpg(process.pid, 0)
    except ProcessLookupError:
        return False
    return True


def _stop_owned_verifier(process: subprocess.Popen[bytes]) -> None:
    """Stop only this invocation's isolated process group and reap its child."""
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait(timeout=1)


def _run_native(command: list[str], directory: Path, identity) -> bytes:
    if os.name != "posix":
        raise ReceiptVerifierError("descriptor-isolated receipt verification is unavailable")
    process = None
    success = False
    result = bytearray()
    selector = selectors.DefaultSelector()
    try:
        process = subprocess.Popen(
            command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, cwd=directory,
            env=native._native_verifier_environment(), close_fds=True, pass_fds=(),
            start_new_session=True, bufsize=0,
            preexec_fn=native._external_tool_preexec(identity),
        )
        assert process.stdout is not None
        os.set_blocking(process.stdout.fileno(), False)
        selector.register(process.stdout, selectors.EVENT_READ)
        deadline = time.monotonic() + VERIFIER_TIMEOUT_SECONDS
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ReceiptVerifierError("native release receipt verification timed out")
            for key, _ in selector.select(min(remaining, 0.05)):
                try:
                    chunk = os.read(key.fd, min(4096, MAX_RESULT_BYTES + 1 - len(result)))
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(process.stdout)
                    break
                result.extend(chunk)
                if len(result) > MAX_RESULT_BYTES:
                    raise ReceiptVerifierError("native release receipt result exceeds its byte bound")
        remaining = deadline - time.monotonic()
        if remaining <= 0 or process.wait(timeout=remaining) != 0:
            raise ReceiptVerifierError("native ReleaseManifest receipt verification failed or is unavailable")
        if _group_exists(process):
            raise ReceiptVerifierError("native release receipt verifier left an active descendant")
        success = True
        return bytes(result)
    except ReceiptVerifierError:
        raise
    except (OSError, ValueError, subprocess.SubprocessError):
        raise ReceiptVerifierError("native ReleaseManifest receipt verification failed or is unavailable") from None
    finally:
        try:
            if process is not None and not success:
                _stop_owned_verifier(process)
        finally:
            if process is not None and process.stdout is not None:
                process.stdout.close()
            selector.close()


def verify_receipt_snapshots(
    snapshots: dict[str, Path], source_digests: dict[str, str],
    verifier_path: Path, verifier_sha256: str, now_unix_ms: int,
) -> bytes:
    """Verify the exact input tuple using a pinned, bounded native executable."""
    try:
        identity = native._external_tool_execution_identity()
        with tempfile.TemporaryDirectory(prefix=".sf11-receipt-native-", dir=snapshots["manifest"].parent) as temporary, ExitStack() as held:
            directory = Path(temporary)
            native._prepare_external_tool_directory(directory, identity)
            _, expected_lineage, descriptors = native._open_release_output_parent(directory)
            for descriptor in descriptors:
                held.callback(os.close, descriptor)
            inputs = {}
            for name, maximum in INPUT_LIMITS.items():
                payload = read_evidence_bytes(snapshots[name], maximum)
                path = directory / name
                native._install_exclusive(path, payload, name, mode=0o400)
                installed = native._handoff_external_tool_file(path, identity, mode=0o400)
                inputs[name] = (path, payload, installed)
            executable = native._native_snapshot_path(directory, verifier_path)
            digest, original_identity = native._snapshot_native_verifier(verifier_path, executable, verifier_sha256)
            native._handoff_external_tool_file(executable, identity, mode=0o500)
            snapshot_digest, snapshot_identity = native._stable_digest(executable, "receipt verifier snapshot", executable=True)
            if digest != snapshot_digest:
                raise ReceiptVerifierError("native release receipt executable changed")
            command = [str(executable), "release-manifest-receipt"]
            for name, (path, _, _) in inputs.items():
                command.extend(["--" + name.replace("_", "-"), str(path)])
            command.extend([
                "--public-key-fingerprint", source_digests["public_key"],
                "--signer-policy-sha256", source_digests["signer_policy"],
                "--custody-trust-sha256", source_digests["custody_trust"],
                "--now-unix-ms", str(now_unix_ms), "--format", "json",
            ])
            result = _run_native(command, directory, identity)
            native._assert_digest_unchanged(executable, "receipt verifier snapshot", digest, snapshot_identity, executable=True)
            native._assert_digest_unchanged(verifier_path, "receipt verifier source", digest, original_identity, executable=True)
            for name, (path, payload, installed) in inputs.items():
                native._assert_unchanged(path, name, payload, installed, max_size=INPUT_LIMITS[name])
            _, lineage, fresh = native._open_release_output_parent(directory)
            for descriptor in reversed(fresh):
                os.close(descriptor)
            if lineage != expected_lineage:
                raise ReceiptVerifierError("native receipt source directory changed")
            return result
    except ReceiptVerifierError:
        raise
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError):
        raise ReceiptVerifierError("native ReleaseManifest receipt verification failed or is unavailable") from None
