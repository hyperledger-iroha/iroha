"""Retired receipt inputs cannot certify the unqualified native candidate."""
from pathlib import Path
import subprocess
import sys


def test_retired_receipt_entrypoint_refuses_historical_control_count(tmp_path):
    root = Path(__file__).resolve().parents[2]
    output = tmp_path / "receipt.json"
    result = subprocess.run(
        [sys.executable, str(root / "scripts/write_sumeragi_v2_release_receipt.py"),
         "--native-amx-grouped-negative-control-count", "58", "--output", str(output)],
        capture_output=True, text=True, check=False,
    )
    assert result.returncode == 2
    assert "Native release qualification is incomplete" in result.stderr
    assert "cross-dataspace atomic settlement (S6)" in result.stderr
    assert "58-control count" in result.stderr
    assert result.stdout == ""
    assert not output.exists()
