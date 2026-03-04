# Security Baseline & PQ Readiness (soranet-pop-m0, SN15-M0-12)

- **TLS/ECH:** Store cert/ECH material in HSM-backed vault entries
  `vault://soranet/soranet-pop-m0/tls` and `.../ech`; rotate quarterly with
  SRCv2 dual-sig (Ed25519 + ML-DSA) and record attestation hashes in
  `ops_summary.json`.
- **Sandboxing:** Run gateway processes under dedicated cgroups with eBPF
  seccomp profiles; isolate WAF and verifier helpers in separate namespaces
  with read-only roots.
- **SBOM + scanning:** Generate SPDX SBOMs during CI for edge + verifier images
  and run nightly vulnerability scans; attach latest scan summary to the
  promotion checklist.
- **Log retention:** Default log retention to 30d with opt-in extensions; Loki
  buckets enforce deletion after expiry to satisfy privacy guarantees.
- **PQ guardrails:** Enable Kyber-capable TLS ciphers on the edge once SRCv2
  certs land; keep anonymous SoraNet circuits preferred and record downgrade
  counts in dashboards.
- **Incident handling:** Tabletop quarterly for cache poison + resolver
  stale-proof scenarios with clear rollback hooks and Alertmanager routing to
  GAR/legal on-call roles.
