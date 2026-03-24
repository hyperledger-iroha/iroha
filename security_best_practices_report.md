# Security Best Practices Report

Date: 2026-03-24

## Executive Summary

I refreshed the earlier Torii/Soracloud report against the current workspace
code and extended the review across the highest-risk server, SDK, and
crypto/serialization surfaces. This audit initially confirmed three
ingress/authentication issues: two high severity and one medium severity.
Those three findings are now closed in the current tree by the remediation
described below.

The four previously reported Soracloud findings about raw private keys over
HTTP, internal-only local-read proxy execution, unmetered public-runtime
fallback, and remote-IP attachment tenancy are no longer live in current code.
Those are marked closed/superseded below with updated code references.

This remained a code-centric audit rather than an exhaustive red-team exercise.
I prioritized externally reachable Torii ingress and request-auth paths, then
spot-checked IVM, `iroha_crypto`, `norito`, and the Swift/Android/JS SDK
request-signing helpers. No live confirmed issue from that ingress/auth slice
remains after the fixes in this report. Follow-up hardening also expanded the
fail-closed startup truth sets for sampled IVM CUDA/Metal accelerator paths;
that work did not confirm a new fail-open issue. The sampled Metal Ed25519
signature path is now restored on this host after fixing multiple ref10 drift
points in the Metal/CUDA ports: positive-basepoint handling in verification,
the `d2` constant, the exact `fe_sq2` reduction path, the stray final
`fe_mul` carry step, and the missing post-op field normalization that let limb
bounds drift across the scalar ladder. Focused Metal regression coverage now
keeps the signature pipeline enabled and verifies `[true, false]` on the
accelerator against the CPU reference path. The sampled startup truth sets now
also directly probe the live vector (`vadd64`, `vand`, `vxor`, `vor`) and
single-round AES batch kernels on both Metal and CUDA before those backends
stay enabled. The remaining accelerator work is runtime validation of the
mirrored CUDA fix and the expanded CUDA truth set on a host with live CUDA
driver support, not a confirmed correctness or fail-open issue in the current
tree.

## High Severity

### SEC-05: App canonical request verification bypassed multisig thresholds (Closed 2026-03-24)

Impact:

- Any single member key of a multisig-controlled account can authorize
  app-facing requests that are supposed to require a threshold or weighted
  quorum.
- This affects every endpoint that trusts `verify_canonical_request`, including
  Soracloud signed mutation ingress, content access, and signed-account ZK
  attachment tenancy.

Evidence:

- `verify_canonical_request` expands a multisig controller into the full member
  public-key list and accepts the first key that verifies the request
  signature, without evaluating threshold or accumulated weight:
  `crates/iroha_torii/src/app_auth.rs:198-210`.
- The actual multisig policy model carries both a `threshold` and weighted
  members, and rejects policies whose threshold exceeds total weight:
  `crates/iroha_data_model/src/account/controller.rs:92-95`,
  `crates/iroha_data_model/src/account/controller.rs:163-178`,
  `crates/iroha_data_model/src/account/controller.rs:188-196`.
- The helper is on the authorization path for Soracloud mutation ingress in
  `crates/iroha_torii/src/lib.rs:2141-2157`, content signed-account access in
  `crates/iroha_torii/src/content.rs:359-360`, and attachment tenancy in
  `crates/iroha_torii/src/lib.rs:7962-7968`.

Why this matters:

- The request signer is treated as the account authority for HTTP admission,
  but the implementation silently downgrades multisig accounts to "any single
  member may act alone."
- That turns a defense-in-depth HTTP signature layer into an authorization
  bypass for multisig-protected accounts.

Recommendation:

- Either reject multisig-controlled accounts at the app-auth layer until a
  proper witness format exists, or extend the protocol so the HTTP request
  carries and verifies a full multisig witness set that satisfies threshold and
  weight.
- Add regressions covering Soracloud mutation middleware, content auth, and ZK
  attachments for below-threshold multisig signatures.

Remediation status:

- Closed in current code by failing closed on multisig-controlled accounts in
  `crates/iroha_torii/src/app_auth.rs`.
- The verifier no longer accepts "any single member may sign" semantics for
  multisig HTTP authorization; multisig requests are rejected until a
  threshold-satisfying witness format exists.
- Regression coverage now includes a dedicated multisig rejection case in
  `crates/iroha_torii/src/app_auth.rs`.

## High Severity

### SEC-06: App canonical request signatures were replayable indefinitely (Closed 2026-03-24)

Impact:

- A captured valid request can be replayed because the signed message has no
  timestamp, nonce, expiry, or replay cache.
- This can repeat state-changing Soracloud mutation requests and reissue
  account-bound content/attachment operations long after the original client
  intended them.

Evidence:

- Torii defines the app canonical request as only
  `METHOD + path + sorted query + body hash` in
  `crates/iroha_torii/src/app_auth.rs:1-17` and
  `crates/iroha_torii/src/app_auth.rs:74-89`.
- The verifier accepts only `X-Iroha-Account` and `X-Iroha-Signature` and does
  not enforce freshness or maintain a replay cache:
  `crates/iroha_torii/src/app_auth.rs:137-218`.
- The JS, Swift, and Android SDK helpers generate the same replay-prone header
  pair with no nonce/timestamp fields:
  `javascript/iroha_js/src/canonicalRequest.js:50-82`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68`, and
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`.
- Torii's operator-signature path already uses the stronger pattern the
  app-facing path is missing: timestamp, nonce, and replay cache in
  `crates/iroha_torii/src/operator_signatures.rs:1-21` and
  `crates/iroha_torii/src/operator_signatures.rs:266-294`.

Why this matters:

- HTTPS alone does not prevent replay by a reverse proxy, debug logger,
  compromised client host, or any intermediary that can record valid requests.
- Because the same scheme is implemented in all major client SDKs, the replay
  weakness is systemic rather than server-only.

Recommendation:

- Add signed freshness material to app-auth requests, at minimum a timestamp
  and nonce, and reject stale or reused tuples with a bounded replay cache.
- Version the app canonical-request format explicitly so Torii and the SDKs can
  deprecate the old two-header scheme safely.
- Add regressions proving replay rejection for Soracloud mutations, content
  access, and attachment CRUD.

Remediation status:

- Closed in current code. Torii now requires the four-header scheme
  (`X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`,
  `X-Iroha-Nonce`) and signs/verifies
  `METHOD + path + sorted query + body hash + timestamp + nonce` in
  `crates/iroha_torii/src/app_auth.rs`.
- Freshness validation now enforces a bounded clock-skew window, validates
  nonce shape, and rejects reused nonces with an in-memory replay cache whose
  knobs are surfaced through `crates/iroha_config/src/parameters/{defaults,actual,user}.rs`.
- The JS, Swift, and Android helpers now emit the same four-header format in
  `javascript/iroha_js/src/canonicalRequest.js`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift`, and
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`.
- Regression coverage now includes positive signature verification plus replay,
  stale-timestamp, and missing-freshness rejection cases in
  `crates/iroha_torii/src/app_auth.rs`.

## Medium Severity

### SEC-07: mTLS enforcement trusted a spoofable forwarded header (Closed 2026-03-24)

Impact:

- Deployments that rely on `require_mtls` can be bypassed if Torii is directly
  reachable or the front proxy does not strip client-supplied
  `x-forwarded-client-cert`.
- The issue is configuration-dependent, but when triggered it turns a claimed
  client-certificate requirement into a plain header check.

Evidence:

- Norito-RPC gating enforces `require_mtls` by calling
  `norito_rpc_mtls_present`, which only checks whether
  `x-forwarded-client-cert` exists and is non-empty:
  `crates/iroha_torii/src/lib.rs:1897-1926`.
- Operator-auth bootstrap/login flows call `check_common`, which rejects only
  when `mtls_present(headers)` is false:
  `crates/iroha_torii/src/operator_auth.rs:562-570`.
- `mtls_present` is also only a non-empty `x-forwarded-client-cert` check in
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`.
- Those operator-auth handlers are still exposed as routes at
  `crates/iroha_torii/src/lib.rs:16658-16672`.

Why this matters:

- A forwarded-header convention is only trustworthy when Torii sits behind a
  hardened proxy that strips and rewrites the header. The code does not verify
  that deployment assumption itself.
- Security controls that silently depend on reverse-proxy hygiene are easy to
  misconfigure during staging, canary, or incident-response routing changes.

Recommendation:

- Prefer direct transport-state enforcement where possible. If a proxy must be
  used, trust an authenticated proxy-to-Torii channel and require an allow-list
  or signed attestation from that proxy instead of raw header presence.
- Document that `require_mtls` is unsafe on directly exposed Torii listeners.
- Add negative tests for forged `x-forwarded-client-cert` input on Norito-RPC
  and operator-auth bootstrap routes.

Remediation status:

- Closed in current code by binding forwarded-header trust to configured proxy
  CIDRs instead of raw header presence alone.
- `crates/iroha_torii/src/limits.rs` now provides the shared
  `has_trusted_forwarded_header(...)` gate, and both Norito-RPC
  (`crates/iroha_torii/src/lib.rs`) and operator-auth
  (`crates/iroha_torii/src/operator_auth.rs`) use it with the caller TCP peer
  address.
- `iroha_config` now exposes `mtls_trusted_proxy_cidrs` for both
  operator-auth and Norito-RPC; defaults are loopback-only.
- Regression coverage now rejects forged `x-forwarded-client-cert` input from
  an untrusted remote in both operator-auth and the shared limits helper.

## Closed Or Superseded Findings From The Earlier Report

- Earlier raw-private-key Soracloud finding: closed. Current mutation ingress
  rejects inline `authority` / `private_key` fields in
  `crates/iroha_torii/src/soracloud.rs:5305-5308`, binds the HTTP signer to the
  mutation provenance in `crates/iroha_torii/src/soracloud.rs:5310-5315`, and
  returns draft transaction instructions instead of server-submitting a signed
  transaction in `crates/iroha_torii/src/soracloud.rs:5556-5565`.
- Earlier internal-only local-read proxy execution finding: closed. Public
  route resolution now skips non-public and update/private-update handlers in
  `crates/iroha_torii/src/soracloud.rs:8445-8463`, and the runtime rejects
  non-public local-read routes in
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`.
- Earlier public-runtime unmetered fallback finding: closed as written. Public
  runtime ingress now enforces rate limits and inflight caps in
  `crates/iroha_torii/src/lib.rs:8837-8852` before resolving a public route in
  `crates/iroha_torii/src/lib.rs:8858-8860`.
- Earlier remote-IP attachment-tenancy finding: closed. Attachment tenancy now
  requires a verified signed account in
  `crates/iroha_torii/src/lib.rs:7962-7968`.
  Attachment tenancy previously inherited SEC-05 and SEC-06; that inheritance
  is closed by the current app-auth remediations above.

## Coverage Notes

- Server/runtime/config/networking: SEC-05, SEC-06, and SEC-07 were confirmed
  during the audit and are now closed in the current tree.
- IVM/crypto/serialization: no additional confirmed finding from this audit
  slice. Positive evidence includes confidential key material zeroization in
  `crates/iroha_crypto/src/confidential.rs:53-60` and replay-aware Soranet PoW
  signed-ticket validation in `crates/iroha_crypto/src/soranet/pow.rs:823-879`.
  Follow-up hardening now also rejects malformed accelerator output in two
  sampled Norito paths: `crates/norito/src/lib.rs` validates accelerated JSON
  Stage-1 tapes before `TapeWalker` dereferences offsets and now also requires
  dynamically loaded Metal/CUDA Stage-1 helpers to prove parity with the
  scalar structural-index builder before activation, and
  `crates/norito/src/core/gpu_zstd.rs` validates GPU-reported output lengths
  before truncating encode/decode buffers. `crates/norito/src/core/simd_crc64.rs`
  now also self-tests dynamically loaded GPU CRC64 helpers against the
  canonical fallback before `hardware_crc64` will trust them, so malformed
  helper libraries fail closed instead of silently changing Norito checksum
  behavior. Invalid helper results now fall back instead of panicking release
  builds or drifting checksum parity. On the IVM side, sampled accelerator
  startup gates now also cover the CUDA Ed25519 `signature_kernel`, CUDA BN254
  add/sub/mul kernels, CUDA `sha256_leaves` / `sha256_pairs_reduce`, the live
  CUDA vector/AES batch kernels (`vadd64`, `vand`, `vxor`, `vor`,
  `aesenc_batch`, `aesdec_batch`), and the matching Metal
  `sha256_leaves`/vector/AES batch kernels before those paths are trusted. The
  sampled Metal Ed25519 signature path is now also back
  inside the live accelerator set on this host: the earlier parity failure was
  fixed by restoring ref10 limb-bound normalization across the scalar ladder,
  and the focused Metal regression now verifies `[s]B`, `[h](-A)`, the
  power-of-two basepoint ladder, and full `[true, false]` batch verification
  on Metal against the CPU reference path. The mirrored CUDA source changes
  compile under `--features cuda --tests`, and the CUDA startup truth set now
  fails closed if the live Merkle leaf/pair kernels drift from the CPU
  reference path. Runtime CUDA validation remains host-limited in this
  environment.
- SDKs/examples: no separate key-storage or transport-validation bug was
  confirmed in the sampled code. The JS, Swift, and Android canonical-request
  helpers have been updated to the new freshness-aware four-header scheme.
- Examples and mobile sample apps were reviewed only at a spot-check level and
  should not be treated as exhaustively audited.

## Validation And Coverage Gaps

- `cargo deny check advisories bans sources` could not run because
  `cargo-deny` is not installed in the environment.
- `bash scripts/fuzz_smoke.sh` returned successfully but only reported that
  `cargo-fuzz` is not installed, so no fuzz smoke actually ran.
- Torii remediation validation now includes:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
- SDK-side remediation validation now includes:
  - `node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `cd IrohaSwift && swift test --filter CanonicalRequestTests`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac`
- Norito follow-up validation now includes:
  - `python3 scripts/check_norito_bindings_sync.py`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
- IVM accelerator follow-up validation now includes:
  - `xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- Focused CUDA lib-test execution remains environment-limited on this host:
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  still fails to link because the CUDA driver symbols (`cu*`) are unavailable.
- Focused Metal runtime validation now runs fully on the accelerator on this
  host: the sampled Ed25519 signature pipeline stays enabled through startup
  self-tests, and `metal_ed25519_batch_matches_cpu` verifies `[true, false]`
  directly on Metal against the CPU reference path.
- I did not rerun a full workspace Rust test sweep, full `npm test`, or the
  full Swift/Android suites during this remediation pass.

## Prioritized Remediation Backlog

### Next Tranche

- Rerun the focused CUDA lib-test self-test slice on a host with CUDA driver
  libraries installed, so the expanded CUDA startup truth set is validated
  beyond `cargo check` and the mirrored Ed25519 normalization fix plus the
  new vector/AES startup probes are exercised at runtime.
- Rerun broader JS/Swift/Android suites once the unrelated suite-level blockers
  on this branch are cleared, so the new canonical-request protocol is covered
  beyond the focused helper tests above.
- Decide whether the long-term app-auth multisig story should remain
  fail-closed or grow a first-class HTTP multisig witness format.

### Monitor

- Continue the focused review of `ivm` hardware-acceleration / unsafe paths
  and the remaining `norito` streaming/crypto boundaries. The JSON Stage-1
  and GPU zstd helper handoffs have now been hardened to fail closed in
  release builds, and the sampled IVM accelerator startup truth sets are now
  broader, but the wider unsafe/determinism review is still open.
