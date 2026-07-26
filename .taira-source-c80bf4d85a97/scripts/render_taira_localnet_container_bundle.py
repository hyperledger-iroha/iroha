#!/usr/bin/env python3
"""Render a `kagami localnet` bundle into container-ready Taira validator configs."""

from __future__ import annotations

import argparse
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any


MIN_VALIDATORS = 4
DEFAULT_NETWORK = "taira-localnet"
DEFAULT_CONTAINER_PREFIX = "taira-localnet-peer"
DEFAULT_IMAGE = "local/taira-validator:prebuilt"
DEFAULT_BASE_P2P_PORT = 31337
DEFAULT_BASE_TORII_PORT = 28080
DEFAULT_RUST_LOG = "info"
SIGNED_GENESIS_MOUNT_PATH = "/config/genesis.signed.nrt"
GENESIS_MOUNT_PATH = "/config/genesis.json"


@dataclass(frozen=True)
class PeerConfig:
    """Minimal peer metadata needed to rewrite the localnet bundle."""

    index: int
    stem: str
    config_path: Path
    public_key: str
    network_address: str
    public_address: str
    torii_address: str
    genesis_path: str
    kura_store_dir: str
    cold_store_root: str
    da_store_root: str


def _load_toml(path: Path) -> dict[str, Any]:
    try:
        import tomllib
    except ModuleNotFoundError:
        try:
            import tomli as tomllib
        except ModuleNotFoundError as error:  # pragma: no cover - environment specific
            raise SystemExit(
                "python3 must provide tomllib (Python 3.11+) or tomli to load peer configs"
            ) from error

    with path.open("rb") as handle:
        payload = tomllib.load(handle)
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a top-level TOML table")
    return payload


def _require_string(payload: dict[str, Any], key: str, context: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{context} field `{key}` must be a non-empty string")
    return value.strip()


def _replace_addr_host_port(value: str, host: str, port: int) -> str:
    match = re.fullmatch(r"addr:(.+)#[^#]+", value)
    if match is None:
        raise ValueError(f"unsupported address format: {value}")
    if ":" in host and not host.startswith("["):
        body = f"[{host}]:{port}"
    else:
        body = f"{host}:{port}"
    return _format_literal("addr", body)


def _format_literal(tag: str, body: str) -> str:
    crc = 0xFFFF
    for byte in tag.encode("utf-8") + b":" + body.encode("utf-8"):
        crc ^= byte << 8
        for _ in range(8):
            if crc & 0x8000:
                crc = ((crc << 1) ^ 0x1021) & 0xFFFF
            else:
                crc = (crc << 1) & 0xFFFF
    return f"{tag}:{body}#{crc:04X}"


def _replace_setting(content: str, key: str, old_value: str, new_value: str) -> str:
    needle = f'{key} = "{old_value}"'
    replacement = f'{key} = "{new_value}"'
    if needle not in content:
        raise ValueError(f"expected `{needle}` in localnet config")
    return content.replace(needle, replacement, 1)


def _replace_single_line(content: str, key: str, rendered_value: str) -> str:
    pattern = rf"^{re.escape(key)} = \[.*\]$"
    updated, count = re.subn(pattern, f"{key} = [{rendered_value}]", content, count=1, flags=re.MULTILINE)
    if count != 1:
        raise ValueError(f"expected a single `{key} = [...]` line in localnet config")
    return updated


def _discover_peers(bundle_dir: Path) -> list[PeerConfig]:
    peers: list[PeerConfig] = []
    for path in sorted(bundle_dir.glob("peer*.toml")):
        match = re.fullmatch(r"peer(\d+)", path.stem)
        if match is None:
            continue
        payload = _load_toml(path)
        network = payload.get("network")
        torii = payload.get("torii")
        genesis = payload.get("genesis")
        kura = payload.get("kura")
        tiered_state = payload.get("tiered_state")
        if not isinstance(network, dict) or not isinstance(torii, dict):
            raise ValueError(f"{path} must define `[network]` and `[torii]` tables")
        if (
            not isinstance(genesis, dict)
            or not isinstance(kura, dict)
            or not isinstance(tiered_state, dict)
        ):
            raise ValueError(
                f"{path} must define `[genesis]`, `[kura]`, and `[tiered_state]` tables"
            )
        peers.append(
            PeerConfig(
                index=int(match.group(1)),
                stem=path.stem,
                config_path=path,
                public_key=_require_string(payload, "public_key", f"{path}"),
                network_address=_require_string(network, "address", f"{path} [network]"),
                public_address=_require_string(network, "public_address", f"{path} [network]"),
                torii_address=_require_string(torii, "address", f"{path} [torii]"),
                genesis_path=_require_string(genesis, "file", f"{path} [genesis]"),
                kura_store_dir=_require_string(kura, "store_dir", f"{path} [kura]"),
                cold_store_root=_require_string(
                    tiered_state, "cold_store_root", f"{path} [tiered_state]"
                ),
                da_store_root=_require_string(
                    tiered_state, "da_store_root", f"{path} [tiered_state]"
                ),
            )
        )
    peers.sort(key=lambda peer: peer.index)
    if len(peers) < MIN_VALIDATORS:
        raise ValueError(
            f"{bundle_dir} must contain at least {MIN_VALIDATORS} peer*.toml files for a representative Taira rollout"
        )
    return peers


def _rewrite_config(content: str, peer: PeerConfig, peers: list[PeerConfig], hostnames: list[str]) -> str:
    trusted_peers = ", ".join(
        f'"{other.public_key}@{_replace_addr_host_port(other.public_address, hostnames[other.index], 1337)}"'
        for other in peers
    )
    telemetry_urls = ", ".join(f'"http://{hostname}:8080/"' for hostname in hostnames)

    content = _replace_setting(
        content,
        "address",
        peer.network_address,
        _replace_addr_host_port(peer.network_address, "0.0.0.0", 1337),
    )
    content = _replace_setting(
        content,
        "public_address",
        peer.public_address,
        _replace_addr_host_port(peer.public_address, hostnames[peer.index], 1337),
    )
    content = _replace_setting(
        content,
        "address",
        peer.torii_address,
        _replace_addr_host_port(peer.torii_address, "0.0.0.0", 8080),
    )
    content = _replace_single_line(content, "trusted_peers", trusted_peers)
    content = _replace_setting(content, "file", peer.genesis_path, SIGNED_GENESIS_MOUNT_PATH)
    content = _replace_setting(content, "store_dir", peer.kura_store_dir, "/storage/kura")
    content = _replace_setting(
        content,
        "cold_store_root",
        peer.cold_store_root,
        "/storage/tiered_state",
    )
    content = _replace_setting(
        content,
        "da_store_root",
        peer.da_store_root,
        "/storage/da_wsv_snapshots",
    )
    content = _replace_single_line(content, "peer_telemetry_urls", telemetry_urls)
    return content


def _write_env_file(
    path: Path,
    *,
    container_name: str,
    image: str,
    config_path: Path,
    storage_path: Path,
    bundle_dir: Path,
    network: str,
    p2p_port: int,
    torii_port: int,
    rust_log: str,
) -> None:
    path.write_text(
        "\n".join(
            [
                f"TAIRA_CONTAINER_NAME={container_name}",
                f"TAIRA_IMAGE={image}",
                f"TAIRA_CONFIG_PATH={config_path.resolve()}",
                f"TAIRA_STORAGE_PATH={storage_path.resolve()}",
                f"TAIRA_P2P_PORT={p2p_port}",
                f"TAIRA_TORII_PORT={torii_port}",
                f"TAIRA_RUST_LOG={rust_log}",
                f"TAIRA_DOCKER_NETWORK={network}",
                f"TAIRA_GENESIS_PATH={(bundle_dir / 'genesis.json').resolve()}",
                f"TAIRA_SIGNED_GENESIS_PATH={(bundle_dir / 'genesis.signed.nrt').resolve()}",
                "",
            ]
        ),
        encoding="utf-8",
    )


def render_bundle(args: argparse.Namespace) -> list[Path]:
    """Render container-ready configs plus wrapper env files."""

    bundle_dir = args.bundle_dir.resolve()
    if not bundle_dir.is_dir():
        raise ValueError(f"bundle directory does not exist: {bundle_dir}")
    genesis_json = bundle_dir / "genesis.json"
    genesis_signed = bundle_dir / "genesis.signed.nrt"
    if not genesis_json.is_file():
        raise ValueError(f"missing genesis manifest JSON at {genesis_json}")
    if not genesis_signed.is_file():
        raise ValueError(f"missing signed genesis at {genesis_signed}")

    peers = _discover_peers(bundle_dir)

    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    hostnames = [f"{args.container_prefix}{peer.index}" for peer in peers]
    written_env_files: list[Path] = []

    for peer in peers:
        hostname = hostnames[peer.index]
        config_path = output_dir / f"{peer.stem}.toml"
        storage_path = output_dir / f"{peer.stem}-storage"
        env_path = output_dir / f"{peer.stem}.env"
        storage_path.mkdir(parents=True, exist_ok=True)

        content = peer.config_path.read_text(encoding="utf-8")
        config_path.write_text(_rewrite_config(content, peer, peers, hostnames), encoding="utf-8")
        _write_env_file(
            env_path,
            container_name=hostname,
            image=args.image,
            config_path=config_path,
            storage_path=storage_path,
            bundle_dir=bundle_dir,
            network=args.network,
            p2p_port=args.base_p2p_port + peer.index,
            torii_port=args.base_torii_port + peer.index,
            rust_log=args.rust_log,
        )
        written_env_files.append(env_path)

    return written_env_files


def parse_args() -> argparse.Namespace:
    """Parse the CLI for localnet container bundle rendering."""

    parser = argparse.ArgumentParser(
        description="Render a kagami localnet bundle into per-peer configs/env files for Docker validation."
    )
    parser.add_argument(
        "--bundle-dir",
        type=Path,
        required=True,
        help="Directory containing kagami localnet outputs such as peer0.toml and genesis.signed.nrt.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        required=True,
        help="Directory where rewritten peer configs, storage roots, and env files should be written.",
    )
    parser.add_argument(
        "--image",
        default=DEFAULT_IMAGE,
        help=f"Container image tag to embed in the generated env files (default: {DEFAULT_IMAGE}).",
    )
    parser.add_argument(
        "--network",
        default=DEFAULT_NETWORK,
        help=f"Docker network name shared by the rendered validators (default: {DEFAULT_NETWORK}).",
    )
    parser.add_argument(
        "--container-prefix",
        default=DEFAULT_CONTAINER_PREFIX,
        help=(
            "Prefix used for validator container names and internal DNS addresses "
            f"(default: {DEFAULT_CONTAINER_PREFIX})."
        ),
    )
    parser.add_argument(
        "--base-p2p-port",
        type=int,
        default=DEFAULT_BASE_P2P_PORT,
        help=f"First host P2P port assigned to peer0 (default: {DEFAULT_BASE_P2P_PORT}).",
    )
    parser.add_argument(
        "--base-torii-port",
        type=int,
        default=DEFAULT_BASE_TORII_PORT,
        help=f"First host Torii port assigned to peer0 (default: {DEFAULT_BASE_TORII_PORT}).",
    )
    parser.add_argument(
        "--rust-log",
        default=DEFAULT_RUST_LOG,
        help=f"RUST_LOG level written into the env files (default: {DEFAULT_RUST_LOG}).",
    )
    return parser.parse_args()


def main() -> int:
    """Render the localnet container bundle and print the generated env files."""

    args = parse_args()
    env_files = render_bundle(args)
    for path in env_files:
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
