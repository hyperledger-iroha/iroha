---
lang: ur
direction: rtl
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-11-29T19:27:21.735343+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/docker_build.md -->

# Docker Builder امیج

یہ کنٹینر `Dockerfile.build` میں متعین ہے اور CI اور مقامی release builds کیلئے تمام toolchain
dependencies شامل کرتا ہے۔ اب یہ امیج ڈیفالٹ طور پر non-root صارف کے طور پر چلتی ہے، اس لئے
Arch Linux کے `libgit2` پیکیج کے ساتھ Git operations بغیر global `safe.directory` workaround
کے کام کرتی رہتی ہیں۔

## Build arguments

- `BUILDER_USER` - کنٹینر کے اندر بنایا جانے والا login نام (ڈیفالٹ: `iroha`).
- `BUILDER_UID` - عددی user id (ڈیفالٹ: `1000`).
- `BUILDER_GID` - بنیادی group id (ڈیفالٹ: `1000`).

جب آپ host سے workspace mount کریں تو مماثل UID/GID values دیں تاکہ generated artifacts لکھنے کے قابل رہیں:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Toolchain directories (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`) configured صارف کی ملکیت میں
ہیں تاکہ Cargo، rustup، اور Poetry کمانڈز root privileges چھوڑنے کے بعد بھی مکمل طور پر فعال رہیں۔

## Builds چلانا

امیج چلاتے وقت اپنے workspace کو `/workspace` (کنٹینر `WORKDIR`) سے attach کریں۔ مثال:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

امیج `docker` گروپ کی رکنیت برقرار رکھتی ہے تاکہ nested Docker commands (مثلاً `docker buildx bake`)
ان CI workflows کیلئے دستیاب رہیں جو host PID اور socket mount کرتے ہیں۔ اپنے ماحول کے مطابق گروپ mappings ایڈجسٹ کریں۔

## Iroha 2 بمقابلہ Iroha 3 artefacts

اب workspace ہر release لائن کیلئے الگ binaries خارج کرتا ہے تاکہ collisions سے بچا جا سکے:
`iroha3`/`iroha3d` (ڈیفالٹ) اور `iroha2`/`iroha2d` (Iroha 2)۔ مطلوبہ جوڑی بنانے کیلئے helpers استعمال کریں:

- `make build` (یا `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) Iroha 3 کیلئے
- `make build-i2` (یا `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) Iroha 2 کیلئے

Selector feature sets (`telemetry` + `schema-endpoint` اور لائن مخصوص `build-i{2,3}` flag) کو pin کرتا ہے
تاکہ Iroha 2 builds غلطی سے Iroha 3 کے defaults نہ لے لیں۔

`--profile` کو `iroha2` یا `iroha3` پر سیٹ کرنے پر `scripts/build_release_bundle.sh` سے بننے والے release bundles خودکار طور
پر درست binary نام منتخب کرتے ہیں۔

</div>
