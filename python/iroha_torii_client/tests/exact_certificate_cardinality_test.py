"""Exact-cardinality regressions for persisted Sumeragi certificate summaries."""

from __future__ import annotations

import pytest
from client_test_support import canonical_hash
from iroha_torii_client.client import _SumeragiV2StatusParser

_EMPTY_NATIVE_MANIFEST_ROOT = (
    "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
)


def _execution_commitment() -> dict[str, object]:
    return {
        "parent_state_root": canonical_hash(0x61),
        "post_state_root": canonical_hash(0x62),
        "ordinary_writes_root": canonical_hash(0x63),
        "kagemusha_top_up_root": None,
        "kagemusha_top_up_count": 0,
        "native_amx_application_manifest_version": 1,
        "native_amx_application_manifest_root": _EMPTY_NATIVE_MANIFEST_ROOT,
        "native_amx_application_manifest_count": 0,
        "lane_finality_manifest": None,
        "merge_carrier": None,
        "executed_block_wire_len": 512,
        "executed_block_wire_hash": canonical_hash(0x64),
    }


def _parse_execution_commitment(payload: dict[str, object]) -> object:
    return _SumeragiV2StatusParser._execution_commitment(payload, context="status")


def _committed_lane_block(**overrides: object) -> dict[str, object]:
    block: dict[str, object] = {
        "lane_id": 2,
        "dataspace_id": 7,
        "lane_incarnation": canonical_hash(0x51),
        "lane_block_height": 1,
        "lane_block_view": 0,
        "descriptor_hash": canonical_hash(0x55),
        "proposal_hash": canonical_hash(0x58),
        "execution_status": "state_applied_by_canonical_block",
        "executable_payload_available": True,
        "subject_hash": canonical_hash(0x53),
        "payload_ownership_hash": canonical_hash(0x56),
        "rbc_instance_hash": canonical_hash(0x57),
        "qc_mode_tag": "iroha2-consensus::permissioned-sumeragi@v2",
        "validator_count": 4,
        "min_quorum": 3,
        "prepare_qc_signer_count": 3,
        "commit_qc_signer_count": 3,
    }
    block.update(overrides)
    return block


@pytest.mark.parametrize("field", ["prepare_qc_signer_count", "commit_qc_signer_count"])
@pytest.mark.parametrize("signer_count", [2, 4])
def test_committed_lane_block_rejects_nonexact_certificate_cardinality(
    field: str, signer_count: int
) -> None:
    with pytest.raises(RuntimeError, match="impossible certified quorum"):
        _SumeragiV2StatusParser._committed_blocks(
            [_committed_lane_block(**{field: signer_count})]
        )


def test_committed_lane_block_rejects_retired_direct_wsv_status() -> None:
    with pytest.raises(RuntimeError, match="invalid execution status"):
        _SumeragiV2StatusParser._committed_blocks(
            [
                _committed_lane_block(
                    execution_status="state_applied_by_direct_execution"
                )
            ]
        )


def test_execution_commitment_preserves_exact_lane_finality_manifest() -> None:
    payload = _execution_commitment()
    assert _parse_execution_commitment(payload).lane_finality_manifest is None
    payload["lane_finality_manifest"] = {
        "root": canonical_hash(0x65),
        "leaf_count": 1024,
    }
    commitment = _parse_execution_commitment(payload)
    assert commitment.lane_finality_manifest.root == canonical_hash(0x65)
    assert commitment.lane_finality_manifest.leaf_count == 1024


def test_execution_commitment_rejects_noncanonical_lane_finality_manifest() -> None:
    invalid_manifests: tuple[object, ...] = (
        [],
        {},
        {"root": canonical_hash(0x65), "leaf_count": 0},
        {"root": canonical_hash(0x65), "leaf_count": 1025},
        {"root": "not-a-hash", "leaf_count": 1},
        {"root": canonical_hash(0x65), "leaf_count": 1, "future": True},
    )
    missing = _execution_commitment()
    del missing["lane_finality_manifest"]
    invalid_payloads = [missing]
    for manifest in invalid_manifests:
        payload = _execution_commitment()
        payload["lane_finality_manifest"] = manifest
        invalid_payloads.append(payload)
    for payload in invalid_payloads:
        with pytest.raises((RuntimeError, TypeError, ValueError)):
            _parse_execution_commitment(payload)
