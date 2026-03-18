---
lang: uz
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f6421d420a704c5c4af335741e309adf641702ddb8c291dce94ea5581557a66
source_last_modified: "2025-12-29T18:16:35.953884+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Katta mahsulot namunasini qidiring

Ushbu misolda aytib o'tilgan FASTPQ ruxsat izlash argumenti kengaytiriladi
`fastpq_plan.md`.  2-bosqich quvur liniyasida prover selektorni baholaydi
(`s_perm`) va guvoh (`perm_hash`) ustunlari past darajali kengaytmada (LDE)
domen, ishlaydigan yirik mahsulot `Z_i` ni yangilaydi va nihoyat butun
Poseidon bilan ketma-ketlik.  Xeshlangan akkumulyator transkriptga qo'shiladi
`fastpq:v1:lookup:product` domeni ostida, oxirgi `Z_i` hali ham mos keladi
ruxsat etilgan jadval mahsuloti `T`.

Biz quyidagi selektor qiymatlari bilan kichik partiyani ko'rib chiqamiz:

| qator | `s_perm` | `perm_hash` |
| --- | -------- | ------------------------------------------------------- |
| 0 | 1 | `0x019a...` (grant roli = auditor, ruxsat = transfer_asset) |
| 1 | 0 | `0xabcd...` (ruxsat o'zgarmaydi) |
| 2 | 1 | `0x42ff...` (rolni bekor qilish = auditor, perm = burn_asset) |

`gamma = 0xdead...` Fiat-Shamir qidiruv muammosi bo'lsin.
transkript.  Prover `Z_0 = 1` ni ishga tushiradi va har bir qatorni katlaydi:

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = 1 * (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1 (selector is zero)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

`s_perm = 0` akkumulyatorni o'zgartirmaydigan qatorlar.  Qayta ishlashdan keyin
Trace, Prover Poseidon transkript uchun `[Z_1, Z_2, ...]` ketma-ketligini xeshlaydi.
hali ham jadvalga mos keladigan `Z_final = Z_3` (yakuniy ishlaydigan mahsulot) nashr etadi
chegara holati.

Jadval tomonida, berilgan ruxsat Merkle daraxti deterministikni kodlaydi
Slot uchun faol ruxsatlar to'plami.  Tekshiruvchi (yoki prover davomida
guvohlar avlodi) hisoblaydi

```
T = product over entries: (entry.hash + gamma)
```

Protokol `Z_final / T = 1` chegara cheklovini amalga oshiradi.  Agar iz
jadvalda mavjud bo'lmagan ruxsatnomani kiritdi (yoki uni o'tkazib yubordi
bo'lsa), asosiy mahsulot nisbati 1 dan farq qiladi va tekshirgich rad etadi.  Chunki
ikkala tomon Goldilocks maydonida `(value + gamma)` ga ko'paytiriladi, nisbat
CPU/GPU backendlarida barqaror bo'lib qoladi.

Misolni armatura uchun Norito JSON sifatida ketma-ketlashtirish uchun qatorni yozing.
Har bir qatordan keyin `perm_hash`, selektor va akkumulyator, masalan:

```json
{
  "gamma": "0xdead...",
  "rows": [
    {"s_perm": 1, "perm_hash": "0x019a...", "z_after": "0x5f10..."},
    {"s_perm": 0, "perm_hash": "0xabcd...", "z_after": "0x5f10..."},
    {"s_perm": 1, "perm_hash": "0x42ff...", "z_after": "0x9a77..."}
  ],
  "table_product": "0x9a77..."
}
```

O'n oltilik to'ldirgichlar (`0x...`) beton Goldilocks bilan almashtirilishi mumkin
avtomatlashtirilgan testlarni yaratishda maydon elementlari.  2-bosqich armatura qo'shimcha ravishda
ishlaydigan akkumulyatorning Poseidon xeshini yozib oling, lekin bir xil JSON shaklini saqlang,
shuning uchun misol kelajakdagi test vektorlari uchun shablon sifatida ikki baravar ko'payishi mumkin.