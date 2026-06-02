#!/usr/bin/env python3
"""Render the shared-edge Taira nginx config from a validator roster."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.parse import urlparse


DEFAULT_PUBLIC_HOST = "taira.sora.org"
DEFAULT_EXPLORER_HOST = "taira-explorer.sora.org"
DEFAULT_TLS_LINEAGE = "taira.sora.org"
DEFAULT_CERTBOT_ROOT = "/var/www/certbot"
DEFAULT_EXPLORER_ROOT = "/var/www/iroha2-block-explorer-web/dist"
DEFAULT_CID_HOST_SUFFIX = "sorafs.taira.sora.org"
DEFAULT_MON_HOST_SUFFIX = "mon.taira.sora.net"
DEFAULT_CLIENT_MAX_BODY_SIZE = "1g"
DEFAULT_UPSTREAM_KEEPALIVE = 64
DEFAULT_UPSTREAM_FAIL_TIMEOUT = "5s"
MIN_VALIDATORS = 4
PUBLIC_TORII_CORS_ORIGINS = [
    "https://test.soraswap.org",
    "http://127.0.0.1:3000",
    "http://localhost:3000",
]
PUBLIC_TORII_CORS_METHODS = "GET, POST, DELETE, OPTIONS"
PUBLIC_TORII_CORS_HEADERS = "accept, authorization, content-type"
PUBLIC_TORII_CORS_EXPOSED_HEADERS = (
    "x-iroha-api-version, x-iroha-api-supported, x-iroha-api-min-proof-version"
)
PUBLIC_TORII_CORS_MAX_AGE = "3600"


@dataclass(frozen=True)
class EdgeValidator:
    slug: str
    upstream_name: str
    validator_host: str
    upstream_address: str


def _load_toml(path: Path) -> dict[str, Any]:
    try:
        import tomllib
    except ModuleNotFoundError:
        try:
            import tomli as tomllib
        except ModuleNotFoundError as error:  # pragma: no cover
            raise SystemExit(
                "python3 must provide tomllib (Python 3.11+) or tomli to load roster TOML"
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


def _split_host_port(value: str, context: str) -> tuple[str, str]:
    text = value.strip()
    if not text:
        raise ValueError(f"{context} must be a non-empty host:port string")
    if text.startswith("["):
        end = text.find("]")
        if end == -1 or end + 2 >= len(text) or text[end + 1] != ":":
            raise ValueError(f"{context} must be a valid [ipv6]:port or host:port value")
        host = text[1:end]
        port = text[end + 2 :]
    else:
        host, sep, port = text.rpartition(":")
        if sep == "" or not host or not port:
            raise ValueError(f"{context} must be a valid host:port value")
    if not port.isdigit():
        raise ValueError(f"{context} port `{port}` must be numeric")
    return host, port


def _normalize_upstream_address(value: str, context: str) -> str:
    host, port = _split_host_port(value, context)
    normalized_host = host
    if host in {"0.0.0.0", "::", "[::]", "localhost"}:
        normalized_host = "127.0.0.1"
    return f"{normalized_host}:{port}"


def _validator_host_from_public_url(value: str, context: str) -> str:
    parsed = urlparse(value)
    if parsed.scheme not in {"http", "https"}:
        raise ValueError(f"{context} must be an http:// or https:// URL")
    if not parsed.hostname:
        raise ValueError(f"{context} must include a hostname")
    return parsed.hostname


def _upstream_name_from_slug(slug: str) -> str:
    return "".join(ch if ch.isalnum() else "_" for ch in slug)


def load_edge_validators(roster_path: Path) -> list[EdgeValidator]:
    payload = _load_toml(roster_path)
    validators_raw = payload.get("validators")
    if not isinstance(validators_raw, list):
        raise ValueError("roster must define a `validators` array of tables")
    if len(validators_raw) < MIN_VALIDATORS:
        raise ValueError(
            f"roster must define at least {MIN_VALIDATORS} validators for Taira"
        )

    default_torii_address = payload.get("torii_address", "0.0.0.0:18080")
    if not isinstance(default_torii_address, str) or not default_torii_address.strip():
        raise ValueError("roster default `torii_address` must be a non-empty string")
    default_torii_address = default_torii_address.strip()

    validators: list[EdgeValidator] = []
    seen_hosts: set[str] = set()
    seen_upstreams: set[str] = set()
    for index, raw in enumerate(validators_raw, start=1):
        if not isinstance(raw, dict):
            raise ValueError(f"validator entry #{index} must be a TOML table")
        slug = _require_string(raw, "slug", f"validator `{index}`")
        torii_public_address = _require_string(
            raw, "torii_public_address", f"validator `{slug}`"
        )
        validator_host = _validator_host_from_public_url(
            torii_public_address, f"validator `{slug}` torii_public_address"
        )
        upstream_source = raw.get("edge_torii_upstream", raw.get("torii_address", default_torii_address))
        if not isinstance(upstream_source, str) or not upstream_source.strip():
            raise ValueError(
                f"validator `{slug}` must set `edge_torii_upstream` or `torii_address` to a non-empty host:port"
            )
        upstream_address = _normalize_upstream_address(
            upstream_source.strip(),
            f"validator `{slug}` edge_torii_upstream",
        )
        if validator_host in seen_hosts:
            raise ValueError(f"validator host `{validator_host}` is duplicated in the roster")
        if upstream_address in seen_upstreams:
            raise ValueError(
                f"edge upstream `{upstream_address}` is duplicated in the roster; "
                "shared-edge nginx expects each validator to expose a distinct upstream target"
            )
        seen_hosts.add(validator_host)
        seen_upstreams.add(upstream_address)
        validators.append(
            EdgeValidator(
                slug=slug,
                upstream_name=_upstream_name_from_slug(slug),
                validator_host=validator_host,
                upstream_address=upstream_address,
            )
        )
    return validators


def _render_proxy_headers(host_expr: str, *, forwarded_host_expr: str | None = None) -> list[str]:
    lines = [
        f"    proxy_set_header Host {host_expr};",
        "    proxy_set_header X-Real-IP $remote_addr;",
        "    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;",
        "    proxy_set_header X-Forwarded-Proto $scheme;",
    ]
    if forwarded_host_expr is not None:
        lines.insert(3, f"    proxy_set_header X-Forwarded-Host {forwarded_host_expr};")
    return lines


def _render_upstream(name: str, server_lines: list[str], keepalive: int) -> list[str]:
    lines = [f"upstream {name} {{"]
    lines.extend(server_lines)
    lines.append(f"  keepalive {keepalive};")
    lines.append("}")
    return lines


def _render_public_torii_cors_server_lines() -> list[str]:
    return [
        "  proxy_hide_header Access-Control-Allow-Origin;",
        "  proxy_hide_header Access-Control-Allow-Methods;",
        "  proxy_hide_header Access-Control-Allow-Headers;",
        "  proxy_hide_header Access-Control-Expose-Headers;",
        "  proxy_hide_header Access-Control-Max-Age;",
        "  add_header Access-Control-Allow-Origin $taira_public_torii_cors_origin always;",
        f'  add_header Access-Control-Allow-Methods "{PUBLIC_TORII_CORS_METHODS}" always;',
        f'  add_header Access-Control-Allow-Headers "{PUBLIC_TORII_CORS_HEADERS}" always;',
        f'  add_header Access-Control-Expose-Headers "{PUBLIC_TORII_CORS_EXPOSED_HEADERS}" always;',
        f'  add_header Access-Control-Max-Age "{PUBLIC_TORII_CORS_MAX_AGE}" always;',
        '  add_header Vary "Origin" always;',
        "",
        "  if ($request_method = OPTIONS) {",
        "    return 204;",
        "  }",
        "",
    ]


def _render_exact_proxy_location(
    path: str,
    upstream: str,
    *,
    host_expr: str,
    websocket: bool = False,
    retry_non_idempotent: bool = False,
    forwarded_host_expr: str | None = None,
) -> list[str]:
    lines = [f"  location = {path} {{", f"    proxy_pass http://{upstream};", "    proxy_http_version 1.1;"]
    if websocket:
        lines.extend(
            [
                "    proxy_set_header Upgrade $http_upgrade;",
                '    proxy_set_header Connection "upgrade";',
            ]
        )
    lines.extend(_render_proxy_headers(host_expr, forwarded_host_expr=forwarded_host_expr))
    if retry_non_idempotent:
        lines.extend(
            [
                "    proxy_next_upstream error timeout http_502 http_503 http_504 invalid_header non_idempotent;",
                "    proxy_next_upstream_tries 4;",
            ]
        )
    lines.extend(
        [
            "    proxy_read_timeout 3600;",
            "    proxy_send_timeout 3600;",
            "    proxy_buffering off;",
            "  }",
        ]
    )
    return lines


def _render_prefix_proxy_location(
    path: str,
    upstream: str,
    *,
    host_expr: str,
    retry_non_idempotent: bool = False,
    forwarded_host_expr: str | None = None,
) -> list[str]:
    lines = [f"  location ^~ {path} {{", f"    proxy_pass http://{upstream};", "    proxy_http_version 1.1;"]
    lines.extend(_render_proxy_headers(host_expr, forwarded_host_expr=forwarded_host_expr))
    if retry_non_idempotent:
        lines.extend(
            [
                "    proxy_next_upstream error timeout http_502 http_503 http_504 invalid_header non_idempotent;",
                "    proxy_next_upstream_tries 4;",
            ]
        )
    lines.extend(
        [
            "    proxy_read_timeout 3600;",
            "    proxy_send_timeout 3600;",
            "    proxy_buffering off;",
            "  }",
        ]
    )
    return lines


def _render_soradns_proxy_location(upstream: str) -> list[str]:
    return [
        "  location ~ ^/soradns/(?<soradns_alias>[^/]+)(?<soradns_rest>/.*)?$ {",
        "    set $soradns_target_path $soradns_rest;",
        "    if ($soradns_target_path = \"\") {",
        "      set $soradns_target_path /;",
        "    }",
        "",
        f"    proxy_pass http://{upstream}$soradns_target_path$is_args$args;",
        "    proxy_http_version 1.1;",
        "    proxy_set_header Host $soradns_alias;",
        "    proxy_set_header X-Real-IP $remote_addr;",
        "    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;",
        "    proxy_set_header X-Forwarded-Host $host;",
        "    proxy_set_header X-Forwarded-Proto $scheme;",
        "    proxy_next_upstream error timeout http_502 http_503 http_504 invalid_header non_idempotent;",
        "    proxy_next_upstream_tries 4;",
        "    proxy_read_timeout 3600;",
        "    proxy_send_timeout 3600;",
        "    proxy_buffering off;",
        "  }",
    ]


def _render_connect_stateful_locations(
    connect_upstream: str,
    *,
    host_expr: str,
    forwarded_host_expr: str,
) -> list[str]:
    lines = [
        "  # IrohaConnect session tokens and MCP Connect tools are node-local; keep",
        "  # session creation, management/status, and websocket authorization on one Torii process.",
    ]
    for location in (
        _render_exact_proxy_location(
            "/v1/connect/session",
            connect_upstream,
            host_expr=host_expr,
            forwarded_host_expr=forwarded_host_expr,
        ),
        _render_prefix_proxy_location(
            "/v1/connect/session/",
            connect_upstream,
            host_expr=host_expr,
            forwarded_host_expr=forwarded_host_expr,
        ),
        _render_exact_proxy_location(
            "/v1/connect/status",
            connect_upstream,
            host_expr=host_expr,
            forwarded_host_expr=forwarded_host_expr,
        ),
        _render_exact_proxy_location(
            "/v1/connect/ws",
            connect_upstream,
            host_expr=host_expr,
            websocket=True,
            forwarded_host_expr=forwarded_host_expr,
        ),
        _render_exact_proxy_location(
            "/v1/mcp",
            connect_upstream,
            host_expr=host_expr,
            forwarded_host_expr=forwarded_host_expr,
        ),
    ):
        lines.extend(location)
        lines.append("")
    return lines


def render_edge_nginx_conf(
    validators: list[EdgeValidator],
    *,
    public_host: str = DEFAULT_PUBLIC_HOST,
    public_upstream_host: str | None = None,
    explorer_host: str = DEFAULT_EXPLORER_HOST,
    tls_lineage: str = DEFAULT_TLS_LINEAGE,
    certbot_root: str = DEFAULT_CERTBOT_ROOT,
    explorer_root: str = DEFAULT_EXPLORER_ROOT,
    cid_host_suffix: str = DEFAULT_CID_HOST_SUFFIX,
    mon_host_suffix: str = DEFAULT_MON_HOST_SUFFIX,
    client_max_body_size: str = DEFAULT_CLIENT_MAX_BODY_SIZE,
    upstream_keepalive: int = DEFAULT_UPSTREAM_KEEPALIVE,
    upstream_fail_timeout: str = DEFAULT_UPSTREAM_FAIL_TIMEOUT,
) -> str:
    if public_upstream_host is None:
        public_upstream_host = (
            validators[1].validator_host if len(validators) > 1 else validators[0].validator_host
        )
    connect_validator = next(
        (validator for validator in validators if validator.validator_host == public_upstream_host),
        validators[1] if len(validators) > 1 else validators[0],
    )
    connect_upstream = f"{connect_validator.upstream_name}_upstream"

    escaped_mon_host_suffix = mon_host_suffix.replace(".", r"\.")
    mon_host_pattern = f"~^.+\\.{escaped_mon_host_suffix}$"
    mon_alias_host_var = "$taira_mon_alias_host"
    server_names = [
        public_host,
        explorer_host,
        mon_host_suffix,
        *[v.validator_host for v in validators],
        f"*.{cid_host_suffix}",
        mon_host_pattern,
    ]
    lines: list[str] = [
        "# Generated by scripts/render_taira_edge_nginx_conf.py from the Taira validator roster.",
        "# Shared-edge stress runs require the main nginx.conf to set:",
        "#   worker_rlimit_nofile 65536;",
        "#   events { worker_connections 16384; }",
        "# Deploy on the shared edge host serving:",
        f"# - {public_host} (Torii endpoint / convenience host)",
        f"# - {explorer_host} (Explorer web UI)",
        "# - validator-specific public Torii hostnames from the roster",
        f"# - *.{cid_host_suffix} (origin-isolated SoraFS site hosts)",
        f"# - <alias>.{mon_host_suffix} (public Soracloud browser gateway hosts)",
        "",
        "map $host $taira_mon_alias_host {",
        '  default "";',
        f"  ~^(?<taira_mon_alias_host_capture>.+)\\.{escaped_mon_host_suffix}$ $taira_mon_alias_host_capture;",
        "}",
        "",
        "map $http_origin $taira_public_torii_cors_origin {",
        '  default "";',
        *[f'  "{origin}" $http_origin;' for origin in PUBLIC_TORII_CORS_ORIGINS],
        "}",
        "",
    ]

    for validator in validators:
        lines.extend(
            _render_upstream(
                f"{validator.upstream_name}_upstream",
                [f"  server {validator.upstream_address};"],
                upstream_keepalive,
            )
        )
        lines.append("")

    shared_upstream_servers = [
        f"  server {validator.upstream_address} max_fails=1 fail_timeout={upstream_fail_timeout};"
        for validator in validators
    ]
    lines.extend(
        _render_upstream("taira_public_edge_upstream", shared_upstream_servers, upstream_keepalive)
    )
    lines.extend(
        [
            "",
            "server {",
            "  listen 80;",
            "  listen [::]:80;",
            f"  server_name {' '.join(server_names)};",
            "",
            "  location ^~ /.well-known/acme-challenge/ {",
            f"    root {certbot_root};",
            '    default_type "text/plain";',
            "  }",
            "",
            "  location / {",
            "    return 301 https://$host$request_uri;",
            "  }",
            "}",
            "",
            "server {",
            "  listen 443 ssl;",
            "  listen [::]:443 ssl;",
            "  http2 on;",
            f"  server_name {public_host};",
            f"  client_max_body_size {client_max_body_size};",
            "",
            f"  ssl_certificate /etc/letsencrypt/live/{tls_lineage}/fullchain.pem;",
            f"  ssl_certificate_key /etc/letsencrypt/live/{tls_lineage}/privkey.pem;",
            "  include /etc/letsencrypt/options-ssl-nginx.conf;",
            "  ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;",
            "",
        ]
    )
    lines.extend(_render_public_torii_cors_server_lines())
    lines.extend(
        _render_connect_stateful_locations(
            connect_upstream,
            host_expr=public_upstream_host,
            forwarded_host_expr="$host",
        )
    )
    for path in ("/v1/app-api/", "/v1/sorafs/storage/", "/v1/sorafs/pin/", "/v1/sorafs/cid/", "/sorafs/cid/"):
        lines.extend(
            _render_prefix_proxy_location(
                path,
                "taira_public_edge_upstream",
                host_expr=public_upstream_host,
                forwarded_host_expr="$host",
            )
        )
        lines.append("")
    lines.extend(
        _render_soradns_proxy_location("taira_public_edge_upstream")
    )
    lines.append("")
    lines.extend(
        _render_prefix_proxy_location(
            "/",
            "taira_public_edge_upstream",
            host_expr=public_upstream_host,
            forwarded_host_expr="$host",
            retry_non_idempotent=True,
        )
    )
    lines.extend(
        [
            "}",
            "",
            "server {",
            "  listen 443 ssl;",
            "  listen [::]:443 ssl;",
            "  http2 on;",
            f"  server_name {mon_host_suffix};",
            f"  client_max_body_size {client_max_body_size};",
            "",
            f"  ssl_certificate /etc/letsencrypt/live/{mon_host_suffix}/fullchain.pem;",
            f"  ssl_certificate_key /etc/letsencrypt/live/{mon_host_suffix}/privkey.pem;",
            "  include /etc/letsencrypt/options-ssl-nginx.conf;",
            "  ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;",
            "",
            "  location = /health {",
            '    default_type "text/plain";',
            "    return 200 \"Healthy\\n\";",
            "  }",
            "",
            "  location = / {",
            '    default_type "text/plain";',
            "    return 200 \"Taira Soracloud Mon gateway\\n\\nUse https://<alias>."
            f"{mon_host_suffix}/<path> for browser clients.\\nExample: "
            f"https://solswap-indexer.sora.{mon_host_suffix}/api/indexer/v1/health\\nDebug "
            f"fallback: https://{mon_host_suffix}/soradns/<alias>/<path>\\n\";",
            "  }",
            "",
        ]
    )
    lines.extend(_render_soradns_proxy_location("taira_public_edge_upstream"))
    lines.extend(
        [
            "}",
            "",
            "server {",
            "  listen 443 ssl;",
            "  listen [::]:443 ssl;",
            "  http2 on;",
            f"  server_name *.{mon_host_suffix} {mon_host_pattern};",
            f"  client_max_body_size {client_max_body_size};",
            "",
            "  # Each pretty host receives an exact certificate at Soracloud alias bind time.",
            f"  # Do not use a wildcard certificate here: `*.{mon_host_suffix}` does not",
            f"  # cover multi-label aliases such as solswap-indexer.sora.{mon_host_suffix}.",
            "  ssl_certificate /etc/letsencrypt/live/$ssl_server_name/fullchain.pem;",
            "  ssl_certificate_key /etc/letsencrypt/live/$ssl_server_name/privkey.pem;",
            "  include /etc/letsencrypt/options-ssl-nginx.conf;",
            "  ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;",
            "",
        ]
    )
    lines.extend(
        _render_prefix_proxy_location(
            "/",
            "taira_public_edge_upstream",
            host_expr=mon_alias_host_var,
            forwarded_host_expr="$host",
            retry_non_idempotent=True,
        )
    )
    lines.extend(
        [
            "}",
            "",
            "server {",
            "  listen 443 ssl;",
            "  listen [::]:443 ssl;",
            "  http2 on;",
            f"  server_name *.{cid_host_suffix};",
            f"  client_max_body_size {client_max_body_size};",
            "",
            f"  ssl_certificate /etc/letsencrypt/live/{tls_lineage}/fullchain.pem;",
            f"  ssl_certificate_key /etc/letsencrypt/live/{tls_lineage}/privkey.pem;",
            "  include /etc/letsencrypt/options-ssl-nginx.conf;",
            "  ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;",
            "",
        ]
    )
    lines.extend(
        _render_prefix_proxy_location(
            "/",
            "taira_public_edge_upstream",
            host_expr="$host",
        )
    )
    lines.extend(["}", ""])

    for validator in validators:
        lines.extend(
            [
                "server {",
                "  listen 443 ssl;",
                "  listen [::]:443 ssl;",
                "  http2 on;",
                f"  server_name {validator.validator_host};",
                f"  client_max_body_size {client_max_body_size};",
                "",
                f"  ssl_certificate /etc/letsencrypt/live/{tls_lineage}/fullchain.pem;",
                f"  ssl_certificate_key /etc/letsencrypt/live/{tls_lineage}/privkey.pem;",
                "  include /etc/letsencrypt/options-ssl-nginx.conf;",
                "  ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;",
                "",
            ]
        )
        lines.extend(
            _render_exact_proxy_location(
                "/v1/connect/ws",
                f"{validator.upstream_name}_upstream",
                host_expr="$host",
                websocket=True,
            )
        )
        lines.append("")
        lines.extend(
            _render_exact_proxy_location(
                "/v1/mcp",
                f"{validator.upstream_name}_upstream",
                host_expr="$host",
            )
        )
        lines.append("")
        lines.extend(
            _render_prefix_proxy_location(
                "/",
                f"{validator.upstream_name}_upstream",
                host_expr="$host",
            )
        )
        lines.extend(["}", ""])

    lines.extend(
        [
            "server {",
            "  listen 443 ssl;",
            "  listen [::]:443 ssl;",
            "  http2 on;",
            f"  server_name {explorer_host};",
            f"  client_max_body_size {client_max_body_size};",
            "",
            f"  ssl_certificate /etc/letsencrypt/live/{tls_lineage}/fullchain.pem;",
            f"  ssl_certificate_key /etc/letsencrypt/live/{tls_lineage}/privkey.pem;",
            "  include /etc/letsencrypt/options-ssl-nginx.conf;",
            "  ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;",
            "",
            f"  root {explorer_root};",
            "  index index.html;",
            "",
            "  location / {",
            "    try_files $uri $uri/ /index.html;",
            "  }",
            "",
            "  location = /status {",
            "    proxy_pass http://taira_public_edge_upstream/status;",
            "    proxy_http_version 1.1;",
            f"    proxy_set_header Host {public_upstream_host};",
            "    proxy_set_header X-Real-IP $remote_addr;",
            "    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;",
            f"    proxy_set_header X-Forwarded-Host {public_host};",
            "    proxy_set_header X-Forwarded-Proto $scheme;",
            "    proxy_next_upstream error timeout http_502 http_503 http_504 invalid_header non_idempotent;",
            "    proxy_next_upstream_tries 4;",
            "    proxy_read_timeout 3600;",
            "    proxy_send_timeout 3600;",
            "    proxy_buffering off;",
            "  }",
            "",
        ]
    )
    lines.extend(
        _render_connect_stateful_locations(
            connect_upstream,
            host_expr=public_upstream_host,
            forwarded_host_expr=public_host,
        )
    )
    lines.extend(
        _render_prefix_proxy_location(
            "/v1/",
            "taira_public_edge_upstream",
            host_expr=public_upstream_host,
            retry_non_idempotent=True,
            forwarded_host_expr=public_host,
        )
    )
    lines.extend(["}", ""])

    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Render the shared-edge Taira nginx config from a validator roster."
    )
    parser.add_argument("--roster", required=True, help="validator roster TOML")
    parser.add_argument("--output", required=True, help="nginx config output path")
    parser.add_argument("--public-host", default=DEFAULT_PUBLIC_HOST)
    parser.add_argument(
        "--public-upstream-host",
        default=None,
        help=(
            "Host header used when proxying the public convenience host to Torii "
            "(defaults to the second validator hostname when present)"
        ),
    )
    parser.add_argument("--explorer-host", default=DEFAULT_EXPLORER_HOST)
    parser.add_argument("--tls-lineage", default=DEFAULT_TLS_LINEAGE)
    parser.add_argument("--certbot-root", default=DEFAULT_CERTBOT_ROOT)
    parser.add_argument("--explorer-root", default=DEFAULT_EXPLORER_ROOT)
    parser.add_argument("--cid-host-suffix", default=DEFAULT_CID_HOST_SUFFIX)
    parser.add_argument("--mon-host-suffix", default=DEFAULT_MON_HOST_SUFFIX)
    parser.add_argument("--client-max-body-size", default=DEFAULT_CLIENT_MAX_BODY_SIZE)
    args = parser.parse_args(argv)

    validators = load_edge_validators(Path(args.roster))
    rendered = render_edge_nginx_conf(
        validators,
        public_host=args.public_host,
        public_upstream_host=args.public_upstream_host,
        explorer_host=args.explorer_host,
        tls_lineage=args.tls_lineage,
        certbot_root=args.certbot_root,
        explorer_root=args.explorer_root,
        cid_host_suffix=args.cid_host_suffix,
        mon_host_suffix=args.mon_host_suffix,
        client_max_body_size=args.client_max_body_size,
    )
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(rendered if rendered.endswith("\n") else f"{rendered}\n", encoding="utf-8")
    print(output_path)
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
