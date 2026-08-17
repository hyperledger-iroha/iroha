"""Tests for the fail-closed deployed-public Taira workload producer."""

from __future__ import annotations

from dataclasses import replace
import hashlib
import inspect
import json
import os
from pathlib import Path
import stat
from typing import NoReturn

import pytest

from scripts import check_taira_public_v2_24h_soak_evidence as checker
from scripts import run_taira_public_v2_24h_soak as runner
from scripts import taira_public_v2_24h_soak_state as lease_state


START_WALL_MS = 2_100_000_000_000
START_MONOTONIC_NS = 20_000_000_000


def digest(label: str) -> str:
    """Return one deterministic nonzero SHA-256-shaped value."""

    return hashlib.sha256(label.encode("ascii")).hexdigest()


def marked_digest(label: str) -> str:
    """Return one digest-shaped value carrying the Iroha marker bit."""

    value = bytearray(hashlib.sha256(label.encode("ascii")).digest())
    value[-1] |= 1
    return value.hex()


def private_root(tmp_path: Path, name: str) -> Path:
    """Create one owner-private empty root accepted by producer and lease code."""

    root = tmp_path / name
    root.mkdir(mode=0o700)
    root.chmod(0o700)
    return root


def captured(
    path: Path,
    payload: bytes,
    *,
    inode: int,
) -> checker.Artifact:
    """Construct one immutable-capture value for a private seam fixture."""

    return checker.Artifact(
        path=path,
        payload=payload,
        sha256=hashlib.sha256(payload).hexdigest(),
        size=len(payload),
        device=7,
        inode=inode,
    )


def context(tmp_path: Path) -> runner.PrerequisiteContext:
    """Return the minimum already-validated context consumed by the executor."""

    anchor_document = {
        "schema": "iroha.taira.public-v2-24h-soak-anchor.v1",
        "observation_started_at_unix_ms": START_WALL_MS - 2_000,
        "observation_completed_at_unix_ms": START_WALL_MS - 1_000,
        "controller_host_id": "taira-controller-01",
        "controller_installation_id": "installation-0001",
        "controller_sha256": digest("controller"),
        "controller_signing_key_id": digest("controller-signing-key"),
        "controller_receipt_sha256": digest("controller-receipt"),
        "controller_signature_sha256": digest("controller-signature"),
        "deploy_end_height": 97,
        "deploy_end_block_hash": {
            "algorithm": checker.IROHA_HASH_ALGORITHM,
            "type": checker.BLOCK_HASH_TYPE,
            "value": marked_digest("deploy-end"),
        },
        "common_start_block_evidence_index": 1,
        "validators": [],
    }
    anchor_payload = runner._canonical_json(anchor_document)
    producer_payload = b"protected-public-soak-producer\n"
    source = {
        "commit": "1" * 40,
        "dpn_validator_release_commit": "2" * 40,
        "cargo_lock_sha256": digest("Cargo.lock"),
        "workspace_source_manifest_sha256": digest("workspace"),
    }
    deploy = {
        "qualification_receipt_id": digest("qualification-receipt"),
        "admission_receipt_id": digest("admission-receipt"),
        "network_name": "taira",
        "chain_id": "fc56984b-2be7-431d-840e-21514d1883f0",
        "network_id": (
            "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"
        ),
        "protocol_version": 4,
        "genesis_block_hash": marked_digest("genesis"),
        "end_height": 97,
        "end_hash": marked_digest("deploy-end"),
        "deployment_completed_at_unix_ms": START_WALL_MS - 60_000,
        "controller_host_id": "taira-controller-01",
        "controller_installation_id": "installation-0001",
        "controller_sha256": digest("controller"),
        "restart_generation": digest("restart-generation"),
        "config_set_sha256": digest("config-set"),
        "topology_sha256": digest("topology"),
        "signed_genesis_sha256": digest("signed-genesis"),
        "supervisor_sha256": digest("supervisor"),
        "receipt_signers": {},
        "native_verifier_receipts": set(),
    }
    return runner.PrerequisiteContext(
        source=source,
        handoff_digests={
            "candidate": digest("candidate"),
            "publication": digest("publication"),
            "deploy": digest("deploy"),
        },
        deploy=deploy,
        candidate=captured(tmp_path / "candidate", b"candidate\n", inode=11),
        publication=captured(tmp_path / "publication", b"publication\n", inode=12),
        deploy_handoff=captured(tmp_path / "deploy", b"deploy\n", inode=13),
        anchor=captured(tmp_path / "anchor", anchor_payload, inode=14),
        anchor_document=anchor_document,
        producer=captured(tmp_path / "producer", producer_payload, inode=15),
        native_binary_sha256=digest("native-binary"),
        native_source_sha256=digest("native-source"),
        iroha3d_sha256=digest("iroha3d"),
    )


class FakeClock:
    """Clock that advances directly to every requested monotonic deadline."""

    def __init__(self) -> None:
        self.current_ns = START_MONOTONIC_NS

    def monotonic_ns(self) -> int:
        return self.current_ns

    def wall_ms(self) -> int:
        return START_WALL_MS + (self.current_ns - START_MONOTONIC_NS) // 1_000_000

    def sleep_until_ns(self, deadline_ns: int) -> None:
        self.current_ns = max(self.current_ns, deadline_ns)


class CountingBackend:
    """Native-backend-shaped fixture that stores no transactions or secrets."""

    def __init__(self, bound: runner.PrerequisiteContext, clock: FakeClock) -> None:
        self.bound = bound
        self.clock = clock
        self.count = 0
        self.first: tuple[int, int] | None = None
        self.last: tuple[int, int] | None = None
        self.closed_at: int | None = None
        self.seal_observer: object | None = None
        self.abort_reasons: list[str] = []
        self.launch_subject_sha256: str | None = None

    def launch(self, **kwargs: object) -> runner.NativeLaunch:
        assert kwargs["anchor_payload"] == self.bound.anchor.payload
        assert kwargs["anchor_sha256"] == self.bound.anchor.sha256
        assert kwargs["producer_identity_sha256"] == self.bound.producer.sha256
        subject_sha256 = kwargs["launch_subject_sha256"]
        assert isinstance(subject_sha256, str)
        subject_document, expected_subject_sha256 = runner._launch_subject(self.bound)
        assert subject_sha256 == expected_subject_sha256
        assert kwargs["launch_subject_payload"] == runner._canonical_json(
            subject_document
        )
        self.launch_subject_sha256 = subject_sha256
        receipt = {
            "schema": runner.LAUNCH_RECEIPT_SCHEMA,
            "schema_version": 1,
            "protocol": runner.LAUNCH_RECEIPT_PROTOCOL,
            "launch_subject_sha256": subject_sha256,
            "soak_anchor_sha256": self.bound.anchor.sha256,
            "producer_identity_sha256": self.bound.producer.sha256,
            "producer_pid": os.getpid(),
            "workload_started_at_unix_ms": START_WALL_MS,
            "workload_started_monotonic_ns": START_MONOTONIC_NS,
            "verifier_binary_sha256": self.bound.native_binary_sha256,
            "verifier_source_sha256": self.bound.native_source_sha256,
            "verification_result": "verified",
        }
        receipt_payload = runner._canonical_json(receipt)
        document = {
            "schema": runner.LAUNCH_ACK_SCHEMA,
            "schema_version": 1,
            "protocol": runner.LAUNCH_ACK_PROTOCOL,
            "launch_subject_sha256": subject_sha256,
            "soak_anchor_sha256": self.bound.anchor.sha256,
            "anchor_observation_completed_at_unix_ms": START_WALL_MS - 1_000,
            "producer_identity_sha256": self.bound.producer.sha256,
            "producer_pid": os.getpid(),
            "workload_started_at_unix_ms": START_WALL_MS,
            "workload_started_monotonic_ns": START_MONOTONIC_NS,
            "native_verifier_binary_sha256": self.bound.native_binary_sha256,
            "native_verifier_source_sha256": self.bound.native_source_sha256,
            "native_verifier_receipt_sha256": hashlib.sha256(
                receipt_payload
            ).hexdigest(),
            "native_verifier_receipt_size_bytes": len(receipt_payload),
            "verification_result": "verified",
        }
        return runner.NativeLaunch(
            acknowledgement=runner._canonical_json(document),
            native_verifier_receipt=receipt_payload,
        )

    def submit_slot(self, sequence: int, scheduled_elapsed_ms: int) -> None:
        assert sequence == self.count
        assert scheduled_elapsed_ms == sequence * runner.SLOT_INTERVAL_MS
        row = (sequence, scheduled_elapsed_ms)
        if self.first is None:
            self.first = row
        self.last = row
        self.count += 1

    def close_workload(self, scheduled_elapsed_ms: int) -> None:
        self.closed_at = scheduled_elapsed_ms

    def seal_capture(self, deadline_monotonic_ns: int) -> runner.NativeCaptureBundle:
        assert deadline_monotonic_ns == START_MONOTONIC_NS + (
            runner.DURATION_MS + runner.CONFIRMATION_DRAIN_MS
        ) * 1_000_000
        if callable(self.seal_observer):
            self.seal_observer()
        closure = {
            "schema": runner.BACKEND_CLOSURE_SCHEMA,
            "schema_version": 1,
            "protocol": runner.BACKEND_CLOSURE_PROTOCOL,
            "launch_subject_sha256": self.launch_subject_sha256,
            "workload_started_at_unix_ms": START_WALL_MS,
            "evidence_completed_at_unix_ms": START_WALL_MS + runner.DURATION_MS,
            "workload_closed_at_elapsed_ms": runner.DURATION_MS,
            "transfer_slot_count": runner.TRANSFER_SLOTS,
            "outstanding_submission_count": 0,
            "signer_session_closed": True,
            "verifier_binary_sha256": self.bound.native_binary_sha256,
            "verifier_source_sha256": self.bound.native_source_sha256,
            "verification_result": "verified",
        }
        return runner.NativeCaptureBundle(
            (),
            (),
            (),
            (),
            (),
            (),
            START_WALL_MS + runner.DURATION_MS,
            runner._canonical_json(closure),
        )

    def abort(self, reason_code: str) -> None:
        self.abort_reasons.append(reason_code)


def test_public_profile_is_exact_and_has_no_override_surface() -> None:
    """The public producer exposes the one release schedule, not tunable knobs."""

    assert runner.DURATION_MS == 86_400_000
    assert runner.SLOT_INTERVAL_MS == 200
    assert runner.TRANSFER_SLOTS == 432_000
    assert runner.TRANSFER_SLOTS * runner.SLOT_INTERVAL_MS == runner.DURATION_MS
    assert runner.CONFIRMATION_DRAIN_MS == 900_000
    help_text = runner.build_parser().format_help()
    for forbidden in (
        "--duration",
        "--slot-count",
        "--slot-interval",
        "--private-key",
        "--signing-key",
        "--skip",
        "--resume",
    ):
        assert forbidden not in help_text


def test_public_barrier_precedes_all_caller_path_and_network_work(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Unprovisioned production refuses without touching any supplied input."""

    def forbidden(*args: object, **kwargs: object) -> NoReturn:
        raise AssertionError(f"caller I/O occurred: {args!r} {kwargs!r}")

    monkeypatch.setattr(runner, "_load_prerequisites", forbidden)
    with pytest.raises(
        runner.PublicSoakRunnerError,
        match="disabled before caller path or network I/O",
    ):
        runner.run_public_soak(
            candidate_handoff=Path("/absent/candidate"),
            publication_handoff=Path("/absent/publication"),
            deploy_handoff=Path("/absent/deploy"),
            soak_anchor=Path("/absent/anchor"),
            state_root=Path("/absent/state"),
            capture_root=Path("/absent/capture"),
            native_producer_endpoint="unix:///absent/native.sock",
            expected_source={},
            iroha3d_sha256="attacker",
            native_verifier_binary_sha256="attacker",
            native_verifier_source_sha256="attacker",
        )


def test_cli_refuses_missing_inputs_before_path_inspection(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """The executable entry point has the same authority-first ordering."""

    monkeypatch.setattr(
        runner,
        "_load_prerequisites",
        lambda **_kwargs: (_ for _ in ()).throw(AssertionError("path I/O")),
    )
    result = runner.main(
        [
            "--candidate-handoff",
            "/absent/candidate",
            "--publication-handoff",
            "/absent/publication",
            "--deploy-handoff",
            "/absent/deploy",
            "--soak-anchor",
            "/absent/anchor",
            "--state-root",
            "/absent/state",
            "--capture-root",
            "/absent/capture",
            "--native-producer-endpoint",
            "unix:///absent/native.sock",
            "--source-commit",
            "1" * 40,
            "--dpn-validator-release-commit",
            "2" * 40,
            "--cargo-lock-sha256",
            "3" * 64,
            "--workspace-source-manifest-sha256",
            "4" * 64,
            "--iroha3d-sha256",
            "5" * 64,
            "--native-verifier-binary-sha256",
            "6" * 64,
            "--native-verifier-source-sha256",
            "7" * 64,
        ]
    )
    assert result == 1
    assert "disabled before caller path or network I/O" in capsys.readouterr().err


def transcript_record(
    sequence: int = 0,
    event: str = "producer-launched",
    elapsed_ms: int = 0,
) -> dict[str, object]:
    """Return one closed secret-free native transcript row."""

    return {
        "sequence": sequence,
        "event": event,
        "recorded_at_unix_ms": START_WALL_MS + elapsed_ms,
        "elapsed_monotonic_ms": elapsed_ms,
        "subject_sha256": digest(f"subject-{event}-{sequence}"),
        "native_receipt_sha256": digest(f"native-receipt-{event}-{sequence}"),
        "result": "verified",
    }


def test_canonical_inventory_publication_is_atomic_private_and_no_replace(
    tmp_path: Path,
) -> None:
    """A finalized capture file is canonical, private, and never overwritten."""

    root = private_root(tmp_path, "capture")
    output = runner._OutputRoot(root)
    try:
        artifact = output.publish_inventory(
            runner.TRANSCRIPT_FILENAME,
            [transcript_record()],
            kind="transcript",
            schema=runner.TRANSCRIPT_SCHEMA,
            fields=runner.TRANSCRIPT_RECORD_FIELDS,
            maximum_bytes=4096,
        )
        path = root / runner.TRANSCRIPT_FILENAME
        lines = path.read_bytes().splitlines(keepends=True)
        assert lines == [
            runner._canonical_json(
                {
                    "record_count": 1,
                    "schema": runner.TRANSCRIPT_SCHEMA,
                    "schema_version": 1,
                }
            ),
            runner._canonical_json(transcript_record()),
        ]
        assert artifact.sha256 == hashlib.sha256(path.read_bytes()).hexdigest()
        assert artifact.records_sha256 == hashlib.sha256(
            b"iroha.taira.public-v2-24h.transcript-records.v1\0" + lines[1]
        ).hexdigest()
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        assert not [entry for entry in root.iterdir() if entry.name.startswith(".")]
        before = path.read_bytes()
        with pytest.raises(runner.PublicSoakRunnerError, match="refusing overwrite"):
            output.publish_inventory(
                runner.TRANSCRIPT_FILENAME,
                [transcript_record()],
                kind="transcript",
                schema=runner.TRANSCRIPT_SCHEMA,
                fields=runner.TRANSCRIPT_RECORD_FIELDS,
                maximum_bytes=4096,
            )
        assert path.read_bytes() == before
    finally:
        output.close()


def test_inventory_encoder_streams_rows_into_the_atomic_publisher(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The 432,001 JSONL chunks are not duplicated in an encoded bytes list."""

    root = private_root(tmp_path, "streaming-capture")
    output = runner._OutputRoot(root)
    observed = {"was_list": True, "payload": b""}

    def consume(
        _self: runner._OutputRoot,
        _name: str,
        chunks: object,
        *,
        maximum_bytes: int,
    ) -> tuple[str, int]:
        assert maximum_bytes == 4096
        observed["was_list"] = isinstance(chunks, (list, tuple))
        observed["payload"] = b"".join(chunks)  # type: ignore[arg-type]
        return (
            hashlib.sha256(observed["payload"]).hexdigest(),
            len(observed["payload"]),
        )

    monkeypatch.setattr(runner._OutputRoot, "_publish_chunks", consume)
    try:
        artifact = output.publish_inventory(
            "streamed.jsonl",
            [transcript_record()],
            kind="transcript",
            schema=runner.TRANSCRIPT_SCHEMA,
            fields=runner.TRANSCRIPT_RECORD_FIELDS,
            maximum_bytes=4096,
        )
    finally:
        output.close()
    assert observed["was_list"] is False
    assert artifact.size_bytes == len(observed["payload"])


def test_transcript_contract_rejects_secret_or_unbounded_fields() -> None:
    """The retained transcript cannot grow a signing-material escape hatch."""

    secret = transcript_record()
    secret["private_key"] = "do-not-persist"
    with pytest.raises(runner.PublicSoakRunnerError, match="fields are not exact"):
        runner._validate_transcript([secret])
    wrong_event = transcript_record()
    wrong_event["event"] = "signed-transaction-body"
    with pytest.raises(runner.PublicSoakRunnerError, match="not allow-listed"):
        runner._validate_transcript([wrong_event])


def test_transcript_requires_natural_launch_window_and_drain_order() -> None:
    """A transcript closes every sample between launch and final drain closure."""

    records = [
        transcript_record(),
        transcript_record(1, "slot-batch-captured", 1_000),
        transcript_record(2, "sample-captured", runner.SAMPLE_INTERVAL_MS),
        transcript_record(3, "workload-window-closed", runner.DURATION_MS),
        transcript_record(4, "confirmation-drain-closed", runner.DURATION_MS),
    ]
    runner._validate_transcript(records, expected_sample_count=1)
    reordered = [dict(row) for row in records]
    reordered[-1], reordered[-2] = reordered[-2], reordered[-1]
    for sequence, row in enumerate(reordered):
        row["sequence"] = sequence
    with pytest.raises(runner.PublicSoakRunnerError, match="clocks regressed|lifecycle"):
        runner._validate_transcript(reordered, expected_sample_count=1)
    with pytest.raises(runner.PublicSoakRunnerError, match="every captured sample"):
        runner._validate_transcript(records, expected_sample_count=2)


def test_launch_acknowledgement_is_process_anchor_and_native_pin_bound(
    tmp_path: Path,
) -> None:
    """A structural launch cannot splice another process, anchor, or verifier."""

    bound = context(tmp_path)
    backend = CountingBackend(bound, FakeClock())
    _subject, subject_sha256 = runner._launch_subject(bound)
    native_launch = backend.launch(
        anchor_payload=bound.anchor.payload,
        anchor_sha256=bound.anchor.sha256,
        producer_identity_sha256=bound.producer.sha256,
        native_verifier_binary_sha256=bound.native_binary_sha256,
        native_verifier_source_sha256=bound.native_source_sha256,
        launch_subject_sha256=subject_sha256,
        launch_subject_payload=runner._canonical_json(_subject),
    )
    document, launch = runner._validate_launch_ack(
        native_launch, bound, subject_sha256
    )
    assert launch.soak_anchor_sha256 == bound.anchor.sha256
    assert launch.producer_launch_sha256 == hashlib.sha256(
        native_launch.acknowledgement
    ).hexdigest()
    assert launch.workload_started_monotonic_ns == START_MONOTONIC_NS
    mutant = json.loads(native_launch.acknowledgement)
    mutant["producer_pid"] = os.getpid() + 1
    with pytest.raises(runner.PublicSoakRunnerError, match="another process"):
        runner._validate_launch_ack(
            runner.NativeLaunch(
                runner._canonical_json(mutant),
                native_launch.native_verifier_receipt,
            ),
            bound,
            subject_sha256,
        )
    mutant = dict(document)
    mutant["soak_anchor_sha256"] = digest("alternate-anchor")
    with pytest.raises(runner.PublicSoakRunnerError, match="exact anchor"):
        runner._validate_launch_ack(
            runner.NativeLaunch(
                runner._canonical_json(mutant),
                native_launch.native_verifier_receipt,
            ),
            bound,
            subject_sha256,
        )


def test_launch_subject_closes_source_handoffs_deploy_binary_and_verifier(
    tmp_path: Path,
) -> None:
    """Any prerequisite splice changes the one domain-separated launch subject."""

    bound = context(tmp_path)
    subject, subject_sha256 = runner._launch_subject(bound)
    assert set(subject) == runner.LAUNCH_SUBJECT_FIELDS
    assert subject["prerequisites"] == {
        "candidate_handoff_sha256": bound.handoff_digests["candidate"],
        "publication_handoff_sha256": bound.handoff_digests["publication"],
        "deploy_handoff_sha256": bound.handoff_digests["deploy"],
    }
    assert subject["iroha3d_sha256"] == bound.iroha3d_sha256
    assert subject["deployment"]["receipt_signers"] == bound.deploy["receipt_signers"]

    alternate_source = dict(bound.source)
    alternate_source["commit"] = "9" * 40
    _document, alternate = runner._launch_subject(
        replace(bound, source=alternate_source)
    )
    assert alternate != subject_sha256
    alternate_handoffs = dict(bound.handoff_digests)
    alternate_handoffs["publication"] = digest("alternate-publication")
    _document, alternate = runner._launch_subject(
        replace(bound, handoff_digests=alternate_handoffs)
    )
    assert alternate != subject_sha256
    _document, alternate = runner._launch_subject(
        replace(bound, iroha3d_sha256=digest("alternate-iroha3d"))
    )
    assert alternate != subject_sha256
    _document, alternate = runner._launch_subject(
        replace(bound, native_binary_sha256=digest("alternate-native"))
    )
    assert alternate != subject_sha256


def test_launch_requires_exact_durable_native_receipt_bytes(tmp_path: Path) -> None:
    """A self-declared native receipt digest cannot replace the receipt artifact."""

    bound = context(tmp_path)
    _subject, subject_sha256 = runner._launch_subject(bound)
    backend = CountingBackend(bound, FakeClock())
    native_launch = backend.launch(
        anchor_payload=bound.anchor.payload,
        anchor_sha256=bound.anchor.sha256,
        producer_identity_sha256=bound.producer.sha256,
        native_verifier_binary_sha256=bound.native_binary_sha256,
        native_verifier_source_sha256=bound.native_source_sha256,
        launch_subject_sha256=subject_sha256,
        launch_subject_payload=runner._canonical_json(_subject),
    )
    with pytest.raises(runner.PublicSoakRunnerError, match="receipt is empty"):
        runner._validate_launch_ack(
            runner.NativeLaunch(native_launch.acknowledgement, b""),
            bound,
            subject_sha256,
        )
    tampered = bytearray(native_launch.native_verifier_receipt)
    tampered[-2] ^= 1
    with pytest.raises(runner.PublicSoakRunnerError, match="exact native receipt"):
        runner._validate_launch_ack(
            runner.NativeLaunch(native_launch.acknowledgement, bytes(tampered)),
            bound,
            subject_sha256,
        )
    ack = json.loads(native_launch.acknowledgement)
    ack["launch_subject_sha256"] = digest("alternate-launch-subject")
    with pytest.raises(runner.PublicSoakRunnerError, match="closed launch subject"):
        runner._validate_launch_ack(
            runner.NativeLaunch(
                runner._canonical_json(ack), native_launch.native_verifier_receipt
            ),
            bound,
            subject_sha256,
        )


def test_success_requires_exact_native_backend_closure_receipt(tmp_path: Path) -> None:
    """Capture cannot succeed while a signer session or submission remains live."""

    bound = context(tmp_path)
    backend = CountingBackend(bound, FakeClock())
    _subject, subject_sha256 = runner._launch_subject(bound)
    native_launch = backend.launch(
        anchor_payload=bound.anchor.payload,
        anchor_sha256=bound.anchor.sha256,
        producer_identity_sha256=bound.producer.sha256,
        native_verifier_binary_sha256=bound.native_binary_sha256,
        native_verifier_source_sha256=bound.native_source_sha256,
        launch_subject_sha256=subject_sha256,
        launch_subject_payload=runner._canonical_json(_subject),
    )
    launch, _producer = runner._validate_launch_ack(
        native_launch, bound, subject_sha256
    )
    bundle = backend.seal_capture(
        START_MONOTONIC_NS
        + (runner.DURATION_MS + runner.CONFIRMATION_DRAIN_MS) * 1_000_000
    )
    receipt = runner._validate_backend_closure(
        bundle.native_backend_closure_receipt,
        context=bound,
        launch=launch,
        completed_ms=bundle.evidence_completed_at_unix_ms,
    )
    assert receipt["signer_session_closed"] is True
    assert receipt["outstanding_submission_count"] == 0
    mutant = dict(receipt)
    mutant["signer_session_closed"] = False
    with pytest.raises(runner.PublicSoakRunnerError, match="claims are not exact"):
        runner._validate_backend_closure(
            runner._canonical_json(mutant),
            context=bound,
            launch=launch,
            completed_ms=bundle.evidence_completed_at_unix_ms,
        )


def test_anchor_and_samples_share_one_challenge_replay_set() -> None:
    """The structural checker cannot accept an anchor challenge in a sample."""

    source = inspect.getsource(runner._publish_capture)
    assert source.count("used_challenges=used_challenges") == 2
    anchor_call = source.index("checker._validate_anchor(")
    sample_call = source.index("checker._validate_samples(")
    declaration = source.index("used_challenges: set[str] = set()")
    assert declaration < anchor_call < sample_call


def test_backend_abort_covers_post_launch_start_failure_but_not_prelaunch_alias(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A launched signer is aborted, while a pre-launch refusal never contacts it."""

    bound = context(tmp_path)
    monkeypatch.setattr(runner, "_load_prerequisites", lambda **_kwargs: bound)
    state_root = private_root(tmp_path, "abort-state")
    capture_root = private_root(tmp_path, "abort-capture")
    backend = CountingBackend(bound, FakeClock())

    primary = RuntimeError("lease creation refused")

    def refuse_lease(*_args: object, **_kwargs: object) -> NoReturn:
        raise primary

    monkeypatch.setattr(lease_state, "start_lease", refuse_lease)
    with pytest.raises(RuntimeError) as failure:
        runner._run_with_native_backend(
            candidate_handoff=tmp_path / "ignored-candidate",
            publication_handoff=tmp_path / "ignored-publication",
            deploy_handoff=tmp_path / "ignored-deploy",
            soak_anchor=tmp_path / "ignored-anchor",
            state_root=state_root,
            capture_root=capture_root,
            expected_source={},
            iroha3d_sha256=digest("binary"),
            native_verifier_binary_sha256=bound.native_binary_sha256,
            native_verifier_source_sha256=bound.native_source_sha256,
            backend=backend,
            clock=FakeClock(),
        )
    assert failure.value is primary
    assert backend.abort_reasons == ["workload_failed"]

    alias_backend = CountingBackend(bound, FakeClock())
    alias_root = private_root(tmp_path, "alias-root")
    with pytest.raises(runner.PublicSoakRunnerError, match="must be distinct"):
        runner._run_with_native_backend(
            candidate_handoff=tmp_path / "ignored-candidate",
            publication_handoff=tmp_path / "ignored-publication",
            deploy_handoff=tmp_path / "ignored-deploy",
            soak_anchor=tmp_path / "ignored-anchor",
            state_root=alias_root,
            capture_root=alias_root,
            expected_source={},
            iroha3d_sha256=digest("binary"),
            native_verifier_binary_sha256=bound.native_binary_sha256,
            native_verifier_source_sha256=bound.native_source_sha256,
            backend=alias_backend,
            clock=FakeClock(),
        )
    assert alias_backend.launch_subject_sha256 is None
    assert alias_backend.abort_reasons == []


def test_abort_failure_surfaces_both_failures_and_preserves_primary_as_cause(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Abort diagnostics do not erase the failure that ended the attempt."""

    bound = context(tmp_path)
    monkeypatch.setattr(runner, "_load_prerequisites", lambda **_kwargs: bound)
    primary = ValueError("primary")
    monkeypatch.setattr(
        lease_state,
        "start_lease",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(primary),
    )

    class AbortFailureBackend(CountingBackend):
        def abort(self, reason_code: str) -> None:
            super().abort(reason_code)
            raise LookupError("abort")

    backend = AbortFailureBackend(bound, FakeClock())
    with pytest.raises(runner.PublicSoakRunnerError) as failure:
        runner._run_with_native_backend(
            candidate_handoff=tmp_path / "ignored-candidate",
            publication_handoff=tmp_path / "ignored-publication",
            deploy_handoff=tmp_path / "ignored-deploy",
            soak_anchor=tmp_path / "ignored-anchor",
            state_root=private_root(tmp_path, "cause-state"),
            capture_root=private_root(tmp_path, "cause-capture"),
            expected_source={},
            iroha3d_sha256=digest("binary"),
            native_verifier_binary_sha256=bound.native_binary_sha256,
            native_verifier_source_sha256=bound.native_source_sha256,
            backend=backend,
            clock=FakeClock(),
        )
    assert "ValueError" in str(failure.value)
    assert "LookupError" in str(failure.value)
    assert failure.value.__cause__ is primary


def test_private_executor_owns_one_lease_across_all_exact_slots_and_capture(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The lease remains live for all 432,000 slots and returns captured only."""

    bound = context(tmp_path)
    clock = FakeClock()
    backend = CountingBackend(bound, clock)
    state_root = private_root(tmp_path, "state")
    capture_root = private_root(tmp_path, "evidence")
    monkeypatch.setattr(runner, "_load_prerequisites", lambda **_kwargs: bound)
    monkeypatch.setattr(runner, "_replay_context", lambda _context: None)
    monkeypatch.setattr(lease_state, "_wall_clock_ms", clock.wall_ms)
    monkeypatch.setattr(lease_state, "_monotonic_ns", clock.monotonic_ns)
    monkeypatch.setattr(lease_state.secrets, "token_bytes", lambda size: b"n" * size)

    def assert_running() -> None:
        assert lease_state.inspect_lease(state_root)["phase"] == lease_state.PHASE_RUNNING

    backend.seal_observer = assert_running

    def publish_stub(**kwargs: object) -> tuple[runner.PublishedArtifact, dict[str, runner.PublishedArtifact]]:
        lease = kwargs["lease"]
        output = kwargs["output"]
        assert isinstance(lease, lease_state.PublicSoakLease)
        assert isinstance(output, runner._OutputRoot)
        assert lease_state.inspect_lease(state_root)["phase"] == lease_state.PHASE_RUNNING
        artifact = output.publish_bytes(
            runner.CAPTURE_FILENAME,
            runner._canonical_json({"captured": True}),
            kind="capture",
            schema=runner.CAPTURE_SCHEMA,
            maximum_bytes=4096,
        )
        return artifact, {}

    monkeypatch.setattr(runner, "_publish_capture", publish_stub)
    attempt = runner._run_with_native_backend(
        candidate_handoff=tmp_path / "ignored-candidate",
        publication_handoff=tmp_path / "ignored-publication",
        deploy_handoff=tmp_path / "ignored-deploy",
        soak_anchor=tmp_path / "ignored-anchor",
        state_root=state_root,
        capture_root=capture_root,
        expected_source={},
        iroha3d_sha256=digest("binary"),
        native_verifier_binary_sha256=bound.native_binary_sha256,
        native_verifier_source_sha256=bound.native_source_sha256,
        backend=backend,
        clock=clock,
    )
    assert backend.count == 432_000
    assert backend.first == (0, 0)
    assert backend.last == (431_999, 86_399_800)
    assert backend.closed_at == 86_400_000
    assert backend.abort_reasons == []
    state = lease_state.inspect_lease(state_root)
    assert state["phase"] == lease_state.PHASE_CAPTURED
    assert state["artifacts"]["capture_set_sha256"] == attempt.capture_set_sha256
    assert attempt.capture_handoff == capture_root / runner.CAPTURE_FILENAME
    assert not attempt.lease._closed
    launch_ack = json.loads(
        (capture_root / runner.LAUNCH_ACK_FILENAME).read_bytes()
    )
    launch_receipt = capture_root / runner.LAUNCH_RECEIPT_FILENAME
    assert launch_receipt.is_file()
    assert hashlib.sha256(launch_receipt.read_bytes()).hexdigest() == launch_ack[
        "native_verifier_receipt_sha256"
    ]
    assert not any(
        field in (capture_root / runner.CAPTURE_FILENAME).read_text()
        for field in ("private_key", "signing_secret", "bearer_token")
    )

    lease_state.record_failed(
        attempt.lease,
        code="admission_failed",
        evidence_sha256=digest("downstream-admission-failed"),
    )
    terminal = lease_state.inspect_quiescent_lease(state_root)
    assert terminal["phase"] == lease_state.PHASE_FAILED
    assert terminal["failure"]["code"] == "admission_failed"


def test_slot_dispatch_fails_on_late_or_early_monotonic_clock() -> None:
    """Neither early wakeups nor delayed slots can be normalized into evidence."""

    class Backend:
        def submit_slot(self, sequence: int, scheduled_elapsed_ms: int) -> None:
            raise AssertionError("invalid clock must fail before submission")

    class EarlyClock:
        def sleep_until_ns(self, deadline_ns: int) -> None:
            self.deadline = deadline_ns

        def monotonic_ns(self) -> int:
            return self.deadline - 1

    with pytest.raises(runner.PublicSoakRunnerError, match="before its monotonic"):
        runner._dispatch_exact_slots(Backend(), EarlyClock(), START_MONOTONIC_NS)  # type: ignore[arg-type]

    class LateClock(EarlyClock):
        def monotonic_ns(self) -> int:
            return self.deadline + 1_000_000_001

    with pytest.raises(runner.PublicSoakRunnerError, match="missed its fixed"):
        runner._dispatch_exact_slots(Backend(), LateClock(), START_MONOTONIC_NS)  # type: ignore[arg-type]
