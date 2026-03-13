---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/dispute-revocation-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9eddda7331e0b00d24cbe5f1131a02246d79f3dc15232fcda1672be788046caf
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: dispute-revocation-runbook
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/dispute_revocation_runbook.md` کی عکاسی کرتا ہے۔ جب تک پرانی Sphinx ڈاکیومنٹیشن ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

## مقصد

یہ رن بک گورننس آپریٹرز کو SoraFS کپیسٹی تنازعات جمع کرانے، منسوخیوں کی ہم آہنگی، اور ڈیٹا کے ڈٹرمنسٹک انخلا کو یقینی بنانے میں رہنمائی کرتی ہے۔

## 1. واقعے کا جائزہ

- **ٹرگر شرائط:** SLA کی خلاف ورزی (uptime/PoR failure)، replication shortfall، یا billing disagreement کی نشاندہی۔
- **ٹیلیمیٹری کی تصدیق:** پرووائیڈر کے لیے `/v2/sorafs/capacity/state` اور `/v2/sorafs/capacity/telemetry` snapshots حاصل کریں۔
- **اسٹیک ہولڈرز کو مطلع کریں:** Storage Team (provider operations)، Governance Council (decision body)، Observability (dashboard updates)۔

## 2. شواہد کا پیکج تیار کریں

1. خام artifacts جمع کریں (telemetry JSON، CLI logs، auditor notes)۔
2. ڈٹرمنسٹک archive (مثلاً tarball) میں normalize کریں؛ درج کریں:
   - BLAKE3-256 digest (`evidence_digest`)
   - media type (`application/zip`, `application/jsonl` وغیرہ)
   - hosting URI (object storage، SoraFS pin، یا Torii-accessible endpoint)
3. گورننس evidence collection bucket میں write-once رسائی کے ساتھ پیکج محفوظ کریں۔

## 3. تنازع جمع کرائیں

1. `sorafs_manifest_stub capacity dispute` کے لیے JSON spec بنائیں:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. CLI چلائیں:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. `dispute_summary.json` ریویو کریں (kind، evidence digest، timestamps)۔
4. گورننس ٹرانزیکشن کیو کے ذریعے Torii `/v2/sorafs/capacity/dispute` کو ریکوئسٹ JSON بھیجیں۔ جواب کی قدر `dispute_id_hex` محفوظ کریں؛ یہی بعد کی منسوخی کارروائیوں اور آڈٹ رپورٹس کا اینکر ہے۔

## 4. انخلا اور منسوخی

1. **Grace window:** پرووائیڈر کو متوقع منسوخی سے آگاہ کریں؛ پالیسی اجازت دے تو pinned data کے انخلا کی اجازت دیں۔
2. **`ProviderAdmissionRevocationV1` بنائیں:**
   - منظور شدہ وجہ کے ساتھ `sorafs_manifest_stub provider-admission revoke` استعمال کریں۔
   - دستخط اور revocation digest ویریفائی کریں۔
3. **منسوخی شائع کریں:**
   - منسوخی ریکوئسٹ Torii کو جمع کریں۔
   - یقینی بنائیں کہ پرووائیڈر adverts بلاک ہیں (متوقع ہے `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` بڑھے)۔
4. **Dashboards اپڈیٹ کریں:** پرووائیڈر کو revoked کے طور پر فلیگ کریں، dispute ID کا حوالہ دیں، اور evidence bundle لنک کریں۔

## 5. Post-mortem اور فالو اپ

- ٹائم لائن، root cause، اور remediation اقدامات گورننس incident tracker میں ریکارڈ کریں۔
- restitution طے کریں (stake slashing، fee clawbacks، customer refunds)۔
- سیکھے گئے اسباق دستاویز کریں؛ ضرورت ہو تو SLA thresholds یا monitoring alerts اپڈیٹ کریں۔

## 6. حوالہ جاتی مواد

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (dispute section)
- `docs/source/sorafs/provider_admission_policy.md` (revocation workflow)
- Observability dashboard: `SoraFS / Capacity Providers`

## چیک لسٹ

- [ ] evidence bundle حاصل کر کے hash کر لیا گیا۔
- [ ] dispute payload مقامی طور پر validate کیا گیا۔
- [ ] Torii dispute ٹرانزیکشن قبول ہوئی۔
- [ ] منسوخی نافذ کی گئی (اگر منظور ہو)۔
- [ ] dashboards/runbooks اپڈیٹ ہوئے۔
- [ ] Post-mortem گورننس کونسل میں جمع کرایا گیا۔
