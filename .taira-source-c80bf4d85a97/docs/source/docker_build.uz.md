---
lang: uz
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker quruvchi tasviri

Bu konteyner `Dockerfile.build` da belgilangan va barcha asboblar zanjirini birlashtiradi
CI va mahalliy relizlar uchun zarur bo'lgan bog'liqliklar. Tasvir endi a sifatida ishlaydi
sukut bo'yicha root bo'lmagan foydalanuvchi, shuning uchun Git operatsiyalari Arch Linux bilan ishlashda davom etadi
`safe.directory` global yechimiga murojaat qilmasdan `libgit2` paketi.

## Argumentlar yarating

- `BUILDER_USER` – konteyner ichida yaratilgan login nomi (standart: `iroha`).
- `BUILDER_UID` – raqamli foydalanuvchi identifikatori (standart: `1000`).
- `BUILDER_GID` – asosiy guruh identifikatori (standart: `1000`).

Ish maydonini xostingizdan ulaganingizda, mos keladigan UID/GID qiymatlarini o'tkazing
yaratilgan artefaktlar yozilishi mumkin bo'lib qoladi:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Asboblar zanjiri kataloglari (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`)
konfiguratsiya qilingan foydalanuvchiga tegishli, shuning uchun Yuk, rustup va Poetry buyruqlari to'liq qoladi
konteyner ildiz huquqlaridan mahrum bo'lgandan keyin ishlaydi.

## Ishlayotgan tuzilmalar

Ish joyini `/workspace` (`WORKDIR` konteyneri) ga ulang.
tasvir. Misol:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

Tasvir `docker` guruhiga a'zolikni saqlaydi, shuning uchun ichki Docker buyruqlari (masalan,
`docker buildx bake`) xost PID-ni o'rnatadigan CI ish oqimlari uchun mavjud bo'lib qoladi
va rozetka. Atrofingiz uchun kerak bo'lganda guruh xaritalarini sozlang.

## Iroha 2 va Iroha 3 ta artefakt

Ish maydoni endi to'qnashuvlarni oldini olish uchun har bir reliz qatoriga alohida ikkiliklarni chiqaradi:
`iroha3`/`iroha3d` (standart) va `iroha2`/`iroha2d` (Iroha 2). Buning uchun yordamchilardan foydalaning
kerakli juftlikni hosil qiling:

- Iroha 3 uchun `make build` (yoki `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`)
- Iroha 2 uchun `make build-i2` (yoki `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`)

Selektor xususiyatlar to'plamini (`telemetry` + `schema-endpoint` va
qatorga xos `build-i{2,3}` bayrog'i) shuning uchun Iroha 2 ta tuzilmalarni tasodifan ololmaydi
Iroha faqat 3 ta standart.

`scripts/build_release_bundle.sh` orqali tuzilgan to'plamlar to'g'ri ikkilikni tanlang
`--profile` `iroha2` yoki `iroha3` ga sozlanganda avtomatik ravishda nomlar.