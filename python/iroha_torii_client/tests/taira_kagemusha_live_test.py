"""Opt-in read-only Kagemusha capability probe against public Taira."""

from __future__ import annotations

import os
import sys
from pathlib import Path
from urllib.parse import urlsplit

import pytest

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import TAIRA_TESTNET_PROFILE, ToriiClient  # noqa: E402


def _credential_free_https_origin(raw: str) -> str:
    message = (
        "IROHA_TAIRA_PUBLIC_ROOT must be a credential-free HTTPS origin "
        "without a path, query, or fragment"
    )
    if raw != raw.strip():
        raise ValueError(message)
    try:
        parsed = urlsplit(raw)
        port = parsed.port
    except ValueError as error:
        raise ValueError(message) from error
    if (
        parsed.scheme.lower() != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path not in ("", "/")
        or parsed.query
        or parsed.fragment
    ):
        raise ValueError(message)
    authority = parsed.hostname
    if ":" in authority:
        authority = f"[{authority}]"
    if port is not None:
        authority = f"{authority}:{port}"
    return f"https://{authority}"


def test_taira_probe_accepts_credential_free_https_origin() -> None:
    assert _credential_free_https_origin("https://taira.sora.org/") == "https://taira.sora.org"


@pytest.mark.parametrize(
    "raw",
    [
        " http://taira.sora.org",
        "http://taira.sora.org",
        "https://@taira.sora.org",
        "https://user@taira.sora.org",
        "https://taira.sora.org/v1",
        "https://taira.sora.org?query=1",
        "https://taira.sora.org#fragment",
    ],
)
def test_taira_probe_rejects_non_origin_overrides(raw: str) -> None:
    with pytest.raises(ValueError, match="credential-free HTTPS origin"):
        _credential_free_https_origin(raw)


@pytest.mark.skipif(
    os.environ.get("IROHA_TAIRA_KAGEMUSHA_READ_ONLY") != "1",
    reason="set IROHA_TAIRA_KAGEMUSHA_READ_ONLY=1 for the public Taira probe",
)
def test_public_taira_exposes_exact_kagemusha_capability() -> None:
    """Exercise the real Python SDK against Taira without mutating chain state."""

    public_root = _credential_free_https_origin(
        os.environ.get(
            "IROHA_TAIRA_PUBLIC_ROOT",
            TAIRA_TESTNET_PROFILE.torii_base_url,
        )
    )
    capability = ToriiClient(public_root).get_offline_capability(timeout=20.0)

    assert capability.cash_handoff_capability == "cash_handoff_v1"
    assert capability.required_bridge_abi_version == 23
    assert capability.max_hops == 8
    assert capability.ready is True
