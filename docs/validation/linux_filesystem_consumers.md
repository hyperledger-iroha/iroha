# Linux filesystem consumer checks

The 2026-10-31 component checks use the current working tree on an Intel
Core i7-11800H, WSL2 Linux `6.18.54.1-microsoft-standard-WSL2`, an ext4 checkout,
and Rust `1.99.1 (58cf8f9c3 2026-10-13)`. They exercise Linux filesystem owners
and isolated loopback services. They do not qualify native Windows, physical
power-loss durability, signed release delivery or a deployed network.

Raw logs, tool identity, source inventories and executable hashes remain
untracked under `dist/intel-platform/wi10-linux-20261031/`. The 451-file
inventory covers the SoraFS node/orchestrator/CAR, SCCP RPC/wallet and
`iroha_fs` source trees plus root build inputs. It is not a complete transitive
dependency or release custody inventory. Its initial digest is
`50752bbb3a19fcc02b5586d366c77d0d6ea9b2fa16554b3de410c94e828eece0`.
The inventory captured before the final Node run has digest
`28ab2e3bcdf917fe95706e6f0eab8515619f5bee7c3515d71e7830c06c97fff5`;
digests use sorted `relative-path + NUL + file-sha256 + newline` records.
Each receipt states the tested owner inputs and executable hash. Later
orchestrator edits and external fixture regeneration are separate reviewed
inputs; the initial inventory is not claimed to remain unchanged. The expanded
463-file final orchestrator snapshot includes its generated TLS control and the
selected CI, governance and native-finality fixture inputs, with digest
`aae750c89cbb574d3f84d0ef0b11fa2d4011a0d2ebae04716fd7e8f49f8f8e6c`.
All 463 selected inputs and five executables, including the separately spawned
CLI, remain unchanged before and after the final 452-test orchestrator run.
A final current-source Cargo library rebuild/no-run retained the exact Node
image already used for all 1,599 library tests and both Kubo controls; the only
changed input in that prior owner/build inventory was the workspace lockfile.
The final pin-workflow run also freezes and rechecks both its harness and the
spawned `sorafs-node` executable. Its retained 462-file snapshot precedes only
the final orchestrator test accounting correction and added fixture inventory;
the Node inputs remain unchanged.

The Node repairs are test-only. The journal deadline control uses paused Tokio
time, retaining the exact 5 ms deadline, 10 ms delay and staged refusal
assertions, and checks that delayed inventory creates no durable entry. Only
the test dependency enables Tokio's `test-util` feature. The publication restart
control follows [SR5](../../specs/sorafs/first_release_reliability_goals.md): retain
the exact authenticated manifest for repair, quarantine the corrupted payload,
refuse reads and proofs, and preserve corrupt bytes. Metadata-corruption startup
refusals remain covered by the library suite.

The CLI checks exposed a production directory-proof gap. `proof verify
--payload=PATH` now reuses the bounded source planner to reconstruct the complete
file layout, checks the canonical chunk envelope against that plan, and retains
the full CAR/root/PoR verifier. It rejects missing multi-file layout, changed
paths/content, mismatched plans, symlink inputs and oversized manifest/CAR files
before emitting a proof summary. Manifest and CAR reads use the existing retained
native file owner. See [CLI behavior](../../specs/sorafs_cli.md).

Other CLI fixture repairs isolate a freshly signed governance node, obtain a
request-bound fee quote, supply an actual native-finality genesis checkpoint,
and make expected deployment refusal terminal instead of leaving a 404 mock in
the production preparation polling loop. Cross-origin signed-request refusal,
no-fallback behavior and unverified publication assertions remain intact. The
CI sample's canonical chunk-plan envelope and proof summary were regenerated
through the CLI; its CAR and both manifests remained byte-identical. The
regeneration receipt is `ci-sample-regeneration.json` in the evidence directory.

Initial deadline, restart, native error-text, outdated fixture and unsupported
directory-proof failures remain in their logs. The old HTTP gateway fixture
also failed admission and hung during mock teardown; it is not accepted
transport evidence. The obsolete HTTP fixture and its mock were removed. Three ordinary generated-local
controls now exercise the real native-finality verifier and TLS transport; the
empty-provider refusal is ordinary too. The test server counts matched requests
before cancellable TLS shutdown; exact client payload/CAR checks establish
delivery. Their signed fixture World preimages
are synthetic and establish no deployment authority. The old deploy attempt
ended with 4 passes and one stale-summary assertion failure after 1,208.36 seconds. It used incomplete preparation mocks and its CLI
path was rebuilt during execution, so none of that attempt qualifies a frozen
image. The repaired current CLI suite completes without the polling delay.

Commands below use
`scripts/cargo_fast.sh --target-slot intel-platform --stable-local-metadata --incremental --jobs 1 --`
unless stated otherwise.

| Cargo command | Result |
| --- | --- |
| `test --locked -p iroha_sccp_wallet --lib -- journal::tests:: pure::evm::tests:: --test-threads=2` | 27 passed: 15 journal and 12 EVM/key controls. One intentionally ignored child is invoked by its parent. Used the ordinary warm target with `--jobs 2`. |
| `test --locked -p iroha_sccp_rpc --lib endpoints::tests:: -- --test-threads=2` | 17 passed, including native sensitive-header, owner-only and single-link custody controls; 128 filtered out. |
| `test --locked -p sorafs_node --lib -- --test-threads=2` | 1,599 passed; three ordinary-suite ignores comprise two explicitly executed Kubo controls and one child probe invoked by its parent. |
| `test --locked -p sorafs_node --test pin_workflows -- --test-threads=2` | 17 passed, including CLI ingest/export/PoR/replay and the repaired restart-quarantine control; one release-package control ignored. `SORAFS_NODE_SKIP_INGEST_TESTS` is unset. |
| `test --locked -p sorafs_orchestrator --test sorafs_cli -- --test-threads=2` | 95 passed after the bounded proof-input repair; no failures or ignores. |
| `test --locked -p sorafs_orchestrator --lib --bins --tests -- --test-threads=2` | 452 passed: 232 library, 122 CLI unit, 3 parity and 95 CLI integration; zero failures or ignores, including all authenticated gateway controls. |

A separate focused control passes the actual `car pack` → `manifest build` →
`storage prepare` → `proof verify` command chain for 43 files spanning multiple
chunks. It checks the complete canonical inventory and payload, then substitutes
one byte in the 42nd entry without changing its length. Preparation refuses the
changed commitment before writing any new output, preserving the prior artifacts.
Only this new control ran (one pass, 95 filtered); the 452-control result above
retains its earlier scope. Cargo reused CLI `9f23c1d2…` unchanged and rebuilt test
image `4b00dec8…`; all 463 selected source hashes and identities stayed unchanged.
The receipt is `dist/linux-sorafs-composed-cli-20261031/result.json`
(`b41b3831ba2facbf94838b71b4f7a562d5a0de753522a815a9704aaf94596b4f`).
This is local command composition evidence, not authenticated installed-release,
network publication or gateway availability qualification.

Both real Kubo publication/restart/tamper controls passed on the final Node
image: 2 passed, no failures or ignores. They explicitly run with
`SORAFS_RUN_KUBO_INTEGRATION=1`, `SORAFS_KUBO_BIN` and
`test --locked -p sorafs_node --lib governance_service::tests::real_kubo_publication_ -- --ignored --nocapture --test-threads=1`.
Kubo 0.42.0 follows the pin in
[the workflow](../../.github/workflows/sorafs-governance-dag-kubo.yml).
The official GitHub release mirror supplied the archive after the primary
distribution endpoint timed out; its SHA-512 matches the workflow pin.
`kubo-tool.json` retains the URL, complete archive checksum and executable hash.

The separately ignored SORA CARS package control requires actual
`SORAFS_QUALIFICATION_DIST` and `SORAFS_QUALIFICATION_PACKAGE` inputs. It remains
unqualified by these component checks.

## Core and service owners

The ordinary warm target builds the Core library with
`--features bench,iroha-core-tests`. The final image passes all 876 selected
controls: the original 830 plus 24 settlement-sidecar, 12 ISO reference-data and
10 membership-storage controls. The selection covers canonical evidence, FASTPQ
artifacts and source archives, native context and storage, pipeline execution,
geometry and tiered State. No selected test is failed or ignored. Runtime is
325.37 seconds; this is test-suite duration, not a performance qualification.

The two prior pipeline-trigger failures came from fixtures that inserted deferred
actions before founding their runtime selection. The fixtures now apply actual
signed genesis and capture/check the trigger's runtime consent and registered
height in a setup overlay. The rejected-status and wrong-event controls remain;
production trigger admission and execution are unchanged. The earlier 828-pass,
two-failure attempt remains recorded separately.

The final Core executable SHA-256 is
`f461937c08ce1908fdc74145e8a02f9b3c1ae78e03bf448e0d9dfcfd704571fc`.
Its image and 2,446 Core Rust/build inputs remain unchanged across the selected
run. This owner inventory is not a complete transitive dependency or release
custody inventory. The exact selection, original build output, run log and
before/after hashes are retained under
`dist/linux-native-consumers-20261031/iroha-linux-core-final-*`.

Fresh CLI, Torii and daemon images also pass their selected Linux controls:

| Owner | Executed scope | Result |
| --- | --- | --- |
| CLI | The original retained-output, deployment, onboarding, release and host-boundary selection | 298 passed; zero failed or ignored. |
| Torii | Webhook persistence (51), metadata (4), gateway compliance (61), ISO reference data (31) | 147 passed; zero failed or ignored. |
| Daemon | Runtime credentials (4), repair-source custody (3) | 7 passed; zero failed or ignored. Two Windows-only repair controls are absent on Linux. |
| Telemetry | Integrity checkpoints and websocket controls with `event-exporter,dev-telemetry` | 28 passed. |
| TestNetwork | Release contract (18), resolved programs (10), development process (3), retained genesis (8), port custody (13), release resolution (10), developer selection (3), build fingerprints (7) | 72 passed; zero failed or ignored. |

The CLI, Torii and daemon runs retain unchanged executable hashes and respective
5,050-, 4,944- and 5,270-input workspace audits. Their selections, logs and audits
are archived beside the Core receipts in `dist/linux-native-consumers-20261031/`.
Those three service audits precede the later test-only privacy experiment module
declaration and qualify their recorded artifacts and inputs. The CLI repairs preserve signed
genesis staking parameters when extending fixture parameters and update a stale
rendered-budget assertion; they do not change live deployment behavior.
TestNetwork's final image has SHA-256
`c2a5d842d38ad969aa2b0c7ce7610714d8ceb062f7d71e61ad3520070bd87202`;
its image and 59 owner/build inputs remain unchanged through the 156.18-second
selected run. Its exact selections and receipts share the same evidence directory.
These program-owner controls use component fixtures, not the four actual shipping executables.
Selected owner checks do not establish full node startup, a four-validator
network, physical crash/power-loss recovery or the four-executable release corridor.
