#!/usr/bin/env python3
"""Resolve canonical Kotlin build outputs and collect their exact SDK SBOMs.

Python 3.12, standard library only. All three runtime artifacts require generated
release POMs at one version. Maven intake requires all three matching original
artifacts, POMs and Gradle metadata; unrelated modules/versions are refused. An
explicit external artifact root never falls back to source-tree output. Collection
creates a new directory;
it does not sign, publish, execute Gradle, or overwrite an earlier inventory.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import re
import xml.etree.ElementTree as ET


SDK_MODULES = ("core-jvm", "client-android", "kagemusha-wallet-android")
LOCAL_INTEGRATION_DIRECTORY = Path("dist/norito-bridge-android-local")


def canonical_directory(value: str, label: str) -> Path:
    path = Path(value)
    if not value or not path.is_absolute() or any(c in value for c in "\n\r\0"):
        raise ValueError(f"{label} must be an absolute non-empty path")
    if str(path) != value or path.resolve(strict=True) != path or not path.is_dir():
        raise ValueError(f"{label} must be canonical and must not traverse symbolic links")
    return path


def local_integration_directory(repository: Path, value: str) -> Path:
    """Authenticate the sole ignored, owned diagnostic artifact directory."""
    repository = canonical_directory(str(repository), "repository")
    artifacts = canonical_directory(value, "local Android artifact directory")
    if artifacts != repository / LOCAL_INTEGRATION_DIRECTORY:
        raise ValueError("local Android artifacts must use the fixed integration directory")
    metadata = artifacts.lstat()
    if (metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) != 0o700
            or not os.access(artifacts, os.R_OK | os.W_OK | os.X_OK)):
        raise ValueError("local Android artifact directory must be owned and mode 0700")
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith("GIT_")}
    environment.update(GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
                       GIT_OPTIONAL_LOCKS="0")
    tracked = subprocess.run(
        ["/usr/bin/git", "-C", str(repository), "ls-files", "-z", "--",
         str(LOCAL_INTEGRATION_DIRECTORY)], env=environment, check=True,
        capture_output=True, timeout=30,
    )
    ignored = subprocess.run(
        ["/usr/bin/git", "-C", str(repository), "check-ignore", "--no-index", "-q",
         "--", str(LOCAL_INTEGRATION_DIRECTORY)], env=environment, check=False,
        capture_output=True, timeout=30,
    )
    if tracked.stdout or ignored.returncode != 0:
        raise ValueError("local Android artifact directory must be ignored with no tracked files")
    return artifacts


def build_root(repository: Path, external: str | None, *,
               local_integration: bool = False) -> Path:
    repository = canonical_directory(str(repository), "repository")
    if local_integration:
        if external is None:
            raise ValueError("local Android integration requires an explicit artifact directory")
        artifacts = local_integration_directory(repository, external)
        return artifacts / "gradle-build/iroha_kotlin_sdk"
    if external is None:
        return repository / "kotlin"
    artifacts = canonical_directory(external, "MOBILE_SDK_ANDROID_ARTIFACT_DIR")
    if artifacts == repository or artifacts.is_relative_to(repository):
        raise ValueError("Android artifacts must be outside the reviewed source tree")
    return artifacts / "gradle-build/iroha_kotlin_sdk"


def module_build(root: Path, module: str, external: str | None) -> Path:
    if module not in SDK_MODULES:
        raise ValueError("not a canonical published Kotlin SDK module")
    return root / module if external is not None else root / module / "build"


def regular_file(path: Path) -> Path:
    if any(c in str(path) for c in "\n\r\0"):
        raise ValueError("artifact paths must not contain line separators")
    if path.resolve(strict=True) != path:
        raise ValueError(f"artifact must not traverse symbolic links: {path}")
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or metadata.st_size == 0:
        raise ValueError(f"artifact must be a non-empty single-link regular file: {path}")
    return path


def sdk_version(value: str) -> str:
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", value):
        raise ValueError("SDK version must be one canonical Maven version token")
    return value


def file_digest(path: Path, algorithm: str = "sha256") -> str:
    """Hash the original regular file with descriptor and stable-metadata checks."""
    regular_file(path)
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW), "rb") as source:
        before = os.fstat(source.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_size == 0:
            raise ValueError("artifact descriptor is not a non-empty single-link file")
        digest = hashlib.file_digest(source, algorithm).hexdigest()
        after = os.fstat(source.fileno())
    def identity(metadata):
        return (metadata.st_dev, metadata.st_ino, metadata.st_size,
                metadata.st_mtime_ns, metadata.st_ctime_ns, metadata.st_nlink)
    if identity(before) != identity(after) or identity(after) != identity(path.lstat()):
        raise ValueError("artifact changed while its original bytes were read")
    return digest


def validate_pom(path: Path, module: str, version: str) -> None:
    regular_file(path)
    if path.stat().st_size > 1024 * 1024:
        raise ValueError("publication POM exceeds the 1 MiB bound")
    original_digest = file_digest(path)
    content = path.read_bytes()
    if hashlib.sha256(content).hexdigest() != original_digest:
        raise ValueError("publication POM changed during parsing")
    if b"<!DOCTYPE" in content.upper() or b"<!ENTITY" in content.upper():
        raise ValueError("publication POM must not declare external or internal entities")
    try:
        document = ET.fromstring(content)
    except ET.ParseError as error:
        raise ValueError("publication POM is not valid XML") from error
    namespace = "{http://maven.apache.org/POM/4.0.0}"
    if document.tag != namespace + "project":
        raise ValueError("publication POM must use the canonical Maven namespace")
    def field(parent, name):
        entries = parent.findall(namespace + name)
        if len(entries) != 1 or entries[0].text is None:
            raise ValueError(f"publication POM needs exactly one {name}")
        return entries[0].text
    expected = {"groupId": "org.hyperledger.iroha.sdk", "artifactId": module,
                "version": version, "packaging": "jar" if module == "core-jvm" else "aar"}
    for name, value in expected.items():
        # Maven omits packaging only for the default JAR format.
        if name == "packaging" and module == "core-jvm" and not document.findall(namespace + name):
            continue
        if field(document, name) != value:
            raise ValueError(f"publication POM {name} disagrees with canonical {module}/{version}")
    owned_dependencies = []
    for dependency in document.findall(namespace + "dependencies/" + namespace + "dependency"):
        if field(dependency, "groupId") == "org.hyperledger.iroha.sdk":
            name = field(dependency, "artifactId")
            if name not in SDK_MODULES or field(dependency, "version") != version:
                raise ValueError("SDK publication dependency must select the same canonical version")
            owned_dependencies.append(name)
    expected_dependencies = {"core-jvm": [], "client-android": ["core-jvm"],
                             "kagemusha-wallet-android": ["client-android"]}[module]
    if sorted(owned_dependencies) != expected_dependencies:
        raise ValueError(f"publication POM has an incomplete or duplicate SDK dependency for {module}")


def validate_module_metadata(path: Path, module: str, version: str,
                             published: Path) -> None:
    regular_file(path)
    if path.stat().st_size > 1024 * 1024:
        raise ValueError("Gradle publication metadata exceeds the 1 MiB bound")
    digest = file_digest(path)
    content = path.read_bytes()
    if hashlib.sha256(content).hexdigest() != digest:
        raise ValueError("Gradle publication metadata changed while read")
    document = json.loads(content)
    component = document.get("component", {})
    if document.get("formatVersion") != "1.1" or (component.get("group"), component.get("module"), component.get("version")) != ("org.hyperledger.iroha.sdk", module, version):
        raise ValueError("Gradle module metadata must bind the exact canonical SDK identity")
    variants = document.get("variants")
    if not isinstance(variants, list) or not variants:
        raise ValueError("Gradle module metadata must retain its variant inventory")
    expected_dependencies = {"core-jvm": [], "client-android": ["core-jvm"],
                             "kagemusha-wallet-android": ["client-android"]}[module]
    names = set()
    usages = set()
    for variant in variants:
        name = variant.get("name")
        if not isinstance(name, str) or not name or name in names or "available-at" in variant:
            raise ValueError("Gradle variants must be unique originals without redirects")
        names.add(name)
        attributes = variant.get("attributes", {})
        category = attributes.get("org.gradle.category")
        documentation = category == "documentation"
        if documentation:
            docstype = attributes.get("org.gradle.docstype")
            if docstype not in ("sources", "javadoc"):
                raise ValueError("only canonical sources/javadoc documentation variants are supported")
            expected_file = path.parent / f"{module}-{version}-{docstype}.jar"
        else:
            usage = attributes.get("org.gradle.usage")
            if (category != "library" or usage not in ("java-api", "java-runtime")
                    or usage in usages or attributes.get("org.gradle.dependency.bundling") != "external"
                    or attributes.get("org.gradle.libraryelements") != ("jar" if module == "core-jvm" else "aar")):
                raise ValueError("exactly one canonical java-api and java-runtime library variant is required")
            if module == "core-jvm" and attributes.get("org.gradle.jvm.version") != 8:
                raise ValueError("canonical core-jvm publication must retain the JDK8 consumer contract")
            usages.add(usage)
            expected_file = published
        owned_dependencies = []
        for dependency in variant.get("dependencies", []):
            if dependency.get("group") == "org.hyperledger.iroha.sdk":
                requested = dependency.get("version", {})
                if (dependency.get("module") not in SDK_MODULES
                        or not isinstance(requested, dict) or requested.get("requires") != version
                        or any(requested.get(key, version) != version for key in ("strictly", "prefers"))
                        or requested.get("rejects")):
                    raise ValueError("Gradle SDK dependency must select exactly the same SDK version")
                owned_dependencies.append(dependency["module"])
        if sorted(owned_dependencies) != ([] if documentation else expected_dependencies):
            raise ValueError("Gradle release variant omits, duplicates or substitutes its SDK dependency graph")
        regular_file(expected_file)
        entries = variant.get("files")
        if not isinstance(entries, list) or len(entries) != 1:
            raise ValueError("each canonical Gradle variant must bind exactly one original artifact")
        entry = entries[0]
        if (entry.get("name") != expected_file.name or entry.get("url") != expected_file.name
                or type(entry.get("size")) is not int or entry["size"] != expected_file.stat().st_size):
            raise ValueError("Gradle variant artifact name/URL/size differs from the original publication")
        if "sha256" not in entry:
            raise ValueError("Gradle variant must bind the original SHA256")
        for algorithm in ("sha256", "sha512", "sha1", "md5"):
            if algorithm in entry and entry[algorithm] != file_digest(expected_file, algorithm):
                raise ValueError("Gradle variant checksum differs from the original publication bytes")
    if usages != {"java-api", "java-runtime"}:
        raise ValueError("Gradle metadata is missing a canonical release variant")


def validate_maven_inventory(maven: Path, version: str) -> None:
    """Do not package an unrelated or older Maven coordinate beside this graph."""
    prefix = Path("org/hyperledger/iroha/sdk")
    for path in sorted(maven.rglob("*")):
        if path.is_dir():
            if path.resolve(strict=True) != path:
                raise ValueError("Maven inventory must not traverse symbolic directories")
            continue
        regular_file(path)
        relative = path.relative_to(maven)
        if relative.parts[:4] != prefix.parts or len(relative.parts) not in (6, 7):
            raise ValueError("Maven inventory contains a noncanonical SDK coordinate")
        module = relative.parts[4]
        if module not in SDK_MODULES:
            raise ValueError("Maven inventory contains a retired or unrelated SDK module")
        name = relative.name
        for suffix in (".md5", ".sha1", ".sha256", ".sha512"):
            if name.endswith(suffix):
                name = name[:-len(suffix)]
                break
        if len(relative.parts) == 6:
            if name != "maven-metadata.xml":
                raise ValueError("Maven inventory contains an unrelated module-root file")
            if path.name != name:
                continue
            content = path.read_bytes()
            if len(content) > 1024 * 1024 or b"<!DOCTYPE" in content.upper() or b"<!ENTITY" in content.upper():
                raise ValueError("Maven version metadata exceeds the bounded XML contract")
            document = ET.fromstring(content)
            if (document.findtext("groupId"), document.findtext("artifactId")) != ("org.hyperledger.iroha.sdk", module):
                raise ValueError("Maven version metadata disagrees with its exact SDK module")
            versions = [element.text for element in document.findall("versioning/versions/version")]
            if versions != [version] or any(document.findtext("versioning/" + key, version) != version for key in ("latest", "release")):
                raise ValueError("Maven metadata must contain only the selected SDK version")
        else:
            extension = "jar" if module == "core-jvm" else "aar"
            allowed = {f"{module}-{version}.{suffix}" for suffix in (extension, "pom", "module")}
            allowed.update({f"{module}-{version}-{suffix}.jar" for suffix in ("sources", "javadoc")})
            if relative.parts[5] != version or name not in allowed:
                raise ValueError("Maven inventory contains another SDK version or an unrelated publication file")


def built_artifacts(repository: Path, external: str | None, *,
                    local_integration: bool = False,
                    version: str | None = None) -> tuple[Path, Path, Path]:
    root = build_root(repository, external, local_integration=local_integration)
    libraries = module_build(root, "core-jvm", external) / "libs"
    jars = sorted(p for p in libraries.glob("core-jvm-*.jar")
                  if not p.name.endswith(("-sources.jar", "-javadoc.jar")))
    if len(jars) != 1:
        raise ValueError("exactly one canonical core-jvm runtime JAR is required")
    jar = regular_file(jars[0])
    selected_version = sdk_version(jar.name[len("core-jvm-"):-len(".jar")])
    if version is not None and selected_version != sdk_version(version):
        raise ValueError("core-jvm runtime JAR disagrees with the requested SDK version")
    outputs = [jar]
    for module in SDK_MODULES[1:]:
        outputs.append(regular_file(module_build(root, module, external)
                                    / f"outputs/aar/{module}-release.aar"))
    # AAR filenames carry no version. Their generated release POMs bind every
    # output and the client->core / wallet->client graph to one exact version.
    for module in SDK_MODULES:
        validate_pom(module_build(root, module, external)
                     / "publications/release/pom-default.xml", module, selected_version)
    return tuple(outputs)


def maven_artifacts(repository: Path, external: str | None, maven: Path,
                    version: str) -> tuple[Path, ...]:
    """Require the complete published graph and byte-identical built artifacts."""
    maven = canonical_directory(str(maven), "Android Maven repository")
    repository = canonical_directory(str(repository), "repository")
    if maven == repository or maven.is_relative_to(repository):
        raise ValueError("Android Maven publication must be outside the reviewed source tree")
    outputs = built_artifacts(repository, external, version=sdk_version(version))
    validate_maven_inventory(maven, version)
    selected = []
    for module, output in zip(SDK_MODULES, outputs, strict=True):
        directory = maven / "org/hyperledger/iroha/sdk" / module / version
        extension = "jar" if module == "core-jvm" else "aar"
        published = regular_file(directory / f"{module}-{version}.{extension}")
        pom = directory / f"{module}-{version}.pom"
        validate_pom(pom, module, version)
        if file_digest(published) != file_digest(output):
            raise ValueError(f"Maven {module} artifact differs from the selected canonical build")
        metadata = directory / f"{module}-{version}.module"
        validate_module_metadata(metadata, module, version, published)
        selected.extend((published, pom, metadata))
    return tuple(selected)


def collect_sboms(repository: Path, external: str | None, destination: Path,
                  version: str | None = None) -> None:
    root = build_root(repository, external)
    parent = canonical_directory(str(destination.parent), "SBOM output parent")
    if destination.name in ("", ".", "..") or destination != parent / destination.name:
        raise ValueError("SBOM destination must have one canonical directory name")
    if destination == repository or destination.is_relative_to(repository):
        raise ValueError("SBOM output must be outside the reviewed source tree")
    documents = {}
    for module in SDK_MODULES:
        path = regular_file(module_build(root, module, external) / "reports/bom/bom.json")
        if path.stat().st_size > 32 * 1024 * 1024:
            raise ValueError("SDK SBOM exceeds the 32 MiB bound")
        content = path.read_bytes()
        document = json.loads(content)
        component = document.get("metadata", {}).get("component", {})
        if document.get("bomFormat") != "CycloneDX" or not isinstance(document.get("components"), list):
            raise ValueError(f"missing CycloneDX component inventory for {module}")
        if component.get("group") != "org.hyperledger.iroha.sdk" or component.get("name") != module:
            raise ValueError(f"SBOM is not owned by canonical Kotlin module {module}")
        if not isinstance(component.get("version"), str) or not component["version"]:
            raise ValueError(f"SBOM module version is missing for {module}")
        if version is not None and component["version"] != version:
            raise ValueError(f"SBOM version disagrees with requested SDK version for {module}")
        documents[f"iroha-{module}.cyclonedx.json"] = content
    # Validate the complete inventory before creating any output. A prior
    # generation, symlink, or competing creator must never be overwritten.
    destination.mkdir(mode=0o700)
    for filename, content in documents.items():
        with (destination / filename).open("xb") as output:
            output.write(content)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--collect-sboms", type=Path)
    parser.add_argument("--print-build-root", action="store_true")
    parser.add_argument("--local-integration", action="store_true")
    parser.add_argument("--validate-local-root", action="store_true")
    parser.add_argument("--version")
    parser.add_argument("--maven-repo", type=Path)
    parser.add_argument("--artifact-dir", default=os.environ.get("MOBILE_SDK_ANDROID_ARTIFACT_DIR"))
    args = parser.parse_args()
    try:
        external = args.artifact_dir
        if args.validate_local_root:
            if (args.local_integration or args.print_build_root
                    or args.collect_sboms is not None or external is None):
                raise ValueError("local directory validation requires only an explicit artifact directory")
            if sys.version_info[:2] != (3, 12) or not sys.flags.isolated:
                raise ValueError("local directory validation requires isolated Python 3.12")
            print(local_integration_directory(args.root, external))
        elif args.print_build_root:
            print(build_root(args.root, external, local_integration=args.local_integration))
        elif args.collect_sboms is not None:
            if args.local_integration:
                raise ValueError("diagnostic Android artifacts cannot enter release SBOM collection")
            collect_sboms(args.root, external, args.collect_sboms, args.version)
        elif args.maven_repo is not None:
            if args.local_integration or args.version is None:
                raise ValueError("Maven validation requires a version and excludes local integration")
            for path in maven_artifacts(args.root, external, args.maven_repo, args.version):
                print(path)
        else:
            for path in built_artifacts(args.root, external, local_integration=args.local_integration,
                                        version=args.version):
                print(path)
        return 0
    except (OSError, ValueError, TypeError, AttributeError, subprocess.SubprocessError) as error:
        print(f"[mobile-sdk-android] ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
