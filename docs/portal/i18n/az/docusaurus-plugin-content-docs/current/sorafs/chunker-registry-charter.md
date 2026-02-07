---
id: chunker-registry-charter
lang: az
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Charter
sidebar_label: Chunker Registry Charter
description: Governance charter for chunker profile submissions and approvals.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

# SoraFS Chunker Registry İdarəetmə Xartiyası

> **Təsdiq edilib:** 29-10-2025-ci il tarixində Sora Parlamentinin İnfrastruktur Paneli tərəfindən (bax.
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Hər hansı düzəliş tələb edir a
> formal idarəetmə səsverməsi; icra qrupları bu sənəd kimi davranmalıdır
> əvəz edən nizamnamə təsdiq olunana qədər normativdir.

Bu nizamnamə SoraFS çinkerinin inkişafı üçün prosesi və rolları müəyyən edir.
reyestr. O, necə yeni olduğunu təsvir etməklə [Chunker Profilinin Müəlliflik Təlimatını](./chunker-profile-authoring.md) tamamlayır.

## Əhatə dairəsi

Nizamnamə `sorafs_manifest::chunker_registry` və hər bir girişə şamil edilir
reyestri istehlak edən hər hansı alətə (manifest CLI, provayder-reklam CLI,
SDKs). O, ləqəbi tətbiq edir və yoxlanılan invariantları idarə edir
`chunker_registry::ensure_charter_compliance()`:

- Profil identifikatorları monoton şəkildə artan müsbət tam ədədlərdir.
- Kanonik tutacaq `namespace.name@semver` **ilk olaraq görünməlidir**
- Alias sətirləri kəsilmiş, unikaldır və kanonik tutacaqlarla toqquşmur
  digər girişlərdən.

## Rollar

- **Müəllif(lər)** – təklifi hazırlayın, qurğuları bərpa edin və toplayın
  determinizm sübutu.
- **Tooling Working Group (TWG)** – dərc ediləndən istifadə edərək təklifi təsdiqləyir
  yoxlanış siyahılarını tərtib edir və reyestr invariantlarının saxlanmasını təmin edir.
- **İdarəetmə Şurası (GC)** – TWG hesabatını nəzərdən keçirir, təklifi imzalayır
  zərf verir və nəşr/köhnəlmə qrafiklərini təsdiq edir.
- **Storage Team** – reyestrin həyata keçirilməsini təmin edir və dərc edir
  sənəd yeniləmələri.

## Həyat dövrü iş axını

1. **Təklifin Göndərilməsi**
   - Müəllif müəllif bələdçisindən təsdiqləmə yoxlama siyahısını işlədir və yaradır
     altında `ChunkerProfileProposalV1` JSON
     `docs/source/sorafs/proposals/`.
   - CLI çıxışını daxil edin:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Qurğular, təklif, determinizm hesabatı və reyestrdən ibarət PR təqdim edin
     yeniləmələr.

2. **Alətlərə Baxış (TWG)**
   - Doğrulama yoxlama siyahısını təkrarlayın (qurğular, qeyri-səlis, manifest/PoR boru kəməri).
   - `cargo test -p sorafs_car --chunker-registry`-i işə salın və əmin olun
     `ensure_charter_compliance()` yeni girişlə keçir.
   - CLI davranışını yoxlayın (`--list-profiles`, `--promote-profile`, axın
     `--json-out=-`) yenilənmiş ləqəbləri və tutacaqları əks etdirir.
   - Nəticələri və keçid/uğursuz statusunu ümumiləşdirən qısa hesabat hazırlayın.

3. **Şura Təsdiqi (GC)**
   - TWG hesabatını və təklif metadatasını nəzərdən keçirin.
   - Təklif toplusunu imzalayın (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     və yanında saxlanılan şura zərfinə imzalar əlavə edin
     qurğular.
   - Səsvermənin nəticəsini idarəetmə protokolunda qeyd edin.

4. **Nəşr**
   - PR-ı birləşdirin, yeniləyin:
     - `sorafs_manifest::chunker_registry_data`.
     - Sənədləşdirmə (`chunker_registry.md`, müəlliflik/uyğunluq təlimatları).
     - Qurğular və determinizm hesabatları.
   - Operatorlara və SDK komandalarına yeni profil və planlaşdırılan təqdimat haqqında məlumat verin.

5. **Depresiya / Gün batımı**
   - Mövcud profili əvəz edən təkliflər ikili nəşri ehtiva etməlidir
     pəncərə (güzəşt dövrləri) və təkmilləşdirmə planı.
     reyestrində və miqrasiya kitabçasını yeniləyin.

6. **Fövqəladə Dəyişikliklər**
   - Silinmə və ya düzəlişlər çoxluğun təsdiqi ilə şuranın səsverməsini tələb edir.
   - TWG riskin azaldılması addımlarını sənədləşdirməli və insident jurnalını yeniləməlidir.

## Alət gözləntiləri

- `sorafs_manifest_chunk_store` və `sorafs_manifest_stub` ifşa edir:
  - Reyestr yoxlaması üçün `--list-profiles`.
  - `--promote-profile=<handle>` istifadə olunan kanonik metadata blokunu yaratmaq üçün
    profili təbliğ edərkən.
  - Hesabatları stdout-a ötürmək üçün `--json-out=-`, təkrarlana bilən nəzərdən keçirməyə imkan verir
    loglar.
- `ensure_charter_compliance()` müvafiq ikili fayllarda işə salındıqda çağırılır
  (`manifest_chunk_store`, `provider_advert_stub`). CI testləri yeni olduqda uğursuz olmalıdır
  yazılar nizamnaməni pozur.

## Qeydlərin aparılması

- Bütün determinizm hesabatlarını `docs/source/sorafs/reports/`-də saxlayın.
- Şura protokolları altında chunker qərarlarına istinad edilir
  `docs/source/sorafs/migration_ledger.md`.
- Hər bir əsas reyestr dəyişikliyindən sonra `roadmap.md` və `status.md`-i yeniləyin.

## İstinadlar

- Müəlliflik təlimatı: [Chunker Profili Müəlliflik Bələdçisi](./chunker-profile-authoring.md)
- Uyğunluq yoxlama siyahısı: `docs/source/sorafs/chunker_conformance.md`
- Reyestr arayışı: [Chunker Profil Registry](./chunker-registry.md)