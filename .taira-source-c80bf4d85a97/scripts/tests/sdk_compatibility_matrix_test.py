"""Validate the public SDK compatibility matrix fixture."""

import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MATRIX_PATH = REPO_ROOT / "fixtures" / "sdk" / "compatibility_matrix.json"
VALID_STATUSES = {"ok", "failed", "no-data"}


def load_matrix() -> dict:
    return json.loads(MATRIX_PATH.read_text(encoding="utf-8"))


def test_sdk_compatibility_matrix_has_public_source_metadata():
    matrix = load_matrix()

    assert matrix["source"]["repo"] == "hyperledger-iroha/iroha"
    assert matrix["source"]["repo_url"] == "https://github.com/hyperledger-iroha/iroha"
    assert matrix["source"]["branch"] == "i23-features"
    assert (
        matrix["source"]["branch_url"]
        == "https://github.com/hyperledger-iroha/iroha/tree/i23-features"
    )

    serialized = json.dumps(matrix)
    assert "../iroha" not in serialized
    assert "dirty" not in serialized


def test_sdk_compatibility_matrix_has_complete_rows():
    matrix = load_matrix()
    sdk_count = len(matrix["included_sdks"])

    assert sdk_count > 0
    assert matrix["stories"]

    for story in matrix["stories"]:
        results = story["results"]
        assert len(results) == sdk_count, story["name"]

        statuses = [result["status"] for result in results]
        assert set(statuses) <= VALID_STATUSES, story["name"]
        assert "no-data" not in statuses, story["name"]


def main() -> None:
    test_sdk_compatibility_matrix_has_public_source_metadata()
    test_sdk_compatibility_matrix_has_complete_rows()


if __name__ == "__main__":
    main()
