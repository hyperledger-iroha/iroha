from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import struct
import subprocess
import sys
from pathlib import Path

import pytest

MODULE_PATH = Path(__file__).parents[1] / "src/iroha_python/privacy_zk_x509_worker.py"
SPEC = importlib.util.spec_from_file_location("privacy_zk_x509_worker_source", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
worker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = worker
SPEC.loader.exec_module(worker)

AUTH_KEY = bytes(range(1, 33))
SEQUENCE_BYTES = (7).to_bytes(8, "big")
SOURCE_COMMIT = "1" * 40
SOURCE_SHA256 = "2" * 64
PROFILE_SHA256 = "3" * 64
WORKSPACE_SOURCE_MANIFEST_SHA256 = "4" * 64
CARGO_LOCK_SHA256 = "5" * 64
EXPECTATIONS_JSON_SHA256 = "6" * 64
EXPECTATIONS_NORITO_SHA256 = "7" * 64
KAT_PROOF_SHA256 = "8" * 64
RESOURCE_CERTIFICATE_SHA256 = "9" * 64
SOUNDNESS_CERTIFICATE_SHA256 = "a" * 64
SOURCE_ALLOWED_SIGNERS_SHA256 = "b" * 64
SOURCE_REVOCATION_SHA256 = "c" * 64
KAT_PROOF_BYTES = 1_234_567
ARTIFACT_BYTES = b"fixture executable; subprocess.run is replaced"
ARTIFACT_SHA256 = hashlib.sha256(ARTIFACT_BYTES).hexdigest()
ISOLATION_PACKAGE_SHA256 = worker._qualified_isolation_package_sha256(
    ARTIFACT_SHA256
)


def executable(tmp_path: Path) -> tuple[Path, str]:
    path = tmp_path / "iroha_zk_x509_prover_worker"
    path.write_bytes(ARTIFACT_BYTES)
    path.chmod(0o700)
    return path, ARTIFACT_SHA256


def identity_payload(
    *,
    source_commit: str = SOURCE_COMMIT,
    source_sha256: str = SOURCE_SHA256,
    profile_sha256: str | None = PROFILE_SHA256,
    kat_proof_sha256: str = KAT_PROOF_SHA256,
    ready: bool = True,
    qualified: bool = True,
    isolation_contract: str | None = None,
    isolation_package_sha256: str | None = ISOLATION_PACKAGE_SHA256,
) -> bytes:
    release_evidence_sha256 = worker._release_evidence_sha256(
        PROFILE_SHA256,
        KAT_PROOF_BYTES,
        kat_proof_sha256,
        EXPECTATIONS_NORITO_SHA256,
        EXPECTATIONS_JSON_SHA256,
        SOUNDNESS_CERTIFICATE_SHA256,
        RESOURCE_CERTIFICATE_SHA256,
    )
    identity = {
        "artifact_self_hash_required": True,
        "cargo_lock_sha256": CARGO_LOCK_SHA256,
        "compiled_profile_sha256": profile_sha256,
        "expectations_json_sha256": EXPECTATIONS_JSON_SHA256,
        "expectations_norito_sha256": EXPECTATIONS_NORITO_SHA256,
        "operation": "prove-and-sign-zk-x509-action-v1",
        "production_profile_ready": ready,
        "protocol_id": worker.PRIVACY_ZK_X509_WORKER_PROTOCOL_ID_V1,
        "protocol_profile_sha256": PROFILE_SHA256,
        "protocol_version": worker.PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1,
        "public_request_schema_version": (
            worker.PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1
        ),
        "qualified_isolation_ready": qualified,
        "isolation_contract": isolation_contract
        or (
            worker._QUALIFIED_ISOLATION_CONTRACT_V1
            if qualified
            else worker._UNAVAILABLE_ISOLATION_CONTRACT_V1
        ),
        "isolation_package_sha256": isolation_package_sha256,
        "kat_proof_bytes": KAT_PROOF_BYTES,
        "kat_proof_sha256": kat_proof_sha256,
        "release_evidence_ready": True,
        "release_evidence_sha256": release_evidence_sha256,
        "resource_certificate_sha256": RESOURCE_CERTIFICATE_SHA256,
        "schema": "iroha.privacy.zk_x509_worker_identity",
        "schema_version": 2,
        "soundness_certificate_sha256": SOUNDNESS_CERTIFICATE_SHA256,
        "source_allowed_signers_sha256": SOURCE_ALLOWED_SIGNERS_SHA256,
        "source_closure_schema": (
            worker.PRIVACY_ZK_X509_WORKER_SOURCE_CLOSURE_SCHEMA_V1
        ),
        "source_commit": source_commit,
        "source_revocation_sha256": SOURCE_REVOCATION_SHA256,
        "source_sha256": source_sha256,
        "workspace_source_manifest_sha256": WORKSPACE_SOURCE_MANIFEST_SHA256,
    }
    return b"\x00" + worker._canonical_json_bytes(identity)


class AuthenticatedFakeRunner:
    def __init__(self, responses: list[bytes]) -> None:
        self.responses = list(responses)
        self.requests: list[tuple[int, int, bytes]] = []
        self.argv: list[list[str]] = []
        self.raw_inputs: list[bytearray] = []

    def __call__(self, args: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
        self.argv.append(args)
        raw_input = kwargs["input"]
        assert isinstance(raw_input, bytearray)
        self.raw_inputs.append(raw_input)
        encoded = bytes(raw_input)
        assert encoded[:32] == AUTH_KEY
        command, sequence, payload = worker._decode_frame(encoded[32:], AUTH_KEY)
        self.requests.append((command, sequence, payload))
        response = self.responses.pop(0)
        stdout = worker._encode_frame(command, sequence, response, AUTH_KEY)
        return subprocess.CompletedProcess(args, 0, stdout, b"")


def deterministic_tokens(count: int) -> bytes:
    if count == 32:
        return AUTH_KEY
    if count == 8:
        return SEQUENCE_BYTES
    raise AssertionError(f"unexpected token request: {count}")


def admission_payload(public_request: Path, secret_bundle: Path) -> bytes:
    metadata = secret_bundle.lstat()
    return b"".join(
        (
            b"\x00\x01",
            hashlib.sha256(public_request.read_bytes()).digest(),
            hashlib.sha256(secret_bundle.read_bytes()).digest(),
            struct.pack(
                ">QQQII",
                metadata.st_dev,
                metadata.st_ino,
                metadata.st_size,
                0o600,
                os.geteuid(),
            ),
        )
    )


def make_controller(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    runner: AuthenticatedFakeRunner,
    **pin_overrides: str,
) -> worker.PrivacyZkX509WorkerControllerV1:
    path, artifact_sha256 = executable(tmp_path)
    monkeypatch.setattr(worker.secrets, "token_bytes", deterministic_tokens)
    monkeypatch.setattr(worker.subprocess, "run", runner)
    return worker.PrivacyZkX509WorkerControllerV1(
        path,
        expected_artifact_sha256=pin_overrides.get(
            "expected_artifact_sha256", artifact_sha256
        ),
        expected_source_commit=pin_overrides.get(
            "expected_source_commit", SOURCE_COMMIT
        ),
        expected_source_sha256=pin_overrides.get(
            "expected_source_sha256", SOURCE_SHA256
        ),
        expected_compiled_profile_sha256=pin_overrides.get(
            "expected_compiled_profile_sha256", PROFILE_SHA256
        ),
        expected_workspace_source_manifest_sha256=pin_overrides.get(
            "expected_workspace_source_manifest_sha256",
            WORKSPACE_SOURCE_MANIFEST_SHA256,
        ),
    )


def test_identity_requires_exact_artifact_source_commit_source_and_profile_pins(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    runner = AuthenticatedFakeRunner([identity_payload()])
    controller = make_controller(monkeypatch, tmp_path, runner)
    assert controller.identity.source_commit == SOURCE_COMMIT
    assert controller.identity.source_sha256 == SOURCE_SHA256
    assert controller.identity.compiled_profile_sha256 == PROFILE_SHA256
    assert controller.identity.qualified_isolation_ready
    assert (
        controller.identity.isolation_package_sha256
        == ISOLATION_PACKAGE_SHA256
    )
    assert (
        controller.identity.workspace_source_manifest_sha256
        == WORKSPACE_SOURCE_MANIFEST_SHA256
    )
    assert runner.requests == [
        (int(worker.PrivacyZkX509WorkerCommandV1.IDENTITY), 7, b"")
    ]
    source_path = tmp_path / "iroha_zk_x509_prover_worker"
    assert runner.argv and all(Path(argv[0]) != source_path for argv in runner.argv)
    assert runner.raw_inputs and all(
        not any(raw_input) for raw_input in runner.raw_inputs
    )


def test_identity_launch_rejects_replaceable_ancestor_before_execution(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    replaceable = tmp_path / "replaceable"
    replaceable.mkdir(mode=0o700)
    path, artifact_sha256 = executable(replaceable)
    replaceable.chmod(0o777)
    runner = AuthenticatedFakeRunner([identity_payload()])
    monkeypatch.setattr(worker.secrets, "token_bytes", deterministic_tokens)
    monkeypatch.setattr(worker.subprocess, "run", runner)
    try:
        with pytest.raises(
            worker.PrivacyZkX509WorkerErrorV1,
            match="exact authenticated launch failed",
        ):
            worker.PrivacyZkX509WorkerControllerV1(
                path,
                expected_artifact_sha256=artifact_sha256,
                expected_source_commit=SOURCE_COMMIT,
                expected_source_sha256=SOURCE_SHA256,
                expected_compiled_profile_sha256=PROFILE_SHA256,
                expected_workspace_source_manifest_sha256=(
                    WORKSPACE_SOURCE_MANIFEST_SHA256
                ),
            )
        assert runner.requests == []
    finally:
        replaceable.chmod(0o700)


def test_constructor_rejects_zero_source_commit_before_launch(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    runner = AuthenticatedFakeRunner([identity_payload()])
    with pytest.raises(ValueError, match="must be nonzero"):
        make_controller(
            monkeypatch,
            tmp_path,
            runner,
            expected_source_commit="0" * 40,
        )
    assert runner.requests == []


@pytest.mark.parametrize(
    "field",
    [
        "expected_artifact_sha256",
        "expected_source_sha256",
        "expected_compiled_profile_sha256",
        "expected_workspace_source_manifest_sha256",
    ],
)
def test_constructor_rejects_zero_sha256_pin_before_launch(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    field: str,
) -> None:
    runner = AuthenticatedFakeRunner([identity_payload()])
    with pytest.raises(ValueError, match="must be nonzero"):
        make_controller(monkeypatch, tmp_path, runner, **{field: "0" * 64})
    assert runner.requests == []


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("expected_source_commit", "4" * 40),
        ("expected_source_sha256", "5" * 64),
        ("expected_compiled_profile_sha256", "6" * 64),
        ("expected_workspace_source_manifest_sha256", "7" * 64),
    ],
)
def test_identity_rejects_every_mismatched_review_pin(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    field: str,
    value: str,
) -> None:
    runner = AuthenticatedFakeRunner([identity_payload()])
    with pytest.raises(worker.PrivacyZkX509WorkerErrorV1, match="reviewed pin"):
        make_controller(monkeypatch, tmp_path, runner, **{field: value})


def test_identity_rejects_missing_compiled_profile_without_candidate_fallback(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    runner = AuthenticatedFakeRunner(
        [identity_payload(profile_sha256=None)]
    )
    with pytest.raises(worker.PrivacyZkX509WorkerErrorV1, match="is malformed"):
        make_controller(monkeypatch, tmp_path, runner)


def test_identity_rejects_zero_release_evidence_constituent(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    runner = AuthenticatedFakeRunner(
        [identity_payload(kat_proof_sha256="0" * 64)]
    )
    with pytest.raises(worker.PrivacyZkX509WorkerErrorV1, match="malformed digest"):
        make_controller(monkeypatch, tmp_path, runner)


def test_identity_rejects_explicitly_unavailable_isolation_before_use(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    runner = AuthenticatedFakeRunner(
        [
            identity_payload(
                ready=False,
                qualified=False,
                isolation_package_sha256=None,
            )
        ]
    )
    with pytest.raises(
        worker.PrivacyZkX509WorkerErrorV1,
        match="isolation launcher is unavailable",
    ):
        make_controller(monkeypatch, tmp_path, runner)
    assert len(runner.requests) == 1


def test_identity_rejects_inconsistent_qualified_isolation_evidence(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    runner = AuthenticatedFakeRunner(
        [identity_payload(qualified=False, ready=True, isolation_package_sha256=None)]
    )
    with pytest.raises(
        worker.PrivacyZkX509WorkerErrorV1,
        match="inconsistent qualified isolation evidence",
    ):
        make_controller(monkeypatch, tmp_path, runner)


def test_identity_rejects_isolation_package_not_bound_to_artifact(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    runner = AuthenticatedFakeRunner(
        [identity_payload(isolation_package_sha256="d" * 64)]
    )
    with pytest.raises(
        worker.PrivacyZkX509WorkerErrorV1,
        match="does not bind the reviewed artifact",
    ):
        make_controller(monkeypatch, tmp_path, runner)


def test_execute_uses_qualified_identity_and_returns_only_public_wire(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    public_request = tmp_path / "public-request.json"
    public_request.write_bytes(b'{"public":"request"}')
    absent_bundle = tmp_path / "owner-only-bundle.x5wb"
    transaction_hash = b"\x71" * 32
    proof_sha256 = b"\x72" * 32
    signed_transaction = b"versioned-signed-transaction"
    execute_response = b"".join(
        (
            b"\x00\x01",
            transaction_hash,
            proof_sha256,
            len(signed_transaction).to_bytes(4, "big"),
            signed_transaction,
        )
    )
    runner = AuthenticatedFakeRunner([identity_payload(), execute_response])
    controller = make_controller(monkeypatch, tmp_path, runner)
    action = controller.execute(
        public_request_path=public_request,
        secret_bundle_path=absent_bundle,
        secret_bundle_sha256=b"\x91" * 32,
    )
    assert action.transaction_hash == transaction_hash
    assert action.proof_sha256 == proof_sha256
    assert action.versioned_signed_transaction == signed_transaction
    assert not absent_bundle.exists()
    assert len(runner.requests) == 2
    request = json.loads(runner.requests[1][2])
    assert request == {
        "schema_version": 1,
        "public_request_path": str(public_request),
        "public_request_sha256": list(
            hashlib.sha256(public_request.read_bytes()).digest()
        ),
        "secret_bundle_path": str(absent_bundle),
        "secret_bundle_sha256": list(b"\x91" * 32),
    }


def test_admit_secret_bundle_authenticates_semantics_and_exact_inode(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    public_request = tmp_path / "public-request.json"
    public_request.write_bytes(b'{"public":"request"}')
    secret_bundle = tmp_path / "owner-only-bundle.x5wb"
    secret_bundle.write_bytes(b"X5WB\x01" + b"\x91" * 100)
    secret_bundle.chmod(0o600)
    bundle_digest = hashlib.sha256(secret_bundle.read_bytes()).digest()
    runner = AuthenticatedFakeRunner(
        [identity_payload(), admission_payload(public_request, secret_bundle)]
    )
    controller = make_controller(monkeypatch, tmp_path, runner)

    admission = controller.admit_secret_bundle(
        public_request_path=public_request,
        secret_bundle_path=secret_bundle,
        secret_bundle_sha256=bundle_digest,
    )

    metadata = secret_bundle.lstat()
    assert admission.public_request_sha256 == hashlib.sha256(
        public_request.read_bytes()
    ).digest()
    assert admission.secret_bundle_sha256 == bundle_digest
    assert (admission.device, admission.inode, admission.size) == (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
    )
    assert admission.mode == 0o600
    assert admission.owner == os.geteuid()
    assert runner.requests[1][0] == int(
        worker.PrivacyZkX509WorkerCommandV1.ADMIT_BUNDLE
    )
    assert json.loads(runner.requests[1][2]) == {
        "schema_version": 1,
        "public_request_path": str(public_request),
        "public_request_sha256": list(admission.public_request_sha256),
        "secret_bundle_path": str(secret_bundle),
        "secret_bundle_sha256": list(bundle_digest),
    }
    assert all(not any(raw_input) for raw_input in runner.raw_inputs)


def test_admit_secret_bundle_rejects_inode_replacement_after_native_receipt(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    public_request = tmp_path / "public-request.json"
    public_request.write_bytes(b'{"public":"request"}')
    secret_bundle = tmp_path / "owner-only-bundle.x5wb"
    secret_bundle.write_bytes(b"X5WB\x01" + b"\x91" * 100)
    secret_bundle.chmod(0o600)
    bundle_digest = hashlib.sha256(secret_bundle.read_bytes()).digest()
    runner = AuthenticatedFakeRunner(
        [identity_payload(), admission_payload(public_request, secret_bundle)]
    )
    original_run = runner.__call__

    def replace_after_receipt(
        args: list[str], **kwargs: object
    ) -> subprocess.CompletedProcess[bytes]:
        completed = original_run(args, **kwargs)
        if runner.requests[-1][0] == int(
            worker.PrivacyZkX509WorkerCommandV1.ADMIT_BUNDLE
        ):
            replacement = tmp_path / "replacement.x5wb"
            replacement.write_bytes(secret_bundle.read_bytes())
            replacement.chmod(0o600)
            os.replace(replacement, secret_bundle)
        return completed

    controller = make_controller(monkeypatch, tmp_path, runner)
    monkeypatch.setattr(worker.subprocess, "run", replace_after_receipt)
    with pytest.raises(
        worker.PrivacyZkX509WorkerErrorV1,
        match="changed or has invalid custody",
    ):
        controller.admit_secret_bundle(
            public_request_path=public_request,
            secret_bundle_path=secret_bundle,
            secret_bundle_sha256=bundle_digest,
        )


def test_authenticated_remote_error_is_typed_and_contains_no_detail(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    runner = AuthenticatedFakeRunner(
        [identity_payload(), bytes((1, 1, int(worker.PrivacyZkX509WorkerErrorCodeV1.WITNESS)))]
    )
    controller = make_controller(monkeypatch, tmp_path, runner)
    with pytest.raises(worker.PrivacyZkX509WorkerRemoteErrorV1) as error:
        controller._invoke(
            worker.PrivacyZkX509WorkerCommandV1.EXECUTE,
            b"{}",
        )
    assert error.value.code is worker.PrivacyZkX509WorkerErrorCodeV1.WITNESS
    assert str(error.value) == "zk-X509 worker rejected request (4)"


def test_bundle_writer_uses_qualified_identity_without_opening_secret_inputs(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    path, artifact_sha256 = executable(tmp_path)
    monkeypatch.setattr(worker.secrets, "token_bytes", deterministic_tokens)
    identity_runner = AuthenticatedFakeRunner([identity_payload()])
    calls = 0

    def fake_run(args: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
        nonlocal calls
        calls += 1
        if len(args) == 1:
            return identity_runner(args, **kwargs)
        assert len(args) == 6 and args[1] == "bundle"
        output = Path(args[5])
        output.write_bytes(b"opaque owner-only bundle")
        output.chmod(0o600)
        return subprocess.CompletedProcess(args, 0, b"\xa5" * 32, b"")

    monkeypatch.setattr(worker.subprocess, "run", fake_run)
    controller = worker.PrivacyZkX509WorkerControllerV1(
        path,
        expected_artifact_sha256=artifact_sha256,
        expected_source_commit=SOURCE_COMMIT,
        expected_source_sha256=SOURCE_SHA256,
        expected_compiled_profile_sha256=PROFILE_SHA256,
        expected_workspace_source_manifest_sha256=(
            WORKSPACE_SOURCE_MANIFEST_SHA256
        ),
    )
    public_request = tmp_path / "public.json"
    public_request.write_bytes(b'{"public":"request"}')
    seed = tmp_path / "seed.bin"
    witness = tmp_path / "witness.bin"
    output = tmp_path / "bundle.x5wb"
    receipt = controller.create_secret_bundle(
        public_request_path=public_request,
        signer_seed_path=seed,
        witness_path=witness,
        output_path=output,
    )
    assert receipt.path == output
    assert receipt.sha256 == b"\xa5" * 32
    assert calls == 2
    assert output.stat().st_mode & 0o777 == 0o600
    assert not seed.exists() and not witness.exists()
