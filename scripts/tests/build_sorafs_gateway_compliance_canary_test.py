from __future__ import annotations

import importlib.util
import json
import stat
import sys
from pathlib import Path

import pytest


SCRIPTS = Path(__file__).resolve().parents[1]
BUILDER_PATH = SCRIPTS / "build_sorafs_gateway_compliance_canary.py"
FIXTURES_PATH = (
    SCRIPTS / "tests" / "check_sorafs_gateway_compliance_rollout_evidence_test.py"
)


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


MODULE = load_module("gateway_compliance_builder", BUILDER_PATH)
FIXTURES = load_module("gateway_compliance_fixtures", FIXTURES_PATH)


def write_json(path: Path, payload: dict) -> None:
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")


def run_builder(
    root: Path,
    payload: dict,
    *,
    kind: str = "catalog_promotion",
    extra: list[str] | None = None,
) -> tuple[int, Path]:
    probe = root / "observed-probe.json"
    out = root / "canonical-evidence.json"
    write_json(probe, payload)
    code = MODULE.main(
        [
            "--kind",
            kind,
            "--probe-artifact",
            str(probe),
            "--out",
            str(out),
            "--now-unix",
            str(FIXTURES.NOW),
            *(extra or []),
        ]
    )
    return code, out


def test_builder_canonicalizes_observed_production_probe(tmp_path: Path) -> None:
    payload = FIXTURES.catalog_promotion()
    code, out = run_builder(tmp_path, payload)
    assert code == 0
    assert json.loads(out.read_text(encoding="utf-8")) == payload
    assert stat.S_IMODE(out.stat().st_mode) == 0o600


@pytest.mark.parametrize("kind,builder", FIXTURES.BUILDERS.items())
def test_builder_accepts_every_canonical_observed_kind(
    tmp_path: Path, kind: str, builder
) -> None:
    code, out = run_builder(tmp_path, builder(), kind=kind)
    assert code == 0
    assert json.loads(out.read_text(encoding="utf-8"))["schema"] == (
        FIXTURES.MODULE.KIND_BY_NAME[kind].schema
    )


def test_builder_requires_an_input_probe_artifact(tmp_path: Path) -> None:
    out = tmp_path / "out.json"
    assert (
        MODULE.main(
            [
                "--kind",
                "catalog_promotion",
                "--out",
                str(out),
                "--now-unix",
                str(FIXTURES.NOW),
            ]
        )
        == 2
    )
    assert not out.exists()


def test_builder_never_synthesizes_verified_claims(tmp_path: Path) -> None:
    payload = FIXTURES.catalog_promotion()
    del payload["catalog_signatures_verified"]
    code, out = run_builder(tmp_path, payload)
    assert code == 2
    assert not out.exists()


def test_removed_verified_claim_cli_is_rejected(tmp_path: Path) -> None:
    payload = FIXTURES.catalog_promotion()
    code, out = run_builder(
        tmp_path,
        payload,
        extra=["--verified-claim", "catalog_signatures_verified"],
    )
    assert code == 2
    assert not out.exists()


def test_non_production_fixture_is_explicit_and_not_promotable(
    tmp_path: Path,
) -> None:
    payload = FIXTURES.catalog_promotion()
    code, out = run_builder(
        tmp_path, payload, extra=["--non-production-fixture"]
    )
    assert code == 0
    fixture = json.loads(out.read_text(encoding="utf-8"))
    assert fixture["status"] == "non_production"
    assert fixture["evidence_scope"] == "non_production_fixture"
    kind, errors = FIXTURES.MODULE.validate_evidence_payload(
        fixture,
        FIXTURES.MODULE.ValidationOptions(
            now_unix=FIXTURES.NOW,
            max_evidence_age_secs=FIXTURES.MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS,
            max_route_latency_ms=FIXTURES.MODULE.DEFAULT_MAX_ROUTE_LATENCY_MS,
            max_reload_latency_ms=FIXTURES.MODULE.DEFAULT_MAX_RELOAD_LATENCY_MS,
            min_gateways=FIXTURES.MODULE.DEFAULT_MIN_GATEWAYS,
            min_catalog_entries=FIXTURES.MODULE.DEFAULT_MIN_CATALOG_ENTRIES,
            min_catalog_changes=FIXTURES.MODULE.DEFAULT_MIN_CATALOG_CHANGES,
            min_honey_probes=FIXTURES.MODULE.DEFAULT_MIN_HONEY_PROBES,
        ),
    )
    assert kind == "catalog_promotion"
    assert errors


def test_kind_mismatch_fails_closed(tmp_path: Path) -> None:
    code, out = run_builder(
        tmp_path, FIXTURES.controller_runtime(), kind="catalog_promotion"
    )
    assert code == 2
    assert not out.exists()


@pytest.mark.parametrize(
    "mutation",
    [
        lambda payload: payload.__setitem__(
            "bundle_digest_hex", FIXTURES.CATALOG_DIGEST
        ),
        lambda payload: payload.__setitem__("proof_token_verified", True),
        lambda payload: payload.__setitem__(
            "catalog_digest_hex", FIXTURES.CATALOG_DIGEST.upper()
        ),
        lambda payload: payload.__setitem__(
            "predecessor_catalog_digest_hex", "0" * 64
        ),
        lambda payload: payload["gateway_acknowledgements"][1].__setitem__(
            "catalog_digest_hex", "11" * 32
        ),
        lambda payload: payload["gateway_acknowledgements"][0].__setitem__(
            "signature_verified", False
        ),
    ],
)
def test_invalid_probe_artifacts_fail_before_write(tmp_path: Path, mutation) -> None:
    payload = FIXTURES.catalog_promotion()
    mutation(payload)
    code, out = run_builder(tmp_path, payload)
    assert code == 2
    assert not out.exists()


def test_stale_probe_fails_before_write(tmp_path: Path) -> None:
    payload = FIXTURES.catalog_promotion()
    payload["generated_at_unix"] -= 90_000
    code, out = run_builder(tmp_path, payload)
    assert code == 2
    assert not out.exists()


def test_probe_symlink_is_rejected(tmp_path: Path) -> None:
    target = tmp_path / "target.json"
    probe = tmp_path / "probe.json"
    out = tmp_path / "out.json"
    write_json(target, FIXTURES.catalog_promotion())
    probe.symlink_to(target)
    code = MODULE.main(
        [
            "--kind",
            "catalog_promotion",
            "--probe-artifact",
            str(probe),
            "--out",
            str(out),
            "--now-unix",
            str(FIXTURES.NOW),
        ]
    )
    assert code == 2
    assert not out.exists()


def test_output_symlink_is_rejected(tmp_path: Path) -> None:
    probe = tmp_path / "probe.json"
    target = tmp_path / "target.json"
    out = tmp_path / "out.json"
    write_json(probe, FIXTURES.catalog_promotion())
    target.write_text("unchanged", encoding="utf-8")
    out.symlink_to(target)
    code = MODULE.main(
        [
            "--kind",
            "catalog_promotion",
            "--probe-artifact",
            str(probe),
            "--out",
            str(out),
            "--now-unix",
            str(FIXTURES.NOW),
        ]
    )
    assert code == 2
    assert target.read_text(encoding="utf-8") == "unchanged"


def test_output_must_not_replace_probe(tmp_path: Path) -> None:
    probe = tmp_path / "probe.json"
    payload = FIXTURES.catalog_promotion()
    write_json(probe, payload)
    code = MODULE.main(
        [
            "--kind",
            "catalog_promotion",
            "--probe-artifact",
            str(probe),
            "--out",
            str(probe),
            "--now-unix",
            str(FIXTURES.NOW),
        ]
    )
    assert code == 2
    assert json.loads(probe.read_text(encoding="utf-8")) == payload


def test_response_file_arguments_are_supported(tmp_path: Path) -> None:
    probe = tmp_path / "probe.json"
    out = tmp_path / "out.json"
    args_file = tmp_path / "builder.args"
    write_json(probe, FIXTURES.catalog_promotion())
    args_file.write_text(
        "\n".join(
            [
                "--kind",
                "catalog_promotion",
                "--probe-artifact",
                str(probe),
                "--out",
                str(out),
                "--now-unix",
                str(FIXTURES.NOW),
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    assert MODULE.main([f"@{args_file}"]) == 0
    assert out.exists()


def test_output_is_deterministic(tmp_path: Path) -> None:
    payload = FIXTURES.catalog_promotion()
    code, first = run_builder(tmp_path, payload)
    assert code == 0
    first_bytes = first.read_bytes()
    second_root = tmp_path / "second"
    second_root.mkdir()
    code, second = run_builder(second_root, payload)
    assert code == 0
    assert second.read_bytes() == first_bytes
