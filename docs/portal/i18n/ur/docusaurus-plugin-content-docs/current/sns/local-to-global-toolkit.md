---
lang: ur
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Local -> Global ایڈریس ٹول کٹ

یہ صفحہ `docs/source/sns/local_to_global_toolkit.md` کا عکاس ہے۔ یہ roadmap آئٹم **ADDR-5c** کے لیے درکار CLI helpers اور runbooks اکٹھے کرتا ہے۔

## جائزہ

- `scripts/address_local_toolkit.sh` `iroha` CLI کو wrap کرتا ہے تاکہ یہ پیدا کرے:
  - `audit.json` -- `iroha tools address audit --format json` کا structured output۔
  - `normalized.txt` -- ہر Local-domain selector کے لیے IH58 (ترجیحی) / compressed (`sora`, second-best) literals۔
- اس اسکرپٹ کو address ingest dashboard (`dashboards/grafana/address_ingest.json`)
  اور Alertmanager rules (`dashboards/alerts/address_ingest_rules.yml`) کے ساتھ استعمال کریں تاکہ
  Local-8 / Local-12 cutover کی حفاظت ثابت ہو۔ Local-8 اور Local-12 collision panels اور
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, اور `AddressInvalidRatioSlo` alerts
  کو manifest تبدیلیاں promote کرنے سے پہلے دیکھیں۔
- UX اور incident-response کے لیے [Address Display Guidelines](address-display-guidelines.md) اور
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) کو دیکھیں۔

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

اختیارات:

- `--format compressed` IH58 کے بجائے `sora...` output کے لیے۔
- `domainless output (default)` تاکہ bare literals نکلیں۔
- `--audit-only` conversion step چھوڑنے کے لیے۔
- `--allow-errors` تاکہ malformed rows پر بھی scan جاری رہے (CLI behavior جیسا)۔

اسکرپٹ رن کے آخر میں artefact paths لکھتا ہے۔ دونوں فائلیں
change-management ticket کے ساتھ منسلک کریں اور Grafana screenshot بھی شامل کریں جو
>=30 دن تک صفر Local-8 detections اور صفر Local-12 collisions دکھائے۔

## CI انضمام

1. اسکرپٹ کو dedicated job میں چلائیں اور outputs اپ لوڈ کریں۔
2. جب `audit.json` Local selectors رپورٹ کرے (`domain.kind = local12`) تو merges روک دیں۔
   default `true` پر رکھیں (صرف dev/test میں regressions کی تشخیص کے وقت `false` کریں) اور
   `iroha tools address normalize` کو CI میں شامل کریں تاکہ
   regressions production تک پہنچنے سے پہلے فیل ہوں۔

مزید تفصیلات، evidence checklists، اور release-note snippet کے لیے سورس دستاویز دیکھیں
جسے آپ cutover کا اعلان کرتے وقت دوبارہ استعمال کر سکتے ہیں۔
