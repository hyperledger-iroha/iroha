---
lang: uz
direction: ltr
source: docs/examples/docs_preview_feedback_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-12-29T18:16:35.071058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hujjatlarni oldindan koʻrish uchun fikr-mulohaza shakli (W1 hamkor toʻlqini)

W1 sharhlovchilarining fikr-mulohazalarini yig'ishda ushbu shablondan foydalaning. Uni takrorlang
hamkor, metama'lumotlarni to'ldiring va to'ldirilgan nusxani ostida saqlang
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.

## Ko'rib chiquvchi metama'lumotlari

- **Hamkor ID:** `partner-w1-XX`
- **Chipta so'rash:** `DOCS-SORA-Preview-REQ-PXX`
- **Taklif yuborildi (UTC):** `YYYY-MM-DD hh:mm`
- **Tasdiqlangan nazorat summasi (UTC):** `YYYY-MM-DD hh:mm`
- **Asosiy diqqat markazlari:** (masalan, _SoraFS orchestrator docs_, _Torii ISO flows_)

## Telemetriya va artefaktni tasdiqlash

| Tekshirish ro'yxati elementi | Natija | Dalil |
| --- | --- | --- |
| Tekshirish summasini tekshirish | ✅ / ⚠️ | Jurnalga yo'l (masalan, `build/checksums.sha256`) |
| Buni sinab ko'ring proksi tutun testi | ✅ / ⚠️ | `npm run manage:tryit-proxy …` transkript parchasi |
| Grafana asboblar panelini ko'rib chiqish | ✅ / ⚠️ | Skrinshot yo'llari |
| Portal tekshiruvi hisobotini ko'rib chiqish | ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

Tekshiruvchi tekshiradigan qo'shimcha SLOlar uchun qatorlarni qo'shing.

## Fikrlar jurnali

| Hudud | Jiddiylik (info/minor/major/blocker) | Tavsif | Tavsiya etilgan tuzatish yoki savol | Kuzatuvchi muammosi |
| --- | --- | --- | --- | --- |
| | | | | |

Oldindan ko'rish uchun oxirgi ustundagi GitHub muammosiga yoki ichki chiptaga murojaat qiling
treker tuzatish elementlarini ushbu shaklga bog'lashi mumkin.

## So'rov xulosasi

1. **Tekshirish summasi yoʻriqnomasi va taklif jarayoniga qanchalik ishonasiz?** (1–5)
2. **Qaysi hujjatlar ko‘proq/eng kam yordam bergan?** (qisqa javob)
3. **Try it proksi yoki telemetriya asboblar paneliga kirishni bloklovchilar bormi?**
4. **Qo‘shimcha mahalliylashtirish yoki mavjud kontent talab qilinadimi?**
5. **GA dan oldin boshqa izohlar bormi?**

Agar tashqi shakldan foydalansangiz, qisqa javoblarni yozib oling va so'rovnoma eksportini ilova qiling.

## Bilimlarni tekshirish

- Bal: `__/10`
- Noto'g'ri savollar (agar mavjud bo'lsa): `[#1, #4, …]`
- Keyingi harakatlar (agar ball < 9/10 bo'lsa): tuzatish chaqiruvi rejalashtirilganmi? y/n

## Ro'yxatdan o'tish

- Ko'rib chiquvchi nomi va vaqt tamg'asi:
- Docs/DevRel sharhlovchisi va vaqt tamg'asi:

Imzolangan nusxani bog'langan artefaktlar bilan saqlang, shunda auditorlar uni takrorlashlari mumkin
qo'shimcha kontekstsiz to'lqin.