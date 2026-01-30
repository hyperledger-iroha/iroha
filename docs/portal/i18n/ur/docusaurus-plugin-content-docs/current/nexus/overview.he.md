---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/nexus/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c2b89adf1ec25af739af783e05cc88a633c06e5bdcf6ec03e80ac6ba8e6c2008
source_last_modified: "2025-11-14T04:43:20.540742+00:00"
translation_last_reviewed: 2026-01-30
---

Nexus (Iroha 3) Iroha 2 کو multi-lane اجرا، گورننس کے دائرہ کار میں ڈیٹا اسپیسز، اور ہر SDK میں مشترکہ ٹولنگ کے ساتھ توسیع دیتا ہے۔ یہ صفحہ مونو ریپو میں نئے خلاصے `docs/source/nexus_overview.md` کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین جلد سمجھ سکیں کہ معماری کے اجزا کیسے جڑتے ہیں۔

## ریلیز لائنز

- **Iroha 2** - کنسورشیم یا نجی نیٹ ورکس کے لئے خود میزبان ڈپلائمنٹس۔
- **Iroha 3 / Sora Nexus** - پبلک multi-lane نیٹ ورک جہاں آپریٹرز ڈیٹا اسپیسز (DS) رجسٹر کرتے ہیں اور مشترکہ گورننس، سیٹلمنٹ، اور آبزرویبیلٹی ٹولنگ وراثت میں لیتے ہیں۔
- دونوں لائنیں ایک ہی workspace (IVM + Kotodama toolchain) سے کمپائل ہوتی ہیں، اس لئے SDK فکسز، ABI اپڈیٹس اور Norito فکسچرز قابلِ نقل رہتے ہیں۔ آپریٹرز Nexus میں شامل ہونے کے لئے `iroha3-<version>-<os>.tar.zst` بَنڈل ڈاؤن لوڈ کرتے ہیں؛ فل اسکرین چیک لسٹ کے لئے `docs/source/sora_nexus_operator_onboarding.md` دیکھیں۔

## بنیادی اجزا

| جزو | خلاصہ | پورٹل لنکس |
|-----------|---------|--------------|
| ڈیٹا اسپیس (DS) | گورننس سے متعین اجرا/اسٹوریج ڈومین جو ایک یا زیادہ lanes رکھتا ہے، ویلیڈیٹر سیٹس، پرائیویسی کلاس، اور فیس + DA پالیسی کا اعلان کرتا ہے۔ | مینفسٹ اسکیمہ کے لئے [Nexus spec](./nexus-spec) دیکھیں۔ |
| Lane | اجرا کا ڈیٹرمنسٹک شَارڈ؛ ایسے commitments جاری کرتا ہے جنہیں عالمی NPoS رنگ ترتیب دیتا ہے۔ Lane کلاسز میں `default_public`, `public_custom`, `private_permissioned`, اور `hybrid_confidential` شامل ہیں۔ | [Lane model](./nexus-lane-model) جیومیٹری، اسٹوریج پری فکسز، اور ریٹینشن کو بیان کرتا ہے۔ |
| ٹرانزیشن پلان | Placeholder identifiers، routing phases، اور dual-profile packaging اس بات کو ٹریک کرتے ہیں کہ single-lane ڈپلائمنٹس کیسے Nexus میں ارتقا کرتے ہیں۔ | [Transition notes](./nexus-transition-notes) ہر مائیگریشن مرحلہ دستاویز کرتی ہیں۔ |
| Space Directory | رجسٹری کنٹریکٹ جو DS مینفسٹس + ورژنز اسٹور کرتا ہے۔ آپریٹرز شامل ہونے سے پہلے کیٹلاگ انٹریز کو اس ڈائریکٹری کے ساتھ ملاتے ہیں۔ | مینفسٹ ڈف ٹریکر `docs/source/project_tracker/nexus_config_deltas/` کے تحت موجود ہے۔ |
| Lane catalog | `[nexus]` کنفیگ سیکشن lane IDs کو aliases، routing پالیسیز اور DA thresholds سے میپ کرتا ہے۔ `irohad --sora --config … --trace-config` آڈٹ کے لئے resolved catalog پرنٹ کرتا ہے۔ | CLI walkthrough کے لئے `docs/source/sora_nexus_operator_onboarding.md` استعمال کریں۔ |
| Settlement router | XOR ٹرانسفر آرکسٹریٹر جو نجی CBDC lanes کو پبلک لیکویڈیٹی lanes سے جوڑتا ہے۔ | `docs/source/cbdc_lane_playbook.md` پالیسی knobs اور ٹیلیمیٹری gates کی وضاحت کرتا ہے۔ |
| Telemetry/SLOs | `dashboards/grafana/nexus_*.json` کے تحت ڈیش بورڈز + الرٹس lane height، DA backlog، settlement latency اور governance queue depth کو کیپچر کرتے ہیں۔ | [Telemetry remediation plan](./nexus-telemetry-remediation) ڈیش بورڈز، الرٹس اور آڈٹ ثبوت کی وضاحت کرتا ہے۔ |

## رول آؤٹ اسنیپ شاٹ

| مرحلہ | فوکس | اخراجی معیار |
|-------|-------|---------------|
| N0 - بند بیٹا | کونسل-مینجڈ registrar (`.sora`)، مینول آپریٹر آن بورڈنگ، جامد lane catalog۔ | دستخط شدہ DS مینفسٹس + گورننس ہینڈ آفز کی مشق۔ |
| N1 - عوامی لانچ | `.nexus` سفکسز، نیلامیاں، self-service registrar، XOR settlement wiring شامل کرتا ہے۔ | resolver/gateway sync ٹیسٹس، بلنگ reconciliation ڈیش بورڈز، dispute tabletop drills۔ |
| N2 - توسیع | `.dao`، reseller APIs، analytics، dispute portal، steward scorecards متعارف کراتا ہے۔ | compliance artefacts ورژن شدہ، policy-jury toolkit آن لائن، treasury transparency reports۔ |
| NX-12/13/14 gate | compliance engine، telemetry dashboards اور documentation کو پارٹنر پائلٹس سے پہلے اکٹھا ریلیز ہونا ضروری ہے۔ | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) شائع، dashboards wired، policy engine merged۔ |

## آپریٹر کی ذمہ داریاں

1. **کنفیگ حفظانِ صحت** - `config/config.toml` کو شائع شدہ lane اور dataspace کیٹلاگ کے ساتھ ہم آہنگ رکھیں؛ ہر ریلیز ٹکٹ کے ساتھ `--trace-config` آؤٹ پٹ محفوظ کریں۔
2. **مینفسٹ ٹریکنگ** - شامل ہونے یا نوڈ اپ گریڈ سے پہلے کیٹلاگ انٹریز کو تازہ ترین Space Directory بَنڈل سے ہم آہنگ کریں۔
3. **ٹیلیمیٹری کوریج** - `nexus_lanes.json`، `nexus_settlement.json` اور متعلقہ SDK ڈیش بورڈز کو expose کریں؛ الرٹس کو PagerDuty سے جوڑیں اور ٹیلیمیٹری remediation پلان کے مطابق سہ ماہی جائزے چلائیں۔
4. **حادثہ رپورٹنگ** - [Nexus operations](./nexus-operations) میں سیوریٹی میٹرکس کی پیروی کریں اور پانچ کاروباری دنوں میں RCA فائل کریں۔
5. **گورننس تیاری** - اپنی lanes پر اثر انداز ہونے والی Nexus کونسل ووٹس میں حصہ لیں اور سہ ماہی rollback ہدایات کی مشق کریں (ٹریکنگ `docs/source/project_tracker/nexus_config_deltas/` کے ذریعے ہوتی ہے)۔

## مزید دیکھیں

- کینونیکل جائزہ: `docs/source/nexus_overview.md`
- تفصیلی اسپیسفیکیشن: [./nexus-spec](./nexus-spec)
- Lane جیومیٹری: [./nexus-lane-model](./nexus-lane-model)
- ٹرانزیشن پلان: [./nexus-transition-notes](./nexus-transition-notes)
- ٹیلیمیٹری remediation پلان: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- آپریشنز runbook: [./nexus-operations](./nexus-operations)
- آپریٹر onboarding گائیڈ: `docs/source/sora_nexus_operator_onboarding.md`
