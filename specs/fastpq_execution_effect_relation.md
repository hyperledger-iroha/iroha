# Complete-entry quantity effect relation

The nominal `FastpqExecutionEffectStatementV1` candidate is not accepted by any
production artifact dispatcher. Its public preparation and local materializer
cover ordered Transfer, Mint and Burn quantity effects. Ordinary capture,
source-finality admission and succinct proof-profile cutover remain incomplete.
Existing AXT transfer-claim policy is unchanged.

## Identity and authority

Each balance includes the exact domainless AccountId, AssetDefinitionId,
AssetBalanceScope and existing `AxtAssetIncarnationV1`. A supply key contains the
same definition and incarnation, with a distinct enum tag. Both use the explicit
`iroha:fastpq:execution-quantity-key:v1\0` prefix followed by the canonical nominal
Norito key frame. The SMT key hash has the additional
`fastpq:execution-effects:v1:key|` domain. These keys cannot substitute for the
predecessor transfer-balance layout or omit a scope/incarnation.

Core already installs the incarnation on registration and removes it on
unregistration. A future ordinary recorder must read the actual current registry
token at each effect, including a preceding same-transaction registration. It
must refuse absent/corrupt tokens. The proof relation does not derive lifecycle
tokens or infer registration from balance gaps. Each distinct incarnation has
separate quantity leaves; authentic execution determines which one was live.

Each effect retains its actual authority-set commitment and operation-specific
authorization context, as well as contiguous execution ordinal. The complete tape
includes network, height, original call/protocol identity, lane incarnation/route
and dataspace. Hashing or decoding those fields does not authorize them. Public
preparation requires independently expected complete-tape and complete-statement
commitments plus exact public inputs. Mint authority is not inferred from the
permission table root or from possession of a sender balance.

## Arithmetic and chronology

Preparation chooses one common scale per exact asset/incarnation from every
original amount, balance and supply quantity. It retains all 19 normalized limbs
and canonical V1 quantity frames. It never narrows to u64 or rounds decimals.

| Effect | Port 0 | Port 1 | Checked arithmetic |
| --- | --- | --- | --- |
| Transfer | Sender balance | Receiver balance | sender after = before - amount; receiver after = before + amount |
| Mint | Affected balance | Definition supply | both after = before + amount; amount is positive |
| Burn | Affected balance | Definition supply | both after = before - amount |

Checked arithmetic rejects underflow, overflow and nonrepresentable ledger
quantities. Affected balance cannot exceed supply. Zero transfers and zero burns
remain explicit effects; a self-transfer uses the real intermediate debit value
before credit. The last value of every exact key must equal its next original
pre-value across every effect kind. No placeholder write, inferred delta or gap
is inserted. Thus transfer→mint/burn→transfer works only with the original
intervening balance and supply facts; removing that effect fails chronology.

The canonical row table is recomputed, stably sorted by full key and operation
rank, and compared byte-for-byte with the offered table. Every typed effect owns
exactly two ports. Pair ordinals preserve original chronology independently of
canonical row sorting, including repeated zero rows and mixed operation tags.
The ordering commitment has its own `fastpq:execution-effects:v1:ordering|`
domain over the complete canonical model transition vector.

## Shared SMT relation and bounds

The existing compact SMT AIR takes two sequential generic leaf updates and their
paths/root endpoints. It contains no debit/credit quantity equation or authority
predicate. Public preparation supplies those typed semantics before the common
private materializer. The shared crate-private `CheckedUpdateTable` adapter
retains exact occurrence triples, leg order, key allocation and all row ports;
it cannot be constructed through the external public API. There is one tree
implementation for both existing transfer preparation and this candidate.

No AIR dimensions, hash domains inside the tree, challenge transcript, backend,
or deterministic acceleration path changes. The materializer derives roots only
for this entry's touched typed quantity leaves, not a full-state root or finality.
Public preparation reads no private siblings and builds no SMT/trace/FFT/LDE.
Effects, rows, canonical public bytes, unique keys and allocation work have
explicit limits. Private construction independently bounds updates, occupied
nodes, siblings and node hashes.

## Remaining cutover obligations

- Capture complete committed quantity effects atomically with actual balance and
  supply changes, authorization and lifecycle context. Preserve rollback,
  transaction rejection tails, native protocol effects and execution-output
  source ownership; do not enable a partial recorder.
- Cover all supported balance/supply mutation owners, including confidential,
  privacy, reserve, escrow, registration/unregistration and protocol paths, or
  establish an exact authorized source projection for non-quantity obligations.
- Replace ordinary transfer-only source framing, counters/reservations, archive
  leaves and proof statement coherently. Source accounting alone must continue
  to accept supported transfer→mint/burn→transfer execution before cutover.
- Authenticate complete statement expectations using source-finality evidence
  and the exact ordinary-writes commitment, with network/height/route/incarnation
  and execution-entry identity. Neither these local roots nor a test-only D7
  helper establish that authentication.
- Bind the new complete statement, operation semantics and roots into the reviewed
  succinct profile, finality checks, storage/transport and bounded verifier.
  Preserve the separate AXT policy. Do not accept both retired and replacement
  ordinary artifact layouts as shipping fallback paths.
- Run model/schema, effect preparation/materialization, existing transfer parity,
  Core chronology/source ownership and end-to-end proof/tamper tests. Regenerate
  canonical reference schema through the repository generator after integration.

The September 26 retained Linux model/prover executables pass all 62 selected
checks: three model tests and 59 shared transfer/effect preparation tests, including
five complete-effect cases. They cover roundtrips and domain separation, mint/burn
interleaving, supply chains, omitted/reordered effects, authority/context changes,
operation tags/rows, high-limb/fractional quantities, zero/self updates, exact bounds
and distinct scope/incarnation keys. The compile exits zero and captured inputs stay
unchanged; its snapshot omitted a vendored dependency, so this is scoped executable
evidence rather than full dependency-closure qualification. The schema owner passes
23 tests; canonical generation adds the eleven effect entries and preserves every
existing entry. Its original generator link failure and separate successful recovery
remain recorded. Core capture qualification remains pending; see the
[dated checkpoint](../docs/history/2026-09-26/fastpq-deep-integration.md).
