"""CLI construction and reporting for the multilane structural checker."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def build_parser(
    description: str | None, default_root: Path
) -> argparse.ArgumentParser:
    """Build the stable command-line interface for the multilane gate."""

    parser = argparse.ArgumentParser(description=description)
    parser.add_argument(
        "--root",
        type=Path,
        default=default_root,
        help="repository root (defaults to the checker-derived root)",
    )
    parser.add_argument(
        "--print-source-manifest-sha256",
        action="store_true",
        help="print the current source-bound multilane gate manifest digest",
    )
    return parser


def report_validation(
    errors: tuple[str, ...], source_manifest: str | None
) -> int:
    """Print one validation result while preserving the gate's exit contract."""

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    if source_manifest is not None:
        print(source_manifest)
        return 0
    print(
        "Sumeragi v2 multilane models are structurally valid: five refinement "
        "kernels (including authenticated Kura retention) and the composed "
        "in-flight state/action relation are source-bound without a production "
        "trace-extraction claim; no Kura retention source check remains pending"
    )
    return 0
