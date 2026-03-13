---
id: norito-rpc-adoption
lang: az
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Kanonik planlaşdırma qeydləri `docs/source/torii/norito_rpc_adoption_schedule.md`-də yaşayır.  
> Bu portal nüsxəsi SDK müəllifləri, operatorları və rəyçiləri üçün yayım gözləntilərini distillə edir.

## Məqsədlər

- Hər SDK-nı (Rust CLI, Python, JavaScript, Swift, Android) ikili Norito-RPC nəqliyyatında AND4 istehsal keçidindən əvvəl uyğunlaşdırın.
- Faza qapılarını, sübut paketlərini və telemetriya qarmaqlarını deterministik saxlayın ki, idarəetmə yayımı yoxlaya bilsin.
- NRPC-4-ün yol xəritəsinin çağırdığı ortaq köməkçilərlə armatur və kanareyka dəlillərini tutmağı əhəmiyyətsiz edin.

## Faza qrafiki

| Faza | Pəncərə | Əhatə dairəsi | Çıxış meyarları |
|-------|--------|-------|---------------|
| **P0 – Laboratoriya pariteti** | Q22025 | Rust CLI + Python tüstü dəstləri CI-də `/v2/norito-rpc` işləyir, JS köməkçisi vahid testlərindən keçir, Android saxta qoşqu ikili nəqli həyata keçirir. | CI-də `python/iroha_python/scripts/run_norito_rpc_smoke.sh` və `javascript/iroha_js/test/noritoRpcClient.test.js` yaşıl; Android qoşqu `./gradlew test`-ə qoşulub. |
| **P1 – SDK önizləmə** | Q32025 | Paylaşılan qurğu paketi yoxlanılıb, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` qeydləri + `artifacts/norito_rpc/`-də JSON, SDK nümunələrində ifşa olunmuş isteğe bağlı Norito nəqliyyat bayraqları. | Qurğu manifest imzalanıb, README yeniləmələri qoşulma istifadəsini, IOS2 bayrağının arxasında mövcud olan Swift önizləmə API-sini göstərir. |
| **P2 – Səhnələndirmə / AND4 önizləmə** | Q12026 | Səhnələndirici Torii hovuzları Norito, Android AND4 ilkin baxış müştəriləri və Swift IOS2 paritet dəstlərinə defolt olaraq ikili nəqliyyata üstünlük verir, telemetriya tablosuna `dashboards/grafana/torii_norito_rpc_observability.json` yerləşmişdir. | `docs/source/torii/norito_rpc_stage_reports.md` kanareyanı ələ keçirir, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` keçir, Android saxta qoşqularının təkrarı uğur/səhv hallarını çəkir. |
| **P3 – İstehsal GA** | Q42026 | Norito bütün SDK-lar üçün standart nəqliyyata çevrilir; JSON köhnəlmiş ehtiyat olaraq qalır. İş arxivi paritet artefaktlarını hər etiketlə buraxın. | Rust/JS/Python/Swift/Android üçün Norito tüstü çıxışı yoxlama siyahısı paketlərini buraxın; Norito və tətbiq edilən JSON xəta dərəcəsi SLO-ları üçün xəbərdarlıq hədləri; `status.md` və buraxılış qeydləri GA sübutlarına istinad edir. |

## SDK çatdırılması və CI qarmaqları

- **Rust CLI və inteqrasiya qoşqu** – `cargo xtask norito-rpc-verify` yerə endikdən sonra Norito nəqlini məcbur etmək üçün `iroha_cli pipeline` tüstü testlərini genişləndirin. `artifacts/norito_rpc/` altında artefaktları saxlayaraq `cargo test -p integration_tests -- norito_streaming` (laboratoriya) və `cargo xtask norito-rpc-verify` (təhsil/GA) ilə qoruyun.
- **Python SDK** – buraxılış tüstüsünü (`python/iroha_python/scripts/release_smoke.sh`) Norito RPC-yə defolt edin, `run_norito_rpc_smoke.sh`-i CI giriş nöqtəsi kimi saxlayın və `python/iroha_python/README.md`-də sənəd paritetini idarə edin. CI hədəfi: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **JavaScript SDK** – `NoritoRpcClient`-i stabilləşdirin, `toriiClientConfig.transport.preferred === "norito_rpc"` zamanı idarəetmə/sorğu köməkçilərinə defolt olaraq Norito-ə icazə verin və `javascript/iroha_js/recipes/`-də başdan-başa nümunələri ələ keçirin. CI dərc etməzdən əvvəl `npm test` və dokerləşdirilmiş `npm run test:norito-rpc` işini işlətməlidir; mənşəli yükləmələr Norito tüstü qeydləri `javascript/iroha_js/artifacts/` altında.
- **Swift SDK** – Norito körpü nəqliyyatını IOS2 bayrağının arxasına keçirin, qurğunun kadansını əks etdirin və Connect/Norito paritet dəstinin `docs/source/sdk/swift/index.md`-də istinad edilən Buildkite zolaqlarında işləməsini təmin edin.
- **Android SDK** – AND4 ilkin baxış müştəriləri və saxta Torii qoşqu, `docs/source/sdk/android/networking.md`-də sənədləşdirilmiş təkrar cəhd/geri çəkilmə telemetriyası ilə Norito-i qəbul edir. Qoşqu `scripts/run_norito_rpc_fixtures.sh --sdk android` vasitəsilə digər SDK-larla qurğuları paylaşır.

## Sübut və avtomatlaşdırma

- `scripts/run_norito_rpc_fixtures.sh` `cargo xtask norito-rpc-verify`-i bükür, stdout/stderr tutur və `fixtures.<sdk>.summary.json` yayır, beləliklə SDK sahiblərinin `status.md`-ə əlavə etmək üçün deterministik artefakt var. CI paketlərini səliqəli saxlamaq üçün `--sdk <label>` və `--out artifacts/norito_rpc/<stamp>/` istifadə edin.
- `cargo xtask norito-rpc-verify` sxem hash paritetini (`fixtures/norito_rpc/schema_hashes.json`) tətbiq edir və Torii `X-Iroha-Error-Code: schema_mismatch` qaytardıqda uğursuz olur. Sazlama üçün hər bir uğursuzluğu JSON geri çəkilmə ilə cütləşdirin.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` və `dashboards/grafana/torii_norito_rpc_observability.json` NRPC-2 üçün xəbərdarlıq müqavilələrini müəyyənləşdirir. Hər tablosuna düzəliş etdikdən sonra skripti işə salın və `promtool` çıxışını kanarya paketində saxlayın.
- `docs/source/runbooks/torii_norito_rpc_canary.md` səhnələşdirmə və istehsal məşqlərini təsvir edir; qurğu hashləri və ya xəbərdarlıq qapıları dəyişdikdə onu yeniləyin.

## Rəyçi yoxlama siyahısı

NRPC-4 mərhələni qeyd etməzdən əvvəl təsdiqləyin:

1. Ən son qurğu paketi heşləri `fixtures/norito_rpc/schema_hashes.json` və `artifacts/norito_rpc/<stamp>/` altında qeydə alınmış müvafiq CI artefaktına uyğun gəlir.
2. SDK README / portal sənədləri JSON-u geri qaytarmağa məcbur etmək və Norito nəqliyyat defoltuna istinad etmək yollarını təsvir edir.
3. Telemetriya tablosunda xəbərdarlıq keçidləri olan ikili stack xəta dərəcəsi panelləri göstərilir və Alertmanager quru işi (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) izləyiciyə əlavə edilib.
4. Buradakı övladlığa götürmə cədvəli izləyici girişinə (`docs/source/torii/norito_rpc_tracker.md`) uyğun gəlir və yol xəritəsi (NRPC-4) eyni sübut paketinə istinad edir.

Cədvəldə nizam-intizamlı qalmaq SDK-lararası davranışı proqnozlaşdırıla bilən saxlayır və idarəçilik auditinin Norito-RPC-ni sifarişli sorğular olmadan qəbul etməyə imkan verir.