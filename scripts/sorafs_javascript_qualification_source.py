"""Fixed original suite/fixture content for the installed JavaScript child.

Pure relation: performs no I/O, imports of suites, npm or native execution. The
pinned catalog owns exact source names and unchanged registration/helper/module
metadata bytes. Fixture bytes remain supplied by the independently authenticated
candidate owner; this relation does not grant that authority or release approval.
TODO: join this private core to the fixed runner/tool closure and candidate owner.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib

from sorafs_javascript_archive import MAX_NAME_BYTES, NpmPathInventory, _require
from sorafs_javascript_dependencies import _json
from sorafs_javascript_package_source import (
    MAX_SOURCE_BYTES, MAX_SOURCE_FILE_BYTES, PACKAGE_RECIPE_SHA256,
)

CATALOG_SHA256 = "d596804a89108bd60cdafcd9369f800d4e4938605de8435571feff606e37a9b1"
MAX_CATALOG_BYTES = 64 * 1024
SOURCE_FILES = 9
FIXTURE_FILES = 182
CORE_FILES = SOURCE_FILES + FIXTURE_FILES
METADATA = "fixtures/sorafs_orchestrator/multi_peer_parity_v1/metadata.json"
PAYLOAD = "fuzz/sorafs_chunker/sf1_profile_v1_input.bin"
PACKAGE = "javascript/iroha_js/package.json"
CONTRACT = "javascript/iroha_js/test/fixtures/sorafs_native_suite_contract_v1.json"
FIXTURE_PREFIXES = ("fixtures/sorafs_manifest/", "fixtures/sorafs_orchestrator/multi_peer_parity_v1/")


@dataclass(frozen=True)
class QualificationMember:
    """Exact source-relative immutable original bytes; no execution statement."""
    path: str
    content: bytes


@dataclass(frozen=True)
class QualificationProjection:
    """Closed private suite/fixture core, separate from npm installed packages."""
    catalog: bytes
    members: tuple[QualificationMember, ...]


def _catalog(raw: bytes) -> tuple[dict[str, str], tuple[str, ...]]:
    """Read only the exact bounded source-owned selection, never caller aliases."""
    _require(type(raw) is bytes and 0 < len(raw) <= MAX_CATALOG_BYTES
             and hashlib.sha256(raw).hexdigest() == CATALOG_SHA256,
             "qualification catalog differs from its reviewed source selection")
    value = _json(raw, MAX_CATALOG_BYTES)
    _require(set(value) == {"schema", "source_files", "fixture_files"}
             and value["schema"] == "sorafs.javascript.qualification_source_catalog.v1"
             and type(value["source_files"]) is list and len(value["source_files"]) == SOURCE_FILES
             and type(value["fixture_files"]) is list and len(value["fixture_files"]) == FIXTURE_FILES,
             "qualification catalog shape differs")
    source = {row["path"]: row["sha256"] for row in value["source_files"]}
    fixtures = tuple(value["fixture_files"])
    _require(len(source) == SOURCE_FILES and source[PACKAGE] == PACKAGE_RECIPE_SHA256
             and CONTRACT in source and METADATA in fixtures and PAYLOAD in fixtures
             and all(name == PAYLOAD or name.startswith(FIXTURE_PREFIXES) for name in fixtures),
             "qualification catalog selection differs")
    inventory = NpmPathInventory()
    for name in (*source, *fixtures):
        inventory.admit(name)
    return source, fixtures


def _source_shape(sources: dict[str, bytes]) -> None:
    """Reject excessive cardinality/names/bytes before set/sort/hash allocation."""
    _require(type(sources) is dict and len(sources) == CORE_FILES,
             "qualification source inventory differs")
    total = 0
    for name, raw in sources.items():
        _require(type(name) is str and 0 < len(name) <= MAX_NAME_BYTES - len("package/")
                 and type(raw) is bytes and len(raw) <= MAX_SOURCE_FILE_BYTES,
                 "qualification source name or per-file byte bound differs")
        total += len(raw)
        _require(total <= MAX_SOURCE_BYTES, "qualification source total byte bound exceeded")


def qualification_projection(sources: dict[str, bytes], *, catalog: bytes) -> QualificationProjection:
    """Join the complete fixed census to unchanged suites and exact payload scope.

    Six existing registration bodies, the pure native requirement helper, the
    canonical assertion contract and copied package metadata are byte-pinned.
    All 182 fixture names are mandatory. Fixtures' semantic/candidate authority
    remains external; a dictionary or this frozen result cannot supply it.
    """
    _source_shape(sources)
    code, fixtures = _catalog(catalog)
    _require(set(sources) == code.keys() | set(fixtures),
             "qualification source census contains omissions, selectors or extra files")
    _require(all(hashlib.sha256(sources[name]).hexdigest() == digest
                 for name, digest in code.items()),
             "qualification shared code or module metadata differs from original")
    metadata = _json(sources[METADATA], 32 * 1024)
    _require(metadata.get("payload_path") == PAYLOAD
             and type(metadata.get("payload_bytes")) is int
             and metadata["payload_bytes"] > 0
             and metadata["payload_bytes"] == len(sources[PAYLOAD]),
             "qualification metadata escapes or differs from the sole original payload")
    return QualificationProjection(catalog, tuple(QualificationMember(name, raw)
                                    for name, raw in sorted(sources.items())))


def validate_qualification_projection(projection: QualificationProjection) -> None:
    """Re-derive a typed immutable relation rather than accepting stored claims."""
    _require(type(projection) is QualificationProjection and type(projection.members) is tuple
             and len(projection.members) == CORE_FILES,
             "qualification projection shape differs")
    for row in projection.members:
        _require(type(row) is QualificationMember and type(row.path) is str
                 and 0 < len(row.path) <= MAX_NAME_BYTES - len("package/")
                 and type(row.content) is bytes and len(row.content) <= MAX_SOURCE_FILE_BYTES,
                 "qualification projection member shape differs")
    sources = {row.path: row.content for row in projection.members}
    _require(qualification_projection(sources, catalog=projection.catalog) == projection,
             "qualification projection differs from its original sorted content")


def verify_qualification_content(projection: QualificationProjection, *,
                                 files: dict[str, bytes], modes: dict[str, int]) -> None:
    """Compare a bounded captured core to originals; does not prove live custody."""
    validate_qualification_projection(projection)
    _source_shape(files)
    _require(type(modes) is dict and len(modes) == CORE_FILES
             and all(type(name) is str and 0 < len(name) <= MAX_NAME_BYTES - len("package/")
                     and type(mode) is int and mode == 0o644 for name, mode in modes.items()),
             "qualification mode inventory differs")
    expected = {row.path: row.content for row in projection.members}
    _require(files == expected and modes.keys() == expected.keys(),
             "qualification captured tree differs from original content")
