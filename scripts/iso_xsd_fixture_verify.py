#!/usr/bin/env python3
"""Verify checked-in ISO 20022 XSD and XML fixture wiring.

Purpose:
  This offline preflight checks that the repository's ISO 20022 fixture
  manifest accurately binds checked-in Standards Editor XSDs to XML fixtures.
  It verifies schema target namespaces, the `Document` root, payload-root
  declarations, XML fixture namespaces, reviewed schema-only entries, and
  reviewed fixture entries whose official XSD package is still pending.

Prerequisites:
  Python 3.11+. No third party Python packages are required. This is a
  structural manifest and namespace preflight; it is not a full XSD validator.

Safety:
  The script is read-only unless ``--summary-out`` is supplied. It does not
  fetch schemas or XML documents over the network.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any


MANIFEST_VERSION = 1
SUMMARY_DIGEST_FIELD = "summary_sha256"
XML_SCHEMA_NS = "http://www.w3.org/2001/XMLSchema"
ISO_NAMESPACE_PREFIX = "urn:iso:std:iso:20022:tech:xsd:"
MESSAGE_DEF_ID_RE = re.compile(r"^[a-z]{4}\.\d{3}\.\d{3}\.\d{2}$")
PROFILE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
MESSAGE_TYPE_RE = re.compile(r"^[a-z]{4}\.\d{3}$")
SOURCE_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
SOURCE_REPOSITORY_RE = re.compile(
    r"^https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$"
)
PROFILE_DIRECTIONS = {"inbound", "outbound", "follow-up"}
ALLOWED_SCHEMA_SOURCE_LICENSES = {"Apache-2.0"}
DEFAULT_MANIFEST = (
    Path(__file__).resolve().parents[1]
    / "fixtures"
    / "iso20022"
    / "xsd"
    / "fixture_manifest.json"
)
DEFAULT_PROFILE_CATALOG = (
    Path(__file__).resolve().parents[1]
    / "crates"
    / "iroha_core"
    / "src"
    / "iso_bridge"
    / "profiles.rs"
)
PROFILE_CATALOG_RE = re.compile(
    r'const\s+DEFAULT_PROFILES_JSON:\s*&str\s*=\s*r(?P<hashes>#*)"(?P<body>.*?)"(?P=hashes);',
    re.S,
)
RESTRICTED_SCHEMA_TEXT_MARKERS = (
    "may only be redistributed upon agreement",
    "no right, or right to authorise others",
    "rent, lease, or sell this component",
    "display publicly, distribute or otherwise provide this component",
)

TOP_LEVEL_KEYS = {"version", "schemas", "fixtures"}
SCHEMA_KEYS = {"path", "message_def_id", "payload_root", "source", "schema_only_reason"}
SCHEMA_SOURCE_KEYS = {"repository", "commit", "path", "license", "sha256"}
FIXTURE_KEYS = {
    "path",
    "message_def_id",
    "payload_root",
    "schema",
    "missing_schema_reason",
}


class FixtureManifestError(RuntimeError):
    """Raised when ISO XSD fixture manifest wiring is malformed or incomplete."""


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _load_json(path: Path) -> Any:
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except FileNotFoundError as error:
        raise FixtureManifestError(f"{path} does not exist") from error
    except json.JSONDecodeError as error:
        raise FixtureManifestError(f"{path} is not valid JSON: {error}") from error


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise FixtureManifestError(f"duplicate key {key!r} in JSON object")
        result[key] = value
    return result


def _load_profile_catalog(path: Path) -> tuple[list[Any], str, str]:
    try:
        raw = path.read_bytes()
    except FileNotFoundError as error:
        raise FixtureManifestError(f"{path} does not exist") from error
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise FixtureManifestError(f"{path} is not valid UTF-8") from error
    match = PROFILE_CATALOG_RE.search(text)
    if match is None:
        raise FixtureManifestError(
            f"{path} does not contain DEFAULT_PROFILES_JSON raw string"
        )
    catalog_json = match.group("body")
    try:
        catalog = json.loads(
            catalog_json,
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except json.JSONDecodeError as error:
        raise FixtureManifestError(
            f"{path} DEFAULT_PROFILES_JSON is not valid JSON: {error}"
        ) from error
    return (
        _require_array(catalog, f"{path}.DEFAULT_PROFILES_JSON"),
        sha256_hex(raw),
        sha256_hex(catalog_json.encode("utf-8")),
    )


def _parse_xml(path: Path) -> ET.Element:
    try:
        return ET.parse(path).getroot()
    except FileNotFoundError as error:
        raise FixtureManifestError(f"{path} does not exist") from error
    except ET.ParseError as error:
        raise FixtureManifestError(f"{path} is not well-formed XML: {error}") from error


def _reject_restricted_schema_terms(path: Path) -> None:
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError as error:
        raise FixtureManifestError(f"{path} does not exist") from error
    except UnicodeDecodeError as error:
        raise FixtureManifestError(f"{path} is not valid UTF-8") from error
    lowered = text.casefold()
    for marker in RESTRICTED_SCHEMA_TEXT_MARKERS:
        if marker in lowered:
            raise FixtureManifestError(
                f"{path} contains restricted redistribution terms; "
                "do not check in licensed Standards Editor packages without redistribution rights"
            )


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise FixtureManifestError(f"{label} must be a JSON object")
    return value


def _require_array(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise FixtureManifestError(f"{label} must be a JSON array")
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        raise FixtureManifestError(f"{label} contains unknown keys: {', '.join(unknown)}")


def _required_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise FixtureManifestError(f"{label}.{key} must be a non-empty string")
    if raw != raw.strip():
        raise FixtureManifestError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _require_message_def_id(value: str, label: str) -> str:
    if MESSAGE_DEF_ID_RE.fullmatch(value) is None:
        raise FixtureManifestError(
            f"{label} must be lowercase ISO message id like pacs.008.001.08"
        )
    return value


def _optional_string(value: dict[str, Any], key: str, label: str) -> str | None:
    raw = value.get(key)
    if raw is None:
        return None
    if not isinstance(raw, str) or not raw.strip():
        raise FixtureManifestError(f"{label}.{key} must be a non-empty string when set")
    if raw != raw.strip():
        raise FixtureManifestError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _is_lower_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _required_sha256(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not _is_lower_sha256(raw):
        raise FixtureManifestError(f"{label}.{key} must be a lowercase SHA-256 digest")
    return raw


def _validate_source_path(raw: str, label: str) -> str:
    if "\\" in raw:
        raise FixtureManifestError(f"{label} must use forward slashes")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise FixtureManifestError(f"{label} must not contain control characters")
    path = Path(raw)
    if path.is_absolute():
        raise FixtureManifestError(f"{label} must be relative, got {raw}")
    if not raw.endswith(".xsd"):
        raise FixtureManifestError(f"{label} must point to an .xsd file")
    if any(part in {"", ".", ".."} for part in path.parts):
        raise FixtureManifestError(f"{label} must not contain empty, dot, or parent segments")
    return raw


def _verify_schema_source(
    value: Any,
    label: str,
    *,
    message_def_id: str,
    schema_sha256: str,
) -> dict[str, str]:
    source = _require_object(value, label)
    _reject_unknown_keys(source, SCHEMA_SOURCE_KEYS, label)
    repository = _required_string(source, "repository", label)
    if SOURCE_REPOSITORY_RE.fullmatch(repository) is None or repository.endswith(".git"):
        raise FixtureManifestError(
            f"{label}.repository must be a canonical https://github.com/<org>/<repo> URL"
        )
    commit = _required_string(source, "commit", label)
    if SOURCE_COMMIT_RE.fullmatch(commit) is None:
        raise FixtureManifestError(f"{label}.commit must be a lowercase 40-hex Git commit")
    source_path = _validate_source_path(_required_string(source, "path", label), f"{label}.path")
    if Path(source_path).name != f"{message_def_id}.xsd":
        raise FixtureManifestError(
            f"{label}.path filename must match message_def_id {message_def_id!r}"
        )
    license_id = _required_string(source, "license", label)
    if license_id not in ALLOWED_SCHEMA_SOURCE_LICENSES:
        raise FixtureManifestError(
            f"{label}.license must be one of "
            + ", ".join(sorted(ALLOWED_SCHEMA_SOURCE_LICENSES))
        )
    source_sha256 = _required_sha256(source, "sha256", label)
    if source_sha256 != schema_sha256:
        raise FixtureManifestError(
            f"{label}.sha256 does not match checked-in XSD bytes"
        )
    return {
        "repository": repository,
        "commit": commit,
        "path": source_path,
        "license": license_id,
        "sha256": source_sha256,
    }


def _split_xml_name(name: str) -> tuple[str | None, str]:
    if name.startswith("{"):
        namespace, local = name[1:].split("}", 1)
        return namespace, local
    return None, name


def _namespace_for(message_def_id: str) -> str:
    return ISO_NAMESPACE_PREFIX + message_def_id


def _message_id_from_namespace(namespace: str | None, label: str) -> str:
    if namespace is None or not namespace.startswith(ISO_NAMESPACE_PREFIX):
        raise FixtureManifestError(f"{label} namespace must start with {ISO_NAMESPACE_PREFIX}")
    value = namespace[len(ISO_NAMESPACE_PREFIX) :]
    if not value:
        raise FixtureManifestError(f"{label} namespace has empty message definition id")
    return _require_message_def_id(value, f"{label} namespace message definition id")


def _schema_child(parent: ET.Element, local_name: str, **attrs: str) -> ET.Element | None:
    for child in parent:
        namespace, local = _split_xml_name(child.tag)
        if namespace != XML_SCHEMA_NS or local != local_name:
            continue
        if all(child.attrib.get(key) == value for key, value in attrs.items()):
            return child
    return None


def _schema_children(parent: ET.Element, local_name: str) -> list[ET.Element]:
    result: list[ET.Element] = []
    for child in parent:
        namespace, local = _split_xml_name(child.tag)
        if namespace == XML_SCHEMA_NS and local == local_name:
            result.append(child)
    return result


def _schema_payload_root(root: ET.Element, path: Path) -> str:
    document_element = _schema_child(root, "element", name="Document")
    if document_element is None:
        raise FixtureManifestError(f"{path} has no top-level xs:element name='Document'")
    document_type = document_element.attrib.get("type")
    if not document_type:
        raise FixtureManifestError(f"{path} Document element does not declare a type")
    document_type = document_type.split(":", 1)[-1]
    document_complex = _schema_child(root, "complexType", name=document_type)
    if document_complex is None:
        raise FixtureManifestError(f"{path} has no xs:complexType name={document_type!r}")
    sequence = _schema_child(document_complex, "sequence")
    if sequence is None:
        raise FixtureManifestError(f"{path} Document complex type has no xs:sequence")
    payload_elements = _schema_children(sequence, "element")
    if len(payload_elements) != 1:
        raise FixtureManifestError(
            f"{path} Document sequence must contain exactly one payload element"
        )
    payload = payload_elements[0].attrib.get("name")
    if not payload:
        raise FixtureManifestError(f"{path} Document payload element has no name")
    return payload


def _validate_relative_path(raw: str, base: Path, containment_root: Path, label: str) -> Path:
    path = Path(raw)
    if path.is_absolute():
        raise FixtureManifestError(f"{label} must be relative, got {raw}")
    resolved = (base / path).resolve()
    root = containment_root.resolve()
    if not resolved.is_relative_to(root):
        raise FixtureManifestError(f"{label} must stay under {root}")
    return resolved


def verify_schema_entry(
    entry: dict[str, Any],
    label: str,
    manifest_dir: Path,
) -> dict[str, Any]:
    """Verify one schema manifest entry and return normalized metadata."""

    _reject_unknown_keys(entry, SCHEMA_KEYS, label)
    rel_path = _required_string(entry, "path", label)
    message_def_id = _require_message_def_id(
        _required_string(entry, "message_def_id", label),
        f"{label}.message_def_id",
    )
    expected_payload_root = _required_string(entry, "payload_root", label)
    schema_only_reason = _optional_string(entry, "schema_only_reason", label)
    if not rel_path.endswith(".xsd"):
        raise FixtureManifestError(f"{label}.path must point to an .xsd file")
    if Path(rel_path).stem != message_def_id:
        raise FixtureManifestError(f"{label}.path stem must equal message_def_id")

    path = _validate_relative_path(rel_path, manifest_dir, manifest_dir, f"{label}.path")
    _reject_restricted_schema_terms(path)
    schema_bytes = path.read_bytes()
    schema_sha256 = sha256_hex(schema_bytes)
    source = _verify_schema_source(
        entry.get("source"),
        f"{label}.source",
        message_def_id=message_def_id,
        schema_sha256=schema_sha256,
    )
    root = _parse_xml(path)
    namespace, local = _split_xml_name(root.tag)
    if namespace != XML_SCHEMA_NS or local != "schema":
        raise FixtureManifestError(f"{path} root must be xs:schema")
    target_namespace = root.attrib.get("targetNamespace")
    expected_namespace = _namespace_for(message_def_id)
    if target_namespace != expected_namespace:
        raise FixtureManifestError(
            f"{path} targetNamespace is {target_namespace!r}, expected {expected_namespace!r}"
        )
    if root.attrib.get("elementFormDefault") != "qualified":
        raise FixtureManifestError(f"{path} elementFormDefault must be qualified")
    payload_root = _schema_payload_root(root, path)
    if payload_root != expected_payload_root:
        raise FixtureManifestError(
            f"{path} payload root is {payload_root!r}, expected {expected_payload_root!r}"
        )
    return {
        "path": rel_path,
        "message_def_id": message_def_id,
        "target_namespace": target_namespace,
        "payload_root": payload_root,
        "schema_only": schema_only_reason is not None,
        "schema_only_reason": schema_only_reason,
        "source": source,
        "sha256": schema_sha256,
    }


def _first_element_child(root: ET.Element, path: Path) -> ET.Element:
    children = [child for child in list(root) if isinstance(child.tag, str)]
    if len(children) != 1:
        raise FixtureManifestError(f"{path} Document must contain exactly one payload element")
    return children[0]


def _validate_fixture_xml_schema(schema_path: Path, fixture_path: Path, label: str) -> None:
    xmllint = shutil.which("xmllint")
    if xmllint is None:
        raise FixtureManifestError(
            "--validate-xml-schema requires xmllint on PATH for offline XSD validation"
        )
    completed = subprocess.run(
        [
            xmllint,
            "--noout",
            "--nonet",
            "--schema",
            str(schema_path),
            str(fixture_path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip()
        if detail:
            detail = ": " + detail[:4096]
        raise FixtureManifestError(f"{label} failed XML schema validation{detail}")


def verify_fixture_entry(
    entry: dict[str, Any],
    label: str,
    manifest_dir: Path,
    schemas_by_path: dict[str, dict[str, Any]],
    *,
    validate_xml_schema: bool,
) -> dict[str, Any]:
    """Verify one XML fixture manifest entry and return normalized metadata."""

    _reject_unknown_keys(entry, FIXTURE_KEYS, label)
    rel_path = _required_string(entry, "path", label)
    message_def_id = _require_message_def_id(
        _required_string(entry, "message_def_id", label),
        f"{label}.message_def_id",
    )
    expected_payload_root = _required_string(entry, "payload_root", label)
    schema_rel = _optional_string(entry, "schema", label)
    missing_schema_reason = _optional_string(entry, "missing_schema_reason", label)
    if schema_rel is not None and missing_schema_reason is not None:
        raise FixtureManifestError(f"{label} cannot set both schema and missing_schema_reason")
    if schema_rel is None and missing_schema_reason is None:
        raise FixtureManifestError(f"{label} must set schema or missing_schema_reason")

    path = _validate_relative_path(
        rel_path,
        manifest_dir,
        manifest_dir.parent,
        f"{label}.path",
    )
    root = _parse_xml(path)
    namespace, local = _split_xml_name(root.tag)
    if local != "Document":
        raise FixtureManifestError(f"{path} root element must be Document")
    namespace_message_id = _message_id_from_namespace(namespace, str(path))
    if namespace_message_id != message_def_id:
        raise FixtureManifestError(
            f"{path} namespace message id is {namespace_message_id!r}, expected {message_def_id!r}"
        )
    payload = _first_element_child(root, path)
    payload_namespace, payload_local = _split_xml_name(payload.tag)
    if payload_namespace != namespace:
        raise FixtureManifestError(f"{path} payload namespace must match Document namespace")
    if payload_local != expected_payload_root:
        raise FixtureManifestError(
            f"{path} payload root is {payload_local!r}, expected {expected_payload_root!r}"
        )

    schema_backed = False
    schema_validated = False
    if schema_rel is not None:
        schema = schemas_by_path.get(schema_rel)
        if schema is None:
            raise FixtureManifestError(f"{label}.schema references unknown schema {schema_rel}")
        if schema["message_def_id"] != message_def_id:
            raise FixtureManifestError(
                f"{label}.schema message id {schema['message_def_id']!r} "
                f"does not match fixture {message_def_id!r}"
            )
        if schema["payload_root"] != expected_payload_root:
            raise FixtureManifestError(
                f"{label}.schema payload root {schema['payload_root']!r} "
                f"does not match fixture {expected_payload_root!r}"
            )
        schema_backed = True
        if validate_xml_schema:
            schema_path = _validate_relative_path(
                schema_rel,
                manifest_dir,
                manifest_dir,
                f"{label}.schema",
            )
            _validate_fixture_xml_schema(schema_path, path, label)
            schema_validated = True

    return {
        "path": rel_path,
        "message_def_id": message_def_id,
        "payload_root": payload_local,
        "schema": schema_rel,
        "schema_backed": schema_backed,
        "schema_validated": schema_validated,
        "missing_schema_reason": missing_schema_reason,
        "sha256": sha256_hex(path.read_bytes()),
    }


def verify_profile_catalog(
    path: Path,
    schema_backed_message_ids: set[str],
) -> dict[str, Any]:
    """Verify profile catalog versions against schema-backed XML fixtures."""

    profiles, profile_catalog_sha256, profile_catalog_json_sha256 = _load_profile_catalog(path)
    versions: list[dict[str, Any]] = []
    missing_schema_versions: list[dict[str, str]] = []
    skipped_family_versions: list[dict[str, str]] = []
    seen_profile_ids: set[str] = set()
    seen_message_profiles: set[tuple[str, str, str]] = set()
    seen_profile_versions: set[tuple[str, str, str, str]] = set()

    for profile_offset, profile_raw in enumerate(profiles):
        profile_label = f"{path}.profiles[{profile_offset}]"
        profile = _require_object(profile_raw, profile_label)
        profile_id = _required_string(profile, "id", profile_label)
        if PROFILE_ID_RE.fullmatch(profile_id) is None:
            raise FixtureManifestError(
                f"{profile_label}.id must be a canonical lowercase profile id"
            )
        if profile_id in seen_profile_ids:
            raise FixtureManifestError(f"{profile_label}.id duplicates profile id {profile_id!r}")
        seen_profile_ids.add(profile_id)
        message_profiles = _require_array(
            profile.get("message_profiles"),
            f"{profile_label}.message_profiles",
        )
        if not message_profiles:
            raise FixtureManifestError(f"{profile_label}.message_profiles must not be empty")
        for message_offset, message_raw in enumerate(message_profiles):
            message_label = f"{profile_label}.message_profiles[{message_offset}]"
            message = _require_object(message_raw, message_label)
            message_type = _required_string(message, "message_type", message_label)
            if MESSAGE_TYPE_RE.fullmatch(message_type) is None:
                raise FixtureManifestError(
                    f"{message_label}.message_type must be lowercase ISO family id"
                )
            direction = _required_string(message, "direction", message_label)
            if direction not in PROFILE_DIRECTIONS:
                raise FixtureManifestError(
                    f"{message_label}.direction must be one of "
                    + ", ".join(sorted(PROFILE_DIRECTIONS))
                )
            message_key = (profile_id, message_type, direction)
            if message_key in seen_message_profiles:
                raise FixtureManifestError(
                    f"{message_label} duplicates profile/message/direction entry"
                )
            seen_message_profiles.add(message_key)
            raw_versions = _require_array(
                message.get("versions"),
                f"{message_label}.versions",
            )
            if not raw_versions:
                raise FixtureManifestError(f"{message_label}.versions must not be empty")
            for version_offset, raw_version in enumerate(raw_versions):
                version_label = f"{message_label}.versions[{version_offset}]"
                if not isinstance(raw_version, str) or not raw_version.strip():
                    raise FixtureManifestError(
                        f"{version_label} must be a non-empty string"
                    )
                if raw_version != raw_version.strip():
                    raise FixtureManifestError(
                        f"{version_label} must not have surrounding whitespace"
                    )
                if MESSAGE_DEF_ID_RE.fullmatch(raw_version) is None:
                    skipped_family_versions.append(
                        {
                            "profile_id": profile_id,
                            "message_type": message_type,
                            "direction": direction,
                            "version": raw_version,
                        }
                    )
                    continue
                if not raw_version.startswith(message_type + "."):
                    raise FixtureManifestError(
                        f"{version_label} {raw_version!r} does not match "
                        f"message_type {message_type!r}"
                    )
                key = (profile_id, message_type, direction, raw_version)
                if key in seen_profile_versions:
                    raise FixtureManifestError(
                        f"{version_label} duplicates profile/message/direction version "
                        f"{raw_version!r}"
                    )
                seen_profile_versions.add(key)
                schema_backed = raw_version in schema_backed_message_ids
                entry = {
                    "profile_id": profile_id,
                    "message_type": message_type,
                    "direction": direction,
                    "message_def_id": raw_version,
                    "schema_backed": schema_backed,
                }
                versions.append(entry)
                if not schema_backed:
                    missing_schema_versions.append(
                        {
                            "profile_id": profile_id,
                            "message_type": message_type,
                            "direction": direction,
                            "message_def_id": raw_version,
                        }
                    )

    return {
        "path": str(path),
        "sha256": profile_catalog_sha256,
        "catalog_json_sha256": profile_catalog_json_sha256,
        "profiles": len(profiles),
        "checked_versions": len(versions),
        "schema_backed_versions": sum(
            1 for version in versions if version["schema_backed"]
        ),
        "missing_schema_versions": missing_schema_versions,
        "skipped_family_versions": skipped_family_versions,
        "versions": versions,
    }


def verify_manifest(path: Path, args: argparse.Namespace) -> dict[str, Any]:
    """Verify the ISO fixture manifest and return a digest-bound summary."""

    manifest = _require_object(_load_json(path), str(path))
    _reject_unknown_keys(manifest, TOP_LEVEL_KEYS, str(path))
    if manifest.get("version") != MANIFEST_VERSION:
        raise FixtureManifestError(f"{path}.version must be {MANIFEST_VERSION}")
    manifest_dir = path.resolve().parent

    raw_schemas = _require_array(manifest.get("schemas"), f"{path}.schemas")
    raw_fixtures = _require_array(manifest.get("fixtures"), f"{path}.fixtures")
    schemas = [
        verify_schema_entry(
            _require_object(entry, f"{path}.schemas[{offset}]"),
            f"{path}.schemas[{offset}]",
            manifest_dir,
        )
        for offset, entry in enumerate(raw_schemas)
    ]
    schema_paths = [schema["path"] for schema in schemas]
    if len(schema_paths) != len(set(schema_paths)):
        raise FixtureManifestError(f"{path}.schemas contains duplicate schema paths")
    schema_ids = [schema["message_def_id"] for schema in schemas]
    if len(schema_ids) != len(set(schema_ids)):
        raise FixtureManifestError(f"{path}.schemas contains duplicate message_def_id values")
    schema_digests = [schema["sha256"] for schema in schemas]
    if len(schema_digests) != len(set(schema_digests)):
        raise FixtureManifestError(f"{path}.schemas contains duplicate schema SHA-256 values")
    schema_sources = [
        (
            schema["source"]["repository"],
            schema["source"]["commit"],
            schema["source"]["path"],
        )
        for schema in schemas
    ]
    if len(schema_sources) != len(set(schema_sources)):
        raise FixtureManifestError(f"{path}.schemas contains duplicate source provenance")
    schemas_by_path = {schema["path"]: schema for schema in schemas}

    fixtures = [
        verify_fixture_entry(
            _require_object(entry, f"{path}.fixtures[{offset}]"),
            f"{path}.fixtures[{offset}]",
            manifest_dir,
            schemas_by_path,
            validate_xml_schema=args.validate_xml_schema,
        )
        for offset, entry in enumerate(raw_fixtures)
    ]
    fixture_paths = [fixture["path"] for fixture in fixtures]
    if len(fixture_paths) != len(set(fixture_paths)):
        raise FixtureManifestError(f"{path}.fixtures contains duplicate fixture paths")
    fixture_digests = [fixture["sha256"] for fixture in fixtures]
    if len(fixture_digests) != len(set(fixture_digests)):
        raise FixtureManifestError(f"{path}.fixtures contains duplicate fixture SHA-256 values")
    backed_schema_paths = {fixture["schema"] for fixture in fixtures if fixture["schema"]}
    schema_only = [
        schema
        for schema in schemas
        if schema["path"] not in backed_schema_paths
    ]
    for schema in schema_only:
        if not schema["schema_only_reason"]:
            raise FixtureManifestError(
                f"{path} schema {schema['path']} has no fixture and no schema_only_reason"
            )
    missing_schema_fixtures = [
        fixture for fixture in fixtures if not fixture["schema_backed"]
    ]
    if args.require_schema_backed_fixtures and missing_schema_fixtures:
        first = missing_schema_fixtures[0]
        raise FixtureManifestError(
            f"{first['path']} is not schema-backed: {first['missing_schema_reason']}"
        )
    if args.require_fixture_for_schema and schema_only:
        first = schema_only[0]
        raise FixtureManifestError(
            f"{first['path']} has no standalone fixture: {first['schema_only_reason']}"
        )
    if args.require_profile_schema_backed_versions and args.profile_catalog is None:
        raise FixtureManifestError(
            "--require-profile-schema-backed-versions requires --profile-catalog"
        )

    schema_backed_message_ids = {
        fixture["message_def_id"] for fixture in fixtures if fixture["schema_backed"]
    }
    profile_catalog = (
        verify_profile_catalog(args.profile_catalog.resolve(), schema_backed_message_ids)
        if args.profile_catalog is not None
        else None
    )
    missing_profile_schema_versions = (
        profile_catalog["missing_schema_versions"] if profile_catalog else []
    )
    if args.require_profile_schema_backed_versions and missing_profile_schema_versions:
        first = missing_profile_schema_versions[0]
        raise FixtureManifestError(
            f"profile {first['profile_id']} version {first['message_def_id']} "
            "is not schema-backed by any checked-in XML fixture"
        )

    summary: dict[str, Any] = {
        "verified_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "manifest": str(path),
        "manifest_sha256": sha256_hex(path.read_bytes()),
        "verified_schemas": len(schemas),
        "verified_fixtures": len(fixtures),
        "schema_backed_fixtures": len(fixtures) - len(missing_schema_fixtures),
        "schema_validated_fixtures": sum(
            1 for fixture in fixtures if fixture["schema_validated"]
        ),
        "profile_checked_versions": (
            profile_catalog["checked_versions"] if profile_catalog else 0
        ),
        "profile_schema_backed_versions": (
            profile_catalog["schema_backed_versions"] if profile_catalog else 0
        ),
        "missing_schema_fixtures": [
            {
                "path": fixture["path"],
                "message_def_id": fixture["message_def_id"],
                "reason": fixture["missing_schema_reason"],
            }
            for fixture in missing_schema_fixtures
        ],
        "schema_only_entries": [
            {
                "path": schema["path"],
                "message_def_id": schema["message_def_id"],
                "reason": schema["schema_only_reason"],
            }
            for schema in schema_only
        ],
        "missing_profile_schema_versions": missing_profile_schema_versions,
        "schemas": schemas,
        "fixtures": fixtures,
        "profile_catalog": profile_catalog,
        "strict": {
            "require_schema_backed_fixtures": args.require_schema_backed_fixtures,
            "require_fixture_for_schema": args.require_fixture_for_schema,
            "require_profile_schema_backed_versions": (
                args.require_profile_schema_backed_versions
            ),
            "validate_xml_schema": args.validate_xml_schema,
        },
    }
    summary[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(summary))
    return summary


def run(args: argparse.Namespace) -> int:
    summary = verify_manifest(args.manifest.resolve(), args)
    text = json.dumps(summary, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        args.summary_out.parent.mkdir(parents=True, exist_ok=True)
        args.summary_out.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify ISO 20022 checked-in XSD/XML fixture manifest wiring."
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="Fixture manifest JSON to verify.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path to write the verification summary JSON.",
    )
    parser.add_argument(
        "--require-schema-backed-fixtures",
        action="store_true",
        help="Fail if any manifest fixture lacks a checked-in XSD package.",
    )
    parser.add_argument(
        "--require-fixture-for-schema",
        action="store_true",
        help="Fail if any checked-in XSD lacks a standalone XML fixture.",
    )
    parser.add_argument(
        "--profile-catalog",
        type=Path,
        default=None,
        help=(
            "Optional Rust profile catalog file containing DEFAULT_PROFILES_JSON "
            f"(default catalog: {DEFAULT_PROFILE_CATALOG})."
        ),
    )
    parser.add_argument(
        "--require-profile-schema-backed-versions",
        action="store_true",
        help=(
            "Fail if any concrete message version advertised by --profile-catalog "
            "lacks a schema-backed checked-in XML fixture."
        ),
    )
    parser.add_argument(
        "--validate-xml-schema",
        action="store_true",
        help="Validate every schema-backed XML fixture against its checked-in XSD with xmllint.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except FixtureManifestError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
