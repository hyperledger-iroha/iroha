---
lang: ur
direction: rtl
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-11-21T18:09:53.463728+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/taikai_anchor_lineage_packet.md کا اردو ترجمہ -->

# Taikai اینکر لائن ایج پیکٹ (ٹیمپلیٹ) (SN13-C)

روڈمیپ آئٹم **SN13-C - Manifests & SoraNS anchors** کے تحت ہر alias روٹیشن کو ایک
حتمی شواہد بنڈل فراہم کرنا لازم ہے۔ اس ٹیمپلیٹ کو اپنے rollout artefact ڈائریکٹری میں
کاپی کریں (مثال کے طور پر
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) اور گورننس میں جمع کرانے سے
پہلے placeholders تبدیل کریں۔

## 1. میٹا ڈیٹا

| فیلڈ | ویلیو |
|------|-------|
| ایونٹ ID | `<taikai.event.launch-2026-07-10>` |
| Stream / rendition | `<main-stage>` |
| Alias namespace / نام | `<sora / docs>` |
| شواہد ڈائریکٹری | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| آپریٹر رابطہ | `<name + email>` |
| GAR / RPT ٹکٹ | `<governance ticket or GAR digest>` |

## Bundle helper (اختیاری)

spool artefacts کاپی کریں اور باقی سیکشنز بھرنے سے پہلے JSON خلاصہ (اختیاری دستخط کے ساتھ)
جاری کریں:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

یہ helper `taikai-anchor-request-*`, `taikai-trm-state-*`, `taikai-lineage-*`, envelopes اور
sentinels کو Taikai spool ڈائریکٹری (`config.da_ingest.manifest_store_dir/taikai`) سے نکالتا
ہے تاکہ شواہد فولڈر میں نیچے حوالہ دی گئی فائلیں پہلے سے موجود ہوں۔

## 2. Lineage ledger اور hint

on-disk lineage ledger اور Torii کی لکھی ہوئی hint JSON دونوں منسلک کریں۔ یہ براہ راست
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` اور
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json` سے آتی ہیں۔

| Artefact | فائل | SHA-256 | نوٹس |
|----------|------|---------|-------|
| Lineage ledger | `taikai-trm-state-docs.json` | `<sha256>` | سابقہ manifest digest/window کا ثبوت۔ |
| Lineage hint | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | SoraNS anchor پر اپ لوڈ سے پہلے لیا گیا۔ |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Anchor payload capture

Torii نے جو POST payload anchor سروس کو بھیجا اسے ریکارڈ کریں۔ payload میں
`envelope_base64`, `ssm_base64`, `trm_base64` اور inline `lineage_hint` شامل ہے؛ audits اسی
capture پر انحصار کرتے ہیں تاکہ SoraNS کو بھیجا گیا hint ثابت ہو سکے۔ Torii اب یہ JSON
خودکار طور پر
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
Taikai spool ڈائریکٹری (`config.da_ingest.manifest_store_dir/taikai/`) میں لکھتا ہے، اس لئے
آپریٹرز HTTP logs کھنگالنے کے بجائے سیدھا کاپی کر سکتے ہیں۔

| Artefact | فائل | SHA-256 | نوٹس |
|----------|------|---------|-------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | `taikai-anchor-request-*.json` (Taikai spool) سے کاپی کی گئی raw درخواست۔ |

## 4. Manifest digest acknowledgement

| فیلڈ | ویلیو |
|------|-------|
| نیا manifest digest | `<hex digest>` |
| پچھلا manifest digest (hint سے) | `<hex digest>` |
| ونڈو start / end | `<start seq> / <end seq>` |
| قبولیت timestamp | `<ISO8601>` |

اوپر ریکارڈ کیے گئے ledger/hint hashes کا حوالہ دیں تاکہ reviewers اس ونڈو کی تصدیق کر
سکیں جو replace ہوئی ہے۔

## 5. Metrics / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (فی alias): `<file path + hash>`

Prometheus/Grafana کا export یا `curl` output فراہم کریں جو counter increment اور اس alias
کے لئے `/status` array دکھائے۔

## 6. Evidence directory manifest

Evidence directory (spool files, payload capture, metrics snapshots) کا deterministic manifest
بنائیں تاکہ governance archive کھولے بغیر ہر hash verify کر سکے۔

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefact | فائل | SHA-256 | نوٹس |
|----------|------|---------|-------|
| Evidence manifest | `manifest.json` | `<sha256>` | اسے governance packet / GAR کے ساتھ منسلک کریں۔ |

## 7. Checklist

- [ ] Lineage ledger کاپی + hashed.
- [ ] Lineage hint کاپی + hashed.
- [ ] Anchor POST payload capture اور hash کیا گیا۔
- [ ] Manifest digest جدول مکمل کیا گیا۔
- [ ] Metrics snapshots export کیے (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] `scripts/repo_evidence_manifest.py` کے ساتھ manifest بنایا گیا۔
- [ ] پیکٹ governance کو hashes + رابطہ معلومات کے ساتھ اپ لوڈ کیا گیا۔

ہر alias روٹیشن کے لئے اس ٹیمپلیٹ کو برقرار رکھنا SoraNS governance bundle کو reproducible
بناتا ہے اور lineage hints کو براہ راست GAR/RPT شواہد سے جوڑتا ہے۔

</div>
