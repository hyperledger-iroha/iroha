"""Typed models for Torii Connect registry and policy responses."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List, Mapping, Optional, Sequence


@dataclass(frozen=True)
class ConnectAppRecord:
    """Registered Connect application metadata."""

    app_id: str
    display_name: Optional[str]
    description: Optional[str]
    icon_url: Optional[str]
    namespaces: Sequence[str]
    metadata: Mapping[str, Any]
    policy: Mapping[str, Any]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAppRecord":
        if not isinstance(payload, Mapping):
            raise TypeError("connect app entry must be an object")
        data = dict(payload)
        app_id = data.get("app_id")
        if not isinstance(app_id, str) or not app_id:
            raise TypeError("connect app entry requires string `app_id` field")

        def _coerce_optional_str(name: str) -> Optional[str]:
            value = data.get(name)
            if value is None:
                return None
            if isinstance(value, str):
                return value
            raise TypeError(f"connect app entry `{name}` must be a string when present")

        namespaces_raw = data.get("namespaces") or []
        if namespaces_raw is None:
            namespaces_raw = []
        if not isinstance(namespaces_raw, list):
            raise TypeError("connect app entry `namespaces` must be a list")
        namespaces: List[str] = []
        for item in namespaces_raw:
            if not isinstance(item, str):
                raise TypeError("connect app entry `namespaces` must contain strings")
            namespaces.append(item)

        metadata_raw = data.get("metadata") or {}
        if metadata_raw is None:
            metadata_raw = {}
        if not isinstance(metadata_raw, Mapping):
            raise TypeError("connect app entry `metadata` must be an object")

        policy_raw = data.get("policy") or {}
        if policy_raw is None:
            policy_raw = {}
        if not isinstance(policy_raw, Mapping):
            raise TypeError("connect app entry `policy` must be an object")

        recognized = {
            "app_id",
            "display_name",
            "description",
            "icon_url",
            "namespaces",
            "metadata",
            "policy",
        }

        extra = {key: value for key, value in data.items() if key not in recognized}
        return cls(
            app_id=app_id,
            display_name=_coerce_optional_str("display_name"),
            description=_coerce_optional_str("description"),
            icon_url=_coerce_optional_str("icon_url"),
            namespaces=tuple(namespaces),
            metadata=dict(metadata_raw),
            policy=dict(policy_raw),
            extra=extra,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Serialize the record back into a JSON-friendly mapping."""

        payload: Dict[str, Any] = dict(self.extra)
        payload["app_id"] = self.app_id
        if self.display_name is not None:
            payload["display_name"] = self.display_name
        if self.description is not None:
            payload["description"] = self.description
        if self.icon_url is not None:
            payload["icon_url"] = self.icon_url
        payload["namespaces"] = list(self.namespaces)
        payload["metadata"] = dict(self.metadata)
        payload["policy"] = dict(self.policy)
        return payload


@dataclass(frozen=True)
class ConnectAppRegistryPage:
    """Paginated Connect application registry results."""

    items: Sequence[ConnectAppRecord]
    total: Optional[int]
    next_cursor: Optional[str]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAppRegistryPage":
        if not isinstance(payload, Mapping):
            raise TypeError("connect app registry payload must be an object")
        data = dict(payload)
        items_raw = data.get("items") or []
        if items_raw is None:
            items_raw = []
        if not isinstance(items_raw, list):
            raise TypeError("connect app registry `items` must be a list")
        items = [ConnectAppRecord.from_payload(entry) for entry in items_raw]

        total_raw = data.get("total")
        total: Optional[int]
        if total_raw is None:
            total = None
        else:
            try:
                total = int(total_raw)
            except (TypeError, ValueError) as exc:
                raise TypeError(
                    "connect app registry `total` must be numeric when present"
                ) from exc

        cursor_raw = data.get("next_cursor")
        if cursor_raw is not None and not isinstance(cursor_raw, str):
            raise TypeError("connect app registry cursor must be a string when present")

        recognized = {"items", "total", "next_cursor"}
        extra = {key: value for key, value in data.items() if key not in recognized}
        return cls(items=tuple(items), total=total, next_cursor=cursor_raw, extra=extra)


@dataclass(frozen=True)
class ConnectAdmissionManifestEntry:
    """Admission control record for a Connect application."""

    app_id: str
    namespaces: Sequence[str]
    metadata: Mapping[str, Any]
    policy: Mapping[str, Any]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAdmissionManifestEntry":
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission entry must be an object")
        data = dict(payload)
        app_id = data.get("app_id")
        if not isinstance(app_id, str) or not app_id:
            raise TypeError("connect admission entry requires string `app_id` field")

        namespaces_raw = data.get("namespaces") or []
        if namespaces_raw is None:
            namespaces_raw = []
        if not isinstance(namespaces_raw, list):
            raise TypeError("connect admission entry `namespaces` must be a list")
        namespaces: List[str] = []
        for item in namespaces_raw:
            if not isinstance(item, str):
                raise TypeError("connect admission entry `namespaces` values must be strings")
            namespaces.append(item)

        metadata_raw = data.get("metadata") or {}
        if metadata_raw is None:
            metadata_raw = {}
        if not isinstance(metadata_raw, Mapping):
            raise TypeError("connect admission entry `metadata` must be an object")

        policy_raw = data.get("policy") or {}
        if policy_raw is None:
            policy_raw = {}
        if not isinstance(policy_raw, Mapping):
            raise TypeError("connect admission entry `policy` must be an object")

        recognized = {"app_id", "namespaces", "metadata", "policy"}
        extra = {key: value for key, value in data.items() if key not in recognized}
        return cls(
            app_id=app_id,
            namespaces=tuple(namespaces),
            metadata=dict(metadata_raw),
            policy=dict(policy_raw),
            extra=extra,
        )


@dataclass(frozen=True)
class ConnectAdmissionManifest:
    """Connect admission manifest describing allowed applications."""

    version: Optional[int]
    entries: Sequence[ConnectAdmissionManifestEntry]
    manifest_hash: Optional[str]
    updated_at: Optional[str]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAdmissionManifest":
        if not isinstance(payload, Mapping):
            raise TypeError("connect admission manifest payload must be an object")
        data = dict(payload)
        entries_raw = data.get("entries") or []
        if entries_raw is None:
            entries_raw = []
        if not isinstance(entries_raw, list):
            raise TypeError("connect admission manifest `entries` must be a list")
        entries = [ConnectAdmissionManifestEntry.from_payload(item) for item in entries_raw]

        version_raw = data.get("version")
        if version_raw is None:
            version: Optional[int] = None
        else:
            try:
                version = int(version_raw)
            except (TypeError, ValueError) as exc:
                raise TypeError("connect admission manifest `version` must be numeric") from exc

        manifest_hash = data.get("manifest_hash")
        if manifest_hash is not None and not isinstance(manifest_hash, str):
            raise TypeError(
                "connect admission manifest `manifest_hash` must be a string when present"
            )
        updated_at = data.get("updated_at")
        if updated_at is not None and not isinstance(updated_at, str):
            raise TypeError("connect admission manifest `updated_at` must be a string when present")

        recognized = {"entries", "version", "manifest_hash", "updated_at"}
        extra = {key: value for key, value in data.items() if key not in recognized}
        return cls(
            version=version,
            entries=tuple(entries),
            manifest_hash=manifest_hash,
            updated_at=updated_at,
            extra=extra,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Serialize the manifest to a JSON-serializable mapping."""

        payload: Dict[str, Any] = dict(self.extra)
        payload["entries"] = [
            {
                "app_id": entry.app_id,
                "namespaces": list(entry.namespaces),
                "metadata": dict(entry.metadata),
                "policy": dict(entry.policy),
                **dict(entry.extra),
            }
            for entry in self.entries
        ]
        if self.version is not None:
            payload["version"] = self.version
        if self.manifest_hash is not None:
            payload["manifest_hash"] = self.manifest_hash
        if self.updated_at is not None:
            payload["updated_at"] = self.updated_at
        return payload


@dataclass(frozen=True)
class ConnectAppPolicyControls:
    """Runtime-configurable Connect policy toggles."""

    relay_enabled: Optional[bool]
    ws_max_sessions: Optional[int]
    ws_per_ip_max_sessions: Optional[int]
    ws_rate_per_ip_per_min: Optional[int]
    session_ttl_ms: Optional[int]
    frame_max_bytes: Optional[int]
    session_buffer_max_bytes: Optional[int]
    ping_interval_ms: Optional[int]
    ping_miss_tolerance: Optional[int]
    ping_min_interval_ms: Optional[int]
    extra: Mapping[str, Any] = field(default_factory=dict, repr=False)

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "ConnectAppPolicyControls":
        if not isinstance(payload, Mapping):
            raise TypeError("connect app policy payload must be an object")
        data = dict(payload)

        def _coerce_optional_int(name: str) -> Optional[int]:
            value = data.get(name)
            if value is None:
                return None
            try:
                return int(value)
            except (TypeError, ValueError) as exc:
                raise TypeError(f"connect app policy field `{name}` must be numeric") from exc

        relay_enabled_raw = data.get("relay_enabled")
        relay_enabled: Optional[bool]
        if relay_enabled_raw is None:
            relay_enabled = None
        elif isinstance(relay_enabled_raw, bool):
            relay_enabled = relay_enabled_raw
        else:
            raise TypeError("connect app policy `relay_enabled` must be boolean when present")

        recognized = {
            "relay_enabled",
            "ws_max_sessions",
            "ws_per_ip_max_sessions",
            "ws_rate_per_ip_per_min",
            "session_ttl_ms",
            "frame_max_bytes",
            "session_buffer_max_bytes",
            "ping_interval_ms",
            "ping_miss_tolerance",
            "ping_min_interval_ms",
        }

        extra = {key: value for key, value in data.items() if key not in recognized}
        return cls(
            relay_enabled=relay_enabled,
            ws_max_sessions=_coerce_optional_int("ws_max_sessions"),
            ws_per_ip_max_sessions=_coerce_optional_int("ws_per_ip_max_sessions"),
            ws_rate_per_ip_per_min=_coerce_optional_int("ws_rate_per_ip_per_min"),
            session_ttl_ms=_coerce_optional_int("session_ttl_ms"),
            frame_max_bytes=_coerce_optional_int("frame_max_bytes"),
            session_buffer_max_bytes=_coerce_optional_int("session_buffer_max_bytes"),
            ping_interval_ms=_coerce_optional_int("ping_interval_ms"),
            ping_miss_tolerance=_coerce_optional_int("ping_miss_tolerance"),
            ping_min_interval_ms=_coerce_optional_int("ping_min_interval_ms"),
            extra=extra,
        )

    def to_payload(self) -> Dict[str, Any]:
        """Serialize the policy controls back to a JSON-serializable mapping."""

        payload: Dict[str, Any] = dict(self.extra)
        if self.relay_enabled is not None:
            payload["relay_enabled"] = self.relay_enabled
        if self.ws_max_sessions is not None:
            payload["ws_max_sessions"] = self.ws_max_sessions
        if self.ws_per_ip_max_sessions is not None:
            payload["ws_per_ip_max_sessions"] = self.ws_per_ip_max_sessions
        if self.ws_rate_per_ip_per_min is not None:
            payload["ws_rate_per_ip_per_min"] = self.ws_rate_per_ip_per_min
        if self.session_ttl_ms is not None:
            payload["session_ttl_ms"] = self.session_ttl_ms
        if self.frame_max_bytes is not None:
            payload["frame_max_bytes"] = self.frame_max_bytes
        if self.session_buffer_max_bytes is not None:
            payload["session_buffer_max_bytes"] = self.session_buffer_max_bytes
        if self.ping_interval_ms is not None:
            payload["ping_interval_ms"] = self.ping_interval_ms
        if self.ping_miss_tolerance is not None:
            payload["ping_miss_tolerance"] = self.ping_miss_tolerance
        if self.ping_min_interval_ms is not None:
            payload["ping_min_interval_ms"] = self.ping_min_interval_ms
        return payload
