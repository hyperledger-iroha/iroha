---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: صلاحیت کی نقل
عنوان: صلاحیت تخروپن رن بک SoraFS
سائڈبار_لیبل: صلاحیت تخروپن رن بک
تفصیل: تولیدی فکسچر ، Prometheus برآمدات اور Grafana ڈیش بورڈز کے ساتھ SF-2C صلاحیت مارکیٹ تخروپن کٹ کا آغاز۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کا آئینہ دار ہے۔ جب تک میراثی اسفنکس دستاویزات کا سیٹ مکمل طور پر ہجرت نہ ہوجائے تب تک دونوں کاپیاں ہم وقت ساز رکھیں۔
:::

اس رن بک نے بتایا ہے کہ SF-2C صلاحیت مارکیٹ تخروپن سویٹ کو کیسے چلائیں اور نتیجے میں ہونے والی پیمائش کو تصور کریں۔ یہ کوٹہ مذاکرات ، فیل اوور ہینڈلنگ ، اور `docs/examples/sorafs_capacity_simulation/` میں ڈٹرمینسٹک فکسچر کا استعمال کرتے ہوئے ختم ہونے والے تدارک کو ختم کرنے کی جانچ کرتا ہے۔ پے لوڈ کی گنجائش اب بھی `sorafs_manifest_stub capacity` استعمال کرتی ہے۔ ظاہر/کار پیکیجنگ اسٹریمز کے لئے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI نمونے تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` `sorafs_manifest_stub capacity` کو Norito پے لوڈ تیار کرنے کے لئے لپیٹتا ہے ، بیس 64 بلوب ، Torii درخواست باڈیوں اور JSON خلاصے کے لئے:

- کوٹہ مذاکرات کے منظر نامے میں حصہ لینے والے فراہم کنندگان کے تین اعلامیہ۔
- فراہم کنندگان کے مابین اسٹیج کو تقسیم کرنے کا ایک نقل۔
- پری ناکامی بیس لائن ، ناکامی کا وقفہ اور فیل اوور بازیافت کے لئے ٹیلی میٹری اسنیپ شاٹس۔
- مصنوعی حادثے کے بعد درخواست میں کمی کے ساتھ پے لوڈ کا تنازعہ۔

تمام نمونے `./artifacts` میں رکھے گئے ہیں (پہلی دلیل کے طور پر ایک مختلف ڈائریکٹری کو پاس کرکے اسے زیر کیا جاسکتا ہے)۔ پڑھنے کے قابل سیاق و سباق کے لئے `_summary.json` فائلوں کو چیک کریں۔

## 2۔ مجموعی نتائج اور ریلیز میٹرکس

```bash
./analyze.py --artifacts ./artifacts
```

تجزیہ کار پیدا کرتا ہے:

- `capacity_simulation_report.json` - مجموعی تقسیم ، فیل اوور ڈیلٹا اور تنازعہ میٹا ڈیٹا۔
- `capacity_simulation.prom` - ٹیکسٹ فائل Prometheus (`sorafs_simulation_*`) میٹرکس ، ٹیکسٹ فائل کلیکٹر نوڈ ایکسپلٹر یا ایک علیحدہ سکریپ نوکری کے لئے موزوں ہے۔

مثال کے طور پر کھرچنا Prometheus تشکیل:

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

`capacity_simulation.prom` پر ٹیکسٹ فائل کلیکٹر کی نشاندہی کریں (اگر نوڈ ایکسپورٹر استعمال کررہے ہیں تو ، اسے `--collector.textfile.directory` کے ذریعے منظور شدہ ڈائریکٹری میں کاپی کریں)۔

## 3. درآمد ڈیش بورڈ Grafana

1. Grafana میں ، `dashboards/grafana/sorafs_capacity_simulation.json` درآمد کریں۔
2. ڈیٹا سورس متغیر `Prometheus` کو مذکورہ بالا سکریپ ہدف پر پابند کریں۔
3. پینل چیک کریں:
   - ** کوٹہ مختص (GIB) ** ہر فراہم کنندہ کے لئے کمٹ/تفویض بیلنس کو ظاہر کرتا ہے۔
   - ** فیل اوور ٹرگر ** جب ناکامی کی پیمائش موصول ہوتی ہے تو*فیل اوور ایکٹو*میں تبدیل ہوجاتی ہے۔
   - ** آؤٹ ٹائم کے دوران اپ ٹائم ڈراپ ** `alpha` فراہم کنندہ کے لئے ڈراپ فیصد کو دکھاتا ہے۔
   - ** درخواست کردہ سلیش فیصد ** تنازعہ کی حقیقت سے تدارک کے گتانک کو تصور کرتا ہے۔

## 4. متوقع چیک

- `sorafs_simulation_quota_total_gib{scope="assigned"}` جب تک کل کمٹ رہتا ہے> = 600 تک `600` کے برابر ہے۔
- `sorafs_simulation_failover_triggered` `1` کو ظاہر کرتا ہے ، اور متبادل فراہم کرنے والے میٹرک کو نمایاں کرتا ہے `beta`۔
- `sorafs_simulation_slash_requested` فراہم کنندہ `alpha` کے لئے `0.15` (15 ٪ SLASH) دکھاتا ہے۔

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` چلائیں اس بات کی تصدیق کے لئے کہ فکسچر ابھی بھی CLI اسکیما کے ذریعہ قبول کیے گئے ہیں۔