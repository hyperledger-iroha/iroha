---
lang: uz
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4493e69ce57c4f691f368fb13c1bbe96e2c73991dfb39045753b5652d2f10a9
source_last_modified: "2026-01-28T17:11:30.702818+00:00"
translation_last_reviewed: 2026-02-07
title: Local → Global Address Toolkit
translator: machine-google-reviewed
---

Bu sahifa aks ettiradi [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md)
mono-repodan. U **ADDR-5c** yoʻl xaritasi elementi talab qiladigan CLI yordamchilari va runbook’larini toʻplaydi.

## Umumiy ko'rinish

- `scripts/address_local_toolkit.sh` ishlab chiqarish uchun `iroha` CLI-ni o'rab oladi:
  - `audit.json` — `iroha tools address audit --format json` dan tuzilgan chiqish.
  - `normalized.txt` - har bir Lokal domen selektori uchun afzal qilingan IH58 / ikkinchi eng yaxshi siqilgan (`sora`) literallari aylantirildi.
- Skriptni manzilni qabul qilish asboblar paneli bilan bog'lang (`dashboards/grafana/address_ingest.json`)
  va Local-8 / ni isbotlash uchun Alertmanager qoidalari (`dashboards/alerts/address_ingest_rules.yml`)
  Mahalliy-12 kesish xavfsiz. Local-8 va Local-12 to'qnashuv panellarini tomosha qiling
  Oldindan `AddressLocal8Resurgence`, `AddressLocal12Collision` va `AddressInvalidRatioSlo` ogohlantirishlar
  aniq o'zgarishlarni rag'batlantirish.
- [Manzilni ko'rsatish bo'yicha ko'rsatmalar](address-display-guidelines.md) va
  UX va voqea-javob konteksti uchun [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md).

## Foydalanish

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format ih58
```

Variantlar:

- IH58 o'rniga `sora…` chiqishi uchun `--format compressed`.
- Yalang'och harflarni chiqarish uchun `domainless output (default)`.
- O'tkazish bosqichini o'tkazib yuborish uchun `--audit-only`.
- Noto'g'ri shakllangan qatorlar paydo bo'lganda skanerlashni davom ettirish uchun `--allow-errors` (CLI xatti-harakatlariga mos keladi).

Skript ish oxirida artefakt yo'llarini yozadi. Ikkala faylni ham biriktiring
nolga tengligini tasdiqlovchi Grafana skrinshoti bilan birga o'zgartirishni boshqarish chiptasi
Mahalliy-8 aniqlash va ≥30 kun davomida nol Mahalliy-12 to'qnashuvi.

## CI integratsiyasi

1. Skriptni maxsus ishda ishga tushiring va uning natijalarini yuklang.
2. `audit.json` mahalliy selektorlar (`domain.kind = local12`) haqida xabar berganda blok birlashadi.
   standart `true` qiymatida (faqat ishlab/sinov klasterlarida `false` ni bekor qilish
   regressiyalarni tashxislash) va qo'shing
   `iroha tools address normalize` dan CI gacha regressiya
   ishlab chiqarishga urishdan oldin urinishlar muvaffaqiyatsizlikka uchraydi.

Batafsil ma'lumot, namunaviy dalillarni tekshirish ro'yxatlari va mijozlarga kesish haqida e'lon qilishda qayta ishlatishingiz mumkin bo'lgan reliz yozuvi parchasi uchun manba hujjatiga qarang.