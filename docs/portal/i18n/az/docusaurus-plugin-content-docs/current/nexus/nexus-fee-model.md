---
id: nexus-fee-model
lang: az
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus fee model updates
description: Mirror of `docs/source/nexus_fee_model.md`, documenting the lane settlement receipts and reconciliation surfaces.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/nexus_fee_model.md`-i əks etdirir. Yapon, İvrit, İspan, Portuqal, Fransız, Rus, Ərəb və Urdu dillərinə tərcümələr köçərkən hər iki nüsxəni düzülmüş saxlayın.
:::

# Nexus Ödəniş Modeli Yeniləmələri

Vahid hesablaşma marşrutlaşdırıcısı indi hər zolaqlı deterministik qəbzləri ələ keçirir
operatorlar qaz debetlərini Nexus ödəniş modelinə uyğunlaşdıra bilərlər.

- Tam marşrutlaşdırıcının arxitekturası, bufer siyasəti, telemetriya matrisi və buraxılması üçün
  ardıcıllıq bax `docs/settlement-router.md`. Bu təlimat necə olduğunu izah edir
  Burada sənədləşdirilmiş parametrlər NX-3 yol xəritəsinin çatdırılması və necə SRE-lərlə əlaqələndirilir
  istehsalda marşrutlaşdırıcıya nəzarət etməlidir.
- Qaz aktivinin konfiqurasiyası (`pipeline.gas.units_per_gas`) a daxildir
  `twap_local_per_xor` decimal, `liquidity_profile` (`tier1`, `tier2`,
  və ya `tier3`) və `volatility_class` (`stable`, `elevated`, `dislocated`).
  Bu bayraqlar məskunlaşma marşrutlaşdırıcısını qidalandırır ki, nəticədə XOR
  sitat zolağın kanonik TWAP və saç düzümü səviyyəsinə uyğun gəlir.
- Qaz ödəyən hər bir əməliyyat `LaneSettlementReceipt` qeyd edir.  Hər biri
  qəbz zəng edənin təqdim etdiyi mənbə identifikatorunu, yerli mikro məbləği,
  XOR'un dərhal səbəbi, XOR'un gözlənilən saç kəsimindən sonra reallaşdı
  dispersiya (`xor_variance_micro`) və millisaniyələrdə blok vaxt damğası.
- Blok icrası hər zolaq/məlumat məkanı üzrə daxilolmaları toplayır və onları dərc edir
  `/v2/sumeragi/status`-də `lane_settlement_commitments` vasitəsilə.  Cəmilər
  `total_local_micro`, `total_xor_due_micro` və
  `total_xor_after_haircut_micro` gecə üçün blokun üzərində cəmləndi
  uzlaşma ixracı.
- Yeni `total_xor_variance_micro` sayğacı təhlükəsizlik marjasının nə qədər olduğunu izləyir
  istehlak (gözlənilən XOR ilə saç kəsimindən sonrakı gözlənti arasındakı fərq),
  və `swap_metadata` deterministik çevrilmə parametrlərini sənədləşdirir
  (TWAP, epsilon, likvidlik profili və dəyişkənlik_sinifi).
  iş vaxtı konfiqurasiyasından asılı olmayaraq təklif daxiletmələrini yoxlayın.

İstehlakçılar mövcud zolağın yanında `lane_settlement_commitments`-ə baxa bilərlər
ödəniş tamponlarının, saç düzümü səviyyələrinin,
və dəyişdirmə icrası konfiqurasiya edilmiş Nexus ödəniş modelinə uyğun gəlir.