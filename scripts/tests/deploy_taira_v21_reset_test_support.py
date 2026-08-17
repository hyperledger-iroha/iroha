"""Shared import-safe fixtures for Taira reset controller tests."""

from scripts import deploy_taira_v21_reset as MODULE

GENESIS_PUBLIC_KEY = "ed0120" + "AB" * 32
GENESIS_EXPECTED_HASH = "00" * 31 + "01"
DPN_VALIDATOR_RELEASE_COMMIT = "d" * 40
