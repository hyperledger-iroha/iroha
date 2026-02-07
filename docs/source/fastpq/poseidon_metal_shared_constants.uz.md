---
lang: uz
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2025-12-29T18:16:35.955568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Poseidon metallining umumiy konstantalari

Metall yadrolari, CUDA yadrolari, Rust proveri va har bir SDK moslamalari birlashishi kerak
Uskunani tezlashtirish uchun bir xil Poseidon2 parametrlari
xeshlash deterministik. Ushbu hujjat kanonik suratni, qanday qilib yozadi
uni qayta tiklash va GPU quvurlari ma'lumotlarni qanday qabul qilishi kutilmoqda.

## Snapshot manifest

Parametrlar `PoseidonSnapshot` RON hujjati sifatida nashr etilgan. Nusxalari
versiya nazorati ostida saqlanadi, shuning uchun GPU asboblar zanjiri va SDKlar qurish vaqtiga tayanmaydi
kod ishlab chiqarish.

| Yo'l | Maqsad | SHA-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` | `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}` dan yaratilgan kanonik surat; GPU qurish uchun haqiqat manbai. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | Kanonik suratni aks ettiradi, shuning uchun Swift birligi sinovlari va XCFramework tutun simi Metall yadrolari kutgan konstantalarni yuklaydi. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Android/Kotlin moslamalari paritet va serializatsiya testlari uchun bir xil manifestga ega. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

Har bir iste'molchi doimiylarni GPUga ulashdan oldin xeshni tekshirishi kerak
quvur liniyasi. Manifest o'zgarganda (yangi parametrlar to'plami yoki profil), SHA va
quyi oqim nometalllarini qulflash bosqichida yangilash kerak.

## Qayta tiklash

Manifest Rust manbalaridan `xtask` yordamida yaratilgan.
yordamchi. Buyruq ham kanonik faylni, ham SDK oynalarini yozadi:

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

Belgilangan joylarni bekor qilish uchun `--constants <path>`/`--vectors <path>` dan foydalaning yoki
`--no-sdk-mirror` faqat kanonik suratni qayta tiklashda. Yordamchi qiladi
bayroq tushirilganda artefaktlarni Swift va Android daraxtlariga aks ettiring,
bu xeshlarni CI uchun hizalangan holda saqlaydi.

## Metall/CUDA konstruktsiyalarini oziqlantirish

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` va
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` dan qayta tiklanishi kerak
  jadval o'zgarganda namoyon bo'ladi.
- Yaxlitlangan va MDS konstantalari ulashgan `MTLBuffer`/`__constant` ga joylashtirilgan.
  manifest tartibiga mos keladigan segmentlar: `round_constants[round][state_width]`
  keyin 3x3 MDS matritsasi.
- `fastpq_prover::poseidon_manifest()` suratni yuklaydi va tasdiqlaydi
  ish vaqti (Metalni isitish vaqtida), shuning uchun diagnostika asboblari buni tasdiqlashi mumkin
  shader konstantalari orqali chop etilgan xeshga mos keladi
  `fastpq_prover::poseidon_manifest_sha256()`.
- SDK armatura o'quvchilari (Swift `PoseidonSnapshot`, Android `PoseidonSnapshot`) va
  Norito oflayn vositalari xuddi shu manifestga tayanadi, bu faqat GPU-ni oldini oladi
  parametr vilkalari.

## Tasdiqlash

1. Manifestni qayta tiklagandan so'ng, mashq qilish uchun `cargo test -p xtask` ni ishga tushiring.
   Poseidon armatura ishlab chiqarish birligi sinovlari.
2. Yangi SHA-256 ni ushbu hujjatda va kuzatuvchi har qanday asboblar panelida yozib oling
   GPU artefaktlari.
3. `cargo test -p fastpq_prover poseidon_manifest_consistency` tahlil qiladi
   `poseidon2.metal` va `fastpq_cuda.cu` qurilish vaqtida va ularning
   Seriyalashtirilgan konstantalar CUDA/Metal jadvallarini saqlagan holda manifestga mos keladi va
   qulflash bosqichida kanonik surat.Manifestni GPU qurish ko'rsatmalari bilan birga saqlash Metal/CUDA ni beradi
ish oqimlari deterministik qo'l siqish: yadrolar xotirasini optimallashtirish uchun bepul
Agar ular umumiy konstantalarni qabul qilsalar va xeshni ochsalar, tartib
paritetlarni tekshirish uchun telemetriya.