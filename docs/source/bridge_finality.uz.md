---
lang: uz
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Ko'prik yakuniyligi isbotlari

Ushbu hujjat birinchi reliz uchun ko'prik yakuniyligi formatini belgilaydi. Isbot
Sumeragi v2 yaratgan va doimiy saqlagan aniq yakuniylik dalilini tashiydi. Isbot
konvertining sxema versiyasi `1`, ichidagi konsensus protokoli versiyasi esa `2`.
Sumeragi v1 sertifikatiga proyeksiya, dekoder yoki fallback yo'li mavjud emas.

## Aniq isbot formati

Norito yoki Norito JSON bilan kodlangan `BridgeFinalityProof` faqat uchta maydonga ega:

```text
{ version, block_header, finality_artifact }
```

- `version` albatta `1` bo'lishi kerak;
- `block_header` so'ralgan balandlikning kanonik `BlockHeader`-idir;
- `finality_artifact` shu blok uchun saqlangan aniq `V2FinalityArtifact`. U height-context
  roster tartibida har bir validatorning BLS-normal PoP-ni (`validator_set_pops`) doimiy
  ravishda o'zida saqlaydi.

Artefakt to'liq va o'zgarmas `HeightContext`, aniq `BlockSubject`, blok hash-i, CommitQC
va rosterga mos PoP-larni o'z ichiga oladi. Height context zanjir, epoch, roster,
`DualQuorum`, DA joylashuvi, leader seed va boshqa konsensus ma'lumotlarini muzlatadi.
Epoch-ni tugatuvchi ota blok context-i optional `next_epoch_snapshot` ni ham o'z ichiga
oladi; bu maydon context id qismi bo'lgani uchun, bola rosterga ruxsat berilishidan oldin
ota CommitQC uni autentifikatsiya qiladi. Finalized snapshot keyingi epoch parametrlari
bilan birga `epoch_end_height` va keyingi rosterga mos `validator_set_pops` ni ham bog'laydi.

## Doimiy saqlash va tekshirish

Finality e'lon qilinishidan yoki block body chiqarilishidan oldin Kura aniq canonical
header va root-authenticated SCCP archive-ni immutable retained-block record-ga yozadi,
keyin exact V2 artifact-ni alohida immutable finality record-da saqlaydi. Ikkala yozuv
ham idempotent bo'lib, shu height-dagi ziddiyatni rad etadi. `build_finality_proof` faqat
retained header va verified finality record-ni o'qiydi; historical block body yoki
mutable world state PoP-ini hech qachon ishlatmaydi. Restart paytida
header/archive/artifact/hash association qayta tekshiriladi. Body eviction to'g'ri
proof-ni yo'qotmaydi; yo'qolgan, buzilgan, ziddiyatli yoki tekshirilmaydigan record fail closed.

Stateless tekshiruvchi version, chain, height, header hash, header-ning canonical predecessor-i
va view-i, context, subject va CommitQC-ni aniq moslashtiradi hamda artefaktdagi barcha
PoP-larni tekshiradi. Imzolovchi indekslar
qat'iy o'suvchi va diapazonda bo'lishi kerak. CommitQC ham validator soni, ham ovoz kuchi
quorumini bajarishi, aniq Sumeragi v2 vote preimage ustidagi BLS aggregate signature esa
to'g'ri bo'lishi shart.

## Ishonch langari va ketma-ket tekshirish

Alohida isbot faqat o'zi olib kelgan roster ostidagi ichki izchillikni ko'rsatadi.
`BridgeFinalityVerifier` birinchi isbotdan oldin aniq ishonilgan `HeightContextId` ni talab
qiladi. So'ng faqat darhol keyingi balandlikni qabul qiladi va bola context-dagi parent
CommitQC-ni oldingi muzlatilgan roster va PoP bilan tekshiradi. Epoch ichida bola artifact
oldingi artifact PoP-larini ko'chiradi; chegarada epoch, roster, quorum, seed va PoP oldingi
ota context-da CommitQC autentifikatsiya qilgan `next_epoch_snapshot` ga, shu jumladan
`epoch_end_height` ga mos bo'lishi kerak. Eski, o'tkazib yuborilgan va bog'lanmagan
balandliklar rad etiladi.

SCCP xuddi shu `BridgeFinalityProof` dan foydalanadi. Xabar bergan roster ostidagi imzoning
o'zi yetarli ishonch emas; governance bilan mahkamlangan checkpoint context/artefaktidan
xabar artefaktigacha har bir bevosita successor tekshirilishi kerak.

## Bundle va API

`BridgeFinalityBundle` aynan `{ commitment, finality_proof }` ko'rinishida. Commitment:
`{ chain_id, height_context_id, block_height, block_hash }`.

- `GET /v1/bridge/finality/{height}` `BridgeFinalityProof` qaytaradi;
- `GET /v1/bridge/finality/bundle/{height}` `BridgeFinalityBundle` qaytaradi.

Retained canonical header yoki aniq doimiy v2 artefakt yo'q yoki yaroqsiz bo'lsa, ikkala
endpoint ham yopiq tarzda muvaffaqiyatsiz tugaydi. Historical block body eviction to'g'ri
proof-ni yo'qotmaydi. Noma'lum maydonlar, qo'llanmaydigan versiyalar va iste'foga chiqarilgan
isbot shakllari rad etilishi kerak.
