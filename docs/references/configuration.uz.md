---
lang: uz
direction: ltr
source: docs/references/configuration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-12-29T18:16:35.913045+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Tezlashtirish

`[accel]` bo'limi IVM va yordamchilar uchun ixtiyoriy apparat tezlashuvini boshqaradi. Hammasi
tezlashtirilgan yo'llar deterministik protsessor zaxiralariga ega; a backend muvaffaqiyatsiz bo'lsa oltin
ish vaqtida o'z-o'zini sinab ko'rish u avtomatik ravishda o'chiriladi va CPUda bajarish davom etadi.

- `enable_cuda` (standart: rost) - CUDA kompilyatsiya qilingan va mavjud bo'lganda foydalaning.
- `enable_metal` (standart: rost) – Mavjud bo'lganda macOS-da Metalldan foydalaning.
- `max_gpus` (standart: 0) – ishga tushirish uchun maksimal GPUlar; `0` avtomatik/qopqoqsiz degan ma'noni anglatadi.
- `merkle_min_leaves_gpu` (standart: 8192) - Merkle yukini tushirish uchun minimal barglar
  GPUga barg xeshing. Faqat g'ayrioddiy tez GPUlar uchun pastroq.
- Kengaytirilgan (ixtiyoriy; odatda oqilona standartlarni meros qilib oladi):
  - `merkle_min_leaves_metal` (standart: meros `merkle_min_leaves_gpu`).
  - `merkle_min_leaves_cuda` (standart: meros `merkle_min_leaves_gpu`).
  - `prefer_cpu_sha2_max_leaves_aarch64` (standart: 32768) – SHA2 bilan ARMv8 da shuncha koʻp barggacha CPU SHA‑2 ni afzal koʻring.
  - `prefer_cpu_sha2_max_leaves_x86` (standart: 32768) – x86/x86_64 da shu qadar koʻp barggacha CPU SHA‑NI ni afzal koʻring.

Eslatmalar
- birinchi navbatda determinizm: tezlashuv hech qachon kuzatilishi mumkin bo'lgan natijalarni o'zgartirmaydi; orqa tomonlar
  init-da oltin testlarni o'tkazing va nomuvofiqliklar aniqlanganda skaler/SIMD ga qayting.
- `iroha_config` orqali sozlash; ishlab chiqarishda atrof-muhit o'zgaruvchilari oldini olish.