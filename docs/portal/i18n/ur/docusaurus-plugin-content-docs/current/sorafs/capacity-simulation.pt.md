---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: صلاحیت کی نقل
عنوان: SoraFS صلاحیت تخروپن رن بک
سائڈبار_لیبل: صلاحیت تخروپن رن بک
تفصیل: تولیدی فکسچر ، Prometheus برآمدات ، اور Grafana ڈیش بورڈز کے ساتھ SF-2C صلاحیت مارکیٹ پلیس انکولیشن ٹول کٹ چلائیں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

اس رن بک میں بتایا گیا ہے کہ SF-2C صلاحیت مارکیٹ پلیس تخروپن کٹ کو کیسے چلائیں اور اس کے نتیجے میں میٹرکس کا تصور کیا جائے۔ یہ کوٹہ مذاکرات ، فیل اوور ہینڈلنگ ، اور `docs/examples/sorafs_capacity_simulation/` میں ڈٹرمینسٹک فکسچر کا استعمال کرتے ہوئے اختتام سے آخر تک ختم کرنے کی توثیق کرتا ہے۔ صلاحیت کے پے لوڈ اب بھی `sorafs_manifest_stub capacity` استعمال کرتے ہیں۔ منشور/کار پیکیجنگ بہاؤ کے لئے `iroha app sorafs toolkit pack` استعمال کریں۔

## 1. CLI نمونے تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` `sorafs_manifest_stub capacity` کو Norito پے لوڈ ، بیس 64 بلبس ، Torii کے لئے باڈیز کی درخواست ، اور JSON ڈائجسٹس کے لئے

- کوٹہ مذاکرات کے منظر نامے میں حصہ لینے والے فراہم کنندگان کے تین بیانات۔
- ایک نقل کا آرڈر جو ان فراہم کنندگان کے مابین اسٹیجنگ ظاہر کرتا ہے۔
- پری ناکامی بیس لائن ، ناکامی کا وقفہ ، اور فیل اوور بازیافت کے لئے ٹیلی میٹری اسنیپ شاٹس۔
- ایک تنازعہ پے لوڈ جس میں مصنوعی ناکامی کے بعد گرنے کی درخواست کی گئی ہے۔

تمام نمونے `./artifacts` پر جاتے ہیں (پہلی دلیل کے طور پر ایک مختلف ڈائرکٹری پاس کرکے تبدیل کریں)۔ پڑھنے کے قابل سیاق و سباق کے لئے `_summary.json` فائلوں کا معائنہ کریں۔

## 2۔ مجموعی نتائج اور جاری میٹرکس جاری کریں

```bash
./analyze.py --artifacts ./artifacts
```

پارسر آؤٹ پٹس:

- `capacity_simulation_report.json` - مجموعی مختص ، فیل اوور ڈیلٹا ، اور تنازعہ میٹا ڈیٹا۔
- `capacity_simulation.prom` - Prometheus (`sorafs_simulation_*`) ٹیکسٹ فائل میٹرکس نوڈ ایکسپورٹر کے ٹیکسٹ فائل کلکٹر یا اسٹینڈ اسٹون کھرچنے والی نوکری کے لئے موزوں ہے۔

Prometheus کھرچنا ترتیب مثال:

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

ٹیکسٹ فائل کلیکٹر کو `capacity_simulation.prom` پر (نوڈ ایکسپورٹر کا استعمال کرتے وقت ، `--collector.textfile.directory` کے ذریعے منظور شدہ ڈائریکٹری میں کاپی کریں) کی طرف اشارہ کریں۔

## 3. Grafana ڈیش بورڈ درآمد کریں

1. Grafana میں ، `dashboards/grafana/sorafs_capacity_simulation.json` درآمد کریں۔
2. ڈیٹا سورس متغیر `Prometheus` کو اوپر کی تشکیل شدہ کھرچنا ہدف سے لنک کریں۔
3. ڈیش بورڈز چیک کریں:
   - ** کوٹہ مختص (GIB) ** ہر فراہم کنندہ کے پرعزم/تفویض بیلنس کو ظاہر کرتا ہے۔
   - ** فیل اوور ٹرگر ***فیل اوور فعال*میں تبدیلیاں جب ناکامی کی پیمائش آتی ہے۔
   - ** آؤٹ ٹائم کے دوران اپ ٹائم ڈراپ ** فراہم کنندہ `alpha` کی فیصد کمی کو ظاہر کرتا ہے۔
   - ** درخواست کردہ سلیش فیصد ** تنازعہ کی حقیقت سے نکلے ہوئے تدارک کے تناسب کو تصور کرتا ہے۔

## 4. متوقع چیک

- `sorafs_simulation_quota_total_gib{scope="assigned"}` کے برابر `600` جبکہ پرعزم کل باقی> = 600۔
- `sorafs_simulation_failover_triggered` `1` اور فال بیک فراہم کرنے والے میٹرک کو نمایاں کریں `beta`۔
- `sorafs_simulation_slash_requested` فراہم کنندہ شناخت کنندہ `alpha` کے لئے `0.15` (15 ٪ SLASH) کی رپورٹ کرتا ہے۔`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` چلائیں اس بات کی تصدیق کے لئے کہ فکسچر ابھی بھی CLI اسکیما کے ذریعہ تعاون یافتہ ہیں۔