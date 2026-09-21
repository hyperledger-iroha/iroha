"""Dependency effective-byte/runtime-origin controls; synthetic inputs qualify no SDK."""
from __future__ import annotations

import io
from pathlib import Path
import struct
import sys
import zipfile

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_java_consumer_artifact as contract
import build_sorafs_java_consumer_artifact as producer


def class_bytes(name: str, source: str = "Dependency.java", *, major: int = 52) -> bytes:
    """Construct inert classfile parser inputs, never executable evidence."""
    def u2(value): return struct.pack(">H", value)
    def utf8(value):
        raw = value.encode()
        return b"\x01" + u2(len(raw)) + raw
    pool = [utf8(name), b"\x07" + u2(1), utf8("java/lang/Object"), b"\x07" + u2(3), utf8("SourceFile"), utf8(source)]
    return b"\xca\xfe\xba\xbe" + u2(0) + u2(major) + u2(7) + b"".join(pool) + u2(0x21) + u2(2) + u2(4) + u2(0) + u2(0) + u2(0) + u2(1) + u2(5) + struct.pack(">I", 2) + u2(6)


def jar(members: dict[str, bytes]) -> bytes:
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w") as archive:
        for name, raw in members.items():
            archive.writestr(name, raw)
    return output.getvalue()


def runtime_jars(root: Path) -> dict[Path, dict[str, bytes]]:
    return contract.dependency_classpath({str(index): jar({name + ".class": class_bytes(name)}) for index, name in enumerate(contract.DEPENDENCY_RUNTIME_OWNERS)}, root)


def load_log(jars: dict[Path, dict[str, bytes]]) -> bytes:
    return b"".join(f"[0.1s][info][class,load] {name.replace('/', '.')} source: {path.as_uri()}\n".encode() for path, classes in jars.items() for name in classes)


def test_effective_inventory_uses_highest_active_jdk21_version_and_preserves_exact_bytes():
    name = "kotlin/Example"
    bodies = {version: class_bytes(name, f"V{version}.java", major=version + 44 if version else 52) for version in (0, 9, 17, 21, 22)}
    members = {"META-INF/MANIFEST.MF": b"Manifest-Version: 1.0\r\nMulti-Release: tr\r\n ue\r\n\r\n", name + ".class": bodies[0]}
    members.update({f"META-INF/versions/{version}/{name}.class": raw for version, raw in bodies.items() if version})
    assert contract.dependency_classes(jar(members)) == {name: bodies[21]}


@pytest.mark.parametrize("manifest", [b"", b"Manifest-Version: 1.0\nMulti-Release: false\n\n", b"Manifest-Version: 1.0\n\nName: ignored\nMulti-Release: true\n\n"])
def test_inactive_versioned_entries_never_supply_runtime_owners(manifest):
    base, only = "kotlin/Base", "kotlin/OnlyVersioned"
    raw = class_bytes(base)
    members = {base + ".class": raw, f"META-INF/versions/9/{base}.class": class_bytes(base, "Override.java", major=53), f"META-INF/versions/9/{only}.class": class_bytes(only, major=53)}
    if manifest: members["META-INF/MANIFEST.MF"] = manifest
    assert contract.dependency_classes(jar(members)) == {base: raw}


def test_case_insensitive_main_flag_and_version_only_owners_follow_jdk21_lookup():
    name = "org/junit/platform/VersionOnly"
    raw = class_bytes(name, major=61)
    assert contract.dependency_classes(jar({"META-INF/MANIFEST.MF": b"Manifest-Version: 1.0\nMuLtI-ReLeAsE: TRUE\n\n", f"META-INF/versions/17/{name}.class": raw})) == {name: raw}


@pytest.mark.parametrize("manifest", [b"Multi-Release: true", b" Multi-Release: true\n", b"Multi-Release: true\nMULTI-RELEASE: false\n", b"Multi-Release:true\n", b"Multi-Release: tr\x00ue\n"])
def test_ambiguous_or_incomplete_main_manifest_refuses(manifest):
    with pytest.raises(contract.ArtifactError):
        contract.dependency_classes(jar({"META-INF/MANIFEST.MF": manifest}))


@pytest.mark.parametrize("name", ["META-INF/manifest.mf", "meta-inf/MANIFEST.MF"])
def test_case_alias_manifest_cannot_hide_classpath_or_multirelease(name):
    with pytest.raises(contract.ArtifactError, match="manifest name"):
        contract.dependency_classes(jar({name: b"Manifest-Version: 1.0\nClass-Path: foreign.jar\n\n"}))


@pytest.mark.parametrize("name", [contract.CORE_OWNERS[0], "org/hyperledger/iroha/qualification/SorafsJavaConsumerQualificationRunner", "java/lang/Fake", "javax/crypto/Fake", "org/w3c/dom/Fake"])
def test_dependency_cannot_hide_forbidden_owners_even_in_inactive_future_versions(name):
    with pytest.raises(contract.ArtifactError, match="shadows"):
        contract.dependency_classes(jar({f"META-INF/versions/22/{name}.class": class_bytes(name, major=66)}))


@pytest.mark.parametrize("entry", ["wrong.class", "META-INF/versions/08/example/Owner.class", "META-INF/versions/8/example/Owner.class", "META-INF/other/example/Owner.class"])
def test_noncanonical_entry_or_declared_owner_mismatch_refuses(entry):
    with pytest.raises(contract.ArtifactError):
        contract.dependency_classes(jar({entry: class_bytes("example/Owner")}))


@pytest.mark.parametrize("raw", [class_bytes("example/Owner", major=66), class_bytes("example/Owner")[:4] + b"\xff\xff" + class_bytes("example/Owner")[6:]])
def test_effective_class_requiring_other_runtime_or_preview_refuses(raw):
    with pytest.raises(contract.ArtifactError, match="runtime"):
        contract.dependency_classes(jar({"example/Owner.class": raw}))


def test_multi_release_directory_bounds_class_target_and_skips_module_descriptors():
    with pytest.raises(contract.ArtifactError, match="versioned directory"):
        contract.dependency_classes(jar({"META-INF/versions/9/example/Owner.class": class_bytes("example/Owner", major=65)}))
    assert contract.dependency_classes(jar({"module-info.class": class_bytes("module-info", major=53)})) == {}


def test_effective_multi_release_collision_between_dependencies_refuses(tmp_path):
    name = "example/Owner"
    first = jar({name + ".class": class_bytes(name)})
    second = jar({"META-INF/MANIFEST.MF": b"Multi-Release: true\n\n", f"META-INF/versions/9/{name}.class": class_bytes(name, major=53)})
    with pytest.raises(contract.ArtifactError, match="shadow effective"):
        contract.dependency_classpath({"b": second, "a": first}, tmp_path)


def test_runtime_origins_require_engine_launcher_and_derive_exact_effective_bytes(tmp_path):
    jars = runtime_jars(tmp_path)
    rows = contract.validate_dependency_origins(load_log(jars), jars, execution=True)
    assert [row["class"] for row in rows] == sorted(contract.DEPENDENCY_RUNTIME_OWNERS)
    assert rows[0]["sha256"] == contract.identity(next(iter(jars.values()))[rows[0]["class"]])["sha256"]
    for missing in contract.DEPENDENCY_RUNTIME_OWNERS:
        reduced = {path: {name: raw for name, raw in classes.items() if name != missing} for path, classes in jars.items()}
        with pytest.raises(contract.ArtifactError, match="required JUnit"):
            contract.validate_dependency_origins(load_log(reduced), jars, execution=True)


@pytest.mark.parametrize("mutation", ["foreign", "wrong_jar", "duplicate", "unknown", "http", "query", "shared", "jrt", "generated", "lookup"])
def test_runtime_rejects_foreign_ambiguous_or_unknown_dependency_loads(tmp_path, mutation):
    jars = runtime_jars(tmp_path); raw = load_log(jars); first = raw.splitlines(keepends=True)[0]
    if mutation == "foreign": raw = raw.replace(b"dependencies/0.jar", b"foreign/0.jar")
    elif mutation == "wrong_jar": raw = raw.replace(b"dependencies/0.jar", b"dependencies/1.jar")
    elif mutation == "duplicate": raw += first
    elif mutation == "unknown": raw += f"[class,load] foreign.Owner source: {(tmp_path/'dependencies/0.jar').as_uri()}\n".encode()
    elif mutation == "http": raw = raw.replace(b"file:", b"https:")
    elif mutation == "query": raw = raw.replace(b"0.jar", b"0.jar?replace")
    elif mutation in ("shared", "jrt", "lookup"):
        origin = {"shared": b"shared objects file", "jrt": b"jrt:/java.base", "lookup": b"__JVM_LookupDefineClass__"}[mutation]
        raw = raw.replace(first, first[:first.index(b"source: ")+8] + origin + b"\n")
    elif mutation == "generated":
        name = contract.DEPENDENCY_RUNTIME_OWNERS[0].replace('/', '.')
        raw += f"[class,load] {name}$$Lambda/0xabc source: {name}\n".encode()
    with pytest.raises(contract.ArtifactError):
        contract.validate_dependency_origins(raw, jars, execution=True)


def test_each_process_has_independent_origins_and_probe_needs_no_junit_execution(tmp_path):
    jars = runtime_jars(tmp_path); raw = load_log(jars)
    assert contract.validate_dependency_origins(raw, jars, execution=True) == contract.validate_dependency_origins(raw, jars, execution=False)
    jdk = b"[class,load] java.lang.Object source: shared objects file\n[class,load] java.lang.invoke.LambdaForm source: jrt:/java.base\n[class,load] java.lang.invoke.LambdaForm$MH/0xab source: __JVM_LookupDefineClass__\n"
    assert contract.validate_dependency_origins(jdk, jars, execution=False) == []
    with pytest.raises(contract.ArtifactError, match="required JUnit"):
        contract.validate_dependency_origins(jdk, jars, execution=True)


def test_producer_dependency_manifest_checks_effective_owner_collision(tmp_path):
    dependencies = []
    for index, module in enumerate(sorted(producer.DEPENDENCIES)):
        name = "example/Clashing" if index < 2 else f"example/Owner{index}"
        members = {name + ".class": class_bytes(name)} if index != 1 else {"META-INF/MANIFEST.MF": b"Multi-Release: true\n\n", f"META-INF/versions/9/{name}.class": class_bytes(name, major=53)}
        path = tmp_path / f"{index}.jar"; body = jar(members); path.write_bytes(body)
        dependencies.append({"module": module, "version": "1.0", "path": str(path), **contract.identity(body)})
    raw = contract.canonical_json({"schema": "sorafs.java_consumer.dependencies.v1", "jars": dependencies})
    with pytest.raises(contract.ArtifactError, match="shadow effective"):
        producer.load_dependencies(raw, contract.identity(raw)["sha256"])


def lambda_class_bytes(name, *, bootstrap_owner="java/lang/invoke/LambdaMetafactory", bootstrap_method="metafactory", malformed=None):
    """Synthetic constant-pool control, never executable qualification bytes."""
    def u2(v):return struct.pack(">H",v)
    def utf8(v):
        b=v.encode();return b"\x01"+u2(len(b))+b
    pool=[utf8(name),b"\x07"+u2(1),utf8("java/lang/Object"),b"\x07"+u2(3),utf8("BootstrapMethods"),utf8(bootstrap_owner),b"\x07"+u2(6),utf8(bootstrap_method),utf8("()V"),b"\x0c"+u2(8)+u2(9),b"\x0a"+u2(7)+u2(10),b"\x0f\x06"+u2(11)]
    payload=u2(1)+u2(12)+u2(0)
    if malformed=="handle":payload=u2(1)+u2(7)+u2(0)
    if malformed=="truncated":payload=payload[:-1]
    if malformed=="trailing":payload+=b"x"
    if malformed=="argument":payload=u2(1)+u2(12)+u2(1)+u2(99)
    prefix=b"\xca\xfe\xba\xbe"+u2(0)+u2(52)+u2(len(pool)+1)+b"".join(pool)
    prefix+=u2(0x21)+u2(2)+u2(4)+u2(0)+u2(0)+u2(0)
    attr=u2(5)+struct.pack(">I",len(payload))+payload
    return prefix+u2(2 if malformed=="duplicate" else 1)+attr*(2 if malformed=="duplicate" else 1)


@pytest.mark.parametrize("name,origin", [
    ("foreign.Owner", "shared objects file"), ("foreign.Owner", "jrt:/java.base"),
    ("java.lang.Object", "https:/foreign"), ("java.lang.Object", "foreign"),
    ("java.lang.Object", "jrt:/java.base/extra"), ("java.lang.Object", "jrt:/unknown.module"),
    ("java.lang.Object", "__JVM_LookupDefineClass__"), ("java.lang.Object", "__dynamic_proxy__"),
    ("java.lang.Object", "__ClassDefiner__"),
    ("java.lang.invoke.LambdaForm$MH/0xab", "__JVM_LookupDefineClass__"),
    ("jdk.proxy1.$Proxy0", "__dynamic_proxy__"),
    ("jdk.internal.reflect.GeneratedMethodAccessor1", "__ClassDefiner__"),
    ("java.util.function.Function$$Lambda/0xab", "java.util.function.Function"),
])
def test_unowned_application_and_unobserved_jdk_generated_origins_reject(tmp_path, name, origin):
    jars = runtime_jars(tmp_path)
    raw = load_log(jars) + f"[class,load] {name} source: {origin}\n".encode()
    with pytest.raises(contract.ArtifactError):
        contract.validate_dependency_origins(raw, jars, execution=True)


def test_supported_jdk_generated_owners_require_their_previously_observed_jdk_bootstrap(tmp_path):
    jars = runtime_jars(tmp_path)
    lines = [
        ("java.lang.invoke.LambdaForm", "shared objects file"),
        ("java.lang.reflect.Proxy", "jrt:/java.base"),
        ("java.lang.reflect.Proxy$ProxyBuilder", "jrt:/java.base"),
        ("jdk.internal.reflect.MethodAccessorGenerator", "jrt:/java.base"),
        ("java.lang.invoke.LambdaForm$MH/0xab", "__JVM_LookupDefineClass__"),
        ("jdk.proxy1.$Proxy0", "__dynamic_proxy__"),
        ("jdk.internal.reflect.GeneratedMethodAccessor1", "__ClassDefiner__"),
        ("java.lang.reflect.Proxy$ProxyBuilder$$Lambda/0xab", "java.lang.reflect.Proxy"),
    ]
    raw = load_log(jars) + b"".join(f"[class,load] {name} source: {origin}\n".encode() for name, origin in lines)
    assert len(contract.validate_dependency_origins(raw, jars, execution=True)) == 2


@pytest.mark.parametrize("version", ["99999999999", "2147483648", "9" * 100])
def test_pathological_multirelease_version_is_bounded_before_decimal_conversion(version):
    with pytest.raises(contract.ArtifactError, match="noncanonical"):
        contract.dependency_classes(jar({f"META-INF/versions/{version}/example/Owner.class": class_bytes("example/Owner")}))


def test_runtime_dependency_lambda_retains_actual_preloaded_effective_bootstrap_bytes(tmp_path):
    jars = runtime_jars(tmp_path)
    owner = contract.DEPENDENCY_RUNTIME_OWNERS[0]
    source = lambda_class_bytes(owner)
    first = next(iter(jars)); jars[first][owner] = source
    name = owner.replace("/", ".")
    generated = f"[class,load] {name}$$Lambda/0xab source: {name}\n".encode()
    rows = contract.validate_dependency_origins(load_log(jars) + generated, jars, execution=True)
    row = next(row for row in rows if row.get("runtime_generated"))
    assert row["enclosing_bytes"] == contract.identity(source)
    assert row["enclosing_class"] == owner and "sha256" not in row
    for raw in (generated + load_log(jars), load_log(jars) + generated + generated):
        with pytest.raises(contract.ArtifactError):
            contract.validate_dependency_origins(raw, jars, execution=True)
