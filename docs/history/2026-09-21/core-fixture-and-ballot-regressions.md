# Core fixture and ballot regression repair

This correction addresses the 119 failures reported from the Core library run
that passed 16,897 tests and ignored 36. It preserves the first-release catalog,
physical storage, canonical history, permissions and finalized-lane checks.

## Corrections

- PLAIN ballot execution rejects typed proposal selectors before looking up a
  standalone referendum's frozen policy. The existing alias regression retains
  every PLAIN and ZK selector assertion.
- Multi-lane fixtures authenticate their complete configured catalog when Kura
  opens. Retained privacy manifests precede catalog publication. Retire/recreate
  cases use lifecycle operations; public baseline rejection and lower-level
  semantic rejection both remain covered.
- Replay, Musubi, IVM and SCCP fixtures authenticate and provision lane storage
  before retaining canonical history. Corrupted replay forks retain the original
  network and configured authority independently of their adversarial bytes.
  The IVM fixture publishes its matching header-time cache with the hash frontier.
- Queue and transaction fixtures stage matching block hashes with membership;
  SNS uses the canonical empty-block fixture commit. Owned replica fixtures open
  Kura with an authenticated configured catalog before persisting lane payloads.
- Governance successors retain their original lane proposals, sign exact
  three-of-four lane Prepare/Commit QCs, and persist authenticated application
  receipts after finalized carrier publication. Observer role setup
  grants its explicit management permission. Lock fixtures include owning
  referenda, frozen policy and distinct custody accounts. History-drift injection
  modifies the existing prepaid tip instead of appending an unauthorized slot.
- Generic STARK coverage accepts canonical profile-qualified near misses and
  explicitly rejects bare aliases. Gas-parameter coverage retains strict address
  rejection and checks the parser's actual diagnostic.

No compatibility decoder, catalog rewrite exception, missing-storage fallback,
permission bypass or availability bypass is introduced.

## Validation

The harness build command is:

```sh
cargo test -p iroha_core --lib \
  --features iroha-core-tests,sumeragi-main-loop-tests,expensive-telemetry --no-run
```

Both libtest selections used `--exact --test-threads=4`:

| Scope | Result |
| --- | --- |
| All 119 reported failures on the final harness | 119 passed, zero failed or ignored; 139.17 seconds |
| Adjacent queue, observation, Musubi, replay, catalog/lifecycle, lane, autoscale, AXT, staking and proof-tag tests | 939 passed, zero failed or ignored; 1,269.84 seconds |

The first reported run passed 116 tests and exposed three remaining fixture
requirements: lane certificates in addition to application receipts, the IVM's
matching header-time cache, and removal of repeated staking registration. The
adjacent selection ran on that first harness. The final harness includes these
three fixture corrections and unused-mut cleanup, compiles without warnings,
and passes all 119 reported regressions. Together the selections cover 1,058
distinct passing tests; they do not claim a full workspace run.

`cargo fmt --all`, `scripts/check_no_legacy_codec.sh`, `git diff --check` and
`python3 scripts/archive_project_history.py verify --archive docs/history/2026-09-06 --check-current`
pass. Root status remains within its 300-line limit. Full workspace and network
qualification were not run.
