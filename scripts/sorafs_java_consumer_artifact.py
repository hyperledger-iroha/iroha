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
QUALIFICATION_PREFIX = "org/hyperledger/iroha/qualification/"
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


# Runtime classpath is fixed to JDK 21; no launcher property overrides are allowed.
DEPENDENCY_RUNTIME_OWNERS = (
    "org/junit/jupiter/engine/JupiterTestEngine",
    "org/junit/platform/launcher/core/LauncherFactory",
)
_JDK_CLASS_PREFIXES = ("java/", "javax/", "jdk/", "sun/", "com/sun/", "org/w3c/", "org/xml/", "org/ietf/")


def _multi_release_manifest(raw: bytes) -> bool:
    """Read only unfolded main attributes; named sections cannot enable MR lookup."""
    if not raw:
        return False
    if len(raw) > 64 * 1024 or not raw.endswith((b"\n", b"\r")) or b"\x00" in raw:
        raise ArtifactError("dependency manifest is excessive or incomplete")
    logical = []
    for line in raw.replace(b"\r\n", b"\n").replace(b"\r", b"\n").split(b"\n"):
        if not line:
            break
        if line.startswith(b" "):
            if not logical:
                raise ArtifactError("dependency manifest has an orphan continuation")
            logical[-1] += line[1:]
        else:
            logical.append(line)
    attributes = {}
    for line in logical:
        match = re.fullmatch(rb"([A-Za-z0-9][A-Za-z0-9_-]{0,69}): (.*)", line)
        if match is None:
            raise ArtifactError("dependency manifest main attribute is malformed")
        key, value = match.groups()
        key = key.lower()
        if key in attributes:
            raise ArtifactError("dependency manifest repeats a main attribute")
        value.decode("utf-8", "strict")
        attributes[key] = value
    if b"class-path" in attributes:
        raise ArtifactError("dependency manifest extends the fixed classpath")
    return attributes.get(b"multi-release", b"").lower() == b"true"


def dependency_classes(raw: bytes) -> dict[str, bytes]:
    """Derive actual JDK-21 effective dependency bytes, including multi-release JARs.

    The highest active version from 9 through 21 overrides a base entry. Inactive
    versioned classes never supply a runtime owner. All class entries still have
    their declared name checked and may not hide SDK, qualification or JDK owners.
    Module descriptors are not runtime classpath classes. No archive is extracted.
    """
    members = archive_members(raw)
    manifests = [name for name in members if name.upper() == "META-INF/MANIFEST.MF"]
    if manifests and manifests != ["META-INF/MANIFEST.MF"]:
        raise ArtifactError("dependency manifest name is noncanonical or repeated")
    multi_release = _multi_release_manifest(members.get("META-INF/MANIFEST.MF", b""))
    selected = {}
    for name, body in members.items():
        if not name.endswith(".class"):
            continue
        if len(body) > MAX_CLASS_BYTES:
            raise ArtifactError("dependency class byte limit exceeded")
        version, relative = 0, name
        if name.startswith("META-INF/"):
            match = re.fullmatch(r"META-INF/versions/([1-9][0-9]{0,9})/(.+\.class)", name)
            if match is None or not 9 <= int(match[1]) <= 2_147_483_647:
                raise ArtifactError("dependency versioned class entry is noncanonical")
            version, relative = int(match[1]), match[2]
        owner = parse_class(body)
        if relative != owner.name + ".class":
            raise ArtifactError("dependency class entry differs from its declared owner")
        if owner.name == "module-info":
            continue
        if owner.name.startswith((SDK_PREFIX, "org/hyperledger/iroha/qualification/", *_JDK_CLASS_PREFIXES)):
            raise ArtifactError("dependency shadows an SDK, qualification or JDK owner")
        if version and owner.major > version + 44:
            raise ArtifactError("dependency class target exceeds its versioned directory")
        if version and (not multi_release or version > 21):
            continue
        if not 45 <= owner.major <= 65 or body[4:6] == b"\xff\xff":
            raise ArtifactError("effective dependency class requires a different runtime")
        previous = selected.get(owner.name)
        if previous is None or version > previous[0]:
            selected[owner.name] = (version, body)
    return {name: selected[name][1] for name in sorted(selected)}


def dependency_classpath(dependencies: dict[str, bytes], root: Path) -> dict[Path, dict[str, bytes]]:
    """Bind disjoint effective owners to the producer's exact sorted private copies."""
    jars = {}
    names = set()
    for index, (_module, raw) in enumerate(sorted(dependencies.items())):
        classes = dependency_classes(raw)
        if names.intersection(classes):
            raise ArtifactError("tool dependencies shadow effective runtime class ownership")
        names.update(classes)
        jars[root / "dependencies" / f"{index}.jar"] = classes
    return jars


def validate_dependency_origins(raw: bytes, jars: dict[Path, dict[str, bytes]], *, execution: bool) -> list[dict[str, object]]:
    """Join one JVM process's dependency loads to its exact effective JAR bytes.

    SDK and qualification classes have their separate mandatory owner check.
    Only JDK/module/shared-image origins and JDK-generated classes are outside this
    dependency check. Foreign file origins, unknown application classes, duplicate
    loads and generated dependency lambdas without their actual bootstrap reject.
    Annotation-only dependencies need not execute; every loaded dependency does.
    """
    if not raw or len(raw) > MAX_LOG_BYTES:
        raise ArtifactError("dependency class-load log byte limit exceeded")
    class_owners = {}
    for path, classes in jars.items():
        for name, body in classes.items():
            if name in class_owners:
                raise ArtifactError("dependency runtime owner is ambiguous")
            class_owners[name] = (path, body)
    seen, originals, jdk_seen = {}, {}, set()
    for line in raw.decode("utf-8", "strict").splitlines():
        match = re.search(r"\[class,load\]\s+(\S+) source: (.+)$", line)
        if match is None:
            continue
        name, origin = match.groups()
        owner = name.replace(".", "/")
        if owner.startswith((SDK_PREFIX, "org/hyperledger/iroha/qualification/")):
            continue
        enclosing = origin.replace(".", "/")
        if re.fullmatch(re.escape(origin) + r"\$\$Lambda/0x[0-9a-fA-F]+", name) and enclosing in class_owners:
            original = originals.get(enclosing)
            if original is None or not parse_class(original).lambda_metafactory or owner in seen:
                raise ArtifactError("generated dependency lambda lacks its exact loaded bootstrap owner")
            seen[owner] = {"class": owner, "runtime_generated": "jdk21-lambda", "enclosing_class": enclosing, "enclosing_bytes": identity(original)}
            continue
        url = urlsplit(origin)
        if owner.startswith(_JDK_CLASS_PREFIXES):
            # Only observed JDK-21 origin forms exempt a JDK namespace owner.
            # Application namespaces cannot impersonate the shared image/modules.
            ordinary = origin == "shared objects file" or re.fullmatch(r"jrt:/(?:java|jdk)(?:\.[A-Za-z][A-Za-z0-9_]*)+", origin) is not None
            generated = False
            if origin == "__JVM_LookupDefineClass__":
                generated = "java/lang/invoke/LambdaForm" in jdk_seen and re.fullmatch(r"java\.lang\.invoke\.LambdaForm\$(?:MH|DMH|BMH)/0x[0-9a-fA-F]+", name) is not None
            elif origin == "__dynamic_proxy__":
                generated = "java/lang/reflect/Proxy" in jdk_seen and re.fullmatch(r"jdk\.proxy[1-9][0-9]*\.\$Proxy[0-9]+", name) is not None
            elif origin == "__ClassDefiner__":
                generated = "jdk/internal/reflect/MethodAccessorGenerator" in jdk_seen and re.fullmatch(r"jdk\.internal\.reflect\.Generated(?:ConstructorAccessor|MethodAccessor|SerializationConstructorAccessor)[1-9][0-9]*", name) is not None
            elif re.fullmatch(re.escape(origin) + r"(?:\$[A-Za-z0-9_]+)*\$\$Lambda/0x[0-9a-fA-F]+", name):
                bootstrap = name.split("$$Lambda/", 1)[0].replace(".", "/")
                generated = enclosing in jdk_seen and bootstrap in jdk_seen
            if (ordinary or generated) and owner not in jdk_seen:
                jdk_seen.add(owner)
                continue
        if url.scheme != "file" or url.netloc or url.query or url.fragment:
            raise ArtifactError("dependency class came from an unknown runtime origin")
        path = Path(url2pathname(url.path))
        expected = class_owners.get(owner)
        if expected is None or path != expected[0] or owner in seen:
            raise ArtifactError("dependency class origin is missing, repeated or substituted")
        body = expected[1]
        originals[owner] = body
        seen[owner] = {"class": owner, "package": path.name, **identity(body)}
    if execution and not set(DEPENDENCY_RUNTIME_OWNERS) <= seen.keys():
        raise ArtifactError("required JUnit dependency owners were not actually loaded")
    return [seen[name] for name in sorted(seen)]


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


def validate_class_origins(raw: bytes, jars: dict[Path, dict[str, bytes]], *, android: bool, consumer_classes: tuple[Path, dict[str, bytes]] | None = None, required_consumer_owners: tuple[str, ...] = ()) -> list[dict[str, object]]:
    """Bind SDK and qualification classes to the actual package/compiled owners."""
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
        if not owner.startswith((SDK_PREFIX, QUALIFICATION_PREFIX)):
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
    required.update(required_consumer_owners)
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
