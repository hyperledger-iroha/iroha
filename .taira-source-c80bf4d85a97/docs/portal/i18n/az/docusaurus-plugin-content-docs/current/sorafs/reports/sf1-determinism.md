---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS SF1 Determinism Dry-Run
summary: Checklist and expected digests for validating the canonical `sorafs.sf1@1.0.0` chunker profile.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS SF1 Determinizm Dry-Run

Bu hesabat kanonik üçün ilkin quru dövrü əhatə edir
`sorafs.sf1@1.0.0` chunker profili. Tooling WG yoxlama siyahısını yenidən işlətməlidir
armatur yeniləmələrini və ya yeni istehlakçı boru kəmərlərini təsdiq edərkən aşağıda. qeyd edin
yoxlanıla bilən izi saxlamaq üçün cədvəldəki hər bir əmrin nəticəsi.

## Yoxlama siyahısı

| Addım | Komanda | Gözlənilən Nəticə | Qeydlər |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Bütün testlər keçir; `vectors` paritet testi uğurla keçdi. | Kanonik qurğuların tərtib edilməsini və Rust tətbiqinə uyğunluğunu təsdiq edir. |
| 2 | `ci/check_sorafs_fixtures.sh` | Skript 0-dan çıxır; aşağıda açıqlanan həzmləri bildirir. | Armaturların təmiz şəkildə bərpa olunduğunu və imzaların bağlı qaldığını yoxlayır. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` üçün giriş reyestr deskriptoruna (`profile_id=1`) uyğun gəlir. | Reyestr metadatasının sinxron qalmasını təmin edir. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | Regenerasiya `--allow-unsigned` olmadan uğurla həyata keçirilir; manifest və imza faylları dəyişməz. | Parça sərhədləri və manifestlər üçün determinizm sübutunu təmin edir. |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript qurğuları ilə Rust JSON arasında heç bir fərq olmadığını bildirir. | Könüllü köməkçi; iş vaxtları arasında pariteti təmin edin (skript Tooling WG tərəfindən saxlanılır). |

## Gözlənilən Həzmlər

- Parça həzmi (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`
- `sf1_profile_v1.json`: `d89a4fdc030b0c7c4911719ea133c780d9f4610b08eef1d6d0e0ca443391718e`
- `sf1_profile_v1.ts`: `9a3bb8e4d96518b3a0a1301046b2d86a793991959ebdd8adda1fb2988e4292dc`
- `sf1_profile_v1.go`: `0f0348b8751b0f85fe874afda3371af75b78fac5dad65182204dcb3cf3e4c0a1`
- `sf1_profile_v1.rs`: `66b5956826c86589a24b71ca6b400cc1335323c6371f1cec9475f09af8743f61`

## Hesabdan Çıxış Qeydiyyatı

| Tarix | Mühəndis | Yoxlama Siyahısının Nəticəsi | Qeydlər |
|------|----------|------------------|-------|
| 2026-02-12 | Alətlər (LLM) | ❌ Uğursuz | Addım 1: `cargo test -p sorafs_chunker` `vectors` dəsti uğursuz oldu, çünki qurğular köhnəlmişdir. Addım 2: `ci/check_sorafs_fixtures.sh` dayandırır—`manifest_signatures.json` repo vəziyyətində yoxdur (işləyən ağacda silinib). Addım 4: `export_vectors` manifest faylı olmadıqda imzaları yoxlaya bilmir. İmzalanmış qurğuları bərpa etməyi (və ya şura açarını təqdim etməyi) və testlərin tələb etdiyi kimi kanonik tutacaqların daxil edilməsi üçün bağlamaları bərpa etməyi tövsiyə edin. |
| 2026-02-12 | Alətlər (LLM) | ✅ Keçdi | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` vasitəsilə bərpa edilmiş qurğular, yalnız kanonik tutacaqlı ləqəb siyahıları və yeni `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4` manifest həzmini yaradır. `cargo test -p sorafs_chunker` və təmiz `ci/check_sorafs_fixtures.sh` qaçışı (yoxlama üçün mərhələli qurğular) ilə təsdiq edilmişdir. Node paritet köməkçisi enənə qədər 5-ci addım gözlənilir. |
| 2026-02-20 | Saxlama Alətləri CI | ✅ Keçdi | Parlament zərfi (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` vasitəsilə gətirildi; skript yenidən yaradılan qurğular, təsdiqlənmiş manifest `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4` və heç bir fərq olmadan Rust qoşqunu (Mövcud olduqda Get/Node addımları yerinə yetirilir) yenidən işə saldı. |

Alət İş Qrupu yoxlama siyahısını işə saldıqdan sonra tarixli sətir əlavə etməlidir. Əgər hər hansı bir addım
uğursuz olarsa, burada əlaqələndirilmiş problemi qeyd edin və əvvəl düzəliş təfərrüatlarını daxil edin
yeni qurğuların və ya profillərin təsdiqlənməsi.