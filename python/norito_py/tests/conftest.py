# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Pytest configuration for Norito Python tests."""

from __future__ import annotations

from pathlib import Path
import os
import sys

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SRC_DIR = PROJECT_ROOT / "src"

_INSTALLED_PACKAGE_MODE = os.environ.get("IROHA_PYTHON_TEST_INSTALLED_PACKAGE")
if _INSTALLED_PACKAGE_MODE not in {None, "1"}:
    raise RuntimeError("IROHA_PYTHON_TEST_INSTALLED_PACKAGE must be unset or 1")

if _INSTALLED_PACKAGE_MODE is None and str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))
