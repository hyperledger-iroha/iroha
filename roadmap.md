# Roadmap

Last updated: 2026-05-29

This roadmap is the public, high-level view of current Hyperledger Iroha work.
The detailed engineering backlog lives in
[`docs/source/engineering_backlog.md`](./docs/source/engineering_backlog.md),
and completed history lives in [`status.md`](./status.md).

## Release and Stabilization

**Status:** active.

- Move the shared Iroha 2 / Iroha 3 codebase toward a broadly consumable
  release with clear release notes, SDK parity, and operator documentation.
- Keep focused validation green for the core transaction pipeline, Torii query
  and control-plane APIs, Norito wire formats, and SDK fixtures before broader
  workspace test runs.
- Complete the first-release Offline Bearer Cash pilot over the ZK note and
  nullifier engine. Swift, Kotlin, and Java Android now expose the Bearer Cash
  v1 wallet, note, receive-request, payment-token, ACK, text-codec, and policy
  names; QR/NFC/Nearby app payloads use only the
  `wallet-offline-bearer-cash-*` prefixes; and shared fixtures publish
  `offline_bearer_cash_v1` policy defaults for custody hops, lineage steps,
  QR/stream payload limits, and Android one-use-key pool sizing. Torii no
  longer carries legacy offline transfer/revocation compatibility routes or
  MCP aliases.
  Shared chain-side `OpenVerifyEnvelope` admission now requires exact active
  verifier-key commitment binding and canonical empty auxiliary bytes for
  generic `VerifyProof`, governance voting proofs, STARK shielded
  transfer/unshield wrappers, IVM-proved overlays, IVM host registered-key
  verify syscalls, Kaigi privacy proofs, RAM-LFE proof receipts, identifier
  proof receipts, confidential-transfer-v2 transfer/unshield admission, and
  the Offline/Kagemusha flows. Private Kaigi fee admission validates its
  fee-binding auxiliary metadata at the transaction boundary and then
  canonicalizes the internal `ZkTransfer` proof to empty auxiliary bytes, while
  anonymous escrow close prechecks validate the confidential-transfer-v2 proof
  envelope before trusting parsed input commitments. The shared Halo2 IPA
  backend verifier also rejects non-empty auxiliary bytes and zero or mismatched
  envelope verifier-key hashes before proof verification, so direct verifier
  callers inherit the same fail-closed baseline; low-level backend dispatch
  also rejects proof boxes whose embedded backend label differs from the
  requested verifier backend. The lightweight preverify/dedup cache also
  decodes recognized `OpenVerifyEnvelope` wrappers and rejects malformed
  backend tags, auxiliary bytes, zero verifier-key hashes, and verifier-key
  commitment mismatches before cache insertion, while Groth16, Halo2/BN254, and
  Halo2/KZG labels remain unsupported before dedup insertion, preventing failed
  preverify attempts from poisoning later valid proofs. The checked verifier
  guardrail wrapper rejects the same trusted-setup labels before backend
  dispatch.
  The production audit path is now topup-anchored and rejects unbound input
  claims, hidden output commitments, cross-asset audits, and public amount
  mismatches; audit output certificates are signature-checked against their
  declared output account before lineage is issued, and recursive proof
  envelopes now require exact active verifier-key commitment binding, inline
  verifier-key length consistency, canonical `offline-note-recursive` circuit
  ids, canonical empty auxiliary bytes, and shared trusted-setup/developer-only
  backend classification before verifier-registry lookup.
  Verifying-key registry admission now rejects inline verifier records with
  inconsistent published key lengths on both register and update, and rejects
  explicit trusted-setup backend labels such as Groth16, Halo2/BN254,
  Halo2/BLS12, and Halo2/KZG before they can enter registry or proof-attachment
  admission; standalone setup labels such as `kzg`, `bn254`, `bn256`, and
  `bls12_381`, plus colon-delimited profiles such as `halo2/ipa:kzg`, are now
  caught by the same shared classifier before broad allowlists can admit them.
  Generic proof attachments also reject developer-only labels before
  envelope matching, and STARK/FRI registry admission applies the same
  trusted-setup label rejection even for keyless records. Verifier-key
  register/update admission also rejects developer-only labels containing
  `debug` or `mock`, including legacy seeded records attempting to refresh
  through update. IVM host verifier snapshots and Torii's
  non-consensus proof/prover worker enforce the same trusted-setup and
  developer-only label policy before
  syscall proof verification or broad backend allowlist matching, and Torii
  prover-worker backend mismatches now stop before verifier-registry lookup.
  The core preverify cache and guardrail dispatch wrappers also reject those
  developer-only labels before dedup insertion or verifier dispatch.
  Torii-generated IVM proof
  attachments include the checked verifier-key commitment for downstream
  proof-submission binding.
  The `zk-preverify` block sidecar path records verified trace digests only;
  the background trace lane revalidates queued traces but no longer emits
  `zk-trace/mock-proof` artifacts while the real transparent IVM trace prover
  remains future work.
  `KagemushaTransfer` is now the chain-side shielded
  offline-offline instruction: it is default-on through settlement config, with
  real execution coverage asserting the default-enabled/non-legacy state; it uses
  the existing ZK asset nullifier/commitment/root accumulator, requires an
  asset-bound confidential-transfer-v2 Halo2 IPA verifier and root hint, rejects
  trusted-setup proof labels, checks the submitted `OpenVerifyEnvelope` backend
  tag, confidential-transfer-v2 circuit id, schema, and verifier-key hash against
  the active asset binding, requires inline verifier-key bytes with matching
  length, commitment, non-zero proof-size cap, and active circuit/version index
  before proof envelope decoding, and leaves legacy bearer-audit forcing
  available only as an explicit migration fallback.
  Compact multi-hop Kagemusha tokens now have a deterministic folded
  public-input transcript in the Rust data model; the transcript canonicalizes
  bounded private hops, rejects duplicate nullifiers/commitments and root
  discontinuities, and binds chain id, asset definition, roots, hop count, and
  aggregate folded-hop digests. The folded statement now carries a proved
  aggregation-mode column; checked transparent pre-fold v1 is accepted, while
  reserved recursive aggregation modes are rejected until their in-circuit
  verifier exists. A Poseidon2 aggregation transcript digest is now derived from
  the same canonical hop sequence as a hash-friendly public accumulator for that
  future recursive verifier. Checked fold construction verifies each private hop
  proof and binds its verified public-input statement plus verifier-key
  id/commitment and a Poseidon2 digest of the verifier-key backend/bytes before
  hashing it into the transcript; optional envelope-hash metadata must match
  the submitted envelope bytes before private hops or chain-side transfers are
  accepted, and raw checked folding enforces the confidential-transfer-v2
  proof-size cap, per-hop shape bounds, root continuity, duplicate-set checks,
  non-zero set entries, mandatory verifier-key commitment and verifier-key-id
  metadata, non-empty verifier-key bytes, canonical empty envelope auxiliary
  bytes, and the 64-hop compact-token corridor before parsing private-hop envelopes. Chain-side
  Kagemusha transfers now apply the same duplicate/non-zero set invariants
  before proof envelope decoding.
  The proof-statement
  preimage is exposed in the data model as `KagemushaProofPublicInputsStatement`
  and hashed through the canonical Norito/Poseidon2 helper, which now rejects
  non-empty envelope auxiliary bytes and zero verifier-key hashes before
  transcript material is derived at both the core verifier helper and the
  public data-model API, so SDKs and future recursive circuits share the same
  canonical target format. The public Kagemusha transcript helpers also reject
  unsupported, trusted-setup, and developer-only backend labels before hashing
  per-hop verifier-key material or folding verifier-key ids. The
  proof-statement helper now also rejects empty circuit ids, schema bytes,
  missing or empty instance columns, empty verifier-key bytes, and empty
  folded-hop verifier-key id names before they can become wildcard transcript
  material. The shared STARK/FRI
  classifier also rejects the profile-less `stark/fri/` prefix,
  trusted-setup STARK/FRI profiles such as KZG, BN254, and BLS12, and any
  STARK/FRI profile containing developer-only `debug` or mock labels before
  verifier admission reaches proof decoding, and Torii proof/prover paths
  mirror that rule while fatal prover-worker attachment classification errors
  return before registry lookup. The shared trusted-setup classifier also
  covers standalone KZG/pairing labels and colon-profile setup labels with
  ASCII-case-insensitive matching, so registry admission, preverify, guardrails,
  and Torii's broad prover allowlists all fail closed on mixed-case forms such
  as `halo2/ipa:KZG` or `halo2/ipa:Mock-Proof`. Compact folded-token
  verification now also has
  explicit final-proof coverage rejecting trusted-setup and developer-only
  backend labels in both direct and record-backed verifier paths. Direct
  Poseidon2 aggregation transcript hashing now validates
  canonical mode/count/index/root continuity, sorted non-zero folded sets,
  duplicate-free membership, and
  supported transparent verifier-key backends before hashing, and rejects
  zero proof public-input digests, verifier-key commitments, or verifier-key
  Poseidon2 digests as wildcard binding material. The folded public-input model
  also rejects zero or over-64 hop counts during data-model context validation,
  and exposes a 1 KiB encoded-size budget plus `norito_encoded_len()` helpers so
  mobile transports can enforce compact QR/NFC payload corridors before adding
  backend proof bytes; folded-context validation rejects over-budget public
  transcripts. The Poseidon2 aggregation statement is exposed publicly as
  `KagemushaPoseidonAggregationTranscriptStatement` with a canonical builder
  and digest helper, giving SDKs and future recursive circuits the same
  canonicalized target layout. The high-level compact-token
  prover uses that checked path before
  emitting the first
  `kagemusha-folded-v1` transparent Halo2 IPA proof, which proves and verifies
  the 30-column folded public statement without a trusted setup and pins the
  final folded proof envelope to canonical empty auxiliary bytes. Derived Halo2
  IPA proving keys for IVM, Offline Note, and Kagemusha now use Norito archives
  that bind the canonical circuit family and verifier-key commitment before raw
  Halo2 key bytes are decoded, rejecting raw or cross-circuit key material while
  preserving production key caching. Mobile bridge
  callers can now invoke the verified-bundle prover through
  `connect_norito_kagemusha_prove_verified_compact_payment_token`, which
  verifies every bundled hop proof before deriving folded public inputs.
  Checked bundle verification now rejects generic active transparent hop
  circuits before compact proof generation; record-backed bundle verification
  also ties private hops to active WSV-style confidential-transfer-v2 verifier
  metadata, including backend tag, circuit id, schema hash, verifier-key
  commitment, key length, proof-size cap, optional inline-key consistency, and
  exact record-set matching with no unrelated records.
  Mobile and bridge callers can use
  `connect_norito_kagemusha_prove_verified_compact_payment_token_with_records`
  with `KagemushaVerifiedFoldRecordBundle` to get those WSV-style checks at the
  FFI boundary, while raw folded-input proof construction stays crate-local.
  Swift, Kotlin/JVM, and Java Android now expose record-backed compact-token
  prover wrappers over that ABI, so mobile wallets can pass
  `KagemushaVerifiedFoldRecordBundle` Norito bytes through the native bridge
  instead of constructing preverified folded public inputs themselves.
  Swift, Kotlin/JVM, and Java Android Offline Note proof binding now also
  rejects substituted recursive verifier ids or proof backend labels before
  accepting wallet-side validation, keeping mobile checks aligned with the
  chain's `halo2/ipa:offline-note-recursive` trust anchor. Draft wallet and
  redeem-planner bundles now carry the explicit unsupported
  `offline-note/draft-placeholder` backend until a real proof provider replaces
  them.
  Current release evidence covers physical iOS App Attest/HCE/CardSession
  availability and Android StrongBox/KeyMint one-use-key validation. The open
  physical gap is the end-to-end cross-platform NFC/HCE payment
  exchange with both devices unlocked and ready; recursive aggregation of the
  private per-hop proofs into the compact no-trusted-setup Kagemusha proof
  remains follow-up work for a later version. The current codebase has no
  in-circuit Halo2 IPA verifier gadget, so aggregation mode `2` stays a
  reserved, explicitly rejected wire value until that verifier exists.
- Continue dependency, documentation, and release hygiene work required by LF
  Decentralized Trust project expectations.

**Next checkpoints:** refreshed release checklist, full validation corridor,
and public release-readiness notes.

## SORA Nexus and Taira

**Status:** active pre-release hardening.

- Use the public Taira testnet to harden consensus, routing, lane-aware
  execution, data availability, operator workflows, and SDK integration.
- Complete the remaining independent-lane consensus, DA/RBC, and cross-lane
  relay validation needed for the first public Nexus release.
- Continue native AMX hardening beyond the implemented attestation data model,
  control-plane message handling, deterministic per-leg vote cache,
  proposer-side prepare/commit gating, 4-peer convergence proof, and
  queue-journal restart replay with longer-running soak, fault injection, and
  independent participant-lane finality work.
- Keep SCCP bridge submission permissionless while requiring outbound message
  records to originate from verified IVM-proved overlays and explicit
  deployment bindings for production-ready EVM lanes.
- Keep live-network signing inputs runtime-only and continue using generated
  per-validator deployment bundles rather than hand-edited production configs.

**Next checkpoints:** multi-lane integration evidence, public operator
runbooks, and testnet-driven feedback from wallet and service integrations.

## IVM, Kotodama, and Norito

**Status:** active first-release hardening.

- Keep the Iroha Virtual Machine syscall and pointer-ABI surface deterministic
  across hardware and peers.
- Make `iroha contract dev` the default first-release contract workflow,
  including manifest-sourced builds, generated interfaces, schema docs,
  profile-aware doctor/smoke commands, and Kotodama test/debug loops.
- Finish compiler-derived access descriptors for remaining opaque host helper
  syscalls.
- Preserve canonical Norito headers and wire layouts for blocks, transactions,
  SDK fixtures, and cross-library compatibility tests. The JavaScript pure
  Norito fallback now covers asset-definition registration frames, and
  Java/Kotlin columnar helpers cover optional string/u32 plus bytes+bool row
  shapes, so remaining SDK parity work should focus on new observable wire
  formats as they land.

**Next checkpoints:** ABI golden updates when the syscall surface changes,
expanded cross-SDK vector coverage, and updated docs for any observable layout
or ABI behavior.

## Privacy, ZK, and FHE

**Status:** active research-to-product integration.

- Replace current deterministic BFV-shaped evaluation scaffolding with the full
  BFV-RNS implementation planned for release.
- Broaden cross-SDK deterministic vectors for encrypted payloads, receipts, and
  opening verification.
- Fold focused ZK/FHE adversarial tests into the long workspace validation
  corridor.

**Next checkpoints:** complete BFV-RNS parameter/key fixtures, Soracloud
multi-input evaluation coverage, and proof/receipt compatibility across Rust,
Kotlin, Java, Swift, and JavaScript.

## Performance and Operations

**Status:** active optimization.

- Continue Sumeragi vNext work toward higher applied throughput while
  preserving deterministic consensus behavior and the hard consensus cadence
  gates.
- Use measured matrix runs, not speculative settings, before accepting higher
  throughput targets.
- Keep hardware acceleration paths feature-gated with deterministic scalar
  fallbacks.

**Next checkpoints:** peer-gap and DA/RBC tail-latency reductions, restarted-peer
replay coverage, and updated operator runbooks when defaults change.

## Community and Governance

**Status:** active growth work.

- Use the official X account, [`@hl_iroha`](https://x.com/hl_iroha/), as the
  primary public cadence for recurring X Spaces, demos, and roadmap Q&A.
- Publish recaps or recording links when available so contributors can follow
  progress asynchronously.
- Grow contributor and maintainer diversity by turning testnet interest,
  CBDC/regulated-finance adoption, and LFDT ecosystem connections into repeat
  reviewers and subsystem owners.

**Next checkpoints:** monthly X Spaces cadence, clearer contributor onboarding,
public follow-up notes for LFDT governance review items, and commit/reveal
hardening for SORA Parliament policy juries.
