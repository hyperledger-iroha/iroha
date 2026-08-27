from __future__ import annotations

from typing import Any

import pytest

from iroha_python import sorafs

CANONICAL_DEFAULTS: dict[str, int] = {
    "positive_ttl_secs": 600,
    "refresh_window_secs": 120,
    "hard_expiry_secs": 900,
    "negative_ttl_secs": 60,
    "revocation_ttl_secs": 300,
    "rotation_max_age_secs": 21_600,
    "successor_grace_secs": 300,
    "governance_grace_secs": 0,
}


class FakeCrypto:
    def __init__(self, payload: Any) -> None:
        self.payload = payload

    def sorafs_alias_policy_defaults(self) -> Any:
        return self.payload


def test_native_alias_policy_defaults_require_exact_v1_surface(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(sorafs, "_crypto", FakeCrypto(dict(CANONICAL_DEFAULTS)))

    assert sorafs.SorafsAliasPolicy.defaults() == sorafs.SorafsAliasPolicy(
        **CANONICAL_DEFAULTS
    )


@pytest.mark.parametrize(
    "payload",
    [
        {
            key: value
            for key, value in CANONICAL_DEFAULTS.items()
            if key != "successor_grace_secs"
        },
        {**CANONICAL_DEFAULTS, "legacy_grace_secs": 0},
        {**CANONICAL_DEFAULTS, 7: 0},
    ],
    ids=["older-native-surface", "unexpected-field", "non-string-field"],
)
def test_native_alias_policy_defaults_reject_incompatible_surfaces(
    monkeypatch: pytest.MonkeyPatch,
    payload: dict[Any, int],
) -> None:
    monkeypatch.setattr(sorafs, "_crypto", FakeCrypto(payload))

    with pytest.raises((RuntimeError, TypeError), match="alias policy"):
        sorafs.SorafsAliasPolicy.defaults()


def test_native_alias_policy_defaults_do_not_fall_back(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class UnavailableCrypto:
        @staticmethod
        def sorafs_alias_policy_defaults() -> Any:
            raise RuntimeError("native extension unavailable")

    monkeypatch.setattr(sorafs, "_crypto", UnavailableCrypto())

    with pytest.raises(RuntimeError, match="native extension unavailable"):
        sorafs.SorafsAliasPolicy.defaults()
