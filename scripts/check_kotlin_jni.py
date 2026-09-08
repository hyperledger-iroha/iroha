#!/usr/bin/env python3
"""Check compiled Kotlin declarations against the native bridge's export table.

Requires Python 3.10+, compiled main class directories for all three SDK modules,
and nm/llvm-nm (or Windows export tooling) for a freshly built bridge. No JVM
classes or native code are loaded. No environment variables are required.

This checks declaration ownership, JDK 8 bytecode, release API constraints, and
exact JNI export ownership. Export names alone cannot attest native argument
types or receiver semantics. The report seals the inspected class files and
library; it does not replace signature validation, source-bound build provenance,
native execution, or hardware qualification. C exports are checked by
check_native_sdk_abi23_artifact.py.
"""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
from typing import Sequence


def _load_sibling(name: str):
    specification = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load {name}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[name] = module
    specification.loader.exec_module(module)
    return module


JVM = _load_sibling("jvm_classfile")
ARTIFACT = _load_sibling("check_native_sdk_abi23_artifact")
MODULES = ("core-jvm", "client-android", "kagemusha-wallet-android")
SDK_PACKAGE = "org/hyperledger/iroha/sdk/"
MAX_CLASS_FILES = 20_000


class AuditError(ValueError):
    """The compiled SDK and the library do not form the canonical JNI boundary."""


def jni_escape(value: str) -> str:
    """Mangle a JVM internal name using UTF-16 code units, including nested types."""
    escaped = []
    for character in value:
        # JNI Design, "Resolving Native Method Names": digit 0..3 at
        # the start or after a slash escape makes native lookup fail.
        if character in "0123" and (not escaped or escaped[-1].endswith("_")):
            raise AuditError(f"JNI name cannot be resolved: {value!r}")
        if character == "/":
            escaped.append("_")
        elif character in {"_", ";", "["}:
            escaped.append({"_": "_1", ";": "_2", "[": "_3"}[character])
        elif character.isascii() and character.isalnum():
            escaped.append(character)
        else:
            encoded = character.encode("utf-16-be")
            escaped.extend("_0" + encoded[i:i + 2].hex() for i in range(0, len(encoded), 2))
    return "".join(escaped)


def native_operations(class_file) -> tuple[dict[str, object], ...]:
    """Derive the exact JNI symbol for every native method in one class."""
    natives = [method for method in class_file.methods if method.native]
    names = Counter(method.name for method in natives)
    operations = []
    for method in natives:
        symbol = "Java_" + jni_escape(class_file.name) + "_" + jni_escape(method.name)
        if names[method.name] > 1:
            parameters, _ = JVM.method_descriptor(method.descriptor)
            symbol += "__" + jni_escape("".join(parameters))
        operations.append({
            "class": class_file.name,
            "method": method.name,
            "descriptor": method.descriptor,
            "static": method.static,
            "symbol": symbol,
        })
    return tuple(operations)


def validate_release_api(classes: dict[str, object]) -> None:
    """Retain fail-closed privacy and explicit signing context API assertions."""
    privacy_name = SDK_PACKAGE + "privacy/PrivacyNativeBridge"
    signer_name = SDK_PACKAGE + "crypto/NativeSignerBridge"
    codec_name = SDK_PACKAGE + "tx/norito/NoritoJavaCodecAdapter"
    for required in (privacy_name, signer_name, codec_name):
        if required not in classes:
            raise AuditError(f"required SDK class is missing: {required}")
    if any(method.name == "<init>" and method.descriptor == "()V"
           and method.flags & JVM.ACC_PUBLIC for method in classes[codec_name].methods):
        raise AuditError("codec adapter construction requires explicit chain context")
    privacy = classes[privacy_name]
    for name, descriptor in (
        ("nativeValidateCompiledProfileCatalog", "([B)I"),
        ("nativeExact12FixtureBundle", "()[B"),
        ("nativeValidateExact12FixtureBundle", "([B)I"),
    ):
        matches = [method for method in privacy.methods if method.name == name]
        if len(matches) != 1 or matches[0].descriptor != descriptor or not matches[0].native or not matches[0].static:
            raise AuditError(f"privacy operation must retain its native declaration: {name}{descriptor}")
    for owner, class_file in classes.items():
        if owner in (privacy_name, privacy_name + "$Companion"):
            for method in class_file.methods:
                if any(fragment in method.name for fragment in ("ProofRequest", "BuildProof", "VerifyProof")) or method.name in ("buildProof", "verifyProof"):
                    raise AuditError(f"unqualified generic privacy proof method: {owner}.{method.name}")
        if owner in (signer_name, signer_name + "$Companion"):
            for method in class_file.methods:
                if method.name in {
                    prefix + operation + "SignedTransaction"
                    for prefix in ("encode", "nativeEncode")
                    for operation in ("Shield", "ZkTransfer", "Unshield")
                }:
                    raise AuditError(f"retired transaction signer method: {owner}.{method.name}")
    for retired in ("ShieldInstruction", "ZkTransferInstruction", "UnshieldInstruction"):
        if SDK_PACKAGE + "core/model/instructions/" + retired in classes:
            raise AuditError(f"retired instruction class: {retired}")
    signer = classes[signer_name]
    account_validation = [method for method in signer.methods
                          if method.name == "nativeValidateAccountAddressCanonical"]
    if (len(account_validation) != 1
            or account_validation[0].descriptor != "([B)[B"
            or not account_validation[0].native
            or not account_validation[0].static):
        raise AuditError("complete account admission requires the canonical static native byte-array boundary")
    for owner, class_file in classes.items():
        if owner in (SDK_PACKAGE + "address/AccountAddress", SDK_PACKAGE + "address/AccountAddress$Companion"):
            if any(method.name == "configureCurveSupport" or method.name.endswith("IgnoringCurveSupport")
                   for method in class_file.methods):
                raise AuditError("account identity must use the fixed V1 algorithm catalog")
    if SDK_PACKAGE + "address/CurveSupportConfig" in classes:
        raise AuditError("retired global account curve configuration is present")
    public = [method for owner, class_file in classes.items()
              if owner in (signer_name, signer_name + "$Companion")
              for method in class_file.methods
              if method.name == "encodeRegisterZkAssetSignedTransaction" and method.flags & JVM.ACC_PUBLIC]
    native = [method for method in signer.methods
              if method.name == "nativeEncodeRegisterZkAssetSignedTransaction" and method.native]
    if not public or len(native) != 1:
        raise AuditError("canonical register-asset signer declarations are missing")
    for method, network_type in [
        *((method, "L" + SDK_PACKAGE + "core/model/NetworkId;") for method in public),
        *((method, "[B") for method in native),
    ]:
        parameters, _ = JVM.method_descriptor(method.descriptor)
        if len(parameters) < 3 or parameters[1:3] != (network_type, "I"):
            raise AuditError("register-asset signing requires explicit network identity and chain discriminant")


def scan_classes(roots: dict[str, Sequence[Path]]) -> tuple[dict[str, object], list[dict[str, object]]]:
    """Scan every module's supplied main output, rejecting omitted or duplicate inputs."""
    if set(roots) != set(MODULES) or any(not roots[module] for module in MODULES):
        raise AuditError("compiled main classes are required for all three Kotlin SDK modules")
    classes = {}
    records = []
    seen_roots = set()
    for module in MODULES:
        for root in roots[module]:
            root = root.resolve(strict=True)
            if root in seen_roots or not root.is_dir():
                raise AuditError(f"invalid or repeated class directory: {root}")
            seen_roots.add(root)
            paths = []
            for entry in root.rglob("*"):
                # pathlib does not descend through directory symlinks. Reject
                # them explicitly so they cannot hide retired declarations.
                if entry.is_symlink():
                    raise AuditError(f"class output contains a symlink: {entry}")
                if entry.suffix == ".class":
                    if not entry.is_file():
                        raise AuditError(f"class input is not a regular file: {entry}")
                    paths.append(entry)
                    if len(records) + len(paths) > MAX_CLASS_FILES:
                        raise AuditError("compiled class inventory exceeds the entry limit")
            paths.sort()
            if not paths:
                raise AuditError(f"class directory is empty: {root}")
            for path in paths:
                if path.is_symlink() or not path.resolve(strict=True).is_relative_to(root):
                    raise AuditError(f"class file escapes its build output: {path}")
                with path.open("rb") as stream:
                    raw = stream.read(JVM.MAX_CLASS_BYTES + 1)
                declaration = JVM.parse_class(raw)
                if path.relative_to(root).as_posix() != declaration.name + ".class":
                    raise AuditError(f"class path and declared owner differ: {path}")
                if declaration.name in classes:
                    raise AuditError(f"duplicate compiled class: {declaration.name}")
                if declaration.major != 52:
                    raise AuditError(f"SDK bytecode must target JDK 8: {declaration.name} has major {declaration.major}")
                natives = native_operations(declaration)
                if natives and (not declaration.name.startswith(SDK_PACKAGE)
                                or not declaration.source_file
                                or not declaration.source_file.endswith(".kt")):
                    raise AuditError(f"native declarations must belong to the Kotlin SDK: {declaration.name}")
                classes[declaration.name] = declaration
                records.append({"module": module, "class": declaration.name,
                                "path": str(path), "sha256": hashlib.sha256(raw).hexdigest(),
                                "source_file": declaration.source_file,
                                "operations": list(natives)})
    if not any(record["operations"] for record in records):
        raise AuditError("compiled SDK declares no native methods")
    validate_release_api(classes)
    return classes, records


def validate_exports(operations: Sequence[dict[str, object]], symbols: Sequence[str]) -> None:
    """Require exact JNI ownership; stale namespaces and undeclared methods fail."""
    expected = [operation["symbol"] for operation in operations]
    if not expected or len(set(expected)) != len(expected):
        raise AuditError("JNI declarations are empty or have colliding mangled symbols")
    observed = [symbol for symbol in symbols if symbol.startswith("Java_")]
    if len(set(observed)) != len(observed):
        raise AuditError("native library contains duplicate JNI export symbols")
    missing = sorted(set(expected) - set(observed))
    unowned = sorted(set(observed) - set(expected))
    if missing or unowned:
        details = []
        if missing:
            details.append(f"{len(missing)} missing JNI exports: " + ", ".join(missing[:20]))
        if unowned:
            details.append(f"{len(unowned)} unowned JNI exports: " + ", ".join(unowned[:20]))
        raise AuditError("; ".join(details))


def audit(roots: dict[str, Sequence[Path]], library: Path) -> dict[str, object]:
    """Seal the inspected inputs around a complete compiled declaration/export check."""
    _, records = scan_classes(roots)
    library = library.resolve(strict=True)
    digest, size = ARTIFACT.stable_artifact_identity(library)
    symbols = ARTIFACT.inspect_exported_symbols(library, required=True)
    operations = [dict(operation, module=record["module"]) for record in records
                  for operation in record["operations"]]
    validate_exports(operations, symbols)
    if ARTIFACT.stable_artifact_identity(library) != (digest, size):
        raise AuditError("native library changed during inspection")
    _, after = scan_classes(roots)
    if records != after:
        raise AuditError("compiled SDK classes changed during inspection")
    encoded = json.dumps(records, sort_keys=True, separators=(",", ":")).encode()
    return {
        "schema_version": 1,
        "valid": True,
        "scope": "compiled_kotlin_declarations_and_exact_jni_export_ownership",
        "native_signatures_qualified": False,
        "native_execution_qualified": False,
        "source_build_provenance_qualified": False,
        "library": {"path": str(library), "sha256": digest, "size_bytes": size},
        "class_inventory_sha256": hashlib.sha256(encoded).hexdigest(),
        "class_count": len(records),
        "native_method_count": len(operations),
        "operations": sorted(operations, key=lambda operation: operation["symbol"]),
        "classes": [{key: value for key, value in record.items() if key != "operations"}
                    for record in records],
    }


def write_report(path: Path, result: dict[str, object]) -> None:
    """Atomically publish evidence without truncating a hardlinked build input."""
    if path.is_symlink():
        raise AuditError("report destination must not be a symlink")
    if path.exists():
        inputs = [Path(result["library"]["path"]),
                  *(Path(record["path"]) for record in result["classes"])]
        if any(path.samefile(source) for source in inputs):
            raise AuditError("report must not alias an inspected build input")
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix="." + path.name + ".", delete=False) as stream:
            temporary = Path(stream.name)
            json.dump(result, stream, indent=2)
            stream.write("\n")
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main(argv: Sequence[str] | None = None) -> int:
    """Check explicit build outputs; write evidence only after a successful check."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--classes", action="append", required=True, metavar="MODULE=DIR",
                        help="main class output for each SDK module; repeat for multiple outputs")
    parser.add_argument("--library", type=Path, required=True, help="fresh connect_norito_bridge library")
    parser.add_argument("--report", type=Path, help="optional JSON evidence destination")
    arguments = parser.parse_args(argv)
    roots = {module: [] for module in MODULES}
    for value in arguments.classes:
        module, separator, path = value.partition("=")
        if not separator or module not in roots or not path:
            parser.error("--classes must be MODULE=DIR for a Kotlin SDK module")
        roots[module].append(Path(path))
    try:
        if arguments.report:
            report = arguments.report.resolve()
            if report == arguments.library.resolve() or any(
                report.is_relative_to(root.resolve()) for paths in roots.values() for root in paths
            ):
                raise AuditError("report must not overwrite an inspected build input")
        result = audit(roots, arguments.library)
        if arguments.report:
            write_report(arguments.report, result)
    except (AuditError, JVM.ClassFileError, ARTIFACT.ArtifactContractError, OSError) as error:
        print(f"Kotlin JNI check failed: {error}", file=sys.stderr)
        return 1
    print(f"Kotlin JNI check passed: {result['native_method_count']} declarations, {result['class_count']} classes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
