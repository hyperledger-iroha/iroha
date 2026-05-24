# Roadmap

Last updated: 2026-05-24

This roadmap is the public, high-level view of current Hyperledger Iroha work.
The detailed engineering backlog lives in
[`docs/source/engineering_backlog.md`](./docs/source/engineering_backlog.md),
and completed history lives in [`status.md`](./status.md).

## Release and Stabilization

**Status:** active.

- Move the shared Iroha 2 / Iroha 3 codebase toward a broadly consumable
  release with clear release notes, SDK parity, and operator documentation.
- Keep focused validation green for the core transaction pipeline, Torii query
  APIs, Norito wire formats, and SDK fixtures before broader workspace test
  runs.
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
  proposer-side prepare/commit gating, 4-peer convergence proof,
  queue-journal restart replay, and routing-plan projection with longer-running
  soak, fault injection, and independent participant-lane finality work.
- Keep SCCP bridge submission permissionless while requiring outbound message
  records to originate from verified IVM-proved overlays, route allowlists to
  be deployment-governed, and production activation to wait for all advertised
  lanes to have cryptographic source-chain proof adapters plus immutable
  destination verifiers. ETH/BSC production targets `evm-groth16-bn254-v1`,
  TRON/TVM production targets `tron-groth16-bn254-v1`, and the secp256k1
  attestation wrapper remains reference-only. The SCCP source
  proof envelope now binds non-SORA source messages to source/target domains,
  proof plan, finality model, message id, payload hash, commitment root, and
  source-event digest before public inputs are derived. Source consensus proof
  material now also carries a plan-specific adapter proof variant for
  ETH/BSC/Solana/TON/TRON/Substrate-family lanes, so generic self-consistency
  blobs or stale witness substitutions cannot masquerade as another chain's
  proof shape. Each adapter statement is now additionally wrapped in a
  FastPQ/OpenVerify proof capsule so adapter metadata, public inputs, and proof
  public IO are cryptographically bound before lane readiness can be considered.
  Source consensus proofs also carry explicit trust-anchor/verifier evidence
  for the source anchor, consensus verifier, message-inclusion verifier,
  finality policy, adapter proof, adapter transcript, and adapter circuit, with
  the evidence hash included in the adapter OpenVerify statement. Those
  evidence records are now sourced from typed `SccpSourceVerifierMaterialV1`
  records; the built-in catalog is placeholder-only and cannot satisfy the
  production gate until real source-chain trust anchors and immutable verifier
  hashes replace it; flipping the placeholder flag or reusing any built-in
  placeholder component still fails closed. Explicit-material production helpers
  now verify source envelopes against caller-supplied ready material, and
  `zk.sccp_source_verifier_materials` now threads configured material into
  on-chain bridge proof admission and the ZK consensus policy hash. The default
  production path remains closed on the placeholder catalog when no configured
  material is present. The EVM destination side now has a
  `SccpGroth16Bn254MessageVerifier` implementation for the
  `evm-groth16-bn254-v1` backend; production still needs the real recursive
  SCCP circuit verifying key and deployment binding before routes can be
  marked ready. Destination rollout records are now bound to domain and chain
  and must carry non-empty verifier identity, non-zero verifier code hash,
  anchor id, matching verifier plan, ready flags, and no blockers before they
  can satisfy the production gate. EVM Groth16 relay packages are signer-free:
  they carry the verifier proof ABI tuple directly and reject attempts to use
  the reference attestation/signer path for the production backend. The
  normalized proof-job builder now has the same explicit signer-free Groth16
  path, so production EVM/BSC proof tooling must provide the Groth16 proof bytes
  and deployment binding instead of falling back to Torii signer attestations.
  The lane readiness surface now separates this local adapter-proof binding
  from the still-missing external consensus verifiers, external
  receipt/message-inclusion verifiers, and source-chain trust anchors.
  Bridge proof admission validates SORA-origin Nexus finality separately from
  non-SORA source-chain envelopes. Nexus block-level SCCP message records are
  restricted to
  SORA-origin payloads; external-source messages must enter through bridge proof
  submission with their source-chain envelope. Disabled SCCP lanes remain
  non-consumable in state-changing Torii endpoints and on-chain bridge proof
  admission even if historical unready-proof diagnostics are enabled in config.
  Torii no longer synthesizes non-SORA source-chain envelopes from local Iroha
  finality; external-source submissions must carry source-adapter proof
  envelopes. JavaScript, Swift, Kotlin, and Java SDKs now expose local-first
  Solana and TON SCCP proof request wrappers so web and mobile UIs can collect
  source witness data, invoke an app-linked prover, and submit the resulting
  proof on-chain without relying on node-side proof generation. The Solana
  source adapter now cryptographically binds `message_proof_hash` to the source
  event digest, transaction-status root, and inclusion branch, and the SDKs
  expose the same helper so UI provers do not pass opaque placeholder message
  proof hashes. TON submission
  packages now carry a real `ton_message_body_boc_v1` message body BOC, with
  proof bytes, public inputs, SCCP bundle bytes, destination binding, and
  statement hash bound into the TON internal-message payload.
- Keep live-network signing inputs runtime-only and continue using generated
  per-validator deployment bundles rather than hand-edited production configs.

**Next checkpoints:** implement the real SCCP source-chain verifier engines
behind the typed adapter variants so ETH/BSC/Solana/TON/TRON/Substrate
consensus/finality and receipt/message inclusion are checked against external
chain rules, land the production Solana and TON recursive prover/verifier
integrations behind the SDK proof request APIs, deploy immutable destination
verifiers and TON verifier-contract bindings, produce multi-lane integration
evidence, publish operator runbooks, and incorporate testnet-driven feedback
from wallet and service integrations.

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
  SDK fixtures, and cross-library compatibility tests.

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

## Consensus, Performance, and Operations

**Status:** active optimization.

- Wire the canonical Sumeragi V1 pure engine through the live network,
  validation, payload, telemetry, and storage adapters while preserving
  deterministic consensus behavior and the hard consensus cadence gates.
- Keep permissioned and NPoS execution on one state machine; validator-set
  source and strict quorum math are the only mode differences.
- Use measured matrix runs, not speculative settings, before accepting higher
  throughput targets.
- Keep hardware acceleration paths feature-gated with deterministic scalar
  fallbacks.

**Next checkpoints:** Sumeragi V1 adapter integration, certified-block
recovery soak coverage, peer-gap and DA/RBC tail-latency reductions,
restarted-peer replay soak coverage beyond the bounded snapshot/Kura gate,
broader formal coverage beyond the current
commit-path, frontier, fork-safety, quorum-policy, RBC deliver-quorum,
RBC causality gate, pending-RBC stash gate, RBC signing-preimage gate, classic Vote/VRF
signing-preimage gate, classic Vote/QC signature-verification gate, VRF
commit/reveal admission gate, classic inbound vote-admission gate,
proposal-hint admission gate, proposal metadata admission gate, QC
signer-bitmap admission, direct BlockCreated admission gate, commit-root
consistency, commit-pipeline recovery gate, commit-pipeline scheduling gate,
commit-result drain gate, commit-job dispatch gate, commit-inflight timeout
gate, post-commit pacemaker kick gate, idle-view proposal budget gate,
pacemaker evaluation gate,
cached proposal-slot timeout gate,
proposal parent resolution gate,
precommit-QC view-change selector gate,
commit-evidence replay gate, block-sync recovery gate, direct certified-block fetch gate,
missing-block fetch planner, missing-block hard-cap recovery gate,
missing-block hard-cap cleanup gate,
missing-block view-change escalation gate, precommit vote-emission gate,
native AMX attestation gate,
native AMX queue-journal replay gate, native AMX routing-plan projection gate,
native AMX receipt validation gate, native AMX control-plane ingress gate,
vNext chain-order helper gate, vNext re-chain helper gate, vNext aggregate
certificate verification gate, vNext signing-preimage gate, vNext
control-certificate ingress gate, vNext slot-lifecycle gate, vNext validation
ownership gate, async vote-verification ownership gate, async QC
aggregate-verification ownership gate, worker-loop drain scheduler gate,
actor-gate priority/fairness gate, worker-loop budget/adaptive-cap gate,
worker ingress routing gate, NPoS VRF epoch-seal staging gate, proposal
assembly gate, Kura durability commit retry gate, restarted-peer replay gate,
post-commit cleanup gate, frontier-gap realignment gate, pure engine tick gate,
pure engine NewView subject projection helper gate, pure engine certificate
prefilter dispatch gate, pure engine certificate prefilter state-handoff gate,
pure engine view-advance saturation gate,
engine NewView-QC gate, pure engine exact NewView-QC highest-QC record gate,
pure engine exact NewView-QC advance gate,
pure engine handle-dispatch gate,
pure engine proposal-ingress gate,
pure engine exact proposal output-field gate,
pure engine exact proposal state-mutation gate,
pure engine exact proposal validation-owner gate,
proposal-lock helper gate,
QC-round compatibility helper gate,
QC reference projection helper gate,
highest-QC record helper gate,
commit-subject helper gate,
payload lookup helper gate,
pure engine prepare-QC gate,
pure engine exact Prepare-QC lock/highest-QC record gate,
pure engine exact Prepare-QC phase-transition gate,
pure engine prepare-vote cache/output gate,
pure engine commit-QC gate,
pure engine exact Commit-QC highest-QC record gate,
pure engine payload-available Commit-QC exact finality gate,
pure engine missing-payload Commit-QC pending/fetch gate,
pure engine Commit-QC validation cleanup gate,
pure engine committed-block gate, pure engine exact committed-block record gate,
pure engine reconfiguration staging gate,
pure engine committed-block cleanup gate,
pure engine exact payload-availability record gate,
pure engine payload-availability gate,
pure engine validation-result gate,
pure engine exact validation-owner cleanup gate,
pure engine exact invalid-validation round/output advance gate,
reconfiguration, certified-recovery, view-change, validation-callback,
certificate-admission, and highest-QC selection bounded models, and updated
operator runbooks when defaults change.

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
