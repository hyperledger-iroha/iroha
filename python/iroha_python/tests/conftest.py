from __future__ import annotations

import sys
from pathlib import Path


def _add_path(path: Path) -> None:
    location = str(path)
    if location not in sys.path:
        sys.path.insert(0, location)


_ROOT = Path(__file__).resolve().parents[2]
_add_path(_ROOT)
_add_path(_ROOT / "norito_py" / "src")
_add_path(_ROOT / "iroha_torii_client")
_add_path(_ROOT / "iroha_python" / "src")
