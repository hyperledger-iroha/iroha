---
lang: az
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! `roadmap.md:M4` tərəfindən istinad edilən məxfi aktivlərin auditi və əməliyyat kitabçası.

# Məxfi Aktivlərin Auditi və Əməliyyatlar Runbook

Bu təlimat auditorların və operatorların etibar etdiyi sübut səthlərini birləşdirir
məxfi aktivlərin hərəkətini təsdiq edərkən. O, fırlanma oyun kitabını tamamlayır
(`docs/source/confidential_assets_rotation.md`) və kalibrləmə kitabçası
(`docs/source/confidential_assets_calibration.md`).

## 1. Seçilmiş Açıqlama və Hadisə Lentləri

- Hər bir məxfi təlimat strukturlaşdırılmış `ConfidentialEvent` faydalı yük yayır
  (`Shielded`, `Transferred`, `Unshielded`) çəkilib
  `crates/iroha_data_model/src/events/data/events.rs:198` və seriyalı
  icraçılar (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  Reqressiya dəsti auditorların etibar edə bilməsi üçün konkret yükləri həyata keçirir
  deterministik JSON tərtibatları (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`).
- Torii bu hadisələri standart SSE/WebSocket boru kəməri vasitəsilə ifşa edir; auditorlar
  `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) istifadə edərək abunə olun
  isteğe bağlı olaraq vahid aktiv tərifinə qədər əhatə dairəsi. CLI nümunəsi:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } }'
  ```

- Siyasət metadatası və gözlənilən keçidlər vasitəsilə mövcuddur
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), Swift SDK tərəfindən əks olunur
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) və sənədləşdirilib
  həm məxfi aktivlərin dizaynı, həm də SDK təlimatları
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Telemetriya, Tablolar və Kalibrləmə Sübutları

- Runtime ölçüləri səth ağacı dərinliyi, öhdəlik/sərhəd tarixi, kök çıxarılması
  sayğaclar və yoxlayıcı-keş vurma nisbətləri
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Grafana idarə panelləri
  `dashboards/grafana/confidential_assets.json` əlaqəli panelləri göndərir və
  `docs/source/confidential_assets.md:401`-də sənədləşdirilmiş iş axını ilə xəbərdarlıqlar.
- Kalibrləmə işləri (NS/op, qaz/op, ns/qaz) imzalanmış loglarla canlıdır
  `docs/source/confidential_assets_calibration.md`. Ən son Apple Silicon
  NEON qaçışı arxivdə saxlanılır
  `docs/source/confidential_assets_calibration_neon_20260428.log` və eyni
  mühasibat kitabçası SIMD-neytral və AVX2 profilləri üçün müvəqqəti imtinaları qeyd edir
  x86 hostları onlayn olur.

## 3. Hadisəyə Cavab & Operator Tapşırıqları

- Rotasiya/təkmilləşdirmə prosedurları mövcuddur
  `docs/source/confidential_assets_rotation.md`, yeni səhnəni necə əhatə edir
  parametr paketləri, siyasət yeniləmələrini planlaşdırın və pul kisələrini/auditorları xəbərdar edin. The
  izləyici (`docs/source/project_tracker/confidential_assets_phase_c.md`) siyahıları
  runbook sahibləri və məşq gözləntiləri.
- İstehsal məşqləri və ya təcili yardım pəncərələri üçün operatorlar sübut əlavə edirlər
  `status.md` qeydləri (məsələn, çox zolaqlı məşq jurnalı) və bunlara daxildir:
  `curl` siyasət keçidlərinin sübutu, Grafana anlıq görüntülər və müvafiq hadisə
  həzm edir ki, auditorlar nanə → köçürmə → vaxt qrafiklərini yenidən qura bilsinlər.

## 4. Xarici baxış tempi

- Təhlükəsizlik araşdırmasının əhatə dairəsi: məxfi sxemlər, parametr registrləri, siyasət
  keçidlər və telemetriya. Bu sənəd və kalibrləmə kitabçası formaları
  satıcılara göndərilən sübut paketi; baxış planı vasitəsilə izlənilir
  `docs/source/project_tracker/confidential_assets_phase_c.md`-də M4.
- Operatorlar `status.md`-i hər hansı satıcı tapıntıları və ya təqibi ilə yeniləməlidirlər
  fəaliyyət maddələri. Xarici baxış tamamlanana qədər bu runbook kimi xidmət edir
  əməliyyat bazası auditorları sınaqdan keçirə bilərlər.