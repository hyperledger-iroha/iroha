---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-RATCHET-RINBOOK
عنوان: سورانیٹ میں پی کیو رچٹ ورزش
سائڈبار_لیبل: پی کیو رچٹ گائیڈ
تفصیل: ایک شفٹ مشق کے لئے اقدامات جو مرحلہ وار پی کیو گمنامی پالیسی کو اپ گریڈ کرنے یا اسے نیچے کرنے کے ل telidation ٹیلی میٹری کی توثیق کے ساتھ ایک مرحلہ وار پی کیو گمنامی پالیسی ہے۔
---

::: نوٹ معیاری ماخذ
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کی عکاسی کرتا ہے۔ پرانی دستاویزات ریٹائر ہونے تک دونوں کاپیاں ایک جیسے رکھیں۔
:::

## مقصد

یہ گائیڈ سورانیٹ کے مرحلہ وار پوسٹ کوانٹم (پی کیو) گمنامی کی پالیسی کے لئے ہنگامی ورزش کی ترتیب کی رہنمائی کرتا ہے۔ آپریٹرز اپ گریڈ کرنے کی مشق کرتے ہیں (اسٹیج اے -> اسٹیج بی -> اسٹیج سی) اور جب پی کیو سپلائی کم چلتی ہے تو اسٹیج بی/اے میں کنٹرول میں کمی ہوتی ہے۔ مشق ٹیلی میٹری ہکس (`sorafs_orchestrator_policy_events_total` ، `sorafs_orchestrator_brownouts_total` ، `sorafs_orchestrator_pq_ratio_*`) کی تصدیق کرتی ہے اور واقعہ ڈرل لاگ کے لئے نوادرات جمع کرتی ہے۔

## تقاضے

- `sorafs_orchestrator` کے لئے تازہ ترین بائنری کی صلاحیت کے ساتھ وزن (`docs/source/soranet/reports/pq_ratchet_validation.md` میں دکھائے جانے والے ورزش کے حوالہ سے یا اس کے بعد)۔
- Prometheus/Grafana پیکیج تک رسائی جو `dashboards/grafana/soranet_pq_ratchet.json` کی خدمت کرتی ہے۔
- گارڈ ڈائرکٹری کے لئے میرا نام سنیپ شاٹ کریں۔ اپنے ورزش سے پہلے ایک کاپی لائیں اور چیک کریں:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

اگر سورس ڈائرکٹری صرف JSON شائع کرتی ہے تو ، مددگاروں کی گردش کو چلانے سے پہلے اسے بائنری Norito میں `soranet-directory build` کے ذریعے دوبارہ حاصل کریں۔

- میٹا ڈیٹا پر قبضہ کریں اور سی ایل آئی کے ذریعے نوادرات جاری کرنے والے گردش تیار کریں:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- آن کال نیٹ ورکنگ اور مانیٹرنگ ٹیموں کے ذریعہ منظور شدہ ونڈو کو تبدیل کریں۔

## اپ گریڈ اقدامات

1. ** چیکنگ اسٹیج **

   پرائمری اسکول کا ریکارڈ:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   اپ گریڈ کرنے سے پہلے `anon-guard-pq` کی توقع کریں۔

2. ** اسٹیج بی (اکثریت پی کیو) میں اپ گریڈ کریں **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظار کریں> = 5 منٹ تک منشور کو تازہ دم کرنے کے لئے۔
   - Grafana میں (پینل `SoraNet PQ Ratchet Drill`) تصدیق کریں کہ "پالیسی واقعات" پینل `outcome=met` کو `stage=anon-majority-pq` کے لئے دکھاتا ہے۔
   - ڈیش بورڈ کا اسکرین شاٹ یا JSON لیں اور اسے واقعے کے لاگ سے منسلک کریں۔

3. ** اسٹیج سی (سخت پی کیو) میں اپ گریڈ کریں **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - تصدیق کریں کہ `sorafs_orchestrator_pq_ratio_*` چارٹ 1.0 جا رہے ہیں۔
   - اس بات کو یقینی بنائیں کہ براؤن آؤٹ کاؤنٹر مستقل رہے۔ بصورت دیگر کمی کے اقدامات پر عمل کریں۔

## براؤن آؤٹ ورزش

1. ** مصنوعی پی کیو کی کمی کو شامل کرنا **

   صرف کلاسک اندراجات میں گارڈ ڈائرکٹری کو تراش کر کھیل کے میدان کے ماحول میں پی کیو ریلے کو غیر فعال کریں ، پھر آرکسٹریٹر کے کیشے کو دوبارہ لوڈ کریں:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. ** براؤن آؤٹ کے ٹیلی میٹری کی نگرانی کریں **

   - ڈیش بورڈ: "براؤن آؤٹ ریٹ" پینل 0 سے اوپر بڑھتا ہے۔
   - پروم کیو ایل: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` کو `anonymity_reason="missing_majority_pq"` کے ساتھ `anonymity_outcome="brownout"` کی اطلاع دینی ہوگی۔

3. ** اسٹیج بی / اسٹیج اے ** کے لئے مسمار کرنا **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   اگر پی کیو سپلائی ناکافی رہتی ہے تو ، `anon-guard-pq` پر کم کریں۔ ورزش اس وقت ختم ہوتی ہے جب براؤن آؤٹ کاؤنٹر مستحکم ہوجاتے ہیں اور اپ گریڈ کو دوبارہ شروع کیا جاسکتا ہے۔

4. ** گارڈ ڈائرکٹری کو بحال کریں **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## ٹیلی میٹری اور نوادرات

- ** ڈیش بورڈ: ** `dashboards/grafana/soranet_pq_ratchet.json`
۔
- ** واقعہ لاگ: ** ٹیلی میٹری کے اقتباسات اور آپریٹر نوٹ کو `docs/examples/soranet_pq_ratchet_fire_drill.log` پر منسلک کریں۔
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

گورننس میں تیار کردہ ڈیٹا اور دستخط منسلک کریں۔

## رول بیکاگر مشق پی کیو کی حقیقی کمی کو ظاہر کرتی ہے تو ، اسٹیج اے پر رہیں ، نیٹ ورکنگ ٹی ایل کو مطلع کریں ، اور جمع شدہ میٹرکس کو گارڈ ڈائرکٹری کی مختلف حالتوں کے ساتھ منسلک کریں۔ معمول کی خدمت کو بحال کرنے کے لئے پہلے پکڑے گئے گارڈ ڈائرکٹری برآمد کا استعمال کریں۔

::: ٹپ رجعت کی کوریج
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` مصنوعی توثیق فراہم کرتا ہے جو اس مشق کی حمایت کرتا ہے۔
:::