"""High-level Connect CLI workflow.

This example script demonstrates how to:

1. Create a Torii client using the high-level Python SDK.
2. Open a Connect session and inspect the typed `ConnectSessionInfo`.
3. Construct a typed `ConnectControlOpen` frame and optionally post it back to Torii.

The script intentionally focuses on developer ergonomics rather than providing a
complete wallet implementation. It uses the same typed dataclasses that the SDK
exports so downstream tooling can easily adapt the flow to their own storage and
UI layers.

Example usage:

```bash
python -m iroha_python.examples.connect_flow \\
  --base-url http://127.0.0.1:8080 \\
  --network-id '<64-lowercase-hex-network-id>' \\
  --sid '<derived-base64url-sid>' \\
  --app-public-key '<32-byte-x25519-public-key-hex>' \\
  --nonce '<16-byte-nonce-hex>' \\
  --auth-token admin-token
```
"""

from __future__ import annotations

import argparse
import base64
import json
import os
from importlib import resources
from pathlib import Path
from typing import Any, Dict, List, Mapping, Optional

from iroha_python import (
    ConnectAppMetadata,
    ConnectControlOpen,
    ConnectDirection,
    ConnectFrame,
    ConnectKeyPair,
    ConnectPermissions,
    ConnectPreviewBootstrapResult,
    ConnectSessionInfo,
    NetworkId,
    bootstrap_connect_preview_session,
    connect_public_key_from_private,
    create_connect_session_preview,
    create_torii_client,
    encode_connect_frame,
)

_TEMPLATE_RESOURCE = "connect_app_metadata.json"
_DEFAULT_METHODS = ("SIGN_REQUEST_TX",)


def _decode_sid(value: str) -> bytes:
    """Decode one canonical unpadded 32-byte Connect SID."""

    if not value or value != value.strip() or "=" in value:
        raise ValueError("SID must be exact unpadded base64url")
    pad = "=" * (-len(value) % 4)
    decoded = base64.urlsafe_b64decode(value + pad)
    canonical = base64.urlsafe_b64encode(decoded).rstrip(b"=").decode("ascii")
    if len(decoded) != 32 or canonical != value:
        raise ValueError("SID must be canonical base64url for exactly 32 bytes")
    return decoded


def _print_session_info(info: ConnectSessionInfo) -> None:
    print("Connect session created")
    print(f"  SID:           {info.sid}")
    print(f"  App deeplink:  {info.app_uri}")
    print(f"  App token:     {info.app_token}")
    print(f"  Wallet token:  {info.wallet_token}")
    print(f"  Relay token:   {info.relay_token}")
    print("  Management token retained for cleanup/status requests")
    if info.expires_at is not None:
        print(f"  Expires at:    {info.expires_at.isoformat(timespec='seconds')}Z")


def _json_safe(value: Any) -> Any:
    if isinstance(value, (bytes, bytearray, memoryview)):
        return base64.b64encode(bytes(value)).decode("ascii")
    if isinstance(value, dict):
        return {key: _json_safe(val) for key, val in value.items()}
    if isinstance(value, list):
        return [_json_safe(item) for item in value]
    if isinstance(value, tuple):
        return [_json_safe(item) for item in value]
    return value


def _load_app_metadata_from_file(path: str) -> ConnectAppMetadata:
    location = Path(path)
    try:
        raw = json.loads(location.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValueError(f"metadata file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"metadata file is not valid JSON: {path}") from exc
    if not isinstance(raw, Mapping):
        raise ValueError("metadata file must contain a JSON object")
    name = raw.get("name")
    if not isinstance(name, str) or not name:
        raise ValueError("metadata file must include a non-empty string 'name'")
    url = raw.get("url")
    if url is not None and not isinstance(url, str):
        raise ValueError("metadata field 'url' must be a string when provided")
    icon_hash = raw.get("icon_hash")
    if icon_hash is not None and not isinstance(icon_hash, str):
        raise ValueError("metadata field 'icon_hash' must be a string when provided")
    return ConnectAppMetadata(name=name, url=url, icon_hash=icon_hash)


def _metadata_template_text() -> str:
    with resources.files("iroha_python.examples").joinpath(_TEMPLATE_RESOURCE).open(
        "r", encoding="utf-8"
    ) as handle:
        return handle.read()


def _write_metadata_template(path: str) -> None:
    destination = Path(path)
    if destination.exists():
        raise ValueError(f"metadata template output already exists: {path}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(_metadata_template_text(), encoding="utf-8")


def _parse_fixed_length_hex(value: str, *, field: str, expected_length: int) -> bytes:
    """Decode a fixed-length hex string (accepts optional 0x prefix)."""

    trimmed = value.strip().lower()
    if trimmed.startswith("0x"):
        trimmed = trimmed[2:]
    try:
        data = bytes.fromhex(trimmed)
    except ValueError as exc:  # pragma: no cover - validation path
        raise ValueError(f"{field} must be a valid hex string") from exc
    if len(data) != expected_length:
        raise ValueError(f"{field} must contain {expected_length} bytes (got {len(data)})")
    return data


def _preview_summary_payload(result: ConnectPreviewBootstrapResult) -> Dict[str, Any]:
    """Return a JSON-friendly summary describing a preview or registered session."""

    preview = result.preview
    payload: Dict[str, Any] = {
        "network_id": preview.network_id.literal,
        "node": preview.node,
        "sid_base64url": preview.sid_base64url,
        "nonce_hex": preview.nonce.hex(),
        "wallet_uri": preview.wallet_uri,
        "app_uri": preview.app_uri,
        "app_public_key": preview.app_key_pair.public_key.hex(),
        "app_private_key": preview.app_key_pair.private_key.hex(),
        "registered": result.session is not None,
    }
    if result.tokens is not None:
        payload["tokens"] = {
            "wallet": result.tokens.wallet,
            "app": result.tokens.app,
        }
    if result.session is not None:
        payload["session"] = result.session.as_dict()
    return payload


def _write_preview_summary(path: Path, result: ConnectPreviewBootstrapResult) -> None:
    """Persist preview metadata/tokens to disk."""

    payload = _preview_summary_payload(result)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def _print_preview_result(result: ConnectPreviewBootstrapResult) -> None:
    """Display preview information and optional tokens."""

    preview = result.preview
    print("Connect preview generated:")
    print(f"  Network ID:     {preview.network_id.literal}")
    if preview.node:
        print(f"  Node:           {preview.node}")
    print(f"  SID (base64url): {preview.sid_base64url}")
    print(f"  Wallet URI:     {preview.wallet_uri}")
    print(f"  App URI:        {preview.app_uri}")
    print(f"  App public key: {preview.app_key_pair.public_key.hex()}")
    print(f"  App private key: {preview.app_key_pair.private_key.hex()}")
    if result.tokens is not None:
        print("  Torii tokens (wallet/app):")
        print(f"    wallet: {result.tokens.wallet}")
        print(f"    app:    {result.tokens.app}")
    if result.session is not None:
        print(f"  Registered Torii session SID: {result.session.sid}")


def build_connect_open_frame(
    session: ConnectSessionInfo,
    *,
    app_public_key: bytes,
    network_id: NetworkId,
    methods: List[str],
    events: List[str],
    sequence: int = 1,
    metadata: Optional[ConnectAppMetadata] = None,
) -> ConnectFrame:
    """Construct a `ConnectFrame` carrying an `Open` control."""

    sid_bytes = _decode_sid(session.sid)
    if app_public_key != session.app_public_key or network_id != session.network_id:
        raise ValueError("Open inputs must match the registered exact Connect session")
    control = ConnectControlOpen(
        app_public_key=app_public_key,
        network_id=network_id,
        permissions=ConnectPermissions(methods=methods, events=events),
        metadata=metadata,
    )
    return ConnectFrame(
        sid=sid_bytes,
        direction=ConnectDirection.APP_TO_WALLET,
        sequence=sequence,
        control=control,
    )


def _run_preview_mode(args: argparse.Namespace, parser: argparse.ArgumentParser) -> None:
    """Handle `--mode preview` flows."""

    if args.write_app_metadata_template:
        parser.error("--write-app-metadata-template cannot be combined with --mode=preview")
    if not args.network_id:
        parser.error("--network-id is required when --mode=preview")
    try:
        network_id = NetworkId.parse(args.network_id)
    except ValueError as exc:
        parser.error(str(exc))

    node = args.preview_node or args.node
    nonce_bytes: Optional[bytes] = None
    if args.preview_nonce:
        try:
            nonce_bytes = _parse_fixed_length_hex(
                args.preview_nonce,
                field="--preview-nonce",
                expected_length=16,
            )
        except ValueError as exc:
            parser.error(str(exc))

    app_key_pair: Optional[ConnectKeyPair] = None
    if args.preview_app_private_key:
        try:
            private_key = _parse_fixed_length_hex(
                args.preview_app_private_key,
                field="--preview-app-private-key",
                expected_length=32,
            )
        except ValueError as exc:
            parser.error(str(exc))
        app_key_pair = ConnectKeyPair(
            private_key=private_key,
            public_key=connect_public_key_from_private(private_key),
        )

    session_options: Optional[Dict[str, Any]] = None
    if args.preview_session_node:
        session_options = {"node": args.preview_session_node}

    if args.preview_register:
        client = create_torii_client(
            args.base_url,
            auth_token=args.auth_token,
            api_token=args.api_token,
        )
        result = bootstrap_connect_preview_session(
            client,
            network_id=network_id,
            node=node,
            nonce=nonce_bytes,
            app_key_pair=app_key_pair,
            register=True,
            session_options=session_options,
        )
    else:
        preview = create_connect_session_preview(
            network_id=network_id,
            node=node,
            nonce=nonce_bytes,
            app_key_pair=app_key_pair,
        )
        result = ConnectPreviewBootstrapResult(
            preview=preview,
            session=None,
            tokens=None,
        )

    _print_preview_result(result)
    if args.preview_output:
        destination = Path(args.preview_output)
        _write_preview_summary(destination, result)
        print(f"Preview summary written to {destination}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Demonstrate a Connect session lifecycle")
    parser.add_argument(
        "--mode",
        choices=("session", "preview"),
        default="session",
        help="Select 'preview' to mint Connect previews or 'session' for the existing flow (default: %(default)s)",
    )
    parser.add_argument(
        "--base-url",
        default="http://127.0.0.1:8080",
        help="Torii base URL (default: %(default)s)",
    )
    parser.add_argument("--sid", help="Canonical unpadded base64url session identifier")
    parser.add_argument(
        "--network-id",
        help="Exact 64-character lowercase hexadecimal NetworkId for the deployment",
    )
    parser.add_argument("--nonce", help="Hex-encoded 16-byte Connect session nonce")
    parser.add_argument(
        "--node",
        default="http://127.0.0.1:8080",
        help="Node address embedded in the deeplink (default: %(default)s)",
    )
    parser.add_argument(
        "--auth-token",
        default=os.getenv("IROHA_TORII_AUTH_TOKEN"),
        help="Authorization bearer token (can also be provided via IROHA_TORII_AUTH_TOKEN)",
    )
    parser.add_argument(
        "--api-token",
        default=os.getenv("IROHA_TORII_API_TOKEN"),
        help="Optional X-API-Token header (or IROHA_TORII_API_TOKEN env)",
    )
    parser.add_argument(
        "--methods",
        action="append",
        dest="methods",
        default=list(_DEFAULT_METHODS),
        help="Requested Connect methods (repeatable, default: SIGN_REQUEST_TX)",
    )
    parser.add_argument(
        "--events",
        action="append",
        dest="events",
        default=[],
        help="Requested Connect events (repeatable)",
    )
    parser.add_argument(
        "--app-public-key",
        help="Hex-encoded 32-byte X25519 public key for the application.",
    )
    parser.add_argument(
        "--sequence",
        type=int,
        default=1,
        help="Sequence number for the Connect control frame (default: %(default)s)",
    )
    parser.add_argument(
        "--app-name",
        help="Optional display name for the application metadata embedded in the Open frame.",
    )
    parser.add_argument(
        "--app-url",
        help="Optional homepage URL advertised in the application metadata.",
    )
    parser.add_argument(
        "--app-icon-hash",
        help="Optional icon multihash (hex) advertised in the application metadata.",
    )
    parser.add_argument(
        "--app-metadata-file",
        help="Path to a JSON file containing application metadata (keys: name[, url, icon_hash]).",
    )
    parser.add_argument(
        "--write-app-metadata-template",
        metavar="PATH",
        help="Write a sample app metadata JSON to PATH and exit.",
    )
    parser.add_argument(
        "--send-open",
        action="store_true",
        help="POST the `ConnectControlOpen` payload to `/v1/connect/control/open` after construction.",
    )
    parser.add_argument(
        "--frame-output",
        help="Optional path for saving the encoded Connect frame (defaults to hex).",
    )
    parser.add_argument(
        "--frame-output-format",
        choices=("hex", "binary"),
        default="hex",
        help="Encoding used when writing `--frame-output` (default: %(default)s).",
    )
    parser.add_argument(
        "--frame-json-output",
        help="Optional path for a JSON serialization of the frame (base64-encodes binary fields).",
    )
    parser.add_argument(
        "--preview-register",
        action="store_true",
        help="When --mode=preview, also register the session with Torii and record the issued tokens.",
    )
    parser.add_argument(
        "--preview-output",
        help="When --mode=preview, write a JSON summary (preview metadata, tokens, session fields) to this path.",
    )
    parser.add_argument(
        "--preview-node",
        help="When --mode=preview, override the node embedded in the preview deeplinks.",
    )
    parser.add_argument(
        "--preview-session-node",
        help="Override the node stored in Torii when registering preview sessions (session options).",
    )
    parser.add_argument(
        "--preview-nonce",
        help="Hex-encoded 16-byte nonce for deterministic preview SID derivation (mode=preview).",
    )
    parser.add_argument(
        "--preview-app-private-key",
        help="Hex-encoded 32-byte X25519 private key for deterministic preview keypairs (mode=preview).",
    )
    args = parser.parse_args()

    if args.mode == "preview":
        _run_preview_mode(args, parser)
        return

    if args.write_app_metadata_template:
        if args.app_metadata_file or args.app_name or args.app_url or args.app_icon_hash:
            parser.error("--write-app-metadata-template cannot combine with metadata flags")
        try:
            _write_metadata_template(args.write_app_metadata_template)
        except ValueError as exc:
            parser.error(str(exc))
        print(f"Metadata template written to {args.write_app_metadata_template}")
        return
    if args.app_metadata_file and (args.app_name or args.app_url or args.app_icon_hash):
        parser.error("--app-metadata-file cannot be combined with --app-name/--app-url/--app-icon-hash")
    if (args.app_url or args.app_icon_hash) and not args.app_name:
        parser.error("--app-name is required when specifying --app-url or --app-icon-hash")
    if not args.sid:
        parser.error("--sid is required (provide --sid or use --write-app-metadata-template)")
    if not args.network_id:
        parser.error("--network-id is required")
    if not args.app_public_key:
        parser.error("--app-public-key is required")
    if not args.nonce:
        parser.error("--nonce is required")
    if args.sequence != 1:
        parser.error("Connect Open sequence must be exactly 1")

    client = create_torii_client(
        args.base_url,
        auth_token=args.auth_token,
        api_token=args.api_token,
    )

    try:
        network_id = NetworkId.parse(args.network_id)
        app_public_key = _parse_fixed_length_hex(
            args.app_public_key, field="--app-public-key", expected_length=32
        )
        nonce = _parse_fixed_length_hex(args.nonce, field="--nonce", expected_length=16)
        sid_bytes = _decode_sid(args.sid)
    except ValueError as exc:
        parser.error(str(exc))

    session_info = client.create_connect_session_info(
        {
            "sid": args.sid,
            "network_id": network_id.literal,
            "app_pk": base64.urlsafe_b64encode(app_public_key).rstrip(b"=").decode("ascii"),
            "nonce": base64.urlsafe_b64encode(nonce).rstrip(b"=").decode("ascii"),
            "node": args.node,
        }
    )
    _print_session_info(session_info)

    if _decode_sid(session_info.sid) != sid_bytes:
        raise RuntimeError("Torii substituted the registered Connect SID")

    if args.app_metadata_file:
        try:
            metadata = _load_app_metadata_from_file(args.app_metadata_file)
        except ValueError as exc:
            parser.error(str(exc))
            return  # Unreachable, parser.error raises SystemExit
    elif args.app_name:
        metadata = ConnectAppMetadata(
            name=args.app_name,
            url=args.app_url,
            icon_hash=args.app_icon_hash,
        )
    else:
        metadata = None

    frame = build_connect_open_frame(
        session_info,
        app_public_key=app_public_key,
        network_id=session_info.network_id,
        methods=list(dict.fromkeys(args.methods)),
        events=list(dict.fromkeys(args.events)),
        sequence=args.sequence,
        metadata=metadata,
    )

    encoded_frame = encode_connect_frame(frame)
    print("Encoded ConnectFrame (Open control):")
    print(encoded_frame.hex())

    if args.frame_output:
        destination = Path(args.frame_output)
        destination.parent.mkdir(parents=True, exist_ok=True)
        if args.frame_output_format == "binary":
            destination.write_bytes(encoded_frame)
        else:
            destination.write_text(encoded_frame.hex() + "\n", encoding="utf-8")
        print(f"Frame written to {destination} ({args.frame_output_format})")

    if args.frame_json_output:
        destination = Path(args.frame_json_output)
        destination.parent.mkdir(parents=True, exist_ok=True)
        json_payload = _json_safe(frame.to_dict())
        destination.write_text(json.dumps(json_payload, indent=2, sort_keys=True), encoding="utf-8")
        print(f"Frame JSON written to {destination}")

    if args.send_open:
        print("Posting ConnectControlOpen via REST ...")
        if frame.control is None:
            raise RuntimeError("connect open frame missing control payload")
        response = client.send_connect_control_frame(session_info.sid, frame.control)
        print("Torii response:", response)


if __name__ == "__main__":
    main()
