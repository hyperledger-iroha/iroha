"""Public Taira profile contracts."""

from dataclasses import FrozenInstanceError

import pytest
from iroha_torii_client import TAIRA_TESTNET_PROFILE, taira_local_signing_context

NETWORK_ID = (
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
)


def test_taira_profile_exposes_exact_public_metadata() -> None:
    assert TAIRA_TESTNET_PROFILE.torii_base_url == "https://taira.sora.org"
    assert TAIRA_TESTNET_PROFILE.chain_id == "fc56984b-2be7-431d-840e-21514d1883f0"
    assert TAIRA_TESTNET_PROFILE.i105_discriminant == 369
    assert TAIRA_TESTNET_PROFILE.kagemusha_asset_definition_id == "7ZepsJTHCVLKsrFFNZGSRGZgvBhv"
    assert TAIRA_TESTNET_PROFILE.kagemusha_asset_alias == "ds#boi.is"
    assert TAIRA_TESTNET_PROFILE.kagemusha_asset_scale == 2
    assert TAIRA_TESTNET_PROFILE.xor_asset_definition_id == "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
    assert TAIRA_TESTNET_PROFILE.xor_asset_alias == "xor#universal"
    assert TAIRA_TESTNET_PROFILE.xor_asset_scale == 9
    with pytest.raises(FrozenInstanceError):
        TAIRA_TESTNET_PROFILE.i105_discriminant = 1  # type: ignore[misc]


def test_taira_context_requires_exact_deployed_network_id() -> None:
    assert taira_local_signing_context(NETWORK_ID).network_id == NETWORK_ID
    with pytest.raises(RuntimeError, match="canonical hash:"):
        taira_local_signing_context(TAIRA_TESTNET_PROFILE.chain_id)
