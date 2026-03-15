---
id: chunker-registry-rollout-checklist
lang: az
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

# SoraFS Registry Rollout Yoxlama Siyahısı

Bu yoxlama siyahısı yeni chunker profili və ya tanıtmaq üçün tələb olunan addımları əks etdirir
nəzarətdən sonra istehsala qədər provayderin qəbul paketi
nizamnaməsi təsdiq edilmişdir.

> **Əhatə dairəsi:** Dəyişdirən bütün buraxılışlara aiddir
> `sorafs_manifest::chunker_registry`, provayderin qəbulu zərfləri və ya
> kanonik qurğu dəstələri (`fixtures/sorafs_chunker/*`).

## 1. Uçuşdan əvvəl Qiymətləndirmə

1. Armaturları bərpa edin və determinizmi yoxlayın:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Determinizm hashlərini təsdiq edin
   `docs/source/sorafs/reports/sf1_determinism.md` (və ya müvafiq profil
   hesabat) bərpa edilmiş artefaktlara uyğun gəlir.
3. `sorafs_manifest::chunker_registry` ilə tərtib olunduğundan əmin olun
   `ensure_charter_compliance()` işlətməklə:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Təklif faylını yeniləyin:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` altında Şura protokolu girişi
   - Determinizm hesabatı

## 2. İdarəetmənin imzalanması

1. Alətlər üzrə İşçi Qrupun hesabatını və təkliflər toplusunu Sora təqdim edin
   Parlament İnfrastruktur Paneli.
2. Təsdiq təfərrüatlarını qeyd edin
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Parlament tərəfindən imzalanmış zərfi qurğularla yanaşı dərc edin:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Zərfin idarəçiliyin əldə edilməsi köməkçisi vasitəsilə əlçatan olduğunu yoxlayın:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Mərhələli Yayılma

üçün [təhsil manifestinin dərs kitabına](./staging-manifest-playbook) baxın
bu addımların ətraflı təsviri.

1. `torii.sorafs` kəşfi aktiv və qəbul ilə Torii yerləşdirin
   icra aktivləşdirildi (`enforce_admission = true`).
2. Təsdiq edilmiş provayderin qəbulu zərflərini səhnələşdirmə reyestrinə itələyin
   `torii.sorafs.discovery.admission.envelopes_dir` tərəfindən istinad edilən kataloq.
3. Provayder reklamlarının kəşf API vasitəsilə yayıldığını yoxlayın:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. İdarəetmə başlıqları ilə manifest/planın son nöqtələrini həyata keçirin:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Telemetriya tablosunu təsdiqləyin (`torii_sorafs_*`) və xəbərdarlıq qaydaları
   səhvsiz yeni profil.

## 4. İstehsalın yayılması

1. İstehsal Torii qovşaqlarına qarşı mərhələli addımları təkrarlayın.
2. Aktivləşdirmə pəncərəsini (tarix/vaxt, güzəşt müddəti, geri qaytarma planı) elan edin.
   operator və SDK kanalları.
3. Aşağıdakıları ehtiva edən buraxılış PR-ni birləşdirin:
   - Yenilənmiş qurğular və zərf
   - Sənəd dəyişiklikləri (nizamnamə istinadları, determinizm hesabatı)
   - Yol xəritəsi/statusun yenilənməsi
4. Buraxılışı etiketləyin və imzalanmış artefaktları mənşəyə görə arxivləşdirin.

## 5. Yayımdan Sonra Audit

1. Yekun ölçüləri əldə edin (kəşf sayları, əldə etmə müvəffəqiyyət dərəcəsi, xəta
   histoqramlar) buraxıldıqdan 24 saat sonra.
2. Qısa xülasə və determinizm hesabatına keçid ilə `status.md`-i yeniləyin.
3. İstənilən təqib tapşırıqlarını (məsələn, əlavə profil yaratma təlimatı) faylı
   `roadmap.md`.