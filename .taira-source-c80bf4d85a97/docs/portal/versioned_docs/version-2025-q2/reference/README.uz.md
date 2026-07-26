---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e1f2fc637a1fc283e3079ffc22ddff70c6eb1e568a21b951b61491529052c234
source_last_modified: "2025-12-29T18:16:35.906895+00:00"
translation_last_reviewed: 2026-02-07
title: Reference Index
slug: /reference
translator: machine-google-reviewed
---

Ushbu bo'lim Iroha uchun "uni spetsifikatsiya sifatida o'qing" materialini jamlaydi. Bu sahifalar shunday bo'lsa ham barqaror bo'lib qoladi
qo'llanmalar va o'quv qo'llanmalar rivojlanadi.

## Bugun mavjud

- **Norito kodek haqida umumiy ma'lumot** - `reference/norito-codec.md` to'g'ridan-to'g'ri vakolatli tashkilotga havolalar
  Portal jadvali to'ldirilayotganda `norito.md` spetsifikatsiyasi.
- **Torii OpenAPI** – `/reference/torii-openapi` so‘nggi Torii REST spetsifikatsiyasini ishlatadi.
  Redok. `npm run sync-openapi` bilan spetsifikatsiyani qayta tiklang.
- **Konfiguratsiya jadvallari** - To'liq parametrlar katalogi saqlanadi
  `docs/source/references/configuration.md`. Portal avto-importni yubormaguncha, unga havola qiling
  Aniq standart va atrof-muhitni bekor qilish uchun Markdown fayli.

## Tez orada

- **Torii REST ma'lumotnomasi** - OpenAPI ta'riflari ushbu bo'lim orqali sinxronlanadi
  `docs/portal/scripts/sync-openapi.mjs` quvur liniyasi yoqilgandan keyin.
- **CLI buyruq indeksi** – Yaratilgan buyruq matritsasi (`crates/iroha_cli/src/commands` aks ettiruvchi)
  kanonik misollar bilan birga bu erga tushadi.
- **IVM ABI jadvallari** – Koʻrsatkich turi va tizimli matritsalar (`crates/ivm/docs` ostida saqlanadi)
  Hujjat yaratish ishi ulangandan so'ng portalga ko'rsatiladi.

## Ushbu indeksni joriy saqlash

Yangi ma'lumotnoma qo'shilganda - yaratilgan API hujjatlari, kodek xususiyatlari, konfiguratsiya matritsalari - joylashtiring
`docs/portal/docs/reference/` ostidagi sahifani oching va uni yuqoriga bog'lang. Agar sahifa avtomatik yaratilgan bo'lsa, esda tuting
skriptni sinxronlash, shuning uchun hissa qo'shuvchilar uni qanday yangilashni bilishadi. Bu mos yozuvlar daraxtini ga qadar foydali saqlaydi
to'liq avtomatik yaratilgan navigatsiya erlari.