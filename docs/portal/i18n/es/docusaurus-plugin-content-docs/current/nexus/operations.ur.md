---
lang: es
direction: ltr
source: docs/portal/docs/nexus/operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-nexus
título: Nexus آپریشنز رن بُک
descripción: Nexus آپریٹر ورک فلو کا فیلڈ کے لئے تیار خلاصہ، جو `docs/source/nexus_operations.md` کی عکاسی کرتا ہے۔
---

اس صفحے کو `docs/source/nexus_operations.md` کے تیز ریفرنس کے طور پر استعمال کریں۔ یہ آپریشنل چیک لسٹ، تبدیلی کے انتظام کے ganchos, اور ٹیلیمیٹری کوریج کی ضروریات کو سمیٹتا ہے جن پر Nexus آپریٹرز کو عمل کرنا ہوتا ہے۔

## لائف سائیکل چیک لسٹ

| مرحلہ | اقدامات | ثبوت |
|-------|--------|----------|
| پری فلائٹ | ریلیز ہیشز/سگنیچرز کی تصدیق کریں، `profile = "iroha3"` کنفرم کریں، اور کنفیگ ٹیمپلیٹس تیار کریں۔ | `scripts/select_release_profile.py` آؤٹ پٹ، checksum لاگ، دستخط شدہ مینفسٹ بَنڈل۔ |
| کیٹلاگ الائنمنٹ | `[nexus]` کیٹلاگ، روٹنگ پالیسی اور DA تھریش ہولڈز کو کونسل کے جاری کردہ مینفسٹ کے مطابق اپ ڈیٹ کریں، پھر `--trace-config` کیپچر کریں۔ | `irohad --sora --config ... --trace-config` آؤٹ پٹ جو incorporación ٹکٹ کے ساتھ محفوظ ہے۔ |
| اسموک اور کٹ اوور | `irohad --sora --config ... --trace-config` Pantalla CLI (`FindNetworkStatus`) Pantalla táctil کریں، اور ایڈمیشن کی درخواست دیں۔ | اسموک ٹیسٹ لاگ + Alertmanager کنفرمیشن۔ |
| اسٹیڈی اسٹیٹ | paneles/alertas مانیٹر کریں، گورننس کی cadencia کے مطابق کیز روٹیٹ کریں، اور جب مینفسٹ بدلے تو configs/runbooks ہم آہنگ کریں۔ | سہ ماہی ریویو منٹس، ڈیش بورڈ اسکرین شاٹس، روٹیشن ٹکٹ IDs۔ |

Incorporación (کلیدوں کی تبدیلی، روٹنگ ٹیمپلیٹس، ریلیز پروفائل کے مراحل) `docs/source/sora_nexus_operator_onboarding.md` میں موجود ہے۔

## تبدیلی کا انتظام1. **ریلیز اپ ڈیٹس** - `status.md`/`roadmap.md` میں اعلانات ٹریک کریں؛ ہر ریلیز PR کے ساتھ incorporación چیک لسٹ منسلک کریں۔
2. **Lane مینفسٹ تبدیلیاں** - Directorio espacial سے دستخط شدہ بَنڈلز کی تصدیق کریں اور انہیں `docs/source/project_tracker/nexus_config_deltas/` کے تحت محفوظ کریں۔
3. **کنفیگریشن ڈیلٹاز** - `config/config.toml` میں ہر تبدیلی کے لئے carril/espacio de datos کا حوالہ دینے والا ٹکٹ ضروری ہے۔ جب نوڈز شامل ہوں یا اپ گریڈ ہوں تو موثر کنفیگ کی ریڈیکٹڈ کاپی محفوظ کریں۔
4. **Taladros de retroceso** - سہ ماہی detener/restaurar/ahumar طریقہ کار کی مشق کریں؛ نتائج `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` میں لاگ کریں۔
5. **Aprobaciones de cumplimiento** - carriles privados/CBDC کو DA پالیسی یا ٹیلیمیٹری perillas de redacción بدلنے سے پہلے cumplimiento منظوری درکار ہے (دیکھیں `docs/source/cbdc_lane_playbook.md`).

## ٹیلیمیٹری اور SLO- Paneles de control: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, y SDK مخصوص ویوز (مثلاً `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` اور Torii/Norito transporte قواعد (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- دیکھنے کے لئے میٹرکس:
  - `nexus_lane_height{lane_id}` - تین ranuras تک پیش رفت صفر ہو تو الرٹ کریں۔
  - `nexus_da_backlog_chunks{lane_id}` - carril مخصوص تھریش ہولڈز سے اوپر الرٹ کریں (ڈیفالٹ 64 público / 8 privado).
  - `nexus_settlement_latency_seconds{lane_id}` - جب P99 900 ms (público) یا 1200 ms (privado) سے اوپر جائے تو الرٹ کریں۔
  - `torii_request_failures_total{scheme="norito_rpc"}` - اگر 5 منٹ کی ratio de error >2% ہو تو الرٹ کریں۔
  - `telemetry_redaction_override_total` - Septiembre 2 فوری؛ یقینی بنائیں کہ anula el cumplimiento de کے لئے ٹکٹس ہوں۔
- [Plan de remediación de telemetría Nexus](./nexus-telemetry-remediation) فارم آپریشنز ریویو نوٹس کے ساتھ منسلک کریں۔

## انسیڈنٹ میٹرکس| شدت | تعریف | ردعمل |
|----------|------------|----------|
| Septiembre 1 | aislamiento del espacio de datos کی خلاف ورزی، asentamiento کا 15 منٹ سے زیادہ رکنا، یا gobernanza y ووٹ میں خرابی۔ | Nexus Primario + Ingeniería de versión + Cumplimiento کو پیج کریں، ایڈمیشن فریز کریں، آرٹیفیکٹس جمع کریں، 30 منٹ، مینفسٹ implementación ناکام۔ | Nexus Primario + SRE کو پیج کریں، <=4 گھنٹے میں مٹیگیٹ کریں، 2 کاروباری دن کے اندر seguimientos فائل کریں۔ |
| Septiembre 3 | غیر رکاوٹی deriva (docs، alertas). | tracker میں لاگ کریں اور sprint کے اندر fix شیڈول کریں۔ |

انسیڈنٹ ٹکٹس میں متاثرہ ID de carril/espacio de datos, مینفسٹ ہیشز، ٹائم لائن، سپورٹنگ میٹرکس/لاگز، اور seguimiento ٹاسکس/مالکان درج ہونا ضروری ہیں۔

## ثبوت آرکائیو

- exportaciones de paquetes/manifiestos/telemetría کو `artifacts/nexus/<lane>/<date>/` کے تحت محفوظ کریں۔
- ہر ریلیز کے لئے configuraciones redactadas + `--trace-config` آؤٹ پٹ محفوظ رکھیں۔
- جب config یا مینفسٹ تبدیلیاں ہوں تو کونسل منٹس + دستخط شدہ فیصلے منسلک کریں۔
- Nexus میٹرکس کے لئے متعلقہ Prometheus ہفتہ وار snapshots 12 ماہ تک محفوظ کریں۔
- ediciones del runbook کو `docs/source/project_tracker/nexus_config_deltas/README.md` میں ریکارڈ کریں تاکہ آڈیٹرز جان سکیں ذمہ داریاں کب بدلیں۔

## متعلقہ مواد- Descripción general: [descripción general de Nexus] (./nexus-overview)
- Especificación: [Especificación Nexus] (./nexus-spec)
- Geometría del carril: [modelo de carril Nexus](./nexus-lane-model)
- Calzas de transición y enrutamiento: [Notas de transición Nexus](./nexus-transition-notes)
- Incorporación de operador: [Incorporación de operador Sora Nexus](./nexus-operator-onboarding)
- Remediación de telemetría: [Nexus plan de remediación de telemetría](./nexus-telemetry-remediation)