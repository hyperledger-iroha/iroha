---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 31b279a990f47774972731f7f6b5181ed3b4625a7cd9d3e015b24c180e129c7b
source_last_modified: "2026-01-22T14:35:36.755283+00:00"
translation_last_reviewed: 2026-02-07
id: operations-playbook
title: SoraFS Operations Playbook
sidebar_label: Operations Playbook
description: Incident response guides and chaos drill procedures for SoraFS operators.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/sorafs_ops_playbook.md` altında saxlanılan runbook-u əks etdirir. Sfenks sənədlər dəsti tam köçənə qədər hər iki nüsxəni sinxron saxlayın.
:::

## Əsas İstinadlar

- Müşahidə oluna bilən aktivlər: `dashboards/alerts/`-də `dashboards/grafana/` və Prometheus xəbərdarlıq qaydaları altında Grafana idarə panellərinə baxın.
- Metrik kataloq: `docs/source/sorafs_observability_plan.md`.
- Orkestrator telemetriya səthləri: `docs/source/sorafs_orchestrator_plan.md`.

## Eskalasiya Matrisi

| Prioritet | Tətik nümunələri | İlkin çağırış | Yedək | Qeydlər |
|----------|------------------|-----------------|--------|-------|
| P1 | Qlobal şlüz kəsilməsi, PoR uğursuzluq dərəcəsi > 5% (15 dəq), replikasiya gecikməsi hər 10 dəqiqədən bir iki dəfə artır | Saxlama SRE | Müşahidə qabiliyyəti TL | Təsir 30 dəqiqədən çox olarsa, idarəetmə şurasını cəlb edin. |
| P2 | Regional şlüz gecikməsi SLO pozuntusu, SLA təsiri olmadan orkestratorun təkrar cəhdi | Müşahidə qabiliyyəti TL | Saxlama SRE | Yayımlamağa davam edin, lakin yeni manifestləri açın. |
| P3 | Kritik olmayan xəbərdarlıqlar (açıq-aşkar köhnəlmə, tutum 80-90%) | Qəbul triajı | Əməliyyat gildiyası | Növbəti iş günü ərzində ünvan. |

## Gateway Kəsilməsi / Zəif Əlçatımlılıq

** Aşkarlama**

- Xəbərdarlıqlar: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- İdarə paneli: `dashboards/grafana/sorafs_gateway_overview.json`.

**Tərhal tədbirlər**

1. Sorğu dərəcəsi paneli vasitəsilə əhatə dairəsini (tək provayder və donanma) təsdiq edin.
2. Əməliyyat konfiqurasiyasında (`docs/source/sorafs_gateway_self_cert.md`) `sorafs_gateway_route_weights`-i dəyişməklə Torii marşrutunu sağlam provayderlərə (əgər çox provayderdirsə) keçin.
3. Əgər bütün provayderlər təsir etmişsə, CLI/SDK müştəriləri üçün “birbaşa əldəetmə” geri qaytarılmasını aktivləşdirin (`docs/source/sorafs_node_client_protocol.md`).

**Üçləmə**

- `sorafs_gateway_stream_token_limit`-ə qarşı axın tokeninin istifadəsini yoxlayın.
- Şlüz qeydlərini TLS və ya qəbul xətaları üçün yoxlayın.
- İxrac edilmiş şlüz sxeminin gözlənilən versiyaya uyğun olmasını təmin etmək üçün `scripts/telemetry/run_schema_diff.sh`-i işə salın.

**İlahi seçimlər**

- Yalnız təsirlənmiş şlüz prosesini yenidən başladın; birdən çox provayder uğursuz olmadıqca, bütün klasteri təkrar emal etməkdən çəkinin.
- Doyma təsdiqlənərsə, axın işarəsi limitini müvəqqəti olaraq 10-15% artırın.
- Stabilləşdirmədən sonra özünü təsdiqləməni (`scripts/sorafs_gateway_self_cert.sh`) yenidən işə salın.

**Hadisədən sonrakı**

- `docs/source/sorafs/postmortem_template.md` istifadə edərək P1 postmortem faylı.
- Təmir əl müdaxilələrinə əsaslanırsa, təqib xaos təlimini planlaşdırın.

## Sübutun Uğursuzluğu Süni (PoR / PoTR)

** Aşkarlama**

- Xəbərdarlıqlar: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- İdarə paneli: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetriya: `provider_reason=corrupt_proof` ilə `torii_sorafs_proof_stream_events_total` və `sorafs.fetch.error` hadisələri.

**Təcili tədbirlər**

1. Manifest reyestrini işarələməklə yeni manifest qəbullarını dondurun (`docs/source/sorafs/manifest_pipeline.md`).
2. Təsirə məruz qalan provayderlər üçün təşviqləri dayandırmaq üçün İdarəetməni xəbərdar edin.

**Üçləmə**

- `sorafs_node_replication_backlog_total` ilə müqayisədə PoR problem növbəsinin dərinliyini yoxlayın.
- Son yerləşdirmələr üçün sübut yoxlama boru kəmərini (`crates/sorafs_node/src/potr.rs`) təsdiq edin.
- Provayderin proqram təminatı versiyalarını operator reyestri ilə müqayisə edin.

**İlahi seçimlər**

- Ən son manifestlə `sorafs_cli proof stream` istifadə edərək PoR təkrarlarını işə salın.
- Əgər sübutlar ardıcıl olaraq uğursuz olarsa, idarəetmə reyestrini yeniləyərək və orkestrator tablolarını yeniləməyə məcbur etməklə provayderi aktiv dəstdən çıxarın.

**Hadisədən sonrakı**

- Növbəti istehsal yerləşdirmədən əvvəl PoR xaos qazma ssenarisini yerinə yetirin.
- Postmortem şablonunda dərsləri çəkin və provayderin ixtisas yoxlama siyahısını yeniləyin.

## Replikasiya Gecikməsi / Geriləmə artımı

** Aşkarlama**

- Xəbərdarlıqlar: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. İdxal
  `dashboards/alerts/sorafs_capacity_rules.yml` və işə salın
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  təşviq etməzdən əvvəl beləliklə, Alertmanager sənədləşdirilmiş hədləri əks etdirir.
- İdarə paneli: `dashboards/grafana/sorafs_capacity_health.json`.
- Metriklər: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Təcili tədbirlər**

1. Arxa planın əhatə dairəsini (tək provayder və ya donanma) yoxlayın və qeyri-vacib replikasiya tapşırıqlarını dayandırın.
2. Əgər gecikmə təcrid olunubsa, replikasiya planlaşdırıcısı vasitəsilə yeni sifarişləri müvəqqəti olaraq alternativ provayderlərə təyin edin.

**Üçləmə**

- Orkestrator telemetriyasını təkrar cəhd partlamaları üçün yoxlayın.
- Yaddaş hədəflərinin kifayət qədər boş yerə malik olduğunu təsdiqləyin (`sorafs_node_capacity_utilisation_percent`).
- Son konfiqurasiya dəyişikliklərini nəzərdən keçirin (yığın profil yeniləmələri, sübut kadansı).

**İlahi seçimlər**

- Məzmunu yenidən paylamaq üçün `--rebalance` seçimi ilə `sorafs_cli`-i işə salın.
- Təsirə məruz qalan provayder üçün replikasiya işçilərini üfüqi olaraq miqyaslayın.
- TTL pəncərələrini yenidən hizalamaq üçün manifest yeniləməsini işə salın.

**Hadisədən sonrakı**

- Provayderin doyma çatışmazlığına diqqət yetirməklə tutumlu qazma planlaşdırın.
- `docs/source/sorafs_node_client_protocol.md`-də replikasiya SLA sənədlərini yeniləyin.

## Təmir İş Qrupu və SLA pozuntuları

** Aşkarlama**

- Xəbərdarlıqlar:
  - `SoraFSRepairBacklogHigh` (növbənin dərinliyi > 50 və ya ən köhnə növbə yaşı > 10 metr üçün 4 saat).
  - `SoraFSRepairEscalations` (> 3 eskalasiya/saat).
  - `SoraFSRepairLeaseExpirySpike` (> 5 icarə müddəti/saat).
  - `SoraFSRetentionBlockedEvictions` (son 15 m-də aktiv təmir nəticəsində saxlama bloklanıb).
- İdarə paneli: `dashboards/grafana/sorafs_capacity_health.json`.

**Təcili tədbirlər**

1. Təsirə məruz qalan provayderləri müəyyən edin (növbənin dərinliyi sıçrayışları) və onlar üçün yeni sancaqlar/replikasiya sifarişlərini dayandırın.
2. Təmir işçisinin canlılığını yoxlayın və əgər təhlükəsizdirsə, işçinin paralelliyini artırın.

**Üçləmə**

- `torii_sorafs_repair_backlog_oldest_age_seconds`-i 4 saatlıq SLA pəncərəsi ilə müqayisə edin.
- `torii_sorafs_repair_lease_expired_total{outcome=...}`-ni qəza/saat əyilmə nümunələri üçün yoxlayın.
- Təkrar manifest/provayder cütləri üçün yüksəldilmiş biletləri nəzərdən keçirin və sübut paketlərini yoxlayın.

**İlahi seçimlər**

- Dayanmış təmir işçilərini yenidən təyin etmək və ya yenidən işə başlamaq; normal iddia axını vasitəsilə yetim icarələri təmizləyin.
- Əlavə SLA təzyiqinin qarşısını almaq üçün təmir işləri apararkən yeni sancaqları tənzimləyin.
- Əgər eskalasiyalar davam edərsə, idarəetməyə keçin və təmir auditinin artefaktlarını əlavə edin.

## Saxlama / GC Təftişi (yalnız oxumaq üçün)

** Aşkarlama**

- Xəbərdarlıqlar: `SoraFSCapacityPressure` və ya davamlı `torii_sorafs_storage_bytes_used` > 90%.
- İdarə paneli: `dashboards/grafana/sorafs_capacity_health.json`.

**Təcili tədbirlər**

1. Yerli saxlama snapşotunu işə salın:
   ```bash
   iroha app sorafs gc inspect --data-dir /var/lib/sorafs
   ```
2. Triaj üçün yalnız vaxtı keçmiş görüntünü çəkin:
   ```bash
   iroha app sorafs gc dry-run --data-dir /var/lib/sorafs
   ```
3. Audit üçün JSON çıxışlarını hadisə biletinə əlavə edin.

**Üçləmə**

- Son tarixləri olanlara qarşı `retention_epoch=0` (müddəti bitməmiş) hesabatı təsdiq edin.
- Hansı məhdudiyyətin effektiv olduğunu görmək üçün GC JSON çıxışında `retention_sources` istifadə edin
  saxlama (`deal_end`, `governance_cap`, `pin_policy` və ya `unbounded`). Sövdələşmə və idarəetmə hədləri
  `sorafs.retention.deal_end_epoch` manifest metadata açarları vasitəsilə təmin edilir və
  `sorafs.retention.governance_cap_epoch`.
- Əgər `dry-run` hesabatlarının müddəti bitmiş manifestlər varsa, lakin tutum bağlı qalırsa, yoxlayın
  aktiv təmir və ya saxlama siyasəti blokun çıxarılmasını ləğv edir.
  Tutumla tetiklenen süpürgələr, müddəti bitmiş manifestləri ən az istifadə edilən sifarişlə çıxarır.
  `manifest_id` qalstuk qıranları.

**İlahi seçimlər**

- GC CLI yalnız oxumaq üçündür. İstehsalda manifestləri və ya parçaları əl ilə silməyin.
- Saxlama siyasətinə düzəlişlər və ya potensialın genişləndirilməsi üçün idarəetməyə yüksəldin
  vaxtı keçmiş məlumatlar avtomatik çıxarılmadan yığıldıqda.

## Xaos Drill Cadence

- **Rüblük**: Kombinə edilmiş şlüz kəsilməsi + orkestratorun təkrar fırtına simulyasiyası.
- **İkiillik**: bərpa ilə iki provayder arasında PoR/PoTR uğursuzluğu inyeksiyası.
- **Aylıq spot yoxlanış**: Səhnələşdirmə manifestlərindən istifadə edərək təkrarlama gecikməsi ssenarisi.
- Paylaşılan runbook jurnalında (`ops/drill-log.md`) məşqləri izləyin:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- Təhlil etməzdən əvvəl jurnalı təsdiqləyin:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Qarşıdan gələn məşqlər üçün `--status scheduled`, tamamlanmış qaçışlar üçün `pass`/`fail` və fəaliyyət elementləri açıq qaldıqda `follow-up` istifadə edin.
- Quru qaçışlar və ya avtomatlaşdırılmış yoxlama üçün `--log` ilə təyinatı ləğv edin; onsuz skript `ops/drill-log.md` yeniləməsini davam etdirir.

## Ölümdən sonrakı Şablon

Hər P1/P2 hadisəsi və xaos qazma retrospektivləri üçün `docs/source/sorafs/postmortem_template.md` istifadə edin. Şablon zaman cədvəlini, təsirin kəmiyyətini, töhfə verən amilləri, düzəldici tədbirləri və sonrakı yoxlama tapşırıqlarını əhatə edir.