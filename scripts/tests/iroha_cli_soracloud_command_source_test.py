#!/usr/bin/env python3
"""Protect the typed SoraCloud CLI command corridor and its test contracts."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = REPO_ROOT / "integration_tests/tests/iroha_cli.rs"
PREIMAGE_SHA256 = "aa1a2f2e6113915b33107d68d255f66194bd3813f853bd031125fe4459a57d43"
EXPECTED_SOURCE_LINES = 4_573

HELPER_START = "struct SoracloudCli<'a>"
HELPER_END = "async fn wait_for_soracloud_json_command"
HELPER_HASH = "834a379d73e93b4efbe1d75c5899cf26e6064608a0cdd502b46f8fb9316577a9"
ADVERTISE_HASH = "107212fbf26920091dee3580de1bbfb8bdf4e1b38bdcfbbcd7ddc8d1cfe84179"

# Hash, bounded-success calls, bounded raw calls, shared success assertions, live network.
FUNCTION_CONTRACTS = {
    "soracloud_status_uses_live_torii_control_plane": (
        "cff28b34307b6cb6be2b43e86630c3bc2294f84883ca9cec6723f90b5857c0ce",
        0,
        1,
        0,
        True,
    ),
    "soracloud_mutations_use_live_torii_control_plane": (
        "a0670560be55d527ba48625a71f626c7bdf972c71e9dfb5810c1a93f225cd431",
        0,
        0,
        4,
        True,
    ),
    "soracloud_scr_host_admission_rejects_invalid_manifests_live_torii_control_plane": (
        "f0bdf226ed77aedbfe4821e29f32d96bc64afb848de7c7b8e19e779da0416a9e",
        0,
        2,
        0,
        True,
    ),
    "soracloud_training_and_model_weight_lifecycle_use_live_torii_control_plane": (
        "7d69e1d478ed37d0e60877f5a54258a8ccada18b7689c78ace1015301a56af8e",
        17,
        0,
        0,
        True,
    ),
    "soracloud_hf_shared_lease_commands_use_live_torii_control_plane": (
        "f0413f835f3772a28238c546fe57a63c5cc18cae896884506522e328ba0e0449",
        0,
        0,
        8,
        True,
    ),
    "soracloud_hf_pre_expiry_renewal_queues_and_promotes_next_window": (
        "e4ae90ef3b1562f1c45444e7d1a1bc5efec1e8d71d035ecbfb18184eaecea765",
        0,
        0,
        4,
        True,
    ),
    "soracloud_hf_shared_lease_prorates_refunds_across_multiple_accounts": (
        "2de85b3b4a99b04aba23baec2eee7fb3e29864a42c388f9567e5ad41b2c86fb0",
        0,
        0,
        3,
        True,
    ),
    "soracloud_templates_deploy_site_and_webapp_with_rollout_and_rollback": (
        "211dec6de4c6001170ada71c21a7c6d57090f8700f4407098d67ba64f489fec6",
        8,
        0,
        0,
        True,
    ),
    "soracloud_agent_autonomy_runtime_uses_live_torii_control_plane": (
        "e7087d328fea5c9c7c2b6b8e2332c017354b5e2ed7c74711f0c218076ec389fc",
        5,
        1,
        0,
        True,
    ),
    "soracloud_agent_wallet_mailbox_and_lease_recovery_use_live_torii_control_plane": (
        "dece98092d6dfa9b1ffa1e0128537529b23ba2f910853d6d7ca53d0d0d1c5e80",
        11,
        1,
        0,
        True,
    ),
    "soracloud_agent_runtime_state_recovers_after_peer_restart_live_torii_control_plane": (
        "90a624fe4509e4c0347e58150c548fed393f5ad8b610fb3eeae5d0fe61b0b5dd",
        14,
        0,
        0,
        True,
    ),
    "soracloud_agent_autonomy_commands_require_torii_url": (
        "14b8f5fd34d5089c7b001fec32240be6544d9b5bf72852ba6e8cf9cdf2e817b6",
        0,
        0,
        0,
        False,
    ),
    "soracloud_agent_wallet_and_mailbox_commands_require_torii_url": (
        "e540179cdbd85d69b51d02136382235b5c01c449a4a7f6ee929b34ce0276eafe",
        0,
        0,
        0,
        False,
    ),
    "soracloud_agent_lease_commands_require_torii_url": (
        "1c6f334959aa2f0efe3eb1e369058c3fe35d44a4791b1b171e22a3bab4aa4edc",
        0,
        0,
        0,
        False,
    ),
    "soracloud_hf_shared_lease_commands_require_torii_url": (
        "0e7aaa9d29b2045fc156f877f2c766f9a70ca27884219490de371f6e671ed4c4",
        0,
        0,
        0,
        False,
    ),
}

REQUIRED_HELPER_TOKENS = (
    "cwd: &'a Path",
    "config: &'a ProgramConfig",
    'command.current_dir(self.cwd).arg("soracloud");',
    "command.envs(self.config.envs());",
    "Ok(command.bounded_output().await?)",
    "failure_context: &'static str",
    "output.status.success()",
    '"{context} failed with status {} and stderr: {}"',
    "$(command.arg($arg);)+",
    "$cli.bounded_output(command).await?",
    "assert_soracloud_success(&output, case.failure_context);",
)

REQUIRED_SECURITY_TOKENS = (
    "!over_cap_deploy.status.success()",
    "!no_write_deploy.status.success()",
    "!revoked_run.status.success()",
    "!expired_wallet.status.success()",
    'contains("resources.cpu_millis exceeds SCR cap")',
    'contains("binding `session_store` requires mutable writes (`ReadWrite`)")',
    'contains("agent.autonomy.run")',
    'contains("lease expired")',
)

FORBIDDEN_HELPER_TOKENS = (
    "Box<dyn Fn",
    "Box<dyn FnMut",
    "impl Fn",
    "callback:",
    "custom_body:",
    "escape_hatch",
)


class GuardError(AssertionError):
    """Raised when the protected SoraCloud CLI source contract changes."""


def _normalized_hash(source: str) -> str:
    return hashlib.sha256(re.sub(r"\s+", "", source).encode()).hexdigest()


def _skip_rust_non_code(source: str, index: int) -> int | None:
    if source.startswith("//", index):
        newline = source.find("\n", index + 2)
        return len(source) if newline < 0 else newline + 1
    if source.startswith("/*", index):
        depth = 1
        cursor = index + 2
        while cursor < len(source):
            if source.startswith("/*", cursor):
                depth += 1
                cursor += 2
            elif source.startswith("*/", cursor):
                depth -= 1
                cursor += 2
                if depth == 0:
                    return cursor
            else:
                cursor += 1
        return len(source)
    for prefix in ("br", "r"):
        if source.startswith(prefix, index):
            cursor = index + len(prefix)
            while cursor < len(source) and source[cursor] == "#":
                cursor += 1
            if cursor < len(source) and source[cursor] == '"':
                hashes = cursor - index - len(prefix)
                terminator = '"' + "#" * hashes
                end = source.find(terminator, cursor + 1)
                return len(source) if end < 0 else end + len(terminator)
    if source[index : index + 1] not in {'"', "'"}:
        return None
    quote = source[index]
    cursor = index + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
            continue
        if source[cursor] == quote:
            return cursor + 1
        cursor += 1
    return len(source)


def _matching_brace(source: str, open_index: int) -> int:
    stack = ["}"]
    cursor = open_index + 1
    pairs = {"(": ")", "[": "]", "{": "}"}
    while cursor < len(source):
        skipped = _skip_rust_non_code(source, cursor)
        if skipped is not None:
            cursor = skipped
            continue
        character = source[cursor]
        if character in pairs:
            stack.append(pairs[character])
        elif character in ")]}":
            if not stack or character != stack.pop():
                raise GuardError("mismatched Rust delimiter in protected function")
            if not stack:
                return cursor
        cursor += 1
    raise GuardError("unterminated Rust function body")


def _function(source: str, name: str) -> tuple[str, int]:
    pattern = re.compile(rf"(?m)^async fn {re.escape(name)}\b")
    matches = list(pattern.finditer(source))
    if len(matches) != 1:
        raise GuardError(f"{name}: expected one async function definition")
    start = matches[0].start()
    open_index = source.find("{", matches[0].end())
    if open_index < 0:
        raise GuardError(f"{name}: missing function body")
    end = _matching_brace(source, open_index)
    return source[start : end + 1], start


def _ordered_attributes(source: str, function_start: int) -> tuple[str, ...]:
    lines = source[:function_start].splitlines()
    attributes = []
    cursor = len(lines) - 1
    while cursor >= 0 and lines[cursor].strip().startswith("#["):
        attributes.append(lines[cursor].strip())
        cursor -= 1
    return tuple(reversed(attributes))


def _helper_region(source: str) -> str:
    if source.count(HELPER_START) != 1 or source.count(HELPER_END) != 1:
        raise GuardError("typed command helper markers must occur exactly once")
    start = source.index(HELPER_START)
    end = source.index(HELPER_END, start)
    return source[start:end]


def validate_source(source: str) -> None:
    if len(source.splitlines()) != EXPECTED_SOURCE_LINES:
        raise GuardError("iroha_cli.rs left the frozen SoraCloud source-line budget")
    helper = _helper_region(source)
    if _normalized_hash(helper) != HELPER_HASH:
        raise GuardError("typed SoraCloud command helper changed")
    for token in REQUIRED_HELPER_TOKENS:
        if token not in helper:
            raise GuardError(f"typed helper lost semantic token {token!r}")
    for token in FORBIDDEN_HELPER_TOKENS:
        if token in helper:
            raise GuardError(f"typed helper gained callback escape hatch {token!r}")

    protected_functions = []
    for name, (expected_hash, success_count, raw_count, assertion_count, live) in (
        FUNCTION_CONTRACTS.items()
    ):
        function, start = _function(source, name)
        protected_functions.append(function)
        if _ordered_attributes(source, start) != ("#[tokio::test]",):
            raise GuardError(f"{name}: ordered test attributes changed")
        if _normalized_hash(function) != expected_hash:
            raise GuardError(f"{name}: semantic source hash changed")
        observed = (
            function.count("run_bounded_soracloud_success!("),
            function.count("run_bounded_soracloud_command!("),
            function.count("assert_soracloud_success("),
        )
        if observed != (success_count, raw_count, assertion_count):
            raise GuardError(f"{name}: command corridor inventory changed: {observed}")
        if "tokio::process::Command::new(program())" in function:
            raise GuardError(f"{name}: direct bounded command skeleton returned")
        peer_count = function.count(".with_min_peers(4)")
        if peer_count != int(live):
            raise GuardError(f"{name}: four-peer live-network contract changed")
        context_pattern = re.compile(rf"stringify!\s*\(\s*{re.escape(name)}\s*\)")
        if live and len(context_pattern.findall(function)) != 1:
            raise GuardError(f"{name}: sandbox network context changed")

    advertise, _start = _function(source, "advertise_soracloud_model_host")
    if _normalized_hash(advertise) != ADVERTISE_HASH:
        raise GuardError("model-host advertising command contract changed")
    if advertise.count("assert_soracloud_success(") != 1:
        raise GuardError("model-host advertising success diagnostic changed")

    protected = helper + "".join(protected_functions)
    for token in REQUIRED_SECURITY_TOKENS:
        if token not in protected:
            raise GuardError(f"SoraCloud adversarial contract lost token {token!r}")


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation preimage must occur once: {old!r}")
    return source.replace(old, new, 1)


def _replace_in_function(source: str, name: str, old: str, new: str) -> str:
    function, _start = _function(source, name)
    if function.count(old) != 1:
        raise AssertionError(f"{name}: mutation preimage must occur once: {old!r}")
    return source.replace(function, function.replace(old, new, 1), 1)


class IrohaCliSoracloudCommandSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text()

    def assert_rejected(self, mutated: str) -> None:
        with self.assertRaises(GuardError):
            validate_source(mutated)

    def test_current_source_preserves_soracloud_command_contracts(self) -> None:
        validate_source(self.source)

    def test_preimage_identity_is_frozen(self) -> None:
        self.assertEqual(
            PREIMAGE_SHA256,
            "aa1a2f2e6113915b33107d68d255f66194bd3813f853bd031125fe4459a57d43",
        )

    def test_name_mutation_is_rejected(self) -> None:
        name = next(iter(FUNCTION_CONTRACTS))
        self.assert_rejected(_replace_once(self.source, f"async fn {name}", f"async fn {name}_x"))

    def test_ordered_attribute_mutation_is_rejected(self) -> None:
        name = next(iter(FUNCTION_CONTRACTS))
        old = f"#[tokio::test]\nasync fn {name}"
        self.assert_rejected(_replace_once(self.source, old, old.replace("tokio::test", "test")))

    def test_four_peer_mutation_is_rejected(self) -> None:
        name = "soracloud_status_uses_live_torii_control_plane"
        mutated = _replace_in_function(
            self.source,
            name,
            ".with_min_peers(4)",
            ".with_min_peers(3)",
        )
        self.assert_rejected(mutated)

    def test_argument_order_mutation_is_rejected(self) -> None:
        old = (
            'SoracloudSuccessCase::new("training-job-start #1");\n'
            '        "model", "training-job-start",'
        )
        new = old.replace('"model", "training-job-start"', '"training-job-start", "model"')
        self.assert_rejected(_replace_once(self.source, old, new))

    def test_success_diagnostic_mutation_is_rejected(self) -> None:
        old = 'SoracloudSuccessCase::new("training-job-start #1")'
        self.assert_rejected(_replace_once(self.source, old, old.replace("#1", "#2")))

    def test_expected_failure_polarity_mutation_is_rejected(self) -> None:
        old = "!over_cap_deploy.status.success()"
        self.assert_rejected(_replace_once(self.source, old, old.removeprefix("!")))

    def test_control_plane_diagnostic_mutation_is_rejected(self) -> None:
        old = 'assert_soracloud_success(&deploy, "hf-deploy");'
        self.assert_rejected(_replace_once(self.source, old, old.replace("hf-deploy", "hf-status")))

    def test_helper_argument_emission_mutation_is_rejected(self) -> None:
        old = "$(command.arg($arg);)+"
        self.assert_rejected(_replace_once(self.source, old, "$(command.args([$arg]);)+"))

    def test_callback_escape_hatch_is_rejected(self) -> None:
        old = "struct SoracloudSuccessCase {"
        mutated = _replace_once(
            self.source,
            old,
            "struct SoracloudSuccessCase {\n    callback: Box<dyn Fn()>,",
        )
        self.assert_rejected(mutated)

    def test_sandbox_context_mutation_is_rejected(self) -> None:
        name = "soracloud_status_uses_live_torii_control_plane"
        old = f"stringify!({name})"
        self.assert_rejected(_replace_once(self.source, old, '"wrong-network-context"'))

    def test_source_growth_is_rejected(self) -> None:
        self.assert_rejected(self.source + "// synthetic growth\n")


if __name__ == "__main__":
    unittest.main()
