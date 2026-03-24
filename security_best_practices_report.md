# Security Best Practices Report

Date: 2026-03-24

## Executive Summary

I reviewed the highest-risk Rust surfaces in this workspace, with emphasis on Torii HTTP ingress, Soracloud local-read/proxy execution, and the ZK attachment store. I found four concrete security issues: two high severity, one medium-high, and one medium. The most serious issue is that Soracloud mutation endpoints currently require callers to transmit raw account private keys to Torii so the server can sign transactions on their behalf.

This audit was code-focused rather than exhaustive. I prioritized externally reachable and recently changed control-plane/runtime paths over the full consensus, codec, and SDK surface.

## High Severity

### SEC-01: Soracloud control-plane mutations require callers to send raw private keys to Torii

Impact: Compromise of Torii, any upstream proxy, request-body logging, tracing, or crash dumps can expose long-lived account private keys that authorize arbitrary on-chain actions beyond the single mutation request.

Evidence:

- Multiple Soracloud mutation DTOs embed `private_key: Option<ExposedPrivateKey>`, for example `SignedBundleRequest` and `SignedRollbackRequest` in `crates/iroha_torii/src/soracloud.rs:339-346` and `crates/iroha_torii/src/soracloud.rs:356-363`.
- Torii refuses to process mutations unless that private key is present and matches the supplied authority in `crates/iroha_torii/src/soracloud.rs:5200-5217`.
- Torii then signs the on-chain transaction server-side with that HTTP-supplied key in `crates/iroha_torii/src/soracloud.rs:5404-5406`.
- The deploy path is representative: after verifying provenance, it still extracts the caller private key and uses it for transaction submission in `crates/iroha_torii/src/soracloud.rs:8671-8692`.

Why this matters:

- Provenance signatures already prove intent for the control-plane payload. Requiring the raw account private key as an additional HTTP field turns Torii into a remote signing oracle.
- API-token protection is not equivalent to key protection. Once Torii can see the private key, every layer that can inspect traffic or process memory becomes part of the signing trust boundary.

Recommendation:

- Remove `private_key` from Soracloud HTTP DTOs.
- Accept either fully signed transactions or a client-signed mutation envelope that Torii verifies but never re-signs.
- If delegated signing is absolutely required, move it behind a KMS/HSM boundary and have Torii reference a signer handle, never the raw key material.

## High Severity

### SEC-02: Any peer can trigger Soracloud proxy execution for arbitrary local-read handlers, including internal-only services

Impact: A malicious or compromised peer can make the authoritative primary execute local reads and inference workloads that never passed Torii’s public-route admission checks, including handlers on services that are marked internal-only.

Evidence:

- Incoming Soracloud proxy requests are executed directly from the peer bus in `crates/iroha_torii/src/lib.rs:8489-8527`.
- The runtime accepts the supplied `SoracloudLocalReadRequest` and executes it after only snapshot/context validation in `crates/irohad/src/soracloud_runtime.rs:425-448`.
- Local-read context resolution checks the requested service, version, and handler class/name, but it does not check route visibility, host, or whether the request originated from a Torii-admitted public route in `crates/irohad/src/soracloud_runtime.rs:5369-5464`.
- Generated HF services are explicitly created with `SoraRouteVisibilityV1::Internal` in `crates/iroha_core/src/soracloud_runtime.rs:178-183`.

Why this matters:

- The HTTP ingress path narrows exposure by resolving only public routes before constructing a request. The peer proxy path skips that boundary entirely.
- In a Byzantine setting, some peers are expected to be faulty. Treating peer-originated application requests as implicitly authorized is too strong.

Recommendation:

- Restrict proxy execution to a narrow, explicit request type for the exact generated-HF `/infer` flow.
- Bind each proxied request to a Torii-generated admission token over the request commitment and verify that token on the primary before execution.
- Reject proxy requests for handlers whose route visibility is not public and do not allow arbitrary service/handler selection from the peer bus.

## Medium-High Severity

### SEC-03: Public Soracloud runtime fallback bypasses Torii’s normal API-token and rate-limit checks

Impact: Unauthenticated callers can hit expensive public local-read and inference handlers without passing through the access-control and costed rate-limiting path used by the rest of Torii, making application-layer denial of service materially easier.

Evidence:

- Soracloud public runtime handling is installed as a router-wide fallback in `crates/iroha_torii/src/lib.rs:17561-17563`.
- The fallback handler resolves the route and executes the local read or primary proxy request directly, but it never calls `check_access`, `check_access_enforced`, or any limiter in `crates/iroha_torii/src/lib.rs:8674-8736`.
- The normal access path applies API-token validation and a token-bucket limiter in `crates/iroha_torii/src/lib.rs:2280-2338`.

Why this matters:

- Public Soracloud reads can fan out into on-node query work, asset assembly, or model inference/proxy execution. Those are much more expensive than lightweight JSON metadata endpoints.
- A catch-all fallback is especially risky because new unmatched public paths automatically inherit this unmetered execution path.

Recommendation:

- Replace the fallback with explicit public runtime routes or a dedicated sub-router.
- Apply a distinct public-runtime limiter keyed by remote IP or another tenant identity before request parsing and before proxying to the primary.
- Add concurrency caps and endpoint-specific cost accounting for inference traffic.

## Medium Severity

### SEC-04: ZK attachment isolation collapses to remote IP when API tokens are disabled

Impact: Different users behind the same NAT, office proxy, mobile carrier gateway, or other shared egress can list, fetch, count, and delete each other’s attachments.

Evidence:

- The attachment tenant model is documented and implemented as “validated API token or remote IP” in `crates/iroha_torii/src/zk_attachments.rs:53-75`.
- Tenant derivation falls back to `from_remote_ip` and then `anonymous` in `crates/iroha_torii/src/lib.rs:7851-7867`.
- Attachment CRUD handlers use only that derived tenant after the generic access check in `crates/iroha_torii/src/lib.rs:7870-7926`.
- Generic access checks enforce an API token only when `torii.require_api_token` is enabled in `crates/iroha_torii/src/lib.rs:2289-2338`.

Why this matters:

- IP address is not a stable security principal. Shared networks are common, and reverse proxies can collapse many end users into one apparent source.
- This is a confidentiality and integrity issue, not just a quota-accounting quirk, because the same tenant key gates read and delete operations.

Recommendation:

- Require a real caller identity for attachment access, such as a validated API token or a signed account identity.
- If anonymous uploads must remain supported, separate upload from read/delete and issue unguessable per-object capability tokens instead of storing all anonymous users in a shared namespace.

## Residual Risks And Coverage Gaps

- I did not perform an exhaustive audit of consensus, Norito codec internals, the Swift/Android SDKs, or every Sorafs/Soracloud code path.
- I did not run `cargo test`, `cargo clippy`, or dynamic probes because this was a read-only security review.
- The findings above are grounded in concrete code paths and should be treated as fix candidates even if broader review later uncovers additional issues.
