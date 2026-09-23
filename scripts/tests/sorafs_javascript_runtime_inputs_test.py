"""Synthetic original-byte runtime controls; no fixture executable is run.

The existing source-owned Mach-O fixture builds the original dependency command.
Additional inert commands are framed independently with struct, not the parser.
"""
from __future__ import annotations

import copy
import builtins
import os
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_javascript_runtime_inputs as owner
import sorafs_javascript_runtime_graph as graph


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def existing_fixture():
    path = ROOT / "pytests/scripts/sumeragi_v2_framework_python_relocation_test.py"
    spec = importlib.util.spec_from_file_location("original_macho_fixture", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module._thin_macho(0x0100000C)


def command(kind, name):
    head = 24 if kind in (0xC, 0xD) else 12
    body = name.encode() + b"\0"
    size = (head + len(body) + 7) & ~7
    return (struct.pack("<3I", kind, size, head) + bytes(head - 12)
            + body + bytes(size - head - len(body)))


def image(kind, commands):
    return struct.pack("<8I", 0xFEEDFACF, 0x0100000C, 0, kind, len(commands),
                       sum(map(len, commands)), 0, 0) + b"".join(commands)


def fixture():
    # Historical labels deliberately do not refer to existing files.
    exe, lib = "/observed/node/bin/node", "/observed/node/lib/Fixture"
    root = image(2, [command(0xE, "/usr/lib/dyld"), command(0x8000001C, "@loader_path"),
                     command(0x8000001C, "@loader_path/../lib"), existing_fixture()[32:]])
    shared = image(6, [command(0xD, lib), command(0xC, "/usr/lib/libSystem.B.dylib")])
    files = {exe: root, lib: shared}
    row = {"schema": owner.SCHEMA, "platform": "darwin", "architecture": "arm64", "version": "24.21.0",
           "selected_executable": "/selected/node/bin/node", "executable": exe,
           "images": [{"path": path, "sha256": sha(raw), "size": len(raw), "mode": 0o755}
                      for path, raw in sorted(files.items())],
           "aliases": [{"path": "/selected/node", "target": "../observed/node", "resolved": "/observed/node"}],
           "edges": [
               {"source": exe, "index": 3, "command": 12, "name": "@rpath/Fixture", "scope": "runtime",
                "candidates": [{"path": "/observed/node/bin/Fixture", "resolved": None},
                               {"path": lib, "resolved": lib}], "selected": lib},
               {"source": lib, "index": 1, "command": 12, "name": "/usr/lib/libSystem.B.dylib",
                "scope": "normal_os", "candidates": [], "selected": None}]}
    return row, files


def inherited_fixture():
    """Three original images with a direct parent and shared rpath leaf."""
    exe = "/observed/node/bin/node"
    parent = "/observed/node/plugins/Parent"
    common = "/observed/node/plugins/lib/Common"
    files = {
        exe: image(2, [command(0xE, "/usr/lib/dyld"),
                       command(0x8000001C, "@loader_path"),
                       command(0x8000001C, "@loader_path/../lib"),
                       command(0xC, parent)]),
        parent: image(6, [command(0xD, parent),
                          command(0x8000001C, "@loader_path/lib"),
                          command(0xC, "@rpath/Common")]),
        common: image(6, [command(0xD, common)]),
    }
    row = {"schema": owner.SCHEMA, "platform": "darwin", "architecture": "arm64",
           "version": "24.21.0", "selected_executable": "/selected/node/bin/node",
           "executable": exe,
           "images": [{"path": path, "sha256": sha(raw), "size": len(raw), "mode": 0o755}
                      for path, raw in sorted(files.items())],
           "aliases": [{"path": "/selected/node", "target": "../observed/node",
                        "resolved": "/observed/node"}],
           "edges": [
               {"source": exe, "index": 3, "command": 12, "name": parent,
                "scope": "runtime", "candidates": [{"path": parent, "resolved": parent}],
                "selected": parent},
               {"source": parent, "index": 2, "command": 12, "name": "@rpath/Common",
                "scope": "runtime", "candidates": [
                    {"path": common, "resolved": common},
                    {"path": "/observed/node/bin/Common", "resolved": None},
                    {"path": "/observed/node/lib/Common", "resolved": None}],
                "selected": common},
           ]}
    return row, files


def packed(row, files):
    raw = owner.canonical_json(row)
    return owner.MAGIC + struct.pack(">Q", len(raw)) + raw + b"".join(files[item["path"]] for item in row["images"]), sha(raw)


def parse(row, files):
    raw, pin = packed(row, files)
    return owner.parse_node_runtime_bundle(raw, expected_manifest_sha256=pin)


def replace_image(row, files, path, body):
    files[path] = body
    record = next(item for item in row["images"] if item["path"] == path)
    record.update(size=len(body), sha256=sha(body))


def test_original_fixture_roundtrip_without_any_historical_path_io(monkeypatch):
    row, files = fixture()
    def no_io(*_args, **_kwargs):
        raise AssertionError("pure relation attempted filesystem I/O")
    monkeypatch.setattr(builtins, "open", no_io)
    monkeypatch.setattr(os, "open", no_io)
    monkeypatch.setattr(Path, "open", no_io)
    monkeypatch.setattr(Path, "resolve", no_io)
    result = parse(row, files)
    assert result.manifest.executable == row["executable"]
    assert result.manifest.edges[0].candidates[0].resolved is None
    assert result.manifest.aliases[0].resolved == "/observed/node"
    assert {path: result.member_bytes(path) for path in files} == files
    with pytest.raises(owner.RuntimeInputError, match="absent"):
        result.member_bytes("/foreign")
    with pytest.raises(Exception):
        result.manifest.version = "24.0.0"


def produce(row, files):
    return owner.produce_node_runtime_manifest(
        version=row["version"], selected_executable=row["selected_executable"],
        executable=row["executable"],
        original_images={path: (body, next(item["mode"] for item in row["images"]
                                           if item["path"] == path))
                         for path, body in files.items()},
        aliases={item["path"]: item["target"] for item in row["aliases"]})


def test_producer_derives_complete_canonical_manifest_without_path_io(monkeypatch):
    row, files = fixture()
    def no_io(*_args, **_kwargs):
        raise AssertionError("pure producer attempted filesystem I/O")
    monkeypatch.setattr(builtins, "open", no_io)
    monkeypatch.setattr(os, "open", no_io)
    monkeypatch.setattr(Path, "open", no_io)
    monkeypatch.setattr(Path, "resolve", no_io)
    manifest = produce(row, files)
    assert manifest.raw == owner.canonical_json(row)
    assert manifest.sha256 == sha(manifest.raw)
    assert manifest.aliases[0].resolved == "/observed/node"
    bundle = (owner.MAGIC + struct.pack(">Q", len(manifest.raw)) + manifest.raw
              + b"".join(files[item.path] for item in manifest.images))
    assert owner.parse_node_runtime_bundle(
        bundle, expected_manifest_sha256=manifest.sha256).manifest == manifest


def test_producer_derives_inherited_shared_slots_from_original_bytes():
    row, files = inherited_fixture()
    produced = produce(row, files)
    assert produced.raw == owner.canonical_json(row)
    assert tuple(slot.path for slot in produced.edges[1].candidates) == (
        "/observed/node/plugins/lib/Common", "/observed/node/bin/Common",
        "/observed/node/lib/Common")
    stale = copy.deepcopy(row)
    stale["edges"][1]["candidates"].pop()
    with pytest.raises(owner.RuntimeInputError, match="command/candidate relation differs"):
        parse(stale, files)


def test_producer_derives_all_slots_for_two_shared_requesters():
    row, files = inherited_fixture()
    exe = row["executable"]
    peer = "/observed/node/plugins/Peer"
    common = "/observed/node/plugins/lib/Common"
    replace_image(row, files, exe, image(2, [
        command(0xE, "/usr/lib/dyld"),
        command(0x8000001C, "@loader_path"),
        command(0x8000001C, "@loader_path/../lib"),
        command(0xC, "/observed/node/plugins/Parent"),
        command(0xC, peer),
    ]))
    files[peer] = image(6, [command(0xD, peer),
                            command(0x8000001C, "@loader_path/lib"),
                            command(0xC, "@rpath/Common")])
    row["images"] = [{"path": path, "sha256": sha(body), "size": len(body), "mode": 0o755}
                     for path, body in sorted(files.items())]
    produced = produce(row, files)
    shared = [edge for edge in produced.edges if edge.name == "@rpath/Common"]
    assert len(shared) == 2
    assert {edge.source for edge in shared} == {"/observed/node/plugins/Parent", peer}
    assert all(tuple(slot.path for slot in edge.candidates) == (
        common, "/observed/node/bin/Common", "/observed/node/lib/Common") for edge in shared)


def test_producer_rederives_slots_after_original_command_change():
    row, files = fixture()
    exe = row["executable"]
    replace_image(row, files, exe, image(2, [
        command(0xE, "/usr/lib/dyld"),
        command(0x8000001C, "@loader_path/../lib"),
        command(0x8000001C, "@loader_path"),
        existing_fixture()[32:],
    ]))
    produced = produce(row, files)
    assert tuple(slot.path for slot in produced.edges[0].candidates) == (
        "/observed/node/lib/Fixture", "/observed/node/bin/Fixture")
    assert produced.edges[0].candidates[1].resolved is None
    assert produced.raw != owner.canonical_json(row)
    with pytest.raises(owner.RuntimeInputError, match="command/candidate relation differs"):
        parse(row, files)


@pytest.mark.parametrize("case", ["version", "image-bytearray", "image-size", "image-mode",
                                  "image-extra", "alias-cycle", "alias-unused", "candidate-budget",
                                  "manifest-budget"])
def test_producer_rejects_invalid_or_unfunded_inputs(case, monkeypatch):
    row, files = fixture()
    originals = {path: (body, 0o755) for path, body in files.items()}
    aliases = {item["path"]: item["target"] for item in row["aliases"]}
    version = row["version"]
    if case == "version": version = "24.01.0"
    elif case == "image-bytearray":
        path = row["executable"]; originals[path] = (bytearray(files[path]), 0o755)
    elif case == "image-size":
        path = row["executable"]; originals[path] = (b"x" * 31, 0o755)
    elif case == "image-mode":
        path = row["executable"]; originals[path] = (files[path], 0o777)
    elif case == "image-extra":
        path = "/unreachable/Extra"; originals[path] = (image(6, [command(0xD, path)]), 0o755)
    elif case == "alias-cycle": aliases["/selected/node"] = "/selected/node"
    elif case == "alias-unused": aliases["/unused"] = "/observed/node"
    elif case == "candidate-budget": monkeypatch.setattr(owner, "MAX_CANDIDATES", 1)
    elif case == "manifest-budget":
        monkeypatch.setattr(owner, "MAX_MANIFEST_BYTES", len(owner.canonical_json(row)) - 1)
    with pytest.raises(owner.RuntimeInputError):
        owner.produce_node_runtime_manifest(
            version=version, selected_executable=row["selected_executable"],
            executable=row["executable"], original_images=originals, aliases=aliases)


@pytest.mark.parametrize("case", ["schema", "platform", "architecture", "version", "version-leading-zero", "extra", "missing",
    "images-empty", "images-reversed", "images-duplicate", "size-bool", "size-small", "size-over", "mode-bool", "mode-write",
    "mode-no-exec", "digest", "selected", "alias-resolution", "alias-dangling", "alias-cycle", "alias-extra", "alias-case",
    "alias-file", "alias-ancestor", "system-original", "path-dot", "path-nfc", "path-surrogate", "path-control",
    "edge-order", "edge-duplicate", "edge-missing", "edge-extra", "edge-name", "edge-index", "edge-index-bool", "edge-command",
    "edge-scope", "candidate-order", "candidate-missing", "candidate-extra", "candidate-present", "selected-wrong", "system-candidate"])
def test_resealed_manifest_rejects_wrong_content_relationships(case):
    row, files = fixture()
    if case in ("schema", "platform", "architecture", "version"): row[case] = "foreign"
    elif case == "version-leading-zero": row["version"] = "24.01.0"
    elif case == "extra": row["approved"] = True
    elif case == "missing": row.pop("aliases")
    elif case == "images-empty": row["images"] = []
    elif case == "images-reversed": row["images"].reverse()
    elif case == "images-duplicate": row["images"].append(copy.deepcopy(row["images"][-1]))
    elif case.startswith("size-"): row["images"][0]["size"] = {"size-bool": True, "size-small": 31, "size-over": owner.MAX_IMAGE_BYTES + 1}[case]
    elif case.startswith("mode-"): row["images"][0]["mode"] = {"mode-bool": True,"mode-write": 0o777,"mode-no-exec": 0o644}[case]
    elif case == "digest": row["images"][0]["sha256"] = "f" * 64
    elif case == "selected": row["selected_executable"] = "/foreign/node"
    elif case == "alias-resolution": row["aliases"][0]["resolved"] = "/observed/node/lib"
    elif case == "alias-dangling": row["aliases"][0]["target"] = "/missing/node"
    elif case == "alias-cycle": row["aliases"][0]["target"] = "/selected/node"
    elif case == "alias-extra": row["aliases"].append({"path":"/unused", "target":"/observed/node", "resolved":"/observed/node"})
    elif case == "alias-case": row["aliases"].append({"path":"/selected/Node", "target":"/observed/node", "resolved":"/observed/node"}); row["aliases"].sort(key=lambda item:item["path"])
    elif case == "alias-file": row["aliases"][0]["path"] = row["executable"]
    elif case == "alias-ancestor": row["aliases"][0]["path"] = "/observed/node"
    elif case == "system-original": row["images"][0]["path"] = "/usr/lib/node"; files["/usr/lib/node"] = files[row["executable"]]
    elif case.startswith("path-"):
        row["selected_executable"] = {"path-dot":"/selected/../node", "path-nfc":"/e\u0301/node", "path-surrogate":"/\ud800/node", "path-control":"/selected/\nnode"}[case]
    elif case == "edge-order": row["edges"].reverse()
    elif case == "edge-duplicate": row["edges"].append(copy.deepcopy(row["edges"][-1]))
    elif case == "edge-missing": row["edges"].pop()
    elif case == "edge-extra": row["edges"].append({**row["edges"][-1], "index":2})
    elif case == "edge-name": row["edges"][0]["name"] = "@rpath/Substitution"
    elif case == "edge-index": row["edges"][0]["index"] = 2
    elif case == "edge-index-bool": row["edges"][0]["index"] = True
    elif case == "edge-command": row["edges"][0]["command"] = 0x80000018
    elif case == "edge-scope": row["edges"][0]["scope"] = "normal_os"
    elif case == "candidate-order": row["edges"][0]["candidates"].reverse()
    elif case == "candidate-missing": row["edges"][0]["candidates"].pop(0)
    elif case == "candidate-extra": row["edges"][0]["candidates"].append({"path":"/other/Fixture", "resolved":None})
    elif case == "candidate-present": row["edges"][0]["candidates"][0]["resolved"] = row["executable"]
    elif case == "selected-wrong": row["edges"][0]["selected"] = row["executable"]
    elif case == "system-candidate": row["edges"][-1]["candidates"] = [{"path":"/usr/lib/libSystem.B.dylib", "resolved":None}]
    with pytest.raises(owner.RuntimeInputError):
        parse(row, files)


@pytest.mark.parametrize("case", ["missing-image", "unreachable-image", "earlier-candidate-inserted", "dangling-candidate-alias", "transitive-missing", "id-collision"])
def test_complete_original_closure_and_negative_candidate_slots(case):
    row, files = fixture()
    exe, lib = row["executable"], row["images"][1]["path"]
    if case == "missing-image": row["images"].pop()
    elif case in ("unreachable-image", "earlier-candidate-inserted"):
        path = "/other/Extra" if case == "unreachable-image" else "/observed/node/bin/Fixture"
        body = image(6, [command(0xD, path)])
        files[path] = body; row["images"].append({"path":path,"size":len(body),"sha256":sha(body),"mode":0o755}); row["images"].sort(key=lambda item:item["path"])
    elif case == "dangling-candidate-alias":
        row["aliases"].insert(0, {"path":"/observed/node/bin/Fixture", "target":"/absent/Fixture", "resolved":"/absent/Fixture"})
    elif case == "transitive-missing": replace_image(row, files, lib, image(6, [command(0xD, lib), command(0xC, "/unlisted/Other")]))
    elif case == "id-collision":
        path = "/other/Extra"; body = image(6, [command(0xD, lib)])
        files[path] = body; row["images"].append({"path":path,"size":len(body),"sha256":sha(body),"mode":0o755}); row["images"].sort(key=lambda item:item["path"])
    with pytest.raises(owner.RuntimeInputError): parse(row, files)


@pytest.mark.parametrize("case", ["fat", "x86", "arm64e", "wrong-type", "reserved", "count-zero", "count-over", "table-over", "table-truncated",
    "unknown", "environment", "weak", "reexport", "lazy", "upward", "duplicate-rpath", "aliased-rpath", "bad-linker", "no-linker", "shared-rpath", "system-dot", "internal-dot", "unknown-token"])
def test_resealed_images_preserve_sole_parser_and_closed_loader_profile(case):
    row, files = fixture(); exe = row["executable"]; body = files[exe]
    replacements = {"fat": (0,0xBEBAFECA), "x86":(4,0x01000007), "arm64e":(8,2), "wrong-type":(12,6), "reserved":(28,1),
                    "count-zero":(16,0), "count-over":(16,4097), "table-over":(20,graph.MAX_COMMAND_BYTES+1), "table-truncated":(20,len(body))}
    if case in replacements:
        at, value = replacements[case]; body = body[:at] + struct.pack("<I",value) + body[at+4:]
    else:
        commands = [command(0xE,"/usr/lib/dyld"), command(0x8000001C,"@loader_path"), command(0x8000001C,"@loader_path/../lib"), existing_fixture()[32:]]
        if case in ("unknown","environment","weak","reexport","lazy","upward"):
            commands.append(command({"unknown":0x12345678,"environment":0x27,"weak":0x80000018,"reexport":0x8000001F,"lazy":0x20,"upward":0x80000023}[case],"/other/Library"))
        elif case == "duplicate-rpath": commands.append(commands[1])
        elif case == "aliased-rpath": commands.append(command(0x8000001C,"/observed/node/bin"))
        elif case == "bad-linker": commands[0] = command(0xE,"/foreign/dyld")
        elif case == "no-linker": commands.pop(0)
        elif case == "shared-rpath":
            lib = row["images"][1]["path"]
            replace_image(row,files,lib,image(6,[command(0xD,lib),command(0x8000001C,"@loader_path"),command(0xC,"@rpath/Other")]))
        elif case == "system-dot": commands[-1] = command(0xC,"/usr/lib/../../opt/evil")
        elif case == "internal-dot": commands[-1] = command(0xC,"@loader_path/link/../evil")
        elif case == "unknown-token": commands[-1] = command(0xC,"@unknown/evil")
        body = image(2,commands)
    replace_image(row, files, exe, body)
    with pytest.raises(owner.RuntimeInputError): parse(row, files)


@pytest.mark.parametrize("case", ["bad-pin", "zero-pin", "noncanonical", "duplicate-json", "trailing", "truncated", "bad-magic", "length-zero", "length-over", "length-truncated", "bytearray"])
def test_envelope_pin_and_exact_byte_ownership(case):
    row, files = fixture(); raw, pin = packed(row,files)
    if case == "bad-pin": pin = "a"*64
    elif case == "zero-pin": pin = "0"*64
    elif case in ("noncanonical","duplicate-json"):
        manifest = (json.dumps(row)+"\n").encode() if case == "noncanonical" else owner.canonical_json(row).replace(b'{',b'{"schema":"discarded",',1)
        pin = sha(manifest); raw=owner.MAGIC+struct.pack(">Q",len(manifest))+manifest+b"".join(files[item["path"]] for item in row["images"])
    elif case == "trailing": raw += b"x"
    elif case == "truncated": raw = raw[:-1]
    elif case == "bad-magic": raw = b"X"+raw[1:]
    elif case.startswith("length-"):
        size = {"length-zero":0,"length-over":owner.MAX_MANIFEST_BYTES+1,"length-truncated":len(raw)}[case]
        raw = owner.MAGIC + struct.pack(">Q",size) + raw[len(owner.MAGIC)+8:]
    elif case == "bytearray": raw = bytearray(raw)
    with pytest.raises(owner.RuntimeInputError): owner.parse_node_runtime_bundle(raw,expected_manifest_sha256=pin)


def test_exact_command_table_admission_before_shared_decoder(monkeypatch):
    row, files = fixture(); body = files[row["executable"]]
    observed = []; actual = graph._parse_macho_thin
    def spy(*args): observed.append(1); return actual(*args)
    monkeypatch.setattr(graph,"_parse_macho_thin",spy)
    size = struct.unpack_from("<I",body,20)[0]
    monkeypatch.setattr(graph,"MAX_COMMAND_BYTES",size)
    parse(row,files); assert len(observed)==2
    observed.clear(); monkeypatch.setattr(graph,"MAX_COMMAND_BYTES",size-1)
    with pytest.raises(owner.RuntimeInputError,match="admission"): parse(row,files)
    assert observed==[]


@pytest.mark.parametrize("limit", ["MAX_MANIFEST_BYTES","MAX_BUNDLE_BYTES","MAX_IMAGES","MAX_ALIASES","MAX_EDGES","MAX_CANDIDATES","MAX_RUNTIME_BYTES","MAX_NAMESPACE_NODES","MAX_NAMESPACE_BYTES"])
def test_exact_and_one_less_resource_bounds(monkeypatch,limit):
    row, files = fixture(); raw,pin = packed(row,files); manifest=owner.parse_node_runtime_manifest(owner.canonical_json(row),expected_sha256=pin)
    if limit.startswith("MAX_NAMESPACE"):
        paths = [item.path for item in manifest.images]+[item.path for item in manifest.aliases]+[item.resolved for item in manifest.aliases]+[manifest.selected_executable]+[slot.path for edge in manifest.edges for slot in edge.candidates]
        names = {name for path in paths for name in (path,*(str(parent) for parent in Path(path).parents))}
        bound = len(names) if limit.endswith("NODES") else sum(len(name.encode()) for name in names)
    else:
        bound={"MAX_MANIFEST_BYTES":len(owner.canonical_json(row)),"MAX_BUNDLE_BYTES":len(raw),"MAX_IMAGES":2,"MAX_ALIASES":1,"MAX_EDGES":2,"MAX_CANDIDATES":2,"MAX_RUNTIME_BYTES":sum(map(len,files.values()))}[limit]
    monkeypatch.setattr(owner,limit,bound)
    owner.parse_node_runtime_bundle(raw,expected_manifest_sha256=pin)
    monkeypatch.setattr(owner,limit,bound-1)
    with pytest.raises(owner.RuntimeInputError): owner.parse_node_runtime_bundle(raw,expected_manifest_sha256=pin)


def test_fixed_resource_contract_literals():
    assert (owner.MAX_IMAGES,owner.MAX_ALIASES,owner.MAX_EDGES,owner.MAX_CANDIDATES)==(128,256,4096,16384)
    assert (owner.MAX_IMAGE_BYTES,owner.MAX_RUNTIME_BYTES,owner.MAX_MANIFEST_BYTES)==(256*1024**2,512*1024**2,4*1024**2)
    assert (graph.MAX_COMMAND_BYTES,graph.MAX_COMMANDS,graph.MAX_RPATHS)==(1024**2,4096,32)


def test_absent_leaf_under_pinned_directory_alias_is_distinct_from_dangling_link():
    row, files = fixture(); exe=row["executable"]
    # Existing selected directory alias also supplies the first, absent rpath.
    root=image(2,[command(0xE,"/usr/lib/dyld"),command(0x8000001C,"/selected/node/bin"),
                  command(0x8000001C,"@executable_path/../lib"),existing_fixture()[32:]])
    replace_image(row,files,exe,root)
    row["edges"][0]["candidates"][0]["path"]="/selected/node/bin/Fixture"
    result=parse(row,files)
    assert result.manifest.edges[0].candidates[0].resolved is None
    row["aliases"].append({"path":"/selected/node/bin/Fixture","target":"/missing/Fixture","resolved":"/missing/Fixture"})
    with pytest.raises(owner.RuntimeInputError): parse(row,files)


def test_shared_own_rpath_present_still_refuses_unimplemented_inherited_and_cached_lookup():
    row,files=fixture();lib=row["images"][1]["path"]
    # This mirrors the locally observed Brotli shape. A physically present own
    # candidate is insufficient to certify dyld loadability/cached-name order.
    replace_image(row,files,lib,image(6,[command(0xD,lib),command(0x8000001C,"@loader_path"),
                                      command(0xC,"@rpath/Fixture")]))
    row["edges"][1]={"source":lib,"index":2,"command":12,"name":"@rpath/Fixture","scope":"runtime",
                     "candidates":[{"path":lib,"resolved":lib}],"selected":lib}
    with pytest.raises(owner.RuntimeInputError,match="command/candidate relation differs"):
        parse(row,files)


def test_shared_rpath_input_relation_includes_every_original_ancestor_slot():
    row, files = inherited_fixture()
    result = parse(row, files)
    assert tuple(slot.path for slot in result.manifest.edges[1].candidates) == (
        "/observed/node/plugins/lib/Common", "/observed/node/bin/Common",
        "/observed/node/lib/Common")
    # A resealed manifest that omits either absent ancestor remains incomplete.
    for index in (1, 2):
        incomplete = copy.deepcopy(row)
        incomplete["edges"][1]["candidates"].pop(index)
        with pytest.raises(owner.RuntimeInputError):
            parse(incomplete, files)


def test_two_shared_rpath_requesters_each_require_original_ancestor_slots():
    row, files = inherited_fixture()
    exe = row["executable"]
    peer = "/observed/node/plugins/Peer"
    common = "/observed/node/plugins/lib/Common"
    replace_image(row, files, exe, image(2, [
        command(0xE, "/usr/lib/dyld"),
        command(0x8000001C, "@loader_path"),
        command(0x8000001C, "@loader_path/../lib"),
        command(0xC, "/observed/node/plugins/Parent"),
        command(0xC, peer),
    ]))
    body = image(6, [command(0xD, peer),
                     command(0x8000001C, "@loader_path/lib"),
                     command(0xC, "@rpath/Common")])
    files[peer] = body
    row["images"].append({"path": peer, "sha256": sha(body),
                          "size": len(body), "mode": 0o755})
    row["images"].sort(key=lambda item: item["path"])
    row["edges"].extend([
        {"source": exe, "index": 4, "command": 12, "name": peer,
         "scope": "runtime", "candidates": [{"path": peer, "resolved": peer}],
         "selected": peer},
        {"source": peer, "index": 2, "command": 12, "name": "@rpath/Common",
         "scope": "runtime", "candidates": [
             {"path": common, "resolved": common},
             {"path": "/observed/node/bin/Common", "resolved": None},
             {"path": "/observed/node/lib/Common", "resolved": None}],
         "selected": common},
    ])
    row["edges"].sort(key=lambda edge: (edge["source"], edge["index"]))
    result = parse(row, files)
    requesters = ("/observed/node/plugins/Parent", peer)
    for source in requesters:
        edge = next(edge for edge in result.manifest.edges if edge.source == source)
        assert len(edge.candidates) == 3
        for missing in (1, 2):
            incomplete = copy.deepcopy(row)
            claim = next(edge for edge in incomplete["edges"] if edge["source"] == source)
            claim["candidates"].pop(missing)
            with pytest.raises(owner.RuntimeInputError,
                               match="command/candidate relation differs"):
                parse(incomplete, files)


def test_shared_rpath_present_ancestor_with_different_original_refuses():
    row, files = inherited_fixture()
    other = "/observed/node/lib/Common"
    body = image(6, [command(0xD, other)])
    files[other] = body
    row["images"].append({"path": other, "sha256": sha(body), "size": len(body), "mode": 0o755})
    row["images"].sort(key=lambda item: item["path"])
    row["edges"][1]["candidates"][2]["resolved"] = other
    with pytest.raises(owner.RuntimeInputError, match="without ambiguity"):
        parse(row, files)


def test_shared_rpath_differing_direct_ancestry_routes_refuse():
    row, files = inherited_fixture()
    exe = row["executable"]
    parent = "/observed/node/plugins/Parent"
    via = "/observed/node/plugins/Via"
    replace_image(row, files, exe, image(2, [command(0xE, "/usr/lib/dyld"),
        command(0x8000001C, "@loader_path"),
        command(0x8000001C, "@loader_path/../lib"),
        command(0xC, parent), command(0xC, via)]))
    body = image(6, [command(0xD, via),
                     command(0x8000001C, "/observed/node/foreign"),
                     command(0xC, parent)])
    files[via] = body
    row["images"].append({"path": via, "sha256": sha(body), "size": len(body), "mode": 0o755})
    row["images"].sort(key=lambda item: item["path"])
    row["edges"].append({"source": exe, "index": 4, "command": 12, "name": via,
                         "scope": "runtime", "candidates": [{"path": via, "resolved": via}],
                         "selected": via})
    row["edges"].append({"source": via, "index": 2, "command": 12, "name": parent,
                         "scope": "runtime", "candidates": [{"path": parent, "resolved": parent}],
                         "selected": parent})
    row["edges"].sort(key=lambda edge: (edge["source"], edge["index"]))
    with pytest.raises(owner.RuntimeInputError, match="differing ancestry"):
        parse(row, files)


def test_shared_rpath_ancestry_state_count_is_admitted_before_growth():
    row, files = inherited_fixture()
    projections = {path: graph.project_node_image(raw, offset=0, size=len(raw), path=path,
                   executable=row["executable"]) for path, raw in files.items()}
    edges = {path: set() for path in files}
    edges[row["executable"]].add("/observed/node/plugins/Parent")
    states = graph.inherited_rpath_states(projections, edges, row["executable"], state_limit=2)
    assert len(states["/observed/node/plugins/Parent"]) == 1
    with pytest.raises(owner.RuntimeInputError, match="ancestry state admission"):
        graph.inherited_rpath_states(projections, edges, row["executable"], state_limit=1)


def test_shared_rpath_cached_install_id_collision_refuses():
    row, files = inherited_fixture()
    common = "/observed/node/plugins/lib/Common"
    parent = "/observed/node/plugins/Parent"
    replace_image(row, files, common, image(6, [command(0xD, parent)]))
    with pytest.raises(owner.RuntimeInputError, match="install ID"):
        parse(row, files)


def test_shared_rpath_same_cached_load_name_cannot_select_two_originals():
    row, files = inherited_fixture()
    exe = row["executable"]
    first = "/observed/node/plugins/Parent"
    second = "/observed/node/other/Parent"
    second_common = "/observed/node/other/lib/Common"
    replace_image(row, files, exe, image(2, [command(0xE, "/usr/lib/dyld"),
        command(0x8000001C, "@loader_path"),
        command(0x8000001C, "@loader_path/../lib"),
        command(0xC, first), command(0xC, second)]))
    files[second] = image(6, [command(0xD, second),
                              command(0x8000001C, "@loader_path/lib"),
                              command(0xC, "@rpath/Common")])
    files[second_common] = image(6, [command(0xD, second_common)])
    row["images"] = [{"path": path, "sha256": sha(raw), "size": len(raw), "mode": 0o755}
                     for path, raw in sorted(files.items())]
    row["edges"].extend([
        {"source": exe, "index": 4, "command": 12, "name": second,
         "scope": "runtime", "candidates": [{"path": second, "resolved": second}],
         "selected": second},
        {"source": second, "index": 2, "command": 12, "name": "@rpath/Common",
         "scope": "runtime", "candidates": [
             {"path": second_common, "resolved": second_common},
             {"path": "/observed/node/bin/Common", "resolved": None},
             {"path": "/observed/node/lib/Common", "resolved": None}],
         "selected": second_common},
    ])
    row["edges"].sort(key=lambda edge: (edge["source"], edge["index"]))
    with pytest.raises(owner.RuntimeInputError, match="cached-name target is ambiguous"):
        parse(row, files)


def test_install_id_cannot_redirect_already_loaded_image_lookup():
    row,files=fixture();lib=row["images"][1]["path"]
    replace_image(row,files,lib,image(6,[command(0xD,row["executable"]),command(0xC,"/usr/lib/libSystem.B.dylib")]))
    with pytest.raises(owner.RuntimeInputError,match="install ID"):
        parse(row,files)


@pytest.mark.parametrize("case",["rpaths","loads","candidate-budget","edges-budget","image-bytes"])
def test_projection_admission_exact_bound_then_refusal(case,monkeypatch):
    row,files=fixture();body=files[row["executable"]]
    kwargs={"offset":0,"size":len(body),"path":row["executable"],"executable":row["executable"]}
    if case=="rpaths":
        kwargs["candidate_limit"]=2
        monkeypatch.setattr(graph,"MAX_RPATHS",2);graph.project_node_image(body,**kwargs)
        monkeypatch.setattr(graph,"MAX_RPATHS",1)
    elif case=="loads":
        kwargs["load_limit"]=1;graph.project_node_image(body,**kwargs);kwargs["load_limit"]=0
    elif case=="candidate-budget":
        kwargs["candidate_limit"]=2;graph.project_node_image(body,**kwargs);kwargs["candidate_limit"]=1
    elif case=="edges-budget":
        kwargs["load_limit"]=1;graph.project_node_image(body,**kwargs);kwargs["load_limit"]=True
    else:
        monkeypatch.setattr(graph,"MAX_IMAGE_BYTES",len(body));graph.project_node_image(body,**kwargs)
        monkeypatch.setattr(graph,"MAX_IMAGE_BYTES",len(body)-1)
    with pytest.raises(owner.RuntimeInputError):graph.project_node_image(body,**kwargs)


def test_aggregate_projection_cannot_allocate_beyond_manifest_budgets(monkeypatch):
    row,files=fixture()
    monkeypatch.setattr(owner,"MAX_EDGES",1)
    # Remove the manifest's second edge so schema passes the same one-edge cap;
    # the actual second image must exhaust original derived-edge admission.
    row["edges"].pop()
    with pytest.raises(owner.RuntimeInputError,match="aggregate load"):
        parse(row,files)


def test_direct_projection_path_is_bounded_before_utf8_allocation():
    import tracemalloc
    oversized = "a" * (1024 * 1024)
    tracemalloc.start()
    try:
        with pytest.raises(owner.RuntimeInputError,match="character bound"):
            graph.text_path(oversized)
        _current,peak=tracemalloc.get_traced_memory()
    finally:
        tracemalloc.stop()
    assert peak < 64 * 1024


@pytest.mark.parametrize("prefix", ["/USR/LIB/review", "/usr/Lib/review", "/system/library/review", "/System/LIBRARY/review", "/usr/lib", "/System/Library"])
def test_resealed_runtime_cannot_alias_normal_os_namespace(prefix):
    row,files=fixture();old="/observed/node"
    relocated=json.loads(json.dumps(row).replace(old,prefix))
    rewritten={}
    for path,body in files.items():
        new_path=path.replace(old,prefix)
        if path==row["executable"]:
            rewritten[new_path]=body
        else:
            rewritten[new_path]=image(6,[command(0xD,new_path),command(0xC,"/usr/lib/libSystem.B.dylib")])
        record=next(item for item in relocated["images"] if item["path"]==new_path)
        record.update(size=len(rewritten[new_path]),sha256=sha(rewritten[new_path]))
    # Keep selected alias coherent so refusal reaches the namespace policy.
    relocated["aliases"][0]["target"]=prefix
    with pytest.raises(owner.RuntimeInputError,match="normal-OS"):
        parse(relocated,rewritten)


@pytest.mark.parametrize("path",["/usr/lib","/System/Library","/USR/LIB","/system/library"])
def test_boundary_directory_itself_cannot_be_an_image(path):
    with pytest.raises(owner.RuntimeInputError,match="normal-OS"):
        graph.normal_os_path(path)
