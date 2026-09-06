# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-87b862d2432f3ab4fcb646561f5a0482493172e59f38e9b4ffd355939abd23d5"></a>

<!-- Original context: Roadmap / KAGEMUSHA product coordinator and durable recovery -->
## KAGEMUSHA product coordinator and durable recovery



<a id="record-9d77fcadf6b31f1455e5361dfe4c71cc12f0cf672d9b99048a67fca81d58c11f"></a>

- Connect the concrete authenticated-history disk store and hardware-bound restore
  constructor to the product coordinator's private state/snapshot lifecycle.
  Reconcile retained speculative operations against current authenticated hardware
  state before resolving them; retained journal evidence is not monetary authority.


<a id="record-7ef02083928120cfd6ebf82eefe7ee867bdf87ea5cbb2f4258c970c81dc15697"></a>

- Complete sealed sender preparation, real recursive witness/proof generation,
  candidate persistence, hardware commit authorization, and mint/redemption finality
  coordination. Rebuild and qualify mobile native artifacts after that integration;
  disk/process tests alone do not qualify physical hardware or offline money flows.



<a id="record-188aa1df0cea683774547e0a88fb33ef214d06a0b51c1b91b2baea03d87e188a"></a>

<!-- Original context: Roadmap / Digital-signature release qualification -->
- Complete the remaining deferred review of offline and specialized signature
  consumers. Browser Connect now uses strict Ed25519 verification, Swift admits
  only canonical prime-order Ed25519 keys and signature points, and mirrored
  Android attestation requires a provisioning-time verifier challenge, committed
  revocation snapshot, and collision-safe alias-to-leaf key binding. Qualify the
  Android controls on real TEE and StrongBox devices, including older API
  behavior, with a governance-authenticated snapshot commitment. Separately
  reconcile the operator `allow_node_key`
  documentation with its effective key set and the validator-roster length
  helper with the proof-of-possession-filtered roster.



<a id="record-78bdc4a7ca9db1d5af58c3fd9526ba2a656850c5709989cd7b371128e72034a6"></a>

<!-- Original context: Roadmap / KAGEMUSHA V1 release completion -->
## KAGEMUSHA V1 release completion

The first-release target is one unbounded-history hardware aggregate balance and
one three-message peer protocol. Backward compatibility is forbidden. Complete
the following work before enabling KAGEMUSHA:



<a id="record-8aea697679d2f93c227e750c6241903e694a046ed1df94dc7b8c6182829b8f6a"></a>

- **Finish the three-message hard cut.** Keep only Request=1, Payment=2, and
  Acknowledgement=3 across Core, Torii, native bridge, Rust fixtures, Swift,
  Kotlin, mirrored Java, JavaScript, Python, C#, JNI, QR, and NFC. Delete every
  request-mode, request-budget, acceptance-intent, acceptance-ticket,
  precommit/no-commit, tag-4/tag-5, and five-message path rather than aliasing or
  deprecating it. Distinct valid payments against one request must all stage;
  only exact credit-ID duplicate/conflict semantics belong to the protocol.


<a id="record-581c16ffcf90e19df29ea7ad85c58652cbc3a314967d0ddc479c6b8650774aaf"></a>

- **Close recursive mint authority.** Integrate the measured k=12 Table8
  mint-hash shards into the ordered k=16 paired claim-fold. Equality-bind every
  canonical leaf byte, typed plan/cursor, proof accumulator, MintAuthorization
  statement, finalized ledger credit, recipient lane, lifecycle, replay ID, and
  envelope commitment. The leaf proof must never authorize money without the
  recursively verified claim-fold and normalized hardware guard.


<a id="record-911ffb48233a3dfbad18a447d282c3749d826916f637dd50be16cf145ad4cfa6"></a>

- **Complete fixed aggregate relations.** Ship Bootstrap, MintFold, SendSplit,
  ReceiveFold, RedeemSplit, and Rotate as fixed-shape paired-Pasta transitions
  over one private `u128` balance and replay root. Remove batch-count, hop,
  ancestry, origin, note, fan-in, and proof-depth admission maxima. Preserve a
  zero-balance successor for full spends and redemptions.


<a id="record-e09af5a018b4d6b2ea773f5fbc6d18b50907e952788abd0dbe62904cc49ec902"></a>

- **Prove real long-history behavior.** Run at least 1,024 real recursive
  handoffs and longer property histories. Complete the 1,000 independently
  funded devices -> merchant receives/folds -> one 1,000-unit payment ->
  recipient spend and full/partial redemption corridor. Measure and enforce
  history-independent proof size and verifier work at depths 8, 64, 1,024, and
  beyond; mocked-recursion models are supporting evidence only.


<a id="record-86690f86b55a3706d402d81012bafe7ff91e171919124567994c177bf44e973e"></a>

- **Finish non-forking hardware persistence.** Implement exact-next
  transitions/one-use successors, rollback-resistant journal and accepted-credit
  inbox, trusted commit time, atomic recoverable certificates, authenticated
  state/outbox, offline rotation/counter rollover, and no software fallback.
  Stage intent and outbox capacity before the one hardware commit; recovery must
  resume the same successor and reproduce every exposed byte. Host-only
  signatures and stock signing services grant no offline monetary authority.


<a id="record-bfd1facdf3a1454e29861cf3837939ee204522611082dca8f9c1779b647e71eb"></a>

- **Complete pooled settlement and routes.** Keep one reserve per asset,
  idempotent finalized top-ups, atomic full/partial redemption, unique terminal
  nullifiers, and reserve-underflow/concurrency protection. Peer transfers never
  touch the reserve. Keep only `/v1/kagemusha/readiness`,
  `/v1/kagemusha/top-up`, `/v1/kagemusha/redeem`, and operation status;
  remove lifecycle, anchor, lineage, and per-top-up drawdown endpoints and reset
  every pre-release wallet/fixture/genesis/WSV legacy state.


<a id="record-6d561bba332a0775f11a6ea8dbdaa19272a20a5cd649f22bf667b32e1efe3416"></a>

- **Regenerate conformance artifacts.** Publish identical canonical Norito
  fixtures and behavior across all SDKs and transports. Keep the paired proof at
  or below 6,528 bytes and the complete raw/text exchange near
  9,211/12,288 bytes without adding a history-dependent envelope.


<a id="record-e70b5aef7e20c84dffa8795183b8d9438b74223bf307e6e27185558443a71ab7"></a>

- **Exercise adversarial recovery.** Cover shuffled concurrent requests,
  multiple distinct payments per request, delayed post-expiry delivery, exact
  duplicate transport, conflicting credit reuse, stale state, forked successors,
  rollback, counter reuse/skip, forged rotation, overflow, proof/output
  substitution, top-up recovery, reserve underflow, duplicate/concurrent
  redemption, zero-balance continuation, verifier rotation, and rollover.
  Inject crashes at every journal, hardware commit, proof, state, inbox, outbox,
  transport, and ACK boundary and prove value conservation plus byte-identical
  recovery.


<a id="record-95add39a2b11d53c2be657d19cd32b3510e3499c0e4586213e92f5f2ada9d93e"></a>

- **Qualify release evidence.** Regenerate the final source/recovery inventory,
  run focused and workspace Rust/SDK/lint/format gates from stable source, obtain
  independent cryptographic review, and complete physical airplane-mode,
  restart, power-loss, clock-rollback, backup/restore, thermal, latency, memory,
  and throughput qualification for every enabled hardware profile.

Current focused structural tests are useful iteration evidence, not release
qualification. Do not mark this section complete until the real recursive,
hardware, settlement, cross-SDK, crash-recovery, and physical-device gates all
pass.


<a id="record-d4edf6497d1878ac4e12aad5c2ba8da0651a3aff4ce1cffa8cc617660a9431ad"></a>

<!-- Original context: Roadmap / Peer transport V1 release evidence -->
## Peer transport V1 release evidence



<a id="record-d74fe3f2b8f29fb2796d25113d06b70e413dbb6b3961740bfb8871c9fa82b017"></a>

- Treat the green codec, lifecycle, replay, callback-epoch, delivery-barrier,
  APDU, and checkpoint suites as automated evidence only. Complete and archive
  the physical-device matrix before promotion:
  - iOS-to-iOS over QR camera, Google Nearby, and entitlement/device-eligible
    Core NFC/CardSession;
  - Android-to-Android over QR camera, Google Nearby, and IsoDep/HCE; and
  - iOS-to-Android in both sender/receiver directions over QR, Nearby, and
    capability-eligible Core NFC/CardSession-to-IsoDep/HCE.


<a id="record-e81ed77b5fca3b6bd46f35ba742623921146a9c500c12143e9275f9b6404d859"></a>

- Exercise the shared profile-1/schema-1 `IPM1`, `IQR1`/`IRQR`, `IPD1`/`IPN1`,
  and F049 NFC vectors in every applicable leg. Promotion evidence must cover
  header-last scans, one lost shard per parity pair, duplicate/conflicting
  frames, quarantine expiry and clean reuse, app background/foreground,
  stop/restart with delayed callbacks, completed-session rollover, replay and
  reordered Nearby records, RF loss before and after each durability boundary,
  retap/`GET_STATUS` recovery, and explicit profile/kind/schema mismatch
  rejection.


<a id="record-100004817f8547fb92bc61cba730e6c34c261a2694ccdd48dcc12e7e578b9f50"></a>

- Verify optimization without changing the common wire: same-platform peers
  must use their mutually supported radio and NFC chunk capabilities, while
  cross-platform NFC uses the minimum advertised safe limit. Record payload
  bytes, hashes, terminal delivery updates, exact compact-payment/native-ACK
  binding, and final durable checkpoints so optimization cannot mask a wire
  divergence.

