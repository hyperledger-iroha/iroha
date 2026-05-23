# Roadmap

Last updated: 2026-05-23

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
restarted-peer replay coverage, broader formal coverage beyond the current
commit-path, frontier, fork-safety, quorum-policy, RBC deliver-quorum,
QC signer-bitmap admission, commit-root consistency, commit-pipeline recovery
gate, commit-evidence replay gate, block-sync recovery gate, precommit
vote-emission gate, native AMX attestation gate, native AMX queue-journal
replay gate, proposal assembly gate, pure engine tick gate, pure engine
NewView-QC gate, pure engine proposal-ingress gate, pure engine prepare-QC
gate, pure engine commit-QC gate, pure engine committed-block gate, pure engine
payload-availability gate, pure engine validation-result gate,
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
