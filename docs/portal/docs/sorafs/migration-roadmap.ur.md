<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bfbe7de97848ee43284d448f9d80b78f68b5e95d36e0f86d2aa12c3633838867
source_last_modified: "2025-11-07T10:28:53.296738+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: "SoraFS مائیگریشن روڈ میپ"
---

> [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md) سے ماخوذ۔

# SoraFS مائیگریشن روڈ میپ (SF-1)

یہ دستاویز `docs/source/sorafs_architecture_rfc.md` میں بیان کردہ مائیگریشن رہنمائی کو
عملی بناتی ہے۔ یہ SF-1 کے deliverables کو ایسے milestones، gate criteria اور owner
checklists میں توسیع دیتی ہے جو اجرا کے لیے تیار ہوں، تاکہ storage، governance، DevRel
اور SDK ٹیمیں legacy artifact hosting سے SoraFS-backed publication کی طرف منتقلی کو
ہم آہنگ کر سکیں۔

یہ روڈ میپ جان بوجھ کر deterministic ہے: ہر milestone مطلوبہ artifacts، command
invocations اور attestation steps کو نام دیتا ہے تاکہ downstream pipelines ایک جیسے
outputs بنائیں اور governance کے پاس audit-able trail برقرار رہے۔

## Milestone overview

| Milestone | Window | Primary goals | Must ship | Owners |
|-----------|--------|---------------|-----------|--------|
| **M0 - Bootstrap** | Weeks 1-6 | deterministic chunker fixtures شائع کرنا اور artifacts کو dual-publish کرنا (legacy + SoraFS). | `sorafs_chunker` fixtures، `sorafs_manifest_stub` CLI integration، migration ledger entries. | Docs, DevRel, Storage |
| **M1 - Deterministic Enforcement** | Weeks 7-12 | signed fixtures نافذ کرنا اور alias proofs stage کرنا جب pipelines expectation flags اپنائیں۔ | nightly fixture verification، council-signed manifests، alias registry staging entries. | Storage, Governance, SDKs |
| **M2 - Registry First** | Weeks 13-20 | pins کو registry کے ذریعے چلانا، legacy bundles freeze کرنا، اور parity telemetry expose کرنا۔ | Pin Registry contract + CLI (`sorafs pin propose/approve`)، observability dashboards، operator runbooks. | Governance, Ops, Observability |
| **M3 - Alias Only** | Week 21+ | legacy hosting کو ختم کرنا اور retrieval کے لیے alias proofs لازمی کرنا۔ | alias-only gateways، parity alerts، SDK defaults اپڈیٹ، legacy teardown notice. | Ops, Networking, SDKs |

Milestone status `docs/source/sorafs/migration_ledger.md` میں ٹریک ہوتا ہے۔ اس روڈ میپ میں
ہر تبدیلی لازمی طور پر ledger کو اپڈیٹ کرے تاکہ governance اور release engineering ہم آہنگ رہیں۔

## Workstreams

### 1. Legacy data rewrap

| Step | Milestone | Description | Owner(s) | Output |
|------|-----------|-------------|----------|--------|
| Inventory & tagging | M0 | legacy bundles کے SHA3-256 digests ایکسپورٹ کریں اور انہیں migration ledger میں لاگ کریں (append-only). | Docs, DevRel | `source_path`, `sha3_digest`, `owner`, `planned_manifest_cid` کے ساتھ ledger entries. |
| Deterministic rebuild | M0-M1 | ہر release artifact کے لیے `sorafs_manifest_stub` چلائیں اور CAR، manifest، signature envelope، اور fetch plan کو `artifacts/<team>/<alias>/<timestamp>/` میں محفوظ کریں۔ | Docs, CI | فی release stamp reproducible CAR + manifest bundles. |
| Validation loop | M1 | `sorafs_fetch` کو staging gateways کے خلاف چلائیں تاکہ chunk boundaries/digests fixtures سے match ہوں۔ pass/fail کو ledger comments میں ریکارڈ کریں۔ | Governance QA | Staging verification report + drift کے لیے GitHub issue. |
| Registry cut-over | M2 | جب manifest digest on-chain موجود ہو تو ledger status کو `Pinned` کریں؛ legacy bundle read-only ہو جاتا ہے (serve مگر modify نہیں). | Governance, Ops | Registry transaction hash، legacy storage کے لیے read-only ticket. |
| Decommission | M3 | 30 دن grace period کے بعد legacy CDN entries ہٹا دیں، DNS change approvals archive کریں، اور post-mortem شائع کریں۔ | Ops | Decommission checklist، DNS change record، incident ticket closure. |

### 2. Deterministic pinning adoption

| Step | Milestone | Description | Owner(s) | Output |
|------|-----------|-------------|----------|--------|
| Fixture rehearsals | M0 | ہفتہ وار dry-runs جو local chunk digests کو `fixtures/sorafs_chunker` کے ساتھ compare کریں۔ `docs/source/sorafs/reports/` میں رپورٹ شائع کریں۔ | Storage Providers | `determinism-<date>.md` pass/fail matrix کے ساتھ۔ |
| Enforce signatures | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` اس وقت fail ہوں جب signatures یا manifests drift کریں۔ Dev overrides کے لیے governance waiver PR کے ساتھ منسلک ہونا چاہیے۔ | Tooling WG | CI log، waiver ticket link (اگر لاگو ہو). |
| Expectation flags | M1 | Pipelines `sorafs_manifest_stub` کو explicit expectations کے ساتھ کال کرتے ہیں تاکہ outputs pin ہوں: | Docs CI | Updated scripts referencing expectation flags (نیچے command block دیکھیں). |
| Registry-first pinning | M2 | `sorafs pin propose` اور `sorafs pin approve` manifest submissions کو wrap کرتے ہیں؛ CLI default `--require-registry` ہے۔ | Governance Ops | Registry CLI audit log، failed proposals کی telemetry. |
| Observability parity | M3 | Prometheus/Grafana dashboards alert کرتے ہیں جب chunk inventories registry manifests سے diverge ہوں؛ alerts ops on-call سے جڑے ہوں۔ | Observability | Dashboard link، alert rule IDs، GameDay results. |

#### Canonical publishing command

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

digest، size اور CID کو انہی متوقع حوالوں سے بدلیں جو migration ledger entry میں ریکارڈ ہیں۔

### 3. Alias transition اور communications

| Step | Milestone | Description | Owner(s) | Output |
|------|-----------|-------------|----------|--------|
| Alias proofs in staging | M1 | Pin Registry staging میں alias claims رجسٹر کریں اور manifests کے ساتھ Merkle proofs لگائیں (`--alias`). | Governance, Docs | Proof bundle manifest کے ساتھ محفوظ + ledger comment میں alias نام۔ |
| Dual DNS + notification | M1-M2 | legacy DNS اور Torii/SoraDNS کو parallel چلائیں؛ operators اور SDK channels پر migration notices شائع کریں۔ | Networking, DevRel | Announcement post + DNS change ticket. |
| Proof enforcement | M2 | Gateways ایسے manifests کو reject کریں جن میں تازہ `Sora-Proof` headers نہ ہوں؛ CI میں `sorafs alias verify` step شامل کریں۔ | Networking | Gateway config patch + CI output جس میں verification success ہو۔ |
| Alias-only rollout | M3 | legacy DNS ہٹائیں، SDK defaults کو Torii/SoraDNS + alias proofs پر اپڈیٹ کریں، rollback window document کریں۔ | SDK Maintainers, Ops | SDK release notes، ops runbook update، rollback plan۔ |

### 4. Communication & audit

- **Ledger discipline:** ہر state change (fixture drift, registry submission, alias activation)
  کو `docs/source/sorafs/migration_ledger.md` میں dated note کے طور پر append کرنا لازمی ہے۔
- **Governance minutes:** council sessions جو pin registry changes یا alias policies approve کریں
  انہیں اس roadmap اور ledger کا حوالہ دینا چاہیے۔
- **External comms:** DevRel ہر milestone پر status updates شائع کرتا ہے (blog + changelog excerpt)
  جو deterministic guarantees اور alias timelines کو نمایاں کریں۔

## Dependencies & risks

| Dependency | Impact | Mitigation |
|------------|--------|------------|
| Pin Registry contract availability | M2 pin-first rollout کو بلاک کرتی ہے۔ | M2 سے پہلے replay tests کے ساتھ contract stage کریں؛ regressions ختم ہونے تک envelope fallback برقرار رکھیں۔ |
| Council signing keys | Manifest envelopes اور registry approvals کے لیے ضروری۔ | Signing ceremony `docs/source/sorafs/signing_ceremony.md` میں دستاویزی ہے؛ overlap کے ساتھ keys rotate کریں اور ledger note رکھیں۔ |
| Gateway parity tooling | Alias proofs اور chunk parity نافذ کرنے کے لیے ضروری۔ | M1 میں gateway updates بھیجیں، M2 criteria تک legacy behaviour کو feature flag کے پیچھے رکھیں۔ |
| SDK release cadence | Clients کو M3 سے پہلے alias proofs پر عمل کرنا ہوگا۔ | SDK release windows کو milestone gates کے ساتھ align کریں؛ migration checklists کو release templates میں شامل کریں۔ |

Residual risks اور mitigations `docs/source/sorafs_architecture_rfc.md` میں بھی درج ہیں
اور adjustments کے وقت cross-reference کرنا چاہیے۔

## Exit criteria checklist

| Milestone | Criteria |
|-----------|----------|
| M0 | - تمام targeted artifacts `sorafs_manifest_stub` کے ساتھ expectation flags سے rebuild ہوئے۔ <br /> - ہر artifact family کے لیے migration ledger populated۔ <br /> - Dual publication (legacy + SoraFS) live۔ |
| M1 | - Nightly fixture job سات دن مسلسل green۔ <br /> - Staging alias proofs CI میں verify۔ <br /> - Governance expectation flag policy کی توثیق کرے۔ |
| M2 | - 100% نئے manifests Pin Registry کے ذریعے route ہوں۔ <br /> - Legacy storage read-only ہو اور incident playbook منظور ہو۔ <br /> - Observability dashboards online ہوں اور alert thresholds سیٹ ہوں۔ |
| M3 | - Alias-only gateways production میں ہوں۔ <br /> - Legacy DNS ہٹا دیا گیا ہو اور change tickets میں reflect ہو۔ <br /> - SDK defaults اپڈیٹ اور release ہو جائیں۔ <br /> - Final status migration ledger میں append ہو۔ |

## Change management

1. PR کے ذریعے تبدیلیاں تجویز کریں اور یہ فائل **اور**
   `docs/source/sorafs/migration_ledger.md` دونوں اپڈیٹ کریں۔
2. PR description میں governance minutes اور CI evidence کے links شامل کریں۔
3. merge کے بعد storage + DevRel mailing list کو summary اور expected operator actions بھیجیں۔

یہ طریقہ کار یقینی بناتا ہے کہ SoraFS rollout deterministic، auditable، اور transparent رہے
اور Nexus لانچ میں شریک ٹیموں کے درمیان ہم آہنگی برقرار رہے۔
