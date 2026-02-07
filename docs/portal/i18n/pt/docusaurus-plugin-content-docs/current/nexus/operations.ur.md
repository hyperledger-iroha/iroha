---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nexo
título: Nexus آپریشنز رن بُک
descrição: Nexus ہے۔
---

O código `docs/source/nexus_operations.md` é um cartão de crédito que pode ser usado para obter mais informações یہ آپریشنل چیک لسٹ, تبدیلی کے انتظام کے ganchos, اور ٹیلیمیٹری کوریج کی ضروریات کو Você pode usar o Nexus para obter mais informações

## لائف سائیکل چیک لسٹ

| مرحلہ | Produtos | ثبوت |
|-------|--------|----------|
| پری فلائٹ | ریلیز ہیشز/سگنیچرز کی تصدیق کریں, `profile = "iroha3"` کنفرم کریں, اور کنفیگ ٹیمپلیٹس تیار کریں۔ | `scripts/select_release_profile.py` é uma soma de verificação de soma de verificação que pode ser obtida por meio de uma soma de verificação |
| کیٹلاگ الائنمنٹ | `[nexus]` کیٹلاگ, روٹنگ پالیسی اور DA تھریش ہولڈز کو کونسل کے جاری کردہ مینفسٹ کے مطابق اپ ڈیٹ کریں, پھر `--trace-config` کیپچر کریں۔ | `irohad --sora --config ... --trace-config` آؤٹ پٹ جو onboarding ٹکٹ کے ساتھ محفوظ ہے۔ |
| اسموک اور کٹ اوور | `irohad --sora --config ... --trace-config` چلائیں, CLI اسموک (`FindNetworkStatus`) چلائیں, ٹیلیمیٹری ایکسپورٹس کی توثیق کریں, اور ایڈمیشن کی درخواست دیں۔ | اسموک ٹیسٹ لاگ + Alertmanager کنفرمیشن۔ |
| اسٹیڈی اسٹیٹ | painéis/alertas configs/runbooks ہم آہنگ کریں۔ | سہ ماہی ریویو منٹس, ڈیش بورڈ اسکرین شاٹس, روٹیشن ٹکٹ IDs۔ |

Onboarding (کلیدوں کی تبدیلی, روٹنگ ٹیمپلیٹس, ریلیز پروفائل کے مراحل) `docs/source/sora_nexus_operator_onboarding.md` میں موجود ہے۔

## تبدیلی کا انتظام

1. **ریلیز اپ ڈیٹس** - `status.md`/`roadmap.md` میں اعلانات ٹریک کریں؛ ہر ریلیز PR کے ساتھ onboarding چیک لسٹ منسلک کریں۔
2. **Lane مینفسٹ تبدیلیاں** - Diretório de espaço `docs/source/project_tracker/nexus_config_deltas/` کے تحت محفوظ کریں۔
3. **کنفیگریشن ڈیلٹاز** - `config/config.toml` میں ہر تبدیلی کے لئے pista/espaço de dados کا حوالہ دینے والا ٹکٹ ضروری ہے۔ جب نوڈز شامل ہوں یا اپ گریڈ ہوں تو موثر کنفیگ کی ریڈیکٹڈ کاپی محفوظ کریں ۔
4. **Exercícios de reversão** - سہ ماہی parar/restaurar/fumar طریقہ کار کی مشق کریں؛ نتائج `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` میں لاگ کریں۔
5. **Aprovações de conformidade** - pistas privadas/CBDC کو DA پالیسی یا ٹیلیمیٹری botões de redação بدلنے سے پہلے conformidade منظوری درکار ہے (دیکھیں `docs/source/cbdc_lane_playbook.md`).

## ٹیلیمیٹری اور SLOs

- Painéis: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, e SDK com suporte e یوز (compatível com `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` e Torii/Norito transporte transporte (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- دیکھنے کے لئے میٹرکس:
  - `nexus_lane_height{lane_id}` - تین slots تک پیش رفت صفر ہو تو الرٹ کریں۔
  - `nexus_da_backlog_chunks{lane_id}` - lane مخصوص تھریش ہولڈز سے اوپر الرٹ کریں (ڈیفالٹ 64 públicas / 8 privadas).
  - `nexus_settlement_latency_seconds{lane_id}` - جب P99 900 ms (público) ou 1200 ms (privado) سے اوپر جائے تو الرٹ کریں۔
  - `torii_request_failures_total{scheme="norito_rpc"}` - اگر 5 منٹ کی taxa de erro >2% ہو تو الرٹ کریں۔
  - `telemetry_redaction_override_total` - 2 de setembro de 2016 یقینی بنائیں کہ substitui کے لئے conformidade ٹکٹس ہوں۔
- [Plano de remediação de telemetria Nexus](./nexus-telemetry-remediation) ہوا فارم آپریشنز ریویو نوٹس کے ساتھ منسلک کریں۔

## انسیڈنٹ میٹرکس

| شدت | تعریف | ردعمل |
|----------|------------|----------|
| 1º de setembro | isolamento de espaço de dados کی خلاف ورزی، liquidação کا 15 منٹ سے زیادہ رکنا، یا governança ووٹ میں خرابی۔ | Nexus Primário + Engenharia de Liberação + Conformidade میں کمیونیکیشن جاری کریں, RCA <=5 کاروباری دن۔ |
| 2 de setembro | SLA de backlog de pista کی خلاف ورزی, ٹیلیمیٹری ponto cego >30 meses de implementação | Nexus Primário + SRE کو پیج کریں، <=4 گھنٹے میں مٹیگیٹ کریں، 2 کاروباری دن کے اندر acompanhamentos فائل کریں۔ |
| 3 de setembro | غیر رکاوٹی drift (docs, alertas). | rastreador میں لاگ کریں اور sprint کے اندر fix شیڈول کریں۔ |

انسیڈنٹ ٹکٹس میں متاثرہ IDs de faixa/espaço de dados, مینفسٹ ہیشز, ٹائم لائن, سپورٹنگ میٹرکس/لاگز, Acompanhamento de acompanhamento ٹاسکس/مالکان درج ہونا ضروری ہیں۔

## ثبوت آرکائیو

- pacotes/manifestos/exportações de telemetria کو `artifacts/nexus/<lane>/<date>/` کے تحت محفوظ کریں۔
- ہر ریلیز کے لئے configurações redigidas + `--trace-config` آؤٹ پٹ محفوظ رکھیں۔
- جب config یا مینفسٹ تبدیلیاں ہوں تو کونسل منٹس + دستخط شدہ فیصلے منسلک کریں۔
- Nexus میٹرکس کے لئے متعلقہ Prometheus ہفتہ وار snapshots 12 ماہ تک محفوظ کریں۔
- edições de runbook کو `docs/source/project_tracker/nexus_config_deltas/README.md` میں ریکارڈ کریں تاکہ آڈیٹرز جان سکیں ذمہ داریاں کب بدلیں۔

## متعلقہ مواد- Visão geral: [Visão geral Nexus] (./nexus-overview)
Especificação: [especificação Nexus] (./nexus-spec)
- Geometria da pista: [modelo de pista Nexus] (./nexus-lane-model)
- Calços de transição e roteamento: [notas de transição Nexus] (./nexus-transition-notes)
- Integração do operador: [integração do operador Sora Nexus](./nexus-operator-onboarding)
- Correção de telemetria: [Plano de remediação de telemetria Nexus](./nexus-telemetry-remediation)