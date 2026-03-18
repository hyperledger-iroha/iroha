---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-RATCHET-RINBOOK
عنوان: سورانیٹ پی کیو رچیٹ سمولکرم
سائڈبار_لیبل: پی کیو رچٹ رن بک
تفصیل: ٹیلیفونک ٹیلی میٹری کی توثیق کے ساتھ انٹرنشپ میں پی کیو گمنامی پالیسی کو فروغ دینے یا اسے نیچے کرنے کے لئے کال ٹیسٹ کے اقدامات۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

## مقصد

یہ رن بک سورنیٹ مراحل میں پوسٹ کوانٹم (پی کیو) کی گمنامی پالیسی کے نقلی ترتیب کی رہنمائی کرتی ہے۔ آپریٹرز پروموشن (اسٹیج اے -> اسٹیج بی -> اسٹیج سی) کی مشق کرتے ہیں اور جب پی کیو کی فراہمی میں کمی آتی ہے تو اسٹیج بی/اے پر دوبارہ کنٹرول شدہ ڈیموشن۔ سمیلاکرم ٹیلی میٹری ہکس (`sorafs_orchestrator_policy_events_total` ، `sorafs_orchestrator_brownouts_total` ، `sorafs_orchestrator_pq_ratio_*`) کی توثیق کرتا ہے اور واقعے کی مشق لاگ کے لئے نمونے جمع کرتا ہے۔

## شرائط

- آخری بائنری `sorafs_orchestrator` صلاحیت کے وزن کے ساتھ (`docs/source/soranet/reports/pq_ratchet_validation.md` میں دکھائے گئے ڈرل ریفرنس کے برابر یا اس سے زیادہ کا ارتکاب کریں)۔
- اسٹیک Prometheus/Grafana تک رسائی جو `dashboards/grafana/soranet_pq_ratchet.json` کی خدمت کرتی ہے۔
- گارڈ ڈائرکٹری کا برائے نام سنیپ شاٹ۔ نقالی سے پہلے ایک کاپی تلاش اور توثیق کریں:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

اگر ماخذ ڈائرکٹری صرف JSON شائع کرتی ہے تو ، گردش مددگار چلانے سے پہلے `soranet-directory build` کے ساتھ Norito بائنری میں دوبارہ انکوڈ کریں۔

- CLI کے ساتھ میٹا ڈیٹا اور پری اسٹیج جاری کرنے والے گردش نمونے پر قبضہ کریں:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- آن کال نیٹ ورکنگ اور مشاہدہ کرنے والی ٹیموں کے ذریعہ منظور شدہ ونڈو کو تبدیل کریں۔

## تشہیر کے اقدامات

1. ** اسٹیج آڈٹ **

   ابتدائی مرحلے کو رجسٹر کریں:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   پروموشن سے پہلے `anon-guard-pq` کا انتظار کریں۔

2. ** اسٹیج بی (اکثریت پی کیو) کو فروغ دیں **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظار کریں> = 5 منٹ کی تازہ کاری کے ل .۔
   - Grafana میں (ڈیش بورڈ `SoraNet PQ Ratchet Drill`) تصدیق کریں کہ "پالیسی واقعات" پینل `outcome=met` کو `stage=anon-majority-pq` کے لئے دکھاتا ہے۔
   - ڈیش بورڈ کے اسکرین شاٹ یا JSON پر قبضہ کریں اور اسے واقعے کے لاگ سے منسلک کریں۔

3. ** اسٹیج سی (سخت پی کیو) کو فروغ دیں **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - چیک کریں کہ ہسٹگرامس `sorafs_orchestrator_pq_ratio_*` 1.0 کی طرف جاتا ہے۔
   - تصدیق کریں کہ براؤن آؤٹ کاؤنٹر فلیٹ رہتا ہے۔ بصورت دیگر ، تخفیف کے مراحل پر عمل کریں۔

## ریلیگریشن / براؤن آؤٹ ڈرل

1. ** مصنوعی پی کیو کی قلت کو راغب کریں **

   صرف کلاسک اندراجات میں گارڈ ڈائرکٹری کو کم کرکے کھیل کے میدان کے ماحول میں پی کیو ریلے کو غیر فعال کریں ، پھر آرکسٹریٹر کیشے کو دوبارہ لوڈ کریں:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. ** براؤن آؤٹ ٹیلی میٹری کا مشاہدہ کریں **

   - ڈیش بورڈ: "براؤن آؤٹ ریٹ" پینل 0 سے اوپر بڑھتا ہے۔
   - پروم کیو ایل: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` `anonymity_reason="missing_majority_pq"` کے ساتھ `anonymity_outcome="brownout"` کی اطلاع دینا چاہئے۔

3. ** اسٹیج بی / اسٹیج اے ** کے لئے ڈیموٹ

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   اگر پی کیو سپلائی ابھی بھی ناکافی ہے تو ، `anon-guard-pq` میں نیچے کی جائیں۔ نقلی اس وقت ختم ہوتی ہے جب براؤن آؤٹ کاؤنٹر مستحکم ہوجاتے ہیں اور پروموشنز کو دوبارہ لاگو کیا جاسکتا ہے۔

4. ** گارڈ ڈائرکٹری کو بحال کریں **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## ٹیلی میٹری اور نمونے- ** ڈیش بورڈ: ** `dashboards/grafana/soranet_pq_ratchet.json`
۔
- ** واقعہ لاگ: ** ٹیلی میٹری کے ٹکڑوں اور آپریٹر نوٹ کو `docs/examples/soranet_pq_ratchet_fire_drill.log` پر منسلک کریں۔
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

گورننس پیکیج میں تیار کردہ میٹا ڈیٹا اور دستخط منسلک کریں۔

## رول بیک

اگر نقلی حقیقی پی کیو کی قلت کو ظاہر کرتی ہے تو ، اسٹیج اے میں رہیں ، نیٹ ورکنگ ٹی ایل کو مطلع کریں اور جمع شدہ میٹرکس کو گارڈ ڈائرکٹری کے ساتھ منسلک کریں۔ معمول کی خدمت کو بحال کرنے کے لئے پہلے پکڑے گئے گارڈ ڈائرکٹری برآمد کا استعمال کریں۔

::: ٹپ رجعت کی کوریج
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` مصنوعی توثیق فراہم کرتا ہے جو اس سمیلکرم کی حمایت کرتا ہے۔
:::