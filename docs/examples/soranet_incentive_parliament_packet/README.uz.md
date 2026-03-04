---
lang: uz
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet Relay Incentive Parlament paketi

Ushbu to'plam Sora Parlamenti tomonidan ma'qullanishi kerak bo'lgan artefaktlarni qamrab oladi
avtomatik o'tkazma to'lovlari (SNNet-7):

- `reward_config.json` - Norito-seriyali uzatiladigan mukofotli dvigatel konfiguratsiyasi, tayyor
  `iroha app sorafs incentives service init` tomonidan yutilishi kerak. The
  `budget_approval_id` boshqaruv bayonnomalarida keltirilgan xeshga mos keladi.
- `shadow_daemon.json` - benefitsiar va obligatsiyalar xaritasi takrorlash orqali iste'mol qilinadi
  jabduqlar (`shadow-run`) va ishlab chiqarish demoni.
- `economic_analysis.md` - 2025-10 -> 2025-11 uchun adolatlilik xulosasi
  soya simulyatsiyasi.
- `rollback_plan.md` - avtomatik to'lovlarni o'chirish uchun operatsion kitob.
- Qo'llab-quvvatlovchi artefaktlar: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Butunlikni tekshirish

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

Dijestlarni parlament bayonnomasida qayd etilgan qiymatlar bilan solishtiring. Tasdiqlash
da tavsiflanganidek, soyada ishlaydigan imzo
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Paketni yangilash

1. `reward_config.json` ni har doim mukofot og'irligi, asosiy to'lov yoki
   tasdiqlash xesh o'zgarishi.
2. 60 kunlik soya simulyatsiyasini qayta ishga tushiring, `economic_analysis.md` ni yangilang.
   yangi topilmalar va JSON + ajratilgan imzo juftligini tasdiqlang.
3. Yangilangan to'plamni Observatoriya asboblar paneli bilan birgalikda parlamentga taqdim eting
   yangilangan tasdiqni talab qilganda eksport.