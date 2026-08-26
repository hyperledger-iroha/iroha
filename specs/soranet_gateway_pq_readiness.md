# SoraGlobal Gateway PQ Readiness (SNNet-15PQ)

The SNNet-15PQ milestone aligns the SoraGlobal gateway CDN with the SNNet-16
post-quantum transport policy. Operators must prove that PoP TLS/ECH material
and mandatory dual-signed SRCv2 identity are ready for first-release service,
that the trustless verifier pipeline enforces cache binding and SDR timing
bounds, and separately attach live PQ-handshake canary evidence with downgrade
telemetry. This static helper does not contact or qualify canary hosts.

## Command

Run the new readiness helper from the repository root:

```
cargo xtask soranet-gateway-pq \
  --srcv2 configs/soranet/gateway_m0/guards/relay.srcv2.cbor \
  --issuer-ed25519-hex <64-lowercase-hex-characters> \
  --issuer-mldsa65-hex <3904-lowercase-hex-characters> \
  --tls-bundle artifacts/soranet/gateway_m0_lab \
  --trustless-config configs/soranet/gateway_m0/gateway_trustless_verifier.toml \
  --at-unix <certificate-evaluation-unix-second> \
  --pop sjc-01
```

Flags:
- `--srcv2` — CBOR-encoded `RelayCertificateBundleV2` for the PoP.
- `--issuer-ed25519-hex` — exact Ed25519 issuer public key obtained from an
  independent trusted governance source (64 canonical lowercase hex characters).
- `--issuer-mldsa65-hex` — exact ML-DSA-65 issuer public key from the same
  independent source (3904 canonical lowercase hex characters).
- `--tls-bundle` — directory with `fullchain.pem`, `privkey.pem`, and
  `ech.json` (the default ACME automation output).
- `--trustless-config` — gateway verifier TOML (`gateway_trustless_verifier.toml`).
- `--at-unix` — explicit non-negative Unix second at which the certificate's
  half-open validity interval and both signatures are verified.
- `--pop` — label used in the summary.
- `--out`/`--output-dir` — override the output directory
  (`artifacts/soranet/gateway_pq` by default).

Outputs:
- `gateway_pq_summary.json` — detailed statuses for time-bound SRCv2
  dual-signature validity, PQ handshake suite coverage, TLS/ECH evidence
  (BLAKE3 fingerprint + ECH config), and trustless verifier flags.
- `gateway_pq_summary.md` — short Markdown recap for governance packets.

## Evidence expectations

- SRCv2 bundles must include both mandatory signatures and advertise NK2/NK3
  suites. The two trusted issuer keys must be distinct from the relay identity
  keys, derive the certificate's exact `issuer_fingerprint`, and verify the
  bundle at the explicit `--at-unix` second. A bundle's embedded relay keys
  never establish issuer trust.
- TLS/ECH bundles must exist and the ECH JSON must parse cleanly.
- Trustless verifier config must reject stale cache versions and verify cache
  binding headers; KZG/SDR paths must be populated, not placeholders.
- Telemetry references are baked into the summary:
  - Handshake dashboard: `dashboards/grafana/soranet_sn16_handshake.json`
  - Alert rules: `dashboards/alerts/soranet_handshake_rules.yml`
- Attach the JSON + Markdown outputs to the PoP’s promotion packet alongside
  separate live canary-handshake results, GameDay/chaos evidence, and GAR
  receipts. The command writes diagnostic evidence on failure but exits
  nonzero unless every component is `ok`.

## Runbook

1. Generate the readiness bundle (command above) and store it under the PoP’s
   evidence root.
2. Push SRCv2 and TLS/ECH rotations to canary hosts first, run an authenticated
   live handshake canary separately, and confirm the helper exits zero with
   `overall_status` equal to `ok`.
3. Watch the SNNet-16 handshake dashboard and alert pack during that live PQ
   canary traffic; record screenshots and alert exports next to the readiness
   bundle.
4. Once canaries stay green, roll to the remaining PoP hosts and keep the
   readiness artefacts attached to the release packet.
