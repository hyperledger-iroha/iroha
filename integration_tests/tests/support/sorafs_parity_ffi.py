"""Invoke the actual maintained C ABI for local SoraFS parity."""
from __future__ import annotations
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import time
import tempfile

request = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
selected = Path(os.environ["IROHA_NATIVE_LIBRARY_PATH"])
if not selected.is_absolute():
    raise ValueError("IROHA_NATIVE_LIBRARY_PATH must be absolute")
if selected.is_dir():
    selected /= {"darwin": "libconnect_norito_bridge.dylib", "win32": "connect_norito_bridge.dll"}.get(
        sys.platform, "libconnect_norito_bridge.so"
    )
metadata = selected.lstat()
if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
    raise ValueError("FFI library must be a regular non-symlink single-link artifact")
identity = (metadata.st_dev, metadata.st_ino, metadata.st_size)
if metadata.st_size > 512 * 1024 * 1024:
    raise ValueError("FFI artifact exceeds the local unit fixture bound")
# Load an owner-private immutable snapshot so a concurrent rebuild cannot change mapped bytes.
with selected.open("rb") as source:
    opened = os.fstat(source.fileno())
    if identity != (opened.st_dev, opened.st_ino, opened.st_size):
        raise ValueError("FFI artifact changed before snapshot capture")
    artifact_bytes = source.read(512 * 1024 * 1024 + 1)
    closed = os.fstat(source.fileno())
    if identity != (closed.st_dev, closed.st_ino, closed.st_size) or len(artifact_bytes) != metadata.st_size:
        raise ValueError("FFI artifact changed during snapshot capture")
artifact_hash = hashlib.sha256(artifact_bytes).hexdigest()
snapshot_owner = tempfile.TemporaryDirectory(prefix="iroha-sorafs-parity-", ignore_cleanup_errors=True)
snapshot = Path(snapshot_owner.name) / selected.name
with snapshot.open("xb") as output:
    output.write(artifact_bytes)
snapshot.chmod(0o400)
library = ctypes.CDLL(str(snapshot))
abi = library.connect_norito_bridge_abi_version
abi.argtypes = []
abi.restype = ctypes.c_uint32
header = Path(__file__).resolve().parents[3] / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
expected_abi = int(re.search(r"^#define CONNECT_NORITO_BRIDGE_ABI_VERSION (\d+)$",
    header.read_text(encoding="utf-8"), re.MULTILINE).group(1))
if abi() != expected_abi:
    raise ValueError("FFI library does not match the maintained ABI")
free = library.connect_norito_free
free.argtypes = [ctypes.c_void_p]
free.restype = None
fetch = library.connect_norito_sorafs_local_fetch
fetch.argtypes = [ctypes.c_char_p, ctypes.c_ulong] * 3 + [
    ctypes.POINTER(ctypes.c_void_p), ctypes.POINTER(ctypes.c_ulong),
    ctypes.POINTER(ctypes.c_void_p), ctypes.POINTER(ctypes.c_ulong),
]
fetch.restype = ctypes.c_int
plan = request["plan"].encode("utf-8")
providers = json.dumps(request["providers"], separators=(",", ":")).encode("utf-8")
options = json.dumps(request["options"], separators=(",", ":")).encode("utf-8")
payload_ptr, report_ptr = ctypes.c_void_p(), ctypes.c_void_p()
payload_len, report_len = ctypes.c_ulong(), ctypes.c_ulong()
try:
    start = time.perf_counter_ns()
    code = fetch(plan, len(plan), providers, len(providers), options, len(options),
        ctypes.byref(payload_ptr), ctypes.byref(payload_len), ctypes.byref(report_ptr), ctypes.byref(report_len))
    duration = time.perf_counter_ns() - start
    if code != 0:
        raise RuntimeError(f"FFI fetch returned error {code}")
    if not payload_ptr.value or not report_ptr.value:
        raise ValueError("successful FFI fetch must return both owned buffers")
    if payload_len.value > 16 * 1024 * 1024 or report_len.value > 16 * 1024 * 1024:
        raise ValueError("FFI parity output exceeds fixture bound")
    payload = ctypes.string_at(payload_ptr, payload_len.value)
    report = json.loads(ctypes.string_at(report_ptr, report_len.value).decode("utf-8"))
finally:
    if payload_ptr.value:
        free(payload_ptr)
    if report_ptr.value:
        free(report_ptr)
after = selected.lstat()
if identity != (after.st_dev, after.st_ino, after.st_size) or artifact_hash != hashlib.sha256(selected.read_bytes()).hexdigest():
    raise ValueError("FFI artifact changed during native parity custody")
if artifact_hash != hashlib.sha256(snapshot.read_bytes()).hexdigest():
    raise ValueError("loaded FFI snapshot changed during native parity custody")
print(json.dumps({"return_code": code, "payload_hex": payload.hex(), "report": report,
    "duration_ns": duration, "artifact_sha256": artifact_hash}, separators=(",", ":")))
