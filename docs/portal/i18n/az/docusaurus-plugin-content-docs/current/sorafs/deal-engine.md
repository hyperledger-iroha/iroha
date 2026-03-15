---
id: deal-engine
lang: az
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Deal Engine
sidebar_label: Deal Engine
description: Overview of the SF-8 deal engine, Torii integration, and telemetry surfaces.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

# SoraFS Deal Engine

SF-8 yol xəritəsi treki, təmin edən SoraFS müqavilə mühərrikini təqdim edir.
arasında saxlama və axtarış müqavilələrinin deterministik uçotu
müştərilər və provayderlər. Razılaşmalar Norito faydalı yükləri ilə təsvir edilmişdir
əqd şərtlərini, istiqrazı əhatə edən `crates/sorafs_manifest/src/deal.rs`-də müəyyən edilmişdir
kilidləmə, ehtimal mikro ödənişlər və hesablaşma qeydləri.

Daxili SoraFS işçisi (`sorafs_node::NodeHandle`) indi
Hər node prosesi üçün `DealEngine` nümunəsi. Mühərrik:

- `DealTermsV1` istifadə edərək sövdələşmələri təsdiqləyir və qeydiyyatdan keçirir;
- replikasiyadan istifadə barədə məlumat verildikdə XOR ilə ifadə olunan ödənişlər hesablanır;
- deterministik istifadə edərək ehtimal mikroödəmə pəncərələrini qiymətləndirir
  Blake3 əsaslı seçmə; və
- idarəetmə üçün uyğun olan mühasibat dəftəri snapshotları və hesablaşma yükləri istehsal edir
  nəşriyyat.

Vahid testləri doğrulama, mikroödəmə seçimi və hesablaşma axınlarını əhatə edir
operatorlar API-lərdən əminliklə istifadə edə bilərlər. İndi yaşayış məntəqələri yayılır
`DealSettlementV1` idarəetmə yükləri, birbaşa SF-12-yə naqillər
nəşr boru kəməri və `sorafs.node.deal_*` OpenTelemetry seriyasını yeniləyin
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) Torii idarə panelləri və SLO üçün
icra. Sonrakı maddələr auditorun təşəbbüsü ilə kəsmə avtomatlaşdırmasına və
ləğv semantikasının idarəetmə siyasəti ilə əlaqələndirilməsi.

İstifadə telemetriyası indi də `sorafs.node.micropayment_*` ölçülər dəstini qidalandırır:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano` və bilet sayğacları
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Bu cəmlər ehtimalı ifşa edir
Lotereya axını operatorların mikroödəmə uduşlarını və kreditin ötürülməsini əlaqələndirə bilsin
hesablaşma nəticələri ilə.

## Torii İnteqrasiya

Torii xüsusi son nöqtələri ifşa edir ki, provayderlər istifadə haqqında məlumat verə və
sifarişli naqillər olmadan işləmə müddəti:

- `POST /v2/sorafs/deal/usage` `DealUsageReport` telemetriyasını qəbul edir və qaytarır
  deterministik uçot nəticələri (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` cari pəncərəni axınla tamamlayır
  nəticədə `DealSettlementRecord` baza64 kodlu `DealSettlementV1` ilə birlikdə
  idarəetmə DAG nəşrinə hazırdır.
- Torii-in `/v2/events/sse` lenti indi `SorafsGatewayEvent::DealUsage`-i yayımlayır
  hər bir istifadə təqdimini ümumiləşdirən qeydlər (dövr, ölçülü GiB-saatlar, bilet
  sayğaclar, deterministik yüklər), `SorafsGatewayEvent::DealSettlement`
  kanonik hesablaşma kitabçası snapshot və əlavə daxil olan qeydlər
  Disk idarəetmə artefaktının BLAKE3 həzmi/ölçüsü/baza64 və
  PDP/PoTR hədləri olduqda `SorafsGatewayEvent::ProofHealth` xəbərdarlıq edir
  aşılmışdır (provayder, pəncərə, tətil/soyutma vəziyyəti, cərimə məbləği). İstehlakçılar bilər
  səsvermə olmadan yeni telemetriya, yaşayış məntəqələri və ya sağlamlığa dair xəbərdarlıqlara reaksiya vermək üçün provayder tərəfindən süzün.

Hər iki son nöqtə yeni vasitəsilə SoraFS kvota çərçivəsində iştirak edir.
`torii.sorafs.quota.deal_telemetry` pəncərəsi operatorlara tənzimləməyə imkan verir
yerləşdirmə başına icazə verilən təqdimetmə dərəcəsi.