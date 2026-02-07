---
lang: es
direction: ltr
source: docs/portal/docs/nexus/overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: descripción general del nexo
título: Sora Nexus کا جائزہ
descripción: Iroha 3 (Sora Nexus) کی معماری کا اعلی سطحی خلاصہ اور مونو ریپو کی کینونیکل دستاویزات کے حوالے۔
---

Nexus (Iroha 3) Iroha 2 carriles múltiples اجرا، گورننس کے دائرہ کار میں ڈیٹا اسپیسز، اور ہر SDK میں مشترکہ ٹولنگ کے ساتھ توسیع دیتا ہے۔ یہ صفحہ مونو ریپو میں نئے خلاصے `docs/source/nexus_overview.md` کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین جلد سمجھ سکیں کہ معماری کے اجزا کیسے جڑتے ہیں۔

## ریلیز لائنز

- **Iroha 2** - کنسورشیم یا نجی نیٹ ورکس کے لئے خود میزبان ڈپلائمنٹس۔
- **Iroha 3 / Sora Nexus** - پبلک multicarril نیٹ ورک جہاں آپریٹرز ڈیٹا اسپیسز (DS) رجسٹر کرتے ہیں اور مشترکہ گورننس، سیٹلمنٹ، اور آبزرویبیلٹی ٹولنگ وراثت میں لیتے ہیں۔
- دونوں لائنیں ایک ہی espacio de trabajo (cadena de herramientas IVM + Kotodama) سے کمپائل ہوتی ہیں، اس لئے SDK فکسز، ABI اپڈیٹس اور Norito فکسچرز قابلِ نقل رہتے ہیں۔ آپریٹرز Nexus میں شامل ہونے کے لئے `iroha3-<version>-<os>.tar.zst` بَنڈل ڈاؤن لوڈ کرتے ہیں؛ فل اسکرین چیک لسٹ کے لئے `docs/source/sora_nexus_operator_onboarding.md` دیکھیں۔

## بنیادی اجزا| جزو | خلاصہ | پورٹل لنکس |
|-----------|---------|--------------|
| ڈیٹا اسپیس (DS) | گورننس سے متعین اجرا/اسٹوریج ڈومین جو ایک یا زیادہ carriles رکھتا ہے، ویلیڈیٹر سیٹس، پرائیویسی کلاس، اور فیس + DA پالیسی کا اعلان کرتا ہے۔ | مینفسٹ اسکیمہ کے لئے [Especificación Nexus](./nexus-spec) دیکھیں۔ |
| Carril | اجرا کا ڈیٹرمنسٹک شَارڈ؛ ایسے compromisos جاری کرتا ہے جنہیں عالمی NPoS رنگ ترتیب دیتا ہے۔ Lane کلاسز میں `default_public`, `public_custom`, `private_permissioned`, اور `hybrid_confidential` شامل ہیں۔ | [Modelo de carril](./nexus-lane-model) |
| ٹرانزیشن پلان | Identificadores de marcador de posición, fases de enrutamiento, اور empaquetado de doble perfil اس بات کو ٹریک کرتے ہیں کہ de un solo carril ڈپلائمنٹس کیسے Nexus میں ارتقا کرتے ہیں۔ | [Notas de transición](./nexus-transition-notes) ہر مائیگریشن مرحلہ دستاویز کرتی ہیں۔ |
| Directorio espacial | رجسٹری کنٹریکٹ جو DS مینفسٹس + ورژنز اسٹور کرتا ہے۔ آپریٹرز شامل ہونے سے پہلے کیٹلاگ انٹریز کو اس ڈائریکٹری کے ساتھ ملاتے ہیں۔ | مینفسٹ ڈف ٹریکر `docs/source/project_tracker/nexus_config_deltas/` کے تحت موجود ہے۔ |
| Catálogo de carriles | `[nexus]` کنفیگ سیکشن ID de carril کو alias، enrutamiento پالیسیز اور umbrales DA سے میپ کرتا ہے۔ `irohad --sora --config … --trace-config` آڈٹ کے لئے catálogo resuelto پرنٹ کرتا ہے۔ | Tutorial de CLI کے لئے `docs/source/sora_nexus_operator_onboarding.md` استعمال کریں۔ || Enrutador de liquidación | XOR ٹرانسفر آرکسٹریٹر جو نجی CBDC lanes کو پبلک لیکویڈیٹی lanes سے جوڑتا ہے۔ | `docs/source/cbdc_lane_playbook.md` پالیسی perillas اور ٹیلیمیٹری puertas کی وضاحت کرتا ہے۔ |
| Telemetría/SLO | `dashboards/grafana/nexus_*.json` کے تحت ڈیش بورڈز + الرٹس altura del carril, acumulación de DA, latencia de liquidación اور profundidad de la cola de gobernanza کو کیپچر کرتے ہیں۔ | [Plan de remediación de telemetría](./nexus-telemetry-remediation) ڈیش بورڈز، الرٹس اور آڈٹ ثبوت کی وضاحت کرتا ہے۔ |

## رول آؤٹ اسنیپ شاٹ

| مرحلہ | فوکس | اخراجی معیار |
|-------|-------|-----------------------|
| N0 - بند بیٹا | Registrador de کونسل-مینجڈ (`.sora`), مینول آپریٹر آن بورڈنگ، جامد lane catalog۔ | دستخط شدہ DS مینفسٹس + گورننس ہینڈ آفز کی مشق۔ |
| N1 - عوامی لانچ | `.nexus` سفکسز، نیلامیاں، registrador de autoservicio, cableado de liquidación XOR شامل کرتا ہے۔ | sincronización de resolución/puerta de enlace ٹیسٹس، بلنگ reconciliación ڈیش بورڈز، disputas simulacros de mesa۔ |
| N2 - توسیع | `.dao`, API de revendedor, análisis, portal de disputas, cuadros de mando del administrador متعارف کراتا ہے۔ | artefactos de cumplimiento ورژن شدہ، kit de herramientas del jurado de políticas آن لائن، informes de transparencia de tesorería۔ |
| Puerta NX-12/13/14 | motor de cumplimiento, paneles de telemetría y documentación | [Descripción general de Nexus](./nexus-overview) + [Operaciones de Nexus](./nexus-operations) Paneles de control cableados, motor de políticas fusionado ۔ |

## آپریٹر کی ذمہ داریاں1. **کنفیگ حفظانِ صحت** - `config/config.toml` کو شائع شدہ lane اور dataspace کیٹلاگ کے ساتھ ہم آہنگ رکھیں؛ ہر ریلیز ٹکٹ کے ساتھ `--trace-config` آؤٹ پٹ محفوظ کریں۔
2. **مینفسٹ ٹریکنگ** - شامل ہونے یا نوڈ اپ گریڈ سے پہلے کیٹلاگ انٹریز کو تازہ ترین Directorio espacial بَنڈل سے ہم آہنگ کریں۔
3. **ٹیلیمیٹری کوریج** - `nexus_lanes.json`، `nexus_settlement.json` اور متعلقہ SDK ڈیش بورڈز کو exponen کریں؛ الرٹس کو PagerDuty سے جوڑیں اور ٹیلیمیٹری remediación پلان کے مطابق سہ ماہی جائزے چلائیں۔
4. **حادثہ رپورٹنگ** - [Operaciones Nexus](./nexus-operations) میں سیوریٹی میٹرکس کی پیروی کریں اور پانچ کاروباری دنوں میں RCA فائل کریں۔
5. **گورننس تیاری** - اپنی carriles پر اثر انداز ہونے والی Nexus کونسل ووٹس میں حصہ لیں اور سہ Reversión automática ہدایات کی مشق کریں (ٹریکنگ `docs/source/project_tracker/nexus_config_deltas/` کے ذریعے ہوتی ہے)۔

## مزید دیکھیں

- Número de modelo: `docs/source/nexus_overview.md`
- Nombre de usuario: [./nexus-spec](./nexus-spec)
- Carril جیومیٹری: [./nexus-lane-model](./nexus-lane-model)
- ٹرانزیشن پلان: [./nexus-transition-notes](./nexus-transition-notes)
- Remediación completa: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook actual: [./nexus-operatives](./nexus-operations)
- آپریٹر incorporación گائیڈ: `docs/source/sora_nexus_operator_onboarding.md`