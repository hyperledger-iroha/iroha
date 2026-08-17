"""Tests for the fail-closed deployed-public Taira soak lease state."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat

import pytest

from scripts import seal_taira_release_controllers as release_controller
from scripts import taira_public_v2_24h_soak_state as lease_state


START_WALL_MS = 2_000_000_000_000
START_MONOTONIC_NS = 10_000_000_000


def digest(label: str) -> str:
    """Return one deterministic nonzero digest-shaped fixture value."""

    return hashlib.sha256(label.encode("ascii")).hexdigest()


def binding() -> lease_state.LeaseBinding:
    """Return one complete source/deploy/controller binding."""

    return lease_state.LeaseBinding(
        source_commit="1" * 40,
        dpn_validator_release_commit="2" * 40,
        cargo_lock_sha256=digest("Cargo.lock"),
        workspace_source_manifest_sha256=digest("workspace"),
        candidate_handoff_sha256=digest("candidate"),
        publication_handoff_sha256=digest("publication"),
        deploy_handoff_sha256=digest("deploy"),
        genesis_block_hash=digest("genesis-block"),
        deploy_end_height=97,
        deploy_end_block_hash=digest("deploy-end-block"),
        deployment_completed_at_unix_ms=START_WALL_MS - 60_000,
        controller_host_id="taira-controller-01",
        controller_installation_id="installation-0001",
        controller_sha256=digest("controller"),
        controller_signing_key_id="controller-signing-key-01",
        native_verifier_binary_sha256=digest("native-verifier-binary"),
        native_verifier_source_sha256=digest("native-verifier-source"),
    )


def producer_launch() -> lease_state.ProducerLaunch:
    """Return one native-verifier-bound slot-zero launch acknowledgement."""

    return lease_state.ProducerLaunch(
        soak_anchor_sha256=digest("soak-anchor"),
        anchor_observation_completed_at_unix_ms=START_WALL_MS - 1_000,
        producer_launch_sha256=digest("producer-launch"),
        producer_identity_sha256=digest("producer-identity"),
        producer_pid=4242,
        workload_started_at_unix_ms=START_WALL_MS,
        workload_started_monotonic_ns=START_MONOTONIC_NS,
    )


def start(
    root: Path,
    bound: lease_state.LeaseBinding | None = None,
    launched: lease_state.ProducerLaunch | None = None,
) -> lease_state.PublicSoakLease:
    """Create a lease using the complete default acknowledgement fixture."""

    return lease_state.start_lease(
        root, bound or binding(), launched or producer_launch()
    )


def private_root(tmp_path: Path, name: str = "lease") -> Path:
    """Create one empty owner-private lease root."""

    root = tmp_path / name
    root.mkdir(mode=0o700)
    root.chmod(0o700)
    return root


@pytest.fixture
def clock(monkeypatch: pytest.MonkeyPatch) -> dict[str, int]:
    """Install independently controllable wall and monotonic clocks."""

    current = {"wall": START_WALL_MS, "monotonic": START_MONOTONIC_NS}
    monkeypatch.setattr(lease_state, "_wall_clock_ms", lambda: current["wall"])
    monkeypatch.setattr(
        lease_state, "_monotonic_ns", lambda: current["monotonic"]
    )
    monkeypatch.setattr(lease_state.secrets, "token_bytes", lambda size: b"n" * size)
    return current


def advance(clock: dict[str, int], milliseconds: int) -> None:
    """Advance both test clocks by an identical duration."""

    clock["wall"] += milliseconds
    clock["monotonic"] += milliseconds * 1_000_000


def read_document(root: Path) -> dict[str, object]:
    """Read the canonical test state without bypassing production validation."""

    return lease_state.inspect_lease(root)


def rewrite_document(
    root: Path,
    mutate: object,
    *,
    refresh_self_digest: bool = True,
) -> None:
    """Apply one hostile state mutation while preserving canonical encoding."""

    state_path = root / lease_state.STATE_FILENAME
    document = json.loads(state_path.read_bytes())
    assert callable(mutate)
    mutate(document)
    if refresh_self_digest:
        document["state_sha256"] = lease_state._state_digest(document)
    state_path.write_bytes(lease_state._canonical_json(document))
    state_path.chmod(0o600)


def test_lease_happy_path_is_exact_atomic_and_terminal(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """A natural 24-hour run closes only after independent admission."""

    root = private_root(tmp_path)
    lease = start(root)
    running = read_document(root)
    assert running["phase"] == lease_state.PHASE_RUNNING
    assert running["profile"] == lease_state.PROFILE
    assert running["history"] == [
        {
            "sequence": 0,
            "phase": lease_state.PHASE_RUNNING,
            "recorded_at_unix_ms": START_WALL_MS,
            "elapsed_monotonic_ms": 0,
            "evidence_sha256": running["binding_sha256"],
        }
    ]
    assert stat.S_IMODE((root / lease_state.STATE_FILENAME).stat().st_mode) == 0o600
    assert stat.S_IMODE((root / lease_state.LOCK_FILENAME).stat().st_mode) == 0o600

    advance(clock, lease_state.DURATION_MS + 1_000)
    lease_state.record_capture(lease, digest("capture"))
    advance(clock, 250)
    lease_state.record_admission_pending(
        lease,
        completion_sha256=digest("completion"),
        authority_subject_sha256=digest("subject"),
        authority_envelope_sha256=digest("authority-envelope"),
    )
    advance(clock, 250)
    lease_state.record_completed(lease, digest("durable-admission"))

    completed = read_document(root)
    assert completed["phase"] == lease_state.PHASE_COMPLETED
    assert [event["phase"] for event in completed["history"]] == [
        lease_state.PHASE_RUNNING,
        lease_state.PHASE_CAPTURED,
        lease_state.PHASE_ADMISSION_PENDING,
        lease_state.PHASE_COMPLETED,
    ]
    assert completed["artifacts"] == {
        "capture_set_sha256": digest("capture"),
        "completion_sha256": digest("completion"),
        "authority_subject_sha256": digest("subject"),
        "authority_envelope_sha256": digest("authority-envelope"),
        "admission_receipt_sha256": digest("durable-admission"),
    }
    assert completed["failure"] is None
    with pytest.raises(lease_state.PublicSoakLeaseError, match="closed"):
        lease_state.record_failed(
            lease, code="controller_shutdown", evidence_sha256=digest("late")
        )


def test_state_library_is_not_misregistered_as_a_controller_operation() -> None:
    """Only a real workload runner may pull the state library into its closure."""

    relative = "scripts/taira_public_v2_24h_soak_state.py"
    assert relative not in release_controller.LINUX_FILES
    assert relative not in release_controller.MACOS_FILES
    assert relative not in release_controller.PYTHON_OPERATIONS.values()
    assert not hasattr(lease_state, "main")


def test_active_and_abandoned_leases_cannot_be_stolen(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """Neither a live lock nor a crash-left state has a resume path."""

    root = private_root(tmp_path)
    lease = start(root)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="live controller"):
        lease_state.inspect_quiescent_lease(root)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="already owns"):
        start(root)
    lease.close()
    assert lease_state.inspect_quiescent_lease(root)["phase"] == (
        lease_state.PHASE_RUNNING
    )
    with pytest.raises(lease_state.PublicSoakLeaseError, match="not empty"):
        start(root)
    assert read_document(root)["phase"] == lease_state.PHASE_RUNNING


@pytest.mark.parametrize(
    ("elapsed", "message"),
    [
        (lease_state.DURATION_MS - 1, "premature"),
        (
            lease_state.DURATION_MS + lease_state.CONFIRMATION_DRAIN_MS + 1,
            "missed",
        ),
    ],
)
def test_capture_rejects_early_or_late_completion(
    tmp_path: Path,
    clock: dict[str, int],
    elapsed: int,
    message: str,
) -> None:
    """Capture must close inside the exact workload plus drain window."""

    lease = start(private_root(tmp_path))
    advance(clock, elapsed)
    with pytest.raises(lease_state.PublicSoakLeaseError, match=message):
        lease_state.record_capture(lease, digest("capture"))
    assert read_document(lease.root)["phase"] == lease_state.PHASE_RUNNING
    lease.close()


def test_clock_divergence_fails_before_state_mutation(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """Wall-clock slewing cannot be hidden behind the monotonic schedule."""

    lease = start(private_root(tmp_path))
    clock["monotonic"] += lease_state.DURATION_MS * 1_000_000
    clock["wall"] += lease_state.DURATION_MS + lease_state.MAX_WALL_MONOTONIC_SKEW_MS + 1
    with pytest.raises(lease_state.PublicSoakLeaseError, match="clocks diverged"):
        lease_state.record_capture(lease, digest("capture"))
    assert read_document(lease.root)["phase"] == lease_state.PHASE_RUNNING
    lease.close()


def test_stale_or_unanchored_producer_launch_is_rejected(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """Lease time begins only from a fresh native-verified slot-zero launch."""

    stale = producer_launch()
    stale = lease_state.ProducerLaunch(
        **{
            **stale.__dict__,
            "workload_started_at_unix_ms": (
                START_WALL_MS - lease_state.MAX_LEASE_CREATION_DELAY_MS - 1
            ),
            "workload_started_monotonic_ns": (
                START_MONOTONIC_NS
                - (lease_state.MAX_LEASE_CREATION_DELAY_MS + 1) * 1_000_000
            ),
            "anchor_observation_completed_at_unix_ms": (
                START_WALL_MS - lease_state.MAX_LEASE_CREATION_DELAY_MS - 2
            ),
        }
    )
    with pytest.raises(lease_state.PublicSoakLeaseError, match="stale"):
        start(private_root(tmp_path, "stale"), launched=stale)

    unanchored = producer_launch()
    unanchored = lease_state.ProducerLaunch(
        **{
            **unanchored.__dict__,
            "anchor_observation_completed_at_unix_ms": (
                START_WALL_MS - lease_state.MAX_ANCHOR_TO_WORKLOAD_GAP_MS - 1
            ),
        }
    )
    with pytest.raises(lease_state.PublicSoakLeaseError, match="soak-anchor"):
        start(private_root(tmp_path, "unanchored"), launched=unanchored)


def test_lease_handle_cannot_cross_a_fork_boundary(
    tmp_path: Path,
    clock: dict[str, int],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """An inherited descriptor/nonce pair cannot transfer controller ownership."""

    lease = start(private_root(tmp_path))
    advance(clock, lease_state.DURATION_MS)
    monkeypatch.setattr(lease_state.os, "getpid", lambda: lease.owner_process_id + 1)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="process boundary"):
        lease_state.record_capture(lease, digest("capture"))
    lease.close()


@pytest.mark.parametrize(
    "phase",
    [
        lease_state.PHASE_RUNNING,
        lease_state.PHASE_CAPTURED,
        lease_state.PHASE_ADMISSION_PENDING,
    ],
)
def test_each_live_phase_can_fail_without_claiming_completion(
    tmp_path: Path, clock: dict[str, int], phase: str
) -> None:
    """Failure terminalization retains only evidence reached before failure."""

    root = private_root(tmp_path, phase)
    lease = start(root)
    if phase != lease_state.PHASE_RUNNING:
        advance(clock, lease_state.DURATION_MS)
        lease_state.record_capture(lease, digest("capture"))
    if phase == lease_state.PHASE_ADMISSION_PENDING:
        lease_state.record_admission_pending(
            lease,
            completion_sha256=digest("completion"),
            authority_subject_sha256=digest("subject"),
            authority_envelope_sha256=digest("envelope"),
        )
    lease_state.record_failed(
        lease, code="evidence_failed", evidence_sha256=digest("failure")
    )
    failed = read_document(root)
    assert failed["phase"] == lease_state.PHASE_FAILED
    assert failed["failure"] == {
        "code": "evidence_failed",
        "evidence_sha256": digest("failure"),
    }
    assert failed["artifacts"]["admission_receipt_sha256"] is None


def test_invalid_transition_does_not_mutate_running_state(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """Receipt/admission state cannot skip capture."""

    lease = start(private_root(tmp_path))
    before = (lease.root / lease_state.STATE_FILENAME).read_bytes()
    with pytest.raises(lease_state.PublicSoakLeaseError, match="captured"):
        lease_state.record_admission_pending(
            lease,
            completion_sha256=digest("completion"),
            authority_subject_sha256=digest("subject"),
            authority_envelope_sha256=digest("envelope"),
        )
    assert (lease.root / lease_state.STATE_FILENAME).read_bytes() == before
    lease.close()


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        (
            lambda state: state["profile"].__setitem__("duration_ms", 86_400_000.0),
            "integer|fixed public-soak",
        ),
        (
            lambda state: state.__setitem__("schema_version", 1.0),
            "schema version.*integer",
        ),
        (
            lambda state: state["binding"]["deployment"].__setitem__(
                "protocol_version", 4.0
            ),
            "integer",
        ),
        (
            lambda state: state["artifacts"].__setitem__(
                "completion_sha256", digest("premature")
            ),
            "premature",
        ),
        (
            lambda state: state.__setitem__("unexpected", "field"),
            "fields are not exact",
        ),
    ],
)
def test_semantic_tampering_fails_even_with_refreshed_self_digest(
    tmp_path: Path,
    clock: dict[str, int],
    mutation: object,
    message: str,
) -> None:
    """The state self-hash cannot turn malformed semantics into evidence."""

    root = private_root(tmp_path)
    lease = start(root)
    lease.close()
    rewrite_document(root, mutation)
    with pytest.raises(lease_state.PublicSoakLeaseError, match=message):
        lease_state.inspect_lease(root)


def test_unrefreshed_state_tamper_is_rejected_by_self_digest(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """A byte-level mutation cannot retain the authenticated state identity."""

    root = private_root(tmp_path)
    lease = start(root)
    lease.close()
    rewrite_document(
        root,
        lambda state: state["binding"]["controller"].__setitem__(
            "host_id", "taira-controller-02"
        ),
        refresh_self_digest=False,
    )
    with pytest.raises(lease_state.PublicSoakLeaseError, match="self-digest"):
        lease_state.inspect_lease(root)


def test_root_and_state_files_must_remain_private_and_unaliased(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """Mode drift and hard links are rejected without touching external files."""

    loose_root = private_root(tmp_path, "loose")
    loose_root.chmod(0o755)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="mode-0700"):
        start(loose_root)

    root = private_root(tmp_path, "hardlink")
    lease = start(root)
    lease.close()
    state_path = root / lease_state.STATE_FILENAME
    external = tmp_path / "external-state"
    external.write_bytes(state_path.read_bytes())
    external.chmod(0o600)
    state_path.unlink()
    os.link(external, state_path)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="single-link"):
        lease_state.inspect_lease(root)
    assert external.is_file()


def test_symbolic_root_and_unexpected_entries_are_rejected(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """The state root cannot be reached through a symlink or share contents."""

    real = private_root(tmp_path, "real")
    alias = tmp_path / "alias"
    alias.symlink_to(real, target_is_directory=True)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="symbolic link"):
        start(alias)

    root = private_root(tmp_path, "unexpected")
    (root / "foreign").write_text("not lease state", encoding="utf-8")
    with pytest.raises(lease_state.PublicSoakLeaseError, match="not empty"):
        start(root)


def test_lock_path_replacement_is_detected_before_transition(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """An unlinked held lock cannot authorize a replacement lock pathname."""

    root = private_root(tmp_path)
    lease = start(root)
    lock_path = root / lease_state.LOCK_FILENAME
    lock_path.unlink()
    lock_path.write_bytes(b"replacement")
    lock_path.chmod(0o600)
    advance(clock, lease_state.DURATION_MS)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="single-link|replaced"):
        lease_state.record_capture(lease, digest("capture"))
    lease.close()


def test_replacement_lock_cannot_forge_quiescent_inspection(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """An unlocked replacement inode cannot hide the still-held live lock."""

    root = private_root(tmp_path)
    lease = start(root)
    lock_path = root / lease_state.LOCK_FILENAME
    lock_path.unlink()
    lock_path.write_bytes(b"replacement")
    lock_path.chmod(0o600)
    with pytest.raises(
        lease_state.PublicSoakLeaseError,
        match="another lock inode|single-link|replaced",
    ):
        lease_state.inspect_quiescent_lease(root)
    lease.close()


def test_lock_replacement_during_transition_cannot_report_success(
    tmp_path: Path,
    clock: dict[str, int],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Every publication revalidates the held and named lock in its own cut."""

    root = private_root(tmp_path)
    lease = start(root)
    original_replace = lease_state._replace_state

    def replace_after_lock_swap(
        active: lease_state.PublicSoakLease,
        state: dict[str, object],
    ) -> None:
        lock_path = root / lease_state.LOCK_FILENAME
        lock_path.unlink()
        lock_path.write_bytes(b"replacement")
        lock_path.chmod(0o600)
        original_replace(active, state)

    monkeypatch.setattr(lease_state, "_replace_state", replace_after_lock_swap)
    advance(clock, lease_state.DURATION_MS)
    with pytest.raises(
        lease_state.PublicSoakLeaseError,
        match="single-link|lock path was replaced",
    ):
        lease_state.record_capture(lease, digest("capture"))
    state_path = root / lease_state.STATE_FILENAME
    assert json.loads(state_path.read_bytes())["phase"] == lease_state.PHASE_RUNNING
    lease.close()


def test_active_binding_rewrite_cannot_follow_the_process_lease(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """Recomputed public hashes cannot replace the process-bound release tuple."""

    root = private_root(tmp_path)
    lease = start(root)

    def substitute_controller(state: dict[str, object]) -> None:
        state["binding"]["controller"]["host_id"] = "taira-controller-02"
        state["binding_sha256"] = lease_state._domain_digest(
            lease_state.BINDING_DOMAIN, state["binding"]
        )
        state["history"][0]["evidence_sha256"] = state["binding_sha256"]

    rewrite_document(root, substitute_controller)
    advance(clock, lease_state.DURATION_MS)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="binding changed"):
        lease_state.record_capture(lease, digest("capture"))
    lease.close()


def test_renamed_root_cannot_continue_through_a_replacement_path(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """The held directory and its canonical controller pathname stay identical."""

    root = private_root(tmp_path)
    lease = start(root)
    moved = tmp_path / "moved-lease"
    root.rename(moved)
    root.mkdir(mode=0o700)
    root.chmod(0o700)
    advance(clock, lease_state.DURATION_MS)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="pathname was replaced"):
        lease_state.record_capture(lease, digest("capture"))
    lease.close()
    assert read_document(moved)["phase"] == lease_state.PHASE_RUNNING


def test_root_replacement_during_initial_publication_strands_the_attempt(
    tmp_path: Path,
    clock: dict[str, int],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Start reconfirms its pathname after the initial durable state publish."""

    root = private_root(tmp_path)
    moved = tmp_path / "moved-during-start"
    original_publish = lease_state._publish_state

    def publish_then_replace(
        root_fd: int, state: dict[str, object], *, initial: bool
    ) -> None:
        original_publish(root_fd, state, initial=initial)
        if initial:
            root.rename(moved)
            root.mkdir(mode=0o700)
            root.chmod(0o700)

    monkeypatch.setattr(lease_state, "_publish_state", publish_then_replace)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="pathname was replaced"):
        start(root)
    assert lease_state.inspect_quiescent_lease(moved)["phase"] == (
        lease_state.PHASE_RUNNING
    )


def test_quiescent_inspection_reconfirms_root_after_state_read(
    tmp_path: Path,
    clock: dict[str, int],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A rename during inspection cannot return a journal from the old path."""

    root = private_root(tmp_path)
    lease = start(root)
    lease.close()
    moved = tmp_path / "moved-during-inspection"
    original_read = lease_state._read_state

    def read_then_replace(root_fd: int) -> dict[str, object]:
        state = original_read(root_fd)
        root.rename(moved)
        root.mkdir(mode=0o700)
        root.chmod(0o700)
        return state

    monkeypatch.setattr(lease_state, "_read_state", read_then_replace)
    with pytest.raises(lease_state.PublicSoakLeaseError, match="pathname was replaced"):
        lease_state.inspect_quiescent_lease(root)


def test_deployment_must_precede_workload_start(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """A future deploy handoff cannot seed a public-soak lease."""

    fixture = binding()
    future = lease_state.LeaseBinding(
        **{
            **fixture.__dict__,
            "deployment_completed_at_unix_ms": START_WALL_MS + 1,
        }
    )
    with pytest.raises(
        lease_state.PublicSoakLeaseError,
        match="anchor predates deployment|before deployment",
    ):
        start(private_root(tmp_path), future)


def test_deployment_hashes_must_carry_the_iroha_marker(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    fixture = binding()
    unmarked = lease_state.LeaseBinding(
        **{**fixture.__dict__, "deploy_end_block_hash": "aa" * 32}
    )
    with pytest.raises(lease_state.PublicSoakLeaseError, match="marker bit"):
        start(private_root(tmp_path), unmarked)


def test_failure_code_and_digest_are_closed_inputs(
    tmp_path: Path, clock: dict[str, int]
) -> None:
    """Arbitrary failure prose and zero evidence identities are forbidden."""

    lease = start(private_root(tmp_path))
    with pytest.raises(lease_state.PublicSoakLeaseError, match="allow-listed"):
        lease_state.record_failed(
            lease, code="ignore-and-complete", evidence_sha256=digest("failure")
        )
    with pytest.raises(lease_state.PublicSoakLeaseError, match="allow-listed"):
        lease_state.record_failed(  # type: ignore[arg-type]
            lease, code=[], evidence_sha256=digest("failure")
        )
    with pytest.raises(lease_state.PublicSoakLeaseError, match="nonzero"):
        lease_state.record_failed(
            lease, code="workload_failed", evidence_sha256="0" * 64
        )
    assert read_document(lease.root)["phase"] == lease_state.PHASE_RUNNING
    lease.close()


def test_hostile_oversized_integer_decode_is_normalized() -> None:
    """The JSON implementation cannot leak a raw integer-conversion failure."""

    hostile = b'{"value":' + (b"9" * 10_000) + b"}\n"
    with pytest.raises(lease_state.PublicSoakLeaseError, match="strict JSON"):
        lease_state._decode_canonical(hostile)


def test_descheduled_clock_sampling_fails_closed(
    tmp_path: Path,
    clock: dict[str, int],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A scheduler pause is rejected instead of misclassified as clock drift."""

    lease = start(private_root(tmp_path))
    base = START_MONOTONIC_NS + lease_state.DURATION_MS * 1_000_000
    clock["wall"] += lease_state.DURATION_MS
    samples = iter(
        [
            base,
            base + 2_000_000_000,
            base + 3_000_000_000,
            base + 5_000_000_000,
            base + 6_000_000_000,
            base + 8_000_000_000,
        ]
    )
    monkeypatch.setattr(lease_state, "_monotonic_ns", lambda: next(samples))
    with pytest.raises(lease_state.PublicSoakLeaseError, match="sampled"):
        lease_state.record_capture(lease, digest("capture"))
    lease.close()
