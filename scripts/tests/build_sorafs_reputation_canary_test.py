"""Tests for scripts/build_sorafs_reputation_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_reputation_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_reputation_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location("build_sorafs_reputation_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reputation_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


NOW_UNIX = 1_800_100_000
GENERATED_AT = NOW_UNIX - 120
SNAPSHOT_ID = "11" * 16
MERKLE_ROOT = "22" * 32
WEIGHTS_DIGEST = "55" * 32
PROVIDER_ID = "provider-a"
PROOF_SIBLING = "33" * 32


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "sorafs-mainnet-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(NOW_UNIX),
        "--snapshot-id-hex",
        SNAPSHOT_ID,
        "--merkle-root-hex",
        MERKLE_ROOT,
        "--weights-digest-hex",
        WEIGHTS_DIGEST,
        "--provider-id",
        PROVIDER_ID,
        "--provider-name",
        "provider-a",
        "--provider-name",
        "provider-b",
    ]
    if kind == "provider":
        args.extend(["--sibling-hex", PROOF_SIBLING])
    if kind == "metrics":
        for metric in CHECKER.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    if kind == "transport":
        args.extend(["--sse-event", "reputation-sse-event-snapshot-00"])
        args.extend(
            ["--websocket-event", "reputation-websocket-event-snapshot-00"]
        )
    return args


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def write_payload(path: Path) -> Path:
    path.write_bytes(b"runtime fixture")
    return path


def raw_metrics() -> dict:
    return {
        "version": 1,
        "por_success_bps": 9_500,
        "pdp_success_bps": 9_400,
        "potr_success_bps": 9_300,
        "latency_health_bps": 9_200,
        "dispute_rate_bps": 100,
        "token_violation_rate_bps": 50,
        "repair_breach_rate_bps": 25,
    }


def raw_provider(provider_id: str, score_bps: int) -> dict:
    return {
        "provider_id": provider_id,
        "score_bps": score_bps,
        "degradation_flags": [],
        "raw_metrics": raw_metrics(),
        "raw_metrics_hash_hex": "66" * 32,
    }


def raw_latest() -> dict:
    return {
        "snapshot_id_hex": SNAPSHOT_ID,
        "generated_at_unix": GENERATED_AT,
        "previous_snapshot_id_hex": None,
        "merkle_root_hex": MERKLE_ROOT,
        "provider_count": 2,
        "returned_provider_count": 2,
        "limit": 100,
        "truncated_providers": False,
        "alpha_bps": 1_500,
        "current_score_weight_bps": 7_500,
        "weights": {
            "version": 1,
            "por_success_bps": 2_200,
            "pdp_success_bps": 2_000,
            "potr_success_bps": 1_800,
            "latency_bps": 1_500,
            "dispute_bps": 1_000,
            "token_violation_bps": 500,
            "repair_breach_bps": 1_000,
        },
        "providers": [
            raw_provider("provider-a", 9_400),
            raw_provider("provider-b", 8_800),
        ],
    }


def raw_provider_response() -> dict:
    return {
        "snapshot_id_hex": SNAPSHOT_ID,
        "generated_at_unix": GENERATED_AT,
        "merkle_root_hex": MERKLE_ROOT,
        "provider": raw_provider(PROVIDER_ID, 9_400),
        "proof": {
            "provider_id": PROVIDER_ID,
            "leaf_index": 0,
            "leaf_count": 2,
            "siblings_hex": [PROOF_SIBLING],
        },
    }


def raw_events() -> dict:
    return {
        "since": 0,
        "limit": 10,
        "count": 1,
        "next_since": 1,
        "events": [
            {
                "version": 1,
                "sequence": 1,
                "snapshot_id_hex": SNAPSHOT_ID,
                "generated_at_unix": GENERATED_AT,
                "merkle_root_hex": MERKLE_ROOT,
                "provider_count": 2,
                "previous_snapshot_id_hex": None,
            }
        ],
    }


def raw_verify(snapshot_path: Path, proof_path: Path) -> dict:
    return {
        "snapshot_path": str(snapshot_path),
        "snapshot_id_hex": SNAPSHOT_ID,
        "generated_at_unix": GENERATED_AT,
        "provider_count": 2,
        "merkle_root_hex": MERKLE_ROOT,
        "alpha_bps": 1_500,
        "current_score_weight_bps": 7_500,
        "valid": True,
        "provider_id": PROVIDER_ID,
        "provider_score_bps": 9_400,
        "proof_path": str(proof_path),
        "proof_leaf_index": 0,
        "proof_sibling_count": 1,
        "proof_verified": True,
    }


def source_args(
    kind: str,
    *,
    source: Path,
    publish: Path,
    out: Path,
    latest: Path | None = None,
    provider: Path | None = None,
    snapshot: Path | None = None,
    proof: Path | None = None,
) -> list[str]:
    args = [
        "--kind",
        kind,
        "--source-cli-json",
        str(source),
        "--publish-evidence",
        str(publish),
        "--out",
        str(out),
        "--now-unix",
        str(NOW_UNIX),
    ]
    if latest is not None:
        args.extend(["--latest-evidence", str(latest)])
    if kind in {"provider", "verify"}:
        args.extend(["--expected-provider-id", PROVIDER_ID])
    if kind == "events":
        args.extend(["--expected-since", "0", "--expected-limit", "10"])
    if kind == "verify":
        assert provider is not None and snapshot is not None and proof is not None
        args.extend(["--provider-evidence", str(provider)])
        args.extend(["--expected-snapshot-path", str(snapshot)])
        args.extend(["--expected-proof-path", str(proof)])
    return args


def test_source_bound_kind_inventory_is_exact() -> None:
    assert MODULE.SOURCE_BOUND_KINDS == frozenset(
        {"latest", "provider", "events", "verify"}
    )
    assert MODULE.MIN_REPUTATION_SCORE_BPS == 500
    assert MODULE.MAX_REPUTATION_SCORE_BPS == 9_900
    assert MODULE.MAX_REPUTATION_DEGRADATION_FLAGS == 5
    assert MODULE.REPUTATION_DEGRADATION_FLAGS == (
        "reserve_warning",
        "reserve_grace",
        "reserve_delinquent",
        "reserve_default",
        "proof_success_below90",
        "proof_success_below80",
        "active_dispute",
        "slashing_event",
        "low_score",
    )


def test_source_bound_mode_rejects_repeated_scalar_options(
    tmp_path: Path,
    capsys,
) -> None:
    publish = canary_path(tmp_path, "publish")
    assert MODULE.main(args_for("publish", tmp_path)) == 0
    source = write_json(tmp_path / "latest.raw", raw_latest())
    out = tmp_path / "latest-live.json"
    args = source_args(
        "latest",
        source=source,
        publish=publish,
        out=out,
    )
    args.extend(["--source-cli-json", str(source)])

    assert MODULE.main(args) == 2

    assert "scalar canary-builder options must not be repeated" in (
        capsys.readouterr().err
    )
    assert not out.exists()


def test_manual_mode_preserves_argfile_override_semantics(tmp_path: Path) -> None:
    args = args_for("metrics", tmp_path)
    override = tmp_path / "override.json"
    args.extend(["--out", str(override)])

    assert MODULE.main(args) == 0

    assert override.is_file()
    assert not canary_path(tmp_path, "metrics").exists()


def assert_rejected_without_artifact(
    args: list[str],
    *,
    kind: str,
    tmp_path: Path,
    capsys,
    expected_error: str,
) -> None:
    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, kind).exists()


def test_builds_payload_free_metrics_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("metrics", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "metrics").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reputation.metrics_canary.v1"
    assert payload["status"] == "passed"
    assert payload["metrics_scrape_success"] is True
    assert payload["provider_count"] == 2
    assert payload["providers"] == [{"name": "provider-a"}, {"name": "provider-b"}]
    assert payload["metric_count"] == len(CHECKER.REQUIRED_METRICS)
    assert payload["metrics"] == list(CHECKER.REQUIRED_METRICS)
    assert payload["response_bodies_included"] is False
    errors = MODULE.validate_generated_payload(payload, MODULE.parse_args(args_for("metrics", tmp_path)))
    assert errors == []


def test_builds_payload_free_transport_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("transport", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "transport").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reputation.transport_canary.v1"
    assert payload["sse_event_count"] == 1
    assert payload["sse_events"] == [{"name": "reputation-sse-event-snapshot-00"}]
    assert payload["websocket_event_count"] == 1
    assert payload["websocket_events"] == [
        {"name": "reputation-websocket-event-snapshot-00"}
    ]
    assert payload["response_bodies_included"] is False
    errors = MODULE.validate_generated_payload(
        payload,
        MODULE.parse_args(args_for("transport", tmp_path)),
    )
    assert errors == []


def test_generated_canaries_pass_full_reputation_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = ["--now-unix", str(NOW_UNIX), "--require-provider", PROVIDER_ID]
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["snapshot_id_hex"] == SNAPSHOT_ID
    assert payload["merkle_root_hex"] == MERKLE_ROOT
    assert payload["valid_reputation_weight_digests"] == [WEIGHTS_DIGEST]
    assert payload["provider_ids"] == [PROVIDER_ID]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_source_bound_cli_artifacts_pass_full_reputation_gate(tmp_path: Path) -> None:
    publish = canary_path(tmp_path, "publish")
    assert MODULE.main(args_for("publish", tmp_path)) == 0
    latest_source = write_json(tmp_path / "latest.raw", raw_latest())
    latest = tmp_path / "latest-live.json"
    assert MODULE.main(
        source_args(
            "latest",
            source=latest_source,
            publish=publish,
            out=latest,
        )
    ) == 0

    provider_source = write_json(tmp_path / "provider.raw", raw_provider_response())
    provider = tmp_path / "provider-live.json"
    assert MODULE.main(
        source_args(
            "provider",
            source=provider_source,
            publish=publish,
            latest=latest,
            out=provider,
        )
    ) == 0

    events_source = write_json(tmp_path / "events.raw", raw_events())
    events = tmp_path / "events-live.json"
    assert MODULE.main(
        source_args(
            "events",
            source=events_source,
            publish=publish,
            latest=latest,
            out=events,
        )
    ) == 0

    snapshot = write_payload(tmp_path / "snapshot.to")
    proof = write_payload(tmp_path / "proof.to")
    verify_source = write_json(
        tmp_path / "verify.raw",
        raw_verify(snapshot, proof),
    )
    verify = tmp_path / "verify-live.json"
    assert MODULE.main(
        source_args(
            "verify",
            source=verify_source,
            publish=publish,
            latest=latest,
            provider=provider,
            snapshot=snapshot,
            proof=proof,
            out=verify,
        )
    ) == 0

    external = []
    for kind in ("metrics", "transport", "consumption"):
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        external.append(canary_path(tmp_path, kind))
    summary = tmp_path / "source-summary.json"
    checker_args = [
        "--now-unix",
        str(NOW_UNIX),
        "--require-provider",
        PROVIDER_ID,
    ]
    for kind, path in (
        ("publish", publish),
        ("latest", latest),
        ("provider", provider),
        ("events", events),
        ("verify", verify),
        ("metrics", external[0]),
        ("transport", external[1]),
        ("consumption", external[2]),
    ):
        checker_args.extend(["--evidence", f"{kind}={path}"])
    checker_args.extend(["--summary-out", str(summary)])

    assert CHECKER.main(checker_args) == 0
    assert json.loads(summary.read_text(encoding="utf-8"))["status"] == "ready"
    assert json.loads(provider.read_text(encoding="utf-8"))["proof"]["leaf_index"] == 0


@pytest.mark.parametrize(
    ("kind", "mutation", "expected_error"),
    (
        (
            "latest",
            lambda payload: payload.__setitem__("provider_count", 3),
            "provider_count must match complete provider inventory",
        ),
        (
            "latest",
            lambda payload: payload.__setitem__("merkle_root_hex", "77" * 32),
            "merkle_root_hex must match publish",
        ),
    ),
)
def test_source_bound_latest_rejects_anchor_and_inventory_mismatches(
    kind: str,
    mutation,
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    del kind
    publish = canary_path(tmp_path, "publish")
    assert MODULE.main(args_for("publish", tmp_path)) == 0
    source_payload = raw_latest()
    mutation(source_payload)
    source = write_json(tmp_path / "latest.raw", source_payload)
    out = tmp_path / "latest-live.json"

    assert MODULE.main(
        source_args(
            "latest",
            source=source,
            publish=publish,
            out=out,
        )
    ) == 2

    assert expected_error in capsys.readouterr().err
    assert not out.exists()


def test_source_bound_latest_accepts_canonical_degradation_flags(
    tmp_path: Path,
) -> None:
    publish = canary_path(tmp_path, "publish")
    assert MODULE.main(args_for("publish", tmp_path)) == 0
    source_payload = raw_latest()
    source_payload["providers"][0]["degradation_flags"] = [
        {"flag": "reserve_warning", "value": None},
        {"flag": "active_dispute", "value": None},
    ]
    source = write_json(tmp_path / "latest.raw", source_payload)
    out = tmp_path / "latest-live.json"

    assert MODULE.main(
        source_args(
            "latest",
            source=source,
            publish=publish,
            out=out,
        )
    ) == 0


@pytest.mark.parametrize(
    ("score_bps", "flags", "expected_error"),
    (
        (
            499,
            [],
            "score_bps must be a canonical bounded integer",
        ),
        (
            9_901,
            [],
            "score_bps must be a canonical bounded integer",
        ),
        (
            9_400,
            [{"flag": "unknown", "value": None}],
            "degradation_flags must contain exact V1 flag objects",
        ),
        (
            9_400,
            [
                {"flag": "active_dispute", "value": None},
                {"flag": "reserve_warning", "value": None},
            ],
            "degradation_flags must be canonically sorted and unique",
        ),
        (
            9_400,
            [
                {"flag": "reserve_warning", "value": None},
                {"flag": "reserve_grace", "value": None},
                {"flag": "reserve_delinquent", "value": None},
                {"flag": "reserve_default", "value": None},
                {"flag": "proof_success_below90", "value": None},
                {"flag": "proof_success_below80", "value": None},
            ],
            "degradation_flags must contain at most 5 entries",
        ),
    ),
)
def test_source_bound_latest_rejects_noncanonical_provider_profiles(
    score_bps: int,
    flags: list[dict[str, object]],
    expected_error: str,
    tmp_path: Path,
    capsys,
) -> None:
    publish = canary_path(tmp_path, "publish")
    assert MODULE.main(args_for("publish", tmp_path)) == 0
    source_payload = raw_latest()
    source_payload["providers"][0]["score_bps"] = score_bps
    source_payload["providers"][0]["degradation_flags"] = flags
    source = write_json(tmp_path / "latest.raw", source_payload)
    out = tmp_path / "latest-live.json"

    assert MODULE.main(
        source_args(
            "latest",
            source=source,
            publish=publish,
            out=out,
        )
    ) == 2

    assert expected_error in capsys.readouterr().err
    assert not out.exists()


def test_source_bound_provider_rejects_context_and_weight_substitution(
    tmp_path: Path,
    capsys,
) -> None:
    publish = canary_path(tmp_path, "publish")
    assert MODULE.main(args_for("publish", tmp_path)) == 0
    latest_source = write_json(tmp_path / "latest.raw", raw_latest())
    latest = tmp_path / "latest-live.json"
    assert MODULE.main(
        source_args(
            "latest",
            source=latest_source,
            publish=publish,
            out=latest,
        )
    ) == 0
    tampered = json.loads(latest.read_text(encoding="utf-8"))
    tampered["environment"] = "staging"
    tampered["weights_digest_hex"] = "77" * 32
    latest.write_text(json.dumps(tampered), encoding="utf-8")
    provider_source = write_json(tmp_path / "provider.raw", raw_provider_response())
    provider = tmp_path / "provider-live.json"

    assert MODULE.main(
        source_args(
            "provider",
            source=provider_source,
            publish=publish,
            latest=latest,
            out=provider,
        )
    ) == 2

    stderr = capsys.readouterr().err
    assert "latest.environment must match publish.environment" in stderr
    assert "latest.weights_digest_hex must match publish.weights_digest_hex" in stderr
    assert not provider.exists()


def test_source_bound_verify_rejects_provider_score_substitution(
    tmp_path: Path,
    capsys,
) -> None:
    publish = canary_path(tmp_path, "publish")
    assert MODULE.main(args_for("publish", tmp_path)) == 0
    latest = tmp_path / "latest-live.json"
    assert MODULE.main(
        source_args(
            "latest",
            source=write_json(tmp_path / "latest.raw", raw_latest()),
            publish=publish,
            out=latest,
        )
    ) == 0
    provider = tmp_path / "provider-live.json"
    assert MODULE.main(
        source_args(
            "provider",
            source=write_json(tmp_path / "provider.raw", raw_provider_response()),
            publish=publish,
            latest=latest,
            out=provider,
        )
    ) == 0
    snapshot = write_payload(tmp_path / "snapshot.to")
    proof = write_payload(tmp_path / "proof.to")
    verify_payload = raw_verify(snapshot, proof)
    verify_payload["provider_score_bps"] = 9_399
    out = tmp_path / "verify-live.json"

    assert MODULE.main(
        source_args(
            "verify",
            source=write_json(tmp_path / "verify.raw", verify_payload),
            publish=publish,
            latest=latest,
            provider=provider,
            snapshot=snapshot,
            proof=proof,
            out=out,
        )
    ) == 2

    assert "provider score must match provider evidence" in capsys.readouterr().err
    assert not out.exists()


def test_publish_anchor_includes_weight_digest(tmp_path: Path) -> None:
    assert MODULE.main(args_for("publish", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "publish").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reputation.publish_snapshot_summary.v1"
    assert payload["weights_digest_hex"] == WEIGHTS_DIGEST


def test_weight_digest_is_required_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("latest", tmp_path)
    index = args.index("--weights-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--weights-digest-hex" in captured.err
    assert "required" in captured.err
    assert not canary_path(tmp_path, "latest").exists()


def test_weight_digest_must_be_lowercase_hex_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("latest", tmp_path)
    args[args.index("--weights-digest-hex") + 1] = "AB" * 32

    assert_rejected_without_artifact(
        args,
        kind="latest",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--weights-digest-hex must be exact lowercase hex length 64"
        ),
    )


def test_provider_id_must_be_canonical(tmp_path: Path, capsys) -> None:
    args = args_for("provider", tmp_path)
    args[args.index("--provider-id") + 1] = "provider_alpha"

    assert_rejected_without_artifact(
        args,
        kind="provider",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--provider-id must match canonical lowercase `provider-*`",
    )


def test_provider_id_rejects_non_production_markers(tmp_path: Path, capsys) -> None:
    args = args_for("provider", tmp_path)
    args[args.index("--provider-id") + 1] = "provider-prod-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="provider",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--provider-id must not contain non-production markers ['placeholder']"
        ),
    )


def test_provider_id_accepts_future_production_label(tmp_path: Path) -> None:
    provider_id = "provider-prod-a-202607"
    args = args_for("provider", tmp_path)
    args[args.index("--provider-id") + 1] = provider_id

    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "provider").read_text("utf-8"))
    assert payload["provider"]["provider_id"] == provider_id
    assert payload["proof"]["provider_id"] == provider_id


def test_response_file_can_build_provider_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "provider.args"
    args_file.write_text(
        "\n".join(args_for("provider", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "provider").read_text("utf-8"))
    assert payload["provider"]["provider_id"] == PROVIDER_ID
    assert payload["proof"]["siblings_hex"] == [PROOF_SIBLING]


def test_provider_requires_proof_sibling_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("provider", tmp_path)
    index = args.index("--sibling-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--sibling-hex is required for provider" in captured.err
    assert not canary_path(tmp_path, "provider").exists()


def test_duplicate_provider_proof_sibling_fails_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("provider", tmp_path)
    args.extend(["--sibling-hex", PROOF_SIBLING])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "duplicate --sibling-hex" in captured.err
    assert not canary_path(tmp_path, "provider").exists()


def test_invalid_provider_proof_sibling_does_not_seed_duplicate_tracking(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("provider", tmp_path)
    args.extend(["--sibling-hex", "aa" * 32])
    args.extend(["--sibling-hex", "AA" * 32])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--sibling-hex must be exact lowercase hex length 64" in captured.err
    assert "duplicate --sibling-hex" not in captured.err
    assert not canary_path(tmp_path, "provider").exists()


def test_metrics_thresholds_fail_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("metrics", tmp_path)
    args.extend(["--snapshot-age-seconds", "691201", "--ingest-lag-seconds", "901"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--snapshot-age-seconds must be <=" in captured.err
    assert "--ingest-lag-seconds must be <=" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_is_required_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("metrics", tmp_path)
    while "--provider-name" in args:
        index = args.index("--provider-name")
        del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider-name is required" in captured.err
    assert (
        "--provider-count must match the number of unique --provider-name values"
        in captured.err
    )
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_must_match_count_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("metrics", tmp_path)
    provider_count_index = args.index("--provider-count") if "--provider-count" in args else -1
    if provider_count_index == -1:
        args.extend(["--provider-count", "3"])
    else:
        args[provider_count_index + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--provider-count must match the number of unique --provider-name values"
        in captured.err
    )
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_must_not_duplicate_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("metrics", tmp_path)
    args.extend(["--provider-name", "provider-a"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider-name must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_must_use_provider_ids_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("metrics", tmp_path)
    args[args.index("--provider-name") + 1] = "provider_alpha"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--provider-name must match canonical lowercase `provider-*`"
        in captured.err
    )
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("metrics", tmp_path)
    args[args.index("--provider-name") + 1] = "provider-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--provider-name must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "metrics").exists()


def test_metrics_inventory_is_required_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("metrics", tmp_path)
    while "--metric" in args:
        index = args.index("--metric")
        del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric must include every required value" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


def test_metrics_inventory_must_not_duplicate_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("metrics", tmp_path)
    args.extend(["--metric", CHECKER.REQUIRED_METRICS[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


def test_metrics_inventory_must_not_include_unknown_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("metrics", tmp_path)
    args.extend(["--metric", "sorafs_reputation_unknown_metric_total"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric contains an unknown value" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


@pytest.mark.parametrize(
    ("option", "duplicate_value", "unknown_value"),
    (
        (
            "--metric",
            CHECKER.REQUIRED_METRICS[0],
            "sorafs_reputation_unknown_metric_total",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = args_for("metrics", tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        kind="metrics",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = args_for("metrics", unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind="metrics",
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_transport_sse_event_inventory_must_match_count_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args.extend(["--sse-event-count", "2"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--sse-event unique values must match --sse-event-count" in captured.err
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_sse_event_inventory_must_not_duplicate_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args.extend(["--sse-event", "reputation-sse-event-snapshot-00"])
    args.extend(["--sse-event-count", "2"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--sse-event must not contain duplicates" in captured.err
    assert "--sse-event unique values must match --sse-event-count" in captured.err
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_sse_event_inventory_requires_production_family_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args[args.index("--sse-event") + 1] = "sse-snapshot-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert MODULE.SSE_EVENT_LABEL_ERROR in captured.err
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_sse_event_inventory_rejects_placeholder_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args[args.index("--sse-event") + 1] = "reputation-sse-event-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--sse-event[0] must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_websocket_event_inventory_must_match_count_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args.extend(["--websocket-event-count", "2"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--websocket-event unique values must match --websocket-event-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_websocket_event_inventory_must_not_duplicate_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args.extend(["--websocket-event", "reputation-websocket-event-snapshot-00"])
    args.extend(["--websocket-event-count", "2"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--websocket-event must not contain duplicates" in captured.err
    assert (
        "--websocket-event unique values must match --websocket-event-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_websocket_event_inventory_requires_production_family_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args[args.index("--websocket-event") + 1] = "websocket-snapshot-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert MODULE.WEBSOCKET_EVENT_LABEL_ERROR in captured.err
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_websocket_event_inventory_rejects_placeholder_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args[args.index("--websocket-event") + 1] = (
        "reputation-websocket-event-placeholder"
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--websocket-event[0] must not contain non-production markers "
        "['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "transport").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("latest", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = tmp_path / "latest-output"
    output_dir.mkdir()
    args = args_for("latest", tmp_path)
    args[args.index("--out") + 1] = str(output_dir)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
