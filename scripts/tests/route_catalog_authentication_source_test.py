"""Source guards for the route-catalog authentication policy matrix."""

from __future__ import annotations

from hashlib import sha256
from pathlib import Path
import re
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
CATALOG_TESTS = (
    REPO_ROOT / "crates/iroha_torii_shared/src/route_catalog/tests.rs"
)
AUTHENTICATION_TESTS = (
    REPO_ROOT
    / "crates/iroha_torii_shared/src/route_catalog/authentication_routes_test.rs"
)
INCLUDE = 'include!("authentication_routes_test.rs");'
HELPER_SHA256 = "007253c6083a5d1583837f9d29685bd3be27bdaf5bdb51f1465ba03b6595da75"
CASE_SHA256 = {
    "application_query_posts_authenticate_before_expensive_compute":
        "14c59414e87abd483e0d0af11e9b7c6f960e922aced66fcdb47d3c50f2e0564b",
    "local_sorafs_governance_state_is_operator_signed":
        "466e491e4c368c9129e23333bf877529c56ec036037156902bc6f67df6facd0b",
    "node_local_core_and_pipeline_reads_require_exact_operator_signatures":
        "1be9e913b3646104ef62b4b9bb29de11ddaf7ab407288baaa8250cdee7c6d899",
    "sorafs_inventory_and_storage_reads_declare_fail_closed_admission":
        "fbf41bff1530fb54baa7196b8a867223541129659d1ec7b0fc2ed41c0aad773b",
    "soracloud_commands_require_exact_account_authentication_and_honest_effects":
        "91ee004a2a315253c8ed2dfcfa937d8b98e871a3f77a1621205871b006d9b8b3",
    "soracloud_sensitive_reads_require_exact_account_authentication":
        "c1fbf275eb9fe58794452febcea15edd0cda222f718679850f936493714e3642",
    "soracloud_public_reads_are_bounded_single_object_discovery":
        "6eeedb2f9693a564400a8cd4a908b5b1c498ac81c1af13eacf77c435225856d1",
    "subscription_commands_require_exact_account_authentication_and_mutation_admission":
        "667a3e3d8a356717efbd062c596dd7e12f6ac4333fc02abbf09864877b0cbf76",
    "application_drafts_and_cryptographic_services_require_exact_account_authentication":
        "1b94673e28fcd8a0220fe4f4aff455f70fc3e700938eac60e5cd8317381398b5",
    "webhook_registry_is_operator_signed_and_effects_are_exact":
        "3d80d2674f8f39f260c25d712ead2900de450c0c141fad3fe2f45fd906aba4a9",
    "zk_attachment_tenant_routes_are_account_authenticated_before_storage_access":
        "1250b5a5efe35d2462d6712c568bc84396ae21d7f9bd4ef1d8b9efd721216320",
    "zk_compute_routes_require_exact_account_authentication":
        "df31661cceec27a27c968294c1cb1c10a7472cbbbea04a3e50d64b0cb87c27fc",
    "state_backed_runtime_and_governance_routes_require_exact_account_authentication":
        "cf6f62b98ae1923914f4869080e1405a9d0587c193943f6c9f0270b7b4e659ee",
    "moderation_dead_letter_routes_are_account_signed_operator_role_posts":
        "f176bc2aca244a2eacb89c106d920a393e0124f8b438ce6fbb53b7db0d98216c",
}
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


def _normalized_sha256(source: str) -> str:
    return sha256(" ".join(source.split()).encode()).hexdigest()


def _macro_invocations(source: str) -> dict[str, str]:
    marker = "named_route_policy_test!"
    invocations: dict[str, str] = {}
    cursor = 0
    pairs = {"(": ")", "[": "]", "{": "}"}
    while (start := source.find(marker, cursor)) >= 0:
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


class RouteCatalogAuthenticationSourceTest(unittest.TestCase):
    def test_authentication_matrix_is_included_exactly_once(self) -> None:
        catalog_tests = CATALOG_TESTS.read_text(encoding="utf-8")
        authentication_tests = AUTHENTICATION_TESTS.read_text(encoding="utf-8")
        self.assertEqual(catalog_tests.count(INCLUDE), 1)
        self.assertRegex(
            authentication_tests,
            r"(?s)macro_rules!\s+named_route_policy_test.*?#\[test\]\s*fn\s+\$name",
        )

    def test_named_case_inventory_and_policy_tokens_are_frozen(self) -> None:
        source = AUTHENTICATION_TESTS.read_text(encoding="utf-8")
        invocations = _macro_invocations(source)
        self.assertEqual(set(invocations), set(CASE_SHA256))
        self.assertEqual(
            {
                name: _normalized_sha256(invocation)
                for name, invocation in invocations.items()
            },
            CASE_SHA256,
        )
        helper = source[: source.index("named_route_policy_test!")]
        self.assertEqual(_normalized_sha256(helper), HELPER_SHA256)

    def test_preexisting_duplicate_tests_remain_only_in_the_catalog_suite(self) -> None:
        catalog_tests = CATALOG_TESTS.read_text(encoding="utf-8")
        authentication_tests = AUTHENTICATION_TESTS.read_text(encoding="utf-8")
        for name in DUPLICATE_TEST_NAMES:
            declaration = re.compile(rf"\bfn\s+{re.escape(name)}\s*\(")
            self.assertEqual(len(declaration.findall(catalog_tests)), 1, name)
            self.assertEqual(len(declaration.findall(authentication_tests)), 0, name)


if __name__ == "__main__":
    unittest.main()
