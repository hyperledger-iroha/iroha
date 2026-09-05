#!/usr/bin/env python3
"""Validate explicit ownership of foundational Cargo features.

Prerequisites: Python 3.11+, or Python 3.9/3.10 with the repository's pinned
``tomli`` dependency installed. The check is read-only and requires no
environment variables.
"""

from __future__ import annotations

import argparse
from pathlib import Path, PurePosixPath
from typing import Any, Iterable

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - exercised on Python <3.11
    import tomli as tomllib


FOUNDATIONAL_DEPENDENCIES = frozenset(
    {
        "iroha_core",
        "iroha_crypto",
        "iroha_data_model",
        "iroha_torii",
        "norito",
    }
)

# These aggregates are the stable ownership boundary for shipping builds. The
# implementation features remain available for focused tests and compatibility
# while callers depend on the smaller vocabulary below.
EXPECTED_FEATURES: dict[str, dict[str, tuple[str, ...]]] = {
    "norito": {
        "default": ("node-codec",),
        "base-codec": ("json",),
        "node-codec": (
            "base-codec",
            "compression",
            "columnar",
            "json-std-io",
            "simd-accel",
            "crc-key-hash",
            "simdutf8-validate",
            "parallel-stage1-rayon",
        ),
        "columnar": (),
        "compression": (),
        "crc-key-hash": (),
        "gpu-compression": ("compression", "dep:gpuzstd_metal"),
        "json": (),
        "json-std-io": (),
        "parallel-stage1": (),
        "parallel-stage1-rayon": ("rayon", "parallel-stage1"),
        "rayon": ("dep:rayon",),
        "simd-accel": (),
        "simdutf8": ("dep:simdutf8",),
        "simdutf8-validate": ("simdutf8",),
    },
    "iroha_crypto": {
        "default": ("node-crypto",),
        "application": ("rand", "json", "ecc-batch", "bfv-accel", "pqc"),
        "bfv-accel": (),
        "bls": (
            "dep:blst",
            "dep:blstrs",
            "dep:group",
            "dep:pairing",
            "dep:subtle",
            "dep:w3f-bls",
        ),
        "consensus": ("rand", "bls"),
        "ecc-batch": ("dep:subtle", "ed25519-dalek/batch"),
        "gost": (
            "dep:once_cell",
            "dep:num-bigint",
            "dep:num-traits",
            "dep:streebog",
            "dep:crypto-bigint",
            "dep:subtle",
        ),
        "json": ("norito/json", "mv"),
        "mv": ("dep:mv",),
        "node-crypto": ("application", "consensus", "gost", "sm", "rayon"),
        "pqc": (
            "dep:pqcrypto-traits",
            "dep:pqcrypto-mldsa",
            "dep:soranet_pq",
            "dep:subtle",
        ),
        "rand": (),
        "rayon": ("dep:rayon",),
        "sm": (
            "sm-ccm",
            "dep:sm2",
            "dep:sm3",
            "dep:sm4",
            "dep:rfc6979",
            "dep:sm4_gcm",
            "sm-neon",
        ),
        "sm-ccm": (),
        "sm-neon": ("dep:sm4-neon", "dep:sm3-neon"),
    },
    "iroha_data_model": {
        "default": ("application-model",),
        "application-model": ("governance", "json", "pqc", "bls", "gost", "sm"),
        "bls": ("iroha_crypto/bls",),
        "bridge": (),
        "gost": ("iroha_crypto/gost",),
        "governance": (),
        "http": (),
        "json": (
            "iroha_version/json",
            "iroha_primitives/json",
            "iroha_crypto/json",
            "norito/json",
            "norito_derive",
            "mv",
        ),
        "mv": ("dep:mv",),
        "norito_derive": ("dep:norito_derive",),
        "pqc": ("iroha_crypto/pqc", "sorafs_manifest/pqc"),
        "privacy-exact12-conformance": (),
        "sm": ("iroha_crypto/sm",),
        "transparent_api": ("iroha_data_model_derive/transparent_api",),
    },
    "iroha_core": {
        "default": ("node", "simd"),
        "runtime": ("json", "bls", "proofs-halo2"),
        "node": (
            "runtime",
            "proofs-stark",
            "app_api",
            "gost",
            "sm",
            "telemetry",
            "zk-preverify",
        ),
        "proofs-halo2": ("zk-halo2", "zk-halo2-ipa", "zk-ipa-native", "circuit-params"),
        "proofs-stark": ("zk-stark",),
        "proofs-full": ("proofs-halo2", "proofs-stark"),
        "app_api": (),
        "bls": ("iroha_crypto/bls", "iroha_data_model/bls"),
        "circuit-params": ("halo2_proofs/circuit-params",),
        "expensive-telemetry": (
            "telemetry",
            "iroha_telemetry/metric-instrumentation",
        ),
        "gost": (
            "iroha_config/gost",
            "iroha_crypto/gost",
            "iroha_data_model/gost",
        ),
        "json": (
            "iroha_data_model/json",
            "iroha_crypto/json",
            "iroha_primitives/json",
        ),
        "sm": (
            "iroha_config/sm",
            "iroha_crypto/sm",
            "iroha_data_model/sm",
        ),
        "telemetry": ("ivm/telemetry",),
        "simd": ("iroha_primitives/simd-accel",),
        "zk-halo2": ("dep:kaigi_zk",),
        "zk-halo2-ipa": ("zk-ipa-native",),
        "zk-ipa-native": (),
        "zk-preverify": (),
        "zk-stark": (),
    },
    "iroha_torii": {
        "default": ("node-api",),
        "node-api": (
            "app_api",
            "app_api_https",
            "transparent_api",
            "app_api_wss",
            "connect",
            "push",
            "telemetry",
            "schema",
            "circuit-params",
            "proofs-full",
            "ipa-commitment",
            "zk-verify-batch",
            "gost",
            "sm",
        ),
        "proofs-halo2": ("zk-halo2", "zk-halo2-ipa"),
        "proofs-stark": ("zk-stark",),
        "proofs-full": ("proofs-halo2", "proofs-stark"),
        "app_api": (),
        "app_api_https": (),
        "app_api_wss": ("dep:tokio-tungstenite",),
        "circuit-params": (
            "iroha_core/circuit-params",
            "halo2_proofs/circuit-params",
        ),
        "connect": (),
        "telemetry": (
            "iroha_telemetry",
            "iroha_core/telemetry",
            "iroha_futures/telemetry",
            "dep:prometheus",
        ),
        "gost": (
            "iroha_config/gost",
            "iroha_core/gost",
            "iroha_crypto/gost",
            "iroha_data_model/gost",
        ),
        "sm": (
            "iroha_config/sm",
            "iroha_core/sm",
            "iroha_crypto/sm",
            "iroha_data_model/sm",
            "iroha_telemetry?/sm",
        ),
        "ipa-commitment": ("dep:iroha_zkp_halo2",),
        "iroha_schema": ("dep:iroha_schema",),
        "iroha_schema_gen": ("dep:iroha_schema_gen",),
        "iroha_telemetry": ("dep:iroha_telemetry",),
        "push": ("app_api",),
        "schema": ("iroha_schema", "iroha_schema_gen"),
        "transparent_api": ("iroha_data_model/transparent_api",),
        "zk-halo2": ("iroha_core/zk-halo2",),
        "zk-halo2-ipa": ("iroha_core/zk-halo2-ipa",),
        "zk-stark": ("iroha_core/zk-stark",),
        "zk-verify-batch": ("dep:iroha_zkp_halo2", "app_api"),
    },
    "irohad": {
        "default": ("daemon", "iroha_core/simd"),
        "daemon": (
            "telemetry",
            "schema-endpoint",
            "gost",
            "sm",
            "dag-recovery-verify",
            "expensive-telemetry",
            "iroha_core/node",
            "iroha_torii/node-api",
            "iroha_crypto/consensus",
            "iroha_crypto/pqc",
            "iroha_data_model/json",
            "norito/node-codec",
        ),
        "telemetry": (
            "iroha_telemetry",
            "iroha_telemetry/event-exporter",
            "iroha_core/telemetry",
            "iroha_torii/telemetry",
        ),
        "expensive-telemetry": ("telemetry", "iroha_core/expensive-telemetry"),
        "gost": (
            "iroha_core/gost",
            "iroha_crypto/gost",
            "iroha_data_model/gost",
            "iroha_config/gost",
            "iroha_torii/gost",
        ),
        "sm": (
            "iroha_core/sm",
            "iroha_crypto/sm",
            "iroha_data_model/sm",
            "iroha_config/sm",
            "iroha_genesis/sm",
            "iroha_telemetry/sm",
            "iroha_torii/sm",
        ),
        "dag-recovery-verify": ("dep:nonzero_ext",),
        "iroha_telemetry": ("dep:iroha_telemetry",),
        "schema-endpoint": ("iroha_torii/schema",),
    },
    "iroha_cli": {
        "default": ("cli",),
        "cli": (
            "bridge",
            "offline-visual-codecs",
            "iroha_core/node",
            "iroha_crypto/consensus",
            "iroha_data_model/json",
            "norito/node-codec",
        ),
        "bridge": (),
        "offline-visual-codecs": ("dep:image",),
    },
    "iroha": {
        "default": ("tls-rustls-native-roots", "gost", "sm"),
        "gost": ("iroha_config/gost", "iroha_crypto/gost", "iroha_data_model/gost"),
        "sm": ("iroha_config/sm", "iroha_crypto/sm", "iroha_data_model/sm"),
        "tls-rustls-native-roots": (
            "reqwest/rustls-tls-native-roots",
            "tokio-tungstenite/rustls-tls-native-roots",
            "tungstenite/rustls-tls-native-roots",
        ),
    },
    "iroha_config": {
        "default": ("gost", "sm"),
        "gost": ("iroha_crypto/gost", "iroha_data_model/gost"),
        "sm": ("iroha_crypto/sm", "iroha_data_model/sm"),
        "sm-ffi-openssl": ("sm", "iroha_crypto/sm-ffi-openssl"),
    },
    "iroha_genesis": {
        "default": ("sm",),
        "sm": ("iroha_config/sm", "iroha_crypto/sm", "iroha_data_model/sm"),
        "sm-ffi-openssl": (
            "sm",
            "iroha_config/sm-ffi-openssl",
            "iroha_crypto/sm-ffi-openssl",
        ),
    },
    "iroha_telemetry": {
        "default": (),
        "event-exporter": (
            "dep:chrono",
            "dep:futures",
            "dep:iroha_logger",
            "dep:tokio",
            "dep:tokio-tungstenite",
            "dep:url",
            "tokio/fs",
            "tokio/io-util",
            "tokio/macros",
            "tokio/net",
            "tokio/rt",
            "tokio/sync",
            "tokio/time",
        ),
        "metric-instrumentation": (
            "iroha_telemetry_derive/metric-instrumentation",
        ),
        "sm": ("iroha_config/sm", "iroha_data_model/sm"),
    },
    "ivm": {
        "telemetry": (),
    },
    "iroha_primitives": {
        "default": ("json", "simd-accel"),
        "json": (),
        "simd-accel": (),
    },
    "iroha_kagami": {
        "default": ("gost", "sm"),
        "gost": ("iroha_config/gost", "iroha_crypto/gost", "iroha_data_model/gost"),
        "sm": (
            "iroha_config/sm",
            "iroha_crypto/sm",
            "iroha_data_model/sm",
            "iroha_genesis/sm",
        ),
    },
    "iroha_zkp_halo2": {
        "default": ("full", "parallel"),
        "full": (
            "model-primitives",
            "dep:tiny-keccak",
            "dep:thiserror",
            "dep:norito",
            "dep:hex",
            "dep:parking_lot",
        ),
        "model-primitives": (),
        "parallel": ("full", "dep:rayon"),
    },
    "iroha_executor": {
        "default": ("bridge",),
        "bridge": (),
    },
}


# These features are deliberately absent from their defining package's local
# default, but are selected by another shipping package for that consumer's
# context. They remain exact-pinned in ``EXPECTED_FEATURES`` and must not drift
# into the defining package's default closure without an explicit policy change.
CONTEXTUAL_SHIPPING_FEATURES: dict[str, tuple[str, ...]] = {
    "norito": (),
    "iroha_crypto": (),
    "iroha_data_model": (
        "bridge",
        "http",
        "privacy-exact12-conformance",
        "transparent_api",
    ),
    "iroha_core": ("expensive-telemetry",),
    "iroha_torii": (),
    "irohad": (),
    "iroha_cli": (),
    "iroha": (),
    "iroha_config": (),
    "iroha_genesis": (),
    "iroha_telemetry": ("event-exporter", "metric-instrumentation", "sm"),
    "ivm": ("telemetry",),
    "iroha_primitives": (),
    "iroha_kagami": (),
    "iroha_zkp_halo2": (),
    "iroha_executor": (),
}


# Every remaining Cargo-visible feature that is not reachable from the local
# default is an explicit non-shipping surface or qualified alternative. This
# and ``CONTEXTUAL_SHIPPING_FEATURES`` form a closed inventory: adding an
# optional dependency may create an implicit Cargo feature, and that new
# feature must receive exactly one classification.
EXPLICIT_OPT_IN_FEATURES: dict[str, tuple[str, ...]] = {
    "norito": (
        "adaptive-telemetry",
        "adaptive-telemetry-log",
        "bench",
        "bench-internal",
        "codec-gpu",
        "codec-gpu-cuda",
        "codec-gpu-metal",
        "cuda-crc64",
        "cuda-stage1",
        "dev-tools",
        "gpu-compression",
        "metal-crc64",
        "metal-stage1",
        "schema-structural",
        "stage1-validate",
        "streaming-fixed-point-dct",
        "streaming-neural-filter",
    ),
    "iroha_crypto": (
        "crypto-parity-tests",
        "dev-tools",
        "ffi_export",
        "iroha_ffi",
        "sm-ffi-openssl",
        "sm-neon-force",
    ),
    "iroha_data_model": (
        "bench",
        "dev-tools",
        "fault_injection",
        "ffi_export",
        "ids_projection",
        "iroha_ffi",
        "test-fixtures",
        "trybuild-tests",
    ),
    "iroha_core": (
        "bench",
        "cuda",
        "dev-tests",
        "dev-tools",
        "fastpq-gpu",
        "goldilocks_backend",
        "halo2-dev-tests",
        "ids_projection",
        "iroha-core-tests",
        "kagemusha-real-proof-harness",
        "kaigi_privacy_mocks",
        "privacy-release-evidence",
        "profiling",
        "proofs-full",
        "quic",
        "sm-ffi-openssl",
        "sumeragi-main-loop-tests",
        "test-network-native-amx-fault-injection",
        "test-network-parliament-signers",
        "test-network-private-settlement-evidence",
        "zk-proof-tags",
        "zk-tests",
    ),
    "iroha_torii": (
        "bench",
        "goldilocks_backend",
        "halo2-dev-tests",
        "pprof",
        "profiling",
        "test-network-private-settlement-route-control",
        "ws_integration_tests",
        "zk-proof-tags",
        "zk-tests",
    ),
    "irohad": (
        "accel-cuda",
        "accel-metal",
        "beep",
        "dev-telemetry",
        "dev-tools",
        "external-software-signer-bin",
        "fastpq-gpu",
        "profiling-endpoint",
        "sm-ffi-openssl",
        "telegram-alerts",
        "test-network-message-control",
        "test-network-parliament-signers",
        "zk-stark",
    ),
    "iroha_cli": ("cli_integration_harness", "dev-tools", "ids_projection"),
    "iroha": (
        "ids_projection",
        "test-fixtures",
        "test-network-private-settlement-evidence",
        "tls-native",
        "tls-native-vendored",
        "tls-rustls-webpki-roots",
    ),
    "iroha_config": ("sm-ffi-openssl",),
    "iroha_genesis": ("dev-tools", "sm-ffi-openssl"),
    "iroha_telemetry": (
        "dev-telemetry",
        "otel-exporter",
        "telegram",
    ),
    "ivm": (
        "beep",
        "bench",
        "cuda",
        "dev-tools",
        "goldilocks_backend",
        "ivm_vrf_tests",
        "ivm_zk_tests",
        "metal",
    ),
    "iroha_primitives": ("bench", "ffi_export", "iroha_ffi", "trybuild-tests"),
    "iroha_kagami": ("dev-tools",),
    "iroha_zkp_halo2": ("bench", "goldilocks_backend", "schema-structural"),
    "iroha_executor": ("debug",),
}


# Explicit opt-ins must not leak into dependencies compiled for ordinary member
# targets. These exact consumers are non-shipping fixture/tool contexts whose
# normal dependencies intentionally need the named surface. Keeping this as a
# closed, occurrence-checked inventory makes every exception reviewable.
NONSHIPPING_EXPLICIT_OPT_IN_DEPENDENCY_ALLOWLIST: tuple[
    tuple[str, str, str], ...
] = (
    ("executor_custom_data_model", "iroha_data_model", "fault_injection"),
    ("integration_tests", "iroha_torii", "ws_integration_tests"),
    ("xtask", "iroha", "test-fixtures"),
    ("xtask", "iroha_torii", "profiling"),
    ("xtask", "iroha_torii", "ws_integration_tests"),
    ("xtask", "norito", "bench-internal"),
)


def _load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as source:
        return tomllib.load(source)


def _dependency_tables(document: dict[str, Any]) -> Iterable[tuple[str, dict[str, Any]]]:
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = document.get(section)
        if isinstance(table, dict):
            yield section, table

    targets = document.get("target", {})
    if not isinstance(targets, dict):
        return
    for target_name, target in targets.items():
        if not isinstance(target, dict):
            continue
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            table = target.get(section)
            if isinstance(table, dict):
                yield f"target.{target_name}.{section}", table


def cargo_visible_features(document: dict[str, Any]) -> dict[str, tuple[str, ...]]:
    """Return explicit and implicit Cargo features declared by one manifest.

    Cargo creates a same-named feature for an optional dependency unless an
    explicit ``dep:<name>`` reference suppresses that shorthand. Keeping the
    implicit entries in this inventory prevents a newly optional dependency
    from silently creating an unreviewed opt-in feature.
    """

    raw_features = document.get("features", {})
    if not isinstance(raw_features, dict):
        raise ValueError("missing [features] table")

    features: dict[str, tuple[str, ...]] = {}
    for feature, raw_members in raw_features.items():
        if not isinstance(feature, str) or not feature:
            raise ValueError("feature names must be non-empty strings")
        if not isinstance(raw_members, list) or not all(
            isinstance(member, str) and member for member in raw_members
        ):
            raise ValueError(f"feature `{feature}` members must be non-empty strings")
        features[feature] = tuple(raw_members)

    explicit_dependency_references = {
        member.removeprefix("dep:")
        for members in features.values()
        for member in members
        if member.startswith("dep:")
    }
    optional_dependencies: set[str] = set()
    for _section, dependencies in _dependency_tables(document):
        optional_dependencies.update(
            dependency
            for dependency, specification in dependencies.items()
            if isinstance(specification, dict)
            and specification.get("optional") is True
        )
    for dependency in sorted(optional_dependencies - explicit_dependency_references):
        features.setdefault(dependency, (f"dep:{dependency}",))

    return features


def local_default_feature_closure(
    features: dict[str, tuple[str, ...]],
) -> frozenset[str]:
    """Return local Cargo features reachable from the local ``default`` feature.

    A non-weak dependency forward (``crate/feature``) activates an optional
    dependency just like its same-named implicit feature. Weak forwards
    (``crate?/feature``) remain conditional and terminal, as do explicit
    optional-dependency activations (``dep:crate``). Bare names are traversed
    only when they name another local explicit or implicit feature.
    """

    if "default" not in features:
        return frozenset()

    reachable: set[str] = set()
    pending = ["default"]
    while pending:
        feature = pending.pop()
        if feature in reachable:
            continue
        reachable.add(feature)
        for member in features[feature]:
            if (
                member in features
                and not member.startswith("dep:")
                and "/" not in member
            ):
                pending.append(member)
                continue
            dependency, separator, _dependency_feature = member.partition("/")
            if (
                separator
                and not dependency.endswith("?")
                and features.get(dependency) == (f"dep:{dependency}",)
            ):
                pending.append(dependency)
    return frozenset(reachable)


def _member_manifest(root: Path, member: str | Path) -> Path:
    path = root / member
    return path if path.name == "Cargo.toml" else path / "Cargo.toml"


def _workspace_patterns(workspace: dict[str, Any], field: str) -> list[str]:
    raw_patterns = workspace.get(field, [])
    if not isinstance(raw_patterns, list):
        raise ValueError(f"`workspace.{field}` must be an array")

    patterns: list[str] = []
    for raw_pattern in raw_patterns:
        if not isinstance(raw_pattern, str):
            raise ValueError(f"`workspace.{field}` paths must be strings")
        normalized = raw_pattern.replace("\\", "/")
        path = PurePosixPath(normalized)
        if (
            not normalized
            or path.is_absolute()
            or ".." in path.parts
            or path.as_posix() != normalized
        ):
            raise ValueError(
                f"`workspace.{field}` contains an invalid path: {raw_pattern!r}"
            )
        patterns.append(normalized)
    return patterns


def _expand_member_patterns(
    root: Path,
    patterns: Iterable[str],
    *,
    require_match: bool,
) -> set[Path]:
    manifests: set[Path] = set()
    root = root.resolve()
    for pattern in patterns:
        matched_manifests: set[Path] = set()
        for candidate in root.glob(pattern):
            manifest = _member_manifest(root, candidate)
            if not manifest.is_file():
                continue
            resolved = manifest.resolve()
            try:
                resolved.relative_to(root)
            except ValueError as error:
                raise ValueError(
                    f"workspace member resolves outside the repository: {manifest}"
                ) from error
            matched_manifests.add(resolved)
        if require_match and not matched_manifests:
            raise ValueError(f"workspace member pattern matches no manifests: {pattern}")
        manifests.update(matched_manifests)
    return manifests


def workspace_member_manifests(
    root: Path, workspace: dict[str, Any]
) -> tuple[Path, ...]:
    """Expand, exclude, and deduplicate every Cargo workspace member manifest."""

    members = _workspace_patterns(workspace, "members")
    if not members:
        raise ValueError("`workspace.members` must contain at least one path")
    excludes = _workspace_patterns(workspace, "exclude")
    included = _expand_member_patterns(root, members, require_match=True)
    excluded = _expand_member_patterns(root, excludes, require_match=False)
    return tuple(sorted(included - excluded))


def _dependency_target_and_features(
    dependency: str,
    specification: Any,
    workspace_dependencies: dict[str, Any],
) -> tuple[str, frozenset[str]]:
    """Resolve a dependency alias and its locally selected Cargo features."""

    inherited: Any = None
    if isinstance(specification, dict) and specification.get("workspace") is True:
        inherited = workspace_dependencies.get(dependency)

    target_package = dependency
    selected_features: set[str] = set()
    for candidate in (inherited, specification):
        if not isinstance(candidate, dict):
            continue
        package = candidate.get("package")
        if isinstance(package, str) and package:
            target_package = package
        raw_features = candidate.get("features", [])
        if isinstance(raw_features, list):
            selected_features.update(
                feature
                for feature in raw_features
                if isinstance(feature, str) and feature
            )

    return target_package, frozenset(selected_features)


def _non_dev_explicit_opt_in_dependency_selections(
    document: dict[str, Any], workspace_dependencies: dict[str, Any]
) -> tuple[tuple[str, str, str, str], ...]:
    """Return non-dev dependency selections of classified explicit opt-ins.

    Rows contain the dependency table, local dependency key, resolved package,
    and selected feature. Workspace dependency features are inherited when the
    member uses ``workspace = true``; ``package`` keys resolve aliases.
    """

    selections: list[tuple[str, str, str, str]] = []
    for section, dependencies in _dependency_tables(document):
        if section.rsplit(".", 1)[-1] == "dev-dependencies":
            continue
        for dependency, specification in dependencies.items():
            target_package, selected_features = _dependency_target_and_features(
                dependency, specification, workspace_dependencies
            )
            opt_ins = EXPLICIT_OPT_IN_FEATURES.get(target_package)
            if opt_ins is None:
                continue
            for feature in sorted(selected_features & frozenset(opt_ins)):
                selections.append((section, dependency, target_package, feature))
    return tuple(selections)


def _check_expected_features(
    document: dict[str, Any], manifest_path: Path
) -> list[str]:
    package = document.get("package", {})
    package_name = package.get("name") if isinstance(package, dict) else None
    expected = EXPECTED_FEATURES.get(package_name)
    if expected is None:
        return []

    errors: list[str] = []
    try:
        actual_features = cargo_visible_features(document)
    except ValueError as error:
        return [f"{manifest_path}: {error}"]

    contextual_members = CONTEXTUAL_SHIPPING_FEATURES.get(package_name)
    opt_in_members = EXPLICIT_OPT_IN_FEATURES.get(package_name)
    if contextual_members is None:
        return [f"{manifest_path}: missing contextual shipping feature inventory"]
    if opt_in_members is None:
        return [f"{manifest_path}: missing explicit opt-in feature inventory"]
    if tuple(sorted(set(contextual_members))) != contextual_members:
        errors.append(
            f"{manifest_path}: contextual shipping feature inventory must be "
            "sorted and contain no duplicates"
        )
    if tuple(sorted(set(opt_in_members))) != opt_in_members:
        errors.append(
            f"{manifest_path}: explicit opt-in feature inventory must be sorted "
            "and contain no duplicates"
        )
    contextual = frozenset(contextual_members)
    opt_in = frozenset(opt_in_members)
    reachable = local_default_feature_closure(actual_features)
    actual_names = frozenset(actual_features)
    expected_names = frozenset(expected)
    expected_portable = expected_names - contextual - opt_in

    for feature in sorted(actual_names - reachable - contextual - opt_in):
        errors.append(
            f"{manifest_path}: Cargo feature `{feature}` is unclassified; "
            "add it to the portable default closure, contextual shipping "
            "inventory, or explicit opt-in inventory"
        )
    for feature in sorted(contextual - actual_names):
        errors.append(
            f"{manifest_path}: stale contextual shipping feature `{feature}` is "
            "not declared by Cargo"
        )
    for feature in sorted(opt_in - actual_names):
        errors.append(
            f"{manifest_path}: stale explicit opt-in feature `{feature}` is not "
            "declared by Cargo"
        )
    for feature in sorted(contextual & opt_in):
        errors.append(
            f"{manifest_path}: feature `{feature}` is classified as both "
            "contextual shipping and explicit opt-in"
        )
    for feature in sorted(contextual & reachable):
        errors.append(
            f"{manifest_path}: contextual shipping feature `{feature}` is "
            "reachable from local `default`"
        )
    for feature in sorted(opt_in & reachable):
        errors.append(
            f"{manifest_path}: explicit opt-in feature `{feature}` is reachable "
            "from `default`"
        )
    for feature in sorted(contextual - expected_names):
        errors.append(
            f"{manifest_path}: contextual shipping feature `{feature}` lacks an "
            "exact feature pin"
        )
    for feature in sorted(reachable - expected_portable):
        errors.append(
            f"{manifest_path}: default-reachable feature `{feature}` lacks an "
            "exact portable feature pin"
        )
    for feature in sorted(expected_portable - reachable):
        errors.append(
            f"{manifest_path}: portable feature `{feature}` is not reachable "
            "from `default`"
        )

    for feature, expected_members in expected.items():
        actual_members = actual_features.get(feature)
        if actual_members is None:
            errors.append(f"{manifest_path}: missing feature aggregate `{feature}`")
            continue
        if actual_members != expected_members:
            errors.append(
                f"{manifest_path}: feature `{feature}` must be "
                f"{list(expected_members)!r}, found {actual_members!r}"
            )
    return errors


def check_repository(root: Path) -> list[str]:
    """Return deterministic feature-hygiene violations for ``root``."""

    root = root.resolve()
    root_manifest_path = root / "Cargo.toml"
    root_manifest = _load_toml(root_manifest_path)
    workspace = root_manifest.get("workspace", {})
    if not isinstance(workspace, dict):
        return [f"{root_manifest_path}: missing [workspace] table"]
    workspace_dependencies = workspace.get("dependencies", {})
    if not isinstance(workspace_dependencies, dict):
        return [f"{root_manifest_path}: `workspace.dependencies` must be a table"]

    errors: list[str] = []
    for label, policy in (
        ("contextual shipping", CONTEXTUAL_SHIPPING_FEATURES),
        ("explicit opt-in", EXPLICIT_OPT_IN_FEATURES),
    ):
        missing_policies = sorted(set(EXPECTED_FEATURES) - set(policy))
        stale_policies = sorted(set(policy) - set(EXPECTED_FEATURES))
        if missing_policies:
            errors.append(
                f"{root_manifest_path}: guarded packages lack {label} feature "
                f"inventories: {missing_policies}"
            )
        if stale_policies:
            errors.append(
                f"{root_manifest_path}: {label} feature inventories name "
                f"unguarded packages: {stale_policies}"
            )
    for dependency in sorted(FOUNDATIONAL_DEPENDENCIES):
        specification = workspace_dependencies.get(dependency)
        if not isinstance(specification, dict):
            errors.append(
                f"{root_manifest_path}: workspace dependency `{dependency}` "
                "must use a table with `default-features = false`"
            )
            continue
        if specification.get("default-features") is not False:
            errors.append(
                f"{root_manifest_path}: workspace dependency `{dependency}` "
                "must set `default-features = false`"
            )
        if "features" in specification:
            errors.append(
                f"{root_manifest_path}: workspace dependency `{dependency}` "
                "must not inject features"
            )

    try:
        member_manifests = workspace_member_manifests(root, workspace)
    except ValueError as error:
        return [*errors, f"{root_manifest_path}: {error}"]

    raw_allowlist = NONSHIPPING_EXPLICIT_OPT_IN_DEPENDENCY_ALLOWLIST
    if tuple(sorted(set(raw_allowlist))) != raw_allowlist:
        errors.append(
            f"{root_manifest_path}: non-shipping explicit opt-in dependency "
            "allowlist must be sorted and contain no duplicates"
        )
    allowlist = frozenset(raw_allowlist)
    valid_allowlist: set[tuple[str, str, str]] = set()
    for consumer, dependency, feature in sorted(allowlist):
        if feature not in EXPLICIT_OPT_IN_FEATURES.get(dependency, ()):
            errors.append(
                f"{root_manifest_path}: non-shipping explicit opt-in dependency "
                f"allowlist entry `{consumer} -> {dependency}/{feature}` does not "
                "name a classified explicit opt-in feature"
            )
            continue
        valid_allowlist.add((consumer, dependency, feature))

    workspace_package_names: set[str] = set()
    observed_allowlist_entries: set[tuple[str, str, str]] = set()
    for manifest_path in member_manifests:
        document = _load_toml(manifest_path)
        package = document.get("package", {})
        package_name = package.get("name") if isinstance(package, dict) else None
        if isinstance(package_name, str) and package_name:
            workspace_package_names.add(package_name)
            for section, dependency_key, target_package, feature in (
                _non_dev_explicit_opt_in_dependency_selections(
                    document, workspace_dependencies
                )
            ):
                entry = (package_name, target_package, feature)
                if entry in valid_allowlist:
                    observed_allowlist_entries.add(entry)
                    continue
                alias = (
                    ""
                    if dependency_key == target_package
                    else f" (package `{target_package}`)"
                )
                errors.append(
                    f"{manifest_path}: package `{package_name}` [{section}] "
                    f"dependency `{dependency_key}`{alias} selects explicit opt-in "
                    f"feature `{feature}` from a non-dev dependency declaration"
                )
        errors.extend(_check_expected_features(document, manifest_path))
        for section, dependencies in _dependency_tables(document):
            for dependency in sorted(FOUNDATIONAL_DEPENDENCIES & dependencies.keys()):
                specification = dependencies[dependency]
                if not isinstance(specification, dict) or specification.get(
                    "default-features"
                ) is not False:
                    errors.append(
                        f"{manifest_path}: [{section}] `{dependency}` must set "
                        "`default-features = false` and select features locally"
                    )

    for consumer, dependency, feature in sorted(
        valid_allowlist - observed_allowlist_entries
    ):
        if dependency not in workspace_package_names:
            continue
        errors.append(
            f"{root_manifest_path}: stale non-shipping explicit opt-in dependency "
            f"allowlist entry `{consumer} -> {dependency}/{feature}` is not selected "
            "by a non-dev workspace dependency declaration"
        )

    return errors


def main() -> int:
    """Run the command-line feature-hygiene check."""

    parser = argparse.ArgumentParser(
        description=(
            "Reject workspace-level feature injection and implicit defaults in "
            "every Cargo workspace member."
        )
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root containing Cargo.toml (default: inferred)",
    )
    args = parser.parse_args()

    errors = check_repository(args.root)
    if errors:
        print("Cargo feature hygiene violations:")
        for error in errors:
            print(f"  - {error}")
        return 1

    print("Cargo feature hygiene check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
