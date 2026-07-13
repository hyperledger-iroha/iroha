# Kagemusha offline cash

Kagemusha is the single offline-cash protocol in the first release. It supports
exact decimal amounts, sender change, offline multihop
spending, and full or partial online redemption. There is no runtime product
mode or alternative offline API. V2 request names, V3 manifests, and bridge ABI
19 are internal wire and artifact versions. The manifest and native capability
schemas have no `mode` field; schema/version, backend, transcript, and circuit
identities pin the exact cryptographic contract.

## Amounts and assets

Every request binds the chain id, asset definition, authoritative asset scale,
and an unsigned `u128` atomic-unit amount. The scale is read from the live asset
definition. Decimal conversion is exact: excess precision, negative values,
zero payments, and overflow are rejected. Top-up debit, note conservation, and
redemption credit use the same scaled `Numeric` value.

A spend consumes one note and creates one recipient output plus optional
sender change. The transition proves, and every verifier rechecks:

```text
sum(inputs) = recipient + change
```

The first-release contract accepts exactly one parent. It exposes no
multi-parent merge mode; fragmented branches remain independently spendable
or redeemable and are never combined by host-side hashing.

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
generation agree.

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
with `initSpend`. The note is not available for offline use until both the chain
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

The receiver runs `verifySpend` and checks the signed request, chain, asset,
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

The authenticated V3 manifest binds source commit, chain, asset, scale,
activation and withdrawal heights, bridge ABI 19, proof size, transcript,
backend, and benchmark evidence. It contains exactly two Pasta-cycle profiles
(transition and state), each with one parameters file, proving key, and
verifying key, plus the content-addressed top-up-finality roster artifact.
Every file has an exact size and SHA-256. Installation streams to private files,
supports resume, verifies every digest, and atomically activates the complete
generation. Offline operations use only an already installed generation.

## Production boundary

Production availability is the conjunction of the wallet release artifact,
Torii readiness, active verifier records, authenticated V3 artifact set, and the
native proof backend. The following compile-time capabilities remain false
until their cryptographic implementations and signed artifacts pass the release
suite:

- `KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE`
- `KAGEMUSHA_RECURSIVE_SPEND_AUTHENTICATED_RELEASE_ENVELOPE_WIRED_V3`
- `KAGEMUSHA_RECURSIVE_SPEND_INIT_BINDS_TOPUP_FINALITY_V2`

These flags are safety gates, not development fallbacks. They must not be
enabled by configuration, mocked receipts, host-only equality checks, or
unverified artifacts.

## Release verification

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
