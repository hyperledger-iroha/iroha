"""Independent-oracle and site-inventory tests for zk-X509 presentation intervals."""

import copy
import json
import subprocess
from pathlib import Path
from runpy import run_path

import pytest


ORACLE = run_path(
    str(Path(__file__).resolve().parents[1] / "check_zk_x509_presentation_interval.py")
)
T = ORACLE["T"]
WIDE = ORACLE["WIDE"]
FRESH_CRL = ORACLE["FRESH_CRL"]


def test_tracked_fixture_is_the_current_rendering() -> None:
    assert ORACLE["FIXTURE"].read_text() == ORACLE["render"]()
    assert ORACLE["main"]([]) == 0


def test_every_window_agrees_between_bounds_and_per_interval_formulations() -> None:
    document = json.loads(ORACLE["FIXTURE"].read_text())
    assert document["schema"] == ORACLE["SCHEMA"]
    windows = 0
    for case in document["cases"]:
        certificates = [
            (row["not_before"], row["not_after"]) for row in case["certificates"]
        ]
        crl = (case["crl"]["this_update"], case["crl"]["next_update"])
        assert ("bounds" in case) != ("bounds_error" in case), case["name"]
        for row in case["windows"]:
            admitted = row["result"] == ORACLE["ADMITTED"]
            assert admitted == ORACLE["per_interval_admits"](
                certificates, crl, row["not_before"], row["not_after"]
            ), (case["name"], row)
            if admitted:
                assert case["bounds"]["earliest_start"] <= row["not_before"]
                assert row["not_after"] <= case["bounds"]["latest_end"]
            windows += 1
        for row in case["blocks"]:
            assert row["admitted"] == (
                row["not_before"] * 1000
                <= row["timestamp_ms"]
                < (row["not_after"] + 1) * 1000
            )
        if "bounds" in case:
            # The two keys are admissible window endpoints, not certificate
            # fields: the earliest start is the latest signed lower bound and
            # the latest end is the earliest signed upper bound.
            assert set(case["bounds"]) == {"earliest_start", "latest_end"}
            assert case["bounds"]["earliest_start"] == max(
                [not_before for not_before, _ in certificates] + [crl[0]]
            )
            assert case["bounds"]["latest_end"] == min(
                [not_after for _, not_after in certificates]
                + [crl[1] - 1, crl[0] + 300, ORACLE["MAX_UNIX_SECONDS"]]
            )
    assert windows >= 70


@pytest.mark.parametrize("position", [0, 1, 2])
def test_earliest_expiry_at_each_path_position_bounds_the_window(position: int) -> None:
    earliest = T + 150
    certificates = [(WIDE[0], T + 86_400)] * 3
    certificates[position] = (WIDE[0], earliest)
    assert ORACLE["bounds"](certificates, FRESH_CRL) == (T, earliest)
    assert ORACLE["window_result"](certificates, FRESH_CRL, T, earliest) == "admitted"
    assert (
        ORACLE["window_result"](certificates, FRESH_CRL, T, earliest + 1)
        == "ends-after-bounds"
    )
    # The retired latest-expiry formula budgets TU + 300; the path is expired by then.
    assert not ORACLE["per_interval_admits"](certificates, FRESH_CRL, T, T + 300)
    assert (
        ORACLE["deadline_unix_ms"](certificates, FRESH_CRL, T + 300)
        == (earliest + 1) * 1000 - 1
    )


def test_crl_next_update_is_exclusive_and_age_is_capped() -> None:
    certificates = [WIDE, WIDE]
    assert ORACLE["window_result"](certificates, (T, T + 200), T, T + 199) == "admitted"
    assert (
        ORACLE["window_result"](certificates, (T, T + 200), T, T + 200)
        == "ends-after-bounds"
    )
    assert ORACLE["bounds"](certificates, (T, T + 3_600)) == (T, T + 300)
    assert (
        ORACLE["window_result"](certificates, (T, T + 3_600), T + 1, T + 301)
        == "ends-after-bounds"
    )
    assert ORACLE["deadline_unix_ms"](certificates, (T, T + 200), T + 250) == (
        T + 200
    ) * 1000 - 1


def test_inconsistent_formulations_are_refused(monkeypatch: pytest.MonkeyPatch) -> None:
    # A bounds function that takes the latest expiry cannot produce vectors.
    def latest_expiry_bounds(certificates, crl):
        lower = max(not_before for not_before, _ in certificates)
        upper = max(not_after for _, not_after in certificates)
        this_update, next_update = crl
        return max(lower, this_update), min(upper, next_update - 1, this_update + 300)

    build = ORACLE["build"]
    monkeypatch.setitem(build.__globals__, "bounds", latest_expiry_bounds)
    with pytest.raises(ORACLE["VectorError"]):
        build()


def test_calendar_ceiling_bounds_the_window_end() -> None:
    last = ORACLE["MAX_UNIX_SECONDS"]
    certificates = [(WIDE[0], last), (WIDE[0], last)]
    # A governed record may stay fresh past the calendar; the window may not.
    crl = (last - 100, last + 50)
    assert ORACLE["bounds"](certificates, crl) == (last - 100, last)
    assert ORACLE["window_result"](certificates, crl, last - 100, last) == "admitted"
    assert (
        ORACLE["window_result"](certificates, crl, last - 100, last + 1)
        == "ends-after-bounds"
    )
    assert not ORACLE["per_interval_admits"](certificates, crl, last - 100, last + 1)
    assert ORACLE["deadline_unix_ms"](certificates, crl, last + 1) == (last + 1) * 1000 - 1
    # A signable CRL ends at the last calendar second, which stays exclusive.
    assert ORACLE["bounds"](certificates, (last - 200, last)) == (last - 200, last - 1)
    names = [case["name"] for case in json.loads(ORACLE["FIXTURE"].read_text())["cases"]]
    assert "calendar-ceiling-crl-next-update" in names
    assert "calendar-ceiling-window-end" in names


def _inventory() -> dict:
    return json.loads(ORACLE["SITES"].read_text())


def test_site_inventory_matches_the_source_tree() -> None:
    inventory = _inventory()
    found = ORACLE["scan_sites"]()
    assert ORACLE["site_problems"](found, inventory) == []
    roles = inventory["sites"]
    assert roles[inventory["definition"]] == ["definition"]
    assert roles[inventory["vectors"]] == ["record"]
    # Statement validation, state admission and the native relation are canonical.
    for path in (
        "crates/iroha_data_model/src/privacy/statements.rs",
        "crates/iroha_core_privacy/src/privacy_state.rs",
        "crates/iroha_core_privacy/src/privacy_engines/zk_x509/relation.rs",
    ):
        assert roles[path] == ["canonical"]
    # The verifier's public shape calls the definition inside the relation code.
    assert roles[
        "crates/iroha_core_privacy/src/privacy_engines/zk_x509/rfc5280_stark.rs"
    ] == ["canonical", "in-relation"]
    # The hand-written CRL registration predicate and both hand-written
    # deadlines are found by the scan and pinned, not classified canonical.
    for path in (
        "crates/iroha_core/src/smartcontracts/isi/privacy.rs",
        "crates/iroha_core/src/privacy_release_evidence/zk_x509.rs",
        "integration_tests/tests/privacy_exact12_zk_x509_network.rs",
    ):
        assert "deferred" in roles[path] and "canonical" not in roles[path]
        assert path in found
        for entry in inventory["deferred"][path]:
            assert entry["owner"] == "X.6" and entry["todo"].startswith("TODO(X.6)")
    assert set(inventory["deferred"]) == {
        path for path, chosen in roles.items() if "deferred" in chosen
    }
    assert set(inventory["roles"]) == set(ORACLE["SITE_ROLES"])
    # The DER-to-bounds helper and prover preflight are recorded as gated.
    relation = "crates/iroha_core_privacy/src/privacy_engines/zk_x509/relation.rs"
    assert 'feature = "privacy-release-evidence"' in inventory["build_gates"][relation][
        "declaration"
    ][0]
    assert inventory["build_gates"][relation]["declaration"][-1] == "pub mod relation;"


def test_site_scan_keeps_paths_with_whitespace_whole(monkeypatch: pytest.MonkeyPatch) -> None:
    def fake_run(command, **_):
        assert "-z" in command and "-l" in command and "-F" in command
        return subprocess.CompletedProcess(
            command,
            0,
            stdout="sdk/Zk X509 Builder.swift\0crates/b.rs\0docs/unrelated notes.md\0",
            stderr="",
        )

    texts = {
        "sdk/Zk X509 Builder.swift": "let end = statement.presentationNotAfterUnixSeconds",
        "crates/b.rs": "if block < record.next_update_unix_seconds {}",
        # A prefilter substring alone is not a site.
        "docs/unrelated notes.md": "ZK_X509_MAX_CHAIN_DEPTH_V1",
    }
    candidates = ORACLE["scan_candidates"]
    monkeypatch.setattr(candidates.__globals__["subprocess"], "run", fake_run)
    assert candidates() == sorted(texts)
    assert ORACLE["scan_sites"](read=texts.__getitem__) == [
        "crates/b.rs",
        "sdk/Zk X509 Builder.swift",
    ]


def test_every_pattern_alternative_is_reached_by_the_prefilter() -> None:
    import re

    spellings = [
        "presentation_not_before_unix_seconds",
        "presentation_not_after_unix_seconds",
        "presentationNotBefore",
        "PresentationNotAfterUnixSeconds",
        "statement.presentation_window()",
        "PrivacyZkX509PresentationBoundsV1",
        "PrivacyZkX509PresentationWindowV1",
        "PrivacyZkX509PresentationIntervalErrorV1",
        "derive_zk_x509_presentation_bounds_v1",
        "validate_zk_x509_presentation_interval_v1",
        "this_update_unix_seconds",
        "next_update_unix_seconds",
        "thisUpdateUnixSeconds",
        "ThisUpdateUnixSeconds",
        "nextUpdateUnixSeconds",
        "NextUpdateUnixSeconds",
        "ZK_X509_MAX_CRL_AGE_SECONDS_V1",
        "ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1",
    ]
    for spelling in spellings:
        assert re.search(ORACLE["SITE_PATTERN"], spelling), spelling
        assert any(literal in spelling for literal in ORACLE["SITE_PREFILTER"]), spelling
    for unrelated in ("ZK_X509_MAX_CHAIN_DEPTH_V1", "crl_number", "not_before"):
        assert not re.search(ORACLE["SITE_PATTERN"], unrelated)


def test_site_inventory_rejects_unclassified_stale_and_unknown_entries() -> None:
    inventory = _inventory()
    found = sorted(inventory["sites"])
    problems = ORACLE["site_problems"]
    assert problems(found, inventory) == []
    # A new SDK statement builder must be classified before it can land.
    builder = "IrohaSwift/Sources/IrohaSwift/ZkX509PresentationBuilder.swift"
    assert problems(sorted([*found, builder]), inventory) == [
        f"unclassified presentation-interval site: {builder}"
    ]
    removed = "crates/iroha_core_privacy/src/privacy_engines/zk_x509/verifier_profile.rs"
    assert problems([path for path in found if path != removed], inventory) == [
        f"stale site inventory entry: {removed}"
    ]
    changed = copy.deepcopy(inventory)
    changed["sites"][removed] = ["trusted"]
    assert f"unknown role 'trusted' for {removed}" in problems(found, changed)
    changed = copy.deepcopy(inventory)
    changed["sites"][removed] = "transport"
    assert (
        f"roles of {removed} must be a non-empty sorted list without duplicates"
        in problems(found, changed)
    )
    changed = copy.deepcopy(inventory)
    changed["sites"][removed] = ["definition"]
    reported = problems(found, changed)
    assert "exactly one site must be the canonical definition" in reported
    assert f"{removed}: `definition` is exclusive to {inventory['definition']}" in reported
    for key in ("pattern", "canonical_api"):
        changed = copy.deepcopy(inventory)
        changed[key] = "window"
        assert any("pattern mismatch" in problem for problem in problems(found, changed))
    changed = copy.deepcopy(inventory)
    del changed["roles"]["deferred"]
    assert "site inventory role descriptions do not match the role set" in problems(
        found, changed
    )


def test_roles_are_checked_against_the_source() -> None:
    inventory = _inventory()
    found = sorted(inventory["sites"])
    problems = ORACLE["site_problems"]
    engine = "crates/iroha_core_privacy/src/privacy_engines/zk_x509/"
    # A file that calls the canonical definition cannot be filed as an
    # independent formulation or as transport only.
    for roles in (["in-relation"], ["transport"]):
        changed = copy.deepcopy(inventory)
        changed["sites"][engine + "rfc5280_stark.rs"] = roles
        assert (
            f"{engine}rfc5280_stark.rs: calls the canonical definition but is not `canonical`"
            in problems(found, changed)
        )
    # A file that does not call it cannot claim to.
    changed = copy.deepcopy(inventory)
    changed["sites"][engine + "der_air.rs"] = ["canonical"]
    assert (
        f"{engine}der_air.rs: `canonical` site does not call the canonical definition"
        in problems(found, changed)
    )
    # Comment lines are not calls.
    role_problems = ORACLE["role_problems"]
    definition = inventory["definition"]
    commented = "// statement.presentation_window().validate()\nlet x = 1;\n"
    assert role_problems("crates/a/src/lib.rs", ["transport"], commented, definition) == []
    called = "let window = statement.presentation_window();\n"
    assert role_problems("crates/a/src/lib.rs", ["transport"], called, definition) == [
        "crates/a/src/lib.rs: calls the canonical definition but is not `canonical`"
    ]
    assert role_problems("crates/a/src/lib.rs", ["canonical"], called, definition) == []
    # A dereferencing assignment is code; block-comment continuations and
    # script comments are not.
    dereferenced = "    *validity =\n        PrivacyZkX509CertificateValidityV1::new(a, b);\n"
    assert role_problems("crates/a/src/lib.rs", ["canonical"], dereferenced, definition) == []
    is_comment_line = ORACLE["is_comment_line"]
    assert not is_comment_line("a.rs", "    *validity = statement.presentation_window();")
    assert is_comment_line("a.rs", "     * statement.presentation_window()")
    assert is_comment_line("a.rs", "    /// statement.presentation_window()")
    assert not is_comment_line("a.rs", "    #[cfg(test)]")
    assert is_comment_line("a.py", "    # statement.presentation_window()")
    # Location rules.
    assert role_problems("crates/a/src/lib.rs", ["in-relation"], "", definition) == [
        f"crates/a/src/lib.rs: `in-relation` site is outside {engine}"
    ]
    assert role_problems("crates/a/src/lib.rs", ["record"], "", definition) != []
    assert role_problems("specs/a.md", ["record"], called, definition) == []
    assert role_problems("crates/a/tests/t.rs", ["transport"], "", definition) == [
        "crates/a/tests/t.rs: a test file must carry `test`"
    ]
    assert role_problems("crates/a/tests/t.rs", ["deferred", "test"], called, definition) == []
    assert role_problems("crates/a/src/x.rs", ["canonical", "test"], called, definition) == [
        "crates/a/src/x.rs: `test` combines only with `deferred`"
    ]
    assert role_problems("crates/a/src/x.rs", ["deferred", "transport"], "", definition) == [
        "crates/a/src/x.rs: `transport` is exclusive"
    ]
    assert ORACLE["is_test_path"]("crates/a/src/b_tests.rs")
    assert ORACLE["is_test_path"]("integration_tests/tests/x.rs")
    assert not ORACLE["is_test_path"]("crates/a/src/attestation.rs")


def test_deferred_sites_are_pinned_to_their_reviewed_source() -> None:
    inventory = _inventory()
    found = sorted(inventory["sites"])
    problems = ORACLE["site_problems"]
    read = ORACLE["read_site"]
    isi = "crates/iroha_core/src/smartcontracts/isi/privacy.rs"
    test_path = inventory["deferred_test"]["path"]

    def reading(path_to_edit: str, old: str, new: str):
        def read_edited(path: str) -> str:
            text = read(path)
            if path == path_to_edit:
                assert old in text
                return text.replace(old, new)
            return text

        return read_edited

    # Loosening the hand-written predicate (nextUpdate inclusive) is reported.
    reported = problems(
        found,
        inventory,
        read=reading(
            isi,
            "|| block_unix_seconds >= record.next_update_unix_seconds",
            "|| block_unix_seconds > record.next_update_unix_seconds",
        ),
    )
    assert any(
        "`validate_zk_x509_crl_freshness_v1` no longer matches its pinned source" in problem
        for problem in reported
    )
    assert any("fragment is not in the site" in problem for problem in reported)
    # So is a boundary-grid test that stops transcribing the same predicate.
    reported = problems(
        found,
        inventory,
        read=reading(
            test_path,
            "|| block_unix_seconds >= record.next_update_unix_seconds",
            "|| block_unix_seconds > record.next_update_unix_seconds",
        ),
    )
    assert any("fragment is not in the boundary-grid test" in problem for problem in reported)
    reported = problems(
        found,
        inventory,
        read=reading(
            test_path,
            f"fn {inventory['deferred_test']['name']}()",
            "fn renamed()",
        ),
    )
    assert any("boundary-grid test" in problem and "is missing" in problem for problem in reported)
    # Reformatting the pinned lines is not drift.
    assert (
        problems(
            found,
            inventory,
            read=reading(
                isi,
                "    let block_unix_seconds = block_timestamp_ms / 1_000;\n",
                "    let block_unix_seconds =\n        block_timestamp_ms / 1_000;\n",
            ),
        )
        == []
    )
    # A deferred role and its pinned entries must agree, in both directions.
    changed = copy.deepcopy(inventory)
    del changed["deferred"][isi]
    assert f"{isi}: `deferred` role and pinned entries must agree" in problems(found, changed)
    changed = copy.deepcopy(inventory)
    changed["sites"][isi] = ["transport"]
    assert f"{isi}: `deferred` role and pinned entries must agree" in problems(found, changed)
    changed = copy.deepcopy(inventory)
    changed["deferred"][isi][0]["todo"] = "later"
    assert any("carries no TODO" in problem for problem in problems(found, changed))
    changed = copy.deepcopy(inventory)
    del changed["deferred"][isi][0]["owner"]
    assert any("must have exactly the keys" in problem for problem in problems(found, changed))


def test_build_gates_are_checked_against_the_declaring_source() -> None:
    inventory = _inventory()
    found = sorted(inventory["sites"])
    problems = ORACLE["site_problems"]
    read = ORACLE["read_site"]
    module = "crates/iroha_core_privacy/src/privacy_engines/zk_x509/mod.rs"
    relation = "crates/iroha_core_privacy/src/privacy_engines/zk_x509/relation.rs"

    def ungated(path: str) -> str:
        text = read(path)
        if path == module:
            gated = (
                '#[cfg(any(test, feature = "privacy-release-evidence"))]\n'
                "#[doc(hidden)]\npub mod relation;"
            )
            assert gated in text
            return text.replace(gated, "#[doc(hidden)]\npub mod relation;")
        return text

    # Shipping the relation module (X.5) must update the inventory and the spec.
    assert problems(found, inventory, read=ungated) == [
        f"{relation}: build gate changed in {module}; update the inventory "
        "and specs/zk_x509_presentation_interval.md"
    ]
    changed = copy.deepcopy(inventory)
    changed["build_gates"]["crates/unknown.rs"] = changed["build_gates"][relation]
    assert "build gate for unclassified site: crates/unknown.rs" in problems(found, changed)


def test_pull_request_workflow_and_make_guards_run_the_check_and_these_tests() -> None:
    root = Path(__file__).resolve().parents[2]
    workflow = (root / ".github/workflows/pr.yml").read_text(encoding="utf-8")
    assert workflow.count("scripts/tests/check_zk_x509_presentation_interval_test.py") == 1
    assert (
        workflow.count("python3 -I -S scripts/check_zk_x509_presentation_interval.py\n") == 1
    )
    makefile = (root / "Makefile").read_text(encoding="utf-8")
    guards = makefile.split("\nguards:\n", 1)[1].split("\n\n", 1)[0]
    assert "\t@python3 scripts/check_zk_x509_presentation_interval.py" in guards.split("\n")
