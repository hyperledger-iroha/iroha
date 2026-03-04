---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: صلاحیت کی نقل
عنوان: SoraFS صلاحیت تخروپن آپریشن دستی
سائڈبار_لیبل: صلاحیت تخروپن گائیڈ
تفصیل: تولیدی فکسچر ، Prometheus برآمدات ، اور Grafana بورڈز کا استعمال کرتے ہوئے SF-2C صلاحیت مارکیٹ تخروپن ٹول کٹ چلائیں۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کی عکاسی کرتا ہے۔ دو ورژن کو مطابقت پذیری میں رکھیں جب تک کہ پوری میراثی اسفنکس دستاویزات کا سیٹ ہجرت نہ ہوجائے۔
:::

یہ گائیڈ وضاحت کرتا ہے کہ SF-2C صلاحیت مارکیٹ تخروپن سویٹ کو کیسے چلائیں اور اس کے نتیجے میں میٹرکس دیکھیں۔ `docs/examples/sorafs_capacity_simulation/` میں لازمی فکسچر کا استعمال کرتے ہوئے کوٹہ مذاکرات ، فیل اوور پروسیسنگ ، اور اختتام سے آخر تک سلیشنگ پروسیسنگ کے لئے چیک۔ صلاحیت کے پے لوڈ اب بھی `sorafs_manifest_stub capacity` استعمال کرتے ہیں۔ ظاہر/کار پیکیجنگ فلوز کے لئے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI مخصوص نمونے بنائیں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` `sorafs_manifest_stub capacity` کو آؤٹ پٹ Norito پے لوڈ ، بیس 64 بلبس ، Torii درخواست باڈیوں ، اور JSON ڈائجسٹ کے لئے

- کوٹہ مذاکرات کے منظر نامے میں حصہ لینے والے سپلائرز کے تین بیانات۔
- ایک کاپی آرڈر ان فراہم کنندگان کے ذریعہ تیار کردہ مینی فیسٹ کو تقسیم کرتا ہے۔
- پری آؤٹٹیج بیس لائن ، بندش کی مدت ، اور فیل اوور بازیافت کے ٹیلی میٹک اسنیپ شاٹس۔
- پے لوڈ کے تنازعہ کو مصنوعی بندش کے بعد کم کرنے کی درخواست کرنا۔

تمام نمونے `./artifacts` کے تحت لکھے گئے ہیں (پہلی دلیل کے طور پر ایک مختلف ڈائرکٹری پاس کرکے تبدیل کیا جاسکتا ہے)۔ پڑھنے کے قابل سیاق و سباق کے لئے `_summary.json` فائلیں دیکھیں۔

## 2. نتائج اکٹھا کریں اور میٹرکس جاری کریں

```bash
./analyze.py --artifacts ./artifacts
```

تجزیہ کار تیار کرتا ہے:

- `capacity_simulation_report.json` - بلک کسٹمائزیشن ، فیل اوور پھیلاؤ ، اور تنازعہ میٹا ڈیٹا۔
- `capacity_simulation.prom` - Prometheus (`sorafs_simulation_*`) کے لئے ٹیکسٹ فائل پیرامیٹرز نوڈ ایکسپورٹر یا اسٹینڈ اسٹون کھرچنے والی نوکری کے ٹیکسٹ فائل کلکٹر کے لئے موزوں ہیں۔

Prometheus میں سکریپ سیٹنگ کی مثال:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

ٹیکسٹ فائل کلیکٹر کو `capacity_simulation.prom` پر نشاندہی کریں (جب نوڈس ایکپورٹر کا استعمال کرتے ہو تو اسے `--collector.textfile.directory` کے ذریعے منظور شدہ ڈائرکٹری میں کاپی کریں)۔

## 3. درآمد بورڈ Grafana

1. Grafana میں ، `dashboards/grafana/sorafs_capacity_simulation.json` درآمد کریں۔
2. ڈیٹا سورس متغیر `Prometheus` کو اوپر کی تشکیل شدہ کھرچنی ہدف سے باندھ دیں۔
3. پینل چیک کریں:
   - ** کوٹہ مختص (GIB) ** ہر فراہم کنندہ کے لئے پرعزم/مختص بیلنس دکھاتا ہے۔
   - ** فیل اوور ٹرگر ***فیل اوور ایکٹو*کی طرف رجوع کرتا ہے جب آؤٹ پٹ میٹرکس تک پہنچ جاتا ہے۔
   - ** آؤٹ ٹائم کے دوران اپ ٹائم ڈراپ ** فراہم کنندہ `alpha` کے لئے نقصان کی فیصد کو پلاٹ کرتا ہے۔
   - ** درخواست کردہ سلیش فیصد ** تنازعات کی حقیقت سے نکالا جانے والا پروسیسنگ فیصد دکھاتا ہے۔

## 4. متوقع امتحانات

- `sorafs_simulation_quota_total_gib{scope="assigned"}` کے برابر `600` جب تک کہ کل باقی باقی> = 600 تک کا ارتکاب کیا جائے۔
- `sorafs_simulation_failover_triggered` `1` دکھاتا ہے اور متبادل فراہم کنندہ میٹرک `beta` کو اجاگر کرتا ہے۔
- `sorafs_simulation_slash_requested` فراہم کنندہ ID `alpha` کے لئے `0.15` (15 ٪ SLASH) دکھاتا ہے۔

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` چلائیں اس بات کا یقین کرنے کے لئے کہ CLI کے نقشے کے ذریعہ ابھی بھی فکسچر کو قبول کیا گیا ہے۔