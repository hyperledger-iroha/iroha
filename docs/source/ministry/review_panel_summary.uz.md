---
lang: uz
direction: ltr
source: docs/source/ministry/review_panel_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7325e72d18ec406eb134622ab51211fbb6582ebcc26bd719499e209db70f761b
source_last_modified: "2025-12-29T18:16:35.983094+00:00"
translation_last_reviewed: 2026-02-07
title: Review Panel Summary Workflow (MINFO-4a)
summary: Generate the neutral referendum summary with balanced citations, AI manifest references, and volunteer brief coverage.
translator: machine-google-reviewed
---

# Ko'rib chiqish panelining xulosasi (MINFO-4a)

“Yo‘l xaritasi” bandi **MINFO-4a — Neytral xulosa generatori** qabul qilingan kun tartibi taklifi, ko‘ngillilar uchun qisqacha korpus va tasdiqlangan AI moderatsiyasi manifestini neytral referendum xulosasiga aylantiradigan takrorlanadigan ish jarayonini talab qiladi. Yetkazib berish kerak:

- Chiqishni Norito tuzilmasi (`ReviewPanelSummaryV1`) sifatida yozib oling, shunda boshqaruv uni manifestlar va byulletenlar bilan birga arxivlashi mumkin.
- Ko'rib chiqish guruhi muvozanatli qo'llab-quvvatlamasa/qarshiliklarga ega bo'lmasa yoki faktlarda iqtiboslar etishmayotgan bo'lsa, tezda muvaffaqiyatsizlikka uchraydi.
- Siyosat hay'ati ovoz berishdan oldin avtomatlashtirilgan va insoniy kontekstni ko'rishini ta'minlab, har bir diqqatga sazovor joylarda AI manifestiga va taklif dalillar to'plamiga murojaat qiling.

## CLI foydalanish

Ish oqimi `cargo xtask` qismi sifatida yuboriladi:

```bash
cargo xtask ministry-panel synthesize \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --volunteer docs/examples/ministry/volunteer_brief_template.json \
  --ai-manifest docs/examples/ai_moderation_calibration_manifest_202602.json \
  --panel-round RP-2026-05 \
--output artifacts/review_panel/AC-2026-001-RP-2026-05.json
```

Majburiy ma'lumotlar:

1. `--proposal` - `AgendaProposalV1` ga rioya qilgan JSON foydali yuk. Xulosa yaratishdan oldin yordamchi sxemani tasdiqlaydi.
2. `--volunteer` - `docs/source/ministry/volunteer_brief_template.md` ga amal qiladigan ko'ngillilar brifinglarining JSON qatori. Mavzudan tashqari yozuvlar avtomatik ravishda e'tiborga olinmaydi.
3. `--ai-manifest` - Boshqaruv tomonidan imzolangan `ModerationReproManifestV1`, kontentni tekshirgan AI qo'mitasini tavsiflaydi.
4. `--panel-round` - Joriy ko'rib chiqish bosqichi uchun identifikator (`RP-YYYY-##`).
5. `--output` - Belgilangan fayl yoki stdout ga o'tkazish uchun `-`. Taklif tilini bekor qilish uchun `--language` va tarixni to‘ldirishda deterministik Unix vaqt tamg‘asini (millisekundlar) berish uchun `--generated-at` dan foydalaning.

Mustaqil xulosa yaratilgandan so'ng, ishga tushiring
[`cargo xtask ministry-panel packet`](referendum_packet.md) yig'ish uchun yordamchi
referendumning to'liq fayli (`ReferendumPacketV1`). Yetkazib berish
Paket buyrug'iga `--summary-out` bir xil xulosa faylini saqlab qoladi.
uni quyi oqim iste'molchilari uchun paket ob'ektiga joylashtirish.

### `ministry-transparency ingest` orqali avtomatlashtirish

Har choraklik dalillar to'plami uchun `cargo xtask ministry-transparency ingest`-ni ishga tushirgan jamoalar endi ko'rib chiqish panelining xulosasini bir xil chiziqqa yopishtirishlari mumkin:

```bash
cargo xtask ministry-transparency ingest \
  --quarter 2026-Q4 \
  --ledger artifacts/ministry/ledger.json \
  --appeals artifacts/ministry/appeals.json \
  --denylist artifacts/ministry/denylist.json \
  --treasury artifacts/ministry/treasury.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --panel-proposal artifacts/ministry/proposal_AC-2026-041.json \
  --panel-ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --panel-summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/ingest.json
```

Barcha to'rtta `--panel-*` bayroqlari birga ta'minlanishi kerak (va `--volunteer` talab qilinadi). Buyruq ko'rib chiqish panelining xulosasini `--panel-summary-out` ga chiqaradi, tahlil qilingan foydali yukni ingest snapshot ichiga joylashtiradi va quyi oqim asboblari dalillarni tasdiqlashi uchun nazorat summasini qayd qiladi.

## Linting va nosozlik rejimlari

Xulosa yozishdan oldin `cargo xtask ministry-panel synthesize` quyidagi invariantlarni qo'llaydi:

- **Muvozanatli pozitsiyalar:** kamida bitta qo'llab-quvvatlovchi brief va bitta qarshi brief bo'lishi kerak. Yo'qolgan qamrov tavsiflovchi xato bilan ishlashni tugatadi.
- **Iqtiboslar qamrovi:** diqqatga sazovor joylar faqat iqtiboslarni o'z ichiga olgan faktlar qatorlaridan ishlab chiqariladi. Yo'qotilgan iqtiboslar hech qachon qurilishni bloklamaydi, lekin har bir ta'sir qilingan qisqacha ma'lumot chiqishda `warnings[]` ostida keltirilgan.
- **Har bir diqqatga sazovor havolalar:** har bir diqqatga sazovor joy (a) ixtiyoriy faktlar qatori(lari), (b) AI manifest identifikatori va (c) taklifning birinchi dalil ilovasiga havolalarni oʻz ichiga oladi, shuning uchun paket har doim imzolangan artefaktlarga bogʻlanadi.Agar biron-bir tekshiruv bajarilmasa, buyruq nolga teng bo'lmagan holat bilan chiqadi va muammoli yozuvga ishora qiladi. Muvaffaqiyatli ishga tushirishlar `ReviewPanelSummaryV1` sxemasiga mos keladigan JSON faylini yozadi va boshqaruv manifestlariga kiritilishi mumkin.

## Chiqish tuzilishi

`ReviewPanelSummaryV1` `crates/iroha_data_model/src/ministry/mod.rs` da yashaydi va `iroha_data_model` kassasi orqali har bir iste'molchi uchun mavjud. Asosiy bo'limlarga quyidagilar kiradi:

- `overview` - Siyosat hakamlar hay'ati paketi uchun sarlavha, neytral qisqacha jumla va qaror konteksti.
- `stance_distribution` - Har bir pozitsiya uchun qisqacha ma'lumotlar va faktlar qatorlari soni. Pastki oqim panellari nashr qilishdan oldin qamrovni tasdiqlash uchun buni o'qiydi.
- `highlights` - To'liq malakali iqtiboslar bilan har bir pozitsiya uchun ikkita fakt xulosasi.
- `ai_manifest` - Qayta ishlab chiqarish manifestidan olingan metadata (manifest UUID, yuguruvchi versiyasi, chegaralar).
- `volunteer_references` - Audit uchun har bir qisqacha statistika (til, pozitsiya, qatorlar, keltirilgan qatorlar).
- `warnings` - O'tkazib yuborilgan narsalarni tavsiflovchi erkin shakldagi lint xabarlar (masalan, iqtiboslar etishmayotgan faktlar qatorlari).

## Misol

`docs/examples/ministry/review_panel_summary_example.json` yordamchi bilan ishlab chiqarilgan to'liq namunani o'z ichiga oladi. U muvozanatli qoʻllab-quvvatlash/qarshilik qamrovi, iqtiboslar uzatish, manifest havolalari va diqqatga sazovor joylarga koʻtarilishi mumkin boʻlmagan faktlar qatorlari uchun ogohlantirish satrlarini namoyish etadi. Neytral xulosani iste'mol qilishi kerak bo'lgan boshqaruv paneli, boshqaruv manifestlari yoki SDK vositalarini kengaytirishda undan foydalaning.

> **Maslahat:** tuzilgan xulosani imzolangan sunʼiy intellekt manifesti va ixtiyoriy qisqacha dayjest bilan birga referendum dalillar toʻplamiga kiriting, shunda hakamlar hayʼati koʻrib chiqish guruhi tomonidan havola qilingan har bir artefaktni tekshirishi mumkin.