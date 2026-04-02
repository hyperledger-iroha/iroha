# ZK Audit Matrix

This matrix records the proof-verification ingress points audited in the
2026-04-02 ZK hardening pass. The goal is to make verifier boundaries explicit:
which surfaces perform real cryptographic verification, what binds the claimed
statement before verification, and which paths are demo-only or non-ZK.

## Matrix

| Surface | Backend family | Runtime criticality | Outer binding checks | Backend verifier used | Residual risk after patch |
| --- | --- | --- | --- | --- | --- |
| Governance ballot / tally | Registry-backed `halo2/ipa` and `stark/fri-v1/*` | Consensus-critical | VK registry lookup, active-key status, backend/circuit match, `vk_hash`, public-input schema hash, namespace / manifest ownership, backend-specific domain binding | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Main risk is now registry/config misuse rather than Fiat-Shamir statement omission. |
| Confidential transfer / unshield | Registry-backed verifier path (current default `halo2/ipa`, STARK family where configured) | Consensus-critical | Policy/VK resolution, `vk_hash`, schema hash, proof-size caps, backend allowlist, wrapper/header checks | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Wrapper and registry binding stay stronger than the standalone helper path. |
| `IvmProved` admission | Registry-backed `halo2/ipa` or `stark/fri-v1/*` | Consensus-critical | `vk_hash`, canonical `ivm-execution` schema hash, circuit id, namespace / manifest match, curve / `k` caps, payload header validation | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. Admission remains pinned to the guarded runtime verifier. |
| Kaigi privacy join / leave / usage | Registry-backed `halo2/ipa` | Consensus-critical for private Kaigi flows | VK registry record, `vk_hash`, schema hash, circuit id, active status | `iroha_core::zk::verify_backend_with_timing_guardrails` | Low. No standalone helper bypass in the production path. |
| RAM-LFE execution receipts | Signed receipt, not a proof verifier | Non-consensus helper / application-facing | Resolver public-key match, signed payload fields, verification mode checks | Signature verification, not a ZK backend | Not affected by transcript-binding bugs; residual risk is signer/key policy, not proof binding. |
| Identifier receipts | Signed receipt layered on RAM-LFE execution payload | Application-facing | Resolver public-key match, signed receipt payload, policy/program linkage | Signature verification, not a ZK backend | Not affected by transcript-binding bugs; residual risk is receipt policy/signing, not ZK verification. |
| Lane relay / FASTPQ | Native FASTPQ prover/verifier | Safety-critical for lane proof checking | Rebuilt transition batch from binding, full `PublicIO` equality (`dsid`, `slot`, roots, hashes), transcript already seeded with `public_io` | `fastpq_prover::verify` | Medium-low. Claims are now checked field-for-field; remaining risk is in FASTPQ arithmetic/circuit correctness rather than omitted public claims. |
| Torii `POST /v1/zk/verify` | None (decode-only) | Non-consensus demo surface | Content-type decode only | None | High if misinterpreted as a verifier; docs/comments now explicitly mark it decode-only. |
| Torii `POST /v1/zk/submit-proof` | None (decode + deterministic id) | Non-consensus demo surface | Content-type decode only, body hash for returned id | None | High if misinterpreted as a verifier; docs/comments now explicitly mark it non-verifying and non-durable. |
| Torii `POST /v1/zk/verify-batch` | Standalone native IPA poly-open helper | Diagnostic only, not ledger-equivalent | Norito envelope decode plus transcript-bound statement (`transcript_label`, curve/`n`, `z`, `t`, `p_g`, optional metadata) | `iroha_zkp_halo2::batch::verify_open_batch` | Medium. Cryptographically stricter after this patch, but still intentionally lacks VK registry / circuit/schema policy enforcement. |
| IVM batch syscall (`SYSCALL_ZK_VERIFY_BATCH`) | Standalone native IPA poly-open helper behind host gating | Runtime helper, but narrower than ledger verifier | Host gating for enablement / curve / max-`k` / batch size plus transcript-bound native envelope metadata | `ivm::zk_verify::batch_verify_open_envelopes` -> `iroha_zkp_halo2::batch::verify_open_batch` | Medium-low. Statement binding is fixed; residual difference versus ledger verification is that this path is still a standalone helper without registry policy. |

## Notes

- The strongest OtterSec-style risk in this repo was the standalone native IPA
  helper, because it previously derived Fiat-Shamir challenges without binding
  the full public statement. That helper now binds `transcript_label`,
  backend/domain size, `z`, `t`, `p_g`, and any optional metadata carried in
  `OpenVerifyEnvelope`.
- FASTPQ already seeded Fiat-Shamir with `public_io`, so its issue was not the
  same bug class. The hardening here closes the verifier-side claim-validation
  gap by requiring field-for-field `PublicIO` equality before accepting the
  proof.
- Production ledger verification remains centered on the guarded
  `iroha_core::zk::verify_backend_with_timing_guardrails` path and should stay
  the reference implementation for future proof-bearing features.
