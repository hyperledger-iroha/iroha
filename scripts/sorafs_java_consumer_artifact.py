"""Bounded actual-package and executed-case contract for SoraFS Java consumers.

This is a host qualification artifact owner, not a release-promotion verifier.
It supplies no assertion that Android device or six-SDK qualification passed.
"""
from __future__ import annotations

import hashlib
import io
import json
import math
import re
import stat
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit
from urllib.request import url2pathname
from xml.parsers import expat
import zipfile

from jvm_classfile import ACC_ABSTRACT, MAX_CLASS_BYTES, parse_class
from sorafs_evidence_json import read_evidence_bytes

SCHEMA = "sorafs.java_source_kotlin.consumer_artifact.v1"
SUITE = "org.hyperledger.iroha.sdk.sorafs.SorafsReferenceValidatorsJavaConsumerTest"
SDK_PREFIX = "org/hyperledger/iroha/sdk/"
ANDROID_OWNER = SDK_PREFIX + "crypto/keystore/KeySecurityPreference"
CORE_OWNERS = (SDK_PREFIX + "sorafs/SorafsReferenceValidators", SDK_PREFIX + "client/JsonParser")
MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_MEMBER_BYTES = 64 * 1024 * 1024
MAX_MEMBERS = 20_000
MAX_REPORT_BYTES = 8 * 1024 * 1024
MAX_LOG_BYTES = 32 * 1024 * 1024
GROUPS = ('exposesBridgeSelectors', 'fixtureBundleInputSnapshotsPayloadBytes', 'boundsFixtureBundleBeforeNativeDispatch', 'boundsGovernanceLogNodeCidBeforeNativeDispatch', 'rejectsGeneratedAtBeforeNativeDispatch', 'rejectsBlankLabelBeforeNativeDispatch', 'rejectsMalformedUnicodeFixtureLabelBeforeNativeDispatch', 'boundsGovernanceDagInputsBeforeNativeDispatch', 'rejectsNonSignableOrderbookPayloadBeforeNativeDispatch', 'rejectsBadSigningKeyBeforeNativeDispatch', 'rejectsInvalidOrderIdDerivationInputsBeforeNativeDispatch', 'rejectsOversizedOrderbookOwnerAccountsBeforeNativeDispatch', 'rejectsOrderbookOrderRequestFieldsBeforeNativeDispatch', 'rejectsOrderbookSettlementReceiptFieldsBeforeNativeDispatch', 'rejectsNoncanonicalXorQuantitiesBeforeNativeDispatch', 'popMembershipStructuralFixturesRequirePresentationBinding', 'validatesOrderbookFixtureWhenNativeBridgeIsAvailable', 'validatesEveryPdpOutcomeFixtureWhenNativeBridgeIsAvailable', 'validatesAppealFinanceCancelAssetLockProfiles', 'validatesLinkedFixtureBundleWhenNativeBridgeIsAvailable', 'validatesEveryReferenceSdkBundleOutcomeByteForByte', 'validatesModerationGovernanceLogNodeOutcomeByteForByte', 'validatesGovernanceDagFixturesAndNegativeVectorsWhenNativeBridgeIsAvailable', 'signsOrderbookFixtureWhenNativeBridgeIsAvailable', 'derivesCanonicalOrderIdWhenNativeBridgeIsAvailable')


class ArtifactError(ValueError):
    """Actual archive, source, class origin or executed-case evidence is invalid."""


def canonical_json(value: object) -> bytes:
    """Encode deterministic UTF-8 public artifact metadata."""
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False,
                       allow_nan=False) + "\n").encode("utf-8")


def read_file(path: Path, maximum: int) -> bytes:
    """Reuse bounded anchored reads; reject empty, linked or unstable inputs."""
    if not path.is_absolute() or ".." in path.parts:
        raise ArtifactError("input must use an absolute canonical path")
    raw = read_evidence_bytes(path, maximum)
    if not raw:
        raise ArtifactError("input must not be empty")
    return raw


def identity(raw: bytes) -> dict[str, object]:
    """Identify exact retained bytes without accepting an asserted digest."""
    return {"sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)}


def archive_members(raw: bytes) -> dict[str, bytes]:
    """Read one bounded archive without extracting names into the filesystem."""
    if not raw or len(raw) > MAX_ARCHIVE_BYTES:
        raise ArtifactError("archive byte limit exceeded")
    result = {}
    total = 0
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        entries = archive.infolist()
        if not entries or len(entries) > MAX_MEMBERS:
            raise ArtifactError("archive member inventory is empty or excessive")
        names = set()
        for entry in entries:
            name = entry.filename
            if entry.orig_filename != name:
                raise ArtifactError("archive contains a normalized or truncated member name")
            parts = PurePosixPath(name).parts
            if (not name or name in names or name.startswith("/") or "\\" in name
                    or "\x00" in name or ":" in name or not parts
                    or any(part in ("", ".", "..") for part in name.rstrip("/").split("/"))
                    or len(name.encode("utf-8")) > 1024):
                raise ArtifactError("archive has an unsafe or duplicate member")
            names.add(name)
            mode = entry.external_attr >> 16
            if stat.S_ISLNK(mode) or (stat.S_IFMT(mode) not in (0, stat.S_IFREG, stat.S_IFDIR)):
                raise ArtifactError("archive contains a non-regular member")
            if entry.flag_bits & 1 or entry.compress_type not in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                raise ArtifactError("archive encryption or compression is unsupported")
            if entry.is_dir():
                if entry.file_size != 0:
                    raise ArtifactError("archive directory carries payload")
                continue
            total += entry.file_size
            if entry.file_size > MAX_MEMBER_BYTES or total > MAX_ARCHIVE_BYTES:
                raise ArtifactError("archive expansion exceeds its limit")
            body = archive.read(entry)
            if len(body) != entry.file_size:
                raise ArtifactError("archive member size differs")
            result[name] = body
    manifest = result.get("META-INF/MANIFEST.MF", b"")
    if re.search(br"(?im)^class-path\s*:", manifest):
        raise ArtifactError("archive manifest must not extend the fixed classpath")
    return result


def jar_classes(raw: bytes, *, sdk: bool) -> dict[str, bytes]:
    """Bind each declared SDK class name to its actual single JAR entry."""
    result = {}
    for name, body in archive_members(raw).items():
        if not name.endswith(".class"):
            continue
        if len(body) > MAX_CLASS_BYTES:
            raise ArtifactError("class byte limit exceeded")
        owner = parse_class(body)
        if not owner.name.startswith(SDK_PREFIX):
            if sdk:
                raise ArtifactError("SDK package contains a foreign class namespace")
            continue
        if not sdk:
            raise ArtifactError("dependency shadows the SDK namespace")
        if owner.name.startswith(SUITE.replace(".", "/")):
            raise ArtifactError("package shadows the Java consumer assertion owner")
        if name != owner.name + ".class" or owner.major != 52 or owner.name in result:
            raise ArtifactError("SDK class entry, identity or JDK-8 target differs")
        result[owner.name] = body
    return result


def package_classes(core: bytes, android: bytes) -> tuple[dict[str, bytes], dict[str, bytes], bytes]:
    """Require distinct actual Kotlin core and Android AAR class ownership."""
    core_classes = jar_classes(core, sdk=True)
    aar = archive_members(android)
    if "classes.jar" not in aar or "AndroidManifest.xml" not in aar:
        raise ArtifactError("Android package lacks classes.jar or AndroidManifest.xml")
    if any(name.startswith("libs/") and name.endswith(".jar") for name in aar):
        raise ArtifactError("Android package introduces an unreviewed embedded classpath")
    android_classes = jar_classes(aar["classes.jar"], sdk=True)
    if core_classes.keys() & android_classes.keys():
        raise ArtifactError("Kotlin core and Android package duplicate SDK classes")
    for name in CORE_OWNERS:
        if name not in core_classes:
            raise ArtifactError("Kotlin core package lacks the required actual SoraFS API")
    if ANDROID_OWNER not in android_classes:
        raise ArtifactError("Android package lacks its required link-probe owner")
    for name, classes, expected_source in (
        (CORE_OWNERS[0], core_classes, "SorafsReferenceValidators.kt"),
        (CORE_OWNERS[1], core_classes, "JsonParser.kt"),
        (ANDROID_OWNER, android_classes, "KeySecurityPreference.kt"),
    ):
        if parse_class(classes[name]).source_file != expected_source:
            raise ArtifactError("package owner does not carry the expected Kotlin source identity")
    return core_classes, android_classes, aar["classes.jar"]


def validate_test_classes(classes: dict[str, bytes]) -> None:
    """Require the actual compiled Java owner and all 25 instance methods."""
    name = SUITE.replace(".", "/")
    if name not in classes:
        raise ArtifactError("compiled Java assertion owner is absent")
    owner = parse_class(classes[name])
    if (owner.name != name or owner.major != 52
            or owner.source_file != "SorafsReferenceValidatorsJavaConsumerTest.java"):
        raise ArtifactError("compiled Java assertion source/target identity differs")
    for method in GROUPS:
        found = [m for m in owner.methods if m.name == method]
        if len(found) != 1 or found[0].descriptor != "()V" or found[0].static or found[0].native or found[0].flags & ACC_ABSTRACT:
            raise ArtifactError("compiled Java assertion group is absent or ambiguous")


def validate_report(raw: bytes) -> tuple[str, ...]:
    """Require exactly the 25 real non-skipped successful JUnit executions."""
    if not raw or len(raw) > MAX_REPORT_BYTES:
        raise ArtifactError("JUnit report byte limit exceeded")
    depth = 0
    seen = []
    root_seen = False

    def start(name: str, attributes: dict[str, str]) -> None:
        nonlocal depth, root_seen
        depth += 1
        if depth == 1:
            if root_seen or name != "testsuite" or attributes != {
                "name": SUITE, "tests": str(len(GROUPS)), "failures": "0", "errors": "0", "skipped": "0",
            }:
                raise ArtifactError("JUnit suite identity or result totals differ")
            root_seen = True
        elif depth == 2 and name == "testcase":
            if set(attributes) != {"classname", "name", "time"} or attributes["classname"] != SUITE:
                raise ArtifactError("JUnit case owner or fields differ")
            method = attributes["name"]
            if method not in GROUPS or method in seen:
                raise ArtifactError("JUnit method is missing, repeated or substituted")
            try:
                duration = float(attributes["time"])
            except ValueError as error:
                raise ArtifactError("JUnit duration is malformed") from error
            if not math.isfinite(duration) or duration < 0:
                raise ArtifactError("JUnit duration is not finite and nonnegative")
            seen.append(method)
        else:
            raise ArtifactError("JUnit report contains an unexpected or nonpassing element")

    def end(_name: str) -> None:
        nonlocal depth
        depth -= 1

    def forbidden(*_args: object) -> None:
        raise ArtifactError("JUnit DTD/entity declarations are forbidden")

    parser = expat.ParserCreate()
    parser.StartElementHandler = start
    parser.EndElementHandler = end
    parser.StartDoctypeDeclHandler = forbidden
    parser.EntityDeclHandler = forbidden
    parser.ExternalEntityRefHandler = forbidden
    try:
        parser.Parse(raw, True)
    except expat.ExpatError as error:
        raise ArtifactError("JUnit report is malformed") from error
    if not root_seen or len(seen) != len(GROUPS) or set(seen) != set(GROUPS):
        raise ArtifactError("JUnit report omits required Java executions")
    return tuple(seen)


def validate_class_origins(raw: bytes, jars: dict[Path, dict[str, bytes]], *, android: bool, consumer_classes: tuple[Path, dict[str, bytes]] | None = None) -> list[dict[str, object]]:
    """Require every loaded SDK class to originate in the exact packaged JARs."""
    if not raw or len(raw) > MAX_LOG_BYTES:
        raise ArtifactError("class-load log byte limit exceeded")
    seen = {}
    originals = {}
    for line in raw.decode("utf-8", "strict").splitlines():
        match = re.search(r"\[class,load\]\s+(\S+) source: (.+)$", line)
        if match is None:
            continue
        name, origin = match.groups()
        owner = name.replace(".", "/")
        if not owner.startswith(SDK_PREFIX):
            continue
        enclosing = origin.replace(".", "/")
        if re.fullmatch(re.escape(origin) + r"\$\$Lambda/0x[0-9a-fA-F]+", name):
            original = originals.get(enclosing)
            if original is None or not parse_class(original).lambda_metafactory or owner in seen:
                raise ArtifactError("generated SDK lambda lacks its exact loaded bootstrap owner")
            seen[owner] = {"class": owner, "runtime_generated": "jdk21-lambda", "enclosing_class": enclosing, "enclosing_bytes": identity(original)}
            continue
        url = urlsplit(origin)
        if url.scheme != "file" or url.netloc or url.query or url.fragment:
            raise ArtifactError("SDK class came from a non-package origin")
        path = Path(url2pathname(url.path))
        classes = jars.get(path)
        if consumer_classes is not None and path == consumer_classes[0]:
            classes = consumer_classes[1]
        if classes is None or owner not in classes or owner in seen:
            raise ArtifactError("SDK class origin is missing, repeated or substituted")
        originals[owner] = classes[owner]
        seen[owner] = {"class": owner, "package": path.name, **identity(classes[owner])}
    required = set(CORE_OWNERS) | ({ANDROID_OWNER} if android else set())
    if consumer_classes is not None:
        required.add(SUITE.replace(".", "/"))
    if not required <= seen.keys():
        raise ArtifactError("required packaged SDK owners were not actually loaded")
    return [seen[name] for name in sorted(seen)]


def validate_library_origin(raw: bytes, expected: Path) -> None:
    """Bind the actual JVM native load to the previously authenticated copy."""
    if not raw or len(raw) > MAX_LOG_BYTES:
        raise ArtifactError("library-load log byte limit exceeded")
    origins = re.findall(r"Loaded library (.+?), handle", raw.decode("utf-8", "strict"))
    native = [path for path in origins if "connect_norito_bridge" in Path(path).name]
    if native != [str(expected)]:
        raise ArtifactError("JVM did not load the exact sole authenticated JNI copy")


def deterministic_archive(members: dict[str, bytes]) -> bytes:
    """Package retained observations with stable names, modes and timestamps."""
    if not members or len(members) > MAX_MEMBERS or any(len(raw) > MAX_MEMBER_BYTES for raw in members.values()) or sum(map(len, members.values())) > MAX_ARCHIVE_BYTES:
        raise ArtifactError("qualification archive inventory exceeds its limits")
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_STORED) as archive:
        for name, raw in sorted(members.items()):
            entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            entry.create_system = 3
            entry.external_attr = (stat.S_IFREG | 0o600) << 16
            archive.writestr(entry, raw)
    raw = output.getvalue()
    if archive_members(raw) != members:
        raise ArtifactError("qualification archive does not replay exactly")
    return raw
