# GAR Enforcement Outline (soranet-pop-m0, SN15-M0-11)

- **Receipt schema:** Every purge/moderation/policy change emits
  `GarEnforcementReceiptV1` with fields `{ action, manifest_cid, cache_version,
  reason, issued_at, issued_by, pop }`. Persist JSON + Norito copies under
  `artifacts/soranet/gateway/soranet-pop-m0/gar_receipts/`.
- **Distribution:** Broadcast enforcement events on NATS subject
  `gar.enforce.soranet-pop-m0` with at-least-once delivery; PoPs ACK by writing
  `gar_enforcement_ack.json` containing receipt hash + applied version.
- **Audit trail:** Nightly job exports merged receipts + ACKs to ClickHouse
  table `gar_enforcement_log` keyed by `soranet-pop-m0` and `manifest_cid`.
  Dashboards read from this table and alert on missing ACKs >15m.
- **Honey tokens:** Reserve two synthetic manifests per sprint to exercise
  purge + moderation flows; record honey probe outcomes next to the receipts and
  attach to governance packets.
- **Runbooks:** When ResolverProofStale fires, block new GAR entries, rotate
  cache-version expectations, and attach the latest enforcement receipt set to
  the rollback bundle.
