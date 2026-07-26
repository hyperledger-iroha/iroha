#!/usr/bin/env python3
"""Generate SoraDNS zonefile skeletons from DNS cutover descriptors (roadmap SN-7)."""

from __future__ import annotations

import argparse
import json
import string
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
import sys
from typing import Any, Sequence


_SCHEMA_VERSION = 1

_FREEZE_STATE_CHOICES = frozenset({"soft", "hard", "thawing", "monitoring", "emergency"})


@dataclass(frozen=True)
class ZonefileOptions:
    ttl: int
    ipv4: tuple[str, ...]
    ipv6: tuple[str, ...]
    cname_target: str | None
    extra_txt: tuple[str, ...]
    spki_pins: tuple[str, ...]
    source_path: str | None
    now: datetime
    freeze_state: str | None = None
    freeze_ticket: str | None = None
    freeze_expires_at: str | None = None
    freeze_notes: tuple[str, ...] = ()
    version: str | None = None
    effective_at: str | None = None
    gar_digest: str | None = None
    cid_override: str | None = None
    proof_override: str | None = None


def _load_json(path: Path) -> dict[str, Any]:
    try:
        raw = path.read_text(encoding="utf-8")
    except OSError as err:  # pragma: no cover - filesystem error propagation
        raise ValueError(f"failed to read {path}: {err}") from err
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as err:
        raise ValueError(f"{path} does not contain valid JSON: {err}") from err
    if not isinstance(data, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return data


def _canonical_hostname(plan: dict[str, Any]) -> tuple[str, str | None]:
    dns_info = plan.get("dns") or {}
    if not isinstance(dns_info, dict):
        dns_info = {}
    hostname = dns_info.get("hostname")
    zone = dns_info.get("zone")
    if hostname:
        return str(hostname), zone
    gateway_host = ((plan.get("gateway_binding") or {}) or {}).get("hostname")
    if gateway_host:
        return str(gateway_host), zone
    alias = plan.get("alias") or {}
    if isinstance(alias, dict):
        namespace = alias.get("namespace")
        name = alias.get("name")
        if namespace and name and zone:
            hostname = f"{namespace}.{name}.{zone}".replace("..", ".").strip(".")
            return hostname, zone
    raise ValueError("cutover plan is missing `dns.hostname` and gateway binding hostname")


def _txt_record(ttl: int, values: Sequence[str]) -> dict[str, Any]:
    return {"type": "TXT", "ttl": ttl, "text": list(values)}


def _metadata_txt_entries(plan: dict[str, Any]) -> list[str]:
    entries: list[str] = []
    alias = plan.get("alias") or {}
    if isinstance(alias, dict):
        namespace = alias.get("namespace")
        name = alias.get("name")
        if namespace and name:
            entries.append(f"Sora-Alias={namespace}:{name}")
        digest = alias.get("manifest_digest_hex")
        if digest:
            entries.append(f"AliasManifest={digest}")
    manifest_hex = ((plan.get("manifest") or {}) or {}).get("blake3_hex")
    if manifest_hex:
        entries.append(f"ManifestDigest={manifest_hex}")
    car_digest = ((plan.get("car") or {}) or {}).get("car_digest_hex")
    if car_digest:
        entries.append(f"CarDigest={car_digest}")
    release = plan.get("release") or {}
    if isinstance(release, dict) and release.get("tag"):
        entries.append(f"ReleaseTag={release['tag']}")
    change_ticket = plan.get("change_ticket")
    if change_ticket:
        entries.append(f"ChangeTicket={change_ticket}")
    dns_info = plan.get("dns") or {}
    if isinstance(dns_info, dict) and dns_info.get("zone"):
        entries.append(f"DNSZone={dns_info['zone']}")
    return entries


def _freeze_txt_entries(metadata: dict[str, Any]) -> list[str]:
    entries: list[str] = []
    state = metadata.get("state")
    if state:
        entries.append(f"Sora-Freeze-State={state}")
    ticket = metadata.get("ticket")
    if ticket:
        entries.append(f"Sora-Freeze-Ticket={ticket}")
    expires_at = metadata.get("expires_at")
    if expires_at:
        entries.append(f"Sora-Freeze-ExpiresAt={expires_at}")
    for note in metadata.get("notes", []):
        entries.append(f"Sora-Freeze-Note={note}")
    return entries


def _freeze_metadata(options: ZonefileOptions) -> dict[str, Any] | None:
    metadata: dict[str, Any] = {}
    if options.freeze_state:
        metadata["state"] = options.freeze_state
    if options.freeze_ticket:
        metadata["ticket"] = options.freeze_ticket
    if options.freeze_expires_at:
        metadata["expires_at"] = options.freeze_expires_at
    if options.freeze_notes:
        metadata["notes"] = list(options.freeze_notes)
    if metadata:
        return metadata
    return None


def _format_utc(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def _derive_zonefile_version(
    plan: dict[str, Any],
    options: ZonefileOptions,
) -> str:
    if options.version:
        return options.version
    release = plan.get("release") or {}
    if isinstance(release, dict):
        tag = release.get("tag")
        if tag:
            return str(tag)
    ticket = plan.get("change_ticket")
    if ticket:
        return str(ticket)
    return _format_utc(options.now)


def _derive_effective_at(plan: dict[str, Any], options: ZonefileOptions) -> str:
    if options.effective_at:
        return options.effective_at
    window = plan.get("cutover_window")
    if isinstance(window, str) and window:
        start = window.split("/", 1)[0].strip()
        if start:
            return _normalize_iso_timestamp(start)
    return _format_utc(options.now)


def _derive_cid(plan: dict[str, Any], options: ZonefileOptions) -> str | None:
    if options.cid_override:
        return options.cid_override
    gateway_binding = plan.get("gateway_binding") or {}
    if isinstance(gateway_binding, dict):
        candidate = gateway_binding.get("content_cid")
        if candidate:
            return str(candidate)
        headers = gateway_binding.get("headers") or {}
        if isinstance(headers, dict):
            candidate = headers.get("Sora-Content-CID")
            if candidate:
                return str(candidate)
    alias = plan.get("alias") or {}
    if isinstance(alias, dict):
        candidate = alias.get("manifest_digest_hex")
        if candidate:
            return str(candidate)
    manifest = plan.get("manifest") or {}
    if isinstance(manifest, dict):
        candidate = manifest.get("blake3_hex")
        if candidate:
            return str(candidate)
    car = plan.get("car") or {}
    if isinstance(car, dict):
        candidate = car.get("car_digest_hex")
        if candidate:
            return str(candidate)
    return None


def _normalize_hex_digest(value: str, label: str) -> str:
    text = value.strip().lower()
    if text.startswith("0x"):
        text = text[2:]
    if len(text) != 64 or any(ch not in string.hexdigits for ch in text):
        raise ValueError(
            f"{label} must be a 32-byte hex digest (found `{value}`)"
        )
    return text


def _derive_gar_digest(plan: dict[str, Any], options: ZonefileOptions) -> str | None:
    if options.gar_digest:
        return options.gar_digest
    gateway_binding = plan.get("gateway_binding") or {}
    if isinstance(gateway_binding, dict):
        headers = gateway_binding.get("headers") or {}
        if isinstance(headers, dict):
            digest = headers.get("Sora-GAR-Digest")
            if digest:
                return _normalize_hex_digest(str(digest), "Sora-GAR-Digest")
    return None


def _derive_proof_literal(
    plan: dict[str, Any],
    options: ZonefileOptions,
) -> str | None:
    if options.proof_override:
        return options.proof_override
    gateway_binding = plan.get("gateway_binding") or {}
    if isinstance(gateway_binding, dict):
        headers = gateway_binding.get("headers") or {}
        if isinstance(headers, dict):
            proof = headers.get("Sora-Proof")
            if proof:
                return str(proof)
        proof = gateway_binding.get("proof_status")
        if proof:
            return str(proof)
    return None


def build_zonefile(
    plan: dict[str, Any],
    options: ZonefileOptions,
) -> dict[str, Any]:
    if options.ttl <= 0:
        raise ValueError("TTL must be greater than zero")
    hostname, zone = _canonical_hostname(plan)
    records: list[dict[str, Any]] = []
    for address in options.ipv4:
        records.append({"type": "A", "ttl": options.ttl, "address": address})
    for address in options.ipv6:
        records.append({"type": "AAAA", "ttl": options.ttl, "address": address})
    if options.cname_target:
        records.append(
            {"type": "CNAME", "ttl": options.ttl, "target": options.cname_target}
        )

    metadata_entries = _metadata_txt_entries(plan)
    gateway_binding = plan.get("gateway_binding") or {}
    if isinstance(gateway_binding, dict):
        alias_label = gateway_binding.get("alias")
        content_cid = gateway_binding.get("content_cid")
        proof_status = gateway_binding.get("proof_status")
        route_binding = gateway_binding.get("headers", {}).get("Sora-Route-Binding")
        if alias_label:
            metadata_entries.append(f"Sora-Name={alias_label}")
        if content_cid:
            metadata_entries.append(f"Sora-Content-CID={content_cid}")
        if proof_status:
            metadata_entries.append(f"Sora-Proof-Status={proof_status}")
        if route_binding:
            metadata_entries.append(f"Sora-Route-Binding={route_binding}")
        headers = gateway_binding.get("headers")
        if isinstance(headers, dict):
            for key, value in sorted(headers.items()):
                records.append(
                    _txt_record(options.ttl, [f"{key}={value}"])
                )
    metadata_entries.extend(options.extra_txt)
    freeze_metadata = _freeze_metadata(options)
    if freeze_metadata:
        metadata_entries.extend(_freeze_txt_entries(freeze_metadata))
    if metadata_entries:
        records.append(_txt_record(options.ttl, metadata_entries))
    if options.spki_pins:
        records.append(
            _txt_record(
                options.ttl,
                [f"Sora-SPKI=sha256/{pin}" for pin in options.spki_pins],
            )
        )
    sora_proof = (gateway_binding or {}).get("headers", {}).get("Sora-Proof")
    if sora_proof:
        records.append(_txt_record(options.ttl, [f"Sora-Proof={sora_proof}"]))

    if not records:
        raise ValueError(
            "no DNS records were generated – provide at least one address, CNAME, or TXT value"
        )

    skeleton = {
        "schema_version": _SCHEMA_VERSION,
        "generated_at": options.now.isoformat(),
        "source_cutover_plan": options.source_path,
        "dns": {"hostname": hostname, "zone": zone},
        "alias": plan.get("alias"),
        "release": plan.get("release"),
        "gateway_binding": {
            "alias": (gateway_binding or {}).get("alias"),
            "hostname": (gateway_binding or {}).get("hostname"),
            "content_cid": (gateway_binding or {}).get("content_cid"),
            "proof_status": (gateway_binding or {}).get("proof_status"),
        },
        "static_zone": {"domain": hostname, "records": records},
    }
    if freeze_metadata:
        skeleton["freeze"] = freeze_metadata
    version = _derive_zonefile_version(plan, options)
    effective_at = _derive_effective_at(plan, options)
    cid = _derive_cid(plan, options)
    if not cid:
        raise ValueError(
            "could not derive a CID from the cutover plan; provide --zonefile-cid"
        )
    gar_digest = _derive_gar_digest(plan, options)
    has_gateway_binding = isinstance(gateway_binding, dict) and bool(gateway_binding)
    if has_gateway_binding and not gar_digest:
        raise ValueError(
            "gateway cutovers must record a GAR digest; pass --gar-digest or include a Sora-GAR-Digest header"
        )
    proof_literal = _derive_proof_literal(plan, options)
    zonefile_metadata: dict[str, Any] = {
        "name": hostname,
        "version": version,
        "ttl": options.ttl,
        "effective_at": effective_at,
        "cid": cid,
    }
    if gar_digest:
        zonefile_metadata["gar_digest"] = gar_digest
    if proof_literal:
        zonefile_metadata["proof"] = proof_literal
    skeleton["zonefile"] = zonefile_metadata
    return skeleton


def _write_json(path: Path, payload: Any, pretty: bool) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if pretty:
        path.write_text(
            json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    else:
        path.write_text(json.dumps(payload) + "\n", encoding="utf-8")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate SoraDNS zonefile skeletons from DNS cutover descriptors.",
    )
    parser.add_argument(
        "--cutover-plan",
        required=True,
        help="Path to `portal.dns-cutover.json` (from docs portal tooling).",
    )
    parser.add_argument(
        "--out",
        required=True,
        help="Destination path for the zonefile skeleton JSON (e.g., artifacts/sns/zonefiles/<zone>/<name>.json).",
    )
    parser.add_argument(
        "--resolver-snippet-out",
        help="Optional path for a resolver-ready snippet containing `static_zones`.",
    )
    parser.add_argument(
        "--ttl",
        type=int,
        default=300,
        help="Record TTL seconds (default: 300).",
    )
    parser.add_argument(
        "--ipv4",
        action="append",
        default=[],
        help="IPv4 address to publish (repeat for multiple).",
    )
    parser.add_argument(
        "--ipv6",
        action="append",
        default=[],
        help="IPv6 address to publish (repeat for multiple).",
    )
    parser.add_argument(
        "--cname-target",
        help="Optional CNAME target (e.g., docs.gateway.sora.net).",
    )
    parser.add_argument(
        "--txt",
        action="append",
        default=[],
        help="Additional TXT entry in key=value form (repeatable).",
    )
    parser.add_argument(
        "--spki-pin",
        action="append",
        default=[],
        help="SHA-256 SPKI pin (base64). Repeat for multiple pins.",
    )
    parser.add_argument(
        "--freeze-state",
        help=(
            "Guardian freeze state to record (soft, hard, thawing, monitoring, emergency)."
        ),
    )
    parser.add_argument(
        "--freeze-ticket",
        help="Guardian/council ticket reference associated with the freeze.",
    )
    parser.add_argument(
        "--freeze-expires-at",
        help="Expected thaw timestamp in RFC3339/ISO 8601 format (e.g., 2025-03-10T12:00Z).",
    )
    parser.add_argument(
        "--freeze-note",
        action="append",
        default=[],
        help="Additional freeze note (repeat for multiple notes).",
    )
    parser.add_argument(
        "--zonefile-version",
        help="Optional version label for the zonefile skeleton (defaults to release tag or timestamp).",
    )
    parser.add_argument(
        "--effective-at",
        help="ISO 8601 timestamp indicating when the zonefile becomes active (defaults to the cutover window start).",
    )
    parser.add_argument(
        "--gar-digest",
        help="BLAKE3-256 digest of the signed GAR payload (hex, required for gateway bindings when not present in Sora-GAR-Digest header).",
    )
    parser.add_argument(
        "--zonefile-cid",
        help="Override the CID recorded in the zonefile metadata (defaults to the gateway binding content CID).",
    )
    parser.add_argument(
        "--zonefile-proof",
        help="Override the proof literal recorded in the zonefile metadata (defaults to the Sora-Proof header when present).",
    )
    parser.add_argument(
        "--pretty",
        action="store_true",
        help="Pretty-print JSON output.",
    )
    return parser.parse_args(argv)


def _normalize_freeze_state(value: str) -> str:
    normalized = value.strip().lower().replace(" ", "_")
    if normalized not in _FREEZE_STATE_CHOICES:
        allowed = ", ".join(sorted(_FREEZE_STATE_CHOICES))
        raise ValueError(
            f"freeze state '{value}' is not supported; choose one of: {allowed}"
        )
    return normalized


def _normalize_iso_timestamp(value: str) -> str:
    text = value.strip()
    if text.endswith("Z"):
        text = f"{text[:-1]}+00:00"
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError as err:  # pragma: no cover - sanity check error path
        raise ValueError(f"freeze expires-at timestamp '{value}' is invalid: {err}") from err
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    normalized = parsed.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")
    return normalized


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    plan_path = Path(args.cutover_plan).resolve()
    out_path = Path(args.out).resolve()
    resolver_path = Path(args.resolver_snippet_out).resolve() if args.resolver_snippet_out else None
    try:
        freeze_state = (
            _normalize_freeze_state(args.freeze_state) if args.freeze_state else None
        )
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    try:
        freeze_expires_at = (
            _normalize_iso_timestamp(args.freeze_expires_at)
            if args.freeze_expires_at
            else None
        )
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    freeze_metadata_requested = bool(
        args.freeze_ticket or args.freeze_expires_at or args.freeze_note
    )
    if freeze_metadata_requested and not freeze_state:
        print(
            "error: --freeze-ticket/--freeze-expires-at/--freeze-note require --freeze-state",
            file=sys.stderr,
        )
        return 1
    try:
        effective_at = (
            _normalize_iso_timestamp(args.effective_at)
            if args.effective_at
            else None
        )
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    try:
        gar_digest = (
            _normalize_hex_digest(args.gar_digest, "--gar-digest")
            if args.gar_digest
            else None
        )
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    cid_override = args.zonefile_cid.strip() if args.zonefile_cid else None
    proof_override = args.zonefile_proof.strip() if args.zonefile_proof else None
    version_override = args.zonefile_version.strip() if args.zonefile_version else None
    try:
        plan = _load_json(plan_path)
        skeleton = build_zonefile(
            plan,
            ZonefileOptions(
                ttl=args.ttl,
                ipv4=tuple(args.ipv4),
                ipv6=tuple(args.ipv6),
                cname_target=args.cname_target,
                extra_txt=tuple(args.txt),
                spki_pins=tuple(args.spki_pin),
                source_path=str(plan_path),
                now=datetime.now(timezone.utc),
                freeze_state=freeze_state,
                freeze_ticket=args.freeze_ticket,
                freeze_expires_at=freeze_expires_at,
                freeze_notes=tuple(args.freeze_note),
                version=version_override,
                effective_at=effective_at,
                gar_digest=gar_digest,
                cid_override=cid_override,
                proof_override=proof_override,
            ),
        )
    except ValueError as err:
        print(f"error: {err}", file=sys.stderr)
        return 1
    _write_json(out_path, skeleton, args.pretty)
    if resolver_path:
        snippet_zone = dict(skeleton["static_zone"])
        freeze_metadata = skeleton.get("freeze")
        if freeze_metadata:
            snippet_zone["freeze"] = freeze_metadata
        resolver_payload = {"static_zones": [snippet_zone]}
        _write_json(resolver_path, resolver_payload, args.pretty)
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
