"""Tests for scripts/render_taira_edge_nginx_conf.py."""

from __future__ import annotations

import importlib.util
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
                f'public_key = "peer-{index}-public"',
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
            "explicit port",
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
    assert "proxy_set_header Host taira-validator-1.sora.org;" in rendered
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

    validators = [
        MODULE.EdgeValidator(
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

    for origin in cors["allowed_origins"]:
        assert f'  "{origin}" $http_origin;' in rendered
    assert (
        f'add_header Access-Control-Allow-Headers "{MODULE.PUBLIC_TORII_CORS_HEADERS}" always;'
        in public_server
    )
    assert (
        "idempotency-key" in MODULE.PUBLIC_TORII_CORS_HEADERS
    ), "browser Kagemusha V1 top-up and redemption require Idempotency-Key"
    for header in (
        "mcp-method",
        "mcp-name",
        "mcp-protocol-version",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-timestamp-ms",
        "x-iroha-nonce",
        "x-iroha-witness",
    ):
        assert header in MODULE.PUBLIC_TORII_CORS_HEADERS
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
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]
    rendered = MODULE.render_edge_nginx_conf(validators)
    assert "proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;" in rendered
    assert "proxy_set_header X-Real-IP $remote_addr;" in rendered


def test_render_edge_nginx_conf_uses_explicit_canonical_public_validator() -> None:
    validators = [
        MODULE.EdgeValidator(
            slug=f"taira-validator-{index}",
            upstream_name=f"taira_validator_{index}",
            validator_host=f"taira-validator-{index}.sora.org",
            upstream_address=f"127.0.0.1:{18079 + index}",
        )
        for index in range(1, 5)
    ]

    rendered = MODULE.render_edge_nginx_conf(
        validators,
        public_upstream_host="taira-validator-3.sora.org",
    )
    public_upstream = rendered.split(
        "upstream taira_public_edge_upstream {", 1
    )[1].split("}", 1)[0]

    assert "server 127.0.0.1:18082 max_fails=1 fail_timeout=5s;" in public_upstream
    assert "127.0.0.1:18080" not in public_upstream
    public_server = rendered.split("server_name taira.sora.org;", 1)[1].split(
        "server_name mon.taira.sora.net;", 1
    )[0]
    assert "proxy_set_header Host taira-validator-3.sora.org;" in public_server
    assert "proxy_pass http://taira_validator_3_upstream;" in public_server


def test_render_edge_nginx_conf_rejects_unknown_public_validator() -> None:
    validators = [
        MODULE.EdgeValidator(
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
            public_upstream_host="not-a-validator.sora.org",
        )
    except ValueError as error:
        assert "must match a validator hostname" in str(error)
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
                slug="Taira-Validator-1",
                upstream_name="taira_validator_1",
                validator_host=validators[0].validator_host,
                upstream_address=validators[0].upstream_address,
            ),
            "lowercase kebab-case",
        ),
        (
            MODULE.EdgeValidator(
                slug=validators[0].slug,
                upstream_name="legacy_sanitized_name",
                validator_host=validators[0].validator_host,
                upstream_address=validators[0].upstream_address,
            ),
            "upstream name must be exactly",
        ),
        (
            MODULE.EdgeValidator(
                slug=validators[0].slug,
                upstream_name=validators[0].upstream_name,
                validator_host="Taira-Validator-1.sora.org",
                upstream_address=validators[0].upstream_address,
            ),
            "exact lowercase DNS spelling",
        ),
        (
            MODULE.EdgeValidator(
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
