"""Tests for scripts/build_sorafs_gateway_compliance_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_gateway_compliance_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_gateway_compliance_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_gateway_compliance_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_gateway_compliance_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


DIGEST = "a" * 64
TOGGLE_DIGEST = "b" * 64
POLICY_DIGEST = "c" * 64
GENERATED_AT = 1_800_100_000


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def checker_options() -> object:
    return CHECKER.ValidationOptions(
        now_unix=GENERATED_AT,
        max_evidence_age_secs=CHECKER.DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=CHECKER.DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_reload_latency_ms=CHECKER.DEFAULT_MAX_RELOAD_LATENCY_MS,
        min_gateways=CHECKER.DEFAULT_MIN_GATEWAYS,
        min_denylist_entries=CHECKER.DEFAULT_MIN_DENYLIST_ENTRIES,
        min_honey_probes=CHECKER.DEFAULT_MIN_HONEY_PROBES,
    )


def canary_path(tmp_path: Path, kind: str) -> Path:
    """Return the output path used by the test arguments for a canary kind."""

    if kind == "controller_runtime":
        return tmp_path / "controller-runtime.json"
    if kind == "moderation_toggle":
        return tmp_path / "moderation-toggle.json"
    return tmp_path / f"{kind}.json"


def feed_promotion() -> dict:
    return {
        "schema": "sorafs.gateway_compliance.feed_promotion_canary.v1",
        "status": "passed",
        "deployment_id": "sorafs-gateway-prod-20260701",
        "environment": "production",
        "deployment_context_reviewed": True,
        "generated_at_unix": GENERATED_AT,
        "external_feeds_normalized": True,
        "feed_signature_verified": True,
        "bundle_pack_verified": True,
        "bundle_diff_reviewed": True,
        "merkle_root_bound": True,
        "update_history_persisted": True,
        "gateway_ack_count": CHECKER.DEFAULT_MIN_GATEWAYS,
        "gateways": [
            {"name": f"gateway-compliance-gateway-{index:02d}"}
            for index in range(CHECKER.DEFAULT_MIN_GATEWAYS)
        ],
        "denylist_entry_count": CHECKER.DEFAULT_MIN_DENYLIST_ENTRIES,
        "denylist_entries": [
            {"name": f"gateway-denylist-entry-{index:02d}"}
            for index in range(CHECKER.DEFAULT_MIN_DENYLIST_ENTRIES)
        ],
        "bundle_digest_hex": DIGEST,
        "policy_digest_hex": POLICY_DIGEST,
        "raw_feeds_included": False,
        "feed_payloads_included": False,
    }


def controller_args(tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        "controller_runtime",
        "--out",
        str(tmp_path / "controller-runtime.json"),
        "--deployment-id",
        "sorafs-gateway-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
        "--bundle-digest-hex",
        DIGEST,
        "--controller-instance-id",
        "gateway-compliance-controller-prod-a",
        "--feed-count",
        "7",
    ]
    for name in (
        "ofac",
        "eu-sanctions",
        "malware",
        "csam-hash",
        "legal-hold",
        "regional-blocklist",
        "appeal-overrides",
    ):
        args.extend(["--feed", name])
    for value in MODULE.CONTROLLER_TRUE_CLAIMS:
        args.extend(["--verified-claim", value])
    return args


def moderation_args(tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        "moderation_toggle",
        "--out",
        str(tmp_path / "moderation-toggle.json"),
        "--deployment-id",
        "sorafs-gateway-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
        "--bundle-digest-hex",
        DIGEST,
        "--toggle-api-url",
        "https://gateway-compliance.internal/toggles",
        "--toggle-count",
        "4",
        "--toggle-digest-hex",
        TOGGLE_DIGEST,
    ]
    for name in (
        "provider-deny",
        "appeal-override",
        "legal-hold",
        "regional-emergency",
    ):
        args.extend(["--toggle", name])
    for value in MODULE.MODERATION_TRUE_CLAIMS:
        args.extend(["--verified-claim", value])
    return args


def args_for(kind: str, tmp_path: Path) -> list[str]:
    """Build reviewed arguments for one gateway-compliance canary kind."""

    if kind == "controller_runtime":
        return controller_args(tmp_path)
    if kind == "moderation_toggle":
        return moderation_args(tmp_path)

    args = [
        "--kind",
        kind,
        "--out",
        str(tmp_path / f"{kind}.json"),
        "--deployment-id",
        "sorafs-gateway-prod-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(GENERATED_AT),
        "--bundle-digest-hex",
        DIGEST,
    ]
    if kind in {"feed_promotion", "governance_approval"}:
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    if kind == "enforcement_probe":
        args.extend(["--route-body-blake3-hex", DIGEST])
        for reason in MODULE.REQUIRED_DENIAL_REASONS:
            args.extend(["--denial-reason", reason])
    elif kind == "honey_audit":
        args.extend(["--audit-digest-hex", DIGEST])
    elif kind == "appeal_override":
        args.extend(["--override-digest-hex", DIGEST])
    elif kind == "transparency_publication":
        args.extend(["--publication-digest-hex", DIGEST])
    elif kind == "observability":
        for metric in MODULE.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    return args


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


def test_builds_payload_free_controller_runtime_canary(tmp_path: Path) -> None:
    assert MODULE.main(controller_args(tmp_path)) == 0

    payload = json.loads((tmp_path / "controller-runtime.json").read_text("utf-8"))

    assert payload["schema"] == "sorafs.gateway_compliance.controller_runtime_canary.v1"
    assert payload["config_source"] == "iroha_config"
    assert payload["external_feed_count"] == 7
    assert payload["fetched_feed_count"] == 7
    assert payload["normalized_feed_count"] == 7
    assert payload["signed_feed_count"] == 7
    assert payload["feeds"] == [
        {"name": "ofac"},
        {"name": "eu-sanctions"},
        {"name": "malware"},
        {"name": "csam-hash"},
        {"name": "legal-hold"},
        {"name": "regional-blocklist"},
        {"name": "appeal-overrides"},
    ]
    for claim in MODULE.CONTROLLER_TRUE_CLAIMS:
        assert payload[claim] is True
    for claim in MODULE.FORBIDDEN_PAYLOAD_CLAIMS["controller_runtime"]:
        assert payload[claim] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "controller_runtime"
    assert errors == []


def test_controller_runtime_requires_complete_required_feeds(
    tmp_path: Path,
    capsys,
) -> None:
    args = controller_args(tmp_path)
    index = args.index("--feed")
    del args[index : index + 2]
    args[args.index("--feed-count") + 1] = "6"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--feed must include every required value" in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_controller_runtime_feed_count_must_match_required_feeds(
    tmp_path: Path,
    capsys,
) -> None:
    args = controller_args(tmp_path)
    args[args.index("--feed-count") + 1] = "6"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--feed-count must match the number of required unique --feed values"
        in captured.err
    )
    assert not (tmp_path / "controller-runtime.json").exists()


def test_controller_instance_id_must_be_canonical(tmp_path: Path, capsys) -> None:
    args = controller_args(tmp_path)
    args[args.index("--controller-instance-id") + 1] = (
        "gateway_compliance_controller_prod_a"
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--controller-instance-id must match canonical lowercase "
        "`gateway-compliance-controller-name`"
    ) in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_controller_instance_id_rejects_generic_controller_family(
    tmp_path: Path,
    capsys,
) -> None:
    args = controller_args(tmp_path)
    args[args.index("--controller-instance-id") + 1] = "compliance-controller-prod-a"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--controller-instance-id must match canonical lowercase "
        "`gateway-compliance-controller-name`"
    ) in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_controller_instance_id_rejects_non_production_markers(
    tmp_path: Path,
    capsys,
) -> None:
    args = controller_args(tmp_path)
    args[args.index("--controller-instance-id") + 1] = (
        "gateway-compliance-controller-prod-placeholder"
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--controller-instance-id must not contain non-production markers "
        "['placeholder']"
    ) in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_controller_instance_id_accepts_gateway_prefixed_future_label(
    tmp_path: Path,
) -> None:
    args = controller_args(tmp_path)
    args[args.index("--controller-instance-id") + 1] = (
        "gateway-compliance-controller-prod-b-2"
    )

    assert MODULE.main(args) == 0

    payload = json.loads((tmp_path / "controller-runtime.json").read_text("utf-8"))
    assert payload["controller_instance_id"] == "gateway-compliance-controller-prod-b-2"


def test_builds_payload_free_moderation_toggle_canary(tmp_path: Path) -> None:
    assert MODULE.main(moderation_args(tmp_path)) == 0

    payload = json.loads((tmp_path / "moderation-toggle.json").read_text("utf-8"))

    assert payload["schema"] == "sorafs.gateway_compliance.moderation_toggle_canary.v1"
    assert payload["config_source"] == "iroha_config"
    assert payload["toggle_count"] == 4
    assert payload["approved_toggle_count"] == 4
    assert payload["toggles"] == [
        {"name": "provider-deny"},
        {"name": "appeal-override"},
        {"name": "legal-hold"},
        {"name": "regional-emergency"},
    ]
    assert payload["toggle_digest_hex"] == TOGGLE_DIGEST
    for claim in MODULE.MODERATION_TRUE_CLAIMS:
        assert payload[claim] is True
    for claim in MODULE.FORBIDDEN_PAYLOAD_CLAIMS["moderation_toggle"]:
        assert payload[claim] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "moderation_toggle"
    assert errors == []


def test_moderation_toggle_requires_complete_required_toggles(
    tmp_path: Path,
    capsys,
) -> None:
    args = moderation_args(tmp_path)
    index = args.index("--toggle")
    del args[index : index + 2]
    args[args.index("--toggle-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--toggle must include every required value" in captured.err
    assert not (tmp_path / "moderation-toggle.json").exists()


def test_moderation_toggle_count_must_match_required_toggles(
    tmp_path: Path,
    capsys,
) -> None:
    args = moderation_args(tmp_path)
    args[args.index("--toggle-count") + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--toggle-count must match the number of required unique --toggle values"
        in captured.err
    )
    assert not (tmp_path / "moderation-toggle.json").exists()


def test_builds_payload_free_observability_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("observability", tmp_path)) == 0

    payload = json.loads((tmp_path / "observability.json").read_text("utf-8"))

    assert payload["schema"] == "sorafs.gateway_compliance.observability_canary.v1"
    assert payload["metrics"] == list(MODULE.REQUIRED_METRICS)
    assert payload["metric_count"] == len(MODULE.REQUIRED_METRICS)
    assert payload["critical_alerts_firing"] is False
    for claim in MODULE.FORBIDDEN_PAYLOAD_CLAIMS["observability"]:
        assert payload[claim] is False
    kind, errors = CHECKER.validate_evidence_payload(payload, checker_options())
    assert kind == "observability"
    assert errors == []


def test_enforcement_probe_requires_route_body_digest(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("enforcement_probe", tmp_path)
    index = args.index("--route-body-blake3-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route-body-blake3-hex must be exact lowercase 32-byte hex" in captured.err
    assert not (tmp_path / "enforcement_probe.json").exists()


def test_toggle_api_url_rejects_encoded_or_secret_bearing_values_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    unsafe_urls = (
        "https://user:private_key@gateway-compliance.internal/toggles",
        "https://gateway-compliance.internal/%2e%2e/toggles",
        "https://gateway-compliance.internal/bad%2Ftoggle",
        "https://gateway-compliance.internal/C%3A/toggles",
        "https://C%3A.gateway-compliance.internal/toggles",
        "https://http%3A.gateway-compliance.internal/toggles",
        "https://gateway-compliance.internal/toggles?token=secret",
    )

    for unsafe_url in unsafe_urls:
        args = moderation_args(tmp_path)
        args[args.index("--toggle-api-url") + 1] = unsafe_url

        assert MODULE.main(args) == 2

        captured = capsys.readouterr()
        assert MODULE.CANARY_URL_ARG_ERROR in captured.err
        assert unsafe_url not in captured.err
        assert not (tmp_path / "moderation-toggle.json").exists()


def test_generated_canaries_pass_full_gateway_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command: list[str] = []
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary), "--now-unix", str(GENERATED_AT)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_bundle_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


@pytest.mark.parametrize(
    ("kind", "constant_name", "replacement", "expected_error"),
    (
        (
            "feed_promotion",
            "DEFAULT_GATEWAYS",
            ("gateway-a", "gateway-compliance-gateway-b", "gateway-compliance-gateway-c"),
            "DEFAULT_GATEWAYS must match canonical lowercase "
            "`gateway-compliance-gateway-name`",
        ),
        (
            "feed_promotion",
            "DEFAULT_DENYLIST_ENTRIES",
            (
                "ofac",
                "gateway-denylist-entry-eu-sanctions",
                "gateway-denylist-entry-malware",
                "gateway-denylist-entry-csam-hash",
                "gateway-denylist-entry-legal-hold",
            ),
            "DEFAULT_DENYLIST_ENTRIES must match canonical lowercase "
            "`gateway-denylist-entry-name`",
        ),
        (
            "honey_audit",
            "DEFAULT_HONEY_PROBES",
            (
                "honey-probe-00",
                "gateway-honey-probe-01",
                "gateway-honey-probe-02",
                "gateway-honey-probe-03",
            ),
            "DEFAULT_HONEY_PROBES must match canonical lowercase "
            "`gateway-honey-probe-*`",
        ),
    ),
)
def test_fixed_inventory_labels_must_use_production_family_before_write(
    kind: str,
    constant_name: str,
    replacement: tuple[str, ...],
    expected_error: str,
    tmp_path: Path,
    capsys,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(MODULE, constant_name, replacement)

    assert_rejected_without_artifact(
        args_for(kind, tmp_path),
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


@pytest.mark.parametrize(
    ("kind", "constant_name", "replacement", "expected_error"),
    (
        (
            "feed_promotion",
            "DEFAULT_GATEWAYS",
            (
                "gateway-compliance-gateway-placeholder",
                "gateway-compliance-gateway-b",
                "gateway-compliance-gateway-c",
            ),
            "DEFAULT_GATEWAYS must not contain non-production markers "
            "['placeholder']",
        ),
        (
            "feed_promotion",
            "DEFAULT_DENYLIST_ENTRIES",
            (
                "gateway-denylist-entry-placeholder",
                "gateway-denylist-entry-eu-sanctions",
                "gateway-denylist-entry-malware",
                "gateway-denylist-entry-csam-hash",
                "gateway-denylist-entry-legal-hold",
            ),
            "DEFAULT_DENYLIST_ENTRIES must not contain non-production markers "
            "['placeholder']",
        ),
        (
            "honey_audit",
            "DEFAULT_HONEY_PROBES",
            (
                "gateway-honey-probe-placeholder",
                "gateway-honey-probe-01",
                "gateway-honey-probe-02",
                "gateway-honey-probe-03",
            ),
            "DEFAULT_HONEY_PROBES must not contain non-production markers "
            "['placeholder']",
        ),
    ),
)
def test_fixed_inventory_labels_reject_non_production_markers_before_write(
    kind: str,
    constant_name: str,
    replacement: tuple[str, ...],
    expected_error: str,
    tmp_path: Path,
    capsys,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(MODULE, constant_name, replacement)

    assert_rejected_without_artifact(
        args_for(kind, tmp_path),
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=expected_error,
    )


def test_response_file_can_build_controller_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "controller.args"
    args_file.write_text("\n".join(controller_args(tmp_path)), encoding="utf-8")

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads((tmp_path / "controller-runtime.json").read_text("utf-8"))
    assert payload["controller_instance_id"] == "gateway-compliance-controller-prod-a"


def test_missing_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = controller_args(tmp_path)
    index = args.index("--verified-claim")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim must include every required value" in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_unknown_verified_claim_fails_closed(tmp_path: Path, capsys) -> None:
    args = controller_args(tmp_path)
    args.extend(["--verified-claim", "shadow-controller-claim"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--verified-claim contains an unknown value" in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "controller_runtime",
            "--verified-claim",
            MODULE.CONTROLLER_TRUE_CLAIMS[0],
            "unreviewed-controller-claim",
        ),
        (
            "moderation_toggle",
            "--verified-claim",
            MODULE.MODERATION_TRUE_CLAIMS[0],
            "unreviewed-toggle-claim",
        ),
        (
            "controller_runtime",
            "--feed",
            MODULE.REQUIRED_CONTROLLER_FEEDS[0],
            "unreviewed-controller-feed",
        ),
        (
            "moderation_toggle",
            "--toggle",
            MODULE.REQUIRED_MODERATION_TOGGLES[0],
            "unreviewed-moderation-toggle",
        ),
        (
            "enforcement_probe",
            "--denial-reason",
            MODULE.REQUIRED_DENIAL_REASONS[0],
            "unreviewed-denial-reason",
        ),
        (
            "observability",
            "--metric",
            MODULE.REQUIRED_METRICS[0],
            "unreviewed-gateway-compliance-metric",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    kind: str,
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = args_for(kind, tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = args_for(kind, unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind=kind,
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_missing_controller_feed_inventory_fails_closed(tmp_path: Path, capsys) -> None:
    args = controller_args(tmp_path)
    while "--feed" in args:
        index = args.index("--feed")
        del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--feed must include every required value" in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_duplicate_controller_feed_inventory_fails_closed(tmp_path: Path, capsys) -> None:
    args = controller_args(tmp_path)
    args[args.index("--feed-count") + 1] = "8"
    args.extend(["--feed", "ofac"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--feed must not contain duplicates" in captured.err
    assert (
        "--feed-count must match the number of required unique --feed values"
        in captured.err
    )
    assert not (tmp_path / "controller-runtime.json").exists()


def test_unknown_controller_feed_inventory_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = controller_args(tmp_path)
    args.extend(["--feed", "shadow-feed"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--feed contains an unknown value" in captured.err
    assert not (tmp_path / "controller-runtime.json").exists()


def test_missing_moderation_toggle_digest_fails_closed(tmp_path: Path, capsys) -> None:
    args = moderation_args(tmp_path)
    index = args.index("--toggle-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--toggle-digest-hex must be exact lowercase 32-byte hex" in captured.err
    assert not (tmp_path / "moderation-toggle.json").exists()


def test_missing_moderation_toggle_inventory_fails_closed(tmp_path: Path, capsys) -> None:
    args = moderation_args(tmp_path)
    while "--toggle" in args:
        index = args.index("--toggle")
        del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--toggle must include every required value" in captured.err
    assert not (tmp_path / "moderation-toggle.json").exists()


def test_duplicate_moderation_toggle_inventory_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = moderation_args(tmp_path)
    args[args.index("--toggle-count") + 1] = "5"
    args.extend(["--toggle", "provider-deny"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--toggle must not contain duplicates" in captured.err
    assert (
        "--toggle-count must match the number of required unique --toggle values"
        in captured.err
    )
    assert not (tmp_path / "moderation-toggle.json").exists()


def test_unknown_moderation_toggle_inventory_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = moderation_args(tmp_path)
    args.extend(["--toggle", "shadow-toggle"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--toggle contains an unknown value" in captured.err
    assert not (tmp_path / "moderation-toggle.json").exists()


def test_duplicate_denial_reason_inventory_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("enforcement_probe", tmp_path)
    args.extend(["--denial-reason", MODULE.REQUIRED_DENIAL_REASONS[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--denial-reason must not contain duplicates" in captured.err
    assert not (tmp_path / "enforcement_probe.json").exists()


def test_unknown_denial_reason_inventory_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("enforcement_probe", tmp_path)
    args.extend(["--denial-reason", "shadow-denial-reason"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--denial-reason contains an unknown value" in captured.err
    assert not (tmp_path / "enforcement_probe.json").exists()


def test_duplicate_metric_inventory_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("observability", tmp_path)
    args.extend(["--metric", MODULE.REQUIRED_METRICS[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric must not contain duplicates" in captured.err
    assert not (tmp_path / "observability.json").exists()


def test_unknown_metric_inventory_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("observability", tmp_path)
    args.extend(["--metric", "sorafs_gateway_shadow_metric_total"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric contains an unknown value" in captured.err
    assert not (tmp_path / "observability.json").exists()


def test_output_symlink_is_rejected(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    symlink = tmp_path / "controller-runtime.json"
    symlink.symlink_to(target)

    assert MODULE.main(controller_args(tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    directory = tmp_path / "controller-runtime.json"
    directory.mkdir()

    assert MODULE.main(controller_args(tmp_path)) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
