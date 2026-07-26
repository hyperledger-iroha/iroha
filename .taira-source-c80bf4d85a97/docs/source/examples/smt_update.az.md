---
lang: az
direction: ltr
source: docs/source/examples/smt_update.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 788902cfafc6c7db6d52d4237b46ffe78193efd57852bc3427a16d7f3cda2f9c
source_last_modified: "2025-12-29T18:16:35.954370+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Seyrək Merkle Yeniləmə Nümunəsi

Bu işlənmiş nümunə FASTPQ Mərhələ 2 izinin necə kodladığını göstərir a
`neighbour_leaf` sütunundan istifadə edərək üzv olmayan şahid. Seyrək Merkle ağacı
Poseidon2 sahə elementləri üzərində binardır. Açarlar kanonikə çevrilir
32 baytlıq kiçik endian sətirlər, sahə elementinə hashed və ən çoxu
significant bits select the branch at each level.

## Ssenari

- Əvvəlcədən yarpaqlar
  - `asset::alice::rose` -> `0x0000_0000_0000_0005` dəyəri ilə hashed açarı `0x12b7...`.
  - `asset::bob::rose` -> `0x0000_0000_0000_0003` dəyəri ilə hashed açarı `0x1321...`.
- Yeniləmə sorğusu: 2 dəyəri ilə `asset::carol::rose` daxil edin.
- Carol üçün kanonik açar hash 5 bitlik `0b01011` prefiksinə qədər genişlənir. The
  mövcud qonşuların `0b01010` (Alice) və `0b01101` (Bob) prefiksləri var.

Prefiksi `0b01011` ilə uyğun gələn heç bir yarpaq olmadığı üçün prover təmin etməlidir.
`(alice, bob)` intervalının boş olduğuna dair əlavə sübut. Mərhələ 2 doldurulur
`path_bit_{level}`, `sibling_{level}` sütunları arasında izləmə sırası,
`node_in_{level}` və `node_out_{level}` (`level` ilə `[0, 31]`). Bütün dəyərlər
kiçik endian formasında kodlanmış Poseidon2 sahə elementləridir:

| səviyyə | `path_bit_level` | `sibling_level` | `node_in_level` | `node_out_level` | Qeydlər |
| ----- | ---------------- | ----------------------------------- | ----------------------------------- | ----------------------------------- | ----- |
| 0 | 1 | `0x241f...` (Alice yarpağı hash) | `0x0000...` | `0x4b12...` (`value_2 = 2`) | Daxil edin: sıfırdan başlayın, yeni dəyəri saxlayın. |
| 1 | 1 | `0x7d45...` (boş sağ) | Poseidon2(`node_out_0`, `sibling_0`) | Poseidon2(`sibling_1`, `node_out_1`) | Prefiks bit 1-i izləyin. |
| 2 | 0 | `0x03ae...` (Bob filialı) | Poseidon2(`node_out_1`, `sibling_1`) | Poseidon2(`node_in_2`, `sibling_2`) | Filial fırlanır, çünki bit = 0. |
| 3 | 1 | `0x9bc4...` | Poseidon2(`node_out_2`, `sibling_2`) | Poseidon2(`sibling_3`, `node_out_3`) | Daha yüksək səviyyələr yuxarıya doğru hashing davam edir. |
| 4 | 0 | `0xe112...` | Poseidon2(`node_out_3`, `sibling_3`) | Poseidon2(`node_in_4`, `sibling_4`) | Kök səviyyəsi; nəticə post-dövlət köküdür. |

Bu sıra üçün `neighbour_leaf` sütunu Bob yarpağı ilə doldurulub
(`key = 0x1321...`, `value = 3`, `hash = Poseidon2(key, value) = 0x03ae...`). Nə vaxt
yoxlayaraq, AIR aşağıdakıları yoxlayır:

1. Təchiz edilən qonşu 2-ci səviyyədə istifadə edilən bacı-qardaşa uyğun gəlir.
2. Qonşu açar leksikoqrafik cəhətdən daxil edilmiş açardan və açardan böyükdür
   sol bacı (Alice) leksikoqrafik cəhətdən daha kiçikdir.
3. Daxil edilmiş yarpağı qonşu ilə əvəz etmək əvvəlcədən dövlət kökünü çoxaldır.Bu yoxlamalar birlikdə sübut edir ki, `(0b01010,) intervalı üçün heç bir yarpaq mövcud deyil.
0b01101)` yeniləmədən əvvəl. FASTPQ izlərini yaradan tətbiqlər istifadə edə bilər
bu layout sözbəsöz; yuxarıdakı ədədi sabitlər illüstrativdir. Tam üçün
JSON şahidi, sütunları yuxarıdakı cədvəldə göründüyü kimi buraxın (ile
ilə seriallaşdırılmış kiçik endian bayt sətirlərindən istifadə edərək, hər səviyyə üçün rəqəmli şəkilçilər
Norito JSON köməkçiləri.