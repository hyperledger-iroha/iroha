---
lang: uz
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SNNet-9 muvofiqlik konfiguratsiyasini takrorlanadigan,
auditga qulay jarayon. Har bir operator uchun uni yurisdiktsiya tekshiruvi bilan bog'lang
bir xil dayjestlar va dalillar sxemasidan foydalanadi.

## qadamlar

1. **Konfiguratsiyani yig‘ing**
   - Import `governance/compliance/soranet_opt_outs.json`.
   - `operator_jurisdictions`-ni chop etilgan attestatsiya dayjestlari bilan birlashtiring
     [yurisdiksiya tekshiruvida](gar-jurisdictional-review).
2. **Tasdiqlash**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - Majburiy emas: `cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **Dalillarni qo‘lga olish**
   - `artifacts/soranet/compliance/<YYYYMMDD>/` ostida saqlang:
     - `config.json` (yakuniy muvofiqlik bloki)
     - `attestations.json` (URI + digestlar)
     - tekshirish jurnallari
     - imzolangan PDF/Norito konvertlariga havolalar
4. **Faollashtirish**
   - Yordamchi dasturni belgilang (`gar-opt-out-<date>`), orkestr/SDK konfiguratsiyasini qayta joylashtiring,
     va kutilgan joyda jurnallarda chiqadigan `compliance_*` hodisalarini tasdiqlang.
5. **Yopish**
   - Boshqaruv kengashiga dalillar to'plamini topshiring.
   - GAR jurnaliga faollashtirish oynasini + tasdiqlovchilarni kiriting.
   - Yurisdiksiya bo'yicha ko'rib chiqish jadvalidan keyingi ko'rib chiqish sanalarini belgilang.

## Tez nazorat ro'yxati

- [ ] `jurisdiction_opt_outs` kanonik katalogga mos keladi.
- [ ] Attestatsiya dayjestlari aniq nusxalangan.
- [ ] Tasdiqlash buyruqlari ishga tushiriladi va arxivlanadi.
- [ ] Dalillar toʻplami `artifacts/soranet/compliance/<date>/` da saqlangan.
- [ ] Rollout yorlig'i + GAR jurnali yangilandi.
- [ ] Keyingi ko'rib chiqish uchun eslatmalar o'rnatildi.

## Shuningdek qarang

- [GAR yurisdiktsiya tekshiruvi](gar-jurisdictional-review)
- [GAR muvofiqligi bo'yicha qo'llanma (manba)](../../../source/soranet/gar_compliance_playbook.md)