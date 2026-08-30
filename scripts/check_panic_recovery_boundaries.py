#!/usr/bin/env python3
"""Reject bare panic-recovery boundaries in audited request-owned workers."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

# These modules deliberately convert worker failures into request/provider
# errors. A bare blocking task here would run outside Torii's task-local panic
# suppression and would therefore signal process shutdown before its JoinError
# was handled.
NO_BARE_BLOCKING = (
    "crates/iroha_torii/src/lib.rs",
    "crates/iroha_torii/src/lib_pipeline_handlers.rs",
    "crates/iroha_torii/src/routing.rs",
    "crates/iroha_torii/src/routing/signed_query_execution.rs",
    "crates/iroha_torii/src/privacy_issuance_api.rs",
    "crates/iroha_torii/src/da/ingest.rs",
    "crates/iroha_torii/src/da/spool.rs",
    "crates/iroha_torii/src/da/taikai.rs",
    "crates/iroha_torii/src/private_settlement.rs",
    "crates/iroha_torii/src/sorafs/api.rs",
    "crates/iroha_torii/src/zk_attachments.rs",
    "crates/iroha_torii/src/zk_prover.rs",
    "crates/iroha_torii/src/sns.rs",
    "crates/iroha_torii/src/parliament_tle_release.rs",
)

REQUIRED_SNIPPETS = {
    "crates/iroha_core/src/panic_hook.rs": (
        "pub fn catch_unwind_suppressed",
        "with_hook_suppressed_async",
        "blocking_worker_reuse_does_not_retain_suppression",
    ),
    "crates/iroha_torii/src/panic_recovery.rs": (
        "pub(crate) fn spawn_blocking_recoverable",
        "pub(crate) fn spawn_joined_recoverable",
        "pub(crate) async fn join_recoverable",
        "ordinary_invariant_panics_remain_unsuppressed",
    ),
    "crates/iroha_torii/src/da/taikai.rs": (
        "read_optional_regular_file",
        "crate::panic_recovery::spawn_blocking_recoverable",
    ),
    "crates/iroha_torii/src/private_settlement.rs": (
        "crate::panic_recovery::spawn_blocking_recoverable",
        "crate::panic_recovery::spawn_joined_recoverable",
    ),
    "crates/iroha_torii/src/sorafs/api.rs": (
        "governance_dag_blocking_response",
        "crate::panic_recovery::spawn_blocking_recoverable",
    ),
    "crates/iroha_core/src/executor.rs": (
        "crate::panic_hook::catch_unwind_suppressed",
    ),
    "crates/iroha_core/src/zk.rs": (
        "let pk = crate::panic_hook::catch_unwind_suppressed",
    ),
}

FORBIDDEN_RECOVERY_SNIPPETS = {
    "crates/iroha_core/src/executor.rs": (
        "std::panic::catch_unwind",
    ),
    "crates/iroha_torii/src/privacy_issuance_api.rs": (
        "catch_unwind(AssertUnwindSafe(operation))",
    ),
    "crates/iroha_torii/src/da/spool.rs": (
        "catch_unwind(AssertUnwindSafe(move || (run)()))",
    ),
}

BARE_BLOCKING = re.compile(r"(?<![A-Za-z0-9_])(?:tokio::task::|task::)spawn_blocking\s*\(")


def main() -> int:
    failures: list[str] = []
    for relative in NO_BARE_BLOCKING:
        source = (ROOT / relative).read_text(encoding="utf-8")
        matches = list(BARE_BLOCKING.finditer(source))
        if matches:
            lines = [str(source.count("\n", 0, match.start()) + 1) for match in matches]
            failures.append(f"{relative}: bare spawn_blocking at line(s) {', '.join(lines)}")

    for relative, snippets in REQUIRED_SNIPPETS.items():
        source = (ROOT / relative).read_text(encoding="utf-8")
        for snippet in snippets:
            if snippet not in source:
                failures.append(f"{relative}: missing audited recovery marker {snippet!r}")

    for relative, snippets in FORBIDDEN_RECOVERY_SNIPPETS.items():
        source = (ROOT / relative).read_text(encoding="utf-8")
        for snippet in snippets:
            if snippet in source:
                failures.append(f"{relative}: unreviewed bare recovery boundary {snippet!r}")

    if failures:
        print("panic recovery boundary guard failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print("panic recovery boundary guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
