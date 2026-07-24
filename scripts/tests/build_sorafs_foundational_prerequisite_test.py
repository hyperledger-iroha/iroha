"""Tests for the two-phase SoraFS foundational prerequisite builder."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shlex
import stat
import sys
from pathlib import Path

import pytest


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "build_sorafs_foundational_prerequisite.py"
)
SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_foundational_prerequisite",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

import check_sorafs_production_readiness as CHECKER  # noqa: E402
import sccp_release_common as RELEASE_CRYPTO  # noqa: E402


NOW_UNIX = 1_800_900_000
GENERATED_AT_UNIX = NOW_UNIX - 30
EVIDENCE_AT_UNIX = NOW_UNIX - 60
MAX_AGE_SECS = 3600
DEPLOYMENT_ID = "sorafs-mainnet-2026-07"
ENVIRONMENT = "production"
RELEASE_SEQUENCE = 1
PREDECESSOR_SHA256 = "00" * 32


def public_key_from_seed(seed: bytes) -> bytes:
    """Derive a temporary Ed25519 public key for one test invocation."""

    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    return RELEASE_CRYPTO._ed_encode(  # noqa: SLF001 - test-only signer
        RELEASE_CRYPTO._ed_scalar_multiply(  # noqa: SLF001
            RELEASE_CRYPTO._ED_BASE,  # noqa: SLF001
            scalar,
        )
    )


def sign(seed: bytes, message: bytes) -> bytes:
    """Sign with a temporary in-memory Ed25519 seed used only by tests."""

    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    prefix = digest[32:]
    public_key = public_key_from_seed(seed)
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little")
    nonce %= RELEASE_CRYPTO._ED_L  # noqa: SLF001
    encoded_r = RELEASE_CRYPTO._ed_encode(  # noqa: SLF001
        RELEASE_CRYPTO._ed_scalar_multiply(  # noqa: SLF001
            RELEASE_CRYPTO._ED_BASE,  # noqa: SLF001
            nonce,
        )
    )
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(),
        "little",
    ) % RELEASE_CRYPTO._ED_L  # noqa: SLF001
    scalar_bytes = (
        (nonce + challenge * scalar) % RELEASE_CRYPTO._ED_L  # noqa: SLF001
    ).to_bytes(32, "little")
    return encoded_r + scalar_bytes


@pytest.fixture
def signer() -> tuple[bytes, bytes]:
    """Return temporary signing material that is never written to disk."""

    seed = os.urandom(32)
    return seed, public_key_from_seed(seed)


def prerequisite_specs(
    *,
    evidence_at_unix: int = EVIDENCE_AT_UNIX,
) -> list[str]:
    """Return exact ordered temporary evidence-anchor arguments."""

    return [
        "{}:{}:{}".format(
            prerequisite_id,
            hashlib.sha256(
                f"temporary:{prerequisite_id}:evidence".encode("ascii")
            ).hexdigest(),
            evidence_at_unix,
        )
        for prerequisite_id in MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    ]


def lane_summary_paths(tmp_path: Path) -> list[tuple[str, Path]]:
    """Create exact temporary lane-summary bytes for the signing fixture."""

    rows: list[tuple[str, Path]] = []
    for gate_name in CHECKER.DEFAULT_REQUIRED_GATES:
        path = tmp_path / f"reviewed-{gate_name}.json"
        if not path.exists():
            payload = (
                gateway_load_summary()
                if gate_name == "gateway_load"
                else {
                    "schema": CHECKER.GATE_BY_NAME[gate_name].schema,
                    "status": "ready",
                }
            )
            path.write_text(
                json.dumps(payload, sort_keys=True),
                encoding="utf-8",
            )
        rows.append((gate_name, path))
    return rows


def prepare_args(
    tmp_path: Path,
    public_key: bytes,
    *,
    output_name: str = "foundational-signing-payload.bin",
    specs: list[str] | None = None,
    generated_at_unix: int = GENERATED_AT_UNIX,
    now_unix: int = NOW_UNIX,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    release_sequence: int = RELEASE_SEQUENCE,
    predecessor_sha256: str = PREDECESSOR_SHA256,
    previous_envelope_name: str | None = None,
) -> list[str]:
    """Build one complete prepare command."""

    values = [
        "prepare",
        "--deployment-id",
        deployment_id,
        "--environment",
        environment,
        "--generated-at-unix",
        str(generated_at_unix),
        "--now-unix",
        str(now_unix),
        "--max-evidence-age-secs",
        str(MAX_AGE_SECS),
        "--release-sequence",
        str(release_sequence),
        "--previous-envelope-sha256",
        predecessor_sha256,
        "--trusted-public-key-hex",
        public_key.hex(),
        "--signing-payload-out",
        str(tmp_path / output_name),
    ]
    if previous_envelope_name is not None:
        values.extend(
            ["--previous-envelope", str(tmp_path / previous_envelope_name)]
        )
    for spec in prerequisite_specs() if specs is None else specs:
        values.extend(["--prerequisite", spec])
    for gate_name, path in lane_summary_paths(tmp_path):
        values.extend(["--lane-summary", f"{gate_name}={path}"])
    return values


def finalize_args(
    tmp_path: Path,
    public_key: bytes,
    *,
    payload_name: str = "foundational-signing-payload.bin",
    signature_name: str = "foundational-signature.bin",
    output_name: str = "foundational-prerequisites.json",
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    release_sequence: int = RELEASE_SEQUENCE,
    predecessor_sha256: str = PREDECESSOR_SHA256,
    now_unix: int = NOW_UNIX,
    previous_envelope_name: str | None = None,
) -> list[str]:
    """Build one complete finalize command."""

    values = [
        "finalize",
        "--signing-payload",
        str(tmp_path / payload_name),
        "--signature-file",
        str(tmp_path / signature_name),
        "--trusted-public-key-hex",
        public_key.hex(),
        "--expected-deployment-id",
        deployment_id,
        "--expected-environment",
        environment,
        "--expected-release-sequence",
        str(release_sequence),
        "--expected-previous-envelope-sha256",
        predecessor_sha256,
        "--now-unix",
        str(now_unix),
        "--max-evidence-age-secs",
        str(MAX_AGE_SECS),
        "--envelope-out",
        str(tmp_path / output_name),
    ]
    if previous_envelope_name is not None:
        values.extend(
            ["--previous-envelope", str(tmp_path / previous_envelope_name)]
        )
    return values


def prepare_and_sign(
    tmp_path: Path,
    seed: bytes,
    public_key: bytes,
) -> bytes:
    """Prepare one request and write its temporary raw detached signature."""

    assert MODULE.main(prepare_args(tmp_path, public_key)) == 0
    payload = (tmp_path / "foundational-signing-payload.bin").read_bytes()
    (tmp_path / "foundational-signature.bin").write_bytes(sign(seed, payload))
    return payload


def decode_unsigned(payload: bytes) -> dict:
    """Decode the unsigned canonical body from one signing payload."""

    assert payload.startswith(MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN)
    return json.loads(
        payload[len(MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN) :]
    )


def write_signing_payload(path: Path, unsigned: dict) -> bytes:
    """Write one canonical binary signing payload used by negative tests."""

    payload = MODULE.foundational_signing_payload(unsigned)
    path.write_bytes(payload)
    return payload


def gateway_load_summary() -> dict:
    """Build one temporary ready lane summary for direct aggregate acceptance."""

    gate = CHECKER.GATE_BY_NAME["gateway_load"]
    required_rows: dict[str, dict] = {}
    for kind_name in gate.required_kinds:
        kind_schema = CHECKER.GATE_REQUIRED_KIND_SCHEMAS["gateway_load"][kind_name]
        fingerprint = {
            "generated_at_unix": GENERATED_AT_UNIX,
            "deployment_id": DEPLOYMENT_ID,
            "environment": ENVIRONMENT,
            "deployment_context_reviewed": True,
            "metric_count": len(CHECKER.GATEWAY_LOAD_REQUIRED_METRICS),
            "metrics": list(CHECKER.GATEWAY_LOAD_REQUIRED_METRICS),
            "policy_digest_hex": "ab" * 32,
            "staging_report_digest_hex": "bc" * 32,
            "suite_report_digest_hex": "cd" * 32,
        }
        required_rows[kind_name] = {
            "schema": kind_schema,
            "present": True,
            "valid": True,
            "artifact_count": 1,
            "artifacts": [
                {
                    "path": f"artifacts/gateway_load/{kind_name}.json",
                    "sha256": "de" * 32,
                    "schema": kind_schema,
                    "status": "passed",
                    "fingerprint": fingerprint,
                    "valid": True,
                    "errors": [],
                }
            ],
            "errors": [],
        }
    recognized_artifacts = []
    for kind_name, row in required_rows.items():
        artifact = dict(row["artifacts"][0])
        artifact["kind"] = kind_name
        recognized_artifacts.append(artifact)
    return {
        "schema": gate.schema,
        "status": "ready",
        "required_kinds": list(gate.required_kinds),
        "thresholds": {"max_evidence_bytes": 2_097_152},
        "evidence_file_count": len(gate.required_kinds),
        "recognized_artifact_count": len(gate.required_kinds),
        "recognized_artifacts": recognized_artifacts,
        "required": required_rows,
        "metric_count_values": [len(CHECKER.GATEWAY_LOAD_REQUIRED_METRICS)],
        "metrics": sorted(CHECKER.GATEWAY_LOAD_REQUIRED_METRICS),
        "valid_policy_digests": ["ab" * 32],
        "valid_staging_report_digests": ["bc" * 32],
        "valid_suite_report_digests": ["cd" * 32],
        "errors": [],
    }


def test_prepare_and_finalize_external_signer_roundtrip(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
) -> None:
    """Emit exact bytes, verify the HSM boundary, and pass the aggregate contract."""

    seed, public_key = signer
    signing_payload = prepare_and_sign(tmp_path, seed, public_key)
    assert stat.S_IMODE(
        (tmp_path / "foundational-signing-payload.bin").stat().st_mode
    ) == 0o600

    unsigned = decode_unsigned(signing_payload)
    assert set(unsigned) == MODULE.FOUNDATIONAL_PREREQUISITE_FIELDS
    assert set(unsigned["signature"]) == MODULE.UNSIGNED_SIGNATURE_FIELDS
    assert "signature_hex" not in unsigned["signature"]
    assert [row["id"] for row in unsigned["prerequisites"]] == list(
        MODULE.FOUNDATIONAL_PREREQUISITE_IDS
    )
    assert [row["gate"] for row in unsigned["lane_summaries"]] == list(
        CHECKER.DEFAULT_REQUIRED_GATES
    )
    assert signing_payload == MODULE.foundational_signing_payload(unsigned)

    assert MODULE.main(finalize_args(tmp_path, public_key)) == 0
    envelope_path = tmp_path / "foundational-prerequisites.json"
    envelope = json.loads(envelope_path.read_text(encoding="utf-8"))
    assert stat.S_IMODE(envelope_path.stat().st_mode) == 0o600
    assert envelope["signature"]["algorithm"] == "ed25519"
    assert bytes.fromhex(envelope["signature"]["signature_hex"]) == sign(
        seed,
        signing_payload,
    )
    assert MODULE.foundational_signing_payload(envelope) == signing_payload

    _summary, errors, context = CHECKER.validate_foundational_prerequisite_summary(
        envelope,
        CHECKER.ValidationOptions(
            now_unix=NOW_UNIX,
            max_summary_artifact_age_secs=MAX_AGE_SECS,
            deployment_id=DEPLOYMENT_ID,
            environment=ENVIRONMENT,
            foundational_signer_public_key=public_key,
            foundational_release_sequence=RELEASE_SEQUENCE,
            foundational_previous_envelope_sha256=PREDECESSOR_SHA256,
        ),
    )
    assert errors == []
    assert context == (DEPLOYMENT_ID, ENVIRONMENT)


def test_prepare_rejects_missing_and_reordered_lane_summary_inventory(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Fail closed unless all 17 exact summary files use canonical lane order."""

    _seed, public_key = signer
    missing = prepare_args(
        tmp_path,
        public_key,
        output_name="missing-lane.bin",
    )
    last_flag = len(missing) - 2
    assert missing[last_flag] == "--lane-summary"
    del missing[last_flag:]
    assert MODULE.main(missing) == 2
    assert "exactly 17 --lane-summary values are required" in capsys.readouterr().err

    reordered = prepare_args(
        tmp_path,
        public_key,
        output_name="reordered-lanes.bin",
    )
    value_indexes = [
        index + 1
        for index, value in enumerate(reordered)
        if value == "--lane-summary"
    ]
    reordered[value_indexes[0]], reordered[value_indexes[1]] = (
        reordered[value_indexes[1]],
        reordered[value_indexes[0]],
    )
    assert MODULE.main(reordered) == 2
    assert (
        "--lane-summary values must match all 17 readiness lanes in canonical order"
        in capsys.readouterr().err
    )
    assert not (tmp_path / "missing-lane.bin").exists()
    assert not (tmp_path / "reordered-lanes.bin").exists()


def test_later_sequence_requires_and_validates_immediate_signed_predecessor(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Bind sequence two to the exact verified sequence-one envelope bytes."""

    seed, public_key = signer
    previous_payload_name = "previous-signing-payload.bin"
    previous_signature_name = "previous-signature.bin"
    previous_envelope_name = "previous-envelope.json"
    previous_prepare = prepare_args(
        tmp_path,
        public_key,
        output_name=previous_payload_name,
        generated_at_unix=GENERATED_AT_UNIX - 15,
    )
    assert MODULE.main(previous_prepare) == 0
    previous_payload = (tmp_path / previous_payload_name).read_bytes()
    (tmp_path / previous_signature_name).write_bytes(sign(seed, previous_payload))
    assert (
        MODULE.main(
            finalize_args(
                tmp_path,
                public_key,
                payload_name=previous_payload_name,
                signature_name=previous_signature_name,
                output_name=previous_envelope_name,
            )
        )
        == 0
    )
    previous_bytes = (tmp_path / previous_envelope_name).read_bytes()
    previous_sha256 = hashlib.sha256(previous_bytes).hexdigest()

    current_prepare = prepare_args(
        tmp_path,
        public_key,
        output_name="current-signing-payload.bin",
        release_sequence=2,
        predecessor_sha256=previous_sha256,
        previous_envelope_name=previous_envelope_name,
    )
    assert MODULE.main(current_prepare) == 0
    current_payload = (tmp_path / "current-signing-payload.bin").read_bytes()
    (tmp_path / "current-signature.bin").write_bytes(sign(seed, current_payload))
    assert (
        MODULE.main(
            finalize_args(
                tmp_path,
                public_key,
                payload_name="current-signing-payload.bin",
                signature_name="current-signature.bin",
                output_name="current-envelope.json",
                release_sequence=2,
                predecessor_sha256=previous_sha256,
                previous_envelope_name=previous_envelope_name,
            )
        )
        == 0
    )

    missing_previous = prepare_args(
        tmp_path,
        public_key,
        output_name="missing-previous.bin",
        release_sequence=2,
        predecessor_sha256=previous_sha256,
    )
    assert MODULE.main(missing_previous) == 2
    assert "--previous-envelope is required" in capsys.readouterr().err

    wrong_digest = prepare_args(
        tmp_path,
        public_key,
        output_name="wrong-predecessor.bin",
        release_sequence=2,
        predecessor_sha256=hashlib.sha256(b"wrong-predecessor").hexdigest(),
        previous_envelope_name=previous_envelope_name,
    )
    assert MODULE.main(wrong_digest) == 2
    assert "does not match the reviewed predecessor" in capsys.readouterr().err

    forged = json.loads(previous_bytes)
    forged_signature = bytearray.fromhex(
        forged["signature"]["signature_hex"]
    )
    forged_signature[0] ^= 0x01
    forged["signature"]["signature_hex"] = forged_signature.hex()
    forged_name = "forged-previous-envelope.json"
    forged_bytes = MODULE.render_envelope(forged)
    (tmp_path / forged_name).write_bytes(forged_bytes)
    forged_prepare = prepare_args(
        tmp_path,
        public_key,
        output_name="forged-predecessor.bin",
        release_sequence=2,
        predecessor_sha256=hashlib.sha256(forged_bytes).hexdigest(),
        previous_envelope_name=forged_name,
    )
    assert MODULE.main(forged_prepare) == 2
    assert "signature verification failed" in capsys.readouterr().err


def test_finalized_envelope_is_accepted_by_direct_aggregate_gate(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
) -> None:
    """Pass the produced file through the real aggregate discovery path."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 0

    evidence_dir = tmp_path / "aggregate-evidence"
    evidence_dir.mkdir()
    (evidence_dir / "foundational-prerequisites.json").write_bytes(
        (tmp_path / "foundational-prerequisites.json").read_bytes()
    )
    (evidence_dir / "gateway-load.json").write_bytes(
        (tmp_path / "reviewed-gateway_load.json").read_bytes(),
    )
    aggregate_out = tmp_path / "aggregate-summary.json"
    assert (
        CHECKER.main(
            [
                "--evidence-dir",
                str(evidence_dir),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--max-summary-artifact-age-secs",
                str(MAX_AGE_SECS),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--foundational-prerequisite-signer-public-key-hex",
                public_key.hex(),
                "--foundational-prerequisite-release-sequence",
                str(RELEASE_SEQUENCE),
                "--foundational-prerequisite-previous-envelope-sha256",
                PREDECESSOR_SHA256,
                "--summary-out",
                str(aggregate_out),
            ]
        )
        == 0
    )
    aggregate = json.loads(aggregate_out.read_text(encoding="utf-8"))
    assert aggregate["status"] == "ready"
    assert aggregate["recognized_summary_count"] == 1
    assert aggregate["foundational_prerequisites"]["valid"] is True


def test_prepare_is_byte_deterministic_and_supports_reviewed_response_files(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
) -> None:
    """Equivalent direct and response-file inputs produce byte-identical payloads."""

    _seed, public_key = signer
    direct_args = prepare_args(
        tmp_path,
        public_key,
        output_name="direct-signing-payload.bin",
    )
    assert MODULE.main(direct_args) == 0

    response_args = prepare_args(
        tmp_path,
        public_key,
        output_name="response-signing-payload.bin",
    )
    response_path = tmp_path / "prepare-foundational.args"
    response_path.write_text(
        "\n".join(shlex.join([value]) for value in response_args) + "\n",
        encoding="utf-8",
    )
    assert MODULE.main([f"@{response_path}"]) == 0
    assert (tmp_path / "direct-signing-payload.bin").read_bytes() == (
        tmp_path / "response-signing-payload.bin"
    ).read_bytes()


@pytest.mark.parametrize(
    ("mutate", "expected"),
    [
        (
            lambda values: values[:-1],
            "exactly nine --prerequisite values are required",
        ),
        (
            lambda values: [values[0], values[0], *values[2:]],
            "must not contain duplicate ids",
        ),
        (
            lambda values: [values[1], values[0], *values[2:]],
            "canonical order",
        ),
        (
            lambda values: [
                values[0],
                values[1].replace("SF-1:", "SF-99:", 1),
                *values[2:],
            ],
            "contain unknown ids",
        ),
        (
            lambda values: [
                values[0].replace(values[0].split(":")[1], "00" * 32),
                *values[1:],
            ],
            "anchor must not be zero",
        ),
        (
            lambda values: [
                values[0].replace(
                    values[0].split(":")[1],
                    values[0].split(":")[1].upper(),
                ),
                *values[1:],
            ],
            "anchor must be canonical lowercase SHA-256",
        ),
        (
            lambda values: [
                values[0],
                "{}:{}:{}".format(
                    "SF-1",
                    values[0].split(":")[1],
                    EVIDENCE_AT_UNIX,
                ),
                *values[2:],
            ],
            "must use unique evidence anchors",
        ),
        (
            lambda values: [
                "{}:{}:{}".format(
                    values[0].split(":")[0],
                    values[0].split(":")[1],
                    NOW_UNIX - MAX_AGE_SECS - 1,
                ),
                *values[1:],
            ],
            "exceeds max summary artifact age",
        ),
        (
            lambda values: [
                "{}:{}:{}".format(
                    values[0].split(":")[0],
                    values[0].split(":")[1],
                    GENERATED_AT_UNIX + 1,
                ),
                *values[1:],
            ],
            "must not be later than the signed envelope",
        ),
    ],
)
def test_prepare_rejects_missing_duplicate_reordered_or_bad_anchors(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    mutate,
    expected: str,
) -> None:
    """Reject malformed foundational inventories before writing signable bytes."""

    _seed, public_key = signer
    args = prepare_args(tmp_path, public_key, specs=mutate(prerequisite_specs()))
    assert MODULE.main(args) == 2
    captured = capsys.readouterr()
    assert expected in captured.err
    assert not (tmp_path / "foundational-signing-payload.bin").exists()


@pytest.mark.parametrize(
    ("overrides", "expected"),
    [
        (
            {"generated_at_unix": NOW_UNIX + 1},
            "generated_at_unix must not be future",
        ),
        (
            {"generated_at_unix": NOW_UNIX - MAX_AGE_SECS - 1},
            "generated_at_unix exceeds max summary artifact age",
        ),
        (
            {"deployment_id": "sorafs-staging-2026-07"},
            "must not contain non-production deployment markers",
        ),
        (
            {"deployment_id": "bearer_token=runtime-only-value"},
            "must not contain secret-looking values",
        ),
        (
            {"environment": "development"},
            "environment must be production",
        ),
        (
            {
                "release_sequence": 1,
                "predecessor_sha256": hashlib.sha256(b"not-the-root").hexdigest(),
            },
            "requires the zero predecessor",
        ),
        (
            {"release_sequence": 2, "predecessor_sha256": "00" * 32},
            "requires a non-zero predecessor",
        ),
    ],
)
def test_prepare_rejects_bad_context_freshness_and_continuity(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    overrides: dict,
    expected: str,
) -> None:
    """Fail closed on non-production context, clock, and chain discontinuity."""

    _seed, public_key = signer
    assert MODULE.main(prepare_args(tmp_path, public_key, **overrides)) == 2
    captured = capsys.readouterr()
    assert expected in captured.err
    assert "runtime-only-value" not in captured.err


def test_prepare_rejects_untrusted_key_and_never_clobbers_output(
    tmp_path: Path,
    capsys,
) -> None:
    """Reject zero trust anchors and preserve an existing destination exactly."""

    output = tmp_path / "foundational-signing-payload.bin"
    output.write_bytes(b"preserve-me")
    assert MODULE.main(prepare_args(tmp_path, b"\x00" * 32)) == 2
    captured = capsys.readouterr()
    assert "must not be the all-zero key" in captured.err
    assert "must not already exist" in captured.err
    assert output.read_bytes() == b"preserve-me"


def test_prepare_rejects_symlink_parent_and_secret_looking_output(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Do not follow output links or render secret-looking path material."""

    _seed, public_key = signer
    target = tmp_path / "target"
    target.mkdir()
    linked = tmp_path / "linked"
    linked.symlink_to(target, target_is_directory=True)
    args = prepare_args(tmp_path, public_key)
    output_index = args.index("--signing-payload-out") + 1
    args[output_index] = str(linked / "payload.bin")
    assert MODULE.main(args) == 2
    captured = capsys.readouterr()
    assert "parent" in captured.err
    assert "must not be a symlink" in captured.err

    secret_args = prepare_args(
        tmp_path,
        public_key,
        output_name="private_key-material.bin",
    )
    assert MODULE.main(secret_args) == 2
    captured = capsys.readouterr()
    assert "canonical safe artifact path" in captured.err
    assert "private_key" not in captured.err


def test_prepare_detects_parent_swap_during_atomic_publication(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    monkeypatch,
    capsys,
) -> None:
    """Pin the output parent and remove the artifact if its pathname is swapped."""

    _seed, public_key = signer
    live_parent = tmp_path / "live"
    pinned_parent = tmp_path / "pinned"
    live_parent.mkdir()
    original_write = MODULE.write_all_checker_summary_bytes
    swapped = False

    def swap_parent_then_write(fd: int, payload: bytes) -> None:
        nonlocal swapped
        if not swapped:
            live_parent.rename(pinned_parent)
            live_parent.mkdir()
            swapped = True
        original_write(fd, payload)

    monkeypatch.setattr(
        MODULE,
        "write_all_checker_summary_bytes",
        swap_parent_then_write,
    )
    assert (
        MODULE.main(
            prepare_args(
                tmp_path,
                public_key,
                output_name="live/foundational-signing-payload.bin",
            )
        )
        == 2
    )
    assert "path changed during atomic publication" in capsys.readouterr().err
    assert not (live_parent / "foundational-signing-payload.bin").exists()
    assert not (pinned_parent / "foundational-signing-payload.bin").exists()


def test_bounded_input_read_detects_parent_swap(
    tmp_path: Path,
    monkeypatch,
) -> None:
    """Reject bytes read through a parent FD when the reviewed pathname moves."""

    live_parent = tmp_path / "live-input"
    pinned_parent = tmp_path / "pinned-input"
    live_parent.mkdir()
    source = live_parent / "signature.bin"
    source.write_bytes(b"\x01" * 64)
    original_read = MODULE.os.read
    swapped = False

    def swap_parent_then_read(fd: int, size: int) -> bytes:
        nonlocal swapped
        if not swapped:
            live_parent.rename(pinned_parent)
            live_parent.mkdir()
            (live_parent / "signature.bin").write_bytes(b"\x02" * 64)
            swapped = True
        return original_read(fd, size)

    monkeypatch.setattr(MODULE.os, "read", swap_parent_then_read)
    payload, errors = MODULE.read_bounded_regular_file(
        source,
        label="--signature-file",
        maximum_bytes=64,
    )
    assert payload is None
    assert errors == ["--signature-file path changed while it was read"]


def test_atomic_publication_removes_destination_when_parent_fsync_fails(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    monkeypatch,
    capsys,
) -> None:
    """Do not leave a publishable artifact after uncertain directory durability."""

    _seed, public_key = signer
    original_fsync = MODULE.os.fsync
    fsync_calls = 0

    def fail_parent_fsync(fd: int) -> None:
        nonlocal fsync_calls
        fsync_calls += 1
        if fsync_calls == 2:
            raise OSError("injected parent fsync failure")
        original_fsync(fd)

    monkeypatch.setattr(MODULE.os, "fsync", fail_parent_fsync)
    assert MODULE.main(prepare_args(tmp_path, public_key)) == 2
    assert "cannot be written" in capsys.readouterr().err
    assert not (tmp_path / "foundational-signing-payload.bin").exists()


@pytest.mark.parametrize(
    ("signature_factory", "expected", "exit_code"),
    [
        (lambda _payload: b"\x00" * 64, "all-zero signature", 2),
        (lambda _payload: b"\x01" * 63, "exactly 64 raw bytes", 2),
        (
            lambda payload: bytes([payload[0] ^ 1, *payload[1:64]]),
            "signature verification failed",
            1,
        ),
    ],
)
def test_finalize_rejects_zero_malformed_and_forged_signatures(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    signature_factory,
    expected: str,
    exit_code: int,
) -> None:
    """Only a strict detached Ed25519 signature can cross finalization."""

    _seed, public_key = signer
    assert MODULE.main(prepare_args(tmp_path, public_key)) == 0
    payload = (tmp_path / "foundational-signing-payload.bin").read_bytes()
    (tmp_path / "foundational-signature.bin").write_bytes(
        signature_factory(payload)
    )
    assert MODULE.main(finalize_args(tmp_path, public_key)) == exit_code
    captured = capsys.readouterr()
    assert expected in captured.err
    assert not (tmp_path / "foundational-prerequisites.json").exists()


def test_finalize_rejects_self_selected_key_and_expected_continuity_mismatch(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Bind the signature to both reviewed trust and continuity inputs."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    alternate_public_key = public_key_from_seed(os.urandom(32))
    assert MODULE.main(finalize_args(tmp_path, alternate_public_key)) == 2
    captured = capsys.readouterr()
    assert "fingerprint must match the operator-trusted key" in captured.err

    assert (
        MODULE.main(
            finalize_args(
                tmp_path,
                public_key,
                release_sequence=RELEASE_SEQUENCE + 1,
                predecessor_sha256=hashlib.sha256(
                    b"missing-reviewed-predecessor"
                ).hexdigest(),
            )
        )
        == 2
    )
    captured = capsys.readouterr()
    assert "--previous-envelope is required" in captured.err

    assert (
        MODULE.main(
            finalize_args(
                tmp_path,
                public_key,
                predecessor_sha256=hashlib.sha256(b"wrong").hexdigest(),
            )
        )
        == 2
    )
    captured = capsys.readouterr()
    assert "--expected-release-sequence 1 requires the zero predecessor" in (
        captured.err
    )


@pytest.mark.parametrize(
    ("payload_factory", "expected"),
    [
        (
            lambda unsigned: b"wrong-domain\x00"
            + MODULE.canonical_json_bytes(unsigned),
            "wrong signature domain",
        ),
        (
            lambda unsigned: MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN
            + json.dumps(unsigned, indent=2, sort_keys=True).encode("ascii"),
            "exact canonical encoding",
        ),
        (
            lambda _unsigned: (
                MODULE.FOUNDATIONAL_PREREQUISITE_SIGNATURE_DOMAIN
                + b'{"schema":"a","schema":"b"}'
            ),
            "strict and duplicate-free",
        ),
    ],
)
def test_finalize_rejects_wrong_domain_noncanonical_and_duplicate_json(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
    payload_factory,
    expected: str,
) -> None:
    """Reject altered encodings before detached signature verification."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    payload_path = tmp_path / "foundational-signing-payload.bin"
    unsigned = decode_unsigned(payload_path.read_bytes())
    payload_path.unlink()
    payload_path.write_bytes(payload_factory(unsigned))
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert expected in captured.err


def test_finalize_rejects_secret_fields_without_leaking_values(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Keep unknown sensitive fields and their values out of diagnostics."""

    seed, public_key = signer
    prepare_and_sign(tmp_path, seed, public_key)
    payload_path = tmp_path / "foundational-signing-payload.bin"
    unsigned = decode_unsigned(payload_path.read_bytes())
    secret_value = "bearer runtime-only-sensitive-material"
    unsigned["private_key"] = secret_value
    payload_path.unlink()
    write_signing_payload(payload_path, unsigned)
    (tmp_path / "foundational-signature.bin").write_bytes(
        sign(seed, payload_path.read_bytes())
    )
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "<sensitive-key>" in captured.err
    assert secret_value not in captured.err
    assert "private_key" not in captured.err


def test_finalize_rejects_symlinked_signature_and_preserves_existing_envelope(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Reject input symlinks and never replace an existing final envelope."""

    seed, public_key = signer
    payload = prepare_and_sign(tmp_path, seed, public_key)
    signature_path = tmp_path / "foundational-signature.bin"
    signature_path.unlink()
    signature_target = tmp_path / "signature-target.bin"
    signature_target.write_bytes(sign(seed, payload))
    signature_path.symlink_to(signature_target)
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "--signature-file must not be a symlink" in captured.err

    signature_path.unlink()
    signature_path.write_bytes(sign(seed, payload))
    envelope = tmp_path / "foundational-prerequisites.json"
    envelope.write_bytes(b"preserve-final-envelope")
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "--envelope-out must not already exist" in captured.err
    assert envelope.read_bytes() == b"preserve-final-envelope"


def test_finalize_rejects_symlinked_input_parent_hardlinks_and_writable_inputs(
    tmp_path: Path,
    signer: tuple[bytes, bytes],
    capsys,
) -> None:
    """Keep every public input behind the stable regular-file preflight."""

    seed, public_key = signer
    payload = prepare_and_sign(tmp_path, seed, public_key)

    target_parent = tmp_path / "input-target"
    target_parent.mkdir()
    (target_parent / "signature.bin").write_bytes(sign(seed, payload))
    linked_parent = tmp_path / "input-linked"
    linked_parent.symlink_to(target_parent, target_is_directory=True)
    args = finalize_args(tmp_path, public_key)
    args[args.index("--signature-file") + 1] = str(
        linked_parent / "signature.bin"
    )
    assert MODULE.main(args) == 2
    captured = capsys.readouterr()
    assert "--signature-file parent" in captured.err
    assert "must not be a symlink" in captured.err

    signature_path = tmp_path / "foundational-signature.bin"
    signature_path.unlink()
    hardlink_source = tmp_path / "signature-source.bin"
    hardlink_source.write_bytes(sign(seed, payload))
    os.link(hardlink_source, signature_path)
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "--signature-file must not be hardlinked" in captured.err

    signature_path.unlink()
    hardlink_source.unlink()
    signature_path.write_bytes(sign(seed, payload))
    signature_path.chmod(0o666)
    assert MODULE.main(finalize_args(tmp_path, public_key)) == 2
    captured = capsys.readouterr()
    assert "--signature-file must not be group- or world-writable" in captured.err


def test_cli_has_no_private_signing_key_input() -> None:
    """Keep every private-key option outside the builder boundary."""

    parser = MODULE.build_parser()
    options: set[str] = set()
    pending = [parser]
    while pending:
        current = pending.pop()
        for action in current._actions:  # noqa: SLF001 - parser contract test
            options.update(action.option_strings)
            choices = getattr(action, "choices", None)
            if isinstance(choices, dict):
                pending.extend(choices.values())
    assert not any("private" in option or "seed" in option for option in options)
