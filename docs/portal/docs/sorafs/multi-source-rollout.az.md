---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee784b5b073019d219d0dfb76d44f6d02a8dbb46c847bfb226f1f72b41ffb1d7
source_last_modified: "2026-01-05T09:28:11.891621+00:00"
translation_last_reviewed: 2026-02-07
id: multi-source-rollout
title: Multi-Source Client Rollout & Blacklisting Runbook
sidebar_label: Multi-Source Rollout Runbook
description: Operational checklist for staged multi-source rollouts and emergency provider blacklisting.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

## Məqsəd

Bu runbook SRE və çağırış üzrə mühəndisləri iki kritik iş axını vasitəsilə istiqamətləndirir:

1. Çoxmənbəli orkestratorun idarə olunan dalğalarda yayılması.
2. Mövcud seansları qeyri-sabitləşdirmədən düzgün davranmayan provayderləri qara siyahıya salmaq və ya prioritetini azaltmaq.

Bu, SF-6 altında çatdırılan orkestrasiya yığınının artıq yerləşdirildiyini güman edir (`sorafs_orchestrator`, şlüz yığın diapazonu API, telemetriya ixracatçıları).

> **Həmçinin bax:** [Orkestrator Əməliyyatları Runbook](./orchestrator-ops.md) hər icra prosedurlarına (hesab tablosunun çəkilişi, mərhələli buraxılış keçidləri, geri çəkilmə) daxil olur. Canlı dəyişikliklər zamanı hər iki istinaddan birlikdə istifadə edin.

## 1. Uçuşdan əvvəl Qiymətləndirmə

1. **İdarəetmə məlumatlarını təsdiq edin.**
   - Bütün namizəd provayderlər diapazon qabiliyyətinin faydalı yükləri və axın büdcələri ilə `ProviderAdvertV1` zərflərini dərc etməlidirlər. `/v1/sorafs/providers` vasitəsilə təsdiqləyin və gözlənilən qabiliyyət sahələri ilə müqayisə edin.
   - Gecikmə/uğursuzluq dərəcələrini təmin edən telemetriya snapshotları hər kanareyka qaçışından əvvəl < 15 dəqiqə köhnə olmalıdır.
2. **Mərhələ konfiqurasiyası.**
   - Qatlı `iroha_config` ağacında orkestr JSON konfiqurasiyasını davam etdirin:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Yayımlama üçün xüsusi limitlərlə JSON-u yeniləyin (`max_providers`, yenidən cəhd büdcələri). Fərqlərin kiçik qalması üçün eyni faylı səhnələşdirmə/istehsal üçün qidalandırın.
3. **Kanonik qurğularla məşq edin.**
   - Manifest/token mühit dəyişənlərini doldurun və deterministik gətirməni işə salın:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     Ətraf mühit dəyişənləri kanareykada iştirak edən hər bir provayder üçün manifest faydalı yük həzmini (hex) və base64 kodlu axın tokenlərini ehtiva etməlidir.
   - Əvvəlki buraxılışdan fərqli `artifacts/canary.scoreboard.json`. Hər hansı yeni uyğun olmayan provayder və ya çəki dəyişikliyi >10% nəzərdən keçirilməsini tələb edir.
4. **Telemetriyanın simli olduğunu yoxlayın.**
   - `docs/examples/sorafs_fetch_dashboard.json`-də Grafana ixracını açın. İrəliləmədən əvvəl `sorafs_orchestrator_*` ölçülərinin səhnələşdirmədə doldurulduğundan əmin olun.

## 2. Fövqəladə Hallar Provayderinin Qara Siyahısına salınması

Provayder pozulmuş parçalara xidmət göstərdikdə, davamlı olaraq vaxt aşımına uğradıqda və ya uyğunluq yoxlanışında uğursuz olduqda bu prosedura əməl edin.

1. **Dəlilləri ələ keçirin.**
   - Ən son gətirmə xülasəsini ixrac edin (`--json-out` çıxışı). Uğursuz yığın indekslərini, provayder ləqəblərini və həzm uyğunsuzluqlarını qeyd edin.
   - `telemetry::sorafs.fetch.*` hədəflərindən müvafiq jurnal çıxarışlarını qeyd edin.
2. **Dərhal ləğvetmə tətbiq edin.**
   - Orkestratora paylanmış telemetriya snapşotunda cəzalandırılmış provayderi qeyd edin (`penalty=true` və ya `token_health` sıxacını `0`-ə təyin edin). Növbəti hesab lövhəsi provayderi avtomatik olaraq istisna edəcək.
   - Ad-hoc tüstü testləri üçün `--deny-provider gw-alpha`-i `sorafs_cli fetch`-ə ötürün ki, uğursuzluq yolu telemetriyanın yayılmasını gözləmədən həyata keçirilsin.
   - Təsirə məruz qalan mühitə yenilənmiş telemetriya/konfiqurasiya paketini yenidən yerləşdirin (səhnələmə → kanarya → istehsal). Dəyişikliyi hadisə jurnalında sənədləşdirin.
3. **Qeydiyyatı təsdiq edin.**
   - Kanonik fiksatorun alınmasını yenidən işə salın. Hesab lövhəsinin `policy_denied` səbəbi ilə provayderi uyğun olmayan kimi qeyd etdiyini təsdiqləyin.
   - Sayğacın rədd edilmiş provayder üçün artımını dayandırdığından əmin olmaq üçün `sorafs_orchestrator_provider_failures_total`-i yoxlayın.
4. **Uzunmüddətli qadağaları gücləndirin.**
   - Əgər provayder 24 saatdan çox bloklanmış qalsa, onun reklamını dəyişdirmək və ya dayandırmaq üçün idarəetmə biletini qaldırın. Səsvermə başa çatana qədər inkar siyahısını yerində saxlayın və telemetriya anlıq görüntülərini təzələyin ki, provayder yenidən hesab tablosuna daxil olmasın.
5. **Geri qaytarma protokolu.**
   - Provayderi bərpa etmək üçün onu rədd edilənlər siyahısından çıxarın, yenidən yerləşdirin və yeni hesab tablosunun şəklini çəkin. Dəyişikliyi ölümdən sonra hadisəyə əlavə edin.

## 3. Mərhələli Yayım Planı

| Faza | Əhatə dairəsi | Tələb olunan siqnallar | Get/Getmə Meyarları |
|-------|-------|------------------|-------------------|
| **Laboratoriya** | Xüsusi inteqrasiya klasteri | Qurğu yüklərinə qarşı əl ilə CLI gətirmə | Bütün parçalar uğur qazanır, provayderin uğursuzluq sayğacları 0-da qalır, təkrar cəhd nisbəti < 5%. |
| **Səhnələşdirmə** | Tam idarəetmə təyyarəsi quruluşu | Grafana idarə paneli qoşuldu; yalnız xəbərdarlıq rejimində xəbərdarlıq qaydaları | `sorafs_orchestrator_active_fetches` hər sınaqdan sonra sıfıra qayıdır; `warn/critical` xəbərdarlıq atəşi yoxdur. |
| **Kanarya** | İstehsal trafikinin ≤10%-i | Peyjer səssizdir, lakin telemetriya real vaxt rejimində izlənilir | Yenidən cəhd əmsalı < 10%, provayder uğursuzluqları məlum səs-küylü həmyaşıdları ilə təcrid olunub, gecikmə histoqramı ilkin səviyyəyə ±20% uyğun gəlir. |
| **Ümumi Mövcudluq** | 100% yayılma | Peycer qaydaları aktiv | 24 saat ərzində sıfır `NoHealthyProviders` xətaları, təkrar cəhd nisbəti sabit, idarə panelinin SLA panelləri yaşıl. |

Hər bir mərhələ üçün:

1. Orkestr JSON-u nəzərdə tutulan `max_providers` ilə yeniləyin və büdcələri yenidən sınayın.
2. `sorafs_cli fetch` və ya SDK inteqrasiya test dəstini kanonik qurğuya və ətraf mühitin təmsilçi manifestinə qarşı işə salın.
3. Hesab lövhəsi + xülasə artefaktları çəkin və onları buraxılış qeydinə əlavə edin.
4. Növbəti mərhələyə keçməzdən əvvəl çağırış üzrə mühəndislə telemetriya tablosunu nəzərdən keçirin.

## 4. Müşahidə oluna bilənlik və insident qarmaqları

- **Metriklər:** Alertmanager monitorlarının `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` və `sorafs_orchestrator_retries_total` olduğundan əmin olun. Ani bir sıçrayış adətən provayderin yük altında pisləşdiyini bildirir.
- ** Qeydlər:** `telemetry::sorafs.fetch.*` hədəflərini paylaşılan jurnal toplayıcısına yönləndirin. Triajı sürətləndirmək üçün `event=complete status=failed` üçün saxlanmış axtarışlar yaradın.
- **Scoreboards:** Uzunmüddətli saxlama üçün hər bir tablo artefaktını davam etdirin. JSON uyğunluq araşdırmaları və mərhələli geri dönmələr üçün sübut izi kimi ikiqat olur.
- **İdarəetmə panelləri:** Kanonik Grafana lövhəsini (`docs/examples/sorafs_fetch_dashboard.json`) `docs/examples/sorafs_fetch_alerts.yaml` siqnal qaydaları ilə istehsal qovluğuna klonlayın.

## 5. Əlaqə və Sənədləşdirmə

- Vaxt möhürü, operator, səbəb və əlaqəli hadisə ilə əməliyyatların dəyişdirilməsi jurnalında hər bir inkar/yüksək dəyişikliyi qeyd edin.
- Müştəri tərəfinin gözləntilərini uyğunlaşdırmaq üçün provayder çəkiləri və ya yenidən cəhd büdcələri dəyişdikdə SDK komandalarını xəbərdar edin.
- GA tamamlandıqdan sonra, `status.md`-i təqdimat xülasəsi ilə yeniləyin və buraxılış qeydlərində bu runbook arayışını arxivləşdirin.