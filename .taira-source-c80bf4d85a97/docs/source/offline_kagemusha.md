# Kagemusha offline cash

Kagemusha is the single offline-cash protocol in the first release. It supports
exact decimal amounts, sender change, offline multihop
spending, and full or partial online redemption. There is no runtime product
mode or alternative offline API. ABI 21 and the V4 chain, recursive, and
artifact carriers are the sole lifecycle surface. V2 names remain only for the
unchanged amount, note-opening, authorization, membership, and finality leaf
types that V4 embeds directly. The manifest and native capability schemas have
no `mode` field; schema/version, backend, transcript, and circuit identities
pin the exact cryptographic contract.

## Amounts and assets

Every request binds the chain id, asset definition, authoritative asset scale,
and an unsigned `u128` atomic-unit amount. The scale is read from the live asset
definition. Decimal conversion is exact: excess precision, negative values,
zero payments, and overflow are rejected. Top-up debit, note conservation, and
redemption credit use the same scaled `Quantity` value.

A spend consumes one or two canonically ordered parent notes and creates one
recipient output plus optional sender change. The transition proves, and every
verifier rechecks:

```text
sum(inputs) = recipient + change
```

The same fixed Eq/Ep circuit and key material accepts initialization, one
parent, and two parents. Two-parent append is the only merge form: it binds the
ordered parent states and proves conservation inside the recursive statement;
host-side hashing or equality checks cannot manufacture a merge.

Every non-zero output is an independently spendable branch. Commitments,
nullifiers, input branches, and output branches must be distinct. Replay,
ancestor/descendant reuse, overlapping siblings, duplicate nullifiers, and
duplicate commitments fail closed.

## Direct Torii API

The lifecycle uses exactly four Torii routes:

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/v1/offline/readiness` | Authoritative scale, block, verifier windows, and artifact requirements |
| `POST` | `/v1/offline/top-up` | Submit `OfflineTopUpRequest` |
| `POST` | `/v1/offline/redeem` | Submit `OfflineRedeemRequest` |
| `GET` | `/v1/offline/operations/{operation_id}` | Observe durable operation state and finality |

Top-up and redemption accept only the canonical typed value with
`Content-Type: application/x-norito`. They do not accept JSON request bodies or
an encoded-byte wrapper. The lowercase 64-hex `Idempotency-Key` is the signed
operation id. An identical retry returns the same operation; reuse with any
different request conflicts. A client retains its local pending operation until
Torii reports final chain finality.

Readiness and operation responses support Torii's typed response negotiation.
Readiness is authoritative only when its block context, live asset scale,
active transfer verifier, top-up-shield verifier, recursive StepEq and
StepEp verifier windows, unshield verifier window, bridge ABI, and artifact
generation agree. Its authenticated `artifact_set` field is required but
nullable. A present value binds the V4 generation, manifest, release policy,
release attestation, issuance window, proof-pair bound, and asset scale to the
atomic recursive verifier pair. It may establish authenticated backend
construction, but it does not establish recursive-lineage admission.
A null value requires both recursive verifier records and backend construction
to be unavailable and exactly one `recursive_v4_registry_unavailable` or
`recursive_v4_registry_malformed` blocker; a present value forbids both.
The recursive pair uses registry backend `halo2/ipa` and exact roles
`kagemusha_recursive_step_eq_v4_verifier_record` with circuit
`kagemusha-recursive-spend-step-eq-compact-layout-v5` and
`kagemusha_recursive_step_ep_v4_verifier_record` with circuit
`kagemusha-recursive-spend-step-ep-compact-lineage-v5`.

## Online to offline

The wallet first obtains the authoritative confidential-tree root, leaf index,
active top-up-shield verifier record, and committed block context. It builds the
zero-input shield proof, signs the complete top-up request with its registered
device authority, and submits it to Torii. Core atomically:

1. validates authorization, operation replay state, chain, scale, and policy;
2. recomputes the authoritative root and leaf index;
3. verifies the top-up-shield public inputs and proof;
4. debits the exact public amount into escrow;
5. appends the initial note commitment; and
6. persists the finalized top-up anchor and operation receipt.

After finality the wallet creates the initial recursive bundle
with the ABI-21 SDK's `initSpendV4`. The note is not available for offline use until both the chain
operation and local encrypted-state transition are durable.

## Offline transfer

The receiver creates a nonce-bound payment request containing its output
commitment, exact amount, asset, scale, verifier generation, and expiry. The
sender authenticates the request, creates recipient and optional change
outputs, proves and locally verifies the transition, then performs the
`cash_handoff_v1` boundary atomically: it irreversibly consumes the selected
parents and durably binds/signs the exact outgoing payment before exposing any
payment byte to a receiver-capable transport.

The peer payload contains the recipient's opaque proof bundle and the exact
proof-bound, secret-free membership witness required for its next spend. Replay
identity remains derived only from the recipient bundle's authenticated split
transition. The payload never carries a spend key, sender change, or local key
reference.

The receiver runs `verifySpendV4` and checks the signed request, chain, asset,
scale, exact amount, recipient commitment, hop limit, verifier activation
window, finalized top-up origin, recursive proof validity, and branch
disjointness. It atomically persists the received note before signing a durable
acknowledgement receipt. The receipt is evidence only, not acceptance or a
sender commit gate. Missing, invalid, or lost acknowledgements never unspend,
roll back, replace, or claw back the exact outgoing payment. Duplicate delivery
and exact retransmission remain idempotent. If no receiver ever obtains the
committed bytes, the sender bears cash-loss risk exactly as with physical cash.

No network or artifact fetch is permitted during send, receive, proof creation,
or peer verification. QR and NFC carry the same canonical request, payment, and
acknowledgement archives.

## Offline to online

Redemption uses the current unshield-v3 evidence API. Full redemption binds a
zero private output. Partial redemption binds exactly one non-zero Kagemusha
change output and proves exact conservation between the redeemed public amount
and the offline change branch.

Core validates the finalized top-up provenance, current recursive proof,
active recursive StepEq, recursive StepEp, and unshield verifier records,
nullifier freshness, exact scale, unshield public inputs, and optional change
branch before mutating balances. It
then consumes the branch nullifier, credits the exact public `Quantity`, appends
the change commitment when present, and persists an idempotent receipt. A wallet
keeps the source note and pending request until finality; retries reuse the same
operation id and bytes.

## Wallet state and artifacts

Wallet state V9 is encrypted and stores a set of notes rather than one aggregate
token. Each note records its opaque bundle, exact atomic amount and scale,
top-up provenance, verifier references, artifact generation, hop count,
operation stage, per-note opening material, and a reference to the wallet-level
hardware-backed spend key. The displayed balance is derived from available
notes. Pending, reserved, spent, quarantined, and redeeming notes are not
silently reclassified.

The authenticated V4 manifest binds source commit, chain, asset, scale,
activation and withdrawal heights, exact bridge ABI 21, proof size, transcript,
backend, and benchmark evidence. It contains exactly two Pasta-cycle profiles
in Eq-then-Ep order. Each profile carries exactly four external artifacts:
`ParamsIPA`, processed proving key, processed verifying key, and the final-key
selector-zero bootstrap witness. The external inventory is exactly eight
files. The bounded `KagemushaStepCircuitParamsV4` value is authenticated inline
in each profile and digest-bound into every artifact header; it is not a
separate streamed file. The content-addressed top-up-finality roster remains
release metadata outside that eight-role cryptographic inventory. Every file
has an exact framed and payload size and SHA-256. Installation streams to
private files, verifies every binding plus the canonical candidate-bound
`promotion-record-v4.norito`, and atomically activates the complete generation.
Bootstrap payload version 5 authenticates the final key-generation circuit's
canonical per-phase virtual-region breakpoints. Runtime rejects malformed or
shape-incompatible breakpoints and builds only a witness-generation circuit
from them, so it does not retain the key-generation constraint graph beside a
processed proving key.
Candidate verifier- and proving-key generation extracts or validates those
breakpoints after synthesis and drops the populated circuit before key
assembly. Proving-key assembly reuses the supplied verifier-key domain and
stages compact permutation scratch instead of retaining a domain-by-column
factor grid or parallel coefficient-clone fan-out. Empty-bootstrap permutation
assembly keeps identity mappings implicit and materializes union-find state
only on the first nontrivial copy, avoiding 17,301,504 bytes for the reviewed
k16, 11-column mapping. Verifier-key construction also builds, commits, and
drops one permutation polynomial at a time. Because the VK retains only
commitments, this removes 20,971,520 bytes of retained permutation columns and
lowers the first commitment's live field payload by approximately 18 MiB after
accounting for the shared omega and delta tables. Those streamed permutation
commitments now finish before assigned fixed columns and bit-packed selectors
expand into ten degree-sized field polynomials. That removes another
11,468,800 bytes from the reviewed first-MSM peak. These changes affect
allocation lifetimes only; compressed and uncompressed canonical processed key
bytes remain unchanged.
Proof generation then transfers ownership of that live circuit and one parsed
processed key into the prover. The circuit is released after witness synthesis;
domain-sized fixed-value and permutation Lagrange preprocessing is released
after its last commitment; and the consumed key yields only its embedded VK
for immediate proof verification before that VK is dropped. A live circuit and
full processed key therefore do not remain borrowed through transcript
finalization or overlap a separately reparsed verifier domain.
The consuming quotient evaluator additionally transforms one
copy-permutation sigma chunk at a time rather than retaining all 11 transformed
columns. It preserves canonical proof bytes while removing roughly 18 MiB of
transient field storage from the reviewed shape. Evaluation domains initialize
only the base FFT table eagerly and leave unused 2n/4n tables lazy (roughly
24 MiB at k16); quotient parts are written directly into the final interleaved
polynomial (roughly 8 MiB); and cached recursive FFT scratch is evicted before
h-piece commitments (roughly 8 MiB). The outer lifecycle remains in a
disposable one-worker Rayon pool. Large MSMs alone acquire process-wide
admission before scalar/base preprocessing and use a fixed two-worker window
pool, so concurrent outer commitments and host core count cannot multiply
preprocessing buffers, bucket tables, or allocator caches. Accumulator order is
unchanged. The checked static admission estimate is 232 MiB, not a
physical-memory prediction; the 256 MiB userspace supervisor remains
authoritative. Production candidate and physical-device peak-memory evidence
remain required before promotion.
Semantic construction loads one authenticated role at a time and drops each raw
carrier after parsing; it never assembles the six-role verifier or eight-role
prover payload inventory in memory. Runtime verifiers are transient rather
than cached per generation. Parent verification is dropped before a prover is
opened; after proving, terminal verification shares that prover's
Params/circuit context and reparses only the two small raw VKs after parsed
proving keys have been dropped. Memory-intensive install, top-up shielding,
recursive proof, and
verification entrypoints share one nonblocking process-wide permit, and a
contending ABI caller receives
`CONNECT_NORITO_ERR_KAGEMUSHA_BUSY` (`-318`) before its large input is copied.
Swift exposes that status as the retryable `proofWorkerBusy` error. If an
otherwise complete install encounters the busy permit, the coordinator retains
the authenticated spools and retries the identical candidate without streaming
the large artifact stream again; cancellation or a different candidate closes
that pending install. JVM artifact ingestion also limits every native write
chunk to 1 MiB; the
Kotlin and mirrored Java SDKs enforce that ceiling before cloning the caller's
array.
Before Halo2 parses an authenticated ParamsIPA, verifier key, or proving key,
an allocation-free structural pass checks its exact degree, commitment counts,
polynomial-vector counts, polynomial lengths, and total encoding length against
the authenticated circuit shape. Serialized inner counts are never trusted as
allocation sizes.
Release verification and finalization likewise authenticate one framed role at
a time and drop its payload before opening the next. They do not reconstruct an
eight-role raw prover container merely to check carrier bindings.
Validator startup performs the same exact shape-derived role-size preflight
before Halo2 parsing. Its decoded-memory budget includes retained IPA vectors,
verifier-key FFT domains, transient release files, and allocator headroom.
Runtime validates the shipped selector-zero proof with the final Step VK and
does not regenerate a bootstrap VK. A release that cannot fit the configured
budget fails closed before verifier parsing.
Swift, Kotlin, and Java release-authentication inputs therefore require that
promotion record alongside the trusted policy, attestation, benchmark evidence,
and review. A partial, unpromoted, or role-substituted generation never becomes
active.

## Validator provisioning and activation

Operators must configure both paths together under `settlement.offline`:
`kagemusha_release_policy_path` names the canonical Norito trust policy, and
`kagemusha_artifact_dir` names the directory whose children are manifest
digests. `kagemusha_max_decoded_bytes` caps the conservative decoded verifier
working-set estimate. It defaults to, and cannot be raised above, 256 MiB per
node; lower deployment limits are accepted. Leaving either path unset,
disabling escrow, omitting every escrow asset, omitting the funded permitted
command issuer, or supplying malformed/corrupt material is a startup error
after Kura replay and before Kura writing, networking, consensus, or Torii.

Startup authenticates every candidate subdirectory, validates framed and
payload sizes and SHA-256 values, parses the six validator-side artifacts
(ParamsIPA, verifying key, and bootstrap witness for Eq and Ep), and builds an
immutable catalog keyed by manifest digest. Loading fails before large artifact
allocation when the decoded estimate exceeds the configured budget. After
parsing, raw ParamsIPA and bootstrap payloads are released; the catalog retains
the parsed verifier and only the serialized verifying keys needed to build
governed activation records. Consensus never reads the filesystem. Wallets and
provers install all eight artifacts, including both
proving keys. Generated `dist/kagemusha/v4/*`, raw parameters, keys, device
logs, and signing inputs remain untracked runtime material.

A validator must be provisioned and restarted with a candidate before
`ActivateKagemushaRecursiveReleaseV4` is submitted. Activation authenticates
the release-policy digest, signed release and evidence, exact-eight inventory,
chain/asset/scale and future issuance window, distinct inline Eq/Ep verifier
records, matching local cached material, and the embedded production iOS and
Android device-attestation policy. The instruction requires both release-
activation and device-policy governance permissions, then publishes the exact
device policy, release, and Eq/Ep records in one consensus transaction overlay.
There is no independently reorderable or standalone release-activation path. A
validator missing non-revoked material for an already active release fails
before joining the voting set. Withdrawal ends new issuance and offline-change
creation, but retained material continues to verify and fully redeem previously
issued branches indefinitely. Later governed device-policy rotation remains a
separate operation and invalidates prior registrations, forcing re-registration.

## Production boundary

Admission is selected by the transaction's exact authenticated ABI-21/V4
release binding. Consensus must contain the release-qualified Eq/Ep records,
the immutable startup catalog must authenticate the same release, and the
production verifier must construct from that material. There is no process-wide
boolean admission shortcut.

Readiness preserves three independent facts. `proof_backend_available` reports
exact backend construction. `recursive_lineage_supported` is true only with a
non-null authenticated `artifact_set`, both distinct active V4 recursive
records, and that constructed backend. `ready` is true only when the complete
blocker set is empty; an unrelated issuer or transfer blocker can therefore
make `ready` false without erasing backend or lineage facts.
`recursive_lineage_unavailable` is present exactly when lineage is false.

`KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE` remains `false` in the
candidate change set. It may be changed only by the final signed promotion
commit after the authenticated review, benchmark, physical-device, and
role-threshold evidence has been added. Even after promotion it is not by
itself an admission or readiness signal: runtime capability stays fail-closed
until the exact authenticated ABI-21/V4 artifact set is installed, its live
inventory is revalidated, and both verifier and prover material construct
successfully.
Top-up and redemption change additionally require the governed selected
release's issuance window to be active. Full redemption authenticates the
parent release for its longer redemption lifetime, so a legitimately issued
note can still be redeemed after issuance closes.

## Release verification

Candidate mode requires the complete source, guard, SDK, and test corridor but
expects production availability to remain false; it does not invent external
evidence. Promotion mode additionally requires a clean signed candidate commit,
the authenticated release bundle, independent cryptographic review, measured
physical Android/iOS evidence within the signed ceilings, signed role-threshold
approval, and the production corridor. Any proof-code change after the
candidate commit invalidates that evidence and requires regeneration.

Physical-device evidence is collected before finalization with the separate,
off-by-default `kagemusha-candidate-evidence-lab` build. That build accepts only
the canonical clean unsigned candidate plus its exact ordered eight KRV4
artifacts and calls the same ABI-21 prover/verifier/recursion implementation.
Its symbols, registry, JNI class, marker-bearing native library, and APK are
distinct from production and are rejected by production packaging. The normal
artifact install and proof entrypoints remain unavailable, and device evidence
must record that production capability stayed false. Candidate-bound Android
evidence V2 hashes the candidate, manifest, source commit/tree, lab binaries,
each framed and payload artifact, the native-accepted inventory, and the exact
lifecycle transcript; V1/status-only evidence cannot be promoted.
The marker-bearing candidate-lab APK has its own path and digest in V2; it is
never relabelled as the separately attested wallet APK used for StrongBox,
rotation, rollback, and device-to-device transfer evidence.

Run the repository corridor without external evidence while preparing a
candidate:

```bash
ci/check_kagemusha_production_readiness.sh candidate
```

For promotion, provision the same canonical policy file used by validators and
the root containing lowercase manifest-digest directories. The corridor invokes
Kagami's typed verifier for every release; it authenticates the policy,
manifest, signed attestation, evidence, exact-eight artifacts, bootstrap
witnesses, and promotion record rather than trusting filenames or JSON alone:

```bash
KAGEMUSHA_V4_RELEASE_POLICY_PATH=/run/iroha/kagemusha/release-policy.norito \
KAGEMUSHA_V4_ARTIFACT_ROOT=/run/iroha/kagemusha/v4 \
  ci/check_kagemusha_production_readiness.sh promotion
```

The policy path is always an explicit runtime input. No build-time environment
variable or embedded policy selects a Kagemusha trust root.

The release driver funds four wallets with `10.75`, spends `6.25`, then `2.10`,
then `0.05` after a receiver restart, and redeems every remaining branch. It
asserts that the exact total remains `10.75`. The same driver covers the minimum
atomic unit, maximum supported precision, excess-precision rejection, full and
partial redemption, fees on and off, and hop depths 1, 2, 4, and 8.

Adversarial coverage includes request and proof tampering, replay, duplicate
delivery, lost acknowledgement, restart at every commit boundary, sibling and
ancestor double spend, artifact interruption and corruption, verifier rotation
and expiry, and network interdiction during every peer hop. Device release gates
measure readiness, proof creation, receive verification, QR/NFC end-to-end
latency, redemption finality, payload size, memory, thermal state, and repeated
lifecycle stability on the oldest supported device.
