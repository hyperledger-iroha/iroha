"""Tests for strict FASTPQ row-usage comparisons."""

import pytest

from scripts.fastpq import check_row_usage


def snapshot(row_usage):
    return {
        "fastpq_batches": [
            {
                "entry_hash": "batch-a",
                "parameter": "fastpq-lane-balanced",
                "row_usage": row_usage,
            }
        ]
    }


def test_build_row_usage_map_accepts_complete_v1_counts():
    payload = snapshot(
        {
            "total_rows": 4,
            "transfer_rows": 3,
            "non_transfer_rows": 1,
            "meta_set_rows": 1,
            "transfer_ratio": 0.75,
        }
    )

    assert check_row_usage.build_row_usage_map(payload) == {
        "batch-a": payload["fastpq_batches"][0]["row_usage"]
    }


def test_build_row_usage_map_rejects_missing_v1_count():
    payload = snapshot(
        {
            "total_rows": 4,
            "transfer_rows": 3,
            "non_transfer_rows": 1,
            "transfer_ratio": 0.75,
        }
    )

    with pytest.raises(SystemExit, match="meta_set_rows missing"):
        check_row_usage.build_row_usage_map(payload)


def test_build_row_usage_map_rejects_count_outside_v1_u32_range():
    payload = snapshot(
        {
            "total_rows": 1 << 32,
            "transfer_rows": 1 << 32,
            "non_transfer_rows": 0,
            "meta_set_rows": 0,
            "transfer_ratio": 1.0,
        }
    )

    with pytest.raises(SystemExit, match="must be ≤ 4294967295"):
        check_row_usage.build_row_usage_map(payload)


def test_build_row_usage_map_rejects_retired_selector_counts():
    payload = snapshot(
        {
            "total_rows": 4,
            "transfer_rows": 3,
            "non_transfer_rows": 1,
            "meta_set_rows": 1,
            "transfer_ratio": 0.75,
            "mint_rows": 0,
        }
    )

    with pytest.raises(SystemExit, match="non-V1 fields: mint_rows"):
        check_row_usage.build_row_usage_map(payload)
