---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS گیٹ وے اور DNS کک آف رن بک

یہ پورٹل کاپی کینونیکل رن بک کو منعکس کرتی ہے جو
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md) میں ہے۔
یہ Decentralized DNS & Gateway ورک اسٹریم کے آپریشنل گارڈ ریلز کو سمیٹتی ہے تاکہ
نیٹ ورکنگ، آپس، اور ڈاکیومنٹیشن لیڈز 2025-03 کے کک آف سے پہلے آٹومیشن اسٹیک کی
مشقیق کر سکیں۔

## اسکوپ اور ڈلیوریبلز

- DNS (SF-4) اور gateway (SF-5) milestones کو جوڑنا، جس میں deterministic host derivation،
  resolver directory releases، TLS/GAR automation، اور evidence capture کی مشق شامل ہو۔
- کک آف ان پٹس (agenda، invite، attendance tracker، GAR telemetry snapshot) کو
  تازہ ترین owner assignments کے ساتھ ہم آہنگ رکھنا۔
- گورننس ریویورز کے لیے قابلِ آڈٹ artefact bundle تیار کرنا: resolver directory
  release notes، gateway probe logs، conformance harness output، اور Docs/DevRel summary۔

## رولز اور ذمہ داریاں

| ورک اسٹریم | ذمہ داریاں | مطلوبہ artefacts |
|------------|------------|------------------|
| Networking TL (DNS stack) | deterministic host plan برقرار رکھنا، RAD directory releases چلانا، resolver telemetry inputs شائع کرنا۔ | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md` کے diffs، اور RAD metadata۔ |
| Ops Automation Lead (gateway) | TLS/ECH/GAR automation drills چلانا، `sorafs-gateway-probe` چلانا، PagerDuty hooks اپڈیٹ کرنا۔ | `artifacts/sorafs_gateway_probe/<ts>/`, probe JSON، `ops/drill-log.md` entries۔ |
| QA Guild & Tooling WG | `ci/check_sorafs_gateway_conformance.sh` چلانا، fixtures کیوریٹ کرنا، Norito self-cert bundles آرکائیو کرنا۔ | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | minutes ریکارڈ کرنا، design pre-read + appendices اپڈیٹ کرنا، اور اسی پورٹل میں evidence summary شائع کرنا۔ | اپڈیٹ شدہ `docs/source/sorafs_gateway_dns_design_*.md` فائلز اور rollout notes۔ |

## ان پٹس اور پری ریکوائرمنٹس

- deterministic host spec (`docs/source/soradns/deterministic_hosts.md`) اور resolver
  attestation scaffolding (`docs/source/soradns/resolver_attestation_directory.md`).
- gateway artefacts: operator handbook، TLS/ECH automation helpers، direct-mode guidance،
  اور self-cert workflow جو `docs/source/sorafs_gateway_*` کے تحت ہے۔
- Tooling: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, اور CI helpers
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secrets: GAR release key، DNS/TLS ACME credentials، PagerDuty routing key،
  اور resolver fetches کے لیے Torii auth token۔

## پری فلائٹ چیک لسٹ

1. `docs/source/sorafs_gateway_dns_design_attendance.md` اپڈیٹ کر کے شرکاء اور agenda
   کنفرم کریں اور موجودہ agenda (`docs/source/sorafs_gateway_dns_design_agenda.md`) شیئر کریں۔
2. `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` اور
   `artifacts/soradns_directory/<YYYYMMDD>/` جیسے artefact roots تیار کریں۔
3. fixtures (GAR manifests، RAD proofs، gateway conformance bundles) ریفریش کریں اور
   یقینی بنائیں کہ `git submodule` کی حالت تازہ ترین rehearsal tag سے میچ کرتی ہے۔
4. secrets (Ed25519 release key، ACME account file، PagerDuty token) ویریفائی کریں اور
   vault checksums کے ساتھ میچ کریں۔
5. drill سے پہلے telemetry targets (Pushgateway endpoint، GAR Grafana board) کا smoke-test کریں۔

## آٹومیشن rehearsal steps

### deterministic host map اور RAD directory release

1. تجویز کردہ manifests سیٹ کے خلاف deterministic host derivation helper چلائیں اور
   تصدیق کریں کہ `docs/source/soradns/deterministic_hosts.md` کے مقابلے میں کوئی drift نہیں۔
2. resolver directory bundle تیار کریں:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. پرنٹ شدہ directory ID، SHA-256، اور output paths کو
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` اور kickoff minutes میں ریکارڈ کریں۔

### DNS telemetry capture

- resolver transparency logs کو ≥10 منٹ تک tail کریں:
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Pushgateway metrics export کریں اور NDJSON snapshots کو run ID directory کے ساتھ آرکائیو کریں۔

### Gateway automation drills

1. TLS/ECH probe چلائیں:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. conformance harness (`ci/check_sorafs_gateway_conformance.sh`) اور self-cert helper
   (`scripts/sorafs_gateway_self_cert.sh`) چلائیں تاکہ Norito attestation bundle ریفریش ہو۔
3. PagerDuty/Webhook events capture کریں تاکہ آٹومیشن پاتھ end-to-end ثابت ہو۔

### Evidence packaging

- `ops/drill-log.md` میں timestamps، participants اور probe hashes اپڈیٹ کریں۔
- run ID directories میں artefacts محفوظ کریں اور Docs/DevRel minutes میں executive summary شائع کریں۔
- kickoff review سے پہلے governance ticket میں evidence bundle لنک کریں۔

## سیشن فیسلیٹیشن اور evidence hand-off

- **Moderator timeline:**
  - T-24 h — Program Management `#nexus-steering` میں reminder + agenda/attendance snapshot پوسٹ کرے۔
  - T-2 h — Networking TL GAR telemetry snapshot ریفریش کر کے `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` میں deltas ریکارڈ کرے۔
  - T-15 m — Ops Automation probe readiness ویریفائی کر کے فعال run ID کو `artifacts/sorafs_gateway_dns/current` میں لکھے۔
  - کال کے دوران — Moderator یہ رن بک شیئر کرے اور live scribe assign کرے؛ Docs/DevRel inline action items ریکارڈ کرے۔
- **Minute template:**
  `docs/source/sorafs_gateway_dns_design_minutes.md` سے skeleton کاپی کریں (portal bundle میں بھی ہے) اور ہر سیشن کے لیے ایک مکمل instance commit کریں۔ attendee roll، decisions، action items، evidence hashes، اور outstanding risks شامل کریں۔
- **Evidence upload:** rehearsal کے `runbook_bundle/` directory کو zip کریں، rendered minutes PDF attach کریں، minutes + agenda میں SHA-256 hashes لکھیں، اور uploads کے بعد governance reviewer alias کو ping کریں جب فائلز `s3://sora-governance/sorafs/gateway_dns/<date>/` میں پہنچ جائیں۔

## Evidence snapshot (March 2025 kickoff)

roadmap اور minutes میں حوالہ دیے گئے تازہ ترین rehearsal/live artefacts
`s3://sora-governance/sorafs/gateway_dns/` bucket میں ہیں۔ نیچے دیے گئے hashes
canonical manifest (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) کو reflect کرتے ہیں۔

- **Dry run — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Bundle tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Minutes PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Live workshop — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Pending upload: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel PDF آنے پر SHA-256 شامل کرے گا۔)_

## Related material

- [Gateway operations playbook](./operations-playbook.md)
- [SoraFS observability plan](./observability-plan.md)
- [Decentralized DNS & Gateway tracker](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
