"""Adversarial tests for the SSH-only Sumeragi v2 release verifier."""

from __future__ import annotations

import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import textwrap
from typing import Any, Callable

import pytest


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
VERIFIER = REPOSITORY_ROOT / "scripts" / "verify_sumeragi_v2_release_identity.py"
FINGERPRINT = "SHA256:" + "A" * 43
PRIMARY_FINGERPRINT = ""
PRINCIPAL = "release@example.invalid"
PUBLIC_KEY = (
    "ssh-ed25519 "
    "AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
)
DATA_MODE = 0o400
TOOL_MODE = 0o500


def _load_verifier_module() -> Any:
    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_release_identity_verifier", VERIFIER
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


VERIFIER_MODULE = _load_verifier_module()


def test_identity_supervision_has_no_forbidden_process_controls() -> None:
    source = VERIFIER.read_text(encoding="utf-8")
    for forbidden in (
        "import signal",
        "os.kill(",
        "os.killpg(",
        ".kill(",
        ".terminate(",
        "start_new_session",
        "def _abort",
        "wait(timeout=",
    ):
        assert forbidden not in source


@pytest.mark.parametrize(
    ("timeout_seconds", "maximum_output_bytes", "program", "message"),
    [
        (
            0,
            1024,
            "import time; time.sleep(0.05)",
            "exceeded its timeout",
        ),
        (
            5,
            32,
            "import sys; "
            "sys.stdout.buffer.write(b'O' * 131072); sys.stdout.flush(); "
            "sys.stderr.buffer.write(b'E' * 131072); sys.stderr.flush()",
            "closed size limit",
        ),
    ],
)
def test_pinned_command_finishes_naturally_before_reporting_latched_violation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    timeout_seconds: int,
    maximum_output_bytes: int,
    program: str,
    message: str,
) -> None:
    monkeypatch.setattr(
        VERIFIER_MODULE, "_COMMAND_TIMEOUT_SECONDS", timeout_seconds
    )
    sentinel = tmp_path / "natural-completion"
    child = (
        f"{program}; from pathlib import Path; "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(VERIFIER_MODULE.VerificationError, match=message):
        VERIFIER_MODULE._run_bounded(
            Path(sys.executable).resolve(strict=True),
            ["-I", "-S", "-c", child],
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            maximum_output_bytes=maximum_output_bytes,
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


@pytest.mark.parametrize(
    "fault_method", ("register", "select", "read", "wait")
)
def test_pinned_command_drains_after_generic_supervisor_exception(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    fault_method: str,
) -> None:
    real_selector = VERIFIER_MODULE.selectors.DefaultSelector

    class FaultingSelector:
        def __init__(self) -> None:
            self._selector = real_selector()
            self._failed = False

        def __getattr__(self, name: str) -> object:
            return getattr(self._selector, name)

        def register(self, *args: object, **kwargs: object) -> object:
            if fault_method == "register" and not self._failed:
                self._failed = True
                raise RuntimeError("injected supervisor failure")
            return self._selector.register(*args, **kwargs)

        def select(self, timeout: float | None = None) -> object:
            if fault_method == "select" and not self._failed:
                self._failed = True
                raise RuntimeError("injected supervisor failure")
            return self._selector.select(timeout)

    monkeypatch.setattr(
        VERIFIER_MODULE.selectors, "DefaultSelector", FaultingSelector
    )
    if fault_method == "read":
        real_read = VERIFIER_MODULE.os.read
        real_popen = VERIFIER_MODULE.subprocess.Popen
        read_armed = False
        read_failed = False

        def faulting_read(descriptor: int, size: int) -> bytes:
            nonlocal read_failed
            if read_armed and not read_failed:
                read_failed = True
                raise RuntimeError("injected supervisor failure")
            return real_read(descriptor, size)

        def arming_popen(*args: object, **kwargs: object) -> object:
            nonlocal read_armed
            process = real_popen(*args, **kwargs)
            read_armed = True
            return process

        monkeypatch.setattr(VERIFIER_MODULE.os, "read", faulting_read)
        monkeypatch.setattr(
            VERIFIER_MODULE.subprocess, "Popen", arming_popen
        )
    if fault_method == "wait":
        real_popen = VERIFIER_MODULE.subprocess.Popen

        class FaultingProcess:
            def __init__(self, *args: object, **kwargs: object) -> None:
                self._process = real_popen(*args, **kwargs)
                self._failed = False

            def __getattr__(self, name: str) -> object:
                return getattr(self._process, name)

            def wait(self) -> int:
                if not self._failed:
                    self._failed = True
                    raise RuntimeError("injected supervisor failure")
                return self._process.wait()

        monkeypatch.setattr(
            VERIFIER_MODULE.subprocess, "Popen", FaultingProcess
        )
    sentinel = tmp_path / "supervisor-exception-natural-completion"
    child = (
        "import sys; "
        "sys.stdout.buffer.write(b'O' * 131072); sys.stdout.flush(); "
        "sys.stderr.buffer.write(b'E' * 131072); sys.stderr.flush(); "
        "from pathlib import Path; "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(RuntimeError, match="injected supervisor failure"):
        VERIFIER_MODULE._run_bounded(
            Path(sys.executable).resolve(strict=True),
            ["-I", "-S", "-c", child],
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            maximum_output_bytes=1024 * 1024,
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


def canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode("utf-8")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def commit_id(raw_commit: bytes, *, object_format: str = "sha1") -> str:
    framed = b"commit " + str(len(raw_commit)).encode("ascii") + b"\0" + raw_commit
    if object_format == "sha1":
        return hashlib.sha1(framed, usedforsecurity=False).hexdigest()
    if object_format == "sha256":
        return hashlib.sha256(framed).hexdigest()
    raise AssertionError(object_format)


def trailer_lines(manifest: str, lock_digest: str) -> list[str]:
    return [
        "Sumeragi-V2-Release-Identity-Version: 1",
        f"Sumeragi-V2-Source-Manifest-SHA256: {manifest}",
        f"Sumeragi-V2-Cargo-Lock-SHA256: {lock_digest}",
    ]


def signed_commit(tree: str, message: bytes, *, armor: str = "ssh") -> bytes:
    if armor == "ssh":
        begin = "-----BEGIN SSH SIGNATURE-----"
        end = "-----END SSH SIGNATURE-----"
    elif armor == "pgp":
        begin = "-----BEGIN PGP SIGNATURE-----"
        end = "-----END PGP SIGNATURE-----"
    elif armor == "x509":
        begin = "-----BEGIN CERTIFICATE-----"
        end = "-----END CERTIFICATE-----"
    else:
        raise AssertionError(armor)
    signature = base64.b64encode(b"closed fake signature").decode("ascii")
    headers = (
        f"tree {tree}\n"
        "author Release Test <release@example.invalid> 1 +0000\n"
        "committer Release Test <release@example.invalid> 1 +0000\n"
        f"gpgsig {begin}\n"
        f" {signature}\n"
        f" {end}\n"
    ).encode("ascii")
    return headers + b"\n" + message


_FAKE_GIT_TEMPLATE = r'''#!__PYTHON__ -B
import base64
import json
import os
from pathlib import Path
import sys

state_path = Path(__STATE_PATH__)
log_path = Path(__LOG_PATH__)
state = json.loads(state_path.read_text(encoding="utf-8"))

raw_args = sys.argv[1:]
args = list(raw_args)
overrides = []
while len(args) >= 2 and args[0] == "-c":
    overrides.append(args[1])
    args = args[2:]
with log_path.open("a", encoding="utf-8") as log:
    log.write(json.dumps({
        "argv": raw_args,
        "command": args,
        "environment": dict(os.environ),
        "overrides": overrides,
    }, sort_keys=True, separators=(",", ":")) + "\n")

def persist():
    state_path.write_text(
        json.dumps(state, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )

def mutate_if_requested():
    if state.get("mutation_done"):
        return
    mutation = state.get("mutate_on_verify")
    if not mutation:
        return
    path = Path(mutation["path"])
    path.write_bytes(base64.b64decode(mutation["data_base64"]))
    if "mode" in mutation:
        path.chmod(mutation["mode"])
    state["mutation_done"] = True
    persist()

if args == ["rev-parse", "--show-toplevel"]:
    sys.stdout.write(state["top_level"] + "\n")
elif args == ["rev-parse", "--verify", "HEAD^{commit}"]:
    index = state.get("head_query_count", 0)
    sequence = state.get("head_sequence", [state["head"]])
    sys.stdout.write(sequence[min(index, len(sequence) - 1)] + "\n")
    state["head_query_count"] = index + 1
    persist()
elif len(args) == 3 and args[:2] == ["rev-parse", "--verify"] and args[2].endswith("^{tree}"):
    oid = args[2][:-7]
    if oid == "HEAD":
        tree = state.get("symbolic_tree", state["tree"])
    else:
        tree = state.get("trees", {}).get(oid, state["tree"])
    sys.stdout.write(tree + "\n")
elif len(args) == 3 and args[:2] == ["cat-file", "commit"]:
    oid = args[2]
    if oid != state["head"]:
        raise SystemExit(91)
    sys.stdout.buffer.write(base64.b64decode(state["raw_commit_base64"]))
elif len(args) == 3 and args[:2] == ["verify-commit", "--raw"]:
    mutate_if_requested()
    oid = args[2]
    effective = state.get("symbolic_signature_oid", state["head"]) if oid == "HEAD" else oid
    signature = state["signatures"].get(effective)
    if signature is None:
        raise SystemExit(92)
    sys.stdout.buffer.write(base64.b64decode(signature.get("verify_stdout_base64", "")))
    sys.stderr.buffer.write(base64.b64decode(signature.get("verify_stderr_base64", "")))
    raise SystemExit(signature.get("verify_status", 0))
elif len(args) == 4 and args[:3] == [
    "show",
    "--no-patch",
    "--format=%G?%x00%GF%x00%GP%x00%GS%x00",
]:
    oid = args[3]
    effective = state.get("symbolic_signature_oid", state["head"]) if oid == "HEAD" else oid
    signature = state["signatures"].get(effective)
    if signature is None:
        raise SystemExit(93)
    fields = [
        signature.get("signature_status", "G"),
        signature.get("fingerprint", ""),
        signature.get("primary_fingerprint", ""),
        signature.get("signer", ""),
    ]
    sys.stdout.buffer.write("\0".join(fields).encode("utf-8") + b"\0\n")
else:
    sys.stderr.write("unsupported fake Git invocation: " + repr(args) + "\n")
    raise SystemExit(97)
'''


def write_fake_git(path: Path, state_path: Path, log_path: Path) -> None:
    script = textwrap.dedent(_FAKE_GIT_TEMPLATE)
    script = script.replace("__PYTHON__", str(Path(sys.executable).resolve()))
    script = script.replace("__STATE_PATH__", repr(str(state_path)))
    script = script.replace("__LOG_PATH__", repr(str(log_path)))
    path.write_text(script, encoding="utf-8")
    path.chmod(0o755)


def write_fake_ssh_keygen(path: Path) -> None:
    path.write_text(
        f"#!{Path(sys.executable).resolve()} -B\nraise SystemExit(1)\n",
        encoding="utf-8",
    )
    path.chmod(0o755)


def write_identity(path: Path, identity: dict[str, Any]) -> None:
    path.write_bytes(canonical_json(identity))


def default_signature() -> dict[str, Any]:
    return {
        "verify_status": 0,
        "verify_stdout_base64": base64.b64encode(b"verify stdout\n").decode(),
        "verify_stderr_base64": base64.b64encode(b"verify stderr\n").decode(),
        "signature_status": "G",
        "fingerprint": FINGERPRINT,
        "primary_fingerprint": PRIMARY_FINGERPRINT,
        "signer": PRINCIPAL,
    }


def make_case(
    tmp_path: Path,
    *,
    message_factory: Callable[[str, str], bytes] | None = None,
    armor: str = "ssh",
    object_format: str = "sha1",
) -> dict[str, Any]:
    root = tmp_path / "source"
    root.mkdir()
    lock_bytes = b"version = 4\n\n[[package]]\nname = \"release-fixture\"\n"
    (root / "Cargo.lock").write_bytes(lock_bytes)
    lock_digest = sha256(lock_bytes)
    manifest = "2" * 64
    width = 40 if object_format == "sha1" else 64
    tree = "1" * width
    if message_factory is None:
        message = (
            "Sumeragi v2 production release\n\n"
            + "\n".join(trailer_lines(manifest, lock_digest))
            + "\n"
        ).encode()
    else:
        message = message_factory(manifest, lock_digest)
    raw_commit = signed_commit(tree, message, armor=armor)
    head = commit_id(raw_commit, object_format=object_format)
    identity = {
        "schema_version": 1,
        "head_commit": head,
        "head_tree": tree,
        "index_tree": tree,
        "workspace_source_manifest_sha256": manifest,
        "cargo_lock_sha256": lock_digest,
    }
    identity_path = tmp_path / "candidate-identity.json"
    write_identity(identity_path, identity)
    state_path = tmp_path / "fake-git-state.json"
    call_log = tmp_path / "fake-git-calls.jsonl"
    git_bin = tmp_path / "pinned-git"
    ssh_keygen = tmp_path / "pinned-ssh-keygen"
    allowed_signers = tmp_path / "allowed_signers"
    revocation = tmp_path / "revocation"
    allowed_bytes = f"{PRINCIPAL} {PUBLIC_KEY}\n".encode()
    allowed_signers.write_bytes(allowed_bytes)
    revocation_bytes = b""
    revocation.write_bytes(revocation_bytes)
    state = {
        "head": head,
        "tree": tree,
        "top_level": str(root.resolve()),
        "raw_commit_base64": base64.b64encode(raw_commit).decode(),
        "signatures": {head: default_signature()},
    }
    state_path.write_bytes(canonical_json(state))
    write_fake_git(git_bin, state_path, call_log)
    write_fake_ssh_keygen(ssh_keygen)
    evidence = tmp_path / "evidence"
    evidence.mkdir(mode=0o700)
    evidence.chmod(0o700)
    outputs = {
        "attestation": evidence / "signature-attestation.json",
        "private_provenance": evidence / "bootstrap-private-provenance.json",
        "transcript": evidence / "verify-transcript.json",
        "raw_commit": evidence / "commit.raw",
        "cargo_lock": evidence / "Cargo.lock",
        "allowed_signers": evidence / "allowed_signers.archive",
        "revocation": evidence / "revocation.archive",
        "git": evidence / "git.release-tool",
        "ssh_keygen": evidence / "ssh-keygen.release-tool",
    }
    return {
        "root": root,
        "identity": identity,
        "identity_path": identity_path,
        "git_bin": git_bin,
        "ssh_keygen": ssh_keygen,
        "allowed_signers": allowed_signers,
        "revocation": revocation,
        "allowed_bytes": allowed_bytes,
        "revocation_bytes": revocation_bytes,
        "state": state,
        "state_path": state_path,
        "call_log": call_log,
        "outputs": outputs,
        "evidence": evidence,
        "raw_commit": raw_commit,
        "lock_bytes": lock_bytes,
        "fingerprint": FINGERPRINT,
    }


def save_state(case: dict[str, Any]) -> None:
    case["state_path"].write_bytes(canonical_json(case["state"]))


def command_for(
    case: dict[str, Any],
    *,
    root: Path | None = None,
    identity: Path | None = None,
    git_bin: Path | None = None,
    ssh_keygen: Path | None = None,
    allowed_signers: Path | None = None,
    revocation: Path | None = None,
    expected_fingerprint: str | None = None,
    digest_overrides: dict[str, str] | None = None,
    output_overrides: dict[str, Path] | None = None,
) -> list[str]:
    git_path = git_bin or case["git_bin"]
    ssh_path = ssh_keygen or case["ssh_keygen"]
    allowed_path = allowed_signers or case["allowed_signers"]
    revocation_path = revocation or case["revocation"]
    digests = {
        "git": sha256(git_path.read_bytes()),
        "ssh": sha256(ssh_path.read_bytes()),
        "allowed": sha256(allowed_path.read_bytes()),
        "revocation": sha256(revocation_path.read_bytes()),
    }
    if digest_overrides:
        digests.update(digest_overrides)
    outputs = dict(case["outputs"])
    if output_overrides:
        outputs.update(output_overrides)
    return [
        sys.executable,
        str(VERIFIER),
        "--root",
        str(root or case["root"]),
        "--identity",
        str(identity or case["identity_path"]),
        "--git-bin",
        str(git_path),
        "--expected-git-sha256",
        digests["git"],
        "--ssh-keygen-bin",
        str(ssh_path),
        "--expected-ssh-keygen-sha256",
        digests["ssh"],
        "--expected-signer-fingerprint",
        expected_fingerprint or case["fingerprint"],
        "--ssh-allowed-signers",
        str(allowed_path),
        "--expected-ssh-allowed-signers-sha256",
        digests["allowed"],
        "--ssh-revocation-file",
        str(revocation_path),
        "--expected-ssh-revocation-sha256",
        digests["revocation"],
        "--attestation-output",
        str(outputs["attestation"]),
        "--bootstrap-private-provenance-output",
        str(outputs["private_provenance"]),
        "--verify-transcript-output",
        str(outputs["transcript"]),
        "--raw-commit-output",
        str(outputs["raw_commit"]),
        "--cargo-lock-output",
        str(outputs["cargo_lock"]),
        "--ssh-allowed-signers-output",
        str(outputs["allowed_signers"]),
        "--ssh-revocation-output",
        str(outputs["revocation"]),
        "--git-archive-output",
        str(outputs["git"]),
        "--ssh-keygen-archive-output",
        str(outputs["ssh_keygen"]),
    ]


def run_case(
    case: dict[str, Any],
    *,
    extra_environment: dict[str, str] | None = None,
    **kwargs: Any,
) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    if extra_environment:
        environment.update(extra_environment)
    return subprocess.run(
        command_for(case, **kwargs),
        cwd=REPOSITORY_ROOT,
        env=environment,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def assert_no_outputs(case: dict[str, Any]) -> None:
    for name, path in case["outputs"].items():
        assert not os.path.lexists(path), name
    assert list(case["evidence"].iterdir()) == []


def calls(case: dict[str, Any]) -> list[dict[str, Any]]:
    return [json.loads(line) for line in case["call_log"].read_text().splitlines()]


def parse_args(case: dict[str, Any], **kwargs: Any) -> Any:
    return VERIFIER_MODULE._parser().parse_args(command_for(case, **kwargs)[2:])


def test_accepts_closed_fake_ssh_signature_and_binds_every_artifact(
    tmp_path: Path,
) -> None:
    case = make_case(tmp_path)

    result = run_case(case)

    assert result.returncode == 0, result.stderr
    expected_bytes = {
        "raw_commit": case["raw_commit"],
        "cargo_lock": case["lock_bytes"],
        "allowed_signers": case["allowed_bytes"],
        "revocation": case["revocation_bytes"],
        "git": case["git_bin"].read_bytes(),
        "ssh_keygen": case["ssh_keygen"].read_bytes(),
    }
    for name, data in expected_bytes.items():
        assert case["outputs"][name].read_bytes() == data
    for name, path in case["outputs"].items():
        expected_mode = TOOL_MODE if name in {"git", "ssh_keygen"} else DATA_MODE
        assert stat.S_IMODE(path.stat().st_mode) == expected_mode, name

    expected_archives = {
        "cargo_lock": case["outputs"]["cargo_lock"].name,
        "git": case["outputs"]["git"].name,
        "raw_commit": case["outputs"]["raw_commit"].name,
        "ssh_allowed_signers": case["outputs"]["allowed_signers"].name,
        "ssh_keygen": case["outputs"]["ssh_keygen"].name,
        "ssh_revocation": case["outputs"]["revocation"].name,
        "verify_transcript": case["outputs"]["transcript"].name,
    }
    expected_archive_ids = {
        "cargo_lock": "release-identity.cargo-lock.v1",
        "git": "release-identity.git.v1",
        "raw_commit": "release-identity.raw-commit.v1",
        "ssh_allowed_signers": "release-identity.ssh-allowed-signers.v1",
        "ssh_keygen": "release-identity.ssh-keygen.v1",
        "ssh_revocation": "release-identity.ssh-revocation.v1",
        "verify_transcript": "release-identity.verify-transcript.v1",
    }

    private_bytes = case["outputs"]["private_provenance"].read_bytes()
    private = json.loads(private_bytes)
    assert private_bytes == canonical_json(private)
    assert private["format"].endswith("bootstrap-private-provenance")
    assert private["schema_version"] == 1
    assert private["candidate"]["root_path"] == str(case["root"].resolve())
    assert private["candidate"]["identity_source_path"] == str(
        case["identity_path"].resolve()
    )
    assert private["archive_names"] == expected_archives
    assert private["execution"]["environment"]["HOME"] == str(case["evidence"])
    assert "LD_PRELOAD" not in private["execution"]["environment"]
    for command in private["execution"]["commands"].values():
        assert command["argv"][-1] == case["identity"]["head_commit"]
        assert command["argv"][-1] != "HEAD"
        assert command["replay_argv"][-1] == case["identity"]["head_commit"]
        assert all(".stage." not in value for value in command["replay_argv"])
        assert any("${EVIDENCE_DIRECTORY}" in value for value in command["replay_argv"])
        assert base64.b64decode(command["stdout_base64"])
        assert command["stdout_sha256"] == sha256(
            base64.b64decode(command["stdout_base64"])
        )
    assert any(
        value == "gpg.format=ssh"
        for value in private["execution"]["policy_overrides"]
    )
    assert any(
        value == "gpg.minTrustLevel=fully"
        for value in private["execution"]["policy_overrides"]
    )
    assert all(
        ".stage." not in value
        for value in private["execution"]["replay"]["policy_overrides"]
    )
    assert (
        private["execution"]["replay"]["environment"]["HOME"]
        == "$" + "{EVIDENCE_DIRECTORY}"
    )
    assert (
        private["execution"]["replay"]["environment"]["XDG_CONFIG_HOME"]
        == "$" + "{EVIDENCE_DIRECTORY}"
    )
    for policy_name in ("ssh_allowed_signers", "ssh_revocation"):
        policy = private["policies"][policy_name]
        assert policy["protected_sha256"] == policy["observed_sha256"]
        assert policy["archive_id"] == expected_archive_ids[policy_name]

    transcript_bytes = case["outputs"]["transcript"].read_bytes()
    transcript = json.loads(transcript_bytes)
    assert transcript_bytes == canonical_json(transcript)
    assert transcript == {
        "archive_ids": expected_archive_ids,
        "candidate_commit_oid": case["identity"]["head_commit"],
        "format": "iroha-sumeragi-v2-release-identity-transcript",
        "operations": transcript["operations"],
        "schema_version": 3,
    }
    expected_operations = {
        "show_signature_metadata": ("git.show-signature-metadata.ssh.v1", 0),
        "verify_commit": ("git.verify-commit.ssh.v1", 0),
        "ssh_keygen_usage": ("ssh-keygen.usage-probe.v1", 1),
    }
    for name, (operation_id, status) in expected_operations.items():
        operation = transcript["operations"][name]
        assert operation["operation_id"] == operation_id
        assert operation["exit_status"] == status
        for stream in ("stdout", "stderr"):
            assert len(operation[f"{stream}_sha256"]) == 64
            assert operation[f"{stream}_size_bytes"] >= 0

    attestation_bytes = case["outputs"]["attestation"].read_bytes()
    attestation = json.loads(attestation_bytes)
    assert attestation_bytes == canonical_json(attestation)
    assert attestation["format"] == "iroha-sumeragi-v2-release-identity-attestation"
    assert attestation["schema_version"] == 3
    assert attestation["candidate"] == {
        "cargo_lock_sha256": case["identity"]["cargo_lock_sha256"],
        "commit_oid": case["identity"]["head_commit"],
        "release_identity_sha256": sha256(case["identity_path"].read_bytes()),
        "source_manifest_sha256": case["identity"][
            "workspace_source_manifest_sha256"
        ],
        "tree_oid": case["identity"]["head_tree"],
    }
    expected_evidence = {
        "cargo_lock": case["lock_bytes"],
        "git": case["git_bin"].read_bytes(),
        "raw_commit": case["raw_commit"],
        "ssh_allowed_signers": case["allowed_bytes"],
        "ssh_keygen": case["ssh_keygen"].read_bytes(),
        "ssh_revocation": case["revocation_bytes"],
        "verify_transcript": transcript_bytes,
    }
    assert set(attestation["archives"]) == set(expected_evidence)
    for name, data in expected_evidence.items():
        artifact = attestation["archives"][name]
        assert artifact["sha256"] == sha256(data), name
        assert artifact["size_bytes"] == len(data), name
        assert artifact["archive_id"] == expected_archive_ids[name]
        expected_mode = "0500" if name in {"git", "ssh_keygen"} else "0400"
        assert artifact["mode"] == expected_mode, name

    forbidden = (
        str(case["root"].resolve()),
        str(case["evidence"].resolve()),
        str(case["git_bin"].resolve()),
        str(case["ssh_keygen"].resolve()),
        str(case["allowed_signers"].resolve()),
        str(case["revocation"].resolve()),
        "CARGO_HOME",
        "RUSTUP_HOME",
        "scaling",
        "source_path",
        "archive_name",
        "argv",
        PRINCIPAL,
        FINGERPRINT,
    )
    for artifact_bytes in (attestation_bytes, transcript_bytes):
        rendered = artifact_bytes.decode("utf-8")
        for secret in forbidden:
            assert secret not in rendered


@pytest.mark.parametrize("status", ["N", "U", "B", "E", "X", "Y", "R"])
def test_rejects_every_non_trusted_signature_status(tmp_path: Path, status: str) -> None:
    case = make_case(tmp_path)
    case["state"]["signatures"][case["identity"]["head_commit"]][
        "signature_status"
    ] = status
    save_state(case)

    result = run_case(case)

    assert result.returncode == 2
    assert "trusted SSH signature status" in result.stderr
    assert_no_outputs(case)


def test_rejects_wrong_protected_fingerprint(tmp_path: Path) -> None:
    case = make_case(tmp_path)

    result = run_case(case, expected_fingerprint="SHA256:" + "B" * 43)

    assert result.returncode == 2
    assert "protected policy" in result.stderr
    assert_no_outputs(case)


def test_rejects_nonempty_ssh_primary_fingerprint_metadata(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    case["state"]["signatures"][case["identity"]["head_commit"]][
        "primary_fingerprint"
    ] = FINGERPRINT
    save_state(case)

    result = run_case(case)

    assert result.returncode == 2
    assert "primary-key fingerprint" in result.stderr
    assert_no_outputs(case)


@pytest.mark.parametrize(
    "policy_line",
    [
        (
            f"{PRINCIPAL} cert-authority ssh-ed25519 "
            "AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n"
        ),
        (
            f"{PRINCIPAL} ssh-ed25519-cert-v01@openssh.com "
            "AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n"
        ),
    ],
)
def test_rejects_ssh_ca_and_certificate_policy(
    tmp_path: Path, policy_line: str
) -> None:
    case = make_case(tmp_path)
    case["allowed_signers"].write_text(policy_line, encoding="utf-8")

    result = run_case(case)

    assert result.returncode == 2
    assert "certificate-authority and certificate keys" in result.stderr
    assert_no_outputs(case)


@pytest.mark.parametrize(
    "option",
    [
        'valid-after="20260717Z"',
        'valid-before="20270717Z"',
    ],
)
def test_rejects_time_bounded_allowed_signers_policy(
    tmp_path: Path, option: str
) -> None:
    case = make_case(tmp_path)
    case["allowed_signers"].write_text(
        f"{PRINCIPAL} {option} {PUBLIC_KEY}\n",
        encoding="utf-8",
    )

    result = run_case(case)

    assert result.returncode == 2
    assert "time-bounded SSH allowed-signers policies" in result.stderr
    assert_no_outputs(case)


def test_rejects_multiple_active_allowed_signers_entries(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    case["allowed_signers"].write_text(
        f"{PRINCIPAL} {PUBLIC_KEY}\nbackup {PUBLIC_KEY}\n",
        encoding="utf-8",
    )

    result = run_case(case)

    assert result.returncode == 2
    assert "exactly one active key" in result.stderr
    assert_no_outputs(case)


@pytest.mark.parametrize("armor", ["pgp", "x509"])
def test_rejects_non_ssh_signature_armor(tmp_path: Path, armor: str) -> None:
    case = make_case(tmp_path, armor=armor)

    result = run_case(case)

    assert result.returncode == 2
    assert "PGP and X509" in result.stderr
    assert_no_outputs(case)


@pytest.mark.parametrize(
    "mutator",
    [
        lambda value: {**value, "unexpected": 1},
        lambda value: {key: item for key, item in value.items() if key != "head_tree"},
        lambda value: {**value, "schema_version": True},
        lambda value: {**value, "head_commit": value["head_commit"].upper()},
        lambda value: {**value, "cargo_lock_sha256": "g" * 64},
        lambda value: {**value, "index_tree": "3" * len(value["index_tree"])},
    ],
)
def test_rejects_malformed_identity_schema(
    tmp_path: Path, mutator: Callable[[dict[str, Any]], dict[str, Any]]
) -> None:
    case = make_case(tmp_path)
    write_identity(case["identity_path"], mutator(case["identity"]))

    result = run_case(case)

    assert result.returncode == 2
    assert "release identity" in result.stderr
    assert_no_outputs(case)


def test_rejects_noncanonical_identity_json(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    case["identity_path"].write_text(
        json.dumps(case["identity"], indent=2) + "\n", encoding="utf-8"
    )

    result = run_case(case)

    assert result.returncode == 2
    assert "not canonical" in result.stderr
    assert_no_outputs(case)


def _trailer_attack(kind: str) -> Callable[[str, str], bytes]:
    def message(manifest: str, lock_digest: str) -> bytes:
        trailers = trailer_lines(manifest, lock_digest)
        if kind == "duplicate":
            text = "release\n\n" + trailers[0] + "\n\n" + "\n".join(trailers) + "\n"
        elif kind == "mis-cased":
            trailers[0] = trailers[0].replace("Sumeragi", "sumeragi")
            text = "release\n\n" + "\n".join(trailers) + "\n"
        elif kind == "nonterminal":
            text = "release\n\n" + "\n".join(trailers) + "\nafter\n"
        elif kind == "reordered":
            text = "release\n\n" + "\n".join(reversed(trailers)) + "\n"
        elif kind == "wrong-version":
            trailers[0] = trailers[0][:-1] + "2"
            text = "release\n\n" + "\n".join(trailers) + "\n"
        elif kind == "stale-manifest":
            trailers[1] = trailers[1][:-64] + "4" * 64
            text = "release\n\n" + "\n".join(trailers) + "\n"
        elif kind == "stale-lock":
            trailers[2] = trailers[2][:-64] + "5" * 64
            text = "release\n\n" + "\n".join(trailers) + "\n"
        elif kind == "missing":
            text = "release without trailers\n"
        else:
            raise AssertionError(kind)
        return text.encode()

    return message


@pytest.mark.parametrize(
    "kind",
    [
        "duplicate",
        "mis-cased",
        "nonterminal",
        "reordered",
        "wrong-version",
        "stale-manifest",
        "stale-lock",
        "missing",
    ],
)
def test_rejects_release_trailer_attacks(tmp_path: Path, kind: str) -> None:
    case = make_case(tmp_path, message_factory=_trailer_attack(kind))

    result = run_case(case)

    assert result.returncode == 2
    assert "terminal Sumeragi v2 release trailer block" in result.stderr
    assert_no_outputs(case)


@pytest.mark.parametrize("name", ["git", "ssh", "allowed", "revocation"])
def test_rejects_wrong_protected_input_hash(tmp_path: Path, name: str) -> None:
    case = make_case(tmp_path)

    result = run_case(case, digest_overrides={name: "f" * 64})

    assert result.returncode == 2
    assert "protected SHA-256" in result.stderr
    assert_no_outputs(case)


def test_rejects_stable_ssh_keygen_copy_that_cannot_execute(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    case["ssh_keygen"].write_text(
        f"#!{Path(sys.executable).resolve()} -B\n"
        "raise SystemExit(79)\n",
        encoding="utf-8",
    )
    case["ssh_keygen"].chmod(0o755)

    result = run_case(case)

    assert result.returncode == 2
    assert "stable private ssh-keygen copy could not execute" in result.stderr
    assert_no_outputs(case)


@pytest.mark.parametrize(
    ("name", "path_key", "replacement"),
    [
        ("identity", "identity_path", b"{}\n"),
        ("Cargo.lock", "lock", b"version = 4\n# changed\n"),
        ("Git", "git_bin", b"#!/bin/sh\nexit 1\n"),
        ("ssh-keygen", "ssh_keygen", b"#!/bin/sh\nexit 1\n"),
        ("allowed-signers", "allowed_signers", b"changed\n"),
        ("revocation-policy", "revocation", b"revoked\n"),
    ],
)
def test_rejects_persistent_input_mutation_during_verification(
    tmp_path: Path, name: str, path_key: str, replacement: bytes
) -> None:
    case = make_case(tmp_path)
    mutation_path = (
        case["root"] / "Cargo.lock" if path_key == "lock" else case[path_key]
    )
    case["state"]["mutate_on_verify"] = {
        "path": str(mutation_path),
        "data_base64": base64.b64encode(replacement).decode(),
    }
    save_state(case)

    result = run_case(case)

    assert result.returncode == 2
    assert "changed during verification" in result.stderr, (name, result.stderr)
    assert_no_outputs(case)


def test_rejects_symbolic_head_a_b_a_signature_substitution(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    candidate = case["identity"]["head_commit"]
    alternate = "b" * len(candidate)
    case["state"]["head_sequence"] = [candidate, candidate]
    case["state"]["symbolic_signature_oid"] = alternate
    case["state"]["signatures"][candidate]["verify_status"] = 1
    case["state"]["signatures"][alternate] = default_signature()
    save_state(case)

    result = run_case(case)

    assert result.returncode == 2
    assert "cryptographic verification" in result.stderr
    verify_call = next(
        call for call in calls(case) if call["command"][:2] == ["verify-commit", "--raw"]
    )
    assert verify_call["command"][-1] == candidate
    assert verify_call["command"][-1] != "HEAD"
    assert_no_outputs(case)


def test_head_tree_fences_derive_tree_from_the_resolved_immutable_oid(
    tmp_path: Path,
) -> None:
    case = make_case(tmp_path)
    candidate = case["identity"]["head_commit"]
    case["state"]["head_sequence"] = [candidate, candidate]
    case["state"]["symbolic_tree"] = "b" * len(case["identity"]["head_tree"])
    case["state"]["trees"] = {candidate: case["identity"]["head_tree"]}
    save_state(case)

    result = run_case(case)

    assert result.returncode == 0, result.stderr
    tree_queries = [
        call["command"][2]
        for call in calls(case)
        if len(call["command"]) == 3
        and call["command"][:2] == ["rev-parse", "--verify"]
        and call["command"][2].endswith("^{tree}")
    ]
    assert tree_queries == [f"{candidate}^{{tree}}", f"{candidate}^{{tree}}"]
    assert "HEAD^{tree}" not in tree_queries


def test_rejects_wrong_git_top_level(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    case["state"]["top_level"] = str(tmp_path.resolve())
    save_state(case)

    result = run_case(case)

    assert result.returncode == 2
    assert "exact Git top-level" in result.stderr
    assert_no_outputs(case)


def test_accepts_sha256_git_object_framing(tmp_path: Path) -> None:
    case = make_case(tmp_path, object_format="sha256")

    result = run_case(case)

    assert result.returncode == 0, result.stderr
    assert len(case["identity"]["head_commit"]) == 64


@pytest.mark.parametrize(
    "which",
    ["identity", "Cargo.lock", "git", "ssh_keygen", "allowed_signers", "revocation"],
)
def test_rejects_symlinked_required_inputs(tmp_path: Path, which: str) -> None:
    case = make_case(tmp_path)
    kwargs: dict[str, Any] = {}
    if which == "Cargo.lock":
        source = case["root"] / "Cargo.lock"
        target = tmp_path / "lock-target"
        target.write_bytes(source.read_bytes())
        source.unlink()
        source.symlink_to(target)
    else:
        key = {
            "identity": "identity_path",
            "git": "git_bin",
        }.get(which, which)
        target = case[key]
        alias = tmp_path / f"{which}-alias"
        alias.symlink_to(target)
        argument = {
            "identity": "identity",
            "git": "git_bin",
        }.get(which, which)
        kwargs[argument] = alias

    result = run_case(case, **kwargs)

    assert result.returncode == 2
    assert_no_outputs(case)


def test_scrubs_loader_git_gnupg_and_agent_environment(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    poisoned = {
        "LD_AUDIT_SENTINEL": "poison",
        "DYLD_TEST_SENTINEL": "poison",
        "GNUPGHOME": "/poison",
        "SSH_AUTH_SOCK": "/poison",
        "GIT_CONFIG_COUNT": "1",
        "GIT_CONFIG_KEY_0": "gpg.ssh.program",
        "GIT_CONFIG_VALUE_0": "/poison",
    }

    result = run_case(case, extra_environment=poisoned)

    assert result.returncode == 0, result.stderr
    admitted = calls(case)[0]["environment"]
    expected_environment = {
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_NOSYSTEM",
        "GIT_CONFIG_SYSTEM",
        "GIT_NO_REPLACE_OBJECTS",
        "GIT_OPTIONAL_LOCKS",
        "GIT_TERMINAL_PROMPT",
        "HOME",
        "LANG",
        "LANGUAGE",
        "LC_ALL",
        "PATH",
        "TZ",
        "XDG_CONFIG_HOME",
    }
    if sys.platform == "darwin":
        expected_environment.add("__CF_USER_TEXT_ENCODING")
        assert admitted["__CF_USER_TEXT_ENCODING"] == f"0x{os.geteuid():X}:0x1:0xE"
    assert set(admitted) == expected_environment
    assert not set(poisoned) & set(admitted)


def test_bounds_combined_verify_output_without_partial_evidence(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    signature = case["state"]["signatures"][case["identity"]["head_commit"]]
    signature["verify_stdout_base64"] = base64.b64encode(
        b"x" * (4 * 1024 * 1024 + 1)
    ).decode()
    signature["verify_stderr_base64"] = ""
    save_state(case)

    result = run_case(case)

    assert result.returncode == 2
    assert "closed size limit" in result.stderr
    assert_no_outputs(case)


def test_requires_one_external_private_evidence_directory(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    outside = tmp_path / "other-evidence"
    outside.mkdir(mode=0o700)
    outside.chmod(0o700)

    split = run_case(
        case,
        output_overrides={"raw_commit": outside / "commit.raw"},
    )
    assert split.returncode == 2
    assert "share one private directory" in split.stderr
    assert_no_outputs(case)

    inside = case["root"] / "evidence"
    inside.mkdir(mode=0o700)
    inside_outputs = {name: inside / path.name for name, path in case["outputs"].items()}
    inside_result = run_case(case, output_overrides=inside_outputs)
    assert inside_result.returncode == 2
    assert "outside the source root" in inside_result.stderr
    assert list(inside.iterdir()) == []

    case["evidence"].chmod(0o755)
    public_result = run_case(case)
    assert public_result.returncode == 2
    assert "exact mode 0700" in public_result.stderr


def test_rejects_shell_unsafe_evidence_path_before_git_config_use(
    tmp_path: Path,
) -> None:
    case = make_case(tmp_path)
    unsafe = tmp_path / "unsafe evidence;$HOME"
    unsafe.mkdir(mode=0o700)
    unsafe.chmod(0o700)
    overrides = {name: unsafe / path.name for name, path in case["outputs"].items()}

    result = run_case(case, output_overrides=overrides)

    assert result.returncode == 2
    assert "shell-safe ASCII" in result.stderr
    assert list(unsafe.iterdir()) == []


def test_existing_output_rejects_without_overwrite(tmp_path: Path) -> None:
    case = make_case(tmp_path)
    case["outputs"]["raw_commit"].write_bytes(b"protected")

    result = run_case(case)

    assert result.returncode == 2
    assert "output already exists" in result.stderr
    assert case["outputs"]["raw_commit"].read_bytes() == b"protected"
    for name, path in case["outputs"].items():
        if name != "raw_commit":
            assert not os.path.lexists(path)


def test_partial_publication_rolls_back_and_never_leaves_marker(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    case = make_case(tmp_path)
    args = parse_args(case)
    original_link = VERIFIER_MODULE.os.link
    destinations: list[str] = []

    def fail_third_link(source: str, destination: str, **kwargs: Any) -> None:
        destinations.append(destination)
        if len(destinations) == 3:
            raise OSError("injected publication failure")
        original_link(source, destination, **kwargs)

    monkeypatch.setattr(VERIFIER_MODULE.os, "link", fail_third_link)

    with pytest.raises(VERIFIER_MODULE.VerificationError, match="publication failed"):
        VERIFIER_MODULE.verify(args)

    assert case["outputs"]["attestation"].name not in destinations
    assert_no_outputs(case)


def test_cleanup_unlink_failure_is_observable_and_marker_stays_absent(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    case = make_case(tmp_path)
    args = parse_args(case)
    original_link = VERIFIER_MODULE.os.link
    original_unlink = VERIFIER_MODULE.os.unlink
    destinations: list[str] = []

    def fail_third_link(source: str, destination: str, **kwargs: Any) -> None:
        destinations.append(destination)
        if len(destinations) == 3:
            raise OSError("injected link failure")
        original_link(source, destination, **kwargs)

    def fail_owned_git_unlink(name: str, **kwargs: Any) -> None:
        if name == case["outputs"]["git"].name:
            raise OSError("injected cleanup failure")
        original_unlink(name, **kwargs)

    monkeypatch.setattr(VERIFIER_MODULE.os, "link", fail_third_link)
    monkeypatch.setattr(VERIFIER_MODULE.os, "unlink", fail_owned_git_unlink)

    with pytest.raises(VERIFIER_MODULE.VerificationError, match="cleanup failed"):
        VERIFIER_MODULE.verify(args)

    assert not case["outputs"]["attestation"].exists()
    assert case["outputs"]["git"].exists()


def test_cleanup_fsync_failure_is_observable(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    case = make_case(tmp_path)
    args = parse_args(case)
    original_link = VERIFIER_MODULE.os.link
    original_fsync = VERIFIER_MODULE.os.fsync
    publication_failed = False

    def fail_first_link(source: str, destination: str, **kwargs: Any) -> None:
        nonlocal publication_failed
        publication_failed = True
        raise OSError("injected link failure")

    def fail_cleanup_fsync(descriptor: int) -> None:
        mode = VERIFIER_MODULE.os.fstat(descriptor).st_mode
        if publication_failed and stat.S_ISDIR(mode):
            raise OSError("injected cleanup fsync failure")
        original_fsync(descriptor)

    monkeypatch.setattr(VERIFIER_MODULE.os, "link", fail_first_link)
    monkeypatch.setattr(VERIFIER_MODULE.os, "fsync", fail_cleanup_fsync)

    with pytest.raises(VERIFIER_MODULE.VerificationError, match="cleanup failed"):
        VERIFIER_MODULE.verify(args)

    assert not case["outputs"]["attestation"].exists()
    assert list(case["evidence"].iterdir()) == []


def test_marker_cleanup_failure_is_fatal_and_not_silently_cleared(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    case = make_case(tmp_path)
    args = parse_args(case)
    original_revalidate = VERIFIER_MODULE._revalidate_evidence_directory
    original_unlink = VERIFIER_MODULE.os.unlink

    def fail_after_marker(directory: Any) -> None:
        original_revalidate(directory)
        if case["outputs"]["attestation"].exists():
            raise VERIFIER_MODULE.VerificationError("injected post-marker failure")

    def fail_marker_unlink(name: str, **kwargs: Any) -> None:
        if name == case["outputs"]["attestation"].name:
            raise OSError("injected marker cleanup failure")
        original_unlink(name, **kwargs)

    monkeypatch.setattr(
        VERIFIER_MODULE, "_revalidate_evidence_directory", fail_after_marker
    )
    monkeypatch.setattr(VERIFIER_MODULE.os, "unlink", fail_marker_unlink)

    with pytest.raises(VERIFIER_MODULE.VerificationError, match="cleanup failed"):
        VERIFIER_MODULE.verify(args)

    assert case["outputs"]["attestation"].exists()


def test_published_prerequisite_is_rehashed_before_marker(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    case = make_case(tmp_path)
    args = parse_args(case)
    original_publish = VERIFIER_MODULE._publish_one
    corrupted = False

    def publish_then_corrupt(directory: Any, artifact: Any) -> None:
        nonlocal corrupted
        original_publish(directory, artifact)
        if not corrupted and artifact.target.label == "Git archive":
            path = case["outputs"]["git"]
            path.chmod(0o700)
            data = bytearray(path.read_bytes())
            data[0] ^= 1
            path.write_bytes(data)
            path.chmod(TOOL_MODE)
            corrupted = True

    monkeypatch.setattr(VERIFIER_MODULE, "_publish_one", publish_then_corrupt)

    with pytest.raises(VERIFIER_MODULE.VerificationError, match="inode hash changed"):
        VERIFIER_MODULE.verify(args)

    assert_no_outputs(case)


def test_attestation_marker_is_linked_last(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    case = make_case(tmp_path)
    args = parse_args(case)
    original_link = VERIFIER_MODULE.os.link
    destinations: list[str] = []

    def record_link(source: str, destination: str, **kwargs: Any) -> None:
        destinations.append(destination)
        original_link(source, destination, **kwargs)

    monkeypatch.setattr(VERIFIER_MODULE.os, "link", record_link)

    VERIFIER_MODULE.verify(args)

    assert destinations[-1] == case["outputs"]["attestation"].name
    assert set(destinations) == {path.name for path in case["outputs"].values()}


def _run_checked(arguments: list[str], *, cwd: Path | None = None) -> str:
    result = subprocess.run(
        arguments,
        cwd=cwd,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert result.returncode == 0, (arguments, result.stdout, result.stderr)
    return result.stdout


def _real_signed_case(
    tmp_path: Path, *, require_relocatable_verifier: bool = True
) -> dict[str, Any]:
    if sys.platform == "darwin":
        resolved_git = _run_checked(["/usr/bin/xcrun", "--find", "git"]).strip()
        git = Path(resolved_git).resolve(strict=True)
    else:
        git = Path(shutil.which("git") or "").resolve(strict=True)
    system_ssh_keygen = Path(shutil.which("ssh-keygen") or "").resolve(strict=True)
    requested_verifier = os.environ.get(
        "SUMERAGI_V2_TEST_RELOCATABLE_SSH_KEYGEN_BIN"
    )
    if requested_verifier:
        ssh_keygen = Path(requested_verifier).resolve(strict=True)
    elif sys.platform == "darwin" and require_relocatable_verifier:
        pytest.skip(
            "macOS /usr/bin/ssh-keygen is not relocatable; set "
            "SUMERAGI_V2_TEST_RELOCATABLE_SSH_KEYGEN_BIN"
        )
    else:
        ssh_keygen = system_ssh_keygen
    root = tmp_path / "real-source"
    root.mkdir()
    _run_checked([str(git), "init", "-q"], cwd=root)
    key = tmp_path / "release-key"
    _run_checked(
        [
            str(system_ssh_keygen),
            "-q",
            "-t",
            "ed25519",
            "-N",
            "",
            "-C",
            PRINCIPAL,
            "-f",
            str(key),
        ]
    )
    fingerprint_output = _run_checked(
        [
            str(system_ssh_keygen),
            "-lf",
            str(key.with_suffix(".pub")),
            "-E",
            "sha256",
        ]
    )
    fingerprint = fingerprint_output.split()[1]
    lock_bytes = b"version = 4\n"
    (root / "Cargo.lock").write_bytes(lock_bytes)
    (root / "payload.txt").write_text("release payload\n", encoding="utf-8")
    manifest = "2" * 64
    message = tmp_path / "message"
    message.write_text(
        "Sumeragi v2 production release\n\n"
        + "\n".join(trailer_lines(manifest, sha256(lock_bytes)))
        + "\n",
        encoding="utf-8",
    )
    _run_checked([str(git), "add", "Cargo.lock", "payload.txt"], cwd=root)
    _run_checked(
        [
            str(git),
            "-c",
            "user.name=Release Test",
            "-c",
            f"user.email={PRINCIPAL}",
            "-c",
            "gpg.format=ssh",
            "-c",
            f"user.signingkey={key}",
            "commit",
            "-q",
            "-S",
            "-F",
            str(message),
        ],
        cwd=root,
    )
    head = _run_checked([str(git), "rev-parse", "HEAD"], cwd=root).strip()
    tree = _run_checked([str(git), "rev-parse", "HEAD^{tree}"], cwd=root).strip()
    identity = {
        "schema_version": 1,
        "head_commit": head,
        "head_tree": tree,
        "index_tree": tree,
        "workspace_source_manifest_sha256": manifest,
        "cargo_lock_sha256": sha256(lock_bytes),
    }
    identity_path = tmp_path / "real-identity.json"
    write_identity(identity_path, identity)
    public_key = key.with_suffix(".pub").read_text(encoding="utf-8").strip()
    allowed_signers = tmp_path / "real-allowed-signers"
    allowed_signers.write_text(f"{PRINCIPAL} {public_key}\n", encoding="utf-8")
    revocation = tmp_path / "real-revocation"
    revocation.write_bytes(b"")
    evidence = tmp_path / "real-evidence"
    evidence.mkdir(mode=0o700)
    evidence.chmod(0o700)
    outputs = {
        "attestation": evidence / "signature-attestation.json",
        "private_provenance": evidence / "bootstrap-private-provenance.json",
        "transcript": evidence / "verify-transcript.json",
        "raw_commit": evidence / "commit.raw",
        "cargo_lock": evidence / "Cargo.lock",
        "allowed_signers": evidence / "allowed_signers.archive",
        "revocation": evidence / "revocation.archive",
        "git": evidence / "git.release-tool",
        "ssh_keygen": evidence / "ssh-keygen.release-tool",
    }
    marker = tmp_path / "malicious-gpg-program-ran"
    malicious = tmp_path / "malicious-gpg-program"
    malicious.write_text(
        f"#!{Path(sys.executable).resolve()} -B\n"
        "from pathlib import Path\n"
        f"Path({str(marker)!r}).write_text('ran')\n"
        "raise SystemExit(0)\n",
        encoding="utf-8",
    )
    malicious.chmod(0o755)
    for key_name in (
        "gpg.ssh.program",
        "gpg.program",
        "gpg.openpgp.program",
        "gpg.x509.program",
    ):
        _run_checked([str(git), "config", key_name, str(malicious)], cwd=root)
    _run_checked([str(git), "config", "gpg.format", "openpgp"], cwd=root)
    _run_checked(
        [str(git), "config", "gpg.ssh.allowedSignersFile", str(tmp_path / "bad")],
        cwd=root,
    )
    _run_checked(
        [str(git), "config", "gpg.ssh.revocationFile", str(tmp_path / "bad")],
        cwd=root,
    )
    return {
        "root": root,
        "identity": identity,
        "identity_path": identity_path,
        "git_bin": git,
        "ssh_keygen": ssh_keygen,
        "system_ssh_keygen": system_ssh_keygen,
        "allowed_signers": allowed_signers,
        "revocation": revocation,
        "outputs": outputs,
        "evidence": evidence,
        "fingerprint": fingerprint,
        "marker": marker,
    }


def test_real_git_accepts_the_ephemeral_ssh_signature_under_closed_overrides(
    tmp_path: Path,
) -> None:
    case = _real_signed_case(tmp_path, require_relocatable_verifier=False)
    git = case["git_bin"]
    ssh_keygen = case["system_ssh_keygen"]
    oid = case["identity"]["head_commit"]
    configuration = [
        "-c",
        "gpg.format=ssh",
        "-c",
        f"gpg.ssh.program={ssh_keygen}",
        "-c",
        f"gpg.ssh.allowedSignersFile={case['allowed_signers']}",
        "-c",
        f"gpg.ssh.revocationFile={case['revocation']}",
        "-c",
        f"gpg.program={ssh_keygen}",
        "-c",
        f"gpg.openpgp.program={ssh_keygen}",
        "-c",
        f"gpg.x509.program={ssh_keygen}",
    ]

    verified = subprocess.run(
        [str(git), *configuration, "verify-commit", "--raw", oid],
        cwd=case["root"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    assert verified.returncode == 0, verified.stderr.decode(errors="replace")
    assert not case["marker"].exists()


def test_real_git_ssh_signature_and_copied_apple_tools_are_executable(
    tmp_path: Path,
) -> None:
    case = _real_signed_case(tmp_path)

    result = run_case(
        case,
        extra_environment={
            "LD_AUDIT_SENTINEL": "poison",
            "DYLD_TEST_SENTINEL": "poison",
            "GNUPGHOME": str(tmp_path / "poison-gpg"),
            "SSH_AUTH_SOCK": str(tmp_path / "poison-agent"),
        },
    )

    assert result.returncode == 0, result.stderr
    assert not case["marker"].exists()
    archived_git = case["outputs"]["git"]
    archived_ssh = case["outputs"]["ssh_keygen"]
    assert archived_git.read_bytes() == case["git_bin"].read_bytes()
    assert archived_ssh.read_bytes() == case["ssh_keygen"].read_bytes()
    assert stat.S_IMODE(archived_git.stat().st_mode) == TOOL_MODE
    assert stat.S_IMODE(archived_ssh.stat().st_mode) == TOOL_MODE
    assert "git version" in _run_checked([str(archived_git), "--version"])
    archived_fingerprint = _run_checked(
        [
            str(archived_ssh),
            "-lf",
            str(tmp_path / "release-key.pub"),
            "-E",
            "sha256",
        ]
    ).split()[1]
    assert archived_fingerprint == case["fingerprint"]
    provenance = json.loads(case["outputs"]["private_provenance"].read_bytes())
    verify_argv = provenance["execution"]["commands"]["verify_commit"]["argv"]
    assert verify_argv[-1] == case["identity"]["head_commit"]
    assert any(value == "gpg.format=ssh" for value in verify_argv)


@pytest.mark.skipif(sys.platform != "darwin", reason="Apple launcher is macOS-only")
def test_rejects_apple_usr_bin_git_launcher(tmp_path: Path) -> None:
    case = make_case(tmp_path)

    result = run_case(case, git_bin=Path("/usr/bin/git"))

    assert result.returncode == 2
    assert "Apple developer-tool launcher" in result.stderr
    assert "xcrun --find git" in result.stderr
    assert_no_outputs(case)


@pytest.mark.skipif(sys.platform != "darwin", reason="Apple platform binary is macOS-only")
def test_rejects_non_relocatable_apple_ssh_keygen(tmp_path: Path) -> None:
    case = make_case(tmp_path)

    result = run_case(case, ssh_keygen=Path("/usr/bin/ssh-keygen"))

    assert result.returncode == 2
    assert "Apple platform binary" in result.stderr
    assert "relocatable checksum-pinned ssh-keygen" in result.stderr
    assert_no_outputs(case)
