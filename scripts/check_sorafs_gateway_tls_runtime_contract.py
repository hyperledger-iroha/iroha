#!/usr/bin/env python3
"""Fail closed when the SoraFS gateway regains a placeholder ACME path."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_evidence_json import read_evidence_bytes  # noqa: E402
from sorafs_evidence_paths import (  # noqa: E402
    inspect_evidence_directory,
    inspect_evidence_file,
    resolve_evidence_path,
)

REQUIRED_RUNTIME_NOTICE = "> **Runtime ACME boundary (V1):**"
MAX_CONTRACT_SOURCE_BYTES = 8 * 1024 * 1024
FORBIDDEN_DOC_CLAIMS = (
    "sorafs-gateway tls renew",
    "fall back to stored cert",
    "deterministic self-signed bundle required for staging drills",
    "Production ACME clients remain available for validated accounts",
    "[`letsencrypt-rs`](https://crates.io/crates/letsencrypt-rs)",
    "Route53 support is available by default",
    "Generate a replacement bundle with the repository wrapper",
)


# This is a narrow source contract, not a Rust parser or native qualification.
# Literal tokens stay atomic so comments/quoted examples cannot supply code.
_RUST_TOKEN = re.compile(
    r'\s+|//[^\n]*|/\*|(?:br|r)(?P<raw_hashes>\#{0,255})"'
    r'|(?:b|c)?"(?:\\.|[^"\\])*"|(?:b)?\'(?:\\.|[^\'\\])\''
    r'|[A-Za-z_][A-Za-z_0-9]*|.',
    re.DOTALL,
)


def _tokens(source: str) -> tuple[str, ...]:
    """Keep code/literal tokens while discarding whitespace and nested comments."""

    result: list[str] = []
    cursor = 0
    while cursor < len(source):
        match = _RUST_TOKEN.match(source, cursor)
        assert match is not None
        token = match.group()
        cursor = match.end()
        if token.isspace() or token.startswith("//"):
            continue
        if token == "/*":
            depth = 1
            while depth:
                opening = source.find("/*", cursor)
                closing = source.find("*/", cursor)
                if closing < 0:
                    return ()
                if 0 <= opening < closing:
                    depth += 1
                    cursor = opening + 2
                else:
                    depth -= 1
                    cursor = closing + 2
            continue
        if match.group("raw_hashes") is not None:
            end_marker = '"' + match.group("raw_hashes")
            end = source.find(end_marker, cursor)
            if end < 0:
                return ()
            cursor = end + len(end_marker)
            token = source[match.start():cursor]
        result.append(token)
    return tuple(result)


def _positions(tokens: tuple[str, ...], expected: tuple[str, ...]) -> list[int]:
    return [
        index for index, token in enumerate(tokens)
        if expected and token == expected[0]
        and tokens[index:index + len(expected)] == expected
    ]


def _contains_once(tokens: tuple[str, ...], source: str) -> bool:
    return len(_positions(tokens, _tokens(source))) == 1


def _body(tokens: tuple[str, ...], owner: str) -> tuple[str, ...]:
    """Read the unique named owner's balanced body; ambiguity fails closed."""

    marker = _tokens(owner)
    starts = _positions(tokens, marker)
    if len(starts) != 1:
        return ()
    start = starts[0] + len(marker)
    while start < len(tokens) and tokens[start] != "{":
        start += 1
    depth = 1
    for end in range(start + 1, len(tokens)):
        if tokens[end] == "{":
            depth += 1
        elif tokens[end] == "}":
            depth -= 1
            if depth == 0:
                return tokens[start + 1:end]
    return ()


_CONTROLLER_CONSTRUCTOR = """
    let hostnames = config.hostnames.clone();
    let automation =
        AcmeAutomation::try_new(config, client_binding.clone(), Arc::clone(&client))?;
    Ok(Self {
        automation: Mutex::new(automation), client_binding, client, tls_state,
        hostnames, poll_interval: DEFAULT_POLL_INTERVAL,
    })
"""
_ACME_CONSTRUCTOR = """
    qualify_acme_client(&client_binding, &client)?;
    Ok(Self { config, client_binding, client, state: AcmeState::default(), })
"""
_PROVIDER_BINDING_FAILURE = """
    if config.acme.enabled != config.acme.provider.is_some() {
        return Err(ToriiBuildError::invalid_configuration(
            "sorafs.gateway.acme.provider",
            "provider binding must be present exactly when ACME is enabled",
        ));
    }
"""
_ENABLED_WITHOUT_CLIENT = """
    (true, None) => {
        return Err(ToriiBuildError::invalid_runtime_dependency(
            "sorafs.gateway.acme",
            "ACME is enabled but no runtime client was supplied",
        ));
    }
"""
_DISABLED_WITH_CLIENT = """
    (false, Some(_)) => {
        return Err(ToriiBuildError::invalid_runtime_dependency(
            "sorafs.gateway.acme",
            "runtime client supplied while ACME is disabled",
        ));
    }
"""
_QUALIFIED_CLIENT_BRANCH = """
    (true, Some(client)) => {
        let provider = config.acme.provider.as_ref().ok_or_else(|| {
            ToriiBuildError::invalid_configuration(
                "sorafs.gateway.acme.provider",
                "ACME is enabled but no provider binding was configured",
            )
        })?;
        let binding = gateway_runtime_provider_binding(provider)?;
        Some(Arc::new(
            TlsAutomationHandle::try_new(
                gateway_acme_config(&config.acme), binding, client,
                Arc::clone(&tls_state),
            ).map_err(|error| {
                ToriiBuildError::invalid_runtime_dependency(
                    "sorafs.gateway.acme",
                    format!("client failed exact startup qualification: {error:?}"),
                )
            })?,
        ))
    }
"""
_RUNTIME_SETTER_EMITTER = """
    (
        $(
            $(#[$attribute:meta])*
            $name:ident($argument:ident: $dependency:ty $(,)?) => $field:ident;
        )+
    ) => {
        $(
            $(#[$attribute])*
            #[must_use]
            pub fn $name(mut self, $argument: $dependency) -> Self {
                self.$field = Some($argument);
                self
            }
        )+
    };
"""
_RUNTIME_SETTER_ROWS = (
    ("""
        with_sorafs_gateway_acme_client(
            client: Arc<dyn iroha_torii::sorafs::gateway::AcmeClient>,
        ) => sorafs_gateway_acme_client;
    """, "irohad:missing-runtime-acme-injection"),
    ("""
        with_sorafs_gateway_compliance_feed_transport(
            transport: Arc<dyn iroha_torii::sorafs::gateway::GatewayComplianceFeedTransport>,
        ) => sorafs_gateway_compliance_feed_transport;
    """, "irohad:missing-runtime-compliance-transport-injection"),
)


def _repository_root_identity(root: Path, failures: list[str]) -> Path | None:
    """Return one validated repository-root identity."""

    identity_errors: list[str] = []
    is_directory = inspect_evidence_directory(root, identity_errors)
    identity = resolve_evidence_path(
        root,
        identity_errors,
        label="gateway TLS contract repository root",
    )
    if is_directory is not True or identity is None or identity_errors:
        failures.append("repository-root:unsafe-or-unresolvable")
        return None
    return identity


def _read(
    root_identity: Path,
    relative: str,
    failures: list[str],
) -> str | None:
    """Read one bounded, regular, repository-contained UTF-8 source file."""

    path = root_identity / relative
    identity_errors: list[str] = []
    is_file = inspect_evidence_file(path, identity_errors)
    identity = resolve_evidence_path(
        path,
        identity_errors,
        label="gateway TLS contract source",
    )
    if (
        is_file is not True
        or identity is None
        or identity_errors
        or not identity.is_relative_to(root_identity)
    ):
        failures.append(f"{relative}:unsafe-or-unresolvable-source")
        return None
    try:
        raw = read_evidence_bytes(path, MAX_CONTRACT_SOURCE_BYTES)
        return raw.decode("utf-8")
    except (OSError, RuntimeError, UnicodeDecodeError, ValueError):
        failures.append(f"{relative}:unreadable-or-oversized-source")
        return None


def check_contract(root: Path) -> list[str]:
    """Return deterministic contract failures relative to ``root``."""

    failures: list[str] = []
    root_identity = _repository_root_identity(root, failures)
    if root_identity is None:
        return failures

    controller = _read(
        root_identity,
        "crates/iroha_torii/src/sorafs/gateway/controller.rs",
        failures,
    )
    module = _read(
        root_identity,
        "crates/iroha_torii/src/sorafs/gateway/mod.rs",
        failures,
    )
    acme = _read(
        root_identity,
        "crates/iroha_torii/src/sorafs/gateway/acme.rs",
        failures,
    )
    xtask = _read(root_identity, "xtask/src/sorafs.rs", failures)
    torii = _read(
        root_identity,
        "crates/iroha_torii/src/lib.rs",
        failures,
    )
    irohad = _read(
        root_identity,
        "crates/irohad/src/main.rs",
        failures,
    )
    runtime_deps = _read(
        root_identity,
        "crates/irohad/src/main/runtime_deps.rs",
        failures,
    )
    if any(
        source is None
        for source in (controller, module, acme, xtask, torii, irohad, runtime_deps)
    ):
        return failures
    assert controller is not None
    assert module is not None
    assert acme is not None
    assert xtask is not None
    assert torii is not None
    assert irohad is not None
    assert runtime_deps is not None

    test_boundary = controller.find("#[cfg(test)]")
    production_controller = (
        controller if test_boundary == -1 else controller[:test_boundary]
    )
    if "SelfSignedAcmeClient" in production_controller:
        failures.append("controller:production-self-signed-client")
    if "SelfSignedAcmeClient" in module:
        failures.append("gateway-module:self-signed-export")
    if "SelfSignedAcmeClient" in acme:
        failures.append("acme-harness:self-signed-client")
    if "Arc<dyn AcmeClient>" not in controller:
        failures.append("controller:missing-runtime-client-boundary")
    if "GatewayProviderBindingV1" not in controller:
        failures.append("controller:missing-config-provider-binding")
    controller_constructor = _body(
        _tokens(production_controller),
        "pub fn try_new",
    )
    if controller_constructor != _tokens(_CONTROLLER_CONSTRUCTOR):
        failures.append("controller:missing-startup-qualification")
    production_acme = acme.split("#[cfg(test)]", 1)[0]
    if _body(_tokens(production_acme), "pub fn try_new") != _tokens(_ACME_CONSTRUCTOR):
        failures.append("acme-harness:missing-startup-qualification")
    if "fn qualification(&self)" not in acme:
        failures.append("acme-harness:missing-provider-qualification")
    if "pub trait AcmeClient: Send + Sync" not in acme:
        failures.append("acme-harness:runtime-client-debug-exposure")
    if '.field("client", &"<runtime-only>")' not in acme:
        failures.append("acme-harness:runtime-client-state-not-redacted")
    if (
        acme.count("qualify_acme_client(&self.client_binding, &self.client)")
        < 2
    ):
        failures.append("acme-harness:missing-operation-qualification-fence")

    renew_start = xtask.find("pub fn gateway_tls_renew(")
    renew_end = xtask.find("\npub fn gateway_tls_revoke(", renew_start)
    if renew_start == -1 or renew_end == -1:
        failures.append("xtask:missing-renew-command-boundary")
    else:
        renewal = xtask[renew_start:renew_end]
        forbidden_renewal = (
            "SelfSignedAcmeClient",
            "AcmeAutomation::new",
            "certificate_pem",
            "private_key_pem",
            "write_file_with_mode",
            "fs::create_dir_all",
        )
        for marker in forbidden_renewal:
            if marker in renewal:
                failures.append(f"xtask:placeholder-renewal:{marker}")
        if "runtime-injected provider client" not in renewal:
            failures.append("xtask:renewal-does-not-fail-closed")

    gateway_builder = _body(_tokens(torii), "fn build_sorafs_gateway_security")
    acme_match = _body(
        gateway_builder,
        "let tls_automation = match (config.acme.enabled, acme_client)",
    )
    if not _contains_once(acme_match, _ENABLED_WITHOUT_CLIENT):
        failures.append("torii:missing-enabled-without-client-startup-failure")
    if not _contains_once(acme_match, _DISABLED_WITH_CLIENT):
        failures.append("torii:missing-disabled-with-client-startup-failure")
    if not _contains_once(gateway_builder, _PROVIDER_BINDING_FAILURE):
        failures.append("torii:missing-provider-binding-startup-failure")
    if not _contains_once(acme_match, _QUALIFIED_CLIENT_BRANCH):
        failures.append("torii:missing-exact-client-qualification")
    if 'include!("main/runtime_deps.rs");' not in irohad:
        failures.append("irohad:missing-runtime-deps-module")
    runtime_tokens = _tokens(runtime_deps.split("#[cfg(test)]", 1)[0])
    if _body(runtime_tokens, "macro_rules! define_runtime_dep_setters_v1") != _tokens(
        _RUNTIME_SETTER_EMITTER
    ):
        failures.append("irohad:runtime-dependency-setter-emitter-drift")
    setters = _body(
        _body(runtime_tokens, "impl IrohaRuntimeDeps"),
        "define_runtime_dep_setters_v1!",
    )
    for row, failure in _RUNTIME_SETTER_ROWS:
        if not _contains_once(setters, row):
            failures.append(failure)
    for marker, failure in (
        (
            "runtime_deps.with_sorafs_gateway_acme_client(client)",
            "irohad:missing-acme-forwarding",
        ),
        (
            "runtime_deps.with_sorafs_gateway_compliance_feed_transport(transport)",
            "irohad:missing-compliance-transport-forwarding",
        ),
    ):
        if marker not in irohad:
            failures.append(failure)

    canonical_relative = "specs/sorafs_gateway_tls_automation.md"
    canonical = _read(root_identity, canonical_relative, failures)
    if canonical is None:
        return failures
    if REQUIRED_RUNTIME_NOTICE not in canonical:
        failures.append(f"{canonical_relative}:missing-runtime-notice")
    for claim in FORBIDDEN_DOC_CLAIMS:
        if claim in canonical:
            failures.append(f"{canonical_relative}:stale-claim:{claim}")

    return failures


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check the fail-closed SoraFS gateway ACME runtime contract."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=SCRIPT_DIR.parent,
        help="repository root",
    )
    args = parser.parse_args()
    failures = check_contract(args.root)
    if failures:
        for failure in failures:
            print(f"[sorafs-gateway-tls] {failure}")
        return 1
    print("[sorafs-gateway-tls] runtime ACME contract is fail-closed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
