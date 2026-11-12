# Monetary relation components

These modules implement component constraints for the future wallet generation.
They are not a complete Send or Receive verifier, and the recursive harness still
uses synthetic monetary transitions. No component exports an acceptance decision
for a wallet operation. Their returned cells must be joined to the same
authenticated predecessor, credential, statement, receipt and public lineage.

All components use the experiment's uniform wiring and the canonical
`iroha_pasta` Poseidon implementation. Byte lookups and Boolean constraints enforce
integer bounds; witness width annotations alone grant no bounds. Callers provide
the shared byte-range lookup table. Native relation sources are under
`crates/iroha_kagemusha_proof/src/operation_relation/`, `controls.rs` and
`witness.rs`; the future Core adds the approved prior-credential root at index 8.

## Implemented contracts

| Module | Constrained result | Required composition boundary |
| --- | --- | --- |
| `monetary_state` | Core30/Rest6 opening and both exact state hashes; lifecycle, integer widths, nonzero identities/roots, control masks and held policy/share consistency | Bind the commitment and public facts to the verified predecessor or successor. A well-formed state is not an authorized state. |
| `request` | Exact 418-byte completed body and 64 signature bytes; canonical words, nonzero identities, distinct wallets, policy pair, bounded amount plus fee, terms/object/credit hashes | Bind both account identities and wallets to the appropriate credentials and all monetary terms to the statement. Receive owns the Request signature and quoted-credential continuity; Send does not repeat that signature check. |
| `statement` | Native 26-field header/effect rules, exact padding and digest for a fixed operation kind | Join state commitments, identity, sequence and public lineage, then enforce the selected transition. The helper does not authorize any of the listed operation kinds. |
| `fee_schedule` | Exact held 191-byte body plus signature digest; presence derived from Rest6[2]; integer basis-point calculation, rounding, min/max clamp and pre-clamp overflow rejection; Request epoch/policy and fee joins | Authenticate the predecessor state and the earlier installation of its held schedule. This component does not repeat the issuer signature. Absence selects constrained canonical dummy bytes and requires zero fee. |
| `send_arithmetic` | Positive amount, u128 gross and affordability against adjusted burn; exact balance/burn, sequence, send ordinal and send-chain update; preservation of every unrelated Core/Rest field | Source adjusted burn from verified public lineage. Bind pending/fee/quota roots, time floor, nonce, credential and receipt through their mandatory owners. Active and Retiring states both may Send. |
| `send_controls` | Ordered u64 interval and time floor; lease/share expiry; quota response span; exact two four-position boundary segments, depth-6 window openings and four ordered usage updates; gross amount charged against every touched window | Authenticate the state and signed share installation, including sorted non-overlapping windows longer than the response bound. Bind interval to the Send statement and output usage root/floor to the successor. Native same-boot anchored-time derivation remains separate. |
| `indexed_map` | Canonical depth-1 through depth-32 search/insertion, strict integer ordering, authenticated predecessor bracket and empty insertion slot against the relinked intermediate root | Bind the root, key, value, presence and resulting root to authenticated state/effect sources. A returned leaf value belongs to the query only when `present = 1`. See [INDEXED_MAP.md](INDEXED_MAP.md). |

Send arithmetic checks `gross = amount + fee` and
`old_balance = gross + adjusted_burn + remaining_spendable` as bounded integers.
It sets `new_balance = old_balance - gross` and `new_burn = adjusted_burn`;
already burned value is not debited twice. This still requires the adjusted-burn
source to be the verified predecessor lineage rather than the old Core's stale
burn field or an arbitrary witness.

## Validation coverage

The component tests separate constraint checks from actual proof verification.
Constraint fixtures instantiate the complete lookup/copy relation and test all
rows; they are not merely host-side validators.

- State tests prove policy-present and policy-absent openings under identical
  matrices, bind both hashes and all state fields, and reject width/root/control
  inconsistencies.
- Request tests pin native golden terms/object/credit hashes and every exported
  port. They cover completed versus open Requests, payer-completion effects,
  field aliases, integer overflow, exact tape lengths and raw signature ownership.
  The Request is also composed into the fee and arithmetic proof tests.
- Statement tests pin native Send/Receive vectors, mutate every exposed field,
  check exact effect padding and cover Bootstrap, Load, Unload, refresh and
  retirement field rules. These are constraint tests, not complete operation
  proofs or implementations of those transitions.
- Held-fee tests compose real state and Request relations, and prove present,
  absent and changed schedule witnesses under one matrix. Adversarial cases
  include overflow hidden by clamping, wrong digest/identity, fees and policy
  epochs.
- Send arithmetic tests compose real state and Request relations, and prove both
  Active and Retiring witnesses. They compare matrices and reject tampered public
  heads/burn, overdrafts, double burn subtraction, ordinal/sequence overflow,
  incorrect send-chain updates and changes to preserved fields.
- Quota/time tests use an isolated source-port harness. They cover
  native padding vectors, time edges, one/two touched windows, ordered root
  updates, forged paths/slots, skipped charges, limits and disabled-quota root
  preservation. A proof test covers all four control masks and a two-window
  boundary crossing under one matrix, with public root/charge-bit tampering.
  These fixtures do not compose the real state and Request relations,
  independently authenticate a signed share or provide an end-to-end proof
  through the recursive Send circuit.
- Indexed-map tests include real proofs for insertion and present/absent search,
  identical matrices across valid witnesses, the native depth-32 golden vector,
  full-width ordering and index-bit checks, and forged routes/paths.

All 126 whole-suite tests passed, including the additional native
fee golden and controls proof tests, with unchanged experiment source hashes. The
source-bound run evidence is recorded in [README.md](README.md).
Component tests do not substitute for the
final recursive chain, full Payment encoding or physical-device measurements.

## Work still required

The next operation owner must compose all ports before exposing a complete Send.
That includes credential/account/payment-key continuity, exact statement and
receipt binding, the verified predecessor's adjusted pending root, pending
descriptor insertion, conditional fee-claim insertion with a constraint-derived
zero-fee gate, controls, nonce and public lineage outputs. A host-selected branch
or separately valid collection of component proofs is insufficient.

Receive still needs its full incoming-payer and receiver Request/credential
joins, signature obligations, replay/consumed-credit checks, pending and lineage
reconciliation, exact value conservation and state/receipt update. The complete
protocol then needs strict wire decoding and native final-accumulator acceptance,
authenticated circuit/generator identities, custody randomness and native/SDK
integration for a deliberately selected future generation. The owner-approved
12,000-byte limit applies to the complete serialized Payment with all separately
transported context and companion data. No complete fit or phone-performance
result is established by these components.
