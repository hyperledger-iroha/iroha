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


@pytest.mark.parametrize("value", [True, 600.0, "600"])
def test_alias_policy_rejects_lossy_integer_inputs(value: object) -> None:
    with pytest.raises(TypeError, match="positive integer"):
        sorafs.SorafsAliasPolicy(
            **{**CANONICAL_DEFAULTS, "positive_ttl_secs": value},  # type: ignore[arg-type]
        )


def test_alias_policy_rejects_u64_overflow() -> None:
    with pytest.raises(ValueError, match="unsigned 64-bit"):
        sorafs.SorafsAliasPolicy(
            **{**CANONICAL_DEFAULTS, "positive_ttl_secs": 1 << 64},
        )


def test_alias_policy_mapping_accepts_only_first_release_field_names(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(sorafs, "_crypto", FakeCrypto(dict(CANONICAL_DEFAULTS)))

    policy = sorafs.SorafsAliasPolicy.from_mapping({"positive_ttl_secs": 700})
    assert policy.positive_ttl_secs == 700
    assert policy.refresh_window_secs == CANONICAL_DEFAULTS["refresh_window_secs"]
    with pytest.raises(ValueError, match="unsupported fields"):
        sorafs.SorafsAliasPolicy.from_mapping({"positiveTtlSecs": 700})


@pytest.mark.parametrize("policy", [{}, False, 0])
def test_evaluate_alias_proof_rejects_falsey_invalid_policy_before_native_call(
    monkeypatch: pytest.MonkeyPatch,
    policy: object,
) -> None:
    class NoCallsCrypto:
        @staticmethod
        def sorafs_evaluate_alias_proof(*_args: object) -> Any:
            raise AssertionError("native evaluation must not be called")

    monkeypatch.setattr(sorafs, "_crypto", NoCallsCrypto())

    with pytest.raises(TypeError, match="SorafsAliasPolicy"):
        sorafs.evaluate_alias_proof("proof", policy=policy)  # type: ignore[arg-type]


@pytest.mark.parametrize("now_secs", [True, 1.5, "1", -1])
def test_evaluate_alias_proof_rejects_ambiguous_time_before_native_call(
    monkeypatch: pytest.MonkeyPatch,
    now_secs: object,
) -> None:
    class NoCallsCrypto:
        @staticmethod
        def sorafs_evaluate_alias_proof(*_args: object) -> Any:
            raise AssertionError("native evaluation must not be called")

    monkeypatch.setattr(sorafs, "_crypto", NoCallsCrypto())
    policy = sorafs.SorafsAliasPolicy(**CANONICAL_DEFAULTS)

    with pytest.raises((TypeError, ValueError), match="now_secs"):
        sorafs.evaluate_alias_proof(
            "proof",
            policy=policy,
            now_secs=now_secs,  # type: ignore[arg-type]
        )
