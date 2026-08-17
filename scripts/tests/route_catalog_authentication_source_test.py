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
HELPER_SHA256 = "cb17ec818a1c49205d7d23d51dfcb37f080b4f4825f98fcc96aaa1f0b6174bd8"
CASE_SHA256 = {
    "offline_receiver_lineage_requires_account_authentication_before_expensive_proof_work":
        "0cc00c6f1d652db23d7eac505623335d4e6572631e948b80d86021bc17000ffc",
    "application_query_posts_authenticate_before_expensive_compute":
        "867e93be703ed16b3b6ca4c70052b5919c24babc81ee00534cfd16dd175db9d5",
    "local_sorafs_governance_state_is_operator_signed":
        "466e491e4c368c9129e23333bf877529c56ec036037156902bc6f67df6facd0b",
    "node_local_core_and_pipeline_reads_require_exact_operator_signatures":
        "1be9e913b3646104ef62b4b9bb29de11ddaf7ab407288baaa8250cdee7c6d899",
    "sorafs_inventory_and_storage_reads_declare_fail_closed_admission":
        "913e3aa407e745420e72b442f5174a9945ae4f29f366c3a15c39bd00301da1be",
    "soracloud_commands_require_exact_account_authentication_and_honest_effects":
        "677445e84e5a9ce7a21b30f7ab77ef4d276eff31ead03f3fb56c99a4f923e8f8",
    "soracloud_sensitive_reads_require_exact_account_authentication":
        "2758c933eb309f4df44b90e4abc5f8bbdaebaa26486a0f8ed8e2fef005eaf623",
    "soracloud_public_reads_are_bounded_single_object_discovery":
        "f6d2f5af26c19c319d1b9688aef72a7325fc91c86064b00908f0869253f12f9d",
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
        "aecc49a2ce79116b0f86c44f82897a5499ead9ca8415c6087c57ac318fe4d112",
    "moderation_dead_letter_routes_are_account_signed_operator_role_posts":
        "3e91e66ff3dba7038d5d728580090095ffb27d22c41c8b2a3f3a063d9c50398d",
}
DUPLICATE_TEST_NAMES = {
    "canonical_catalog_includes_exact_gateway_and_directory_routes",
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
