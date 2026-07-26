#!/usr/bin/env python3
"""Validate one closed OCI layout and write a deterministic OCI archive."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tarfile
from pathlib import Path, PurePosixPath

from release_artifact_contract import (
    ReleaseArtifactError,
    canonical_json_bytes,
    canonical_relative_path,
    exclusive_output_fd,
    load_json_object,
    parse_source_date_epoch,
    scan_inventory_paths,
    stable_hash_relative,
    stable_open_relative,
    stable_read_relative,
)


MAX_LAYOUT_FILE_SIZE = 4 * 1024 * 1024 * 1024
MAX_LAYOUT_SIZE = 8 * 1024 * 1024 * 1024
MAX_JSON_SIZE = 16 * 1024 * 1024
MAX_ANNOTATIONS = 64
OCI_INDEX_MEDIA_TYPE = "application/vnd.oci.image.index.v1+json"
OCI_MANIFEST_MEDIA_TYPE = "application/vnd.oci.image.manifest.v1+json"
OCI_CONFIG_MEDIA_TYPE = "application/vnd.oci.image.config.v1+json"
OCI_LAYER_MEDIA_TYPES = frozenset(
    {
        "application/vnd.oci.image.layer.v1.tar",
        "application/vnd.oci.image.layer.v1.tar+gzip",
        "application/vnd.oci.image.layer.v1.tar+zstd",
    }
)
_DIGEST_RE = re.compile(r"sha256:([0-9a-f]{64})")


def _bounded_text(value: object, label: str, *, maximum: int = 1024) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > maximum
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
    ):
        raise ReleaseArtifactError(f"{label} must be bounded control-free text")
    return value


def _annotations(value: object, label: str) -> dict[str, str]:
    if not isinstance(value, dict) or len(value) > MAX_ANNOTATIONS:
        raise ReleaseArtifactError(
            f"{label} must be an object with at most {MAX_ANNOTATIONS} entries"
        )
    result: dict[str, str] = {}
    for raw_key, raw_value in value.items():
        key = _bounded_text(raw_key, f"{label} key", maximum=256)
        result[key] = _bounded_text(
            raw_value,
            f"{label}[{key!r}]",
            maximum=4096,
        )
    return result


def _descriptor(
    value: object,
    label: str,
    *,
    allowed_media_types: frozenset[str],
    allow_annotations: bool,
    allow_platform: bool,
) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ReleaseArtifactError(f"{label} must be an object")
    allowed = {"mediaType", "digest", "size"}
    if allow_annotations:
        allowed.add("annotations")
    if allow_platform:
        allowed.add("platform")
    if not {"mediaType", "digest", "size"} <= set(value) or not set(value) <= allowed:
        raise ReleaseArtifactError(
            f"{label} fields must be a closed OCI descriptor schema"
        )
    media_type = value["mediaType"]
    if not isinstance(media_type, str) or media_type not in allowed_media_types:
        raise ReleaseArtifactError(f"{label} has an unsupported media type")
    digest = value["digest"]
    if not isinstance(digest, str) or _DIGEST_RE.fullmatch(digest) is None:
        raise ReleaseArtifactError(
            f"{label} digest must be canonical lowercase sha256"
        )
    size = value["size"]
    if isinstance(size, bool) or not isinstance(size, int) or size <= 0:
        raise ReleaseArtifactError(f"{label} size must be a positive integer")
    result: dict[str, object] = {
        "mediaType": media_type,
        "digest": digest,
        "size": size,
    }
    if "annotations" in value:
        result["annotations"] = _annotations(
            value["annotations"],
            f"{label}.annotations",
        )
    if "platform" in value:
        platform = value["platform"]
        if not isinstance(platform, dict):
            raise ReleaseArtifactError(f"{label}.platform must be an object")
        allowed_platform = {
            "architecture",
            "os",
            "os.features",
            "os.version",
            "variant",
        }
        if (
            not {"architecture", "os"} <= set(platform)
            or not set(platform) <= allowed_platform
        ):
            raise ReleaseArtifactError(
                f"{label}.platform has unsupported or missing fields"
            )
        normalized_platform: dict[str, object] = {
            "architecture": _bounded_text(
                platform["architecture"],
                f"{label}.platform.architecture",
                maximum=64,
            ),
            "os": _bounded_text(
                platform["os"],
                f"{label}.platform.os",
                maximum=64,
            ),
        }
        for name in ("os.version", "variant"):
            if name in platform:
                normalized_platform[name] = _bounded_text(
                    platform[name],
                    f"{label}.platform.{name}",
                    maximum=128,
                )
        if "os.features" in platform:
            features = platform["os.features"]
            if (
                not isinstance(features, list)
                or len(features) > 64
                or not all(isinstance(item, str) for item in features)
            ):
                raise ReleaseArtifactError(
                    f"{label}.platform.os.features must be a bounded string array"
                )
            normalized_platform["os.features"] = [
                _bounded_text(
                    item,
                    f"{label}.platform.os.features",
                    maximum=128,
                )
                for item in features
            ]
        result["platform"] = normalized_platform
    return result


def _tar_info(
    name: str,
    *,
    epoch: int,
    size: int = 0,
    directory: bool,
) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mode = 0o755 if directory else 0o644
    info.mtime = epoch
    info.size = size
    info.pax_headers = {}
    if directory:
        info.type = tarfile.DIRTYPE
    return info


def _directories(files: list[str]) -> set[str]:
    result: set[str] = set()
    for relative in files:
        parts = PurePosixPath(relative).parts
        for length in range(1, len(parts)):
            result.add(PurePosixPath(*parts[:length]).as_posix())
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--layout-root", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--source-date-epoch", required=True)
    parser.add_argument("--expected-ref-name", required=True)
    parser.add_argument("--expected-os", required=True)
    parser.add_argument("--expected-architecture", required=True)
    args = parser.parse_args()

    try:
        epoch = parse_source_date_epoch(args.source_date_epoch)
        expected_ref_name = _bounded_text(
            args.expected_ref_name,
            "expected OCI reference name",
            maximum=512,
        )
        expected_os = _bounded_text(
            args.expected_os,
            "expected OCI operating system",
            maximum=64,
        )
        expected_architecture = _bounded_text(
            args.expected_architecture,
            "expected OCI architecture",
            maximum=64,
        )
        layout = Path(args.layout_root)
        output = Path(args.output)
        layout_absolute = Path(os.path.abspath(layout))
        output_absolute = Path(os.path.abspath(output))
        if os.path.commonpath((layout_absolute, output_absolute)) == str(
            layout_absolute
        ):
            raise ReleaseArtifactError(
                "OCI archive output must be outside the source layout"
            )

        files = scan_inventory_paths(layout)
        if "index.json" not in files or "oci-layout" not in files:
            raise ReleaseArtifactError(
                "OCI layout must contain index.json and oci-layout"
            )
        captured: dict[str, object] = {}
        total_size = 0
        for relative in files:
            info = stable_hash_relative(
                layout,
                relative,
                max_size=MAX_LAYOUT_FILE_SIZE,
            )
            total_size += info.size
            if total_size > MAX_LAYOUT_SIZE:
                raise ReleaseArtifactError(
                    f"OCI layout exceeds {MAX_LAYOUT_SIZE} bytes"
                )
            captured[relative] = info

        def read_json(relative: str, label: str) -> dict[str, object]:
            info, payload = stable_read_relative(
                layout,
                canonical_relative_path(relative),
                max_size=MAX_JSON_SIZE,
                return_payload=True,
            )
            assert payload is not None
            if info != captured[relative]:
                raise ReleaseArtifactError(
                    f"{label} changed after the OCI layout was captured"
                )
            return load_json_object(payload, label)

        layout_marker = read_json("oci-layout", "OCI layout marker")
        if layout_marker != {"imageLayoutVersion": "1.0.0"}:
            raise ReleaseArtifactError("unsupported OCI layout marker")

        index = read_json("index.json", "OCI image index")
        if (
            not {"schemaVersion", "mediaType", "manifests"} <= set(index)
            or not set(index) <= {
                "schemaVersion",
                "mediaType",
                "manifests",
                "annotations",
            }
            or index["schemaVersion"] != 2
            or index["mediaType"] != OCI_INDEX_MEDIA_TYPE
        ):
            raise ReleaseArtifactError("OCI image index schema is not supported")
        if "annotations" in index:
            _annotations(index["annotations"], "OCI image index annotations")
        raw_manifests = index["manifests"]
        if not isinstance(raw_manifests, list) or len(raw_manifests) != 1:
            raise ReleaseArtifactError(
                "OCI image index must contain exactly one image manifest"
            )
        manifest_descriptor = _descriptor(
            raw_manifests[0],
            "OCI image manifest descriptor",
            allowed_media_types=frozenset({OCI_MANIFEST_MEDIA_TYPE}),
            allow_annotations=True,
            allow_platform=True,
        )
        annotations = manifest_descriptor.get("annotations")
        if (
            not isinstance(annotations, dict)
            or annotations.get("org.opencontainers.image.ref.name")
            != expected_ref_name
        ):
            raise ReleaseArtifactError(
                "OCI image manifest reference annotation does not match "
                "--expected-ref-name"
            )
        descriptor_platform = manifest_descriptor.get("platform")
        if descriptor_platform is not None and (
            not isinstance(descriptor_platform, dict)
            or descriptor_platform.get("os") != expected_os
            or descriptor_platform.get("architecture") != expected_architecture
        ):
            raise ReleaseArtifactError(
                "OCI image manifest platform does not match the expected platform"
            )

        def resolve_descriptor(
            descriptor: dict[str, object],
            label: str,
        ) -> str:
            digest = str(descriptor["digest"])
            digest_hex = digest.removeprefix("sha256:")
            relative = f"blobs/sha256/{digest_hex}"
            if relative not in captured:
                raise ReleaseArtifactError(
                    f"{label} references a missing OCI blob"
                )
            info = captured[relative]
            if info.sha256 != digest_hex or info.size != descriptor["size"]:
                raise ReleaseArtifactError(
                    f"{label} digest or size does not match its OCI blob"
                )
            return relative

        manifest_relative = resolve_descriptor(
            manifest_descriptor,
            "OCI image manifest descriptor",
        )
        manifest = read_json(manifest_relative, "OCI image manifest")
        if (
            not {"schemaVersion", "mediaType", "config", "layers"} <= set(manifest)
            or not set(manifest) <= {
                "schemaVersion",
                "mediaType",
                "config",
                "layers",
                "annotations",
            }
            or manifest["schemaVersion"] != 2
            or manifest["mediaType"] != OCI_MANIFEST_MEDIA_TYPE
        ):
            raise ReleaseArtifactError("OCI image manifest schema is not supported")
        if "annotations" in manifest:
            _annotations(manifest["annotations"], "OCI image manifest annotations")
        config_descriptor = _descriptor(
            manifest["config"],
            "OCI image config descriptor",
            allowed_media_types=frozenset({OCI_CONFIG_MEDIA_TYPE}),
            allow_annotations=True,
            allow_platform=False,
        )
        raw_layers = manifest["layers"]
        if not isinstance(raw_layers, list) or not raw_layers:
            raise ReleaseArtifactError(
                "OCI image manifest must contain at least one layer"
            )
        if len(raw_layers) > 1024:
            raise ReleaseArtifactError("OCI image manifest has too many layers")
        layer_descriptors = [
            _descriptor(
                raw,
                f"OCI image layer descriptor {index}",
                allowed_media_types=OCI_LAYER_MEDIA_TYPES,
                allow_annotations=True,
                allow_platform=False,
            )
            for index, raw in enumerate(raw_layers)
        ]
        config_relative = resolve_descriptor(
            config_descriptor,
            "OCI image config descriptor",
        )
        config = read_json(config_relative, "OCI image config")
        if (
            config.get("os") != expected_os
            or config.get("architecture") != expected_architecture
        ):
            raise ReleaseArtifactError(
                "OCI image config does not match the expected platform"
            )
        referenced_blobs = {manifest_relative, config_relative}
        for index, descriptor in enumerate(layer_descriptors):
            referenced_blobs.add(
                resolve_descriptor(
                    descriptor,
                    f"OCI image layer descriptor {index}",
                )
            )
        actual_blobs = {
            relative
            for relative in files
            if relative.startswith("blobs/")
        }
        if actual_blobs != referenced_blobs:
            raise ReleaseArtifactError(
                "OCI layout blob inventory is not exactly the reachable image graph"
            )
        if set(files) != {"index.json", "oci-layout"} | actual_blobs:
            raise ReleaseArtifactError(
                "OCI layout contains files outside the closed image inventory"
            )

        layout_rows = [
            {
                "path": relative,
                "sha256": captured[relative].sha256,
                "size": captured[relative].size,
            }
            for relative in files
        ]
        layout_sha256 = hashlib.sha256(
            canonical_json_bytes(layout_rows)
        ).hexdigest()

        entries = sorted(_directories(files) | set(files))
        with exclusive_output_fd(output, mode=0o644) as output_fd:
            with os.fdopen(os.dup(output_fd), "wb", closefd=True) as output_file:
                with tarfile.open(
                    fileobj=output_file,
                    mode="w|",
                    format=tarfile.USTAR_FORMAT,
                ) as archive:
                    for relative in entries:
                        if relative not in captured:
                            archive.addfile(
                                _tar_info(
                                    relative,
                                    epoch=epoch,
                                    directory=True,
                                )
                            )
                            continue
                        before = captured[relative]
                        with stable_open_relative(
                            layout,
                            relative,
                            expected=before,
                        ) as source_fd:
                            with os.fdopen(
                                os.dup(source_fd),
                                "rb",
                                closefd=True,
                            ) as source_file:
                                archive.addfile(
                                    _tar_info(
                                        relative,
                                        epoch=epoch,
                                        size=before.size,
                                        directory=False,
                                    ),
                                    source_file,
                                )
                        if (
                            stable_hash_relative(
                                layout,
                                relative,
                                max_size=MAX_LAYOUT_FILE_SIZE,
                            )
                            != before
                        ):
                            raise ReleaseArtifactError(
                                f"OCI layout entry {relative!r} changed "
                                "during archive creation"
                            )
            if scan_inventory_paths(layout) != files:
                raise ReleaseArtifactError(
                    "OCI layout inventory changed during archive creation"
                )

        print(
            json.dumps(
                {
                    "config_digest": config_descriptor["digest"],
                    "file_count": len(files),
                    "layout_sha256": layout_sha256,
                    "manifest_digest": manifest_descriptor["digest"],
                },
                sort_keys=True,
                separators=(",", ":"),
                ensure_ascii=True,
            )
        )
    except (
        OSError,
        ReleaseArtifactError,
        tarfile.TarError,
    ) as exc:
        print(f"release OCI archive error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
