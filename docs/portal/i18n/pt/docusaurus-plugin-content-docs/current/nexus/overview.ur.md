---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visão geral do nexo
título: Sora Nexus کا جائزہ
description: Iroha 3 (Sora Nexus) کی معماری کا اعلی سطحی خلاصہ اور مونو ریپو کی کینونیکل دستاویزات کے حوالے۔
---

Nexus (Iroha 3) Iroha 2 کو multi-lane اجرا، گورننس کے دائرہ کار میں ڈیٹا اسپیسز, O SDK do SDK é um recurso que você pode usar para obter mais informações یہ صفحہ مونو ریپو میں نئے خلاصے `docs/source/nexus_overview.md` کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین جلد سمجھ سکیں کہ معماری کے اجزا کیسے جڑتے ہیں۔

## ریلیز لائنز

- **Iroha 2** - کنسورشیم یا نجی نیٹ ورکس کے لئے خود میزبان ڈپلائمنٹس۔
- **Iroha 3 / Sora Nexus** - Multi-lane نیٹ ورک جہاں آپریٹرز ڈیٹا اسپیسز (DS) رجسٹر کرتے ہیں اور مشترکہ گورننس, سیٹلمنٹ, اور آبزرویبیلٹی ٹولنگ وراثت میں لیتے ہیں۔
- دونوں لائنیں ایک ہی espaço de trabalho (IVM + Kotodama toolchain) سے کمپائل ہوتی ہیں, اس لئے SDK فکسز, ABI اپڈیٹس اور Norito فکسچرز قابلِ نقل رہتے ہیں۔ آپریٹرز Nexus میں شامل ہونے کے لئے `iroha3-<version>-<os>.tar.zst` بَنڈل ڈاؤن لوڈ کرتے ہیں؛ فل اسکرین چیک لسٹ کے لئے `docs/source/sora_nexus_operator_onboarding.md` دیکھیں۔

## بنیادی اجزا

| sim | خلاصہ | پورٹل لنکس |
|-----------|---------|-------------|
| ڈیٹا اسپیس (DS) | گورننس سے متعین اجرا/اسٹوریج ڈومین جو ایک یا زیادہ pistas رکھتا ہے, ویلیڈیٹر سیٹس، پرائیویسی کلاس، اور فیس + DA پالیسی کا اعلان کرتا ہے۔ | مینفسٹ اسکیمہ کے لئے [Nexus spec](./nexus-spec) دیکھیں۔ |
| Pista | اجرا کا ڈیٹرمنسٹک شَارڈ؛ ایسے compromissos جاری کرتا ہے جنہیں عالمی NPoS رنگ ترتیب دیتا ہے۔ Lane کلاسز میں `default_public`, `public_custom`, `private_permissioned`, اور `hybrid_confidential` شامل ہیں۔ | [Modelo de pista](./nexus-lane-model) |
| ٹرانزیشن پلان | Identificadores de espaço reservado, fases de roteamento, e embalagens de perfil duplo. کرتے ہیں۔ | [Notas de transição](./nexus-transition-notes) ہر مائیگریشن مرحلہ دستاویز کرتی ہیں۔ |
| Diretório Espacial | رجسٹری کنٹریکٹ جو DS مینفسٹس + ورژنز اسٹور کرتا ہے۔ آپریٹرز شامل ہونے سے پہلے کیٹلاگ انٹریز کو اس ڈائریکٹری کے ساتھ ملاتے ہیں۔ | مینفسٹ ڈف ٹریکر `docs/source/project_tracker/nexus_config_deltas/` کے تحت موجود ہے۔ |
| Catálogo de pistas | `[nexus]` contém IDs de pista e aliases, roteamento, limites de DA e limites de DA. `irohad --sora --config … --trace-config` آڈٹ کے لئے catálogo resolvido پرنٹ کرتا ہے۔ | Passo a passo da CLI para `docs/source/sora_nexus_operator_onboarding.md` |
| Roteador de liquidação | XOR ٹرانسفر آرکسٹریٹر جو نجی CBDC lanes کو پبلک لیکویڈیٹی lanes سے جوڑتا ہے۔ | `docs/source/cbdc_lane_playbook.md` پالیسی botões e ٹیلیمیٹری portões کی وضاحت کرتا ہے۔ |
| Telemetria/SLOs | `dashboards/grafana/nexus_*.json` کے تحت ڈیش بورڈز + الرٹس altura da pista, backlog DA, latência de liquidação e profundidade da fila de governança کو کیپچر کرتے ہیں۔ | [Plano de remediação de telemetria](./nexus-telemetry-remediation) |

## رول آؤٹ اسنیپ شاٹ

| مرحلہ | فوکس | اخراجی معیار |
|-------|-------|---------------|
| N0 - بند بیٹا | Registrador کونسل-مینجڈ (`.sora`), مینول آپریٹر آن بورڈنگ, جامد lane catalog۔ | دستخط شدہ DS مینفسٹس + گورننس ہینڈ آفز کی مشق۔ |
| N1 - عوامی لانچ | `.nexus` سفکسز, نیلامیاں, registrador de autoatendimento, fiação de liquidação XOR شامل کرتا ہے۔ | sincronização de resolvedor/gateway ٹیسٹس, بلنگ reconciliação ڈیش بورڈز, exercícios de mesa de disputa۔ |
| N2 - توسیع | `.dao`, APIs de revendedor, análises, portal de disputas, scorecards de administrador متعارف کراتا ہے۔ | artefatos de conformidade ورژن شدہ, kit de ferramentas do júri de políticas آن لائن, relatórios de transparência do tesouro۔ |
| Portão NX-12/13/14 | mecanismo de conformidade, painéis de telemetria e documentação | [Visão geral Nexus](./nexus-overview) + [Operações Nexus](./nexus-operations) شائع, painéis com fio, mecanismo de política mesclado۔ |

## آپریٹر کی ذمہ داریاں

1. **کنفیگ حفظانِ صحت** - `config/config.toml` کو شائع شدہ lane اور dataspace کیٹلاگ کے ساتھ ہم آہنگ رکھیں؛ ہر ریلیز ٹکٹ کے ساتھ `--trace-config` آؤٹ پٹ محفوظ کریں۔
2. **مینفسٹ ٹریکنگ** - شامل ہونے یا نوڈ اپ گریڈ سے پہلے کیٹلاگ انٹریز کو تازہ ترین Diretório de espaço بَنڈل سے ہم آہنگ کریں۔
3. **ٹیلیمیٹری کوریج** - `nexus_lanes.json`, `nexus_settlement.json` اور متعلقہ SDK ڈیش بورڈز کو expor کریں؛ الرٹس کو PagerDuty سے جوڑیں اور ٹیلیمیٹری remediação پلان کے مطابق سہ ماہی جائزے چلائیں۔
4. **حادثہ رپورٹنگ** - [operações Nexus](./nexus-operations) میں سیوریٹی میٹرکس کی پیروی کریں اور Um cartão RCA de cartão RCA
5. **گورننس تیاری** - اپنی lanes پر اثر انداز ہونے والی Nexus کونسل ووٹس میں حصہ لیں اور سہ ماہی rollback ہدایات کی مشق کریں (ٹریکنگ `docs/source/project_tracker/nexus_config_deltas/` کے ذریعے ہوتی ہے)۔

## مزید دیکھیں- Número de identificação: `docs/source/nexus_overview.md`
- تفصیلی اسپیسفیکیشن: [./nexus-spec](./nexus-spec)
- Lane جیومیٹری: [./nexus-lane-model](./nexus-lane-model)
- ٹرانزیشن پلان: [./nexus-transition-notes](./nexus-transition-notes)
- remediação de ٹیلیمیٹری: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook آپریشنز: [./nexus-operações](./nexus-operations)
- Cartão de integração: `docs/source/sora_nexus_operator_onboarding.md`