"""Tests for scripts/render_taira_edge_nginx_conf.py."""

from __future__ import annotations

import importlib.util
import re

import pytest
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "render_taira_edge_nginx_conf.py"
SPEC = importlib.util.spec_from_file_location("render_taira_edge_nginx_conf", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)
REPO_ROOT = MODULE_PATH.parents[1]
EXAMPLE_ROSTER_PATH = REPO_ROOT / "configs/soranexus/taira/validator_roster.example.toml"
CHECKED_IN_EXAMPLE_PATH = REPO_ROOT / "configs/soranexus/taira/taira-explorer.nginx.conf"
TAIRA_CONFIG_PATH = REPO_ROOT / "configs/soranexus/taira/config.toml"


def _location_block(server: str, marker: str) -> str:
    start = server.index(marker)
    rest = server[start:]
    next_location = rest.find("\n  location ", len(marker))
    if next_location == -1:
        return rest
    return rest[:next_location]



def _bls_key(index: int) -> str:
    return "ea0130" + f"{0xA0 + index:02X}" * 48


def _write_roster(
    path: Path,
    *,
    torii_address: str = "0.0.0.0:18080",
    include_edge_upstreams: bool = True,
    include_soracloud_alias_route: bool = False,
    validator_count: int = 4,
) -> None:
    parts = [f'torii_address = "{torii_address}"', ""]
    if include_soracloud_alias_route:
        parts.extend(
            [
                "[[soracloud_alias_routes]]",
                'alias = "solswap-indexer.sora"',
                'edge_upstream = "127.0.0.1:8788"',
                "",
            ]
        )
    for index in range(1, validator_count + 1):
        parts.extend(
            [
                "[[validators]]",
                f'slug = "taira-validator-{index}"',
                f'public_key = "{_bls_key(index)}"',
                f'pop_hex = "peer-{index}-pop"',
                f'public_address = "taira-validator-{index}.sora.org:1337"',
                f'torii_public_address = "https://taira-validator-{index}.sora.org"',
            ]
        )
        if include_edge_upstreams:
            parts.append(f'edge_torii_upstream = "127.0.0.1:{18079 + index}"')
        parts.append("")
    path.write_text("\n".join(parts), encoding="utf-8")


def test_load_edge_validators_uses_explicit_edge_upstreams(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)

    validators = MODULE.load_edge_validators(roster_path)

    assert [validator.upstream_address for validator in validators] == [
        "127.0.0.1:18080",
        "127.0.0.1:18081",
        "127.0.0.1:18082",
        "127.0.0.1:18083",
    ]
    assert validators[0].validator_host == "taira-validator-1.sora.org"


def test_load_edge_validators_rejects_missing_or_legacy_upstream_fields(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path, include_edge_upstreams=False)

    try:
        MODULE.load_edge_validators(roster_path)
    except ValueError as error:
        assert "missing canonical field" in str(error)
    else:  # pragma: no cover
        raise AssertionError("load_edge_validators accepted missing canonical upstreams")

    legacy = roster_path.read_text(encoding="utf-8").replace(
        'torii_public_address = "https://taira-validator-1.sora.org"',
        'torii_public_address = "https://taira-validator-1.sora.org"\n'
        'torii_address = "127.0.0.1:29080"',
        1,
    )
    roster_path.write_text(legacy, encoding="utf-8")
    try:
        MODULE.load_edge_validators(roster_path)
    except ValueError as error:
        assert "unknown first-release field" in str(error)
        assert "`torii_address`" in str(error)
    else:  # pragma: no cover
        raise AssertionError("load_edge_validators accepted legacy validator alias")


def test_roster_requires_exactly_four_validators_and_rejects_unknowns(
    tmp_path: Path,
) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    for count in (3, 5):
        _write_roster(roster_path, validator_count=count)
        try:
            MODULE.load_edge_validators(roster_path)
        except ValueError as error:
            assert "exactly 4 validators" in str(error)
        else:  # pragma: no cover
            raise AssertionError(f"accepted a {count}-validator Taira edge roster")

    _write_roster(roster_path)
    roster_path.write_text(
        'legacy_edge_mode = true\n' + roster_path.read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    try:
        MODULE.load_edge_validators(roster_path)
    except ValueError as error:
        assert "unknown first-release field" in str(error)
        assert "`legacy_edge_mode`" in str(error)
    else:  # pragma: no cover
        raise AssertionError("accepted an unknown top-level roster field")


def test_validator_values_require_exact_canonical_spelling(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    mutations = (
        (
            'slug = "taira-validator-1"',
            'slug = "Taira-Validator-1"',
            "lowercase kebab-case",
        ),
        (
            'slug = "taira-validator-1"',
            'slug = "taira_validator_1"',
            "lowercase kebab-case",
        ),
        (
            'slug = "taira-validator-1"',
            'slug = " taira-validator-1"',
            "surrounding whitespace",
        ),
        (
            'torii_public_address = "https://taira-validator-1.sora.org"',
            'torii_public_address = "HTTPS://taira-validator-1.sora.org"',
            "exact canonical spelling",
        ),
        (
            'torii_public_address = "https://taira-validator-1.sora.org"',
            'torii_public_address = "https://Taira-Validator-1.sora.org"',
            "exact canonical spelling",
        ),
        (
            'torii_public_address = "https://taira-validator-1.sora.org"',
            'torii_public_address = "https://taira-validator-1.sora.org."',
            "trailing dot",
        ),
        (
            'torii_public_address = "https://taira-validator-1.sora.org"',
            'torii_public_address = "https://taira-validator-1.sora.org/"',
            "must not contain credentials",
        ),
        (
            'torii_public_address = "https://taira-validator-1.sora.org"',
            'torii_public_address = "https://taira-validator-1.sora.org:443"',
            "exact canonical spelling",
        ),
        (
            'torii_public_address = "https://taira-validator-1.sora.org"',
            'torii_public_address = "http://taira-validator-1.sora.org"',
            "exact https:// DNS origin",
        ),
        (
            'edge_torii_upstream = "127.0.0.1:18080"',
            'edge_torii_upstream = "0.0.0.0:18080"',
            "wildcard address",
        ),
        (
            'edge_torii_upstream = "127.0.0.1:18080"',
            'edge_torii_upstream = "localhost:18080"',
            "localhost alias",
        ),
        (
            'edge_torii_upstream = "127.0.0.1:18080"',
            'edge_torii_upstream = "127.000.0.1:18080"',
            "IPv4 host must use exact canonical spelling",
        ),
        (
            'edge_torii_upstream = "127.0.0.1:18080"',
            'edge_torii_upstream = "127.0.0.1:018080"',
            "canonical decimal spelling",
        ),
        (
            'edge_torii_upstream = "127.0.0.1:18080"',
            'edge_torii_upstream = "127.0.0.1:65536"',
            "between 1 and 65535",
        ),
        (
            'edge_torii_upstream = "127.0.0.1:18080"',
            'edge_torii_upstream = "127.0.0.1:18080 "',
            "surrounding whitespace",
        ),
    )

    for old, new, expected in mutations:
        _write_roster(roster_path)
        roster_path.write_text(
            roster_path.read_text(encoding="utf-8").replace(old, new, 1),
            encoding="utf-8",
        )
        try:
            MODULE.load_edge_validators(roster_path)
        except ValueError as error:
            assert expected in str(error)
        else:  # pragma: no cover
            raise AssertionError(f"accepted non-canonical roster value {new!r}")


def test_render_edge_nginx_conf_includes_all_public_routes() -> None:
    validators = [
        MODULE.EdgeValidator(
            public_key=_bls_key(index),
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]

    rendered = MODULE.render_edge_nginx_conf(validators)

    assert "server_name taira.sora.org taira-explorer.sora.org" in rendered
    assert "server_name *.sorafs.taira.sora.org;" in rendered
    assert "map $host $taira_mon_alias_host" in rendered
    assert "server_name mon.taira.sora.net;" in rendered
    assert "Taira Soracloud Mon gateway" in rendered
    assert "/soradns/" not in rendered
    assert "$soradns_" not in rendered
    assert "server_name *.mon.taira.sora.net ~^.+\\.mon\\.taira\\.sora\\.net$;" in rendered
    assert "proxy_set_header Host $taira_mon_alias_host;" in rendered
    assert "proxy_set_header X-Forwarded-Host $host;" in rendered
    assert "proxy_set_header Host taira-validator-1.sora.org;" not in rendered
    public_upstream = rendered.split(
        "upstream taira_public_edge_upstream {", 1
    )[1].split("}", 1)[0]
    assert "server 127.0.0.1:18080 max_fails=1 fail_timeout=5s;" in public_upstream
    assert "127.0.0.1:18081" not in public_upstream
    assert "127.0.0.1:18082" not in public_upstream
    assert "127.0.0.1:18083" not in public_upstream
    assert "proxy_pass http://taira_public_edge_upstream;" in rendered
    assert "proxy_pass http://taira_validator_1_upstream;" in rendered
    assert "location = /v1/connect/session" in rendered
    assert "location ^~ /v1/connect/session/" in rendered
    public_server = rendered.split("server_name taira.sora.org;", 1)[1].split(
        "server_name mon.taira.sora.net;", 1
    )[0]
    explorer_server = rendered.split("server_name taira-explorer.sora.org;", 1)[1].split(
        "server_name taira-validator-1.sora.org;", 1
    )[0]
    for marker in (
        "location = /v1/connect/session",
        "location ^~ /v1/connect/session/",
        "location = /v1/connect/status",
        "location = /v1/connect/status/aggregate",
        "location = /v1/connect/ws",
        "location = /v1/mcp",
    ):
        block = _location_block(public_server, marker)
        assert "proxy_pass http://taira_validator_1_upstream;" in block
        assert "proxy_next_upstream" not in block
        assert marker not in explorer_server
    assert "root /Users/administrator/dev/iroha2-block-explorer-web/dist;" in explorer_server
    assert "location / {" in explorer_server
    assert "try_files $uri $uri/ /index.html;" in explorer_server
    assert "proxy_pass" not in explorer_server
    assert [
        line.strip()
        for line in explorer_server.splitlines()
        if line.lstrip().startswith("include ")
    ] == ["include /etc/letsencrypt/options-ssl-nginx.conf;"]
    assert "client_max_body_size" not in explorer_server
    assert "location = /v1/mcp" in rendered
    assert "location ^~ /v1/app-api/" in rendered
    assert "client_max_body_size 1g;" in rendered


def test_public_torii_cors_matches_runtime_policy_and_browser_sdk_headers() -> None:
    cors = MODULE._load_toml(TAIRA_CONFIG_PATH)["torii"]["cors"]

    assert MODULE.PUBLIC_TORII_CORS_ORIGINS == cors["allowed_origins"]
    assert MODULE.PUBLIC_TORII_CORS_METHODS == ", ".join(cors["allowed_methods"])
    assert MODULE.PUBLIC_TORII_CORS_HEADERS == ", ".join(cors["allowed_headers"])
    assert MODULE.PUBLIC_TORII_CORS_EXPOSED_HEADERS == ", ".join(
        cors["exposed_headers"]
    )
    assert set(cors["allowed_origins"]) == {
        "http://127.0.0.1:3000",
        "http://localhost:3000",
        "https://taira-explorer.sora.org",
        "https://test.soraswap.org",
        "https://dweb.link",
        "https://ipfs.io",
        "https://cloudflare-ipfs.com",
        "https://w3s.link",
        "https://nftstorage.link",
        "https://bokolo.soramitsu.io",
        "https://cbsi-banking.soramitsu.io",
        "https://cbsi-core.soramitsu.io",
        "https://bokolo-pob.soramitsu.io",
        "https://bokolo-bred.soramitsu.io",
        "https://bokolo-anz.soramitsu.io",
        "https://bokolo-bsp.soramitsu.io",
        "https://bokolo-m-selen.soramitsu.io",
        "https://bokolo-ezipei.soramitsu.io",
        "https://bpng.soramitsu.io",
        "https://mibank.soramitsu.io",
        "https://explorer-bpng.soramitsu.io",
        "https://bokolo-explorer.soramitsu.io",
    }
    assert len(cors["allowed_origins"]) == len(set(cors["allowed_origins"]))
    assert set(cors["allowed_methods"]) == {"GET", "POST", "DELETE", "OPTIONS"}
    assert not cors.get("allow_credentials", False)

    validators = [
        MODULE.EdgeValidator(
            public_key=_bls_key(index),
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]
    rendered = MODULE.render_edge_nginx_conf(validators)
    public_server = rendered.split("server_name taira.sora.org;", 1)[1].split(
        "server_name mon.taira.sora.net;", 1
    )[0]

    origin_map = rendered.split(
        "map $http_origin $taira_public_torii_cors_origin {\n", 1
    )[1].split("\n}", 1)[0]
    # Check the whole map: only exact allowlisted origins may echo. Every other
    # origin must use the empty default, never a wildcard, regex, or default echo.
    assert origin_map.splitlines() == [
        '  default "";',
        *[f'  "{origin}" $http_origin;' for origin in cors["allowed_origins"]],
    ]
    for rejected_origin in (
        "https://not-allowed.example",
        "https://mibank.soramitsu.io.attacker.example",
        "https://explorer-bpng.soramitsu.io.attacker.example",
        "http://mibank.soramitsu.io",
        "http://explorer-bpng.soramitsu.io",
        "null",
        "*",
    ):
        assert rejected_origin not in cors["allowed_origins"]
        assert f'  "{rejected_origin}" $http_origin;' not in origin_map
    assert (
        "add_header Access-Control-Allow-Origin $taira_public_torii_cors_origin always;"
        in public_server
    )
    assert "Access-Control-Allow-Credentials" not in public_server
    assert "if ($request_method = OPTIONS) {\n    return 204;\n  }" in public_server
    assert (
        f'add_header Access-Control-Allow-Headers "{MODULE.PUBLIC_TORII_CORS_HEADERS}" always;'
        in public_server
    )
    allowed_headers = {
        header.strip().lower() for header in MODULE.PUBLIC_TORII_CORS_HEADERS.split(",")
    }
    assert allowed_headers == {header.lower() for header in cors["allowed_headers"]}
    assert "*" not in allowed_headers
    assert (
        "idempotency-key" in allowed_headers
    ), "browser KAGEMUSHA V1 top-up and redemption require Idempotency-Key"
    for header in (
        "accept",
        "content-type",
        "x-client-app",
        "x-request-id",
        "x-account-id",
        "x-correlation-id",
        "x-api-token",
        "mcp-method",
        "mcp-name",
        "mcp-protocol-version",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-timestamp-ms",
        "x-iroha-nonce",
        "x-iroha-witness",
    ):
        assert header in allowed_headers
    assert (
        f'add_header Access-Control-Expose-Headers "{MODULE.PUBLIC_TORII_CORS_EXPOSED_HEADERS}" always;'
        in public_server
    )
    assert "location" in MODULE.PUBLIC_TORII_CORS_EXPOSED_HEADERS
    assert "retry-after" in MODULE.PUBLIC_TORII_CORS_EXPOSED_HEADERS


def test_public_edge_is_the_only_trusted_torii_forwarding_hop() -> None:
    torii = MODULE._load_toml(TAIRA_CONFIG_PATH)["torii"]

    assert torii["transport"]["trusted_proxy_cidrs"] == ["127.0.0.1/32"]
    assert "127.0.0.1/32" in torii["preauth_allow_cidrs"]

    validators = [
        MODULE.EdgeValidator(
            public_key=_bls_key(index),
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]
    rendered = MODULE.render_edge_nginx_conf(validators)
    assert "proxy_set_header X-Forwarded-For $remote_addr;" in rendered
    assert "$proxy_add_x_forwarded_for" not in rendered
    assert "proxy_set_header X-Real-IP $remote_addr;" in rendered


def test_render_edge_nginx_conf_uses_explicit_canonical_public_validator() -> None:
    validators = [
        MODULE.EdgeValidator(
            public_key=_bls_key(index),
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]

    rendered = MODULE.render_edge_nginx_conf(
        validators,
        public_upstream_validator="taira-validator-3",
    )
    public_upstream = rendered.split(
        "upstream taira_public_edge_upstream {", 1
    )[1].split("}", 1)[0]

    assert "server 127.0.0.1:18082 max_fails=1 fail_timeout=5s;" in public_upstream
    assert "127.0.0.1:18080" not in public_upstream
    public_server = rendered.split("server_name taira.sora.org;", 1)[1].split(
        "server_name mon.taira.sora.net;", 1
    )[0]
    assert "proxy_set_header Host taira-validator-3.sora.org;" not in public_server
    assert "proxy_set_header Host $host;" in public_server
    host_lines = [line.strip() for line in public_server.splitlines() if "proxy_set_header Host " in line]
    assert host_lines and all(line == "proxy_set_header Host $host;" for line in host_lines)
    assert "proxy_pass http://taira_validator_3_upstream;" in public_server


def test_render_edge_nginx_conf_rejects_unknown_public_validator() -> None:
    validators = [
        MODULE.EdgeValidator(
            public_key=_bls_key(index),
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]

    try:
        MODULE.render_edge_nginx_conf(
            validators,
            public_upstream_validator="not-a-validator",
        )
    except ValueError as error:
        assert "must match an exact validator slug" in str(error)
    else:  # pragma: no cover
        raise AssertionError("accepted an unknown canonical public validator")


def test_parse_soracloud_alias_routes_requires_canonical_values() -> None:
    routes = MODULE.parse_soracloud_alias_routes(
        ["solswap-indexer.sora=127.0.0.1:8788"]
    )

    assert routes == [
        MODULE.SoracloudAliasRoute(
            alias="solswap-indexer.sora",
            upstream_name="soracloud_solswap_indexer_sora_upstream",
            upstream_address="127.0.0.1:8788",
            pretty_host="solswap-indexer.sora.mon.taira.sora.net",
        )
    ]

    for value, expected in (
        ("solswap-indexer.sora", "ALIAS=HOST:PORT"),
        (
            "Solswap-Indexer.Sora=127.0.0.1:8788",
            "exact lowercase DNS spelling",
        ),
        ("solswap-indexer.sora.=127.0.0.1:8788", "trailing dot"),
        ("solswap/indexer.sora=127.0.0.1:8788", "canonical lowercase DNS labels"),
        ("solswap-.sora=127.0.0.1:8788", "canonical lowercase DNS labels"),
        ("solswap-indexer.sora=0.0.0.0:8788", "wildcard address"),
        ("solswap-indexer.sora=[::]:8788", "wildcard address"),
        ("solswap-indexer.sora=localhost:8788", "localhost alias"),
        (
            "solswap-indexer.sora=127.0.0.1:08788",
            "canonical decimal spelling",
        ),
        (
            "solswap-indexer.sora=127.0.0.1:not-a-port",
            "canonical decimal spelling",
        ),
        (" solswap-indexer.sora=127.0.0.1:8788", "canonical lowercase DNS labels"),
        ("solswap-indexer.sora=127.0.0.1:8788 ", "surrounding whitespace"),
    ):
        try:
            MODULE.parse_soracloud_alias_routes([value])
        except ValueError as error:
            assert expected in str(error)
        else:  # pragma: no cover
            raise AssertionError(f"accepted unsafe route {value!r}")

    try:
        MODULE.parse_soracloud_alias_routes(
            [
                "solswap-indexer.sora=127.0.0.1:8788",
                "solswap-indexer.sora=127.0.0.1:8789",
            ]
        )
    except ValueError as error:
        assert "duplicated" in str(error)
    else:  # pragma: no cover
        raise AssertionError("accepted duplicate Soracloud alias route")


def test_load_soracloud_alias_route_specs_from_roster(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path, include_soracloud_alias_route=True)

    assert MODULE.load_soracloud_alias_route_specs(roster_path) == [
        "solswap-indexer.sora=127.0.0.1:8788"
    ]
    routes = MODULE.parse_soracloud_alias_routes(
        MODULE.load_soracloud_alias_route_specs(roster_path)
    )
    assert routes[0].upstream_address == "127.0.0.1:8788"


def test_load_soracloud_alias_route_specs_rejects_bad_roster_entries(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    _write_roster(roster_path)
    roster_text = roster_path.read_text(encoding="utf-8")

    for extra, expected in (
        ('soracloud_alias_routes = "bad"\n', "array of tables"),
        (
            '[[soracloud_alias_routes]]\nedge_upstream = "127.0.0.1:8788"\n',
            "missing canonical field",
        ),
        (
            '[[soracloud_alias_routes]]\nalias = "solswap-indexer.sora"\n',
            "missing canonical field",
        ),
        (
            '[[soracloud_alias_routes]]\n'
            'alias = "solswap-indexer.sora"\n'
            'upstream_address = "127.0.0.1:8788"\n',
            "unknown first-release field",
        ),
        (
            '[[soracloud_alias_routes]]\n'
            'alias = "solswap-indexer.sora"\n'
            'upstream = "127.0.0.1:8788"\n',
            "unknown first-release field",
        ),
    ):
        prefix, marker, suffix = roster_text.partition("[[validators]]")
        roster_path.write_text(
            f"{prefix}{extra}\n{marker}{suffix}",
            encoding="utf-8",
        )
        try:
            MODULE.load_soracloud_alias_route_specs(roster_path)
        except ValueError as error:
            assert expected in str(error)
        else:  # pragma: no cover
            raise AssertionError(f"accepted bad route entry {extra!r}")


def test_render_requires_exactly_four_validators() -> None:
    validators = [
        MODULE.EdgeValidator(
            public_key=_bls_key(index),
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]
    for drifted in (validators[:-1], validators + [validators[-1]]):
        try:
            MODULE.render_edge_nginx_conf(drifted)
        except ValueError as error:
            assert "exactly 4 edge validators" in str(error)
        else:  # pragma: no cover
            raise AssertionError("renderer accepted a non-four-validator cohort")


def test_render_rejects_noncanonical_preconstructed_values() -> None:
    validators = [
        MODULE.EdgeValidator(
            public_key=_bls_key(index),
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]

    drifted_values = (
        (
            MODULE.EdgeValidator(
                public_key=_bls_key(1),
                slug="Taira-Validator-1",
                upstream_name="taira_validator_1",
                validator_host=validators[0].validator_host,
                upstream_address=validators[0].upstream_address,
            ),
            "lowercase kebab-case",
        ),
        (
            MODULE.EdgeValidator(
                public_key=_bls_key(1),
                slug=validators[0].slug,
                upstream_name="legacy_sanitized_name",
                validator_host=validators[0].validator_host,
                upstream_address=validators[0].upstream_address,
            ),
            "upstream name must be exactly",
        ),
        (
            MODULE.EdgeValidator(
                public_key=_bls_key(1),
                slug=validators[0].slug,
                upstream_name=validators[0].upstream_name,
                validator_host="Taira-Validator-1.sora.org",
                upstream_address=validators[0].upstream_address,
            ),
            "exact lowercase DNS spelling",
        ),
        (
            MODULE.EdgeValidator(
                public_key=_bls_key(1),
                slug=validators[0].slug,
                upstream_name=validators[0].upstream_name,
                validator_host=validators[0].validator_host,
                upstream_address="0.0.0.0:18080",
            ),
            "wildcard address",
        ),
    )
    for drifted, expected in drifted_values:
        cohort = [drifted, *validators[1:]]
        try:
            MODULE.render_edge_nginx_conf(cohort)
        except ValueError as error:
            assert expected in str(error)
        else:  # pragma: no cover
            raise AssertionError(f"renderer accepted non-canonical value {drifted!r}")

    route = MODULE.parse_soracloud_alias_routes(
        ["solswap-indexer.sora=127.0.0.1:8788"]
    )[0]
    drifted_route = MODULE.SoracloudAliasRoute(
        alias=route.alias,
        upstream_name="legacy_sanitized_name",
        upstream_address=route.upstream_address,
        pretty_host=route.pretty_host,
    )
    try:
        MODULE.render_edge_nginx_conf(
            validators,
            soracloud_alias_routes=[drifted_route],
        )
    except ValueError as error:
        assert "upstream name must be exactly" in str(error)
    else:  # pragma: no cover
        raise AssertionError("renderer accepted a normalized Soracloud route record")


def test_render_edge_nginx_conf_can_pin_soracloud_alias_route_to_service_upstream() -> None:
    validators = [
        MODULE.EdgeValidator(
            public_key=_bls_key(index),
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]
    routes = MODULE.parse_soracloud_alias_routes(
        ["solswap-indexer.sora=127.0.0.1:8788"]
    )

    rendered = MODULE.render_edge_nginx_conf(
        validators,
        soracloud_alias_routes=routes,
    )

    assert "upstream soracloud_solswap_indexer_sora_upstream {" in rendered
    assert "  server 127.0.0.1:8788;" in rendered
    assert (
        "solswap-indexer.sora.mon.taira.sora.net ~^.+\\.mon\\.taira\\.sora\\.net$;"
    ) in rendered
    assert "server_name solswap-indexer.sora.mon.taira.sora.net;" in rendered
    exact_host_server = rendered.split(
        "server_name solswap-indexer.sora.mon.taira.sora.net;",
        1,
    )[1].split("server_name *.mon.taira.sora.net", 1)[0]
    assert (
        "ssl_certificate /etc/letsencrypt/live/"
        "solswap-indexer.sora.mon.taira.sora.net/fullchain.pem;"
    ) in exact_host_server
    assert (
        "proxy_pass http://soracloud_solswap_indexer_sora_upstream;"
    ) in exact_host_server
    assert "proxy_set_header Host solswap-indexer.sora;" in exact_host_server
    assert "proxy_set_header X-Forwarded-Host $host;" in exact_host_server

    assert "/soradns/" not in rendered
    assert "$soradns_" not in rendered

    wildcard_mon_server = rendered.split(
        "server_name *.mon.taira.sora.net ~^.+\\.mon\\.taira\\.sora\\.net$;",
        1,
    )[1]
    assert "proxy_set_header Host $taira_mon_alias_host;" in wildcard_mon_server


def test_main_writes_rendered_conf(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    output_path = tmp_path / "taira.sora.org.conf"
    _write_roster(roster_path)

    exit_code = MODULE.main(["--roster", str(roster_path), "--output", str(output_path)])

    assert exit_code == 0
    rendered = output_path.read_text(encoding="utf-8")
    assert "Generated by scripts/render_taira_edge_nginx_conf.py" in rendered
    assert "server 127.0.0.1:18080;" in rendered


def test_main_writes_soracloud_alias_route(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    output_path = tmp_path / "taira.sora.org.conf"
    _write_roster(roster_path)

    exit_code = MODULE.main(
        [
            "--roster",
            str(roster_path),
            "--output",
            str(output_path),
            "--soracloud-alias-route",
            "solswap-indexer.sora=127.0.0.1:8788",
        ]
    )

    assert exit_code == 0
    rendered = output_path.read_text(encoding="utf-8")
    assert "upstream soracloud_solswap_indexer_sora_upstream {" in rendered
    assert "server 127.0.0.1:8788;" in rendered
    assert "server_name solswap-indexer.sora.mon.taira.sora.net;" in rendered


def test_main_writes_soracloud_alias_route_from_roster(tmp_path: Path) -> None:
    roster_path = tmp_path / "validator_roster.toml"
    output_path = tmp_path / "taira.sora.org.conf"
    _write_roster(roster_path, include_soracloud_alias_route=True)

    exit_code = MODULE.main(["--roster", str(roster_path), "--output", str(output_path)])

    assert exit_code == 0
    rendered = output_path.read_text(encoding="utf-8")
    assert "upstream soracloud_solswap_indexer_sora_upstream {" in rendered
    assert "server 127.0.0.1:8788;" in rendered
    assert "server_name solswap-indexer.sora.mon.taira.sora.net;" in rendered


def test_checked_in_example_matches_rendered_example_roster() -> None:
    validators = MODULE.load_edge_validators(EXAMPLE_ROSTER_PATH)
    rendered = MODULE.render_edge_nginx_conf(validators)
    checked_in = CHECKED_IN_EXAMPLE_PATH.read_text(encoding="utf-8")

    assert validators[0].upstream_address == "127.0.0.1:29080"
    assert checked_in.rstrip("\n") == rendered.rstrip("\n")


def _shared_host_roster(path: Path) -> None:
    _write_roster(path)
    text = path.read_text(encoding="utf-8")
    for index in range(1, 5):
        text = text.replace(
            f'https://taira-validator-{index}.sora.org"',
            f'https://test.example.org:{8442 + index}"',
        )
    path.write_text(text, encoding="utf-8")


def test_shared_hostname_ports_bind_distinct_tls_listeners_and_exact_upstreams(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    _shared_host_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    assert [(row.validator_host, row.https_port) for row in validators] == [
        ("test.example.org", port) for port in range(8443, 8447)
    ]
    rendered = MODULE.render_edge_nginx_conf(
        validators, public_host="test.example.org", public_upstream_validator="taira-validator-3"
    )
    http = rendered.split("  listen 80;", 1)[1].split("\n}", 1)[0]
    names = next(line for line in http.splitlines() if "server_name" in line)
    assert names.split().count("test.example.org") == 1
    for index, port in enumerate(range(8443, 8447), start=1):
        server = rendered.split(f"  listen {port} ssl;", 1)[1].split("\n}", 1)[0]
        assert f"  listen [::]:{port} ssl;" in server
        assert "  server_name test.example.org;" in server
        assert f"proxy_pass http://taira_validator_{index}_upstream;" in server
        assert "ssl_certificate /etc/letsencrypt/live/" in server
        assert "proxy_set_header Host $host;" in server
        assert "rewrite " not in server
        assert "proxy_pass http://taira_validator_" in server
    public = rendered.split("upstream taira_public_edge_upstream {", 1)[1].split("}", 1)[0]
    assert "server 127.0.0.1:18082 max_fails=1 fail_timeout=5s;" in public


def test_validator_listener_rejects_duplicates_reserved_ports_and_edge_collisions(tmp_path: Path) -> None:
    from dataclasses import replace
    roster = tmp_path / "roster.toml"
    _shared_host_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    invalid = [
        replace(validators[0], https_port=80),
        replace(validators[0], https_port=0),
        replace(validators[0], https_port=65536),
        replace(validators[0], https_port=True),
    ]
    for hostname in [
        "taira-explorer.sora.org", "mon.taira.sora.net",
        "app.mon.taira.sora.net", "nested.app.mon.taira.sora.net",
        "site.sorafs.taira.sora.org",
    ]:
        invalid.append(replace(validators[0], validator_host=hostname, https_port=443))
    for first in invalid:
        try:
            MODULE.render_edge_nginx_conf([first, *validators[1:]])
        except ValueError:
            pass
        else:
            raise AssertionError(f"accepted colliding or invalid listener {first!r}")
    duplicate = replace(validators[1], https_port=validators[0].https_port)
    try:
        MODULE.render_edge_nginx_conf([validators[0], duplicate, *validators[2:]])
    except ValueError as error:
        assert "listener" in str(error) and "duplicated" in str(error)
    else:
        raise AssertionError("accepted the same hostname and effective port twice")
    text = roster.read_text(encoding="utf-8").replace(":8444", ":8443", 1)
    roster.write_text(text, encoding="utf-8")
    try:
        MODULE.load_edge_validators(roster)
    except ValueError as error:
        assert "listener" in str(error) and "duplicated" in str(error)
    else:
        raise AssertionError("accepted duplicate roster listeners")


def test_validator_listener_origins_reject_noncanonical_ports_and_signed_path_rewrites(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    for origin in [
        "https://test.example.org:0", "https://test.example.org:80",
        "https://test.example.org:443", "https://test.example.org:08443",
        "https://test.example.org:65536", "https://test.example.org:8443/",
        "https://test.example.org:8443/validator-1", "https://user@test.example.org:8443",
        "https://test.example.org:8443?query=1", "https://test.example.org:8443#fragment",
        "https://127.0.0.1:8443", "https://0177.0.0.1:8443", "https://0x7f.0.0.1:8443",
        "https://localhost:8443", "https://test.local:8443",
    ]:
        _shared_host_roster(roster)
        roster.write_text(roster.read_text(encoding="utf-8").replace(
            "https://test.example.org:8443", origin, 1), encoding="utf-8")
        try:
            MODULE.load_edge_validators(roster)
        except ValueError:
            pass
        else:
            raise AssertionError(f"accepted noncanonical public listener origin {origin!r}")


def test_main_selects_same_host_upstream_by_exact_validator_slug(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    output = tmp_path / "edge.conf"
    _shared_host_roster(roster)
    assert MODULE.main([
        "--roster", str(roster), "--output", str(output),
        "--public-upstream-validator", "taira-validator-3",
    ]) == 0
    rendered = output.read_text(encoding="utf-8")
    public = rendered.split("upstream taira_public_edge_upstream {", 1)[1].split("}", 1)[0]
    assert "server 127.0.0.1:18082 max_fails=1 fail_timeout=5s;" in public
    try:
        MODULE.main(["--roster", str(roster), "--output", str(output),
                     "--public-upstream-host", "test.example.org"])
    except SystemExit as error:
        assert error.code == 2
    else:
        raise AssertionError("accepted removed ambiguous hostname selector")


def test_scoped_validator_listeners_bind_explicit_interfaces_and_preserve_paths(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    _shared_host_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    rendered = MODULE.render_validator_listeners_conf(
        validators,
        listen_addresses=["203.0.113.10", "2001:db8::10"],
        tls_certificate="/opt/homebrew/var/letsencrypt/config/live/test.example.org/fullchain.pem",
        tls_certificate_key="/opt/homebrew/var/letsencrypt/config/live/test.example.org/privkey.pem",
    )
    assert rendered.count("server_name test.example.org;") == 4
    assert "listen 443" not in rendered and "listen 80" not in rendered
    assert "listen [::]:" not in rendered and "listen 8443" not in rendered
    assert "ssl_dhparam" not in rendered and "include " not in rendered
    assert "taira_public_edge" not in rendered and "explorer" not in rendered
    assert "rewrite " not in rendered
    assert "$proxy_add_x_forwarded_for" not in rendered
    for index, port in enumerate(range(8443, 8447), start=1):
        block = rendered.split(f"listen 203.0.113.10:{port} ssl;", 1)[1].split("\n}", 1)[0]
        assert f"listen [2001:db8::10]:{port} ssl;" in block
        assert f"server 127.0.0.1:{18079 + index};" in rendered
        assert f"proxy_pass http://taira_validator_{index}_upstream;" in block
        assert f"proxy_pass http://taira_validator_{index}_upstream/" not in block
        assert "proxy_set_header Host $host;" in block
        assert "proxy_set_header X-Forwarded-For $remote_addr;" in block
        assert "location = /v1/mcp {" in block
        assert "location = /v1/connect/ws {" in block


def test_scoped_listener_rejects_ambiguous_bindings_and_tls_directive_injection(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    _shared_host_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    canonical = dict(
        listen_addresses=["203.0.113.10"],
        tls_certificate="/var/tls/fullchain.pem",
        tls_certificate_key="/var/tls/privkey.pem",
    )
    for addresses in (
        [], ["0.0.0.0"], ["::"], ["224.0.0.1"], ["ff02::1"],
        ["203.000.113.10"], ["example.org"], ["203.0.113.10:8443"],
        ["fe80::1%en0"], ["2001:0db8::10"], ["::ffff:cb00:710a"],
        ["203.0.113.10", "203.0.113.10"],
    ):
        try:
            MODULE.render_validator_listeners_conf(validators, **{**canonical, "listen_addresses": addresses})
        except ValueError:
            pass
        else:
            raise AssertionError(f"accepted ambiguous listen bindings {addresses!r}")
    for field, value in (
        ("tls_certificate", "/var/tls/../fullchain.pem"),
        ("tls_certificate", "/var//tls/fullchain.pem"),
        ("tls_certificate", "/var/tls/fullchain.pem;include /tmp/bad"),
        ("tls_certificate_key", "/var/tls/key\nlisten 443 ssl;"),
        ("tls_certificate_key", "relative.key"),
        ("tls_certificate_key", "/var/tls/fullchain.pem"),
        ("client_max_body_size", "1g;include /tmp/bad"),
    ):
        try:
            MODULE.render_validator_listeners_conf(validators, **{**canonical, field: value})
        except ValueError:
            pass
        else:
            raise AssertionError(f"accepted unsafe scoped argument {field}={value!r}")


def test_main_scoped_listeners_require_complete_explicit_inputs_before_output(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    output = tmp_path / "peers.conf"
    _shared_host_roster(roster)
    base = ["--roster", str(roster), "--output", str(output)]
    scoped = ["--validator-listeners-only", "--validator-listen-address", "203.0.113.10",
              "--tls-certificate", "/var/tls/fullchain.pem", "--tls-certificate-key", "/var/tls/privkey.pem"]
    for flags in (scoped[:-2], scoped[1:], scoped + ["--public-upstream-validator", "taira-validator-1"]):
        try:
            MODULE.main(base + flags)
        except SystemExit as error:
            assert error.code == 2
        else:
            raise AssertionError("accepted incomplete or mixed route scope")
        assert not output.exists()
    assert MODULE.main(base + scoped) == 0
    rendered = output.read_text(encoding="utf-8")
    assert "listen 203.0.113.10:8443 ssl;" in rendered
    assert "listen 443 ssl;" not in rendered


def test_private_backends_restrict_one_edge_and_preserve_sanitized_caller_headers(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    _shared_host_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    rendered = MODULE.render_private_backend_listeners_conf(
        validators, listen_address="192.168.64.3", port_base=18080,
        trusted_edge_address="192.168.64.1",
    )
    assert "ssl" not in rendered and "rewrite " not in rendered
    assert rendered.count("allow 192.168.64.1;") == 4
    assert rendered.count("deny all;") == 4
    for index in range(4):
        block = rendered.split(f"listen 192.168.64.3:{18080 + index};", 1)[1].split("\n}", 1)[0]
        assert f"proxy_pass http://taira_validator_{index + 1}_upstream;" in block
        assert f"proxy_pass http://taira_validator_{index + 1}_upstream/" not in block
        assert "proxy_set_header X-Real-IP $http_x_real_ip;" in block
        assert "proxy_set_header X-Forwarded-For $http_x_forwarded_for;" in block
        assert "proxy_set_header X-Forwarded-Proto $http_x_forwarded_proto;" in block
        assert 'if ($http_x_forwarded_for != $http_x_real_ip) { return 400; }' in block
        assert 'if ($http_x_forwarded_proto != "https") { return 400; }' in block
        assert 'if ($http_x_real_ip = "") { return 400; }' in block


def test_private_backends_refuse_public_bindings_port_overflow_and_nonloopback_upstreams(tmp_path: Path) -> None:
    from dataclasses import replace
    roster = tmp_path / "roster.toml"
    _shared_host_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    canonical = dict(listen_address="192.168.64.3", port_base=18080, trusted_edge_address="192.168.64.1")
    for field, value in (
        ("listen_address", "208.83.1.62"), ("listen_address", "127.0.0.1"),
        ("listen_address", "0.0.0.0"), ("listen_address", "192.168.064.3"),
        ("trusted_edge_address", "192.168.64.3"), ("trusted_edge_address", "localhost"),
        ("trusted_edge_address", "::1"), ("trusted_edge_address", "192.168.64.1;allow all"),
        ("port_base", 1023), ("port_base", 65533), ("port_base", True),
    ):
        try:
            MODULE.render_private_backend_listeners_conf(validators, **{**canonical, field: value})
        except ValueError:
            pass
        else:
            raise AssertionError(f"accepted unsafe backend argument {field}={value!r}")
    drifted = [replace(validators[0], upstream_address="192.168.64.3:10080"), *validators[1:]]
    try:
        MODULE.render_private_backend_listeners_conf(drifted, **canonical)
    except ValueError as error:
        assert "exact loopback" in str(error)
    else:
        raise AssertionError("accepted a nonloopback private Torii backend")


def test_main_private_backend_scope_is_complete_and_disjoint_before_output(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    output = tmp_path / "private-backends.conf"
    _shared_host_roster(roster)
    base = ["--roster", str(roster), "--output", str(output)]
    scoped = ["--private-backend-listeners-only", "--backend-listen-address", "192.168.64.3",
              "--backend-port-base", "18080", "--backend-trusted-edge-address", "192.168.64.1"]
    for flags in (scoped[:-2], scoped[1:], scoped + ["--validator-listeners-only"],
                  scoped + ["--tls-certificate", "/var/tls/fullchain.pem"]):
        try:
            MODULE.main(base + flags)
        except SystemExit as error:
            assert error.code == 2
        else:
            raise AssertionError("accepted incomplete or mixed private backend scope")
        assert not output.exists()
    assert MODULE.main(base + scoped) == 0
    assert "listen 192.168.64.3:18083;" in output.read_text(encoding="utf-8")


def _shared_public_roster(path: Path) -> None:
    _write_roster(path)
    text = path.read_text(encoding="utf-8")
    for index in range(1, 5):
        text = text.replace(f"https://taira-validator-{index}.sora.org", "https://taira.sora.org")
    path.write_text(text, encoding="utf-8")


def _finality_peer_map(rendered: str):
    block = rendered.split("map $args $taira_finality_peer_upstream {\n", 1)[1].split("\n}", 1)[0]
    assert block.splitlines()[0] == '  default "";'
    assert "~*" not in block
    entries = []
    for line in block.splitlines()[1:]:
        match = re.fullmatch(r"  ~(\^peer_id=ea0130[0-9A-F]{96}\$) (taira_validator_[1-4]_upstream);", line)
        assert match is not None, line
        entries.append((re.compile(match[1]), match[2]))
    assert len(entries) == 4
    return entries


def _selected_peer(entries, args: str) -> str:
    matches = [upstream for pattern, upstream in entries if pattern.fullmatch(args)]
    assert len(matches) <= 1
    return matches[0] if matches else ""


def test_shared_public_root_has_one_server_and_four_exact_signed_peer_routes(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    _shared_public_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    assert {row.validator_host for row in validators} == {"taira.sora.org"}
    assert {row.https_port for row in validators} == {443}
    assert [row.public_key for row in validators] == [_bls_key(index) for index in range(1, 5)]
    rendered = MODULE.render_edge_nginx_conf(validators, public_upstream_validator="taira-validator-3")
    assert rendered.count("  server_name taira.sora.org;\n") == 1
    entries = _finality_peer_map(rendered)
    for index in range(1, 5):
        assert _selected_peer(entries, "peer_id=" + _bls_key(index)) == f"taira_validator_{index}_upstream"
    public = rendered.split("  server_name taira.sora.org;\n", 1)[1].split("\n}", 1)[0]
    marker = "location ~ ^/v1/bridge/finality/attestation/(?:[1-9][0-9]*|latest)$"
    finality = _location_block(public, marker)
    assert 'if ($taira_finality_peer_upstream = "") { return 400; }' in finality
    assert "proxy_pass http://$taira_finality_peer_upstream;" in finality
    assert "proxy_next_upstream off;" in finality
    assert "proxy_set_header Host $host;" in finality
    assert "proxy_set_header X-Forwarded-Host $host;" in finality
    assert "proxy_set_header X-Forwarded-For $remote_addr;" in finality
    assert "rewrite " not in finality and "$request_uri" not in finality
    assert "location ^~ / {" not in public
    fallback = _location_block(public, "location / {")
    assert "proxy_pass http://taira_public_edge_upstream;" in fallback
    assert "$taira_finality_peer_upstream" not in fallback
    convenience = rendered.split("upstream taira_public_edge_upstream {", 1)[1].split("}", 1)[0]
    assert "127.0.0.1:18082" in convenience and "127.0.0.1:18080" not in convenience


@pytest.mark.parametrize("selector", [
    "", "peer_id=unknown", "peer_id={key}&peer_id={key}", "peer_id={key}&peer_id={other}",
    "peer_id={other}&peer_id={key}", "peer_id={key}&height=1", "height=1&peer_id={key}",
    "peer_id={key}&", "peer_id=%65a0130{suffix}", "peer%5Fid={key}",
    "Peer_id={key}", "PEER_ID={key}", "peer_id={lower}", "peer_id={upper}",
])
def test_raw_finality_peer_selector_refuses_unknown_duplicate_escaped_additional_and_case_forms(tmp_path: Path, selector: str) -> None:
    roster = tmp_path / "roster.toml"
    _shared_public_roster(roster)
    rendered = MODULE.render_edge_nginx_conf(MODULE.load_edge_validators(roster))
    entries = _finality_peer_map(rendered)
    key = _bls_key(1)
    args = selector.format(key=key, other=_bls_key(2), suffix=key[6:], lower=key.lower(), upper=key.upper())
    assert _selected_peer(entries, args) == ""


@pytest.mark.parametrize("fault", ["missing", "placeholder", "wrong_prefix", "lower_hex", "short", "duplicate_key", "duplicate_slug", "duplicate_upstream"])
def test_roster_peer_routing_requires_four_distinct_canonical_bls_keys_slugs_and_upstreams(tmp_path: Path, fault: str) -> None:
    roster = tmp_path / "roster.toml"
    _shared_public_roster(roster)
    text = roster.read_text(encoding="utf-8")
    if fault == "missing":
        text = text.replace(f'public_key = "{_bls_key(1)}"\n', "", 1)
    elif fault == "placeholder":
        text = text.replace(_bls_key(1), "REPLACE_WITH_BLS_PUBLIC_KEY_1", 1)
    elif fault == "wrong_prefix":
        text = text.replace(_bls_key(1), "ed0120" + _bls_key(1)[6:], 1)
    elif fault == "lower_hex":
        text = text.replace(_bls_key(1), _bls_key(1).lower(), 1)
    elif fault == "short":
        text = text.replace(_bls_key(1), _bls_key(1)[:-2], 1)
    elif fault == "duplicate_key":
        text = text.replace(_bls_key(2), _bls_key(1), 1)
    elif fault == "duplicate_slug":
        text = text.replace('slug = "taira-validator-2"', 'slug = "taira-validator-1"', 1)
    else:
        text = text.replace("127.0.0.1:18081", "127.0.0.1:18080", 1)
    roster.write_text(text, encoding="utf-8")
    with pytest.raises(ValueError):
        MODULE.load_edge_validators(roster)


def test_shared_scoped_tls_listener_is_deduplicated_and_retains_peer_map(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    _shared_public_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    rendered = MODULE.render_validator_listeners_conf(validators, listen_addresses=["203.0.113.10"],
        tls_certificate="/var/tls/fullchain.pem", tls_certificate_key="/var/tls/privkey.pem")
    assert rendered.count("  listen 203.0.113.10:443 ssl;") == 1
    assert rendered.count("  server_name taira.sora.org;") == 1
    entries = _finality_peer_map(rendered)
    for index in range(1, 5):
        assert _selected_peer(entries, "peer_id=" + _bls_key(index)) == f"taira_validator_{index}_upstream"


def test_public_gateway_binds_explicit_interfaces_tls_one_root_and_selected_convenience_peer(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    _shared_public_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    rendered = MODULE.render_public_gateway_conf(validators,
        listen_addresses=["203.0.113.10", "2001:db8::10"],
        tls_certificate="/var/tls/actual-public-chain.pem", tls_certificate_key="/var/tls/actual-public-key.pem",
        public_upstream_validator="taira-validator-3")
    assert rendered.count("\nserver {\n") == 1
    assert "  listen 203.0.113.10:443 ssl;" in rendered
    assert "  listen [2001:db8::10]:443 ssl;" in rendered
    assert "  listen 443 ssl;" not in rendered and "listen [::]:443" not in rendered
    assert rendered.count("  server_name taira.sora.org;") == 1
    assert "ssl_certificate /var/tls/actual-public-chain.pem;" in rendered
    assert "ssl_certificate_key /var/tls/actual-public-key.pem;" in rendered
    assert "/etc/letsencrypt" not in rendered
    assert "server_name taira-explorer.sora.org" not in rendered
    assert "map $http_origin $taira_public_torii_cors_origin" in rendered
    entries = _finality_peer_map(rendered)
    for index in range(1, 5):
        assert _selected_peer(entries, "peer_id=" + _bls_key(index)) == f"taira_validator_{index}_upstream"
    block = _location_block(rendered, "location ~ ^/v1/bridge/finality/attestation/(?:[1-9][0-9]*|latest)$")
    assert "proxy_next_upstream off;" in block and "proxy_cache off;" in block
    assert "proxy_pass http://$taira_finality_peer_upstream;" in block and "rewrite " not in block
    assert "location ^~ / {" not in rendered
    assert "proxy_pass http://taira_public_edge_upstream;" in _location_block(rendered, "location / {")
    assert "location = /v1/connect/session" in rendered
    assert "proxy_pass http://taira_validator_3_upstream;" in _location_block(rendered, "location = /v1/mcp")
    convenience = rendered.split("upstream taira_public_edge_upstream {", 1)[1].split("}", 1)[0]
    assert "server 127.0.0.1:18082 max_fails=1 fail_timeout=5s;" in convenience


@pytest.mark.parametrize("fault", ["nonshared", "missing_address", "wildcard", "tls_injection", "same_tls", "unknown_pin"])
def test_public_gateway_requires_exact_shared_root_and_explicit_safe_native_inputs(tmp_path: Path, fault: str) -> None:
    from dataclasses import replace
    roster = tmp_path / "roster.toml"
    _shared_public_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    arguments = dict(listen_addresses=["203.0.113.10"], tls_certificate="/var/tls/chain.pem", tls_certificate_key="/var/tls/key.pem")
    if fault == "nonshared":
        validators = [replace(validators[0], validator_host="individual.example.org"), *validators[1:]]
    elif fault == "missing_address":
        arguments["listen_addresses"] = []
    elif fault == "wildcard":
        arguments["listen_addresses"] = ["0.0.0.0"]
    elif fault == "tls_injection":
        arguments["tls_certificate"] = "/var/tls/chain.pem;include /tmp/other"
    elif fault == "same_tls":
        arguments["tls_certificate_key"] = arguments["tls_certificate"]
    else:
        arguments["public_upstream_validator"] = "unknown-peer"
    with pytest.raises(ValueError):
        MODULE.render_public_gateway_conf(validators, **arguments)


def test_cli_public_gateway_rejects_incomplete_or_mixed_scope_and_accepts_explicit_shared_gateway(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    output = tmp_path / "public.conf"
    _shared_public_roster(roster)
    base = ["--roster", str(roster), "--output", str(output)]
    scope = ["--public-gateway-only", "--gateway-listen-address", "203.0.113.10",
        "--tls-certificate", "/var/tls/chain.pem", "--tls-certificate-key", "/var/tls/key.pem"]
    for args in (scope[:-2], ["--public-gateway-only"], scope + ["--validator-listeners-only"],
                 scope + ["--validator-listen-address", "203.0.113.10"],
                 scope + ["--backend-listen-address", "192.168.64.3"],
                 scope + ["--soracloud-alias-route", "alias.sora=127.0.0.1:8788"],
                 ["--gateway-listen-address", "203.0.113.10"]):
        with pytest.raises(SystemExit) as refused:
            MODULE.main(base + args)
        assert refused.value.code == 2 and not output.exists()
    assert MODULE.main(base + scope + ["--public-host", "taira.sora.org", "--public-upstream-validator", "taira-validator-3"]) == 0
    rendered = output.read_text(encoding="utf-8")
    assert rendered.count("\nserver {\n") == 1
    assert "listen 203.0.113.10:443 ssl;" in rendered


def test_finality_attestation_prefix_rejects_noncanonical_heights_and_path_aliases(tmp_path: Path) -> None:
    roster = tmp_path / "roster.toml"
    _shared_public_roster(roster)
    validators = MODULE.load_edge_validators(roster)
    rendered = MODULE.render_public_gateway_conf(validators, listen_addresses=["203.0.113.10"],
        tls_certificate="/var/tls/chain.pem", tls_certificate_key="/var/tls/key.pem")
    marker = "location ~ ^/v1/bridge/finality/attestation/(?:[1-9][0-9]*|latest)$"
    regex_block = _location_block(rendered, marker)
    prefix = "location /v1/bridge/finality/attestation/ {"
    refusal = _location_block(rendered, prefix)
    assert "return 400;" in refusal and "proxy_pass" not in refusal
    assert "location ^~ /v1/bridge/finality/attestation/" not in rendered
    assert rendered.index(marker) < rendered.index(prefix)
    assert "proxy_pass http://$taira_finality_peer_upstream;" in regex_block
    path_match = re.compile(r"^/v1/bridge/finality/attestation/(?:[1-9][0-9]*|latest)$")
    entries = _finality_peer_map(rendered)
    for tail in ("1", "12345", "latest"):
        assert path_match.fullmatch("/v1/bridge/finality/attestation/" + tail)
        assert _selected_peer(entries, "peer_id=" + _bls_key(1)) == "taira_validator_1_upstream"
        assert _selected_peer(entries, "peer_id=unknown") == ""
    for tail in ("01", "0", "00", "+1", "-1", "1/", "latest/", "LATEST", "1/extra", "latest/extra"):
        path = "/v1/bridge/finality/attestation/" + tail
        assert path.startswith("/v1/bridge/finality/attestation/") and not path_match.fullmatch(path)
        # The plain longest-prefix location returns400 without reaching the
        # convenience upstream when the canonical regex does not match.
        assert "proxy_pass" not in refusal
    assert "X-Iroha-Finality-Challenge" in MODULE.PUBLIC_TORII_CORS_HEADERS
    assert "X-Iroha-Finality-Challenge" in rendered
