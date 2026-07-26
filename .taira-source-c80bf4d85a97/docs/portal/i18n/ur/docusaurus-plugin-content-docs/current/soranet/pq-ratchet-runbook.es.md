---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-RATCHET-RINBOOK
عنوان: سورانیٹ پی کیو رچٹ موک
سائڈبار_لیبل: پی کیو رچٹ رن بک
تفصیل: ٹائرڈ پی کیو گمنامی پالیسی کو فروغ دینے یا ڈیموٹنگ کرتے وقت ٹیسٹ کے اقدامات جو ٹیلی میٹری کی توثیق کے ساتھ تھے۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

## مقصد

یہ رن بک سورانیٹ کے ٹائرڈ پوسٹ کوانٹم (پی کیو) کی شناخت نہ ہونے کی پالیسی کے لئے ڈرل کی ترتیب کی رہنمائی کرتی ہے۔ آپریٹرز دونوں پروموشن (اسٹیج اے -> اسٹیج بی -> اسٹیج سی) کی تکرار کرتے ہیں اور جب پی کیو کی فراہمی میں کمی آتی ہے تو اسٹیج بی/اے پر کنٹرول شدہ انحطاط۔ ڈرل ٹیلی میٹری ہکس (`sorafs_orchestrator_policy_events_total` ، `sorafs_orchestrator_brownouts_total` ، `sorafs_orchestrator_pq_ratio_*`) کی توثیق کرتی ہے اور واقعے کی مشق لاگ کے لئے نمونے جمع کرتی ہے۔

## شرائط

- صلاحیت کے وزن کے ساتھ تازہ ترین بائنری `sorafs_orchestrator` (`docs/source/soranet/reports/pq_ratchet_validation.md` میں دکھائے گئے موک حوالہ پر یا اس کے بعد)۔
- Prometheus/Grafana اسٹیک تک رسائی جو `dashboards/grafana/soranet_pq_ratchet.json` کی خدمت کرتی ہے۔
- گارڈ ڈائرکٹری کا برائے نام سنیپ شاٹ۔ ڈرل سے پہلے ایک کاپی لائیں اور تصدیق کریں:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

اگر ماخذ ڈائرکٹری صرف JSON شائع کرتی ہے تو ، گردش کے مددگار چلانے سے پہلے `soranet-directory build` کے ساتھ بائنری Norito میں دوبارہ انک کوڈ کریں۔

- CLI کے ساتھ میٹا ڈیٹا اور پری اسٹیج جاری کرنے والے گردش نمونے پر قبضہ کریں:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- نیٹ ورکنگ ٹیموں کے ذریعہ منظور شدہ ونڈو کو تبدیل کریں۔

## تشہیر کے اقدامات

1. ** اسٹیج آڈٹ **

   ابتدائی مرحلے کو رجسٹر کریں:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   فروغ دینے سے پہلے `anon-guard-pq` کا انتظار کریں۔

2. ** اسٹیج بی (اکثریت پی کیو) کو فروغ دیں **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظار کریں> = 5 منٹ کی تازہ کاری کے لئے۔
   - Grafana میں (ڈیش بورڈ `SoraNet PQ Ratchet Drill`) تصدیق کریں کہ "پالیسی واقعات" پینل `outcome=met` کو `stage=anon-majority-pq` کے لئے دکھاتا ہے۔
   - اسکرین شاٹ یا JSON پینل پر قبضہ کریں اور اسے واقعے کے لاگ سے منسلک کریں۔

3. ** اسٹیج سی (سخت پی کیو) کو فروغ دیں **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - تصدیق کریں کہ `sorafs_orchestrator_pq_ratio_*` ہسٹگرامس 1.0 میں ہیں۔
   - تصدیق کریں کہ براؤن آؤٹ کاؤنٹر فلیٹ رہتا ہے۔ اگر نہیں تو ، ڈاؤن گریڈ مراحل پر عمل کریں۔

## ڈاؤن گریڈ/براؤن آؤٹ ڈرل

1. ** PQ کی مصنوعی قلت کو اکساتا ہے **

   صرف کلاسک اندراجات میں گارڈ ڈائرکٹری کو تراش کر کھیل کے میدان کے ماحول میں پی کیو ریلے کو غیر فعال کریں ، پھر آرکسٹریٹر کیشے کو دوبارہ لوڈ کریں:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. ** براؤن آؤٹ ٹیلی میٹری دیکھیں **

   - ڈیش بورڈ: "براؤن آؤٹ ریٹ" پینل 0 سے اوپر جاتا ہے۔
   - پروم کیو ایل: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` کو `anonymity_reason="missing_majority_pq"` کے ساتھ `anonymity_outcome="brownout"` کی اطلاع دینی ہوگی۔

3. ** اسٹیج بی / اسٹیج اے ** کے لئے مسمار کیا گیا

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   اگر پی کیو سپلائی ابھی بھی ناکافی ہے تو ، `anon-guard-pq` میں نیچے کی جائیں۔ جب براؤن آؤٹ کاؤنٹر مستحکم ہوتا ہے اور پروموشنز کو دوبارہ لاگو کیا جاسکتا ہے تو ڈرل ختم ہوتی ہے۔

4. ** گارڈ ڈائرکٹری کو بحال کریں **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## ٹیلی میٹری اور نمونے- ** ڈیش بورڈ: ** `dashboards/grafana/soranet_pq_ratchet.json`
۔
- ** واقعہ لاگ: ** ٹیلی میٹری کے ٹکڑوں اور آپریٹر نوٹ کو `docs/examples/soranet_pq_ratchet_fire_drill.log` پر جوڑتا ہے۔
۔

مثال:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

گورننس پیکیج میں تیار کردہ میٹا ڈیٹا اور دستخط منسلک کرتا ہے۔

## رول بیک

اگر ڈرل اصل پی کیو کی قلت کا پتہ لگاتا ہے تو ، یہ اسٹیج اے میں رہتا ہے ، نیٹ ورکنگ ٹی ایل کو مطلع کرتا ہے ، اور جمع شدہ میٹرکس کو جوڑتا ہے اور ساتھ ہی گارڈ ڈائرکٹری کے ساتھ ساتھ واقعہ ٹریکر سے فرق ہوتا ہے۔ معمول کی خدمت کو بحال کرنے کے لئے پہلے پکڑے گئے گارڈ ڈائرکٹری ایکسپورٹ کا استعمال کریں۔

::: ٹپ رجعت کی کوریج
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` مصنوعی توثیق فراہم کرتا ہے جو اس نقالی کی حمایت کرتا ہے۔
:::