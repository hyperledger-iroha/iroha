# ZK delivery plan revision 7

Status: proposed implementation plan. Prepared 2026-10-04 against source
`f94d3d9724f97ff3d579e2ca66ae3239f15b2850`. No implementation or release outcome is marked complete.

This plan turns the supplied revision 6 review into executable work while preserving
the decisions to expose native ISIs through IVM/Kotodama, provide one-call SDK
operations, remove activation/review gates, avoid an OPRF-only replacement for
RAM-LFE, and give each production role one implementation owner.

The original revision 6 document, graph and linked findings file were unavailable.
This is a reconstructed revision with a new 52-task graph, not a verified edit of
the original 52 tasks. It covers all problems described in the supplied review and
the current source paths inspected for them. It cannot claim to preserve unseen
requirements. F.1 owns reconciliation if the original source becomes available;
unrelated work can begin immediately.

The accompanying [graph](zk_delivery_graph.json) records owners, dependencies,
outputs and acceptance criteria. These are implementation deliverables and evidence
requirements, not runtime activation conditions. Existing
[ZK goals](zk_first_release_goals.md), [privacy closure](privacy_first_release_closure.md)
and [IVM goals](kotodama_ivm_completion.md) describe current implementation and
remaining technical obligations. Where those records prescribe activation or
review prerequisites, F.4/P.5 explicitly replace that target behavior and update
the affected text with implementation. Existing failures remain failures.

## Decisions and boundaries

Every shipped native ledger ISI must be callable through IVM/Kotodama under the
same canonical executor and applicable permissions. Calling an administrative
instruction does not confer the authority to execute it. Proof preparation, phone
challenge interaction and distributed key ceremonies remain off-chain work
orchestrated by SDKs/services; the ledger verifies their canonical statements and
applies their authorized effects. There is no second contract-specific verifier.

Ordinary transaction validity depends on valid cryptography, authenticated
configuration and keys, permissions, finality, revocation, replay protection,
conservation and deterministic resource bounds. It does not depend on a reviewer
signature, qualification receipt, release checklist, staffing milestone or
administrative activation bit. Removing these gates does not make malformed
proofs, absent implementations or known-insecure encryption valid. Until a real
implementation exists, its operation has an accurate unavailable/unsupported
error and remains unfinished work. Independent review proceeds alongside
implementation and produces findings, not permission tokens.

IVM remains the sole VM and ABI V1 remains the sole ABI. Norito owns binary and
JSON contracts. No Wasm/WASI, compatibility decoder, retired backend alias or
parallel production implementation is introduced. Accelerated implementations
must preserve deterministic results and a correct scalar fallback.

Two new design proposals make the delivery work concrete:

- Use fresh, beneficiary-bound phone-control credentials from an explicitly
  identified carrier or verification provider. This is an external trust choice,
  not a fact established by the current code or the original excerpt.
- Deliver maximum X509 proofs in canonical transactions and blocks with a proposed
  16 MiB block payload default and coordinated availability/transport parameters.
  This changes the proposed network payload budget, not the fixed proof-size or
  prover performance targets.

These proposals are part of the plan for review; this document does not modify
network configuration or establish a provider relationship.

## Native ISI and SDK contract

One canonical protocol catalog maps an operation to its native ISI, proof relation,
state transition, permissions, resource contract and SDK/Kotodama bindings. The
existing twelve-protocol envelope is the starting point. Protocol-specific builders
may provide useful types but must delegate to that owner rather than create
another authoritative submission route.

An ordinary public SDK operation performs prepare, local prove, configured signing,
submission and finality tracking in one call. A configured account/signer and
network context may be created once. Applications must not manually select
transcript internals, supply dummy witnesses or assemble multiple public API stages.
Interactive signing and phone challenges use callbacks within the operation.
External proving requires explicit configuration of its witness and trust boundary;
private witnesses are never silently uploaded.

A proof-operation batch is one signed, atomic transaction with ordered actions.
Success means verified finality. A batch that exceeds transaction, VM or work limits
fails before submission and, where its size is predictable, before expensive
proving. It is never silently split. A timeout after submission yields a recoverable
pending transaction identity rather than a false failure or a newly signed retry.
Cancellation distinguishes local work that can stop from a submitted operation whose
final outcome must still be recovered.

Multi-round DKG and service setup use separately named workflow APIs. One workflow
call may span transactions, exposes durable progress and partial completion, and
can resume after interruption. It does not promise atomicity across those
transactions. It does not allow ordinary proof batches to evade the atomic contract.

The supported SDK inventory is one list: Rust; Kotlin/JVM with Java consumers and
Android adapters; Swift; JavaScript; Python; and C#. Java consumes Kotlin-owned
implementation. SDK fixtures, catalogs, examples and documentation are generated
or checked against this inventory. Supported environments remain explicit, including
native-only proof capabilities and actual Apple/Windows/Android qualification.
There is no browser Wasm proving path.

The intent format must bind the complete canonical unsigned operation and each
ordered action without circular dependence on generated proofs. Contract execution
consumes signed predeclared proof actions exactly once in their authorized
invocation scope. Unused, changed, duplicated or undeclared actions abort the
transaction. Native, contract and SDK routes use the same intent algorithm and
preserve atomic rollback.

## Phone claims and RAM-LFE

The phone relation proves three distinct facts: canonical number representation,
uniqueness of its active binding, and current control of that number. A keyed
nullifier, encryption or E.164 syntax cannot establish real-world control.

The proposed credential provider confirms a fresh challenge and issues evidence
bound to the beneficiary AccountId, network, policy, nonce and validity interval.
The proof connects that evidence to the same canonical number used in encrypted
evaluation and the stable identifier. The provider learns the number; the resolver
and ledger should not learn the plaintext through this flow. The threat model must
also account for malicious issuers, SIM/account takeover, low-entropy enumeration
and number reassignment. Calling this evidence legal ownership would be inaccurate.

R.1 must name the issuer policy and credential verification mechanism before R.2
can be implemented. If the intended attestor-free policy also forbids every external
source of phone-control evidence, the phone product has an unresolved trust
requirement; cryptography over a bare number does not resolve it. This is recorded
as a scoped design dependency, not permission to insert an audit/activation gate
or claim that an unauthenticated registration is usable.

Claims have a mandatory bounded lifetime, exclusive live binding, authorized
revocation, renewal and a defined reassignment path. Preserve existing expiry and
revocation behavior. Use the earliest applicable evidence expiry. Define how
nullifier/key rotation preserves global uniqueness and how stale lookups fail.
The one-call SDK handles challenge interaction and ambiguous submission recovery.

RAM-LFE must implement secure encrypted program execution and the complete
execution relation. Plaintext keyed evaluation and an OPRF-only substitute do not
meet that requirement. Retire the HKDF RAM-LFE backend coherently across actual
consumers, schema tags, codecs and fixtures. Retain legitimate HKDF key derivation
in unrelated roles.

## Shared FHE and SoraCloud

The proposed `iroha_fhe` crate establishes a real low-level boundary: validated RNS
representations, modular polynomial arithmetic, NTT, basis conversion and explicit
rounding. Protocol-specific encryption/noise semantics, transcript construction,
BFV versus BGV/MKHE behavior and key ceremonies remain owned by their protocols.
A shared arithmetic owner does not mean merging mathematically different protocols.

Inventory production paths separately from test-only reference implementations.
Independent small test oracles are useful and need not be erased as duplicate
production implementations. Move genuine shared production primitives and remove
their superseded owners; update workspace membership and the lockfile together.

SoraCloud FHE bootstrap depends on shared arithmetic and a secure bootstrap/profile
construction. SoraCloud threshold services depend on the shared threshold
construction. Neither depends on RAM-LFE phone or application delivery, and
threshold delivery does not wait for unrelated bootstrap work. ZK-AMS consumes
shared arithmetic while completing its own actual-source, composite and qPCS work.

## Economic correctness under the no-gate model

Each value-bearing protocol gets a matrix of its issuance authority, private
conservation relation, public reserve boundary, nullifiers, asset isolation and
cross-pool effects. Separate three failure consequences: fraudulent private
balances, theft of existing reserves, and creation of transparent supply.

For public bridges, debit an exact governed reserve before crediting the equal
withdrawal; deposits transfer existing assets. Reject underflow, wrong asset,
wrong namespace and bypasses through other transfer routes. Independently
authorized issuance consumes a committed allowance. Any supply cap is a
transaction rule sourced from genesis/committed state, not an operator environment
variable or an activation switch. A cap must not be silently added to an asset
whose issuance policy does not define one.

Private IVM already has reserve-backed transfers; preserve and test them. Current
FCMP++/PQ-MASP effects must not be described as having a public bridge they do not
have. If the new product adds one, its reserve/issuance semantics are part of that
delivery. Settlement must preserve balanced authorized effects and complete
source binding.

A reserve limit can bound transparent withdrawals while fraudulent private notes
remain possible if a relation is broken. State the residual exposure. There is
no generic independent public counter that automatically proves hidden
conservation without a specified privacy-compatible construction.

Fault-injection tests may simulate erroneous verifier acceptance to test independent
ledger controls, but only inside test code. Consensus faults use the deterministic
simulator. No production bypass, empty-block behavior or node toggle is introduced.

## X509 proof delivery and freshness

Keep the proof ceiling at 9,437,184 bytes. The inspected source reports a 9,412,944-byte
maximum proof and a 4 MiB default block payload limit. The current transaction
wire default is 10 MiB. A proof fitting its proof budget therefore does not yet
establish a usable transaction or block delivery.

Select the canonical signed-transaction and `SignedBlockWire` path. X.2 targets
16 MiB block payloads and explicitly coordinates committed chain parameters,
RS16 availability geometry, transport/frame overhead, block sync, persistence,
proposer admission and deterministic verification budgets. Preserve the 10 MiB
transaction limit only if the complete supported maximum envelope fits with its
signatures and framing. If it does not, record and resolve the envelope/parameter
choice before claiming delivery. SDKs reject oversized atomic batches.

All proof bytes remain available through the mandatory authenticated block/payload
path. A hash-only reference or new unauthenticated external fetch is not a delivery
shortcut. A different transport would require a separate specified availability,
verification and replay contract and is outside this selected approach.

X.3 retains the complete supported shape, 300-second proving target, 12 GiB RSS
budget and 32 GiB enforced address-space budget. The inspected maximum took
1,437.185689 seconds; it does not meet the proving target. Increasing a block
payload budget cannot fix that failure. Record ordinary/maximal proof generation,
verification, resource and privacy evidence independently.

Presentation bounds must satisfy:

```text
presentation_start >= max(certificate.notBefore for the entire chain)
presentation_end   <= min(certificate.notAfter  for the entire chain)
presentation_end   <  CRL.nextUpdate for every applicable CRL
```

Retain other existing CRL validity/revocation predicates. Test the limiting
certificate at every chain position and exact boundary behavior. An erroneous
latest-expiry rule is security-relevant.

X.4 is the usable maximum-proof delivery: four-validator submission, finality,
canonical persistence, restart and independent verification of the same statement
and proof. Record proof, transaction and block byte counts separately.

## Hash selection, measurement and anchored execution

H.1 identifies the WG workload and its scope; the excerpt does not provide its
definition. Do not invent an expansion or claim that an unspecified workload was
measured. Exploratory candidate measurements may inform selection. Freeze the
selected algorithm, security parameters, workload and measurement procedure in
H.2 before final candidate measurements in H.3. Retain current protocol-specific
hashes unless an explicitly justified change is selected. Bind source, profile,
artifacts and configuration to every result. Material changes invalidate affected
measurements.

Ordinary execution and AXT require full native relations and authoritative finalized
source state. Neither caller-supplied commitments nor binding-only proofs establish
execution or spend authority. Anchored AXT must provide native/SDK execution,
permission and replay protection, atomic failure and durable terminal recovery.

Delete the superseded driver only after anchored AXT and replacement caller
coverage exist. Its deletion does not depend on a global review or staffing phase.
The final implementation keeps one production driver and no compatibility shim.

## Delivery graph and work ownership

An edge means a necessary technical output is consumed before the dependent task
can complete. Design/prototyping can proceed against a specified interface earlier.
Owners name responsible disciplines; staffing assignment is not an execution gate.
There is no global S.0 or W0 prerequisite.

Start immediately: **F.1, F.2, F.3, F.4, C.1, R.1, X.1, H.1, V.1**.
These are exactly the roots of the graph. In particular, resource design, monetary
invariants, ownership mapping, phone trust, X509 freshness and measurement
specification can proceed in parallel.

Every task below has a path to a named usable delivery or evidence record. Review
and measurement remain visible tasks without becoming transaction-admission
dependencies. Task counts are bookkeeping, not a measure of plan quality.

| ID | Task | Owner | Requires |
| --- | --- | --- | --- |
| F.1 | Canonical operation and consumer inventory | Data model and protocol owners | — |
| F.2 | Unified end-to-end resource contract | Consensus, IVM and configuration | — |
| F.3 | Protocol economic invariant matrix | Ledger economics and protocol owners | — |
| F.4 | Non-gating runtime and document contract | Core, SDK and documentation owners | — |
| I.1 | Ordered multi-action transaction intent | Data model and executor | F.1 |
| I.2 | Signed contract proof declarations | Executor and IVM host | I.1 |
| I.3 | Canonical native ISI bridge | IVM and Core executor | I.2, F.2 |
| I.4 | Deterministic bridge charging and rollback | IVM and transaction runtime | I.3, F.2 |
| I.5 | ABI V1 and typed Kotodama surface | IVM ABI and Kotodama | I.3, I.4 |
| I.6 | Atomic native and contract batch delivery | Core integration | I.5, P.1, P.5 |
| C.1 | FHE and plaintext consumer ownership map | Crypto and ZK-AMS owners | — |
| C.2 | Shared deterministic FHE arithmetic | Crypto and acceleration owners | C.1 |
| C.3 | Secure encrypted computation profiles | FHE cryptography owners | C.1, F.2 |
| C.4 | Threshold key and share protocol | Threshold cryptography and custody | C.2, C.3 |
| C.5 | ZK-AMS production relation and arithmetic adoption | ZK-AMS and qPCS owners | C.2, C.4, H.2, F.2 |
| C.6 | SoraCloud FHE bootstrap delivery | SoraCloud and FHE owners | C.2, C.3, F.2 |
| C.7 | SoraCloud threshold computation delivery | SoraCloud and threshold owners | C.4, F.2 |
| R.1 | Phone-control trust and lifecycle contract | Identity and SDK owners | — |
| R.2 | Private identifier and ownership relation | Identity cryptography | R.1, C.3, H.2 |
| R.3 | Complete RAM-LFE program relation | RAM-LFE and FHE owners | C.2, C.3, H.2, F.2 |
| R.4 | Identifier claim state transition | Core identity | R.2, R.3, F.3, F.4 |
| R.5 | One-call private phone delivery | Identity, SDK and integration | R.4, I.6, S.1 |
| R.6 | Retire the plaintext RAM-LFE backend | Crypto, schema and SDK owners | C.1, R.5 |
| P.1 | Single Exact12 native dispatch owner | Privacy model and Core | F.1 |
| P.2 | Independent public monetary invariants | Core assets and privacy | F.3, P.1 |
| P.3 | Private conservation and proof relations | Value-bearing privacy protocol owners | F.3, P.1, H.2 |
| P.4 | Credential and authorization relation completion | Credential and authorization owners | P.1, H.2, F.2 |
| P.5 | Remove activation and review predicates | Core, Torii and SDK | F.4, P.1 |
| P.6 | Exact12 lifecycle and economic integration | Privacy integration | P.2, P.3, P.4, P.5, C.5, X.3 |
| X.1 | Certificate and CRL interval semantics | X509 relation and SDK | — |
| X.2 | Canonical maximum-proof delivery parameters | Consensus, configuration and X509 | F.2, P.1 |
| X.3 | Complete maximum X509 construction | X509 cryptography and performance | X.1, H.2, F.2 |
| X.4 | Maximum X509 finality and restart delivery | X509 and four-peer integration | X.2, X.3, I.5, S.1, V.1 |
| H.1 | Hash and WG measurement specification | Cryptographic profile and benchmark owners | — |
| H.2 | Freeze candidate algorithm and profile | Cryptographic profile owners | H.1 |
| H.3 | Measure the frozen candidate | Performance and hardware owners | H.2 |
| A.1 | Finalized source and settlement binding | FASTPQ, AXT and State owners | F.1, F.3 |
| A.2 | Complete ordinary execution relation | IVM and FASTPQ proof owners | A.1, H.2, F.2 |
| A.3 | Anchored AXT native delivery | AXT, Core and integration | A.2, I.5, P.2 |
| A.4 | Retire the superseded driver | Runtime, SDK and packaging owners | A.3, V.2 |
| S.1 | One-call atomic SDK core | Rust client and shared native owners | F.1, I.1, F.2 |
| S.2 | Durable multi-round workflow APIs | SDK and service owners | F.1 |
| S.3 | Kotlin Java and Android delivery | Kotlin SDK and Android owners | S.1, S.2, I.5 |
| S.4 | Swift and C sharp delivery | Swift, C sharp and bridge owners | S.1, S.2, I.5 |
| S.5 | JavaScript and Python delivery | JavaScript, Python and native owners | S.1, S.2, I.5 |
| S.6 | Complete SDK conformance delivery | All SDK and integration owners | S.3, S.4, S.5, P.6, X.4, R.5, C.6, C.7, A.3 |
| V.1 | Plan and graph integrity checks | Planning and tooling owners | — |
| V.2 | Four-peer execution and recovery coverage | Integration and runtime owners | I.6, S.1, P.5, A.3 |
| V.3 | Complete current-candidate proof evidence | Protocol testing owners | P.6, C.5, X.3, A.2, H.2 |
| V.4 | Independent security and hardware evidence | Independent reviewers and platform owners | V.3, H.3, C.2 |
| V.5 | Integrated product and retirement evidence | SDK, runtime and release owners | S.6, V.2, X.4, R.6, A.4 |
| V.6 | Candidate evidence and documentation reconciliation | Release and documentation owners | V.3, V.4, V.5 |

| ID | Delivery | Requires |
| --- | --- | --- |
| D.ISI | Atomic native ISI and Kotodama path | I.6, P.5 |
| D.CLOUD | SoraCloud bootstrap and threshold services | C.6, C.7, S.2 |
| D.PHONE | Verified private phone claims and backend retirement | R.5, R.6 |
| D.X509 | Maximum X509 proof through finality and restart | X.4 |
| D.AXT | Anchored AXT and superseded driver removal | A.4 |
| D.PROOFS | All twelve protocols and coupled proof evidence | V.3 |
| D.SDK | Complete retained SDK contract | S.6 |
| D.EVIDENCE | Combined candidate evidence and remaining gaps | V.6 |

## Task contracts

These contracts are rendered from the graph. Update both artifacts together;
the checker detects table, deliverable and acceptance drift.

### F.1 Canonical operation and consumer inventory

Deliverable: Map every Exact12 protocol and the RAM-LFE, SoraCloud, settlement and AXT operations to one native ISI, verifier, state owner, permission, Kotodama binding and supported SDK entrypoint.

Acceptance: No operation or existing consumer is silently omitted. Identify retired routes and test-only references. Reconcile the unavailable r6 identifiers if that source is recovered; do not claim the original 52 tasks were preserved.

### F.2 Unified end-to-end resource contract

Deliverable: Define proof, transaction, block, RS16, transport, VM memory, scratch, queued effects and deterministic verification-work budgets including Norito overhead.

Acceptance: For every accepted operation identify an actual path through every limit. Boundary and one-over cases agree across admission, proposer, follower and VM. Wall-clock time never determines consensus validity.

### F.3 Protocol economic invariant matrix

Deliverable: Classify private issuance, transparent supply changes, deposits, withdrawals, reserves, settlement and inter-pool movement for every value-bearing protocol.

Acceptance: Name the independently checked invariant, authorized issuance source, remaining exposure after erroneous proof acceptance, and mutation test for each economic effect. A reserve cap is never labeled proof soundness.

### F.4 Non-gating runtime and document contract

Deliverable: Separate protocol validity and authenticated configuration from review status, qualification receipts and administrative activation bits.

Acceptance: Specify removal of review/activation prerequisites from runtime availability and SDK calls. Preserve signatures, permissions, valid keys, revocation, finality, replay and resource checks; reconcile contradictory current documents when implementation changes.

### I.1 Ordered multi-action transaction intent

Deliverable: Replace exactly-one privacy submission with one canonical ordered multi-action intent and shared Norito vectors.

Acceptance: Bind network, authority, complete unsigned payload and per-action index, protocol and statement without circular proof hashes. Reject reordering, substitution, omission and replay. Retire superseded first-release layouts without fallback decoders.

### I.2 Signed contract proof declarations

Deliverable: Allow contracts to consume signed predeclared proof actions through the same transaction-intent contract.

Acceptance: An action matches its declaration and invocation scope and is consumed once; undeclared, changed, duplicated and unconsumed declarations fail atomically. Cover conditional calls, nested contracts and mixed ordinary instructions.

### I.3 Canonical native ISI bridge

Deliverable: Route canonical InstructionBox calls from IVM to the existing native executor for the complete declared ISI surface.

Acceptance: Authorized native and contract execution produce equivalent permitted effects. Preserve contract subject and delegated permissions without granting signer powers implicitly; reject malformed payloads and unauthorized administrative operations.

### I.4 Deterministic bridge charging and rollback

Deliverable: Account for decode, proof verification, storage, temporary memory and queued effects across all supported pointer sources.

Acceptance: Maximum valid heap, input and literal paths execute within measured bounds; invalid and over-budget requests fail before effects commit. Failure in a later action rolls back all earlier ledger effects and charges follow the canonical fee policy.

### I.5 ABI V1 and typed Kotodama surface

Deliverable: Publish typed calls and update argument descriptors, ABI hash, manifests and relevant goldens for the expanded bridge.

Acceptance: Remain ABI V1. Change number-list goldens only when numbers change. Compile and execute representative bindings for all operation families, with unknown numbers and unsupported pointer types rejected deterministically.

### I.6 Atomic native and contract batch delivery

Deliverable: Deliver direct and Kotodama transactions containing multiple privacy actions through one atomic executor.

Acceptance: Demonstrate genuine supported proofs, cross-action binding, ordinary-instruction composition, duplicate rejection and later-action rollback. This component delivery does not claim all twelve engines or platforms are qualified.

### C.1 FHE and plaintext consumer ownership map

Deliverable: Inventory production RNS/NTT owners, BFV/BGV protocol logic, test oracles, HKDF RAM-LFE users and all generated consumers.

Acceptance: Assign each reusable primitive one lowest-layer owner. Record genuine plaintext PRF use separately from encrypted evaluation. Do not classify test-only key-generation references as a complete production stack.

### C.2 Shared deterministic FHE arithmetic

Deliverable: Establish the proposed iroha_fhe boundary for reusable RNS, modular polynomial arithmetic, NTT, basis conversion and explicit rounding; migrate callers.

Acceptance: Keep the dependency graph acyclic and manifests/lockfile coherent. Scalar, Metal, NEON/SIMD and CUDA paths used by supported targets match canonical vectors; provide deterministic fallback and retain independent mathematical test oracles.

### C.3 Secure encrypted computation profiles

Deliverable: Replace the known-insecure exact BFV construction with explicitly specified secure encryption, parameter and noise semantics.

Acceptance: Actual key/encrypt/evaluate/decrypt controls cover admitted maxima and malformed inputs; no plaintext fallback or refresh-zero surrogate. State security assumptions and unresolved analyses; signatures and test success cannot repair an insecure equation.

### C.4 Threshold key and share protocol

Deliverable: Implement explicit participant threshold, key generation, epoch custody and verifiable decryption shares.

Acceptance: Reject malformed, replayed, wrong-epoch and wrong-ciphertext shares; insufficient participation fails and the declared threshold succeeds. Verify secret lifecycle and malicious-party cases without borrowing consensus quorum rules.

### C.5 ZK-AMS production relation and arithmetic adoption

Deliverable: Migrate shared arithmetic while retaining BGV/MKHE semantics, and complete actual-source replay, composite admission and bounded qPCS/FRI work.

Acceptance: Use genuine full-size source/proof pairs, malicious-party controls, decryption-share checks and phase-two/three cases. Preserve whole-proof limits and reject treating fixture proofs or test-only collective machinery as production completion.

### C.6 SoraCloud FHE bootstrap delivery

Deliverable: Deliver genuine noise-refresh bootstrap with ciphertext, key, parameter, artifact and output binding.

Acceptance: Actual maximum supported inputs preserve plaintext and demonstrate refresh and deterministic validation; malformed relations fail. No RAM-LFE application task, phone flow or staffing item is a prerequisite.

### C.7 SoraCloud threshold computation delivery

Deliverable: Integrate threshold custody and decryption with SoraCloud job authorization and durable lifecycle.

Acceptance: Cover unauthorized job, malicious share, rotation/revocation, participant loss and interrupted recovery. Depend on shared threshold primitives, not RAM-LFE delivery or unrelated bootstrap completion.

### R.1 Phone-control trust and lifecycle contract

Deliverable: Specify the proposed phone-control provider credential, challenge interaction, privacy leakage, revocation, renewal and number recycling.

Acceptance: Resolve the external trust choice explicitly; number syntax and nullifier uniqueness are insufficient. Credential binds beneficiary, network, policy, nonce and bounded validity. Canonicality-attestor removal must not silently reappear as an audit approval gate.

### R.2 Private identifier and ownership relation

Deliverable: Bind canonical E.164, encrypted number, fresh phone-control credential and stable secret-keyed nullifier in one statement.

Acceptance: Reject wrong beneficiary/network/policy, forged or expired/revoked credential, mismatched encrypted input and replay. Equivalent numbers and randomized encryptions have one identity; key rotation preserves uniqueness without revealing the number.

### R.3 Complete RAM-LFE program relation

Deliverable: Implement actual encrypted program semantics, output binding and secure refresh using the shared arithmetic owner.

Acceptance: Prove execution rather than an input/output commitment alone. Test invalid trace, substituted program/key/input/output and real maximum workloads; plaintext keyed evaluation and OPRF-only behavior cannot substitute.

### R.4 Identifier claim state transition

Deliverable: Preserve exclusive live bindings, mandatory bounded expiry, revocation and explicit renewal/reassignment rules under native ISIs.

Acceptance: Bind proof to authenticated claimant and current state. Test competing claims, expiry eviction, authorized revocation, recycled-number reassignment, key rotation and recovery. Expiry uses the earliest applicable credential, receipt, key and policy bound.

### R.5 One-call private phone delivery

Deliverable: Deliver one call that obtains a challenge response, encrypts, proves, submits and returns a finalized phone binding.

Acceptance: Caller interaction uses a callback within the same operation. Test challenge denial, cancellation, unknown submission outcome, safe resume and payment resolution to the verified binding. Document provider-visible data and number-recycling behavior.

### R.6 Retire the plaintext RAM-LFE backend

Deliverable: Remove HkdfSha3_512PrfV1 and its RAM-LFE tags, dispatch, schemas, fixtures and SDK paths after all inventoried callers have replacements.

Acceptance: No retained consumer uses the retired backend or alias; old tags fail consistently. Move required plaintext-PRF behavior to an explicitly non-RAM-LFE role. Preserve unrelated HKDF key derivation and Java-consumer assertion coverage through Kotlin.

### P.1 Single Exact12 native dispatch owner

Deliverable: Map all twelve registered protocols to one canonical envelope/dispatch path and one verifier/state executor per role.

Acceptance: Protocol-specific builders and adapters delegate to that path; retire duplicate authoritative routes. Distinguish enum presence, implemented verification, state effects and usable delivery instead of claiming twelve production-ready protocols.

### P.2 Independent public monetary invariants

Deliverable: Enforce exact reserve debits/credits, asset and namespace isolation, committed issuance allowances and cross-pool conservation where applicable.

Acceptance: Test underflow, wrong reserve/asset/scope, ordinary-transfer bypass, duplicate effects and batch rollback. Preserve existing private-IVM reserve controls. Any new FCMP++/PQ-MASP public bridge requires its own specified invariant and covered state transition.

### P.3 Private conservation and proof relations

Deliverable: Complete ownership, ranges, nullifiers, conservation and issuance authorization for Orchard, private IVM, PQ-MASP, FCMP++, confidential assets and settlement.

Acceptance: Each relation has real positive and adversarial proofs and named mutations. Distinguish fraudulent private balances, reserve theft and transparent minting; public caps do not claim to eliminate hidden inflation or protect every depositor.

### P.4 Credential and authorization relation completion

Deliverable: Complete ZK-ACE, PGC, VeRange composition, Vega/Figure 9, Jindo and Bootle/Lantern statement, key and lifecycle requirements.

Acceptance: Genuine full-shape proofs cover authority, issuer revocation, key custody, replay, ranges and context binding as applicable. Record security assumptions and composition limits; component verification cannot confer stronger application guarantees.

### P.5 Remove activation and review predicates

Deliverable: Remove qualification receipts, reviewer signatures and activation-state prerequisites from runtime admission and SDK availability.

Acceptance: Transaction validity still enforces supported canonical profile, valid cryptography, permissions, committed key policy, replay and resource limits. Missing or invalid implementations return accurate typed errors; deleting a gate cannot turn an invalid proof or insecure backend into success.

### P.6 Exact12 lifecycle and economic integration

Deliverable: Exercise all twelve protocols through their canonical native owner with lifecycle and economic assertions.

Acceptance: Every protocol has a real supported success case and applicable tamper, stale-source, replay, authorization and rollback cases. Classify missing engine work explicitly; no count based solely on enum registration or skipped tests.

### X.1 Certificate and CRL interval semantics

Deliverable: Align presentation bounds with every certificate and the existing CRL freshness predicates.

Acceptance: Test earliest expiry at leaf/intermediate/root, certificate boundary equality, one-unit overflow, CRL nextUpdate exclusion and builder/verifier agreement. Treat acceptance of expired credentials as a security defect.

### X.2 Canonical maximum-proof delivery parameters

Deliverable: Propose a 16 MiB block payload default with coordinated RS16/transport/sync bounds; preserve the 9,437,184-byte proof ceiling and test the 10 MiB transaction envelope budget.

Acceptance: Measure the entire maximum canonical signed transaction. It must fit the chosen transaction limit with framing and supported signatures; if it does not, resolve the parameter design explicitly before claiming delivery. No off-chain proof-fetch bypass.

### X.3 Complete maximum X509 construction

Deliverable: Complete the joint certificate/CRL/ownership/disclosure relation and produce genuine maximum supported proofs within unchanged proof and prover resource targets.

Acceptance: Independent verifier, wrong-genesis, all nonce substitutions, mutation and privacy analysis accompany exact bytes, wall time, RSS and address-space records. Preserve the 300-second proving target; report failure without shrinking supported coverage or relabeling success.

### X.4 Maximum X509 finality and restart delivery

Deliverable: Submit the maximum proof through native and contract routes and the one-call builder into canonical signed blocks.

Acceptance: Four validators reach finality, persist, restart and independently verify the exact committed proof/statement. Capture proof, transaction and block sizes separately; oversized or tampered requests fail without ledger effects.

### H.1 Hash and WG measurement specification

Deliverable: Identify the WG workload from the source plan, candidate algorithms, security parameters and representative/maximal workloads.

Acceptance: Preserve current protocol-specific primitives unless an explicit justified replacement is selected. Exploratory benchmarks are labeled exploratory. Resolve the meaning and boundaries of WG before claiming that workload is covered.

### H.2 Freeze candidate algorithm and profile

Deliverable: Choose the candidate hash algorithms and full profile before final proof/benchmark measurements; regenerate affected identities and fixtures.

Acceptance: Specify transcript/commitment ordering, security and composition assumptions, work accounting and exact source/profile digests. A material algorithm/profile change creates a new candidate; there is no runtime reviewer-signoff condition.

### H.3 Measure the frozen candidate

Deliverable: Measure the selected WG/hash workload on the exact candidate with reproducible inputs, artifacts and configuration.

Acceptance: Record algorithm/profile/source/artifact identity, workload, platform, warm/cold policy and actual timing/resource samples. Prior measurements cannot qualify a changed hash. Unsupported hardware remains explicitly unmeasured.

### A.1 Finalized source and settlement binding

Deliverable: Specify authoritative finalized anchors, complete source/spend effects, participant intent and recovery identity for ordinary and AXT execution.

Acceptance: Reject self-asserted roots, substituted source/amount/authority, stale or wrong-network anchors and replay. Define atomicity and terminal recovery using committed protocol state.

### A.2 Complete ordinary execution relation

Deliverable: Complete native execution proofs and full-effect ordinary admission with bounded source custody and private masking.

Acceptance: Real maximum ordinary proofs and independent verification cover actual execution, original funding and authoritative State. Binding-only circuits and replay commitments cannot substitute; resource and hardware evidence remains candidate-specific.

### A.3 Anchored AXT native delivery

Deliverable: Implement anchored private invocation/AXT through the canonical native executor and transaction contract.

Acceptance: Four-peer success, wrong/stale anchor, intent substitution, invalid participant, replay and restart cases preserve atomic effects and terminal outcomes. Keep concrete source/settlement latency failures visible.

### A.4 Retire the superseded driver

Deliverable: Delete superseded driver code, wiring, configuration, exports, examples and packaging after anchored replacement and caller coverage exist.

Acceptance: Every supported former caller uses the replacement and its tests. No compatibility shim or parallel production driver remains. Deletion depends on anchored AXT delivery and replacement coverage, not merely ordinary execution.

### S.1 One-call atomic SDK core

Deliverable: Provide typed local prepare/prove/sign/submit/finality operations and explicit atomic batch support over one signed transaction.

Acceptance: Use configured account/signer context, secure randomness and private witness ownership. Reject oversized batches early without splitting. Unknown network outcomes return recoverable pending identities; completion means verified finality, not HTTP acceptance.

### S.2 Durable multi-round workflow APIs

Deliverable: Expose separately named bootstrap/DKG workflow calls with idempotent steps, durable progress and resumable transaction identities.

Acceptance: State cross-transaction partial completion explicitly. Test interruption before submission, after ambiguous submission and after irreversible progress; resume checks committed outcome before retrying and never fabricates cross-transaction atomicity.

### S.3 Kotlin Java and Android delivery

Deliverable: Implement common APIs through Kotlin-owned modules, Java-callable consumers and Android-specific adapters.

Acceptance: Preserve shared fixtures and every migrated Java assertion, JDK 8 API guard, no reflection and Android/core separation. Distinguish host JNI from physical-device qualification; remove replaced Java implementations only after migration.

### S.4 Swift and C sharp delivery

Deliverable: Implement common operation/workflow contracts and witness ownership through the canonical native bridge.

Acceptance: Same signed bytes, intent binding, typed errors, finality and recovery cases as Rust; actual Apple/Windows native artifacts and device/OS evidence are named separately. No managed fallback verifier with different semantics.

### S.5 JavaScript and Python delivery

Deliverable: Implement common operation/workflow contracts using supported native runtimes and coherent packaging.

Acceptance: Shared byte/error/finality vectors, cancellation, secure witness handling and installed-package smoke cases pass. No Wasm/WASI runtime or alternate codec path; do not claim unsupported browser-native proving.

### S.6 Complete SDK conformance delivery

Deliverable: Demonstrate the single canonical SDK capability matrix across Rust, Kotlin/Java/Android, Swift, JavaScript, Python and C sharp.

Acceptance: Each supported operation has real direct/native and relevant contract examples, identical intent/wire fixtures, typed rejection, unknown-outcome recovery and finality evidence. Unsupported platform capabilities are visible and remain outstanding.

### V.1 Plan and graph integrity checks

Deliverable: Maintain one machine-readable task graph and matching plan tables with one shared validator.

Acceptance: Check identifiers, dependencies, cycles, true roots, delivery reachability and named ordering constraints. Text/JSON formats run identical checks and exit consistently. Passing this checker means structural consistency only.

### V.2 Four-peer execution and recovery coverage

Deliverable: Exercise representative real native/contract/SDK transactions and anchored AXT on one four-validator candidate.

Acceptance: Verify finality, permissions, replay, batch rollback, retained history and restart recovery. Preserve signed RS16 custody and exact Sumeragi committees/quorums; network faults in protocol testing use the deterministic simulator.

### V.3 Complete current-candidate proof evidence

Deliverable: Re-run all twelve engines and coupled ordinary/AXT constructions against exact candidate source and fixtures.

Acceptance: Actual proofs, adversarial relations, serialization roundtrips and resource controls preserve substantive assertions. Report passed, failed and unexecuted cases separately; earlier component receipts cannot certify changed source.

### V.4 Independent security and hardware evidence

Deliverable: Review soundness, zero knowledge, Fiat-Shamir/qROM/composition and side channels, and measure actual supported scalar/accelerator platforms.

Acceptance: Findings identify source/profile/artifact and concrete attack or assumption. Missing evidence remains visible. This work does not supply an activation token, approve transactions or block unrelated implementation.

### V.5 Integrated product and retirement evidence

Deliverable: Run installed native SDK, phone, SoraCloud, X509, atomic batch and AXT workflows with finality and restart on the combined candidate.

Acceptance: Publish exact artifacts, results and remaining platform gaps. Verify old backend/driver/routes and aliases are absent; no success is inferred from component counts or a structural graph check.

### V.6 Candidate evidence and documentation reconciliation

Deliverable: Reconcile changed-source tests, build/lint/format/codec/ABI and SDK results, current blockers and public documentation.

Acceptance: An evidence record states what is implemented, tested, failed and unknown; no automatic release-readiness claim. Review completion is not a runtime admission predicate or deployment activation gate.

## Validation and reporting

Run the same structural checks with either presentation format:

```sh
python3 scripts/check_zk_delivery_plan.py
python3 scripts/check_zk_delivery_plan.py --format json
python3 -m unittest discover -s scripts/tests -p check_zk_delivery_plan_test.py
```

These commands validate the new plan and graph, not the unavailable original
bundled script. Both output formats check the same semantics and return the same
exit status. Structural success does not establish code correctness, cryptographic
soundness, performance or release readiness.

For implementation, use focused crate tests and real positive/negative proofs,
Norito roundtrips and the legacy-codec guard for serialization changes, ABI and
pointer-type goldens for ABI work, and shared SDK/native fixtures. Run the Sumeragi
simulator/mutation controls for affected consensus rules and real four-peer
integration for delivery. Use at least four validators and exact protocol quorums.
Run actual supported accelerator/platform tests when those claims are made;
an enabled feature or build is not hardware execution.

Apply repository formatting, strict relevant lint, workspace builds/tests and
installed SDK tests to the combined candidate as the validation budget permits.
Build steps use the repository's twenty-minute allowance. Missing time, platforms
or dependencies are reported as unexecuted, not passed. Do not interrupt another
agent's Cargo/Rust processes or bypass commit signing.

Every finding records the source/profile, claimed invariant, concrete failure
scenario, severity rationale, responsible task and regression evidence. A refuted
finding includes the counterexample or narrower supported claim. Counts of
reviewers, findings or refutations do not establish completeness or improvement.
Classify security validity errors separately from editorial mistakes.

Completion reporting distinguishes implemented, tested, independently assessed,
failed and unknown. Review evidence remains advisory to runtime admission.
Deleting activation gates must not erase known technical failures or manufacture
a claim of release readiness. Public guides belong in optional `iroha-docs`;
repository builds and checks never depend on that sibling checkout.

## Source anchors and remaining provenance

The source snapshot supports the following starting facts, not implementation
completion of this plan:

- [IVM host dispatch](../crates/iroha_core/src/smartcontracts/ivm/host.rs):
  generic instruction bridge currently accepts the ballot operation rather than
  the complete ISI surface. Its payload transport has input, allocated heap and
  validated literal paths; a 64 KiB input window is not the entire memory story.
- [Privacy transaction intent](../crates/iroha_data_model/src/transaction/signed.rs):
  current privacy admission restricts direct submission count and dynamic
  IVM/contract routes. Both require work, beyond exposing a syscall.
- [ABI descriptor](../crates/ivm_abi/src/syscalls.rs) and
  [argument descriptions](../crates/ivm_abi/src/syscalls_doc_gen.rs):
  argument semantics enter the ABI hash; retaining a syscall number does not
  eliminate descriptor/hash work.
- [Identifier execution](../crates/iroha_core/src/smartcontracts/isi/identifier.rs):
  existing claims support expiry and authorized revocation. Current canonicality
  evidence is not itself a complete stated phone-control policy.
- [RAM-LFE backends](../crates/iroha_crypto/src/ram_lfe.rs) and
  [encrypted resolver](../crates/iroha_torii/src/identifier_resolution.rs):
  HKDF is accepted as a plaintext backend; the encrypted route rejects it.
  Current BFV profiles are rejected as insecure, so retirement is paired with
  actual replacement rather than removal of the rejection.
- [ZK-AMS RNS/BGV owner](../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe.rs)
  and [collective references](../crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe/collective.rs):
  shared-owner migration must distinguish production operations and test-only code.
- [Protocol registry](../crates/iroha_data_model/src/privacy.rs):
  the canonical catalog contains twelve protocols, not five.
- [Privacy state effects](../crates/iroha_core/src/smartcontracts/isi/privacy.rs)
  and [asset transfers](../crates/iroha_core/src/smartcontracts/isi/asset.rs):
  private-IVM public transfers already use reserve-backed accounting;
  existing FCMP++/PQ-MASP effects do not provide the same public bridge.
- [Chain and transaction defaults](../crates/iroha_data_model/src/parameter/system.rs)
  and [Sumeragi parameter checks](../crates/iroha_sumeragi/src/pacemaker.rs):
  block, transaction, DA and transport limits must agree.
- [X509 relation](../crates/iroha_core_privacy/src/privacy_engines/zk_x509/relation.rs):
  presentation validity is bounded by every chain certificate and applicable CRL.
- [Current status](../status.md) and [ZK goals](zk_first_release_goals.md):
  maximum X509 size and performance observations are candidate-bound; RAM-LFE and
  several proof/platform outcomes remain incomplete.

TODO: Reconcile the original r6 plan, its graph, the definition of WG and the
linked findings if those artifacts are supplied. Record additions, retained
requirements and removals explicitly. Do not mark this TODO resolved from this
reconstruction or treat its absence as a reason to block unrelated root work.
