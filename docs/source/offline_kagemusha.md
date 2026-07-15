# Kagemusha offline cash

Kagemusha is the single offline-cash protocol in the first release. It supports
exact decimal amounts, sender change, offline multihop
spending, and full or partial online redemption. There is no runtime product
mode or alternative offline API. ABI 20 and the V4 chain, recursive, and
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
redemption credit use the same scaled `Numeric` value.

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
`kagemusha-recursive-spend-step-eq-authenticated-layout-v4` and
`kagemusha_recursive_step_ep_v4_verifier_record` with circuit
`kagemusha-recursive-spend-step-ep-authenticated-layout-v4`.

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
with the ABI-20 SDK's `initSpendV4`. The note is not available for offline use until both the chain
operation and local encrypted-state transition are durable.

## Offline transfer

The receiver creates a nonce-bound payment request containing its output
commitment, exact amount, asset, scale, verifier generation, and expiry. The
sender authenticates, reserves its selected inputs, creates recipient and
optional change outputs, proves the transition, verifies the result locally,
and durably stages the outgoing payment and local change.

The peer payload contains the recipient's opaque proof bundle and the exact
proof-bound, secret-free membership witness required for its next spend. Replay
identity remains derived only from the recipient bundle's authenticated split
transition. The payload never carries a spend key, sender change, or local key
reference.

The receiver runs `verifySpendV4` and checks the signed request, chain, asset,
scale, exact amount, recipient commitment, hop limit, verifier activation
window, finalized top-up origin, recursive proof validity, and branch
disjointness. It atomically persists the received note before signing a durable
acknowledgement. The sender marks reserved inputs spent only after verifying
that acknowledgement. Duplicate delivery and lost acknowledgements are
idempotent across transport loss and process restart.

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
then consumes the branch nullifier, credits the exact public `Numeric`, appends
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
activation and withdrawal heights, exact bridge ABI 20, proof size, transcript,
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
Swift, Kotlin, and Java release-authentication inputs therefore require that
promotion record alongside the trusted policy, attestation, benchmark evidence,
and review. A partial, unpromoted, or role-substituted generation never becomes
active.

## Validator provisioning and activation

Operators configure both optional paths together under `settlement.offline`:
`kagemusha_release_policy_path` names the canonical Norito trust policy, and
`kagemusha_artifact_dir` names the directory whose children are manifest
digests. Leaving both unset is valid and keeps Kagemusha readiness false while
the node otherwise operates normally. Supplying only one path, an empty path,
a malformed policy, an invalid release directory, or corrupt artifact material
is a startup error.

Startup authenticates every candidate subdirectory, validates framed and
payload sizes and SHA-256 values, parses the six validator-side artifacts
(ParamsIPA, verifying key, and bootstrap witness for Eq and Ep), and builds an
immutable catalog keyed by manifest digest. Consensus never reads the
filesystem. Wallets and provers install all eight artifacts, including both
proving keys. Generated `dist/kagemusha/v4/*`, raw parameters, keys, device
logs, and signing inputs remain untracked runtime material.

A validator must be provisioned and restarted with a candidate before
`ActivateKagemushaRecursiveReleaseV4` is submitted. Activation authenticates
the policy digest, signed release and evidence, exact-eight inventory,
chain/asset/scale and future issuance window, distinct inline Eq/Ep verifier
records, and matching local cached material before writing consensus state
atomically. A validator missing non-revoked material for an already active
release fails before joining the voting set. Withdrawal ends new issuance and
offline-change creation, but retained material continues to verify and fully
redeem previously issued branches indefinitely.

## Production boundary

Admission is selected by the transaction's exact authenticated ABI-20/V4
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
until the exact authenticated ABI-20/V4 artifact set is installed, its live
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
artifacts and calls the same ABI-20 prover/verifier/recursion implementation.
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
