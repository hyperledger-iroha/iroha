---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c11fe861e7052b113b91249eb9e39adca67a3b3cc20acf497f0785e37498504c
source_last_modified: "2025-12-29T18:16:35.196700+00:00"
translation_last_reviewed: 2026-02-07
id: priority-snapshot-2025-03
title: Priority Snapshot — March 2025 (Beta)
description: Mirror of the 2025-03 Nexus steering snapshot; pending ACKs before public rollout.
translator: machine-google-reviewed
---

> Kanonik manba: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> Holat: **Beta / boshqaruv ACKs kutilmoqda** (Tarmoq, saqlash, Docs yetakchilari).

## Umumiy ko'rinish

Mart oyining surati hujjatlar/kontent-tarmoq tashabbuslarini
SoraFS yetkazib berish treklari (SF‑3, SF‑6b, SF‑9). Bir marta barcha etakchilar buni tan olishadi
Nexus boshqaruv kanalidagi surat, yuqoridagi “Beta” yozuvini olib tashlang.

### Diqqat mavzular

1. **Asosiy suratni tarqatish** — tashakkurnomalarni to‘plang va ularni tizimga kiriting.
   03.05.2025 Kengash bayonnomasi.
2. **Gateway/DNS start-out yopilishi** — yangi yordam to'plamini mashq qiling (6-bo'lim).
   runbookda) 2025-03-03 seminardan oldin.
3. **Operator runbook migration** — `Runbook Index` portali jonli; beta-ni ochib berish
   ko'rib chiquvchi ro'yxatdan o'tgandan so'ng URL manzilini oldindan ko'rish.
4. **SoraFS yetkazib berish iplari** — SF‑3/6b/9 qolgan ishni reja/yo‘l xaritasi bilan tekislang:
   - `sorafs-node` PoR qabul qilish ishchisi + oxirgi holat.
   - Rust/JS/Swift orkestr integratsiyasi bo'ylab CLI/SDK bog'lovchi jilo.
   - PoR koordinatori ish vaqti simlari va GovernanceLog voqealari.

To'liq jadval, tarqatish nazorat ro'yxati va jurnal yozuvlari uchun manba faylga qarang.