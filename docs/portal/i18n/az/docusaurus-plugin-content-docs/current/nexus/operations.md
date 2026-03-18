---
id: nexus-operations
lang: az
direction: ltr
source: docs/portal/docs/nexus/operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus operations runbook
description: Field-ready summary of the Nexus operator workflow, mirroring `docs/source/nexus_operations.md`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Bu səhifəni sürətli istinad qardaşı kimi istifadə edin
`docs/source/nexus_operations.md`. O, əməliyyat yoxlama siyahısını distillə edir, dəyişdirir
idarəetmə qarmaqları və Nexus operatorlarının tələb etdiyi telemetriya əhatə dairəsi tələbləri
izləyin.

## Həyat dövrü yoxlama siyahısı

| Mərhələ | Fəaliyyətlər | Sübut |
|-------|--------|----------|
| Uçuşdan əvvəl | Buraxılış hashlarını/imzalarını yoxlayın, `profile = "iroha3"`-i təsdiqləyin və konfiqurasiya şablonlarını hazırlayın. | `scripts/select_release_profile.py` çıxışı, yoxlama jurnalı, imzalanmış manifest paketi. |
| Kataloqun düzülməsi | `[nexus]` kataloqunu, marşrutlaşdırma siyasətini və şura tərəfindən verilən manifest üzrə DA hədlərini yeniləyin, sonra `--trace-config`-i əldə edin. | `irohad --sora --config … --trace-config` çıxışı uçuş bileti ilə birlikdə saxlanılır. |
| Duman və kəsmə | `irohad --sora --config … --trace-config`-i işə salın, CLI tüstüsünü (`FindNetworkStatus`) yerinə yetirin, telemetriya ixracını doğrulayın və qəbul tələb edin. | Smoke-test log + Alertmanager təsdiqi. |
| Sabit vəziyyət | Tablolara/xəbərdarlıqlara nəzarət edin, idarəetmə ritminə görə düymələri fırladın və hər dəfə dəyişiklik olduqda konfiqurasiyaları/runbookları sinxronlaşdırın. | Rüblük nəzərdən keçirmə dəqiqələri, tablosuna ekran görüntüləri, fırlanma bileti identifikatorları. |

Ətraflı işə qəbul (açarların dəyişdirilməsi, marşrutlaşdırma şablonları, buraxılış profili addımları)
`docs/source/sora_nexus_operator_onboarding.md`-də qalır.

## Dəyişikliklərin idarə edilməsi

1. **Reliz yeniləmələri** – `status.md`/`roadmap.md`-də elanları izləyin; əlavə edin
   hər buraxılış PR-a daxil olma yoxlama siyahısı.
2. **Zolaq manifest dəyişiklikləri** – Kosmik Kataloqdan imzalanmış paketləri yoxlayın və
   onları `docs/source/project_tracker/nexus_config_deltas/` altında arxivləşdirin.
3. **Konfiqurasiya deltaları** – hər `config/config.toml` dəyişiklik bilet tələb edir
   zolağa/məlumat sahəsinə istinad edir. Effektiv konfiqurasiyanın redaktə edilmiş nüsxəsini saxlayın
   qovşaqlar qoşulduqda və ya təkmilləşdikdə.
4. **Geri döndərmə matkapları** – rüblük məşq dayandırma/bərpa/tüstüləmə prosedurları; log
   `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` altında nəticələr.
5. **Uyğunluq təsdiqləri** – özəl/CBDC zolaqları uyğunluq qeydiyyatını təmin etməlidir
   DA siyasətini və ya telemetriya redaktə düymələrini dəyişdirməzdən əvvəl (bax
   `docs/source/cbdc_lane_playbook.md`).

## Telemetriya və SLO

- İdarə panelləri: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, plus
  SDK-ya xüsusi baxışlar (məsələn, `android_operator_console.json`).
- Xəbərdarlıqlar: `dashboards/alerts/nexus_audit_rules.yml` və Torii/Norito nəqliyyat
  qaydalar (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Baxılacaq ölçülər:
  - `nexus_lane_height{lane_id}` – üç yuva üçün sıfır irəliləyiş barədə xəbərdarlıq.
  - `nexus_da_backlog_chunks{lane_id}` – zolağa aid hədlərin üstündə xəbərdarlıq
    (standart 64 ictimai / 8 özəl).
  - `nexus_settlement_latency_seconds{lane_id}` – P99 900ms-dən çox olduqda xəbərdarlıq
    (ictimai) və ya 1200ms (özəl).
  - `torii_request_failures_total{scheme="norito_rpc"}` – 5 dəqiqəlik xəta olduqda xəbərdarlıq
    nisbət > 2%.
  - `telemetry_redaction_override_total` – Sev2 dərhal; ləğv edilməsini təmin edin
    uyğunluq biletləri var.
- Telemetriyanın bərpası yoxlama siyahısını işə salın
  [Nexus telemetriya remediasiya planı](./nexus-telemetry-remediation) ən azı
  rüblük və doldurulmuş formanı əməliyyatların nəzərdən keçirilməsi qeydlərinə əlavə edin.

## Hadisə matrisi

| Ciddilik | Tərif | Cavab |
|----------|------------|----------|
| Sev1 | Məlumat məkanının izolyasiyasının pozulması, hesablaşmanın dayandırılması >15 dəqiqə və ya idarəetmədə səsvermə korrupsiyası. | Səhifə Nexus İbtidai + Buraxılış Mühəndisliyi + Uyğunluq, qəbulu dondurun, artefaktları toplayın, xəbərləri dərc edin ≤60dəq, RCA ≤5 iş günü. |
| Sev2 | Zolaqlı gecikmə SLA pozuntusu, telemetriya kor nöqtəsi >30 dəqiqə, uğursuz manifest buraxılması. | Səhifə Nexus İbtidai + SRE, ≤4 saat azaldın, 2 iş günü ərzində təqibləri fayl edin. |
| Sev3 | Bloklanmayan sürüşmə (sənədlər, xəbərdarlıqlar). | İzləyiciyə daxil olun, sprint daxilində düzəltməyi planlaşdırın. |

Hadisə biletləri təsirə məruz qalan zolaq/məlumat məkanı identifikatorlarını, manifest xeşlərini qeyd etməlidir,
vaxt qrafiki, dəstəkləyici ölçülər/loglar və təqib tapşırıqları/sahibləri.

## Sübut arxivi

- `artifacts/nexus/<lane>/<date>/` altında paketləri/manifestləri/temetriya ixracını saxlayın.
- Hər buraxılış üçün redaktə edilmiş konfiqurasiyaları + `--trace-config` çıxışını saxlayın.
- Konfiqurasiya və ya manifest dəyişdirildikdə şura protokollarını + imzalanmış qərarları əlavə edin.
- 12 ay ərzində Nexus ölçülərinə uyğun həftəlik Prometheus anlık görüntülərini qoruyun.
- `docs/source/project_tracker/nexus_config_deltas/README.md`-də runbook redaktələrini qeyd edin
  beləliklə, auditorlar məsuliyyətlərin nə vaxt dəyişdiyini bilirlər.

## Əlaqədar material

- İcmal: [Nexus icmalı](./nexus-overview)
- Spesifikasiya: [Nexus spesifikasiyası](./nexus-spec)
- Zolaq həndəsəsi: [Nexus zolaq modeli](./nexus-lane-model)
- Keçid və marşrutlaşdırıcılar: [Nexus keçid qeydləri](./nexus-transition-notes)
- Operatorun işə salınması: [Sora Nexus operatorun işə salınması](./nexus-operator-onboarding)
- Telemetriyanın düzəldilməsi: [Nexus telemetriyanın bərpası planı](./nexus-telemetry-remediation)