---
lang: uz
direction: ltr
source: docs/portal/docs/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e153602cfb465bd5f65bab0cf97c44604bba982a7a7f1edc8d5af8fd67a9e29
source_last_modified: "2026-01-22T16:26:46.504508+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito Boshlash

Ushbu tezkor qo'llanma Kotodama shartnomasini tuzish uchun minimal ish jarayonini ko'rsatadi,
yaratilgan Norito bayt kodini tekshirish, uni mahalliy ishga tushirish va joylashtirish
Iroha tuguniga.

## Old shartlar

1. Rust asboblar zanjirini (1.76 yoki undan yangiroq) o'rnating va ushbu omborni tekshiring.
2. Qo'llab-quvvatlovchi ikkilik fayllarni yarating yoki yuklab oling:
   - `koto_compile` – Kotodama kompilyatori, IVM/Norito bayt kodini chiqaradi
   - `ivm_run` va `ivm_tool` - mahalliy ijro va tekshirish yordamchi dasturlari
   - `iroha_cli` - Torii orqali shartnomani joylashtirish uchun foydalaniladi

   Makefile ombori bu ikkilik fayllarni `PATH` da kutadi. Siz ham qila olasiz
   oldindan qurilgan artefaktlarni yuklab oling yoki ularni manbadan yarating. Agar siz kompilyatsiya qilsangiz
   asboblar zanjirini mahalliy sifatida ishlating, Makefile yordamchilarini ikkiliklarga yo'naltiring:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. O'rnatish bosqichiga yetganingizda Iroha tugunining ishlayotganligiga ishonch hosil qiling. The
   Quyidagi misollar Torii ga sizning sahifangizda sozlangan URL orqali kirish mumkinligini taxmin qiladi.
   `iroha_cli` profili (`~/.config/iroha/cli.toml`).

## 1. Kotodama shartnomasini tuzing

Ombor minimal "salom dunyo" shartnomasini yuboradi
`examples/hello/hello.ko`. Uni Norito/IVM bayt kodiga (`.to`) kompilyatsiya qiling:

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Asosiy bayroqlar:

- `--abi 1` shartnomani ABI 1-versiyasiga qulflaydi (qo'llab-quvvatlanadigan yagona versiya:
  yozish vaqti).
- `--max-cycles 0` cheksiz bajarilishini so'raydi; cheklash uchun ijobiy raqamni o'rnating
  nol-bilim isbotlari uchun tsikl to'ldirish.

## 2. Norito artefaktini tekshiring (ixtiyoriy)

Sarlavha va oʻrnatilgan metamaʼlumotlarni tekshirish uchun `ivm_tool` dan foydalaning:

```sh
ivm_tool inspect target/examples/hello.to
```

Siz ABI versiyasini, yoqilgan xususiyat bayroqlarini va eksport qilingan yozuvni ko'rishingiz kerak
ball. Bu joylashtirishdan oldin tezkor aqlni tekshirish.

## 3. Shartnomani mahalliy sifatida boshqaring

Harakatni tasdiqlash uchun `ivm_run` bilan bayt kodini bajaring.
tugun:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` misoli salomlashishni qayd qiladi va `SET_ACCOUNT_DETAIL` tizim chaqiruvini chiqaradi.
Nashr qilishdan oldin kontrakt mantig'i bo'yicha takrorlashda mahalliy sifatida ishlash foydalidir
u zanjirda.

## 4. `iroha_cli` orqali tarqatish

Shartnomadan qoniqsangiz, uni CLI yordamida tugunga joylashtiring.
Vakolat hisobini, uning imzo kalitini va `.to` faylini yoki
Base64 foydali yuk:

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

Buyruq Norito manifest + bayt-kod paketini Torii orqali yuboradi va chop etadi
natijada tranzaksiya holati. Tranzaktsiya amalga oshirilgandan so'ng, kod
Javobda ko'rsatilgan xesh manifestlarni yoki ro'yxat misollarini olish uchun ishlatilishi mumkin:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii ga qarshi ishlang

Ro'yxatdan o'tgan bayt-kod bilan siz ko'rsatma yuborish orqali uni chaqirishingiz mumkin
Bu saqlangan kodga murojaat qiladi (masalan, `iroha_cli ledger transaction submit` orqali
yoki dastur mijozingiz). Hisob ruxsatnomalari kerakli ruxsat berganligiga ishonch hosil qiling
tizim chaqiruvlari (`set_account_detail`, `transfer_asset` va boshqalar).

## Maslahatlar va muammolarni bartaraf etish

- Taqdim etilgan misollarni kompilyatsiya qilish va bajarish uchun `make examples-run` dan foydalaning.
  otish. Ikkilik fayllar yoqilmagan bo'lsa, `KOTO`/`IVM` muhit o'zgaruvchilarini bekor qiling
  `PATH`.
- Agar `koto_compile` ABI versiyasini rad etsa, kompilyator va tugunni tekshiring.
  ikkalasi ham maqsadli ABI v1 (`koto_compile --abi` ro'yxat uchun argumentlarsiz ishga tushiring)
  qo'llab-quvvatlash).
- CLI hex yoki Base64 imzo kalitlarini qabul qiladi. Sinov uchun siz foydalanishingiz mumkin
  `iroha_cli tools crypto keypair` tomonidan chiqarilgan kalitlar.
- Norito foydali yuklarni tuzatishda `ivm_tool disassemble` kichik buyrug'i yordam beradi
  ko'rsatmalarni Kotodama manbasi bilan bog'lang.

Ushbu oqim CI va integratsiya testlarida qo'llaniladigan bosqichlarni aks ettiradi. Chuqurroq uchun
Kotodama grammatikasiga, tizim diagrammalariga va Norito ichki qismlariga o'ting, qarang:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`