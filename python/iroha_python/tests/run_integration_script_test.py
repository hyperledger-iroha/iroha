"""Tests for the Docker integration harness preflight."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "scripts" / "run_integration.py"
SPEC = importlib.util.spec_from_file_location("iroha_python_run_integration", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
RUN_INTEGRATION = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUN_INTEGRATION)


def test_default_start_does_not_narrow_the_validator_stack(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The compatibility-named compose fixture must start all four validators."""

    monkeypatch.delenv("COMPOSE_SERVICE", raising=False)
    args = RUN_INTEGRATION._parse_args([])
    assert args.service is None


def test_default_compose_artifact_preflight_fails_closed_and_accepts_exact_records(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """The runtime body, verifier key, and exact hash must be canonical files."""

    for name in RUN_INTEGRATION.GENESIS_ARTIFACT_FILE_ENV:
        monkeypatch.delenv(name, raising=False)
    with pytest.raises(RuntimeError, match="IROHA_GENESIS_PUBLIC_KEY_FILE is required"):
        RUN_INTEGRATION._validate_default_compose_genesis_artifacts(
            RUN_INTEGRATION.DEFAULT_COMPOSE_FILE
        )

    public_path = tmp_path / "public.key"
    signed_path = tmp_path / "genesis.signed.nrt"
    hash_path = tmp_path / "genesis.expected_hash"
    public_path.write_text("public-without-newline", encoding="utf-8")
    signed_path.write_bytes(b"signed-genesis")
    hash_path.write_text(f"hash:{'0' * 63}1#C50E\n", encoding="utf-8")
    monkeypatch.setenv("IROHA_GENESIS_PUBLIC_KEY_FILE", str(public_path))
    monkeypatch.setenv("IROHA_GENESIS_SIGNED_FILE", str(signed_path))
    monkeypatch.setenv("IROHA_GENESIS_EXPECTED_HASH_FILE", str(hash_path))
    with pytest.raises(RuntimeError, match="exactly one non-empty record"):
        RUN_INTEGRATION._validate_default_compose_genesis_artifacts(
            RUN_INTEGRATION.DEFAULT_COMPOSE_FILE
        )

    public_path.write_text("public\n", encoding="utf-8")
    hash_path.write_text(f"{'0' * 63}1\n", encoding="utf-8")
    with pytest.raises(RuntimeError, match="canonical checked NetworkId"):
        RUN_INTEGRATION._validate_default_compose_genesis_artifacts(
            RUN_INTEGRATION.DEFAULT_COMPOSE_FILE
        )

    hash_path.write_text(f"hash:{'0' * 63}1#c50e\n", encoding="utf-8")
    with pytest.raises(RuntimeError, match="canonical checked NetworkId"):
        RUN_INTEGRATION._validate_default_compose_genesis_artifacts(
            RUN_INTEGRATION.DEFAULT_COMPOSE_FILE
        )

    hash_path.write_text(f"hash:{'0' * 63}1#C50F\n", encoding="utf-8")
    with pytest.raises(RuntimeError, match="canonical checked NetworkId"):
        RUN_INTEGRATION._validate_default_compose_genesis_artifacts(
            RUN_INTEGRATION.DEFAULT_COMPOSE_FILE
        )

    hash_path.write_text(f"hash:{'0' * 63}1#C50E\n", encoding="utf-8")
    RUN_INTEGRATION._validate_default_compose_genesis_artifacts(
        RUN_INTEGRATION.DEFAULT_COMPOSE_FILE
    )
