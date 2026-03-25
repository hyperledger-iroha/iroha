# Security Best Practices Report

Date: 2026-03-25

## Executive Summary

I refreshed the earlier Torii/Soracloud report against the current workspace
code and extended the review across the highest-risk server, SDK, and
crypto/serialization surfaces. This audit initially confirmed three
ingress/authentication issues: two high severity and one medium severity.
Those three findings are now closed in the current tree by the remediation
described below. Follow-up transport and internal-dispatch review confirmed
eight additional medium-severity issues: one outbound P2P identity-binding
gap, one outbound P2P TLS-downgrade default, two Torii trust-boundary bugs in
webhook delivery and MCP internal dispatch, one cross-SDK sensitive-transport
gap in the Swift, Java/Android, Kotlin, and JS clients, one SoraFS
trusted-proxy/client-IP policy gap, one SoraFS local-proxy
bind/authentication gap, and one peer-telemetry geo-lookup plaintext
fallback. Those later findings are also closed in the current tree.

The four previously reported Soracloud findings about raw private keys over
HTTP, internal-only local-read proxy execution, unmetered public-runtime
fallback, and remote-IP attachment tenancy are no longer live in current code.
Those are marked closed/superseded below with updated code references.

This remained a code-centric audit rather than an exhaustive red-team exercise.
I prioritized externally reachable Torii ingress and request-auth paths, then
spot-checked IVM, `iroha_crypto`, `norito`, the Swift/Android/JS SDK
request-signing helpers, the peer-telemetry geo path, and the SoraFS
workstation proxy helpers plus the SoraFS pin/gateway client-IP policy
surfaces. No live confirmed issue from that ingress/auth, egress-policy,
peer-telemetry geo, sampled P2P transport defaults, MCP-dispatch, sampled SDK
transport, SoraFS trusted-proxy/client-IP policy, or local-proxy slice
remains after the fixes in this report.
Follow-up hardening also expanded the fail-closed startup truth sets for
sampled IVM CUDA/Metal accelerator paths; that work did not confirm a new
fail-open issue. The sampled Metal Ed25519
signature path is now restored on this host after fixing multiple ref10 drift
points in the Metal/CUDA ports: positive-basepoint handling in verification,
the `d2` constant, the exact `fe_sq2` reduction path, the stray final
`fe_mul` carry step, and the missing post-op field normalization that let limb
bounds drift across the scalar ladder. Focused Metal regression coverage now
keeps the signature pipeline enabled and verifies `[true, false]` on the
accelerator against the CPU reference path. The sampled startup truth sets now
also directly probe the live vector (`vadd64`, `vand`, `vxor`, `vor`) and
single-round AES batch kernels on both Metal and CUDA before those backends
stay enabled. Later dependency scanning added seven live third-party findings
to the backlog, but the current tree has since removed both active `tar`
advisories by dropping the `xtask` Rust `tar` dependency and replacing the
`iroha_crypto` `libsodium-sys-stable` interop tests with OpenSSL-backed
equivalents. The remaining dependency triage now consists of one
runtime-reachable but precondition-gated TLS advisory (`rustls-webpki`), two
direct PQ replacement advisories (`pqcrypto-dilithium`, `pqcrypto-kyber`), and
two transitive unmaintained macro crates (`derivative`, `paste`). The
remaining accelerator work is
runtime validation of the mirrored CUDA fix and the expanded CUDA truth set on
a host with live CUDA driver support, not a confirmed correctness or fail-open
issue in the current tree.

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

## Medium Severity

### SEC-08: Outbound P2P dials did not bind the authenticated key to the intended peer id (Closed 2026-03-25)

Impact:

- An outbound dial to peer `X` could complete as any other peer `Y` whose key
  successfully signed the app-layer handshake, because the handshake
  authenticated "a key on this connection" but never checked that the key was
  the peer id the network actor intended to reach.
- In permissioned overlays the later topology/allow-list checks still drop the
  wrong key, so this was primarily a substitution / reachability bug rather
  than a direct consensus impersonation bug. In public overlays it could let a
  compromised address, DNS response, or relay endpoint substitute a different
  observer identity on an outbound dial.

Evidence:

- The outbound peer state stores the intended `peer_id` in
  `crates/iroha_p2p/src/peer.rs:5153-5179`, but the old handshake flow
  dropped that value before signature verification.
- `GetKey::read_their_public_key` verified the signed handshake payload and
  then immediately constructed a `Peer` from the advertised remote public key
  in `crates/iroha_p2p/src/peer.rs:6266-6355`, with no comparison against the
  `peer_id` originally supplied to `connecting(...)`.
- The same transport stack explicitly disables TLS / QUIC certificate
  verification for P2P in `crates/iroha_p2p/src/transport.rs`, so binding the
  application-layer authenticated key to the intended peer id is the critical
  identity check on outbound connections.

Why this matters:

- The design intentionally pushes peer authentication above the transport
  layer, which makes the handshake key check the only durable identity binding
  on outbound dials.
- Without that check, the network layer could silently treat "successfully
  authenticated some peer" as equivalent to "reached the peer we dialed,"
  which is a weaker guarantee and can distort topology/reputation state.

Recommendation:

- Carry the intended outbound `peer_id` through the signed-handshake stages and
  fail closed if the verified remote key does not match it.
- Keep a focused regression that proves a validly signed handshake from the
  wrong key is rejected while a normal signed handshake still succeeds.

Remediation status:

- Closed in current code. `ConnectedTo` and the downstream handshake states now
  carry the expected outbound `PeerId`, and
  `GetKey::read_their_public_key` rejects a mismatched authenticated key with
  `HandshakePeerMismatch` in `crates/iroha_p2p/src/peer.rs`.
- Focused regression coverage now includes
  `outgoing_handshake_rejects_unexpected_peer_identity` and the existing
  positive `handshake_v1_defaults_to_trust_gossip` path in
  `crates/iroha_p2p/src/peer.rs`.

### SEC-09: HTTPS/WSS webhook delivery re-resolved vetted hostnames at connect time (Closed 2026-03-25)

Impact:

- Secure webhook delivery validated destination DNS answers against the webhook
  egress policy, but then discarded those vetted addresses and let the client
  stack resolve the hostname again during the actual HTTPS or WSS connect.
- An attacker who can influence DNS between validation and connect time could
  potentially rebind a previously allowed hostname onto a blocked private or
  operator-only destination and bypass the CIDR-based webhook guard.

Evidence:

- The egress guard resolves and filters candidate destination addresses in
  `crates/iroha_torii/src/webhook.rs:1746-1829`, and the secure delivery paths
  pass those vetted address lists into the HTTPS / WSS helpers.
- The old HTTPS helper then built a generic client against the original URL
  host in `crates/iroha_torii/src/webhook.rs` and did not bind the connection
  to the vetted address set, which meant DNS resolution happened again inside
  the HTTP client.
- The old WSS helper likewise called `tokio_tungstenite::connect_async(url)`
  against the original hostname, which also re-resolved the host instead of
  reusing the already approved address.

Why this matters:

- Destination allow-lists only work if the address that was checked is the one
  the client actually connects to.
- Re-resolving after policy approval creates a DNS rebinding / TOCTOU gap on a
  path that operators are likely to trust for SSRF-style containment.

Recommendation:

- Pin vetted DNS answers into the actual HTTPS connect path while preserving
  the original hostname for SNI / certificate validation.
- For WSS, connect the TCP socket directly to a vetted address and run the TLS
  websocket handshake over that stream instead of calling a hostname-based
  convenience connector.

Remediation status:

- Closed in current code. `crates/iroha_torii/src/webhook.rs` now derives
  `https_delivery_dns_override(...)` and
  `websocket_pinned_connect_addr(...)` from the vetted address set.
- HTTPS delivery now uses `reqwest::Client::builder().resolve_to_addrs(...)`
  so the original hostname stays visible to TLS while the TCP connect is
  pinned to the already approved addresses.
- WSS delivery now opens a raw `TcpStream` to a vetted address and performs
  `tokio_tungstenite::client_async_tls_with_config(...)` over that stream,
  which avoids a second DNS lookup after policy validation.
- Regression coverage now includes
  `https_delivery_dns_override_pins_vetted_domain_addresses`,
  `https_delivery_dns_override_skips_ip_literals`, and
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` in
  `crates/iroha_torii/src/webhook.rs`.

### SEC-10: MCP internal route dispatch stamped loopback and inherited allow-list privilege (Closed 2026-03-25)

Impact:

- When Torii MCP was enabled, internal tool dispatch rewrote every request as
  loopback regardless of the actual caller. Routes that trust caller CIDR for
  privilege or throttling bypass could therefore see MCP traffic as
  `127.0.0.1`.
- The issue was configuration-gated because MCP is disabled by default and the
  affected routes still depend on an allow-list or similar loopback trust
  policy, but it turned MCP into a privilege-escalation bridge once those
  knobs were enabled together.

Evidence:

- `dispatch_route(...)` in `crates/iroha_torii/src/mcp.rs` previously inserted
  `x-iroha-remote-addr: 127.0.0.1` and synthetic loopback `ConnectInfo` for
  every internally dispatched request.
- `iroha.parameters.get` is exposed on the MCP surface in read-only mode, and
  `/v1/parameters` bypasses normal auth when the caller IP belongs to the
  configured allow-list in `crates/iroha_torii/src/lib.rs:5879-5888`.
- `apply_extra_headers(...)` also accepted arbitrary `headers` entries from
  the MCP caller, so reserved internal trust headers such as
  `x-iroha-remote-addr` and `x-forwarded-client-cert` were not explicitly
  protected.

Why this matters:

- Internal bridge layers must preserve the original trust boundary. Replacing
  the real caller with loopback effectively treats every MCP caller as an
  internal client once the request crosses the bridge.
- The bug is subtle because the externally visible MCP profile can still look
  read-only while the internal HTTP route sees a more privileged origin.

Recommendation:

- Preserve the caller IP that the outer `/v1/mcp` request already received
  from Torii's remote-address middleware and synthesize `ConnectInfo` from
  that value instead of loopback.
- Treat ingress-only trust headers such as `x-iroha-remote-addr` and
  `x-forwarded-client-cert` as reserved internal headers so MCP callers cannot
  smuggle or override them through the `headers` argument.

Remediation status:

- Closed in current code. `crates/iroha_torii/src/mcp.rs` now derives the
  internally dispatched remote IP from the outer request's injected
  `x-iroha-remote-addr` header and synthesizes `ConnectInfo` from that real
  caller IP instead of loopback.
- `apply_extra_headers(...)` now drops both `x-iroha-remote-addr` and
  `x-forwarded-client-cert` as reserved internal headers, so MCP callers
  cannot spoof loopback / ingress-proxy trust through tool arguments.
- Regression coverage now includes
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers`, and
  `apply_extra_headers_blocks_reserved_internal_headers` in
  `crates/iroha_torii/src/mcp.rs`.

### SEC-11: SDK clients allowed sensitive request material over insecure or cross-host transports (Closed 2026-03-25)

Impact:

- The sampled Swift, Java/Android, Kotlin, and JS clients did not
  consistently treat all sensitive request shapes as transport-sensitive.
  Depending on the helper, callers could send bearer/API-token headers, raw
  `private_key*` JSON fields, or app canonical-auth signing material over
  plain `http` / `ws` or through cross-host absolute URL overrides.
- In the JS client specifically, `canonicalAuth` headers were added after
  `_request(...)` finished its transport checks, and body-only `private_key`
  JSON did not count as sensitive transport at all.

Evidence:

- Swift now centralizes the guard in
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` and applies it from
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`,
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, and
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift`; prior to this pass
  those helpers did not share a single transport-policy gate.
- Java/Android now centralizes the same policy in
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  and applies it from `NoritoRpcClient.java`, `ToriiRequestBuilder.java`,
  `OfflineToriiClient.java`, `SubscriptionToriiClient.java`,
  `stream/ToriiEventStreamClient.java`, and
  `websocket/ToriiWebSocketClient.java`.
- Kotlin now mirrors that policy in
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  and applies it from the matching JVM client/request-builder/event-stream /
  websocket surfaces.
- JS `ToriiClient._request(...)` now treats `canonicalAuth` plus JSON bodies
  containing `private_key*` fields as sensitive transport material in
  `javascript/iroha_js/src/toriiClient.js`, and the telemetry event shape in
  `javascript/iroha_js/index.d.ts` now records `hasSensitiveBody` /
  `hasCanonicalAuth` when `allowInsecure` is used.

Why this matters:

- Mobile, browser, and local-dev helpers often point at mutable dev/staging
  base URLs. If the client does not pin sensitive requests to the configured
  scheme/host, a convenience absolute URL override or plain-HTTP base can
  turn the SDK into a secret-exfiltration or signed-request downgrade path.
- The risk is broader than bearer tokens. Raw `private_key` JSON and fresh
  canonical-auth signatures are also security-sensitive on the wire and
  should not silently bypass transport policy.

Recommendation:

- Centralize transport validation in each SDK and apply it before network I/O
  for all sensitive request shapes: auth headers, app canonical-auth signing,
  and raw `private_key*` JSON.
- Keep `allowInsecure` as an explicit local/dev escape hatch only, and emit
  telemetry when callers opt into it.
- Add focused regressions on the shared request builders rather than only on
  higher-level convenience methods, so future helpers inherit the same guard.

Remediation status:

- Closed in current code. The sampled Swift, Java/Android, Kotlin, and JS
  clients now reject insecure or cross-host transports for the sensitive
  request shapes above unless the caller opts into the documented dev-only
  insecure mode.
- Swift focused regressions now cover insecure Norito-RPC auth headers,
  insecure Connect websocket transport, and raw-`private_key` Torii request
  bodies.
- Kotlin focused regressions now cover insecure Norito-RPC auth headers,
  offline/subscription `private_key` bodies, SSE auth headers, and websocket
  auth headers.
- Java/Android focused regressions now cover insecure Norito-RPC auth
  headers, offline/subscription `private_key` bodies, SSE auth headers, and
  websocket auth headers through the shared Gradle harness.
- JS focused regressions now cover insecure and cross-host
  `private_key`-body requests plus insecure `canonicalAuth` requests in
  `javascript/iroha_js/test/transportSecurity.test.js`, while
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` now runs the positive
  canonical-auth path on a secure base URL.

### SEC-12: SoraFS local QUIC proxy accepted non-loopback binds without client authentication (Closed 2026-03-25)

Impact:

- `LocalQuicProxyConfig.bind_addr` could previously be set to `0.0.0.0`, a
  LAN IP, or any other non-loopback address, which exposed the "local"
  workstation proxy as a remotely reachable QUIC listener.
- That listener did not authenticate clients. Any reachable peer that could
  complete the QUIC/TLS session and send a version-matched handshake could
  then open `tcp`, `norito`, `car`, or `kaigi` streams depending on which
  bridge modes were configured.
- In `bridge` mode, that turned operator misconfiguration into a remote TCP
  relay and local-file streaming surface on the operator workstation.

Evidence:

- `LocalQuicProxyConfig::parsed_bind_addr(...)` in
  `crates/sorafs_orchestrator/src/proxy.rs` previously only parsed the socket
  address and did not reject non-loopback interfaces.
- `spawn_local_quic_proxy(...)` in the same file starts a QUIC server with a
  self-signed certificate and `.with_no_client_auth()`.
- `handle_connection(...)` accepted any client whose `ProxyHandshakeV1`
  version matched the single supported protocol version and then entered the
  application stream loop.
- `handle_tcp_stream(...)` dials arbitrary `authority` values via
  `TcpStream::connect(...)`, while `handle_norito_stream(...)`,
  `handle_car_stream(...)`, and `handle_kaigi_stream(...)` stream local files
  from configured spool/cache directories.

Why this matters:

- A self-signed certificate protects server identity only if the client
  chooses to verify it. It does not authenticate the client. Once the proxy
  was reachable off-loopback, the handshake path amounted to version-only
  admission.
- The API and docs describe this helper as a local workstation proxy for
  browser/SDK integrations, so allowing remotely reachable bind addresses was
  a trust-boundary mismatch, not an intended remote-service mode.

Recommendation:

- Fail closed on any non-loopback `bind_addr` so the current helper cannot be
  exposed beyond the local workstation.
- If remote proxy exposure ever becomes a product requirement, introduce
  explicit client authentication/capability admission first instead of
  relaxing the bind guard.

Remediation status:

- Closed in current code. `crates/sorafs_orchestrator/src/proxy.rs` now
  rejects non-loopback bind addresses with `ProxyError::BindAddressNotLoopback`
  before the QUIC listener starts.
- The config field documentation in
  `docs/source/sorafs/developer/orchestrator.md` and
  `docs/portal/docs/sorafs/orchestrator-config.md` now documents
  `bind_addr` as loopback-only.
- Regression coverage now includes
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` and the existing
  positive local bridge test
  `proxy::tests::tcp_stream_bridge_transfers_payload` in
  `crates/sorafs_orchestrator/src/proxy.rs`.

### SEC-13: Outbound P2P TLS-over-TCP silently downgraded to plaintext by default (Closed 2026-03-25)

Impact:

- Enabling `network.tls_enabled=true` did not actually enforce TLS-only
  outbound transport unless operators also discovered and set
  `tls_fallback_to_plain=false`.
- Any TLS handshake failure or timeout on the outbound path therefore
  downgraded the dial to plaintext TCP by default, which removed transport
  confidentiality and integrity against on-path attackers or misbehaving
  middleboxes.
- The signed application handshake still authenticated the peer identity, so
  this was a transport-policy downgrade rather than a peer-spoofing bypass.

Evidence:

- `tls_fallback_to_plain` defaulted to `true` in
  `crates/iroha_config/src/parameters/user.rs`, so the fallback was active
  unless operators explicitly overrode it in config.
- `Connecting::connect_tcp(...)` in `crates/iroha_p2p/src/peer.rs` attempts a
  TLS dial whenever `tls_enabled` is set, but on TLS errors or timeouts it
  logs a warning and falls back to plaintext TCP whenever
  `tls_fallback_to_plain` is enabled.
- The operator-facing sample config in `crates/iroha_kagami/src/wizard.rs` and
  the public P2P transport docs in `docs/source/p2p*.md` also advertised
  plaintext fallback as the default behavior.

Why this matters:

- Once operators turn on TLS, the safer expectation is fail closed: if the TLS
  session cannot be established, the dial should fail rather than quietly
  shedding transport protections.
- Leaving the downgrade on by default makes the deployment sensitive to
  network-path quirks, proxy interference, and active handshake disruption in a
  way that is easy to miss during rollout.

Recommendation:

- Keep plaintext fallback as an explicit compatibility knob, but default it to
  `false` so `network.tls_enabled=true` means TLS-only unless the operator opts
  into downgrade behavior.

Remediation status:

- Closed in current code. `crates/iroha_config/src/parameters/user.rs` now
  defaults `tls_fallback_to_plain` to `false`.
- The default-config fixture snapshot, Kagami sample config, and default-like
  P2P/Torii test helpers now mirror that hardened runtime default.
- The duplicated `docs/source/p2p*.md` docs now describe plaintext fallback as
  an explicit opt-in instead of the shipped default.

### SEC-14: Peer telemetry geo lookup silently fell back to plaintext third-party HTTP (Closed 2026-03-25)

Impact:

- Enabling `torii.peer_geo.enabled=true` without an explicit endpoint caused
  Torii to send peer hostnames to a built-in plaintext
  `http://ip-api.com/json/...` service.
- That leaked peer telemetry targets onto an unauthenticated third-party HTTP
  dependency and let any on-path attacker or compromised endpoint feed forged
  location metadata back into Torii.
- The feature was opt-in, but the public telemetry docs and sample config
  advertised the built-in default, which made the insecure deployment pattern
  likely once operators enabled peer geo lookups.

Evidence:

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` previously defined
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` and
  `construct_geo_query(...)` used that default whenever
  `GeoLookupConfig.endpoint` was `None`.
- The peer telemetry monitor spawns `collect_geo(...)` whenever
  `geo_config.enabled` is true in
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`, so the plaintext
  fallback was reachable in shipped runtime code rather than test-only code.
- The config defaults in `crates/iroha_config/src/parameters/defaults.rs` and
  `crates/iroha_config/src/parameters/user.rs` leave `endpoint` unset, and the
  duplicated telemetry docs in `docs/source/telemetry*.md` plus
  `docs/source/references/peer.template.toml` explicitly documented the
  built-in fallback when operators enabled the feature.

Why this matters:

- Peer telemetry should not silently egress peer hostnames over plaintext HTTP
  to a third-party service once operators flip on a convenience flag.
- A hidden insecure default also undermines change review: operators can
  enable geo lookup without realizing they have introduced external
  third-party metadata disclosure and unauthenticated response handling.

Recommendation:

- Remove the built-in geo endpoint default.
- Require an explicit HTTPS endpoint when peer geo lookups are enabled and
  skip lookups otherwise.
- Keep focused regressions proving missing or non-HTTPS endpoints fail closed.

Remediation status:

- Closed in current code. `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  now rejects missing endpoints with `MissingEndpoint`, rejects non-HTTPS
  endpoints with `InsecureEndpoint`, and skips peer geo lookup instead of
  silently falling back to a plaintext built-in service.
- `crates/iroha_config/src/parameters/user.rs` no longer injects an implicit
  endpoint at parse time, so the unset config state remains explicit all the
  way to runtime validation.
- The duplicated telemetry docs and the canonical sample config in
  `docs/source/references/peer.template.toml` now state that
  `torii.peer_geo.endpoint` must be explicitly configured with HTTPS when the
  feature is enabled.
- Regression coverage now includes
  `construct_geo_query_requires_explicit_endpoint`,
  `construct_geo_query_rejects_non_https_endpoint`,
  `collect_geo_requires_explicit_endpoint_when_enabled`, and
  `collect_geo_rejects_non_https_endpoint` in
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`.

### SEC-15: SoraFS pin and gateway policy lacked trusted-proxy-aware client-IP resolution (Closed 2026-03-25)

Impact:

- Reverse-proxied Torii deployments could previously collapse distinct SoraFS
  callers onto the proxy socket address when evaluating storage-pin CIDR
  allow-lists, storage-pin per-client throttles, and gateway client
  fingerprints.
- That weakened abuse controls on `/v1/sorafs/storage/pin` and the SoraFS
  gateway download surfaces in common reverse-proxy topologies by making
  multiple clients share one bucket or one allow-list identity.
- The default router still injects internal remote metadata, so this was not a
  fresh unauthenticated ingress bypass, but it was a real trust-boundary gap
  for proxy-aware deployments and a handler-boundary over-trust of the
  internal remote-IP header.

Evidence:

- `crates/iroha_config/src/parameters/user.rs` and
  `crates/iroha_config/src/parameters/actual.rs` previously had no general
  `torii.transport.trusted_proxy_cidrs` knob, so proxy-aware canonical client
  IP resolution was not configurable at the general Torii ingress boundary.
- `inject_remote_addr_header(...)` in `crates/iroha_torii/src/lib.rs`
  previously overwrote the internal `x-iroha-remote-addr` header from
  `ConnectInfo` alone, which dropped trusted forwarded client-IP metadata from
  real reverse proxies.
- `PinSubmissionPolicy::enforce(...)` in
  `crates/iroha_torii/src/sorafs/pin.rs` and
  `gateway_client_fingerprint(...)` in `crates/iroha_torii/src/sorafs/api.rs`
  did not share a trusted-proxy-aware canonical-IP resolution step at the
  handler boundary.
- Storage-pin throttling in `crates/iroha_torii/src/sorafs/pin.rs` also keyed
  on bearer token alone whenever a token was present, which meant multiple
  proxied clients sharing one valid pin token were forced into the same rate
  bucket even after their client IPs were distinguished.

Why this matters:

- Reverse proxies are a normal deployment pattern for Torii. If the runtime
  cannot consistently distinguish a trusted proxy from an untrusted caller, IP
  allow-lists and per-client throttles stop meaning what operators think they
  mean.
- SoraFS pin and gateway paths are explicitly abuse-sensitive surfaces, so
  collapsing callers into the proxy IP or over-trusting stale forwarded
  metadata is operationally significant even when the base route still
  requires other admission.

Recommendation:

- Add a general Torii `trusted_proxy_cidrs` config surface and resolve the
  canonical client IP once from `ConnectInfo` plus any pre-existing forwarded
  header only when the socket peer is in that allow-list.
- Reuse that canonical-IP resolution inside SoraFS handler paths instead of
  blindly trusting the internal header.
- Scope shared-token storage-pin throttles by token plus canonical client IP
  when both are present.

Remediation status:

- Closed in current code. `crates/iroha_config/src/parameters/defaults.rs`,
  `crates/iroha_config/src/parameters/user.rs`, and
  `crates/iroha_config/src/parameters/actual.rs` now expose
  `torii.transport.trusted_proxy_cidrs`, defaulting it to an empty list.
- `crates/iroha_torii/src/lib.rs` now resolves the canonical client IP with
  `limits::ingress_remote_ip(...)` inside the ingress middleware and rewrites
  the internal `x-iroha-remote-addr` header only from trusted proxies.
- `crates/iroha_torii/src/sorafs/pin.rs` and
  `crates/iroha_torii/src/sorafs/api.rs` now resolve canonical client IPs
  against `state.trusted_proxy_nets` at the handler boundary for storage-pin
  policy and gateway client fingerprinting, so direct handler paths cannot
  over-trust stale forwarded-IP metadata.
- Storage-pin throttling now keys shared bearer tokens by `token + canonical
  client IP` when both are present, preserving per-client buckets for shared
  pin tokens.
- Regression coverage now includes
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`,
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`,
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`,
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`,
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`, and the
  config fixture
  `torii_transport_trusted_proxy_cidrs_default_to_empty`.

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

## Dependency Findings

- `cargo deny check advisories bans sources --hide-inclusion-graph` now runs
  directly against the tracked `deny.toml` and reports five live dependency
  findings from the current workspace lockfile.
- The `tar` advisories are no longer present in the active dependency graph:
  `xtask/src/mochi.rs` now uses `Command::new("tar")` with a fixed argument
  vector, and `iroha_crypto` no longer pulls `libsodium-sys-stable` for
  Ed25519 interop tests after swapping those checks to OpenSSL.
- Current findings:
  - `RUSTSEC-2026-0049`: `rustls-webpki` has a CRL Distribution Point matching
    bug.
  - `RUSTSEC-2024-0388`: `derivative` is unmaintained.
  - `RUSTSEC-2024-0436`: `paste` is unmaintained.
  - `RUSTSEC-2024-0380`: `pqcrypto-dilithium` has been replaced by
    `pqcrypto-mldsa`.
  - `RUSTSEC-2024-0381`: `pqcrypto-kyber` has been replaced by
    `pqcrypto-mlkem`.
- Impact triage:
  - `rustls-webpki` is runtime-reachable through the `reqwest` /
    `hyper-rustls` client stack in production crates such as
    `crates/iroha_torii`, `crates/iroha_cli`, `crates/iroha_telemetry`, and
    `crates/sorafs_car`. However, the specific advisory requires CRL-based
    revocation checking. I found no repository code that loads CRLs or builds
    a revocation-aware verifier; the sampled call sites use the default
    `reqwest::Client::builder()` path in
    `crates/iroha_torii/src/soracloud.rs:2952-2959` and
    `reqwest::Client::builder()` in
    `crates/iroha_torii/src/webhook.rs:1941-1957`. This means the dependency is
    present on runtime paths, but the advisory appears latent in the current
    tree unless CRL handling is added or enabled. This is an inference from
    the advisory preconditions plus the repo search results.
  - The previously reported `tar` advisories are closed for the active
    dependency graph. `cargo tree -p xtask -e normal -i tar`,
    `cargo tree -p iroha_crypto -e all -i tar`, and
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` now all fail
    with “package ID specification ... did not match any packages,” and
    `cargo deny` no longer reports `RUSTSEC-2026-0067` or
    `RUSTSEC-2026-0068`.
  - `pqcrypto-dilithium` and `pqcrypto-kyber` remain direct dependencies in
    `crates/iroha_crypto/Cargo.toml`, so their replacement advisories are real
    product-maintenance work rather than tooling-only noise.
  - `derivative` and `paste` are not used directly in the workspace source.
    They enter transitively through the BLS / arkworks stack under
    `w3f-bls`, so removing them requires upstream or dependency-stack changes
    rather than a local macro cleanup.

## Coverage Notes

- Server/runtime/config/networking: SEC-05, SEC-06, SEC-07, SEC-08, SEC-09,
  SEC-10, SEC-12, SEC-13, SEC-14, and SEC-15 were confirmed during the audit
  and are now closed in the current tree. Additional hardening in the current
  tree now also makes Connect websocket/session admission fail closed when the
  internal injected remote-IP header is missing, instead of defaulting that
  condition to loopback.
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
- SDKs/examples: SEC-11 was confirmed during the sampled transport-focused
  pass across the Swift, Java/Android, Kotlin, and JS clients, and that
  finding is now closed in the current tree. The JS, Swift, and Android
  canonical-request helpers have also been updated to the new
  freshness-aware four-header scheme.
  The sampled QUIC streaming transport review also did not produce a live
  runtime finding in the current tree: `StreamingClient::connect(...)`,
  `StreamingServer::bind(...)`, and the capability-negotiation helpers are
  currently exercised only from test code in `crates/iroha_p2p` and
  `crates/iroha_core`, so the permissive self-signed verifier in that helper
  path is presently test/helper-only rather than a shipped ingress surface.
- Examples and mobile sample apps were reviewed only at a spot-check level and
  should not be treated as exhaustively audited.

## Validation And Coverage Gaps

- `cargo deny check advisories bans sources --hide-inclusion-graph` now runs
  directly with the tracked `deny.toml`. Under that current-schema run,
  `bans` and `sources` are clean, while `advisories` fails with the five
  dependency findings listed above.
- Dependency-graph cleanup validation for the closed `tar` findings passed:
  `cargo tree -p xtask -e normal -i tar`,
  `cargo tree -p iroha_crypto -e all -i tar`, and
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` now all report
  “package ID specification ... did not match any packages,” while
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`,
  and
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  all passed.
- `bash scripts/fuzz_smoke.sh` now runs real libFuzzer sessions via
  `cargo +nightly fuzz`, but the IVM half of the script did not finish within
  this pass because the first nightly build for `tlv_validate` was still in
  progress at handoff. That build has since completed far enough to execute the
  generated libFuzzer binary directly:
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  now reaches the libFuzzer run loop and exits cleanly after 200 runs from an
  empty corpus. The Norito half completed successfully after fixing the
  harness/manifest drift and the `json_from_json_equiv` fuzz-target compile
  break.
- Torii remediation validation now includes:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture`
  - the narrower `--no-default-features --features app_api,app_api_https` Torii
    test matrix still has unrelated existing compile failures in DA /
    Soracloud-gated lib-test code, so this pass validated the shipped
    default-feature MCP path and the `app_api_https` webhook path rather than
    claiming full minimal-feature coverage.
- Trusted-proxy/SoraFS remediation validation now includes:
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- P2P remediation validation now includes:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- SoraFS local-proxy remediation validation now includes:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  - `/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- P2P TLS-default remediation validation now includes:
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- SDK-side remediation validation now includes:
  - `node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `cd IrohaSwift && swift test --filter CanonicalRequestTests`
  - `cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'`
  - `cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'`
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'`
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  - `node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `node --test javascript/iroha_js/test/toriiSubscriptions.test.js`
- Norito follow-up validation now includes:
  - `python3 scripts/check_norito_bindings_sync.py`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh` (Norito targets `json_parse_string`,
    `json_parse_string_ref`, `json_skip_value`, and `json_from_json_equiv`
    passed after harness/target fixes)
- IVM fuzz follow-up validation now includes:
  - `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
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

- Triage and remediate the five remaining dependency findings from cargo-deny,
  prioritizing a lockfile update for `rustls-webpki` once lockfile edits are
  allowed, then deciding whether to replace or explicitly accept the four
  unmaintained/replaced crates.
- Decide whether the direct PQ dependencies in `iroha_crypto`
  (`pqcrypto-dilithium`, `pqcrypto-kyber`) should move to their FIPS-aligned
  successors or remain as a documented compatibility choice for the first
  release.
- Track the transitive `derivative` / `paste` debt to the BLS / arkworks stack
  and remove it when the upstream dependency chain can be updated without
  destabilizing the cryptography surface.
- Rerun the full nightly IVM fuzz-smoke script on a warm cache so
  `tlv_validate` / `kotodama_lower` have stable recorded results next to the
  now-green Norito targets. A direct `tlv_validate` binary run now completes,
  but the full scripted nightly smoke is still outstanding.
- Rerun the focused CUDA lib-test self-test slice on a host with CUDA driver
  libraries installed, so the expanded CUDA startup truth set is validated
  beyond `cargo check` and the mirrored Ed25519 normalization fix plus the
  new vector/AES startup probes are exercised at runtime.
- Rerun broader JS/Swift/Android/Kotlin suites once the unrelated suite-level
  blockers on this branch are cleared, so the new canonical-request and
  transport-security guards are covered beyond the focused helper tests above.
- Decide whether the long-term app-auth multisig story should remain
  fail-closed or grow a first-class HTTP multisig witness format.

### Monitor

- Continue the focused review of `ivm` hardware-acceleration / unsafe paths
  and the remaining `norito` streaming/crypto boundaries. The JSON Stage-1
  and GPU zstd helper handoffs have now been hardened to fail closed in
  release builds, and the sampled IVM accelerator startup truth sets are now
  broader, but the wider unsafe/determinism review is still open.
