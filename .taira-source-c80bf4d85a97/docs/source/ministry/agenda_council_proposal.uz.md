---
lang: uz
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2025-12-29T18:16:35.977493+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kun tartibidagi kengash taklifi sxemasi (MINFO-2a)

Yoʻl xaritasi maʼlumotnomasi: **MINFO-2a — Taklif formati tekshiruvchisi.**

Kun tartibi kengashi ish jarayoni fuqarolar tomonidan taqdim etilgan qora ro'yxat va siyosat o'zgarishlarini to'playdi
takliflarni boshqaruv panellari ko'rib chiqishdan oldin. Ushbu hujjat ni belgilaydi
kanonik foydali yuk sxemasi, dalillar talablari va takrorlanishni aniqlash qoidalari
yangi validator (`cargo xtask ministry-agenda validate`) tomonidan iste'mol qilinadi, shuning uchun
taklif qiluvchilar JSON jo'natmalarini portalga yuklashdan oldin ularni mahalliy sifatida ko'rsatishi mumkin.

## Yuk ko'rinishi

Kun tartibidagi takliflar `AgendaProposalV1` Norito sxemasidan foydalanadi
(`iroha_data_model::ministry::AgendaProposalV1`). Maydonlar qachon JSON sifatida kodlangan
CLI/portal sirtlari orqali yuborish.

| Maydon | Tur | Talablar |
|-------|------|--------------|
| `version` | `1` (u16) | `AGENDA_PROPOSAL_VERSION_V1` ga teng boʻlishi kerak. |
| `proposal_id` | string (`AC-YYYY-###`) | Barqaror identifikator; tekshirish vaqtida amalga oshiriladi. |
| `submitted_at_unix_ms` | u64 | Unix davridan beri millisekundlar. |
| `language` | string | BCP‑47 yorlig'i (`"en"`, `"ja-JP"` va boshqalar). |
| `action` | enum (`add-to-denylist`, `remove-from-denylist`, `amend-policy`) | Vazirlikdan chora koʻrishni soʻragan. |
| `summary.title` | string | ≤256 belgi tavsiya etiladi. |
| `summary.motivation` | string | Nima uchun harakat talab qilinadi. |
| `summary.expected_impact` | string | Agar harakat qabul qilingan bo'lsa, natijalar. |
| `tags[]` | kichik harflar qatorlari | Ixtiyoriy triaj belgilari. Ruxsat etilgan qiymatlar: `csam`, `malware`, `fraud`, `harassment`, `impersonation`, `policy-escalation`, `policy-escalation`, ```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```00, Norito, Norito, Norito. |
| `targets[]` | ob'ektlar | Bir yoki bir nechta hash oilasi yozuvlari (pastga qarang). |
| `evidence[]` | ob'ektlar | Bir yoki bir nechta dalil qo'shimchalari (pastga qarang). |
| `submitter.name` | string | Ko'rsatilgan nom yoki tashkilot. |
| `submitter.contact` | string | Elektron pochta, Matritsa tutqichi yoki telefon; umumiy boshqaruv panelidan tahrirlangan. |
| `submitter.organization` | string (ixtiyoriy) | Sharhlovchi foydalanuvchi interfeysida koʻrinadi. |
| `submitter.pgp_fingerprint` | string (ixtiyoriy) | 40 olti burchakli bosh barmoq izi. |
| `duplicates[]` | strings | Ilgari yuborilgan taklif identifikatorlariga ixtiyoriy havolalar. |

### Maqsadli yozuvlar (`targets[]`)

Har bir maqsad taklifga havola qilingan xesh oilasi dayjestini ifodalaydi.

| Maydon | Tavsif | Tasdiqlash |
|-------|-------------|------------|
| `label` | Sharhlovchi konteksti uchun qulay ism. | Bo'sh emas. |
| `hash_family` | Xesh identifikatori (`blake3-256`, `sha256` va boshqalar). | ASCII harflari/raqamlari/`-_.`, ≤48 ta belgi. |
| `hash_hex` | Dijest kichik olti harf bilan kodlangan. | ≥16 bayt (32 hex belgi) va yaroqli hex boʻlishi kerak. |
| `reason` | Nima uchun hazm qilish kerakligi haqida qisqacha tavsif. | Bo'sh emas. |

Tasdiqlovchi bir xildagi `hash_family:hash_hex` ikki nusxadagi juftlarni rad etadi
Agar bir xil barmoq izi allaqachon mavjud bo'lsa, taklif va hisobotlar ziddiyatlari
dublikat ro'yxatga olish kitobi (pastga qarang).

### Dalil qo'shimchalari (`evidence[]`)

Tekshiruvchilar qo'llab-quvvatlovchi kontekstni olishlari mumkin bo'lgan dalillar yozuvlari hujjati.| Maydon | Tur | Eslatmalar |
|-------|------|-------|
| `kind` | enum (`url`, `torii-case`, `sorafs-cid`, `attachment`) | Ovqat hazm qilish talablarini aniqlaydi. |
| `uri` | string | HTTP(S) URL, Torii holat identifikatori yoki SoraFS URI. |
| `digest_blake3_hex` | string | `sorafs-cid` va `attachment` turlari uchun talab qilinadi; boshqalar uchun ixtiyoriy. |
| `description` | string | Sharhlovchilar uchun ixtiyoriy erkin shakldagi matn. |

### Ro'yxatga olish kitobining dublikati

Operatorlar takrorlanishining oldini olish uchun mavjud barmoq izlari reestrini yuritishi mumkin
holatlar. Tasdiqlovchi quyidagi shakldagi JSON faylini qabul qiladi:

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

Taklif maqsadi yozuvga to‘g‘ri kelsa, tekshiruvchi bekor qiladi
`--allow-registry-conflicts` ko'rsatilgan (ogohlantirishlar hali ham chiqariladi).
Buning uchun [`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) dan foydalaning
dublikatni o'zaro bog'laydigan referendumga tayyor xulosani yarating
ro'yxatga olish kitobi va siyosat lavhalari.

## CLI foydalanish

Bitta taklifni belgilang va uni dublikat registriga qarshi tekshiring:

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

`--allow-registry-conflicts` dan takroriy hitlarni ogohlantirish darajasiga tushirish uchun o'ting
tarixiy tekshiruvlarni o'tkazish.

CLI bir xil Norito sxemasiga va yetkazib berilgan tekshirish yordamchilariga tayanadi.
`iroha_data_model`, shuning uchun SDK/portallar `AgendaProposalV1::validate` dan qayta foydalanishi mumkin
izchil xatti-harakatlar usuli.

## Saralash CLI (MINFO-2b)

Yoʻl xaritasi maʼlumotnomasi: **MINFO-2b — Koʻp oʻrinli tartiblash va audit jurnali.**

Kun tartibidagi kengash ro'yxati endi fuqarolar uchun deterministik tartiblash orqali boshqariladi
har bir tirajni mustaqil tekshirishi mumkin. Yangi buyruqdan foydalaning:

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` - JSON fayli har bir munosib a'zoni tavsiflaydi:

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  Misol fayli manzilda yashaydi
  `docs/examples/ministry/agenda_council_roster.json`. Ixtiyoriy maydonlar (rol,
  tashkilot, aloqa, metama'lumotlar) Merkle bargida ushlanadi, shuning uchun auditorlar
  qura tashlashni ta'minlagan ro'yxatni isbotlashi mumkin.

- `--slots` - to'ldirish uchun kengash o'rinlari soni.
- `--seed` — 32 baytlik BLAKE3 urugʻi (64 ta kichik hex belgi)
  qura tashlash uchun boshqaruv bayonnomalari.
- `--out` - ixtiyoriy chiqish yo'li. Agar o'tkazib yuborilsa, JSON xulosasi chop etiladi
  stdout.

### Chiqish xulosasi

Buyruq `SortitionSummary` JSON blobini chiqaradi. Namuna chiqishi quyidagi manzilda saqlanadi
`docs/examples/ministry/agenda_sortition_summary_example.json`. Asosiy maydonlar:

| Maydon | Tavsif |
|-------|-------------|
| `algorithm` | Saralash yorlig'i (`agenda-sortition-blake3-v1`). |
| `roster_digest` | Ro'yxat faylining BLAKE3 + SHA-256 dayjestlari (auditlar bir xil a'zolar ro'yxatida ishlashini tasdiqlash uchun ishlatiladi). |
| `seed_hex` / `slots` | Auditorlar o'yinni takrorlashi uchun CLI ma'lumotlarini aks ettiring. |
| `merkle_root_hex` | Ro'yxatdagi Merkle daraxtining ildizi (`xtask/src/ministry_agenda.rs` da `hash_node`/`hash_leaf` yordamchilari). |
| `selected[]` | Har bir slot uchun yozuvlar, jumladan, kanonik aʼzo metamaʼlumotlari, mos indeks, asl roʻyxat indeksi, deterministik chizish entropiyasi, barg xeshi va Merkle isboti birodarlar. |

### Durangni tekshirish1. `roster_path` tomonidan havola qilingan roʻyxatni oling va uning BLAKE3/SHA-256-ni tekshiring.
   Dijestlar xulosaga mos keladi.
2. CLI-ni bir xil urug '/ uyalar / ro'yxat bilan qayta ishga tushiring; natijada `selected[].member_id`
   buyurtma e'lon qilingan xulosaga mos kelishi kerak.
3. Muayyan a'zo uchun JSON seriyali a'zosi yordamida Merkle bargini hisoblang
   (`norito::json::to_vec(&sortition_member)`) va har bir isbot xashda katlayın. Final
   digest `merkle_root_hex` ga teng bo'lishi kerak. Misol xulosasidagi yordamchi ko'rsatadi
   `eligible_index`, `leaf_hash_hex` va `merkle_proof[]`ni qanday birlashtirish kerak.

Ushbu artefaktlar tekshirilishi mumkin bo'lgan tasodifiylik uchun MINFO-2b talabini qondiradi,
k-of-m tanlash va zanjirdagi API simli bo'lgunga qadar faqat audit jurnallarini qo'shish.

## Tasdiqlash xatosi ma'lumotnomasi

`AgendaProposalV1::validate` `AgendaProposalValidationError` variantlarini chiqaradi
har doim foydali yuk liniyadan chiqmasa. Quyidagi jadval eng keng tarqalganini umumlashtiradi
xatolar, shuning uchun portal ko'rib chiquvchilari CLI chiqishini amaliy ko'rsatmalarga tarjima qilishlari mumkin.| Xato | Ma'nosi | Tuzatish |
|-------|---------|-------------|
| `UnsupportedVersion { expected, found }` | `version` foydali yuki validator tomonidan qoʻllab-quvvatlanadigan sxemadan farq qiladi. | Versiya `expected`ga mos kelishi uchun eng soʻnggi sxema toʻplamidan foydalanib JSONni qayta yarating. |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` bo'sh yoki `AC-YYYY-###` shaklida emas. | Qayta yuborishdan oldin hujjatlashtirilgan formatga muvofiq noyob identifikatorni to'ldiring. |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` nolga teng yoki yo'q. | Unix millisekundlarida yuborish vaqt tamg'asini yozib oling. |
| `InvalidLanguageTag { value }` | `language` yaroqli BCP‑47 teg emas. | `en`, `ja-JP` kabi standart teg yoki BCP‑47 tomonidan tan olingan boshqa tildan foydalaning. |
| `MissingSummaryField { field }` | `summary.title`, `.motivation` yoki `.expected_impact`lardan biri boʻsh. | Ko'rsatilgan xulosa maydoni uchun bo'sh bo'lmagan matnni taqdim eting. |
| `MissingSubmitterField { field }` | `submitter.name` yoki `submitter.contact` etishmayapti. | Ko'rib chiquvchilar taklif etuvchi bilan bog'lanishi uchun etishmayotgan yuboruvchi metama'lumotlarini taqdim eting. |
| `InvalidTag { value }` | `tags[]` yozuvi ruxsat etilgan ro'yxatda yo'q. | Tegni hujjatlashtirilgan qiymatlardan biriga olib tashlang yoki nomini o'zgartiring (`csam`, `malware` va boshqalar). |
| `MissingTargets` | `targets[]` massivi bo'sh. | Kamida bitta maqsadli xesh guruhini kiriting. |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | Maqsadli yozuvda `label` yoki `reason` maydonlari yoʻq. | Qayta yuborishdan oldin indekslangan yozuv uchun kerakli maydonni to'ldiring. |
| `InvalidHashFamily { index, value }` | `hash_family` yorlig'i qo'llab-quvvatlanmaydi. | Xesh familiyalarini ASCII alfanumerik va `-_` bilan cheklang. |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | Dijest yaroqsiz hex yoki 16 baytdan qisqa. | Indekslangan maqsad uchun kichik harfli olti burchakli dayjestni (≥32 olti burchakli belgilar) taqdim eting. |
| `DuplicateTarget { index, fingerprint }` | Maqsadli dayjest oldingi yozuv yoki registrdagi barmoq izini takrorlaydi. | Dublikatlarni olib tashlang yoki tasdiqlovchi dalillarni bitta maqsadga birlashtiring. |
| `MissingEvidence` | Hech qanday dalil qo'shimchalari taqdim etilmagan. | Reproduksiya materiallari bilan bog'langan kamida bitta dalil yozuvini ilova qiling. |
| `MissingEvidenceUri { index }` | Dalil yozuvida `uri` maydoni mavjud emas. | Indekslangan dalil yozuvi uchun olinadigan URI yoki ish identifikatorini taqdim eting. |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | Dijestni talab qiladigan dalil yozuvi (SoraFS CID yoki biriktirma) yoʻq yoki yaroqsiz `digest_blake3_hex`. | Indekslangan yozuv uchun 64 belgidan iborat kichik BLAKE3 dayjestini taqdim eting. |

## Misollar

- `docs/examples/ministry/agenda_proposal_example.json` - kanonik,
  ikkita dalil ilovasi bilan lint-clean taklif yuki.
- `docs/examples/ministry/agenda_duplicate_registry.json` - boshlang'ich registr
  bitta BLAKE3 barmoq izi va mantiqiy ma'lumotlarni o'z ichiga oladi.

Portal vositalarini birlashtirish yoki CI yozishda ushbu fayllardan shablon sifatida qayta foydalaning
avtomatlashtirilgan taqdimotlarni tekshiradi.