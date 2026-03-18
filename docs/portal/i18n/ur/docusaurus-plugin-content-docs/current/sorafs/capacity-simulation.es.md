---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/capacity-simulation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: صلاحیت کی نقل
عنوان: SoraFS صلاحیت تخروپن رن بک
سائڈبار_لیبل: صلاحیت تخروپن رن بک
تفصیل: تولیدی فکسچر ، Prometheus برآمدات اور Grafana ڈیش بورڈز کے ساتھ SF-2C صلاحیت مارکیٹ پلیس انکولیشن ٹول کٹ چلائیں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` کی عکاسی کرتا ہے۔ اس وقت تک دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ اسفینکس میں قائم میراثی دستاویزات مکمل طور پر ہجرت نہ ہوجائیں۔
:::

اس رن بک میں بتایا گیا ہے کہ SF-2C صلاحیت مارکیٹ پلیس تخروپن کٹ کو کیسے چلائیں اور اس کے نتیجے میں میٹرکس دیکھیں۔ `docs/examples/sorafs_capacity_simulation/` میں ڈٹرمینسٹک فکسچر کا استعمال کرتے ہوئے کوٹہ مذاکرات ، فیل اوور مینجمنٹ ، اور اختتام سے آخر تک ختم ہونے والے تدارک کی توثیق کرتا ہے۔ صلاحیت کے پے لوڈ اب بھی `sorafs_manifest_stub capacity` استعمال کرتے ہیں۔ منشور/کار پیکیجنگ بہاؤ کے لئے `iroha app sorafs toolkit pack` استعمال کرتا ہے۔

## 1. CLI نمونے تیار کریں

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` `sorafs_manifest_stub capacity` کو Norito پے لوڈز ، بیس 64 بلبس ، Torii کے لئے باڈیز کی درخواست کریں ، اور JSON ڈائجسٹس کے لئے درخواست کریں:

- کوٹہ مذاکرات کے منظر نامے میں حصہ لینے والے سپلائرز کے تین بیانات۔
- ایک نقل کا حکم جو ان فراہم کنندگان کے مابین اسٹیجنگ کو ظاہر کرتا ہے۔
- پری کریش بیس لائن ، کریش وقفہ ، اور فیل اوور بازیافت کے لئے ٹیلی میٹری اسنیپ شاٹس۔
- ایک تنازعہ پے لوڈ جس میں نقلی زوال کے بعد گرنے کی درخواست کی گئی ہے۔

تمام نمونے `./artifacts` کے تحت لکھے گئے ہیں (آپ پہلی دلیل کے طور پر ایک مختلف ڈائرکٹری پاس کرکے اسے اوور رائڈ کرسکتے ہیں)۔ پڑھنے کے قابل سیاق و سباق کے لئے فائلوں `_summary.json` کا معائنہ کرتا ہے۔

## 2. نتائج اور آؤٹ پٹ میٹرکس شامل کریں

```bash
./analyze.py --artifacts ./artifacts
```

پارسر تیار کرتا ہے:

- `capacity_simulation_report.json` - شامل کردہ میپنگز ، فیل اوور ڈیلٹا اور تنازعہ میٹا ڈیٹا۔
- `capacity_simulation.prom` - Prometheus (`sorafs_simulation_*`) ٹیکسٹ فائل میٹرکس نوڈ ایکسپورٹر ٹیکسٹ فائل کلیکٹر یا اسٹینڈ اسٹون کھرچنے والی نوکری کے لئے موزوں ہے۔

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

ٹیکسٹ فائل کلیکٹر کو `capacity_simulation.prom` پر اشارہ کریں (اگر آپ نوڈ ایکسپورٹر استعمال کرتے ہیں تو ، اسے `--collector.textfile.directory` کے ذریعے منظور شدہ ڈائریکٹری میں کاپی کریں)۔

## 3. Grafana ڈیش بورڈ درآمد کریں

1. Grafana میں ، `dashboards/grafana/sorafs_capacity_simulation.json` درآمد کریں۔
2. ڈیٹا سورس متغیر `Prometheus` کو اوپر کی تشکیل شدہ کھرچنا ہدف سے لنک کریں۔
3. پینل چیک کریں:
   - ** کوٹہ مختص (GIB) ** ہر سپلائر کے پرعزم/مختص بیلنس کو ظاہر کرتا ہے۔
   - ** فیل اوور ٹرگر ***فیل اوور ایکٹو*میں تبدیلیاں*جب کریش میٹرکس آتے ہیں۔
   - ** آؤٹ ٹائم کے دوران اپ ٹائم ڈراپ ** گراف فراہم کنندہ `alpha` کے لئے فیصد نقصان۔
   - ** درخواست کردہ سلیش فیصد ** مقابلہ شدہ حقیقت سے نکالے گئے تدارک کے تناسب کو ظاہر کرتا ہے۔

## 4. متوقع چیک- `sorafs_simulation_quota_total_gib{scope="assigned"}` `600` کے برابر ہے جبکہ کل پرعزم باقی ہے> = 600۔
- `sorafs_simulation_failover_triggered` `1` اور متبادل فراہم کرنے والے میٹرک کو نمایاں کرتا ہے `beta`۔
- `sorafs_simulation_slash_requested` سپلائر شناخت کنندہ `alpha` کے لئے `0.15` (15 ٪ SLASH) کی رپورٹ کرتا ہے۔

`cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` چلائیں اس بات کی تصدیق کے لئے کہ فکسچر ابھی بھی CLI اسکیما کے ذریعہ قبول کیے گئے ہیں۔