---
id: payment-settlement-plan
lang: az
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Kanonik mənbə: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

Yol xəritəsi tapşırığı **SN-5 — Ödəniş və Hesablaşma Xidməti** deterministi təqdim edir
Sora Name Service üçün ödəniş təbəqəsi. Hər qeydiyyat, yenilənmə və ya pulun geri qaytarılması
strukturlaşdırılmış Norito faydalı yük yaymalıdır ki, xəzinədarlıq, stüardlar və idarəetmə
elektron cədvəllər olmadan maliyyə axınlarını təkrarlayın. Bu səhifə spesifikasiyası distillə edir
portal auditoriyası üçün.

## Gəlir modeli

- Əsas rüsum (`gross_fee`) qeydiyyatçının qiymət matrisindən əldə edilir.  
- Xəzinədarlıq `gross_fee × 0.70` alır, stüardlar qalan minus alırlar
  tavsiye bonusları (10% ilə məhdudlaşır).  
- Könüllü olaraq saxlanmalar idarəçiliyə mübahisələr zamanı stüard ödənişlərini dayandırmağa imkan verir.  
- Məskunlaşma paketləri `ledger_projection` blokunu betonla ifşa edir
  `Transfer` ISI-lər beləliklə avtomatlaşdırma XOR hərəkətlərini birbaşa Torii-ə göndərə bilər.

## Xidmətlər və avtomatlaşdırma

| Komponent | Məqsəd | Sübut |
|-----------|---------|----------|
| `sns_settlementd` | Siyasəti tətbiq edir, paketləri işarələyir, `/v1/sns/settlements` səthlərini göstərir. | JSON paketi + hash. |
| Hesablaşma növbəsi və yazıçı | Idempotent növbə + `iroha_cli app sns settlement ledger` tərəfindən idarə olunan kitab təqdim edən. | Paket hash ↔ tx hash manifest. |
| Barışıq işi | `docs/source/sns/reports/` altında gündəlik fərq + aylıq hesabat. | Markdown + JSON həzmi. |
| Geri ödəniş masası | `/settlements/{id}/refund` vasitəsilə idarəetmə tərəfindən təsdiqlənmiş geri qaytarmalar. | `RefundRecordV1` + bilet. |

CI köməkçiləri bu axınları əks etdirir:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Müşahidə və hesabat

- İdarə panelləri: xəzinədarlıq üçün `dashboards/grafana/sns_payment_settlement.json`
  stüard yekunları, yönləndirmə ödənişləri, növbə dərinliyi və geri ödəmə gecikməsi.
- Xəbərdarlıqlar: `dashboards/alerts/sns_payment_settlement_rules.yml` monitorları gözləyir
  yaş, uzlaşma uğursuzluqları və kitab sürüşməsi.
- Hesabatlar: gündəlik həzmlər (`settlement_YYYYMMDD.{json,md}`) aylıq çevrilir
  həm Git, həm də internetə yüklənən hesabatlar (`settlement_YYYYMM.md`).
  idarəetmə obyekti mağazası (`s3://sora-governance/sns/settlements/<period>/`).
- İdarəetmə paketləri idarə panellərini, CLI qeydlərini və şuradan əvvəl təsdiqləri birləşdirir
  imzalanma.

## Yayımlama siyahısı

1. Prototip sitat + mühasibat kitabçası köməkçiləri və quruluş paketini ələ keçirin.
2. `sns_settlementd` növbə + yazıcı, naqil panelləri və məşqlə işə salın
   xəbərdarlıq testləri (`promtool test rules ...`).
3. Geri qaytarma köməkçisi və aylıq hesabat şablonu təqdim edin; güzgü artefaktları
   `docs/portal/docs/sns/reports/`.
4. Partnyor məşqini həyata keçirin (bütün ay hesablaşmalar) və ələ keçirin
   SN-5-i tamamlanmış kimi qeyd edən idarəetmə səsi.

Dəqiq sxem tərifləri üçün mənbə sənədinə baxın, açın
suallar və gələcək düzəlişlər.