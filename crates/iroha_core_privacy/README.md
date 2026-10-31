# Native privacy and execution proofs

`iroha_core_privacy` owns native privacy engines, compiled protocol profiles,
privacy state records and validation plans, verified ledger effects, and
deterministic execution proof relations. It has no dependency on `iroha_core`
or the P2P runtime. Core owns world-state storage, transaction authority,
admission, and application of the returned effects; this crate never opens or
commits a node's world state.

`privacy_verifier::verify_privacy_envelope_v1` is the single dispatch of the
thirteen action roles of the twelve registered protocols. It resolves the one
role of an envelope, asks the typed registry in `privacy_dispatch`
(`PRIVACY_ROLE_DISPATCH_TABLE_V1`) whether that role is verified, runs the
role's native verifier and seals only an effect of that role. A role the
registry records as unsupported returns the typed
`PrivacyVerificationErrorV1::UnsupportedOperation` with its concrete
limitation; it never succeeds. The registry keeps verification, the effect
class, whether the effect is reachable, and delivery as separate facts, and no
role is recorded as delivered.

The implementation uses `ivm` for prepared-contract execution diagnostics and
the IVM step relation, alongside `ivm_abi` for ABI constants. It uses
`fastpq_prover` for accelerated Goldilocks transforms and `fastpq_isi` for the
shared FASTPQ profile identity. These dependencies retain their existing
runtime implementations, including deterministic acceleration and fallback
behavior. The transitive graph therefore includes IVM's configuration and
Sumeragi dependencies. The crate boundary removes Core ownership; it does not
claim an isolated cryptographic-only dependency graph.

Explicit Norito schema identities are independent of the Rust crate location.
The existing `iroha_core::privacy_*` schema names and fixed logging targets
remain the canonical identities. No facade or fallback decoder is provided
at the old public Core module paths. Source-bound compiled manifests name
the current owner paths; historical table preimages retain their original
source paths and hashes.

Default features are `zk-stark` and `simd`. `zk-stark` supplies the ZK-ACE STARK
backend and its compiled profile. `simd` enables SHA-3's assembly backend without
changing outputs. Node and SDK consumers declare their required verifier
features explicitly. `privacy-release-evidence` adds the native evidence
constructors and exact-twelve conformance model; `test-utils` exposes fixture
material without node storage. Neither evidence nor fixture features belong
in production shipping graphs. These features do not introduce runtime
switches for IVM syscalls or opcodes.

On macOS, the target dependency also enables the existing FASTPQ `fastpq-gpu`
feature, including when this crate uses `--no-default-features`. Eligible
transforms use Metal only after the immutable compiled bundle and device are
admitted and the complete staging allowance fits the existing memory budget.
Absent or refused bundles, unsupported targets, and insufficient headroom retain
the deterministic CPU path; uncertain device completion still refuses proving.
This target forwarding leaves other platforms' dependency defaults unchanged
and adds no CUDA compiler discovery to driverless Linux builds. Hardware
availability does not establish complete-proof performance or release readiness.

Bootle/Lantern interprets its five fixed `u64` rejection-constant limbs through
an explicit 64-byte little-endian integer encoding on both 32-bit and 64-bit
hosts. The constants, Q256 thresholds, random draw order and sampling-profile
identity remain unchanged. Known-answer tests pin all four integer constants,
exact reciprocal floor bounds, standard/bimodal thresholds and the profile digest.

The ordinary `privacy_engines::zk_x509::holder` API derives the exact wallet
ownership-signature digest and its canonical public message, constructs a bounded
owned witness, and produces a complete self-verified MAIN/CA credential proof.
Message-signing APIs such as `SHA256withECDSA` consume `ownership_signing_message_v1`;
prehash-signing APIs consume `ownership_challenge_v1`. Both sign the same once-hashed
message; passing the digest to a message signer would hash twice and is rejected. The wallet keeps the subject
signing key. The consuming prover clears the owned witness and its encoded
buffer on return or unwind. The API is present with default features and with
`--no-default-features`; it does not require `privacy-release-evidence`.
The consuming `ZkX509HolderAttemptV1` checks the full six-term estimate only
at preparation, then the remaining proving/publication/submission/clock terms
before the native prover. `ZkX509PendingCredentialProofV1::release` rechecks the
unchanged original statement against newly authenticated State and chain time,
checks publication/submission/clock reserve, and only then exposes the exact
proof bytes. Explicit local times must progress forwards. Neither a cached
snapshot nor the local budget establishes wall-clock freshness on an idle chain.
Paired-host enrollment, authenticated encrypted transport, host revocation and
mobile/desktop bindings remain separate X.5 work. Native availability does not
claim resource qualification or change the current consensus profile policy.

X.509 proving expands thousands of masked columns onto the common evaluation
domain and replays native sources to stay within the memory ceiling. CPU coset
evaluation skips butterflies outside the public coefficient extent, including
the complete masking tail; it never inspects private values to choose work.
For common-domain log22 queries, the native-log5/log8 columns use direct Horner
evaluation when their complete masked coefficient extent and selected rows fit
the public bounds. This avoids a full device transform for a sparse query.
Bounded P-256 value replay returns only its requested columns while preserving
the original product traversal and terminal checks. These optimizations retain
the same polynomial values, proof format, transcript and resource limits.
Joined query replay retains sibling-pair Merkle roots, reducing the maximum
replayed set to 272 rows. Selected pairs and every retained root are verified
before returning the original root and frontier; upper tree reductions use
bounded parallel tiles. The larger private cuts remain in every phase's budget.
For native-log15/16/18/19 query batches of eight columns, an interleaved CPU
transform shares public traversal and twiddle powers across the columns and
evaluates only that smaller set. Its full scratch still occupies eight field
columns and clears on every exit. Singleton batches retain device admission;
backend selection depends only on the measured public geometry.
Composition mixes scalar residues into Fp4 with exact widened accumulators and
bounded carry correction. SHA base replay projects only the requested cells;
P-256 writer replay projects public inactive rows without building full rows.
Both source paths retain their validation and terminal checks.
The memory plan derives its peak from complete MAIN/CA phase lifetimes,
including retained Merkle cuts, and explicitly reserves two bounded coefficient
replay batches for caching. Admission and cache capacity therefore do not rely
on incidental padding in the hash-state representation.
Joined transforms reuse only the headroom of absent registration owners, after
global admission. Metal's exact-root forecast charges its sole unpublished
staging allocation and no-copy view, the complete pool and twiddle caches, and
the existing dispatch and process reserves. Paired global FFT stages reduce
device memory passes while preserving resource barriers and publish-after-wait.

The ignored release tests `common22_prefix_fft_reports_full_output_parity_and_timings`
and `value_auxiliary_compact_projection_cost_diagnostic` compare these kernels
with their dense references. The ignored
`selected_low_degree_horner_pruned_and_metal_cost_keep_all_query_rows` comparison
forces the original Metal path alongside both CPU paths and checks exact values.
Measure complete proving separately with
`maximum_structural_credential_proof_with_retained_public_receipt`; its retained
phase tree distinguishes source replay, transforms, composition and verification,
and its unchanged 300-second assertion remains a qualification requirement.

Focused validation reuses the ordinary warm target and native jobserver. Use a
stable `--target-slot` only when another build runs concurrently:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- check -p iroha_core_privacy --lib
scripts/cargo_fast.sh --stable-local-metadata --incremental -- check -p iroha_core_privacy --lib --features test-utils,privacy-release-evidence
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_core_privacy --lib
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_core_privacy --lib material_fixture
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_core_privacy --test zk_x509_holder_prover
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_core_privacy --no-default-features --test zk_x509_holder_prover
```

The ordinary suite preserves protocol assertions and ignored qualification
tests. Passing component tests does not establish native privacy release
qualification or whole-network settlement readiness.
