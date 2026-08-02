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


def test_default_compose_custody_preflight_fails_closed_and_accepts_exact_records(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Both runtime key files must be exact newline-terminated records."""

    for name in RUN_INTEGRATION.GENESIS_KEY_FILE_ENV:
        monkeypatch.delenv(name, raising=False)
    with pytest.raises(RuntimeError, match="IROHA_GENESIS_PUBLIC_KEY_FILE is required"):
        RUN_INTEGRATION._validate_default_compose_genesis_custody(
            RUN_INTEGRATION.DEFAULT_COMPOSE_FILE
        )

    public_path = tmp_path / "public.key"
    private_path = tmp_path / "private.key"
    public_path.write_text("public-without-newline", encoding="utf-8")
    private_path.write_text("private\n", encoding="utf-8")
    monkeypatch.setenv("IROHA_GENESIS_PUBLIC_KEY_FILE", str(public_path))
    monkeypatch.setenv("IROHA_GENESIS_PRIVATE_KEY_FILE", str(private_path))
    with pytest.raises(RuntimeError, match="exactly one non-empty key record"):
        RUN_INTEGRATION._validate_default_compose_genesis_custody(
            RUN_INTEGRATION.DEFAULT_COMPOSE_FILE
        )

    public_path.write_text("public\n", encoding="utf-8")
    RUN_INTEGRATION._validate_default_compose_genesis_custody(
        RUN_INTEGRATION.DEFAULT_COMPOSE_FILE
    )
