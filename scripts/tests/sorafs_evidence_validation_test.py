"""Tests for shared SoraFS evidence validation helpers."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_evidence_validation import (  # noqa: E402
    build_evidence_artifact,
    build_kinded_evidence_artifact,
    build_required_evidence_summary,
    collect_string_values,
    count_evidence_artifacts,
    count_evidence_files,
    count_recognized_evidence_artifacts,
    distinct_evidence_values_are_consistent,
    deployment_context_summary,
    evidence_artifact_digest_set,
    evidence_artifact_detail,
    evidence_artifact_fingerprint,
    evidence_artifact_is_valid,
    evidence_artifact_kind,
    evidence_artifact_schema,
    evidence_gate_status,
    evidence_schema_by_kind,
    finalize_custom_required_evidence_rows,
    hashable_evidence_values,
    init_evidence_artifact_buckets,
    is_hex,
    mark_required_evidence_invalid,
    mark_required_evidence_invalid_if_present,
    mark_required_evidence_summary_invalid,
    missing_required_evidence_values,
    require_2xx_status,
    require_advancing_int_pair,
    require_bool_true,
    require_count_equal,
    require_count_length_match,
    require_count_match,
    require_count_value_equal,
    require_false,
    require_false_or_absent,
    require_false_or_governed,
    require_hex,
    require_hex_string_array,
    require_config_backed_governance_approval,
    require_governance_approval,
    require_iroha_config_binding,
    require_int_range,
    require_known_schema,
    require_maximum_int,
    require_maximum_number,
    require_maximum_value,
    require_minimum_int,
    require_minimum_value,
    require_non_negative_int,
    require_non_negative_number,
    require_object,
    require_object_array,
    require_optional_hex,
    require_passed_status,
    require_policy_digest,
    require_positive_int,
    require_recent_timestamp,
    record_consistent_evidence_value,
    record_custom_required_evidence_artifact,
    record_evidence_artifact,
    record_evidence_validation_errors,
    record_explicit_evidence_validation_errors,
    record_evidence_digest_mismatch_errors,
    record_inconsistent_evidence_values_error,
    record_missing_required_evidence_value_errors,
    record_missing_required_or_observed_evidence_error,
    record_observed_evidence_value,
    record_snapshot_bound_evidence_artifact,
    record_string_value_binding_errors,
    record_string_tuple_binding_errors,
    validate_bound_evidence_digest_references,
    validate_bound_evidence_tuple_references,
    recognized_evidence_artifacts_are_valid,
    required_evidence_has_all_kinds,
    required_evidence_has_any_kind,
    required_or_observed_evidence_values_are_present,
    required_evidence_summary_is_valid,
    require_rollout_deployment_id,
    require_rollout_environment,
    require_score_bps,
    require_status_in,
    require_string,
    require_string_equal,
    require_string_in,
    require_string_not_equal,
    require_string_tuple_in,
    require_string_type,
    require_string_value_equal,
    require_string_value_in,
    require_string_coverage,
    require_sum_equal,
    require_zero_count,
    required_evidence_kind_names,
    record_consistent_deployment_context,
    validate_snapshot_bound_evidence_artifacts,
    validate_standard_evidence_payload,
)


def test_build_evidence_artifact_records_payload_free_fingerprint() -> None:
    payload = {
        "schema": "example.schema.v1",
        "status": "passed",
        "digest_hex": "a" * 64,
        "secret": "runtime-only",
    }

    artifact = build_evidence_artifact(
        Path("evidence.json"),
        "b" * 64,
        payload,
        [],
        ("digest_hex", "missing_field"),
    )

    assert artifact == {
        "path": "evidence.json",
        "sha256": "b" * 64,
        "schema": "example.schema.v1",
        "status": "passed",
        "fingerprint": {
            "digest_hex": "a" * 64,
            "missing_field": None,
        },
        "valid": True,
        "errors": [],
    }
    assert "secret" not in artifact["fingerprint"]


def test_build_evidence_artifact_preserves_invalid_error_bucket() -> None:
    validation_errors = ["status must be `passed`"]

    artifact = build_evidence_artifact(
        Path("bad.json"),
        "c" * 64,
        {"schema": "example.schema.v1", "status": "failed"},
        validation_errors,
        ("status",),
    )

    assert artifact["valid"] is False
    assert artifact["errors"] is validation_errors
    assert artifact["fingerprint"] == {"status": "failed"}


def test_build_evidence_artifact_rejects_malformed_error_buckets() -> None:
    for validation_errors in ("bad", ["bad", 7], None):
        artifact = build_evidence_artifact(
            Path("bad.json"),
            "c" * 64,
            {"schema": "example.schema.v1", "status": "failed"},
            validation_errors,
            ("status",),
        )

        assert artifact["valid"] is False
        assert artifact["errors"] == [
            "artifact validation errors must be a sequence of strings"
        ]

    artifact = build_evidence_artifact(
        Path("bad.json"),
        "c" * 64,
        {"schema": "example.schema.v1", "status": "failed"},
        ("status must be `passed`",),
        ("status",),
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["status must be `passed`"]


def test_build_evidence_artifact_rejects_malformed_payload_or_fingerprint_fields() -> None:
    validation_errors: list[str] = []
    artifact = build_evidence_artifact(
        Path("bad.json"),
        "c" * 64,
        "not-an-object",
        validation_errors,
        ("status",),
    )

    assert artifact["schema"] is None
    assert artifact["status"] is None
    assert artifact["fingerprint"] == {}
    assert artifact["valid"] is False
    assert artifact["errors"] is validation_errors
    assert artifact["errors"] == ["artifact fingerprint payload must be an object"]

    artifact = build_evidence_artifact(
        Path("bad.json"),
        "c" * 64,
        {"schema": "example.schema.v1", "status": "failed"},
        [],
        "status",
    )

    assert artifact["fingerprint"] == {}
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "artifact fingerprint fields must be a sequence of strings"
    ]

    artifact = build_evidence_artifact(
        Path("bad.json"),
        "c" * 64,
        {"schema": "example.schema.v1", "status": "failed"},
        [],
        ("status", ""),
    )

    assert artifact["fingerprint"] == {}
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "artifact fingerprint fields must be non-empty strings"
    ]


def test_build_evidence_artifact_rejects_malformed_sha256() -> None:
    validation_errors: list[str] = []
    artifact = build_evidence_artifact(
        Path("bad.json"),
        "not-a-digest",
        {"schema": "example.schema.v1", "status": "passed"},
        validation_errors,
        ("status",),
    )

    assert artifact["sha256"] == ""
    assert artifact["valid"] is False
    assert artifact["errors"] is validation_errors
    assert artifact["errors"] == [
        "artifact sha256 must be a 64-character lowercase hex string"
    ]

    artifact = build_evidence_artifact(
        Path("bad.json"),
        "A" * 64,
        {"schema": "example.schema.v1", "status": "passed"},
        [],
        ("status",),
    )

    assert artifact["sha256"] == ""
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "artifact sha256 must be a 64-character lowercase hex string"
    ]


def test_build_kinded_evidence_artifact_records_snapshot_fingerprint() -> None:
    validation_errors: list[str] = []
    payload = {
        "schema": "sorafs.reputation.provider.v1",
        "provider_id": "provider-a",
        "raw_provider_record_included": False,
    }

    artifact = build_kinded_evidence_artifact(
        kind_name="provider",
        path=Path("provider.json"),
        digest="d" * 64,
        payload=payload,
        validation_errors=validation_errors,
        fingerprint_fields=("provider_id", "missing_field"),
        fingerprint_values={
            "snapshot_id_hex": "a" * 64,
            "merkle_root_hex": "b" * 64,
        },
    )

    assert artifact == {
        "kind": "provider",
        "path": "provider.json",
        "sha256": "d" * 64,
        "fingerprint": {
            "provider_id": "provider-a",
            "missing_field": None,
            "snapshot_id_hex": "a" * 64,
            "merkle_root_hex": "b" * 64,
        },
        "valid": True,
        "errors": validation_errors,
    }
    assert "raw_provider_record_included" not in artifact["fingerprint"]


def test_build_kinded_evidence_artifact_preserves_invalid_error_bucket() -> None:
    validation_errors = ["provider_id must be a non-empty string"]

    artifact = build_kinded_evidence_artifact(
        kind_name="provider",
        path=Path("provider.json"),
        digest="e" * 64,
        payload={"provider_id": ""},
        validation_errors=validation_errors,
        fingerprint_fields=("provider_id",),
    )

    assert artifact["valid"] is False
    assert artifact["errors"] is validation_errors
    assert artifact["fingerprint"] == {"provider_id": ""}


def test_build_kinded_evidence_artifact_rejects_malformed_error_buckets() -> None:
    for validation_errors in ("bad", ["bad", 7], None):
        artifact = build_kinded_evidence_artifact(
            kind_name="provider",
            path=Path("provider.json"),
            digest="e" * 64,
            payload={"provider_id": ""},
            validation_errors=validation_errors,
            fingerprint_fields=("provider_id",),
        )

        assert artifact["valid"] is False
        assert artifact["errors"] == [
            "artifact validation errors must be a sequence of strings"
        ]

    artifact = build_kinded_evidence_artifact(
        kind_name="provider",
        path=Path("provider.json"),
        digest="e" * 64,
        payload={"provider_id": ""},
        validation_errors=("provider_id must be a non-empty string",),
        fingerprint_fields=("provider_id",),
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["provider_id must be a non-empty string"]


def test_build_kinded_evidence_artifact_rejects_malformed_fingerprint_inputs() -> None:
    validation_errors: list[str] = []
    artifact = build_kinded_evidence_artifact(
        kind_name="provider",
        path=Path("provider.json"),
        digest="e" * 64,
        payload="not-an-object",
        validation_errors=validation_errors,
        fingerprint_fields=("provider_id",),
        fingerprint_values={"snapshot_id_hex": "a" * 64},
    )

    assert artifact["fingerprint"] == {"snapshot_id_hex": "a" * 64}
    assert artifact["valid"] is False
    assert artifact["errors"] is validation_errors
    assert artifact["errors"] == ["artifact fingerprint payload must be an object"]

    artifact = build_kinded_evidence_artifact(
        kind_name="provider",
        path=Path("provider.json"),
        digest="e" * 64,
        payload={"provider_id": "provider-a"},
        validation_errors=[],
        fingerprint_fields="provider_id",
    )

    assert artifact["fingerprint"] == {}
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "artifact fingerprint fields must be a sequence of strings"
    ]

    artifact = build_kinded_evidence_artifact(
        kind_name="provider",
        path=Path("provider.json"),
        digest="e" * 64,
        payload={"provider_id": "provider-a"},
        validation_errors=[],
        fingerprint_fields=("provider_id",),
        fingerprint_values=[("snapshot_id_hex", "a" * 64)],
    )

    assert artifact["fingerprint"] == {"provider_id": "provider-a"}
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "artifact fingerprint values must be a mapping"
    ]

    artifact = build_kinded_evidence_artifact(
        kind_name="provider",
        path=Path("provider.json"),
        digest="e" * 64,
        payload={"provider_id": "provider-a"},
        validation_errors=[],
        fingerprint_fields=("provider_id",),
        fingerprint_values={1: "bad"},
    )

    assert artifact["fingerprint"] == {"provider_id": "provider-a"}
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "artifact fingerprint value keys must be non-empty strings"
    ]


def test_build_kinded_evidence_artifact_rejects_malformed_kind_or_sha256() -> None:
    validation_errors: list[str] = []
    artifact = build_kinded_evidence_artifact(
        kind_name="",
        path=Path("provider.json"),
        digest="not-a-digest",
        payload={"provider_id": "provider-a"},
        validation_errors=validation_errors,
        fingerprint_fields=("provider_id",),
    )

    assert artifact["kind"] == "<unknown>"
    assert artifact["sha256"] == ""
    assert artifact["valid"] is False
    assert artifact["errors"] is validation_errors
    assert artifact["errors"] == [
        "artifact kind must be a non-empty string",
        "artifact sha256 must be a 64-character lowercase hex string",
    ]

    artifact = build_kinded_evidence_artifact(
        kind_name=7,
        path=Path("provider.json"),
        digest="E" * 64,
        payload={"provider_id": "provider-a"},
        validation_errors=[],
        fingerprint_fields=("provider_id",),
    )

    assert artifact["kind"] == "<unknown>"
    assert artifact["sha256"] == ""
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "artifact kind must be a non-empty string",
        "artifact sha256 must be a 64-character lowercase hex string",
    ]


def test_record_evidence_artifact_appends_to_kind_bucket() -> None:
    buckets = init_evidence_artifact_buckets(("route", "manifest"))
    artifact = {"valid": True, "path": "route.json"}
    errors: list[str] = []

    assert record_evidence_artifact(buckets, "route", artifact, errors) is True

    assert buckets["route"] == [artifact]
    assert buckets["route"][0] is artifact
    assert buckets["manifest"] == []
    assert errors == []


def test_record_evidence_artifact_reports_missing_kind_bucket() -> None:
    buckets = init_evidence_artifact_buckets(("route",))
    artifact = {"valid": True, "path": "manifest.json"}
    errors: list[str] = []

    assert record_evidence_artifact(buckets, "manifest", artifact, errors) is False

    assert buckets["route"] == []
    assert errors == ["recognized evidence kind `manifest` has no artifact bucket"]


def test_record_evidence_artifact_rejects_malformed_inputs() -> None:
    errors: list[str] = []
    assert record_evidence_artifact("bad", "route", {"valid": True}, errors) is False
    assert record_evidence_artifact({}, "", {"valid": True}, errors) is False
    assert record_evidence_artifact({}, 7, {"valid": True}, errors) is False

    buckets = init_evidence_artifact_buckets(("route",))
    assert record_evidence_artifact(buckets, "route", "bad", errors) is False

    assert buckets == {"route": []}
    assert errors == [
        "recognized evidence artifacts by kind must be a mapping",
        "recognized evidence kind must be a non-empty string",
        "recognized evidence kind must be a non-empty string",
        "recognized `route` evidence artifact must be an object",
    ]


def test_evidence_artifact_is_valid_requires_explicit_true() -> None:
    assert evidence_artifact_is_valid({"valid": True}) is True
    assert evidence_artifact_is_valid({"valid": False}) is False
    assert evidence_artifact_is_valid({"valid": 1}) is False
    assert evidence_artifact_is_valid({}) is False


def test_evidence_artifact_kind_returns_string_or_none() -> None:
    assert evidence_artifact_kind({"kind": "provider"}) == "provider"
    assert evidence_artifact_kind({"kind": ""}) == ""
    assert evidence_artifact_kind({"kind": None}) is None
    assert evidence_artifact_kind({}) is None


def test_evidence_artifact_accessors_reject_non_mapping_without_traceback() -> None:
    artifact = "not an artifact row"

    assert evidence_artifact_is_valid(artifact) is False
    assert evidence_artifact_kind(artifact) is None
    assert evidence_artifact_fingerprint(artifact) == {}
    assert evidence_artifact_detail(artifact, "cycle") == {}
    assert evidence_artifact_schema(artifact) == "<unknown>"


def test_mark_required_evidence_invalid_creates_row_and_returns_errors() -> None:
    required: dict[str, dict[str, object]] = {}

    errors = mark_required_evidence_invalid(required, "provider")
    errors.append("missing provider")

    assert required == {
        "provider": {
            "valid": False,
            "errors": ["missing provider"],
            "artifacts": [],
        }
    }


def test_mark_required_evidence_invalid_rebuilds_malformed_errors() -> None:
    required: dict[str, dict[str, object]] = {
        "latest": {"valid": True, "errors": "old", "artifacts": []}
    }

    errors = mark_required_evidence_invalid(required, "latest")
    errors.append("new")

    assert required["latest"]["valid"] is False
    assert required["latest"]["errors"] == [
        "required `latest` errors must be a list",
        "new",
    ]


def test_mark_required_evidence_invalid_repairs_malformed_row() -> None:
    required = {"latest": "bad"}

    errors = mark_required_evidence_invalid(required, "latest")
    errors.append("new")

    assert required["latest"] == {
        "valid": False,
        "errors": ["required `latest` row must be an object", "new"],
        "artifacts": [],
    }


def test_mark_required_evidence_summary_invalid_marks_every_row() -> None:
    required: dict[str, dict[str, object]] = {
        "provider": {"valid": True, "errors": [], "artifacts": []},
        "latest": {"valid": True, "errors": "old", "artifacts": []},
    }

    mark_required_evidence_summary_invalid(required, "summary mismatch")

    assert required["provider"]["valid"] is False
    assert required["provider"]["errors"] == ["summary mismatch"]
    assert required["latest"]["valid"] is False
    assert required["latest"]["errors"] == [
        "required `latest` errors must be a list",
        "summary mismatch",
    ]


def test_mark_required_evidence_invalid_if_present_skips_unknown_kind() -> None:
    required: dict[str, dict[str, object]] = {
        "provider": {"valid": True, "errors": [], "artifacts": []},
    }

    errors = mark_required_evidence_invalid_if_present(required, "provider")
    errors.append("provider failed")

    assert required["provider"]["valid"] is False
    assert required["provider"]["errors"] == ["provider failed"]
    assert mark_required_evidence_invalid_if_present(required, "optional") == []
    assert mark_required_evidence_invalid_if_present(required, None) == []
    assert set(required) == {"provider"}


def test_required_evidence_summary_is_valid_requires_explicit_true() -> None:
    assert required_evidence_summary_is_valid({"route": {"valid": True}}) is True
    assert required_evidence_summary_is_valid({"route": {"valid": False}}) is False
    assert required_evidence_summary_is_valid({"route": {"valid": 1}}) is False
    assert required_evidence_summary_is_valid({"route": {}}) is False


def test_required_evidence_summary_is_valid_fails_closed_on_malformed_rows() -> None:
    assert required_evidence_summary_is_valid(None) is False
    assert required_evidence_summary_is_valid("bad") is False
    assert required_evidence_summary_is_valid({"route": "bad"}) is False
    assert required_evidence_summary_is_valid({"route": None}) is False


def test_required_evidence_has_any_kind_matches_candidates() -> None:
    assert required_evidence_has_any_kind(
        ("publish", "latest", "provider"),
        ("provider", "events"),
    )
    assert required_evidence_has_any_kind(
        {"metrics", "transport"},
        ("provider", "metrics"),
    )
    assert not required_evidence_has_any_kind(("publish", "latest"), ("provider",))
    assert not required_evidence_has_any_kind((), ("provider",))


def test_required_evidence_has_any_kind_fails_closed_on_malformed_kinds() -> None:
    assert required_evidence_has_any_kind("provider", ("provider",)) is False
    assert required_evidence_has_any_kind(("provider",), "provider") is False
    assert (
        required_evidence_has_any_kind({"provider": True}, ("provider",)) is False
    )
    assert (
        required_evidence_has_any_kind(("provider",), {"provider": True}) is False
    )
    assert required_evidence_has_any_kind(("provider", None), ("provider",)) is False
    assert required_evidence_has_any_kind(("provider",), ("",)) is False


def test_required_evidence_has_all_kinds_matches_candidates() -> None:
    assert required_evidence_has_all_kinds(
        ("billing_cycle", "reference_price", "reconciliation"),
        ("billing_cycle", "reference_price"),
    )
    assert required_evidence_has_all_kinds(
        {"metrics", "transport"},
        ("transport",),
    )
    assert required_evidence_has_all_kinds(("publish",), ())
    assert not required_evidence_has_all_kinds(
        ("billing_cycle",),
        ("billing_cycle", "reference_price"),
    )


def test_required_evidence_has_all_kinds_fails_closed_on_malformed_kinds() -> None:
    assert (
        required_evidence_has_all_kinds("billing_cycle", ("billing_cycle",)) is False
    )
    assert (
        required_evidence_has_all_kinds(("billing_cycle",), "billing_cycle") is False
    )
    assert (
        required_evidence_has_all_kinds(
            {"billing_cycle": True},
            ("billing_cycle",),
        )
        is False
    )
    assert (
        required_evidence_has_all_kinds(
            ("billing_cycle",),
            {"billing_cycle": True},
        )
        is False
    )
    assert required_evidence_has_all_kinds((None,), ()) is False
    assert required_evidence_has_all_kinds(("billing_cycle",), ("",)) is False


def test_missing_required_evidence_values_preserves_required_order() -> None:
    assert missing_required_evidence_values(
        ("provider-b", "provider-a", "provider-c"),
        {"provider-a"},
    ) == ["provider-b", "provider-c"]
    assert missing_required_evidence_values(
        ["provider-a"],
        ("provider-a", "provider-b"),
    ) == []
    assert missing_required_evidence_values((), {"provider-a"}) == []


def test_hashable_evidence_values_skips_falsey_and_unhashable_values() -> None:
    assert hashable_evidence_values(
        ("provider-a", "", 0, ["provider-b"], {"provider_id": "provider-c"}, 3)
    ) == {"provider-a", 3}


def test_hashable_evidence_values_rejects_scalar_or_mapping_containers() -> None:
    assert hashable_evidence_values("provider-a") == set()
    assert hashable_evidence_values(b"provider-a") == set()
    assert hashable_evidence_values({"provider_id": "provider-a"}) == set()
    assert hashable_evidence_values(None) == set()


def test_missing_required_evidence_values_skips_unhashable_observed_values() -> None:
    assert missing_required_evidence_values(
        ("provider-a", "provider-b"),
        (["provider-a"], {"provider_id": "provider-b"}, "provider-a"),
    ) == ["provider-b"]


def test_missing_required_evidence_values_rejects_scalar_containers() -> None:
    assert missing_required_evidence_values("provider-a", {"provider-a"}) == [
        "provider-a"
    ]
    assert missing_required_evidence_values(["provider-a"], "provider-a") == [
        "provider-a"
    ]
    assert missing_required_evidence_values(
        {"provider_id": "provider-a"},
        {"provider-a"},
    ) == [{"provider_id": "provider-a"}]


def test_missing_required_evidence_values_fails_closed_on_unhashable_required_values() -> None:
    required = ["provider-a", {"provider_id": "provider-b"}]

    assert missing_required_evidence_values(required, {"provider-a"}) == [
        {"provider_id": "provider-b"}
    ]


def test_record_missing_required_evidence_value_errors_marks_missing_values() -> None:
    required = {
        "provider": {"valid": True, "errors": [], "artifacts": []},
    }

    missing = record_missing_required_evidence_value_errors(
        required,
        "provider",
        ("provider-a", "provider-b"),
        {"provider-a"},
        lambda provider_id: f"missing provider/proof evidence for `{provider_id}`",
    )

    assert missing == ["provider-b"]
    assert required["provider"]["valid"] is False
    assert required["provider"]["errors"] == [
        "missing provider/proof evidence for `provider-b`"
    ]


def test_record_missing_required_evidence_value_errors_skips_complete_values() -> None:
    required = {
        "provider": {"valid": True, "errors": [], "artifacts": []},
    }

    missing = record_missing_required_evidence_value_errors(
        required,
        "provider",
        ("provider-a",),
        {"provider-a"},
        lambda provider_id: f"missing provider/proof evidence for `{provider_id}`",
    )

    assert missing == []
    assert required["provider"]["valid"] is True
    assert required["provider"]["errors"] == []


def test_required_or_observed_evidence_values_are_present() -> None:
    assert required_or_observed_evidence_values_are_present(("provider-a",), set())
    assert required_or_observed_evidence_values_are_present((), {"provider-a"})
    assert not required_or_observed_evidence_values_are_present((), set())


def test_required_or_observed_evidence_values_ignore_malformed_values() -> None:
    assert required_or_observed_evidence_values_are_present(
        ("provider-a",),
        ([],),
    )
    assert required_or_observed_evidence_values_are_present(
        (),
        ("provider-a", {"provider_id": "provider-b"}),
    )
    assert not required_or_observed_evidence_values_are_present(
        ("", {"provider_id": "provider-a"}),
        ([],),
    )
    assert not required_or_observed_evidence_values_are_present("provider-a", set())
    assert not required_or_observed_evidence_values_are_present((), "provider-a")


def test_record_missing_required_or_observed_evidence_error_marks_empty_values() -> None:
    required = {
        "provider": {"valid": True, "errors": [], "artifacts": []},
    }

    recorded = record_missing_required_or_observed_evidence_error(
        required,
        "provider",
        (),
        set(),
        "at least one provider proof must be verified",
    )

    assert recorded is True
    assert required["provider"]["valid"] is False
    assert required["provider"]["errors"] == [
        "at least one provider proof must be verified"
    ]


def test_record_missing_required_or_observed_evidence_error_marks_malformed_values() -> None:
    required = {
        "provider": {"valid": True, "errors": [], "artifacts": []},
    }

    recorded = record_missing_required_or_observed_evidence_error(
        required,
        "provider",
        ("", {"provider_id": "provider-a"}),
        ([],),
        "at least one provider proof must be verified",
    )

    assert recorded is True
    assert required["provider"]["valid"] is False
    assert required["provider"]["errors"] == [
        "at least one provider proof must be verified"
    ]


def test_record_missing_required_or_observed_evidence_error_skips_present_values() -> None:
    required = {
        "provider": {"valid": True, "errors": [], "artifacts": []},
    }

    assert (
        record_missing_required_or_observed_evidence_error(
            required,
            "provider",
            ("provider-a",),
            set(),
            "at least one provider proof must be verified",
        )
        is False
    )
    assert (
        record_missing_required_or_observed_evidence_error(
            required,
            "provider",
            (),
            {"provider-a"},
            "at least one provider proof must be verified",
        )
        is False
    )
    assert required["provider"]["valid"] is True
    assert required["provider"]["errors"] == []


def test_distinct_evidence_values_are_consistent_allows_zero_or_one_value() -> None:
    assert distinct_evidence_values_are_consistent(set())
    assert distinct_evidence_values_are_consistent({3})
    assert not distinct_evidence_values_are_consistent({3, 4})


def test_distinct_evidence_values_are_consistent_fails_closed_on_malformed_values() -> None:
    assert not distinct_evidence_values_are_consistent(None)
    assert not distinct_evidence_values_are_consistent("")
    assert not distinct_evidence_values_are_consistent("a")
    assert not distinct_evidence_values_are_consistent({"provider": "provider-a"})


def test_record_inconsistent_evidence_values_error_marks_summary_invalid() -> None:
    required = {
        "latest": {"valid": True, "errors": [], "artifacts": []},
        "provider": {"valid": True, "errors": [], "artifacts": []},
    }

    recorded = record_inconsistent_evidence_values_error(
        required,
        {3, 4},
        "latest",
        "provider counts differ across rollout evidence",
    )

    assert recorded is True
    assert required["latest"]["valid"] is False
    assert required["provider"]["valid"] is False
    assert required["latest"]["errors"] == [
        "provider counts differ across rollout evidence"
    ]
    assert required["provider"]["errors"] == []


def test_record_inconsistent_evidence_values_error_marks_malformed_values() -> None:
    required = {
        "latest": {"valid": True, "errors": [], "artifacts": []},
        "provider": {"valid": True, "errors": [], "artifacts": []},
    }

    recorded = record_inconsistent_evidence_values_error(
        required,
        "provider-a",
        "latest",
        "provider counts differ across rollout evidence",
    )

    assert recorded is True
    assert required["latest"]["valid"] is False
    assert required["provider"]["valid"] is False
    assert required["latest"]["errors"] == [
        "provider counts differ across rollout evidence"
    ]


def test_record_inconsistent_evidence_values_error_skips_consistent_values() -> None:
    required = {
        "latest": {"valid": True, "errors": [], "artifacts": []},
        "provider": {"valid": True, "errors": [], "artifacts": []},
    }

    recorded = record_inconsistent_evidence_values_error(
        required,
        {3},
        "latest",
        "provider counts differ across rollout evidence",
    )

    assert recorded is False
    assert required["latest"]["valid"] is True
    assert required["provider"]["valid"] is True
    assert required["latest"]["errors"] == []


def test_record_string_tuple_binding_errors_returns_normalized_binding() -> None:
    artifact = {"path": "cycle-bound.json", "valid": True, "errors": []}
    errors: list[str] = []

    binding = record_string_tuple_binding_errors(
        artifact,
        ("AA", "bb"),
        {("aa", "bb")},
        errors,
        message="cycle tuple must match",
    )

    assert binding == ("aa", "bb")
    assert artifact["valid"] is True
    assert artifact["errors"] == []
    assert errors == []


def test_record_string_tuple_binding_errors_marks_artifact_invalid() -> None:
    artifact = {"path": "cycle-bound.json", "valid": True, "errors": []}
    errors: list[str] = []

    binding = record_string_tuple_binding_errors(
        artifact,
        ("AA", None),
        {("aa", "bb")},
        errors,
        message="cycle tuple must match",
    )

    assert binding is None
    assert artifact["valid"] is False
    assert artifact["errors"] == ["cycle tuple must match"]
    assert errors == ["cycle-bound.json: cycle tuple must match"]


def test_record_string_tuple_binding_errors_rejects_empty_values() -> None:
    artifact = {"path": "cycle-bound.json", "valid": True, "errors": []}
    errors: list[str] = []

    binding = record_string_tuple_binding_errors(
        artifact,
        ("AA", ""),
        {("aa", "")},
        errors,
        message="cycle tuple must match",
    )

    assert binding is None
    assert artifact["valid"] is False
    assert artifact["errors"] == ["cycle tuple must match"]
    assert errors == ["cycle-bound.json: cycle tuple must match"]


def test_record_string_tuple_binding_errors_rejects_malformed_artifact_rows() -> None:
    errors: list[str] = []

    binding = record_string_tuple_binding_errors(
        "bad",
        ("AA", None),
        {("aa", "bb")},
        errors,
        message="cycle tuple must match",
    )

    assert binding is None
    assert errors == ["<unknown>: cycle tuple must match"]


def test_require_string_tuple_in_rejects_malformed_containers() -> None:
    errors: list[str] = []

    assert (
        require_string_tuple_in("AB", {("a", "b")}, errors, message="cycle tuple")
        is None
    )
    assert (
        require_string_tuple_in(
            ("AA", "BB"),
            {("aa", "bb"): True},
            errors,
            message="cycle tuple",
        )
        is None
    )

    assert errors == ["cycle tuple", "cycle tuple"]


def test_record_string_value_binding_errors_returns_normalized_value() -> None:
    artifact = {"path": "public-head-bound.json", "valid": True, "errors": []}
    errors: list[str] = []

    binding = record_string_value_binding_errors(
        artifact,
        "AA",
        {"aa"},
        errors,
        message="public head must match",
    )

    assert binding == "aa"
    assert artifact["valid"] is True
    assert artifact["errors"] == []
    assert errors == []


def test_record_string_value_binding_errors_marks_artifact_invalid() -> None:
    artifact = {"path": "public-head-bound.json", "valid": True, "errors": []}
    errors: list[str] = []

    binding = record_string_value_binding_errors(
        artifact,
        None,
        {"aa"},
        errors,
        message="public head must match",
    )

    assert binding is None
    assert artifact["valid"] is False
    assert artifact["errors"] == ["public head must match"]
    assert errors == ["public-head-bound.json: public head must match"]


def test_record_string_value_binding_errors_rejects_empty_values() -> None:
    artifact = {"path": "public-head-bound.json", "valid": True, "errors": []}
    errors: list[str] = []

    binding = record_string_value_binding_errors(
        artifact,
        "",
        {""},
        errors,
        message="public head must match",
    )

    assert binding is None
    assert artifact["valid"] is False
    assert artifact["errors"] == ["public head must match"]
    assert errors == ["public-head-bound.json: public head must match"]


def test_record_string_value_binding_errors_rejects_malformed_artifact_rows() -> None:
    errors: list[str] = []

    binding = record_string_value_binding_errors(
        ["bad"],
        None,
        {"aa"},
        errors,
        message="public head must match",
    )

    assert binding is None
    assert errors == ["<unknown>: public head must match"]


def test_require_string_value_in_rejects_malformed_allowed_containers() -> None:
    errors: list[str] = []

    assert (
        require_string_value_in("AA", "aabb", errors, message="public head must match")
        is None
    )
    assert (
        require_string_value_in(
            "AA",
            {"aa": True},
            errors,
            message="public head must match",
        )
        is None
    )
    assert (
        require_string_value_in(
            "AA",
            {"aa", 1},
            errors,
            message="public head must match",
        )
        is None
    )

    assert errors == [
        "public head must match",
        "public head must match",
        "public head must match",
    ]


def test_evidence_artifact_digest_set_collects_valid_digests() -> None:
    valid_artifact = {
        "valid": True,
        "fingerprint": {"root_digest_hex": "AA"},
    }
    invalid_artifact = {
        "valid": False,
        "fingerprint": {"root_digest_hex": "BB"},
    }
    missing_artifact = {
        "valid": True,
        "fingerprint": {"root_digest_hex": None},
    }
    empty_artifact = {
        "valid": True,
        "fingerprint": {"root_digest_hex": ""},
    }

    assert evidence_artifact_digest_set(
        [valid_artifact, invalid_artifact, missing_artifact, empty_artifact],
        "root_digest_hex",
    ) == {"aa"}


def test_record_evidence_digest_mismatch_errors_marks_invalid() -> None:
    artifact = {
        "kind": "commitment_root",
        "path": "commitment-root.json",
        "valid": True,
        "errors": [],
        "fingerprint": {"root_digest_hex": "BB"},
    }
    errors: list[str] = []

    record_evidence_digest_mismatch_errors(
        artifacts=[artifact],
        digest_field="root_digest_hex",
        allowed_digests={"aa"},
        errors=errors,
        error="commitment_root root digest must match",
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["commitment_root root digest must match"]
    assert errors == [
        "commitment-root.json: commitment_root root digest must match"
    ]


def test_validate_bound_evidence_digest_references_accepts_valid_anchor() -> None:
    artifact = {
        "kind": "observability",
        "path": "observability.json",
        "valid": True,
        "errors": [],
        "fingerprint": {"bundle_digest_hex": "AA"},
    }
    errors: list[str] = []

    validate_bound_evidence_digest_references(
        required_kinds=("observability",),
        missing_anchor_required_kinds=("feed_promotion", "observability"),
        bound_artifacts=[("observability", artifact)],
        valid_anchor_digests={"aa"},
        digest_field="bundle_digest_hex",
        errors=errors,
        binding_error_template="{kind_name} bundle digest must match",
        missing_anchor_error_template="{kind_name} bundle digest needs anchor",
    )

    assert artifact["valid"] is True
    assert artifact["errors"] == []
    assert errors == []


def test_validate_bound_evidence_digest_references_accepts_kind_field_map() -> None:
    juror_artifact = {
        "kind": "juror_client",
        "path": "juror-client.json",
        "valid": True,
        "errors": [],
        "fingerprint": {"synced_root_digest_hex": "AA"},
    }
    service_artifact = {
        "kind": "verifier_service",
        "path": "verifier-service.json",
        "valid": True,
        "errors": [],
        "fingerprint": {"root_digest_hex": "AA"},
    }
    errors: list[str] = []

    validate_bound_evidence_digest_references(
        required_kinds=("juror_client", "verifier_service"),
        missing_anchor_required_kinds=("juror_client", "verifier_service"),
        bound_artifacts=[
            ("juror_client", juror_artifact),
            ("verifier_service", service_artifact),
        ],
        valid_anchor_digests={"aa"},
        digest_field="root_digest_hex",
        digest_field_by_kind={"juror_client": "synced_root_digest_hex"},
        errors=errors,
        binding_error_template="{kind_name} root binding must match",
        missing_anchor_error_template="{kind_name} root binding needs anchor",
    )

    assert juror_artifact["valid"] is True
    assert service_artifact["valid"] is True
    assert errors == []


def test_validate_bound_evidence_digest_references_marks_mismatch() -> None:
    artifact = {
        "kind": "observability",
        "path": "observability.json",
        "valid": True,
        "errors": [],
        "fingerprint": {"bundle_digest_hex": "BB"},
    }
    errors: list[str] = []

    validate_bound_evidence_digest_references(
        required_kinds=("observability",),
        missing_anchor_required_kinds=("feed_promotion", "observability"),
        bound_artifacts=[("observability", artifact)],
        valid_anchor_digests={"aa"},
        digest_field="bundle_digest_hex",
        errors=errors,
        binding_error_template="{kind_name} bundle digest must match",
        missing_anchor_error_template="{kind_name} bundle digest needs anchor",
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["observability bundle digest must match"]
    assert errors == ["observability.json: observability bundle digest must match"]


def test_validate_bound_evidence_digest_references_marks_missing_anchor() -> None:
    artifact = {
        "kind": "observability",
        "path": "observability.json",
        "valid": True,
        "errors": [],
        "fingerprint": {"bundle_digest_hex": "AA"},
    }
    errors: list[str] = []

    validate_bound_evidence_digest_references(
        required_kinds=("observability",),
        missing_anchor_required_kinds=("feed_promotion", "observability"),
        bound_artifacts=[("observability", artifact)],
        valid_anchor_digests=set(),
        digest_field="bundle_digest_hex",
        errors=errors,
        binding_error_template="{kind_name} bundle digest must match",
        missing_anchor_error_template="{kind_name} bundle digest needs anchor",
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["observability bundle digest needs anchor"]
    assert errors == ["observability.json: observability bundle digest needs anchor"]


def test_validate_bound_evidence_digest_references_records_summary_error() -> None:
    artifact = {
        "kind": "observability",
        "path": "observability.json",
        "valid": True,
        "errors": [],
        "fingerprint": {"bundle_digest_hex": "AA"},
    }
    errors: list[str] = []

    validate_bound_evidence_digest_references(
        required_kinds=("observability",),
        missing_anchor_required_kinds=("observability",),
        bound_artifacts=[("observability", artifact)],
        valid_anchor_digests=set(),
        digest_field="bundle_digest_hex",
        errors=errors,
        binding_error_template="{kind_name} bundle digest must match",
        missing_anchor_error_template="{kind_name} bundle digest needs anchor",
        missing_anchor_summary_error="bundle anchor missing",
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["observability bundle digest needs anchor"]
    assert errors == [
        "observability.json: observability bundle digest needs anchor",
        "bundle anchor missing",
    ]


def test_validate_bound_evidence_digest_references_skips_optional_missing_anchor() -> None:
    artifact = {
        "kind": "observability",
        "path": "observability.json",
        "valid": True,
        "errors": [],
        "fingerprint": {"bundle_digest_hex": "AA"},
    }
    errors: list[str] = []

    validate_bound_evidence_digest_references(
        required_kinds=("feed_promotion",),
        missing_anchor_required_kinds=("observability",),
        bound_artifacts=[("observability", artifact)],
        valid_anchor_digests=set(),
        digest_field="bundle_digest_hex",
        errors=errors,
        binding_error_template="{kind_name} bundle digest must match",
        missing_anchor_error_template="{kind_name} bundle digest needs anchor",
    )

    assert artifact["valid"] is True
    assert artifact["errors"] == []
    assert errors == []


def test_validate_bound_evidence_digest_references_rejects_malformed_artifact_pairs() -> None:
    errors: list[str] = []

    validate_bound_evidence_digest_references(
        required_kinds=("observability",),
        missing_anchor_required_kinds=("observability",),
        bound_artifacts="observability",
        valid_anchor_digests={"aa"},
        digest_field="bundle_digest_hex",
        errors=errors,
        binding_error_template="{kind_name} bundle digest must match",
        missing_anchor_error_template="{kind_name} bundle digest needs anchor",
    )

    assert errors == [
        "bound evidence artifacts must be a sequence of (kind, artifact) pairs"
    ]


def test_validate_bound_evidence_digest_references_rejects_malformed_missing_anchor_pairs() -> None:
    artifact = {
        "kind": "observability",
        "path": "observability.json",
        "valid": True,
        "errors": [],
        "fingerprint": {"bundle_digest_hex": "AA"},
    }
    errors: list[str] = []

    validate_bound_evidence_digest_references(
        required_kinds=("observability",),
        missing_anchor_required_kinds=("observability",),
        bound_artifacts=[("observability", artifact)],
        missing_anchor_artifacts={"observability": artifact},
        valid_anchor_digests=set(),
        digest_field="bundle_digest_hex",
        errors=errors,
        binding_error_template="{kind_name} bundle digest must match",
        missing_anchor_error_template="{kind_name} bundle digest needs anchor",
        missing_anchor_summary_error="bundle anchor missing",
    )

    assert artifact["valid"] is True
    assert artifact["errors"] == []
    assert errors == [
        "missing-anchor evidence artifacts must be a sequence of (kind, artifact) pairs"
    ]


def test_validate_bound_evidence_tuple_references_accepts_valid_anchor() -> None:
    artifact = {
        "kind": "cycle_bound",
        "path": "cycle-bound.json",
        "valid": True,
        "errors": [],
        "fingerprint": {
            "statement_bundle_digest_hex": "AA",
            "reconciliation_digest_hex": "BB",
        },
    }
    errors: list[str] = []

    validate_bound_evidence_tuple_references(
        required_kinds=("cycle_bound",),
        missing_anchor_required_kinds=("cycle_bound",),
        bound_artifacts=[("cycle_bound", artifact)],
        valid_anchor_bindings={("aa", "bb")},
        binding_fields=("statement_bundle_digest_hex", "reconciliation_digest_hex"),
        errors=errors,
        binding_error_template="{kind_name} cycle tuple must match",
        missing_anchor_error_template="{kind_name} cycle tuple needs anchor",
    )

    assert artifact["valid"] is True
    assert artifact["errors"] == []
    assert errors == []


def test_validate_bound_evidence_tuple_references_marks_mismatch() -> None:
    artifact = {
        "kind": "cycle_bound",
        "path": "cycle-bound.json",
        "valid": True,
        "errors": [],
        "fingerprint": {
            "statement_bundle_digest_hex": "AA",
            "reconciliation_digest_hex": "CC",
        },
    }
    errors: list[str] = []

    validate_bound_evidence_tuple_references(
        required_kinds=("cycle_bound",),
        missing_anchor_required_kinds=("cycle_bound",),
        bound_artifacts=[("cycle_bound", artifact)],
        valid_anchor_bindings={("aa", "bb")},
        binding_fields=("statement_bundle_digest_hex", "reconciliation_digest_hex"),
        errors=errors,
        binding_error_template="{kind_name} cycle tuple must match",
        missing_anchor_error_template="{kind_name} cycle tuple needs anchor",
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["cycle_bound cycle tuple must match"]
    assert errors == ["cycle-bound.json: cycle_bound cycle tuple must match"]


def test_validate_bound_evidence_tuple_references_marks_missing_anchor() -> None:
    artifact = {
        "kind": "cycle_bound",
        "path": "cycle-bound.json",
        "valid": True,
        "errors": [],
        "fingerprint": {
            "statement_bundle_digest_hex": "AA",
            "reconciliation_digest_hex": "BB",
        },
    }
    errors: list[str] = []

    validate_bound_evidence_tuple_references(
        required_kinds=("cycle_bound",),
        missing_anchor_required_kinds=("cycle_bound",),
        bound_artifacts=[("cycle_bound", artifact)],
        valid_anchor_bindings=set(),
        binding_fields=("statement_bundle_digest_hex", "reconciliation_digest_hex"),
        errors=errors,
        binding_error_template="{kind_name} cycle tuple must match",
        missing_anchor_error_template="{kind_name} cycle tuple needs anchor",
        missing_anchor_summary_error="cycle anchor missing",
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["cycle_bound cycle tuple needs anchor"]
    assert errors == [
        "cycle-bound.json: cycle_bound cycle tuple needs anchor",
        "cycle anchor missing",
    ]


def test_validate_bound_evidence_tuple_references_skips_optional_missing_anchor() -> None:
    artifact = {
        "kind": "cycle_bound",
        "path": "cycle-bound.json",
        "valid": True,
        "errors": [],
        "fingerprint": {
            "statement_bundle_digest_hex": "AA",
            "reconciliation_digest_hex": "BB",
        },
    }
    errors: list[str] = []

    validate_bound_evidence_tuple_references(
        required_kinds=("billing_cycle",),
        missing_anchor_required_kinds=("cycle_bound",),
        bound_artifacts=[("cycle_bound", artifact)],
        valid_anchor_bindings=set(),
        binding_fields=("statement_bundle_digest_hex", "reconciliation_digest_hex"),
        errors=errors,
        binding_error_template="{kind_name} cycle tuple must match",
        missing_anchor_error_template="{kind_name} cycle tuple needs anchor",
        missing_anchor_summary_error="cycle anchor missing",
    )

    assert artifact["valid"] is True
    assert artifact["errors"] == []
    assert errors == []


def test_validate_bound_evidence_tuple_references_rejects_malformed_artifact_pairs() -> None:
    errors: list[str] = []

    validate_bound_evidence_tuple_references(
        required_kinds=("cycle_bound",),
        missing_anchor_required_kinds=("cycle_bound",),
        bound_artifacts=[("cycle_bound", "not an artifact")],
        valid_anchor_bindings={("aa", "bb")},
        binding_fields=("statement_bundle_digest_hex", "reconciliation_digest_hex"),
        errors=errors,
        binding_error_template="{kind_name} cycle tuple must match",
        missing_anchor_error_template="{kind_name} cycle tuple needs anchor",
    )

    assert errors == [
        "bound evidence artifacts must be a sequence of (kind, artifact) pairs"
    ]


def test_record_consistent_evidence_value_reports_mismatches() -> None:
    values: dict[str, str] = {}
    errors: list[str] = []

    record_consistent_evidence_value(
        values,
        "snapshot_id_hex",
        "",
        "publish",
        errors,
    )
    assert values == {}
    assert errors == []

    record_consistent_evidence_value(
        values,
        "snapshot_id_hex",
        "aa",
        "publish",
        errors,
    )
    record_consistent_evidence_value(
        values,
        "snapshot_id_hex",
        "aa",
        "latest",
        errors,
    )
    record_consistent_evidence_value(
        values,
        "snapshot_id_hex",
        "bb",
        "provider",
        errors,
    )

    assert values == {"snapshot_id_hex": "aa"}
    assert errors == [
        "provider.snapshot_id_hex `bb` does not match `aa`",
    ]


def test_record_consistent_evidence_value_reports_malformed_values() -> None:
    values: dict[str, str] = {}
    errors: list[str] = []

    record_consistent_evidence_value(
        values,
        "snapshot_id_hex",
        ["aa"],
        "publish",
        errors,
    )
    record_consistent_evidence_value(
        values,
        "snapshot_id_hex",
        {"id": "bb"},
        "latest",
        errors,
    )

    assert values == {}
    assert errors == [
        "publish.snapshot_id_hex must be a string",
        "latest.snapshot_id_hex must be a string",
    ]


def test_record_consistent_deployment_context_records_summary() -> None:
    values: dict[str, str] = {}
    errors: list[str] = []
    artifact = {
        "fingerprint": {
            "deployment_id": "sorafs-staging-a",
            "environment": "staging",
        },
    }

    record_consistent_deployment_context(values, artifact, "gateway", errors)
    record_consistent_deployment_context(values, artifact, "settlement", errors)

    assert values == {
        "deployment_id": "sorafs-staging-a",
        "environment": "staging",
    }
    assert deployment_context_summary(values) == {
        "deployment_id": "sorafs-staging-a",
        "environment": "staging",
    }
    assert errors == []


def test_record_consistent_deployment_context_reports_mixed_context() -> None:
    values: dict[str, str] = {}
    errors: list[str] = []
    gateway = {
        "path": "gateway.json",
        "valid": True,
        "errors": [],
        "fingerprint": {
            "deployment_id": "sorafs-staging-a",
            "environment": "staging",
        },
    }
    settlement = {
        "path": "settlement.json",
        "valid": True,
        "errors": [],
        "fingerprint": {
            "deployment_id": "sorafs-staging-b",
            "environment": "release",
        },
    }

    record_consistent_deployment_context(values, gateway, "gateway", errors)
    record_consistent_deployment_context(values, settlement, "settlement", errors)

    assert deployment_context_summary(values) == {
        "deployment_id": "sorafs-staging-a",
        "environment": "staging",
    }
    assert gateway["valid"] is True
    assert gateway["errors"] == []
    assert settlement["valid"] is False
    assert settlement["errors"] == [
        "settlement.deployment_id `sorafs-staging-b` does not match "
        "`sorafs-staging-a`",
        "settlement.environment `release` does not match `staging`",
    ]
    assert errors == [
        "settlement.json: settlement.deployment_id `sorafs-staging-b` does not match "
        "`sorafs-staging-a`",
        "settlement.json: settlement.environment `release` does not match `staging`",
    ]


def test_record_observed_evidence_value_records_truthy_values() -> None:
    provider_ids: set[str] = set()
    provider_counts: set[int] = set()

    record_observed_evidence_value(provider_ids, "")
    record_observed_evidence_value(provider_ids, "provider-a")
    record_observed_evidence_value(provider_ids, "provider-a")
    record_observed_evidence_value(provider_counts, 0)
    record_observed_evidence_value(provider_counts, 3)

    assert provider_ids == {"provider-a"}
    assert provider_counts == {3}


def test_record_observed_evidence_value_skips_unhashable_values() -> None:
    values: set[object] = set()

    record_observed_evidence_value(values, ["provider-a"])
    record_observed_evidence_value(values, {"provider_id": "provider-a"})
    record_observed_evidence_value(values, "provider-a")

    assert values == {"provider-a"}


def test_record_snapshot_bound_evidence_artifact_routes_valid_records() -> None:
    valid_snapshot_bindings: set[tuple[str, str]] = set()
    snapshot_bound_artifacts: list[dict[str, object]] = []
    provider_record = {"kind": "provider"}

    record_snapshot_bound_evidence_artifact(
        kind_name="publish",
        artifact={"kind": "publish"},
        snapshot_id="AA",
        merkle_root="BB",
        valid=False,
        anchor_kinds=("publish", "latest"),
        bound_kinds=("provider", "metrics"),
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
    )
    assert valid_snapshot_bindings == set()
    assert snapshot_bound_artifacts == []

    record_snapshot_bound_evidence_artifact(
        kind_name="publish",
        artifact={"kind": "publish"},
        snapshot_id="AA",
        merkle_root="BB",
        valid=True,
        anchor_kinds=("publish", "latest"),
        bound_kinds=("provider", "metrics"),
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
    )
    assert valid_snapshot_bindings == {("aa", "bb")}
    assert snapshot_bound_artifacts == []

    record_snapshot_bound_evidence_artifact(
        kind_name="latest",
        artifact={"kind": "latest"},
        snapshot_id="CC",
        merkle_root="",
        valid=True,
        anchor_kinds=("publish", "latest"),
        bound_kinds=("provider", "metrics"),
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
    )
    assert valid_snapshot_bindings == {("aa", "bb")}

    record_snapshot_bound_evidence_artifact(
        kind_name="provider",
        artifact=provider_record,
        snapshot_id="AA",
        merkle_root="BB",
        valid=True,
        anchor_kinds=("publish", "latest"),
        bound_kinds=("provider", "metrics"),
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
    )
    assert snapshot_bound_artifacts == [provider_record]

    record_snapshot_bound_evidence_artifact(
        kind_name="unknown",
        artifact={"kind": "unknown"},
        snapshot_id="AA",
        merkle_root="BB",
        valid=True,
        anchor_kinds=("publish", "latest"),
        bound_kinds=("provider", "metrics"),
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
    )
    assert snapshot_bound_artifacts == [provider_record]


def test_record_snapshot_bound_evidence_artifact_ignores_malformed_anchor_values() -> None:
    valid_snapshot_bindings: set[tuple[str, str]] = set()
    snapshot_bound_artifacts: list[dict[str, object]] = []

    record_snapshot_bound_evidence_artifact(
        kind_name="publish",
        artifact={"kind": "publish"},
        snapshot_id=["AA"],
        merkle_root="BB",
        valid=True,
        anchor_kinds=("publish", "latest"),
        bound_kinds=("provider", "metrics"),
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
    )
    record_snapshot_bound_evidence_artifact(
        kind_name="latest",
        artifact={"kind": "latest"},
        snapshot_id="AA",
        merkle_root={"root": "BB"},
        valid=True,
        anchor_kinds=("publish", "latest"),
        bound_kinds=("provider", "metrics"),
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
    )

    assert valid_snapshot_bindings == set()
    assert snapshot_bound_artifacts == []


def test_record_snapshot_bound_evidence_artifact_rejects_malformed_kind_containers() -> None:
    valid_snapshot_bindings: set[tuple[str, str]] = set()
    snapshot_bound_artifacts: list[dict[str, object]] = []
    provider_record = {"kind": "provider"}

    record_snapshot_bound_evidence_artifact(
        kind_name="publish",
        artifact={"kind": "publish"},
        snapshot_id="AA",
        merkle_root="BB",
        valid=True,
        anchor_kinds="publish",
        bound_kinds=("provider",),
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
    )
    record_snapshot_bound_evidence_artifact(
        kind_name="provider",
        artifact=provider_record,
        snapshot_id="AA",
        merkle_root="BB",
        valid=True,
        anchor_kinds=("publish",),
        bound_kinds={"provider": True},
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
    )

    assert valid_snapshot_bindings == set()
    assert snapshot_bound_artifacts == []


def test_validate_snapshot_bound_evidence_artifacts_records_mismatches() -> None:
    required = {
        "provider": {"valid": True, "errors": [], "artifacts": []},
        "latest": {"valid": True, "errors": [], "artifacts": []},
    }
    artifact = {
        "kind": "provider",
        "path": "provider.json",
        "fingerprint": {
            "snapshot_id_hex": "cc",
            "merkle_root_hex": "dd",
        },
        "valid": True,
        "errors": [],
    }

    validate_snapshot_bound_evidence_artifacts(
        required=required,
        required_kinds=("provider", "latest"),
        bound_kinds=("provider",),
        valid_snapshot_bindings={("aa", "bb")},
        snapshot_bound_artifacts=[artifact],
        required_anchor_kind="latest",
        binding_error="binding must match anchor",
        binding_summary_error="summary binding must match anchor",
        missing_anchor_error="missing anchor",
        missing_anchor_summary_error="summary missing anchor",
        missing_required_anchor_error="required anchor missing",
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["binding must match anchor"]
    assert required["provider"]["valid"] is False
    assert required["provider"]["errors"] == [
        "provider.json: summary binding must match anchor"
    ]
    assert required["latest"]["valid"] is True
    assert required["latest"]["errors"] == []


def test_validate_snapshot_bound_evidence_artifacts_records_missing_anchor() -> None:
    required = {
        "provider": {"valid": True, "errors": [], "artifacts": []},
        "latest": {"valid": True, "errors": [], "artifacts": []},
    }
    artifact = {
        "kind": "provider",
        "path": "provider.json",
        "fingerprint": {
            "snapshot_id_hex": "aa",
            "merkle_root_hex": "bb",
        },
        "valid": True,
        "errors": [],
    }

    validate_snapshot_bound_evidence_artifacts(
        required=required,
        required_kinds=("provider", "latest"),
        bound_kinds=("provider",),
        valid_snapshot_bindings=set(),
        snapshot_bound_artifacts=[artifact],
        required_anchor_kind="latest",
        binding_error="binding must match anchor",
        binding_summary_error="summary binding must match anchor",
        missing_anchor_error="missing anchor",
        missing_anchor_summary_error="summary missing anchor",
        missing_required_anchor_error="required anchor missing",
    )

    assert artifact["valid"] is False
    assert artifact["errors"] == ["missing anchor"]
    assert required["provider"]["valid"] is False
    assert required["provider"]["errors"] == ["provider.json: summary missing anchor"]
    assert required["latest"]["valid"] is False
    assert required["latest"]["errors"] == ["required anchor missing"]


def test_finalize_custom_required_evidence_rows_fails_closed() -> None:
    required = {
        "provider": {"valid": False, "errors": [], "artifacts": [{"valid": True}]},
        "latest": {"valid": True, "errors": [], "artifacts": []},
        "metrics": {"valid": True, "errors": "bad", "artifacts": "bad"},
        "transport": {"valid": True, "errors": [], "artifacts": [{"valid": 1}]},
    }

    finalize_custom_required_evidence_rows(required, evidence_label="evidence")

    assert required["provider"]["valid"] is True
    assert required["provider"]["errors"] == []
    assert required["latest"]["valid"] is False
    assert required["latest"]["errors"] == ["missing required `latest` evidence"]
    assert required["metrics"]["valid"] is False
    assert required["metrics"]["artifacts"] == []
    assert required["metrics"]["errors"] == [
        "required `metrics` errors must be a list",
        "missing required `metrics` evidence",
    ]
    assert required["transport"]["valid"] is False


def test_finalize_custom_required_evidence_rows_rejects_error_shape_drift() -> None:
    required = {
        "provider": {"valid": True, "errors": "bad", "artifacts": [{"valid": True}]},
    }

    finalize_custom_required_evidence_rows(required, evidence_label="evidence")

    assert required["provider"]["valid"] is False
    assert required["provider"]["errors"] == [
        "required `provider` errors must be a list"
    ]
    assert required["provider"]["artifacts"] == [{"valid": True}]


def test_finalize_custom_required_evidence_rows_rejects_malformed_rows() -> None:
    required = {"provider": "bad"}

    finalize_custom_required_evidence_rows(required, evidence_label="evidence")

    assert required["provider"] == {
        "valid": False,
        "errors": ["required `provider` row must be an object"],
        "artifacts": [],
    }


def test_record_custom_required_evidence_artifact_updates_existing_rows() -> None:
    artifact = {"valid": False, "path": "provider.json"}
    required = {
        "provider": {"valid": False, "errors": "bad", "artifacts": "bad"},
        "latest": {"valid": False, "errors": [], "artifacts": []},
    }

    assert record_custom_required_evidence_artifact(
        required,
        "provider",
        artifact,
        ("provider proof failed",),
    ) is True
    assert record_custom_required_evidence_artifact(
        required,
        "ignored",
        {"valid": True},
        ("ignored",),
    ) is False

    assert required["provider"]["artifacts"] == [artifact]
    assert required["provider"]["errors"] == [
        "required `provider` errors must be a list",
        "provider proof failed",
    ]
    assert required["latest"]["artifacts"] == []


def test_record_custom_required_evidence_artifact_repairs_malformed_row() -> None:
    artifact = {"valid": True, "path": "provider.json"}
    required = {"provider": "bad"}

    assert record_custom_required_evidence_artifact(
        required,
        "provider",
        artifact,
        (),
    ) is True

    assert required["provider"] == {
        "valid": False,
        "errors": ["required `provider` row must be an object"],
        "artifacts": [artifact],
    }


def test_record_custom_required_evidence_artifact_rejects_error_shape_drift() -> None:
    artifact = {"valid": False, "path": "provider.json"}
    required = {
        "provider": {"valid": False, "errors": [], "artifacts": []},
        "metrics": {"valid": False, "errors": [], "artifacts": []},
    }

    assert record_custom_required_evidence_artifact(
        required,
        "provider",
        artifact,
        "bad",
    ) is True
    assert record_custom_required_evidence_artifact(
        required,
        "metrics",
        {"valid": False, "path": "metrics.json"},
        ("ok", 1),
    ) is True

    assert required["provider"]["errors"] == [
        "required `provider` artifact errors must be a sequence of strings"
    ]
    assert required["metrics"]["errors"] == [
        "required `metrics` artifact errors must be a sequence of strings"
    ]


def test_evidence_artifact_fingerprint_returns_mapping_or_empty() -> None:
    fingerprint = {"digest_hex": "a" * 64}

    assert evidence_artifact_fingerprint({"fingerprint": fingerprint}) is fingerprint
    assert evidence_artifact_fingerprint({"fingerprint": []}) == {}
    assert evidence_artifact_fingerprint({}) == {}


def test_evidence_artifact_detail_returns_mapping_or_empty() -> None:
    detail = {"digest_hex": "a" * 64}

    assert evidence_artifact_detail({"cycle": detail}, "cycle") is detail
    assert evidence_artifact_detail({"cycle": []}, "cycle") == {}
    assert evidence_artifact_detail({}, "cycle") == {}


def test_evidence_artifact_schema_returns_string_or_unknown() -> None:
    assert evidence_artifact_schema({"schema": "sorafs.example.v1"}) == (
        "sorafs.example.v1"
    )
    assert evidence_artifact_schema({"schema": ""}) == "<unknown>"
    assert evidence_artifact_schema({"schema": None}) == "<unknown>"
    assert evidence_artifact_schema({}) == "<unknown>"


def test_count_evidence_artifacts_sums_all_kind_buckets() -> None:
    assert count_evidence_artifacts(
        {
            "routes": [{"valid": True}, {"valid": False}],
            "governance": [{"valid": True}],
            "optional": [],
        }
    ) == 3


def test_count_evidence_artifacts_rejects_malformed_buckets_without_traceback() -> None:
    assert count_evidence_artifacts("bad") == 0
    assert count_evidence_artifacts({"routes": "bad"}) == 0
    assert count_evidence_artifacts({"routes": b"bad"}) == 0
    assert count_evidence_artifacts({"routes": None}) == 0


def test_count_recognized_evidence_artifacts_counts_custom_rows() -> None:
    assert count_recognized_evidence_artifacts(
        ({"path": "one.json"}, {"path": "two.json"})
    ) == 2
    assert count_recognized_evidence_artifacts(()) == 0


def test_count_recognized_evidence_artifacts_rejects_strings_without_traceback() -> None:
    assert count_recognized_evidence_artifacts("bad") == 0
    assert count_recognized_evidence_artifacts(b"bad") == 0
    assert count_recognized_evidence_artifacts(None) == 0


def test_recognized_evidence_artifacts_are_valid_requires_explicit_true() -> None:
    assert recognized_evidence_artifacts_are_valid(({"valid": True},)) is True
    assert recognized_evidence_artifacts_are_valid(()) is True
    assert recognized_evidence_artifacts_are_valid(({"valid": False},)) is False
    assert recognized_evidence_artifacts_are_valid(({"valid": 1},)) is False
    assert recognized_evidence_artifacts_are_valid(({},)) is False
    assert recognized_evidence_artifacts_are_valid(("bad",)) is False
    assert recognized_evidence_artifacts_are_valid("bad") is False
    assert recognized_evidence_artifacts_are_valid(None) is False


def test_count_evidence_files_counts_discovered_paths() -> None:
    assert count_evidence_files([Path("one.json"), Path("two.json")]) == 2
    assert count_evidence_files(()) == 0


def test_count_evidence_files_rejects_strings_without_traceback() -> None:
    assert count_evidence_files("evidence.json") == 0
    assert count_evidence_files(b"evidence.json") == 0
    assert count_evidence_files(None) == 0


def test_init_evidence_artifact_buckets_returns_empty_kind_lists() -> None:
    buckets = init_evidence_artifact_buckets(("route", "manifest"))

    assert buckets == {"route": [], "manifest": []}
    assert buckets["route"] is not buckets["manifest"]


def test_init_evidence_artifact_buckets_rejects_malformed_kind_names() -> None:
    assert init_evidence_artifact_buckets("route") == {}
    assert init_evidence_artifact_buckets({"route": True}) == {}
    assert init_evidence_artifact_buckets(()) == {}
    assert init_evidence_artifact_buckets(("route", "route")) == {}
    assert init_evidence_artifact_buckets(("route", 1)) == {}


def test_evidence_gate_status_reflects_summary_errors() -> None:
    assert evidence_gate_status([]) == "ready"
    assert evidence_gate_status(["missing required route rollout evidence"]) == "blocked"


def test_evidence_gate_status_fails_closed_on_malformed_errors() -> None:
    assert evidence_gate_status(None) == "blocked"
    assert evidence_gate_status("") == "blocked"
    assert evidence_gate_status(b"") == "blocked"


def test_record_evidence_validation_errors_adds_path_prefixes() -> None:
    errors = ["existing"]

    record_evidence_validation_errors(
        Path("evidence.json"),
        ["schema must be a string", "status must be `passed`"],
        errors,
    )

    assert errors == [
        "existing",
        "evidence.json: schema must be a string",
        "evidence.json: status must be `passed`",
    ]


def test_record_evidence_validation_errors_rejects_string_without_character_split() -> None:
    errors: list[str] = []

    record_evidence_validation_errors(Path("evidence.json"), "bad", errors)

    assert errors == [
        "evidence.json: validation errors must be a sequence of strings"
    ]


def test_record_evidence_validation_errors_rejects_scalar_without_traceback() -> None:
    errors: list[str] = []

    record_evidence_validation_errors(Path("evidence.json"), None, errors)

    assert errors == [
        "evidence.json: validation errors must be a sequence of strings"
    ]


def test_record_evidence_validation_errors_rejects_non_string_entry() -> None:
    errors: list[str] = []

    record_evidence_validation_errors(Path("evidence.json"), ["schema", 7], errors)

    assert errors == [
        "evidence.json: validation errors must be a sequence of strings"
    ]


def test_record_explicit_evidence_validation_errors_only_records_explicit_paths(
    tmp_path: Path,
) -> None:
    explicit = tmp_path / "explicit.json"
    discovered = tmp_path / "discovered.json"
    explicit.write_text("{}", encoding="utf-8")
    discovered.write_text("{}", encoding="utf-8")

    errors: list[str] = []
    record_explicit_evidence_validation_errors(
        explicit,
        {explicit.resolve()},
        ["schema must be a string"],
        errors,
    )
    record_explicit_evidence_validation_errors(
        discovered,
        {explicit.resolve()},
        ["schema must be a string"],
        errors,
    )

    assert errors == [f"{explicit}: schema must be a string"]


def test_build_required_evidence_summary_reports_present_valid_and_missing() -> None:
    errors: list[str] = []
    artifact = {"valid": True, "path": "ready.json"}

    summary = build_required_evidence_summary(
        ("ready", "missing"),
        {"ready": [artifact]},
        {"ready": "sorafs.ready.v1", "missing": "sorafs.missing.v1"},
        errors,
        evidence_label="rollout",
    )

    assert summary["ready"] == {
        "schema": "sorafs.ready.v1",
        "present": True,
        "valid": True,
        "artifact_count": 1,
        "artifacts": [artifact],
        "errors": [],
    }
    assert summary["missing"] == {
        "schema": "sorafs.missing.v1",
        "present": False,
        "valid": False,
        "artifact_count": 0,
        "artifacts": [],
        "errors": [],
    }
    assert errors == ["missing required missing rollout evidence"]


def test_build_required_evidence_summary_reports_invalid_release_artifacts() -> None:
    errors: list[str] = []
    artifact = {"valid": False, "path": "release.json"}

    summary = build_required_evidence_summary(
        ("archive",),
        {"archive": [artifact]},
        {"archive": "sorafs.archive.v1"},
        errors,
        evidence_label="release",
    )

    assert summary["archive"]["present"] is True
    assert summary["archive"]["valid"] is False
    assert summary["archive"]["artifact_count"] == 1
    assert summary["archive"]["artifacts"] == [artifact]
    assert errors == ["archive release evidence has invalid artifact(s)"]


def test_build_required_evidence_summary_rejects_missing_schema_metadata() -> None:
    errors: list[str] = []
    artifact = {"valid": True, "path": "ready.json", "errors": []}

    summary = build_required_evidence_summary(
        ("ready", "blank"),
        {"ready": [artifact], "blank": [artifact]},
        {"blank": ""},
        errors,
        evidence_label="rollout",
    )

    assert summary["ready"] == {
        "schema": None,
        "present": True,
        "valid": False,
        "artifact_count": 1,
        "artifacts": [artifact],
        "errors": ["required `ready` schema must be configured"],
    }
    assert summary["blank"] == {
        "schema": None,
        "present": True,
        "valid": False,
        "artifact_count": 1,
        "artifacts": [artifact],
        "errors": ["required `blank` schema must be configured"],
    }
    assert errors == [
        "required ready rollout schema must be configured",
        "required blank rollout schema must be configured",
    ]


def test_build_required_evidence_summary_rejects_malformed_metadata_maps() -> None:
    errors: list[str] = []

    summary = build_required_evidence_summary(
        ("ready",),
        "not-a-map",
        ["not", "a", "map"],
        errors,
        evidence_label="release",
    )

    assert summary["ready"] == {
        "schema": None,
        "present": False,
        "valid": False,
        "artifact_count": 0,
        "artifacts": [],
        "errors": ["required `ready` schema must be configured"],
    }
    assert errors == [
        "release artifacts by kind must be a mapping",
        "release schema by kind must be a mapping",
        "required ready release schema must be configured",
        "missing required ready release evidence",
    ]


def test_build_required_evidence_summary_rejects_mixed_deployments() -> None:
    errors: list[str] = []
    gateway = {
        "valid": True,
        "path": "gateway.json",
        "errors": [],
        "fingerprint": {
            "deployment_id": "sorafs-staging-a",
            "environment": "staging",
        },
    }
    settlement = {
        "valid": True,
        "path": "settlement.json",
        "errors": [],
        "fingerprint": {
            "deployment_id": "sorafs-staging-b",
            "environment": "release",
        },
    }

    summary = build_required_evidence_summary(
        ("gateway", "settlement"),
        {"gateway": [gateway], "settlement": [settlement]},
        {
            "gateway": "sorafs.gateway.v1",
            "settlement": "sorafs.settlement.v1",
        },
        errors,
        evidence_label="rollout",
    )

    assert summary["gateway"]["valid"] is False
    assert summary["settlement"]["valid"] is False
    assert summary["gateway"]["errors"] == [
        "rollout evidence deployment context must match across artifacts"
    ]
    assert summary["settlement"]["errors"] == [
        "rollout evidence deployment context must match across artifacts"
    ]
    assert summary["gateway"]["artifacts"][0]["valid"] is True
    assert summary["gateway"]["artifacts"][0]["errors"] == []
    assert summary["settlement"]["artifacts"][0]["valid"] is False
    assert summary["settlement"]["artifacts"][0]["errors"] == [
        "settlement.deployment_id `sorafs-staging-b` does not match "
        "`sorafs-staging-a`",
        "settlement.environment `release` does not match `staging`",
    ]
    assert errors == [
        "settlement.json: settlement.deployment_id `sorafs-staging-b` does not match "
        "`sorafs-staging-a`",
        "settlement.json: settlement.environment `release` does not match `staging`",
    ]


def test_build_required_evidence_summary_uses_fail_closed_validity() -> None:
    errors: list[str] = []
    artifacts = [{"path": "missing-valid.json"}, {"valid": 1, "path": "truthy.json"}]

    summary = build_required_evidence_summary(
        ("archive",),
        {"archive": artifacts},
        {"archive": "sorafs.archive.v1"},
        errors,
        evidence_label="release",
    )

    assert summary["archive"]["present"] is True
    assert summary["archive"]["valid"] is False
    assert summary["archive"]["artifact_count"] == 2
    assert summary["archive"]["artifacts"] == artifacts
    assert errors == ["archive release evidence has invalid artifact(s)"]


def test_build_required_evidence_summary_rejects_malformed_required_kinds() -> None:
    errors: list[str] = []
    artifact = {"valid": True, "path": "ready.json"}

    assert (
        build_required_evidence_summary(
            "ready",
            {"ready": [artifact]},
            {"ready": "sorafs.ready.v1"},
            errors,
            evidence_label="rollout",
        )
        == {}
    )
    assert (
        build_required_evidence_summary(
            {"ready": True},
            {"ready": [artifact]},
            {"ready": "sorafs.ready.v1"},
            errors,
            evidence_label="release",
        )
        == {}
    )
    assert (
        build_required_evidence_summary(
            ("ready", 1),
            {"ready": [artifact]},
            {"ready": "sorafs.ready.v1"},
            errors,
            evidence_label="canary",
        )
        == {}
    )

    assert errors == [
        "rollout required evidence kinds must be a sequence of strings",
        "release required evidence kinds must be a sequence of strings",
        "canary required evidence kinds must be a sequence of strings",
    ]


def test_build_required_evidence_summary_rejects_empty_required_kinds() -> None:
    errors: list[str] = []

    assert (
        build_required_evidence_summary(
            (),
            {},
            {},
            errors,
            evidence_label="rollout",
        )
        == {}
    )
    assert errors == ["rollout required evidence kinds must not be empty"]


def test_build_required_evidence_summary_rejects_duplicate_required_kinds() -> None:
    errors: list[str] = []

    assert (
        build_required_evidence_summary(
            ("ready", "ready", "archive", "archive"),
            {
                "ready": [{"valid": True}],
                "archive": [{"valid": True}],
            },
            {
                "ready": "sorafs.ready.v1",
                "archive": "sorafs.archive.v1",
            },
            errors,
            evidence_label="release",
        )
        == {}
    )
    assert errors == [
        "release required evidence kinds must not contain duplicates "
        "['archive', 'ready']"
    ]


def test_build_required_evidence_summary_rejects_malformed_artifact_buckets() -> None:
    errors: list[str] = []

    summary = build_required_evidence_summary(
        ("text", "mapping", "missing"),
        {
            "text": "bad",
            "mapping": {"valid": True},
        },
        {
            "text": "sorafs.text.v1",
            "mapping": "sorafs.mapping.v1",
            "missing": "sorafs.missing.v1",
        },
        errors,
        evidence_label="rollout",
    )

    for name in ("text", "mapping", "missing"):
        assert summary[name]["present"] is False
        assert summary[name]["valid"] is False
        assert summary[name]["artifact_count"] == 0
        assert summary[name]["artifacts"] == []
    assert summary["text"]["errors"] == [
        "required `text` artifacts must be a sequence"
    ]
    assert summary["mapping"]["errors"] == [
        "required `mapping` artifacts must be a sequence"
    ]
    assert summary["missing"]["errors"] == []
    assert errors == [
        "required text rollout artifacts must be a sequence",
        "required mapping rollout artifacts must be a sequence",
        "missing required missing rollout evidence",
    ]


def test_required_evidence_kind_names_returns_summary_copy() -> None:
    required_kinds = ["route", "manifest"]

    names = required_evidence_kind_names(required_kinds)
    required_kinds.append("late")

    assert names == ["route", "manifest"]


def test_required_evidence_kind_names_rejects_malformed_containers() -> None:
    assert required_evidence_kind_names("route") == []
    assert required_evidence_kind_names({"route": True}) == []
    assert required_evidence_kind_names(("route", 1)) == []


def test_required_evidence_kind_names_rejects_empty_or_duplicate_kinds() -> None:
    assert required_evidence_kind_names(()) == []
    assert required_evidence_kind_names(("route", "route")) == []
    assert required_evidence_kind_names(["route", "manifest", "route"]) == []


def test_evidence_schema_by_kind_extracts_kind_schema_map() -> None:
    class Kind:
        def __init__(self, schema: str) -> None:
            self.schema = schema

    assert evidence_schema_by_kind(
        {
            "route": Kind("sorafs.route.v1"),
            "manifest": Kind("sorafs.manifest.v1"),
        }
    ) == {
        "route": "sorafs.route.v1",
        "manifest": "sorafs.manifest.v1",
    }


def test_evidence_schema_by_kind_rejects_malformed_kind_registry() -> None:
    class Kind:
        def __init__(self, schema: object) -> None:
            self.schema = schema

    class MissingSchema:
        pass

    assert evidence_schema_by_kind("route") == {}
    assert evidence_schema_by_kind({1: Kind("sorafs.route.v1")}) == {}
    assert evidence_schema_by_kind({"": Kind("sorafs.route.v1")}) == {}
    assert evidence_schema_by_kind({"route": MissingSchema()}) == {}
    assert evidence_schema_by_kind({"route": Kind("")}) == {}
    assert evidence_schema_by_kind({"route": Kind(1)}) == {}
    assert evidence_schema_by_kind(
        {
            "route": Kind("sorafs.route.v1"),
            "manifest": Kind(""),
        }
    ) == {}


def test_require_object_returns_object_values() -> None:
    errors: list[str] = []
    payload = {"ok": True}

    assert require_object(payload, "payload", errors) is payload
    assert errors == []


def test_require_object_reports_non_object_values() -> None:
    errors: list[str] = []

    assert require_object([], "payload.routes[0]", errors) == {}
    assert errors == ["payload.routes[0] must be an object"]


def test_require_object_array_returns_indexed_object_records() -> None:
    errors: list[str] = []
    payload = {"routes": [{"name": "healthz"}, {"name": "status"}]}

    assert require_object_array(payload, "routes", errors) == [
        (0, {"name": "healthz"}),
        (1, {"name": "status"}),
    ]
    assert errors == []


def test_require_object_array_reports_missing_or_empty_arrays() -> None:
    errors: list[str] = []

    assert require_object_array({}, "routes", errors) == []
    assert require_object_array({"routes": []}, "routes", errors) == []

    assert errors == [
        "routes must be a non-empty array",
        "routes must be a non-empty array",
    ]


def test_require_object_array_reports_non_object_items() -> None:
    errors: list[str] = []

    assert require_object_array({"routes": ["bad"]}, "routes", errors) == [(0, {})]
    assert errors == ["routes[0] must be an object"]


def test_require_object_array_rejects_malformed_payloads() -> None:
    errors: list[str] = []

    assert require_object_array("bad", "routes", errors) == []

    assert errors == ["payload must be an object"]


def test_require_string_returns_stripped_non_empty_values() -> None:
    errors: list[str] = []

    assert require_string({"field": " value "}, "field", errors) == "value"
    assert errors == []


def test_require_string_reports_blank_or_non_string_values() -> None:
    errors: list[str] = []

    assert require_string({"field": "   "}, "field", errors) == ""
    assert require_string({"count": 1}, "count", errors) == ""
    assert errors == [
        "field must be a non-empty string",
        "count must be a non-empty string",
    ]


def test_require_string_type_returns_string_values_without_trimming() -> None:
    errors: list[str] = []

    assert require_string_type({"schema": ""}, "schema", errors) == ""
    assert require_string_type({"schema": " value "}, "schema", errors) == " value "
    assert errors == []


def test_require_string_type_reports_non_string_values_with_path() -> None:
    errors: list[str] = []

    assert require_string_type({"schema": 1}, "schema", errors) is None
    assert (
        require_string_type({"schema": None}, "schema", errors, path="artifact.schema")
        is None
    )

    assert errors == [
        "schema must be a string",
        "artifact.schema must be a string",
    ]


def test_require_string_helpers_reject_malformed_payloads() -> None:
    errors: list[str] = []

    assert require_string("bad", "schema", errors) == ""
    assert require_string_type("bad", "schema", errors) is None

    assert errors == [
        "payload must be an object",
        "payload must be an object",
    ]


def test_require_count_value_equal_accepts_matching_count() -> None:
    errors: list[str] = []

    require_count_value_equal(
        {"logged_session_count": 3},
        "logged_session_count",
        3,
        "session_count",
        errors,
    )

    assert errors == []


def test_require_count_value_equal_reports_mismatched_count() -> None:
    errors: list[str] = []

    require_count_value_equal(
        {"logged_session_count": 2},
        "logged_session_count",
        3,
        "session_count",
        errors,
    )

    assert errors == ["logged_session_count must equal session_count"]


def test_require_count_value_equal_skips_invalid_expected_count() -> None:
    errors: list[str] = []

    require_count_value_equal(
        {"logged_session_count": 2},
        "logged_session_count",
        0,
        "session_count",
        errors,
    )

    assert errors == []


def test_require_known_schema_returns_mapped_kind() -> None:
    errors: list[str] = []
    kinds = {"sorafs.test.v1": "test-kind"}

    assert require_known_schema(
        {"schema": "sorafs.test.v1"}, kinds, "SoraFS test artifact", errors
    ) == "test-kind"
    assert errors == []


def test_require_known_schema_reports_type_and_unknown_schema_errors() -> None:
    errors: list[str] = []
    kinds = {"sorafs.test.v1": "test-kind"}

    assert require_known_schema({"schema": 1}, kinds, "SoraFS test artifact", errors) is None
    assert (
        require_known_schema(
            {"schema": "sorafs.other.v1"},
            kinds,
            "SoraFS test artifact",
            errors,
        )
        is None
    )

    assert errors == [
        "schema must be a string",
        "schema `sorafs.other.v1` is not a recognized SoraFS test artifact",
    ]


def test_require_known_schema_rejects_malformed_schema_registry() -> None:
    errors: list[str] = []

    assert (
        require_known_schema(
            {"schema": "sorafs.test.v1"},
            "bad-registry",
            "SoraFS test artifact",
            errors,
        )
        is None
    )

    assert errors == ["SoraFS test artifact schema registry must be a mapping"]


def test_validate_standard_evidence_payload_runs_shared_wrapper_checks() -> None:
    class Kind:
        name = "route"

    seen: list[tuple[str, str]] = []

    def validate_kind(kind: Kind, payload: dict[str, object], errors: list[str]) -> None:
        seen.append((kind.name, str(payload.get("status"))))
        errors.append("kind-specific failure")

    kind_name, errors = validate_standard_evidence_payload(
        {
            "schema": "sorafs.test.v1",
            "status": "passed",
            "deployment_id": "prod-sorafs-1",
            "environment": "production",
            "responseBody": "payload leaked",
        },
        {"sorafs.test.v1": Kind()},
        "SoraFS test artifact",
        {"response_body"},
        "rollout evidence",
        validate_kind,
        require_reviewed_deployment_context=True,
    )

    assert kind_name == "route"
    assert seen == [("route", "passed")]
    assert errors == [
        "responseBody must not be present in rollout evidence",
        "kind-specific failure",
    ]


def test_validate_standard_evidence_payload_rejects_malformed_payload() -> None:
    kind_name, errors = validate_standard_evidence_payload(
        "bad-payload",
        {"sorafs.test.v1": object()},
        "SoraFS test artifact",
        set(),
        "rollout evidence",
        lambda _kind, _payload, _errors: None,
    )

    assert kind_name is None
    assert errors == ["payload must be an object"]


def test_validate_standard_evidence_payload_rejects_malformed_kind_name() -> None:
    class MissingName:
        pass

    called = False

    def validate_kind(
        _kind: MissingName,
        _payload: dict[str, object],
        _errors: list[str],
    ) -> None:
        nonlocal called
        called = True

    kind_name, errors = validate_standard_evidence_payload(
        {"schema": "sorafs.test.v1"},
        {"sorafs.test.v1": MissingName()},
        "SoraFS test artifact",
        set(),
        "rollout evidence",
        validate_kind,
    )

    assert kind_name is None
    assert called is False
    assert errors == ["SoraFS test artifact schema kind must have a non-empty name"]


def test_require_string_equal_accepts_exact_matches() -> None:
    errors: list[str] = []

    require_string_equal({"schema": "expected"}, "schema", "expected", errors)

    assert errors == []


def test_require_string_equal_reports_mismatches() -> None:
    errors: list[str] = []

    require_string_equal({"schema": "other"}, "schema", "expected", errors)

    assert errors == ["schema must be `expected`"]


def test_require_string_equal_supports_path_and_unquoted_expected_label() -> None:
    errors: list[str] = []

    require_string_equal(
        {"schema": "other"},
        "schema",
        "sorafs.test.v1",
        errors,
        path="artifact.schema",
        quote_expected=False,
    )

    assert errors == ["artifact.schema must be sorafs.test.v1"]


def test_require_string_equal_rejects_malformed_payloads() -> None:
    errors: list[str] = []

    require_string_equal("bad", "schema", "expected", errors)

    assert errors == ["payload must be an object"]


def test_require_string_value_equal_accepts_matching_values() -> None:
    errors: list[str] = []

    assert (
        require_string_value_equal(
            "provider-a",
            "proof.provider_id",
            "provider-a",
            "provider.provider_id",
            errors,
        )
        == "provider-a"
    )

    assert errors == []


def test_require_string_value_equal_reports_mismatches() -> None:
    errors: list[str] = []

    assert (
        require_string_value_equal(
            "provider-b",
            "proof.provider_id",
            "provider-a",
            "provider.provider_id",
            errors,
        )
        == "provider-b"
    )

    assert errors == ["proof.provider_id must match provider.provider_id"]


def test_require_string_value_equal_reports_blank_or_non_string_values() -> None:
    errors: list[str] = []

    assert (
        require_string_value_equal(
            ["provider-a"],
            "proof.provider_id",
            "provider-a",
            "provider.provider_id",
            errors,
        )
        == ""
    )
    assert (
        require_string_value_equal(
            "",
            "proof.provider_id",
            "provider-a",
            "provider.provider_id",
            errors,
        )
        == ""
    )
    assert (
        require_string_value_equal(
            "provider-a",
            "proof.provider_id",
            {},
            "provider.provider_id",
            errors,
        )
        == "provider-a"
    )
    assert (
        require_string_value_equal(
            "provider-a",
            "proof.provider_id",
            "",
            "provider.provider_id",
            errors,
        )
        == "provider-a"
    )

    assert errors == [
        "proof.provider_id must be a non-empty string",
        "proof.provider_id must be a non-empty string",
        "provider.provider_id must be a non-empty string",
        "provider.provider_id must be a non-empty string",
    ]


def test_require_string_in_accepts_allowed_string_values() -> None:
    errors: list[str] = []

    assert (
        require_string_in(
            {"manual_trigger_route_state": "wired"},
            "manual_trigger_route_state",
            ("wired", "retired"),
            errors,
        )
        == "wired"
    )

    assert errors == []


def test_require_string_in_reports_disallowed_values() -> None:
    errors: list[str] = []

    assert (
        require_string_in(
            {"manual_trigger_route_state": "missing"},
            "manual_trigger_route_state",
            ("wired", "retired"),
            errors,
        )
        == ""
    )

    assert errors == ["manual_trigger_route_state must be `wired` or `retired`"]


def test_require_string_in_rejects_malformed_allowed_containers() -> None:
    errors: list[str] = []

    assert (
        require_string_in(
            {"manual_trigger_route_state": "wired"},
            "manual_trigger_route_state",
            "wired-retired",
            errors,
        )
        == ""
    )
    assert (
        require_string_in(
            {"manual_trigger_route_state": "wired"},
            "manual_trigger_route_state",
            {"wired": True},
            errors,
            path="archive.manual_trigger_route_state",
        )
        == ""
    )
    assert (
        require_string_in(
            {"manual_trigger_route_state": "wired"},
            "manual_trigger_route_state",
            ("wired", 1),
            errors,
        )
        == ""
    )

    assert errors == [
        "manual_trigger_route_state allowed values must be a sequence of strings",
        (
            "archive.manual_trigger_route_state allowed values must be a "
            "sequence of strings"
        ),
        "manual_trigger_route_state allowed values must be a sequence of strings",
    ]


def test_require_rollout_deployment_id_accepts_reviewed_ids() -> None:
    errors: list[str] = []

    assert (
        require_rollout_deployment_id(
            {"deployment_id": "orderbook-staging-a"}, errors
        )
        == "orderbook-staging-a"
    )
    assert (
        require_rollout_deployment_id(
            {"deployment_id": "reference.sdk-release_2026-06"}, errors
        )
        == "reference.sdk-release_2026-06"
    )

    assert errors == []


def test_require_rollout_deployment_id_rejects_unreviewed_markers() -> None:
    errors: list[str] = []

    assert require_rollout_deployment_id({"deployment_id": "orderbook-dev-a"}, errors) == ""
    assert require_rollout_deployment_id({"deployment_id": "mock.release"}, errors) == ""
    assert require_rollout_deployment_id({"deployment_id": "gateway-localnet"}, errors) == ""

    assert errors == [
        "deployment_id must not contain non-reviewed deployment markers ['dev']",
        "deployment_id must not contain non-reviewed deployment markers ['mock']",
        "deployment_id must not contain non-reviewed deployment markers ['localnet']",
    ]


def test_require_rollout_deployment_id_rejects_compact_handoff_markers() -> None:
    errors: list[str] = []

    assert (
        require_rollout_deployment_id(
            {"deployment_id": "gateway-notproductionready-a"}, errors
        )
        == ""
    )
    assert (
        require_rollout_deployment_id(
            {"deployment_id": "reference-replacebeforeproduction-202606"},
            errors,
        )
        == ""
    )
    assert (
        require_rollout_deployment_id(
            {"deployment_id": "orderbook-placeholderreview"}, errors
        )
        == ""
    )
    assert (
        require_rollout_deployment_id(
            {"deployment_id": "repair-nonproduction-a"}, errors
        )
        == ""
    )
    assert (
        require_rollout_deployment_id(
            {"deployment_id": "gateway-notprod-a"}, errors
        )
        == ""
    )
    assert (
        require_rollout_deployment_id(
            {"deployment_id": "reference-development-lane"}, errors
        )
        == ""
    )

    assert errors == [
        "deployment_id must not contain non-reviewed deployment markers "
        "['notproductionready']",
        "deployment_id must not contain non-reviewed deployment markers "
        "['replacebeforeproduction']",
        "deployment_id must not contain non-reviewed deployment markers ['placeholder']",
        "deployment_id must not contain non-reviewed deployment markers "
        "['nonproduction']",
        "deployment_id must not contain non-reviewed deployment markers ['notprod']",
        "deployment_id must not contain non-reviewed deployment markers "
        "['development']",
    ]


def test_require_rollout_deployment_id_rejects_invalid_shape() -> None:
    errors: list[str] = []

    assert require_rollout_deployment_id({"deployment_id": "-prod"}, errors) == ""
    assert require_rollout_deployment_id({"deployment_id": "prod lane"}, errors) == ""
    assert require_rollout_deployment_id({"deployment_id": "a" * 129}, errors) == ""
    assert require_rollout_deployment_id({"deployment_id": ""}, errors) == ""

    assert errors == [
        "deployment_id must be 1-128 ASCII letters, digits, '.', '_' or '-' "
        "and start/end with a letter or digit",
        "deployment_id must be 1-128 ASCII letters, digits, '.', '_' or '-' "
        "and start/end with a letter or digit",
        "deployment_id must be 1-128 ASCII letters, digits, '.', '_' or '-' "
        "and start/end with a letter or digit",
        "deployment_id must be a non-empty string",
    ]


def test_require_iroha_config_binding_accepts_full_binding() -> None:
    errors: list[str] = []

    require_iroha_config_binding(
        {"iroha_config_bound": True, "config_source": "iroha_config"},
        errors,
    )

    assert errors == []


def test_require_iroha_config_binding_reports_bound_and_source_errors() -> None:
    errors: list[str] = []

    require_iroha_config_binding(
        {"iroha_config_bound": False, "config_source": "environment"},
        errors,
    )

    assert errors == [
        "iroha_config_bound must be true",
        "config_source must be `iroha_config`",
    ]


def test_require_iroha_config_binding_supports_split_call_sites() -> None:
    errors: list[str] = []

    require_iroha_config_binding(
        {"iroha_config_bound": True},
        errors,
        source_field=None,
    )
    require_iroha_config_binding(
        {"config_source": "iroha_config"},
        errors,
        bound_field=None,
    )

    assert errors == []


def test_require_governance_approval_accepts_recorded_approval() -> None:
    errors: list[str] = []

    require_governance_approval(
        {"approved": True, "governance_vote_recorded": True},
        errors,
    )

    assert errors == []


def test_require_governance_approval_reports_missing_flags() -> None:
    errors: list[str] = []

    require_governance_approval(
        {"approved": False, "governance_vote_recorded": False},
        errors,
    )

    assert errors == [
        "approved must be true",
        "governance_vote_recorded must be true",
    ]


def test_require_config_backed_governance_approval_accepts_config_binding() -> None:
    errors: list[str] = []

    require_config_backed_governance_approval(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "config_source": "iroha_config",
        },
        errors,
    )

    assert errors == []


def test_require_config_backed_governance_approval_reports_missing_contract() -> None:
    errors: list[str] = []

    require_config_backed_governance_approval(
        {
            "approved": False,
            "governance_vote_recorded": False,
            "iroha_config_bound": False,
            "config_source": "env",
        },
        errors,
    )

    assert errors == [
        "approved must be true",
        "governance_vote_recorded must be true",
        "iroha_config_bound must be true",
        "config_source must be `iroha_config`",
    ]


def test_config_and_governance_helpers_reject_malformed_payloads() -> None:
    errors: list[str] = []

    require_iroha_config_binding("bad", errors)
    require_governance_approval([], errors)
    require_config_backed_governance_approval(None, errors)

    assert errors == ["payload must be an object"] * 3


def test_require_rollout_environment_accepts_reviewed_labels() -> None:
    errors: list[str] = []

    assert require_rollout_environment({"environment": "staging"}, errors) == "staging"
    assert require_rollout_environment({"environment": "production"}, errors) == "production"
    assert require_rollout_environment({"environment": "prod"}, errors) == "prod"
    assert require_rollout_environment({"environment": "release"}, errors) == "release"

    assert errors == []


def test_require_rollout_environment_rejects_unreviewed_labels() -> None:
    errors: list[str] = []

    assert require_rollout_environment({"environment": "dev"}, errors) == ""
    assert require_rollout_environment({"environment": "localnet"}, errors) == ""
    assert require_rollout_environment({"environment": "mock"}, errors) == ""
    assert require_rollout_environment({"environment": ""}, errors) == ""

    assert errors == [
        "environment must be one of ['prod', 'production', 'release', 'staging']",
        "environment must be one of ['prod', 'production', 'release', 'staging']",
        "environment must be one of ['prod', 'production', 'release', 'staging']",
        "environment must be a non-empty string",
    ]


def test_require_passed_status_accepts_passed_status() -> None:
    errors: list[str] = []

    require_passed_status({"status": "passed"}, errors)

    assert errors == []


def test_require_passed_status_reports_missing_or_non_passed_status() -> None:
    errors: list[str] = []

    require_passed_status({}, errors)
    require_passed_status({"status": "failed"}, errors, path="artifact.status")

    assert errors == [
        "status must be passed",
        "artifact.status must be passed",
    ]


def test_require_status_in_accepts_allowed_statuses() -> None:
    errors: list[str] = []

    assert require_status_in({"status": "verified"}, ("verified",), errors) == "verified"
    assert (
        require_status_in(
            {},
            ("accepted", "published", "ready", "ok"),
            errors,
            path="snapshot.status",
            allow_absent=True,
        )
        == ""
    )

    assert errors == []


def test_require_status_in_reports_disallowed_statuses() -> None:
    errors: list[str] = []

    assert require_status_in({"status": "failed"}, ("verified",), errors) == ""
    assert (
        require_status_in(
            {"status": "failed"},
            ("accepted", "published", "ready", "ok"),
            errors,
            path="snapshot.status",
            allow_absent=True,
        )
        == ""
    )

    assert errors == [
        "status must be verified",
        "snapshot.status must be accepted/published/ready/ok when present",
    ]


def test_require_status_in_rejects_malformed_allowed_containers() -> None:
    errors: list[str] = []

    assert require_status_in({"status": "verified"}, "verified", errors) == ""
    assert (
        require_status_in(
            {"status": "verified"},
            {"verified": True},
            errors,
            path="snapshot.status",
        )
        == ""
    )
    assert (
        require_status_in(
            {},
            ("verified", 1),
            errors,
            path="optional.status",
            allow_absent=True,
        )
        == ""
    )

    assert errors == [
        "status allowed statuses must be a sequence of strings",
        "snapshot.status allowed statuses must be a sequence of strings",
        "optional.status allowed statuses must be a sequence of strings",
    ]


def test_require_status_helpers_reject_malformed_payloads() -> None:
    errors: list[str] = []

    require_passed_status("bad", errors)
    assert require_status_in("bad", ("verified",), errors) == ""

    assert errors == [
        "payload must be an object",
        "payload must be an object",
    ]


def test_is_hex_accepts_exact_hex_lengths() -> None:
    assert is_hex("abcdef012345", 12)
    assert is_hex("ABCDEF012345", 12)


def test_is_hex_rejects_wrong_length_or_non_hex_values() -> None:
    assert not is_hex("abcd", 8)
    assert not is_hex("not-hex!", 8)


def test_require_hex_returns_lowercase_hex_values() -> None:
    errors: list[str] = []

    assert (
        require_hex({"digest": "ABCDEF012345"}, "digest", 12, errors)
        == "abcdef012345"
    )
    assert errors == []


def test_require_hex_reports_missing_or_invalid_values() -> None:
    errors: list[str] = []

    assert require_hex({"digest": "xyz"}, "digest", 12, errors) == ""
    assert require_hex({}, "missing", 12, errors) == ""
    assert errors == [
        "digest must be 12 hex characters",
        "missing must be a non-empty string",
    ]


def test_require_hex_uses_path_override_for_errors() -> None:
    errors: list[str] = []

    assert (
        require_hex({"digest": "not-hex"}, "digest", 12, errors, path="items[0].digest")
        == ""
    )
    assert require_hex({}, "digest", 12, errors, path="items[1].digest") == ""
    assert errors == [
        "items[0].digest must be 12 hex characters",
        "items[1].digest must be a non-empty string",
    ]


def test_require_hex_rejects_malformed_payloads() -> None:
    errors: list[str] = []

    assert require_hex("bad", "digest", 12, errors) == ""

    assert errors == ["payload must be an object"]


def test_require_policy_digest_normalizes_valid_digest() -> None:
    errors: list[str] = []

    assert require_policy_digest({"policy_digest_hex": "AB" * 32}, errors) == "ab" * 32
    assert errors == []


def test_require_policy_digest_reports_missing_or_bad_digest() -> None:
    errors: list[str] = []

    assert require_policy_digest({}, errors) == ""
    assert require_policy_digest({"policy_digest_hex": "not-hex"}, errors) == ""

    assert errors == [
        "policy_digest_hex must be a non-empty string",
        "policy_digest_hex must be 64 hex characters",
    ]


def test_require_optional_hex_accepts_absent_null_and_hex_values() -> None:
    errors: list[str] = []

    require_optional_hex({}, "digest", 12, errors)
    require_optional_hex({"digest": None}, "digest", 12, errors)
    require_optional_hex({"digest": "ABCDEF012345"}, "digest", 12, errors)

    assert errors == []


def test_require_optional_hex_reports_invalid_values() -> None:
    errors: list[str] = []

    require_optional_hex({"digest": "xyz"}, "digest", 12, errors)
    require_optional_hex({"other": 1}, "other", 12, errors)

    assert errors == [
        "digest must be null or 12 hex characters",
        "other must be null or 12 hex characters",
    ]


def test_require_optional_hex_rejects_malformed_payloads() -> None:
    errors: list[str] = []

    require_optional_hex("bad", "digest", 12, errors)

    assert errors == ["payload must be an object"]


def test_require_hex_string_array_accepts_optional_arrays() -> None:
    errors: list[str] = []

    assert (
        require_hex_string_array({"siblings_hex": []}, "siblings_hex", 12, errors)
        == []
    )
    assert (
        require_hex_string_array(
            {"siblings_hex": ["ABCDEF012345"]}, "siblings_hex", 12, errors
        )
        == ["abcdef012345"]
    )
    assert errors == []


def test_require_hex_string_array_reports_required_array_shape() -> None:
    errors: list[str] = []

    assert require_hex_string_array({}, "siblings_hex", 12, errors) == []
    assert (
        require_hex_string_array(
            {"digests": []}, "digests", 12, errors, non_empty=True
        )
        == []
    )

    assert errors == [
        "siblings_hex must be an array",
        "digests must be a non-empty array",
    ]


def test_require_hex_string_array_reports_length_invalid_and_duplicate_values() -> None:
    errors: list[str] = []

    assert require_hex_string_array(
        {"digests": ["ABCDEF012345", "bad", "abcdef012345"]},
        "digests",
        12,
        errors,
        non_empty=True,
        expected_length=2,
        expected_length_label="digest_count",
        unique=True,
    ) == ["abcdef012345", "abcdef012345"]

    assert errors == [
        "digests length must equal digest_count",
        "digests[1] must be 12 hex characters",
        "digests[2] must be unique",
    ]


def test_require_hex_string_array_uses_path_override() -> None:
    errors: list[str] = []

    require_hex_string_array(
        {"siblings_hex": [1]},
        "siblings_hex",
        12,
        errors,
        path="proof.siblings_hex",
    )

    assert errors == ["proof.siblings_hex[0] must be 12 hex characters"]


def test_require_hex_string_array_rejects_malformed_payloads() -> None:
    errors: list[str] = []

    assert require_hex_string_array("bad", "siblings_hex", 12, errors) == []

    assert errors == ["payload must be an object"]


def test_require_bool_true_accepts_true() -> None:
    errors: list[str] = []

    require_bool_true({"ready": True}, "ready", errors)

    assert errors == []


def test_require_bool_true_rejects_false_or_missing_values() -> None:
    errors: list[str] = []

    require_bool_true({"ready": False}, "ready", errors)
    require_bool_true({}, "missing", errors)

    assert errors == ["ready must be true", "missing must be true"]


def test_require_bool_true_uses_path_override_in_errors() -> None:
    errors: list[str] = []

    require_bool_true(
        {"passed": False},
        "passed",
        errors,
        path="routes[0].passed",
    )

    assert errors == ["routes[0].passed must be true"]


def test_require_bool_true_rejects_malformed_payloads() -> None:
    errors: list[str] = []

    require_bool_true("bad", "ready", errors)

    assert errors == ["payload must be an object"]


def test_require_false_accepts_false() -> None:
    errors: list[str] = []

    require_false({"included": False}, "included", errors)

    assert errors == []


def test_require_false_rejects_true_or_missing_values() -> None:
    errors: list[str] = []

    require_false({"included": True}, "included", errors)
    require_false({}, "missing", errors)

    assert errors == ["included must be false", "missing must be false"]


def test_require_false_rejects_malformed_payloads() -> None:
    errors: list[str] = []

    require_false("bad", "included", errors)

    assert errors == ["payload must be an object"]


def test_require_false_or_absent_accepts_false_or_missing_values() -> None:
    errors: list[str] = []

    require_false_or_absent({"debug": False}, "debug", errors)
    require_false_or_absent({"debug": None}, "debug", errors)
    require_false_or_absent({}, "missing", errors)

    assert errors == []


def test_require_false_or_absent_rejects_non_false_values() -> None:
    errors: list[str] = []

    require_false_or_absent({"debug": True}, "debug", errors)
    require_false_or_absent({"string": "false"}, "string", errors)
    require_false_or_absent({"numeric": 0}, "numeric", errors)
    require_false_or_absent({"object": {}}, "object", errors)

    assert errors == [
        "debug must be false when present",
        "string must be false when present",
        "numeric must be false when present",
        "object must be false when present",
    ]


def test_require_false_or_absent_rejects_malformed_payloads() -> None:
    errors: list[str] = []

    require_false_or_absent("bad", "debug", errors)

    assert errors == ["payload must be an object"]


def test_require_false_or_governed_accepts_false_or_governed_true() -> None:
    errors: list[str] = []

    require_false_or_governed(
        {"execution_enabled": False},
        "execution_enabled",
        "execution_governed",
        errors,
    )
    require_false_or_governed(
        {"execution_enabled": True, "execution_governed": True},
        "execution_enabled",
        "execution_governed",
        errors,
    )

    assert errors == []


def test_require_false_or_governed_rejects_ungoverned_or_non_bool_values() -> None:
    errors: list[str] = []

    require_false_or_governed(
        {"execution_enabled": True},
        "execution_enabled",
        "execution_governed",
        errors,
    )
    require_false_or_governed({"string": "false"}, "string", "string_governed", errors)
    require_false_or_governed({"numeric": 0}, "numeric", "numeric_governed", errors)
    require_false_or_governed({"missing": None}, "missing", "missing_governed", errors)

    assert errors == [
        "execution_governed must be true",
        "string must be false or explicitly governed",
        "numeric must be false or explicitly governed",
        "missing must be false or explicitly governed",
    ]


def test_require_false_or_governed_rejects_malformed_payloads() -> None:
    errors: list[str] = []

    require_false_or_governed("bad", "execution_enabled", "execution_governed", errors)

    assert errors == ["payload must be an object"]


def test_require_positive_int_returns_positive_integers() -> None:
    errors: list[str] = []

    assert require_positive_int({"count": 3}, "count", errors) == 3
    assert errors == []


def test_require_positive_int_rejects_bool_zero_and_negative_values() -> None:
    errors: list[str] = []

    assert require_positive_int({"flag": True}, "flag", errors) == 0
    assert require_positive_int({"zero": 0}, "zero", errors) == 0
    assert require_positive_int({"negative": -1}, "negative", errors) == 0
    assert errors == [
        "flag must be a positive integer",
        "zero must be a positive integer",
        "negative must be a positive integer",
    ]


def test_require_numeric_integer_helpers_reject_malformed_payloads() -> None:
    errors: list[str] = []

    assert require_positive_int("bad", "count", errors) == 0
    assert require_minimum_int([], "provider_count", 3, errors) == 0
    assert require_non_negative_int(None, "lag", errors) == 0
    assert (
        require_int_range(
            7,
            "score",
            errors,
            min_value=2,
            max_value=10,
            path="provider.score_bps",
        )
        == 2
    )
    assert require_advancing_int_pair("bad", "since", "next_since", errors) == (0, 0)
    assert require_maximum_int(("bad",), "latency", 10, errors, minimum=1) == 1

    assert errors == ["payload must be an object"] * 6


def test_require_minimum_int_accepts_values_at_or_above_minimum() -> None:
    errors: list[str] = []

    assert require_minimum_int({"provider_count": 3}, "provider_count", 3, errors) == 3
    assert require_minimum_int({"provider_count": 4}, "provider_count", 3, errors) == 4
    assert errors == []


def test_require_minimum_int_reports_values_below_minimum() -> None:
    errors: list[str] = []

    assert require_minimum_int({"provider_count": 2}, "provider_count", 3, errors) == 2

    assert errors == ["provider_count must be at least 3"]


def test_require_minimum_int_reuses_positive_integer_validation() -> None:
    errors: list[str] = []

    assert require_minimum_int({"provider_count": 0}, "provider_count", 3, errors) == 0

    assert errors == [
        "provider_count must be a positive integer",
        "provider_count must be at least 3",
    ]


def test_require_string_not_equal_accepts_other_values() -> None:
    errors: list[str] = []

    assert (
        require_string_not_equal(
            {"proof_system": "groth16_membership_v1"},
            "proof_system",
            "transcript_digest_v1",
            errors,
        )
        == "groth16_membership_v1"
    )

    assert errors == []


def test_require_string_not_equal_rejects_disallowed_value() -> None:
    errors: list[str] = []

    assert (
        require_string_not_equal(
            {"proof_system": "transcript_digest_v1"},
            "proof_system",
            "transcript_digest_v1",
            errors,
            message="proof_system must be production proof backend",
        )
        == "transcript_digest_v1"
    )

    assert errors == ["proof_system must be production proof backend"]


def test_require_minimum_value_accepts_values_at_or_above_minimum() -> None:
    errors: list[str] = []

    assert require_minimum_value(3, "provider_count", 3, errors) == 3
    assert require_minimum_value(4, "provider_count", 3, errors) == 4

    assert errors == []


def test_require_minimum_value_reports_values_below_minimum() -> None:
    errors: list[str] = []

    assert require_minimum_value(2, "provider_count", 3, errors) == 2

    assert errors == ["provider_count must be at least 3"]


def test_require_minimum_value_supports_custom_messages() -> None:
    errors: list[str] = []

    assert (
        require_minimum_value(
            1,
            "result_count",
            2,
            errors,
            message="result_count must be at least quorum",
        )
        == 1
    )

    assert errors == ["result_count must be at least quorum"]


def test_require_maximum_value_accepts_values_at_or_below_maximum() -> None:
    errors: list[str] = []

    assert require_maximum_value(5, "quorum", 5, errors) == 5
    assert require_maximum_value(4, "quorum", 5, errors) == 4

    assert errors == []


def test_require_maximum_value_reports_values_above_maximum() -> None:
    errors: list[str] = []

    assert require_maximum_value(6, "quorum", 5, errors) == 6

    assert errors == ["quorum must be <= 5"]


def test_require_maximum_value_supports_custom_messages() -> None:
    errors: list[str] = []

    assert (
        require_maximum_value(
            8,
            "quorum",
            7,
            errors,
            message="quorum must be <= panel_size",
        )
        == 8
    )

    assert errors == ["quorum must be <= panel_size"]


def test_require_non_negative_int_returns_zero_and_positive_integers() -> None:
    errors: list[str] = []

    assert require_non_negative_int({"zero": 0}, "zero", errors) == 0
    assert require_non_negative_int({"count": 3}, "count", errors) == 3
    assert errors == []


def test_require_non_negative_int_rejects_bool_and_negative_values() -> None:
    errors: list[str] = []

    assert require_non_negative_int({"flag": False}, "flag", errors) == 0
    assert require_non_negative_int({"negative": -1}, "negative", errors) == 0
    assert errors == [
        "flag must be a non-negative integer",
        "negative must be a non-negative integer",
    ]


def test_require_zero_count_accepts_zero_counts() -> None:
    errors: list[str] = []

    require_zero_count({"failure_count": 0}, "failure_count", errors)

    assert errors == []


def test_require_zero_count_reports_non_zero_counts() -> None:
    errors: list[str] = []

    require_zero_count({"failure_count": 2}, "failure_count", errors)

    assert errors == ["failure_count must be 0"]


def test_require_zero_count_reuses_non_negative_integer_validation() -> None:
    errors: list[str] = []

    require_zero_count({"failure_count": True}, "failure_count", errors)

    assert errors == ["failure_count must be a non-negative integer"]


def test_require_int_range_accepts_inclusive_bounds() -> None:
    errors: list[str] = []

    assert (
        require_int_range(
            {"score": 0}, "score", errors, min_value=0, max_value=10_000
        )
        == 0
    )
    assert (
        require_int_range(
            {"score": 10_000}, "score", errors, min_value=0, max_value=10_000
        )
        == 10_000
    )
    assert errors == []


def test_require_int_range_reports_invalid_values_with_path_or_custom_message() -> None:
    errors: list[str] = []

    assert (
        require_int_range(
            {"score": True},
            "score",
            errors,
            min_value=0,
            max_value=10_000,
            path="provider.score_bps",
        )
        == 0
    )
    assert (
        require_int_range(
            {"count": 11},
            "count",
            errors,
            min_value=1,
            max_value=10,
            message="count must be bounded",
        )
        == 1
    )

    assert errors == [
        "provider.score_bps must be an integer in 0..=10000",
        "count must be bounded",
    ]


def test_require_advancing_int_pair_accepts_strictly_advancing_values() -> None:
    errors: list[str] = []

    assert require_advancing_int_pair(
        {"since": 0, "next_since": 1}, "since", "next_since", errors
    ) == (0, 1)
    assert errors == []


def test_require_advancing_int_pair_rejects_invalid_or_non_advancing_values() -> None:
    errors: list[str] = []

    assert require_advancing_int_pair(
        {"since": -1, "next_since": 0}, "since", "next_since", errors
    ) == (0, 0)
    assert require_advancing_int_pair(
        {"since": 2, "next_since": 2}, "since", "next_since", errors
    ) == (2, 2)

    assert errors == [
        "since must be a non-negative integer",
        "next_since must be a positive integer",
        "next_since must advance past since",
    ]


def test_require_score_bps_accepts_scores_in_range() -> None:
    errors: list[str] = []

    require_score_bps({"score": 0}, "score", errors)
    require_score_bps({"score": 10_000}, "score", errors)

    assert errors == []


def test_require_score_bps_reports_invalid_or_out_of_range_scores() -> None:
    errors: list[str] = []

    require_score_bps({"flag": True}, "flag", errors)
    require_score_bps({"score": 10_001}, "score", errors)

    assert errors == [
        "flag must be a non-negative integer",
        "score must be <= 10000",
    ]


def test_require_2xx_status_accepts_success_status_codes() -> None:
    errors: list[str] = []

    require_2xx_status({"status_code": 200}, "status_code", errors)
    require_2xx_status({"status_code": 299}, "status_code", errors)

    assert errors == []


def test_require_2xx_status_reports_non_2xx_or_non_integer_values() -> None:
    errors: list[str] = []

    require_2xx_status({"status_code": 199}, "status_code", errors)
    require_2xx_status({"status_code": 300}, "status_code", errors)
    require_2xx_status({"status_code": True}, "status_code", errors)
    require_2xx_status({"status_code": "200"}, "status_code", errors)

    assert errors == [
        "status_code must be a 2xx status",
        "status_code must be a 2xx status",
        "status_code must be a 2xx status",
        "status_code must be a 2xx status",
    ]


def test_require_2xx_status_uses_path_override_in_errors() -> None:
    errors: list[str] = []

    require_2xx_status(
        {"status_code": 500},
        "status_code",
        errors,
        path="routes[0].status_code",
    )

    assert errors == ["routes[0].status_code must be a 2xx status"]


def test_require_status_and_number_helpers_reject_malformed_payloads() -> None:
    errors: list[str] = []

    require_2xx_status("bad", "status_code", errors)
    assert require_non_negative_number([], "latency_ms", errors) == 0.0
    assert require_maximum_number(None, "lag_seconds", 10, errors) == 0.0

    assert errors == ["payload must be an object"] * 3


def test_require_non_negative_number_returns_numeric_values_as_float() -> None:
    errors: list[str] = []

    assert require_non_negative_number({"zero": 0}, "zero", errors) == 0.0
    assert require_non_negative_number({"ratio": 1.25}, "ratio", errors) == 1.25
    assert errors == []


def test_require_non_negative_number_rejects_bool_and_negative_values() -> None:
    errors: list[str] = []

    assert require_non_negative_number({"flag": True}, "flag", errors) == 0.0
    assert require_non_negative_number({"negative": -0.5}, "negative", errors) == 0.0
    assert errors == [
        "flag must be a non-negative number",
        "negative must be a non-negative number",
    ]


def test_require_non_negative_number_rejects_non_finite_values() -> None:
    errors: list[str] = []

    assert require_non_negative_number({"nan": float("nan")}, "nan", errors) == 0.0
    assert require_non_negative_number({"inf": float("inf")}, "inf", errors) == 0.0
    assert (
        require_non_negative_number({"neg_inf": float("-inf")}, "neg_inf", errors)
        == 0.0
    )

    assert errors == [
        "nan must be a non-negative number",
        "inf must be a non-negative number",
        "neg_inf must be a non-negative number",
    ]


def test_require_non_negative_number_uses_path_override_in_errors() -> None:
    errors: list[str] = []

    assert (
        require_non_negative_number(
            {"latency_ms": -1},
            "latency_ms",
            errors,
            path="routes[0].latency_ms",
        )
        == 0.0
    )

    assert errors == ["routes[0].latency_ms must be a non-negative number"]


def test_require_maximum_number_accepts_values_at_or_below_maximum() -> None:
    errors: list[str] = []

    assert (
        require_maximum_number({"latency_ms": 1_500}, "latency_ms", 1_500, errors)
        == 1_500.0
    )
    assert (
        require_maximum_number({"latency_ms": 12.5}, "latency_ms", 20, errors)
        == 12.5
    )
    assert errors == []


def test_require_maximum_number_reports_values_above_maximum() -> None:
    errors: list[str] = []

    assert (
        require_maximum_number({"latency_ms": 1_501}, "latency_ms", 1_500, errors)
        == 1_501.0
    )

    assert errors == ["latency_ms must be <= 1500"]


def test_require_maximum_number_reuses_non_negative_number_validation() -> None:
    errors: list[str] = []

    assert (
        require_maximum_number({"latency_ms": -1}, "latency_ms", 10, errors)
        == 0.0
    )
    assert (
        require_maximum_number(
            {"lag_seconds": float("nan")},
            "lag_seconds",
            10,
            errors,
        )
        == 0.0
    )

    assert errors == [
        "latency_ms must be a non-negative number",
        "lag_seconds must be a non-negative number",
    ]


def test_require_maximum_int_accepts_values_at_or_below_maximum() -> None:
    errors: list[str] = []

    assert require_maximum_int({"lag": 0}, "lag", 10, errors) == 0
    assert require_maximum_int({"latency": 10}, "latency", 10, errors, minimum=1) == 10

    assert errors == []


def test_require_maximum_int_reports_values_above_maximum() -> None:
    errors: list[str] = []

    assert require_maximum_int({"lag": 11}, "lag", 10, errors) == 11
    assert (
        require_maximum_int(
            {"latency": 101},
            "latency",
            100,
            errors,
            minimum=1,
            path="verifier.latency",
        )
        == 101
    )

    assert errors == ["lag must be <= 10", "verifier.latency must be <= 100"]


def test_require_maximum_int_reuses_integer_validation() -> None:
    errors: list[str] = []

    assert require_maximum_int({"flag": True}, "flag", 10, errors) == 0
    assert require_maximum_int({"lag": -1}, "lag", 10, errors) == 0
    assert require_maximum_int({"latency": 0}, "latency", 10, errors, minimum=1) == 1

    assert errors == [
        "flag must be a non-negative integer",
        "lag must be a non-negative integer",
        "latency must be a positive integer",
    ]


def test_require_count_equal_returns_total_when_passed_matches() -> None:
    errors: list[str] = []

    assert (
        require_count_equal({"total": 2, "passed": 2}, "total", "passed", errors)
        == 2
    )
    assert errors == []


def test_require_count_equal_reports_total_or_passed_mismatches() -> None:
    errors: list[str] = []

    assert (
        require_count_equal({"total": 0, "passed": 0}, "total", "passed", errors)
        == 0
    )
    assert (
        require_count_equal({"total": 2, "passed": 1}, "total", "passed", errors)
        == 2
    )
    assert errors == [
        "total must be a positive integer",
        "passed must equal total",
    ]


def test_require_count_match_accepts_equal_counts() -> None:
    errors: list[str] = []

    require_count_match({"total": 2, "passed": 2}, "total", "passed", errors)

    assert errors == []


def test_require_count_match_returns_after_invalid_total() -> None:
    errors: list[str] = []

    require_count_match({"total": 0, "passed": 3}, "total", "passed", errors)

    assert errors == ["total must be a positive integer"]


def test_require_count_match_reports_passed_count_mismatch() -> None:
    errors: list[str] = []

    require_count_match({"total": 2, "passed": 1}, "total", "passed", errors)

    assert errors == ["passed must equal total"]


def test_require_count_helpers_reject_malformed_payloads() -> None:
    errors: list[str] = []

    assert require_count_equal("bad", "total", "passed", errors) == 0
    require_count_value_equal([], "logged_session_count", 2, "session_count", errors)
    require_count_match(None, "total", "passed", errors)

    assert errors == ["payload must be an object"] * 3


def test_require_count_length_match_accepts_matching_collection_lengths() -> None:
    errors: list[str] = []

    require_count_length_match(2, [(0, {}), (1, {})], "count", "events", errors)

    assert errors == []


def test_require_count_length_match_reports_length_mismatches() -> None:
    errors: list[str] = []

    require_count_length_match(
        1, [(0, {}), (1, {})], "artifact_count", "artifacts", errors
    )

    assert errors == ["artifact_count must equal artifacts length"]


def test_require_sum_equal_accepts_matching_part_counts() -> None:
    errors: list[str] = []

    require_sum_equal(
        3,
        (("accepted_valid_proof_count", 1), ("rejected_invalid_proof_count", 2)),
        "proof_probe_count",
        errors,
    )

    assert errors == []


def test_require_sum_equal_reports_sum_mismatches() -> None:
    errors: list[str] = []

    require_sum_equal(
        4,
        (("approved_appeal_count", 1), ("rejected_appeal_count", 2)),
        "appeal_probe_count",
        errors,
    )

    assert errors == [
        "approved_appeal_count plus rejected_appeal_count must equal "
        "appeal_probe_count"
    ]


def test_require_sum_equal_can_skip_zero_total() -> None:
    errors: list[str] = []

    require_sum_equal(
        0,
        (("accepted_valid_proof_count", 1), ("rejected_invalid_proof_count", 2)),
        "proof_probe_count",
        errors,
        skip_zero_total=True,
    )

    assert errors == []


def test_require_recent_timestamp_accepts_fresh_timestamps() -> None:
    errors: list[str] = []

    assert (
        require_recent_timestamp(
            {"generated_at_unix": 90},
            "generated_at_unix",
            errors,
            now_unix=100,
            max_age_secs=30,
        )
        == 90
    )
    assert errors == []


def test_require_recent_timestamp_rejects_future_and_stale_timestamps() -> None:
    errors: list[str] = []

    assert (
        require_recent_timestamp(
            {"generated_at_unix": 101},
            "generated_at_unix",
            errors,
            now_unix=100,
            max_age_secs=30,
        )
        == 101
    )
    assert (
        require_recent_timestamp(
            {"generated_at_unix": 69},
            "generated_at_unix",
            errors,
            now_unix=100,
            max_age_secs=30,
        )
        == 69
    )
    assert errors == [
        "generated_at_unix must not be in the future",
        "generated_at_unix is older than 30 seconds",
    ]


def test_require_recent_timestamp_uses_path_override_for_age_errors() -> None:
    errors: list[str] = []

    assert (
        require_recent_timestamp(
            {"generated_at_unix": 101},
            "generated_at_unix",
            errors,
            now_unix=100,
            max_age_secs=30,
            path="publish.generated_at_unix",
        )
        == 101
    )
    assert (
        require_recent_timestamp(
            {"generated_at_unix": 69},
            "generated_at_unix",
            errors,
            now_unix=100,
            max_age_secs=30,
            path="latest.generated_at_unix",
        )
        == 69
    )
    assert errors == [
        "publish.generated_at_unix must not be in the future",
        "latest.generated_at_unix is older than 30 seconds",
    ]


def test_require_recent_timestamp_returns_zero_after_positive_int_failure() -> None:
    errors: list[str] = []

    assert (
        require_recent_timestamp(
            {"generated_at_unix": 0},
            "generated_at_unix",
            errors,
            now_unix=100,
            max_age_secs=30,
        )
        == 0
    )
    assert errors == ["generated_at_unix must be a positive integer"]


def test_collect_string_values_reads_scalars_and_dict_fields() -> None:
    payload = {
        "items": [
            " alpha ",
            {"name": " beta "},
            {"name": ""},
            {"name": 3},
            {},
        ]
    }

    assert collect_string_values(payload, "items", "") == {"alpha"}
    assert collect_string_values(payload, "items", "name") == {"beta"}


def test_collect_string_values_can_preserve_exact_dict_values() -> None:
    payload = {
        "items": [
            "alpha",
            {"name": " beta "},
            {"name": "   "},
        ]
    }

    assert collect_string_values(
        payload,
        "items",
        "name",
        allow_scalar_items=False,
        trim_values=False,
    ) == {" beta "}


def test_collect_string_values_exact_scalar_mode_ignores_blank_values() -> None:
    payload = {"items": [" alpha ", "   ", ""]}

    assert collect_string_values(payload, "items", "", trim_values=False) == {" alpha "}


def test_string_coverage_helpers_reject_malformed_payloads() -> None:
    errors: list[str] = []

    assert collect_string_values("bad", "items", "name") == set()
    require_string_coverage("bad", "routes", "name", ("healthz",), errors)

    assert errors == ["payload must be an object"]


def test_require_string_coverage_reports_missing_value_label() -> None:
    errors: list[str] = []

    require_string_coverage(
        {"metrics": ["present"]},
        "metrics",
        "",
        ("present", "missing"),
        errors,
    )

    assert errors == ["metrics must include value `missing`"]


def test_require_string_coverage_reports_missing_field_label() -> None:
    errors: list[str] = []

    require_string_coverage(
        {"routes": [{"name": "status"}]},
        "routes",
        "name",
        ("status", "healthz"),
        errors,
    )

    assert errors == ["routes must include name `healthz`"]


def test_require_string_coverage_rejects_malformed_required_values() -> None:
    errors: list[str] = []

    require_string_coverage(
        {"routes": [{"name": "status"}]},
        "routes",
        "name",
        "status",
        errors,
    )
    require_string_coverage(
        {"routes": [{"name": "status"}]},
        "routes",
        "name",
        {"status": True},
        errors,
    )
    require_string_coverage(
        {"routes": [{"name": "status"}]},
        "routes",
        "name",
        ("status", ""),
        errors,
    )

    assert errors == [
        "routes required values must be a sequence of strings",
        "routes required values must be a sequence of strings",
        "routes required values must be a sequence of strings",
    ]
