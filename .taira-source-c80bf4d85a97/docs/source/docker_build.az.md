---
lang: az
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker Qurucu Şəkli

Bu konteyner `Dockerfile.build`-də müəyyən edilib və bütün alətlər zəncirini birləşdirir
CI və yerli buraxılış quruluşları üçün tələb olunan asılılıqlar. Şəkil indi a kimi işləyir
qeyri-root istifadəçisi, buna görə də Git əməliyyatları Arch Linux ilə işləməyə davam edir
Qlobal `safe.directory` həllinə müraciət etmədən `libgit2` paketi.

## Arqumentlər qurun

- `BUILDER_USER` – konteyner daxilində yaradılmış giriş adı (defolt: `iroha`).
- `BUILDER_UID` – rəqəmli istifadəçi identifikatoru (defolt: `1000`).
- `BUILDER_GID` – əsas qrup id (defolt: `1000`).

İş sahəsini hostunuzdan quraşdırdığınız zaman uyğun UID/GID dəyərlərini ötürün
yaradılan artefaktlar yazıla bilər:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Alətlər silsiləsi kataloqları (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`)
konfiqurasiya edilmiş istifadəçiyə məxsusdur, ona görə də Cargo, rustup və Poetry əmrləri tam olaraq qalır
konteyner kök imtiyazlarını itirdikdən sonra funksionaldır.

## İşləyən quruluşlar

İş yerinizi `/workspace` (konteyner `WORKDIR`) ilə birləşdirin.
şəkil. Misal:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

Şəkil `docker` qrup üzvlüyünü saxlayır, beləliklə iç içə Docker əmrləri (məs.
`docker buildx bake`) host PID-ni quraşdıran CI iş axınları üçün əlçatan qalır
və rozetka. Ətrafınız üçün lazım olduqda qrup xəritələrini tənzimləyin.

## Iroha 2 vs Iroha 3 artefakt

İş sahəsi indi toqquşmaların qarşısını almaq üçün buraxılış sətirinə ayrı ikili faylları buraxır:
`iroha3`/`iroha3d` (standart) və `iroha2`/`iroha2d` (Iroha 2). Köməkçilərdən istifadə edin
istədiyiniz cütü istehsal edin:

- Iroha 3 üçün `make build` (və ya `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`)
- Iroha 2 üçün `make build-i2` (və ya `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`)

Selektor funksiya dəstlərini (`telemetry` + `schema-endpoint` üstəgəl
sətirə məxsus `build-i{2,3}` bayrağı) buna görə də Iroha 2 quruluşu təsadüfən götürə bilməz
Iroha 3-yalnız defolt.

`scripts/build_release_bundle.sh` vasitəsilə qurulmuş buraxılış paketləri düzgün binar seçin
`--profile` `iroha2` və ya `iroha3` olaraq təyin edildikdə avtomatik adlar.