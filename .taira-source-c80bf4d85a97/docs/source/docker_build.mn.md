---
lang: mn
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker Барилгачин зураг

Энэ контейнер нь `Dockerfile.build`-д тодорхойлогдсон бөгөөд бүх хэрэгслийн гинжийг багцалдаг.
CI болон орон нутгийн хувилбаруудыг бүтээхэд шаардлагатай хамаарал. Зураг нь одоо байдлаар ажиллаж байна
анхдагчаар root бус хэрэглэгч тул Git-ийн үйлдлүүд Arch Linux-тай үргэлжлүүлэн ажилладаг
`libgit2` багцыг дэлхийн `safe.directory` тойрон гарах аргад ашиглахгүйгээр.

## Аргумент үүсгэх

- `BUILDER_USER` – чингэлэг дотор үүсгэсэн нэвтрэх нэр (өгөгдмөл: `iroha`).
- `BUILDER_UID` – тоон хэрэглэгчийн ID (өгөгдмөл: `1000`).
- `BUILDER_GID` – үндсэн бүлгийн ID (өгөгдмөл: `1000`).

Та өөрийн хостоос ажлын талбарыг холбохдоо тохирох UID/GID утгыг дамжуулна уу
үүсгэсэн олдворууд бичих боломжтой хэвээр байна:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Хэрэгслийн гинжний лавлахууд (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`)
нь тохируулсан хэрэглэгчийн эзэмшилд байдаг тул Cargo, rustup болон Poetry командууд бүрэн хэвээр үлдэнэ
Контейнер root эрхийг хассан үед ажиллах болно.

## Ажиллаж байна

Дуудлага хийхдээ ажлын талбараа `/workspace` (`WORKDIR` сав) руу холбоно уу.
зураг. Жишээ:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

Зураг нь `docker` бүлгийн гишүүнчлэлийг хадгалдаг тул Docker командуудыг оруулдаг (жишээ нь:
`docker buildx bake`) хост PID-г холбодог CI ажлын урсгалд ашиглах боломжтой хэвээр байна
болон залгуур. Өөрийн орчинд шаардлагатай бол бүлгийн зураглалыг тохируулна уу.

## Iroha 2 эсрэг Iroha 3 олдвор

Одоо ажлын талбар нь мөргөлдөөнөөс зайлсхийхийн тулд хувилбар бүрт тусдаа хоёртын файлуудыг ялгаруулдаг:
`iroha3`/`iroha3d` (өгөгдмөл) ба `iroha2`/`iroha2d` (Iroha 2). Туслах хүмүүсийг ашиглана уу
хүссэн хосыг гаргана:

- `make build` (эсвэл `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) Iroha 3
- Iroha 2-д зориулсан `make build-i2` (эсвэл `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`)

Сонгогч нь функцийн багцуудыг (`telemetry` + `schema-endpoint` дээр нэмэх нь
шугамын тусгай `build-i{2,3}` туг) тул Iroha 2 бүтээцийг санамсаргүйгээр авах боломжгүй.
Iroha Зөвхөн 3-н өгөгдмөл.

`scripts/build_release_bundle.sh`-ээр бүтээгдсэн багцуудыг зөв хоёртын файлыг сонго
`--profile`-г `iroha2` эсвэл `iroha3` гэж тохируулсан үед автоматаар нэрлэнэ.