---
lang: uz
direction: ltr
source: docs/source/examples/smt_update.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 788902cfafc6c7db6d52d4237b46ffe78193efd57852bc3427a16d7f3cda2f9c
source_last_modified: "2025-12-29T18:16:35.954370+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sparse Merkle yangilash misoli

Ushbu ishlangan misol FASTPQ Stage 2 izi qanday kodlashini ko'rsatadi
a'zo bo'lmagan guvoh `neighbour_leaf` ustunidan foydalangan holda. Siyrak Merkle daraxti
Poseidon2 maydon elementlari ustida ikkilik hisoblanadi. Kalitlar kanonikga aylantiriladi
32 baytli kichik endian satrlar, maydon elementiga xeshlangan va eng ko'p
muhim bitlar har bir darajadagi filialni tanlaydi.

## Ssenariy

- Oldindan davlat barglari
  - `asset::alice::rose` -> `0x0000_0000_0000_0005` qiymatiga ega `0x12b7...` xeshlangan kalit.
  - `asset::bob::rose` -> xeshlangan kalit `0x1321...` qiymati `0x0000_0000_0000_0003`.
- Yangilash so'rovi: 2 qiymati bilan `asset::carol::rose` kiriting.
- Kerol uchun kanonik kalit xeshi 5 bitli `0b01011` prefiksiga kengayadi. The
  mavjud qo'shnilarda `0b01010` (Alice) va `0b01101` (Bob) prefikslari mavjud.

Prefiksi `0b01011` ga to'g'ri keladigan barg yo'qligi sababli, prover buni taqdim etishi kerak.
`(alice, bob)` oralig'i bo'sh ekanligiga qo'shimcha dalil. 2-bosqich to'planadi
`path_bit_{level}`, `sibling_{level}` ustunlaridagi kuzatuv qatori,
`node_in_{level}` va `node_out_{level}` (`level` `[0, 31]` da). Barcha qadriyatlar
Litsenziya shaklida kodlangan Poseidon2 maydon elementlari:

| darajasi | `path_bit_level` | `sibling_level` | `node_in_level` | `node_out_level` | Eslatmalar |
| ----- | ---------------- | ----------------------------------- | ----------------------------------- | ----------------------------------- | ----- |
| 0 | 1 | `0x241f...` (Alis barglari xeshi) | `0x0000...` | `0x4b12...` (`value_2 = 2`) | Qo'shish: noldan boshlang, yangi qiymatni saqlang. |
| 1 | 1 | `0x7d45...` (bo'sh o'ng) | Poseidon2(`node_out_0`, `sibling_0`) | Poseidon2(`sibling_1`, `node_out_1`) | 1-bit prefiksini kuzatib boring. |
| 2 | 0 | `0x03ae...` (Bob filiali) | Poseidon2(`node_out_1`, `sibling_1`) | Poseidon2(`node_in_2`, `sibling_2`) | Filial buriladi, chunki bit = 0. |
| 3 | 1 | `0x9bc4...` | Poseidon2(`node_out_2`, `sibling_2`) | Poseidon2(`sibling_3`, `node_out_3`) | Yuqori darajalar yuqoriga qarab xeshingni davom ettiradi. |
| 4 | 0 | `0xe112...` | Poseidon2(`node_out_3`, `sibling_3`) | Poseidon2(`node_in_4`, `sibling_4`) | Ildiz darajasi; natija post-davlat ildizidir. |

Bu qator uchun `neighbour_leaf` ustuni Bob bargi bilan to'ldirilgan
(`key = 0x1321...`, `value = 3`, `hash = Poseidon2(key, value) = 0x03ae...`). Qachon
tekshirganda, AIR quyidagilarni tekshiradi:

1. Taqdim etilgan qo'shni 2-darajada qo'llaniladigan birodarga mos keladi.
2. Qo‘shni kalit leksikografik jihatdan kiritilgan kalitdan katta va
   chap qardosh (Alis) leksikografik jihatdan kichikroq.
3. Kiritilgan bargni qo'shni bilan almashtirish oldingi holat ildizini qayta ishlab chiqaradi.Bu tekshiruvlar birgalikda `(0b01010,) oralig'ida barg mavjud emasligini isbotlaydi.
0b01101)` yangilanishdan oldin. FASTPQ izlarini yaratuvchi dasturlardan foydalanish mumkin
bu tartib so'zma-so'z; yuqoridagi raqamli konstantalar tasviriydir. To'liq uchun
JSON guvohi, ustunlarni yuqoridagi jadvalda ko'rsatilganidek chiqaring (bilan
bilan ketma-ketlashtirilgan kichik endian bayt satrlaridan foydalangan holda, har bir daraja uchun raqamli qo'shimchalar
Norito JSON yordamchilari.