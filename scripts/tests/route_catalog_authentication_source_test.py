"""Source guards for the route-catalog authentication policy matrix."""

from __future__ import annotations

from pathlib import Path
import re
import unittest

from scripts.formal.rust_text import mask_rust_comments


REPO_ROOT = Path(__file__).resolve().parents[2]
CATALOG_TESTS = (
    REPO_ROOT / "crates/iroha_torii_shared/src/route_catalog/tests.rs"
)
AUTHENTICATION_TESTS = (
    REPO_ROOT
    / "crates/iroha_torii_shared/src/route_catalog/authentication_routes_test.rs"
)
INCLUDE = 'include!("authentication_routes_test.rs");'
REQUIRED_POLICY_CASES = ('application_query_posts_authenticate_before_expensive_compute', 'local_sorafs_governance_state_is_operator_signed', 'node_local_core_and_pipeline_reads_require_exact_operator_signatures', 'sorafs_inventory_and_storage_reads_declare_fail_closed_admission', 'soracloud_commands_require_exact_account_authentication_and_honest_effects', 'soracloud_sensitive_reads_require_exact_account_authentication', 'soracloud_public_reads_are_bounded_single_object_discovery', 'subscription_commands_require_exact_account_authentication_and_mutation_admission', 'application_drafts_and_cryptographic_services_require_exact_account_authentication', 'webhook_registry_is_operator_signed_and_effects_are_exact', 'zk_attachment_tenant_routes_are_account_authenticated_before_storage_access', 'zk_compute_routes_require_exact_account_authentication', 'account_and_node_bootstrap_capabilities_are_public', 'state_backed_runtime_and_governance_routes_require_exact_account_authentication', 'moderation_dead_letter_routes_are_account_signed_operator_role_posts')
DUPLICATE_TEST_NAMES = {
    "canonical_catalog_includes_host_gateway_and_directory_routes",
    "public_runtime_gateway_authentication_is_exactly_scoped",
    "dedicated_onboarding_authentication_is_exactly_scoped",
    "formerly_bearer_only_routes_require_exact_signatures",
    "iso20022_routes_require_fresh_operator_signatures",
    "vpn_and_push_device_routes_declare_canonical_account_authentication",
    "trusted_internal_account_reads_are_not_projected_to_public_tooling",
    "account_alias_visibility_and_signed_operator_routes_declare_exact_authentication",
}


def _macro_invocations(source: str) -> dict[str, str]:
    marker = "named_route_policy_test!"
    invocations: dict[str, str] = {}
    cursor = 0
    masked = mask_rust_comments(source)
    pairs = {"(": ")", "[": "]", "{": "}"}
    while (start := masked.find(marker, cursor)) >= 0:
        opening = source.find("(", start + len(marker))
        stack: list[str] = []
        quote: str | None = None
        escaped = False
        end = opening
        while end < len(source):
            character = source[end]
            if quote is not None:
                if escaped:
                    escaped = False
                elif character == "\\":
                    escaped = True
                elif character == quote:
                    quote = None
            elif character == '"':
                quote = character
            elif character in pairs:
                stack.append(pairs[character])
            elif stack and character == stack[-1]:
                stack.pop()
                if not stack:
                    end += 1
                    while end < len(source) and source[end] in " \t\r\n;":
                        end += 1
                    break
            end += 1
        else:
            raise AssertionError("unbalanced named_route_policy_test invocation")
        invocation = source[start:end]
        name_match = re.search(
            r"named_route_policy_test!\s*\(\s*([A-Za-z0-9_]+)", invocation
        )
        if name_match is None:
            raise AssertionError("route policy test invocation has no test name")
        name = name_match.group(1)
        if name in invocations:
            raise AssertionError(f"duplicate route policy test invocation: {name}")
        invocations[name] = invocation
        cursor = end
    return invocations


def _validate_policy_matrix(source: str) -> None:
    """Require named coverage and direct comparisons for every policy dimension."""
    invocations = _macro_invocations(source)
    if not set(REQUIRED_POLICY_CASES).issubset(invocations):
        raise AssertionError("required authentication policy case is missing")
    helper = source[:source.index("named_route_policy_test!(")]
    for field in ("stable_route_id", "method", "path", "surface", "effect", "admission",
                  "authentication", "projections", "path_normalization"):
        if f"assert_expected_route_value!(route, expected, {field});" not in helper:
            raise AssertionError("route policy comparison missing: " + field)
    for name in REQUIRED_POLICY_CASES:
        if "assert_route_polic" not in invocations[name]:
            raise AssertionError("policy case has no direct assertions: " + name)


class RouteCatalogAuthenticationSourceTest(unittest.TestCase):
    def test_authentication_matrix_is_included_exactly_once(self) -> None:
        catalog_tests = CATALOG_TESTS.read_text(encoding="utf-8")
        authentication_tests = AUTHENTICATION_TESTS.read_text(encoding="utf-8")
        self.assertEqual(catalog_tests.count(INCLUDE), 1)
        self.assertRegex(
            authentication_tests,
            r"(?s)macro_rules!\s+named_route_policy_test.*?#\[test\]\s*fn\s+\$name",
        )

    def test_named_cases_assert_every_authentication_policy_dimension(self) -> None:
        _validate_policy_matrix(AUTHENTICATION_TESTS.read_text(encoding="utf-8"))

    def test_policy_comparison_and_named_coverage_mutations_are_rejected(self) -> None:
        source = AUTHENTICATION_TESTS.read_text(encoding="utf-8")
        for old, new in (
            ("assert_expected_route_value!(route, expected, authentication);", ""),
            ("application_query_posts_authenticate_before_expensive_compute", "retired_case"),
            ("assert_expected_route_value!(route, expected, admission);", ""),
        ):
            with self.subTest(marker=old), self.assertRaises(AssertionError):
                _validate_policy_matrix(source.replace(old, new, 1))
        name = REQUIRED_POLICY_CASES[0]
        with self.assertRaisesRegex(AssertionError, "duplicate"):
            _validate_policy_matrix(source + "\nnamed_route_policy_test!(" + name + ", {});\n")

    def test_preexisting_duplicate_tests_remain_only_in_the_catalog_suite(self) -> None:
        catalog_tests = CATALOG_TESTS.read_text(encoding="utf-8")
        authentication_tests = AUTHENTICATION_TESTS.read_text(encoding="utf-8")
        for name in DUPLICATE_TEST_NAMES:
            declaration = re.compile(rf"\bfn\s+{re.escape(name)}\s*\(")
            self.assertEqual(len(declaration.findall(catalog_tests)), 1, name)
            self.assertEqual(len(declaration.findall(authentication_tests)), 0, name)


if __name__ == "__main__":
    unittest.main()
