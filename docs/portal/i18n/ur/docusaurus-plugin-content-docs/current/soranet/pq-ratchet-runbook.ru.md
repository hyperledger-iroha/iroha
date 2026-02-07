---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-RATCHET-RINBOOK
عنوان: ڈرل پی کیو رچٹ سورانیٹ
سائڈبار_لیبل: رن بوک پی کیو رچٹ
تفصیل: ٹیلیفونک ٹیلی میٹری کی توثیق کے ساتھ اسٹیج پی کیو گمنامی پالیسی کو بڑھانے یا کم کرنے کے لئے کال پر ریہرسل اقدامات۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کا آئینہ دار ہے۔ جب تک میراثی دستاویزات ریٹائر نہ ہوجائیں دونوں کاپیاں ہم آہنگی میں رکھیں۔
:::

## مقصد

اس رن بک میں سورانیٹ کے اسٹیجڈ پوسٹ کوانٹم (پی کیو) کی گمنامی کی پالیسی کے فائر ڈرل تسلسل کی وضاحت کی گئی ہے۔ آپریٹرز دونوں پروموشن (اسٹیج اے -> اسٹیج بی -> اسٹیج سی) دونوں پر کام کرتے ہیں اور جب سپلائی پی کیو کے قطرے ہوتے ہیں تو اسٹیج بی/اے پر کنٹرول شدہ ڈیموشن۔ ڈرل ٹیلی میٹری ہکس (`sorafs_orchestrator_policy_events_total` ، `sorafs_orchestrator_brownouts_total` ، `sorafs_orchestrator_pq_ratio_*`) کی توثیق کرتا ہے اور واقعے کی ریہرسل لاگ کے لئے نوادرات جمع کرتا ہے۔

## شرائط

- تازہ ترین `sorafs_orchestrator` بائنری کی صلاحیت کے ساتھ بائنری (`docs/source/soranet/reports/pq_ratchet_validation.md` سے حوالہ ڈرل کے برابر یا بعد میں)۔
- Prometheus/Grafana اسٹیک تک رسائی ، جو `dashboards/grafana/soranet_pq_ratchet.json` کی خدمت کرتا ہے۔
- برائے نام گارڈ ڈائریکٹری اسنیپ شاٹ۔ ڈرل سے پہلے ایک کاپی حاصل کریں اور جانچ کریں:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

اگر ماخذ ڈائرکٹری صرف JSON شائع کرتی ہے تو ، گردش مددگار چلانے سے پہلے `soranet-directory build` کے ذریعے Norito بائنری میں دوبارہ انک کوڈ کریں۔

- CLI کا استعمال کرتے ہوئے جاری کرنے والے گردش کے میٹا ڈیٹا اور پری مرحلے کے نوادرات پر قبضہ کریں:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- چینل نیٹ ورکنگ اور مشاہدہ کرنے والی ٹیموں کے ذریعہ تبدیلی ونڈو کی منظوری دی گئی ہے۔

## تشہیر کے اقدامات

1. ** اسٹیج آڈٹ **

   ابتدائی مرحلے کو ٹھیک کریں:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   تشہیر سے پہلے ، `anon-guard-pq` کا انتظار کریں۔

2. ** اسٹیج بی میں تشہیر (اکثریت پی کیو) **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظار کریں> = 5 منٹ کی تازہ کاری کے ل .۔
   - Grafana (ڈیش بورڈ `SoraNet PQ Ratchet Drill`) میں ، یقینی بنائیں کہ "پالیسی ایونٹس" پینل `stage=anon-majority-pq` کے لئے `outcome=met` دکھاتا ہے۔
   - پینل کے اسکرین شاٹ یا JSON پر قبضہ کریں اور اسے واقعے کے لاگ سے منسلک کریں۔

3. ** اسٹیج سی میں تشہیر (سخت پی کیو) **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - چیک کریں کہ ہسٹگرام `sorafs_orchestrator_pq_ratio_*` 1.0 میں ہے۔
   - اس بات کو یقینی بنائیں کہ براؤن آؤٹ کاؤنٹر فلیٹ رہتا ہے۔ بصورت دیگر تخفیف اقدامات پر عمل کریں۔

## تخفیف / براؤن آؤٹ ڈرل

1. ** مصنوعی پی کیو کی کمی کو راغب کریں **

   کلاسیکی اندراجات میں گارڈ ڈائرکٹری کو تراش کر کھیل کے میدان کے ماحول میں پی کیو ریلے کو غیر فعال کریں ، پھر آرکسٹریٹر کیشے کو دوبارہ لوڈ کریں:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. ** ٹیلی میٹری براؤن آؤٹ دیکھیں **

   - ڈیش بورڈ: "براؤن آؤٹ ریٹ" پینل 0 سے اوپر چھلانگ لگاتا ہے۔
   - پروم کیو ایل: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` کو `anonymity_reason="missing_majority_pq"` کے ساتھ `anonymity_outcome="brownout"` کی اطلاع دینی ہوگی۔

3. ** اسٹیج بی / اسٹیج اے ** کے لئے مسمار کرنا **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   اگر سپلائی پی کیو ابھی بھی ناکافی ہے تو ، `anon-guard-pq` تک کم ہے۔ جب براؤن آؤٹ کاؤنٹرز مستحکم ہوجاتے ہیں اور پروموشنز کو دوبارہ اپلائی کی جاسکتی ہے تو ڈرل مکمل ہوجاتی ہے۔

4. ** گارڈ ڈائرکٹری کو بحال کریں **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## ٹیلی میٹری اور نوادرات- ** ڈیش بورڈ: ** `dashboards/grafana/soranet_pq_ratchet.json`
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

اگر ڈرل ایک حقیقی پی کیو کی کمی کو ظاہر کرتا ہے تو ، اسٹیج اے پر رہیں ، نیٹ ورکنگ ٹی ایل کو مطلع کریں اور جمع شدہ میٹرکس کو گارڈ ڈائرکٹری کے ساتھ منسلک کریں۔ معمول کی خدمت کو بحال کرنے کے لئے پہلے پکڑے گئے گارڈ ڈائرکٹری برآمد کا استعمال کریں۔

::: ٹپ رجعت کی کوریج
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` مصنوعی توثیق فراہم کرتا ہے جو اس ڈرل کی حمایت کرتا ہے۔
:::