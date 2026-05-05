---
lang: he
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Local -> Global ایڈریس ٹول کٹ

یہ صفحہ `docs/source/sns/local_to_global_toolkit.md` کا عکاس ہے۔ یہ roadmap آئٹم **ADDR-5c** کے لیے درکار CLI helpers اور runbooks اکٹھے کرتا ہے۔

## جائزہ

- `scripts/address_local_toolkit.sh` `iroha` CLI کو wrap کرتا ہے تاکہ یہ پیدا کرے:
  - `audit.json` -- `iroha tools address audit --format json` פלט מובנה.
  - `normalized.txt` -- ہر Local-domain selector کے لیے i105 (ترجیحی) / i105 literals۔
- اس اسکرپٹ کو address ingest dashboard (`dashboards/grafana/address_ingest.json`)
  כללי Alertmanager (`dashboards/alerts/address_ingest_rules.yml`)
  Local-8 / Local-12 cutover. Local-8 אוור Local-12 לוחות התנגשות אוור
  התראות `AddressLocal8Resurgence`, `AddressLocal12Collision`, אור `AddressInvalidRatioSlo`
  کو manifest تبدیلیاں promote کرنے سے پہلے دیکھیں۔
- UX اور incident-response کے لیے [Address Display Guidelines](address-display-guidelines.md) اور
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) کو دیکھیں۔

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

اختیارات:

- `--format i105` i105 کے بجائے `sora...` output کے لیے۔
- `domainless output (default)` تاکہ bare literals نکلیں۔
- `--audit-only` conversion step چھوڑنے کے لیے۔
- `--allow-errors` تاکہ malformed rows پر بھی scan جاری رہے (CLI behavior جیسا)۔

اسکرپٹ رن کے آخر میں artefact paths لکھتا ہے۔ دونوں فائلیں
כרטיס ניהול שינויים.
>=30 دن تک صفر Local-8 detections اور صفر Local-12 collisions دکھائے۔

## CI انضمام

1. اسکرپٹ کو dedicated job میں چلائیں اور outputs اپ لوڈ کریں۔
2. جب `audit.json` Local selectors رپورٹ کرے (`domain.kind = local12`) تو merges روک دیں۔
   default `true` پر رکھیں (صرف dev/test میں regressions کی تشخیص کے وقت `false` کریں) اور
   `iroha tools address normalize` کو CI میں شامل کریں تاکہ
   regressions production تک پہنچنے سے پہلے فیل ہوں۔

مزید تفصیلات، evidence checklists، اور release-note snippet کے لیے سورس دستاویز دیکھیں
جسے آپ cutover کا اعلان کرتے وقت دوبارہ استعمال کر سکتے ہیں۔