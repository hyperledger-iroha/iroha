"""Import-safe health and lifecycle projections for the Taira reset controller.

The controller injects its validation primitives and fixed public-network
constants so this module never imports a second copy of a running
``__main__`` deployment controller. Runtime signing material remains
file-sourced at call time and is never serialized or reported here.
"""

from __future__ import annotations

import dataclasses
import hashlib
import json
import os
import stat
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable, Optional


def configure_runtime(
    *,
    deployment_error: type[RuntimeError],
    fail_callback: Callable[[str], Any],
    parse_json: Callable[[bytes, str], dict[str, Any]],
    load_operator_context: Callable[[str, Path], Any],
    require_acl_free: Callable[[Path, str], os.stat_result],
    metadata_identity_callback: Callable[[os.stat_result], tuple[int, ...]],
    require_lifecycle_node_ids: Callable[[object], dict[str, str]],
    receipt_signer_map: Callable[[object], dict[str, dict[str, object]]],
    max_http_bytes: int,
    max_terminal_unhealthy_bytes: int,
    block_hash_re: Any,
    commit_re: Any,
    sha256_re: Any,
    lifecycle_node_id_re: Any,
    peer_count: int,
    slugs: tuple[str, ...],
    lane_count: int,
    lane_dataspace_bindings: tuple[tuple[int, str, str, int], ...],
    physical_dataspaces: tuple[tuple[str, int], ...],
    terminal_unhealthy_schema: str,
    lifecycle_state_schema: str,
    lifecycle_binding_domain: bytes,
) -> None:
    """Bind controller-owned primitives without importing controller identity."""

    globals().update(
        {
            "DeploymentError": deployment_error,
            "fail": fail_callback,
            "parse_json_bytes": parse_json,
            "load_operator_context_from_file": load_operator_context,
            "require_acl_free_path": require_acl_free,
            "metadata_identity": metadata_identity_callback,
            "require_authenticated_lifecycle_node_ids": require_lifecycle_node_ids,
            "receipt_signer_public_map": receipt_signer_map,
            "MAX_HTTP_BYTES": max_http_bytes,
            "MAX_TERMINAL_UNHEALTHY_BYTES": max_terminal_unhealthy_bytes,
            "BLOCK_HASH_RE": block_hash_re,
            "COMMIT_RE": commit_re,
            "SHA256_RE": sha256_re,
            "LIFECYCLE_NODE_ID_RE": lifecycle_node_id_re,
            "PEER_COUNT": peer_count,
            "SLUGS": slugs,
            "TAIRA_LANE_COUNT": lane_count,
            "TAIRA_LANE_DATASPACE_BINDINGS": lane_dataspace_bindings,
            "TAIRA_PHYSICAL_DATASPACES": physical_dataspaces,
            "TERMINAL_UNHEALTHY_SCHEMA": terminal_unhealthy_schema,
            "LIFECYCLE_STATE_SCHEMA": lifecycle_state_schema,
            "LIFECYCLE_BINDING_DOMAIN": lifecycle_binding_domain,
        }
    )


def http_json(url: str, timeout: float = 2.0) -> dict[str, Any]:
    """Fetch one bounded JSON response without retaining error bodies."""

    request = urllib.request.Request(
        url,
        method="GET",
        headers={"Accept": "application/json", "User-Agent": "taira-v21-reset/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            if response.status != 200:
                fail(f"health endpoint returned HTTP {response.status}: {url}")
            body = response.read(MAX_HTTP_BYTES + 1)
    except (OSError, urllib.error.URLError, TimeoutError) as error:
        raise DeploymentError(f"health endpoint is unavailable: {url}") from error
    if len(body) > MAX_HTTP_BYTES:
        fail(f"health endpoint response exceeds {MAX_HTTP_BYTES} bytes: {url}")
    return parse_json_bytes(body, f"health response from {url}")


def http_ok(url: str, timeout: float = 2.0) -> None:
    """Require one bounded HTTP 200 response without parsing or retaining its body."""

    request = urllib.request.Request(
        url,
        method="GET",
        headers={"Accept": "*/*", "User-Agent": "taira-v21-reset/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            if response.status != 200:
                fail(f"health endpoint returned HTTP {response.status}: {url}")
            body = response.read(MAX_HTTP_BYTES + 1)
    except (OSError, urllib.error.URLError, TimeoutError) as error:
        raise DeploymentError(f"health endpoint is unavailable: {url}") from error
    if len(body) > MAX_HTTP_BYTES:
        fail(f"health endpoint response exceeds {MAX_HTTP_BYTES} bytes: {url}")


class _RejectRedirects(urllib.request.HTTPRedirectHandler):
    """Prevent replay of a fresh operator signature at a redirected target."""

    def redirect_request(self, request, file_pointer, code, message, headers, new_url):
        del request, file_pointer, code, message, headers, new_url
        return None


def build_operator_http_getter(network_id: str, private_key_file: Path) -> HttpGetter:
    """Build a token-free, no-redirect getter with a fresh signature per request."""

    context = load_operator_context_from_file(network_id, private_key_file)
    opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({}),
        _RejectRedirects(),
    )

    def operator_http_json(url: str, timeout: float = 2.0) -> dict[str, Any]:
        parsed = urllib.parse.urlsplit(url)
        if parsed.path != "/v1/sumeragi/status":
            return http_json(url, timeout)
        if (
            parsed.scheme not in {"http", "https"}
            or not parsed.netloc
            or parsed.username is not None
            or parsed.password is not None
            or parsed.fragment
        ):
            fail("operator endpoint must be an absolute credential-free HTTP(S) URL")
        target = parsed.path + (f"?{parsed.query}" if parsed.query else "")
        headers = context.headers("GET", target, b"")
        headers.update(
            {"Accept": "application/json", "User-Agent": "taira-v21-reset/1"}
        )
        request = urllib.request.Request(url, method="GET", headers=headers)
        try:
            with opener.open(request, timeout=timeout) as response:
                if response.status != 200:
                    fail(f"operator endpoint returned HTTP {response.status}: {url}")
                body = response.read(MAX_HTTP_BYTES + 1)
        except (OSError, urllib.error.URLError, TimeoutError) as error:
            raise DeploymentError(f"operator endpoint is unavailable: {url}") from error
        if len(body) > MAX_HTTP_BYTES:
            fail(f"operator endpoint response exceeds {MAX_HTTP_BYTES} bytes: {url}")
        return parse_json_bytes(body, f"operator response from {url}")

    return operator_http_json


def require_uint(value: object, label: str, *, positive: bool = False) -> int:
    """Require one non-boolean unsigned JSON integer."""

    if not isinstance(value, int) or isinstance(value, bool) or value < int(positive):
        fail(f"{label} is not a valid unsigned integer")
    return value


def normalized_block_hash(value: object, label: str) -> str:
    """Normalize a canonical Iroha block hash to lowercase hexadecimal."""

    if not isinstance(value, str):
        fail(f"{label} is not a block hash")
    match = BLOCK_HASH_RE.fullmatch(value)
    if match is None:
        fail(f"{label} is not a canonical block hash")
    normalized = match.group(1).lower()
    if int(normalized[-2:], 16) & 1 == 0:
        fail(f"{label} does not carry the Iroha marker bit")
    return normalized


def nested(payload: dict[str, Any], *keys: str) -> object:
    """Return a nested mapping value, or ``None`` on a missing object."""

    current: object = payload
    for key in keys:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def tagged_unit(value: object, key: str, label: str, allowed: set[str]) -> str:
    """Decode one canonical tagged-unit status value."""

    if (
        not isinstance(value, dict)
        or set(value) != {key, "details"}
        or not isinstance(value.get(key), str)
        or value.get(key) not in allowed
        or value.get("details") is not None
    ):
        fail(f"{label} is not a canonical tagged unit")
    tag = value[key]
    assert isinstance(tag, str)
    return tag


def published_source_commit(status: dict[str, Any]) -> str:
    """Read the exact full build commit from public node status."""

    build = status.get("build")
    if not isinstance(build, dict):
        fail("/status omitted its build identity")
    for key in ("git_commit_sha", "git_sha", "commit_sha", "commit"):
        value = build.get(key)
        if isinstance(value, str) and COMMIT_RE.fullmatch(value.lower()):
            return value.lower()
    fail("/status omitted one full build Git commit")


def published_dpn_validator_release_commit(status: dict[str, Any]) -> str:
    """Read the exact DPN validator release commit from public node status."""

    build = status.get("build")
    if not isinstance(build, dict):
        fail("/status omitted its build identity")
    value = build.get("dpn_validator_release_commit")
    if not isinstance(value, str) or COMMIT_RE.fullmatch(value) is None:
        fail("/status omitted one full DPN validator release commit")
    return value


@dataclasses.dataclass(frozen=True)
class PeerSample:
    """Coherent commit and lane/dataspace topology observed from one validator."""

    label: str
    height: int
    block_hash: str
    context: str
    node: str
    build: str
    config: str
    nexus_topology: str


@dataclasses.dataclass(frozen=True)
class FleetSample:
    """One exact four-validator common-commit sample."""

    height: int
    block_hash: str
    context: str
    build: str
    config: str
    nexus_topology: str
    nodes: tuple[str, ...]


@dataclasses.dataclass(frozen=True)
class RestartProofResult:
    """Validated post-restart fleet state and bounded measured recovery time."""

    fleet: FleetSample
    duration_ms: int


HttpGetter = Callable[[str, float], dict[str, Any]]
HealthGetter = Callable[[str, float], None]
TerminalChecker = Callable[[], None]


def no_terminal_check() -> None:
    """Default no-op for focused read-path tests without a runtime layout."""


def deployment_completed_at_unix_ms() -> int:
    """Return one positive millisecond timestamp for a completed deployment."""

    value = time.time_ns() // 1_000_000
    if value <= 0:
        fail("deployment completion clock is not positive")
    return value


def deployed_config_set_sha256(bundle: BundlePlan) -> str:
    """Hash the exact ordered public validator config identity map."""

    value = {peer.slug: peer.config_sha256 for peer in bundle.peers}
    if tuple(value) != SLUGS:
        fail("deployed config set is not the exact ordered validator set")
    payload = json.dumps(
        value, ensure_ascii=True, sort_keys=True, separators=(",", ":")
    ).encode("ascii")
    return hashlib.sha256(payload).hexdigest()


def deployed_topology_sha256(nexus_topology: str) -> str:
    """Hash one already canonical exact-seven-lane topology projection."""

    try:
        value = json.loads(nexus_topology)
    except (TypeError, ValueError, json.JSONDecodeError) as error:
        raise DeploymentError("deployed topology is not canonical JSON") from error
    canonical = json.dumps(
        value, ensure_ascii=True, sort_keys=True, separators=(",", ":")
    )
    if canonical != nexus_topology:
        fail("deployed topology is not canonical JSON")
    return hashlib.sha256(canonical.encode("ascii")).hexdigest()


def supervisor_terminal_binding(
    binary_sha256: str,
    binary_info: os.stat_result,
    config_sha256: str,
    restart_generation: str,
) -> str:
    """Reproduce the supervisor's redaction-safe runtime binding."""

    payload = {
        "binary_sha256": binary_sha256,
        "binary_stat_seal": [
            binary_info.st_dev,
            binary_info.st_ino,
            binary_info.st_size,
            binary_info.st_mtime_ns,
            binary_info.st_ctime_ns,
        ],
        "config_sha256": config_sha256,
        "restart_generation": restart_generation,
        "schema": TERMINAL_UNHEALTHY_SCHEMA,
    }
    return hashlib.sha256(
        json.dumps(
            payload,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("ascii")
    ).hexdigest()


def supervisor_lifecycle_binding(
    runtime_binding_sha256: str,
    restart_generation: str,
    validator_id: str,
    node_id: str,
) -> str:
    """Reproduce the supervisor's domain-separated lifecycle binding."""

    if (
        SHA256_RE.fullmatch(runtime_binding_sha256) is None
        or SHA256_RE.fullmatch(restart_generation) is None
        or validator_id not in SLUGS
        or LIFECYCLE_NODE_ID_RE.fullmatch(node_id) is None
    ):
        fail("lifecycle binding inputs are not canonical")
    payload = {
        "node_id": node_id,
        "restart_generation": restart_generation,
        "runtime_binding_sha256": runtime_binding_sha256,
        "schema": LIFECYCLE_STATE_SCHEMA,
        "validator_id": validator_id,
    }
    encoded = (
        json.dumps(
            payload,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")
    return hashlib.sha256(LIFECYCLE_BINDING_DOMAIN + encoded).hexdigest()


def deployed_receipt_signer_map(
    bundle: BundlePlan,
    sources: SourcePlan,
    binary_info: os.stat_result,
    restart_generation: str,
) -> dict[str, dict[str, object]]:
    """Bind each public receipt signer to its exact deployed runtime identity."""

    node_ids = require_authenticated_lifecycle_node_ids(bundle)
    if len(bundle.peers) != PEER_COUNT:
        fail("deployed receipt signer map requires the exact four-peer plan")
    binary_stat_seal = [
        binary_info.st_dev,
        binary_info.st_ino,
        binary_info.st_size,
        binary_info.st_mtime_ns,
        binary_info.st_ctime_ns,
    ]
    public_map = receipt_signer_public_map(bundle.receipt_signers)
    for peer, signer in zip(bundle.peers, bundle.receipt_signers, strict=True):
        if peer.slug != signer.slug:
            fail("deployed receipt signer order differs from the deploy peer plan")
        runtime_binding = supervisor_terminal_binding(
            sources.binary_sha256,
            binary_info,
            peer.config_sha256,
            restart_generation,
        )
        public_map[peer.slug].update(
            {
                "binary_stat_seal": list(binary_stat_seal),
                "config_sha256": peer.config_sha256,
                "lifecycle_binding_sha256": supervisor_lifecycle_binding(
                    runtime_binding,
                    restart_generation,
                    peer.slug,
                    node_ids[peer.slug],
                ),
                "runtime_binding_sha256": runtime_binding,
            }
        )
    return public_map


def terminal_unhealthy_path(runtime_root: Path, peer: PeerPlan, binding: str) -> Path:
    """Return the identity-scoped private marker for one peer supervisor."""

    return (
        runtime_root
        / "terminal"
        / f"validator-{peer.number}-{binding}-terminal-unhealthy.json"
    )


def require_terminal_marker(
    path: Path,
    peer: PeerPlan,
    owner_uid: int,
    owner_gid: int,
    expected_binding: str,
) -> None:
    """Authenticate one marker and raise a redaction-safe terminal error."""

    try:
        before = require_acl_free_path(path, "terminal-unhealthy marker")
    except FileNotFoundError:
        return
    except (DeploymentError, OSError) as error:
        raise DeploymentError(
            f"{peer.label} terminal-unhealthy marker is unsafe"
        ) from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_uid != owner_uid
        or before.st_gid != owner_gid
        or before.st_nlink != 1
        or stat.S_IMODE(before.st_mode) != 0o600
        or not 0 < before.st_size <= MAX_TERMINAL_UNHEALTHY_BYTES
    ):
        fail(f"{peer.label} terminal-unhealthy marker is unsafe")
    try:
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
        )
    except OSError as error:
        raise DeploymentError(
            f"{peer.label} terminal-unhealthy marker is unsafe"
        ) from error
    try:
        body = bytearray()
        while len(body) <= MAX_TERMINAL_UNHEALTHY_BYTES:
            chunk = os.read(
                descriptor,
                min(
                    256,
                    MAX_TERMINAL_UNHEALTHY_BYTES + 1 - len(body),
                ),
            )
            if not chunk:
                break
            body.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        metadata_identity(before) != metadata_identity(after)
        or len(body) > MAX_TERMINAL_UNHEALTHY_BYTES
    ):
        fail(f"{peer.label} terminal-unhealthy marker is unsafe")
    try:
        payload = json.loads(body)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise DeploymentError(
            f"{peer.label} terminal-unhealthy marker is unsafe"
        ) from error
    if (
        not isinstance(payload, dict)
        or set(payload)
        != {
            "binding_sha256",
            "fatal_fingerprint_sha256",
            "hit_count",
            "schema",
        }
        or payload.get("schema") != TERMINAL_UNHEALTHY_SCHEMA
        or payload.get("hit_count") != 3
        or not isinstance(payload.get("binding_sha256"), str)
        or SHA256_RE.fullmatch(payload["binding_sha256"]) is None
        or not isinstance(payload.get("fatal_fingerprint_sha256"), str)
        or SHA256_RE.fullmatch(payload["fatal_fingerprint_sha256"]) is None
        or payload.get("binding_sha256") != expected_binding
        or (
            json.dumps(
                payload,
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
        != body
    ):
        fail(f"{peer.label} terminal-unhealthy marker is unsafe")
    fail(f"{peer.label} entered terminal-unhealthy state")


def require_no_terminal_unhealthy(
    bundle: BundlePlan,
    runtime_root: Path,
    bindings: dict[str, str],
) -> None:
    """Fail fast when any supervisor has durably stopped respawning."""

    for peer in bundle.peers:
        binding = bindings.get(peer.label)
        if binding is None or SHA256_RE.fullmatch(binding) is None:
            fail("terminal-unhealthy binding map is incomplete")
        require_terminal_marker(
            terminal_unhealthy_path(runtime_root, peer, binding),
            peer,
            bundle.owner_uid,
            bundle.owner_gid,
            binding,
        )


def validate_peer_health(
    peer: PeerPlan,
    bundle: BundlePlan,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> PeerSample:
    """Validate readiness, exact lane/dataspace topology, and durable consensus."""

    root = f"http://127.0.0.1:{peer.torii_port}"
    health_getter(f"{root}/health", 2.0)
    health_getter(f"{root}/readyz", 2.0)

    lifecycle = getter(f"{root}/v1/nexus/lifecycle", 2.0)
    lanes = lifecycle.get("lanes")
    if lifecycle.get("version") != 1 or lifecycle.get("nexus_enabled") is not True:
        fail(f"{peer.label} Nexus lifecycle identity is invalid")
    lane_count = lifecycle.get("lane_count")
    if (
        not isinstance(lane_count, int)
        or isinstance(lane_count, bool)
        or lane_count != TAIRA_LANE_COUNT
    ):
        fail(f"{peer.label} Nexus lifecycle lane_count is not exactly 7")
    if not isinstance(lanes, list):
        fail(f"{peer.label} Nexus lifecycle omitted its lane catalog")
    if len(lanes) != TAIRA_LANE_COUNT:
        fail(f"{peer.label} Nexus lifecycle does not contain exactly seven lanes")
    expected_lane_records = [
        {"id": lane_id, "alias": lane_alias, "dataspace_id": dataspace_id}
        for lane_id, lane_alias, _dataspace_alias, dataspace_id in (
            TAIRA_LANE_DATASPACE_BINDINGS
        )
    ]
    observed_lane_records: list[dict[str, int | str]] = []
    seen_lane_ids: set[int] = set()
    seen_lane_aliases: set[str] = set()
    for position, lane in enumerate(lanes):
        if not isinstance(lane, dict):
            fail(f"{peer.label} Nexus lifecycle lane {position} is malformed")
        lane_id = lane.get("id")
        alias = lane.get("alias")
        dataspace_id = lane.get("dataspace_id")
        if not isinstance(lane_id, int) or isinstance(lane_id, bool) or lane_id < 0:
            fail(f"{peer.label} Nexus lifecycle lane {position} has an invalid id")
        if not isinstance(alias, str) or not alias:
            fail(f"{peer.label} Nexus lifecycle lane {position} has an invalid alias")
        if (
            not isinstance(dataspace_id, int)
            or isinstance(dataspace_id, bool)
            or dataspace_id < 0
        ):
            fail(
                f"{peer.label} Nexus lifecycle lane {position} has an invalid dataspace id"
            )
        if lane_id in seen_lane_ids or alias in seen_lane_aliases:
            fail(f"{peer.label} Nexus lifecycle duplicates a lane id or alias")
        seen_lane_ids.add(lane_id)
        seen_lane_aliases.add(alias)
        observed_lane_records.append(
            {"id": lane_id, "alias": alias, "dataspace_id": dataspace_id}
        )
    if observed_lane_records != expected_lane_records:
        fail(
            f"{peer.label} does not expose the exact canonical "
            "seven-lane/five-dataspace topology"
        )
    observed_dataspace_ids = {
        record["dataspace_id"] for record in observed_lane_records
    }
    expected_dataspace_ids = {
        dataspace_id for _alias, dataspace_id in TAIRA_PHYSICAL_DATASPACES
    }
    if observed_dataspace_ids != expected_dataspace_ids:
        fail(f"{peer.label} does not expose exactly five physical dataspaces")
    catalog_hash = lifecycle.get("catalog_hash")
    if not isinstance(catalog_hash, str) or BLOCK_HASH_RE.fullmatch(catalog_hash) is None:
        fail(f"{peer.label} Nexus lifecycle omitted a canonical catalog identity")

    canonical_lane_binding_evidence = [
        {
            "lane_id": lane_id,
            "lane_alias": lane_alias,
            "dataspace_id": dataspace_id,
            "dataspace_alias": dataspace_alias,
        }
        for lane_id, lane_alias, dataspace_alias, dataspace_id in (
            TAIRA_LANE_DATASPACE_BINDINGS
        )
    ]
    canonical_physical_dataspace_evidence = [
        {"dataspace_id": dataspace_id, "dataspace_alias": dataspace_alias}
        for dataspace_alias, dataspace_id in TAIRA_PHYSICAL_DATASPACES
    ]

    status = getter(f"{root}/status", 2.0)
    blocks = require_uint(
        status.get("blocks"), f"{peer.label} /status.blocks", positive=True
    )
    if published_source_commit(status) != expected_source_commit:
        fail(f"{peer.label} publishes the wrong build source commit")
    if (
        published_dpn_validator_release_commit(status)
        != expected_dpn_validator_release_commit
    ):
        fail(f"{peer.label} publishes the wrong DPN validator release commit")

    sumeragi = getter(f"{root}/v1/sumeragi/status", 2.0)
    if (
        sumeragi.get("protocol_version") != 4
        or sumeragi.get("restart_required") is not False
    ):
        fail(f"{peer.label} is not running one restart-clean Sumeragi v2 reducer")
    reducer_height = require_uint(
        sumeragi.get("height"), f"{peer.label} reducer height", positive=True
    )
    committed = require_uint(
        sumeragi.get("last_committed_height"),
        f"{peer.label} last_committed_height",
        positive=True,
    )
    if committed != blocks:
        fail(f"{peer.label} /status.blocks differs from durable committed height")
    if committed > reducer_height:
        fail(f"{peer.label} committed height is ahead of its reducer height")
    context_record = sumeragi.get("height_context")
    if not isinstance(context_record, dict):
        fail(f"{peer.label} omitted its frozen height context")
    validator_count = require_uint(
        context_record.get("validator_count"),
        f"{peer.label} frozen validator count",
        positive=True,
    )
    quorum = context_record.get("quorum")
    if not isinstance(quorum, dict):
        fail(f"{peer.label} omitted its frozen quorum")
    context_min_signers = require_uint(
        quorum.get("min_signers"), f"{peer.label} frozen minimum signers"
    )
    context_total_power = require_uint(
        quorum.get("total_power"), f"{peer.label} frozen total power", positive=True
    )
    mode = tagged_unit(
        context_record.get("mode"),
        "mode",
        f"{peer.label} consensus mode",
        {"permissioned", "npos"},
    )
    if (
        validator_count != PEER_COUNT
        or context_min_signers != 3
        or context_total_power < PEER_COUNT
        or (mode == "permissioned" and context_total_power != PEER_COUNT)
    ):
        fail(f"{peer.label} frozen context is not the exact four-validator quorum")
    subject = sumeragi.get("last_committed_subject")
    if not isinstance(subject, dict):
        fail(f"{peer.label} omitted the durable committed subject")
    block_hash = normalized_block_hash(
        subject.get("block_hash"), f"{peer.label} committed block"
    )
    qc_height = require_uint(
        nested(sumeragi, "last_commit_qc", "certificate", "round", "height"),
        f"{peer.label} CommitQC height",
        positive=True,
    )
    if qc_height != committed:
        fail(f"{peer.label} CommitQC height differs from committed height")
    require_uint(
        nested(sumeragi, "last_commit_qc", "certificate", "round", "view"),
        f"{peer.label} CommitQC view",
    )
    tagged_unit(
        nested(sumeragi, "last_commit_qc", "certificate", "phase"),
        "phase",
        f"{peer.label} CommitQC phase",
        {"commit"},
    )
    qc_subject = nested(sumeragi, "last_commit_qc", "certificate", "subject")
    if qc_subject != subject:
        fail(f"{peer.label} CommitQC subject differs from committed subject")
    commit_record = sumeragi.get("last_commit_qc")
    assert isinstance(commit_record, dict)
    commit_validators = require_uint(
        commit_record.get("validator_count"),
        f"{peer.label} CommitQC validator count",
        positive=True,
    )
    commit_signers = require_uint(
        commit_record.get("signer_count"), f"{peer.label} CommitQC signer count"
    )
    commit_min_signers = require_uint(
        commit_record.get("min_signers"), f"{peer.label} CommitQC minimum signers"
    )
    commit_signed_power = require_uint(
        commit_record.get("signed_power"), f"{peer.label} CommitQC signed power"
    )
    commit_total_power = require_uint(
        commit_record.get("total_power"),
        f"{peer.label} CommitQC total power",
        positive=True,
    )
    if (
        commit_validators != PEER_COUNT
        or commit_min_signers != 3
        or commit_signers != commit_min_signers
        or commit_total_power != context_total_power
        or commit_signed_power > commit_total_power
        or commit_signed_power * 3 <= commit_total_power * 2
        or (mode == "permissioned" and commit_signed_power != commit_signers)
    ):
        fail(f"{peer.label} durable CommitQC lacks the exact four-validator quorum")
    context = sumeragi.get("height_context_id")
    node_fingerprint = sumeragi.get("node_fingerprint")
    build_fingerprint = sumeragi.get("build_fingerprint")
    config_fingerprint = sumeragi.get("config_fingerprint")
    if any(
        value in (None, "", {})
        for value in (context, node_fingerprint, build_fingerprint, config_fingerprint)
    ):
        fail(f"{peer.label} omitted a required reducer fingerprint")

    canonical = lambda value: json.dumps(
        value, ensure_ascii=True, sort_keys=True, separators=(",", ":")
    )
    return PeerSample(
        label=peer.label,
        height=committed,
        block_hash=block_hash,
        context=canonical(context),
        node=canonical(node_fingerprint),
        build=canonical(build_fingerprint),
        config=canonical(config_fingerprint),
        nexus_topology=canonical(
            {
                "observed_catalog_hash": catalog_hash.lower(),
                "observed_lane_count": lane_count,
                "canonical_lane_bindings": canonical_lane_binding_evidence,
                "canonical_physical_dataspaces": (
                    canonical_physical_dataspace_evidence
                ),
            }
        ),
    )


def capture_fleet(
    bundle: BundlePlan,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> FleetSample:
    """Require all four direct validators to expose one exact common commit."""

    samples = [
        validate_peer_health(
            peer,
            bundle,
            expected_source_commit,
            expected_dpn_validator_release_commit,
            getter=getter,
            health_getter=health_getter,
        )
        for peer in bundle.peers
    ]
    baseline = samples[0]
    for sample in samples[1:]:
        for field in (
            "height",
            "block_hash",
            "context",
            "build",
            "config",
            "nexus_topology",
        ):
            if getattr(sample, field) != getattr(baseline, field):
                fail(f"four-validator fleet disagrees on {field}")
    nodes = tuple(sorted(sample.node for sample in samples))
    if len(set(nodes)) != PEER_COUNT:
        fail("four validator roots do not expose four distinct node identities")
    return FleetSample(
        height=baseline.height,
        block_hash=baseline.block_hash,
        context=baseline.context,
        build=baseline.build,
        config=baseline.config,
        nexus_topology=baseline.nexus_topology,
        nodes=nodes,
    )


def wait_for_fleet_sample(
    bundle: BundlePlan,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
    deadline: float,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
    terminal_checker: TerminalChecker = no_terminal_check,
) -> FleetSample:
    """Retry startup/alignment failures until one coherent sample is available."""

    last_error: Optional[Exception] = None
    while time.monotonic() < deadline:
        terminal_checker()
        try:
            sample = capture_fleet(
                bundle,
                expected_source_commit,
                expected_dpn_validator_release_commit,
                getter=getter,
                health_getter=health_getter,
            )
        except (DeploymentError, OSError) as error:
            last_error = error
            time.sleep(1)
            continue
        terminal_checker()
        return sample
    raise DeploymentError(f"four-validator readiness did not converge: {last_error}")


def wait_for_advancement(
    bundle: BundlePlan,
    expected_source_commit: str,
    expected_dpn_validator_release_commit: str,
    previous: FleetSample,
    deadline: float,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
    terminal_checker: TerminalChecker = no_terminal_check,
) -> FleetSample:
    """Require a later common height with a different common block hash."""

    last_error: Optional[Exception] = None
    while time.monotonic() < deadline:
        terminal_checker()
        try:
            current = capture_fleet(
                bundle,
                expected_source_commit,
                expected_dpn_validator_release_commit,
                getter=getter,
                health_getter=health_getter,
            )
            if (
                current.height > previous.height
                and current.block_hash != previous.block_hash
                and current.build == previous.build
                and current.config == previous.config
                and current.nexus_topology == previous.nexus_topology
                and current.nodes == previous.nodes
            ):
                advanced = True
            else:
                advanced = False
                last_error = DeploymentError(
                    "fleet has not advanced one stable common build/config/topology"
                )
        except (DeploymentError, OSError) as error:
            last_error = error
            advanced = False
        if advanced:
            terminal_checker()
            return current
        time.sleep(1)
    raise DeploymentError(f"four-validator consensus did not advance: {last_error}")
