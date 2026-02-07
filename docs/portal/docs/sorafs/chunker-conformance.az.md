---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d948fcd78a564487591aeba23d4587de337913984fd3a5861a83f2a9a23887d9
source_last_modified: "2026-01-05T09:28:11.855022+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-conformance
title: SoraFS Chunker Conformance Guide
sidebar_label: Chunker Conformance
description: Requirements and workflows for preserving the deterministic SF1 chunker profile across fixtures and SDKs.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

Bu təlimat hər bir tətbiqin qalmaq üçün riayət etməli olduğu tələbləri kodlaşdırır
SoraFS deterministik chunker profili (SF1) ilə uyğunlaşdırılmışdır. O da
regenerasiya iş prosesini, imzalama siyasətini və yoxlama addımlarını belə sənədləşdirir
SDK-larda qurğu istehlakçıları sinxron qalır.

## Kanonik Profil

- Profil dəstəyi: `sorafs.sf1@1.0.0`
- Giriş toxumu (hex): `0000000000dec0ded`
- Hədəf ölçüsü: 262144 bayt (256KiB)
- Minimum ölçü: 65536 bayt (64KiB)
- Maksimum ölçü: 524288 bayt (512KiB)
- Rolling polinom: `0x3DA3358B4DC173`
- Dişli masa toxumu: `sorafs-v1-gear`
- Fasilə maskası: `0x0000FFFF`

İstinad tətbiqi: `sorafs_chunker::chunk_bytes_with_digests_profile`.
İstənilən SIMD sürətləndirilməsi eyni sərhədlər və həzmlər yaratmalıdır.

## Armatur dəsti

`cargo run --locked -p sorafs_chunker --bin export_vectors` bərpa edir
qurğular və `fixtures/sorafs_chunker/` altında aşağıdakı faylları yayır:

- `sf1_profile_v1.{json,rs,ts,go}` - Rust üçün kanonik yığın sərhədləri,
  TypeScript və Go istehlakçıları. Hər bir fayl kanonik sapı reklam edir
  `profile_aliases`-də ilk (və yeganə) giriş. Sifariş tərəfindən icra edilir
  `ensure_charter_compliance` və dəyişdirilməməlidir.
- `manifest_blake3.json` — Hər bir qurğu faylını əhatə edən BLAKE3 tərəfindən təsdiqlənmiş manifest.
- `manifest_signatures.json` - Manifest üzərində Şura imzaları (Ed25519)
  həzm etmək.
- `sf1_profile_v1_backpressure.json` və `fuzz/` daxilində xam korpus —
  chunker geri təzyiq testləri tərəfindən istifadə edilən deterministik axın ssenariləri.

### İmzalama Siyasəti

Armaturun bərpası **gərək** etibarlı şura imzasını daxil etməlidir. Generator
`--allow-unsigned` açıq şəkildə (nəzərdə tutulan) qəbul edilmədikdə imzasız çıxışı rədd edir
yalnız yerli təcrübə üçün). İmza zərfləri yalnız əlavə olunur və
hər imzalayan üçün təkrarlanır.

Şura imzası əlavə etmək üçün:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Doğrulama

CI köməkçisi `ci/check_sorafs_fixtures.sh` ilə generatoru təkrarlayır
`--locked`. Qurğular sürüşürsə və ya imzalar yoxdursa, iş uğursuz olur. istifadə edin
bu skripti gecə iş axınlarında və qurğu dəyişikliklərini təqdim etməzdən əvvəl.

Əllə yoxlama addımları:

1. `cargo test -p sorafs_chunker`-i işə salın.
2. Yerli olaraq `ci/check_sorafs_fixtures.sh` çağırın.
3. `git status -- fixtures/sorafs_chunker`-in təmiz olduğunu təsdiq edin.

## Kitabı təkmilləşdirin

Yeni chunker profili təklif edərkən və ya SF1-i yeniləyərkən:

Həmçinin bax: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) üçün
metadata tələbləri, təklif şablonları və yoxlama siyahıları.

1. Yeni parametrlərlə `ChunkProfileUpgradeProposalV1` (bax. RFC SF‑1) layihəsini hazırlayın.
2. `export_vectors` vasitəsilə qurğuları bərpa edin və yeni manifest həzmini qeyd edin.
3. Tələb olunan şura kvorumu ilə manifest imzalayın. Bütün imzalar olmalıdır
   `manifest_signatures.json`-ə əlavə edilmişdir.
4. Təsirə məruz qalmış SDK qurğularını (Rust/Go/TS) yeniləyin və iş vaxtı arası pariteti təmin edin.
5. Parametrlər dəyişdikdə qeyri-səlis korpusu bərpa edin.
6. Bu təlimatı yeni profil sapı, toxum və həzm ilə yeniləyin.
7. Dəyişikliyi yenilənmiş testlər və yol xəritəsi yeniləmələri ilə birlikdə təqdim edin.

Bu prosesi izləmədən yığın sərhədlərinə və ya həzmlərə təsir edən dəyişikliklər
etibarsızdır və birləşdirilməməlidir.