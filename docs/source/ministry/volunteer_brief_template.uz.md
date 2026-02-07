---
lang: uz
direction: ltr
source: docs/source/ministry/volunteer_brief_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cae4747782524b545fdcd52e7523cce0f5b60ddb85f32c747c5f57a63f85ccdc
source_last_modified: "2025-12-29T18:16:35.984008+00:00"
translation_last_reviewed: 2026-02-07
title: Volunteer Brief Template
summary: Structured template for roadmap item MINFO-3a covering balanced briefs, fact tables, conflict disclosures, and moderation tags.
translator: machine-google-reviewed
---

# Ko'ngillilar uchun qisqacha shablon (MINFO-3a)

Yoʻl xaritasi maʼlumotnomasi: **MINFO-3a — Balanslangan qisqacha andozalar va ziddiyatlarni oshkor qilish.**

Ko‘ngillilarning qisqacha taqdimotlari qora ro‘yxatni o‘zgartirish yoki boshqa vazirlik ijrosini ta’minlash bo‘yicha takliflar taklif qilinganda fuqarolar hay’atlari boshqaruv tomonidan ko‘rib chiqilishini istagan pozitsiyalarni umumlashtiradi. MINFO-3a har bir brifing deterministik tuzilmaga amal qilishini talab qiladi, shuning uchun shaffoflik quvuri (1) taqqoslanadigan faktlar jadvallarini ko'rsatishi, (2) manfaatlar to'qnashuvi oshkor etilishini tasdiqlashi va (3) mavzudan tashqari xabarlarni avtomatik ravishda o'chirib tashlashi yoki belgilashi mumkin. Bu sahifa `cargo xtask ministry-transparency` da yuborilgan asboblar tomonidan kutilgan kanonik maydonlarni, CSV uslubidagi faktlar jadvali tartibini va moderatsiya teglarini belgilaydi.

> **Norito sxemasi:** `iroha_data_model::ministry::VolunteerBriefV1` tuzilmasi (`1` versiyasi) endi barcha taqdimnomalar uchun vakolatli sxema hisoblanadi. Asboblar va portal tekshiruvchilari qisqacha ma'lumotni nashr etishdan yoki panel xulosalarida havola qilishdan oldin `VolunteerBriefV1::validate` ga qo'ng'iroq qilishadi.

## Yuborish yuki tuzilishi

| Bo'lim | Maydonlar | Talablar |
|---------|--------|--------------|
| **Konvert** | `version` (u16) | `1` boʻlishi kerak. Versiya himoyasi Vazirlikka sxemani noaniqliksiz rivojlantirishga imkon beradi. |
| **Shaxs va pozitsiya** | `brief_id` (string, kalendar yili uchun yagona), `proposal_id` (qora roʻyxat yoki siyosat harakati uchun havolalar), `language` (BCP-47), `stance` (`support`/`oppose`/`context`), `submitted_at` (RFC3339) | Barcha maydonlar talab qilinadi. `stance` asboblar panelini ta'minlaydi va ruxsat etilgan lug'atga mos kelishi kerak. |
| **Muallif haqida ma'lumot** | `author.name`, `author.organization` (ixtiyoriy), `author.contact`, `author.no_conflicts_certified` (bool) | `author.contact` umumiy asboblar panelidan tahrirlangan, ammo xom artefaktda saqlanadi. `no_conflicts_certified: true` o'rnating, agar muallif hech qanday oshkor etilmasligini tasdiqlagan bo'lsa. |
| **Xulosa** | `summary.title`, `summary.abstract`, `summary.requested_action` | Matnning umumiy ko'rinishi faktlar jadvali yonida paydo bo'ldi. `summary.abstract` ni ≤2000 belgigacha cheklang. |
| **Faktlar jadvali** | `fact_table` massivi (keyingi bo'limga qarang) | Hatto qisqa briflar uchun ham talab qilinadi. CLI va oshkoralik ishi faktlar jadvalisiz taqdimotlarni rad etadi. |
| **Oshkorlar** | `disclosures` massivi YOKI `author.no_conflicts_certified: true` | Har bir oshkor qatorida `type` (`financial`, `employment`, `governance`, `family`, `other`), `other`, Norito, Norito, `relationship` va `details`. |
| **Moderatsiya metamaʼlumotlari** | `moderation.off_topic` (bool), `moderation.tags` (enum satrlari qatori), `moderation.notes` | Sharhlovchilar tomonidan astroturfing yoki aloqador bo'lmagan taqdimotlarni bostirish uchun foydalaniladi. Mavzudan tashqari yozuvlar asboblar paneliga hissa qo'shmaydi. |

## Faktlar jadvali spetsifikatsiyasi

Har bir `fact_table` qatori mashinada oʻqiladigan daʼvoni qamrab oladi. Qatorlarni JSON obyektlari sifatida quyidagi maydonlar bilan saqlang:| Maydon | Tavsif |
|-------|-------------|
| `claim_id` | Barqaror identifikator (masalan, `VB-2026-04-F1`). |
| `claim` | Bir jumladan iborat fakt yoki ta'sir bayonoti. |
| `status` | `corroborated`, `disputed`, `context-only` dan biri. |
| `impact` | `governance`, `technical`, `compliance`, `community` bir yoki bir nechtasini oʻz ichiga olgan massiv. |
| `citations` | Bo'sh bo'lmagan qatorlar qatori. URL manzillar, Torii ish identifikatorlari yoki CID ma'lumotnomalari qabul qilinadi. |
| `evidence_digest` | Qo'llab-quvvatlovchi hujjatlarning ixtiyoriy BLAKE3 nazorat summasi. |

Avtomatlashtirish bo'yicha eslatmalar:
- Nashr ko'rsatkichlari kartalarini yaratish uchun qabul qilingan ish `fact_rows` va `fact_rows_with_citation` hisoblanadi. Iqtibossiz qatorlar hali ham odam o'qiy oladigan jadvalda ko'rinadi, ammo etishmayotgan dalillar sifatida kuzatiladi.
- Da'volarni qisqa tuting va boshqaruv takliflarida ishlatiladigan bir xil identifikatorlarga havola qiling, shuning uchun o'zaro bog'lanish deterministikdir.

## Nizolarni oshkor qilish talablari

1. Moliyaviy, bandlik, boshqaruv yoki oilaviy rishtalar mavjud bo'lganda kamida bitta oshkora yozuvni taqdim eting.
2. `author.no_conflicts_certified: true` dan “maʼlum ziddiyatlar yoʻq” deb tasdiqlang. Taqdim etilgan materiallar oshkor qilish yozuvini yoki `true` sertifikatini o'z ichiga olishi kerak; aks holda, ular qabul qilish vaqtida belgilanadi.
3. Ommaviy hujjatlar mavjud boʻlganda (masalan, korporativ arizalar, DAO ovozlari) `disclosures[i].evidence` ni qoʻshing. Dalillar "yo'q" sertifikatlari uchun ixtiyoriy, lekin qat'iy tavsiya etiladi.

## Moderatsiya teglari va mavzudan tashqari ishlov berish

Moderatsiyani ko'rib chiquvchilar shaffoflik tizimiga kirishdan oldin yuborilgan narsalarni belgilashlari mumkin:

- `moderation.off_topic: true` `off_topic_rejections` hisoblagichini oshirganda, agregat hisoblardan yozuvni olib tashlaydi. Qator hali ham audit uchun xom arxivda mavjud.
- `moderation.tags` enum qiymatlarini qabul qiladi: `duplicate`, `needs-translation`, `needs-follow-up`, `spam`, `astroturf`, I100NI7300. Teglar to'liq qisqacha ma'lumotni qayta o'qimasdan, quyi oqimdagi sharhlovchilarga tahlil qilishda yordam beradi.
- `moderation.notes` moderatsiya qarorining qisqacha asoslanishini saqlaydi (≤512 belgi).

## Taqdim etish roʻyxati

1. Ushbu shablon yoki quyida tavsiflangan yordamchi CLI yordamida JSON foydali yukini to‘ldiring.
2. Hech bo'lmaganda bitta faktlar jadvali qatorini to'ldiring; har bir qator uchun iqtiboslarni kiriting.
3. Oshkoralarni taqdim eting yoki `author.no_conflicts_certified: true` ni aniq belgilang.
4. Moderatsiya metamaʼlumotlarini (standart `off_topic: false`) qoʻshing, shunda koʻrib chiquvchilar tezda sinovdan oʻtishlari mumkin.
5. Yuklashdan oldin foydali yukni `cargo xtask ministry-transparency ingest --volunteer <file>` yoki istalgan Norito validator bilan tasdiqlang.

## Validatsiya CLI (MINFO-3)

Hozirda ombor ko'ngillilar brifinglari uchun maxsus validatorni jo'natadi:

```bash
cargo xtask ministry-transparency volunteer-validate \
  --input docs/examples/ministry/volunteer_brief_template.json \
  --json-output artifacts/ministry/volunteer_lint_report.json
```

Asosiy xatti-harakatlar:- individual JSON obyektlarini *yoki* brifing massivlarini qabul qiladi; `--input` ni bir necha marta bir ishga tushirishda bir nechta fayllarni to'ldirish uchun o'tkazing.
- xatolar va ogohlantirishlar sonini ko'rsatadigan har bir qisqacha xulosani chiqaradi; ogohlantirishlar bo'sh iqtiboslar ro'yxatini yoki ortiqcha eslatmalarni ta'kidlaydi, xatolar esa nashrni bloklaydi.
- Majburiy maydonlar (`brief_id`, `proposal_id`, `stance`, faktlar jadvali mazmuni, oshkor qilish yoki `no_conflicts_certified`) ushbu shablonga mos kelishini va enum qiymatlari hujjatlashtirilgan lugʻatlar ichida qolishini taʼminlaydi.
- `--json-output <path>` o'rnatilganda validator har bir qisqacha ma'lumotni (taklif identifikatori, pozitsiyasi, holati, xatolar/ogohlantirishlar) jamlagan holda mashinada o'qiladigan manifestni yozadi. Portalning `npm run generate:volunteer-lint` buyrug'i ushbu manifestni har bir taklif sahifasi yonida lint holatini ko'rsatish uchun sarflaydi.

Ko‘ngillilar tomonidan yuborilgan jo‘natmalar shaffoflikni qabul qilish vazifasiga yetguncha **MINFO-3** ga muvofiq bo‘lishini ta’minlash uchun buyruqni portal ish oqimlariga yoki CIga integratsiya qiling.

## Foydali yukga misol

To'liq to'ldirilgan misol uchun `docs/examples/ministry/volunteer_brief_template.json` ga qarang, jumladan faktlar jadvali qatorlari, oshkor qilish va moderatsiya teglari. Pastki oqim asboblar paneli xom JSON-ni iste'mol qiladi va avtomatik ravishda hisoblab chiqadi:

- `total_briefs` (mavzudan tashqari taqdimotlar bundan mustasno)
- `fact_rows` / `fact_rows_with_citation`
- `disclosures_missing`
- `off_topic_rejections`

Agar yangi maydonlar kerak bo'lsa, boshqaruv dalillari takrorlanishi mumkin bo'lib qolishi uchun ushbu hujjatni va qabul qilish sarhisobini (`xtask/src/ministry.rs`) bir xil o'zgartirishda yangilang.

## Nashrning SLA va portal yuzasi (MINFO-3)

Fuqarolarning murojaatlari shaffof boʻlishini taʼminlash uchun portal endi tekshiruvdan oʻtgandan soʻng belgilangan kadens boʻyicha qisqacha maʼlumotlarni eʼlon qiladi:

1. **T+0–6hours:** ko‘ngillilar arizasi yoki `cargo xtask ministry-transparency ingest` orqali yuboriladi. Tekshiruvchilar `VolunteerBriefV1::validate` ni ishga tushiradilar, noto'g'ri tuzilgan foydali yuklarni rad etadilar va lint hisobotlarini chiqaradilar (etishmayotgan oshkorlar, takroriy fakt identifikatorlari va boshqalar).
2. **T+6–24 soat:** qabul qilingan brifinglar tarjima/triaj uchun navbatga qo‘yiladi. Moderatsiya teglari (`needs-translation`, `duplicate`, `policy-escalation`, …) qoʻllaniladi va mavzudan tashqari yozuvlar arxivlanadi, lekin umumiy hisoblardan chiqarib tashlanadi.
3. **T+24–48hours:** portal qisqacha maʼlumotni tegishli taklif sahifasi bilan birga eʼlon qiladi. Har bir e'lon qilingan taklif endi "Ko'ngillilar fikrlari" bilan bog'lanadi, shuning uchun sharhlovchilar JSON-ni ochmasdan qo'llab-quvvatlash/qarshilik/kontekst brifinglarini o'qishlari mumkin.

Agar taqdimnoma `policy-escalation` yoki `astroturf` deb belgilangan boʻlsa, SLA **12 soat**gacha qattiqlashadi, shuning uchun boshqaruv tezda javob berishi mumkin. Operatorlar SLAni hujjatlar portalidagi (`docs/portal/docs/ministry/volunteer-briefs.md`) **Ko‘ngillilar haqida qisqacha ma’lumot** sahifasi orqali tekshirishlari mumkin, unda eng so‘nggi nashr oynalari, lint holati va Norito artefaktlariga havolalar ro‘yxati keltirilgan.