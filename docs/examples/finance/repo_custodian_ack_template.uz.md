---
lang: uz
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c52d7f2c5ec9dc4cda81895561bc1261659935c94bf3f7febb0867f4981fe616
source_last_modified: "2026-01-22T16:26:46.472177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo saqlovchini tasdiqlovchi shabloni

Repo (ikki tomonlama yoki uch tomonlama) vasiyga murojaat qilganda ushbu shablondan foydalaning
`RepoAgreement::custodian` orqali. Maqsad - qamoqda saqlash SLA, marshrutni yozib olish
hisoblar va aktivlarni ko'chirishdan oldin kontaktlarni burg'ulash. Shablonni o'zingizga nusxa ko'chiring
dalillar katalogi (masalan
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`), to'ldiring
to'ldirgichlarni o'rnating va faylni bo'limda tasvirlangan boshqaruv paketining bir qismi sifatida xeshlang
`docs/source/finance/repo_ops.md` §2.8.

## 1. Metadata

| Maydon | Qiymat |
|-------|-------|
| Shartnoma identifikatori | `<repo-yyMMdd-XX>` |
| Saqlovchi hisob identifikatori | `<ih58...>` |
| Tayyorlangan / sana | `<custodian ops lead>` |
| Ish stolidagi kontaktlar tan olingan | `<desk lead + counterparty>` |
| Dalillar katalogi | ``artifacts/finance/repo/<slug>/`` |

## 2. Saqlash doirasi

- **Olingan garov ta'riflari:** `<list of asset definition ids>`
- **Naqd pul birligi / hisob-kitob temir yo'li:** `<xor#sora / other>`
- **Qamoqqa olish oynasi:** `<start/end timestamps or SLA summary>`
- **Doimiy ko'rsatmalar:** `<hash + path to standing instruction document>`
- **Avtomatlashtirish uchun zaruriy shartlar:** `<scripts, configs, or runbooks custodian will invoke>`

## 3. Marshrutlash va monitoring

| Element | Qiymat |
|------|-------|
| Saqlash hamyoni / buxgalteriya hisobi | `<asset ids or ledger path>` |
| Monitoring kanali | `<Slack/phone/on-call rotation>` |
| Matkap kontakti | `<primary + backup>` |
| Majburiy ogohlantirishlar | `<PagerDuty service, Grafana board, etc.>` |

## 4. Bayonotlar

1. *Qamoqqa olishga tayyorlik:* “Biz bosqichma-bosqich `repo initiate` yukini ko‘rib chiqdik.
   yuqoridagi identifikatorlar va sanab o'tilgan SLA bo'yicha garovni qabul qilishga tayyor
   §2da.”
2. *Orqaga qaytarish majburiyati:* “Agar biz yuqorida ko‘rsatilgan qayta tiklash kitobini bajaramiz, agar
   voqea qo'mondoni tomonidan boshqariladi va CLI jurnallari va xeshlarni taqdim etadi
   `governance/drills/<timestamp>.log`.”
3. *Dalillarni saqlash:* “Biz tan olgan holda turamiz
   kamida `<duration>` uchun ko'rsatmalar va CLI jurnallari va ularni taqdim eting
   so'rov bo'yicha moliya kengashi."

Quyida imzo qo'ying (elektron imzolar boshqaruv orqali yuborilganda qabul qilinadi
kuzatuvchi).

| Ism | Rol | Imzo / sana |
|------|------|-----------------|
| `<custodian ops lead>` | Kastodiy operator | `<signature>` |
| `<desk lead>` | Stol | `<signature>` |
| `<counterparty>` | Qarama-qarshi tomon | `<signature>` |

> Imzolangandan so'ng faylni xeshlang (masalan: `sha256sum custodian_ack_<cust>.md`) va
> boshqaruv paketlari jadvaliga dayjestni yozib oling, shunda sharhlovchilar buni tekshirishlari mumkin
> ovoz berish paytida havola qilingan tasdiq baytlari.