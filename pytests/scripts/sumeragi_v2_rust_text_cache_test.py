"""Pure text retention and exact-source loader controls (no Rust build needed)."""

from __future__ import annotations

from collections import OrderedDict
from concurrent.futures import ThreadPoolExecutor
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
from threading import Barrier

import pytest


ROOT = Path(__file__).resolve().parents[2]
FORMAL = ROOT / "scripts/formal"
TEXT_NAME = "sumeragi_v2_rust_text.py"
READER_NAME = "sumeragi_v2_multilane_reviewed_rust_source.py"
INVENTORY_NAME = "sumeragi_v2_proof_ledger_source_inventory.py"
SPEC = importlib.util.spec_from_file_location("_rust_text_cache_unit", FORMAL / TEXT_NAME)
assert SPEC is not None and SPEC.loader is not None
TEXT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TEXT)


@pytest.mark.parametrize("source", [
    "fn ordinary<'a>(x: &'a str) { x }",
    "a // line\nb /* outer /* nested */ end */ c",
    r'''b"quote\"//" c"str" "\u{1234}" 'x' b'\x7f' '\u{01}' ''',
    '''r###" // /* \" nested "### br#"byte"# cr##"C"## code''',
    "x /* unterminated", 'x "unterminated\\', "x r##\"unterminated",
    "'a 'static 'é' '😀' // Unicode界\r\nnext\n",
    "/*\n*/\n//\n'\\n' \"\n\" r#\"\n\"#",
    "include!(\"path.rs\"); #[path = \"module.rs\"] mod child;",
    "",
])
def test_cached_mask_preserves_exact_lexer_output(source: str) -> None:
    cache = TEXT._MaskCache(64 * 1024, 32)
    expected = TEXT._mask_rust_comments_uncached(source)
    first = cache.mask(source)
    second = cache.mask(source.encode().decode())
    assert first == second == expected
    assert len(first) == len(source)
    assert [i for i, c in enumerate(first) if c == "\n"] == [
        i for i, c in enumerate(source) if c == "\n"
    ]
    assert cache.info()["hits"] == cache.info()["misses"] == 1


def test_mask_hides_literals_comments_and_preserves_lifetimes() -> None:
    source = "a /*x*/ b //y\n'life 'x' r#\"q\"# c\"s\""
    assert TEXT.mask_rust_comments(source) == "a       b    \n'life" + " " * 16


def test_identical_allocated_text_scans_once(monkeypatch: pytest.MonkeyPatch) -> None:
    source = "/* exact independent text */" * 10
    copy = source.encode().decode()
    assert source == copy and source is not copy
    original = TEXT._mask_rust_comments_uncached
    calls = []
    monkeypatch.setattr(TEXT, "_mask_rust_comments_uncached", lambda s: (calls.append(s), original(s))[1])
    cache = TEXT._MaskCache(64 * 1024, 32)
    assert cache.mask(source) is cache.mask(copy)
    assert calls == [source]


def test_exact_byte_limit_one_below_and_wide_unicode() -> None:
    source = '"界😀"' * 12
    masked = TEXT._mask_rust_comments_uncached(source)
    exact = sys.getsizeof(OrderedDict([(source, masked)])) + sys.getsizeof(source) + sys.getsizeof(masked)
    admitted = TEXT._MaskCache(exact, 2)
    refused = TEXT._MaskCache(exact - 1, 2)
    assert admitted.mask(source) == refused.mask(source) == masked
    assert admitted.info()["entries"] == 1
    assert admitted.info()["retained_bytes"] == exact
    assert refused.info()["entries"] == 0
    assert refused.info()["retained_bytes"] <= exact - 1
    refused.mask(source)
    assert refused.info()["misses"] == 2


def test_entry_limit_lru_and_clear() -> None:
    cache = TEXT._MaskCache(64 * 1024, 2)
    cache.mask("a")
    cache.mask("b")
    cache.mask("a")
    cache.mask("c")
    cache.mask("a")
    assert cache.info()["hits"] == 2
    cache.mask("b")
    assert cache.info()["misses"] == 4
    assert cache.info()["entries"] == 2
    cache.clear()
    assert cache.info()["entries"] == cache.info()["hits"] == cache.info()["misses"] == 0
    assert cache.info()["retained_bytes"] == sys.getsizeof(OrderedDict())


def test_real_default_entry_limit_and_string_retention_bound() -> None:
    cache = TEXT._MaskCache(TEXT._MAX_MASK_CACHE_BYTES, TEXT._MAX_MASK_CACHE_ENTRIES)
    for index in range(2100):
        cache.mask(f"/* source {index} */")
        assert cache.info()["entries"] <= 2048
        assert cache.info()["retained_bytes"] <= 64 * 1024 * 1024
    assert cache.info()["entries"] == 2048
    byte_limited = TEXT._MaskCache(8192, 2048)
    for index in range(100):
        byte_limited.mask(f"/* {index} */" + "界😀" * 100)
        assert byte_limited.info()["retained_bytes"] <= 8192
    assert byte_limited.info()["entries"] < 100


def test_oversized_entry_does_not_evict_useful_content() -> None:
    cache = TEXT._MaskCache(1024, 10)
    first = cache.mask("small")
    cache.mask("界" * 4096)
    assert cache.info()["entries"] == 1
    assert cache.mask("small") is first
    assert cache.info()["retained_bytes"] <= 1024


def test_duplicate_threaded_publication_counts_one_entry(monkeypatch: pytest.MonkeyPatch) -> None:
    cache = TEXT._MaskCache(4096, 10)
    barrier = Barrier(2)
    original = TEXT._mask_rust_comments_uncached
    def simultaneous(source: str) -> str:
        barrier.wait(timeout=5)
        return original(source)
    monkeypatch.setattr(TEXT, "_mask_rust_comments_uncached", simultaneous)
    with ThreadPoolExecutor(max_workers=2) as workers:
        outputs = list(workers.map(cache.mask, ["/* same */", "/* same */"]))
    assert outputs[0] is outputs[1]
    assert cache.info()["entries"] == 1
    expected = sys.getsizeof(OrderedDict([("/* same */", outputs[0])])) + sys.getsizeof("/* same */") + sys.getsizeof(outputs[0])
    assert cache.info()["retained_bytes"] == expected


def test_failed_scan_is_not_retained(monkeypatch: pytest.MonkeyPatch) -> None:
    cache = TEXT._MaskCache(4096, 10)
    original = TEXT._mask_rust_comments_uncached
    def fail(_source: str) -> str:
        raise ValueError("scan refused")
    monkeypatch.setattr(TEXT, "_mask_rust_comments_uncached", fail)
    with pytest.raises(ValueError, match="scan refused"):
        cache.mask("text")
    assert cache.info()["entries"] == 0
    monkeypatch.setattr(TEXT, "_mask_rust_comments_uncached", original)
    assert cache.mask("text") == "text"
    assert cache.info()["misses"] == 2


def copied_checker(tmp_path: Path) -> Path:
    directory = tmp_path / "scripts/formal"
    directory.mkdir(parents=True)
    for name in (TEXT_NAME, READER_NAME, INVENTORY_NAME):
        shutil.copyfile(FORMAL / name, directory / name)
    return directory


def child(directory: Path, body: str) -> dict:
    bootstrap = f'''
import importlib.util, json, os, pathlib, sys, types
base = pathlib.Path({str(directory)!r})
def load(name):
    spec = importlib.util.spec_from_file_location(name, base / {READER_NAME!r})
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module
'''
    result = subprocess.run([sys.executable, "-B", "-c", bootstrap + body], capture_output=True, text=True, timeout=30, check=False)
    assert result.returncode == 0, result.stdout + result.stderr
    return json.loads(result.stdout)


def test_loader_shares_aliases_and_executes_source_not_stale_pyc(tmp_path: Path) -> None:
    directory = copied_checker(tmp_path)
    observed = child(directory, f'''
import py_compile
path = base / {TEXT_NAME!r}
original = path.read_bytes()
path.write_bytes(original + b"\\n_SENTINEL = 'A'\\n")
old = path.stat()
py_compile.compile(str(path), doraise=True)
path.write_bytes(original + b"\\n_SENTINEL = 'B'\\n")
os.utime(path, ns=(old.st_atime_ns, old.st_mtime_ns))
a = load("reader_one")
b = load("reader_two")
a._RUST_TEXT_HELPER._clear_mask_cache()
a._mask_rust_comments("/*same*/")
b._mask_rust_comments("/*same*/")
print(json.dumps({{"shared": a._RUST_TEXT_HELPER is b._RUST_TEXT_HELPER,
 "sentinel": a._RUST_TEXT_HELPER._SENTINEL, "stats": a._RUST_TEXT_HELPER._mask_cache_info()}}))
''')
    assert observed["shared"] is True
    assert observed["sentinel"] == "B"
    assert observed["stats"]["hits"] == observed["stats"]["misses"] == 1


@pytest.mark.parametrize("mode", ["foreign", "unknown"])
def test_loader_rejects_unestablished_module(tmp_path: Path, mode: str) -> None:
    directory = copied_checker(tmp_path)
    observed = child(directory, f'''
module = types.ModuleType("_iroha_sumeragi_v2_rust_text")
module.__file__ = str(base / {TEXT_NAME!r}) if {mode!r} == "unknown" else "/foreign/helper.py"
sys.modules[module.__name__] = module
try:
    load("reader")
except RuntimeError as error:
    print(json.dumps({{"error": str(error)}}))
else:
    raise AssertionError("foreign module admitted")
''')
    assert "foreign or unauthenticated module" in observed["error"]


@pytest.mark.parametrize("mode", ["missing", "symlink"])
def test_loader_rejects_missing_or_symlink_source(tmp_path: Path, mode: str) -> None:
    directory = copied_checker(tmp_path)
    path = directory / TEXT_NAME
    path.unlink()
    if mode == "symlink":
        path.symlink_to(FORMAL / TEXT_NAME)
    observed = child(directory, '''
try:
    load("reader")
except RuntimeError as error:
    print(json.dumps({"error": str(error)}))
else:
    raise AssertionError("invalid source admitted")
''')
    assert "reviewed Rust text helper" in observed["error"]


def test_warm_context_rejects_same_size_same_mtime_helper_edit(tmp_path: Path) -> None:
    directory = copied_checker(tmp_path)
    observed = child(directory, f'''
a = load("reader")
with a._reviewed_rust_source_cache():
    a._mask_rust_comments("/* warm */")
path = base / {TEXT_NAME!r}
old = path.stat()
payload = path.read_bytes()
changed = payload.replace(b"Pure Rust masking", b"Pure Rust MASKING", 1)
assert len(changed) == len(payload) and changed != payload
path.write_bytes(changed)
os.utime(path, ns=(old.st_atime_ns, old.st_mtime_ns))
try:
    with a._reviewed_rust_source_cache():
        raise AssertionError("changed helper entered validation")
except RuntimeError as error:
    assert a._ACTIVE_REVIEWED_RUST_SOURCE_CACHE is None
    assert a._ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE is None
    print(json.dumps({{"error": str(error)}}))
''')
    assert "executed source changed" in observed["error"]
