---
lang: az
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-12-29T18:16:35.082870+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraFS CI Nümunə Qurğuları

Bu kataloq nümunədən yaradılan deterministik artefaktları paketləyir
`fixtures/sorafs_manifest/ci_sample/` altında faydalı yük. Paket nümayiş etdirir
CI iş axınlarının həyata keçirdiyi uçdan-uca SoraFS qablaşdırma və imzalama boru kəməri.

## Artefakt İnventarizasiyası

| Fayl | Təsvir |
|------|-------------|
| `payload.txt` | Qurğu skriptləri tərəfindən istifadə olunan mənbə yükü (düz mətn nümunəsi). |
| `payload.car` | `sorafs_cli car pack` tərəfindən yayılan CAR arxivi. |
| `car_summary.json` | Xülasə `car pack` tərəfindən yaradılmışdır. |
| `chunk_plan.json` | Parça diapazonlarını və provayderin gözləntilərini təsvir edən JSON-nu gətirmə planı. |
| `manifest.to` | `sorafs_cli manifest build` tərəfindən hazırlanmış Norito manifest. |
| `manifest.json` | Sazlama üçün insan tərəfindən oxuna bilən manifest göstərilməsi. |
| `proof.json` | `sorafs_cli proof verify` tərəfindən yayılan PoR xülasəsi. |
| `manifest.bundle.json` | `sorafs_cli manifest sign` tərəfindən yaradılan açarsız imza paketi. |
| `manifest.sig` | Manifestə uyğun olan ayrılmış Ed25519 imzası. |
| `manifest.sign.summary.json` | İmzalama zamanı yayılan CLI xülasəsi (heşlər, paket metadata). |
| `manifest.verify.summary.json` | `manifest verify-signature`-dən CLI xülasəsi. |

Buraxılış qeydlərində və sənədlərdə istinad edilən bütün həzmlər mənbədən götürülüb
bu fayllar. `ci/check_sorafs_cli_release.sh` iş axını eyni şeyi bərpa edir
artefaktlar və onları törədilmiş versiyalardan fərqləndirir.

## Armaturun bərpası

Armatur dəstini bərpa etmək üçün depo kökündən aşağıdakı əmrləri yerinə yetirin.
Onlar `sorafs-cli-fixture` iş axını tərəfindən istifadə olunan addımları əks etdirir:

```bash
sorafs_cli car pack \
  --input fixtures/sorafs_manifest/ci_sample/payload.txt \
  --car-out fixtures/sorafs_manifest/ci_sample/payload.car \
  --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to \
  --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --car fixtures/sorafs_manifest/ci_sample/payload.car \
  --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig \
  --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)" \
  --issued-at 1700000000 \
  > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b \
  > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

Hər hansı bir addım fərqli hashlər yaradırsa, qurğuları yeniləməzdən əvvəl araşdırın.
CI iş axınları reqressiyaları aşkar etmək üçün deterministik çıxışa əsaslanır.

## Gələcək Əhatə

Əlavə chunker profilləri və sübut formatları yol xəritəsindən çıxdıqda,
onların kanonik qurğuları bu kataloqa əlavə olunacaq (məsələn,
`sorafs.sf2@1.0.0` (bax: `fixtures/sorafs_manifest/ci_sample_sf2/`) və ya PDP
axın sübutları). Hər yeni profil eyni strukturu izləyəcək - faydalı yük, CAR,
plan, manifest, sübutlar və imza artefaktları - aşağı axın avtomatlaşdırılması belə edə bilər
xüsusi skript olmadan diff buraxılışları.