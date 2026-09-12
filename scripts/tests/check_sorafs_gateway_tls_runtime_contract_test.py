from __future__ import annotations

import importlib.util
import shutil

import pytest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts/check_sorafs_gateway_tls_runtime_contract.py"
SPEC = importlib.util.spec_from_file_location("gateway_tls_contract", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

CONTRACT_FIXTURE_PATHS = (
    "crates/iroha_torii/src/sorafs/gateway/controller.rs",
    "crates/iroha_torii/src/sorafs/gateway/mod.rs",
    "crates/iroha_torii/src/sorafs/gateway/acme.rs",
    "crates/iroha_torii/src/lib.rs",
    "crates/irohad/src/main.rs",
    "crates/irohad/src/main/runtime_deps.rs",
    "xtask/src/sorafs.rs",
    "specs/sorafs_gateway_tls_automation.md",
)


def copy_contract_fixture(root: Path) -> None:
    for relative in CONTRACT_FIXTURE_PATHS:
        source = REPO_ROOT / relative
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    # Every negative starts from a real, currently passing source snapshot.
    assert MODULE.check_contract(root) == []


def mutate_once(root: Path, relative: str, before: str, after: str) -> None:
    """Prove the one intended source edit is present and effective."""

    assert before != after
    path = root / relative
    original = path.read_text(encoding="utf-8")
    assert original.count(before) == 1, (relative, before, original.count(before))
    mutated = original.replace(before, after, 1)
    assert mutated != original
    path.write_text(mutated, encoding="utf-8")


def test_repository_runtime_acme_contract_is_fail_closed() -> None:
    assert MODULE.check_contract(REPO_ROOT) == []


def test_gateway_handbook_withdraws_instead_of_using_certificate_fallback() -> None:
    handbook = (
        REPO_ROOT / "specs/sorafs_gateway_deployment_handbook.md"
    ).read_text(encoding="utf-8")

    assert "sorafs-gateway tls renew" not in handbook
    assert "fall back to stored cert" not in handbook
    assert "Withdraw the affected gateway from admission and traffic" in handbook
    assert "audited runtime ACME adapter and controller boundary" in handbook


def test_guard_uses_shared_identity_and_bounded_read_contract() -> None:
    source = SCRIPT_PATH.read_text(encoding="utf-8")

    assert "from sorafs_evidence_json import read_evidence_bytes" in source
    assert "inspect_evidence_directory" in source
    assert "inspect_evidence_file" in source
    assert "resolve_evidence_path" in source
    assert "MAX_CONTRACT_SOURCE_BYTES" in source
    assert ".read_text(" not in source
    assert "args.root.resolve()" not in source


def test_guard_rejects_production_self_signed_export(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)

    module_path = root / "crates/iroha_torii/src/sorafs/gateway/mod.rs"
    module_path.write_text(
        module_path.read_text(encoding="utf-8")
        + "\npub use controller::SelfSignedAcmeClient;\n",
        encoding="utf-8",
    )
    assert "gateway-module:self-signed-export" in MODULE.check_contract(root)


@pytest.mark.parametrize(
    "relative,before,after,expected",
    (
        (
            "xtask/src/sorafs.rs",
            "pub fn gateway_tls_revoke(",
            "fn fake() { AcmeAutomation::new(); }\n\npub fn gateway_tls_revoke(",
            "xtask:placeholder-renewal:AcmeAutomation::new",
        ),
        (
            "specs/sorafs_gateway_tls_automation.md",
            "> **Runtime ACME boundary (V1):**",
            "> **Runtime ACME boundary (V1):**\n"
            "Production ACME clients remain available for validated accounts.",
            "specs/sorafs_gateway_tls_automation.md:stale-claim:"
            "Production ACME clients remain available for validated accounts",
        ),
    ),
)
def test_guard_rejects_fake_renewal_and_stale_docs(
    tmp_path: Path, relative: str, before: str, after: str, expected: str,
) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    mutate_once(root, relative, before, after)
    assert MODULE.check_contract(root) == [expected]


@pytest.mark.parametrize(
    "relative,before,after,expected",
    (
        (
            "crates/irohad/src/main.rs",
            "runtime_deps.with_sorafs_gateway_compliance_feed_transport(transport)",
            "runtime_deps",
            "irohad:missing-compliance-transport-forwarding",
        ),
        (
            "crates/irohad/src/main.rs",
            "runtime_deps.with_sorafs_gateway_acme_client(client)",
            "runtime_deps",
            "irohad:missing-acme-forwarding",
        ),
        (
            "crates/irohad/src/main.rs",
            'include!("main/runtime_deps.rs");',
            '// runtime dependency module omitted',
            "irohad:missing-runtime-deps-module",
        ),
        (
            "crates/irohad/src/main/runtime_deps.rs",
            "with_sorafs_gateway_acme_client(",
            "without_sorafs_gateway_acme_client(",
            "irohad:missing-runtime-acme-injection",
        ),
        (
            "crates/irohad/src/main/runtime_deps.rs",
            "with_sorafs_gateway_compliance_feed_transport(",
            "without_sorafs_gateway_compliance_feed_transport(",
            "irohad:missing-runtime-compliance-transport-injection",
        ),
    ),
)
def test_guard_rejects_missing_daemon_runtime_forwarding(
    tmp_path: Path, relative: str, before: str, after: str, expected: str,
) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    mutate_once(root, relative, before, after)
    assert MODULE.check_contract(root) == [expected]


@pytest.mark.parametrize(
    "before,after,expected",
    (
        (
            "qualify_acme_client(&self.client_binding, &self.client)?;",
            "Ok(())?;",
            "acme-harness:missing-operation-qualification-fence",
        ),
        (
            "if let Err(error) = qualify_acme_client(&self.client_binding, &self.client)",
            "if let Err(error) = Ok(())",
            "acme-harness:missing-operation-qualification-fence",
        ),
        (
            "pub trait AcmeClient: Send + Sync",
            "pub trait AcmeClient: std::fmt::Debug + Send + Sync",
            "acme-harness:runtime-client-debug-exposure",
        ),
        (
            '.field("client", &"<runtime-only>")',
            '.field("client", &self.client)',
            "acme-harness:runtime-client-state-not-redacted",
        ),
        (
            "qualify_acme_client(&client_binding, &client)?;",
            "let _ = qualify_acme_client(&client_binding, &client);",
            "acme-harness:missing-startup-qualification",
        ),
    ),
)
def test_guard_rejects_missing_acme_identity_fences(
    tmp_path: Path, before: str, after: str, expected: str,
) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    mutate_once(root, "crates/iroha_torii/src/sorafs/gateway/acme.rs", before, after)
    assert MODULE.check_contract(root) == [expected]


def test_guard_rejects_symlinked_contract_source(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    module_path = root / "crates/iroha_torii/src/sorafs/gateway/mod.rs"
    external = tmp_path / "external-module.rs"
    external.write_text(module_path.read_text(encoding="utf-8"), encoding="utf-8")
    module_path.unlink()
    module_path.symlink_to(external)

    assert (
        "crates/iroha_torii/src/sorafs/gateway/mod.rs:"
        "unsafe-or-unresolvable-source"
        in MODULE.check_contract(root)
    )


def test_guard_rejects_oversized_contract_source(
    tmp_path: Path,
    monkeypatch,
) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    monkeypatch.setattr(MODULE, "MAX_CONTRACT_SOURCE_BYTES", 32)

    assert (
        "crates/iroha_torii/src/sorafs/gateway/controller.rs:"
        "unreadable-or-oversized-source"
        in MODULE.check_contract(root)
    )


def test_guard_rejects_symlinked_repository_root(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    root_alias = tmp_path / "repo-alias"
    root_alias.symlink_to(root, target_is_directory=True)

    assert MODULE.check_contract(root_alias) == [
        "repository-root:unsafe-or-unresolvable"
    ]


@pytest.mark.parametrize(
    "replacement",
    (
        "AcmeAutomation::new(config, client_binding.clone(), Arc::clone(&client));",
        "AcmeAutomation::try_new(config, other_binding.clone(), Arc::clone(&client))?;",
        "AcmeAutomation::try_new(config, client_binding.clone(), Arc::clone(&other_client))?;",
        "AcmeAutomation::try_new(config, client_binding.clone(), Arc::clone(&client));",
    ),
)
def test_guard_requires_controller_qualified_constructor(
    tmp_path: Path, replacement: str,
) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    mutate_once(
        root, "crates/iroha_torii/src/sorafs/gateway/controller.rs",
        "AcmeAutomation::try_new(config, client_binding.clone(), Arc::clone(&client))?;",
        replacement,
    )
    assert MODULE.check_contract(root) == ["controller:missing-startup-qualification"]


@pytest.mark.parametrize(
    "before,after,expected",
    (
        (
            "if config.acme.enabled != config.acme.provider.is_some() {",
            "if config.acme.enabled == config.acme.provider.is_some() {",
            "torii:missing-provider-binding-startup-failure",
        ),
        (
            'ToriiBuildError::invalid_configuration(\n'
            '            "sorafs.gateway.acme.provider",\n'
            '            "provider binding must be present exactly when ACME is enabled",',
            'ToriiBuildError::invalid_runtime_dependency(\n'
            '            "sorafs.gateway.acme.provider",\n'
            '            "provider binding must be present exactly when ACME is enabled",',
            "torii:missing-provider-binding-startup-failure",
        ),
        (
            '"sorafs.gateway.acme.provider",\n'
            '            "provider binding must be present exactly when ACME is enabled",',
            '"wrong.component",\n'
            '            "provider binding must be present exactly when ACME is enabled",',
            "torii:missing-provider-binding-startup-failure",
        ),
        (
            '(true, None) => {\n'
            '            return Err(ToriiBuildError::invalid_runtime_dependency(\n'
            '                "sorafs.gateway.acme",',
            '(false, None) => {\n'
            '            return Err(ToriiBuildError::invalid_runtime_dependency(\n'
            '                "sorafs.gateway.acme",',
            "torii:missing-enabled-without-client-startup-failure",
        ),
        (
            'ToriiBuildError::invalid_runtime_dependency(\n'
            '                "sorafs.gateway.acme",\n'
            '                "ACME is enabled but no runtime client was supplied",',
            'ToriiBuildError::invalid_configuration(\n'
            '                "sorafs.gateway.acme",\n'
            '                "ACME is enabled but no runtime client was supplied",',
            "torii:missing-enabled-without-client-startup-failure",
        ),
        (
            '"sorafs.gateway.acme",\n'
            '                "ACME is enabled but no runtime client was supplied",',
            '"wrong.component",\n'
            '                "ACME is enabled but no runtime client was supplied",',
            "torii:missing-enabled-without-client-startup-failure",
        ),
        (
            '(false, Some(_)) => {\n'
            '            return Err(ToriiBuildError::invalid_runtime_dependency(\n'
            '                "sorafs.gateway.acme",',
            '(true, Some(_)) => {\n'
            '            return Err(ToriiBuildError::invalid_runtime_dependency(\n'
            '                "sorafs.gateway.acme",',
            "torii:missing-disabled-with-client-startup-failure",
        ),
        (
            'let binding = gateway_runtime_provider_binding(provider)?;',
            'let binding = gateway_runtime_provider_binding(other_provider)?;',
            "torii:missing-exact-client-qualification",
        ),
        (
            'TlsAutomationHandle::try_new(\n'
            '                    gateway_acme_config(&config.acme),',
            'TlsAutomationHandle::new(\n'
            '                    gateway_acme_config(&config.acme),',
            "torii:missing-exact-client-qualification",
        ),
    ),
)
def test_guard_requires_typed_torii_startup_branches(
    tmp_path: Path, before: str, after: str, expected: str,
) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    mutate_once(root, "crates/iroha_torii/src/lib.rs", before, after)
    assert MODULE.check_contract(root) == [expected]


@pytest.mark.parametrize(
    "before,after,expected",
    (
        (
            'client: Arc<dyn iroha_torii::sorafs::gateway::AcmeClient>,',
            'client: Arc<dyn WrongClient>,',
            "irohad:missing-runtime-acme-injection",
        ),
        (
            ') => sorafs_gateway_acme_client;',
            ') => wrong_field;',
            "irohad:missing-runtime-acme-injection",
        ),
        (
            'transport: Arc<dyn iroha_torii::sorafs::gateway::GatewayComplianceFeedTransport>,',
            'transport: Arc<dyn WrongTransport>,',
            "irohad:missing-runtime-compliance-transport-injection",
        ),
        (
            ') => sorafs_gateway_compliance_feed_transport;',
            ') => wrong_field;',
            "irohad:missing-runtime-compliance-transport-injection",
        ),
        (
            'self.$field = Some($argument);',
            'self.$field = None;',
            "irohad:runtime-dependency-setter-emitter-drift",
        ),
        (
            'pub fn $name(mut self, $argument: $dependency) -> Self {',
            'fn $name(mut self, $argument: $dependency) -> Self {',
            "irohad:runtime-dependency-setter-emitter-drift",
        ),
    ),
)
def test_guard_requires_typed_daemon_setter_owner(
    tmp_path: Path, before: str, after: str, expected: str,
) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    mutate_once(root, "crates/irohad/src/main/runtime_deps.rs", before, after)
    assert MODULE.check_contract(root) == [expected]


@pytest.mark.parametrize("decoy", ("comment", "string", "raw-string", "outside-owner"))
@pytest.mark.parametrize("row", ("acme", "compliance"))
def test_setter_rows_require_code_in_the_actual_invocation(
    tmp_path: Path, decoy: str, row: str,
) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    if row == "acme":
        marker = (
            'with_sorafs_gateway_acme_client(\n'
            '            client: Arc<dyn iroha_torii::sorafs::gateway::AcmeClient>,\n'
            '        ) => sorafs_gateway_acme_client;'
        )
        expected = "irohad:missing-runtime-acme-injection"
    else:
        marker = (
            'with_sorafs_gateway_compliance_feed_transport(\n'
            '            transport: Arc<dyn iroha_torii::sorafs::gateway::GatewayComplianceFeedTransport>,\n'
            '        ) => sorafs_gateway_compliance_feed_transport;'
        )
        expected = "irohad:missing-runtime-compliance-transport-injection"
    replacement = {
        "comment": "/* " + marker + " */",
        "string": '"' + marker + '"',
        "raw-string": 'r###"' + marker + '"###',
        # The row still exists as actual tokens, but outside the selected invocation.
        "outside-owner": "} " + marker + " define_unrelated_setters! {",
    }[decoy]
    mutate_once(root, "crates/irohad/src/main/runtime_deps.rs", marker, replacement)
    failures = MODULE.check_contract(root)
    assert expected in failures
    assert set(failures) <= {
        "irohad:missing-runtime-acme-injection",
        "irohad:missing-runtime-compliance-transport-injection",
    }


def test_guard_tokens_preserve_literal_identity_and_nested_comments() -> None:
    assert MODULE._tokens('left /* outer /* nested */ tail */ right // line\n end') == (
        "left", "right", "end",
    )
    assert MODULE._tokens('"fn fake() {}" r##"raw /* body */"##') == (
        '"fn fake() {}"', 'r##"raw /* body */"##',
    )
    assert MODULE._tokens(r"'{' '\\' 'a") == ("'{'", r"'\\'", "'", "a")
    assert MODULE._tokens('/* unterminated') == ()
    assert MODULE._tokens('r###"unterminated') == ()
    assert MODULE._body(MODULE._tokens('fn owner() { "}"; { inner(); } }'), 'fn owner') == (
        '"}"', ';', '{', 'inner', '(', ')', ';', '}',
    )
    assert MODULE._body(MODULE._tokens('fn owner() {} fn owner() {}'), 'fn owner') == ()
    assert MODULE._body(MODULE._tokens('fn owner() {'), 'fn owner') == ()
    assert MODULE._body(MODULE._tokens('fn other() {}'), 'fn owner') == ()


def test_current_structural_owners_accept_formatting_and_comments(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    mutate_once(
        root, "crates/iroha_torii/src/sorafs/gateway/controller.rs",
        "AcmeAutomation::try_new(config, client_binding.clone(), Arc::clone(&client))?;",
        "AcmeAutomation /* retained */ :: try_new(\n"
        " config, client_binding.clone(), Arc::clone(&client)) ? ;",
    )
    assert MODULE.check_contract(root) == []
