---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقرير اختبار mergulhe o líquido em SF-2c

Data: 21/03/2026

## النطاق

Deixe a água de molho SoraFS e deixe de molho SF-2c.

- **اختبار mergulhe متعدد المزوّدين لمدة 30 dias:** يتم عبر
  `capacity_fee_ledger_30_day_soak_deterministic` é
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  O arnês de segurança é de 30 anos de comprimento, e o arnês é de 30 anos de idade.
  ledger é um livro-razão que pode ser usado para armazenar dados. Resumo do resumo de Blake3
  (`capacity_soak_digest=...`) O CI deve ser removido do computador e do computador.
- **عقوبات نقص التسليم:** تُفرض بواسطة
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (نفس الملف). يؤكد الاختبار أن عتبات greves e cooldowns e slashing للضمان وعدّادات
  razão.

## التنفيذ

Deixe de molho:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

تكتمل الاختبارات في أقل من ثانية على حاسوب محمول قياسي ولا تتطلب fixtures خارجية.

## الرصد

يعرض Torii آن لقطات رصيد المزوّدين جنبًا إلى جنب مع livros de taxas حتى تتمكن لوحات المتابعة
من الضبط على الأرصدة المنخفضة وpenalty strikes:

- REST: `GET /v2/sorafs/capacity/state`
  Deixe o livro-razão embebido. راجع
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importação Grafana: `dashboards/grafana/sorafs_capacity_penalties.json`
  عدادات greves المصدّرة وإجمالي العقوبات والضمان المربوط حتى يتمكن فريق
  As linhas de base das linhas de base absorvem مع البيئات الحية.

## المتابعة

- Deixe a água de molho no CI durante a imersão (camada de fumaça).
- توسيع لوحة Grafana بأهداف scrape من Torii بمجرد تفعيل صادرات telemetria الإنتاجية.