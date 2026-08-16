"""Closed value types shared by the Python Sumeragi-v2 status parser."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


class SumeragiV2StatusPhase(str, Enum):
    """High-level state of the authoritative Sumeragi v2 reducer."""

    AWAITING_PROPOSAL = "awaiting_proposal"
    RECONSTRUCTING_PAYLOAD = "reconstructing_payload"
    VALIDATING_PAYLOAD = "validating_payload"
    PREPARE = "prepare"
    COMMIT = "commit"
    PENDING_APPLY = "pending_apply"


class SumeragiV2BodyState(str, Enum):
    """Local state of the proposal body reported by Sumeragi v2."""

    MISSING = "missing"
    RECONSTRUCTING = "reconstructing"
    STORED = "stored"
    VALIDATED = "validated"
    PENDING_APPLY = "pending_apply"
    APPLIED = "applied"


class SumeragiV2GlobalPhase(str, Enum):
    """Global two-phase consensus phase."""

    PREPARE = "prepare"
    COMMIT = "commit"


@dataclass(frozen=True)
class SumeragiV2LaneFinalityManifestCommitment:
    """Exact Merkle root and non-zero lane-finality leaf count."""

    root: str
    leaf_count: int


for _model in tuple(globals().values()):
    if isinstance(_model, type) and _model.__module__ == __name__:
        _model.__module__ = f"{__package__}.client"
del _model
