---
slug: /norito/quickstart
lang: uz
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Quickstart
description: Build, validate, and deploy a Kotodama contract with the release tooling and default single-peer network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Ushbu ko'rsatma ishlab chiquvchilar o'rganishda amal qilishini kutgan ish jarayonini aks ettiradi
Norito va Kotodama birinchi marta: deterministik yagona teng tarmoqni yuklash,
shartnoma tuzing, uni mahalliy sifatida quriting, keyin uni Torii orqali yuboring.
CLIga havola.

Shartnoma namunasi qo'ng'iroq qiluvchining hisobiga kalit/qiymat juftligini yozadi, shuning uchun siz qila olasiz
yon ta'sirini darhol `iroha_cli` bilan tekshiring.

## Old shartlar

- [Docker](https://docs.docker.com/engine/install/) Compose V2 yoqilgan (ishlatilgan)
  `defaults/docker-compose.single.yml` da belgilangan namunaviy tengdoshni ishga tushirish uchun).
- Yuklab olmasangiz yordamchi ikkilik fayllarni yaratish uchun Rust asboblar zanjiri (1.76+).
  nashr etilganlar.
- `koto_compile`, `ivm_run` va `iroha_cli` ikkilik. Siz ularni dan qurishingiz mumkin
  quyida ko'rsatilgandek ish joyini tekshirish yoki mos keladigan reliz artefaktlarini yuklab oling:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Yuqoridagi ikkilik fayllarni ish maydonining qolgan qismi bilan birga o'rnatish xavfsiz.
> Ular hech qachon `serde`/`serde_json` ga ulanmaydi; Norito kodeklari oxirigacha qo'llaniladi.

## 1. Yagona peerli ishlab chiquvchi tarmoqni ishga tushiring

Repozitoriyga `kagami swarm` tomonidan yaratilgan Docker Compose toʻplami kiradi.
(`defaults/docker-compose.single.yml`). Bu standart genezis, mijoz simlar
Torii konfiguratsiyasi va sog'liq problari uchun `http://127.0.0.1:8080` manzilidan foydalanish mumkin.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Idishni ishlayotgan holda qoldiring (oldingi yoki ajratilgan holda). Hammasi
keyingi CLI qo'ng'iroqlari `defaults/client.toml` orqali ushbu tengdoshni nishonga oladi.

## 2. Shartnoma muallifi

Ishchi katalog yarating va minimal Kotodama misolini saqlang:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Versiya boshqaruvida Kotodama manbalarini saqlash afzalroq. Portalda joylashgan misollar
> agar siz [Norito misollar galereyasi](./examples/) ostida ham mavjud
> boyroq boshlanish nuqtasini xohlaysiz.

## 3. IVM bilan kompilyatsiya qiling va quruq ishga tushiring

Shartnomani IVM/Norito bayt kodiga (`.to`) tuzing va uni mahalliy sifatida bajaring.
tarmoqqa tegmasdan oldin xost tizimi qo'ng'iroqlari muvaffaqiyatli ekanligini tasdiqlang:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Yuguruvchi `info("Hello from Kotodama")` jurnalini chop etadi va bajaradi
Masxara qilingan xostga qarshi `SET_ACCOUNT_DETAIL` tizimi. Agar ixtiyoriy `ivm_tool`
ikkilik mavjud, `ivm_tool inspect target/quickstart/hello.to` ni ko'rsatadi
ABI sarlavhasi, xususiyat bitlari va eksport qilingan kirish nuqtalari.

## 4. Torii orqali bayt kodini yuboring

Tugun ishlayotgan holda, kompilyatsiya qilingan baytekodni CLI yordamida Torii ga yuboring.
Standart ishlab chiqish identifikatori ochiq kalitdan olingan
`defaults/client.toml`, shuning uchun hisob identifikatori
```
i105...
```

Torii URL manzili, zanjir identifikatori va imzo kalitini taʼminlash uchun konfiguratsiya faylidan foydalaning:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI tranzaktsiyani Norito bilan kodlaydi, uni dev kaliti bilan imzolaydi va
uni yugurayotgan tengdoshiga topshiradi. `set_account_detail` uchun Docker jurnallarini tomosha qiling
syscall yoki bajarilgan tranzaksiya xesh uchun CLI chiqishini kuzatib boring.

## 5. Holat o'zgarishini tekshiring

Shartnomada yozilgan hisob ma'lumotlarini olish uchun bir xil CLI profilidan foydalaning:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

Siz Norito tomonidan qo'llab-quvvatlanadigan JSON foydali yukini ko'rishingiz kerak:

```json
{
  "hello": "world"
}
```

Agar qiymat etishmayotgan boʻlsa, Docker yozish xizmati hali ham ishlayotganligini tasdiqlang.
ishlayotgani va `iroha` tomonidan xabar qilingan tranzaksiya xeshi `Committed` ga yetganligi
davlat.

## Keyingi qadamlar

- Koʻrish uchun avtomatik yaratilgan [misol galereyasi](./examples/) bilan tanishing
  Norito tizim chaqiruvlariga qanchalik rivojlangan Kotodama parchalari xaritasi.
- Chuqurroq ma'lumot olish uchun [Norito ishga tushirish qo'llanmasini](./getting-started) o'qing.
  kompilyator/yuguruvchi vositalarini tushuntirish, manifestni joylashtirish va IVM
  metadata.
- O'zingizning shartnomalaringizni takrorlashda `npm run sync-norito-snippets` dan foydalaning
  portal hujjatlari va artefaktlar qolishi uchun yuklab olinadigan parchalarni qayta tiklash uchun ish maydoni
  `crates/ivm/docs/examples/` ostidagi manbalar bilan sinxronlashtirilgan.