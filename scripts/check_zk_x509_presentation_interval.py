#!/usr/bin/env python3
"""Check the zk-X509 presentation-interval vectors and site inventory.

The canonical definition is Rust:
`crates/iroha_data_model/src/privacy/zk_x509_interval.rs`.

Vectors. This script is an independent oracle. It derives every expected
admission from the per-interval predicates (each certificate and the CRL
checked on its own) and separately from the earliest/latest bounds, and
refuses to emit a vector on which the two formulations disagree. Rust tests
then check the canonical definition, the native reference relation and the
in-relation numeric rows against the tracked file; SDK statement builders
consume the same file.

Sites. `specs/zk_x509_presentation_interval_sites.json` classifies every
source file that uses one of the identifier spellings in `SITE_PATTERN`: the
public window fields, the governed CRL update fields, the two interval
ceilings and the canonical Rust API. A file that starts using one (for example
a new SDK statement builder) fails this check until it is classified.

This is a name tripwire, not a proof of absence: code that computes a window
under other names is not found here. The shared vectors and each builder's
conformance tests are the control for that.

Roles are reviewed classifications. The check enforces what is mechanical:
every scanned file is classified and no entry is stale; a `canonical` site
calls the canonical API and a non-test site that calls it is `canonical`; an
`in-relation` site lives in the zk-X509 engine; a `record` lives under
`specs/`, `fixtures/` or `scripts/`; a test file carries `test`; `transport`
and `definition` are exclusive; every `deferred` site is pinned to the exact
hand-written source lines it was reviewed against, so an edit there fails
until the site is re-reviewed or migrated; and every recorded build gate is
still declared in the source.

Prerequisites: Python 3.10+ standard library and `git` on `PATH`, run from a
checkout. No environment variables are read.

Usage:
    python3 scripts/check_zk_x509_presentation_interval.py           # check both
    python3 scripts/check_zk_x509_presentation_interval.py --write   # regenerate vectors
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "fixtures/zk/x509/interval_vectors_v1.json"
SCHEMA = "iroha.zk-x509.presentation-interval-vectors.v1"
SITES = ROOT / "specs/zk_x509_presentation_interval_sites.json"
SITES_SCHEMA = "iroha.zk-x509.presentation-interval-sites.v1"
# Snake, camel and Pascal spellings of the two public statement fields and the
# two governed CRL update fields, the two interval ceilings, and the canonical
# Rust interval API.
SITE_PATTERN = (
    "presentation_not_(before|after)|[pP]resentationNot(Before|After)"
    "|presentation_window\\(|ZkX509Presentation(Bounds|Window|Interval)"
    "|zk_x509_presentation_(bounds|interval)_v1"
    "|(this|next)_update_unix_seconds|([tT]his|[nN]ext)UpdateUnixSeconds"
    "|ZK_X509_MAX_(CRL_AGE|PRESENTATION_WINDOW)"
)
# Fixed substrings, one inside every alternative of `SITE_PATTERN`. The scan
# asks git for files containing any of them and then applies the pattern; a
# single extended-regex scan of the whole checkout is an order of magnitude
# slower.
SITE_PREFILTER = (
    "presentation_not_",
    "resentationNot",
    "presentation_window(",
    "ZkX509Presentation",
    "zk_x509_presentation_",
    "_update_unix_seconds",
    "UpdateUnixSeconds",
    "ZK_X509_MAX_",
)
# A non-comment line matching this calls the canonical definition.
CANONICAL_API_PATTERN = (
    "PrivacyZkX509(Presentation(Bounds|Window|IntervalError)|CrlUpdateInterval"
    "|CertificateValidity)V1|presentation_window\\(\\)|\\.update_interval\\(\\)"
    "|zk_x509_presentation_(bounds|interval)_v1"
)
SITE_ROLES = (
    "definition",
    "canonical",
    "in-relation",
    "deferred",
    "transport",
    "test",
    "record",
)
IN_RELATION_PREFIX = "crates/iroha_core_privacy/src/privacy_engines/zk_x509/"
RECORD_PREFIXES = ("specs/", "fixtures/", "scripts/")
HASH_COMMENT_SUFFIXES = (".py", ".sh", ".toml", ".yml", ".yaml")
DEFERRED_KEYS = ("name", "owner", "equals", "todo", "source", "transcribed")
DEFERRED_TEST_KEYS = ("path", "name")
BUILD_GATE_KEYS = ("declared_in", "declaration", "note")

MAX_CRL_AGE_SECONDS = 300
MAX_PRESENTATION_WINDOW_SECONDS = 300
# 9999-12-31T23:59:59Z, the last RFC 5280 GeneralizedTime second.
MAX_UNIX_SECONDS = 253_402_300_799
MIN_CHAIN_DEPTH = 2
MAX_CHAIN_DEPTH = 3

# 2023-01-01T00:00:00Z: UTCTime range, shared by the native fixtures.
T = 1_672_531_200
# 2022-01-01T00:00:00Z and 2030-01-01T00:00:00Z.
NB_WIDE = 1_640_995_200
NA_WIDE = 1_893_456_000
# 2050-01-01T00:00:00Z: first GeneralizedTime second under RFC 5280.
T50 = 2_524_608_000

ADMITTED = "admitted"


class VectorError(ValueError):
    """A declared vector is inconsistent between the two formulations."""


def bounds(certificates, crl):
    """Earliest/latest formulation. Returns `(lower, upper)` or an error code."""
    if not MIN_CHAIN_DEPTH <= len(certificates) <= MAX_CHAIN_DEPTH:
        return "invalid-chain-depth"
    if any(not_after < not_before for not_before, not_after in certificates):
        return "invalid-certificate-validity"
    lower = max(not_before for not_before, _ in certificates)
    upper = min(MAX_UNIX_SECONDS, *(not_after for _, not_after in certificates))
    if lower > upper:
        return "disjoint-intervals"
    this_update, next_update = crl
    if next_update <= this_update:
        return "invalid-crl-update-interval"
    crl_upper = min(next_update - 1, this_update + MAX_CRL_AGE_SECONDS, MAX_UNIX_SECONDS)
    lower = max(lower, this_update)
    upper = min(upper, crl_upper)
    if lower > upper:
        return "disjoint-intervals"
    return lower, upper


def window_result(certificates, crl, start, end):
    """Canonical result code of one window under the earliest/latest bounds."""
    derived = bounds(certificates, crl)
    if isinstance(derived, str):
        return derived
    lower, upper = derived
    if end <= start or end - start > MAX_PRESENTATION_WINDOW_SECONDS:
        return "invalid-window"
    if start < lower:
        return "starts-before-bounds"
    if end > upper:
        return "ends-after-bounds"
    return ADMITTED


def per_interval_admits(certificates, crl, start, end):
    """Independent formulation: every signed interval is checked on its own."""
    this_update, next_update = crl
    return (
        MIN_CHAIN_DEPTH <= len(certificates) <= MAX_CHAIN_DEPTH
        and start < end
        and end - start <= MAX_PRESENTATION_WINDOW_SECONDS
        and all(
            not_before <= not_after and not_before <= start and end <= not_after
            for not_before, not_after in certificates
        )
        and this_update < next_update
        and this_update <= start
        and end < next_update
        and end - this_update <= MAX_CRL_AGE_SECONDS
        and end <= MAX_UNIX_SECONDS
    )


def deadline_unix_ms(certificates, crl, presentation_not_after):
    """`min(TU + 301, NU, Cmin + 1, PA + 1) * 1000 - 1`, Cmin the earliest expiry."""
    this_update, next_update = crl
    earliest_expiry = min(not_after for _, not_after in certificates)
    return (
        min(
            this_update + MAX_CRL_AGE_SECONDS + 1,
            next_update,
            earliest_expiry + 1,
            presentation_not_after + 1,
            MAX_UNIX_SECONDS + 1,
        )
        * 1000
        - 1
    )


def block_admitted(start, end, timestamp_ms):
    """Block admission truncates the millisecond timestamp to its second."""
    return start <= timestamp_ms // 1000 <= end


def _case(name, note, certificates, crl, windows, *, signable=True, deadlines=(), blocks=()):
    return {
        "name": name,
        "note": note,
        "certificates": certificates,
        "crl": crl,
        "windows": windows,
        "signable": signable,
        "deadlines": list(deadlines),
        "blocks": list(blocks),
    }


WIDE = (NB_WIDE, NA_WIDE)
FRESH_CRL = (T, T + 301)

DECLARED_CASES = [
    _case(
        "crl-age-bound",
        "A long-lived CRL is fresh for exactly 300 seconds after thisUpdate.",
        [WIDE, WIDE],
        (T, T + 3_600),
        [
            (T, T + 300),
            (T, T + 301),
            (T + 1, T + 301),
            (T - 1, T + 200),
            (T, T + 1),
            (T + 10, T + 10),
            (T + 10, T + 9),
        ],
        deadlines=[T + 300, T + 1],
        blocks=[
            (T, T + 300, T * 1000 - 1),
            (T, T + 300, T * 1000),
            (T, T + 300, (T + 300) * 1000 + 999),
            (T, T + 300, (T + 301) * 1000),
        ],
    ),
    _case(
        "crl-next-update-exclusive",
        "nextUpdate is exclusive: a window ending at nextUpdate is rejected.",
        [WIDE, WIDE],
        (T, T + 200),
        [
            (T, T + 199),
            (T, T + 200),
            (T + 100, T + 199),
            (T + 199, T + 200),
            (T + 198, T + 199),
        ],
        deadlines=[T + 199, T + 200, T + 250],
        blocks=[
            (T, T + 199, (T + 199) * 1000 + 999),
            (T, T + 199, (T + 200) * 1000),
        ],
    ),
    _case(
        "earliest-expiry-at-leaf",
        "The leaf expires first.",
        [(NB_WIDE, T + 120), (NB_WIDE, T + 86_400), WIDE],
        FRESH_CRL,
        [
            (T, T + 120),
            (T, T + 121),
            (T + 119, T + 120),
            (T + 120, T + 121),
            (T, T + 300),
        ],
        deadlines=[T + 120, T + 300],
        blocks=[
            (T, T + 120, (T + 120) * 1000 + 999),
            (T, T + 120, (T + 121) * 1000),
        ],
    ),
    _case(
        "earliest-expiry-at-intermediate",
        "The intermediate CA expires before the leaf; the latest-expiry formula "
        "would admit the last window.",
        [(NB_WIDE, T + 86_400), (NB_WIDE, T + 150), WIDE],
        FRESH_CRL,
        [
            (T, T + 150),
            (T, T + 151),
            (T + 149, T + 150),
            (T + 150, T + 151),
            (T, T + 300),
        ],
        deadlines=[T + 150, T + 300],
        blocks=[
            (T, T + 150, (T + 150) * 1000 + 999),
            (T, T + 150, (T + 151) * 1000),
        ],
    ),
    _case(
        "earliest-expiry-at-root",
        "The root CA expires before the intermediate and the leaf.",
        [(NB_WIDE, T + 86_400), (NB_WIDE, T + 43_200), (NB_WIDE, T + 90)],
        FRESH_CRL,
        [
            (T, T + 90),
            (T, T + 91),
            (T + 89, T + 90),
            (T + 90, T + 91),
            (T, T + 300),
        ],
        deadlines=[T + 90, T + 300],
        blocks=[
            (T, T + 90, (T + 90) * 1000 + 999),
            (T, T + 90, (T + 91) * 1000),
        ],
    ),
    _case(
        "earliest-expiry-at-root-depth-two",
        "Two-certificate path whose root expires before the leaf.",
        [(NB_WIDE, T + 86_400), (NB_WIDE, T + 45)],
        FRESH_CRL,
        [(T, T + 45), (T, T + 46), (T + 44, T + 45), (T, T + 300)],
        deadlines=[T + 45, T + 300],
        blocks=[
            (T, T + 45, (T + 45) * 1000 + 999),
            (T, T + 45, (T + 46) * 1000),
        ],
    ),
    _case(
        "latest-not-before-at-leaf",
        "The leaf becomes valid last.",
        [(T + 30, NA_WIDE), WIDE, WIDE],
        FRESH_CRL,
        [(T + 30, T + 300), (T + 29, T + 300), (T + 30, T + 31), (T + 29, T + 30)],
        deadlines=[T + 300],
        blocks=[
            (T + 30, T + 300, (T + 30) * 1000 - 1),
            (T + 30, T + 300, (T + 30) * 1000),
        ],
    ),
    _case(
        "latest-not-before-at-intermediate",
        "The intermediate CA becomes valid last.",
        [WIDE, (T + 40, NA_WIDE), WIDE],
        FRESH_CRL,
        [(T + 40, T + 300), (T + 39, T + 300), (T + 40, T + 41)],
        deadlines=[T + 300],
    ),
    _case(
        "latest-not-before-at-root",
        "The root CA becomes valid last.",
        [WIDE, WIDE, (T + 50, NA_WIDE)],
        FRESH_CRL,
        [(T + 50, T + 300), (T + 49, T + 300), (T + 50, T + 51)],
        deadlines=[T + 300],
    ),
    _case(
        "certificates-bind-both-sides",
        "Different certificates bind the start and the end.",
        [(T + 10, T + 100), (T + 20, T + 95), (T + 5, T + 90)],
        FRESH_CRL,
        [
            (T + 20, T + 90),
            (T + 19, T + 90),
            (T + 20, T + 91),
            (T + 10, T + 90),
            (T + 20, T + 95),
            (T + 20, T + 100),
        ],
        deadlines=[T + 90, T + 100],
    ),
    _case(
        "single-shared-second",
        "The signed intervals share one second, which no two-second window fits.",
        [(T + 100, NA_WIDE), (NB_WIDE, T + 100)],
        FRESH_CRL,
        [(T + 100, T + 101), (T + 99, T + 100), (T + 100, T + 100)],
        deadlines=[T + 100],
    ),
    _case(
        "disjoint-certificates",
        "The leaf becomes valid after the intermediate CA has expired.",
        [(T + 200, T + 400), (NB_WIDE, T + 100), WIDE],
        FRESH_CRL,
        [(T + 200, T + 250), (T + 50, T + 100), (T + 100, T + 200)],
    ),
    _case(
        "certificate-expired-before-crl",
        "An expired credential: the leaf expired before the CRL thisUpdate.",
        [(NB_WIDE, T - 1), WIDE],
        FRESH_CRL,
        [(T, T + 1), (T - 2, T - 1), (T - 1, T)],
    ),
    _case(
        "certificate-not-yet-valid-within-crl",
        "The root CA becomes valid only after the CRL freshness horizon.",
        [WIDE, (T + 301, NA_WIDE)],
        FRESH_CRL,
        [(T, T + 300), (T + 301, T + 302)],
    ),
    _case(
        "generalized-time-boundary",
        "Signed times straddle the UTCTime/GeneralizedTime switch at 2050.",
        [(T50 - 86_400, T50 + 100), (NB_WIDE, T50 + 31_536_000)],
        (T50 - 100, T50 + 150),
        [
            (T50 - 100, T50 + 100),
            (T50 - 100, T50 + 101),
            (T50 - 101, T50 + 100),
            (T50 - 1, T50),
            (T50, T50 + 1),
        ],
        deadlines=[T50 + 100, T50 + 149],
    ),
    _case(
        "calendar-ceiling-crl-next-update",
        "Certificates and the CRL nextUpdate carry the last RFC 5280 second "
        "(99991231235959Z); nextUpdate stays exclusive at the calendar ceiling.",
        [(NB_WIDE, MAX_UNIX_SECONDS), (NB_WIDE, MAX_UNIX_SECONDS)],
        (MAX_UNIX_SECONDS - 200, MAX_UNIX_SECONDS),
        [
            (MAX_UNIX_SECONDS - 200, MAX_UNIX_SECONDS - 1),
            (MAX_UNIX_SECONDS - 200, MAX_UNIX_SECONDS),
            (MAX_UNIX_SECONDS - 2, MAX_UNIX_SECONDS - 1),
            (MAX_UNIX_SECONDS - 201, MAX_UNIX_SECONDS - 1),
        ],
        deadlines=[MAX_UNIX_SECONDS - 1, MAX_UNIX_SECONDS],
        blocks=[
            (
                MAX_UNIX_SECONDS - 200,
                MAX_UNIX_SECONDS - 1,
                (MAX_UNIX_SECONDS - 1) * 1000 + 999,
            ),
            (MAX_UNIX_SECONDS - 200, MAX_UNIX_SECONDS - 1, MAX_UNIX_SECONDS * 1000),
        ],
    ),
    _case(
        "calendar-ceiling-window-end",
        "A governed CRL record whose nextUpdate lies beyond the RFC 5280 "
        "calendar cannot be DER-signed; the window still ends at the last "
        "calendar second at the latest.",
        [(NB_WIDE, MAX_UNIX_SECONDS), (NB_WIDE, MAX_UNIX_SECONDS)],
        (MAX_UNIX_SECONDS - 100, MAX_UNIX_SECONDS + 50),
        [
            (MAX_UNIX_SECONDS - 100, MAX_UNIX_SECONDS),
            (MAX_UNIX_SECONDS - 100, MAX_UNIX_SECONDS + 1),
            (MAX_UNIX_SECONDS - 1, MAX_UNIX_SECONDS),
            (MAX_UNIX_SECONDS, MAX_UNIX_SECONDS + 1),
        ],
        signable=False,
        deadlines=[MAX_UNIX_SECONDS, MAX_UNIX_SECONDS + 1],
        blocks=[
            (
                MAX_UNIX_SECONDS - 100,
                MAX_UNIX_SECONDS,
                MAX_UNIX_SECONDS * 1000 + 999,
            ),
            (MAX_UNIX_SECONDS - 100, MAX_UNIX_SECONDS, (MAX_UNIX_SECONDS + 1) * 1000),
        ],
    ),
    _case(
        "invalid-chain-depth-one",
        "A lone certificate is not a path.",
        [WIDE],
        FRESH_CRL,
        [(T, T + 1)],
        signable=False,
    ),
    _case(
        "invalid-chain-depth-four",
        "Four certificates exceed the closed profile.",
        [WIDE, WIDE, WIDE, WIDE],
        FRESH_CRL,
        [(T, T + 1)],
        signable=False,
    ),
    _case(
        "invalid-certificate-validity",
        "A certificate whose notAfter precedes its notBefore.",
        [WIDE, (T + 10, T + 9)],
        FRESH_CRL,
        [(T, T + 1)],
        signable=False,
    ),
    _case(
        "invalid-crl-update-interval",
        "A CRL whose nextUpdate equals its thisUpdate.",
        [WIDE, WIDE],
        (T, T),
        [(T, T + 1)],
        signable=False,
    ),
]

REQUIRED_CASES = (
    "earliest-expiry-at-leaf",
    "earliest-expiry-at-intermediate",
    "earliest-expiry-at-root",
    "crl-next-update-exclusive",
    "crl-age-bound",
    "calendar-ceiling-crl-next-update",
    "calendar-ceiling-window-end",
)


def build() -> dict:
    """Evaluate every declared case under both formulations."""
    cases = []
    for declared in DECLARED_CASES:
        certificates = declared["certificates"]
        crl = declared["crl"]
        derived = bounds(certificates, crl)
        case = {
            "name": declared["name"],
            "note": declared["note"],
            "signable": declared["signable"],
            "certificates": [
                {"not_before": not_before, "not_after": not_after}
                for not_before, not_after in certificates
            ],
            "crl": {"this_update": crl[0], "next_update": crl[1]},
        }
        if isinstance(derived, str):
            case["bounds_error"] = derived
        else:
            # Inclusive admissible window endpoints: the earliest start is the
            # latest signed lower bound and the latest end is the earliest
            # signed upper bound.
            case["bounds"] = {
                "earliest_start": derived[0],
                "latest_end": derived[1],
            }
        windows = []
        for start, end in declared["windows"]:
            result = window_result(certificates, crl, start, end)
            if (result == ADMITTED) != per_interval_admits(certificates, crl, start, end):
                raise VectorError(
                    f"{declared['name']}: [{start}, {end}] differs between formulations"
                )
            windows.append({"not_before": start, "not_after": end, "result": result})
        case["windows"] = windows
        if declared["deadlines"] and isinstance(derived, str):
            raise VectorError(f"{declared['name']}: deadline without bounds")
        deadlines = []
        for presentation_not_after in declared["deadlines"]:
            deadline = deadline_unix_ms(certificates, crl, presentation_not_after)
            if deadline != (min(derived[1], presentation_not_after) + 1) * 1000 - 1:
                raise VectorError(f"{declared['name']}: deadline formulations differ")
            deadlines.append(
                {
                    "presentation_not_after": presentation_not_after,
                    "deadline_unix_ms": deadline,
                }
            )
        case["deadlines"] = deadlines
        case["blocks"] = [
            {
                "not_before": start,
                "not_after": end,
                "timestamp_ms": timestamp_ms,
                "admitted": block_admitted(start, end, timestamp_ms),
            }
            for start, end, timestamp_ms in declared["blocks"]
        ]
        cases.append(case)
    names = [case["name"] for case in cases]
    if len(set(names)) != len(names):
        raise VectorError("duplicate case name")
    missing = [name for name in REQUIRED_CASES if name not in names]
    if missing:
        raise VectorError(f"missing required cases: {missing}")
    return {
        "schema": SCHEMA,
        "definition": "crates/iroha_data_model/src/privacy/zk_x509_interval.rs",
        "max_crl_age_seconds": MAX_CRL_AGE_SECONDS,
        "max_presentation_window_seconds": MAX_PRESENTATION_WINDOW_SECONDS,
        "max_unix_seconds": MAX_UNIX_SECONDS,
        "min_chain_depth": MIN_CHAIN_DEPTH,
        "max_chain_depth": MAX_CHAIN_DEPTH,
        "keys": {
            "bounds.earliest_start": "Earliest admissible window start, inclusive: "
            "the latest of every certificate notBefore and the CRL thisUpdate.",
            "bounds.latest_end": "Latest admissible window end, inclusive: the "
            "earliest of every certificate notAfter, the CRL nextUpdate - 1, "
            "thisUpdate + max_crl_age_seconds and max_unix_seconds.",
            "signable": "Whether the signed intervals can be DER-encoded as a path "
            "and CRL of the closed profile; other cases exercise the definition only.",
            "windows.result": "`admitted` or the stable rejection code of the "
            "canonical definition.",
            "deadlines.deadline_unix_ms": "Last admissible block timestamp for a "
            "presentation ending at presentation_not_after.",
        },
        "cases": cases,
    }


def render() -> str:
    """Canonical tracked rendering."""
    return json.dumps(build(), indent=2) + "\n"


def scan_candidates() -> list[str]:
    """Every tracked or unignored text file containing a prefilter substring."""
    command = ["git", "grep", "-l", "-z", "-I", "--untracked", "-F"]
    for literal in SITE_PREFILTER:
        command += ["-e", literal]
    result = subprocess.run(
        [*command, "--", "."],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode not in (0, 1):
        raise VectorError(f"git grep failed: {result.stderr.strip()}")
    # NUL-separated, so a path containing whitespace stays one entry.
    return sorted(path for path in result.stdout.split("\0") if path)


def read_site(path: str) -> str:
    """Text of one scanned file."""
    return (ROOT / path).read_text(errors="replace")


def scan_sites(read=read_site) -> list[str]:
    """Every tracked or unignored file that matches `SITE_PATTERN`."""
    return [path for path in scan_candidates() if re.search(SITE_PATTERN, read(path))]


def is_test_path(path: str) -> bool:
    """Whether a file is test-only by its location or name."""
    name = path.rsplit("/", 1)[-1]
    return (
        "/tests/" in path
        or path.startswith("tests/")
        or name == "tests.rs"
        or name.endswith(("_tests.rs", "_test.rs", "_test.py", "Tests.swift", "Test.kt"))
    )


def is_comment_line(path: str, line: str) -> bool:
    """Whether a line is a whole-line comment in the file's language."""
    stripped = line.strip()
    if path.endswith(HASH_COMMENT_SUFFIXES):
        return stripped.startswith("#")
    # C-family: line comments, and block-comment openers and continuations.
    # A dereference such as `*value = ...` is code.
    return stripped.startswith(("//", "/*")) or stripped == "*" or stripped.startswith(("* ", "*/"))


def code_lines(path: str, text: str) -> list[str]:
    """Lines that are not whole-line comments."""
    return [line for line in text.splitlines() if not is_comment_line(path, line)]


def collapse(text: str) -> str:
    """Whitespace-insensitive form used to compare pinned source lines."""
    return "".join(text.split())


def role_problems(path: str, roles: list[str], text: str, definition: str) -> list[str]:
    """Mechanical evidence that a reviewed classification still fits its file."""
    problems = []
    chosen = set(roles)
    calls_definition = any(
        re.search(CANONICAL_API_PATTERN, line) for line in code_lines(path, text)
    )
    if "definition" in chosen and (path != definition or chosen != {"definition"}):
        problems.append(f"{path}: `definition` is exclusive to {definition}")
    if "canonical" in chosen and not calls_definition:
        problems.append(f"{path}: `canonical` site does not call the canonical definition")
    if calls_definition and not chosen & {"definition", "canonical", "test", "record"}:
        problems.append(f"{path}: calls the canonical definition but is not `canonical`")
    if "in-relation" in chosen and not path.startswith(IN_RELATION_PREFIX):
        problems.append(f"{path}: `in-relation` site is outside {IN_RELATION_PREFIX}")
    if "record" in chosen and (chosen != {"record"} or not path.startswith(RECORD_PREFIXES)):
        problems.append(f"{path}: `record` is exclusive and lives under {RECORD_PREFIXES}")
    if "transport" in chosen and chosen != {"transport"}:
        problems.append(f"{path}: `transport` is exclusive")
    if is_test_path(path) and "test" not in chosen and "record" not in chosen:
        problems.append(f"{path}: a test file must carry `test`")
    if "test" in chosen and not chosen <= {"test", "deferred"}:
        problems.append(f"{path}: `test` combines only with `deferred`")
    return problems


def deferred_problems(path: str, entries, text: str, test_text: str) -> list[str]:
    """A deferred site must still contain the exact lines it was reviewed against.

    `source` pins the reviewed lines of the site. Each `transcribed` fragment
    must appear both in the site and in the boundary-grid test that compares
    the hand-written formula with the canonical definition, so the test cannot
    drift from the source it stands for.
    """
    if not isinstance(entries, list) or not entries:
        return [f"{path}: deferred site has no pinned entry"]
    problems = []
    source = collapse(text)
    test_source = collapse(test_text)
    for entry in entries:
        if not isinstance(entry, dict) or tuple(entry) != DEFERRED_KEYS:
            problems.append(f"{path}: deferred entry must have exactly the keys {DEFERRED_KEYS}")
            continue
        label = f"{path}: deferred `{entry['name']}`"
        if not all(isinstance(entry[key], str) and entry[key] for key in DEFERRED_KEYS[:4]):
            problems.append(f"{label} has an empty field")
        if "TODO" not in entry["todo"]:
            problems.append(f"{label} carries no TODO")
        pinned = entry["source"]
        if not isinstance(pinned, list) or not pinned:
            problems.append(f"{label} pins no source lines")
        elif collapse("".join(pinned)) not in source:
            problems.append(
                f"{label} no longer matches its pinned source; re-review the site and "
                "either migrate it to the canonical definition or update the pin"
            )
        transcribed = entry["transcribed"]
        if not isinstance(transcribed, list) or not transcribed:
            problems.append(f"{label} names no transcribed fragment")
            continue
        for fragment in transcribed:
            if collapse(fragment) not in source:
                problems.append(f"{label} fragment is not in the site: {fragment}")
            if collapse(fragment) not in test_source:
                problems.append(f"{label} fragment is not in the boundary-grid test: {fragment}")
    return problems


def build_gate_problems(path: str, gate, read) -> list[str]:
    """A recorded build gate must still be declared exactly as recorded."""
    if not isinstance(gate, dict) or tuple(gate) != BUILD_GATE_KEYS:
        return [f"{path}: build gate must have exactly the keys {BUILD_GATE_KEYS}"]
    declaration = gate["declaration"]
    if not isinstance(declaration, list) or not declaration:
        return [f"{path}: build gate records no declaration"]
    try:
        declared = collapse(read(gate["declared_in"]))
    except OSError:
        return [f"{path}: build gate declaring file {gate['declared_in']} is unreadable"]
    if collapse("".join(declaration)) not in declared:
        return [
            f"{path}: build gate changed in {gate['declared_in']}; update the inventory "
            "and specs/zk_x509_presentation_interval.md"
        ]
    return []


def site_problems(found: list[str], inventory: dict, read=read_site) -> list[str]:
    """Differences between the scanned sources and the tracked inventory."""
    problems = []
    if inventory.get("schema") != SITES_SCHEMA:
        problems.append("site inventory schema mismatch")
    if inventory.get("pattern") != SITE_PATTERN:
        problems.append("site inventory pattern mismatch")
    if inventory.get("canonical_api") != CANONICAL_API_PATTERN:
        problems.append("site inventory canonical-API pattern mismatch")
    if set(inventory.get("roles", {})) != set(SITE_ROLES):
        problems.append("site inventory role descriptions do not match the role set")
    definition = inventory.get("definition")
    sites = inventory.get("sites", {})
    deferred = inventory.get("deferred", {})
    gates = inventory.get("build_gates", {})
    deferred_test = inventory.get("deferred_test")
    test_text = ""
    if not isinstance(deferred_test, dict) or tuple(deferred_test) != DEFERRED_TEST_KEYS:
        problems.append(f"deferred_test must have exactly the keys {DEFERRED_TEST_KEYS}")
    else:
        try:
            test_text = read(deferred_test["path"])
        except OSError:
            problems.append(f"deferred boundary-grid test file {deferred_test['path']} is unreadable")
        if f"fn {deferred_test['name']}()" not in test_text:
            problems.append(
                f"boundary-grid test `{deferred_test['name']}` is missing from "
                f"{deferred_test['path']}"
            )
    for path in found:
        if path not in sites:
            problems.append(f"unclassified presentation-interval site: {path}")
    definitions = 0
    for path, roles in sites.items():
        if (
            not isinstance(roles, list)
            or not roles
            or len(set(roles)) != len(roles)
            or roles != sorted(roles)
        ):
            problems.append(f"roles of {path} must be a non-empty sorted list without duplicates")
            continue
        unknown = [role for role in roles if role not in SITE_ROLES]
        if unknown:
            problems.append(f"unknown role {unknown[0]!r} for {path}")
            continue
        definitions += roles.count("definition")
        if path not in found:
            problems.append(f"stale site inventory entry: {path}")
            continue
        text = read(path)
        problems.extend(role_problems(path, roles, text, definition))
        if ("deferred" in roles) != (path in deferred):
            problems.append(f"{path}: `deferred` role and pinned entries must agree")
        elif path in deferred:
            problems.extend(deferred_problems(path, deferred[path], text, test_text))
    for path in deferred:
        if path not in sites:
            problems.append(f"deferred entry for unclassified site: {path}")
    for path, gate in gates.items():
        if path not in sites:
            problems.append(f"build gate for unclassified site: {path}")
        else:
            problems.extend(build_gate_problems(path, gate, read))
    if definitions != 1:
        problems.append("exactly one site must be the canonical definition")
    if list(sites) != sorted(sites):
        problems.append("site inventory is not sorted")
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--write", action="store_true", help="regenerate the tracked vectors")
    arguments = parser.parse_args(argv)
    rendered = render()
    if arguments.write:
        FIXTURE.parent.mkdir(parents=True, exist_ok=True)
        FIXTURE.write_text(rendered)
        return 0
    status = 0
    if not FIXTURE.exists() or FIXTURE.read_text() != rendered:
        print(
            f"{FIXTURE.relative_to(ROOT)} is stale; run "
            "python3 scripts/check_zk_x509_presentation_interval.py --write",
            file=sys.stderr,
        )
        status = 1
    for problem in site_problems(scan_sites(), json.loads(SITES.read_text())):
        print(problem, file=sys.stderr)
        status = 1
    return status


if __name__ == "__main__":
    raise SystemExit(main())
