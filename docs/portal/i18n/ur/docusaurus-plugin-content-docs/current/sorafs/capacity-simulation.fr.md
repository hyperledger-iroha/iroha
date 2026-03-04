---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: صلاحیت کی نقل
عنوان: صلاحیت تخروپن رن بک SoraFS
سائڈبار_لیبل: صلاحیت تخروپن رن بک
تفصیل: تولیدی فکسچر ، Prometheus برآمدات اور Grafana ڈیش بورڈز کے ساتھ SF-2C صلاحیت مارکیٹ پلیس تخروپن کٹ ورزش کریں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ میراثی اسفنکس دستاویزات کا سیٹ مکمل طور پر ہجرت نہ ہوجائے۔
:::

اس رن بک میں بتایا گیا ہے کہ SF-2C صلاحیت مارکیٹ پلیس تخروپن کٹ کو کیسے چلائیں اور اس کے نتیجے میں میٹرکس کا تصور کیا جائے۔ یہ کوٹہ مذاکرات ، فیل اوور مینجمنٹ اور `docs/examples/sorafs_capacity_simulation/` میں ڈٹرمینسٹک فکسچر کا استعمال کرتے ہوئے اختتام سے آخر تک ختم کرنے کی توثیق کرتا ہے۔ صلاحیت کے پے لوڈ ہمیشہ `sorafs_manifest_stub capacity` استعمال کرتے ہیں۔ ظاہر/کار پیکیجنگ اسٹریمز کے لئے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI نمونے تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` `sorafs_manifest_stub capacity` کو Norito پے لوڈز ، بیس 64 بلبس ، Torii درخواست باڈیوں ، اور JSON ڈائجسٹس کے لئے۔

- کوٹہ مذاکرات کے منظر نامے میں حصہ لینے والے سپلائرز کے تین اعلامیہ۔
- ان فراہم کنندگان کے مابین اسٹیجنگ کو مختص کرنے کا ایک نقل آرڈر۔
- پری ناکامی بیس لائن ، ناکامی کا وقفہ اور فیل اوور بازیافت کے لئے ٹیلی میٹری اسنیپ شاٹس۔
- ایک تنازعہ پے لوڈ جو مصنوعی بندش کے بعد گرنے کی درخواست کرتا ہے۔

تمام نمونے `./artifacts` کے تحت دائر کیے جاتے ہیں (پہلی دلیل کے طور پر کسی اور ڈائرکٹری کو پاس کرکے تبدیل کریں)۔ پڑھنے کے قابل سیاق و سباق کے لئے `_summary.json` فائلوں کا معائنہ کریں۔

## 2۔ مجموعی نتائج اور آؤٹ پٹ میٹرکس

```bash
./analyze.py --artifacts ./artifacts
```

تجزیہ کار تیار کرتا ہے:

- `capacity_simulation_report.json` - مجموعی مختص ، فیل اوور ڈیلٹا اور قانونی چارہ جوئی میٹا ڈیٹا۔
- `capacity_simulation.prom` - ٹیکسٹ فائل میٹرکس Prometheus (`sorafs_simulation_*`) نوڈ ایکسپورٹر ٹیکسٹ فائل کلیکٹر یا آزاد کھرچنے والی نوکری کے مطابق ڈھال لیا گیا۔

کھرچنے والی ترتیب Prometheus کی مثال:

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

ٹیکسٹ فائل کلیکٹر کو `capacity_simulation.prom` پر ہدایت کریں (اگر نوڈ ایکسپورٹر استعمال کررہے ہیں تو ، اسے `--collector.textfile.directory` کے ذریعے منظور شدہ ڈائریکٹری میں کاپی کریں)۔

## 3. درآمد ڈیش بورڈ Grafana

1. Grafana میں ، `dashboards/grafana/sorafs_capacity_simulation.json` درآمد کریں۔
2. ڈیٹا سورس متغیر `Prometheus` کو اوپر کی تشکیل شدہ کھرچنا ہدف کے ساتھ منسلک کریں۔
3. نشانیاں چیک کریں:
   - ** کوٹہ مختص (GIB) ** ہر سپلائر کے لئے پرعزم/تفویض بیلنس دکھاتا ہے۔
   - ** فیل اوور ٹرگر ** جب ناکامی کی پیمائش آتی ہے تو*فیل اوور ایکٹو*میں تبدیل ہوجاتی ہے۔
   - ** آؤٹ ٹائم کے دوران اپ ٹائم ڈراپ ** فراہم کنندہ `alpha` کے لئے فیصد کے نقصان کو پلاٹ کرتا ہے۔
   - ** درخواست کردہ سلیش فیصد ** تنازعہ کی حقیقت سے نکلے ہوئے تدارک کے تناسب کو تصور کرتا ہے۔

## 4. متوقع تصدیق- `sorafs_simulation_quota_total_gib{scope="assigned"}` جب تک کل پرعزم ہے> = 600 تک `600` کے برابر ہے۔
- `sorafs_simulation_failover_triggered` `1` اور متبادل فراہم کنندہ میٹرک ہائی لائٹس `beta` کو ظاہر کرتا ہے۔
- `sorafs_simulation_slash_requested` وینڈر ID `alpha` کے لئے `0.15` (15 ٪ SLASH) کی وضاحت کرتا ہے۔

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` چلائیں اس بات کی تصدیق کے لئے کہ فکسچر ابھی بھی CLI اسکیما کے ذریعہ قبول کیے گئے ہیں۔