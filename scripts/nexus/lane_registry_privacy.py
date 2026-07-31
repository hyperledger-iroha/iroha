"""Strict first-release validation for Nexus lane privacy commitments."""

from __future__ import annotations

import re
from typing import Dict, List

_HEX_DIGEST = re.compile(r"^[0-9a-fA-F]{64}$")
_ENTRY_FIELDS = frozenset({"id", "scheme", "merkle"})
_MERKLE_FIELDS = frozenset({"root", "max_depth"})


def summarize_merkle_privacy_commitments(manifest: Dict) -> List[Dict[str, object]]:
    """Validate and summarize the Merkle-only privacy commitment manifest surface.

    The first release rejects every non-Merkle scheme and every removed or
    unknown scheme-specific field. This keeps the bundle tooling aligned with
    the node manifest parser instead of publishing entries admission will
    reject.
    """

    raw_entries = manifest.get("privacy_commitments")
    if raw_entries is None:
        return []
    if not isinstance(raw_entries, list):
        raise ValueError("`privacy_commitments` must be an array")

    commits: List[Dict[str, object]] = []
    seen_ids = set()
    for index, entry in enumerate(raw_entries):
        context = f"privacy_commitments[{index}]"
        if not isinstance(entry, dict):
            raise ValueError(f"`{context}` must be an object")
        unknown = sorted(set(entry) - _ENTRY_FIELDS)
        if unknown:
            raise ValueError(
                f"`{context}` contains unsupported fields: {', '.join(unknown)}",
            )

        commitment_id = entry.get("id")
        if (
            isinstance(commitment_id, bool)
            or not isinstance(commitment_id, int)
            or not 0 <= commitment_id <= 0xFFFF
        ):
            raise ValueError(f"`{context}.id` must be an unsigned 16-bit integer")
        if commitment_id in seen_ids:
            raise ValueError(f"`{context}.id` duplicates commitment id {commitment_id}")
        seen_ids.add(commitment_id)

        scheme = entry.get("scheme")
        if not isinstance(scheme, str) or scheme.strip().lower() != "merkle":
            raise ValueError(
                f"`{context}.scheme` must be `merkle`; proof-system commitments "
                "require a real on-chain verifying-key-backed verifier",
            )

        merkle = entry.get("merkle")
        if not isinstance(merkle, dict):
            raise ValueError(f"`{context}.merkle` must be an object")
        unknown_merkle = sorted(set(merkle) - _MERKLE_FIELDS)
        if unknown_merkle:
            raise ValueError(
                f"`{context}.merkle` contains unsupported fields: "
                f"{', '.join(unknown_merkle)}",
            )

        root = merkle.get("root")
        if not isinstance(root, str):
            raise ValueError(f"`{context}.merkle.root` must be a 32-byte hex digest")
        normalized_root = root.strip()
        if normalized_root.lower().startswith("0x"):
            normalized_root = normalized_root[2:]
        if _HEX_DIGEST.fullmatch(normalized_root) is None:
            raise ValueError(f"`{context}.merkle.root` must be a 32-byte hex digest")

        max_depth = merkle.get("max_depth")
        if (
            isinstance(max_depth, bool)
            or not isinstance(max_depth, int)
            or not 1 <= max_depth <= 0xFF
        ):
            raise ValueError(f"`{context}.merkle.max_depth` must be in 1..=255")

        commits.append({"id": commitment_id, "scheme": "merkle"})

    commits.sort(key=lambda item: int(item["id"]))
    return commits
