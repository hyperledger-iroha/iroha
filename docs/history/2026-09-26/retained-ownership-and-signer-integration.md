# Retained ownership and signer integration — 2026-09-26

Work remains on `/Users/takemiyamakoto/devstuff/iroha`, branch `optimizations`.
This is an uncommitted implementation checkpoint. It is not a frozen candidate
or release qualification. The existing first-release requirements, software
custody policy and production cryptographic/resource ceilings remain in force.

## Integrated source

- Canonical proposal hashing borrows the original signed payload instead of
  deep-cloning the whole block and constructing an unused versioned copy.
  The sole V1 schema, frame header, signature set, resultless payload and hash
  remain authoritative. Complete payload/frame buffers still require funding.
- Retained validation moves the decoded signed body into its existing candidate
  slot only while the original source/capture owner needs it. Full signed-body
  equality rejects a changed retry. Ready owners release that body before
  marker persistence; panic and successful publication retain the subject
  tombstone. The test asserts a stable nested signature allocation across the
  move, rather than the movable outer Rust value's address.
- Native preparation moves its frozen source block into the recorder after
  global validation. All recorder callers use that consuming interface. The
  original State/generation checks and source/body authority checks remain.
  The initial source clone and full nested allocation admission remain open.
- Kura's finality and retained sidecars decode directly from their capped byte
  slices, preserving canonicality and cumulative Norito decoder limits. The
  removed input copies and proposal clone are removed from the named cold-read
  working-set charge. This does not fund the whole historical signer replay.
- The BFV eight-party roster requires canonical, distinct, nonzero Ed25519 keys.
  The unavailable private-share relation and independent production parameter/
  audit qualification still reject production use.
- Release-manifest role 13 participates in the exact signed Check binding.
  Its executed operation and daemon production source remain closed. Signer
  Check authentication verifies execution against the already authenticated
  lineage body, removing a second read/decode of that body. No key-use,
  signer-purpose or promotion gate opens.
- The X509 one-SHA diagnostic measures a conditional size reduction only; it
  neither removes certificate coverage nor admits an oversized production proof.
- Two unnecessary x86_64 `unsafe` wrappers in vendored `num-bigint` are removed
  for the pinned Rust toolchain; arithmetic is unchanged.

The pre-edit source files and patch identities are preserved in the ignored
`target/first-release-integration-20260926-before` directory. Source edits are
reviewed incremental cuts; F02, F03, F04, F06, F07, F09 and the release gates
remain open.

## Scoped validation and concurrent source change

All five Apple bridge targets built and the ABI-24 XCFramework passed artifact
validation. The matching native fingerprint was
`d7c089dfacd695ae5373321e8484931013ed8674f58b2cf5e6b99a209adc17fe`.
The subsequent focused Swift run passed 57 tests with no failures or skips:
19 Sumeragi wire, 20 Sumeragi Torii, two conviction/native golden and 16 App
Attest tests. A long expected-byte expression was split into ordered appends,
and the status fixture's stale epoch assertion was aligned with Rust's genesis
epoch zero. Both changes are test-only; pre/post native fingerprints matched.

That result belongs to the pre-merge source snapshot. Another task then began
reconciling `origin/optimizations`; this task preserved all eight conflicting
files and their three index stages before reviewing them. The user confirmed
the other task owns merge resolution. This task did not resolve or stage those
conflict files, create a branch/worktree, or make a commit. Current-source SDK
and native qualification must be repeated after integration settles.

The first DataModel test attempt stopped at a concurrent Rust merge marker.
After the other task removed the markers, the retry acquired the build lock
and started compiling. Focused DataModel, Core and crypto execution is still
pending at this checkpoint; static review is not a test pass.

The prior Python 3.12 readiness session no longer has retrievable terminal
output, so no full-suite pass is claimed from it. A fresh pinned 3.12.14 run is
recording a durable log, JUnit XML and exit result under
`target/sorafs-production-readiness-py312-20260926`. Its result is pending.

Formal source selectors and adversarial mutations are being aligned with the
consuming source/retained-body interfaces. This does not replace the required
TLC/Apalache/TLAPS/Verus bounds, four-validator lifecycle matrix, distributed
soaks, proof/hardware parity, independent audits or signed release evidence.
