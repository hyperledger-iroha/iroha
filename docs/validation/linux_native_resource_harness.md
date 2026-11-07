# Required Linux native resource harness

This change moves only `NativeRunTests` out of the broad helper pytest command
and uses an unconditional `linux_resource_harness` aggregate in the same workflow.
`rust_required` accepts this job only when its result is `success`; missing pins,
failed admission, expired artifacts, cancellation and skipped execution all fail
the required aggregate. Four separate GitHub-hosted VMs invoke the existing Linux native runner for the
closed partitions in `ci/linux_native_harness_shards.json`. Their disjoint union
must match all 20 names in `ci/linux_native_harness_tests.json` and the actual
`NativeRunTests` class. The required aggregate accepts every named result exactly
once, the total count of 20, exit status zero, and zero skipped tests.

The checked-in approval template intentionally fails. No binary has been approved
by this proposal. Enabling the job requires independent review of a real candidate
and setting the repository variable described below. Generating a candidate or
calculating its hashes does not approve it. There is no build, alternate binary,
diagnostic fixture, latest-artifact selection, or automatic approval path in this
consumer. Production publication is outside this job.

## Three independent checkouts

The current PR supplies the harness, tests and consumer in `harness/`. A protected
repository variable, `LINUX_OBSERVER_APPROVAL_COMMIT`, selects an exact 40-character
commit containing `ci/linux_observer_approval.json` in `observer-approval/`. Its
approved `source_commit` selects a third, clean checkout at `observer-source/`.
The current observer candidate uses the clean base
`3743ff550ca3632e92b23e5d60d084b41a069b29`; the PR checkout is never used as the
observer source. The input job captures the approval variable once and freezes both the API head
commit and actual harness merge commit, run and attempt, approval bytes, complete
approved pins, consumer, runner, native test source, inventory and partition
digests. Each shard and the aggregate checks its clean checkouts and original
bytes against that identical context before use. Each shard rechecks them after
execution and retains the approval bytes and commit with its results.

Treat changes to that repository variable and to the selected approval commit as
approval operations. Access to update the variable must remain restricted to
authorized maintainers. A PR cannot supply its own replacement approval through
this workflow. The reviewer evidence URL and reviewer name document the decision;
they do not independently authenticate it. The protected variable and reviewed,
immutable configuration commit are the trust boundary. Do not set the variable to
a candidate producer's output without an independent review.

The separate observer checkout must be clean before and after execution. The
existing native fixture opens and retains the reviewed source originals, compares
their complete manifest with the independently approved source-manifest digest,
validates normal publisher provenance, and admits the held original native module.
No build runs in that source checkout. The consumer raises its descriptor soft
limit only when needed to retain the complete source; an insufficient hard limit
fails the job. Output is a new directory outside all source checkouts. Existing
attempts are not overwritten.

## Evidence required before selecting an approval commit

The independent review must identify and check all of the following against the
actual successful `linux_observer_candidate.yml` run:

1. Repository, producer API `head_sha` as `producer_head_commit`, actual driver
   checkout commit as `producer_commit`, run ID, run attempt and workflow path.
   Keep the two commits independently pinned: a `pull_request` producer's
   `github.sha` is the merge checkout commit, so it must not be assumed equal to
   the API head commit. See GitHub's [pull request event contract](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#pull_request).
   The review must cover the producer workflow and script at its actual checkout, including
   the fixed clean observer base, normal publisher invocation and absence of
   pre-review bridge execution. A candidate receipt alone is not authority.
2. Artifact ID, exact attempt-specific name, unexpired status and the SHA-256 of
   the complete original ZIP. Bind the API artifact and run metadata to the same
   reviewed producer run and attempt. Check the GitHub-provided artifact digest
   and independently reviewed archive bytes; do not promote a locally calculated
   digest directly into approval.
3. The original `review/candidate.json` and its SHA-256. Verify its source commit,
   producer revision, `bridge_executed: false`, target, source-manifest digest,
   module digest, provenance digest, publisher digest, retained source custody,
   toolchain and build command evidence. Compare these claims with the preserved
   producer logs and the clean source.
4. The whole approved source-manifest digest, target
   `x86_64-unknown-linux-gnu`, original `libiroha_process_bridge.so` digest and
   original `publisher-source.json` digest. Inspect normal publisher provenance
   and its input/toolchain/environment closure. Approval covers those original
   bytes, not a regenerated or rewritten provenance document. Preserve the
   publisher/source evidence needed to reproduce this review.
5. The independent reviewer identity, permanent review evidence URL in this
   repository, UTC review timestamp and review decision. Commit the completed
   `iroha.linux-observer-approval.v1` record with `status: approved`, then have an
   authorized maintainer select that immutable commit in
   `LINUX_OBSERVER_APPROVAL_COMMIT`. Record this selection with the release
   evidence. No secrets belong in this record.

Unknown values in the template remain null until that review is complete. There
are no placeholder production digests in this proposal. Synthetic values in the
offline control tests are confined to temporary fixtures and are never loaded as
native code or written into the approval record.

## Transport and execution checks

The job requests the exact approved artifact ID from the approved run, retains the
GitHub artifact and run-attempt metadata, and downloads the original ZIP without
extracting it. The pinned download action exposes `skip-decompress` and
`digest-mismatch: error`; see its [input contract](https://github.com/actions/download-artifact/blob/9000827ccba6bdab643e8b6fd33ac0654aef8333/action.yml).
The consumer independently compares the complete archive with the approved digest
before processing any member. It rejects duplicate, traversal or symlink members
and extracts only the three approved originals with bounded reads. It compares
the extracted originals with their separate approved digests and compares the
receipt with the approval. The bridge directory contains exactly the original
module and original provenance document expected by the existing loader.

The native runner receives all four approved admission pins and separate source
and bridge roots. Each shard passes its fixed list of repeated `--test` arguments
to name the actual existing methods; no method body, internal loop or assertion
is split or changed. Independent processes avoid the fixture global monkeypatches
interfering with each other. An AST comparison fails if the current class changes without an updated
reviewed inventory; the consumer cannot silently run fewer methods. Each native
method continues to exercise the normal-publication fixture, original custody,
native child observation, refusal cases and public resource harness. This is not
a replacement suite or a platform skip.

Before the native run, the job requires a disposable GitHub-hosted VM and a
non-root runner identity. It retains the kernel, identity and original
`kernel.perf_event_paranoid` value, invokes only
`sudo -n sysctl -w kernel.perf_event_paranoid=2`, and requires an exact readback of
`2`. Failure to set or read back the policy fails the job; before/after values
and setup stdout/stderr remain with the metadata evidence. This policy permits
the observer's self-scoped user-space software perf event while retaining the
kernel-profiling restriction described in the [Linux perf access policy](https://www.kernel.org/doc/html/latest/admin-guide/perf-security.html).
The setup runs only on this disposable CI VM. Python and all tests retain the
ordinary runner identity; no root test execution or resource-limit bypass is
added. Setting the policy does not prove event support, ring operation or native
observer behavior: all 20 actual tests must still pass with the approved module.

The native runner receives only PATH, HOME, locale and Python bytecode settings,
not the API token. Full test output, explicit method names, command, current
harness commit, runner digest, approved pins and outcome are retained in the
result artifact, along with source approval and producer metadata. Each native VM
retains the original 90-minute deadline and unchanged native resource limits. It does not terminate unrelated host processes.

The workflows request 30-day artifact retention, but effective retention is
controlled by repository policy. GitHub reports that candidate artifact
`13570671555` expires on `2026-11-14T10:18:59Z` (seven days). An expired or deleted
candidate intentionally fails required CI. Before its actual expiry, arrange a
new independently reviewed immutable candidate or an explicitly reviewed archival
transport change; there is no unpinned fallback. Approval changes are separate
from ordinary PR harness changes.

## Four original packets and the required aggregate

`linux_resource_harness_inputs` freezes authority once; the fixed matrix runs
`native-1` through `native-4`, with `fail-fast: false` and separate disposable VMs.
A shard receipt is explicitly `shard_only` and cannot qualify the full suite.
Each VM repeats the original approved ZIP, clean source, retained original module,
provenance, native custody and perf-policy admission. The original 20 test bodies
and their refusal, resource-limit and salvage loops remain byte-identical.

The always-running `linux_resource_harness` requires successful input and matrix
jobs. It retrieves current attempt-specific jobs and run metadata, selects exactly
four attempt-named artifact IDs, and downloads the four original ZIPs without
extracting them. The data-only aggregate verifies their GitHub API sizes/digests,
run/head/attempt, exact successful job names, shared context and pins, original
producer metadata and original approved candidate/module/provenance digests. It
requires each shard's original raw log to contain precisely its assigned method
results once, an exact terminal count, plain `OK`, and no skip/failure/error.
Every method must occur exactly once across all four packets. Missing, duplicate,
unknown, cancelled, partial, wrong-head or wrong-pin evidence fails closed.

GitHub artifact metadata does not supply a direct producer job ID. Binding uses
the reviewed exact job/artifact names, current run and attempt, API head and
matching packet context; the receipt states this limitation rather than inventing
a job-ID attestation. Physical file identities and process handles belong to each
VM and are enforced by its unchanged native fixtures, not compared across hosts.
The aggregate retains each API job/runner identity and time interval. It reports
20 methods across four hosts, not a single-host sequential benchmark, publication
approval or a guaranteed speedup.

Partition balancing uses static estimates of 62 `public_run` admissions. The 33
salvage replays bypass `ObserverDelivery`; they remain intact and are secondary,
unmeasured cost. These estimates motivate separate VMs but do not predict exact
runtime. A full sequential `run` remains supported for local qualification with
all 20 methods; arbitrary runtime subset selection is not exposed by the consumer.

## Local control verification and activation

Run the new stdlib control suite with:

```sh
python3 -B -m unittest discover -s scripts/tests -p 'required_linux_resource_harness*_test.py' -v
```

These tests check refusal of missing pins, substitutions, archive hazards,
incomplete or skipped suite output, and the actual required aggregate's handling
of non-success outcomes. The perf-step control runs the workflow's exact shell
with fake command functions, checking refusal of failed setup, wrong readback,
self-hosted execution and a root runner without changing the test host's policy.
They do not load a bridge or claim to execute any of the
20 real native tests. Qualification of the required job remains pending until the
Linux run with independently approved originals reports all 20 methods passing.

Apply the workflow addition, scoped broad-pytest deselection, consumer, inventory, fixed partition,
controls and documentation together. Do not apply the deselection by itself.
The `Required affected Rust validation` workflow aggregate refuses a failed,
skipped or cancelled native harness. GitHub's `optimizations` branch currently
has no required status-check policy (verified on 2026-11-07), so the aggregate
alone does not prevent manual merging. Release review must verify passing
validation checks before merging. This change does not modify repository branch
rules; approval selection uses the separate maintainer-controlled variable.
