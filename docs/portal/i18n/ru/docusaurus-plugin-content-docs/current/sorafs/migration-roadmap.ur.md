---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: "SoraFS مائیگریشن روڈ میپ"
---

> [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md) سے ماخوذ۔

# SoraFS مائیگریشن روڈ میپ (SF-1)

یہ دستاویز `docs/source/sorafs_architecture_rfc.md` میں بیان کردہ مائیگریشن رہنمائی کو
عملی بناتی ہے۔ یہ SF-1 کے deliverables کو ایسے milestones، gate criteria اور owner
checklists میں توسیع دیتی ہے جو اجرا کے لیے تیار ہوں، تاکہ storage، governance، DevRel
ہم آہنگ کر سکیں۔

یہ روڈ میپ جان بوجھ کر deterministic ہے: ہر milestone مطلوبہ artifacts، command
invocations اور attestation steps کو نام دیتا ہے تاکہ downstream pipelines ایک جیسے
outputs بنائیں اور governance کے پاس audit-able trail برقرار رہے۔

## Milestone overview

| Milestone | Window | Primary goals | Must ship | Owners |
|-----------|--------|---------------|-----------|--------|
| **M1 - Deterministic Enforcement** | Weeks 7-12 | signed fixtures نافذ کرنا اور alias proofs stage کرنا جب pipelines expectation flags اپنائیں۔ | nightly fixture verification، council-signed manifests، alias registry staging entries. | Storage, Governance, SDKs |

Milestone status `docs/source/sorafs/migration_ledger.md` میں ٹریک ہوتا ہے۔ اس روڈ میپ میں
ہر تبدیلی لازمی طور پر ledger کو اپڈیٹ کرے تاکہ governance اور release engineering ہم آہنگ رہیں۔

## Workstreams

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
| Proof enforcement | M2 | Gateways ایسے manifests کو reject کریں جن میں تازہ `Sora-Proof` headers نہ ہوں؛ CI میں `sorafs alias verify` step شامل کریں۔ | Networking | Gateway config patch + CI output جس میں verification success ہو۔ |

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
| SDK release cadence | Clients کو M3 سے پہلے alias proofs پر عمل کرنا ہوگا۔ | SDK release windows کو milestone gates کے ساتھ align کریں؛ migration checklists کو release templates میں شامل کریں۔ |

Residual risks اور mitigations `docs/source/sorafs_architecture_rfc.md` میں بھی درج ہیں
اور adjustments کے وقت cross-reference کرنا چاہیے۔

## Exit criteria checklist

| Milestone | Criteria |
|-----------|----------|
| M1 | - Nightly fixture job سات دن مسلسل green۔ <br /> - Staging alias proofs CI میں verify۔ <br /> - Governance expectation flag policy کی توثیق کرے۔ |

## Change management

1. PR کے ذریعے تبدیلیاں تجویز کریں اور یہ فائل **اور**
   `docs/source/sorafs/migration_ledger.md` دونوں اپڈیٹ کریں۔
2. PR description میں governance minutes اور CI evidence کے links شامل کریں۔
3. merge کے بعد storage + DevRel mailing list کو summary اور expected operator actions بھیجیں۔

یہ طریقہ کار یقینی بناتا ہے کہ SoraFS rollout deterministic، auditable، اور transparent رہے
اور Nexus لانچ میں شریک ٹیموں کے درمیان ہم آہنگی برقرار رہے۔
