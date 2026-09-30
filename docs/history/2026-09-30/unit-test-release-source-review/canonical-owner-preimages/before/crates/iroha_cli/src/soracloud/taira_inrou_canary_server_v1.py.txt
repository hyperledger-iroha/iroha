#!/usr/bin/env python3
"""Canonical stdlib-only HTTP server for the Taira Inrou V1 canary."""

import hashlib
import json
import os
import stat
from http.server import BaseHTTPRequestHandler, HTTPServer


STATE_SCHEMA_VERSION = 1
STATE_FILE_NAME = "taira-inrou-canary-state-v1.json"
STATE_FILE_MAX_BYTES = 1024
MAX_BOOT_SEQUENCE = (1 << 64) - 1
APP_DATA_MOUNT_PATH_V1 = "/var/lib/soracloud/volumes/app_data"
APP_DATA_MARKER_DOMAIN_V1 = b"iroha:taira:inrou-canary:app-data-marker:v1\0"
GUEST_BOOT_ID_DOMAIN_V1 = b"iroha:taira:inrou-canary:guest-boot-id:v1\0"


def require_nonblank_environment(name):
    """Return one exact non-blank environment value or reject startup."""
    value = os.environ.get(name)
    if value is None or not value or value != value.strip():
        raise ValueError(f"{name} must be set to a non-blank value")
    return value


def require_decimal_environment(name, minimum, maximum):
    """Return one canonical base-10 environment integer in the given range."""
    raw = require_nonblank_environment(name)
    if not raw.isascii() or not raw.isdecimal():
        raise ValueError(f"{name} must be a canonical base-10 integer")
    value = int(raw, 10)
    if raw != str(value) or not minimum <= value <= maximum:
        raise ValueError(
            f"{name} must be a canonical base-10 integer from {minimum} to {maximum}"
        )
    return value


def require_canonical_guest_boot_id(value):
    """Return one exact lowercase Linux boot UUID or reject it."""
    if not isinstance(value, str) or len(value) != 36:
        raise ValueError("guest boot ID must be one canonical lowercase UUID")
    for index, character in enumerate(value):
        if index in (8, 13, 18, 23):
            if character != "-":
                raise ValueError("guest boot ID must be one canonical lowercase UUID")
        elif character not in "0123456789abcdef":
            raise ValueError("guest boot ID must be one canonical lowercase UUID")
    return value


def read_guest_boot_id():
    """Read the exact identity of the current Linux kernel boot."""
    with open("/proc/sys/kernel/random/boot_id", "rb", buffering=0) as boot_file:
        raw = boot_file.read(38)
    if len(raw) != 37 or not raw.endswith(b"\n"):
        raise ValueError("guest kernel exposed a noncanonical boot ID")
    try:
        value = raw[:-1].decode("ascii")
    except UnicodeDecodeError as error:
        raise ValueError("guest kernel exposed a non-ASCII boot ID") from error
    return require_canonical_guest_boot_id(value)


def sha256_hex(domain, payload):
    """Hash one V1-framed byte string for public evidence."""
    return hashlib.sha256(domain + payload).hexdigest()


def guest_boot_id_sha256(guest_boot_id):
    """Return the domain-separated digest of one canonical guest boot ID."""
    canonical = require_canonical_guest_boot_id(guest_boot_id)
    return sha256_hex(GUEST_BOOT_ID_DOMAIN_V1, canonical.encode("ascii"))


def require_app_data_environment():
    """Return the one exact runtime-owned app-data volume projection."""
    directory = require_nonblank_environment("SORACLOUD_LEASE_VOLUME_APP_DATA_DIR")
    mount_path = require_nonblank_environment(
        "SORACLOUD_LEASE_VOLUME_APP_DATA_MOUNT_PATH"
    )
    if directory != mount_path:
        raise ValueError("app-data directory and mount path must be identical")
    if not os.path.isabs(directory) or os.path.normpath(directory) != directory:
        raise ValueError("app-data directory must be one canonical absolute path")
    if directory != APP_DATA_MOUNT_PATH_V1:
        raise ValueError(
            f"app-data directory must be exactly {APP_DATA_MOUNT_PATH_V1}"
        )
    return directory


def open_app_data_directory(path):
    """Open and attest the exact owner-only app-data mount directory."""
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory_fd = os.open(path, flags)
    try:
        metadata = os.fstat(directory_fd)
        if not stat.S_ISDIR(metadata.st_mode):
            raise ValueError("app-data path is not a directory")
        if metadata.st_uid != os.geteuid():
            raise ValueError("app-data directory is not owned by the service user")
        if stat.S_IMODE(metadata.st_mode) != 0o700:
            raise ValueError("app-data directory mode must be exactly 0700")
    except BaseException:
        os.close(directory_fd)
        raise
    return directory_fd


def reject_duplicate_object_pairs(pairs):
    """Construct one JSON object while rejecting duplicate keys."""
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate state field: {key}")
        result[key] = value
    return result


def canonical_state_bytes(state):
    """Encode one exact, ordered V1 durable-state object."""
    ordered = {
        "schema_version": state["schema_version"],
        "service": state["service"],
        "service_version": state["service_version"],
        "replica_slot": state["replica_slot"],
        "marker_hex": state["marker_hex"],
        "boot_sequence": state["boot_sequence"],
        "last_guest_boot_id_sha256": state["last_guest_boot_id_sha256"],
    }
    return json.dumps(
        ordered, ensure_ascii=True, separators=(",", ":")
    ).encode("ascii")


def require_lower_hex(value, label):
    """Return one exact lowercase 32-byte hexadecimal value."""
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise ValueError(f"{label} must be exactly 64 lowercase hexadecimal characters")
    return value


def validate_state(state, raw, service_name, service_version, replica_slot):
    """Validate one exact V1 durable-state object and its canonical bytes."""
    expected_fields = {
        "schema_version",
        "service",
        "service_version",
        "replica_slot",
        "marker_hex",
        "boot_sequence",
        "last_guest_boot_id_sha256",
    }
    if not isinstance(state, dict) or set(state) != expected_fields:
        raise ValueError("durable state must contain the exact V1 field set")
    if type(state["schema_version"]) is not int or state["schema_version"] != 1:
        raise ValueError("durable state schema_version must be exactly 1")
    if state["service"] != service_name:
        raise ValueError("durable state service identity does not match this workload")
    if state["service_version"] != service_version:
        raise ValueError("durable state service version does not match this workload")
    if type(state["replica_slot"]) is not int or state["replica_slot"] != replica_slot:
        raise ValueError("durable state replica slot does not match this workload")
    require_lower_hex(state["marker_hex"], "durable marker")
    sequence = state["boot_sequence"]
    if type(sequence) is not int or not 1 <= sequence <= MAX_BOOT_SEQUENCE:
        raise ValueError("durable boot sequence is outside the V1 u64 range")
    require_lower_hex(
        state["last_guest_boot_id_sha256"], "durable guest boot ID digest"
    )
    if canonical_state_bytes(state) != raw:
        raise ValueError("durable state bytes are not canonical V1 JSON")
    return state


def read_state(directory_fd, service_name, service_version, replica_slot):
    """Read and validate durable state, or return None only for first boot."""
    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
    try:
        state_fd = os.open(STATE_FILE_NAME, flags, dir_fd=directory_fd)
    except FileNotFoundError:
        return None
    try:
        metadata = os.fstat(state_fd)
        if not stat.S_ISREG(metadata.st_mode):
            raise ValueError("durable state is not a regular file")
        if metadata.st_nlink != 1:
            raise ValueError("durable state must have exactly one filesystem link")
        if metadata.st_uid != os.geteuid():
            raise ValueError("durable state is not owned by the service user")
        if stat.S_IMODE(metadata.st_mode) != 0o600:
            raise ValueError("durable state mode must be exactly 0600")
        chunks = []
        remaining = STATE_FILE_MAX_BYTES + 1
        while remaining:
            chunk = os.read(state_fd, remaining)
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        raw = b"".join(chunks)
        if not raw or len(raw) > STATE_FILE_MAX_BYTES:
            raise ValueError("durable state length is outside the V1 bound")
    finally:
        os.close(state_fd)
    try:
        state = json.loads(
            raw.decode("ascii"), object_pairs_hook=reject_duplicate_object_pairs
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("durable state is not exact ASCII JSON") from error
    return validate_state(state, raw, service_name, service_version, replica_slot)


def write_state(directory_fd, state):
    """Atomically and durably install one canonical V1 state revision."""
    payload = canonical_state_bytes(state)
    temporary_name = None
    temporary_fd = None
    try:
        for _attempt in range(128):
            candidate = f".{STATE_FILE_NAME}.{os.urandom(16).hex()}.tmp"
            try:
                temporary_fd = os.open(
                    candidate,
                    os.O_WRONLY
                    | os.O_CREAT
                    | os.O_EXCL
                    | os.O_NOFOLLOW
                    | os.O_CLOEXEC,
                    0o600,
                    dir_fd=directory_fd,
                )
                temporary_name = candidate
                break
            except FileExistsError:
                continue
        if temporary_fd is None or temporary_name is None:
            raise OSError("unable to allocate an exclusive durable-state staging file")
        os.fchmod(temporary_fd, 0o600)
        offset = 0
        while offset < len(payload):
            written = os.write(temporary_fd, payload[offset:])
            if written <= 0:
                raise OSError("short write while staging durable state")
            offset += written
        os.fsync(temporary_fd)
        os.close(temporary_fd)
        temporary_fd = None
        os.replace(
            temporary_name,
            STATE_FILE_NAME,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
        )
        temporary_name = None
        os.fsync(directory_fd)
    finally:
        if temporary_fd is not None:
            os.close(temporary_fd)
        if temporary_name is not None:
            try:
                os.unlink(temporary_name, dir_fd=directory_fd)
            except FileNotFoundError:
                pass


def load_or_create_health_state(
    app_data_path, service_name, service_version, replica_slot, guest_boot_id
):
    """Load durable identity and advance it only for a new guest kernel boot."""
    boot_digest = guest_boot_id_sha256(guest_boot_id)
    directory_fd = open_app_data_directory(app_data_path)
    try:
        state = read_state(
            directory_fd, service_name, service_version, replica_slot
        )
        if state is None:
            state = {
                "schema_version": STATE_SCHEMA_VERSION,
                "service": service_name,
                "service_version": service_version,
                "replica_slot": replica_slot,
                "marker_hex": os.urandom(32).hex(),
                "boot_sequence": 1,
                "last_guest_boot_id_sha256": boot_digest,
            }
            write_state(directory_fd, state)
        elif state["last_guest_boot_id_sha256"] != boot_digest:
            if state["boot_sequence"] == MAX_BOOT_SEQUENCE:
                raise ValueError("durable boot sequence cannot advance beyond u64")
            state = dict(state)
            state["boot_sequence"] += 1
            state["last_guest_boot_id_sha256"] = boot_digest
            write_state(directory_fd, state)
    finally:
        os.close(directory_fd)
    return state


def health_payload(service_name, service_version, replica_slot, state):
    """Build the exact nine-field public health evidence object."""
    marker = bytes.fromhex(state["marker_hex"])
    return {
        "schema_version": STATE_SCHEMA_VERSION,
        "service": service_name,
        "service_version": service_version,
        "runtime": "Inrou",
        "replica_slot": replica_slot,
        "identity": f"{service_name}:replica:{replica_slot}",
        "app_data_marker_sha256": sha256_hex(APP_DATA_MARKER_DOMAIN_V1, marker),
        "boot_sequence": state["boot_sequence"],
        "guest_boot_id_sha256": state["last_guest_boot_id_sha256"],
    }


class HealthHandler(BaseHTTPRequestHandler):
    """Serve the exact Taira Inrou V1 health identity."""

    payload = {}

    def do_GET(self):
        """Serve only the canonical health route."""
        if self.path.partition("?")[0] != "/health":
            self.send_error(404)
            return
        payload = json.dumps(
            self.payload, ensure_ascii=True, separators=(",", ":")
        ).encode("ascii")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, format_string, *args):
        """Keep the release canary quiet except for startup failures."""
        del format_string, args


def main():
    """Validate the runtime projection and durable state before binding."""
    try:
        port = require_decimal_environment("PORT", 1, 65535)
        service_name = require_nonblank_environment("HTTP_SERVICE_NAME")
        replica_slot = require_decimal_environment("SORACLOUD_REPLICA_SLOT", 1, 65535)
        service_version = require_nonblank_environment("SORACLOUD_SERVICE_VERSION")
        app_data_path = require_app_data_environment()
        state = load_or_create_health_state(
            app_data_path,
            service_name,
            service_version,
            replica_slot,
            read_guest_boot_id(),
        )
        HealthHandler.payload = health_payload(
            service_name, service_version, replica_slot, state
        )
    except (OSError, ValueError) as error:
        raise SystemExit(f"Taira Inrou configuration error: {error}") from None
    HTTPServer(("0.0.0.0", port), HealthHandler).serve_forever()


if __name__ == "__main__":
    main()
