# zk-X509 presentation interval

Source contract for task X.1 of the [ZK delivery plan](zk_delivery_plan.md). It
states which public presentation windows a zk-X509 credential admits, where
that is decided, and how builders and verifiers are kept in agreement. It makes
no proof-soundness, performance or deployment claim.

## Definition

One definition owns the predicate:
`crates/iroha_data_model/src/privacy/zk_x509_interval.rs`
(`PrivacyZkX509PresentationBoundsV1`, `PrivacyZkX509PresentationWindowV1`). It
takes already-parsed Unix seconds and is compiled in every build.

For a leaf-first path with validity periods `[notBefore_i, notAfter_i]` (both
inclusive) and its governed complete CRL `[thisUpdate, nextUpdate)`, a window
`[start, end]` of inclusive consensus seconds is admitted exactly when

| Predicate | Bound |
|---|---|
| shape | `start < end` and `end - start <= 300` |
| every certificate, lower | `start >= notBefore_i`, so `start >=` the **latest** `notBefore` |
| every certificate, upper | `end <= notAfter_i`, so `end <=` the **earliest** `notAfter` (leaf, intermediate or root) |
| CRL `thisUpdate`, inclusive | `start >= thisUpdate` |
| CRL `nextUpdate`, exclusive | `end < nextUpdate`; equality rejects |
| CRL age | `end - thisUpdate <= 300` |
| calendar | `end <= 253402300799` (`9999-12-31T23:59:59Z`) |

The bounds type names the two admissible window endpoints, not certificate
fields: `earliest_start` is the latest signed lower bound and `latest_end` is
the earliest signed upper bound. The shared vectors use the same two keys.

The executing block second `block_timestamp_ms / 1000` must lie in
`[start, end]`. With `TU`, `NU`, `PA` the CRL times and the public end, and
`Cmin` the earliest `notAfter` in the path, the last admissible block
timestamp is

```text
min(TU + 301, NU, Cmin + 1, PA + 1) * 1000 - 1   milliseconds
```

Revision 6 wrote `Cmax` (latest expiry) in this formula. That budgets a
deadline after an intermediate or root certificate has expired; presenting
such a credential is a security defect. Every other revocation predicate
(complete-CRL binding, serial non-membership, revocation precedence,
registration freshness) is unchanged.

## Where it is decided

`zk_x509_presentation_interval_sites.json` classifies every source file that
uses the window fields, the governed CRL update fields, the two ceilings or the
canonical API.

- `canonical` sites take their verdict or their chosen window from the
  definition. In every build: statement validation, authoritative-state
  admission, the verifier's public shape and the strict DER-to-bounds helper
  `privacy_engines::zk_x509::presentation::derive_zk_x509_presentation_bounds_v1`.
  The helper and native reference relation use one complete strict certificate
  and CRL parser. The native reference relation and complete prover are also
  available in ordinary builds; prover preflight calls state admission and
  the native relation. Only release fixtures retain the test/evidence gate
  recorded under `build_gates`.
- `in-relation` sites (the DER/RFC 5280 AIR and its numeric relation slots) are
  deliberately independent formulations: one lower and one upper slot per
  certificate, a strict `nextUpdate` slot and a `thisUpdate + 300` slot. Tests
  compare them with the definition; they share only its constants.
- `deferred` sites compute by hand what the definition computes and do not call
  it yet: CRL registration and rotation freshness in the ISI executor
  (`validate_zk_x509_crl_freshness_v1`), the release-evidence submission
  deadline, and the deadline and expected window in the four-peer network
  test. Each is pinned to its reviewed source lines and carries a `TODO(X.6)`.

State admission sees the public window and the governed CRL record. The
certificate validity periods are private, so only the proof binds them: a
window past a private certificate expiry passes state admission and is
rejected by the relation.

No SDK builds an X509 statement today. Ordinary Rust holder preflight accepts
the exact leaf-first certificate DER path and complete CRL DER, validates the
closed encoding profile, and calls the canonical interval definition. Typed
failures have one public owner, `privacy_engines::zk_x509::errors`. A successful
bounds result does not authenticate signatures, certificate path linkage,
governance, revocation or possession, and does not establish proof validity.
The native reference relation performs those separate checks. Any SDK builder
added with X.5 must use this helper or reproduce the shared vectors in its own
language and be added to the site inventory. The ordinary native
`zk_x509::holder` API exposes the canonical wallet ownership challenge,
bounded owned witness construction and complete self-verifying prover.
Statement builders, SDK bindings, paired-host transport and endpoint privacy
policy remain X.5 work.

## Holder privacy

The window is public and the certificate validity dates are private. When a
certificate, not the public CRL, is the binding bound, the widest window ends
exactly at the earliest `notAfter`, and a window that starts at
`earliest_start` starts exactly at the latest `notBefore`. Either publishes an
exact private date. It can only happen within 300 seconds of an expiry or an
issuance. `window_publishes_private_bound` reports it from the combined
bounds, the CRL-only bounds and the window; a `false` result is not a general
privacy guarantee. Whether a wallet refuses or presents a shorter window is
X.5 policy.

## Checks

- `python3 scripts/check_zk_x509_presentation_interval.py` (run by `make guards`
  and the PR workflow) — independent per-interval oracle against
  `fixtures/zk/x509/interval_vectors_v1.json`, and the site inventory against
  the source tree.
- `cargo test -p iroha_data_model --test zk_x509_presentation_interval` —
  definition, vectors, exhaustive boundary grid, deadline, the latest-expiry
  regression, the disclosure predicate and the deferred-site boundary grid.
- `cargo test -p iroha_core_privacy --test zk_x509_presentation_bounds` —
  ordinary-build public API over genuinely signed two- and three-certificate
  paths, every inclusive certificate endpoint, exclusive CRL `nextUpdate`,
  freshness, malformed/deep/oversized DER, calendar boundaries and valid retry.
  Run without `privacy-release-evidence`; the library dependency has no test cfg.
- `cargo test -p iroha_core_privacy --lib presentation_interval_tests` —
  genuinely signed paths with the earliest expiry and latest `notBefore` at
  the leaf, intermediate and root; boundary equality; one-second overflow;
  `nextUpdate` exclusion; the DER-to-bounds helper, native relation, state
  admission, prover preflight, DER AIR and numeric rows all agree.
- `cargo test -p iroha_core_privacy --lib forged_window_end` — the RFC 5280 AIR
  base constraints over rows forged for a window end past the earliest expiry
  (leaf, intermediate, root), at `nextUpdate`, and past the CRL age.
- `cargo test -p iroha_core_privacy --lib relation_slot_schedule` — the
  verifier-fixed slot schedule equals the definition on the boundary grid.

What the site check does and does not establish:

- The scan is a name tripwire over the identifier spellings in `pattern`. Code
  that computes a window under other names is not found; the shared vectors
  and each builder's conformance tests are the control for that.
- Roles are reviewed classifications. Mechanically checked: every scanned file
  is classified and no entry is stale; a `canonical` site calls the canonical
  API and a non-test site that calls it is `canonical`; `in-relation` sites
  are in the engine; test files carry `test`; each `deferred` site still
  contains its pinned lines, and the boundary-grid test still transcribes
  them; each `build_gates` declaration is still in the source.

## Open

- No complete X5S1 proof was generated for the new path shapes; proving at
  these shapes belongs to X.3/X.4. The constraint-level tests cover the base
  (challenge-independent) residues; the slack-to-range copy is an auxiliary
  bus, shown there by its tuple columns and not by a proof.
- The `deferred` sites are not migrated. The ISI predicate equals
  `from_crl(...)` containing the block second for every block second within the
  RFC 5280 calendar; the definition additionally rejects a later second. X.6
  owns the migration and end-to-end deadline budgeting.
- The ordinary DER-to-bounds helper is interval preflight only. The ordinary
  holder API separately invokes the complete native prover and local verifier;
  release fixtures and process diagnostics remain gated. This API alone does
  not complete X.5/S.1. Existing parser list allocations are bounded but not
  fallibly admitted, and matching native build/test, end-to-end paired-host and
  physical-device qualification remain required.

## Local holder scheduling budget (X.6 component)

`PrivacyZkX509HolderScheduleV1` borrows one exact public statement and derives
bounds from the corresponding parsed certificate path and CRL interval using the
canonical definition. Construction requires that those bounds admit the original
public window. `PrivacyZkX509HolderBudgetV1` supplies six explicit millisecond
estimates: observation, preparation, proving, publication jitter, submission and
clock margin. Their checked sum is conservative: overlapping work earns no
implicit deduction, and no estimate has an implicit default.

Assessment first compares the caller's authenticated current selection with the
entire original statement. A change to any CRL, policy or anchor identity, digest
or epoch supersedes the schedule, including a digest change at an equal epoch or
a lower epoch. The comparison also retains the wallet challenge, governed
context and public window. Equality does not authenticate a selection: callers
must authenticate both selections and obtain the parsed intervals from the exact
original signed material.

For an unchanged selection, checked addition of the full budget to the supplied
local schedule-start Unix millisecond must fit the canonical inclusive deadline.
Equality at the last admissible millisecond fits; one millisecond beyond requires
repreparation. Budget and timestamp overflow are errors, never saturation or an
extended window. The result does not assert actual completion, authorize an early
block, or change the on-chain CRL lineage/rotation/revocation rules. The helper
neither tracks a monotonic observed clock nor establishes wall-clock freshness on
an idle chain. Its diagnostics do not expose private certificate bounds, and it
does not choose or publish window endpoints. The existing endpoint-disclosure
check remains the caller's separate privacy decision.

The ordinary `zk_x509::holder::ZkX509HolderAttemptV1` now owns the exact
statement and witness. Preparation invokes the real engine's codec, State and
native signed-relation preflight before extracting private certificate/CRL
intervals. It checks the full six-term estimate. Its consuming `prove_with_rng`
revalidates the unchanged original against newly authenticated State, trusted
chain time and network/genesis, then checks proving, publication jitter,
submission and clock margin before invoking the complete native prover.

The self-verified proof remains opaque inside `ZkX509PendingCredentialProofV1`.
Its consuming `release` repeats original-statement/State/time/network validation
and checks only publication, submission and clock reserve. Completed stages are
not charged again. Explicit local time inputs cannot move backwards. Every
phase retains the original signed/public deadline; failed, superseded or expired
attempts consume their owner and never return rewritten statements or fallback
CRLs. Private timing diagnostics stay redacted. Release and subsequent consensus
submission are distinct: intervening State/time changes still require a fresh
check, and ordinary admission remains authoritative.

TODO(X.6): authenticated State acquisition, merged-event observation, actual
submission ownership, SDK integration, issuer spacing/withholding/slow-issuer
and starvation controls remain open. A revoked State lookup must stop the caller
rather than supply a cached active snapshot. This API cannot certify that a caller
supplied its newest snapshot, infer live wall-clock freshness from a chain
snapshot or force idle chains to produce blocks. E.1/X.4 measured timing,
maximum-proof and network rollover controls remain separate requirements.
