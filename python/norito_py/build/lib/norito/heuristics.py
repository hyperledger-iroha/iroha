"""Heuristics mirroring the Rust Norito defaults."""

from __future__ import annotations

AOS_NCB_SMALL_N: int = 64
COMBO_NO_DELTA_SMALL_N_IF_EMPTY: int = 2
COMBO_ID_DELTA_MIN_ROWS: int = 2
COMBO_ENABLE_ID_DELTA: bool = True
COMBO_ENABLE_NAME_DICT: bool = True
COMBO_DICT_RATIO_MAX: float = 0.40
COMBO_DICT_AVG_LEN_MIN: float = 8.0

__all__ = [
    "AOS_NCB_SMALL_N",
    "COMBO_NO_DELTA_SMALL_N_IF_EMPTY",
    "COMBO_ID_DELTA_MIN_ROWS",
    "COMBO_ENABLE_ID_DELTA",
    "COMBO_ENABLE_NAME_DICT",
    "COMBO_DICT_RATIO_MAX",
    "COMBO_DICT_AVG_LEN_MIN",
]
