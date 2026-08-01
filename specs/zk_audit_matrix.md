# ZK Audit Matrix

This matrix records the proof-verification ingress points audited in the
2026-04-02 and 2026-05-16 ZK hardening passes. The goal is to make verifier boundaries explicit:
which surfaces perform real cryptographic verification, what binds the claimed
statement before verification, and which paths are demo-only or non-ZK.

## Matrix

| Surface | Backend family | Runtime criticality | Outer binding checks | Backend verifier used | Residual risk after patch |
| --- | --- | --- | --- | --- | --- |
| Governance ballot / tally | Registry-backed `halo2/ipa` and `stark/fri/*` | Consensus-critical | VK registry lookup, active-key status, backend/circuit match, `vk_hash`, public-input schema hash, namespace / manifest ownership, backend-specific domain binding | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Main risk is now registry/config misuse rather than Fiat-Shamir statement omission. |
| Confidential transfer / unshield | Registry-backed verifier path (current default `halo2/ipa`, STARK family where configured) | Consensus-critical | Policy/VK resolution, `vk_hash`, schema hash, proof-size caps, backend allowlist, wrapper/header checks | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Wrapper and registry binding stay stronger than the standalone helper path. |
| `IvmProved` admission | Registry-backed `halo2/ipa` or `stark/fri/*` | Consensus-critical | `vk_hash`, canonical `ivm-execution` schema hash, circuit id, namespace / manifest match, curve / `k` caps, payload header validation | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Admission remains pinned to the guarded runtime verifier. |
| Kaigi privacy join / usage | Registry-backed `halo2/ipa` | Consensus-critical for private Kaigi flows | VK registry record, `vk_hash`, schema hash, canonical circuit id, active status, exact commitment/nullifier/root public-input binding | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. The roster join uses domain-separated constrained Poseidon; on-chain private leave remains disabled until it has a dedicated membership circuit. |
| RAM-LFE execution receipts | Resolver signature or policy-published `halo2/ipa` verifier metadata, as required by policy | Non-consensus helper / application-facing | Policy/backend/mode binding; native backend-registry admission; canonical envelope; circuit, schema, and verifier-key hashes; public instance bound to the execution payload hash; runtime enablement and envelope/proof byte caps | Signature verification or `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Proof-mode receipts use the same node-configured guardrails as other native backend verification paths. |
| Identifier receipts | Signed or policy-published Halo2 RAM-LFE execution attestation plus a signed output opening | Consensus-critical claim admission / application-facing verification | Policy/program linkage, output-opening signature, derived opaque identifier and receipt hash, plus the guarded RAM-LFE proof binding above | Signature verification or the shared guarded RAM-LFE verifier | Low. Consensus and stateless verification now share one proof-validation path, preventing policy or resource-limit drift. |
| Lane relay / FASTPQ | Native FASTPQ prover/verifier | Safety-critical for lane proof checking | Rebuilt transition batch from binding, full `PublicIO` equality (`dsid`, `slot`, roots, hashes), transcript already seeded with `public_io` | `fastpq_prover::verify` | Medium-low. Claims are now checked field-for-field; remaining risk is in FASTPQ arithmetic/circuit correctness rather than omitted public claims. |
| Torii `POST /v1/zk/verify-batch` | Standalone native IPA poly-open helper | Diagnostic only, not ledger-equivalent | Configured total-body cap before decode; finite batch/envelope/curve-`k`/label caps; the wire selects only curve/`n` and the verifier derives the deterministic V1 generators; transcript-bound statement (`transcript_label`, complete derived parameter fingerprint, curve/`n`, `z`, `t`, `p_g`, optional metadata); proof-round shape checks | `iroha_zkp_halo2::batch::verify_open_batch_with_limits` | Low for the standalone primitive. Callers cannot encode alternate generator relations, the embedding surface has no unbounded handler, and resource use is bounded, but the diagnostic endpoint intentionally lacks ledger VK registry / circuit/schema policy enforcement. |
| IVM batch syscall (`SYSCALL_ZK_VERIFY_BATCH`) | Registry-backed `halo2/ipa` and `stark/fri/*` verifier on `CoreHost`; disabled on `DefaultHost` | Runtime helper with ledger-grade binding on the node host | Outer `OpenVerifyEnvelope` header checks, VK registry lookup, circuit/schema/manifest/curve/`max_k` or STARK profile enforcement, then backend verification with guardrails | `iroha_core::smartcontracts::ivm::host::CoreHost` -> `iroha_core::zk::verify_backend_with_timing_guardrails` | Low on the runtime host. `DefaultHost` intentionally returns `ERR_DISABLED`, so the remaining risk is misuse of a non-runtime host rather than a standalone verifier bypass. |

## Notes

- The strongest OtterSec-style risk in this repo was the standalone native IPA
  helper, because it previously derived Fiat-Shamir challenges without binding
  the full public statement. That helper now binds `transcript_label`,
  backend/domain size, the exact deterministic parameter fingerprint, `z`,
  `t`, `p_g`, and any optional metadata carried in `OpenVerifyEnvelope`.
- FASTPQ already seeded Fiat-Shamir with `public_io`, so its issue was not the
  same bug class. The hardening here closes the verifier-side claim-validation
  gap by requiring field-for-field `PublicIO` equality before accepting the
  proof.
- Production ledger verification remains centered on the guarded
  `iroha_core::zk::verify_backend_with_timing_guardrails` path and should stay
  the reference implementation for future proof-bearing features.
- The pre-release decode-only `/v1/zk/verify` and `/v1/zk/submit-proof`
  routes were removed instead of retaining success responses that could be
  confused with cryptographic or ledger acceptance.
- Block headers and peer handshakes now include a `zk_policy_hash` in the
  confidential feature digest, so peers commit to the consensus-relevant ZK
  verifier policy instead of trusting node-local timeout or worker settings.
- Generic `VerifyProof` is registry-only: it requires `vk_ref`, rejects inline
  VKs, and enforces active VK record, circuit/version, schema, namespace, gas
  schedule, and commitment binding before calling the backend verifier.
- Local verifier elapsed time is reported for telemetry but is no longer a
  consensus rejection condition. Runtime limits that can change block validity
  are sourced from the committed ZK policy.
- The standalone Halo2 helper's wire carries only `(version, curve, n)` and the
  verifier derives the one deterministic V1 parameter set for that selector.
  Canonical sets live in a bounded process-local cache and Fiat-Shamir
  challenges come from a running transcript state. This removes the
  caller-chosen generator surface entirely, as well as the previously noted
  unbounded-cache and quadratic-history rough edges, without changing its
  non-ledger status.
