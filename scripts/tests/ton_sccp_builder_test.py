"""Adversarial unit tests for the fail-closed TON SCCP release builder."""

from __future__ import annotations

import base64
import copy
import hashlib
import io
import os
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

import sccp_release_common as common  # noqa: E402
import ton_sccp_builder as builder  # noqa: E402


def _keypair(label: str) -> tuple[bytes, bytes, int]:
    entropy = hashlib.sha256(f"ton-builder-test:{label}".encode("ascii")).digest()
    digest = hashlib.sha512(entropy).digest()
    scalar_bytes = bytearray(digest[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    scalar = int.from_bytes(scalar_bytes, "little")
    public = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, scalar))
    return public, digest[32:], scalar


def _sign(keypair: tuple[bytes, bytes, int], message: bytes) -> str:
    public, prefix, scalar = keypair
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little") % common._ED_L
    encoded_r = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, nonce))
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public + message).digest(), "little"
    ) % common._ED_L
    encoded_s = ((nonce + challenge * scalar) % common._ED_L).to_bytes(32, "little")
    signature = encoded_r + encoded_s
    assert common.verify_ed25519(public, signature, message)
    return base64.b64encode(signature).decode("ascii")


def _policy() -> tuple[dict[str, object], dict[str, tuple[bytes, bytes, int]]]:
    keys = {role: _keypair(role) for role in builder.APPROVER_ROLES}
    policy: dict[str, object] = {
        "schema": builder.POLICY_SCHEMA,
        "source": {
            "commit": "10" * 20,
            "commit_signer_fingerprint": "0123456789abcdef",
            "source_date_epoch": 1_700_000_000,
        },
        "builder": {
            "image": f"registry.example/iroha-ton-builder@sha256:{'20' * 32}",
            "platform": builder.PLATFORM,
            "driver_path": "/usr/local/bin/iroha-sccp-ton-builder-final-v1",
            "acton_archive_sha256": builder.ACTON_ARCHIVE_SHA256,
            "acton_reported_version": builder.ACTON_VERSION,
            "tolk_reported_version": builder.TOLK_VERSION,
            "host_python_sha256": "2f" * 32,
            "host_git_sha256": "30" * 32,
            "host_docker_sha256": "40" * 32,
            "host_commit_verifier_sha256": "41" * 32,
            "toolchain_inventory": [
                {
                    "path": "toolchain/acton",
                    "role": "acton-executable",
                    "sha256": "50" * 32,
                    "size_bytes": 100,
                    "executable": True,
                },
                {
                    "path": "toolchain/driver",
                    "role": "builder-driver",
                    "sha256": "60" * 32,
                    "size_bytes": 101,
                    "executable": True,
                },
                {
                    "path": "toolchain/stdlib/common.tolk",
                    "role": "tolk-stdlib",
                    "sha256": "70" * 32,
                    "size_bytes": 102,
                    "executable": False,
                },
            ],
        },
        "limits": {
            "max_artifacts": 128,
            "max_artifact_bytes": 16 * 1024 * 1024,
            "max_total_bytes": 256 * 1024 * 1024,
            "max_log_bytes": 1024 * 1024,
            "timeout_seconds": 1800,
        },
        "approvers": [
            {
                "role": role,
                "signer_id": f"ton-{role}",
                "public_key_hex": keys[role][0].hex(),
            }
            for role in builder.APPROVER_ROLES
        ],
    }
    return policy, keys


def _unsigned_lock() -> dict[str, object]:
    return {
        "schema": builder.LOCK_SCHEMA,
        "builder_policy_sha256": "80" * 32,
        "source_closure_sha256": "90" * 32,
        "source_commit": "10" * 20,
        "artifact_tree_sha256": "a0" * 32,
        "artifacts": [
            {
                "path": "build/TairaXorSccpBridge.json",
                "sha256": "b0" * 32,
                "size_bytes": 100,
                "executable": False,
            }
        ],
        "toolchain_inventory": [
            {
                "path": "toolchain/acton",
                "role": "acton-executable",
                "sha256": "50" * 32,
                "size_bytes": 100,
                "executable": True,
            }
        ],
    }


def _signed_lock(
    unsigned: dict[str, object],
    policy: dict[str, object],
    keys: dict[str, tuple[bytes, bytes, int]],
) -> dict[str, object]:
    payload = builder.output_lock_signing_payload(unsigned)
    approvers = policy["approvers"]
    assert isinstance(approvers, list)
    return {
        **copy.deepcopy(unsigned),
        "provenance": [
            {
                "role": role,
                "signer_id": approvers[index]["signer_id"],
                "algorithm": "ed25519",
                "public_key_hex": approvers[index]["public_key_hex"],
                "signature_b64": _sign(keys[role], payload),
            }
            for index, role in enumerate(builder.APPROVER_ROLES)
        ],
    }


def test_policy_closes_versions_image_host_tools_inventory_and_approvers() -> None:
    policy, _ = _policy()
    assert builder.validate_policy(policy) == policy
    for mutation in range(10):
        candidate = copy.deepcopy(policy)
        if mutation == 0:
            candidate["builder"]["image"] = "registry.example/builder:latest"
        elif mutation == 1:
            candidate["builder"]["platform"] = "linux/arm64"
        elif mutation == 2:
            candidate["builder"]["acton_archive_sha256"] = "00" * 32
        elif mutation == 3:
            candidate["builder"]["acton_reported_version"] = "acton 1.1.0"
        elif mutation == 4:
            candidate["builder"]["tolk_reported_version"] = "1.4.0"
        elif mutation == 5:
            candidate["builder"]["host_git_sha256"] = "00" * 32
        elif mutation == 6:
            candidate["builder"]["toolchain_inventory"].pop()
        elif mutation == 7:
            candidate["approvers"][1]["public_key_hex"] = candidate["approvers"][0][
                "public_key_hex"
            ]
        elif mutation == 8:
            candidate["source"]["commit"] = "1" * 39
        else:
            candidate["extra"] = True
        with pytest.raises(builder.TonBuilderError):
            builder.validate_policy(candidate)


def test_output_lock_requires_two_exact_fresh_independent_signatures() -> None:
    policy, keys = _policy()
    unsigned = _unsigned_lock()
    signed = _signed_lock(unsigned, policy, keys)
    assert builder.validate_signed_lock(
        signed,
        expected_unsigned=unsigned,
        policy=policy,
    ) == signed

    for mutation in range(6):
        candidate = copy.deepcopy(signed)
        if mutation == 0:
            candidate["artifact_tree_sha256"] = "c0" * 32
        elif mutation == 1:
            candidate["provenance"].reverse()
        elif mutation == 2:
            candidate["provenance"][0]["signature_b64"] = candidate["provenance"][1][
                "signature_b64"
            ]
        elif mutation == 3:
            candidate["provenance"][0]["algorithm"] = "ed25519ph"
        elif mutation == 4:
            candidate["provenance"][0]["signature_b64"] = "AA=="
        else:
            candidate["legacy"] = True
        with pytest.raises(builder.TonBuilderError):
            builder.validate_signed_lock(
                candidate,
                expected_unsigned=unsigned,
                policy=policy,
            )


def test_tree_scanner_hashes_regular_files_and_rejects_symlinks(tmp_path: Path) -> None:
    root = tmp_path / "artifacts"
    root.mkdir(mode=0o700)
    artifact = root / "contract.json"
    artifact.write_bytes(b'{"code":"bounded-public-bytecode"}\n')
    artifact.chmod(0o600)
    entries = builder._scan_tree(
        root,
        label="test artifact tree",
        maximum_files=4,
        maximum_file_bytes=1024,
        maximum_total_bytes=4096,
        scan_text=True,
    )
    assert entries == [
        {
            "path": "contract.json",
            "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
            "size_bytes": artifact.stat().st_size,
            "executable": False,
        }
    ]
    (root / "alias.json").symlink_to(artifact)
    with pytest.raises(builder.TonBuilderError, match="symlink"):
        builder._scan_tree(
            root,
            label="test artifact tree",
            maximum_files=4,
            maximum_file_bytes=1024,
            maximum_total_bytes=4096,
            scan_text=True,
        )


def test_candidate_publication_is_private_exclusive_and_manifest_last(tmp_path: Path) -> None:
    source = tmp_path / "source"
    artifact = source / "artifacts" / "build" / "contract.json"
    artifact.parent.mkdir(parents=True, mode=0o700)
    payload = b'{"contract":"canonical"}\n'
    artifact.write_bytes(payload)
    artifact.chmod(0o600)
    unsigned = _unsigned_lock()
    unsigned["artifacts"] = [
        {
            "path": "build/contract.json",
            "sha256": hashlib.sha256(payload).hexdigest(),
            "size_bytes": len(payload),
            "executable": False,
        }
    ]
    output = tmp_path / "candidate"
    builder._publish_candidate(output, build_output=source, unsigned_lock=unsigned)
    assert (output.stat().st_mode & 0o077) == 0
    published = output / "artifacts" / "build" / "contract.json"
    assert published.read_bytes() == payload
    assert (published.stat().st_mode & 0o077) == 0
    assert (output / "unsigned-output-lock.json").read_bytes() == common.canonical_json_file_bytes(
        unsigned
    )
    assert (output / "output-lock-signing-payload.bin").read_bytes() == (
        builder.output_lock_signing_payload(unsigned)
    )
    with pytest.raises(builder.TonBuilderError, match="never overwrites"):
        builder._publish_candidate(output, build_output=source, unsigned_lock=unsigned)


def test_release_builder_has_no_path_acton_or_single_build_production_escape() -> None:
    python_source = (SCRIPTS / "ton_sccp_builder.py").read_text(encoding="utf-8")
    wrapper = (SCRIPTS / "sccp_ton_contract_build.sh").read_text(encoding="utf-8")
    assert "ACTON_BIN" not in python_source + wrapper
    assert "--network=none" in python_source
    assert "--platform=linux/amd64" in python_source
    assert "--pull=never" in python_source
    assert "--read-only" in python_source
    assert "--cap-drop=ALL" in python_source
    assert python_source.count("_run_container_build(") >= 3
    assert "report_one != report_two" in python_source
    assert builder.APPROVER_ROLES == ("release-engineering", "release-security")
    for field in (
        "ton_builder_policy_sha256",
        "ton_source_closure_sha256",
        "ton_output_lock_sha256",
    ):
        assert field in python_source
    mode = os.stat(SCRIPTS / "ton_sccp_builder.py").st_mode
    assert mode & 0o111


def test_cli_shape_errors_do_not_echo_untrusted_arguments() -> None:
    marker = "authorization=Bearer-do-not-echo"
    result = subprocess.run(
        [sys.executable, str(SCRIPTS / "ton_sccp_builder.py"), "--unknown", marker],
        cwd=ROOT,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert result.returncode == 2
    assert marker not in result.stdout + result.stderr
    assert len(result.stderr) < 1024


def test_policy_requires_a_nonzero_commit_verifier_digest() -> None:
    policy, _ = _policy()
    for value in (None, "00" * 32, "short"):
        candidate = copy.deepcopy(policy)
        if value is None:
            del candidate["builder"]["host_commit_verifier_sha256"]
        else:
            candidate["builder"]["host_commit_verifier_sha256"] = value
        with pytest.raises(builder.TonBuilderError):
            builder.validate_policy(candidate)


def test_git_environment_ignores_ambient_configuration(monkeypatch: pytest.MonkeyPatch) -> None:
    for key in ("GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM", "GIT_CONFIG_COUNT", "GIT_DIR", "HOME"):
        monkeypatch.setenv(key, "ambient-setting")
    environment = builder._closed_environment(source_date_epoch=1_700_000_000)
    assert environment["GIT_CONFIG_NOSYSTEM"] == "1"
    assert environment["GIT_CONFIG_GLOBAL"] == os.devnull
    assert environment["GIT_NO_REPLACE_OBJECTS"] == "1"
    assert environment["GIT_NO_LAZY_FETCH"] == "1"
    assert environment["GIT_TERMINAL_PROMPT"] == "0"
    assert environment["SOURCE_DATE_EPOCH"] == "1700000000"
    assert "ambient-setting" not in environment.values()


def test_git_reads_original_objects_despite_local_replacement_refs(tmp_path: Path) -> None:
    git_path = shutil.which("git")
    if git_path is None:
        pytest.skip("Git unavailable")
    git = Path(git_path)

    def run(*arguments: str, payload: bytes | None = None) -> bytes:
        return subprocess.run(
            [str(git), "-C", str(tmp_path), *arguments],
            input=payload, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env=builder._closed_environment(),
        ).stdout.strip()

    run("init", "--quiet")
    original = run("hash-object", "-w", "--stdin", payload=b"approved source\n").decode()
    replacement = run("hash-object", "-w", "--stdin", payload=b"unapproved source\n").decode()
    run("update-ref", f"refs/replace/{original}", replacement)
    assert builder._git_command(git, tmp_path, ("cat-file", "blob", original)) == b"approved source\n"


def test_git_operations_disable_repository_command_hooks(monkeypatch: pytest.MonkeyPatch) -> None:
    calls = []

    def run(executable, arguments, **kwargs):
        calls.append((arguments, kwargs))
        return b"", b""

    monkeypatch.setattr(builder, "_run_bounded", run)
    builder._git_command(Path("/approved/git"), ROOT, ("status", "--porcelain=v1"))
    arguments, options = calls[0]
    assert "core.fsmonitor=false" in arguments
    assert f"core.hooksPath={os.devnull}" in arguments
    assert options["environment"]["GIT_NO_REPLACE_OBJECTS"] == "1"


@pytest.mark.parametrize("path", ["relative/verifier", "/approved/tool name", "/approved/../verifier"])
def test_commit_verifier_path_is_rejected_before_git(
    path: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    def unexpected_git(*args, **kwargs):
        pytest.fail("invalid verifier path reached Git")

    monkeypatch.setattr(builder, "_git_command", unexpected_git)
    policy, _ = _policy()
    with pytest.raises(builder.TonBuilderError, match="canonical shell-inert"):
        builder._verify_source_and_archive(Path("/approved/git"), Path(path), policy, tmp_path / "source.tar")


def test_both_signature_checks_bind_every_verifier_format(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    policy, _ = _policy()
    commands = []

    def git_command(git, root, arguments, **kwargs):
        commands.append(arguments)
        if "--show-toplevel" in arguments:
            return str(ROOT).encode() + b"\n"
        if "--show-object-format=storage" in arguments:
            return b"sha1\n"
        if "--git-path" in arguments:
            return str(tmp_path).encode() + b"\n"
        if "rev-parse" in arguments:
            return policy["source"]["commit"].encode() + b"\n"
        if "--format=%G?%x00%GF%x00%GP%x00" in arguments:
            fingerprint = policy["source"]["commit_signer_fingerprint"].encode()
            return b"G\x00" + fingerprint + b"\x00" + fingerprint + b"\x00\n"
        if "--format=%ct" in arguments:
            return b"0\n"  # Stop before archive creation; only signature dispatch is under test.
        return b""

    monkeypatch.setattr(builder, "_git_command", git_command)
    with pytest.raises(builder.TonBuilderError, match="commit time"):
        builder._verify_source_and_archive(
            Path("/approved/git"), Path("/approved/verifier"), policy, tmp_path / "source.tar",
        )
    checks = [args for args in commands if "verify-commit" in args or "--format=%G?%x00%GF%x00%GP%x00" in args]
    assert len(checks) == 2
    for arguments in checks:
        assert "gpg.format=openpgp" in arguments
        for slot in ("gpg.program", "gpg.openpgp.program", "gpg.x509.program", "gpg.ssh.program"):
            assert f"{slot}=/approved/verifier" in arguments


def test_production_rejects_unapproved_commit_verifier_before_build(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    policy, _ = _policy()
    policy_bytes = common.canonical_json_file_bytes(policy)
    policy_path = tmp_path / "policy.json"
    policy_path.write_bytes(policy_bytes)

    def open_executable(path, *, label):
        hashes = {
            "pinned Python executable": "2f" * 32,
            "pinned Git executable": "30" * 32,
            "pinned Docker executable": "40" * 32,
            "pinned OpenPGP commit signature verifier": "42" * 32,
        }
        return Path(path), (1, 2, 3, 4, 5), hashes[label]

    def unexpected_build(*args, **kwargs):
        pytest.fail("unapproved commit verifier reached container work")

    monkeypatch.setattr(builder, "_open_stable_executable", open_executable)
    monkeypatch.setattr(builder, "_inspect_image", unexpected_build)
    with pytest.raises(builder.TonBuilderError, match="verifier does not match"):
        builder._production_build(
            policy_path=policy_path, trusted_policy_sha256=hashlib.sha256(policy_bytes).hexdigest(),
            git_path="/approved/git", docker_path="/approved/docker",
            commit_verifier_path="/approved/verifier",
        )


def test_source_archive_uses_the_same_git_isolation(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    policy, _ = _policy()
    archive_bytes = b"bounded archive stream\n"
    invocations = []

    def git_command(git, root, arguments, **kwargs):
        if "--show-toplevel" in arguments:
            return str(ROOT).encode() + b"\n"
        if "--show-object-format=storage" in arguments:
            return b"sha1\n"
        if "--git-path" in arguments:
            return str(tmp_path).encode() + b"\n"
        if "rev-parse" in arguments:
            return policy["source"]["commit"].encode() + b"\n"
        if "--format=%G?%x00%GF%x00%GP%x00" in arguments:
            fingerprint = policy["source"]["commit_signer_fingerprint"].encode()
            return b"G\x00" + fingerprint + b"\x00" + fingerprint + b"\x00\n"
        if "--format=%ct" in arguments:
            return str(policy["source"]["source_date_epoch"]).encode() + b"\n"
        return b""

    class ArchiveProcess:
        def __init__(self, arguments, **kwargs):
            invocations.append((arguments, kwargs))
            self.stdout = io.BytesIO(archive_bytes)
            self.stderr = io.BytesIO()

        def wait(self, timeout=None):
            return 0

    monkeypatch.setattr(builder, "_git_command", git_command)
    monkeypatch.setattr(builder.subprocess, "Popen", ArchiveProcess)
    archive = tmp_path / "source.tar"
    digest = builder._verify_source_and_archive(
        Path("/approved/git"), Path("/approved/verifier"), policy, archive,
    )
    assert digest == hashlib.sha256(archive_bytes).hexdigest()
    assert archive.read_bytes() == archive_bytes
    arguments, options = invocations[0]
    assert "core.fsmonitor=false" in arguments
    assert f"core.hooksPath={os.devnull}" in arguments
    assert options["env"]["GIT_CONFIG_NOSYSTEM"] == "1"
    assert options["env"]["GIT_CONFIG_GLOBAL"] == os.devnull
    assert options["env"]["GIT_NO_REPLACE_OBJECTS"] == "1"
    assert options["env"]["GIT_NO_LAZY_FETCH"] == "1"
    assert options["env"]["GIT_ATTR_NOSYSTEM"] == "1"
    assert options["env"]["GIT_DIR"] != str(ROOT / ".git")
    for slot in ("gpg.program", "gpg.openpgp.program", "gpg.x509.program", "gpg.ssh.program"):
        assert f"{slot}=/approved/verifier" in arguments


@pytest.mark.parametrize("mode", ["production-prepare", "production-release"])
def test_production_cli_requires_commit_verifier(mode: str) -> None:
    arguments = [
        mode, "--policy", "/approved/policy.json", "--trusted-policy-sha256", "11" * 32,
        "--git", "/approved/git", "--docker", "/approved/docker", "--output-dir", "/approved/output",
    ]
    if mode == "production-release":
        arguments.extend(["--signed-output-lock", "/approved/lock.json"])
    with pytest.raises(builder.TonBuilderError, match="invalid final-V1 shape"):
        builder._parser().parse_args(arguments)
    parsed = builder._parser().parse_args(arguments + ["--commit-verifier", "/approved/verifier"])
    assert parsed.commit_verifier == "/approved/verifier"


@pytest.mark.parametrize("object_format", ["sha1", "sha256"])
@pytest.mark.parametrize("dirty", [None, "staged", "worktree", "untracked", "staged-gitlink"])
def test_source_archive_uses_only_signed_attributes_and_no_repository_filters(
    object_format: str, dirty: str | None, tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    git_path = shutil.which("git")
    if git_path is None:
        pytest.skip("Git unavailable")
    git = Path(git_path)
    repository = tmp_path / "repository"
    repository.mkdir(mode=0o700)
    scratch = tmp_path / "scratch"
    scratch.mkdir(mode=0o700)

    def run(*arguments: str, payload: bytes | None = None) -> bytes:
        return subprocess.run(
            [str(git), "-C", str(repository), *arguments],
            input=payload, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env=builder._closed_environment(),
        ).stdout.strip()

    # Assemble inert Git objects directly: this test does not create signatures
    # or execute a production verifier. Only the signature result is mocked.
    run("init", "--quiet", f"--object-format={object_format}", "--template=")
    blobs = {
        ".gitattributes": b"keep.txt filter=sccp-test\nomitted.txt export-ignore\nexpanded.txt export-subst\n",
        "expanded.txt": b"$Format:%H$\n",
        "keep.txt": b"approved source bytes\n",
        "omitted.txt": b"intentionally excluded by signed attributes\n",
    }
    entries = []
    for name, contents in sorted(blobs.items()):
        oid = run("hash-object", "-w", "--stdin", payload=contents).decode()
        entries.append(f"100644 blob {oid}\t{name}\n")
    oid_length = 40 if object_format == "sha1" else 64
    entries.append(f"160000 commit {'7' * oid_length}\toptional-docs\n")
    tree = run("mktree", payload="".join(entries).encode()).decode()
    commit = run(
        "hash-object", "-w", "-t", "commit", "--stdin",
        payload=(
            f"tree {tree}\nauthor Fixture <fixture@example.invalid> 1700000000 +0000\n"
            "committer Fixture <fixture@example.invalid> 1700000000 +0000\n\n"
            "Isolated archive regression fixture\n"
        ).encode(),
    ).decode()
    run("update-ref", "HEAD", commit)
    run("read-tree", "--reset", "-u", commit)
    # Optional gitlink worktrees can contain independent, unusable metadata.
    # Parent-source validation must never recurse into that child checkout.
    child_git = repository / "optional-docs" / ".git"
    child_git.mkdir(parents=True)
    (child_git / "objects").mkdir()
    (child_git / "refs").mkdir()
    (child_git / "HEAD").write_text("ref: refs/heads/fixture\n")
    (child_git / "config").write_text("[core]\n\trepositoryformatversion = 999\n")
    # These files are deliberately outside the signed tree. Required, absent
    # filter executables make any accidental helper selection fail the test.
    (repository / ".git" / "info").mkdir(exist_ok=True)
    (repository / ".git" / "info" / "attributes").write_text("keep.txt export-ignore\n")
    ambient_attributes = tmp_path / "ambient-attributes"
    ambient_attributes.write_text("expanded.txt export-ignore\n")
    run("config", "core.attributesFile", str(ambient_attributes))
    for kind in ("clean", "smudge", "process"):
        run("config", f"filter.sccp-test.{kind}", str(tmp_path / "unavailable-filter"))
    run("config", "filter.sccp-test.required", "true")
    # Force a worktree recheck even when its bytes still match the index.
    keep = repository / "keep.txt"
    modified = keep.stat().st_mtime_ns + 2_000_000_000
    os.utime(keep, ns=(modified, modified))
    if dirty == "worktree":
        keep.write_bytes(b"changed working tree\n")
    elif dirty == "staged":
        staged = run("hash-object", "-w", "--stdin", payload=b"changed index\n").decode()
        run("update-index", "--cacheinfo", f"100644,{staged},keep.txt")
    elif dirty == "untracked":
        (repository / "untracked.txt").write_bytes(b"untracked source\n")
    elif dirty == "staged-gitlink":
        run("update-index", "--cacheinfo", f"160000,{'8' * oid_length},optional-docs")
    index = repository / ".git" / "index"
    original_index = index.read_bytes()
    original_index_mtime = index.stat().st_mtime_ns
    fingerprint = "0123456789abcdef"
    original_git_command = builder._git_command

    def git_command(executable, root, arguments, **kwargs):
        if "verify-commit" in arguments:
            return b""
        if "--format=%G?%x00%GF%x00%GP%x00" in arguments:
            return f"G\0{fingerprint}\0{fingerprint}\0\n".encode()
        return original_git_command(executable, root, arguments, **kwargs)

    monkeypatch.setattr(builder, "ROOT", repository)
    monkeypatch.setattr(builder, "_git_command", git_command)
    policy = {"source": {
        "commit": commit, "commit_signer_fingerprint": fingerprint, "source_date_epoch": 1_700_000_000,
    }}
    archive_path = scratch / "source.tar"
    if dirty is not None:
        with pytest.raises(builder.TonBuilderError, match="completely clean"):
            builder._verify_source_and_archive(git, Path("/approved/verifier"), policy, archive_path)
    else:
        digest = builder._verify_source_and_archive(git, Path("/approved/verifier"), policy, archive_path)
        assert digest == hashlib.sha256(archive_path.read_bytes()).hexdigest()
        with tarfile.open(archive_path) as archive:
            assert "source/omitted.txt" not in archive.getnames()
            assert archive.extractfile("source/keep.txt").read() == blobs["keep.txt"]
            assert archive.extractfile("source/expanded.txt").read() == (commit + "\n").encode()
    assert index.read_bytes() == original_index
    assert index.stat().st_mtime_ns == original_index_mtime


@pytest.mark.parametrize(
    "object_format,commit", [("sha512", "1" * 40), ("sha1", "1" * 64), ("sha256", "0" * 64)],
)
def test_isolated_git_directory_rejects_invalid_source_identity(
    object_format: str, commit: str, tmp_path: Path,
) -> None:
    with pytest.raises(common.SccpReleaseError, match="identity is not canonical"):
        common.create_isolated_git_directory(tmp_path, object_format=object_format, commit=commit)


def test_isolated_git_directory_rejects_shared_scratch_parent(tmp_path: Path) -> None:
    parent = tmp_path / "shared"
    parent.mkdir(mode=0o755)
    with pytest.raises(common.SccpReleaseError, match="owner-only"):
        common.create_isolated_git_directory(parent, object_format="sha1", commit="1" * 40)
