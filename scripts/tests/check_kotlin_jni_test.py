"""Fail-closed coverage for compiled Kotlin ownership and exact JNI linkage."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "check_kotlin_jni.py"
SPEC = importlib.util.spec_from_file_location("check_kotlin_jni", SCRIPT)
assert SPEC and SPEC.loader
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)
JVM = GUARD.JVM
SDK = GUARD.SDK_PACKAGE
PRIVACY = SDK + "privacy/PrivacyNativeBridge"
SIGNER = SDK + "crypto/NativeSignerBridge"
CODEC = SDK + "tx/norito/NoritoJavaCodecAdapter"


def class_bytes(name, methods=(), *, major=52, source="Fixture.kt", extra_utf8=()):
    """Build small declaration fixtures; javac supplies the independent oracle below."""
    pool = []

    def utf8(value):
        raw = value.encode("utf-8", "surrogatepass").replace(b"\0", b"\xc0\x80")
        pool.append(b"\x01" + struct.pack(">H", len(raw)) + raw)
        return len(pool)

    def class_entry(value):
        index = utf8(value)
        pool.append(b"\x07" + struct.pack(">H", index))
        return len(pool)

    owner, parent = class_entry(name), class_entry("java/lang/Object")
    rendered = []
    for method in methods:
        rendered.append(struct.pack(">HHHH", method.flags, utf8(method.name), utf8(method.descriptor), 0))
    attributes = []
    if source is not None:
        attributes.append(struct.pack(">HIH", utf8("SourceFile"), 2, utf8(source)))
    for value in extra_utf8:
        utf8(value)
    return (
        struct.pack(">IHHH", 0xCAFEBABE, 0, major, len(pool) + 1) + b"".join(pool)
        + struct.pack(">HHHHHH", 0x0421, owner, parent, 0, 0, len(rendered))
        + b"".join(rendered) + struct.pack(">H", len(attributes)) + b"".join(attributes)
    )


def native(name, descriptor):
    return JVM.Method(name, descriptor, JVM.ACC_NATIVE | JVM.ACC_STATIC | 2)


def release_methods():
    return {
        CODEC: (JVM.Method("<init>", "(I)V", JVM.ACC_PUBLIC),),
        PRIVACY: (
            native("nativeValidateCompiledProfileCatalog", "([B)I"),
            native("nativeExact12FixtureBundle", "()[B"),
            native("nativeValidateExact12FixtureBundle", "([B)I"),
            native("nativeValidateExact12CapabilityManifestForNetworkV1", "([B[B)I"),
            native("nativeRequireExact12CapabilityTupleForNetworkV1", "([BI[B)Z"),
            native("nativeValidateExact12SubmitProofConstructionForNetworkV1", "([BI[B[B)Z"),
        ),
        SIGNER: (
            JVM.Method("encodeRegisterZkAssetSignedTransaction",
                       "(IL" + SDK + "core/model/NetworkId;I)V", JVM.ACC_PUBLIC | JVM.ACC_STATIC),
            native("nativeEncodeRegisterZkAssetSignedTransaction", "(I[BI)V"),
            native("nativeValidateAccountAddressCanonical", "([B)[B"),
        ),
    }


def build_outputs(tmp_path):
    roots = {module: [tmp_path / module] for module in GUARD.MODULES}
    for module, paths in roots.items():
        owner = SDK + "fixture/" + module.replace("-", "_")
        path = paths[0] / (owner + ".class")
        path.parent.mkdir(parents=True)
        path.write_bytes(class_bytes(owner))
    for owner, methods in release_methods().items():
        path = roots["core-jvm"][0] / (owner + ".class")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(class_bytes(owner, methods))
    return roots


def test_parser_reads_javac_headers_without_loading_classes(tmp_path):
    javac = shutil.which("javac")
    if javac is None:
        pytest.skip("JDK compiler is required for the independent classfile/header oracle")
    source = tmp_path / "NativeOracle.java"
    source.write_text('''package oracle;
public class NativeOracle {
  public static final String UNUSED = "\\u0000\\ud83d\\ude80";
  public native long overloaded(int value);
  public native long overloaded(byte[][] value);
  public static native void under_score(String value);
  public void under_score(int value) {}
  public static class Nested { public native byte[] call(); }
}
''', encoding="utf-8")
    subprocess.run([javac, "--release", "8", "-h", str(tmp_path), "-d", str(tmp_path), str(source)],
                   check=True, capture_output=True, timeout=60)
    outer = JVM.parse_class((tmp_path / "oracle/NativeOracle.class").read_bytes())
    nested = JVM.parse_class((tmp_path / "oracle/NativeOracle$Nested.class").read_bytes())
    assert outer.major == nested.major == 52
    assert outer.source_file == "NativeOracle.java"
    assert not [method for method in outer.methods if method.name == "<init>"][0].native
    operations = GUARD.native_operations(outer) + GUARD.native_operations(nested)
    headers = "\n".join(path.read_text() for path in tmp_path.glob("*.h"))
    assert len(operations) == 4
    for operation in operations:
        assert operation["symbol"] + "\n" in headers
    assert {operation["symbol"] for operation in operations} == {
        "Java_oracle_NativeOracle_overloaded__I",
        "Java_oracle_NativeOracle_overloaded___3_3B",
        "Java_oracle_NativeOracle_under_1score",
        "Java_oracle_NativeOracle_00024Nested_call",
    }
    assert [operation["static"] for operation in operations] == [False, False, True, False]


def test_parser_rejects_every_truncation_and_trailing_data():
    raw = class_bytes("sdk/Fixture", [native("call", "([B)J")])
    assert JVM.parse_class(raw).methods[0].descriptor == "([B)J"
    for end in range(len(raw)):
        with pytest.raises(JVM.ClassFileError):
            JVM.parse_class(raw[:end])
    with pytest.raises(JVM.ClassFileError, match="trailing"):
        JVM.parse_class(raw + b"x")
    with pytest.raises(JVM.ClassFileError, match="byte limit"):
        JVM.parse_class(b"x" * (JVM.MAX_CLASS_BYTES + 1))


def test_parser_rejects_ambiguous_native_declarations():
    method = native("call", "()V")
    with pytest.raises(JVM.ClassFileError, match="duplicate method"):
        JVM.parse_class(class_bytes("sdk/F", [method, method]))
    with pytest.raises(JVM.ClassFileError, match="incompatible"):
        JVM.parse_class(class_bytes("sdk/F", [native("<init>", "()V")]))
    with pytest.raises(JVM.ClassFileError, match="incompatible"):
        JVM.parse_class(class_bytes("sdk/F", [JVM.Method("call", "()V", JVM.ACC_NATIVE | JVM.ACC_ABSTRACT)]))
    malformed = bytearray(class_bytes("sdk/F"))
    malformed[10] = 99
    with pytest.raises(JVM.ClassFileError, match="constant-pool tag"):
        JVM.parse_class(bytes(malformed))


@pytest.mark.parametrize("descriptor", ["", "I", "(V)V", "([V)V", "([)V", "()", "()VV", "(L;)V", "(Lx)V", "(Lx//y;)V", "(" + "I" * 256 + ")V", "(" + "[" * 256 + "B)V"])
def test_invalid_method_descriptors_fail(descriptor):
    with pytest.raises(JVM.ClassFileError):
        JVM.method_descriptor(descriptor)


def test_modified_utf8_and_jni_mangling_handle_utf16():
    declaration = JVM.parse_class(class_bytes("sdk/\ud83d\ude80", extra_utf8=("unused\0", "\ud800")))
    assert declaration.name == "sdk/🚀"
    assert GUARD.jni_escape(declaration.name + "$_[;") == "sdk__0d83d_0de80_00024_1_3_2"


@pytest.mark.parametrize("precursor", ["0method", "1class", "2method", "3method", "sdk/0Native", "[Lsdk/2Native;"])
def test_jni_escape_rejects_unresolvable_precursor_names(precursor):
    with pytest.raises(GUARD.AuditError, match="cannot be resolved"):
        GUARD.jni_escape(precursor)
    assert GUARD.jni_escape("_1method") == "_11method"


def test_scan_and_exact_exports_cover_all_modules(tmp_path):
    roots = build_outputs(tmp_path)
    classes, records = GUARD.scan_classes(roots)
    assert len(classes) == len(records) == 6
    assert {record["module"] for record in records} == set(GUARD.MODULES)
    operations = [operation for record in records for operation in record["operations"]]
    exports = [operation["symbol"] for operation in operations]
    GUARD.validate_exports(operations, exports + ["connect_norito_free"])
    with pytest.raises(GUARD.AuditError, match="missing JNI exports"):
        GUARD.validate_exports(operations, exports[1:])
    for unowned in ("Java_org_hyperledger_iroha_android_F_call", "Java_pg_product_F_call", exports[0] + "Stale"):
        with pytest.raises(GUARD.AuditError, match="unowned JNI exports"):
            GUARD.validate_exports(operations, exports + [unowned])
    with pytest.raises(GUARD.AuditError, match="duplicate JNI"):
        GUARD.validate_exports(operations, exports + exports[:1])
    with pytest.raises(GUARD.AuditError, match="colliding"):
        GUARD.validate_exports(operations + operations[:1], exports)


@pytest.mark.parametrize("mutation", ["missing_module", "empty_output", "duplicate_class", "misplaced", "jdk9", "java_native", "wrong_owner", "no_source", "symlink_directory", "fifo"])
def test_scan_rejects_incomplete_or_noncanonical_classes(tmp_path, mutation):
    roots = build_outputs(tmp_path)
    privacy = roots["core-jvm"][0] / (PRIVACY + ".class")
    if mutation == "missing_module":
        del roots["client-android"]
    elif mutation == "empty_output":
        for path in roots["client-android"][0].rglob("*.class"):
            path.unlink()
    elif mutation == "duplicate_class":
        copy = roots["client-android"][0] / (PRIVACY + ".class")
        copy.parent.mkdir(parents=True)
        copy.write_bytes(privacy.read_bytes())
    elif mutation == "misplaced":
        privacy.rename(privacy.with_name("Wrong.class"))
    elif mutation == "jdk9":
        privacy.write_bytes(class_bytes(PRIVACY, release_methods()[PRIVACY], major=53))
    elif mutation == "java_native":
        privacy.write_bytes(class_bytes(PRIVACY, release_methods()[PRIVACY], source="Fixture.java"))
    elif mutation == "no_source":
        privacy.write_bytes(class_bytes(PRIVACY, release_methods()[PRIVACY], source=None))
    elif mutation == "symlink_directory":
        retired = tmp_path / "hidden"
        retired.mkdir()
        (retired / "UnshieldInstruction.class").write_bytes(
            class_bytes(SDK + "core/model/instructions/UnshieldInstruction"))
        link = roots["core-jvm"][0] / (SDK + "core/model/instructions")
        link.parent.mkdir(parents=True)
        link.symlink_to(retired, target_is_directory=True)
    elif mutation == "fifo":
        if not hasattr(os, "mkfifo"):
            pytest.skip("FIFO inputs are Unix-only")
        privacy.unlink()
        os.mkfifo(privacy)
    else:
        owner = "org/hyperledger/iroha/android/Bridge"
        path = roots["core-jvm"][0] / (owner + ".class")
        path.parent.mkdir(parents=True)
        path.write_bytes(class_bytes(owner, [native("call", "()V")]))
    with pytest.raises(GUARD.AuditError):
        GUARD.scan_classes(roots)


@pytest.mark.parametrize("mutation", ["missing_privacy", "managed_validator", "validator_signature", "generic_proof", "retired_signer", "retired_instruction", "implicit_public_network", "implicit_native_network", "implicit_companion_network", "public_native_bytes", "default_codec_context"])
def test_release_api_constraints_replace_reflection(mutation):
    declarations = release_methods()
    if mutation == "missing_privacy":
        del declarations[PRIVACY]
    elif mutation == "default_codec_context":
        declarations[CODEC] += (JVM.Method("<init>", "()V", JVM.ACC_PUBLIC),)
    elif mutation == "managed_validator":
        first, *rest = declarations[PRIVACY]
        declarations[PRIVACY] = (JVM.Method(first.name, first.descriptor, JVM.ACC_STATIC), *rest)
    elif mutation == "validator_signature":
        first, *rest = declarations[PRIVACY]
        declarations[PRIVACY] = (native(first.name, "([B)J"), *rest)
    elif mutation == "generic_proof":
        declarations[PRIVACY + "$Companion"] = (JVM.Method("buildProof", "()[B", JVM.ACC_PUBLIC),)
    elif mutation == "retired_signer":
        declarations[SIGNER] += (native("nativeEncodeShieldSignedTransaction", "()[B"),)
    elif mutation == "retired_instruction":
        declarations[SDK + "core/model/instructions/UnshieldInstruction"] = ()
    elif mutation == "implicit_companion_network":
        declarations[SIGNER + "$Companion"] = (
            JVM.Method("encodeRegisterZkAssetSignedTransaction", "(ILjava/lang/String;I)V", JVM.ACC_PUBLIC),
        )
    else:
        public, private, account = declarations[SIGNER]
        if mutation == "public_native_bytes":
            public = JVM.Method(public.name, "(I[BI)V", public.flags | JVM.ACC_NATIVE)
        elif mutation == "implicit_public_network":
            public = JVM.Method(public.name, "(ILjava/lang/String;I)V", public.flags)
        else:
            private = native(private.name, "(II[B)V")
        declarations[SIGNER] = (public, private, account)
    classes = {owner: JVM.parse_class(class_bytes(owner, methods)) for owner, methods in declarations.items()}
    with pytest.raises(GUARD.AuditError):
        GUARD.validate_release_api(classes)


def test_audit_seals_inputs_without_claiming_runtime_qualification(tmp_path, monkeypatch):
    roots = build_outputs(tmp_path)
    _, records = GUARD.scan_classes(roots)
    exports = [operation["symbol"] for record in records for operation in record["operations"]]
    library = tmp_path / "libbridge.so"
    library.write_bytes(b"test export inventory")
    monkeypatch.setattr(GUARD.ARTIFACT, "inspect_exported_symbols", lambda *args, **kwargs: tuple(exports))
    report = GUARD.audit(roots, library)
    assert report["valid"] and report["native_method_count"] == 8
    assert not report["native_execution_qualified"]
    assert not report["native_signatures_qualified"]
    assert not report["source_build_provenance_qualified"]
    assert len(report["class_inventory_sha256"]) == len(report["library"]["sha256"]) == 64

    def change_class(*args, **kwargs):
        path = roots["core-jvm"][0] / (PRIVACY + ".class")
        path.write_bytes(class_bytes(PRIVACY, release_methods()[PRIVACY], source="Changed.kt"))
        return tuple(exports)

    monkeypatch.setattr(GUARD.ARTIFACT, "inspect_exported_symbols", change_class)
    with pytest.raises(GUARD.AuditError, match="classes changed"):
        GUARD.audit(roots, library)


@pytest.mark.parametrize("mutation", ("missing", "return_type", "instance", "managed", "curve_switch", "bypass", "retired_config"))
def test_complete_account_native_admission_is_required(mutation):
    declarations = release_methods()
    public, private, account = declarations[SIGNER]
    if mutation == "missing":
        declarations[SIGNER] = (public, private)
    elif mutation in {"return_type", "instance", "managed"}:
        descriptor = "([B)Z" if mutation == "return_type" else account.descriptor
        flags = account.flags
        if mutation == "instance":
            flags &= ~JVM.ACC_STATIC
        if mutation == "managed":
            flags &= ~JVM.ACC_NATIVE
        declarations[SIGNER] = (public, private, JVM.Method(account.name, descriptor, flags))
    elif mutation == "retired_config":
        declarations[SDK + "address/CurveSupportConfig"] = ()
    else:
        name = "configureCurveSupport" if mutation == "curve_switch" else "parseEncodedIgnoringCurveSupport"
        declarations[SDK + "address/AccountAddress$Companion"] = (
            JVM.Method(name, "()V", JVM.ACC_PUBLIC),
        )
    classes = {owner: JVM.parse_class(class_bytes(owner, methods)) for owner, methods in declarations.items()}
    with pytest.raises(GUARD.AuditError, match="account"):
        GUARD.validate_release_api(classes)


def test_cli_does_not_overwrite_build_inputs(tmp_path):
    roots = build_outputs(tmp_path)
    library = tmp_path / "libbridge.so"
    library.write_bytes(b"unchanged")
    arguments = ["--library", str(library), "--report", str(library)]
    for module, paths in roots.items():
        arguments += ["--classes", module + "=" + str(paths[0])]
    assert GUARD.main(arguments) == 1
    assert library.read_bytes() == b"unchanged"


def test_report_rejects_input_aliases_and_replaces_its_own_inode(tmp_path):
    library, class_path = tmp_path / "library", tmp_path / "Class.class"
    library.write_bytes(b"native bytes")
    class_path.write_bytes(b"class bytes")
    result = {"library": {"path": str(library)}, "classes": [{"path": str(class_path)}]}
    report = tmp_path / "report.json"
    report.hardlink_to(class_path)
    with pytest.raises(GUARD.AuditError, match="alias"):
        GUARD.write_report(report, result)
    assert class_path.read_bytes() == b"class bytes"
    report.unlink()
    report.symlink_to(library)
    with pytest.raises(GUARD.AuditError, match="symlink"):
        GUARD.write_report(report, result)
    report.unlink()
    report.write_text("old report")
    old_report = tmp_path / "old-report"
    old_report.hardlink_to(report)
    GUARD.write_report(report, result)
    assert json.loads(report.read_text()) == result
    assert old_report.read_text() == "old report"
    assert not list(tmp_path.glob(".report.json.*"))


@pytest.mark.parametrize("index", range(6))
@pytest.mark.parametrize("mutation", ("missing", "return", "managed", "instance"))
def test_privacy_class_contract_rejects_every_native_declaration_drift(index, mutation):
    methods = list(release_methods()[PRIVACY])
    current = methods[index]
    if mutation == "missing":
        del methods[index]
    else:
        descriptor = current.descriptor[:-1] + "J" if mutation == "return" else current.descriptor
        flags = current.flags & ~JVM.ACC_NATIVE if mutation == "managed" else current.flags & ~JVM.ACC_STATIC if mutation == "instance" else current.flags
        methods[index] = JVM.Method(current.name, descriptor, flags)
    with pytest.raises(GUARD.AuditError, match="native declaration"):
        GUARD.validate_privacy_api({PRIVACY: JVM.parse_class(class_bytes(PRIVACY, methods))})


@pytest.mark.parametrize("name", ("nativeValidateExact12CapabilityManifest", "nativeRequireExact12CapabilityTuple", "nativeValidateExact12SubmitProofConstruction", "buildProof", "verifyProof", "nativeBuildProof", "nativeVerifyProof", "PrivacyProofRequest"))
def test_privacy_class_contract_rejects_every_retired_method(name):
    classes = {PRIVACY: JVM.parse_class(class_bytes(PRIVACY, release_methods()[PRIVACY]))}
    classes[PRIVACY + "$Companion"] = JVM.parse_class(class_bytes(PRIVACY + "$Companion", [native(name, "()[B")]))
    with pytest.raises(GUARD.AuditError):
        GUARD.validate_privacy_api(classes)


def test_scoped_privacy_classfile_contract_seals_actual_bytes(tmp_path):
    for suffix in ("", "$Companion"):
        name = PRIVACY + suffix
        path = tmp_path / (name + ".class")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(class_bytes(name, release_methods()[PRIVACY] if not suffix else (), source="PrivacyNativeBridge.kt"))
    report = GUARD.audit_privacy_classfiles(tmp_path)
    assert len(report["classes"]) == 2
    assert report["native_executed"] is False and report["release_qualified"] is False
    (tmp_path / (PRIVACY + "$Companion.class")).unlink()
    with pytest.raises(GUARD.AuditError):
        GUARD.audit_privacy_classfiles(tmp_path)


@pytest.mark.parametrize("owner,source", (
    (SDK + "privacy/PrivacyConfidentialWitnessV1", "PrivacyConfidentialWitness.kt"),
    (SDK + "privacy/PrivacyConfidentialNoteWitnessV1", "PrivacyConfidentialWitness.kt"),
    (SDK + "privacy/NoteWitnessAdapter", "PrivacyConfidentialWitness.kt"),
    (SDK + "privacy/fixtures/RetiredPrivacyConfidentialWitnessCodecs", "RetiredPrivacyConfidentialWitnessFixture.kt"),
    (SDK + "privacy/Relocated", "RetiredPrivacyConfidentialWitnessFixture.kt"),
    ("org/hyperledger/iroha/android/privacy/PrivacyConfidentialWitness", "PrivacyConfidentialWitness.java"),
))
def test_compiled_privacy_contract_rejects_retired_witnesses_and_test_fixtures(tmp_path, owner, source):
    for name, methods, file in (
        (PRIVACY, release_methods()[PRIVACY], "PrivacyNativeBridge.kt"),
        (PRIVACY + "$Companion", (), "PrivacyNativeBridge.kt"),
        (owner, (), source),
    ):
        path = tmp_path / (name + ".class")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(class_bytes(name, methods, source=file))
    with pytest.raises(GUARD.AuditError, match="retired confidential witness or test fixture"):
        GUARD.audit_privacy_classfiles(tmp_path)
    classes = {owner: JVM.parse_class(class_bytes(owner, (), source=source))}
    with pytest.raises(GUARD.AuditError, match="retired confidential witness or test fixture"):
        GUARD.validate_retired_privacy_witness_classes(classes)


def test_compiled_privacy_contract_seals_nonprivacy_main_classes_too(tmp_path):
    for suffix in ("", "$Companion"):
        name = PRIVACY + suffix
        path = tmp_path / (name + ".class")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(class_bytes(name, release_methods()[PRIVACY] if not suffix else (), source="PrivacyNativeBridge.kt"))
    owner = SDK + "privacy/PrivacyProtocolIdV1"
    path = tmp_path / (owner + ".class")
    path.write_bytes(class_bytes(owner, (), source="PrivacyNativeBridge.kt"))
    assert len(GUARD.audit_privacy_classfiles(tmp_path)["classes"]) == 3
    link = path.parent / "hidden"
    link.symlink_to(tmp_path / "absent")
    with pytest.raises(GUARD.AuditError, match="symlink"):
        GUARD.audit_privacy_classfiles(tmp_path)
