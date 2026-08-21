#!/usr/bin/env python3
"""Drive the fixed Taira Python authority paths in an isolated Linux mount.

Prerequisites are Linux root, a root-owned setup JSON file, the repository path,
an empty root-owned work directory, and the test adapter already installed at
the production libexec path.  This helper provisions and serves all eight
fixed roles, installs administrator-issued run assignments, exercises all
seven former authority barriers, and exercises the qualification role through
the unchanged fixed native wrapper.  It never accepts authority roots,
bindings, sockets, or a verifier-binary override.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import json
import os
import re
import signal
import stat
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from unittest import mock


SETUP_SCHEMA = "iroha.taira.python-native-authority-e2e-setup.v1"
RESULT_SCHEMA = "iroha.taira.python-native-authority-e2e-result.v1"
ASSIGNMENT_SCHEMA = "iroha.taira.authority-run-assignment.v1"
ROLE_LABELS = (
    "native-evidence",
    "privacy-protocol-origin",
    "privacy-governance",
    "qualification",
    "deploy-issuance",
    "rollout-observation",
    "public-soak-observation",
    "public-soak-replay-admission",
)
EXPECTED_CLOCKS = {
    "native-evidence": 1_900_000_000_000,
    "privacy-protocol-origin": 1_900_000_000_000,
    "privacy-governance": 1_800_000_000_001,
    "qualification": 1_900_000_000_000,
    "deploy-issuance": 1_900_000_000_000,
    "rollout-observation": 1_900_000_000_000,
    "public-soak-observation": 1_800_000_001_000,
    "public-soak-replay-admission": 1_800_000_003_000,
}
HEX_64 = re.compile(r"[0-9a-f]{64}")
MAX_SETUP_BYTES = 1024 * 1024
COMMAND_TIMEOUT = 30
START_TIMEOUT = 20.0


class DriverError(RuntimeError):
    """The isolated native authority exercise failed closed."""


@dataclass(frozen=True)
class RoleSetup:
    service_uid: int
    client_uid: int
    administrator_uid: int
    policy_sha256: str
    clock_unix_millis: int


@dataclass(frozen=True)
class Setup:
    roles: dict[str, RoleSetup]
    retained_private_key: bytes
    native_assignment: dict[str, str]


@dataclass
class Fleet:
    client: object
    setup: Setup
    work_root: Path
    servers: list[subprocess.Popen[bytes]]

    @property
    def binary(self) -> Path:
        return Path(self.client.FIXED_VERIFIER_BINARY)

    def _run(
        self,
        arguments: list[str],
        *,
        uid: int = 0,
        payload: bytes | None = None,
        pass_fds: tuple[int, ...] = (),
    ) -> bytes:
        completed = subprocess.run(
            [str(self.binary), *arguments],
            input=payload,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=COMMAND_TIMEOUT,
            cwd="/",
            env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
            close_fds=True,
            pass_fds=pass_fds,
            preexec_fn=_credential_drop(uid),
        )
        if completed.returncode != 0:
            diagnostic = completed.stderr.decode("utf-8", "replace").strip()
            raise DriverError(
                f"fixed authority command {arguments[0]!r} failed for UID {uid}: "
                f"{diagnostic or 'no diagnostic'}"
            )
        return completed.stdout

    def provision(self) -> None:
        if os.geteuid() != 0:
            raise DriverError("the native authority driver requires Linux root")
        if not self.binary.is_file():
            raise DriverError("the fixed native authority adapter is absent")
        for role in ROLE_LABELS:
            binding = self.client.ROLE_REGISTRY[role]
            role_setup = self.setup.roles[role]
            self._run(
                [
                    "prepare-role",
                    "--role",
                    role,
                    "--service-uid",
                    str(role_setup.service_uid),
                ]
            )
            pending = binding.state_directory / "binding-install-v1.norito"
            with contextlib.ExitStack() as stack:
                wrapping_fd = stack.enter_context(
                    _inherited_bytes(_wrapping_key(role))
                )
                extra: list[str] = []
                inherited = [wrapping_fd]
                if role == "privacy-governance":
                    retained_fd = stack.enter_context(
                        _inherited_bytes(self.setup.retained_private_key)
                    )
                    inherited.append(retained_fd)
                    extra.extend(
                        ("--retained-genesis-key-fd", str(retained_fd))
                    )
                elif role == "public-soak-replay-admission":
                    observation = self.client.ROLE_REGISTRY[
                        "public-soak-observation"
                    ].binding_path
                    observation_fd = os.open(
                        observation,
                        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0),
                    )
                    stack.callback(os.close, observation_fd)
                    inherited.append(observation_fd)
                    extra.extend(("--observation-binding-fd", str(observation_fd)))
                self._run(
                    [
                        "provision",
                        "--role",
                        role,
                        "--state-directory",
                        str(binding.state_directory),
                        "--binding-out",
                        str(pending),
                        "--service-id",
                        binding.service_id,
                        "--administrator-id",
                        binding.administrator_id,
                        "--service-uid",
                        str(role_setup.service_uid),
                        "--client-uid",
                        str(role_setup.client_uid),
                        "--administrator-uid",
                        str(role_setup.administrator_uid),
                        "--key-revision",
                        "1",
                        "--policy-revision",
                        "1",
                        "--policy-sha256",
                        role_setup.policy_sha256,
                        "--wrapping-key-fd",
                        str(wrapping_fd),
                        *extra,
                    ],
                    uid=role_setup.service_uid,
                    pass_fds=tuple(inherited),
                )
            self._run(["install-binding", "--role", role])

    def start(self) -> None:
        for role in ROLE_LABELS:
            binding = self.client.ROLE_REGISTRY[role]
            role_setup = self.setup.roles[role]
            administrator_socket = binding.request_socket.with_name(
                "administrator-v1.sock"
            )
            wrapping_read, wrapping_write = os.pipe()
            try:
                os.write(wrapping_write, _wrapping_key(role))
            finally:
                os.close(wrapping_write)
            log_path = self.work_root / f"server-{role}.log"
            log = open(log_path, "xb", buffering=0)
            try:
                process = subprocess.Popen(
                    [
                        str(self.binary),
                        "serve",
                        "--role",
                        role,
                        "--state-directory",
                        str(binding.state_directory),
                        "--binding",
                        str(binding.binding_path),
                        "--request-socket",
                        str(binding.request_socket),
                        "--administrator-socket",
                        str(administrator_socket),
                        "--wrapping-key-fd",
                        str(wrapping_read),
                    ],
                    stdin=subprocess.DEVNULL,
                    stdout=log,
                    stderr=log,
                    cwd="/",
                    env={
                        "LANG": "C",
                        "LC_ALL": "C",
                        "PATH": "/usr/bin:/bin",
                    },
                    close_fds=True,
                    pass_fds=(wrapping_read,),
                    preexec_fn=_credential_drop(role_setup.service_uid),
                    start_new_session=True,
                )
            finally:
                os.close(wrapping_read)
                log.close()
            self.servers.append(process)
        deadline = time.monotonic() + START_TIMEOUT
        while time.monotonic() < deadline:
            if any(process.poll() is not None for process in self.servers):
                break
            if all(
                _is_socket(self.client.ROLE_REGISTRY[role].request_socket)
                and _is_socket(
                    self.client.ROLE_REGISTRY[role].request_socket.with_name(
                        "administrator-v1.sock"
                    )
                )
                for role in ROLE_LABELS
            ):
                for role in ROLE_LABELS:
                    status = self.client.preflight(role)
                    expected = self.setup.roles[role]
                    if (
                        status["service_uid"] != expected.service_uid
                        or status["client_uid"] != expected.client_uid
                        or status["policy_revision"] != 1
                    ):
                        raise DriverError(f"{role} status differs from setup")
                return
            time.sleep(0.02)
        failed = [
            (ROLE_LABELS[index], process.returncode)
            for index, process in enumerate(self.servers)
            if process.poll() is not None
        ]
        raise DriverError(f"authority servers did not become ready: {failed}")

    def stop(self) -> None:
        for process in self.servers:
            if process.poll() is None:
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(process.pid, signal.SIGTERM)
        deadline = time.monotonic() + 5.0
        for process in self.servers:
            remaining = max(0.0, deadline - time.monotonic())
            with contextlib.suppress(subprocess.TimeoutExpired):
                process.wait(timeout=remaining)
        for process in self.servers:
            if process.poll() is None:
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)

    def assign(
        self,
        role: str,
        subject: dict[str, object],
        artifacts: tuple[object, ...] = (),
    ) -> str:
        manifest = []
        for ordinal, artifact in enumerate(artifacts):
            path = Path(artifact.path)
            payload = path.read_bytes()
            manifest.append(
                {
                    "name": artifact.name,
                    "ordinal": ordinal,
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "size": len(payload),
                }
            )
        subject_bytes = self.client.canonical_json_bytes(subject)[:-1]
        manifest_bytes = self.client.canonical_json_bytes(manifest)[:-1]
        run_id = self.client.derive_run_id(role, subject)
        role_setup = self.setup.roles[role]
        clock = role_setup.clock_unix_millis
        assignment: dict[str, object] = {
            "artifact_manifest_sha256": hashlib.sha256(
                manifest_bytes
            ).hexdigest(),
            "expires_at_unix_millis": clock + 60_000,
            "issued_at_unix_millis": clock - 10,
            "key_revision": 1,
            "not_before_unix_millis": clock - 1,
            "policy_revision": 1,
            "policy_sha256": role_setup.policy_sha256,
            "role": role,
            "run_id": run_id,
            "schema": ASSIGNMENT_SCHEMA,
            "subject_sha256": hashlib.sha256(subject_bytes).hexdigest(),
        }
        if role == "native-evidence":
            assignment.update(self.setup.native_assignment)
        output = self._run(
            ["assign-run", "--role", role],
            uid=role_setup.administrator_uid,
            payload=self.client.canonical_json_bytes(assignment),
        )
        value = self.client.decode_canonical_json(output, "run-assignment result")
        returned_assignment = value.get("assignment")
        if (
            value.get("schema")
            != "iroha.taira.authority-run-assignment-result.v1"
            or value.get("role") != role
            or value.get("status") not in {"assigned", "replayed"}
            or not isinstance(returned_assignment, dict)
            or returned_assignment.get("run_id") != run_id
        ):
            raise DriverError(f"{role} assignment returned another run")
        return run_id


def _credential_drop(uid: int):
    def drop() -> None:
        os.setgroups([])
        os.setgid(uid)
        os.setuid(uid)

    return drop


@contextlib.contextmanager
def _inherited_bytes(payload: bytes):
    read_fd, write_fd = os.pipe()
    try:
        os.write(write_fd, payload)
    finally:
        os.close(write_fd)
    try:
        yield read_fd
    finally:
        os.close(read_fd)


def _wrapping_key(role: str) -> bytes:
    return hashlib.sha256(
        b"iroha:taira:python-native-e2e-wrapping-key:v1\0" + role.encode("ascii")
    ).digest()


def _is_socket(path: Path) -> bool:
    try:
        return stat.S_ISSOCK(path.lstat().st_mode)
    except OSError:
        return False


def _strict_object(payload: bytes, label: str) -> dict[str, object]:
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise DriverError(f"{label} is not JSON") from error
    if not isinstance(value, dict):
        raise DriverError(f"{label} is not an object")
    return value


def _load_setup(path: Path) -> Setup:
    if not path.is_absolute() or path.is_symlink():
        raise DriverError("setup JSON path must be absolute and nonsymlinked")
    before = path.stat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != 0
        or before.st_nlink != 1
        or before.st_mode & 0o022
        or before.st_size <= 0
        or before.st_size > MAX_SETUP_BYTES
    ):
        raise DriverError("setup JSON has an unsafe identity")
    payload = path.read_bytes()
    after = path.stat()
    if (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    ) != (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ):
        raise DriverError("setup JSON changed while reading")
    value = _strict_object(payload, "setup JSON")
    expected_fields = {
        "schema",
        "roles",
        "clocks_unix_millis",
        "governance_retained_private_key_hex",
        "native_assignment",
    }
    if set(value) != expected_fields or value["schema"] != SETUP_SCHEMA:
        raise DriverError("setup JSON fields or schema differ")
    roles_value = value["roles"]
    clocks = value["clocks_unix_millis"]
    if not isinstance(roles_value, dict) or set(roles_value) != set(ROLE_LABELS):
        raise DriverError("setup role registry is not exact")
    if not isinstance(clocks, dict) or clocks != EXPECTED_CLOCKS:
        raise DriverError("setup role clocks differ from the test adapter")
    roles: dict[str, RoleSetup] = {}
    all_uids: list[int] = []
    for role in ROLE_LABELS:
        row = roles_value[role]
        if not isinstance(row, dict) or set(row) != {
            "administrator_uid",
            "client_uid",
            "policy_sha256",
            "service_uid",
        }:
            raise DriverError(f"{role} setup fields differ")
        service_uid = row["service_uid"]
        client_uid = row["client_uid"]
        administrator_uid = row["administrator_uid"]
        policy = row["policy_sha256"]
        if any(
            isinstance(uid, bool) or not isinstance(uid, int) or not 0 <= uid < 2**32 - 1
            for uid in (service_uid, client_uid, administrator_uid)
        ):
            raise DriverError(f"{role} setup UID is invalid")
        if (role == "qualification") != (service_uid == 0):
            raise DriverError("only qualification may use root as service UID")
        if client_uid == 0 or administrator_uid == 0:
            raise DriverError(f"{role} client and administrator UIDs must be nonroot")
        if not isinstance(policy, str) or HEX_64.fullmatch(policy) is None or policy == "0" * 64:
            raise DriverError(f"{role} policy digest is invalid")
        all_uids.extend((service_uid, client_uid, administrator_uid))
        roles[role] = RoleSetup(
            service_uid=service_uid,
            client_uid=client_uid,
            administrator_uid=administrator_uid,
            policy_sha256=policy,
            clock_unix_millis=EXPECTED_CLOCKS[role],
        )
    nonzero = [uid for uid in all_uids if uid != 0]
    if len(nonzero) != len(set(nonzero)):
        raise DriverError("authority UIDs are reused across roles")
    private_hex = value["governance_retained_private_key_hex"]
    if not isinstance(private_hex, str) or HEX_64.fullmatch(private_hex) is None:
        raise DriverError("retained governance private key is invalid")
    native = value["native_assignment"]
    if not isinstance(native, dict) or set(native) != {
        "controller_digest",
        "controller_host_id",
        "controller_installation_id",
        "run_nonce",
    }:
        raise DriverError("native assignment fields differ")
    for field in ("controller_digest", "run_nonce"):
        item = native[field]
        if not isinstance(item, str) or HEX_64.fullmatch(item) is None or item == "0" * 64:
            raise DriverError(f"native assignment {field} is invalid")
    for field in ("controller_host_id", "controller_installation_id"):
        item = native[field]
        if not isinstance(item, str) or not item or len(item) > 128:
            raise DriverError(f"native assignment {field} is invalid")
    return Setup(roles, bytes.fromhex(private_hex), dict(native))


def _freeze(artifacts: tuple[object, ...]) -> None:
    seen: set[tuple[int, int]] = set()
    for artifact in artifacts:
        path = Path(artifact.path)
        info = path.lstat()
        identity = (info.st_dev, info.st_ino)
        if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or identity in seen:
            raise DriverError(f"fixture artifact is unsafe: {path}")
        seen.add(identity)
        os.chown(path, 0, -1)
        path.chmod(0o400)


def _protocol_arguments(modules: SimpleNamespace, root: Path, receipt_id: str):
    fixture = modules.protocol_fixture
    return {
        "expected_source": fixture.SOURCE,
        "expected_validator_binary_sha256": fixture.BINDINGS[
            "validator_binary_sha256"
        ],
        "expected_linux_release_archive_sha256": fixture.BINDINGS[
            "linux_release_archive_sha256"
        ],
        "expected_exact12_matrix_sha256": fixture.BINDINGS[
            "exact12_matrix_sha256"
        ],
        "expected_artifact_handoff_sha256": fixture.BINDINGS[
            "artifact_handoff_sha256"
        ],
        "expected_receipt_id": receipt_id,
        "now_unix": fixture.NOW,
    }


def _mutate_signature(value: dict[str, object]) -> dict[str, object]:
    changed = json.loads(json.dumps(value))
    signature = changed.get("signature")
    if not isinstance(signature, str) or not signature:
        raise DriverError("sidecar has no mutable signature field")
    changed["signature"] = ("0" if signature[0] != "0" else "1") + signature[1:]
    return changed


def _exercise(modules: SimpleNamespace, fleet: Fleet, work: Path) -> dict[str, object]:
    client = modules.client
    results: dict[str, object] = {}

    native_dir = work / "native"
    native_dir.mkdir()
    native_args = modules.native_fixture._args(native_dir)
    native_artifacts = modules.native._authority_artifacts(native_args)
    _freeze(native_artifacts)
    native_subject = modules.native._build_untrusted_authority_structure(native_args)
    fleet.assign("native-evidence", native_subject, native_artifacts)
    native_output = native_dir / "authority.json"
    if modules.native.main(
        modules.paths_fixture._native_cli_args(native_args, native_output, "create")
    ) != 0 or modules.native.main(
        modules.paths_fixture._native_cli_args(native_args, native_output, "verify")
    ) != 0:
        raise DriverError("native evidence public create/verify failed")
    results["native-evidence"] = "authorized-and-historically-verified"

    protocol_dir = work / "protocol"
    protocol_root = protocol_dir / "evidence"
    protocol_dir.mkdir()
    receipt_id = modules.protocol_fixture.build_valid_evidence(protocol_root)
    protocol_kwargs = _protocol_arguments(modules, protocol_root, receipt_id)
    _, protocol_subject, protocol_artifacts = modules.protocol._validated_authority_request(
        protocol_root, **protocol_kwargs
    )
    _freeze(protocol_artifacts)
    fleet.assign("privacy-protocol-origin", protocol_subject, protocol_artifacts)
    issued_protocol = modules.protocol.validate_evidence_directory(
        protocol_root, **protocol_kwargs
    )
    issued_protocol.persist_sidecars()
    modules.protocol.verify_authenticated_evidence_directory(
        protocol_root, **protocol_kwargs
    )
    results["privacy-protocol-origin"] = "authorized-and-historically-verified"

    governance_request, _, governance_subject = (
        modules.governance_fixture._request_from_rust_fixture()
    )
    fleet.assign("privacy-governance", governance_subject)
    issued_governance = (
        modules.governance.request_authenticated_governance_transaction_v1(
            governance_request
        )
    )
    modules.governance.validate_authenticated_governance_receipt_v1(
        governance_request,
        issued_governance.durable_receipt,
        authority_envelope_payload=issued_governance.authority_envelope,
    )
    results["privacy-governance"] = "authorized-and-historically-verified"

    qualification_dir = work / "qualification"
    qualification_dir.mkdir()
    qualification_rows = (
        ("source/capability/exact12-capability-manifest-v1.norito", b"NRT0qualification-capability-fixture-v1\0"),
        ("source/sdk/iroha_python_privacy_v1.whl", b"qualification-wheel-fixture-v1\0"),
        ("source/worker/iroha_privacy_wallet_worker", b"qualification-worker-fixture-v1\0"),
        ("source/abi22/libconnect_norito_bridge.so", b"qualification-abi22-fixture-v1\0"),
    )
    qualification_artifacts = []
    for ordinal, (name, payload) in enumerate(qualification_rows):
        path = qualification_dir / f"artifact-{ordinal}"
        path.write_bytes(payload)
        qualification_artifacts.append(client.Artifact(name, path))
    qualification_artifacts_tuple = tuple(qualification_artifacts)
    _freeze(qualification_artifacts_tuple)
    qualification_subject = {"case": "authority-owned-sandbox", "schema_version": 1}
    modules.qualification.require_native_qualification_isolation(None)
    fleet.assign("qualification", qualification_subject, qualification_artifacts_tuple)
    qualification_result = client.authorize(
        "qualification",
        qualification_subject,
        artifacts=qualification_artifacts_tuple,
    )
    claims = qualification_result.authority_envelope.get("claims")
    role_result = claims.get("role_result") if isinstance(claims, dict) else None
    if not isinstance(role_result, dict) or "probe_results" not in role_result:
        raise DriverError("qualification service omitted hermetic probe results")
    client.verify_receipt(
        "qualification",
        qualification_subject,
        authority_envelope=qualification_result.authority_envelope,
        durable_receipt=qualification_result.durable_receipt,
        artifacts=qualification_artifacts_tuple,
    )
    results["qualification"] = (
        "former-barrier-authenticated-and-fixed-wrapper-authorized-and-verified"
    )

    deploy_dir = work / "deploy"
    deploy_dir.mkdir()
    admission, bundle, sources = modules.paths_fixture._deploy_plans(deploy_dir)
    deploy_subject = modules.deploy._deploy_authority_subject(
        admission, bundle, sources
    )
    deploy_artifacts = modules.deploy._deploy_authority_artifacts(
        admission, bundle, sources
    )
    _freeze(deploy_artifacts)
    fleet.assign("deploy-issuance", deploy_subject, deploy_artifacts)
    deploy_args = argparse.Namespace(
        allow_absent_old_child=False,
        apply=False,
        bundle=bundle.root,
        expected_dpn_validator_release_commit=admission.dpn_validator_release_commit,
        expected_production_reset_manifest_sha256=admission.reset_manifest_sha256,
        expected_source_commit=admission.source_commit,
        maximum_fsync_latency_ms=modules.deploy.DEFAULT_MAXIMUM_FSYNC_LATENCY_MS,
        minimum_free_bytes=modules.deploy.DEFAULT_MINIMUM_FREE_BYTES,
    )
    snapshots = tuple(
        SimpleNamespace(
            path=deploy_dir / f"{slug}.plist",
            managed=SimpleNamespace(child_was_present=True),
        )
        for slug in modules.deploy.SLUGS
    )
    with (
        mock.patch.object(modules.deploy, "require_sealed_external_tool_identity", return_value=None),
        mock.patch.object(modules.deploy, "validate_arguments", return_value=None),
        mock.patch.object(modules.deploy, "verify_deployment_admission", return_value=admission),
        mock.patch.object(modules.deploy, "validate_bundle", return_value=bundle),
        mock.patch.object(modules.deploy, "validate_sources", return_value=sources),
        mock.patch.object(modules.deploy, "require_inputs_match_admission", return_value=None),
        mock.patch.object(modules.deploy, "require_admission_archive_unchanged", return_value=None),
        mock.patch.object(modules.deploy, "require_mutable_bundle_identities", return_value=None),
        mock.patch.object(modules.deploy, "validate_dry_run_kagemusha_exact_config", return_value=False),
        mock.patch.object(modules.deploy, "capture_old_cohort", return_value=snapshots),
    ):
        dry_report = modules.deploy.execute(deploy_args, ops=SimpleNamespace())
    if dry_report["deploy_authority_status"] != "verified":
        raise DriverError("deploy public dry-run consumed or refused its lease")
    applied = modules.deploy._authorize_deploy_lease(
        admission, bundle, sources, apply=True
    )
    client.verify_receipt(
        "deploy-issuance",
        deploy_subject,
        authority_envelope=applied.authority_envelope,
        durable_receipt=applied.durable_receipt,
        artifacts=deploy_artifacts,
    )
    finalized = modules.deploy._finalize_deploy_lease(
        admission,
        bundle,
        sources,
        applied,
        outcome="success",
        result={"applied": True, "fixture": "authority-only"},
    )
    if finalized.status not in {"finalized", "replayed"}:
        raise DriverError("deployment lease was not finalized")
    replayed_finalization = modules.deploy._finalize_deploy_lease(
        admission,
        bundle,
        sources,
        applied,
        outcome="success",
        result={"applied": True, "fixture": "authority-only"},
    )
    if (
        replayed_finalization.status != "replayed"
        or replayed_finalization.authority_envelope_bytes
        != finalized.authority_envelope_bytes
        or replayed_finalization.durable_receipt_bytes
        != finalized.durable_receipt_bytes
    ):
        raise DriverError("deployment finalization retry was not byte-identical")
    results["deploy-issuance"] = "dry-run-verified-apply-consumed-and-finalized"

    rollout_plan = modules.rollout.expected_plan()
    rollout_result = modules.rollout_fixture._valid_result()
    rollout_subject = modules.rollout._rollout_authority_subject(
        rollout_result, rollout_plan
    )
    fleet.assign("rollout-observation", rollout_subject)
    issued_rollout = modules.rollout.validate_result(
        rollout_result, plan=rollout_plan
    )
    modules.rollout.verify_authenticated_result(
        rollout_result,
        plan=rollout_plan,
        authority_envelope=issued_rollout.authority_envelope,
        durable_receipt=issued_rollout.durable_receipt,
    )
    original_envelope = client.decode_canonical_json(
        issued_rollout.authority_envelope, "rollout authority envelope"
    )
    mutated = _mutate_signature(original_envelope)
    try:
        modules.rollout.verify_authenticated_result(
            rollout_result,
            plan=rollout_plan,
            authority_envelope=client.canonical_json_bytes(mutated),
            durable_receipt=issued_rollout.durable_receipt,
        )
    except modules.rollout.RolloutContractError:
        pass
    else:
        raise DriverError("mutated rollout sidecar was accepted")
    results["rollout-observation"] = "authorized-historically-verified-and-mutation-refused"

    core = modules.soak_fixture.subject_core()
    completed = modules.soak_fixture.COMPLETED_MS
    observation_subject = modules.public_soak._observation_subject(core, completed)
    fleet.assign("public-soak-observation", observation_subject)
    observation = client.authorize("public-soak-observation", observation_subject)
    observation_payload = observation.authority_envelope_bytes
    replay_subject = modules.public_soak._replay_subject(
        observation_payload, core, completed
    )
    fleet.assign("public-soak-replay-admission", replay_subject)
    admission_payload = modules.public_soak.consume_fresh_public_soak_admission(
        observation_payload,
        subject_core=core,
        completed_at_unix_ms=completed,
    )
    modules.public_soak.verify_authenticated_public_soak_authority_envelope(
        observation_payload,
        durable_admission_receipt=admission_payload,
        subject_core=core,
        completed_at_unix_ms=completed,
    )
    results["public-soak"] = "fresh-admission-consumed-and-historically-verified"

    binding_checks = (
        (
            "native-evidence",
            lambda: modules.native.build_authority(argparse.Namespace()),
            modules.native.TairaReleaseAuthorityError,
        ),
        (
            "privacy-protocol-origin",
            lambda: modules.protocol.validate_evidence_directory(
                Path("/poison-protocol-input"),
                expected_source=object(),
                expected_validator_binary_sha256="bad",
                expected_linux_release_archive_sha256="bad",
                expected_exact12_matrix_sha256="bad",
                expected_artifact_handoff_sha256="bad",
                expected_receipt_id="bad",
                now_unix=0,
            ),
            modules.protocol.PrivacyProtocolEvidenceError,
        ),
        (
            "privacy-governance",
            lambda: modules.governance.request_authenticated_governance_transaction_v1(
                object()
            ),
            modules.governance.PrivacyGovernanceAuthorityError,
        ),
        (
            "qualification",
            lambda: modules.qualification.require_native_qualification_isolation(None),
            modules.qualification.QualificationHandoffError,
        ),
        (
            "deploy-issuance",
            lambda: modules.deploy.execute(argparse.Namespace()),
            modules.deploy.DeploymentError,
        ),
        (
            "rollout-observation",
            lambda: modules.rollout.validate_result(object(), plan=object()),
            modules.rollout.RolloutContractError,
        ),
        (
            "public-soak-observation",
            lambda: modules.public_soak.consume_fresh_public_soak_admission(
                b"poison-public-soak-input",
                subject_core=object(),
                completed_at_unix_ms=0,
            ),
            modules.public_soak.PublicSoakAuthorityError,
        ),
        (
            "public-soak-replay-admission",
            lambda: modules.public_soak.consume_fresh_public_soak_admission(
                b"poison-public-soak-input",
                subject_core=object(),
                completed_at_unix_ms=0,
            ),
            modules.public_soak.PublicSoakAuthorityError,
        ),
    )
    for role, operation, expected_error in binding_checks:
        _assert_binding_failure_precedes_input(
            client, role, operation, expected_error
        )
    results["fail-before-input"] = (
        "all-seven-layered-boundaries-refused-poisoned-bindings-before-input"
    )
    return results


def _rewrite_existing(path: Path, payload: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | getattr(os, "O_CLOEXEC", 0))
    try:
        os.ftruncate(descriptor, 0)
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise DriverError("short binding rewrite")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _assert_binding_failure_precedes_input(
    client: object,
    role: str,
    operation: object,
    expected_error: type[BaseException],
) -> None:
    binding = client.ROLE_REGISTRY[role].binding_path
    original = binding.read_bytes()
    poisoned = bytearray(original)
    poisoned[-1] ^= 1
    try:
        _rewrite_existing(binding, bytes(poisoned))
        try:
            operation()
        except expected_error:
            pass
        except BaseException as error:
            raise DriverError(
                f"{role} inspected poisoned caller input before binding refusal: "
                f"{type(error).__name__}"
            ) from error
        else:
            raise DriverError(f"{role} accepted a poisoned installed binding")
    finally:
        _rewrite_existing(binding, original)
    client.preflight(role)


def _load_modules(repo_root: Path) -> SimpleNamespace:
    sys.path.insert(0, str(repo_root))
    from scripts import deploy_taira_v21_reset as deploy
    from scripts import build_privacy_v1_boi_handoff as qualification
    from scripts import taira_authority_client as client
    from scripts import taira_privacy_governance_authority as governance
    from scripts import taira_privacy_protocol_receipt as protocol
    from scripts import taira_privacy_rollout_contract as rollout
    from scripts import taira_public_soak_authority_contract as public_soak
    from scripts import taira_release_authority as native
    from scripts.tests import taira_privacy_governance_authority_test as governance_fixture
    from scripts.tests import taira_privacy_protocol_receipt_test as protocol_fixture
    from scripts.tests import taira_privacy_rollout_contract_test as rollout_fixture
    from scripts.tests import taira_public_soak_authority_contract_test as soak_fixture
    from scripts.tests import taira_python_authority_paths_test as paths_fixture
    from scripts.tests import taira_release_authority_test as native_fixture

    if Path(client.FIXED_VERIFIER_BINARY) not in (
        Path("/usr/libexec/iroha/taira_release_authority"),
        Path("/usr/local/libexec/iroha/taira_release_authority"),
    ):
        raise DriverError("Python client does not use a fixed production verifier")
    return SimpleNamespace(**locals())


def _absolute_directory(path: Path, label: str) -> Path:
    if not path.is_absolute() or path.is_symlink() or not path.is_dir():
        raise DriverError(f"{label} must be an absolute nonsymlinked directory")
    return path.resolve(strict=True)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, required=True)
    parser.add_argument("--work-root", type=Path, required=True)
    parser.add_argument("--setup", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    fleet: Fleet | None = None
    try:
        if sys.platform != "linux":
            raise DriverError("the native authority E2E driver is Linux-only")
        repo_root = _absolute_directory(args.repo_root, "repository root")
        work_root = _absolute_directory(args.work_root, "work root")
        if any(work_root.iterdir()):
            raise DriverError("work root must be empty")
        setup = _load_setup(args.setup)
        modules = _load_modules(repo_root)
        original_client_functions = (
            modules.client.preflight,
            modules.client.authorize,
            modules.client.verify_receipt,
            modules.client.finalize_deployment,
        )
        fleet = Fleet(modules.client, setup, work_root, [])
        fleet.provision()
        fleet.start()
        checks = _exercise(modules, fleet, work_root)
        if original_client_functions != (
            modules.client.preflight,
            modules.client.authorize,
            modules.client.verify_receipt,
            modules.client.finalize_deployment,
        ):
            raise DriverError("fixed Python native client functions were replaced")
        print(
            json.dumps(
                {
                    "checks": checks,
                    "qualification_barrier": "authenticated-fixed-service",
                    "ready": True,
                    "schema": RESULT_SCHEMA,
                },
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
        )
        return 0
    except (DriverError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Taira Python native authority E2E failed: {error}", file=sys.stderr)
        return 1
    finally:
        if fleet is not None:
            fleet.stop()


if __name__ == "__main__":
    raise SystemExit(main())
