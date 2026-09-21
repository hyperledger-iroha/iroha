"""Installed byte/live parity; fixtures never become production process authority."""
from __future__ import annotations

from dataclasses import FrozenInstanceError
import hashlib
import json
import os
from pathlib import Path
from urllib.parse import unquote, urlsplit

import pytest

from sorafs_python_dependency_install_test import harness, full_environment, record_bytes
import sorafs_python_dependency_install as dependencies

verifier = dependencies.verifier


def inputs(harness, tmp_path, owner):
    _archives, files, kwargs = full_environment(harness, tmp_path)
    content = next(value for value in kwargs["native_sdk_content"] if value.owner == owner)
    source = Path(unquote(urlsplit(content.source_uri).path))
    original = source.read_bytes()
    parsed = verifier.parse_wheel_bytes(original, owner=owner, extension_suffixes=(".abi3.so",))
    captured = {member.name:files[dependencies.SITE + member.name] for member in content.files}
    return parsed, captured, content, kwargs["environment"], source


def check(parsed, captured, content):
    return verifier.verify_installed_wheel_bytes(parsed, source_uri=content.source_uri,
                                                wheel_sha256=content.wheel_sha256, installed_files=captured)


@pytest.mark.parametrize("owner", (verifier.NATIVE_OWNER, verifier.SDK_OWNER))
def test_same_relation_has_live_and_captured_parity(harness,tmp_path,owner,monkeypatch):
    parsed,captured,content,environment,source=inputs(harness,tmp_path,owner)
    assert check(parsed,captured,content)==content
    assert not any(hasattr(content,name) for name in ("path","seal","qualified","passed"))
    assert all(type(value) is verifier.WheelMember and not hasattr(value,"seal") for value in content.files)
    with pytest.raises(FrozenInstanceError):content.wheel_sha256="1"*64
    preflight=verifier.preflight_wheel(source,verifier.seal_wheel(source).render(),owner=owner,extension_suffixes=(".abi3.so",))
    layout=verifier.derive_installed_layout(environment_root=environment,site_roots={environment/dependencies.SITE.rstrip("/")},wheel=preflight)
    calls=[];original=verifier.verify_installed_wheel_bytes
    def observed(*args,**kwargs):
        calls.append(kwargs["installed_files"])
        return original(*args,**kwargs)
    monkeypatch.setattr(verifier,"verify_installed_wheel_bytes",observed)
    live=verifier.verify_installed_files(preflight,layout)
    assert live.content==content and calls==[captured]
    assert all(type(value) is verifier.StableFile for value in live.files)


@pytest.mark.parametrize("mutation", ["source","native","metadata","record-forgery","missing","extra","installer","requested","direct-url","direct-hash","direct-duplicate","record-missing","record-extra","record-self"])
def test_captured_bytes_reject_resealed_source_metadata_and_origin_substitutions(harness,tmp_path,mutation):
    parsed,captured,content,_environment,_source=inputs(harness,tmp_path,verifier.NATIVE_OWNER)
    assert check(parsed,captured,content)==content
    dist=parsed.dist_info_root;record=dist+"/RECORD"
    if mutation in ("source","record-forgery"):captured[parsed.package_member]=b"substituted source\n"
    elif mutation=="native":captured[parsed.native_member]=b"substituted native\n"
    elif mutation=="metadata":captured[dist+"/METADATA"]+=b"Unauthorized: true\n"
    elif mutation=="missing":del captured[parsed.package_member]
    elif mutation=="extra":captured[parsed.owner.package+"/foreign.py"]=b"extra"
    elif mutation=="installer":captured[dist+"/INSTALLER"]=b"other\n"
    elif mutation=="requested":captured[dist+"/REQUESTED"]=b"not empty"
    elif mutation=="direct-url":
        row=json.loads(captured[dist+"/direct_url.json"]);row["url"]="file:///foreign.whl";captured[dist+"/direct_url.json"]=json.dumps(row).encode()
    elif mutation=="direct-hash":
        row=json.loads(captured[dist+"/direct_url.json"]);row["archive_info"]["hashes"]["sha256"]="1"*64;captured[dist+"/direct_url.json"]=json.dumps(row).encode()
    elif mutation=="direct-duplicate":
        raw=captured[dist+"/direct_url.json"];captured[dist+"/direct_url.json"]=b'{"url":"file:///foreign.whl",'+raw[1:]
    elif mutation=="record-missing":captured[record]=b"\n".join(captured[record].splitlines()[1:])+b"\n"
    elif mutation=="record-extra":captured[record]+=b"foreign.py,,\n"
    elif mutation=="record-self":captured[record]=captured[record].replace((record+",,\n").encode(),(record+",sha256=wrong,0\n").encode())
    if mutation in ("record-forgery","direct-url","direct-hash","direct-duplicate","installer","requested","metadata"):
        captured[record]=record_bytes(harness,{name:raw for name,raw in captured.items() if name!=record},record)
    with pytest.raises(verifier.VerificationError):check(parsed,captured,content)


def test_byte_entry_never_reads_or_imports_and_cannot_be_used_as_live_owner(harness,tmp_path,monkeypatch):
    parsed,captured,content,environment,source=inputs(harness,tmp_path,verifier.SDK_OWNER)
    source.unlink()
    def forbidden(*_args,**_kwargs):raise AssertionError("captured entry attempted live I/O or import")
    monkeypatch.setattr(verifier,"_read_stable_regular_file",forbidden)
    monkeypatch.setattr(Path,"open",forbidden);monkeypatch.setattr(os,"open",forbidden)
    monkeypatch.setattr(verifier.importlib.util,"module_from_spec",forbidden)
    assert check(parsed,captured,content)==content
    with pytest.raises((AttributeError,verifier.VerificationError)):
        verifier.verify_installed_files(content,None)


@pytest.mark.parametrize("source_uri,digest", [("https://host/wheel.whl","1"*64),("file:///a/../b.whl","1"*64),("file://host/wheel.whl","1"*64),("file:///wheel.whl?x","1"*64),("file:///wheel.whl","BAD")])
def test_content_requires_canonical_original_identity_labels(harness,tmp_path,source_uri,digest):
    parsed,captured,_content,_environment,_source=inputs(harness,tmp_path,verifier.SDK_OWNER)
    with pytest.raises(verifier.VerificationError):
        verifier.verify_installed_wheel_bytes(parsed,source_uri=source_uri,wheel_sha256=digest,installed_files=captured)


def test_exact_captured_byte_bounds(harness,tmp_path,monkeypatch):
    parsed,captured,content,_environment,_source=inputs(harness,tmp_path,verifier.SDK_OWNER)
    total=sum(map(len,captured.values()));maximum=max(map(len,captured.values()))
    monkeypatch.setattr(verifier,"MAX_TOTAL_UNCOMPRESSED_BYTES",total)
    monkeypatch.setattr(verifier,"MAX_MEMBER_BYTES",maximum)
    assert check(parsed,captured,content)==content
    monkeypatch.setattr(verifier,"MAX_TOTAL_UNCOMPRESSED_BYTES",total-1)
    with pytest.raises(verifier.VerificationError,match="bounds"):check(parsed,captured,content)


@pytest.mark.parametrize("source_uri", [
    "file:///C:/wheels/iroha%20sdk.whl",
    "file://server/share/wheels/iroha%20sdk.whl",
    "file:///qualified/wheels/iroha%20sdk.whl",
])
def test_captured_uri_labels_preserve_original_host_path_grammar(harness, tmp_path, source_uri):
    parsed, captured, content, _environment, _source = inputs(harness, tmp_path, verifier.SDK_OWNER)
    record = parsed.dist_info_root + "/RECORD"
    direct = parsed.dist_info_root + "/direct_url.json"
    row = json.loads(captured[direct])
    row["url"] = source_uri
    captured[direct] = json.dumps(row).encode()
    captured[record] = record_bytes(harness, {name: raw for name, raw in captured.items() if name != record}, record)
    result = verifier.verify_installed_wheel_bytes(
        parsed, source_uri=source_uri, wheel_sha256=content.wheel_sha256, installed_files=captured)
    assert result.source_uri == source_uri
    assert result.owner == verifier.SDK_OWNER
    assert not hasattr(result, "path") and not hasattr(result, "seal")


def test_live_capture_admits_remaining_bytes_before_reading(harness, tmp_path, monkeypatch):
    _parsed, captured, _content, environment, source = inputs(harness, tmp_path, verifier.SDK_OWNER)
    preflight = verifier.preflight_wheel(
        source, verifier.seal_wheel(source).render(), owner=verifier.SDK_OWNER, extension_suffixes=(".abi3.so",))
    layout = verifier.derive_installed_layout(
        environment_root=environment, site_roots={environment / dependencies.SITE.rstrip("/")}, wheel=preflight)
    limit = sum(map(len, captured.values())) - 1
    original = verifier._read_stable_regular_file
    calls, consumed = [], 0
    def observed(path, **kwargs):
        nonlocal consumed
        assert kwargs["max_bytes"] == min(verifier.MAX_MEMBER_BYTES, limit - consumed)
        calls.append((path, kwargs["max_bytes"]))
        raw, seal = original(path, **kwargs)
        consumed += len(raw)
        return raw, seal
    monkeypatch.setattr(verifier, "MAX_TOTAL_UNCOMPRESSED_BYTES", limit)
    monkeypatch.setattr(verifier, "_read_stable_regular_file", observed)
    with pytest.raises(verifier.VerificationError, match="size bound"):
        verifier.verify_installed_files(preflight, layout)
    assert calls and consumed <= limit


@pytest.mark.parametrize("mutation", ("extra", "duplicate", "aggregate"))
def test_producer_capture_preserves_inventory_and_remaining_bound(harness, tmp_path, monkeypatch, mutation):
    from types import SimpleNamespace
    import sorafs_python_producer_inputs as producer

    _parsed, _captured, _content, environment, source = inputs(harness, tmp_path, verifier.SDK_OWNER)
    wheel = verifier.preflight_wheel(
        source, verifier.seal_wheel(source).render(), owner=verifier.SDK_OWNER, extension_suffixes=(".abi3.so",))
    layout = verifier.derive_installed_layout(
        environment_root=environment, site_roots={environment / dependencies.SITE.rstrip("/")}, wheel=wheel)
    live = verifier.verify_installed_files(wheel, layout)
    rows = tuple(SimpleNamespace(path=str(value.path), seal=value.seal) for value in live.files)
    if mutation == "extra":
        rows += (SimpleNamespace(path=str(layout.site_root / "foreign.py"), seal=live.files[0].seal),)
    elif mutation == "duplicate":
        rows += (rows[0],)
    observation = SimpleNamespace(owner=wheel.owner.package, version=wheel.metadata_version,
                                  path=str(wheel.path), seal=wheel.seal, installed_files=rows)
    limit = sum(value.seal.size for value in live.files) - 1
    if mutation == "aggregate":
        monkeypatch.setattr(verifier, "MAX_TOTAL_UNCOMPRESSED_BYTES", limit)
    with producer.OriginalInputs() as original:
        original_read = original.read
        calls, consumed = [], 0
        def observed(path, maximum, **kwargs):
            nonlocal consumed
            assert mutation == "aggregate", "invalid inventory was read"
            assert maximum == min(verifier.MAX_MEMBER_BYTES, limit - consumed)
            calls.append((path, maximum))
            raw = original_read(path, maximum, **kwargs)
            consumed += len(raw)
            return raw
        monkeypatch.setattr(original, "read", observed)
        with pytest.raises((producer.ArtifactError, producer.child.QualificationError)):
            producer.installed_wheel_join(observation, wheel, original)
        assert bool(calls) == (mutation == "aggregate")
        assert consumed <= limit
