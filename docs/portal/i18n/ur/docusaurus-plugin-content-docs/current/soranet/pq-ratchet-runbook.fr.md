---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-ratchet-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-RATCHET-RINBOOK
عنوان: پی کیو رچیٹ سورنیٹ تخروپن
سائڈبار_لیبل: رن بوک پی کیو رچٹ
تفصیل: پی کیو گمنامی پالیسی کو فروغ دینے یا اس کو کم کرنے کے لئے کال پر ریہرسل اقدامات اور ڈٹرمینسٹک ٹیلی میٹری کی توثیق کریں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/soranet/pq_ratchet_runbook.md` کی عکاسی کرتا ہے۔ پرانے دستاویزات کا سیٹ ریٹائر ہونے تک دونوں کاپیاں منسلک رکھیں۔
:::

## مقصد

یہ رن بک سورانیٹ کے پرتوں والے پوسٹ کوانٹم (پی کیو) کی گمنامی پالیسی کے لئے فائر ڈرل ترتیب کی رہنمائی کرتی ہے۔ آپریٹرز پروموشن (اسٹیج اے -> اسٹیج بی -> اسٹیج سی) کے ساتھ ساتھ جب پی کیو کی پیش کش کرتے ہیں تو اسٹیج بی/اے میں کنٹرول شدہ ڈیموشن کو بھی دہراتا ہے۔ ڈرل ٹیلی میٹری ہکس (`sorafs_orchestrator_policy_events_total` ، `sorafs_orchestrator_brownouts_total` ، `sorafs_orchestrator_pq_ratio_*`) کی توثیق کرتی ہے اور واقعے کی ریہرسل لاگ کے نمونے جمع کرتی ہے۔

## شرائط

- صلاحیت کے وزن کے ساتھ تازہ ترین بائنری `sorafs_orchestrator` (`docs/source/soranet/reports/pq_ratchet_validation.md` میں ڈرل ریفرنس کے برابر یا بعد میں اس کا ارتکاب کریں)۔
- اسٹیک Prometheus/Grafana تک رسائی جو `dashboards/grafana/soranet_pq_ratchet.json` کی خدمت کرتی ہے۔
- گارڈ ڈائرکٹری کا برائے نام سنیپ شاٹ۔ سوراخ کرنے سے پہلے ایک کاپی اکٹھا اور تصدیق کریں:

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

- آن کال نیٹ ورکنگ اور مشاہدہ کرنے والی ٹیموں کے ذریعہ منظور شدہ ونڈو کو تبدیل کریں۔

## تشہیر کے اقدامات

1. ** آڈٹ انٹرنشپ **

   ابتدائی کورس ریکارڈ کریں:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   پروموشن سے پہلے `anon-guard-pq` کا انتظار کریں۔

2. ** اسٹیج بی (اکثریت پی کیو) میں تشہیر **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - انتظار کریں> = 5 منٹ کی تازہ کاری کے لئے۔
   - Grafana میں (ڈیش بورڈ `SoraNet PQ Ratchet Drill`) تصدیق کریں کہ "پالیسی واقعات" پینل `outcome=met` کو `stage=anon-majority-pq` کے لئے دکھاتا ہے۔
   - پینل کے اسکرین شاٹ یا JSON پر قبضہ کریں اور اسے کریش لاگ سے منسلک کریں۔

3. ** اسٹیج سی (سخت پی کیو) کی تشہیر **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - چیک کریں کہ ہسٹگرامس `sorafs_orchestrator_pq_ratio_*` 1.0 کی طرف جاتا ہے۔
   - تصدیق کریں کہ براؤن آؤٹ کاؤنٹر فلیٹ رہتا ہے۔ بصورت دیگر ڈاون گریڈ مراحل پر عمل کریں۔

## تخفیف/براؤن آؤٹ ڈرل

1. ** مصنوعی پی کیو کی کمی کو راغب کریں **

   صرف کلاسک اندراجات میں گارڈ ڈائرکٹری کو تراش کر کھیل کے میدان کے ماحول میں پی کیو ریلے کو غیر فعال کریں ، پھر آرکسٹریٹر کیشے کو دوبارہ لوڈ کریں:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. ** براؤن آؤٹ ٹیلی میٹری کا مشاہدہ کریں **

   - ڈیش بورڈ: "براؤن آؤٹ ریٹ" پینل 0 سے اوپر بڑھتا ہے۔
   - پروم کیو ایل: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` کو `anonymity_reason="missing_majority_pq"` کے ساتھ `anonymity_outcome="brownout"` کی اطلاع دینی ہوگی۔

3. ** اسٹیج بی / اسٹیج اے ** میں نیچے کی گریڈ

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   اگر پی کیو سپلائی ناکافی رہتی ہے تو ، `anon-guard-pq` میں نیچے کی جائیں۔ جب براؤن آؤٹ کاؤنٹر مستحکم ہوتا ہے اور پروموشنز کو دوبارہ لاگو کیا جاسکتا ہے تو ڈرل ختم ہوتی ہے۔

4. ** گارڈ ڈائرکٹری کو بحال کریں **

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## ٹیلی میٹری اور نمونے- ** ڈیش بورڈ: ** `dashboards/grafana/soranet_pq_ratchet.json`
۔
- ** واقعہ لاگ: ** ٹیلی میٹری کے ٹکڑوں اور آپریٹر نوٹ کو `docs/examples/soranet_pq_ratchet_fire_drill.log` میں شامل کریں۔
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

گورننس فائل میں تیار کردہ میٹا ڈیٹا اور دستخط منسلک کریں۔

## رول بیک

اگر ڈرل ایک حقیقی پی کیو کی کمی کو ظاہر کرتی ہے تو ، اسٹیج اے پر رہیں ، نیٹ ورکنگ ٹی ایل کو مطلع کریں اور جمع شدہ میٹرکس کے ساتھ ساتھ گارڈ ڈائرکٹری کو بھی واقعہ ٹریکر سے مختلف کرتا ہے۔ عام خدمت کو بحال کرنے کے لئے پہلے گارڈ ڈائرکٹری کیپچر ایکسپورٹ کا استعمال کریں۔

::: ٹپ رجعت کی کوریج
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` مصنوعی توثیق فراہم کرتا ہے جو اس ڈرل کی حمایت کرتا ہے۔
:::