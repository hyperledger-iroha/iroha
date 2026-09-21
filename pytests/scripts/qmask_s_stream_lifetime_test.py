"""Compile the actual private S-stream module to enforce borrowed chunk custody.

Leaf I/O stand-ins make these fast Rust type-system controls independent of Cargo.
They do not exercise encryption, resource accounting or production proof semantics.
"""
from __future__ import annotations
import json
from pathlib import Path
import shutil
import subprocess
import pytest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/global_lookup_statement_v1/vector_arithmetic_plane_openings_v1/ordered_snapshot_v1/q_mask_s_file_v1/stream_v1.rs"
HARNESS = Path(__file__).parent / "fixtures/qmask_s_stream_lifetime_harness.rs.in"
prefix='let mut file=make_file(); let mut read=file.begin_block_read_v1(0).unwrap(); let chunk=read.read_next_slot_v1().unwrap(); '
cases={
 'serial_drop':(prefix+'inspect(chunk.as_slice_v1()); assert_eq!(chunk.len_v1(),16384); drop(chunk); let next=read.read_next_slot_v1().unwrap(); drop(next); drop(read); drop(file);',True,()),
 'normal_scope_end':(prefix+'inspect(chunk.as_slice_v1());',True,()),
 'second_read':(prefix+'let next=read.read_next_slot_v1().unwrap(); inspect(chunk.as_slice_v1()); drop(next);',False,('E0499',)),
 'finish_parent_read':(prefix+'read.finish_v1().unwrap(); inspect(chunk.as_slice_v1());',False,('E0505',)),
 'drop_parent_file':(prefix+'drop(file); inspect(chunk.as_slice_v1());',False,('E0505',)),
 'last_use_then_drop_file':(prefix+'inspect(chunk.as_slice_v1()); drop(file);',False,('E0505',)),
 'last_use_then_drop_read':(prefix+'inspect(chunk.as_slice_v1()); drop(read);',False,('E0505',)),
 'plaintext_borrow_escape':(prefix+'let bytes=chunk.as_slice_v1(); drop(chunk); inspect(bytes);',False,('E0505',)),
 'owner_escape_scope':('let escaped; { let mut file=make_file(); let mut read=file.begin_block_read_v1(0).unwrap(); escaped=read.read_next_slot_v1().unwrap(); } inspect(escaped.as_slice_v1());',False,('E0597',)),
 'clone':(prefix+'let copy=chunk.clone(); inspect(copy.as_slice_v1());',False,('E0599',)),
 'raw_owner_extract':(prefix+'let raw=chunk.chunk; inspect(raw.as_slice_v1());',False,('E0616',)),
}


def compile_case(tmp_path: Path, source: Path, body: str):
    rustc = shutil.which("rustc")
    if rustc is None:
        pytest.skip("pinned Rust compiler required for actual-module lifetime controls")
    driver = tmp_path / "lifetime.rs"
    driver.write_text(HARNESS.read_text().replace("@SOURCE@", str(source)).replace("@BODY@", body))
    result = subprocess.run(
        [rustc, "--edition", "2024", "--crate-type", "lib", "--emit=metadata",
         "--error-format=json", str(driver), "-o", str(tmp_path / "lifetime.rmeta")],
        capture_output=True, text=True, timeout=60, cwd=ROOT,
    )
    diagnostics = [json.loads(line) for line in result.stderr.splitlines()]
    codes = [item["code"]["code"] for item in diagnostics
             if item.get("level") == "error" and item.get("code")]
    return result, codes


@pytest.mark.parametrize("name", cases)
def test_actual_stream_chunk_custody(tmp_path: Path, name: str):
    body, passes, required_codes = cases[name]
    result, codes = compile_case(tmp_path, SOURCE, body)
    assert (result.returncode == 0) == passes, result.stderr
    assert set(required_codes).issubset(codes), result.stderr


def test_destructor_boundary_detects_implicit_drop_regression(tmp_path: Path):
    source = SOURCE.read_text()
    start = source.index("impl Drop for QMaskSReadChunkV1<'_> {")
    end = source.index("impl QMaskSBlockReadV1<'_> {", start)
    mutant = tmp_path / "without_destructor.rs"
    mutant.write_text(source[:start] + source[end:])
    result, _ = compile_case(tmp_path, mutant, cases["last_use_then_drop_read"][0])
    assert result.returncode == 0, result.stderr
    # The unchanged actual module must reject this very same program.
    result, codes = compile_case(tmp_path, SOURCE, cases["last_use_then_drop_read"][0])
    assert result.returncode != 0 and "E0505" in codes, result.stderr
