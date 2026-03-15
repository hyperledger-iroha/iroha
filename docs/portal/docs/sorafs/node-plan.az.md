---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3852a0f039b664344f9cbce7d2514172cfe97cd838b68755f764d4fe183b22cc
source_last_modified: "2026-01-05T09:28:11.898207+00:00"
translation_last_reviewed: 2026-02-07
id: node-plan
title: SoraFS Node Implementation Plan
sidebar_label: Node Implementation Plan
description: Translate the SF-3 storage roadmap into actionable engineering work with milestones, tasks, and test coverage.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

SF-3, Iroha/Torii prosesini SoraFS yaddaş provayderinə çevirən ilk işlək `sorafs-node` qutusunu təqdim edir. Çatdırılmaları ardıcıllaşdırarkən bu planı [node storage guide](node-storage.md), [provayderin qəbul siyasəti](provider-admission-policy.md) və [saxlama tutumu bazarının yol xəritəsi](storage-capacity-marketplace.md) ilə birlikdə istifadə edin.

## Hədəf Sahəsi (Milestone M1)

1. **Yüksək mağaza inteqrasiyası.** `sorafs_car::ChunkStore`-ni konfiqurasiya edilmiş məlumat kataloqunda yığın baytları, manifestlər və PoR ağaclarını saxlayan davamlı arxa hissə ilə sarın.
2. **Gateway son nöqtələri.** Torii prosesi çərçivəsində pin təqdimi, yığın gətirmə, PoR seçmə və yaddaş telemetriyası üçün Norito HTTP son nöqtələrini ifşa edin.
3. **Konfiqurasiya santexnika.** `SoraFsStorage` konfiqurasiya strukturunu əlavə edin (aktiv bayraq, tutum, kataloqlar, paralellik limitləri) `iroha_config`, `iroha_core` və I18NI000003X.
4. **Kvota/planlaşdırma.** Operator tərəfindən müəyyən edilmiş disk/paralellik məhdudiyyətlərini və arxa təzyiqlə növbə sorğularını tətbiq edin.
5. **Telemetriya.** Uğurlu pin, yığın gətirmə gecikməsi, tutumdan istifadə və PoR seçmə nəticələri üçün ölçülər/loqlar buraxın.

## İş Dağılımı

### A. Sandıq və Modul Strukturu

| Tapşırıq | Sahib(lər) | Qeydlər |
|------|----------|-------|
| Modullarla `crates/sorafs_node` yaradın: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Saxlama Komandası | Torii inteqrasiyası üçün təkrar istifadə edilə bilən növlərin təkrar ixracı. |
| `SoraFsStorage`-dən xəritələnmiş `StorageConfig` tətbiq edin (istifadəçi → faktiki → defolt). | Storage Team / Config WG | Norito/`iroha_config` qatlarının deterministik qaldığından əmin olun. |
| Sancaqlar/gətirmələr təqdim etmək üçün Torii istifadə etdiyi `NodeHandle` fasadını təmin edin. | Saxlama Komandası | Saxlama daxili hissələrini və async santexnikasını əhatə edin. |

### B. Davamlı yığın mağazası

| Tapşırıq | Sahib(lər) | Qeydlər |
|------|----------|-------|
| Diskdəki manifest indeksi (`sled`/`sqlite`) ilə `sorafs_car::ChunkStore` diskin arxa ucunu yaradın. | Saxlama Komandası | Deterministik tərtibat: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| `ChunkStore::sample_leaves` istifadə edərək PoR metadatasını (64KiB/4KiB ağacları) qoruyun. | Saxlama Komandası | Yenidən başladıqdan sonra təkrarı dəstəkləyin; korrupsiyaya qarşı tez uğursuzluq. |
| Başlanğıcda bütövlük təkrarını həyata keçirin (rehash manifestləri, natamam sancaqları kəsin). | Saxlama Komandası | Təkrar oynatma tamamlanana qədər Torii blokunu işə salın. |

### C. Gateway Son Nöqtələri

| Son nöqtə | Davranış | Tapşırıqlar |
|----------|-----------|-------|
| `POST /sorafs/pin` | `PinProposalV1` qəbul edin, manifestləri təsdiqləyin, qəbulu növbəyə qoyun, manifest CID ilə cavab verin. | Yığma profilini təsdiqləyin, kvotaları tətbiq edin, yığın mağazası vasitəsilə məlumat yayımlayın. |
| `GET /sorafs/chunks/{cid}` + diapazon sorğusu | `Content-Chunker` başlıqları ilə yığın baytlara xidmət edin; diapazon qabiliyyəti spesifikasiyasına hörmət edin. | Planlayıcı + axın büdcələrindən istifadə edin (SF-2d diapazonu qabiliyyətinə bağlayın). |
| `POST /sorafs/por/sample` | Manifest və geriyə sübut paketi üçün PoR nümunəsini işə salın. | Dükan nümunəsini təkrar istifadə edin, Norito JSON yükləri ilə cavab verin. |
| `GET /sorafs/telemetry` | Xülasə: tutum, PoR müvəffəqiyyəti, əldə etmə xətası sayları. | Panellər/operatorlar üçün məlumat təmin edin. |

`sorafs_node::por` vasitəsilə iş vaxtı santexnika mövzuları PoR qarşılıqlı əlaqəsini təmin edir: izləyici hər bir `PorChallengeV1`, `PorProofV1` və `AuditVerdictV1`-ni qeyd edir, beləliklə `CapacityMeter` qiymətləndirmə göstəricilərini əks etdirmir. Torii məntiqi.【crates/sorafs_node/src/scheduler.rs#L147】

İcra qeydləri:

- `norito::json` faydalı yükləri ilə Torii-in Axum yığınından istifadə edin.
- Cavablar üçün Norito sxemləri əlavə edin (`PinResultV1`, `FetchErrorV1`, telemetriya strukturları).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` indi geriləmə dərinliyini və ən qədim dövrü/son tarixi və
  tərəfindən dəstəklənən hər bir provayder üçün ən son uğur/uğursuzluq zaman damğaları
  `sorafs_node::NodeHandle::por_ingestion_status` və Torii qeyd edir
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` üçün ölçü cihazları tablosuna.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:18 83】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Planlaşdırıcı və Kvota İcrası

| Tapşırıq | Təfərrüatlar |
|------|---------|
| Disk kvotası | Diskdəki baytları izləmək; `max_capacity_bytes`-dən çox olduqda yeni sancaqları rədd edin. Gələcək siyasətlər üçün boşaltma qarmaqlarını təmin edin. |
| Paralelliyi gətir | Qlobal semafor (`max_parallel_fetches`) üstəgəl SF-2d diapazonundan alınan hər provayder büdcəsi. |
| Pin növbəsi | Görkəmli qəbul işlərini məhdudlaşdırın; növbə dərinliyi üçün Norito status son nöqtələrini ifşa edin. |
| PoR kadans | `por_sample_interval_secs` tərəfindən idarə olunan fon işçisi. |

### E. Telemetriya və Logging

Metriklər (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (`result` etiketli histoqram)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Qeydlər / hadisələr:

- İdarəetmə qəbulu üçün strukturlaşdırılmış Norito telemetriyası (`StorageTelemetryV1`).
- İstifadə > 90% və ya PoR uğursuzluq zolağı həddi aşdıqda xəbərdarlıqlar.

### F. Sınaq Strategiyası

1. **Vahid testləri.** Yığın saxlama davamlılığı, kvota hesablamaları, planlaşdırıcı dəyişkənlikləri (bax: `crates/sorafs_node/src/scheduler.rs`).  
2. **İnteqrasiya testləri** (`crates/sorafs_node/tests`). Pin → gediş-dönüş gətirin, bərpanı yenidən başladın, kvotadan imtina, PoR seçmə sübutunun yoxlanılması.  
3. **Torii inteqrasiya testləri.** Yaddaş aktivləşdirilməklə Torii-i işə salın, `assert_cmd` vasitəsilə HTTP son nöqtələrini həyata keçirin.  
4. **Xaos yol xəritəsi.** Gələcək təlimlər diskin tükənməsini, yavaş IO-nu, provayderin çıxarılmasını simulyasiya edir.

## Asılılıqlar

- SF-2b qəbul siyasəti — reklamdan əvvəl qovşaqların qəbul zərflərini yoxlamasını təmin edin.  
- SF-2c tutum bazarı — telemetriyanı tutum bəyannamələrinə birləşdirin.  
- SF-2d reklam uzantıları — mövcud olduqda diapazon qabiliyyətini + axın büdcələrini istehlak edin.

## Mərhələ Çıxış Meyarları

- `cargo run -p sorafs_node --example pin_fetch` yerli qurğulara qarşı işləyir.  
- Torii `--features sorafs-storage` ilə qurur və inteqrasiya testlərindən keçir.  
- Sənədləşdirmə ([node storage guide](node-storage.md)) konfiqurasiya defoltları + CLI nümunələri ilə yeniləndi; operator runbook mövcuddur.  
- Səhnələşdirmə panellərində görünən telemetriya; tutumun doyması və PoR uğursuzluqları üçün konfiqurasiya edilmiş xəbərdarlıqlar.

## Sənədləşdirmə və Əməliyyat Təslim Edilənlər

- Konfiqurasiya defoltları, CLI istifadəsi və problemlərin aradan qaldırılması addımları ilə [node storage arayışını](node-storage.md) yeniləyin.  
- SF-3 inkişaf etdikcə [node əməliyyatları runbook](node-operations.md) tətbiqi ilə uyğunlaşdırılsın.  
- Tərtibatçı portalında `/sorafs/*` son nöqtələri üçün API arayışlarını dərc edin və Torii işləyiciləri yerləşdikdən sonra onları OpenAPI manifestinə köçürün.