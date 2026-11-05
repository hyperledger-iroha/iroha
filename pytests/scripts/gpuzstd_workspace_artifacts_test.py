"""Check exact backend export admission without claiming native artifact qualification."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "gpuzstd_workspace_artifacts",
    ROOT / "crates/gpuzstd_cuda/tests/verify_workspace_artifacts.py",
)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def test_backend_exports_are_exact_and_unknown_owners_are_rejected() -> None:
    assert CHECKER.c_abi_symbols("gpuzstd_cuda") == (
        "gpuzstd_cuda_compress", "gpuzstd_cuda_decompress",
    )
    assert CHECKER.c_abi_symbols("gpuzstd_metal") == (
        "gpu_zstd_compress", "gpu_zstd_decompress",
    )
    with pytest.raises(ValueError, match="unknown GPU Zstd backend"):
        CHECKER.c_abi_symbols("unknown")


@pytest.mark.parametrize("mutation", [None, "old_cuda_exports", "duplicate_cuda", "metal_import"])
def test_co_built_artifact_symbol_admission_keeps_backend_ownership(
    monkeypatch: pytest.MonkeyPatch, mutation: str | None,
) -> None:
    artifacts = {
        "gpuzstd_cuda": {"plugin": Path("cuda.so"), "rlib": Path("cuda.rlib")},
        "gpuzstd_metal": {"plugin": Path("metal.so")},
    }
    monkeypatch.setattr(CHECKER.sys, "platform", "linux")

    def symbol_output(*command: str) -> str:
        assert command[0] == "nm"
        if command[1] == "-a":
            if mutation == "metal_import" and command[-1] == "cuda.rlib":
                return "0000 U gpuzstd_metal_device_owner\n"
            return "0000 T portable_frame_codec\n"
        assert command[1:3] == ("-D", "--defined-only")
        if command[-1] == "metal.so" or mutation == "old_cuda_exports":
            return "0000 T gpu_zstd_compress\n0001 T gpu_zstd_decompress\n"
        symbols = "0000 T gpuzstd_cuda_compress\n0001 T gpuzstd_cuda_decompress\n"
        if mutation == "duplicate_cuda":
            symbols += "0002 T gpuzstd_cuda_compress\n"
        return symbols

    monkeypatch.setattr(CHECKER, "output", symbol_output)
    if mutation is None:
        CHECKER.inspect_symbols(artifacts)
    else:
        error = "Metal implementation" if mutation == "metal_import" else "exactly one gpuzstd_cuda_compress"
        with pytest.raises(AssertionError, match=error):
            CHECKER.inspect_symbols(artifacts)
